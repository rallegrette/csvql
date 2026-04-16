# csvql

Run SQL queries against CSV files from the terminal — with a hand-rolled lexer, recursive descent parser, and typed query engine, all in pure Rust.

## Quick Start

```bash
cargo build --release

# Query with GROUP BY and aggregation
csvql query "SELECT department, avg(salary), count(*) FROM employees.csv GROUP BY department ORDER BY avg(salary) DESC"

# Filter, sort, limit
csvql query "SELECT name, salary FROM employees.csv WHERE salary > 100000 ORDER BY salary DESC LIMIT 5"

# JOIN two CSV files
csvql query "SELECT e.name, d.location FROM employees.csv AS e JOIN departments.csv AS d ON e.department = d.dept_name"

# Inspect schema
csvql schema employees.csv

# Export results to CSV
csvql query "SELECT * FROM data.csv WHERE active = TRUE" --output results.csv
```

## Architecture

```
src/
├── main.rs        CLI entry point (clap)
├── lexer.rs       Tokenizer — SQL text → token stream
├── ast.rs         AST types — Expr, SelectStatement, BinaryOp, etc.
├── parser.rs      Recursive descent parser — tokens → AST
├── types.rs       Runtime value types — Value enum, Row, Table
├── loader.rs      CSV loading — schema inference & type coercion
├── aggregator.rs  Aggregator trait + COUNT, SUM, AVG, MIN, MAX
├── engine.rs      Query execution — eval, filter, group, join, sort
├── output.rs      Pretty table rendering (comfy-table) + CSV export
└── error.rs       Error types (thiserror)
```

## SQL Support

### Clauses
- `SELECT` (with aliases, expressions, `DISTINCT`)
- `FROM` (with table aliases)
- `JOIN` / `LEFT JOIN` (with `ON`)
- `WHERE`
- `GROUP BY`
- `HAVING`
- `ORDER BY` (`ASC` / `DESC`)
- `LIMIT` / `OFFSET`

### Expressions
- Column references (qualified: `t.col`, unqualified: `col`)
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `=`, `!=`, `<>`, `<`, `>`, `<=`, `>=`
- Logical: `AND`, `OR`, `NOT`
- `IS NULL` / `IS NOT NULL`
- `IN (val1, val2, ...)`
- `BETWEEN low AND high`
- `LIKE` (with `%` and `_` wildcards)
- `CASE WHEN ... THEN ... ELSE ... END`
- String concatenation: `||`

### Aggregate Functions
- `COUNT(*)`, `COUNT(column)`
- `SUM(column)`
- `AVG(column)`
- `MIN(column)`
- `MAX(column)`

### Scalar Functions
- `UPPER()`, `LOWER()`, `TRIM()`
- `LENGTH()`, `SUBSTR()`
- `ABS()`, `ROUND()`
- `COALESCE()`, `NULLIF()`, `TYPEOF()`

### Type System
- Automatic schema inference: `INTEGER`, `FLOAT`, `BOOLEAN`, `STRING`
- Type coercion on load (string → int/float/bool)
- NULL handling (`""`, `"NULL"`, `"NA"` → NULL)

## Crates Used

| Crate | Purpose |
|-------|---------|
| `csv` | CSV file parsing |
| `clap` | CLI argument parsing |
| `thiserror` | Custom error types |
| `comfy-table` | Pretty terminal table output |
| `chrono` | Date column support |

## License

MIT
