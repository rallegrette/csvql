/// The full AST for a parsed SQL SELECT statement.
#[derive(Debug, Clone)]
pub struct SelectStatement {
    pub distinct: bool,
    pub columns: Vec<SelectColumn>,
    pub from: FromClause,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SelectColumn {
    /// SELECT *
    Wildcard,
    /// SELECT expr or SELECT expr AS alias
    Expr { expr: Expr, alias: Option<String> },
}

#[derive(Debug, Clone)]
pub struct FromClause {
    pub table: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: String,
    pub alias: Option<String>,
    pub on: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
}

#[derive(Debug, Clone)]
pub struct OrderByItem {
    pub expr: Expr,
    pub descending: bool,
}

/// Expression tree — the heart of the AST.
#[derive(Debug, Clone)]
pub enum Expr {
    /// A column reference, optionally qualified: table.column
    Column {
        table: Option<String>,
        name: String,
    },

    /// Integer literal
    IntegerLiteral(i64),

    /// Float literal
    FloatLiteral(f64),

    /// String literal
    StringLiteral(String),

    /// Boolean literal
    BooleanLiteral(bool),

    /// NULL
    Null,

    /// Binary operation: left op right
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },

    /// Unary operation: NOT expr, -expr
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expr>,
    },

    /// Function call: COUNT(x), AVG(salary), UPPER(name)
    Function {
        name: String,
        args: Vec<Expr>,
        #[allow(dead_code)]
        distinct: bool,
    },

    /// expr IS NULL / IS NOT NULL
    IsNull {
        expr: Box<Expr>,
        negated: bool,
    },

    /// expr IN (val1, val2, ...)
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },

    /// expr BETWEEN low AND high
    BetweenExpr {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
        negated: bool,
    },

    /// expr LIKE pattern
    LikeExpr {
        expr: Box<Expr>,
        pattern: Box<Expr>,
        negated: bool,
    },

    /// CASE WHEN ... THEN ... ELSE ... END
    CaseExpr {
        operand: Option<Box<Expr>>,
        when_clauses: Vec<(Expr, Expr)>,
        else_clause: Option<Box<Expr>>,
    },

    /// Star (*) — used in COUNT(*)
    Star,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
    Concat,
}

impl std::fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOperator::Add => write!(f, "+"),
            BinaryOperator::Sub => write!(f, "-"),
            BinaryOperator::Mul => write!(f, "*"),
            BinaryOperator::Div => write!(f, "/"),
            BinaryOperator::Mod => write!(f, "%"),
            BinaryOperator::Eq => write!(f, "="),
            BinaryOperator::Neq => write!(f, "!="),
            BinaryOperator::Lt => write!(f, "<"),
            BinaryOperator::Gt => write!(f, ">"),
            BinaryOperator::Lte => write!(f, "<="),
            BinaryOperator::Gte => write!(f, ">="),
            BinaryOperator::And => write!(f, "AND"),
            BinaryOperator::Or => write!(f, "OR"),
            BinaryOperator::Concat => write!(f, "||"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Not,
    Neg,
}
