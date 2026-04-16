use crate::ast::*;
use crate::error::{CsvqlError, Result};
use crate::lexer::Token;

/// Recursive descent parser for a SQL SELECT subset.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<SelectStatement> {
        let stmt = self.parse_select()?;
        self.expect(Token::Eof)?;
        Ok(stmt)
    }

    // ── Helpers ──────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: Token) -> Result<Token> {
        let tok = self.advance();
        if std::mem::discriminant(&tok) == std::mem::discriminant(&expected) {
            Ok(tok)
        } else {
            Err(CsvqlError::UnexpectedToken {
                expected: expected.to_string(),
                found: tok.to_string(),
            })
        }
    }

    fn match_token(&mut self, expected: &Token) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, expected: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(expected)
    }

    fn expect_identifier(&mut self) -> Result<String> {
        match self.advance() {
            Token::Identifier(s) => Ok(s),
            other => Err(CsvqlError::UnexpectedToken {
                expected: "identifier".into(),
                found: other.to_string(),
            }),
        }
    }

    // ── SELECT ───────────────────────────────────────────────

    fn parse_select(&mut self) -> Result<SelectStatement> {
        self.expect(Token::Select)?;

        let distinct = self.match_token(&Token::Distinct);

        let columns = self.parse_select_columns()?;

        self.expect(Token::From)?;
        let from = self.parse_from()?;

        let joins = self.parse_joins()?;

        let where_clause = if self.match_token(&Token::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let group_by = if self.match_token(&Token::GroupBy) {
            self.parse_expr_list()?
        } else {
            vec![]
        };

        let having = if self.match_token(&Token::Having) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let order_by = if self.match_token(&Token::OrderBy) {
            self.parse_order_by_list()?
        } else {
            vec![]
        };

        let limit = if self.match_token(&Token::Limit) {
            match self.advance() {
                Token::Integer(n) => Some(n as usize),
                other => {
                    return Err(CsvqlError::UnexpectedToken {
                        expected: "integer".into(),
                        found: other.to_string(),
                    })
                }
            }
        } else {
            None
        };

        let offset = if self.match_token(&Token::Offset) {
            match self.advance() {
                Token::Integer(n) => Some(n as usize),
                other => {
                    return Err(CsvqlError::UnexpectedToken {
                        expected: "integer".into(),
                        found: other.to_string(),
                    })
                }
            }
        } else {
            None
        };

        Ok(SelectStatement {
            distinct,
            columns,
            from,
            joins,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
        })
    }

    // ── Select columns ───────────────────────────────────────

    fn parse_select_columns(&mut self) -> Result<Vec<SelectColumn>> {
        let mut cols = vec![self.parse_select_column()?];
        while self.match_token(&Token::Comma) {
            cols.push(self.parse_select_column()?);
        }
        Ok(cols)
    }

    fn parse_select_column(&mut self) -> Result<SelectColumn> {
        if self.check(&Token::Star) {
            self.advance();
            return Ok(SelectColumn::Wildcard);
        }

        let expr = self.parse_expr()?;
        let alias = if self.match_token(&Token::As) {
            Some(self.expect_identifier()?)
        } else if let Token::Identifier(_) = self.peek() {
            // implicit alias (without AS)
            if !self.check(&Token::From)
                && !self.check(&Token::Comma)
                && !self.check(&Token::Eof)
            {
                Some(self.expect_identifier()?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(SelectColumn::Expr { expr, alias })
    }

    // ── FROM ─────────────────────────────────────────────────

    fn parse_from(&mut self) -> Result<FromClause> {
        if self.check(&Token::LParen) {
            self.advance();
            if self.check(&Token::Select) {
                let sub = self.parse_select()?;
                self.expect(Token::RParen)?;
                let alias = if self.match_token(&Token::As) {
                    Some(self.expect_identifier()?)
                } else if let Token::Identifier(s) = self.peek() {
                    let upper = s.to_uppercase();
                    if !matches!(
                        upper.as_str(),
                        "WHERE" | "GROUP" | "ORDER" | "HAVING" | "LIMIT" | "OFFSET"
                            | "JOIN" | "INNER" | "LEFT" | "ON"
                    ) {
                        Some(self.expect_identifier()?)
                    } else {
                        None
                    }
                } else {
                    None
                };
                return Ok(FromClause {
                    source: TableRef::Subquery(Box::new(sub)),
                    alias,
                });
            }
            self.pos -= 1;
        }

        let table = self.expect_identifier()?;
        let alias = self.parse_optional_alias()?;
        Ok(FromClause {
            source: TableRef::File(table),
            alias,
        })
    }

    fn parse_optional_alias(&mut self) -> Result<Option<String>> {
        if self.match_token(&Token::As) {
            return Ok(Some(self.expect_identifier()?));
        }
        // Peek for an implicit alias — must not be a keyword
        if let Token::Identifier(s) = self.peek() {
            let upper = s.to_uppercase();
            if !matches!(
                upper.as_str(),
                "WHERE"
                    | "GROUP"
                    | "ORDER"
                    | "HAVING"
                    | "LIMIT"
                    | "OFFSET"
                    | "JOIN"
                    | "INNER"
                    | "LEFT"
                    | "ON"
            ) {
                return Ok(Some(self.expect_identifier()?));
            }
        }
        Ok(None)
    }

    // ── JOIN ─────────────────────────────────────────────────

    fn parse_joins(&mut self) -> Result<Vec<JoinClause>> {
        let mut joins = Vec::new();
        loop {
            let join_type = if self.match_token(&Token::Join) || self.check(&Token::Inner) {
                if self.check(&Token::Inner) {
                    self.advance(); // INNER
                    self.expect(Token::Join)?;
                }
                JoinType::Inner
            } else if self.match_token(&Token::LeftJoin) {
                JoinType::Left
            } else if self.check(&Token::Left) {
                self.advance();
                self.expect(Token::Join)?;
                JoinType::Left
            } else {
                break;
            };

            let table = self.expect_identifier()?;
            let alias = self.parse_optional_alias()?;
            self.expect(Token::On)?;
            let on = self.parse_expr()?;

            joins.push(JoinClause {
                join_type,
                table,
                alias,
                on,
            });
        }
        Ok(joins)
    }

    // ── ORDER BY ─────────────────────────────────────────────

    fn parse_order_by_list(&mut self) -> Result<Vec<OrderByItem>> {
        let mut items = vec![self.parse_order_by_item()?];
        while self.match_token(&Token::Comma) {
            items.push(self.parse_order_by_item()?);
        }
        Ok(items)
    }

    fn parse_order_by_item(&mut self) -> Result<OrderByItem> {
        let expr = self.parse_expr()?;
        let descending = if self.match_token(&Token::Desc) {
            true
        } else {
            self.match_token(&Token::Asc);
            false
        };
        Ok(OrderByItem { expr, descending })
    }

    // ── Expression list ──────────────────────────────────────

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>> {
        let mut exprs = vec![self.parse_expr()?];
        while self.match_token(&Token::Comma) {
            exprs.push(self.parse_expr()?);
        }
        Ok(exprs)
    }

    // ── Expression parsing (precedence climbing) ─────────────

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;
        while self.match_token(&Token::Or) {
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_not()?;
        while self.match_token(&Token::And) {
            let right = self.parse_not()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr> {
        if self.match_token(&Token::Not) {
            let operand = self.parse_not()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Not,
                operand: Box::new(operand),
            });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_addition()?;

        // IS [NOT] NULL
        if self.check(&Token::Is) {
            self.advance();
            let negated = self.match_token(&Token::Not);
            self.expect(Token::Null)?;
            return Ok(Expr::IsNull {
                expr: Box::new(left),
                negated,
            });
        }

        // [NOT] IN (...)
        let negated_prefix = if self.check(&Token::Not) {
            let saved = self.pos;
            self.advance();
            if self.check(&Token::In) || self.check(&Token::Between) || self.check(&Token::Like) {
                true
            } else {
                self.pos = saved;
                false
            }
        } else {
            false
        };

        if self.match_token(&Token::In) {
            self.expect(Token::LParen)?;
            if self.check(&Token::Select) {
                let sub = self.parse_select()?;
                self.expect(Token::RParen)?;
                return Ok(Expr::InList {
                    expr: Box::new(left),
                    list: vec![Expr::Subquery(Box::new(sub))],
                    negated: negated_prefix,
                });
            }
            let list = self.parse_expr_list()?;
            self.expect(Token::RParen)?;
            return Ok(Expr::InList {
                expr: Box::new(left),
                list,
                negated: negated_prefix,
            });
        }

        if self.match_token(&Token::Between) {
            let low = self.parse_addition()?;
            self.expect(Token::And)?;
            let high = self.parse_addition()?;
            return Ok(Expr::BetweenExpr {
                expr: Box::new(left),
                low: Box::new(low),
                high: Box::new(high),
                negated: negated_prefix,
            });
        }

        if self.match_token(&Token::Like) {
            let pattern = self.parse_addition()?;
            return Ok(Expr::LikeExpr {
                expr: Box::new(left),
                pattern: Box::new(pattern),
                negated: negated_prefix,
            });
        }

        // Standard comparison operators
        loop {
            let op = match self.peek() {
                Token::Eq => BinaryOperator::Eq,
                Token::Neq => BinaryOperator::Neq,
                Token::Lt => BinaryOperator::Lt,
                Token::Gt => BinaryOperator::Gt,
                Token::Lte => BinaryOperator::Lte,
                Token::Gte => BinaryOperator::Gte,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_addition(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Sub,
                Token::Concat => BinaryOperator::Concat,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                Token::Percent => BinaryOperator::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        if self.match_token(&Token::Minus) {
            let operand = self.parse_primary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Neg,
                operand: Box::new(operand),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match self.peek().clone() {
            Token::Integer(n) => {
                let n = n;
                self.advance();
                Ok(Expr::IntegerLiteral(n))
            }
            Token::Float(n) => {
                let n = n;
                self.advance();
                Ok(Expr::FloatLiteral(n))
            }
            Token::StringLiteral(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::StringLiteral(s))
            }
            Token::Boolean(b) => {
                let b = b;
                self.advance();
                Ok(Expr::BooleanLiteral(b))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            Token::Star => {
                self.advance();
                Ok(Expr::Star)
            }
            Token::Case => self.parse_case(),
            Token::LParen => {
                self.advance();
                if self.check(&Token::Select) {
                    let sub = self.parse_select()?;
                    self.expect(Token::RParen)?;
                    return Ok(Expr::Subquery(Box::new(sub)));
                }
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance();

                // Function call: name(...)
                if self.check(&Token::LParen) {
                    return self.parse_function_call(name);
                }

                // Qualified column: table.column
                if name.contains('.') {
                    if let Some(dot_pos) = name.find('.') {
                        let table = &name[..dot_pos];
                        let col = &name[dot_pos + 1..];
                        return Ok(Expr::Column {
                            table: Some(table.to_string()),
                            name: col.to_string(),
                        });
                    }
                }

                Ok(Expr::Column {
                    table: None,
                    name,
                })
            }
            other => Err(CsvqlError::ParseError(format!(
                "Unexpected token in expression: {other}"
            ))),
        }
    }

    fn parse_function_call(&mut self, name: String) -> Result<Expr> {
        self.expect(Token::LParen)?;

        let distinct = self.match_token(&Token::Distinct);

        let args = if self.check(&Token::RParen) {
            vec![]
        } else {
            self.parse_expr_list()?
        };

        self.expect(Token::RParen)?;

        Ok(Expr::Function {
            name: name.to_uppercase(),
            args,
            distinct,
        })
    }

    fn parse_case(&mut self) -> Result<Expr> {
        self.expect(Token::Case)?;

        let operand = if !self.check(&Token::When) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        let mut when_clauses = Vec::new();
        while self.match_token(&Token::When) {
            let condition = self.parse_expr()?;
            self.expect(Token::Then)?;
            let result = self.parse_expr()?;
            when_clauses.push((condition, result));
        }

        let else_clause = if self.match_token(&Token::Else) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect(Token::End)?;

        Ok(Expr::CaseExpr {
            operand,
            when_clauses,
            else_clause,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_sql(sql: &str) -> SelectStatement {
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_simple_select() {
        let stmt = parse_sql("SELECT name, age FROM people.csv");
        assert_eq!(stmt.columns.len(), 2);
        assert_eq!(stmt.from.table_name(), "people.csv");
        assert!(stmt.where_clause.is_none());
    }

    #[test]
    fn test_select_star() {
        let stmt = parse_sql("SELECT * FROM data.csv");
        assert!(matches!(stmt.columns[0], SelectColumn::Wildcard));
    }

    #[test]
    fn test_where_clause() {
        let stmt = parse_sql("SELECT name FROM people.csv WHERE age > 25");
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn test_group_by_with_aggregation() {
        let stmt =
            parse_sql("SELECT department, avg(salary) FROM emp.csv GROUP BY department");
        assert_eq!(stmt.group_by.len(), 1);
    }

    #[test]
    fn test_order_by_desc() {
        let stmt =
            parse_sql("SELECT name FROM people.csv ORDER BY age DESC");
        assert_eq!(stmt.order_by.len(), 1);
        assert!(stmt.order_by[0].descending);
    }

    #[test]
    fn test_join() {
        let stmt = parse_sql(
            "SELECT a.name, b.city FROM people.csv AS a JOIN cities.csv AS b ON a.city_id = b.id",
        );
        assert_eq!(stmt.joins.len(), 1);
        assert_eq!(stmt.joins[0].join_type, JoinType::Inner);
    }

    #[test]
    fn test_complex_query() {
        let stmt = parse_sql(
            "SELECT department, count(*), avg(salary) FROM employees.csv \
             WHERE age >= 21 AND active = TRUE \
             GROUP BY department \
             HAVING avg(salary) > 50000 \
             ORDER BY avg(salary) DESC \
             LIMIT 10",
        );
        assert!(stmt.distinct == false);
        assert_eq!(stmt.columns.len(), 3);
        assert!(stmt.where_clause.is_some());
        assert_eq!(stmt.group_by.len(), 1);
        assert!(stmt.having.is_some());
        assert_eq!(stmt.order_by.len(), 1);
        assert_eq!(stmt.limit, Some(10));
    }
}
