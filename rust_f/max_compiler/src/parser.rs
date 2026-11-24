use crate::{
    ast::*,
    error::{MaxDiagnostic, Severity},
    token::{Token, TokenKind as K},
};
use crate::naming::Casing;

/// Our glorious parser to convert lexer tokens into statements or expressions.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<MaxDiagnostic>
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0, diagnostics: Vec::new() }
    }

    /// All statements (expressions can't be standalon, with the exception of the ExpressionStatement)
    pub fn parse(mut self) -> (Vec<Statement>, Vec<MaxDiagnostic>) {
        let mut statements = Vec::new();
        while self.pos < self.tokens.len() {
            let start = self.pos;
            match self.parse_statement() {
                Ok(s) => statements.push(s),
                Err(d) => {
                    self.diagnostics.push(d);
                    // Always synchronise and prevent infinite loops
                    if self.pos == start { self.pos += 1; }
                    self.synchronise(&[]);
                }
            }
        }
        // Ensure we return both
        (statements, self.diagnostics)
    }

    // ---------------------------- //
    // ------ COMMON HELPERS ------ //
    // ---------------------------- //

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn eof_error(&self) -> MaxDiagnostic {
        let span = self.tokens.last().map(|t| t.span).unwrap_or_default();
        span.error("unexpected-eof", "Unexpected end of file.")
    }

    fn current_safe(&self) -> Result<&Token, MaxDiagnostic> {
        self.current().ok_or_else(|| self.eof_error())
    }

    fn consume(&mut self) -> Result<Token, MaxDiagnostic> {
        let token = self.current_safe()?.clone();
        self.pos += 1;
        Ok(token)
    }

    /// The kind `offset` tokens ahead. An offset of 0 is the current token
    fn peek_kind(&self, offset: usize) -> Option<K> {
        self.tokens.get(self.pos + offset).map(|t| t.kind)
    }

    /// Checks the current token kind with `peek_kind(0)`
    fn check(&self, kind: K) -> bool {
        self.peek_kind(0) == Some(kind)
    }

    /**
    Checks the current token kind against the given Kind, **without** consuming.
    If the kind matches, then return a `MaxDiagnostic` Error with the given code and message.\
    Use this to give informative messages on what should not be in the code. Note that since
    this returns the diagnostic, the parser will stop, assuming it's an error
    */
    fn forbid(&self, kind: K, code: &'static str, message: impl Into<String>) -> Result<(), MaxDiagnostic> {
        match self.current_safe() {
            Ok(t) if t.kind == kind => Err(t.span.error(code, message)),
            _ => Ok(()),
        }
    }

    /// Record an error of the current token if it matches, skip the token, and keep going.
    /// This also returns whether the token matched or not
    fn reject(&mut self, kind: K, code: &'static str, message: impl Into<String>) -> bool {
        if !self.check(kind) { return false; }

        let span = self.tokens[self.pos].span;
        self.diagnostics.push(span.error(code, message));
        self.pos += 1;
        true
    }

    /// Record a warning, and leave the token for normal parsing
    fn warn_on(&mut self, kind: K, code: &'static str, message: impl Into<String>) -> bool {
        if !self.check(kind) { return false; }

        let span = self.tokens[self.pos].span;
        self.diagnostics.push(span.warning(code, message));
        true
    }

    /// Consumes the current token if it has the given kind, returning if it was successful or not as a bool
    fn eat(&mut self, kind: K) -> bool {
        if self.check(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// This returns the current token, checking it's kind. This also consumes the current token, moving up one token into the Parser.
    fn expect<T>(&mut self, kind: K, message: Option<T>) -> Result<Token, MaxDiagnostic>
        where T: Into<String>
    {
        let token = self.consume()?;
        let msg = message.map(|m| m.into()).unwrap_or(format!("Expected {:?}, got '{}'", kind, token.value));
        if token.kind != kind {
            return Err(token.span.error(
                "unexpected-token",
                msg,
            ));
        }
        Ok(token)
    }

    /// Expects a conclude statement specifically. All errors caused by this have the same code.
    fn expect_conclusion(&mut self, message: impl Into<String>) -> Result<Token, MaxDiagnostic> {
        let token = self.consume()?;
        if token.kind != K::ConcludeStatement {
            return Err(token.span.error(
                "expected-conclude",
                message.into(),
            ));
        }
        Ok(token)
    }

    /// An error pointing at the current token, or the last one when the file has ended
    fn error_here(&self, code: &'static str, message: impl Into<String>) -> MaxDiagnostic {
        match self.tokens.get(self.pos).or_else(|| self.tokens.last()) {
            Some(token) => token.span.error(code, message),
            None => MaxDiagnostic::new(Severity::Error, code, message.into(), Span::new(0, 0, 0, 0)),
        }
    }

    /// Words that are keywords but can still be used as member names, e.g. `flag.then(...)` or `x.in`
    fn is_contextual_keyword(kind: K) -> bool {
        matches!(
            kind,
            K::Decl | K::Resort | K::Say | K::Def | K::TaskType | K::Model | K::ModelType | K::Boolean | K::IfBegin
                | K::Then | K::Elif | K::Else | K::Maybe | K::ConcludeStatement | K::Return
                | K::Yield | K::While | K::For | K::Foreach | K::Do | K::Nil | K::As | K::Not
                | K::And | K::Or | K::To | K::Step | K::Of | K::In | K::With
                | K::Using | K::Enum | K::Modifier | K::Members | K::Break | K::Skip
                | K::Match | K::Case
        )
    }

    /// Words that are used to start statements
    fn is_statement_starter(kind: K) -> bool {
        matches!(
            kind,
            K::Decl | K::Say | K::Def | K::IfBegin | K::Then
            | K::Elif | K::Else | K::Maybe | K::While | K::For
            | K::Foreach | K::Using | K::Enum | K::Match | K::Return
            | K::Yield | K::Break | K::Skip | K::Emit
        )
    }

    /// Parses statements until one of the `stops` is the current token. The stop is not consumed.
    /// The opener span given is used to provide the diagnostic error position
    fn parse_block(&mut self, stops: &[K], opener: Span) -> Result<Vec<Statement>, MaxDiagnostic> {
        let mut body = Vec::new();
        while let Some(kind) = self.peek_kind(0) {
            if stops.contains(&kind) { return Ok(body); }

            let start = self.pos;
            match self.parse_statement() {
                Ok(statement) => body.push(statement),
                Err(diagnostic) => {
                    self.diagnostics.push(diagnostic);
                    if self.pos == start { self.pos += 1; }   // always make progress
                    self.synchronise(stops);
                }
            }
        }
        Err(opener.error("missing-conclude", "This block is never closed; add a 'conclude'"))
    }

    /// Continues to move the parser up as long as the consumed token is not of the given `stops`
    /// or a statement starter (see in [`parser`][`Parser`] -> [`is_statement_starter`]).\
    /// Use this to synchronise a block back to it's original place if returns a diagnostic
    fn synchronise(&mut self, stops: &[K]) {
        while let Some(kind) = self.peek_kind(0) {
            if stops.contains(&kind) || Self::is_statement_starter(kind) { break; }
            self.pos += 1;
        }
    }

    /// True where a bare `return` or `yield` has no value to parse
    fn at_value_end(&self) -> bool {
        match self.peek_kind(0) {
            None => true,
            Some(kind) => matches!(
                kind,
                // These are what can follow a return statement
                K::Semicolon | K::ConcludeStatement | K::Elif | K::Else | K::Maybe | K::Case
            ),
        }
    }

    // ---------------------------- //
    // ------- EXPRESSIONS -------- //
    // ---------------------------- //
    // In order of our Operations System, starting with Unary and moving to Binary
    //   or -> and -> comparison -> + - -> * / % -> not -> factor

    fn parse_expression(&mut self) -> Result<Expression, MaxDiagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expression, MaxDiagnostic> {
        let mut left = self.parse_and()?;
        while self.eat(K::Or) {
            let right = self.parse_and()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Logical { left: Box::new(left), op: LogicalOp::Or, right: Box::new(right) }, span);
        }
        
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression, MaxDiagnostic> {
        let mut left = self.parse_comparison()?;
        while self.eat(K::And) {
            let right = self.parse_comparison()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Logical { left: Box::new(left), op: LogicalOp::And, right: Box::new(right) }, span);
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression, MaxDiagnostic> {
        let mut left = self.parse_additive()?;
        while let Some(op) = self.peek_kind(0).and_then(Self::comparison_op) {
            // Yonder and beyond our comparison operand!
            self.pos += 1;

            let right = self.parse_additive()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Comparison { left: Box::new(left), op, right: Box::new(right) }, span);
        }
        
        Ok(left)
    }

    /// Checks if a given kind of token is a Comparison-Like,
    /// returning `Option<ComparisonOp>` if matching
    fn comparison_op(kind: K) -> Option<ComparisonOp> {
        match kind {
            K::Comparator => Some(ComparisonOp::Equal),
            K::NotEqualTo => Some(ComparisonOp::NotEqual),
            K::Lesser => Some(ComparisonOp::Lesser),
            K::LesserEqual => Some(ComparisonOp::LesserEqual),
            K::Greater => Some(ComparisonOp::Greater),
            K::GreaterEqual => Some(ComparisonOp::GreaterEqual),
            _ => None,
        }
    }

    /// This first calls `parse_term`
    fn parse_additive(&mut self) -> Result<Expression, MaxDiagnostic> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek_kind(0) {
                Some(K::Plus) => BinaryOp::Add,
                Some(K::Minus) => BinaryOp::Subtract,
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_term()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Binary { left: Box::new(left), op, right: Box::new(right) }, span);
        }
        Ok(left)
    }

    /// This first calls `parse_unary`
    fn parse_term(&mut self) -> Result<Expression, MaxDiagnostic> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_kind(0) {
                Some(K::Multiply) => BinaryOp::Multiply,
                Some(K::Divide) => BinaryOp::Divide,
                Some(K::Modulus) => BinaryOp::Modulus,
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_factor()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Binary { left: Box::new(left), op, right: Box::new(right) }, span);
        }
        Ok(left)
    }

    /// After consuming the 'not', this calls `parse_expression`, otherwise `parse_factor`
    fn parse_unary(&mut self) -> Result<Expression, MaxDiagnostic> {
        if self.eat(K::Not) {
            let operand = self.parse_comparison()?;
            let span = operand.span.clone();
            return Ok(Expression::new(ExpressionKind::Unary { op: UnaryOp::Not, operand: Box::new(operand) }, span));
        }
        self.parse_factor()
    }

    /// This consumes the current token, returning an Expression based on the Token kind.
    /// This performs `parse_postfix` on most branches.
    fn parse_factor(&mut self) -> Result<Expression, MaxDiagnostic> {
        let token = self.consume()?;

        let span: Span = token.span;

        match token.kind {
            K::Nil => self.parse_postfix(Expression::new(ExpressionKind::Nil, span)),

            K::Integer => {
                let value = token.value.parse::<i64>().map_err(|_| {
                    token.span.error("invalid-integer-literal", format!("Integer literal '{}' is out of range", token.value))
                })?;
                self.parse_postfix(Expression::new(ExpressionKind::Integer(value), span))
            }

            K::Float => {
                let value = token.value.parse::<f64>().map_err(|_| {
                    token.span.error("invalid-float-literal", format!("Float literal '{}' is out of range", token.value))
                })?;
                self.parse_postfix(Expression::new(ExpressionKind::Float(value), span))
            }

            // Negative numbers
            K::Minus => {
                let operand = self.parse_factor()?;
                Ok(Expression::new(ExpressionKind::Unary { op: UnaryOp::Negate, operand: Box::new(operand) }, span))
            }

            K::StringLiteral => self.parse_postfix(Expression::new(ExpressionKind::Str(token.value), span)),

            K::Boolean => {
                // Our different booleans. Because I decided a possible boolean was a reaaal good idea...
                let literal = match token.value.as_str() {
                    "true" => Expression::new(ExpressionKind::Boolean(true), span),
                    "false" => Expression::new(ExpressionKind::Boolean(false), span),
                    "possible" => Expression::new(ExpressionKind::Possible, span),
                    other => return Err(token.span.error("unknown-boolean", format!("Unknown boolean value: {other}"))),
                };
                self.parse_postfix(literal)
            }

            K::Identifier => self.parse_postfix(Expression::new(ExpressionKind::Variable(token.value), span)),

            K::LeftParenthesis => {
                // T'was a lambda?
                if self.is_lambda_head() {
                    return self.parse_lambda();
                }
                let inner = self.parse_expression()?;

                self.expect(K::RightParenthesis, Some("Expected a right parenthesis to close a left parenthesis"))?;
                self.parse_postfix(inner)
            }

            K::LeftBracket => {
                let list = self.parse_list()?;
                self.parse_postfix(list)
            }

            K::LeftCurlyBracket => {
                let dictionary = self.parse_dictionary()?;
                self.parse_postfix(dictionary)
            }

            // Manually create 'this'
            K::This => self.parse_postfix(Expression::new(ExpressionKind::This, span)),

            _ => Err(token.span.error("factor-error", format!("Expected an expression, got '{}'", token.value))),
        }
    }

    /// Handles `.member`, `[index]` and `(arguments)` chains after any expression
    fn parse_postfix(&mut self, mut expression: Expression) -> Result<Expression, MaxDiagnostic> {
        let span: Span = expression.span.clone();

        loop {
            match self.peek_kind(0) {
                // Most likely member accessing
                Some(K::Dot) => {
                    self.pos += 1;
                    let member = self.consume_member_name()?;
                    expression = Expression::new(ExpressionKind::MemberAccess { target: Box::new(expression), member }, span);
                }

                // Most likely indexing
                Some(K::LeftBracket) => {
                    self.pos += 1;
                    let index = self.parse_expression()?;
                    self.expect(K::RightBracket, Some("Expected a right bracket to close a left bracket"))?;
                    expression = Expression::new(ExpressionKind::Index { target: Box::new(expression), index: Box::new(index) }, span);
                }
                
                // Most likely a task call
                Some(K::LeftParenthesis) => {
                    expression = self.parse_task_call(expression)?;
                }
                
                _ => return Ok(expression),
            }
        }
    }

    /// Consumes a member name by ensuring it is a valid contextual keyword (see `is_contextual_keyword()`),
    /// otherwise erring
    fn consume_member_name(&mut self) -> Result<String, MaxDiagnostic> {
        let token = self.consume()?;
        if token.kind == K::Identifier || Self::is_contextual_keyword(token.kind) {
            Ok(token.value)
        } else {
            Err(token.span.error(
                "missing-member-identifier",
                format!("Expected member identifier after '.', got '{}'", token.value),
            ))
        }
    }

    /**
    Parses a call's argument list: everything between `(` and `)`.

    Each argument is either `expression` or `identifier = expression`. The name is detected
    with a two token lookahead: an `Identifier` followed by a lone `=` (`K::Equals`).
    That lookahead is safe because the lexer turns `==` into `Comparator` and `=>` into
    `EvaluateTo`, so a lone `=` can never be anything else here. Max has no assignment
    *expressions* (assignments are statements), so `foo(x = 5)` has no other meaning.

    A plain variable such as `foo(x)` stays an unnamed argument. Only `x =` makes it a name.

    The parser deliberately does NOT check the names. It cannot know the callee's
    parameters, so everything is checked later in `bind_arguments`.
    */
    fn parse_task_call(&mut self, callee: Expression) -> Result<Expression, MaxDiagnostic> {
        self.expect::<&str>(K::LeftParenthesis, None)?;

        let span: Span = self.current_safe()?.span;

        // Each arg is assigned to it's name
        let mut arguments: Vec<Argument> = Vec::new();
        if !self.check(K::RightParenthesis) {
            loop {
                // The '=' always signifies the first name name given is the name of the parameter
                let name = if self.check(K::Identifier) && self.peek_kind(1) == Some(K::Equals) {
                    let token = self.consume()?;
                    self.pos += 1; // the '='

                    Some(Spanned { span: token.span, node: token.value })
                } else {
                    None
                };

                let value = self.parse_expression()?;
                arguments.push(Argument { name, value });

                if !self.eat(K::Comma) { break; }
            }
        }
        self.expect(K::RightParenthesis, Some("Expected a right parenthesis to close a left parenthesis"))?;

        Ok(Expression::new(ExpressionKind::Callable { callee: Box::new(callee), arguments }, span))
    }

    // ---------------------------- //
    // ---- LISTS AND DICTS ------- //
    // ---------------------------- //

    /// The opening '[' is already consumed. A trailing comma is allowed.
    fn parse_list(&mut self) -> Result<Expression, MaxDiagnostic> {
        let span: Span = self.current_safe()?.span;

        let mut items = Vec::new();
        while !self.check(K::RightBracket) {
            items.push(self.parse_expression()?);
            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect(K::RightBracket, Some("Expected a right bracket to close a left bracket"))?;
        Ok(Expression::new(ExpressionKind::List(items), span))
    }

    /// The opening '{' is already consumed
    fn parse_dictionary(&mut self) -> Result<Expression, MaxDiagnostic> {
        let span: Span = self.current_safe()?.span;
        
        let mut entries = Vec::new();
        while !self.check(K::RightCurlyBracket) {
            let key = self.parse_expression()?;
            self.expect(K::Colon, Some("A key-value pair in a dictionary must be seperated by a colon ':'"))?; // Colon to seperate key : value

            let value = self.parse_expression()?;
            entries.push((key, value));

            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect(K::RightCurlyBracket, Some("Expected a right curly-bracket to close a left curly-bracket"))?;
        Ok(Expression::new(ExpressionKind::Dictionary(entries), span))
    }

    // ---------------------------- //
    // --------- LAMBDAS ---------- //
    // ---------------------------- //

    /// Called with the '(' already consumed: is the matching ')' followed by '=>'?
    fn is_lambda_head(&self) -> bool {
        let mut depth = 1;
        for (offset, token) in self.tokens[self.pos..].iter().enumerate() {
            match token.kind {
                K::LeftParenthesis => depth += 1,
                K::RightParenthesis => {
                    depth -= 1;
                    if depth == 0 {
                        return self.peek_kind(offset + 1) == Some(K::EvaluateTo);
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// The '(' is already consumed
    fn parse_lambda(&mut self) -> Result<Expression, MaxDiagnostic> {
        // Get our span here, right now at the beginning
        let span: Span = self.current_safe()?.span;

        let mut parameters = Vec::new();
        if !self.check(K::RightParenthesis) {
            loop {
                let token = self.expect(K::Identifier, Some("A parameter name is always expected after a comma"))?;
                let span = token.span;
                let name = Spanned { node: token.value, span};
                // Param types are NOT supported in lambdas!
                let kind: ParameterKind = ParameterKind::Required;

                // Provide diagnostics on them these for info
                self.reject(K::QuestionMark, "optional-lambda-parameter", "A lambda parameter can not be optional");
                self.reject(K::Ellipsis, "variadic-lambda-parameter", "A lambda parameter can not be variadic");

                // Allow 'as' for type specification
                let type_ref = if self.eat(K::As) { Some(self.parse_span_type()?) } else { None };

                parameters.push(Parameter { type_ref, name, kind });

                if !self.eat(K::Comma) {
                    break;
                }
            }
        }
        self.expect(K::RightParenthesis, Some("Lambda parameters are ended by a right bracket"))?;
        self.expect(K::EvaluateTo, Some("Lambda bodies are started by '=>'"))?;

        let body = if self.eat(K::Do) {
            let statements: Vec<Statement> = self.parse_block(&[K::ConcludeStatement], span)?;
            self.expect_conclusion("A Lambda block body must be ended by `conclude`")?;
            LambdaBody::Block(statements)
        } else {
            LambdaBody::Expression(Box::new(self.parse_expression()?))
        };

        Ok(Expression::new(ExpressionKind::Lambda(Lambda { parameters, body }), span))
    }

    // ---------------------------- //
    // --------- TYPES ------------ //
    // ---------------------------- //

    /// Parses a type from an identifier token. If it is not a valid Maxtype, it is assumed an enum type.
    /// Ensuring that the type name exists is done later
    fn parse_type(&mut self) -> Result<TypeRef, MaxDiagnostic> {
        let token = self.expect::<&str>(K::Identifier, None)?;

        match token.value.as_str() {
            "int" => Ok(TypeRef::Int),
            "float" => Ok(TypeRef::Float),
            "string" => Ok(TypeRef::Str),
            "boolean" => Ok(TypeRef::Boolean),

            "coroutine" => Ok(TypeRef::Coroutine),

            "list" => self.parse_list_type(),
            "dictionary" => self.parse_dictionary_type(),

            // Anything else is assumed to be an enum. The compiler reports it if it isn't
            other => Ok(TypeRef::Named(other.to_string())),
        }
    }

    /// A variant of `parse_type` that returns the `TypeRef` in a `Spanned` holding the found identifier Span
    fn parse_span_type(&mut self) -> Result<Spanned<TypeRef>, MaxDiagnostic> {
        let token = self.expect::<&str>(K::Identifier, None)?;

        let span = token.span;

        match token.value.as_str() {
            "int" => Ok(Spanned{node: TypeRef::Int, span}),
            "float" => Ok(Spanned{node: TypeRef::Float, span}),
            "string" => Ok(Spanned{node: TypeRef::Str, span}),
            "boolean" => Ok(Spanned{node: TypeRef::Boolean, span}),

            "coroutine" => Ok(Spanned{node: TypeRef::Coroutine, span}),

            "list" => self.parse_spanned_list_type(span),
            "dictionary" => self.parse_spanned_dictionary_type(span),

            // Anything else is assumed to be an enum. The compiler reports it if it isn't
            other => Ok(Spanned{node: TypeRef::Named(other.to_string()), span}),
        }
    }

    /// `list`, `list of int`, or `list<int>` are all valid
    fn parse_list_type(&mut self) -> Result<TypeRef, MaxDiagnostic> {
        if self.eat(K::Of) {
            let element = self.parse_type()?;
            return Ok(TypeRef::List(Some(Box::new(element))));
        }
        if self.eat(K::Lesser) {
            let element = self.parse_type()?;
            
            self.expect(K::Greater, Some("Close a short iterable element type with '>'"))?;
            return Ok(TypeRef::List(Some(Box::new(element))));
        }
        Ok(TypeRef::List(None))
    }

    /// A variant of `parse_list_type`, returning the `TypeRef` in a `Spanned`
    fn parse_spanned_list_type(&mut self, span: Span) -> Result<Spanned<TypeRef>, MaxDiagnostic> {
        if self.eat(K::Of) {
            // Get the current identifiers span first
            // This is more accurate than using the span parameter given
            let span = self.current_safe()?.span;
            let element = self.parse_type()?;
            return Ok(Spanned{node: TypeRef::List(Some(Box::new(element))), span});
        }
        if self.eat(K::Lesser) {
            let span = self.current_safe()?.span;
            let element = self.parse_type()?;
            
            self.expect(K::Greater, Some("Close a short iterable element type with '>'"))?;
            return Ok(Spanned{node: TypeRef::List(Some(Box::new(element))), span});
        }
        // Simply the current span
        Ok(Spanned{node: TypeRef::List(None), span})
    }

    /// `dictionary`, `dictionary of (string, int)`, or `dictionary<string, int>` are all valid
    fn parse_dictionary_type(&mut self) -> Result<TypeRef, MaxDiagnostic> {
        if self.eat(K::Of) {
            self.expect(K::LeftParenthesis, Some("Start a long dictionary key-value type with '('"))?;
            let key = self.parse_type()?;
            self.expect(K::Comma, Some("The key and value types for a dictionary type are seperated with a comma"))?;

            let value = self.parse_type()?;
            self.expect(K::RightParenthesis, Some("Close a long dictionary key-value type with ')'"))?;

            return Ok(TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))));
        }
        if self.eat(K::Lesser) {
            let key = self.parse_type()?;
            self.expect(K::Comma, Some("The key and value types for a dictionary type are seperated with a comma"))?;

            let value = self.parse_type()?;
            self.expect(K::Greater, Some("Close a short dictionary key-value type with '>'"))?;

            return Ok(TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))));
        }
        Ok(TypeRef::Dictionary(None, None))
    }

    /// A variant of `parse_dictionary_type` that returns the `TypeRef`  in a `Spanned`
    fn parse_spanned_dictionary_type(&mut self, span: Span) -> Result<Spanned<TypeRef>, MaxDiagnostic> {
        if self.eat(K::Of) {
            self.expect(K::LeftParenthesis, Some("Start a long dictionary key-value type with '('"))?;
            let key = self.parse_type()?;
            self.expect(K::Comma, Some("The key and value types for a dictionary type are seperated with a comma"))?;

            let value = self.parse_type()?;
            self.expect(K::RightParenthesis, Some("Close a long dictionary key-value type with ')'"))?;

            return Ok(Spanned{node: TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))), span});
        }
        if self.eat(K::Lesser) {
            let key = self.parse_type()?;
            self.expect(K::Comma, Some("The key and value types for a dictionary type are seperated with a comma"))?;

            let value = self.parse_type()?;
            self.expect(K::Greater, Some("Close a short dictionary key-value type with '>'"))?;

            return Ok(Spanned{node: TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))), span});
        }
        Ok(Spanned{node: TypeRef::Dictionary(None, None), span})
    }

    // ---------------------------- //
    // -------- STATEMENTS -------- //
    // ---------------------------- //

    /// Parses statements in general, calling other functions, and returning a final Statement kind
    fn parse_statement(&mut self) -> Result<Statement, MaxDiagnostic> {
        let token = self
            .current_safe()
            .cloned()?;

        // Our span for the statement's beginning
        let span = token.span;

        let kind: StatementKind = match token.kind {
            K::Decl => StatementKind::Declaration(self.parse_declaration()?),
            K::Def => StatementKind::TaskDefinition(self.parse_definition()?),
            K::Model => StatementKind::Model(self.parse_model()?),
            K::Say => {
                self.pos += 1;
                StatementKind::Say(self.parse_expression()?)
            }

            K::IfBegin => StatementKind::If(self.parse_if()?),
            K::While => StatementKind::While(self.parse_while()?),
            K::For => StatementKind::For(self.parse_for()?),
            K::Foreach => StatementKind::Foreach(self.parse_foreach()?),
            K::Match => StatementKind::Match(self.parse_match()?),

            K::Break => {
                self.pos += 1;
                StatementKind::Break
            },
            K::Skip => {
                self.pos += 1;
                StatementKind::Skip
            },

            K::Identifier => self.parse_identifier_statement()?,
            K::Enum => StatementKind::Enum(self.parse_enum()?),

            K::Return => {
                self.pos += 1;
                StatementKind::Return(self.parse_optional_value()?)
            }
            K::Yield => {
                self.pos += 1;
                StatementKind::Yield(self.parse_optional_value()?)
            }
            K::Emit => {
                self.pos += 1;
                let call = self.parse_expression()?;
                if !matches!(call.kind, ExpressionKind::Callable { .. }) {
                    return Err(token.span.error("missing-event-call", "Expected an event call after 'emit'"))
                };
                StatementKind::Emit(call)
            }

            K::Using => StatementKind::Using(self.parse_using()?),

            // Invalid statement kind?
            _ => return Err(token.span.error(
                "statement-error",
                format!("Unexpected token '{}', expected a statement", token.value),
            )),
        };

        Ok(Statement { kind, span })
    }

    /// The value after `return` or `yield`, which may be missing.
    /// Empty return values require a semicolon at the end
    fn parse_optional_value(&mut self) -> Result<Option<Expression>, MaxDiagnostic> {
        if self.at_value_end() {
            // Only a semicolon is consumed. 'conclude' belongs to the enclosing block
            self.eat(K::Semicolon);
            return Ok(None);
        }
        Ok(Some(self.parse_expression()?))
    }

    /// Returns an Assignment Operator depending on what kind of assignment token kind was given
    fn assign_op(kind: K) -> Option<AssignOp> {
        match kind {
            K::Equals => Some(AssignOp::Assign),
            K::PlusEqual => Some(AssignOp::Add),
            K::MinusEqual => Some(AssignOp::Subtract),
            K::MultiplyEqual => Some(AssignOp::Multiply),
            K::DivideEqual => Some(AssignOp::Divide),
            K::ModulusEqual => Some(AssignOp::Modulus),
            _ => None,
        }
    }

    /// Checks for if the current token and the next token are of the same kind,
    /// returning a diagnostic if so
    fn double_token_check(&self, kind: K, code: &'static str, message: impl Into<String>) -> Result<(), MaxDiagnostic> {
        let Some(first) = self.current() else { return Ok(()); };
        let binding = Token::empty();
        let second = self.tokens.get(self.pos + 1).unwrap_or(&binding);

        // Check for the double
        if first.kind == kind && second.kind == kind {
            // Create a span of BOTH tokens
            let span: Span = Span::new(
                first.span.start_line, first.span.start_column, second.span.end_line, second.span.end_column
            );

            return Err(MaxDiagnostic::new(Severity::Error, code, message.into(), span));
        }
        Ok(())
    }

    /// A statement starting with an identifier: an assignment or a bare task call
    fn parse_identifier_statement(&mut self) -> Result<StatementKind, MaxDiagnostic> {
        let target = self.expect::<&str>(K::Identifier, None)?;
        let span = target.span;
        let name = target.value;
        // Start with 'this' or a normal variable
        let start = if target.kind == K::This {
            ExpressionKind::This } else {
            ExpressionKind::Variable(name)
        };
        let target = self.parse_postfix(Expression::new(start, span))?;

        if let Some(op) = self.peek_kind(0).and_then(Self::assign_op) {
            self.pos += 1;
            let value = self.parse_expression()?;
            return Ok(StatementKind::Assignment(Assignment { target, value, op }));
        }

        // '++' and '--' are not allowed in Max \\
        self.double_token_check(K::Plus, "plus-plus-usage", "Use '+=' instead")?;
        self.double_token_check(K::Minus, "minus-minus-usage", "Use '-=' instead")?;

        if matches!(target.kind, ExpressionKind::Callable { .. }) {
            return Ok(StatementKind::Expression(target));
        }

        Err(self.error_here("unknown-identifier-statement", "Expected an assignment or task call."))
    }

    fn parse_declaration(&mut self) -> Result<Declaration, MaxDiagnostic> {
        self.expect::<&str>(K::Decl, None)?;

        let mut modifiers = ModifierSet::default();
        while self.check(K::Modifier) {
            let token = self.consume()?;
            // We check for modifiers...
            match token.value.as_str() {
                "fixed" => {
                    if modifiers.mutability.is_some() {
                        return Err(token.span.error("mutability-overload", "Cannot have more than one mutability modifier on a variable declaration."));
                    }
                    modifiers.mutability = Some(Mutability::Fixed);
                }

                "open" | "monic" => {
                    if modifiers.lifecycle.is_some() {
                        return Err(token.span.error("lifecycle-overload", "Cannot have more than one lifecycle modifier on a variable declaration."));
                    }
                    modifiers.lifecycle = Some(if token.value == "open" { Lifecycle::Open } else { Lifecycle::Monic });
                }

                _ => unreachable!("The Lexer only produces modifier tokens for fixed, open and static"),
            }
        }

        let name_token = self.expect(K::Identifier, Some("A valid name must be entered after the modifiers in a declaration"))?;
        let name_span = name_token.span;
        let name = name_token.value;

        let type_ref = if self.eat(K::As) { Some(self.parse_span_type()?) } else { None };

        let value: Expression;

        // No '=' means automatically a `nil`
        if !self.check(K::Equals) {
            value = Expression::not_real(ExpressionKind::Nil);
            // Fixed variables MUST be initialised first
            if let Some(Mutability::Fixed) = modifiers.mutability {
                name_span.error("uninitialised-fixed-declaration", "A fixed declaration must be initialised first!");
            }
        } else {
            // Consume the '='. We use 'expect' for safety at all times!
            self.expect::<&str>(K::Equals, None)?;
            value = self.parse_expression()?;
        }

        let mut resortable = None;
        if self.eat(K::Comma) {
            self.expect(K::Resort, Some("A comma after a declaration signifies a resort value"))?;
            let resort = self.parse_expression()?;
            if matches!(resort.kind, ExpressionKind::Nil) {
                return Err(self.error_here("resort-nil", "Cannot resort a value to nil"));
            }
            resortable = Some(resort);
        }

        if self.warn_on(K::Semicolon, "unneeded-semicolon", "This semicolon is unnecessary") {
            self.pos += 1; // We don't stop the parser at the semicolon
        }

        Ok(Declaration { name, modifiers, type_ref, value, resortable, name_span })
    }

    fn parse_if(&mut self) -> Result<IfStatement, MaxDiagnostic> {
        let if_span = self.expect::<&str>(K::IfBegin, None)?.span;
        let condition = self.parse_expression()?;
        self.expect(K::Then, Some("An if condition must be followed by 'then'"))?;

        let stops = [K::Elif, K::Maybe, K::Else, K::ConcludeStatement];
        let body = self.parse_block(&stops, if_span)?;
        let first = ConditionalBranch { condition, body };

        // A list of elifs possible, each with their own conditions and block
        let mut elifs = Vec::new();
        while self.eat(K::Elif) {
            let condition = self.parse_expression()?;
            // Reject assignments, and also parse the assignment anyway just to continue parsing
            if self.reject(K::Equals, "assignment-in-condition", "'='  assigns a value; use '==' to compare values") {
                let _ = self.parse_expression();
            }
            self.expect(K::Then, Some("An elif condition must be followed by 'then'"))?;

            let body = self.parse_block(&stops, if_span)?;
            elifs.push(ConditionalBranch { condition, body });
        }

        // There can only be one maybe block and one else block. So they are just Options
        let mut maybe_block = None;
        let mut else_block = None;

        loop {
            let token = self
                .current_safe()
                .cloned()?;

            match token.kind {
                K::Maybe => {
                    if maybe_block.is_some() {
                        return Err(token.span.error("maybe-overload", "Cannot have more than one 'maybe' branch"));
                    }
                    self.pos += 1;
                    maybe_block = Some(self.parse_block(&stops, if_span)?);
                }
                K::Else => {
                    if else_block.is_some() {
                        return Err(token.span.error("else-overload", "Cannot have more than one 'else' branch"));
                    }
                    self.pos += 1;
                                
                    let nested_if = self.check(K::IfBegin);     // remember BEFORE parsing the block

                    match self.parse_block(&stops, if_span) {
                        Ok(block) => else_block = Some(block),
                        Err(mut d) => {
                            if nested_if && d.code == "missing-conclude" {
                                // Add extra detail to the message!
                                d.message.push_str(
                                    ". The 'if' after 'else' opens a nested if with its own 'conclude'; \
                                    use 'elif' to continue this chain, or conclude the nested if");
                            }
                            return Err(d);
                        }
                    }
                }
                K::Elif => {
                    return Err(token.span.error("misplaced-elif", "Cannot have any 'elif' branches after a 'maybe' or 'else' branch"));
                }
                K::ConcludeStatement => break,
                _ => return Err(token.span.error("unknown-if-branch", "Unexpected token after if/elif branches")),
            }
        }

        self.expect_conclusion("Expected a 'conclude' to end an if statement")?;

        Ok(IfStatement {
            first,
            elifs,
            maybe_block: maybe_block.unwrap_or_default(),
            else_block: else_block.unwrap_or_default(),
        })
    }

    /// A while statement is alike running a look with an if statement at the beginning as the loop condition
    fn parse_while(&mut self) -> Result<WhileStatement, MaxDiagnostic> {
        let while_span = self.expect::<&str>(K::While, None)?.span;
        let condition = self.parse_expression()?;
        self.expect(K::Do, Some("A while condition must be followed by 'do'"))?;
        let body = self.parse_block(&[K::ConcludeStatement], while_span)?;

        self.expect_conclusion("Expected a 'conclude' to end a while loop")?;
        Ok(WhileStatement { condition, body })
    }

    /// For statements are specific to integer enumerating
    fn parse_for(&mut self) -> Result<ForStatement, MaxDiagnostic> {
        let for_span = self.expect::<&str>(K::For, None)?.span;
        let name_token = self.expect(K::Identifier, Some("Type a variable name first after 'for'"))?;
        // Even in a for statement, we need a name span...
        let name_span = name_token.span;
        let name = name_token.value;

        self.forbid(K::In, "for-in", "'for' counts between numbers, use `foreach x in` to iterate over a collection")?;

        self.expect(K::Equals, Some("Expected '=' to assign the variable name a value"))?;
        let start = self.parse_expression()?;

        self.expect(K::To, Some("Use 'to' in order to signal where the for loop ends"))?;
        let limit = self.parse_expression()?;

        // Our step is 1 by default
        let step = if self.eat(K::Comma) {
            self.expect(K::Step, Some("A comma after the inital for loop declaration indicates a step value"))?;
            self.parse_expression()?
        } else {
            // Let our span be where the step *would* be 🍕🍕🍕 I got hungry
            let span: Span = self.current_safe()?.span;
            Expression::new(ExpressionKind::Integer(1), span)
        };

        self.expect(K::Do, Some("Expected a 'do' to start a for loop body"))?;
        let body = self.parse_block(&[K::ConcludeStatement], for_span)?;
        self.expect_conclusion("Expected a 'conclude' to end a for loop")?;

        // The declaration for the for statement looper
        let declaration = Declaration {
            name,
            modifiers: ModifierSet::default(),
            type_ref: None,
            value: start,
            resortable: None,
            name_span
        };

        Ok(ForStatement { declaration, limit, step, body })
    }

    /// Foreach statements are specific to looping through an iteration
    fn parse_foreach(&mut self) -> Result<ForeachStatement, MaxDiagnostic> {
        let foreach_span = self.expect::<&str>(K::Foreach, None)?.span;

        /*
        Why a list of variables?
        So collection vars are allowed.
        E.g.

        for (index, item) in listed.enumerate() do
            ...
        conclude

        enumerate() returns a dictionary, so we can get the index (keys) and item (values) with a tuple
        */
        let mut variables = Vec::new();
        if self.eat(K::LeftParenthesis) {
            // foreach (item, index) in ...
            loop {
                variables.push(self.expect(K::Identifier, Some("In variable collection of a foreach condition, a comma indicates another variable name"))?.value);
                if !self.eat(K::Comma) {
                    break;
                }
            }
            self.expect(K::RightParenthesis, Some("Foreach variable collection must be ended by ')'"))?;
        } else {
            variables.push(self.expect(K::Identifier, Some("Type a variable name first in a foreach loop condition"))?.value);
        }

        self.expect(K::In, Some("Follow the declared name by 'in'"))?;
        let iterable = self.parse_expression()?;
        self.expect(K::Do, Some("Expected a 'do' to start foreach loop body"))?;
        let body = self.parse_block(&[K::ConcludeStatement], foreach_span)?;
        self.expect_conclusion("Expected a 'conclude' to end a foreach loop body")?;

        Ok(ForeachStatement { variables, iterable, body })
    }

    fn parse_match(&mut self) -> Result<MatchStatement, MaxDiagnostic> {
        self.expect::<&str>(K::Match, None)?;
        let condition = self.parse_expression()?;
        self.expect(K::Then, Some("Expected 'then' after match statement condition"))?;

        let (cases, default) = self.parse_cases()?;

        Ok(MatchStatement { condition, cases, default })
    }

    /// Parses the cases from a match statement, starting after the 'then'.
    /// This returns the Vec of Cases, as well as an Optional 'Else' case as a tuple
    fn parse_cases(&mut self) -> Result<(Vec<Case>, Option<Vec<Statement>>), MaxDiagnostic> {
        // Aka an empty match statement
        if self.eat(K::ConcludeStatement) {
            return Ok((vec![], None));
        }

        // `case` ends the previous body, so cases need no `conclude` of their own
        let stops = [K::Case, K::Else, K::ConcludeStatement];

        let mut cases: Vec<Case> = vec![];
        let mut else_case: Option<Vec<Statement>> = None;

        loop {

            // Very special exception for the one 'else' case
            if let Some(kind) = self.peek_kind(0) && kind == K::Else {
                let else_token = self.consume()?;
                // ONLY Conclude is expected at the end. Meaning the 'else' case MUST be the last case
                else_case = Some(self.parse_block(&[K::ConcludeStatement], else_token.span)?);
                self.expect_conclusion("No other cases can follow an 'else' case; Expected a 'conclude'")?;
                break;
            }

            let case_span = self.expect(K::Case, Some("A match case is started by 'case'"))?.span;
            let expression = self.parse_expression()?;
            self.expect(K::Do, Some("Expected a 'do' after a match case condition"))?;
            let body = self.parse_block(&stops, case_span)?;

            let case: Case = Case {
                condition: expression,
                body
            };
            cases.push(case);

            // Check for if the block stopped at a conclude statement
            if self.eat(K::ConcludeStatement) {
                break;
            }
        }
        
        // NOTE: Due to the 'eat' for both conclude statement checks,
        // This fn always moves immediately unto the next token after the match statement

        Ok((cases, else_case))
    }

    /// Validates the order of a given Vector of `Parameter`.
    /// `Parameter::Optional` must always be after required params,
    /// and `Parameter::Variadic` must always be the last param, with only one existing
    fn validate_parameter_order(params: &Vec<Parameter>) -> Result<(), MaxDiagnostic> {
        let mut seen_optional: bool = false;

        for i in 0..params.len() {
            match &params[i].kind {
                ParameterKind::Required => if seen_optional {
                    return Err(MaxDiagnostic::new(Severity::Error, "required-after-optional", format!("Required parameter '{}' cannot follow an optional parameter", params[i].name.node), params[i].name.span));
                }

                ParameterKind::Optional => seen_optional = true,

                // If it is not the last param
                ParameterKind::Variadic => if i != params.len() - 1 {
                    return Err(MaxDiagnostic::new(Severity::Error, "required-after-optional", format!("Variadic parameter '{}' must be the last parameter", params[i].name.node), params[i].name.span));
                }
            }
        }

        Ok(())
    }

    /// Parses parameters, this always begins by expecting a left parenthesis
    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, MaxDiagnostic> {
        self.expect::<&str>(K::LeftParenthesis, None)?;

        let mut parameters = Vec::new();
        if self.eat(K::RightParenthesis) {
            return Ok(parameters);
        }

        loop {
            let token = self.expect(K::Identifier, Some("A comma in parameters indicates another parameter name"))?;
            let span = token.span;
            let name = Spanned { node: token.value, span};
            let mut kind: ParameterKind = ParameterKind::Required;

            // Symbols to differentiate param types
            if self.eat(K::QuestionMark) {
                kind = ParameterKind::Optional
            }
            else if self.eat(K::Ellipsis) {
                kind = ParameterKind::Variadic
            }

            // Allow 'as' for type specification
            let type_ref = if self.eat(K::As) { Some(self.parse_span_type()?) } else { None };

            parameters.push(Parameter { type_ref, name, kind });

            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect(K::RightParenthesis, Some("Expected ')' to end parameters"))?;

        Self::validate_parameter_order(&parameters)?;

        Ok(parameters)
    }

    fn parse_definition(&mut self) -> Result<TaskDefinition, MaxDiagnostic> {
        let def_span = self.expect::<&str>(K::Def, None)?.span;

        let mut is_native = false;

        // A sneak peek!
        if self.check(K::Identifier) {
            // `native` is only special straight after `define`
            let native_word = self.consume()?;
            if native_word.value != "native" {
                return Err(native_word.span.error("define-not-native", "Identifier after 'define' can only be 'native'"));
            }
            else {
                // 'native' was found, so now it is true!
                is_native = true;
            }
        }

        // We don't check the kind_token here, it's already checked for it's value afterwards
        let kind_token = self.expect::<&str>(K::TaskType, None)?;
        let kind = match kind_token.value.as_str() {
            "function" => TaskKind::Function,
            "coroutine" => TaskKind::Coroutine,
            "event" => TaskKind::Event,
            other => return Err(kind_token.span.error("unknown-tasktype", format!("Unknown task type '{other}'"))),
        };

        let name_token = self.expect(K::Identifier, Some("Required to give a task definition a name"))?;
        let name_span = name_token.span;
        let name = name_token.value;

        let parameters: Vec<Parameter> =
        // '=>'
        if self.eat(K::EvaluateTo) {
            Vec::new() // No params by default
        } else if self.check(K::With) {
            let with = self.consume()?;
            if kind == TaskKind::Coroutine {
                return Err(with.span.error("param-coroutine", "Coroutines cannot intake parameters."));
            }

            self.parse_parameters()?
        } else {
            return Err(self.error_here("task-param-syntax-error", "Expected a task name to be followed by '=>' or parameters ('with')"));
        };

        let return_type = if self.eat(K::Return) {
            Some(self.parse_span_type()?)
        } else {
            None
        };

        if self.eat(K::Semicolon) {
            if !is_native {
                return Err(self.error_here("non-native-task-semicolon", "Cannot use a semicolon to show an empty body for a none-native task"));
            }
            else {
                return Ok(TaskDefinition { kind, name, is_native, parameters, return_type, body: vec![], name_span });
            }
        }

        self.expect(K::Do, Some("Start a task body with 'do'"))?;
        let body = self.parse_block(&[K::ConcludeStatement], def_span)?;
        self.expect_conclusion("Expected a 'conclude' to end a task body")?;

        // Native checks!
        if is_native {
            if kind == TaskKind::Coroutine {
                return Err(kind_token.span.error("native-coroutine", "A coroutine cannot be native"));
            }
            if !body.is_empty() {
                return Err(kind_token.span.error("non-empty-native-task", "A native definition must have an empty body"));
            }
        }

        Ok(TaskDefinition { kind, name, is_native, parameters, return_type, body, name_span })
    }

    fn parse_model(&mut self) -> Result<ModelDefinition, MaxDiagnostic> {
        self.expect::<&str>(K::Model, None)?;
        let model_type = self.expect(K::ModelType, Some("Expected a valid model type after 'model'"))?;
        match model_type.value.as_str() {
            "object" => {
                let object = self.parse_object()?;

                Ok(ModelDefinition::Object(object))
            }

            _ => return Err(model_type.span.error("unsupported-model-type", format!("Model type '{}' is not supported yet", model_type.value)))
        }
    }

    /// Parses right after the model type in a model definition
    fn parse_object(&mut self) -> Result<ObjectDefinition, MaxDiagnostic> {
        let name_token = self.expect(K::Identifier, Some("Object models require a name"))?;
        
        // Same syntax as tasks!
        let parameters: Vec<Parameter> =
        if self.eat(K::EvaluateTo) {
            Vec::new() // No params by default
        } else if self.check(K::With) {
            self.eat(K::With);
            self.parse_parameters()?
        } else {
            return Err(self.error_here("object-param-syntax-error", "Expected an object name to be followed by '=>' or parameters ('with')"));
        };

        let members_span = self.expect(K::Members, Some("Object definition members are started by \"members\""))?.span;

        let (members, end_span): (Vec<ModelMember>, Span) = self.parse_object_members()?;

        let object = ObjectDefinition {
            name: Spanned { node: name_token.value, span: name_token.span },
            span: members_span.to(end_span),
            parameters,
            members
        };

        Ok(object)
    }

    /// Parses what's after "members" in an object. This returns both the final
    /// Vector of `ModelMember`s and the final `Span` at the *conclude*
    fn parse_object_members(&mut self) -> Result<(Vec<ModelMember>, Span), MaxDiagnostic> {
        let mut members: Vec<ModelMember> = Vec::new();

        while !self.check(K::ConcludeStatement) {
            let attributes: Attributes = self.parse_attributes()?;
            let now = self.current_safe()?;
            let model_member: ModelMember = match &now.kind {
                K::Decl => ModelMember::Field(FieldMember { attributes, declaration: self.parse_declaration()? }),
                K::Def => ModelMember::Task(TaskMember { attributes, definition: self.parse_definition()? }),

                _ => return Err(now.span.error("invalid-object-member", format!("Cannot create an object member from '{}'", now.value)))
            };

            members.push(model_member);
        };

        let end_span = self.expect_conclusion("Expected 'conclude' after an object model definition")?.span;

        Ok((members, end_span))
    }

    /// Parses the attributes within a model. This does not require the attribute
    /// syntax to exist, and if it does not, then the default Attributes are returned.
    /// This method allows stacking attributes like so:
    /// 
    /// ```text
    /// #[static]
    /// #[private]
    /// ...
    /// ```
    fn parse_attributes(&mut self) -> Result<Attributes, MaxDiagnostic> {
        // Start as default, so if no '#' is found, then fall through anyway
        let mut attributes: Attributes = Attributes { visibility: None, is_static: false };

        // NOW we loop!
        // Looping means consecutive attributes work together on one field
        while self.eat(K::Hashtag) {
            self.expect(K::LeftBracket, Some("Expected '[' to start attribute syntax"))?;
            while let Ok(found) = self.consume() {
                if found.kind != K::Identifier {
                    return Err(found.span.error("unknown-attribute", format!("Attribute name was not a valid identifier: '{}'", found.value)));
                }

                match found.value.as_str() {
                    "public" => if attributes.visibility.is_none() {
                        attributes.visibility = Some(Visibility::Public);
                    } else { return Err(found.span.error("duplicate-visibility", "Visibility attribute is already given")); }
                    "private" => if attributes.visibility.is_none() {
                        attributes.visibility = Some(Visibility::Private);
                    } else { return Err(found.span.error("duplicate-visibility", "Visibility attribute is already given")); }
                    "protected" => if attributes.visibility.is_none() {
                        attributes.visibility = Some(Visibility::Protected);
                    } else { return Err(found.span.error("duplicate-visibility", "Visibility attribute is already given")); }
                    
                    "static" => if !attributes.is_static {
                        attributes.is_static = true;
                    } else { return Err(found.span.error("duplicate-static", "Static attribute is already given")); }

                    _ => return Err(found.span.error("unknown-attribute", format!("Unknown attribute given: '{}'", found.value)))
                }
                
                // Click and collect!
                // The 'eat' happens first, so the parser moves up to where the RBrack would be anyway
                // This allows trailing commas
                if self.eat(K::Comma) && self.current_safe()?.kind != K::RightBracket {
                    continue;
                } else { self.eat(K::RightBracket); break; }
            }
        }

        Ok(attributes)
    }

    fn parse_enum(&mut self) -> Result<EnumDefinition, MaxDiagnostic> {
        self.expect::<&str>(K::Enum, None)?;
        let ident_token = self.expect(K::Identifier, Some("An enum must initialise with a name"))?;
        let name_span = ident_token.span;
        let identifier = ident_token.value;
        // Test for pascal case
        if !Casing::is_pascal_case(&identifier) {
            let suggestion = Casing::to_pascal_case(&identifier);
            if !suggestion.is_empty() { // a name like `___` has nothing to suggest, so technically nothing to warn about
                self.diagnostics.push(name_span.warning(
                    "pascal-case-recommendation",
                    format!("Enum name `{identifier}` is recommended to be PascalCase (`{suggestion}`)"),
                ));
            }
        };

        self.expect(K::Members, Some("Define the body of members with 'members'"))?;
        // Enum members parsed in a func to allow recursive sub-members
        let members = self.parse_enum_members()?;
        Ok(EnumDefinition { identifier, members, name_span })
    }

    /// Parses members up to and including their closing `conclude`.
    /// A member may have `$ members ... conclude` (parsed recursively) or `$ OtherEnum`.
    fn parse_enum_members(&mut self) -> Result<Vec<EnumMember>, MaxDiagnostic> {
        let mut members = Vec::new();
        let mut enumerable = 0;

        while !self.check(K::ConcludeStatement) {
            let name_token = self.expect(K::Identifier, Some("A comma in an enum always signifies another enum member"))?;
            let span = name_token.span;
            let name = name_token.value;

            let mut sub_members = Vec::new();
            let mut sub_enum_reference = None;

            if self.eat(K::Dollar) {
                if self.eat(K::Members) {
                    sub_members = self.parse_enum_members()?;
                } else {
                    sub_enum_reference = Some(self.expect(K::Identifier, Some("Give a valid sub enum reference if not sub-members"))?.value);
                    self.forbid(K::Dollar, "sub-reference-members", "Cannot create sub members for an sub-enum reference")?;
                }
            }

            members.push(EnumMember { name, enumerable, sub_members, sub_enum_reference, span });
            enumerable += 1;

            if !self.eat(K::Comma) {
                break;
            }
        }

        self.expect_conclusion("Expected a 'conclude' to end an enum body")?;
        Ok(members)
    }

    /// `using module;` or `using module.member.member;`
    fn parse_using(&mut self) -> Result<UsingStatement, MaxDiagnostic> {
        self.expect::<&str>(K::Using, None)?;

        let module_token = self.expect(K::Identifier, Some("Expected a valid identifier following 'using'"))?;
        let module_span: Span = module_token.span;
        let module = module_token.value;

        // Push members in to allow grabbing specific things from a module
        let mut members = Vec::new();
        while !self.check(K::Semicolon) {
            self.expect(K::Dot, Some("Expected a dot for member access. Use ';' to end the using statement"))?;
            members.push(self.expect(K::Identifier, Some("A dot indicates a name to member access from a module"))?.value);
        }
        self.expect::<&str>(K::Semicolon, None)?;

        Ok(UsingStatement { module, members, module_span })
    }
}