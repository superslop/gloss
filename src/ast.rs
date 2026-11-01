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

/// An expression: something that evaluates to a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Str(String),
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
        params: Vec<String>,
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
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Return {
        value: Option<Expr>,
        line: usize,
    },
}
