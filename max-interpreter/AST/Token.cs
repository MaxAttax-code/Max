// Under license to the reader by the author of the Max Language®
// Relaying these files and/or distributing or publishing them without the author's consent may incur a fine

namespace Max
{
    /// <summary>
    /// An individual token that has been packaged by the Lexer
    /// </summary>
    /// <param name="type">The Tokentype that has been attached. Never null</param>
    /// <param name="value">The value that comes with it. Never null</param>
    public class Token(TokenType type, string value)
    {
        public TokenType Type { get; } = type;
        public string Value { get; } = value;

        public override string ToString()
        {
            return $"Token({Type}, {Value})";
        }
    }
}