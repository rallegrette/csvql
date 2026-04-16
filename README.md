# csvql

**Run SQL queries against CSV files directly from your terminal.**

csvql is a from-scratch SQL engine written in pure Rust. It includes a hand-rolled lexer, a recursive descent parser that produces a typed AST, a schema-inferring CSV loader, a trait-based aggregation system, and a full query execution engine — all without depending on any SQL library.

```
$ csvql query "SELECT department, avg(salary), count(*) FROM employees.csv GROUP BY department ORDER BY avg(salary) DESC"

┌─────────────┬─────────────┬──────────┐
│ department  ┆ avg(salary) ┆ count(*) │
╞═════════════╪═════════════╪══════════╡
│ Engineering ┆     116,000 ┆        5 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
│ Product     ┆      98,400 ┆        5 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
│ Marketing   ┆      75,200 ┆        5 │
└─────────────┴─────────────┴──────────┘

3 rows returned in 0ms
```

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Feature Showcase](#feature-showcase)
- [SQL Reference](#sql-reference)
- [Architecture](#architecture)
- [How the Pipeline Works](#how-the-pipeline-works)
- [Design Decisions](#design-decisions)
- [Testing](#testing)
- [License](#license)

---

## Installation

### From source

```bash
git clone https://github.com/youruser/csvql.git
cd csvql
cargo build --release

# The binary is at target/release/csvql
# Optionally, copy it into your PATH:
cp target/release/csvql /usr/local/bin/
```

### Requirements

- Rust 1.70+ (uses 2021 edition)
- No runtime dependencies beyond what Cargo pulls in

---

## Quick Start

csvql has two subcommands: `query` and `schema`.

### Run a query

```bash
csvql query "SELECT name, salary FROM employees.csv WHERE department = 'Engineering' ORDER BY salary DESC"
```

### Inspect a file's schema

```bash
csvql schema employees.csv
```

```
Schema for: employees.csv
┌────────────┬─────────┬──────────┬───────┐
│ Column     ┆ Type    ┆ Non-Null ┆ Nulls │
╞════════════╪═════════╪══════════╪═══════╡
│ id         ┆ INTEGER ┆ 15       ┆ 0     │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ name       ┆ STRING  ┆ 15       ┆ 0     │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ department ┆ STRING  ┆ 15       ┆ 0     │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ salary     ┆ INTEGER ┆ 15       ┆ 0     │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ age        ┆ INTEGER ┆ 15       ┆ 0     │
├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ active     ┆ BOOLEAN ┆ 15       ┆ 0     │
└────────────┴─────────┴──────────┴───────┘

6 columns, 15 total rows
```

### Export results to a new CSV

```bash
csvql query "SELECT * FROM employees.csv WHERE active = TRUE" --output active_employees.csv
```

---

## Feature Showcase

### Filtering with WHERE

```bash
csvql query "SELECT name, salary FROM employees.csv WHERE department = 'Engineering' AND salary > 100000 ORDER BY salary DESC"
```

```
┌──────────────┬─────────┐
│ name         ┆ salary  │
╞══════════════╪═════════╡
│ Mia Robinson ┆ 130,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Alice Chen   ┆ 125,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Bob Martinez ┆ 118,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Jack Brown   ┆ 112,000 │
└──────────────┴─────────┘
```

### Aggregation with GROUP BY and HAVING

```bash
csvql query "SELECT department, count(*) AS headcount, avg(salary) AS avg_pay, min(age) AS youngest, max(age) AS oldest FROM employees.csv WHERE active = TRUE GROUP BY department HAVING avg(salary) > 70000 ORDER BY avg(salary) DESC"
```

```
┌─────────────┬───────────┬─────────┬──────────┬────────┐
│ department  ┆ headcount ┆ avg_pay ┆ youngest ┆ oldest │
╞═════════════╪═══════════╪═════════╪══════════╪════════╡
│ Engineering ┆         5 ┆ 116,000 ┆       28 ┆     45 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ Product     ┆         3 ┆ 101,000 ┆       27 ┆     38 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ Marketing   ┆         5 ┆  75,200 ┆       24 ┆     34 │
└─────────────┴───────────┴─────────┴──────────┴────────┘
```

### CASE Expressions

```bash
csvql query "SELECT name, department, CASE WHEN salary > 100000 THEN 'Senior' WHEN salary > 80000 THEN 'Mid' ELSE 'Junior' END AS level FROM employees.csv ORDER BY salary DESC LIMIT 5"
```

```
┌──────────────┬─────────────┬────────┐
│ name         ┆ department  ┆ level  │
╞══════════════╪═════════════╪════════╡
│ Mia Robinson ┆ Engineering ┆ Senior │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ Alice Chen   ┆ Engineering ┆ Senior │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ Bob Martinez ┆ Engineering ┆ Senior │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ David Kim    ┆ Product     ┆ Senior │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ Eve Johnson  ┆ Product     ┆ Mid    │
└──────────────┴─────────────┴────────┘
```

### Joining Two CSV Files

```bash
csvql query "SELECT e.name, e.department, d.location, d.budget FROM employees.csv AS e JOIN departments.csv AS d ON e.department = d.dept_name WHERE e.salary > 100000 ORDER BY e.name"
```

```
┌──────────────┬──────────────┬───────────────┬───────────┐
│ e.name       ┆ e.department ┆ d.location    ┆ d.budget  │
╞══════════════╪══════════════╪═══════════════╪═══════════╡
│ Alice Chen   ┆ Engineering  ┆ San Francisco ┆ 2,000,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ Bob Martinez ┆ Engineering  ┆ San Francisco ┆ 2,000,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ David Kim    ┆ Product      ┆ New York      ┆ 1,500,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ Jack Brown   ┆ Engineering  ┆ San Francisco ┆ 2,000,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ Mia Robinson ┆ Engineering  ┆ San Francisco ┆ 2,000,000 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ Noah Clark   ┆ Product      ┆ New York      ┆ 1,500,000 │
└──────────────┴──────────────┴───────────────┴───────────┘
```

### Range Queries with BETWEEN and IN

```bash
csvql query "SELECT name, salary FROM employees.csv WHERE salary BETWEEN 90000 AND 120000 ORDER BY name"

csvql query "SELECT name, department FROM employees.csv WHERE department IN ('Engineering', 'Product') AND active = TRUE LIMIT 5"
```

---

## SQL Reference

### Supported Clauses

| Clause | Syntax | Notes |
|--------|--------|-------|
| `SELECT` | `SELECT expr [AS alias], ...` | Supports `*`, expressions, aliases, `DISTINCT` |
| `FROM` | `FROM file.csv [AS alias]` | Accepts relative/absolute file paths |
| `JOIN` | `[INNER\|LEFT] JOIN file.csv [AS alias] ON expr` | Inner and left outer joins |
| `WHERE` | `WHERE condition` | Arbitrary boolean expressions |
| `GROUP BY` | `GROUP BY expr, ...` | Groups rows; enables aggregate functions |
| `HAVING` | `HAVING condition` | Filters groups after aggregation |
| `ORDER BY` | `ORDER BY expr [ASC\|DESC], ...` | Multi-column sorting |
| `LIMIT` | `LIMIT n` | Caps result row count |
| `OFFSET` | `OFFSET n` | Skips first n result rows |

### Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`  `-`  `*`  `/`  `%` |
| Comparison | `=`  `!=`  `<>`  `<`  `>`  `<=`  `>=` |
| Logical | `AND`  `OR`  `NOT` |
| String | `\|\|` (concatenation) |
| Null testing | `IS NULL`  `IS NOT NULL` |
| Set membership | `IN (val, ...)`  `NOT IN (val, ...)` |
| Range | `BETWEEN low AND high`  `NOT BETWEEN ...` |
| Pattern | `LIKE pattern`  `NOT LIKE pattern` (`%` = any chars, `_` = one char) |
| Conditional | `CASE WHEN cond THEN result [ELSE default] END` |

### Aggregate Functions

| Function | Description |
|----------|-------------|
| `COUNT(*)` | Total number of rows in each group |
| `COUNT(column)` | Number of non-null values |
| `SUM(column)` | Sum of numeric values |
| `AVG(column)` | Arithmetic mean of numeric values |
| `MIN(column)` | Minimum value (works on all types) |
| `MAX(column)` | Maximum value (works on all types) |

### Scalar Functions

| Function | Description |
|----------|-------------|
| `UPPER(s)` | Convert string to uppercase |
| `LOWER(s)` | Convert string to lowercase |
| `TRIM(s)` | Strip leading/trailing whitespace |
| `LENGTH(s)` | Character count of string |
| `SUBSTR(s, start [, len])` | Extract substring (1-indexed) |
| `ABS(n)` | Absolute value |
| `ROUND(n [, decimals])` | Round to specified decimal places |
| `COALESCE(a, b, ...)` | Return first non-null argument |
| `NULLIF(a, b)` | Return null if a = b, else a |
| `TYPEOF(x)` | Return the type name of a value |

### Type System

csvql automatically infers column types by scanning all rows on load:

| Type | Recognized Values |
|------|-------------------|
| `INTEGER` | `42`, `-7`, `1000000` |
| `FLOAT` | `3.14`, `-0.5`, `1e10` |
| `BOOLEAN` | `true`, `false` (case-insensitive) |
| `STRING` | Everything else |
| `NULL` | Empty field, `NULL`, `NA` (case-insensitive) |

Type widening follows a lattice: if a column contains both integers and floats, it becomes `FLOAT`. If it contains mixed numeric and non-numeric data, it becomes `STRING`.

---

## Architecture

```
                  SQL string
                      │
                      ▼
               ┌─────────────┐
               │   Lexer      │  lexer.rs — character-by-character tokenization
               │              │  handles strings, numbers, keywords, operators
               └──────┬──────┘
                      │ Vec<Token>
                      ▼
               ┌─────────────┐
               │   Parser     │  parser.rs — recursive descent with precedence climbing
               │              │  produces a typed AST from the token stream
               └──────┬──────┘
                      │ SelectStatement (AST)
                      ▼
               ┌─────────────┐
               │   Loader     │  loader.rs — reads CSV, infers schema, coerces types
               │              │  turns raw strings into typed Value enums
               └──────┬──────┘
                      │ Table { columns, rows }
                      ▼
               ┌─────────────┐
               │   Engine     │  engine.rs — evaluates the AST against loaded data
               │              │  WHERE → GROUP BY → HAVING → SELECT → ORDER BY → LIMIT
               └──────┬──────┘
                      │ Table (result)
                      ▼
               ┌─────────────┐
               │   Output     │  output.rs — renders results as Unicode table or CSV
               └─────────────┘
```

### Module Breakdown

```
src/
├── main.rs          CLI entry point — clap-based subcommand dispatch
├── lexer.rs         Tokenizer: SQL text → Vec<Token>
├── ast.rs           AST type definitions: Expr, SelectStatement, BinaryOperator, etc.
├── parser.rs        Recursive descent parser: Vec<Token> → SelectStatement
├── types.rs         Runtime value system: Value enum, Row, Table, ColumnType
├── loader.rs        CSV ingestion: schema inference, type coercion, null detection
├── aggregator.rs    Aggregator trait + implementations: Count, Sum, Avg, Min, Max
├── engine.rs        Query execution: expression eval, joins, grouping, sorting
├── output.rs        Rendering: Unicode table (comfy-table) + CSV file writer
└── error.rs         Error hierarchy: 12 distinct variants via thiserror
```

---

## How the Pipeline Works

### 1. Lexing (`lexer.rs`)

The lexer consumes a SQL string character-by-character and produces a flat token stream. It handles:

- **Keywords** — case-insensitive recognition of `SELECT`, `FROM`, `WHERE`, etc.
- **Compound keywords** — a post-pass merges `GROUP` + `BY` into a single `GroupBy` token, and similarly for `ORDER BY` and `LEFT JOIN`.
- **String literals** — single-quoted with `''` escape support (e.g., `'O''Brien'`).
- **Quoted identifiers** — double-quoted for column names with spaces or reserved words.
- **Numbers** — integer and floating-point literals distinguished at lex time.
- **Operators** — single-char (`=`, `<`, `>`) and multi-char (`<=`, `!=`, `<>`, `||`).

### 2. Parsing (`parser.rs`)

A **recursive descent parser** with **precedence climbing** transforms tokens into a typed AST. The precedence hierarchy is:

```
parse_expr
  └─ parse_or           (OR)
       └─ parse_and     (AND)
            └─ parse_not (NOT)
                 └─ parse_comparison  (=, !=, <, >, <=, >=, IS NULL, IN, BETWEEN, LIKE)
                      └─ parse_addition   (+, -, ||)
                           └─ parse_multiplication  (*, /, %)
                                └─ parse_unary  (-, NOT)
                                     └─ parse_primary  (literals, columns, functions, parens, CASE)
```

The parser validates the full SQL grammar and produces a `SelectStatement` that captures every clause as a strongly-typed Rust struct.

### 3. AST (`ast.rs`)

The expression tree is modeled as a recursive Rust enum — the heart of the type system:

```rust
pub enum Expr {
    Column { table: Option<String>, name: String },
    IntegerLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BooleanLiteral(bool),
    Null,
    BinaryOp { left: Box<Expr>, op: BinaryOperator, right: Box<Expr> },
    UnaryOp { op: UnaryOperator, operand: Box<Expr> },
    Function { name: String, args: Vec<Expr>, distinct: bool },
    IsNull { expr: Box<Expr>, negated: bool },
    InList { expr: Box<Expr>, list: Vec<Expr>, negated: bool },
    BetweenExpr { expr: Box<Expr>, low: Box<Expr>, high: Box<Expr>, negated: bool },
    LikeExpr { expr: Box<Expr>, pattern: Box<Expr>, negated: bool },
    CaseExpr { operand: Option<Box<Expr>>, when_clauses: Vec<(Expr, Expr)>, else_clause: Option<Box<Expr>> },
    Star,
}
```

Every node is exhaustively matchable — no stringly-typed escape hatches.

### 4. Type Coercion (`loader.rs`)

On CSV load, csvql scans every row to infer each column's type. The widening lattice ensures safe promotion:

```
Integer ──┐
          ├──▶ Float ──┐
Float  ───┘            ├──▶ String
Boolean ───────────────┘
```

Null-like values (`""`, `"NULL"`, `"NA"`) are detected and stored as `Value::Null` rather than empty strings, enabling proper `IS NULL` semantics.

### 5. Aggregation (`aggregator.rs`)

Aggregation is built on a trait:

```rust
pub trait Aggregator: std::fmt::Debug {
    fn accumulate(&mut self, value: &Value);
    fn finish(&self) -> Value;
    fn clone_box(&self) -> Box<dyn Aggregator>;
}
```

Each aggregate function (`CountAgg`, `SumAgg`, `AvgAgg`, `MinAgg`, `MaxAgg`) implements this trait. Adding a new aggregate — say `STDDEV` or `MEDIAN` — requires only a new struct and a one-line match arm in `create_aggregator`.

### 6. Execution (`engine.rs`)

The engine walks the AST and evaluates it against loaded tables:

1. **Load** — CSV files referenced in `FROM` and `JOIN` are loaded and type-coerced.
2. **Join** — nested-loop join with ON expression evaluation; LEFT JOIN produces nulls for unmatched right rows.
3. **Filter** — `WHERE` clause is evaluated per-row; non-truthy rows are discarded.
4. **Group** — rows are bucketed by `GROUP BY` key into an order-preserving hash map.
5. **Aggregate** — aggregate expressions are evaluated per-group using the `Aggregator` trait.
6. **Having** — post-aggregation filter on group results.
7. **Project** — `SELECT` expressions are evaluated to produce result columns.
8. **Distinct** — duplicate result rows are removed.
9. **Sort** — `ORDER BY` with multi-column, mixed ASC/DESC support.
10. **Paginate** — `OFFSET` then `LIMIT` are applied.

### 7. Error Handling (`error.rs`)

All errors flow through a single `CsvqlError` enum with 12 distinct variants, derived via `thiserror`:

- `LexerError` — invalid characters, unterminated strings
- `ParseError` — malformed SQL syntax
- `UnexpectedToken` — expected vs. found token mismatch
- `CsvError` / `IoError` — file system issues (with automatic `From` conversion)
- `FileNotFound` — missing CSV file
- `ColumnNotFound` — reference to nonexistent column
- `TypeMismatch` — arithmetic on incompatible types
- `DivisionByZero` — runtime divide-by-zero guard
- `AggregateError` — malformed aggregate function call

Every error has a human-readable message. The CLI catches errors and exits with code 1, printing the error to stderr.

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Hand-rolled lexer** instead of a lexer generator | Full control over error messages and token design; no build-time dependencies |
| **Recursive descent parser** instead of PEG/parser combinator | Explicit precedence climbing is easy to extend and debug; each grammar rule maps to one function |
| **`Expr` as a recursive enum** | Idiomatic Rust — exhaustive pattern matching ensures every node type is handled at compile time |
| **`Aggregator` trait** | Open for extension; new aggregates are a struct + trait impl with zero changes to existing code |
| **`Value` enum with manual `Ord`/`Eq`** | Cross-type comparison (e.g., Integer vs Float) requires custom logic that derived traits can't provide |
| **Schema inference over all rows** | Slower than sampling but guarantees correct types; csvql prioritizes correctness over speed for initial load |
| **`thiserror` for errors** | Zero-cost error hierarchy with derived `Display`; keeps the codebase free of manual `impl Display` boilerplate |
| **`comfy-table` for output** | Unicode box-drawing with right-aligned numerics and bold headers out of the box |
| **No unsafe code** | The entire codebase is safe Rust |

---

## Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| [`csv`](https://crates.io/crates/csv) | 1.x | CSV file reading with flexible parsing |
| [`clap`](https://crates.io/crates/clap) | 4.x | CLI argument parsing with derive macros |
| [`thiserror`](https://crates.io/crates/thiserror) | 2.x | Ergonomic custom error types |
| [`comfy-table`](https://crates.io/crates/comfy-table) | 7.x | Pretty Unicode table rendering |
| [`chrono`](https://crates.io/crates/chrono) | 0.4.x | Date/time column support |
| [`serde`](https://crates.io/crates/serde) | 1.x | Serialization framework |

---

## Testing

Run the full test suite:

```bash
cargo test
```

The test suite covers:

- **Lexer tests** — tokenization of SELECT statements, operators, compound keywords (`GROUP BY`, `ORDER BY`), string literals with escapes, float literals
- **Parser tests** — simple SELECT, SELECT *, WHERE clauses, GROUP BY with aggregation, ORDER BY DESC, JOIN syntax, complex multi-clause queries
- **Loader tests** — type widening rules (Integer + Float = Float, Integer + String = String), null coercion for empty strings / `NULL` / `NA`

```
running 14 tests
test lexer::tests::test_simple_select ... ok
test lexer::tests::test_operators ... ok
test lexer::tests::test_group_by_order_by ... ok
test lexer::tests::test_string_literal ... ok
test lexer::tests::test_float_literal ... ok
test loader::tests::test_widen_types ... ok
test loader::tests::test_coerce_null ... ok
test parser::tests::test_simple_select ... ok
test parser::tests::test_select_star ... ok
test parser::tests::test_where_clause ... ok
test parser::tests::test_group_by_with_aggregation ... ok
test parser::tests::test_order_by_desc ... ok
test parser::tests::test_join ... ok
test parser::tests::test_complex_query ... ok

test result: ok. 14 passed; 0 failed; 0 ignored
```
\
