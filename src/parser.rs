//! Parser: turns the token list into an AST.
//!
//! This is a recursive-descent parser: one method per grammar rule, with
//! expression precedence handled by a chain of methods — assignment
//! (lowest) down through logical, equality, comparison, term, factor,
//! unary, and call (highest) to primary expressions.

use crate::ast::{BinaryOp, Expr, InterpPart, LogicalOp, MatchArm, Param, Pattern, Stmt, UnaryOp};
use crate::error::GlossError;
use crate::lexer::{StrPart, Token, TokenKind};

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
            TokenKind::Try => self.parse_try(),
            TokenKind::Import => self.parse_import(),
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
    /// Each parameter may carry a default: `fn greet(name = "world")`.
    fn parse_function_tail(&mut self) -> Result<(Vec<Param>, Vec<Stmt>), GlossError> {
        self.expect(&TokenKind::LeftParen, "expected '(' after function name")?;
        let mut params = Vec::new();
        if !matches!(self.current_kind(), TokenKind::RightParen) {
            loop {
                let (name, _line) = self.expect_identifier("expected a parameter name")?;
                let default = if self.eat(&TokenKind::Equal) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                params.push(Param { name, default });
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

    /// Parses `try { ... } catch err { ... }`.
    fn parse_try(&mut self) -> Result<Stmt, GlossError> {
        self.advance(); // 'try'
        let body = self.parse_block()?;
        self.expect(&TokenKind::Catch, "expected 'catch' after the try block")?;
        let (catch_variable, _line) =
            self.expect_identifier("expected an error variable name after 'catch'")?;
        let catch_body = self.parse_block()?;
        Ok(Stmt::Try {
            body,
            catch_variable,
            catch_body,
        })
    }

    /// Parses `import "path/to/file.gloss";`.
    fn parse_import(&mut self) -> Result<Stmt, GlossError> {
        let import_token = self.advance(); // 'import'
        let token = self.tokens[self.pos].clone();
        let path = match token.kind {
            TokenKind::Str(text) => {
                self.advance();
                text
            }
            _ => {
                return Err(GlossError::parse(
                    token.line,
                    format!(
                        "expected a string path after 'import', found '{}'",
                        token.lexeme
                    ),
                ));
            }
        };
        self.expect(&TokenKind::Semicolon, "expected ';' after import")?;
        Ok(Stmt::Import {
            path,
            line: import_token.line,
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
        self.parse_pipe()
    }

    /// The pipe operator `|>` is the lowest-precedence operator and
    /// left-associative: `x |> f` is `f(x)`, and `x |> f(a, b)` is
    /// `f(x, a, b)` — the piped value becomes the first argument.
    /// Because it binds loosest, parenthesize a piped result when it
    /// is used inside a larger expression. The transformation into a
    /// plain call happens here, at parse time.
    fn parse_pipe(&mut self) -> Result<Expr, GlossError> {
        let mut left = self.parse_assignment()?;
        while matches!(self.current_kind(), TokenKind::Pipe) {
            let pipe_token = self.advance();
            let target = self.parse_assignment()?;
            left = match target {
                Expr::Call {
                    callee,
                    arguments,
                    line,
                } => {
                    let mut new_arguments = vec![left];
                    new_arguments.extend(arguments);
                    Expr::Call {
                        callee,
                        arguments: new_arguments,
                        line,
                    }
                }
                Expr::Variable { .. }
                | Expr::Function { .. }
                | Expr::Index { .. }
                | Expr::DotAccess { .. } => Expr::Call {
                    callee: Box::new(target),
                    arguments: vec![left],
                    line: pipe_token.line,
                },
                _ => {
                    return Err(GlossError::parse(
                        pipe_token.line,
                        "the right side of '|>' must be a function or a function call",
                    ));
                }
            };
        }
        Ok(left)
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
                Expr::DotAccess {
                    object,
                    name,
                    line,
                } => Ok(Expr::DotAssign {
                    object,
                    name,
                    value: Box::new(value),
                    line,
                }),
                _ => Err(GlossError::parse(
                    equals.line,
                    "invalid assignment target (expected a variable, an index, or a map field)",
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
            } else if matches!(self.current_kind(), TokenKind::Dot) {
                let dot = self.advance();
                let (name, _line) =
                    self.expect_identifier("expected a field name after '.'")?;
                expr = Expr::DotAccess {
                    object: Box::new(expr),
                    name,
                    line: dot.line,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    /// Parses a match expression:
    /// `match value { pattern => body, ..., _ => fallback }`.
    fn parse_match(&mut self) -> Result<Expr, GlossError> {
        let match_token = self.advance(); // 'match'
        let scrutinee = self.parse_expression()?;
        self.expect(&TokenKind::LeftBrace, "expected '{' after the match value")?;
        let mut arms = Vec::new();
        while !matches!(self.current_kind(), TokenKind::RightBrace) {
            if self.at_end() {
                return Err(GlossError::parse(
                    self.current_line(),
                    "unterminated match expression (missing '}')",
                ));
            }
            let pattern = self.parse_pattern()?;
            self.expect(&TokenKind::FatArrow, "expected '=>' after a match pattern")?;
            let body = self.parse_expression()?;
            arms.push(MatchArm { pattern, body });
            // Arms are separated by commas; a trailing comma is fine.
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RightBrace, "expected '}' after match arms")?;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            line: match_token.line,
        })
    }

    /// Parses one match pattern: a literal, `_`, or a binding name.
    fn parse_pattern(&mut self) -> Result<Pattern, GlossError> {
        let token = self.tokens[self.pos].clone();
        match token.kind {
            TokenKind::Number(number) => {
                self.advance();
                Ok(Pattern::Number(number))
            }
            TokenKind::Minus => {
                self.advance();
                let number_token = self.tokens[self.pos].clone();
                match number_token.kind {
                    TokenKind::Number(number) => {
                        self.advance();
                        Ok(Pattern::Number(-number))
                    }
                    _ => Err(GlossError::parse(
                        number_token.line,
                        "expected a number after '-' in a match pattern",
                    )),
                }
            }
            TokenKind::Str(text) => {
                self.advance();
                Ok(Pattern::Str(text))
            }
            TokenKind::True => {
                self.advance();
                Ok(Pattern::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Pattern::Bool(false))
            }
            TokenKind::Nil => {
                self.advance();
                Ok(Pattern::Nil)
            }
            TokenKind::Identifier(name) => {
                self.advance();
                if name == "_" {
                    Ok(Pattern::Wildcard)
                } else {
                    Ok(Pattern::Binding(name))
                }
            }
            _ => Err(GlossError::parse(
                token.line,
                format!(
                    "expected a match pattern (a literal, '_', or a name), found '{}'",
                    token.lexeme
                ),
            )),
        }
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
            TokenKind::InterpStr(parts) => {
                self.advance();
                let mut interp_parts = Vec::new();
                for part in parts {
                    match part {
                        StrPart::Text(text) => interp_parts.push(InterpPart::Text(text)),
                        StrPart::Expr(inner_tokens) => {
                            // The lexer already tokenized the embedded
                            // expression; parse it with a sub-parser.
                            let mut sub_parser = Parser {
                                tokens: inner_tokens,
                                pos: 0,
                            };
                            let inner = sub_parser.parse_expression()?;
                            if !sub_parser.at_end() {
                                return Err(GlossError::parse(
                                    token.line,
                                    "unexpected tokens after the expression in string interpolation",
                                ));
                            }
                            interp_parts.push(InterpPart::Expr(Box::new(inner)));
                        }
                    }
                }
                Ok(Expr::Interpolated(interp_parts))
            }
            TokenKind::Match => self.parse_match(),
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
