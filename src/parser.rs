//! Parser: turns the token list into an AST.
//!
//! This is a recursive-descent parser: one method per grammar rule, with
//! expression precedence handled by a chain of methods — assignment
//! (lowest) down through logical, equality, comparison, term, factor,
//! unary, and call (highest) to primary expressions.

use crate::ast::{BinaryOp, Expr, LogicalOp, Stmt, UnaryOp};
use crate::error::GlossError;
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

/// Parses a whole Gloss program (a list of statements).
pub fn parse(tokens: Vec<Token>) -> Result<Vec<Stmt>, GlossError> {
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_program()
}

impl Parser {
    // ----- token helpers ------------------------------------------------

    fn current_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn current_line(&self) -> usize {
        self.tokens[self.pos].line
    }

    fn current_lexeme(&self) -> &str {
        &self.tokens[self.pos].lexeme
    }

    fn at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    /// Consumes the current token and returns it.
    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    /// Consumes the current token if it is `kind`; reports success.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.current_kind() == kind {
            self.advance();
            return true;
        }
        false
    }

    /// Consumes the current token if it is `kind`, else a parse error.
    fn expect(&mut self, kind: &TokenKind, message: &str) -> Result<Token, GlossError> {
        if self.current_kind() == kind {
            return Ok(self.advance());
        }
        Err(GlossError::parse(
            self.current_line(),
            format!("{message}, found '{}'", self.current_lexeme()),
        ))
    }

    /// Consumes an identifier token and returns its name and line.
    fn expect_identifier(&mut self, message: &str) -> Result<(String, usize), GlossError> {
        if let TokenKind::Identifier(name) = self.current_kind().clone() {
            let line = self.current_line();
            self.advance();
            return Ok((name, line));
        }
        Err(GlossError::parse(
            self.current_line(),
            format!("{message}, found '{}'", self.current_lexeme()),
        ))
    }

    // ----- program and statements ----------------------------------------

    fn parse_program(&mut self) -> Result<Vec<Stmt>, GlossError> {
        let mut statements = Vec::new();
        while !self.at_end() {
            // Tolerate stray empty statements (`;;`).
            if self.eat(&TokenKind::Semicolon) {
                continue;
            }
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Stmt, GlossError> {
        match self.current_kind() {
            TokenKind::LeftBrace => {
                let statements = self.parse_block()?;
                Ok(Stmt::Block(statements))
            }
            TokenKind::Let => self.parse_let(),
            TokenKind::Fn => self.parse_function_declaration(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for_in(),
            TokenKind::Return => self.parse_return(),
            _ => {
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::Semicolon, "expected ';' after expression")?;
                Ok(Stmt::Expression(expr))
            }
        }
    }

    /// Parses `{ statement* }` and returns the statements inside.
    fn parse_block(&mut self) -> Result<Vec<Stmt>, GlossError> {
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut statements = Vec::new();
        while !matches!(self.current_kind(), TokenKind::RightBrace) && !self.at_end() {
            if self.eat(&TokenKind::Semicolon) {
                continue;
            }
            statements.push(self.parse_statement()?);
        }
        self.expect(&TokenKind::RightBrace, "expected '}' after block")?;
        Ok(statements)
    }

    fn parse_let(&mut self) -> Result<Stmt, GlossError> {
        self.advance(); // 'let'
        let (name, _line) = self.expect_identifier("expected a variable name after 'let'")?;
        let initializer = if self.eat(&TokenKind::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon, "expected ';' after let declaration")?;
        Ok(Stmt::Let { name, initializer })
    }

    fn parse_function_declaration(&mut self) -> Result<Stmt, GlossError> {
        self.advance(); // 'fn'
        let (name, _line) = self.expect_identifier("expected a function name after 'fn'")?;
        let (params, body) = self.parse_function_tail()?;
        Ok(Stmt::Function { name, params, body })
    }

    /// Parses the shared `(params) { body }` part of a function.
    fn parse_function_tail(&mut self) -> Result<(Vec<String>, Vec<Stmt>), GlossError> {
        self.expect(&TokenKind::LeftParen, "expected '(' after function name")?;
        let mut params = Vec::new();
        if !matches!(self.current_kind(), TokenKind::RightParen) {
            loop {
                let (param, _line) = self.expect_identifier("expected a parameter name")?;
                params.push(param);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RightParen, "expected ')' after parameters")?;
        let body = self.parse_block()?;
        Ok((params, body))
    }

    fn parse_if(&mut self) -> Result<Stmt, GlossError> {
        self.advance(); // 'if'
        let condition = self.parse_expression()?;
        let then_branch = Box::new(self.parse_statement()?);
        let else_branch = if self.eat(&TokenKind::Else) {
            // `else if` chains fall out naturally: the else branch is
            // itself an if statement.
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, GlossError> {
        self.advance(); // 'while'
        let condition = self.parse_expression()?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::While { condition, body })
    }

    fn parse_for_in(&mut self) -> Result<Stmt, GlossError> {
        let for_token = self.advance(); // 'for'
        let (variable, _line) = self.expect_identifier("expected a loop variable after 'for'")?;
        self.expect(&TokenKind::In, "expected 'in' after the loop variable")?;
        let iterable = self.parse_expression()?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::ForIn {
            variable,
            iterable,
            body,
            line: for_token.line,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, GlossError> {
        let return_token = self.advance(); // 'return'
        let value = if matches!(self.current_kind(), TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::Semicolon, "expected ';' after return value")?;
        Ok(Stmt::Return {
            value,
            line: return_token.line,
        })
    }

    // ----- expressions -----------------------------------------------------

    fn parse_expression(&mut self) -> Result<Expr, GlossError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, GlossError> {
        let expr = self.parse_logical_or()?;
        if self.current_kind() == &TokenKind::Equal {
            let equals = self.advance();
            let value = self.parse_assignment()?; // right-associative
            return match expr {
                Expr::Variable { name, line } => Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                    line,
                }),
                Expr::Index {
                    object,
                    index,
                    line,
                } => Ok(Expr::IndexAssign {
                    object,
                    index,
                    value: Box::new(value),
                    line,
                }),
                _ => Err(GlossError::parse(
                    equals.line,
                    "invalid assignment target (expected a variable or an index)",
                )),
            };
        }
        Ok(expr)
    }

    fn parse_logical_or(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_logical_and()?;
        while self.eat(&TokenKind::OrOr) {
            let right = self.parse_logical_and()?;
            left = Expr::Logical {
                left: Box::new(left),
                op: LogicalOp::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_equality()?;
        while self.eat(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            left = Expr::Logical {
                left: Box::new(left),
                op: LogicalOp::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::EqualEqual => BinaryOp::Equal,
                TokenKind::BangEqual => BinaryOp::NotEqual,
                _ => break,
            };
            let operator = self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                line: operator.line,
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                _ => break,
            };
            let operator = self.advance();
            let right = self.parse_term()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                line: operator.line,
            };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => break,
            };
            let operator = self.advance();
            let right = self.parse_factor()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                line: operator.line,
            };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.current_kind() {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Modulo,
                _ => break,
            };
            let operator = self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                line: operator.line,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, GlossError> {
        match self.current_kind() {
            TokenKind::Bang => {
                let operator = self.advance();
                let right = self.parse_unary()?;
                return Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    right: Box::new(right),
                    line: operator.line,
                });
            }
            TokenKind::Minus => {
                let operator = self.advance();
                let right = self.parse_unary()?;
                return Ok(Expr::Unary {
                    op: UnaryOp::Negate,
                    right: Box::new(right),
                    line: operator.line,
                });
            }
            _ => {}
        }
        self.parse_call()
    }

    fn parse_call(&mut self) -> Result<Expr, GlossError> {
        let mut expr = self.parse_primary()?;
        loop {
            if matches!(self.current_kind(), TokenKind::LeftParen) {
                let paren = self.advance();
                let mut arguments = Vec::new();
                if !matches!(self.current_kind(), TokenKind::RightParen) {
                    loop {
                        arguments.push(self.parse_expression()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RightParen, "expected ')' after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    arguments,
                    line: paren.line,
                };
            } else if matches!(self.current_kind(), TokenKind::LeftBracket) {
                let bracket = self.advance();
                let index = self.parse_expression()?;
                self.expect(&TokenKind::RightBracket, "expected ']' after index")?;
                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index),
                    line: bracket.line,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, GlossError> {
        let token = self.tokens[self.pos].clone();
        match token.kind {
            TokenKind::Number(number) => {
                self.advance();
                Ok(Expr::Number(number))
            }
            TokenKind::Str(text) => {
                self.advance();
                Ok(Expr::Str(text))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Nil => {
                self.advance();
                Ok(Expr::Nil)
            }
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(Expr::Variable {
                    name,
                    line: token.line,
                })
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RightParen, "expected ')' after expression")?;
                Ok(expr)
            }
            TokenKind::LeftBracket => {
                // Array literal: [expr, expr, ...]
                self.advance();
                let mut elements = Vec::new();
                if !matches!(self.current_kind(), TokenKind::RightBracket) {
                    loop {
                        elements.push(self.parse_expression()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.current_kind(), TokenKind::RightBracket) {
                            break; // trailing comma
                        }
                    }
                }
                self.expect(&TokenKind::RightBracket, "expected ']' after array literal")?;
                Ok(Expr::ArrayLiteral(elements))
            }
            TokenKind::LeftBrace => {
                // Map literal: {"key": expr, ...} — keys are strings.
                self.advance();
                let mut entries = Vec::new();
                if !matches!(self.current_kind(), TokenKind::RightBrace) {
                    loop {
                        let key_token = self.tokens[self.pos].clone();
                        let key = match key_token.kind {
                            TokenKind::Str(text) => {
                                self.advance();
                                text
                            }
                            _ => {
                                return Err(GlossError::parse(
                                    key_token.line,
                                    "map keys must be strings",
                                ));
                            }
                        };
                        self.expect(&TokenKind::Colon, "expected ':' after map key")?;
                        let value = self.parse_expression()?;
                        entries.push((key, value));
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.current_kind(), TokenKind::RightBrace) {
                            break; // trailing comma
                        }
                    }
                }
                self.expect(&TokenKind::RightBrace, "expected '}' after map literal")?;
                Ok(Expr::MapLiteral(entries))
            }
            TokenKind::Fn => {
                // Function expression: fn (params) { body }
                // An optional name may follow `fn`; it is accepted and
                // ignored (the value is bound by the surrounding code).
                self.advance();
                if matches!(self.current_kind(), TokenKind::Identifier(_)) {
                    self.advance();
                }
                let (params, body) = self.parse_function_tail()?;
                Ok(Expr::Function { params, body })
            }
            _ => Err(GlossError::parse(
                token.line,
                format!("expected an expression, found '{}'", token.lexeme),
            )),
        }
    }
}
