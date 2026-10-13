// Under license to the reader by the author of the Max Language®
// Relaying these files and/or distributing or publishing them without the author's consent may incur a fine

namespace Max
{
    public class Lexer(string code)
    {
        private readonly string code = code;
        private int position = 0;

        /// <summary>
        /// Takes a look at the next character from the current position in the lexer
        /// </summary>
        /// <returns>The next character, or none if the code has ended</returns>
        private char? Peek()
        {
            if (position + 1 < code.Length)
            {
                return code[position + 1];
            }

            return null;
        }

        /// <summary>
        /// Takes a look at the character that is the amount given away from the current position in the lexer
        /// </summary>
        /// <returns>The found character, or none if the code has ended</returns>
        private char? Peek(int amount)
        {
            if (amount < 0) throw new MaxLogicException("Cannot call Peek(number) with a negative number");

            if (position + amount < code.Length)
            {
                return code[position + amount];
            }

            return null;
        }

        /// <summary>
        /// Checks if the next course of positions in the Lexer matches the substring given
        /// </summary>
        /// <param name="sub">The sub string to look for</param>
        /// <returns>If the next found string matches</returns>
        private bool Matching(string sub)
        {
            int charCount = sub.Length;

            for (int i = 0; i < sub.Length; i++)
            {
                int positionNow = position + i;
                char currChar = sub[i];

                if (positionNow < code.Length && code[positionNow] == currChar)
                {
                    continue;
                } else
                {
                    return false;
                }
            }

            // If that top return statement was not hit, then it's true!
            return true;
        }

        public List<Token> Tokenize()
        {
            List<Token> tokens = [];

            while (position < code.Length)
            {
                char character = code[position];

                // Ignore whitespace (and blackspaces I guess. Equality for all?)
                if (char.IsWhiteSpace(character))
                {
                    position++;
                    continue;
                }

                // Strings, bot not chars
                if (character == '"')
                {
                    tokens.Add(ReadString());
                    continue;
                }

                // Numbers (will add Floats later)
                if (char.IsDigit(character))
                {
                    tokens.Add(ReadNumber());
                    continue;
                }

                // Words / keywords / identifiers. You name it! Oh wait, no we do
                if (char.IsLetter(character) || character == '_')
                {
                    tokens.Add(ReadWord());
                    continue;
                }

                // Operators and symbols
                switch (character)
                {
                    case '=':
                        tokens.Add(ReadEquals());
                        break;

                    case '+':
                        tokens.Add(ReadArithmeticOperator(character));
                        break;

                    case '-':
                        tokens.Add(ReadArithmeticOperator(character));
                        break;

                    case '*':
                        tokens.Add(ReadArithmeticOperator(character));
                        break;

                    case '/':
                        Token? token = HandleSlash();

                        if (token != null)
                            tokens.Add(token);
                        break;

                    case '%':
                        tokens.Add(ReadArithmeticOperator(character));
                        break;

                    case '(':
                        tokens.Add(new Token(
                            TokenType.LeftParenthesis,
                            "("
                        ));
                        position++;
                        break;

                    case ')':
                        tokens.Add(new Token(
                            TokenType.RightParenthesis,
                            ")"
                        ));
                        position++;
                        break;

                    case '[':
                        tokens.Add(new Token(
                            TokenType.LeftBracket,
                            "["
                        ));
                        position++;
                        break;

                    case ']':
                        tokens.Add(new Token(
                            TokenType.RightBracket,
                            "]"
                        ));
                        position++;
                        break;

                    case '{':
                        tokens.Add(new Token(
                            TokenType.LeftCurlyBracket,
                            "{"
                        ));
                        position++;
                        break;

                    case '}':
                        tokens.Add(new Token(
                            TokenType.RightCurlyBracket,
                            "}"
                        ));
                        position++;
                        break;

                    case '<':
                        tokens.Add(ReadLesser());
                        break;

                    case '>':
                        tokens.Add(ReadGreater());
                        break;

                    case ',':
                        tokens.Add(new Token(
                            TokenType.Comma,
                            ","
                        ));
                        position++;
                        break;

                    case ':':
                        tokens.Add(new Token(
                            TokenType.Colon,
                            ":"
                        ));
                        position++;
                        break;

                    case ';':
                        tokens.Add(new Token(
                            TokenType.Semicolon,
                            ";"
                        ));
                        position++;
                        break;

                    case '!':
                        tokens.Add(ReadExclamation());
                        break;

                    case '.':
                        tokens.Add(new Token(
                            TokenType.Dot,
                            "."
                        ));
                        position++;
                        break;

                    default:
                        Console.WriteLine(
                            $"Unknown character: {character}"
                        );

                        position++;
                        break;
                }
            }

            return tokens;
        }
        
        private Token ReadString()
            {
                position++; // Skip opening '"'

                string value = "";

                while (position < code.Length && code[position] != '"')
                {
                    value += code[position];
                    position++;
                }

                if (position >= code.Length)
                {
                    throw new MaxSyntaxException(
                        "Unterminated string literal."
                    );
                }

                position++; // Skip closing '"', they're like twins!

                return new Token(TokenType.String, value);
            }

        private Token ReadNumber()
        {
            // We encapsulate the number in string. It will become an actual number later
            string number = "";

            bool isFloat = false;

            while (position < code.Length && (char.IsDigit(code[position]) || code[position] == '.'))
            {
                if (code[position] == '.' && !isFloat)
                {
                    isFloat = true;
                } else if (code[position] == '.' && isFloat)
                {
                    throw new MaxSyntaxException("A float cannot have more than one decimal point");
                }

                number += code[position];
                position++;
            }

            if (isFloat)
            {
                return new Token(TokenType.Float, number);
            } else
            {
                return new Token(TokenType.Integer, number);
            }
        }

        private Token ReadWord()
        {
            string word = "";

            // Allows letters, digits, or underscores
            while (position < code.Length && (char.IsLetterOrDigit(code[position]) || code[position] == '_'))
            {
                word += code[position];
                position++;
            }

            return word switch
            {
                "using" => new Token(TokenType.Using, word),

                "declare" => new Token(TokenType.Declare, word),
                "resort" => new Token(TokenType.Resort, word),
                "say" => new Token(TokenType.Say, word),
                "define" => new Token(TokenType.Define, word),
                
                "function" or "coroutine" => new Token(TokenType.TaskType, word),
                "static" or "open" or "fixed" => new Token(TokenType.Modifier, word),

                "true" => new Token(TokenType.Boolean, word),
                "false" => new Token(TokenType.Boolean, word),
                "possible" => new Token(TokenType.Boolean, word),

                "if" => new Token(TokenType.IfBegin, word),
                "then" => new Token(TokenType.Then, word),
                "elif" => new Token(TokenType.Elif, word),
                "else" => new Token(TokenType.Else, word),
                "maybe" => new Token(TokenType.Maybe, word),
                "conclude" => new Token(TokenType.ConcludeStatement, word),
                "return" => new Token(TokenType.Return, word),
                "yield" => new Token(TokenType.Yield, word),

                "while" => new Token(TokenType.While, word),
                "for" => new Token(TokenType.For, word),
                "foreach" => new Token(TokenType.Foreach, word),
                "do" => new Token(TokenType.Do, word),

                "nil" => new Token(TokenType.Nil, word),
                "as" => new Token(TokenType.As, word),
                "not" => new Token(TokenType.Not, word),
                "and" => new Token(TokenType.And, word),
                "or" => new Token(TokenType.Or, word),
                "to" => new Token(TokenType.To, word),
                "step" => new Token(TokenType.Step, word),
                "of" => new Token(TokenType.Of, word),
                "in" => new Token(TokenType.In, word),
                "with" => new Token(TokenType.With, word),

                _ => new Token(TokenType.Identifier, word)
            };
        }

        private Token ReadEquals()
        {
            // Don't worry, Peek doesn't mean what you think it means irl, just look at it's definition
            if (Peek() == '=')
            {
                position += 2;
                return new Token(TokenType.Comparator, "==");
            } else if (Peek() == '>')
            {
                position += 2;
                return new Token(TokenType.EvaluateTo, "=>");
            }

            position += 1;
            return new Token(TokenType.Equals, "=");
        }

        private Token ReadLesser()
        {
            if (Peek() == '=')
            {
                position += 2;
                return new Token(TokenType.LesserEqual, "<=");
            }

            position++;
            return new Token(TokenType.Lesser, "<");
        }

        private Token ReadGreater()
        {
            if (Peek() == '=')
            {
                position += 2;
                return new Token(TokenType.GreaterEqual, ">=");
            }

            position++;
            return new Token(TokenType.Greater, ">");
        }

        private Token ReadArithmeticOperator(char operatorChar)
        {
            if (Peek() == '=')
            {
                position += 2;
                return operatorChar switch
                {
                    '+' => new Token(TokenType.PlusEqual, "+="),
                    '-' => new Token(TokenType.MinusEqual, "-="),
                    '*' => new Token(TokenType.MultiplyEqual, "*="),
                    '/' => new Token(TokenType.DivideEqual, "/="),
                    '%' => new Token(TokenType.ModulusEqual, "%="),

                    _ => throw new MaxSyntaxException($"Unknown arithmetic operator: {operatorChar}")
                };
            }

            position++;
            return operatorChar switch
            {
                '+' => new Token(TokenType.Plus, "+"),
                '-' => new Token(TokenType.Minus, "-"),
                '*' => new Token(TokenType.Multiply, "*"),
                '/' => new Token(TokenType.Divide, "/"),
                '%' => new Token(TokenType.Modulus, "%"),
                
                _ => throw new MaxSyntaxException($"Unknown arithmetic operator: {operatorChar}")
            };
        }

        private Token? HandleSlash() // Our '/' handler
        {
            // Step by step we do this
            // 1. Check for multiline comment: ///
            if (Peek(1) == '/' && Peek(2) == '/')
            {
                SkipBlockComment();
                return null;
            }

            // 2. Check for single-line comment: //
            if (Peek(1) == '/')
            {
                SkipLineComment();
                return null;
            }

            // 3. Regular division operator: / or /=
            Token symbol = ReadArithmeticOperator(code[position]);
            return symbol;
        }

        private void SkipLineComment()
        {
            // If it's a newline, we stop
            while (position < code.Length && code[position] != '\n')
            {
                position++;
            }
        }

        private void SkipBlockComment()
        {
            position += 3;
            while (position < code.Length && !Matching(@"\\\"))
            {
                position++;
            }

            if (position < code.Length) position += 3; // We skip the closing block
            // Error otherwise
            else throw new MaxEndFileException("Unterminated block comment, please end a block comment with '\\\\\\'");
        }

        private Token ReadExclamation()
        {
            if (Peek() == '=')
            {
                position += 2;
                return new Token(TokenType.NotEqualTo, "!=");
            }

            position++;
            return new Token(TokenType.Not, "!");
        }
    }
}