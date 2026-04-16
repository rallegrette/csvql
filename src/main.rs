mod aggregator;
mod ast;
mod engine;
mod error;
mod lexer;
mod loader;
mod output;
mod parser;
mod types;

use std::time::Instant;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "csvql",
    about = "Run SQL queries against CSV files from the terminal",
    version,
    author
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a SQL query against CSV file(s)
    Query {
        /// The SQL query to execute
        sql: String,

        /// Write output as CSV to a file instead of a table
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Show the inferred schema of a CSV file
    Schema {
        /// Path to the CSV file
        file: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Query { sql, output } => run_query(&sql, output.as_deref()),
        Commands::Schema { file } => run_schema(&file),
    }
}

fn run_query(sql: &str, output_path: Option<&str>) {
    let start = Instant::now();

    let mut lex = lexer::Lexer::new(sql);
    let tokens = match lex.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let mut parser = parser::Parser::new(tokens);
    let stmt = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let result = match engine::execute(&stmt) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let elapsed = start.elapsed();
    let row_count = result.rows.len();

    if let Some(path) = output_path {
        let mut file = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error writing output: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = output::write_csv(&result, &mut file) {
            eprintln!("Error writing CSV: {e}");
            std::process::exit(1);
        }
        println!(
            "{row_count} rows written to {path} in {}ms",
            elapsed.as_millis()
        );
    } else {
        println!();
        println!("{}", output::render_table(&result));
        println!();
        let row_word = if row_count == 1 { "row" } else { "rows" };
        println!(
            "{row_count} {row_word} returned in {}ms",
            elapsed.as_millis()
        );
    }
}

fn run_schema(file: &str) {
    match loader::infer_schema(file) {
        Ok(schema) => {
            use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};

            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec![
                Cell::new("Column"),
                Cell::new("Type"),
                Cell::new("Non-Null"),
                Cell::new("Nulls"),
            ]);

            for (name, col_type, non_null, nulls) in &schema {
                table.add_row(vec![
                    Cell::new(name),
                    Cell::new(col_type),
                    Cell::new(non_null),
                    Cell::new(nulls),
                ]);
            }

            println!();
            println!("Schema for: {file}");
            println!("{table}");
            println!();
            println!(
                "{} columns, {} total rows",
                schema.len(),
                schema.first().map(|(_, _, nn, n)| nn + n).unwrap_or(0)
            );
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
