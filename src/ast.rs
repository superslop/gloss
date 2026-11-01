//! Abstract syntax tree (AST): the parsed shape of a Gloss program.
//!
//! The parser produces these nodes; the interpreter walks them directly
//! (that is what "tree-walk interpreter" means). Every node that can fail
//! at runtime carries the line it came from, for error reporting.

/// Binary operators that need both operands evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

/// Logical operators, evaluated with short-circuiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

/// One part of an interpolated string: literal text, or an embedded
/// expression whose display form is inserted (`"sum = {1 + 2}"`).
#[derive(Debug, Clone, PartialEq)]
pub enum InterpPart {
    Text(String),
    Expr(Box<Expr>),
}

/// A match pattern: a literal to compare against, `_` (matches
/// anything), or a bare name (matches anything and binds the value).
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Number(f64),
    Str(String),
    Bool(bool),
    Nil,
    Wildcard,
    Binding(String),
}

/// One `pattern => body` arm of a match expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

/// One function parameter, with an optional default value expression
/// that is evaluated at call time when the argument is omitted.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
}

/// An expression: something that evaluates to a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Str(String),
    Interpolated(Vec<InterpPart>),
    Bool(bool),
    Nil,
    Variable {
        name: String,
        line: usize,
    },
    Assign {
        name: String,
        value: Box<Expr>,
        line: usize,
    },
    IndexAssign {
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
        line: usize,
    },
    /// Map field access by name: `user.name`, shorthand for
    /// `user["name"]`.
    DotAccess {
        object: Box<Expr>,
        name: String,
        line: usize,
    },
    DotAssign {
        object: Box<Expr>,
        name: String,
        value: Box<Expr>,
        line: usize,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        line: usize,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        line: usize,
    },
    Logical {
        left: Box<Expr>,
        op: LogicalOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        right: Box<Expr>,
        line: usize,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
        line: usize,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        line: usize,
    },
    ArrayLiteral(Vec<Expr>),
    MapLiteral(Vec<(String, Expr)>),
    /// An anonymous (or named) function value: `fn (a, b) { ... }`.
    Function {
        params: Vec<Param>,
        body: Vec<Stmt>,
    },
}

/// A statement: something that is executed for its effect.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expression(Expr),
    Let {
        name: String,
        initializer: Option<Expr>,
    },
    Block(Vec<Stmt>),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    ForIn {
        variable: String,
        iterable: Expr,
        body: Box<Stmt>,
        line: usize,
    },
    Function {
        name: String,
        params: Vec<Param>,
        body: Vec<Stmt>,
    },
    Return {
        value: Option<Expr>,
        line: usize,
    },
    /// `try { ... } catch err { ... }`: a runtime error in the try
    /// block runs the catch block with `err` bound to the message.
    Try {
        body: Vec<Stmt>,
        catch_variable: String,
        catch_body: Vec<Stmt>,
    },
    /// `import "path/to/file.gloss";` — executes another file in the
    /// current global environment.
    Import {
        path: String,
        line: usize,
    },
}
