use std::collections::HashMap;

use crate::aggregator::create_aggregator;
use crate::ast::*;
use crate::error::{CsvqlError, Result};
use crate::loader::load_csv;
use crate::types::{Row, Table, Value};

/// Execute a parsed SELECT statement and return the resulting table.
pub fn execute(stmt: &SelectStatement) -> Result<Table> {
    let mut base_table = load_csv(&stmt.from.table)?;

    if let Some(alias) = &stmt.from.alias {
        base_table.name = alias.clone();
    }

    // Perform JOINs
    for join in &stmt.joins {
        let mut right = load_csv(&join.table)?;
        if let Some(alias) = &join.alias {
            right.name = alias.clone();
        }
        base_table = execute_join(&base_table, &right, join)?;
    }

    // WHERE filter
    let rows = if let Some(ref where_expr) = stmt.where_clause {
        base_table
            .rows
            .into_iter()
            .filter(|row| {
                eval_expr(where_expr, row, &base_table.columns)
                    .map(|v| v.is_truthy())
                    .unwrap_or(false)
            })
            .collect()
    } else {
        base_table.rows
    };

    // Determine if this is an aggregate query
    let has_aggregates = stmt
        .columns
        .iter()
        .any(|c| matches!(c, SelectColumn::Expr { expr, .. } if contains_aggregate(expr)));
    let has_group_by = !stmt.group_by.is_empty();

    let (result_columns, result_rows) = if has_aggregates || has_group_by {
        execute_aggregate(&stmt.columns, &rows, &base_table.columns, &stmt.group_by, &stmt.having)?
    } else {
        execute_simple(&stmt.columns, &rows, &base_table.columns)?
    };

    // DISTINCT
    let result_rows = if stmt.distinct {
        let mut seen = Vec::new();
        result_rows
            .into_iter()
            .filter(|row| {
                let key: Vec<String> = row.values.iter().map(|v| format!("{v}")).collect();
                if seen.contains(&key) {
                    false
                } else {
                    seen.push(key);
                    true
                }
            })
            .collect()
    } else {
        result_rows
    };

    // ORDER BY
    let mut result_rows = result_rows;
    if !stmt.order_by.is_empty() {
        sort_rows(&mut result_rows, &stmt.order_by, &result_columns)?;
    }

    // OFFSET
    let result_rows = if let Some(offset) = stmt.offset {
        result_rows.into_iter().skip(offset).collect()
    } else {
        result_rows
    };

    // LIMIT
    let result_rows = if let Some(limit) = stmt.limit {
        result_rows.into_iter().take(limit).collect()
    } else {
        result_rows
    };

    Ok(Table {
        name: "result".to_string(),
        columns: result_columns,
        rows: result_rows,
    })
}

// ── Simple (non-aggregate) execution ─────────────────────────

fn execute_simple(
    select_cols: &[SelectColumn],
    rows: &[Row],
    columns: &[String],
) -> Result<(Vec<String>, Vec<Row>)> {
    let mut col_names = Vec::new();
    let mut col_exprs: Vec<Option<Expr>> = Vec::new();

    for sc in select_cols {
        match sc {
            SelectColumn::Wildcard => {
                for col in columns {
                    col_names.push(col.clone());
                    col_exprs.push(None);
                }
            }
            SelectColumn::Expr { expr, alias } => {
                let name = alias
                    .clone()
                    .unwrap_or_else(|| expr_display_name(expr));
                col_names.push(name);
                col_exprs.push(Some(expr.clone()));
            }
        }
    }

    let result_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            let values: Vec<Value> = col_exprs
                .iter()
                .enumerate()
                .map(|(i, expr_opt)| {
                    if let Some(expr) = expr_opt {
                        eval_expr(expr, row, columns).unwrap_or(Value::Null)
                    } else {
                        row.values.get(i).cloned().unwrap_or(Value::Null)
                    }
                })
                .collect();
            Row { values }
        })
        .collect();

    Ok((col_names, result_rows))
}

// ── Aggregate execution ──────────────────────────────────────

fn execute_aggregate(
    select_cols: &[SelectColumn],
    rows: &[Row],
    columns: &[String],
    group_by_exprs: &[Expr],
    having: &Option<Expr>,
) -> Result<(Vec<String>, Vec<Row>)> {
    // Build groups
    let groups = build_groups(rows, group_by_exprs, columns)?;

    let mut col_names = Vec::new();
    let mut col_defs: Vec<SelectColumn> = Vec::new();

    for sc in select_cols {
        match sc {
            SelectColumn::Wildcard => {
                return Err(CsvqlError::ParseError(
                    "Cannot use * with GROUP BY".into(),
                ))
            }
            SelectColumn::Expr { expr, alias } => {
                let name = alias
                    .clone()
                    .unwrap_or_else(|| expr_display_name(expr));
                col_names.push(name);
                col_defs.push(sc.clone());
            }
        }
    }

    let mut result_rows = Vec::new();

    for (_key, group_rows) in &groups {
        let values: Vec<Value> = col_defs
            .iter()
            .map(|col| match col {
                SelectColumn::Expr { expr, .. } => {
                    eval_aggregate_expr(expr, group_rows, columns).unwrap_or(Value::Null)
                }
                _ => Value::Null,
            })
            .collect();

        // HAVING filter
        if let Some(having_expr) = having {
            let having_val = eval_aggregate_expr(having_expr, group_rows, columns)?;
            if !having_val.is_truthy() {
                continue;
            }
        }

        result_rows.push(Row { values });
    }

    Ok((col_names, result_rows))
}

fn build_groups(
    rows: &[Row],
    group_by_exprs: &[Expr],
    columns: &[String],
) -> Result<Vec<(Vec<String>, Vec<Row>)>> {
    if group_by_exprs.is_empty() {
        // No GROUP BY: entire result is one group
        return Ok(vec![("__all__".into(), rows.to_vec())].into_iter().map(|(k, v)| (vec![k], v)).collect());
    }

    let mut map: HashMap<Vec<String>, Vec<Row>> = HashMap::new();
    let mut order: Vec<Vec<String>> = Vec::new();

    for row in rows {
        let key: Vec<String> = group_by_exprs
            .iter()
            .map(|expr| {
                eval_expr(expr, row, columns)
                    .map(|v| format!("{v}"))
                    .unwrap_or_default()
            })
            .collect();

        if !map.contains_key(&key) {
            order.push(key.clone());
        }
        map.entry(key).or_default().push(row.clone());
    }

    Ok(order.into_iter().map(|k| {
        let rows = map.remove(&k).unwrap();
        (k, rows)
    }).collect())
}

/// Evaluate an expression that may contain aggregate functions.
fn eval_aggregate_expr(expr: &Expr, group_rows: &[Row], columns: &[String]) -> Result<Value> {
    match expr {
        Expr::Function { name, args, .. } if is_aggregate_function(name) => {
            let is_star = args.first().map(|a| matches!(a, Expr::Star)).unwrap_or(false);
            let mut agg = create_aggregator(name, is_star).ok_or_else(|| {
                CsvqlError::AggregateError {
                    function: name.clone(),
                }
            })?;

            for row in group_rows {
                let val = if is_star {
                    Value::Integer(1)
                } else if let Some(arg) = args.first() {
                    eval_expr(arg, row, columns)?
                } else {
                    Value::Integer(1)
                };
                agg.accumulate(&val);
            }

            Ok(agg.finish())
        }
        Expr::BinaryOp { left, op, right } => {
            let l = eval_aggregate_expr(left, group_rows, columns)?;
            let r = eval_aggregate_expr(right, group_rows, columns)?;
            eval_binary_op(&l, op, &r)
        }
        Expr::Column { .. } => {
            // In aggregate context, a non-aggregated column returns the first row's value
            if let Some(first_row) = group_rows.first() {
                eval_expr(expr, first_row, columns)
            } else {
                Ok(Value::Null)
            }
        }
        other => {
            if let Some(first_row) = group_rows.first() {
                eval_expr(other, first_row, columns)
            } else {
                Ok(Value::Null)
            }
        }
    }
}

fn is_aggregate_function(name: &str) -> bool {
    matches!(
        name.to_uppercase().as_str(),
        "COUNT" | "SUM" | "AVG" | "MIN" | "MAX"
    )
}

fn contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Function { name, .. } => is_aggregate_function(name),
        Expr::BinaryOp { left, right, .. } => contains_aggregate(left) || contains_aggregate(right),
        Expr::UnaryOp { operand, .. } => contains_aggregate(operand),
        _ => false,
    }
}

// ── JOIN ─────────────────────────────────────────────────────

fn execute_join(left: &Table, right: &Table, join: &JoinClause) -> Result<Table> {
    let mut combined_columns: Vec<String> = left.columns.clone();
    for col in &right.columns {
        combined_columns.push(col.clone());
    }

    let mut result_rows = Vec::new();
    let left_len = left.columns.len();

    for left_row in &left.rows {
        let mut matched = false;

        for right_row in &right.rows {
            // Build combined row for ON evaluation
            let mut combined_values = left_row.values.clone();
            combined_values.extend(right_row.values.clone());
            let combined_row = Row {
                values: combined_values,
            };

            let on_result = eval_join_expr(&join.on, &combined_row, left, right, left_len)?;
            if on_result.is_truthy() {
                result_rows.push(combined_row);
                matched = true;
            }
        }

        // LEFT JOIN: include left row with NULLs for right side
        if !matched && join.join_type == JoinType::Left {
            let mut values = left_row.values.clone();
            values.extend(std::iter::repeat(Value::Null).take(right.columns.len()));
            result_rows.push(Row { values });
        }
    }

    Ok(Table {
        name: format!("{}_{}", left.name, right.name),
        columns: combined_columns,
        rows: result_rows,
    })
}

fn eval_join_expr(
    expr: &Expr,
    row: &Row,
    left: &Table,
    right: &Table,
    left_len: usize,
) -> Result<Value> {
    match expr {
        Expr::Column { table, name } => {
            if let Some(tbl) = table {
                if tbl.eq_ignore_ascii_case(&left.name) {
                    if let Some(idx) = left.column_index(name) {
                        return Ok(row.values.get(idx).cloned().unwrap_or(Value::Null));
                    }
                }
                if tbl.eq_ignore_ascii_case(&right.name) {
                    if let Some(idx) = right.column_index(name) {
                        return Ok(row
                            .values
                            .get(left_len + idx)
                            .cloned()
                            .unwrap_or(Value::Null));
                    }
                }
                Err(CsvqlError::ColumnNotFound(format!("{tbl}.{name}")))
            } else {
                // Try left first, then right
                if let Some(idx) = left.column_index(name) {
                    return Ok(row.values.get(idx).cloned().unwrap_or(Value::Null));
                }
                if let Some(idx) = right.column_index(name) {
                    return Ok(row
                        .values
                        .get(left_len + idx)
                        .cloned()
                        .unwrap_or(Value::Null));
                }
                Err(CsvqlError::ColumnNotFound(name.clone()))
            }
        }
        Expr::BinaryOp { left: l, op, right: r } => {
            let lv = eval_join_expr(l, row, left, right, left_len)?;
            let rv = eval_join_expr(r, row, left, right, left_len)?;
            eval_binary_op(&lv, op, &rv)
        }
        other => {
            let all_columns: Vec<String> = left
                .columns
                .iter()
                .chain(right.columns.iter())
                .cloned()
                .collect();
            eval_expr(other, row, &all_columns)
        }
    }
}

// ── Expression evaluation ────────────────────────────────────

fn eval_expr(expr: &Expr, row: &Row, columns: &[String]) -> Result<Value> {
    match expr {
        Expr::Column { table, name } => {
            let col_name = if let Some(tbl) = table {
                // Qualified: search for exact match or strip table prefix
                format!("{tbl}.{name}")
            } else {
                name.clone()
            };

            // Try qualified name first
            if let Some(idx) = columns.iter().position(|c| c.eq_ignore_ascii_case(&col_name)) {
                return Ok(row.values.get(idx).cloned().unwrap_or(Value::Null));
            }

            // Try unqualified
            if let Some(idx) = columns.iter().position(|c| c.eq_ignore_ascii_case(name)) {
                return Ok(row.values.get(idx).cloned().unwrap_or(Value::Null));
            }

            Err(CsvqlError::ColumnNotFound(col_name))
        }

        Expr::IntegerLiteral(n) => Ok(Value::Integer(*n)),
        Expr::FloatLiteral(n) => Ok(Value::Float(*n)),
        Expr::StringLiteral(s) => Ok(Value::String(s.clone())),
        Expr::BooleanLiteral(b) => Ok(Value::Boolean(*b)),
        Expr::Null => Ok(Value::Null),
        Expr::Star => Ok(Value::Integer(1)),

        Expr::BinaryOp { left, op, right } => {
            let l = eval_expr(left, row, columns)?;
            let r = eval_expr(right, row, columns)?;
            eval_binary_op(&l, op, &r)
        }

        Expr::UnaryOp { op, operand } => {
            let val = eval_expr(operand, row, columns)?;
            match op {
                UnaryOperator::Not => Ok(Value::Boolean(!val.is_truthy())),
                UnaryOperator::Neg => match val {
                    Value::Integer(n) => Ok(Value::Integer(-n)),
                    Value::Float(n) => Ok(Value::Float(-n)),
                    _ => Err(CsvqlError::TypeMismatch {
                        operation: "negate".into(),
                        left: val.type_name().into(),
                        right: "N/A".into(),
                    }),
                },
            }
        }

        Expr::Function { name, args, .. } => {
            eval_scalar_function(name, args, row, columns)
        }

        Expr::IsNull { expr, negated } => {
            let val = eval_expr(expr, row, columns)?;
            let is_null = val.is_null();
            Ok(Value::Boolean(if *negated { !is_null } else { is_null }))
        }

        Expr::InList {
            expr,
            list,
            negated,
        } => {
            let val = eval_expr(expr, row, columns)?;
            let found = list
                .iter()
                .any(|item| eval_expr(item, row, columns).map(|v| v == val).unwrap_or(false));
            Ok(Value::Boolean(if *negated { !found } else { found }))
        }

        Expr::BetweenExpr {
            expr,
            low,
            high,
            negated,
        } => {
            let val = eval_expr(expr, row, columns)?;
            let lo = eval_expr(low, row, columns)?;
            let hi = eval_expr(high, row, columns)?;
            let in_range = val >= lo && val <= hi;
            Ok(Value::Boolean(if *negated { !in_range } else { in_range }))
        }

        Expr::LikeExpr {
            expr,
            pattern,
            negated,
        } => {
            let val = eval_expr(expr, row, columns)?;
            let pat = eval_expr(pattern, row, columns)?;
            let matched = match (&val, &pat) {
                (Value::String(s), Value::String(p)) => like_match(s, p),
                _ => false,
            };
            Ok(Value::Boolean(if *negated { !matched } else { matched }))
        }

        Expr::CaseExpr {
            operand,
            when_clauses,
            else_clause,
        } => {
            if let Some(op) = operand {
                let op_val = eval_expr(op, row, columns)?;
                for (when_expr, then_expr) in when_clauses {
                    let when_val = eval_expr(when_expr, row, columns)?;
                    if op_val == when_val {
                        return eval_expr(then_expr, row, columns);
                    }
                }
            } else {
                for (when_expr, then_expr) in when_clauses {
                    let when_val = eval_expr(when_expr, row, columns)?;
                    if when_val.is_truthy() {
                        return eval_expr(then_expr, row, columns);
                    }
                }
            }
            if let Some(else_expr) = else_clause {
                eval_expr(else_expr, row, columns)
            } else {
                Ok(Value::Null)
            }
        }
    }
}

fn eval_binary_op(left: &Value, op: &BinaryOperator, right: &Value) -> Result<Value> {
    // NULL propagation (except AND/OR)
    if matches!(op, BinaryOperator::Eq) && left.is_null() && right.is_null() {
        return Ok(Value::Boolean(true));
    }
    if (left.is_null() || right.is_null())
        && !matches!(op, BinaryOperator::And | BinaryOperator::Or)
    {
        return Ok(Value::Null);
    }

    match op {
        BinaryOperator::Add => numeric_op(left, right, |a, b| a + b, |a, b| a + b),
        BinaryOperator::Sub => numeric_op(left, right, |a, b| a - b, |a, b| a - b),
        BinaryOperator::Mul => numeric_op(left, right, |a, b| a * b, |a, b| a * b),
        BinaryOperator::Div => {
            if let Some(r) = right.to_f64() {
                if r == 0.0 {
                    return Err(CsvqlError::DivisionByZero);
                }
            }
            numeric_op(left, right, |a, b| a / b, |a, b| a / b)
        }
        BinaryOperator::Mod => numeric_op(left, right, |a, b| a % b, |a, b| a % b),

        BinaryOperator::Eq => Ok(Value::Boolean(left == right)),
        BinaryOperator::Neq => Ok(Value::Boolean(left != right)),
        BinaryOperator::Lt => Ok(Value::Boolean(left < right)),
        BinaryOperator::Gt => Ok(Value::Boolean(left > right)),
        BinaryOperator::Lte => Ok(Value::Boolean(left <= right)),
        BinaryOperator::Gte => Ok(Value::Boolean(left >= right)),

        BinaryOperator::And => Ok(Value::Boolean(left.is_truthy() && right.is_truthy())),
        BinaryOperator::Or => Ok(Value::Boolean(left.is_truthy() || right.is_truthy())),

        BinaryOperator::Concat => {
            Ok(Value::String(format!("{left}{right}")))
        }
    }
}

fn numeric_op(
    left: &Value,
    right: &Value,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Result<Value> {
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(int_op(*a, *b))),
        _ => {
            let a = left.to_f64().ok_or_else(|| CsvqlError::TypeMismatch {
                operation: "arithmetic".into(),
                left: left.type_name().into(),
                right: right.type_name().into(),
            })?;
            let b = right.to_f64().ok_or_else(|| CsvqlError::TypeMismatch {
                operation: "arithmetic".into(),
                left: left.type_name().into(),
                right: right.type_name().into(),
            })?;
            Ok(Value::Float(float_op(a, b)))
        }
    }
}

/// Built-in scalar functions.
fn eval_scalar_function(
    name: &str,
    args: &[Expr],
    row: &Row,
    columns: &[String],
) -> Result<Value> {
    let evaluated: Vec<Value> = args
        .iter()
        .map(|a| eval_expr(a, row, columns))
        .collect::<Result<Vec<_>>>()?;

    match name.to_uppercase().as_str() {
        "UPPER" => match evaluated.first() {
            Some(Value::String(s)) => Ok(Value::String(s.to_uppercase())),
            Some(v) => Ok(Value::String(format!("{v}").to_uppercase())),
            None => Ok(Value::Null),
        },
        "LOWER" => match evaluated.first() {
            Some(Value::String(s)) => Ok(Value::String(s.to_lowercase())),
            Some(v) => Ok(Value::String(format!("{v}").to_lowercase())),
            None => Ok(Value::Null),
        },
        "LENGTH" | "LEN" => match evaluated.first() {
            Some(Value::String(s)) => Ok(Value::Integer(s.len() as i64)),
            Some(Value::Null) => Ok(Value::Null),
            Some(v) => Ok(Value::Integer(format!("{v}").len() as i64)),
            None => Ok(Value::Null),
        },
        "TRIM" => match evaluated.first() {
            Some(Value::String(s)) => Ok(Value::String(s.trim().to_string())),
            other => Ok(other.cloned().unwrap_or(Value::Null)),
        },
        "SUBSTR" | "SUBSTRING" => {
            let s = match evaluated.first() {
                Some(Value::String(s)) => s.clone(),
                Some(v) => format!("{v}"),
                None => return Ok(Value::Null),
            };
            let start = evaluated
                .get(1)
                .and_then(|v| v.to_i64())
                .unwrap_or(1)
                .max(1) as usize
                - 1;
            let len = evaluated
                .get(2)
                .and_then(|v| v.to_i64())
                .map(|n| n as usize);

            let result: String = if let Some(l) = len {
                s.chars().skip(start).take(l).collect()
            } else {
                s.chars().skip(start).collect()
            };
            Ok(Value::String(result))
        }
        "ABS" => match evaluated.first() {
            Some(Value::Integer(n)) => Ok(Value::Integer(n.abs())),
            Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
            _ => Ok(Value::Null),
        },
        "ROUND" => {
            let n = evaluated.first().and_then(|v| v.to_f64());
            let decimals = evaluated.get(1).and_then(|v| v.to_i64()).unwrap_or(0);
            match n {
                Some(val) => {
                    let factor = 10_f64.powi(decimals as i32);
                    Ok(Value::Float((val * factor).round() / factor))
                }
                None => Ok(Value::Null),
            }
        }
        "COALESCE" => {
            for val in &evaluated {
                if !val.is_null() {
                    return Ok(val.clone());
                }
            }
            Ok(Value::Null)
        }
        "NULLIF" => {
            if evaluated.len() >= 2 && evaluated[0] == evaluated[1] {
                Ok(Value::Null)
            } else {
                Ok(evaluated.into_iter().next().unwrap_or(Value::Null))
            }
        }
        "CAST" => Ok(evaluated.into_iter().next().unwrap_or(Value::Null)),
        "TYPEOF" => match evaluated.first() {
            Some(v) => Ok(Value::String(v.type_name().to_string())),
            None => Ok(Value::Null),
        },
        _ => Err(CsvqlError::ParseError(format!(
            "Unknown function: {name}"
        ))),
    }
}

/// SQL LIKE pattern matching (% and _ wildcards).
fn like_match(s: &str, pattern: &str) -> bool {
    let s_chars: Vec<char> = s.chars().collect();
    let p_chars: Vec<char> = pattern.chars().collect();
    like_match_recursive(&s_chars, 0, &p_chars, 0)
}

fn like_match_recursive(s: &[char], si: usize, p: &[char], pi: usize) -> bool {
    if pi == p.len() {
        return si == s.len();
    }

    match p[pi] {
        '%' => {
            // % matches zero or more characters
            for i in si..=s.len() {
                if like_match_recursive(s, i, p, pi + 1) {
                    return true;
                }
            }
            false
        }
        '_' => {
            // _ matches exactly one character
            si < s.len() && like_match_recursive(s, si + 1, p, pi + 1)
        }
        ch => {
            si < s.len()
                && s[si].to_ascii_lowercase() == ch.to_ascii_lowercase()
                && like_match_recursive(s, si + 1, p, pi + 1)
        }
    }
}

// ── Sorting ──────────────────────────────────────────────────

fn sort_rows(
    rows: &mut [Row],
    order_by: &[OrderByItem],
    columns: &[String],
) -> Result<()> {
    rows.sort_by(|a, b| {
        for item in order_by {
            let va = eval_sort_expr(&item.expr, a, columns);
            let vb = eval_sort_expr(&item.expr, b, columns);
            let cmp = va.cmp(&vb);
            let cmp = if item.descending { cmp.reverse() } else { cmp };
            if cmp != std::cmp::Ordering::Equal {
                return cmp;
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(())
}

/// Evaluate expressions for ORDER BY, where aggregate results
/// are already materialized in the result columns.
fn eval_sort_expr(expr: &Expr, row: &Row, columns: &[String]) -> Value {
    match expr {
        Expr::Column { name, .. } => {
            if let Some(idx) = columns.iter().position(|c| c.eq_ignore_ascii_case(name)) {
                row.values.get(idx).cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        // For aggregate functions in ORDER BY, match against the result column name
        Expr::Function { .. } => {
            let display = expr_display_name(expr);
            if let Some(idx) = columns.iter().position(|c| c.eq_ignore_ascii_case(&display)) {
                row.values.get(idx).cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        _ => eval_expr(expr, row, columns).unwrap_or(Value::Null),
    }
}

/// Produce a display name for an expression (used for column headers).
pub fn expr_display_name(expr: &Expr) -> String {
    match expr {
        Expr::Column { table: Some(t), name } => format!("{t}.{name}"),
        Expr::Column { name, .. } => name.clone(),
        Expr::Function { name, args, .. } => {
            let arg_strs: Vec<String> = args.iter().map(expr_display_name).collect();
            format!("{}({})", name.to_lowercase(), arg_strs.join(", "))
        }
        Expr::Star => "*".to_string(),
        Expr::IntegerLiteral(n) => n.to_string(),
        Expr::FloatLiteral(n) => n.to_string(),
        Expr::StringLiteral(s) => format!("'{s}'"),
        Expr::BinaryOp { left, op, right } => {
            format!(
                "{} {} {}",
                expr_display_name(left),
                op,
                expr_display_name(right)
            )
        }
        _ => "?".to_string(),
    }
}
