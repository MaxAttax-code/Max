namespace Max
{
    public interface IAstNode
    {
        
    }

    public interface IExpression : IAstNode
    {
    }

    public interface IStatement : IAstNode
    {
    }

    public enum TokenType
    {
        Using,

        Integer,
        Float,
        String,
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
        /// <summary>
        /// Aka the percentage sign, '%'
        /// </summary>
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
        /// <summary>
        /// Aka the Dollar Sign, '$'
        /// </summary>
        Subform,
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

        /// <summary>
        /// Aka an Equals sign followed by Greater than, '=>'
        /// </summary>
        EvaluateTo,
        LineComment,
        BlockCommentStart,
        BlockCommentEnd,
    }

    public enum UnaryOperator
    {
        Not,
        Negate
    }

    public enum LogicalOperator
    {
        And,
        Or
    }
}