//! Runtime values in Gloss.
//!
//! Gloss is dynamically typed: one `Value` enum covers every type a Gloss
//! program can hold. Arrays and maps live behind `Rc<RefCell<...>>` so
//! that assignment, function arguments, and closures all share the same
//! underlying collection — mutation through one reference is visible
//! through the others, which is the behaviour Gloss users expect.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Stmt;
use crate::environment::Environment;

/// Built-in functions provided by the interpreter itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Print,
    Len,
    Push,
}

impl Builtin {
    pub fn name(&self) -> &'static str {
        match self {
            Builtin::Print => "print",
            Builtin::Len => "len",
            Builtin::Push => "push",
        }
    }
}

/// A user-defined Gloss function: its code plus the environment it was
/// defined in (its closure).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionValue {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub closure: Rc<RefCell<Environment>>,
}

/// Any value a Gloss program can hold.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<String, Value>>>),
    Function(Rc<FunctionValue>),
    Builtin(Builtin),
}

impl Value {
    /// The Gloss type name, used in error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::Str(_) => "string",
            Value::Array(_) => "array",
            Value::Map(_) => "map",
            Value::Function(_) => "function",
            Value::Builtin(_) => "builtin function",
        }
    }

    /// Gloss truthiness: only `false` and `nil` are falsey.
    /// Everything else — including 0 and "" — is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(value) => *value,
            _ => true,
        }
    }

    /// How the value is shown by `print`. Integers print without a
    /// trailing `.0`; strings print raw (without quotes) at the top
    /// level and inside collections.
    pub fn display(&self) -> String {
        match self {
            Value::Nil => "nil".to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Number(number) => format_number(*number),
            Value::Str(text) => text.clone(),
            Value::Array(items) => {
                let parts: Vec<String> = items
                    .borrow()
                    .iter()
                    .map(|item| item.display())
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Map(entries) => {
                // Sort keys so map output is deterministic.
                let borrowed = entries.borrow();
                let mut keys: Vec<&String> = borrowed.keys().collect();
                keys.sort();
                let parts: Vec<String> = keys
                    .iter()
                    .map(|key| format!("\"{}\": {}", key, borrowed[*key].display()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Value::Function(_) => "<function>".to_string(),
            Value::Builtin(builtin) => format!("<builtin {}>", builtin.name()),
        }
    }
}

/// Formats a number the way Gloss prints it: whole numbers have no
/// decimal part (`3`, not `3.0`); fractional numbers use Rust's
/// default `f64` formatting (`3.5`).
pub fn format_number(number: f64) -> String {
    if number.is_finite() && number.fract() == 0.0 && number.abs() < 9_007_199_254_740_992.0 {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}
