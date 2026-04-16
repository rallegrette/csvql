use std::path::Path;

use crate::error::{CsvqlError, Result};
use crate::types::{ColumnType, Row, Table, Value};

/// Load a CSV file into a Table, performing schema inference and type coercion.
pub fn load_csv(path: &str) -> Result<Table> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(CsvqlError::FileNotFound(path.to_string()));
    }

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(path_obj)?;

    let headers: Vec<String> = reader
        .headers()?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let raw_rows: Vec<Vec<String>> = reader
        .records()
        .filter_map(|r| r.ok())
        .map(|record| record.iter().map(|field| field.to_string()).collect())
        .collect();

    let col_types = infer_types(&headers, &raw_rows);

    let rows: Vec<Row> = raw_rows
        .iter()
        .map(|raw| {
            let values: Vec<Value> = raw
                .iter()
                .enumerate()
                .map(|(i, field)| coerce_value(field, col_types.get(i).copied()))
                .collect();
            Row { values }
        })
        .collect();

    let name = path_obj
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    Ok(Table {
        name,
        columns: headers,
        rows,
    })
}

/// Infer column types by sampling all rows.
fn infer_types(headers: &[String], rows: &[Vec<String>]) -> Vec<ColumnType> {
    let mut types = vec![ColumnType::Integer; headers.len()];

    for row in rows {
        for (i, field) in row.iter().enumerate() {
            if i >= types.len() {
                continue;
            }
            let trimmed = field.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") || trimmed == "NA" {
                continue; // nulls don't affect type inference
            }

            let field_type = if trimmed.eq_ignore_ascii_case("true")
                || trimmed.eq_ignore_ascii_case("false")
            {
                ColumnType::Boolean
            } else if trimmed.parse::<i64>().is_ok() {
                ColumnType::Integer
            } else if trimmed.parse::<f64>().is_ok() {
                ColumnType::Float
            } else {
                ColumnType::String
            };

            types[i] = widen_type(types[i], field_type);
        }
    }

    types
}

/// Widen types: Integer < Float < String, Boolean stands alone
fn widen_type(current: ColumnType, new: ColumnType) -> ColumnType {
    use ColumnType::*;
    match (current, new) {
        (a, b) if a == b => a,
        (Integer, Float) | (Float, Integer) => Float,
        (Boolean, Boolean) => Boolean,
        _ => String,
    }
}

fn coerce_value(field: &str, col_type: Option<ColumnType>) -> Value {
    let trimmed = field.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") || trimmed == "NA" {
        return Value::Null;
    }

    match col_type {
        Some(ColumnType::Integer) => trimmed
            .parse::<i64>()
            .map(Value::Integer)
            .unwrap_or_else(|_| Value::String(trimmed.to_string())),
        Some(ColumnType::Float) => trimmed
            .parse::<f64>()
            .map(Value::Float)
            .unwrap_or_else(|_| Value::String(trimmed.to_string())),
        Some(ColumnType::Boolean) => {
            if trimmed.eq_ignore_ascii_case("true") {
                Value::Boolean(true)
            } else if trimmed.eq_ignore_ascii_case("false") {
                Value::Boolean(false)
            } else {
                Value::String(trimmed.to_string())
            }
        }
        Some(ColumnType::String) | None => Value::String(trimmed.to_string()),
    }
}

/// Report the inferred schema for a CSV file.
pub fn infer_schema(path: &str) -> Result<Vec<(String, ColumnType, usize, usize)>> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(CsvqlError::FileNotFound(path.to_string()));
    }

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(path_obj)?;

    let headers: Vec<String> = reader
        .headers()?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let raw_rows: Vec<Vec<String>> = reader
        .records()
        .filter_map(|r| r.ok())
        .map(|record| record.iter().map(|field| field.to_string()).collect())
        .collect();

    let col_types = infer_types(&headers, &raw_rows);
    let total_rows = raw_rows.len();

    let null_counts: Vec<usize> = (0..headers.len())
        .map(|i| {
            raw_rows
                .iter()
                .filter(|row| {
                    row.get(i)
                        .map(|f| {
                            let t = f.trim();
                            t.is_empty() || t.eq_ignore_ascii_case("null") || t == "NA"
                        })
                        .unwrap_or(true)
                })
                .count()
        })
        .collect();

    Ok(headers
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            (
                name,
                col_types[i],
                total_rows - null_counts[i],
                null_counts[i],
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widen_types() {
        assert_eq!(
            widen_type(ColumnType::Integer, ColumnType::Float),
            ColumnType::Float
        );
        assert_eq!(
            widen_type(ColumnType::Integer, ColumnType::String),
            ColumnType::String
        );
        assert_eq!(
            widen_type(ColumnType::Integer, ColumnType::Integer),
            ColumnType::Integer
        );
    }

    #[test]
    fn test_coerce_null() {
        assert!(matches!(
            coerce_value("", Some(ColumnType::Integer)),
            Value::Null
        ));
        assert!(matches!(
            coerce_value("NULL", Some(ColumnType::String)),
            Value::Null
        ));
        assert!(matches!(
            coerce_value("NA", Some(ColumnType::Float)),
            Value::Null
        ));
    }
}
