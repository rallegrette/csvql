mod aggregator;
mod ast;
mod engine;
mod error;
mod lexer;
mod loader;
mod output;
mod parser;
mod planner;
mod types;

use std::io::{self, IsTerminal, Write};
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;

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

        /// Write output to a file instead of stdout
        #[arg(short, long)]
        output: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Show the inferred schema of a CSV file
    Schema {
        /// Path to the CSV file
        file: String,
    },

    /// Show the execution plan for a query without running it
    Explain {
        /// The SQL query to explain
        sql: String,
    },

    /// Start an interactive SQL shell
    Repl,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Table,
    Csv,
    Json,
    Markdown,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Query { sql, output, format } => run_query(&sql, output.as_deref(), format),
        Commands::Schema { file } => run_schema(&file),
        Commands::Explain { sql } => run_explain(&sql),
        Commands::Repl => run_repl(),
    }
}

fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && io::stdout().is_terminal()
}

fn parse_and_execute(sql: &str) -> Result<types::Table, error::CsvqlError> {
    let mut lex = lexer::Lexer::new(sql);
    let tokens = lex.tokenize()?;
    let mut p = parser::Parser::new(tokens);
    let stmt = p.parse()?;
    engine::execute(&stmt)
}

fn run_query(sql: &str, output_path: Option<&str>, format: OutputFormat) {
    let start = Instant::now();

    let mut lex = lexer::Lexer::new(sql);
    let tokens = match lex.tokenize() {
        Ok(t) => t,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let mut p = parser::Parser::new(tokens);
    let stmt = match p.parse() {
        Ok(s) => s,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let result = match engine::execute(&stmt) {
        Ok(r) => r,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let elapsed = start.elapsed();
    let row_count = result.rows.len();

    if let Some(path) = output_path {
        let mut file = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{} {e}", "Error:".red().bold());
                std::process::exit(1);
            }
        };
        let write_result = match format {
            OutputFormat::Csv | OutputFormat::Table => output::write_csv(&result, &mut file),
            OutputFormat::Json => output::write_json(&result, &mut file),
            OutputFormat::Markdown => {
                write!(file, "{}", output::render_markdown(&result)).map_err(|e| e)
            }
        };
        if let Err(e) = write_result {
            eprintln!("{} {e}", "Error:".red().bold());
            std::process::exit(1);
        }
        let row_word = if row_count == 1 { "row" } else { "rows" };
        println!(
            "{row_count} {row_word} written to {path} in {}ms",
            elapsed.as_millis()
        );
    } else {
        print_result(&result, format, elapsed.as_millis());
    }
}

fn print_result(result: &types::Table, format: OutputFormat, elapsed_ms: u128) {
    let color = use_color();
    let row_count = result.rows.len();

    match format {
        OutputFormat::Table => {
            println!();
            if color {
                println!("{}", output::render_table_colored(result));
            } else {
                println!("{}", output::render_table(result));
            }
            println!();
            output::print_row_footer(row_count, elapsed_ms, color);
        }
        OutputFormat::Csv => {
            let mut stdout = io::stdout().lock();
            let _ = output::write_csv(result, &mut stdout);
        }
        OutputFormat::Json => {
            let mut stdout = io::stdout().lock();
            let _ = output::write_json(result, &mut stdout);
            println!();
        }
        OutputFormat::Markdown => {
            println!("{}", output::render_markdown(result));
        }
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
            if use_color() {
                println!("{} {}", "Schema for:".bold(), file.cyan());
            } else {
                println!("Schema for: {file}");
            }
            println!("{table}");
            println!();
            println!(
                "{} columns, {} total rows",
                schema.len(),
                schema.first().map(|(_, _, nn, n)| nn + n).unwrap_or(0)
            );
        }
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    }
}

fn run_explain(sql: &str) {
    let mut lex = lexer::Lexer::new(sql);
    let tokens = match lex.tokenize() {
        Ok(t) => t,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let mut p = parser::Parser::new(tokens);
    let stmt = match p.parse() {
        Ok(s) => s,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let plan = planner::build_plan(&stmt);
    println!();
    if use_color() {
        println!("{}", "QueryPlan".bold().green());
    } else {
        println!("QueryPlan");
    }
    print!("{plan}");
}

fn print_error(e: &error::CsvqlError) {
    if use_color() {
        eprintln!("{} {e}", "Error:".red().bold());
    } else {
        eprintln!("Error: {e}");
    }
}

// ── REPL ─────────────────────────────────────────────────────

fn run_repl() {
    use rustyline::error::ReadlineError;
    use rustyline::history::DefaultHistory;
    use rustyline::{Config, EditMode, Editor};

    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .auto_add_history(true)
        .build();

    let mut rl: Editor<ReplHelper, DefaultHistory> =
        Editor::with_config(config).expect("Failed to create editor");
    rl.set_helper(Some(ReplHelper));

    let history_path = dirs_history_path();
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    println!();
    if use_color() {
        println!(
            "{}  {}",
            "csvql".bold().cyan(),
            "interactive SQL shell".dimmed()
        );
        println!(
            "{}",
            "Type a SQL query, or .help for commands, .quit to exit."
                .dimmed()
        );
    } else {
        println!("csvql  interactive SQL shell");
        println!("Type a SQL query, or .help for commands, .quit to exit.");
    }
    println!();

    loop {
        let prompt = if use_color() {
            format!("{} ", "csvql>".green().bold())
        } else {
            "csvql> ".to_string()
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match trimmed {
                    ".quit" | ".exit" | ".q" => break,
                    ".help" | ".h" => {
                        print_repl_help();
                        continue;
                    }
                    cmd if cmd.starts_with(".schema ") => {
                        let file = cmd.strip_prefix(".schema ").unwrap().trim();
                        run_schema(file);
                        println!();
                        continue;
                    }
                    cmd if cmd.starts_with(".explain ") => {
                        let sql = cmd.strip_prefix(".explain ").unwrap().trim();
                        run_explain(sql);
                        println!();
                        continue;
                    }
                    _ => {}
                }

                let start = Instant::now();
                match parse_and_execute(trimmed) {
                    Ok(result) => {
                        print_result(&result, OutputFormat::Table, start.elapsed().as_millis());
                        println!();
                    }
                    Err(e) => {
                        print_error(&e);
                        println!();
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!("Goodbye.");
                break;
            }
            Err(e) => {
                eprintln!("Readline error: {e}");
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }
}

fn dirs_history_path() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .map(|home| format!("{home}/.csvql_history"))
}

fn print_repl_help() {
    let commands = [
        (".help", "Show this help message"),
        (".quit", "Exit the shell"),
        (".schema <file>", "Show schema for a CSV file"),
        (".explain <sql>", "Show query execution plan"),
    ];

    println!();
    if use_color() {
        println!("{}", "Commands:".bold());
        for (cmd, desc) in &commands {
            println!("  {}  {}", cmd.cyan(), desc.dimmed());
        }
        println!();
        println!(
            "{}",
            "Or type any SQL query to execute it.".dimmed()
        );
    } else {
        println!("Commands:");
        for (cmd, desc) in &commands {
            println!("  {cmd}  {desc}");
        }
        println!();
        println!("Or type any SQL query to execute it.");
    }
    println!();
}

struct ReplHelper;

impl rustyline::Helper for ReplHelper {}
impl rustyline::highlight::Highlighter for ReplHelper {}
impl rustyline::hint::Hinter for ReplHelper {
    type Hint = String;
}
impl rustyline::completion::Completer for ReplHelper {
    type Candidate = String;
}
impl rustyline::validate::Validator for ReplHelper {}
