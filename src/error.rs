use thiserror::Error;

#[derive(Debug, Error)]
pub enum CsvqlError {
    #[error("Lexer error at position {position}: {message}")]
    LexerError { position: usize, message: String },

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unexpected token: expected {expected}, found {found}")]
    UnexpectedToken { expected: String, found: String },

    #[error("Unexpected end of input: {0}")]
    UnexpectedEof(String),

    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Type mismatch: cannot {operation} on {left} and {right}")]
    TypeMismatch {
        operation: String,
        left: String,
        right: String,
    },

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Aggregate function '{function}' requires a column argument")]
    AggregateError { function: String },

    #[error("Cannot use aggregate function in WHERE clause")]
    AggregateInWhere,

    #[error("Column '{column}' must appear in GROUP BY clause or be used in an aggregate function")]
    UngroupedColumn { column: String },

    #[error("Ambiguous column reference '{column}' — qualify with table name")]
    AmbiguousColumn { column: String },

    #[error("Join error: {0}")]
    JoinError(String),
}

pub type Result<T> = std::result::Result<T, CsvqlError>;
