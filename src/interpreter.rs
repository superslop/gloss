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
use std::path::PathBuf;
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, InterpPart, LogicalOp, Pattern, Stmt, UnaryOp};
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
    /// Directory that relative `import` paths (and REPL/file lookups)
    /// start from. Set by the CLI to the main script's directory.
    base_dir: Option<PathBuf>,
    /// Arguments exposed to Gloss code through the `args()` builtin.
    script_args: Vec<String>,
    /// Canonical paths of files currently being imported, used to
    /// detect import cycles.
    import_stack: Vec<PathBuf>,
}

impl Default for Interpreter {
    fn default() -> Interpreter {
        Interpreter::new()
    }
}

impl Interpreter {
    /// Creates an interpreter with a fresh global environment that has
    /// all the builtin functions (the standard library) defined.
    pub fn new() -> Interpreter {
        let globals = Environment::new_global();
        {
            let mut global_env = globals.borrow_mut();
            for builtin in Builtin::all() {
                global_env.define(builtin.name(), Value::Builtin(builtin));
            }
        }
        Interpreter {
            environment: globals.clone(),
            globals,
            output: String::new(),
            call_depth: 0,
            base_dir: None,
            script_args: Vec::new(),
            import_stack: Vec::new(),
        }
    }

    /// Sets the directory that relative `import` paths resolve
    /// against. The CLI calls this with the main script's directory.
    pub fn set_base_dir(&mut self, dir: PathBuf) {
        self.base_dir = Some(dir);
    }

    /// Sets the argument list returned by the `args()` builtin. The
    /// CLI passes everything after the script path.
    pub fn set_args(&mut self, args: Vec<String>) {
        self.script_args = args;
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
            Stmt::Try {
                body,
                catch_variable,
                catch_body,
            } => {
                let try_env = Environment::new_child(self.environment.clone());
                let result = self.execute_block(body, try_env);
                match result {
                    Err(Control::Error(error)) => {
                        // A runtime error lands here: run the catch
                        // block with the error message bound. `return`
                        // and errors raised inside catch itself are
                        // NOT caught — they keep propagating.
                        let catch_env = Environment::new_child(self.environment.clone());
                        catch_env
                            .borrow_mut()
                            .define(catch_variable, Value::Str(error.message.clone()));
                        self.execute_block(catch_body, catch_env)
                    }
                    other => other,
                }
            }
            Stmt::Import { path, line } => self.execute_import(path, *line),
        }
    }

    /// Executes an `import "path"` statement: the file is read, parsed,
    /// and run in the global environment, so everything it defines
    /// (functions, variables) becomes available afterwards. Relative
    /// paths resolve against the importing file's directory (tracked
    /// in `base_dir`); importing a file that is already being imported
    /// is a cycle and a runtime error.
    fn execute_import(&mut self, import_path: &str, line: usize) -> EvalResult<()> {
        let requested = PathBuf::from(import_path);
        let resolved = if requested.is_absolute() {
            requested
        } else if let Some(base) = &self.base_dir {
            base.join(requested)
        } else {
            requested
        };

        // Canonicalization both locates the file and gives a stable
        // identity for cycle detection.
        let canonical = match std::fs::canonicalize(&resolved) {
            Ok(path) => path,
            Err(error) => {
                return Err(GlossError::runtime(
                    line,
                    format!("cannot import '{import_path}': {error}"),
                )
                .into());
            }
        };
        if self.import_stack.contains(&canonical) {
            return Err(GlossError::runtime(
                line,
                format!("import cycle detected at '{import_path}'"),
            )
            .into());
        }
        let source = match std::fs::read_to_string(&canonical) {
            Ok(source) => source,
            Err(error) => {
                return Err(GlossError::runtime(
                    line,
                    format!("cannot import '{import_path}': {error}"),
                )
                .into());
            }
        };
        let tokens = lexer::tokenize(&source)?;
        let statements = parser::parse(tokens)?;

        // While the imported file runs, its own relative imports
        // resolve against its own directory, and its definitions go
        // into the global environment.
        self.import_stack.push(canonical.clone());
        let previous_base = self.base_dir.clone();
        if let Some(parent) = canonical.parent() {
            self.base_dir = Some(parent.to_path_buf());
        }
        let previous_env = std::mem::replace(&mut self.environment, self.globals.clone());
        let mut result = Ok(());
        for statement in &statements {
            if let Err(control) = self.execute(statement) {
                result = Err(control);
                break;
            }
        }
        self.environment = previous_env;
        self.base_dir = previous_base;
        self.import_stack.pop();
        result
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
            Expr::Interpolated(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        InterpPart::Text(text) => result.push_str(text),
                        InterpPart::Expr(inner) => {
                            let value = self.evaluate(inner)?;
                            result.push_str(&value.display());
                        }
                    }
                }
                Ok(Value::Str(result))
            }
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

            Expr::DotAccess {
                object,
                name,
                line,
            } => {
                let object_value = self.evaluate(object)?;
                match &object_value {
                    Value::Map(entries) => {
                        // Same rule as index reads: a missing key is nil.
                        let borrowed = entries.borrow();
                        Ok(borrowed.get(name).cloned().unwrap_or(Value::Nil))
                    }
                    other => Err(GlossError::runtime(
                        *line,
                        format!(
                            "cannot use '.' field access on a value of type {} (only maps have fields)",
                            other.type_name()
                        ),
                    )
                    .into()),
                }
            }

            Expr::DotAssign {
                object,
                name,
                value,
                line,
            } => {
                let object_value = self.evaluate(object)?;
                let new_value = self.evaluate(value)?;
                match &object_value {
                    Value::Map(entries) => {
                        entries.borrow_mut().insert(name.clone(), new_value.clone());
                        Ok(new_value)
                    }
                    other => Err(GlossError::runtime(
                        *line,
                        format!(
                            "cannot assign a field on a value of type {} (only maps have fields)",
                            other.type_name()
                        ),
                    )
                    .into()),
                }
            }

            Expr::Match {
                scrutinee,
                arms,
                line,
            } => {
                let subject = self.evaluate(scrutinee)?;
                for arm in arms {
                    let binding = match &arm.pattern {
                        Pattern::Wildcard => Some(None),
                        Pattern::Binding(name) => Some(Some((name.clone(), subject.clone()))),
                        Pattern::Nil => {
                            if matches!(subject, Value::Nil) {
                                Some(None)
                            } else {
                                None
                            }
                        }
                        Pattern::Bool(expected) => match &subject {
                            Value::Bool(actual) if actual == expected => Some(None),
                            _ => None,
                        },
                        Pattern::Number(expected) => match &subject {
                            Value::Number(actual) if actual == expected => Some(None),
                            _ => None,
                        },
                        Pattern::Str(expected) => match &subject {
                            Value::Str(actual) if actual == expected => Some(None),
                            _ => None,
                        },
                    };
                    if let Some(binding) = binding {
                        // A binding pattern's name is visible only
                        // while its arm body is evaluated.
                        if let Some((name, value)) = binding {
                            let arm_env = Environment::new_child(self.environment.clone());
                            arm_env.borrow_mut().define(&name, value);
                            let previous =
                                std::mem::replace(&mut self.environment, arm_env);
                            let result = self.evaluate(&arm.body);
                            self.environment = previous;
                            return result;
                        }
                        return self.evaluate(&arm.body);
                    }
                }
                Err(GlossError::runtime(
                    *line,
                    "no pattern matched in match expression (add a '_' arm to handle other values)",
                )
                .into())
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
                let total = function.params.len();
                let required = function
                    .params
                    .iter()
                    .filter(|param| param.default.is_none())
                    .count();
                if arguments.len() > total {
                    return Err(GlossError::runtime(
                        line,
                        format!(
                            "expected at most {total} argument(s) but got {}",
                            arguments.len()
                        ),
                    )
                    .into());
                }
                if arguments.len() < required {
                    return Err(GlossError::runtime(
                        line,
                        format!(
                            "expected at least {required} argument(s) but got {}",
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
                    for (param, argument) in
                        function.params.iter().zip(arguments.iter())
                    {
                        borrowed.define(&param.name, argument.clone());
                    }
                }
                // Omitted arguments fall back to their default
                // expression, evaluated at call time inside the call
                // environment (so a default can use earlier parameters
                // and the function's closure variables).
                for param in function.params.iter().skip(arguments.len()) {
                    let default = match &param.default {
                        Some(default) => default,
                        None => {
                            return Err(GlossError::runtime(
                                line,
                                format!("missing argument for parameter '{}'", param.name),
                            )
                            .into());
                        }
                    };
                    let previous =
                        std::mem::replace(&mut self.environment, call_env.clone());
                    let value = self.evaluate(default);
                    self.environment = previous;
                    let value = value?;
                    call_env.borrow_mut().define(&param.name, value);
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
        let name = builtin.name();
        match builtin {
            Builtin::Print => {
                let parts: Vec<String> =
                    arguments.iter().map(|value| value.display()).collect();
                self.output.push_str(&parts.join(" "));
                self.output.push('\n');
                Ok(Value::Nil)
            }
            Builtin::Len => {
                expect_count(name, &arguments, 1, line)?;
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
                expect_count(name, &arguments, 2, line)?;
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
            Builtin::Type => {
                expect_count(name, &arguments, 1, line)?;
                Ok(Value::Str(arguments[0].gloss_type().to_string()))
            }
            Builtin::Str => {
                expect_count(name, &arguments, 1, line)?;
                Ok(Value::Str(arguments[0].display()))
            }
            Builtin::Num => {
                expect_count(name, &arguments, 1, line)?;
                match &arguments[0] {
                    Value::Number(number) => Ok(Value::Number(*number)),
                    Value::Str(text) => match text.trim().parse::<f64>() {
                        Ok(number) => Ok(Value::Number(number)),
                        Err(_) => Err(GlossError::runtime(
                            line,
                            format!("num could not parse '{text}' as a number"),
                        )
                        .into()),
                    },
                    other => Err(GlossError::runtime(
                        line,
                        format!(
                            "num needs a string or a number, got {}",
                            other.type_name()
                        ),
                    )
                    .into()),
                }
            }
            Builtin::Upper => {
                expect_count(name, &arguments, 1, line)?;
                let text = string_arg(name, &arguments[0], line)?;
                Ok(Value::Str(text.to_uppercase()))
            }
            Builtin::Lower => {
                expect_count(name, &arguments, 1, line)?;
                let text = string_arg(name, &arguments[0], line)?;
                Ok(Value::Str(text.to_lowercase()))
            }
            Builtin::Trim => {
                expect_count(name, &arguments, 1, line)?;
                let text = string_arg(name, &arguments[0], line)?;
                Ok(Value::Str(text.trim().to_string()))
            }
            Builtin::Contains => {
                expect_count(name, &arguments, 2, line)?;
                match &arguments[0] {
                    Value::Str(haystack) => {
                        let needle = string_arg(name, &arguments[1], line)?;
                        Ok(Value::Bool(haystack.contains(&needle)))
                    }
                    Value::Array(items) => {
                        let found = items
                            .borrow()
                            .iter()
                            .any(|item| values_equal(item, &arguments[1]));
                        Ok(Value::Bool(found))
                    }
                    other => Err(GlossError::runtime(
                        line,
                        format!(
                            "contains needs a string or an array first, got {}",
                            other.type_name()
                        ),
                    )
                    .into()),
                }
            }
            Builtin::Split => {
                expect_count(name, &arguments, 2, line)?;
                let text = string_arg(name, &arguments[0], line)?;
                let separator = string_arg(name, &arguments[1], line)?;
                let pieces: Vec<Value> = text
                    .split(&separator)
                    .map(|piece| Value::Str(piece.to_string()))
                    .collect();
                Ok(Value::Array(Rc::new(RefCell::new(pieces))))
            }
            Builtin::Join => {
                expect_count(name, &arguments, 2, line)?;
                let items = array_items(name, &arguments[0], line)?;
                let separator = string_arg(name, &arguments[1], line)?;
                let parts: Vec<String> =
                    items.iter().map(|item| item.display()).collect();
                Ok(Value::Str(parts.join(&separator)))
            }
            Builtin::Abs => {
                expect_count(name, &arguments, 1, line)?;
                let number = number_arg(name, &arguments[0], line)?;
                Ok(Value::Number(number.abs()))
            }
            Builtin::Floor => {
                expect_count(name, &arguments, 1, line)?;
                let number = number_arg(name, &arguments[0], line)?;
                Ok(Value::Number(number.floor()))
            }
            Builtin::Ceil => {
                expect_count(name, &arguments, 1, line)?;
                let number = number_arg(name, &arguments[0], line)?;
                Ok(Value::Number(number.ceil()))
            }
            Builtin::Round => {
                expect_count(name, &arguments, 1, line)?;
                let number = number_arg(name, &arguments[0], line)?;
                Ok(Value::Number(number.round()))
            }
            Builtin::Sqrt => {
                expect_count(name, &arguments, 1, line)?;
                let number = number_arg(name, &arguments[0], line)?;
                if number < 0.0 {
                    return Err(GlossError::runtime(
                        line,
                        format!("sqrt needs a non-negative number, got {number}"),
                    )
                    .into());
                }
                Ok(Value::Number(number.sqrt()))
            }
            Builtin::Min => {
                expect_count(name, &arguments, 2, line)?;
                let a = number_arg(name, &arguments[0], line)?;
                let b = number_arg(name, &arguments[1], line)?;
                Ok(Value::Number(a.min(b)))
            }
            Builtin::Max => {
                expect_count(name, &arguments, 2, line)?;
                let a = number_arg(name, &arguments[0], line)?;
                let b = number_arg(name, &arguments[1], line)?;
                Ok(Value::Number(a.max(b)))
            }
            Builtin::Range => {
                if arguments.is_empty() || arguments.len() > 2 {
                    return Err(GlossError::runtime(
                        line,
                        format!(
                            "range expects 1 or 2 arguments but got {}",
                            arguments.len()
                        ),
                    )
                    .into());
                }
                let start = if arguments.len() == 1 {
                    0.0
                } else {
                    number_arg(name, &arguments[0], line)?
                };
                let end = if arguments.len() == 1 {
                    number_arg(name, &arguments[0], line)?
                } else {
                    number_arg(name, &arguments[1], line)?
                };
                if start.fract() != 0.0 || end.fract() != 0.0 {
                    return Err(GlossError::runtime(
                        line,
                        "range bounds must be whole numbers",
                    )
                    .into());
                }
                let mut values = Vec::new();
                let mut current = start as i64;
                while current < end as i64 {
                    values.push(Value::Number(current as f64));
                    current += 1;
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Builtin::Map => {
                expect_count(name, &arguments, 2, line)?;
                let items = array_items(name, &arguments[0], line)?;
                let function = arguments[1].clone();
                let mut results = Vec::new();
                for item in items {
                    let mapped = self.call_value(function.clone(), vec![item], line)?;
                    results.push(mapped);
                }
                Ok(Value::Array(Rc::new(RefCell::new(results))))
            }
            Builtin::Filter => {
                expect_count(name, &arguments, 2, line)?;
                let items = array_items(name, &arguments[0], line)?;
                let function = arguments[1].clone();
                let mut results = Vec::new();
                for item in items {
                    let keep =
                        self.call_value(function.clone(), vec![item.clone()], line)?;
                    if keep.is_truthy() {
                        results.push(item);
                    }
                }
                Ok(Value::Array(Rc::new(RefCell::new(results))))
            }
            Builtin::Reduce => {
                expect_count(name, &arguments, 3, line)?;
                let items = array_items(name, &arguments[0], line)?;
                let function = arguments[2].clone();
                let mut accumulator = arguments[1].clone();
                for item in items {
                    accumulator =
                        self.call_value(function.clone(), vec![accumulator, item], line)?;
                }
                Ok(accumulator)
            }
            Builtin::ReadFile => {
                expect_count(name, &arguments, 1, line)?;
                let path = string_arg(name, &arguments[0], line)?;
                match std::fs::read_to_string(&path) {
                    Ok(contents) => Ok(Value::Str(contents)),
                    Err(error) => Err(GlossError::runtime(
                        line,
                        format!("read_file could not read '{path}': {error}"),
                    )
                    .into()),
                }
            }
            Builtin::WriteFile => {
                expect_count(name, &arguments, 2, line)?;
                let path = string_arg(name, &arguments[0], line)?;
                // The content is written in its display form, so
                // numbers and other values work too, not just strings.
                let content = arguments[1].display();
                match std::fs::write(&path, content) {
                    Ok(()) => Ok(Value::Nil),
                    Err(error) => Err(GlossError::runtime(
                        line,
                        format!("write_file could not write '{path}': {error}"),
                    )
                    .into()),
                }
            }
            Builtin::Args => {
                expect_count(name, &arguments, 0, line)?;
                let values: Vec<Value> = self
                    .script_args
                    .iter()
                    .map(|arg| Value::Str(arg.clone()))
                    .collect();
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
        }
    }
}

/// Fails unless `arguments` has exactly `expected` entries.
fn expect_count(
    name: &str,
    arguments: &[Value],
    expected: usize,
    line: usize,
) -> EvalResult<()> {
    if arguments.len() != expected {
        return Err(GlossError::runtime(
            line,
            format!(
                "{name} expects {expected} argument(s) but got {}",
                arguments.len()
            ),
        )
        .into());
    }
    Ok(())
}

/// Extracts a number argument for a builtin, or a clear error.
fn number_arg(name: &str, value: &Value, line: usize) -> EvalResult<f64> {
    match value {
        Value::Number(number) => Ok(*number),
        other => Err(GlossError::runtime(
            line,
            format!("{name} needs a number, got {}", other.type_name()),
        )
        .into()),
    }
}

/// Extracts a string argument for a builtin, or a clear error.
fn string_arg(name: &str, value: &Value, line: usize) -> EvalResult<String> {
    match value {
        Value::Str(text) => Ok(text.clone()),
        other => Err(GlossError::runtime(
            line,
            format!("{name} needs a string, got {}", other.type_name()),
        )
        .into()),
    }
}

/// Copies the items of an array argument for a builtin (the copy lets
/// callbacks run without holding a borrow on the array).
fn array_items(name: &str, value: &Value, line: usize) -> EvalResult<Vec<Value>> {
    match value {
        Value::Array(items) => Ok(items.borrow().clone()),
        other => Err(GlossError::runtime(
            line,
            format!("{name} needs an array, got {}", other.type_name()),
        )
        .into()),
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
