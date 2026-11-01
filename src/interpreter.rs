//! Interpreter: walks the AST and executes it (a tree-walk interpreter).
//!
//! The interpreter owns three things:
//! - the global environment (kept between `interpret` calls, which is
//!   what makes the REPL keep its variables),
//! - the current environment (swapped as blocks and calls are entered
//!   and restored when they are left),
//! - an output buffer that `print` writes into, so tests and the CLI
//!   can read exactly what a program printed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, LogicalOp, Stmt, UnaryOp};
use crate::environment::Environment;
use crate::error::GlossError;
use crate::lexer;
use crate::parser;
use crate::value::{Builtin, FunctionValue, Value};

/// Internal control flow: executing code can either fail with an error
/// or bail out of a function with a `return` value.
enum Control {
    Error(GlossError),
    Return(Value),
}

impl From<GlossError> for Control {
    fn from(error: GlossError) -> Control {
        Control::Error(error)
    }
}

type EvalResult<T> = Result<T, Control>;

pub struct Interpreter {
    globals: Rc<RefCell<Environment>>,
    environment: Rc<RefCell<Environment>>,
    output: String,
    call_depth: usize,
}

impl Default for Interpreter {
    fn default() -> Interpreter {
        Interpreter::new()
    }
}

impl Interpreter {
    /// Creates an interpreter with a fresh global environment that has
    /// the builtin functions (`print`, `len`, `push`) defined.
    pub fn new() -> Interpreter {
        let globals = Environment::new_global();
        {
            let mut global_env = globals.borrow_mut();
            global_env.define("print", Value::Builtin(Builtin::Print));
            global_env.define("len", Value::Builtin(Builtin::Len));
            global_env.define("push", Value::Builtin(Builtin::Push));
        }
        Interpreter {
            environment: globals.clone(),
            globals,
            output: String::new(),
            call_depth: 0,
        }
    }

    /// Runs one chunk of Gloss source. The global environment and the
    /// output buffer are kept, so calling this repeatedly behaves like
    /// one long session (used by the REPL).
    pub fn interpret(&mut self, source: &str) -> Result<(), GlossError> {
        let tokens = lexer::tokenize(source)?;
        let statements = parser::parse(tokens)?;
        for statement in &statements {
            match self.execute(statement) {
                Ok(()) => {}
                Err(Control::Error(error)) => return Err(error),
                Err(Control::Return(_)) => {
                    // `execute` already rejects `return` outside a
                    // function, so this arm is unreachable in practice.
                    return Err(GlossError::runtime(1, "'return' outside of a function"));
                }
            }
        }
        Ok(())
    }

    /// Everything printed so far.
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Returns everything printed so far and clears the buffer.
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }

    /// The global environment (exposed for tests and debugging).
    pub fn globals(&self) -> Rc<RefCell<Environment>> {
        self.globals.clone()
    }

    // ----- statements ------------------------------------------------------

    fn execute(&mut self, statement: &Stmt) -> EvalResult<()> {
        match statement {
            Stmt::Expression(expr) => {
                self.evaluate(expr)?;
                Ok(())
            }
            Stmt::Let { name, initializer } => {
                let value = match initializer {
                    Some(expr) => self.evaluate(expr)?,
                    None => Value::Nil,
                };
                self.environment.borrow_mut().define(name, value);
                Ok(())
            }
            Stmt::Block(statements) => {
                let child = Environment::new_child(self.environment.clone());
                self.execute_block(statements, child)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.evaluate(condition)?;
                if condition_value.is_truthy() {
                    self.execute(then_branch)
                } else if let Some(else_branch) = else_branch {
                    self.execute(else_branch)
                } else {
                    Ok(())
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let condition_value = self.evaluate(condition)?;
                    if !condition_value.is_truthy() {
                        break;
                    }
                    self.execute(body)?;
                }
                Ok(())
            }
            Stmt::ForIn {
                variable,
                iterable,
                body,
                line,
            } => {
                let iterable_value = self.evaluate(iterable)?;
                let items = match &iterable_value {
                    Value::Array(items) => items.borrow().clone(),
                    other => {
                        return Err(GlossError::runtime(
                            *line,
                            format!(
                                "for-in can only iterate over an array, got {}",
                                other.type_name()
                            ),
                        )
                        .into());
                    }
                };
                // Each iteration gets a fresh scope holding the loop
                // variable, so the body cannot leak it outwards.
                for item in items {
                    let loop_env = Environment::new_child(self.environment.clone());
                    loop_env.borrow_mut().define(variable, item);
                    let previous = std::mem::replace(&mut self.environment, loop_env);
                    let result = self.execute(body);
                    self.environment = previous;
                    result?;
                }
                Ok(())
            }
            Stmt::Function { name, params, body } => {
                let function = FunctionValue {
                    params: params.clone(),
                    body: body.clone(),
                    closure: self.environment.clone(),
                };
                self.environment
                    .borrow_mut()
                    .define(name, Value::Function(Rc::new(function)));
                Ok(())
            }
            Stmt::Return { value, line } => {
                if self.call_depth == 0 {
                    return Err(GlossError::runtime(*line, "'return' outside of a function").into());
                }
                let return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => Value::Nil,
                };
                Err(Control::Return(return_value))
            }
        }
    }

    /// Runs statements inside `env`, restoring the previous environment
    /// afterwards — even when a `return` or an error cuts the block short.
    fn execute_block(
        &mut self,
        statements: &[Stmt],
        env: Rc<RefCell<Environment>>,
    ) -> EvalResult<()> {
        let previous = std::mem::replace(&mut self.environment, env);
        let mut result = Ok(());
        for statement in statements {
            match self.execute(statement) {
                Ok(()) => {}
                Err(control) => {
                    result = Err(control);
                    break;
                }
            }
        }
        self.environment = previous;
        result
    }

    // ----- expressions -------------------------------------------------------

    fn evaluate(&mut self, expr: &Expr) -> EvalResult<Value> {
        match expr {
            Expr::Number(number) => Ok(Value::Number(*number)),
            Expr::Str(text) => Ok(Value::Str(text.clone())),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::Nil => Ok(Value::Nil),

            Expr::Variable { name, line } => {
                let value = self.environment.borrow().get(name);
                match value {
                    Some(value) => Ok(value),
                    None => Err(GlossError::runtime(
                        *line,
                        format!("undefined variable '{name}'"),
                    )
                    .into()),
                }
            }

            Expr::Assign { name, value, line } => {
                let new_value = self.evaluate(value)?;
                let assigned = self.environment.borrow_mut().assign(name, new_value.clone());
                if assigned {
                    Ok(new_value)
                } else {
                    Err(GlossError::runtime(
                        *line,
                        format!("cannot assign to undefined variable '{name}' (declare it with 'let' first)"),
                    )
                    .into())
                }
            }

            Expr::IndexAssign {
                object,
                index,
                value,
                line,
            } => {
                let object_value = self.evaluate(object)?;
                let index_value = self.evaluate(index)?;
                let new_value = self.evaluate(value)?;
                self.assign_index(object_value, index_value, new_value.clone(), *line)?;
                Ok(new_value)
            }

            Expr::Binary {
                left,
                op,
                right,
                line,
            } => {
                let left_value = self.evaluate(left)?;
                let right_value = self.evaluate(right)?;
                self.evaluate_binary(left_value, *op, right_value, *line)
            }

            Expr::Logical { left, op, right } => {
                let left_value = self.evaluate(left)?;
                match op {
                    LogicalOp::And => {
                        if !left_value.is_truthy() {
                            return Ok(Value::Bool(false));
                        }
                        let right_value = self.evaluate(right)?;
                        Ok(Value::Bool(right_value.is_truthy()))
                    }
                    LogicalOp::Or => {
                        if left_value.is_truthy() {
                            return Ok(Value::Bool(true));
                        }
                        let right_value = self.evaluate(right)?;
                        Ok(Value::Bool(right_value.is_truthy()))
                    }
                }
            }

            Expr::Unary { op, right, line } => {
                let value = self.evaluate(right)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
                    UnaryOp::Negate => match value {
                        Value::Number(number) => Ok(Value::Number(-number)),
                        other => Err(GlossError::runtime(
                            *line,
                            format!("unary '-' needs a number, got {}", other.type_name()),
                        )
                        .into()),
                    },
                }
            }

            Expr::Call {
                callee,
                arguments,
                line,
            } => {
                let callee_value = self.evaluate(callee)?;
                let mut argument_values = Vec::new();
                for argument in arguments {
                    argument_values.push(self.evaluate(argument)?);
                }
                self.call_value(callee_value, argument_values, *line)
            }

            Expr::Index {
                object,
                index,
                line,
            } => {
                let object_value = self.evaluate(object)?;
                let index_value = self.evaluate(index)?;
                self.read_index(object_value, index_value, *line)
            }

            Expr::ArrayLiteral(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    values.push(self.evaluate(element)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }

            Expr::MapLiteral(entries) => {
                let mut map = HashMap::new();
                for (key, value_expr) in entries {
                    let value = self.evaluate(value_expr)?;
                    map.insert(key.clone(), value);
                }
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            }

            Expr::Function { params, body } => {
                let function = FunctionValue {
                    params: params.clone(),
                    body: body.clone(),
                    closure: self.environment.clone(),
                };
                Ok(Value::Function(Rc::new(function)))
            }
        }
    }

    // ----- operations -----------------------------------------------------

    fn evaluate_binary(
        &self,
        left: Value,
        op: BinaryOp,
        right: Value,
        line: usize,
    ) -> EvalResult<Value> {
        match op {
            BinaryOp::Equal => Ok(Value::Bool(values_equal(&left, &right))),
            BinaryOp::NotEqual => Ok(Value::Bool(!values_equal(&left, &right))),
            BinaryOp::Add => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::Str(_), _) | (_, Value::Str(_)) => {
                    // A string on either side means concatenation; the
                    // other side is converted with its display form.
                    Ok(Value::Str(format!(
                        "{}{}",
                        left.display(),
                        right.display()
                    )))
                }
                _ => Err(GlossError::runtime(
                    line,
                    format!(
                        "'+' needs two numbers or a string, got {} and {}",
                        left.type_name(),
                        right.type_name()
                    ),
                )
                .into()),
            },
            BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Modulo => {
                let (a, b) = number_operands(&left, &right, op, line)?;
                match op {
                    BinaryOp::Subtract => Ok(Value::Number(a - b)),
                    BinaryOp::Multiply => Ok(Value::Number(a * b)),
                    BinaryOp::Divide => {
                        if b == 0.0 {
                            Err(GlossError::runtime(line, "division by zero").into())
                        } else {
                            Ok(Value::Number(a / b))
                        }
                    }
                    BinaryOp::Modulo => {
                        if b == 0.0 {
                            Err(GlossError::runtime(line, "modulo by zero").into())
                        } else {
                            Ok(Value::Number(a % b))
                        }
                    }
                    _ => unreachable!(),
                }
            }
            BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual => {
                let result = match (&left, &right) {
                    (Value::Number(a), Value::Number(b)) => Some(compare(op, a, b)),
                    (Value::Str(a), Value::Str(b)) => Some(compare_str(op, a, b)),
                    _ => None,
                };
                match result {
                    Some(value) => Ok(Value::Bool(value)),
                    None => Err(GlossError::runtime(
                        line,
                        format!(
                            "comparison needs two numbers or two strings, got {} and {}",
                            left.type_name(),
                            right.type_name()
                        ),
                    )
                    .into()),
                }
            }
        }
    }

    fn read_index(&self, object: Value, index: Value, line: usize) -> EvalResult<Value> {
        match (&object, &index) {
            (Value::Array(items), Value::Number(_)) => {
                let borrowed = items.borrow();
                let position = array_position(&index, line, borrowed.len())?;
                Ok(borrowed[position].clone())
            }
            (Value::Map(entries), Value::Str(key)) => {
                // A missing key reads as nil instead of an error.
                let borrowed = entries.borrow();
                Ok(borrowed.get(key).cloned().unwrap_or(Value::Nil))
            }
            (Value::Array(_), other) => Err(GlossError::runtime(
                line,
                format!("array index must be a number, got {}", other.type_name()),
            )
            .into()),
            (Value::Map(_), other) => Err(GlossError::runtime(
                line,
                format!("map key must be a string, got {}", other.type_name()),
            )
            .into()),
            (other, _) => Err(GlossError::runtime(
                line,
                format!("cannot index a value of type {}", other.type_name()),
            )
            .into()),
        }
    }

    fn assign_index(
        &self,
        object: Value,
        index: Value,
        new_value: Value,
        line: usize,
    ) -> EvalResult<()> {
        match (&object, &index) {
            (Value::Array(items), Value::Number(_)) => {
                let mut borrowed = items.borrow_mut();
                let position = array_position(&index, line, borrowed.len())?;
                borrowed[position] = new_value;
                Ok(())
            }
            (Value::Map(entries), Value::Str(key)) => {
                entries.borrow_mut().insert(key.clone(), new_value);
                Ok(())
            }
            (Value::Array(_), other) => Err(GlossError::runtime(
                line,
                format!("array index must be a number, got {}", other.type_name()),
            )
            .into()),
            (Value::Map(_), other) => Err(GlossError::runtime(
                line,
                format!("map key must be a string, got {}", other.type_name()),
            )
            .into()),
            (other, _) => Err(GlossError::runtime(
                line,
                format!("cannot index-assign a value of type {}", other.type_name()),
            )
            .into()),
        }
    }

    fn call_value(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        line: usize,
    ) -> EvalResult<Value> {
        match callee {
            Value::Builtin(builtin) => self.call_builtin(builtin, arguments, line),
            Value::Function(function) => {
                if arguments.len() != function.params.len() {
                    return Err(GlossError::runtime(
                        line,
                        format!(
                            "expected {} argument(s) but got {}",
                            function.params.len(),
                            arguments.len()
                        ),
                    )
                    .into());
                }
                // The call environment is a child of the environment the
                // function was *defined* in — that is what makes it a
                // closure.
                let call_env = Environment::new_child(function.closure.clone());
                {
                    let mut borrowed = call_env.borrow_mut();
                    for (param, argument) in function.params.iter().zip(arguments.iter()) {
                        borrowed.define(param, argument.clone());
                    }
                }
                self.call_depth += 1;
                let result = self.execute_block(&function.body, call_env);
                self.call_depth -= 1;
                match result {
                    Ok(()) => Ok(Value::Nil),
                    Err(Control::Return(value)) => Ok(value),
                    Err(Control::Error(error)) => Err(Control::Error(error)),
                }
            }
            other => Err(GlossError::runtime(
                line,
                format!("can only call a function, got {}", other.type_name()),
            )
            .into()),
        }
    }

    fn call_builtin(
        &mut self,
        builtin: Builtin,
        arguments: Vec<Value>,
        line: usize,
    ) -> EvalResult<Value> {
        match builtin {
            Builtin::Print => {
                let parts: Vec<String> =
                    arguments.iter().map(|value| value.display()).collect();
                self.output.push_str(&parts.join(" "));
                self.output.push('\n');
                Ok(Value::Nil)
            }
            Builtin::Len => {
                if arguments.len() != 1 {
                    return Err(GlossError::runtime(
                        line,
                        format!("len expects 1 argument but got {}", arguments.len()),
                    )
                    .into());
                }
                let length = match &arguments[0] {
                    Value::Str(text) => text.chars().count(),
                    Value::Array(items) => items.borrow().len(),
                    Value::Map(entries) => entries.borrow().len(),
                    other => {
                        return Err(GlossError::runtime(
                            line,
                            format!(
                                "len needs a string, array, or map, got {}",
                                other.type_name()
                            ),
                        )
                        .into());
                    }
                };
                Ok(Value::Number(length as f64))
            }
            Builtin::Push => {
                if arguments.len() != 2 {
                    return Err(GlossError::runtime(
                        line,
                        format!("push expects 2 arguments but got {}", arguments.len()),
                    )
                    .into());
                }
                match &arguments[0] {
                    Value::Array(items) => {
                        items.borrow_mut().push(arguments[1].clone());
                        Ok(Value::Nil)
                    }
                    other => Err(GlossError::runtime(
                        line,
                        format!(
                            "push needs an array as its first argument, got {}",
                            other.type_name()
                        ),
                    )
                    .into()),
                }
            }
        }
    }
}

/// Extracts two numeric operands or produces a clear runtime error.
fn number_operands(
    left: &Value,
    right: &Value,
    op: BinaryOp,
    line: usize,
) -> EvalResult<(f64, f64)> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok((*a, *b)),
        _ => Err(GlossError::runtime(
            line,
            format!(
                "operator '{op:?}' needs two numbers, got {} and {}",
                left.type_name(),
                right.type_name()
            ),
        )
        .into()),
    }
}

/// Numeric comparison shared by `< <= > >=`.
fn compare(op: BinaryOp, a: &f64, b: &f64) -> bool {
    match op {
        BinaryOp::Less => a < b,
        BinaryOp::LessEqual => a <= b,
        BinaryOp::Greater => a > b,
        BinaryOp::GreaterEqual => a >= b,
        _ => unreachable!(),
    }
}

/// String comparison shared by `< <= > >=`.
fn compare_str(op: BinaryOp, a: &str, b: &str) -> bool {
    match op {
        BinaryOp::Less => a < b,
        BinaryOp::LessEqual => a <= b,
        BinaryOp::Greater => a > b,
        BinaryOp::GreaterEqual => a >= b,
        _ => unreachable!(),
    }
}

/// Validates an array index value and converts it to a position.
fn array_position(index: &Value, line: usize, length: usize) -> EvalResult<usize> {
    let number = match index {
        Value::Number(number) => *number,
        other => {
            return Err(GlossError::runtime(
                line,
                format!("array index must be a number, got {}", other.type_name()),
            )
            .into());
        }
    };
    if number.fract() != 0.0 {
        return Err(GlossError::runtime(
            line,
            format!("array index must be a whole number, got {number}"),
        )
        .into());
    }
    if number < 0.0 || number as usize >= length {
        return Err(GlossError::runtime(
            line,
            format!("array index {number} out of bounds (length {length})"),
        )
        .into());
    }
    Ok(number as usize)
}

/// Gloss equality: values of different types are never equal; arrays,
/// maps, and functions are equal only when they are the same object.
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
        (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b),
        (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
        (Value::Builtin(a), Value::Builtin(b)) => a == b,
        _ => false,
    }
}
