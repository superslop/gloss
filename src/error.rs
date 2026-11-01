//! Error types for Gloss.
//!
//! Every error (lex, parse, or runtime) carries the source line where it
//! happened, so the CLI can point the user at the exact problem.

use std::fmt;

/// Which stage of the interpreter produced the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Lex,
    Parse,
    Runtime,
}

impl ErrorKind {
    pub fn label(&self) -> &'static str {
        match self {
            ErrorKind::Lex => "Lex error",
            ErrorKind::Parse => "Parse error",
            ErrorKind::Runtime => "Runtime error",
        }
    }
}

/// A single Gloss error with a line number and a human-readable message.
#[derive(Debug, Clone, PartialEq)]
pub struct GlossError {
    pub kind: ErrorKind,
    pub line: usize,
    pub message: String,
}

impl GlossError {
    pub fn lex(line: usize, message: impl Into<String>) -> GlossError {
        GlossError {
            kind: ErrorKind::Lex,
            line,
            message: message.into(),
        }
    }

    pub fn parse(line: usize, message: impl Into<String>) -> GlossError {
        GlossError {
            kind: ErrorKind::Parse,
            line,
            message: message.into(),
        }
    }

    pub fn runtime(line: usize, message: impl Into<String>) -> GlossError {
        GlossError {
            kind: ErrorKind::Runtime,
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for GlossError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}: {}",
            self.kind.label(),
            self.line,
            self.message
        )
    }
}

impl std::error::Error for GlossError {}
