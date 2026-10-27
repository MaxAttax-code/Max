#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Using,
    Integer,
    Float,
    StringLiteral,
    Boolean,
    Identifier,
    Nil,
    As,
    To,
    Step,
    Of,
    In,
    With,
    Members,

    Declare,
    Resort,
    Say,
    Define,
    Enum,

    TaskType,
    Modifier,

    IfBegin,
    Then,
    Elif,
    Else,
    Maybe,
    ConcludeStatement,
    Return,
    Yield,

    While,
    Do,
    For,
    Foreach,

    Equals,
    Comparator,

    Plus,
    Minus,
    Multiply,
    Divide,
    /// Aka the percentage sign, '%'
    Modulus,

    PlusEqual,
    MinusEqual,
    MultiplyEqual,
    DivideEqual,
    ModulusEqual,

    Comma,
    Dot,
    Semicolon,
    Colon,
    /// Aka the Dollar Sign, '$'
    Dollar,
    Not,
    And,
    Or,
    NotEqualTo,

    LeftParenthesis,
    RightParenthesis,
    LeftBracket,
    RightBracket,
    LeftCurlyBracket,
    RightCurlyBracket,

    Lesser,
    LesserEqual,
    Greater,
    GreaterEqual,

    /// Aka an Equals sign followed by Greater than, '=>'
    EvaluateTo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    // Positional info
    pub line: u32,
    pub column: u32,
}

impl Token {
    pub fn new(kind: TokenKind, value: impl Into<String>, line: u32, column: u32) -> Self {
        Token {
            kind,
            value: value.into(),
            line,
            column
        }
    }
}