use crate::error::{CsvqlError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select,
    From,
    Where,
    GroupBy,
    OrderBy,
    Having,
    Limit,
    Offset,
    As,
    On,
    Join,
    LeftJoin,
    Inner,
    Left,
    Asc,
    Desc,
    And,
    Or,
    Not,
    In,
    Between,
    Like,
    Is,
    Null,
    Distinct,
    Case,
    When,
    Then,
    Else,
    End,

    // Literals
    Integer(i64),
    Float(f64),
    StringLiteral(String),
    Boolean(bool),

    // Identifiers
    Identifier(String),

    // Operators
    Star,        // *
    Plus,        // +
    Minus,       // -
    Slash,       // /
    Percent,     // %
    Eq,          // =
    Neq,         // != or <>
    Lt,          // <
    Gt,          // >
    Lte,         // <=
    Gte,         // >=
    Concat,      // ||

    // Punctuation
    Comma,
    Dot,
    LParen,
    RParen,

    // End
    Eof,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Select => write!(f, "SELECT"),
            Token::From => write!(f, "FROM"),
            Token::Where => write!(f, "WHERE"),
            Token::GroupBy => write!(f, "GROUP BY"),
            Token::OrderBy => write!(f, "ORDER BY"),
            Token::Having => write!(f, "HAVING"),
            Token::Limit => write!(f, "LIMIT"),
            Token::Offset => write!(f, "OFFSET"),
            Token::As => write!(f, "AS"),
            Token::On => write!(f, "ON"),
            Token::Join => write!(f, "JOIN"),
            Token::LeftJoin => write!(f, "LEFT JOIN"),
            Token::Inner => write!(f, "INNER"),
            Token::Left => write!(f, "LEFT"),
            Token::Asc => write!(f, "ASC"),
            Token::Desc => write!(f, "DESC"),
            Token::And => write!(f, "AND"),
            Token::Or => write!(f, "OR"),
            Token::Not => write!(f, "NOT"),
            Token::In => write!(f, "IN"),
            Token::Between => write!(f, "BETWEEN"),
            Token::Like => write!(f, "LIKE"),
            Token::Is => write!(f, "IS"),
            Token::Null => write!(f, "NULL"),
            Token::Distinct => write!(f, "DISTINCT"),
            Token::Case => write!(f, "CASE"),
            Token::When => write!(f, "WHEN"),
            Token::Then => write!(f, "THEN"),
            Token::Else => write!(f, "ELSE"),
            Token::End => write!(f, "END"),
            Token::Integer(n) => write!(f, "{n}"),
            Token::Float(n) => write!(f, "{n}"),
            Token::StringLiteral(s) => write!(f, "'{s}'"),
            Token::Boolean(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Token::Identifier(s) => write!(f, "{s}"),
            Token::Star => write!(f, "*"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::Eq => write!(f, "="),
            Token::Neq => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::Lte => write!(f, "<="),
            Token::Gte => write!(f, ">="),
            Token::Concat => write!(f, "||"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Eof => write!(f, "EOF"),
        }
    }
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                tokens.push(Token::Eof);
                break;
            }

            let token = self.next_token()?;
            tokens.push(token);
        }

        self.merge_compound_keywords(&mut tokens);
        Ok(tokens)
    }

    /// Merge sequences like [Group, Identifier("BY")] into [GroupBy]
    fn merge_compound_keywords(&self, tokens: &mut Vec<Token>) {
        let mut i = 0;
        while i + 1 < tokens.len() {
            let merged = match (&tokens[i], &tokens[i + 1]) {
                (Token::Identifier(a), Token::Identifier(b))
                    if a.eq_ignore_ascii_case("GROUP") && b.eq_ignore_ascii_case("BY") =>
                {
                    Some(Token::GroupBy)
                }
                (Token::Identifier(a), Token::Identifier(b))
                    if a.eq_ignore_ascii_case("ORDER") && b.eq_ignore_ascii_case("BY") =>
                {
                    Some(Token::OrderBy)
                }
                (Token::Left, Token::Join) => Some(Token::LeftJoin),
                _ => None,
            };

            if let Some(tok) = merged {
                tokens[i] = tok;
                tokens.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn next_token(&mut self) -> Result<Token> {
        let ch = self.peek().unwrap();

        match ch {
            '\'' => self.read_string(),
            '"' => self.read_quoted_identifier(),
            '0'..='9' => self.read_number(),
            'a'..='z' | 'A'..='Z' | '_' => self.read_identifier_or_keyword(),
            '*' => {
                self.advance();
                Ok(Token::Star)
            }
            '+' => {
                self.advance();
                Ok(Token::Plus)
            }
            '-' => {
                self.advance();
                Ok(Token::Minus)
            }
            '/' => {
                self.advance();
                Ok(Token::Slash)
            }
            '%' => {
                self.advance();
                Ok(Token::Percent)
            }
            '=' => {
                self.advance();
                Ok(Token::Eq)
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Neq)
                } else {
                    Err(CsvqlError::LexerError {
                        position: self.pos,
                        message: "Expected '=' after '!'".into(),
                    })
                }
            }
            '<' => {
                self.advance();
                match self.peek() {
                    Some('=') => {
                        self.advance();
                        Ok(Token::Lte)
                    }
                    Some('>') => {
                        self.advance();
                        Ok(Token::Neq)
                    }
                    _ => Ok(Token::Lt),
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Gte)
                } else {
                    Ok(Token::Gt)
                }
            }
            '|' => {
                self.advance();
                if self.peek() == Some('|') {
                    self.advance();
                    Ok(Token::Concat)
                } else {
                    Err(CsvqlError::LexerError {
                        position: self.pos,
                        message: "Expected '|' after '|'".into(),
                    })
                }
            }
            ',' => {
                self.advance();
                Ok(Token::Comma)
            }
            '.' => {
                self.advance();
                Ok(Token::Dot)
            }
            '(' => {
                self.advance();
                Ok(Token::LParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RParen)
            }
            _ => Err(CsvqlError::LexerError {
                position: self.pos,
                message: format!("Unexpected character: '{ch}'"),
            }),
        }
    }

    fn read_string(&mut self) -> Result<Token> {
        self.advance(); // consume opening quote
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('\'') => {
                    // escaped single quote ('')
                    if self.peek() == Some('\'') {
                        self.advance();
                        s.push('\'');
                    } else {
                        break;
                    }
                }
                Some(ch) => s.push(ch),
                None => {
                    return Err(CsvqlError::LexerError {
                        position: self.pos,
                        message: "Unterminated string literal".into(),
                    });
                }
            }
        }
        Ok(Token::StringLiteral(s))
    }

    fn read_quoted_identifier(&mut self) -> Result<Token> {
        self.advance(); // consume opening "
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some(ch) => s.push(ch),
                None => {
                    return Err(CsvqlError::LexerError {
                        position: self.pos,
                        message: "Unterminated quoted identifier".into(),
                    });
                }
            }
        }
        Ok(Token::Identifier(s))
    }

    fn read_number(&mut self) -> Result<Token> {
        let start = self.pos;
        let mut has_dot = false;

        while let Some(ch) = self.peek() {
            if ch == '.' && !has_dot {
                has_dot = true;
                self.advance();
            } else if ch.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        let num_str: String = self.input[start..self.pos].iter().collect();
        if has_dot {
            num_str
                .parse::<f64>()
                .map(Token::Float)
                .map_err(|_| CsvqlError::LexerError {
                    position: start,
                    message: format!("Invalid float: {num_str}"),
                })
        } else {
            num_str
                .parse::<i64>()
                .map(Token::Integer)
                .map_err(|_| CsvqlError::LexerError {
                    position: start,
                    message: format!("Invalid integer: {num_str}"),
                })
        }
    }

    fn read_identifier_or_keyword(&mut self) -> Result<Token> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                self.advance();
            } else {
                break;
            }
        }

        let word: String = self.input[start..self.pos].iter().collect();

        // If it contains a dot, it could be a qualified column (table.column)
        // or a filename (data.csv). We keep it as a single identifier.
        let token = match word.to_uppercase().as_str() {
            "SELECT" => Token::Select,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "HAVING" => Token::Having,
            "LIMIT" => Token::Limit,
            "OFFSET" => Token::Offset,
            "AS" => Token::As,
            "ON" => Token::On,
            "JOIN" => Token::Join,
            "INNER" => Token::Inner,
            "LEFT" => Token::Left,
            "ASC" => Token::Asc,
            "DESC" => Token::Desc,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "IN" => Token::In,
            "BETWEEN" => Token::Between,
            "LIKE" => Token::Like,
            "IS" => Token::Is,
            "NULL" => Token::Null,
            "TRUE" => Token::Boolean(true),
            "FALSE" => Token::Boolean(false),
            "DISTINCT" => Token::Distinct,
            "CASE" => Token::Case,
            "WHEN" => Token::When,
            "THEN" => Token::Then,
            "ELSE" => Token::Else,
            "END" => Token::End,
            _ => Token::Identifier(word),
        };

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let mut lexer = Lexer::new("SELECT name, age FROM people.csv");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Select,
                Token::Identifier("name".into()),
                Token::Comma,
                Token::Identifier("age".into()),
                Token::From,
                Token::Identifier("people.csv".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_operators() {
        let mut lexer = Lexer::new("age >= 18 AND salary != 0");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Identifier("age".into()),
                Token::Gte,
                Token::Integer(18),
                Token::And,
                Token::Identifier("salary".into()),
                Token::Neq,
                Token::Integer(0),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_group_by_order_by() {
        let mut lexer = Lexer::new("GROUP BY dept ORDER BY salary DESC");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::GroupBy,
                Token::Identifier("dept".into()),
                Token::OrderBy,
                Token::Identifier("salary".into()),
                Token::Desc,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let mut lexer = Lexer::new("name = 'O''Brien'");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Identifier("name".into()),
                Token::Eq,
                Token::StringLiteral("O'Brien".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_float_literal() {
        let mut lexer = Lexer::new("3.14");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::Float(3.14), Token::Eof]);
    }
}
