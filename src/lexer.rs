//! Lexer: turns Gloss source text into a flat list of tokens.
//!
//! The lexer is a single left-to-right scan. It skips whitespace and
//! `//` line comments, and it tracks the current line number so every
//! token (and every lex error) knows where it came from.

use crate::error::GlossError;

/// One piece of an interpolated string literal: plain text, or the
/// token list of an embedded `{expression}` (tokenized recursively by
/// the lexer; the parser turns it into an expression). `{{` and `}}`
/// in the source are escapes that produce literal braces in text.
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Text(String),
    Expr(Vec<Token>),
}

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
    Dot,

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
    Pipe,
    FatArrow,

    // Literals and names
    Number(f64),
    Str(String),
    InterpStr(Vec<StrPart>),
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
    Match,
    Try,
    Catch,
    Import,

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
            '.' => {
                tokens.push(Token::new(TokenKind::Dot, ".".to_string(), start_line));
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
                if peek_next(pos) == Some('>') {
                    tokens.push(Token::new(TokenKind::FatArrow, "=>".to_string(), start_line));
                    pos += 2;
                } else if peek_next(pos) == Some('=') {
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
                if peek_next(pos) == Some('>') {
                    tokens.push(Token::new(TokenKind::Pipe, "|>".to_string(), start_line));
                    pos += 2;
                } else if peek_next(pos) == Some('|') {
                    tokens.push(Token::new(TokenKind::OrOr, "||".to_string(), start_line));
                    pos += 2;
                } else {
                    return Err(GlossError::lex(
                        start_line,
                        "unexpected character '|' (did you mean '||' or '|>'?)",
                    ));
                }
            }

            // String literal with the escapes \n, \", \\ (plus \t and \r)
            // and interpolation: `{expr}` inserts a value, and `{{` /
            // `}}` produce literal braces. A string with at least one
            // interpolation becomes an InterpStr token; the embedded
            // expression source is tokenized recursively right here.
            '"' => {
                let mut text = String::new();
                let mut parts: Vec<StrPart> = Vec::new();
                let mut has_interp = false;
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
                            Some('n') => text.push('\n'),
                            Some('t') => text.push('\t'),
                            Some('r') => text.push('\r'),
                            Some('"') => text.push('"'),
                            Some('\\') => text.push('\\'),
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
                    } else if ch == '{' && peek_next(pos) == Some('{') {
                        // `{{` is an escaped literal brace.
                        text.push('{');
                        raw.push_str("{{");
                        pos += 2;
                    } else if ch == '{' {
                        // Start of an interpolation. Find the matching
                        // close brace, tokenize the inside, and keep the
                        // tokens as a StrPart for the parser.
                        let close = match find_interp_close(&chars, pos) {
                            Some(close) => close,
                            None => {
                                return Err(GlossError::lex(
                                    start_line,
                                    "unterminated interpolation in string (missing '}')",
                                ));
                            }
                        };
                        let expr_source: String = chars[pos + 1..close].iter().collect();
                        if expr_source.trim().is_empty() {
                            return Err(GlossError::lex(
                                start_line,
                                "empty interpolation in string ('{}')",
                            ));
                        }
                        for raw_char in &chars[pos..=close] {
                            raw.push(*raw_char);
                        }
                        let mut inner_tokens = tokenize(&expr_source)?;
                        for token in inner_tokens.iter_mut() {
                            // Inner tokens are numbered from line 1;
                            // shift them onto the string's line.
                            token.line += start_line - 1;
                        }
                        if !text.is_empty() {
                            parts.push(StrPart::Text(std::mem::take(&mut text)));
                        }
                        parts.push(StrPart::Expr(inner_tokens));
                        has_interp = true;
                        pos = close + 1;
                    } else if ch == '}' && peek_next(pos) == Some('}') {
                        // `}}` is an escaped literal brace.
                        text.push('}');
                        raw.push_str("}}");
                        pos += 2;
                    } else {
                        text.push(ch);
                        raw.push(ch);
                        pos += 1;
                    }
                }
                if !closed {
                    return Err(GlossError::lex(start_line, "unterminated string"));
                }
                if has_interp {
                    if !text.is_empty() {
                        parts.push(StrPart::Text(text));
                    }
                    tokens.push(Token::new(TokenKind::InterpStr(parts), raw, start_line));
                } else {
                    tokens.push(Token::new(TokenKind::Str(text), raw, start_line));
                }
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
                    "match" => TokenKind::Match,
                    "try" => TokenKind::Try,
                    "catch" => TokenKind::Catch,
                    "import" => TokenKind::Import,
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

/// Finds the `}` that closes the interpolation opened at `open`
/// (the index of a `{` character). Nested braces count towards the
/// depth, and braces inside string literals are skipped — including
/// interpolations nested inside those strings. Strings in Gloss do
/// not span lines, so a newline also means "not found".
fn find_interp_close(chars: &[char], open: usize) -> Option<usize> {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut i = open;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
                i += 1;
                continue;
            }
            if c == '{' {
                // A nested interpolation inside this inner string:
                // jump past its matching close brace.
                match find_interp_close(chars, i) {
                    Some(end) => {
                        i = end + 1;
                        continue;
                    }
                    None => return None,
                }
            }
            if c == '\n' {
                return None;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_string = true;
            }
            '{' => {
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            '\n' => return None,
            _ => {}
        }
        i += 1;
    }
    None
}
