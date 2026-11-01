//! The `gloss` command-line program.
//!
//! Usage:
//! - `gloss run <file.gloss>` — execute a Gloss source file.
//! - `gloss` or `gloss repl` — start an interactive REPL.
//! - `gloss --help` / `gloss --version`.

use std::io::{BufRead, Write};
use std::process::ExitCode;

use gloss::Interpreter;

const HELP: &str = "\
Gloss 0.1.0 — a small programming language implemented in Rust.

USAGE:
    gloss run <file.gloss>    Execute a Gloss source file
    gloss repl                Start the interactive REPL
    gloss                     Start the interactive REPL
    gloss --help              Show this help text
    gloss --version           Show the version

In the REPL, variables and functions persist between lines, and an
error on one line does not end the session.
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    match arguments.first().map(|arg| arg.as_str()) {
        None => run_repl(),
        Some("repl") => run_repl(),
        Some("run") => match arguments.get(1) {
            Some(path) => run_file(path),
            None => {
                eprintln!("error: 'gloss run' needs a file path");
                eprintln!("usage: gloss run <file.gloss>");
                ExitCode::from(2)
            }
        },
        Some("--help") | Some("-h") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("--version") | Some("-V") => {
            println!("gloss {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown command '{other}'");
            eprintln!("usage: gloss run <file.gloss> | gloss repl | gloss --help");
            ExitCode::from(2)
        }
    }
}

/// Executes one Gloss file. Anything the program printed goes to
/// stdout; any error goes to stderr and the exit code is non-zero.
fn run_file(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error: could not read '{path}': {error}");
            return ExitCode::from(1);
        }
    };

    let mut interpreter = Interpreter::new();
    let result = interpreter.interpret(&source);

    // Show whatever the program printed, even if it then failed.
    print!("{}", interpreter.output());
    let _ = std::io::stdout().flush();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

/// The read-eval-print loop. One `Interpreter` is kept for the whole
/// session, so state persists between lines. Errors are reported and
/// the loop continues.
fn run_repl() -> ExitCode {
    println!("Gloss {} — type Gloss code, Ctrl-D to exit.", env!("CARGO_PKG_VERSION"));

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut interpreter = Interpreter::new();

    loop {
        print!("gloss> ");
        let _ = std::io::stdout().flush();

        match lines.next() {
            Some(Ok(line)) => {
                if line.trim().is_empty() {
                    continue;
                }
                match interpreter.interpret(&line) {
                    Ok(()) => {
                        print!("{}", interpreter.take_output());
                        let _ = std::io::stdout().flush();
                    }
                    Err(error) => {
                        // Flush anything printed before the error.
                        print!("{}", interpreter.take_output());
                        let _ = std::io::stdout().flush();
                        eprintln!("{error}");
                    }
                }
            }
            Some(Err(error)) => {
                eprintln!("error: could not read input: {error}");
                return ExitCode::from(1);
            }
            None => {
                // End of input (Ctrl-D).
                println!();
                return ExitCode::SUCCESS;
            }
        }
    }
}
