//! Lexer: turns Gloss source text into a flat list of tokens.
//!
//! The lexer is a single left-to-right scan. It skips whitespace and
//! `//` line comments, and it tracks the current line number so every
//! token (and every lex error) knows where it came from.

use crate::error::GlossError;

/// The kind of a token. Keywords get their own variants so the parser
/// never has to compare identifier strings.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Single-character punctuation
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Semicolon,
    Colon,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,

    // Literals and names
    Number(f64),
    Str(String),
    Identifier(String),

    // Keywords
    Let,
    Fn,
    If,
    Else,
    While,
    For,
    In,
    Return,
    True,
    False,
    Nil,

    Eof,
}

/// One token: its kind, the exact source text, and its line.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
}

impl Token {
    fn new(kind: TokenKind, lexeme: String, line: usize) -> Token {
        Token { kind, lexeme, line }
    }
}

/// Scans `source` and returns all tokens, ending with a single `Eof`.
pub fn tokenize(source: &str) -> Result<Vec<Token>, GlossError> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut pos: usize = 0;
    let mut line: usize = 1;

    // Small helpers kept as closures over the character buffer.
    let peek = |pos: usize| -> Option<char> { chars.get(pos).copied() };
    let peek_next = |pos: usize| -> Option<char> { chars.get(pos + 1).copied() };

    while let Some(c) = peek(pos) {
        let start_line = line;
        match c {
            // Whitespace (newlines bump the line counter).
            ' ' | '\r' | '\t' => {
                pos += 1;
            }
            '\n' => {
                line += 1;
                pos += 1;
            }

            // Punctuation and operators.
            '(' => {
                tokens.push(Token::new(TokenKind::LeftParen, "(".to_string(), start_line));
                pos += 1;
            }
            ')' => {
                tokens.push(Token::new(TokenKind::RightParen, ")".to_string(), start_line));
                pos += 1;
            }
            '{' => {
                tokens.push(Token::new(TokenKind::LeftBrace, "{".to_string(), start_line));
                pos += 1;
            }
            '}' => {
                tokens.push(Token::new(TokenKind::RightBrace, "}".to_string(), start_line));
                pos += 1;
            }
            '[' => {
                tokens.push(Token::new(TokenKind::LeftBracket, "[".to_string(), start_line));
                pos += 1;
            }
            ']' => {
                tokens.push(Token::new(TokenKind::RightBracket, "]".to_string(), start_line));
                pos += 1;
            }
            ',' => {
                tokens.push(Token::new(TokenKind::Comma, ",".to_string(), start_line));
                pos += 1;
            }
            ';' => {
                tokens.push(Token::new(TokenKind::Semicolon, ";".to_string(), start_line));
                pos += 1;
            }
            ':' => {
                tokens.push(Token::new(TokenKind::Colon, ":".to_string(), start_line));
                pos += 1;
            }
            '+' => {
                tokens.push(Token::new(TokenKind::Plus, "+".to_string(), start_line));
                pos += 1;
            }
            '-' => {
                tokens.push(Token::new(TokenKind::Minus, "-".to_string(), start_line));
                pos += 1;
            }
            '*' => {
                tokens.push(Token::new(TokenKind::Star, "*".to_string(), start_line));
                pos += 1;
            }
            '%' => {
                tokens.push(Token::new(TokenKind::Percent, "%".to_string(), start_line));
                pos += 1;
            }
            '/' => {
                if peek_next(pos) == Some('/') {
                    // Line comment: skip until the end of the line.
                    while let Some(ch) = peek(pos) {
                        if ch == '\n' {
                            break;
                        }
                        pos += 1;
                    }
                } else {
                    tokens.push(Token::new(TokenKind::Slash, "/".to_string(), start_line));
                    pos += 1;
                }
            }
            '!' => {
                if peek_next(pos) == Some('=') {
                    tokens.push(Token::new(TokenKind::BangEqual, "!=".to_string(), start_line));
                    pos += 2;
                } else {
                    tokens.push(Token::new(TokenKind::Bang, "!".to_string(), start_line));
                    pos += 1;
                }
            }
            '=' => {
                if peek_next(pos) == Some('=') {
                    tokens.push(Token::new(TokenKind::EqualEqual, "==".to_string(), start_line));
                    pos += 2;
                } else {
                    tokens.push(Token::new(TokenKind::Equal, "=".to_string(), start_line));
                    pos += 1;
                }
            }
            '<' => {
                if peek_next(pos) == Some('=') {
                    tokens.push(Token::new(TokenKind::LessEqual, "<=".to_string(), start_line));
                    pos += 2;
                } else {
                    tokens.push(Token::new(TokenKind::Less, "<".to_string(), start_line));
                    pos += 1;
                }
            }
            '>' => {
                if peek_next(pos) == Some('=') {
                    tokens.push(Token::new(
                        TokenKind::GreaterEqual,
                        ">=".to_string(),
                        start_line,
                    ));
                    pos += 2;
                } else {
                    tokens.push(Token::new(TokenKind::Greater, ">".to_string(), start_line));
                    pos += 1;
                }
            }
            '&' => {
                if peek_next(pos) == Some('&') {
                    tokens.push(Token::new(TokenKind::AndAnd, "&&".to_string(), start_line));
                    pos += 2;
                } else {
                    return Err(GlossError::lex(
                        start_line,
                        "unexpected character '&' (did you mean '&&'?)",
                    ));
                }
            }
            '|' => {
                if peek_next(pos) == Some('|') {
                    tokens.push(Token::new(TokenKind::OrOr, "||".to_string(), start_line));
                    pos += 2;
                } else {
                    return Err(GlossError::lex(
                        start_line,
                        "unexpected character '|' (did you mean '||'?)",
                    ));
                }
            }

            // String literal with the escapes \n, \", \\ (plus \t and \r).
            '"' => {
                let mut value = String::new();
                let mut raw = String::from("\"");
                pos += 1; // opening quote
                let mut closed = false;
                while let Some(ch) = peek(pos) {
                    if ch == '"' {
                        raw.push('"');
                        pos += 1;
                        closed = true;
                        break;
                    }
                    if ch == '\n' {
                        return Err(GlossError::lex(start_line, "unterminated string"));
                    }
                    if ch == '\\' {
                        let escaped = peek_next(pos);
                        match escaped {
                            Some('n') => value.push('\n'),
                            Some('t') => value.push('\t'),
                            Some('r') => value.push('\r'),
                            Some('"') => value.push('"'),
                            Some('\\') => value.push('\\'),
                            _ => {
                                return Err(GlossError::lex(
                                    line,
                                    "invalid escape sequence in string",
                                ));
                            }
                        }
                        raw.push('\\');
                        if let Some(e) = escaped {
                            raw.push(e);
                        }
                        pos += 2;
                    } else {
                        value.push(ch);
                        raw.push(ch);
                        pos += 1;
                    }
                }
                if !closed {
                    return Err(GlossError::lex(start_line, "unterminated string"));
                }
                tokens.push(Token::new(TokenKind::Str(value), raw, start_line));
            }

            // Number literal: digits with an optional fractional part.
            '0'..='9' => {
                let mut text = String::new();
                while let Some(ch) = peek(pos) {
                    if ch.is_ascii_digit() {
                        text.push(ch);
                        pos += 1;
                    } else {
                        break;
                    }
                }
                if peek(pos) == Some('.') {
                    // Only consume the '.' if a digit follows it.
                    if let Some(next) = peek_next(pos) {
                        if next.is_ascii_digit() {
                            text.push('.');
                            pos += 1;
                            while let Some(ch) = peek(pos) {
                                if ch.is_ascii_digit() {
                                    text.push(ch);
                                    pos += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
                let number: f64 = text.parse().map_err(|_| {
                    GlossError::lex(start_line, format!("invalid number '{text}'"))
                })?;
                tokens.push(Token::new(TokenKind::Number(number), text, start_line));
            }

            // Identifier or keyword.
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut text = String::new();
                while let Some(ch) = peek(pos) {
                    if ch.is_alphanumeric() || ch == '_' {
                        text.push(ch);
                        pos += 1;
                    } else {
                        break;
                    }
                }
                let kind = match text.as_str() {
                    "let" => TokenKind::Let,
                    "fn" => TokenKind::Fn,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "for" => TokenKind::For,
                    "in" => TokenKind::In,
                    "return" => TokenKind::Return,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "nil" => TokenKind::Nil,
                    _ => TokenKind::Identifier(text.clone()),
                };
                tokens.push(Token::new(kind, text, start_line));
            }

            other => {
                return Err(GlossError::lex(
                    start_line,
                    format!("unexpected character '{other}'"),
                ));
            }
        }
    }

    tokens.push(Token::new(TokenKind::Eof, "".to_string(), line));
    Ok(tokens)
}
