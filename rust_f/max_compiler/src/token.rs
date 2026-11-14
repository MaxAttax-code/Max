use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Specific to `Token::empty()`
    Empty,
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

    Decl,
    Resort,
    Say,
    Def,
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
    Emit,

    While,
    Do,
    For,
    Foreach,
    Match,
    Case,

    Break,
    Skip,

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
    Dollar,
    Not,
    And,
    Or,
    NotEqualTo,
    /// '...'
    Ellipsis,
    QuestionMark,

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
    pub span: Span
}

impl Token {
    pub fn new(kind: TokenKind, value: impl Into<String>, start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Token {
            kind,
            value: value.into(),
            span: Span::new(start_line, start_column, end_line, end_column)
        }
    }
    
    pub fn new_alt(kind: TokenKind, value: impl Into<String>, (start_line, start_column): (u32, u32), (end_line, end_column): (u32, u32)) -> Self {
        Token {
            kind,
            value: value.into(),
            span: Span::new(start_line, start_column, end_line, end_column)
        }
    }
    
    pub fn new_from(kind: TokenKind, value: impl Into<String>, span: Span) -> Self {
        Token {
            kind,
            value: value.into(),
            span
        }
    }

    /// Creates an empty, default token
    pub fn empty() -> Self {
        Token { kind: TokenKind::Empty, value: "nothing".to_owned(), span: Span::new(0, 0, 0, 0) }
    }
}