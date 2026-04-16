use comfy_table::{presets::UTF8_FULL, Attribute, Cell, CellAlignment, ContentArrangement, Table as ComfyTable};

use crate::types::{Table, Value};

/// Render a result Table as a pretty Unicode box-drawing table.
pub fn render_table(table: &Table) -> String {
    let mut ct = ComfyTable::new();
    ct.load_preset(UTF8_FULL);
    ct.set_content_arrangement(ContentArrangement::Dynamic);

    let header_cells: Vec<Cell> = table
        .columns
        .iter()
        .map(|col| {
            Cell::new(col)
                .add_attribute(Attribute::Bold)
        })
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

fn format_value(val: &Value) -> String {
    match val {
        Value::Null => "NULL".to_string(),
        Value::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Integer(n) => format_integer(*n),
        Value::Float(n) => format_float(*n),
        Value::String(s) => s.clone(),
    }
}

fn format_integer(n: i64) -> String {
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
        // Add thousand separators to the integer part
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

/// Write result table as CSV to the given writer.
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
