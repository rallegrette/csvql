use colored::Colorize;
use comfy_table::{
    presets::UTF8_FULL, Attribute, Cell, CellAlignment, ContentArrangement, Table as ComfyTable,
};

use crate::types::{Table, Value};

pub fn render_table(table: &Table) -> String {
    let mut ct = ComfyTable::new();
    ct.load_preset(UTF8_FULL);
    ct.set_content_arrangement(ContentArrangement::Dynamic);

    let header_cells: Vec<Cell> = table
        .columns
        .iter()
        .map(|col| Cell::new(col).add_attribute(Attribute::Bold))
        .collect();
    ct.set_header(header_cells);

    for row in &table.rows {
        let cells: Vec<Cell> = row
            .values
            .iter()
            .map(|val| {
                let cell = Cell::new(format_value(val));
                match val {
                    Value::Integer(_) | Value::Float(_) => {
                        cell.set_alignment(CellAlignment::Right)
                    }
                    _ => cell,
                }
            })
            .collect();
        ct.add_row(cells);
    }

    ct.to_string()
}

pub fn render_table_colored(table: &Table) -> String {
    let mut ct = ComfyTable::new();
    ct.load_preset(UTF8_FULL);
    ct.set_content_arrangement(ContentArrangement::Dynamic);

    let header_cells: Vec<Cell> = table
        .columns
        .iter()
        .map(|col| Cell::new(col).add_attribute(Attribute::Bold))
        .collect();
    ct.set_header(header_cells);

    for row in &table.rows {
        let cells: Vec<Cell> = row
            .values
            .iter()
            .map(|val| {
                let display = format_value_colored(val);
                let cell = Cell::new(display);
                match val {
                    Value::Integer(_) | Value::Float(_) => {
                        cell.set_alignment(CellAlignment::Right)
                    }
                    _ => cell,
                }
            })
            .collect();
        ct.add_row(cells);
    }

    ct.to_string()
}

fn format_value(val: &Value) -> String {
    match val {
        Value::Null => "NULL".to_string(),
        Value::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Integer(n) => format_integer(*n),
        Value::Float(n) => format_float(*n),
        Value::String(s) => s.clone(),
    }
}

fn format_value_colored(val: &Value) -> String {
    match val {
        Value::Null => "NULL".dimmed().to_string(),
        Value::Boolean(b) => {
            let s = if *b { "true" } else { "false" };
            s.yellow().to_string()
        }
        Value::Integer(n) => format_integer(*n).cyan().to_string(),
        Value::Float(n) => format_float(*n).cyan().to_string(),
        Value::String(s) => s.clone(),
    }
}

pub fn format_integer(n: i64) -> String {
    if n < 0 {
        return format!("-{}", format_integer(-n));
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn format_float(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format_integer(n as i64)
    } else {
        let rounded = (n * 100.0).round() / 100.0;
        let s = format!("{:.2}", rounded);
        if let Some(dot_pos) = s.find('.') {
            let int_part = &s[..dot_pos];
            let frac_part = &s[dot_pos..];
            let formatted_int = if let Ok(i) = int_part.parse::<i64>() {
                format_integer(i)
            } else {
                int_part.to_string()
            };
            format!("{formatted_int}{frac_part}")
        } else {
            s
        }
    }
}

pub fn write_csv<W: std::io::Write>(table: &Table, writer: &mut W) -> std::io::Result<()> {
    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record(&table.columns)?;
    for row in &table.rows {
        let fields: Vec<String> = row.values.iter().map(|v| format!("{v}")).collect();
        wtr.write_record(&fields)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn write_json<W: std::io::Write>(table: &Table, writer: &mut W) -> std::io::Result<()> {
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = table
        .rows
        .iter()
        .map(|row| {
            table
                .columns
                .iter()
                .zip(row.values.iter())
                .map(|(col, val)| (col.clone(), value_to_json(val)))
                .collect()
        })
        .collect();

    let json = serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".to_string());
    write!(writer, "{json}")?;
    Ok(())
}

fn value_to_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Integer(n) => serde_json::json!(n),
        Value::Float(n) => serde_json::json!(n),
        Value::String(s) => serde_json::Value::String(s.clone()),
    }
}

pub fn render_markdown(table: &Table) -> String {
    if table.columns.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();

    let header = table
        .columns
        .iter()
        .map(|c| format!(" {c} "))
        .collect::<Vec<_>>()
        .join("|");
    lines.push(format!("|{header}|"));

    let separator = table
        .columns
        .iter()
        .map(|_| "---".to_string())
        .collect::<Vec<_>>()
        .join("|");
    lines.push(format!("|{separator}|"));

    for row in &table.rows {
        let cols = row
            .values
            .iter()
            .map(|v| format!(" {} ", format_value(v)))
            .collect::<Vec<_>>()
            .join("|");
        lines.push(format!("|{cols}|"));
    }

    lines.join("\n")
}

pub fn print_row_footer(row_count: usize, elapsed_ms: u128, use_color: bool) {
    let row_word = if row_count == 1 { "row" } else { "rows" };
    if use_color {
        let count_str = format!("{row_count}").bold();
        let time_str = format!("{elapsed_ms}ms").dimmed();
        println!("{count_str} {row_word} returned in {time_str}");
    } else {
        println!("{row_count} {row_word} returned in {elapsed_ms}ms");
    }
}
