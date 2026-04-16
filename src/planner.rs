use std::fmt;
use std::path::Path;

use colored::Colorize;

use crate::ast::*;
use crate::engine::expr_display_name;

#[derive(Debug)]
pub struct PlanNode {
    pub name: String,
    pub detail: String,
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    fn new(name: &str, detail: &str) -> Self {
        PlanNode {
            name: name.to_string(),
            detail: detail.to_string(),
            children: Vec::new(),
        }
    }

    fn with_child(mut self, child: PlanNode) -> Self {
        self.children.push(child);
        self
    }
}

impl fmt::Display for PlanNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_tree(f, "", "", true)
    }
}

impl PlanNode {
    fn fmt_tree(
        &self,
        f: &mut fmt::Formatter<'_>,
        current_prefix: &str,
        child_prefix: &str,
        is_root: bool,
    ) -> fmt::Result {
        let name_colored = self.name.bold();
        if is_root {
            if self.detail.is_empty() {
                writeln!(f, "{name_colored}")?;
            } else {
                writeln!(f, "{name_colored} ({})", self.detail.dimmed())?;
            }
        } else if self.detail.is_empty() {
            writeln!(f, "{current_prefix}{name_colored}")?;
        } else {
            writeln!(f, "{current_prefix}{name_colored} ({})", self.detail.dimmed())?;
        }

        for (i, child) in self.children.iter().enumerate() {
            let is_last = i == self.children.len() - 1;
            let (conn, next_prefix) = if is_last {
                (format!("{child_prefix}└── "), format!("{child_prefix}    "))
            } else {
                (format!("{child_prefix}├── "), format!("{child_prefix}│   "))
            };
            child.fmt_tree(f, &conn, &next_prefix, false)?;
        }

        Ok(())
    }
}

pub fn build_plan(stmt: &SelectStatement) -> PlanNode {
    let mut current = build_scan_node(stmt);

    if !stmt.joins.is_empty() {
        for join in &stmt.joins {
            let join_type_str = match join.join_type {
                JoinType::Inner => "INNER",
                JoinType::Left => "LEFT",
            };
            let on_str = expr_display_name(&join.on);
            let join_node = PlanNode::new(
                &format!("{join_type_str} JOIN"),
                &format!("{} ON {on_str}", join.table),
            );
            let mut new_node = PlanNode::new("NestedLoopJoin", join_type_str);
            new_node.children.push(current);
            new_node.children.push(join_node);
            current = new_node;
        }
    }

    if let Some(ref where_expr) = stmt.where_clause {
        let filter_str = expr_display_name(where_expr);
        let filter_node = PlanNode::new("Filter", &filter_str).with_child(current);
        current = filter_node;
    }

    let has_aggregates = stmt.columns.iter().any(|c| {
        matches!(c, SelectColumn::Expr { expr, .. } if contains_aggregate_expr(expr))
    });

    if has_aggregates || !stmt.group_by.is_empty() {
        let group_cols: Vec<String> = stmt.group_by.iter().map(expr_display_name).collect();
        let group_str = if group_cols.is_empty() {
            "whole table".to_string()
        } else {
            format!("GROUP BY {}", group_cols.join(", "))
        };

        let agg_exprs: Vec<String> = stmt
            .columns
            .iter()
            .filter_map(|c| match c {
                SelectColumn::Expr { expr, .. } if contains_aggregate_expr(expr) => {
                    Some(expr_display_name(expr))
                }
                _ => None,
            })
            .collect();

        let mut agg_node = PlanNode::new("HashAggregate", &group_str);
        if !agg_exprs.is_empty() {
            agg_node.children.push(PlanNode::new(
                "Aggregates",
                &agg_exprs.join(", "),
            ));
        }
        agg_node.children.push(current);
        current = agg_node;
    }

    if let Some(ref having_expr) = stmt.having {
        let having_str = expr_display_name(having_expr);
        let having_node = PlanNode::new("HavingFilter", &having_str).with_child(current);
        current = having_node;
    }

    let proj_cols: Vec<String> = stmt
        .columns
        .iter()
        .map(|c| match c {
            SelectColumn::Wildcard => "*".to_string(),
            SelectColumn::Expr { expr, alias } => alias
                .clone()
                .unwrap_or_else(|| expr_display_name(expr)),
        })
        .collect();
    current = PlanNode::new("Project", &proj_cols.join(", ")).with_child(current);

    if stmt.distinct {
        current = PlanNode::new("Distinct", "").with_child(current);
    }

    if !stmt.order_by.is_empty() {
        let sort_keys: Vec<String> = stmt
            .order_by
            .iter()
            .map(|o| {
                let dir = if o.descending { "DESC" } else { "ASC" };
                format!("{} {dir}", expr_display_name(&o.expr))
            })
            .collect();
        current = PlanNode::new("Sort", &sort_keys.join(", ")).with_child(current);
    }

    if let Some(offset) = stmt.offset {
        current = PlanNode::new("Offset", &offset.to_string()).with_child(current);
    }

    if let Some(limit) = stmt.limit {
        current = PlanNode::new("Limit", &limit.to_string()).with_child(current);
    }

    current
}

fn build_scan_node(stmt: &SelectStatement) -> PlanNode {
    match &stmt.from.source {
        TableRef::File(path) => {
            let row_est = estimate_rows(path);
            let col_est = estimate_cols(path);
            let detail = format!("{path}, {col_est} cols, ~{row_est} rows");
            PlanNode::new("CsvScan", &detail)
        }
        TableRef::Subquery(sub) => {
            let alias = stmt.from.alias.as_deref().unwrap_or("subquery");
            let sub_plan = build_plan(sub);
            PlanNode::new("SubqueryScan", alias).with_child(sub_plan)
        }
    }
}

fn estimate_rows(path: &str) -> usize {
    let p = Path::new(path);
    if !p.exists() {
        return 0;
    }
    let mut rdr = match csv::ReaderBuilder::new().flexible(true).from_path(p) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    rdr.records().count()
}

fn estimate_cols(path: &str) -> usize {
    let p = Path::new(path);
    if !p.exists() {
        return 0;
    }
    let mut rdr = match csv::ReaderBuilder::new().flexible(true).from_path(p) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    match rdr.headers() {
        Ok(h) => h.len(),
        Err(_) => 0,
    }
}

fn contains_aggregate_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Function { name, .. } => matches!(
            name.to_uppercase().as_str(),
            "COUNT" | "SUM" | "AVG" | "MIN" | "MAX"
        ),
        Expr::BinaryOp { left, right, .. } => {
            contains_aggregate_expr(left) || contains_aggregate_expr(right)
        }
        Expr::UnaryOp { operand, .. } => contains_aggregate_expr(operand),
        _ => false,
    }
}
