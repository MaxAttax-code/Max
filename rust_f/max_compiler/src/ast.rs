use crate::{error::MaxError, token::Token};

/// Where something starts in the source. Line and column both start at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: u32,
    pub column: u32,
}

impl Span {
    pub fn new(line: u32, column: u32) -> Self {
        Span { line, column }
    }
}

impl From<&Token> for Span {
    fn from(token: &Token) -> Self {
        Span { line: token.line, column: token.column }
    }
}

impl Span {
    pub fn error(self, message: impl Into<String>) -> MaxError {
        MaxError::Compile { message: message.into(), line: self.line, column: self.column }
    }
}

/// Any value that also remembers where it came from
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

/// A type annotation as written in source. Enum names are not resolved here: the compiler checks `Named`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Int,
    Float,
    Str,
    Boolean,
    Coroutine,
    List(Option<Box<TypeRef>>),
    Dictionary(Option<Box<TypeRef>>, Option<Box<TypeRef>>),
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp { Add, Subtract, Multiply, Divide, Modulus }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp { Equal, NotEqual, Lesser, LesserEqual, Greater, GreaterEqual }

impl ComparisonOp {
    /// The string the VM's `Compare` instruction expects
    pub fn symbol(self) -> &'static str {
        match self {
            ComparisonOp::Equal => "==",
            ComparisonOp::NotEqual => "!=",
            ComparisonOp::Lesser => "<",
            ComparisonOp::LesserEqual => "<=",
            ComparisonOp::Greater => ">",
            ComparisonOp::GreaterEqual => ">=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp { Not, Negate }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp { And, Or }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp { Assign, Add, Subtract, Multiply, Divide, Modulus }

impl AssignOp {
    /// The string the VM's `Update` and `UpdateIndex` instructions expect
    pub fn symbol(self) -> &'static str {
        match self {
            AssignOp::Assign => "=",
            AssignOp::Add => "+=",
            AssignOp::Subtract => "-=",
            AssignOp::Multiply => "*=",
            AssignOp::Divide => "/=",
            AssignOp::Modulus => "%=",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub span: Span
}

impl Expression {
    pub fn new(kind: ExpressionKind, span: Span) -> Self {
        Self {
            kind, span
        }
    }

    pub fn new_alt(kind: ExpressionKind, line: u32, column: u32) -> Self {
        Self {
            kind,
            span: Span { line, column }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionKind {
    Integer(i64),
    Float(f64),
    Str(String),
    Boolean(bool),
    Possible,
    Nil,
    List(Vec<Expression>),
    Dictionary(Vec<(Expression, Expression)>),
    Variable(String),

    Binary { left: Box<Expression>, op: BinaryOp, right: Box<Expression> },
    Comparison { left: Box<Expression>, op: ComparisonOp, right: Box<Expression> },
    Logical { left: Box<Expression>, op: LogicalOp, right: Box<Expression> },
    Unary { op: UnaryOp, operand: Box<Expression> },

    Index { target: Box<Expression>, index: Box<Expression> },
    MemberAccess { target: Box<Expression>, member: String },
    Call { callee: Box<Expression>, arguments: Vec<Expression> },
    Lambda(Lambda),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lambda {
    pub parameters: Vec<String>,
    pub body: LambdaBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    /// `(x) => x + 1`
    Expression(Box<Expression>),
    /// `(x) => do ... conclude`
    Block(Vec<Statement>),
}

/// For task definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind { Function, Coroutine }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle { Open, Static }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability { Fixed }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModifierSet {
    pub lifecycle: Option<Lifecycle>,
    pub mutability: Option<Mutability>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub type_ref: Option<Spanned<TypeRef>>,
    pub name: String,
}

/// An enum member as parsed. The compiler adds the enum name and path when it builds the bytecode.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumMember {
    pub name: String,
    pub enumerable: i64,
    pub sub_members: Vec<EnumMember>,
    pub sub_enum_reference: Option<String>,
    pub span: Span
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub modifiers: ModifierSet,
    pub type_ref: Option<Spanned<TypeRef>>,
    pub value: Expression,
    pub resortable: Option<Expression>,
    pub name_span: Span
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub target: Expression,
    pub value: Expression,
    pub op: AssignOp,
}

/// Condition branch of an If Statement
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalBranch {
    pub condition: Expression,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStatement {
    pub first: ConditionalBranch,
    pub elifs: Vec<ConditionalBranch>,
    pub maybe_block: Vec<Statement>,
    pub else_block: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStatement {
    pub condition: Expression,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStatement {
    pub declaration: Declaration,
    pub limit: Expression,
    /// `Integer(1)` when the source has no `step`
    pub step: Expression,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForeachStatement {
    pub variables: Vec<String>,
    pub iterable: Expression,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskDefinition {
    pub kind: TaskKind,
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Spanned<TypeRef>>,
    pub body: Vec<Statement>,
    pub name_span: Span
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDefinition {
    pub identifier: String,
    pub members: Vec<EnumMember>,
    pub name_span: Span
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsingStatement {
    pub module: String,
    pub module_span: Span,
    /// Empty when the statement names only a module
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Using(UsingStatement),

    Declaration(Declaration),
    Assignment(Assignment),
    Say(Expression),

    If(IfStatement),

    While(WhileStatement),
    For(ForStatement),
    Foreach(ForeachStatement),

    /// A bare task call such as `list.add(1)`
    Expression(Expression),

    Return(Option<Expression>),
    Yield(Option<Expression>),

    TaskDefinition(TaskDefinition),
    Enum(EnumDefinition),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span
}