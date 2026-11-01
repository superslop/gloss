//! Gloss — a small programming language implemented in Rust.
//!
//! Gloss is a layer on top of Rust's ecosystem that removes Rust's
//! user-facing pain points from the code you write: Gloss code has no
//! lifetimes, no borrow checker, and no trait bounds. You write dynamic,
//! garbage-collected-style code; the (safe, std-only) Rust interpreter
//! underneath handles the rest.
//!
//! This crate is the v0.2 tree-walk interpreter. Quick start:
//!
//! ```no_run
//! let output = gloss::run_source("print(1 + 2);").unwrap();
//! assert_eq!(output, "3\n");
//! ```

pub mod ast;
pub mod environment;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod value;

pub use error::GlossError;
pub use interpreter::Interpreter;
pub use value::Value;

/// Runs a chunk of Gloss source in a fresh interpreter and returns
/// everything the program printed.
///
/// This is the simplest entry point, used by the integration tests and
/// by anyone embedding Gloss.
pub fn run_source(source: &str) -> Result<String, GlossError> {
    let mut interpreter = Interpreter::new();
    interpreter.interpret(source)?;
    Ok(interpreter.take_output())
}

/// Runs Gloss source with a base directory for relative `import`
/// paths (the directory of the main script, when there is one).
pub fn run_source_in_dir(
    source: &str,
    base_dir: &std::path::Path,
) -> Result<String, GlossError> {
    let mut interpreter = Interpreter::new();
    interpreter.set_base_dir(base_dir.to_path_buf());
    interpreter.interpret(source)?;
    Ok(interpreter.take_output())
}
