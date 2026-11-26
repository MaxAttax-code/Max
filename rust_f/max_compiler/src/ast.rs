use crate::{error::{MaxDiagnostic, Severity}};

/// Where something starts in the source. Line and column both start at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32
}

impl Span {
    pub fn new(start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Span { start_line, start_column, end_line, end_column }
    }

    pub fn char((char_line, char_column): (u32, u32)) -> Self {
        Span { start_line: char_line, start_column: char_column, end_line: char_line, end_column: char_column }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start_line, self.start_column, other.end_line, other.end_column)
    }

    pub fn error(self, code: &'static str, message: impl Into<String>) -> MaxDiagnostic {
        MaxDiagnostic::new(Severity::Error, code, message.into(), self)
    }
    pub fn warning(self, code: &'static str, message: impl Into<String>) -> MaxDiagnostic {
        MaxDiagnostic::new(Severity::Warning, code, message.into(), self)
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

    pub fn new_alt(kind: ExpressionKind, start_line: u32, start_column: u32) -> Self {
        Self {
            kind,
            span: Span { start_line, start_column,end_line: 0, end_column: 0 }
        }
    }

    /// Creates with the full line and columns in a span
    pub fn new_altf(kind: ExpressionKind, start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Self {
            kind,
            span: Span { start_line, start_column, end_line, end_column }
        }
    }

    /// Creates an Expression with the Spans positions set to (0, 0, 0, 0).
    /// This is used for replacement values that act like the user declared an Expression,
    /// despite not being written in the document
    pub fn not_real(kind: ExpressionKind) -> Self {
        Self { kind, span: Span { start_line: 0, start_column: 0, end_line: 0, end_column: 0 } }
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
    This,

    Binary { left: Box<Expression>, op: BinaryOp, right: Box<Expression> },
    Comparison { left: Box<Expression>, op: ComparisonOp, right: Box<Expression> },
    Logical { left: Box<Expression>, op: LogicalOp, right: Box<Expression> },
    Unary { op: UnaryOp, operand: Box<Expression> },

    Index { target: Box<Expression>, index: Box<Expression> },
    MemberAccess { target: Box<Expression>, member: String },
    Callable { callee: Box<Expression>, arguments: Vec<Argument> },
    Lambda(Lambda),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lambda {
    pub parameters: Vec<Parameter>,
    pub body: LambdaBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    /// `(x) => x + 1`
    Expression(Box<Expression>),
    /// `(x) => do ... conclude`
    Block(Vec<Statement>),
}

/*
    -------------------
    ------ TASKS ------
    -------------------
*/

/// For task definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind { Function, Coroutine, Event }

impl std::fmt::Display for TaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskKind::Function => write!(f, "function"),
            TaskKind::Coroutine => write!(f, "coroutine"),
            TaskKind::Event => write!(f, "event")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle { Open, Monic }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability { Fixed }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModifierSet {
    pub lifecycle: Option<Lifecycle>,
    pub mutability: Option<Mutability>,
}

/// Seperate from lib.rs, requires no struct
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterKind {
    Required,
    Optional,
    Variadic
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub type_ref: Option<Spanned<TypeRef>>,
    pub name: Spanned<String>,
    pub kind: ParameterKind
}

/**
One argument in a call: `value` or `name = value`.

This replaces the plain `Expression` that `Call` used to hold. A Vec of these keeps the
**order the user wrote them in**, which matters twice: evaluation order (side effects
run as written) and positional filling (unnamed arguments fill in the order they appear).

```text
foo(b = 2, 1)   ->  [ Argument { name: Some("b"), value: 2 },
                      Argument { name: None,      value: 1 } ]
```
*/
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    /// Some(..) for `name = value`, otherwise None. None means the VM will match the argument with it's corresponding parameter by order
    pub name: Option<Spanned<String>>,
    pub value: Expression,
}

/*
    -------------------
    ------ MODELS -----
    -------------------
*/

/// Visibility modifiers on model fields or tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Accessible to all. The default
    Public,
    /// Accessible only in itself
    Private,
    /// Accessible in itself or inherited classes
    Protected
}

/// Provides attributes to a field or task within a model
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes {
    pub visibility: Option<Visibility>,
    pub is_static: bool
}

impl Attributes {
    /// A Markdown list for `documentation`
    pub fn markdown(&self) -> String {
        let visibility = match self.visibility {
            Some(Visibility::Public) => "public",
            Some(Visibility::Private) => "private",
            Some(Visibility::Protected) => "protected",
            None => "public (default)",
        };
        format!("- **Visibility:** {visibility}\n- **Static:** {}", self.is_static)
    }
}

pub trait ModelMemberKind {
    fn to_name(&self) -> String;
}

/// A field member declared in a model
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMember {
    pub attributes: Attributes,
    pub declaration: Declaration
}

impl ModelMemberKind for FieldMember {
    fn to_name(&self) -> String {
        self.declaration.name.clone()
    }
}

/// A task member declared in a model
#[derive(Debug, Clone, PartialEq)]
pub struct TaskMember {
    pub attributes: Attributes,
    pub definition: TaskDefinition
}

impl ModelMemberKind for TaskMember {
    fn to_name(&self) -> String {
        self.definition.name.clone()
    }
}

/// A general enum for the kinds of members that a model can hold
#[derive(Debug, Clone, PartialEq)]
pub enum ModelMember {
    Field(FieldMember),
    Task(TaskMember)
}

/// This Definition is defined for *both* objects and structs. The
/// differences between them are either handled during parsing/compiling
/// or at run-time.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectLikeDefinition {
    pub is_struct: bool,
    pub name: Spanned<String>,
    /// Span of the entirety of the class, starting from "members"
    pub span: Span,
    /// Object parameters are equal to task parameters
    pub parameters: Vec<Parameter>,
    pub members: Vec<ModelMember>
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelDefinition {
    ObjectLike(ObjectLikeDefinition),
}

/*
    -------------------
    ----- GENERAL -----
    -------------------
*/

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

/*
    -------------------
    --- STATEMENTS ----
    -------------------
*/

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
pub struct Case {
    /// The expression to match against the match statements result
    pub condition: Expression,
    /// The body. Keep in mind with declaring a value to match statement, all case bodies require a return statement.
    pub body: Vec<Statement>
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchStatement {
    /// What the cases match against
    pub condition: Expression,
    pub cases: Vec<Case>,
    /// Our 'else' case. Only holds a block of statements as it has no condition
    pub default: Option<Vec<Statement>>
}


#[derive(Debug, Clone, PartialEq)]
pub struct TaskDefinition {
    pub kind: TaskKind,
    pub name: String,
    pub is_native: bool,
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
    Match(MatchStatement),

    /// A bare task call such as `list.add(1)`
    Expression(Expression),

    Return(Option<Expression>),
    Yield(Option<Expression>),
    Emit(Expression),

    // Pretty special, they have no operands
    Break,
    Skip,

    TaskDefinition(TaskDefinition),
    Model(ModelDefinition),
    Enum(EnumDefinition),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span
}