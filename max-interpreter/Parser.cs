using System.Globalization;

namespace Max;

public class Parser(List<Token> tokens)
{
    private readonly List<Token> tokens = tokens;
    private int position = 0;

    #region Common Helpers

    /// <summary>
    /// Simply returns the current token from the tokens list.
    /// </summary>
    /// <returns>The current token, else null.</returns>
    private Token? CurrentToken()
    {
        if (position < tokens.Count)
        {
            return tokens[position];
        }

        return null;
    }

    /// <summary>
    /// Advance the token list by one element.
    /// </summary>
    private void Advance()
    {
        position++;
    }

    /// <summary>
    /// Advance the token list by one element, and return the was-current token.
    /// </summary>
    /// <returns>The was-current token before advancing.</returns>
    /// <exception cref="MaxEndFileException">When the file unexpectedly ends</exception>
    private Token Consume()
    {
        Token? token = CurrentToken() ?? throw new MaxEndFileException("Unexpected end of file.");
        
        position++;
        return token;
    }

    /// <summary>
    /// Consumes the current token, but also allows type checking to be automatically incorporated
    /// </summary>
    /// <param name="expectedType">The type the consumed token must be</param>
    /// <returns>The consumed token</returns>
    /// <exception cref="MaxSyntaxException"></exception>
    private Token Consume(TokenType expectedType) 
    {
        Token token = Consume();

        if (!Match(token, expectedType))
        {
            throw new MaxSyntaxException($"Expected the token that has been consumed before position {position} to be of TokenType {expectedType}");
        }

        return token;
    }

    /// <summary>
    /// Checks if the Current Token in the Parser matches the given Type
    /// </summary>
    /// <param name="type">The Type to match with</param>
    /// <returns>Whether the type matches or not</returns>
    /// <exception cref="MaxNilException"></exception>
    private bool Match(TokenType type)
    {
        return CurrentToken()?.Type == type;
    }

    /// <summary>
    /// Checks if a given Tokens type and given Type are matching
    /// </summary>
    /// <param name="token">The Token to check</param>
    /// <param name="type">The Type to match with</param>
    /// <returns>Whether it matches or not</returns>
    private static bool Match(Token token, TokenType type)
    {
        return token.Type == type;
    }

    /// <summary>
    /// Peeks inside the token list, returning the next Token from the current index on the list.
    /// </summary>
    /// <returns>The next token, or null if it doesn't exist</returns>
    private Token? Peek()
    {
        // Our check logic
        if (position + 1 > tokens.Count)
            return null;
        return tokens[position + 1];
    }

    /// <summary>
    /// Peeks inside the token list by the amount given, starting from the current position.
    /// </summary>
    /// <param name="amount">The amount to move from the current position</param>
    /// <returns>The token found at the position, or null if it doesn't exist</returns>
    private Token? Peek(int amount)
    {
        // Can't have backwards peeking!
        if (amount < 0) throw new Exception("Cannot Peek backwards into the tokens list via negative integers!");

        if (position + amount > tokens.Count)
            return null;
        return tokens[position + amount];
    }

    /// <summary>
    /// Checks if the given tokentype is a kind of keyword. If so, it returns true.
    /// </summary>
    /// <param name="type">The Tokentype to inspect</param>
    /// <returns></returns>
    private static bool IsContextualKeyword(TokenType type)
    {
        return type switch
        {
            // ALL the keywords we want!
            // We must do this so that identifier names in postfix expressions are valid.
            // E.g. we should allow something like object.conclude() or object.resort, as they are not referencing the actual keywords!
            TokenType.Declare or
            TokenType.Resort or
            TokenType.Say or
            TokenType.Define or
            TokenType.TaskType or
            TokenType.TaskType or
            TokenType.Boolean or
            TokenType.Boolean or
            TokenType.Boolean or
            TokenType.IfBegin or
            TokenType.Then or
            TokenType.Elif or
            TokenType.Else or
            TokenType.Maybe or
            TokenType.ConcludeStatement or
            TokenType.Return or
            TokenType.Yield or
            TokenType.While or
            TokenType.For or
            TokenType.Foreach or
            TokenType.Do or
            TokenType.Nil or
            TokenType.As or
            TokenType.Not or
            TokenType.And or
            TokenType.Or or
            TokenType.To or
            TokenType.Step or
            TokenType.Of or
            TokenType.In or
            TokenType.With => true,
            
            _ => false
        };
    }

    #endregion

    #region Steps to parsing an expression (starts from bottom, climbs up to top then descends to bottom again)

    /// <summary>
    /// Returns a Node based on what the current token is.
    /// </summary>
    /// <returns>The Node specific to the token type</returns>
    /// <exception cref="MaxSyntaxException">If a parenthesis is missing, or a valid token is missing.</exception>
    private IExpression Factor()
    {
        Token token = Consume();

        // Keep in mind that everything is parsed with Postfix Expressions syntactically, but not semantically
        switch (token.Type)
        {
            case TokenType.Nil:
                return ParsePostfixExpression(new NilLiteral());

            case TokenType.Integer:
                return ParsePostfixExpression(new IntegerLiteral(int.Parse(token.Value)));

            case TokenType.Float:
                // Yes you must parse it through a double first
                return ParsePostfixExpression(new FloatLiteral(
                    double.Parse(token.Value, CultureInfo.InvariantCulture)
                ));

            // Allows the parsing of Negative Numbers
            case TokenType.Minus:
                return new UnaryExpression(
                    UnaryOperator.Negate,
                    Factor() // Parse the current number factor
                );

            case TokenType.String:
                return ParsePostfixExpression(new StringLiteral(token.Value));

            case TokenType.Boolean:
                return ParsePostfixExpression(ConvertBoolean(token.Value));

            case TokenType.Identifier:
                // Parsing a postfix expression allows member accessing within identifiers
                // Keep in mind this means it returns what's the result of the expression, but not the original identifier
                return ParsePostfixExpression(ParseIdentifier(token));

            case TokenType.LeftParenthesis:
                {
                    // Check if it's a lambda
                    if (IsLambdaHead())
                    {
                        return ParseLambda();
                    }

                    // Currently just wraps an expression within the parenthesis
                    IExpression result = Expression();

                    Consume(TokenType.RightParenthesis);

                    return ParsePostfixExpression(result);
                }

            case TokenType.LeftBracket:
                return ParsePostfixExpression(ParseList());

            case TokenType.LeftCurlyBracket:
                return ParsePostfixExpression(ParseDictionary());

            default:
                throw new MaxSyntaxException($"Expected an expression at position {position}, got {token}");
            
        }
    }

    /// <summary>
    /// Parses any Unary Operations along the way
    /// </summary>
    /// <seealso cref="Factor()"/>
    /// <returns>The Unary Expression, else the Factor</returns>
    private IExpression ParseUnary()
    {
        if (Match(TokenType.Not))
        {
            Advance();

            // Check for parse unary again
            IExpression operand = ParseExpression();

            return new UnaryExpression(UnaryOperator.Not, operand);
        }

        return Factor();
    }

    /// <summary>
    /// Evaluates the terms starting from the Factor of the current token. Evaluates multiplication and division if possible.
    /// </summary>
    /// <seealso cref="ParseUnary()"/>
    /// <returns>A Binary Expression, or the result of the Factor</returns>
    /// <exception cref="MaxArithmeticException">If an error appeared during Arithemetics</exception>
    private IExpression Term()
    {
        IExpression result = ParseUnary();

        while (CurrentToken() != null)
        {
            Token curr = CurrentToken()!;

            if (Match(curr, TokenType.Multiply))
            {
                Advance();

                try
                {
                    IExpression rightHand = Factor();

                    result = new BinaryExpression(
                        result,
                        "*",
                        rightHand
                    );
                }
                catch
                {
                    throw new MaxArithmeticException("Error trying to do multiplication!");
                }
            }
            else if (Match(curr, TokenType.Divide))
            {
                Advance();

                try
                {
                    IExpression rightHand = Factor();

                    result = new BinaryExpression(
                        result,
                        "/",
                        rightHand
                    );
                }
                catch
                {
                    throw new MaxArithmeticException("Error trying to do division!");
                }
            }
            // Our new and beautiful Modulus! (v1.0.5)
            else if (Match(curr, TokenType.Modulus))
            {
                Advance();

                try
                {
                    IExpression rightHand = Factor();

                    result = new BinaryExpression(
                        result,
                        "%",
                        rightHand
                    );
                }
                catch
                {
                    throw new MaxArithmeticException("Error trying to do division!");
                }
            }

            else break;
        }

        return result;
    }

    /// <summary>
    /// Outputs the Term of the current token. Evaluates addition and subtraction as well.
    /// </summary>
    /// <seealso cref="Term()"/>
    /// <returns>A Binary Expression or the result from the Term</returns>
    /// <exception cref="MaxArithmeticException">If an error appeared during Arithmetics</exception>
    private IExpression Expression()
    {
        IExpression result = Term();

        while (CurrentToken() != null)
        {
            Token curr = CurrentToken()!;

            if (Match(curr, TokenType.Plus))
            {
                Advance();

                try
                {
                    IExpression rightHand = Term();

                    result = new BinaryExpression(
                        result,
                        "+",
                        rightHand
                    );
                }
                catch
                {
                    throw new MaxArithmeticException("Error trying to do addition!");
                }
            }
            else if (Match(curr, TokenType.Minus))
            {
                Advance();

                try
                {
                    IExpression rightHand = Term();

                    result = new BinaryExpression(
                        result,
                        "-",
                        rightHand
                    );
                }
                catch
                {
                    throw new MaxArithmeticException("Error trying to do subtraction!");
                }
            }

            else break;
        }

        return result;
    }

    /// <summary>
    /// Outputs the comparable value from a token, via Boolean or Expression means.
    /// </summary>
    /// <seealso cref="Expression()"/>
    /// <returns>The Comparable value</returns>
    /// <exception cref="MaxComparisonException">If no comparable token was in the current token</exception>
    private IExpression ParseComparison()
    {
        IExpression result = Expression();

        while (CurrentToken() != null)
        {
            Token token = CurrentToken()!;

            string? comparator = token.Type switch
            {
                TokenType.Comparator => "==",
                TokenType.NotEqualTo => "!=",
                TokenType.Lesser => "<",
                TokenType.LesserEqual => "<=",
                TokenType.Greater => ">",
                TokenType.GreaterEqual => ">=",
                
                _ => null
            };

            if (comparator == null)
            {
                break;
            }

            Advance();

            IExpression rightHand = Expression();

            result = new ComparisonExpression(
                result,
                comparator,
                rightHand
            );
        }

        return result;
    }

    /// <summary>
    /// Allows the parsing of an 'and' expression
    /// </summary>
    /// <seealso cref="ParseComparison()"/>
    /// <returns>A Logical Expression, else simply moves on with the Comparison Operand result</returns>
    private IExpression ParseAnd()
    {
        IExpression left = ParseComparison();

        while (Match(TokenType.And))
        {
            Advance();

            IExpression right = ParseComparison();

            left = new LogicalExpression(
                left,
                LogicalOperator.And,
                right
            );
        }

        return left;
    }

    /// <summary>
    /// Allows the parsing of an 'or' expression. Keep in mind this calls ParseAnd to get it's result
    /// </summary>
    /// <seealso cref="ParseAnd()"/>
    /// <returns>A Logical Expression, else the expression gotten from ParseAnd()</returns>
    private IExpression ParseOr()
    {
        IExpression left = ParseAnd();

        while (Match(TokenType.Or))
        {
            Advance();

            IExpression right = ParseAnd();

            left = new LogicalExpression(
                left,
                LogicalOperator.Or,
                right
            );
        }

        return left;
    }

    /// <summary>
    /// Calls the series of functions in order to parse an expression. Starting from ParseOr.
    /// </summary>
    /// <seealso cref="ParseOr()"/>
    /// <returns>The result of the ParseOr, or an Or logic expression</returns>
    private IExpression ParseExpression()
    {
        return ParseOr();
    }

    #endregion

    #region Parsing a List

    private ListLiteral ParseList()
    {
        var items = new List<IExpression>();

        // When the list is empty (i.e. [])
        if (Match(TokenType.RightBracket))
        {
            Advance();
            return new ListLiteral(items);
        }

        do
        {
            // Add to list
            items.Add(ParseExpression());

            // If it ain't a comma or right bracket, it's outta town!
            if (Match(TokenType.Comma))
            {
                Advance();
            }
            else if (!Match(TokenType.RightBracket))
            {
                throw new MaxSyntaxException(
                    "List items can only have a comma or ending bracket after them"
                );
            }
        }
        // As long as it isn't the right brack
        while (!Match(TokenType.RightBracket));

        Advance(); // Past the right bracket ']'

        return new ListLiteral(items);
    }

    #endregion

    #region Parsing a Dictionary

    private DictionaryLiteral ParseDictionary()
    {
        List<(IExpression Key, IExpression Value)> entries = [];
        
        // When the dictionary is empty (i.e. {})
        if (Match(TokenType.RightCurlyBracket))
        {
            Advance();
            return new DictionaryLiteral(entries);
        }

        do
        {
            // Get the key
            var key = ParseExpression();

            Consume(TokenType.Colon); // Past the required colon

            // Get the value
            var value = ParseExpression();

            // Now add
            entries.Add((key, value));

            // As usual the checks
            if (Match(TokenType.Comma))
            {
                Advance();
            }
            else if (!Match(TokenType.RightCurlyBracket))
            {
                throw new MaxSyntaxException(
                    "List items can only have a comma or ending bracket after them"
                );
            }
        }
        // As long as it isn't the right curly bracket
        while (!Match(TokenType.RightCurlyBracket));

        Advance(); // Past the right curly bracket '}'

        return new DictionaryLiteral(entries);
    }

    #endregion

    #region Parsing Declaration

    private DeclarationStatement ParseDeclaration()
    {
        // Skip past 'declare'
        Advance();

        // Get the modifier, or just the variable name
        Token token = Consume();

        ModifierSet modifiers = new();

        // For each modifier found
        while (Match(token, TokenType.Modifier))
        {
            string modifierName = token.Value;

            switch (modifierName)
            {
                case "fixed":
                {
                    if (modifiers.MutabilityModifier != null)
                        throw new MaxVariableException("Cannot have more than one mutability modifier on a variable declaration.");

                    modifiers.MutabilityModifier = MutabilityModifier.Fixed;
                    break;
                }

                case "open":
                {
                    if (modifiers.LifecycleModifier != null)
                        throw new MaxVariableException("Cannot have more than one lifecycle modifier on a variable declaration.");

                    modifiers.LifecycleModifier = LifecycleModifier.Open;
                    break;
                }

                case "static":
                {
                    if (modifiers.LifecycleModifier != null)
                        throw new MaxVariableException("Cannot have more than one lifecycle modifier on a variable declaration.");

                    modifiers.LifecycleModifier = LifecycleModifier.Static;
                    break;
                }
            }

            token = Consume();
        }

        if (!Match(token, TokenType.Identifier))
        {
            throw new MaxVariableException("Expected a variable name after 'declare'");
        }

        MaxType? type = null;

        // Check for the as keyword, in case of type assignment
        if (Match(TokenType.As))
        {
            Advance();

            Token type_token = CurrentToken()!;

            if (!Match(type_token, TokenType.Identifier))
            {
                throw new MaxTypeException("Expected a type name after 'as'");
            }

            string typeName = Consume().Value; // Get the name of the type

            if (TypeRegistry.Max_TypeNames.TryGetValue(typeName, out MaxType? assignedType))
            {
                // Target iterable types for further type annotations
                // Will return the normal assignedType anyway if it fails
                type = AttemptIterableTyping(assignedType);
            } else
            {
                throw new MaxTypeException("Type assignment to a variable must be a real type");
            }
        }

        // Now we should be at the equals sign
        Token equals = CurrentToken()!;

        if (!Match(equals, TokenType.Equals))
        {
            throw new MaxSyntaxException("Expected an assigning operator after the variable name in a declare statement");
        }

        Advance();

        // Now just parse whatever else comes
        IExpression expression = ParseExpression();

        IExpression? resortable = null;

        if (CurrentToken() == null)
        {
            return new DeclarationStatement(token.Value, modifiers, type, expression, resortable);
        }

        Token comma = CurrentToken()!;

        // Start checking for resort
        if (Match(comma, TokenType.Comma))
        {
            Advance();
            if (!Match(TokenType.Resort))
            {
                throw new MaxSyntaxException("Expected a resort keyword after a comma during a declaration statement");
            }

            Advance();

            // Parse what is to be resorted
            resortable = ParseExpression();

            if (resortable is NilLiteral)
            {
                throw new MaxNilException("Cannot resort a value to nil");
            }
        }
        
        return new DeclarationStatement(token.Value, modifiers, type, expression, resortable!);
    }

    /// <summary>
    /// Attempt to find type connotations on an iterable type
    /// </summary>
    /// <param name="listingType">The resulting type that will be assigned</param>
    /// <returns>The listingType parameter, whether modified or not</returns>
    /// <exception cref="MaxTypeException"></exception>
    private MaxType AttemptIterableTyping(MaxType type)
    {
        return type switch
        {
            ListType => ParseListType(),
            DictionaryType => ParseDictionaryType(),
            _ => type
        };
    }

    /// <summary>
    /// Parse the current list and it's typing
    /// </summary>
    /// <returns></returns>
    private ListType ParseListType()
    {
        // More verbose and readable
        if (Match(TokenType.Of))
        {
            Advance();

            // Also attempt typing here
            // REMEMBER, ParseType also calls AttemptIterableTyping too, so it is recursive!
            MaxType elementType = ParseType();

            return new ListType(elementType);
        }

        // Quicker and easier to nest
        if (Match(TokenType.Lesser))
        {
            Advance();

            MaxType elementType = ParseType();

            Consume(TokenType.Greater);

            return new ListType(elementType);
        }

        // Just a normal list. Straight up yo!
        return new ListType(null);
    }

    private DictionaryType ParseDictionaryType()
    {
        // Alike the lists syntax

        if (Match(TokenType.Of))
        {
            Advance();

            // That's right, since it is a key, value type thing. It is in parenthesis
            Consume(TokenType.LeftParenthesis);

            MaxType keyType = ParseType();

            // Requires value typing too to follow
            Consume(TokenType.Comma);

            MaxType valueType = ParseType();

            Consume(TokenType.RightParenthesis);

            return new DictionaryType(keyType, valueType);
        }

        if (Match(TokenType.Lesser))
        {
            // More of the same stuff
            Advance();

            MaxType keyType = ParseType();

            Consume(TokenType.Comma);

            MaxType valueType = ParseType();

            Consume(TokenType.Greater);

            return new DictionaryType(keyType, valueType);
        }

        // No specification
        return new DictionaryType(null, null);
    }

    #endregion

    #region Parsing Identifiers
    // (Variable Modification)

    /// <summary>
    /// Parse an Identifier Statement
    /// </summary>
    /// <returns>Is resulted from ParsePostfixExpression</returns>
    /// <seealso cref="ParsePostfixExpression()"/>
    /// <exception cref="MaxSyntaxException"></exception>
    private IStatement ParseIdentifierStatement()
{
    if (Match(TokenType.Identifier))
    {
        Token identifier = Consume();

        IExpression expression = ParseIdentifier(identifier);

        // Allow indexing/member access chains
        expression = ParsePostfixExpression(expression);

        // If there is an assignment operation, do as needed
        if (CurrentToken() != null &&
            IsAssignmentOperator(CurrentToken()!))
        {
            return ParseAssignment(expression);
        }

        // Do a Task Call when possible
        if (expression is TaskCall)
        {
            return new ExpressionStatement(expression);
        }

        throw new MaxSyntaxException(
            "Expected an assignment or task call."
        );
    }

    throw new MaxSyntaxException(
        "Expected an identifier statement."
    );
}

    /// <summary>
    /// Parses an identifier. This involves indexing and task calling within
    /// </summary>
    /// <param name="currentToken">The identifier to parse</param>
    /// <returns>A Variable Referenec of the token inputted, or an Indexing Expression, or a Task Call</returns>
    /// <exception cref="MaxSyntaxException"></exception>
    private static VariableReference ParseIdentifier(Token currentToken)
    {
        // Create new variable expression with the current name
        return new VariableReference(currentToken.Value);
    }

    /// <summary>
    /// Parses a task call, which is initiated by using brackets after an identifier
    /// </summary>
    /// <param name="name">Name of the token to call upon</param>
    /// <returns>A TaskCall of the name and arguments</returns>
    private TaskCall ParseTaskCall(IExpression callable)
    {
        Consume(TokenType.LeftParenthesis);

        List<IExpression> arguments = [];

        if (!Match(TokenType.RightParenthesis))
        {
            do
            {
                // Add to arguments
                arguments.Add(ParseExpression());

                // Per comma, one more expression
                if (Match(TokenType.Comma))
                {
                    Advance();
                    continue;
                }

                break;
            }
            while (true);
        }

        Consume(TokenType.RightParenthesis);

        return new TaskCall(callable, arguments);
    }

    /// <summary>
    /// Parses an assignment statement
    /// </summary>
    /// <param name="target">The target expression to parse</param>
    /// <returns>A complete Assignment Statement</returns>
    /// <exception cref="MaxSyntaxException"></exception>
    private AssignmentStatement ParseAssignment(IExpression target)
    {
        Token modifier = CurrentToken()
            ?? throw new MaxEndFileException(
                "No modifier following the assignment target"
            );

        string assignmentType = modifier.Type switch
        {
            TokenType.Equals => "=",
            TokenType.PlusEqual => "+=",
            TokenType.MinusEqual => "-=",
            TokenType.MultiplyEqual => "*=",
            TokenType.DivideEqual => "/=",
            TokenType.ModulusEqual => "%=",

            _ => throw new MaxSyntaxException(
                "Expected an assigning operator after the assignment target"
            ),
        };

        Advance();

        IExpression value = ParseExpression();

        return new AssignmentStatement(
            target,
            value,
            assignmentType
        );
    }

    /// <summary>
    /// Parses a Postfix expression ('.'). Includes recursion
    /// </summary>
    /// <param name="target">The target expression</param>
    /// <returns>A MemberAccessExpression</returns>
    /// <exception cref="MaxSyntaxException"></exception>
    private IExpression ParsePostfixExpression(IExpression expression)
    {
        while (CurrentToken() is not null)
        {
            switch (CurrentToken()!.Type)
            {
                case TokenType.Dot:
                {
                    Advance();

                    Token member = ConsumeMemberName();
                    
                    expression = new MemberAccessExpression(
                        expression,
                        member.Value
                    );

                    continue;
                }

                case TokenType.LeftBracket:
                {
                    Advance();

                    IExpression index = ParseExpression();

                    Consume(TokenType.RightBracket);

                    expression = new IndexingExpression(
                        expression,
                        index
                    );

                    continue;
                }

                case TokenType.LeftParenthesis:
                {
                    expression = ParseTaskCall(expression);
                    
                    continue;
                }

                default: return expression;
            }
        }

        return expression;
    }

    /// <summary>
    /// Consumes a member name and returns it. Through `IsContextualKeyword`, keywords are also considered as identifiers.
    /// </summary>
    /// <returns>An Identifier Token with the name value</returns>
    /// <seealso cref="IsContextualKeyword"/>
    /// <exception cref="MaxSyntaxException"></exception>
    private Token ConsumeMemberName()
    {
        Token current = CurrentToken() ?? throw new MaxSyntaxException("Expected a valid name for a member name");

        // Check if it's a standard identifier OR any keyword that can double as a property/method name
        if (current.Type == TokenType.Identifier || IsContextualKeyword(current.Type))
        {
            Advance();
            // Return a normalized token as an Identifier
            return new Token(TokenType.Identifier, current.Value);
        }

        throw new MaxSyntaxException($"Expected member identifier after '.', got '{current.Value}'");
    }

    private static bool IsAssignmentOperator(Token token)
    {
        if (token == null)
            return false;
        
        return token.Type is
            TokenType.Equals or
            TokenType.PlusEqual or
            TokenType.MinusEqual or
            TokenType.MultiplyEqual or
            TokenType.DivideEqual or
            TokenType.ModulusEqual;
    }

    #endregion

    #region Parsing Lambdas

    /// <summary>
    /// Helper function that determines if brackets represent a Lambda Expression
    /// </summary>
    /// <returns></returns>
    private bool IsLambdaHead()
    {
        // If it's an empty parameter list `()` followed by `=>`
        // Use CurrentToken() btw since Factor consumes it's token, automatically progressing by one
        if (CurrentToken() != null && CurrentToken()!.Type == TokenType.RightParenthesis && Peek() != null && Peek()!.Type == TokenType.EvaluateTo)
        {
            return true;
        }

        // Otherwise check the tokens inside `(...)` to check if `=>` follows `)`
        int offset = 1;
        int depth = 1;

        while (offset < tokens.Count)
        {
            var token = Peek(offset)!;
            if (token.Type == TokenType.LeftParenthesis) depth++;
            else if (token.Type == TokenType.RightParenthesis) depth--;

            if (depth == 0)
            {
                // The token right after closing `)` MUST be `=>` if a lambda!
                return Peek(offset + 1)!.Type == TokenType.EvaluateTo;
            }

            offset++;
        }

        return false;
    }

    private LambdaExpression ParseLambda()
    {
        List<string> parameters = [];

        // Check for params whilst there is no right parenthesis
        if (!Match(TokenType.RightParenthesis)) {
            do
            {
                // Get the token for the name
                var nameToken = Consume(TokenType.Identifier);
                parameters.Add(nameToken.Value);

                if (Match(TokenType.RightParenthesis))
                {
                    break;
                }
                // Naturally require commas between params if not ending 
                else if (Match(TokenType.Comma))
                {
                    Advance();
                } else
                {
                    throw new MaxSyntaxException("Expected a comma between params or an ending parenthesis in lambda params");
                }
            }
            while (!Match(TokenType.RightParenthesis));
        }

        Consume(TokenType.RightParenthesis);

        // The '=>'
        Consume(TokenType.EvaluateTo);

        // When it is 'then' (an entire block of code)
        if (CurrentToken() != null && Match(CurrentToken()!, TokenType.Do))
        {
            Advance();

            List<IStatement> executable = [];

            // Add to body
            while (CurrentToken() != null)
            {
                Token token = CurrentToken()!;

                if (Match(token, TokenType.ConcludeStatement))
                {
                    break;
                }

                executable.Add(ParseStatement());
            }

            Advance();

            return new LambdaExpression(parameters, executable);
        }
        // Just a single liner
        else
        {
            IExpression singler = ParseExpression();

            // Lambda with no body, but rather just a single liner. It'll find love one day.
            return new LambdaExpression(parameters, null, singler);
        }
    }

    #endregion

    #region Parsing Say

    private SayStatement ParseSay()
    {
        Advance(); // Past say

        IExpression sayable = ParseExpression();

        return new SayStatement(sayable);
    }

    #endregion

    #region Parsing Return/Yield

    private ReturnStatement ParseReturn()
    {
        Advance(); // Past "return"

        if (Match(TokenType.Semicolon) || Match(TokenType.ConcludeStatement))
        {
            Advance();
            return new ReturnStatement(null);
        }

        // Now get the value to return
        IExpression returnable = ParseExpression();

        return new ReturnStatement(returnable);
    }

    private YieldStatement ParseYield()
    {
        Advance();

        if (Match(TokenType.Semicolon) || Match(TokenType.ConcludeStatement))
        {
            Advance();
            return new YieldStatement();
        }

        // Now get the value to return
        IExpression? returnable = ParseExpression();

        return new YieldStatement(returnable);
    }

    #endregion

    #region Parsing If

    private IfStatement ParseIf()
    {
        Advance(); // Past "if"

        IExpression expression = ParseExpression();

        if (!Match(TokenType.Then))
        {
            throw new MaxSyntaxException(
                "Expected a 'then' keyword after an if condition statement"
            );
        }

        Advance(); // Past "then"

        // List of all the executable
        List<IStatement> executable = [];
        List<ConditionalBranch> branches = [];
        List<IStatement> maybe_executable = [];
        List<IStatement> else_executable = [];

        // Parse first branch (always exists)
        while (CurrentToken() != null)
        {
            Token token = CurrentToken()!;

            // Break when seeing these three
            if (Match(token, TokenType.Elif) ||
                Match(token, TokenType.Maybe) ||
                Match(token, TokenType.ConcludeStatement) ||
                Match(token, TokenType.Else))
            {
                break;
            }

            executable.Add(ParseStatement());
        }

        // Parse elif branches if they exist
        while (Match(TokenType.Elif))
        {
            Advance(); // past "elif"

            IExpression elifExpression = ParseExpression();

            if (!Match(TokenType.Then))
            {
                throw new MaxSyntaxException(
                    "Expected a 'then' keyword after an elif condition statement"
                );
            }

            Advance(); // past "then" again

            List<IStatement> elifExecutable = [];

            // Essentially same logic as normal branches, just infinite amount of elif branches possible
            while (CurrentToken() != null)
            {
                Token token = CurrentToken()!;

                if (Match(token, TokenType.Elif) ||
                    Match(token, TokenType.Maybe) ||
                    Match(token, TokenType.Else) ||
                    Match(token, TokenType.ConcludeStatement))
                {
                    break;
                }

                elifExecutable.Add(ParseStatement());
            }

            branches.Add(
                new ConditionalBranch(
                    elifExpression,
                    elifExecutable
                )
            );
        }

        bool hasMaybe = false; // Already has a maybe statement
        bool hasElse = false; // Same here for else

        while (CurrentToken() != null)
        {
            Token token = CurrentToken()!;

            // When it finds a maybe block, add to it
            if (Match(token, TokenType.Maybe))
            {
                if (hasMaybe)
                {
                    throw new MaxSyntaxException(
                        "Cannot have more than one 'maybe' branch"
                    );
                }

                // Mark it true
                hasMaybe = true;
                Advance();

                while (CurrentToken() != null)
                {
                    token = CurrentToken()!;

                    if (Match(token, TokenType.Elif))
                    {
                        throw new MaxSyntaxException(
                            "Cannot have any 'elif' branches after a 'maybe' or 'else' branch"
                        );
                    }

                    if (Match(token, TokenType.Maybe) ||
                        Match(token, TokenType.Else) ||
                        Match(token, TokenType.ConcludeStatement))
                    {
                        break;
                    }

                    maybe_executable.Add(ParseStatement());
                }
            }
            // Else if it finds an else block, add to it
            else if (Match(token, TokenType.Else))
            {
                if (hasElse)
                {
                    throw new MaxSyntaxException(
                        "Cannot have more than one 'else' branch"
                    );
                }

                hasElse = true;
                Advance();

                while (CurrentToken() != null)
                {
                    token = CurrentToken()!;

                    if (Match(token, TokenType.Elif))
                    {
                        throw new MaxSyntaxException(
                            "Cannot have any 'elif' branches after a 'maybe' or 'else' branch"
                        );
                    }

                    if (Match(token, TokenType.Maybe) ||
                        Match(token, TokenType.Else) ||
                        Match(token, TokenType.ConcludeStatement))
                    {
                        break;
                    }

                    else_executable.Add(ParseStatement());
                }
            }
            // Else it will find a conclude statement and break
            else if (Match(token, TokenType.ConcludeStatement))
            {
                break;
            }
            else
            {
                throw new MaxSyntaxException(
                    "Unexpected token after if/elif branches"
                );
            }
        }

        // Must conclude now
        if (!Match(TokenType.ConcludeStatement))
        {
            throw new MaxSyntaxException(
                "Expected 'conclude' keyword after if statement"
            );
        }

        Advance();

        // And now return
        return new IfStatement(
            new ConditionalBranch(expression, executable),
            branches,
            maybe_executable,
            else_executable
        );
    }

    #endregion

    #region Parsing While

    private WhileStatement ParseWhile()
    {
        Advance(); // Past "while"

        IExpression expression = ParseExpression();

        if (!Match(TokenType.Do))
        {
            throw new MaxSyntaxException(
                "Expected a 'do' keyword after a while condition statement"
            );
        }

        Advance(); // Past "do"

        List<IStatement> executable = [];

        while (CurrentToken() != null)
        {
            Token token = CurrentToken()!;

            if (Match(token, TokenType.ConcludeStatement))
            {
                break;
            }

            executable.Add(ParseStatement());
        }

        Advance(); // Make sure to advance past the conclude statement

        return new WhileStatement(expression, executable);
    }

    #endregion

    #region Parsing For

    private ForStatement ParseFor()
    {
        Advance(); // past "for"

        // The "name = value" part
        Token declarable = CurrentToken()!;

        if (!Match(declarable, TokenType.Identifier))
        {
            throw new MaxTypeException("Declared variable inside for loop must be written as an identifier");
        }

        Advance(); // Past the identifier

        Consume(TokenType.Equals); // Check for equals
        
        IExpression expressable = ParseExpression();

        // Create a declaration from the components
        DeclarationStatement declaration = new(
            declarable.Value,
            new ModifierSet(),
            null,
            expressable,
            null
        );

        Consume(TokenType.To); // Check for "to" keyword

        // Now check the new expression
        IExpression toExpression = ParseExpression();

        IExpression? stepCounter;

        if (Match(TokenType.Comma))
        {
            Advance();

            // Grab the should-be step keyword
            Token Step = Consume();

            // Having a comma means expecting a step keyword
            if (!Match(Step, TokenType.Step))
            {
                throw new MaxSyntaxException("Expected the 'step' keyword in the for loop after the comma");
            }

            stepCounter = ParseExpression();
        }
        else // Else simply make n/a
        {
            stepCounter = null;
        }

        Consume(TokenType.Do);

        List<IStatement> executables = [];

        while (CurrentToken() != null)
        {
            Token currentToken = CurrentToken()!;

            if (Match(currentToken, TokenType.ConcludeStatement))
            {
                Advance(); // Skip conclude for next parsing

                return new ForStatement(
                    declaration,
                    toExpression,
                    executables,
                    stepCounter
                );
            }

            executables.Add(ParseStatement());
        }

        return new ForStatement(
            declaration,
            toExpression,
            executables,
            stepCounter
        );
    }

    #endregion

    #region Parsing Foreach

    private ForeachStatement ParseForeach()
    {
        Advance(); // As always

        // Get the item name(s), since we allow multiple declarations depending on what we are doing (e.g. enumerating!)
        List<string> loopVariables = [];

        // Check if variables are wrapped in parentheses, e.g. foreach (item, index) in list.enumerate()
        if (Match(TokenType.LeftParenthesis))
        {
            Advance(); // Skip '('

            do
            {
                var varToken = Consume(TokenType.Identifier);
                loopVariables.Add(varToken.Value);

                if (Match(TokenType.Comma))
                {
                    Advance(); // Skip ','
                }
                else if (!Match(TokenType.RightParenthesis))
                {
                    throw new MaxSyntaxException("Expected ',' or ')' after variable name in foreach loop");
                }
            }
            while (!Match(TokenType.RightParenthesis));

            Consume(TokenType.RightParenthesis); // Skip ')'
        }
        else
        {
            // Single variable syntax, e.g. foreach item in list
            var itemName = Consume(TokenType.Identifier);
            loopVariables.Add(itemName.Value);
        }

        Consume(TokenType.In);

        // Get the collection to iterate
        var iterable = ParseExpression();

        Consume(TokenType.Do);

        List<IStatement> executables = [];

        // Identical structure to a for loop of course
        while (CurrentToken() != null)
        {
            Token currentToken = CurrentToken()!;

            if (Match(currentToken, TokenType.ConcludeStatement))
            {
                Advance();
                
                return new ForeachStatement(
                    loopVariables,
                    iterable,
                    executables
                );
            }

            executables.Add(ParseStatement());
        }
        
        return new ForeachStatement(
            loopVariables,
            iterable,
            executables
        );
    }

    #endregion

    #region Parsing Define

    /// <summary>
    /// Parses the task type of the current token
    /// </summary>
    /// <returns>The Task Type if valid, else an error</returns>
    /// <exception cref="MaxTypeException"></exception>
    private TaskType ParseTaskType()
    {
        // Check for tasktype
        Token token = Consume(TokenType.TaskType);

        return token.Value switch
        {
            "function" => TaskType.Function,
            "coroutine" => TaskType.Coroutine,

            _ => throw new MaxTypeException("Expected a TaskType to parse through. Ensure it is a valid TaskType being sent through")
        };
    }

    private List<Parameter> ParseParameters()
    {
        // Expect the proper syntax here
        Consume(TokenType.LeftParenthesis);

        // List of params
        List<Parameter> parameters = [];

        // For immediate return
        if (Match(TokenType.RightParenthesis))
        {
            Advance();

            return parameters;
        }

        // Check for params whilst there is no right bracket
        do
        {
            string name = Consume().Value;

            if (Match(TokenType.As))
            {
                Advance();

                // Get the typing
                if (!TypeRegistry.Max_TypeNames.TryGetValue(CurrentToken()!.Value, out MaxType? type))
                {
                    throw new MaxTypeException("Expected a valid type for a typed parameter");
                }

                // Advance to either a comma, right parenthesis, or 'of' for iterable typing
                Advance();

                // Now attempt!
                type = AttemptIterableTyping(type);

                Parameter newTypedParam = new(type, name);

                parameters.Add(newTypedParam);
            } else
            {
                Parameter newParam = new(null, name);

                parameters.Add(newParam);
            }

            if (Match(TokenType.RightParenthesis))
            {
                break;
            }
            // Naturally require commas between params if not ending 
            else if (Match(TokenType.Comma))
            {
                Advance();
            } else
            {
                throw new MaxSyntaxException("Expected a comma between params or an ending bracket in task params");
            }
        }
        while (!Match(TokenType.RightParenthesis));

        Advance(); // Past parenthesis

        return parameters;
    }

    private TaskDefinition ParseDefinition()
    {
        Advance();

        // Get the task type
        TaskType taskType = ParseTaskType();

        // Make sure it's a valid name
        if (!Match(TokenType.Identifier))
        {
            throw new MaxSyntaxException("Expected an identifier name after the TaskType in a definition statement");
        }

        string name = Consume().Value;

        if (Match(TokenType.EvaluateTo))
        {
            Advance();

            MaxType? returnType = null;

            // Yes also allows a return type for parameterless!
            if (Match(TokenType.Return))
            {
                Advance();

                string typeName = Consume().Value;

                if (TypeRegistry.Max_TypeNames.TryGetValue(typeName, out MaxType? assignedType))
                {
                    returnType = assignedType;
                } else
                {
                    throw new MaxSyntaxException("Expected a valid type to be the return type of a task definition");
                }
            }
            
            Consume(TokenType.Do);

            List<IStatement> body = [];
            
            // Classic structure...
            while (CurrentToken() != null)
            {
                Token currentToken = CurrentToken()!;

                if (Match(currentToken, TokenType.ConcludeStatement))
                {
                    Advance();
                    
                    // Return a new task definition
                    // Parameter is represented as empty
                    return new TaskDefinition(
                        taskType,
                        name,
                        [],
                        returnType,
                        body
                    );
                }

                body.Add(ParseStatement());
            }
            
            throw new MaxSyntaxException("Expected 'conclude' at the end of task definition.");
        }
        else if (Match(TokenType.With))
        {
            // Coroutines are prohibited from using parameters! No 'with' syntax allowed!
            if (taskType == TaskType.Coroutine)
            {
                throw new MaxSyntaxException("Coroutines cannot intake parameters.");
            }

            // Do parameter checking here
            Advance();

            // Parse Parameters
            //
            List<Parameter> parameters = ParseParameters();
            //

            MaxType? returnType = null;

            if (Match(TokenType.Return))
            {
                Advance();

                string typeName = Consume().Value;

                if (TypeRegistry.Max_TypeNames.TryGetValue(typeName, out MaxType? assignedType))
                {
                    returnType = assignedType;
                } else
                {
                    throw new MaxSyntaxException("Expected a valid type to be the return type of a task definition");
                }
            }

            Consume(TokenType.Do);

            List<IStatement> body = [];
            
            // Classic structure...
            while (CurrentToken() != null)
            {
                Token currentToken = CurrentToken()!;

                if (Match(currentToken, TokenType.ConcludeStatement))
                {
                    Advance();
                    
                    return new TaskDefinition(
                        taskType,
                        name,
                        parameters,
                        returnType,
                        body
                    );
                }

                body.Add(ParseStatement());
            }
            
            throw new MaxSyntaxException("Expected 'conclude' at the end of task definition.");
        }
        else
        {
            throw new MaxSyntaxException("Expected a task name to be followed by evaluation or parameters!");
        }
    }

    #endregion

    #region Parsing Using

    private UsingStatement ParseUsing()
    {
        Advance(); // Past 'using'

        if (CurrentToken() is not null)
        {
            string module = Consume().Value;

            List<string>? memberNames = null;

            // Semicolons are required to signify when a using statement ends!
            while (CurrentToken() != null && !Match(TokenType.Semicolon))
            {
                // No semicolon? We expect member accessing!
                Consume(TokenType.Dot);

                string memberName = Consume().Value;

                memberNames ??= [];

                memberNames.Add(memberName);
            }

            Advance(); // Advance the final semicolon

            return new UsingStatement(module, memberNames);
        }
        else
        {
            throw new MaxSyntaxException("Expected a module name minimum after 'using'");
        }
    }

    #endregion

    #region General Parsing Functions

    private IStatement ParseStatement()
    {
        Token token = CurrentToken() ?? throw new MaxNilException("Token to be parsed was null");

        return token.Type switch
        {
            TokenType.Declare => ParseDeclaration(),
            TokenType.Say => ParseSay(),
            TokenType.IfBegin => ParseIf(),
            TokenType.While => ParseWhile(),
            TokenType.For => ParseFor(),
            TokenType.Foreach => ParseForeach(),
            TokenType.Identifier => ParseIdentifierStatement(),
            TokenType.Define => ParseDefinition(),
            TokenType.Return => ParseReturn(),
            TokenType.Yield => ParseYield(),
            TokenType.Using => ParseUsing(),

            _ => throw new MaxTypeException($"Unexpected Token recieved when trying to parse position {position}; {token}. The token should be a statement.")
        };
    }

    /// <summary>
    /// Parses the type of the Current Token.
    /// This also allows recursive/nested typing.
    /// NOTE: This also advances the parser!
    /// </summary>
    /// <returns>The MaxType of the current token</returns>
    /// <exception cref="MaxTypeException"></exception>
    private MaxType ParseType()
    {
        var typeName = Consume().Value;

        if (!TypeRegistry.Max_TypeNames.TryGetValue(typeName,  out MaxType? type))
        {
            throw new MaxTypeException("Type assignment must use a real type.");
        }

        return AttemptIterableTyping(type);
    }

    private static IExpression ConvertBoolean(string boolean)
    {
        return boolean switch
        {
            "true" => new BooleanLiteral(true),
            "false" => new BooleanLiteral(false),
            "possible" => new PossibleLiteral(),

            _ => throw new MaxValueException($"Unknown boolean value: {boolean}")
        };
    }

    public List<IStatement> Parse()
    {
        List<IStatement> statements = [];

        while (position < tokens.Count)
        {
            statements.Add(ParseStatement());
        }

        return statements;
    }

    #endregion
}