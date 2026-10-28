use crate::{
    ast::*,
    error::MaxError,
    token::{Token, TokenKind as K},
};

/// Our glorious parser to convert lexer tokens into statements or expressions.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// All statements (expressions can't be standalon, with the exception of the ExpressionStatement)
    pub fn parse(mut self) -> Result<Vec<Statement>, MaxError> {
        let mut statements = Vec::new();
        while self.pos < self.tokens.len() {
            statements.push(self.parse_statement()?);
        }

        Ok(statements)
    }

    // ---------------------------- //
    // ------ COMMON HELPERS ------ //
    // ---------------------------- //

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// The kind `offset` tokens ahead. An offset of 0 is the current token
    fn peek_kind(&self, offset: usize) -> Option<K> {
        self.tokens.get(self.pos + offset).map(|t| t.kind)
    }

    /// Checks the current token kind with `peek_kind(0)`
    fn check(&self, kind: K) -> bool {
        self.peek_kind(0) == Some(kind)
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

    fn consume(&mut self) -> Result<Token, MaxError> {
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| MaxError::EndOfFile(String::from("Unexpected end of file.")))?;
        self.pos += 1;
        Ok(token)
    }

    /// This returns the current token, checking it's kind. This also consumes the current token, moving up one token into the Parser.
    fn expect(&mut self, kind: K) -> Result<Token, MaxError> {
        let token = self.consume()?;
        if token.kind != kind {
            return Err(Self::error_at(
                &token,
                format!("Expected {:?}, got '{}'", kind, token.value),
            ));
        }
        Ok(token)
    }

    /// A simple Syntax Error message at the pointed token
    fn error_at(token: &Token, message: impl Into<String>) -> MaxError {
        MaxError::Syntax { message: message.into(), line: token.line, column: token.column }
    }

    /// An error pointing at the current token, or the last one when the file has ended
    fn error_here(&self, message: impl Into<String>) -> MaxError {
        match self.tokens.get(self.pos).or_else(|| self.tokens.last()) {
            Some(token) => Self::error_at(token, message),
            None => MaxError::Syntax { message: message.into(), line: 1, column: 1 },
        }
    }

    /// Words that are keywords but can still be used as member names, e.g. `flag.then(...)` or `x.in`
    fn is_contextual_keyword(kind: K) -> bool {
        matches!(
            kind,
            K::Declare | K::Resort | K::Say | K::Define | K::TaskType | K::Boolean | K::IfBegin
                | K::Then | K::Elif | K::Else | K::Maybe | K::ConcludeStatement | K::Return
                | K::Yield | K::While | K::For | K::Foreach | K::Do | K::Nil | K::As | K::Not
                | K::And | K::Or | K::To | K::Step | K::Of | K::In | K::With
                | K::Using | K::Enum | K::Modifier | K::Members | K::Break | K::Skip
                | K::Match | K::Case
        )
    }

    /// Parses statements until one of the `stops` is the current token. The stop is not consumed.
    fn parse_block(&mut self, stops: &[K]) -> Result<Vec<Statement>, MaxError> {
        let mut body: Vec<Statement> = Vec::new();
        while let Some(token) = self.current() {
            if stops.contains(&token.kind) {
                return Ok(body);
            }
            body.push(self.parse_statement()?);
        }
        Err(MaxError::EndOfFile(String::from("Expected 'conclude' before the end of the file.")))
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

    fn parse_expression(&mut self) -> Result<Expression, MaxError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expression, MaxError> {
        let mut left = self.parse_and()?;
        while self.eat(K::Or) {
            let right = self.parse_and()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Logical { left: Box::new(left), op: LogicalOp::Or, right: Box::new(right) }, span);
        }
        
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression, MaxError> {
        let mut left = self.parse_comparison()?;
        while self.eat(K::And) {
            let right = self.parse_comparison()?;
            let span = left.span.clone();
            left = Expression::new(ExpressionKind::Logical { left: Box::new(left), op: LogicalOp::And, right: Box::new(right) }, span);
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression, MaxError> {
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
    fn parse_additive(&mut self) -> Result<Expression, MaxError> {
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
    fn parse_term(&mut self) -> Result<Expression, MaxError> {
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

    /// Same behaviour as the C# parser: `not` takes a WHOLE expression, so `not a == b and c`
    /// means `not (a == b and c)`.
    /// After consuming the 'not', this calls `parse_expression`, otherwise `parse_factor`
    fn parse_unary(&mut self) -> Result<Expression, MaxError> {
        if self.eat(K::Not) {
            let operand = self.parse_expression()?;
            let span = operand.span.clone();
            return Ok(Expression::new(ExpressionKind::Unary { op: UnaryOp::Not, operand: Box::new(operand) }, span));
        }
        self.parse_factor()
    }

    /// This consumes the current token, returning an Expression based on the Token kind.
    /// This performs `parse_postfix` on most branches.
    fn parse_factor(&mut self) -> Result<Expression, MaxError> {
        let token = self.consume()?;

        let span: Span = Span::from(&token);

        match token.kind {
            K::Nil => self.parse_postfix(Expression::new(ExpressionKind::Nil, span)),

            K::Integer => {
                let value = token.value.parse::<i64>().map_err(|_| {
                    Self::error_at(&token, format!("Integer literal '{}' is out of range", token.value))
                })?;
                self.parse_postfix(Expression::new(ExpressionKind::Integer(value), span))
            }

            K::Float => {
                let value = token.value.parse::<f64>().map_err(|_| {
                    Self::error_at(&token, format!("Invalid float literal '{}'", token.value))
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
                    other => return Err(Self::error_at(&token, format!("Unknown boolean value: {other}"))),
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

                self.expect(K::RightParenthesis)?;
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

            _ => Err(Self::error_at(&token, format!("Expected an expression, got '{}'", token.value))),
        }
    }

    /// Handles `.member`, `[index]` and `(arguments)` chains after any expression
    fn parse_postfix(&mut self, mut expression: Expression) -> Result<Expression, MaxError> {
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
                    self.expect(K::RightBracket)?;
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
    fn consume_member_name(&mut self) -> Result<String, MaxError> {
        let token = self.consume()?;
        if token.kind == K::Identifier || Self::is_contextual_keyword(token.kind) {
            Ok(token.value)
        } else {
            Err(Self::error_at(
                &token,
                format!("Expected member identifier after '.', got '{}'", token.value),
            ))
        }
    }

    /// Parses a task call, consuming the left parenthesis,
    /// parsing an expression as an argument, with commas indicating another argument following,
    /// finall consuming the final right parenthesis.
    fn parse_task_call(&mut self, callee: Expression) -> Result<Expression, MaxError> {
        self.expect(K::LeftParenthesis)?;

        let span: Span = Span::from(self.current()
            .unwrap_or_else(|| panic!("Error from attempting to get span during task call"))
        );

        let mut arguments = Vec::new();
        if !self.check(K::RightParenthesis) {
            loop {
                let arg = self.parse_expression()?;
                arguments.push(arg);
                if !self.eat(K::Comma) {
                    break;
                }
            }
        }
        self.expect(K::RightParenthesis)?;

        Ok(Expression::new(ExpressionKind::Call { callee: Box::new(callee), arguments }, span))
    }

    // ---------------------------- //
    // ---- LISTS AND DICTS ------- //
    // ---------------------------- //

    /// The opening '[' is already consumed. A trailing comma is allowed.
    fn parse_list(&mut self) -> Result<Expression, MaxError> {
        let span: Span = Span::from(self.current()
            .unwrap_or_else(|| panic!("Error occurred when attempting to create Span during `parse_list`"))
        );

        let mut items = Vec::new();
        while !self.check(K::RightBracket) {
            items.push(self.parse_expression()?);
            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect(K::RightBracket)?;
        Ok(Expression::new(ExpressionKind::List(items), span))
    }

    /// The opening '{' is already consumed
    fn parse_dictionary(&mut self) -> Result<Expression, MaxError> {
        let span: Span = Span::from(self.current()
            .unwrap_or_else(|| panic!("Error occurred when attempting to create Span during `parse_list`"))
        );
        
        let mut entries = Vec::new();
        while !self.check(K::RightCurlyBracket) {
            let key = self.parse_expression()?;
            self.expect(K::Colon)?; // Colon to seperate key : value

            let value = self.parse_expression()?;
            entries.push((key, value));

            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect(K::RightCurlyBracket)?;
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
    fn parse_lambda(&mut self) -> Result<Expression, MaxError> {
        // Get our span here, right now at the beginning
        let span: Span = Span::from(self.current()
            .unwrap_or_else(|| panic!("Error occurred when attempting to create Span during `parse_list`"))
        );

        let mut parameters = Vec::new();
        if !self.check(K::RightParenthesis) {
            loop {
                parameters.push(self.expect(K::Identifier)?.value);
                if !self.eat(K::Comma) {
                    break;
                }
            }
        }
        self.expect(K::RightParenthesis)?;
        self.expect(K::EvaluateTo)?;

        let body = if self.eat(K::Do) {
            let statements: Vec<Statement> = self.parse_block(&[K::ConcludeStatement])?;
            self.expect(K::ConcludeStatement)?;
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
    fn parse_type(&mut self) -> Result<TypeRef, MaxError> {
        let token = self.expect(K::Identifier)?;

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
    fn parse_span_type(&mut self) -> Result<Spanned<TypeRef>, MaxError> {
        let token = self.expect(K::Identifier)?;

        let span = Span::from(&token);

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
    fn parse_list_type(&mut self) -> Result<TypeRef, MaxError> {
        if self.eat(K::Of) {
            let element = self.parse_type()?;
            return Ok(TypeRef::List(Some(Box::new(element))));
        }
        if self.eat(K::Lesser) {
            let element = self.parse_type()?;
            
            self.expect(K::Greater)?;
            return Ok(TypeRef::List(Some(Box::new(element))));
        }
        Ok(TypeRef::List(None))
    }

    /// A variant of `parse_list_type`, returning the `TypeRef` in a `Spanned`
    fn parse_spanned_list_type(&mut self, span: Span) -> Result<Spanned<TypeRef>, MaxError> {
        if self.eat(K::Of) {
            // Get the current identifiers span first
            // This is more accurate than using the span parameter given
            let span = Span::from(self.current()
                .unwrap_or_else(|| panic!("Attempted to get type name when parsing list type"))
            );
            let element = self.parse_type()?;
            return Ok(Spanned{node: TypeRef::List(Some(Box::new(element))), span});
        }
        if self.eat(K::Lesser) {
            let span = Span::from(self.current()
                .unwrap_or_else(|| panic!("Attempted to get type name when parsing list type"))
            );
            let element = self.parse_type()?;
            
            self.expect(K::Greater)?;
            return Ok(Spanned{node: TypeRef::List(Some(Box::new(element))), span});
        }
        // Simply the current span
        Ok(Spanned{node: TypeRef::List(None), span})
    }

    /// `dictionary`, `dictionary of (string, int)`, or `dictionary<string, int>` are all valid
    fn parse_dictionary_type(&mut self) -> Result<TypeRef, MaxError> {
        if self.eat(K::Of) {
            self.expect(K::LeftParenthesis)?;
            let key = self.parse_type()?;
            self.expect(K::Comma)?;

            let value = self.parse_type()?;
            self.expect(K::RightParenthesis)?;

            return Ok(TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))));
        }
        if self.eat(K::Lesser) {
            let key = self.parse_type()?;
            self.expect(K::Comma)?;

            let value = self.parse_type()?;
            self.expect(K::Greater)?;

            return Ok(TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))));
        }
        Ok(TypeRef::Dictionary(None, None))
    }

    /// A variant of `parse_dictionary_type` that returns the `TypeRef`  in a `Spanned`
    fn parse_spanned_dictionary_type(&mut self, span: Span) -> Result<Spanned<TypeRef>, MaxError> {
        if self.eat(K::Of) {
            self.expect(K::LeftParenthesis)?;
            let key = self.parse_type()?;
            self.expect(K::Comma)?;

            let value = self.parse_type()?;
            self.expect(K::RightParenthesis)?;

            return Ok(Spanned{node: TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))), span});
        }
        if self.eat(K::Lesser) {
            let key = self.parse_type()?;
            self.expect(K::Comma)?;

            let value = self.parse_type()?;
            self.expect(K::Greater)?;

            return Ok(Spanned{node: TypeRef::Dictionary(Some(Box::new(key)), Some(Box::new(value))), span});
        }
        Ok(Spanned{node: TypeRef::Dictionary(None, None), span})
    }

    // ---------------------------- //
    // -------- STATEMENTS -------- //
    // ---------------------------- //

    /// Parses statements in general, calling other functions, and returning a final Statement kind
    fn parse_statement(&mut self) -> Result<Statement, MaxError> {
        let token = self
            .current()
            .cloned()
            .ok_or_else(|| MaxError::EndOfFile(String::from("Expected a statement.")))?;

        // Our span for the statement's beginning
        let span = Span::from(&token);

        let kind: StatementKind = match token.kind {
            K::Declare => StatementKind::Declaration(self.parse_declaration()?),
            K::Define => StatementKind::TaskDefinition(self.parse_definition()?),
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

            K::Using => StatementKind::Using(self.parse_using()?),

            // Invalid statement kind?
            _ => return Err(Self::error_at(
                &token,
                format!("Unexpected token '{}', expected a statement", token.value),
            )),
        };

        Ok(Statement { kind, span })
    }

    /// The value after `return` or `yield`, which may be missing.
    /// Empty return values require a semicolon at the end
    fn parse_optional_value(&mut self) -> Result<Option<Expression>, MaxError> {
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

    /// A statement starting with an identifier: an assignment or a bare task call
    fn parse_identifier_statement(&mut self) -> Result<StatementKind, MaxError> {
        let target = self.expect(K::Identifier)?;
        let span = Span::from(&target);
        let name = target.value;
        let target = self.parse_postfix(Expression::new(ExpressionKind::Variable(name), span))?;

        if let Some(op) = self.peek_kind(0).and_then(Self::assign_op) {
            self.pos += 1;
            let value = self.parse_expression()?;
            return Ok(StatementKind::Assignment(Assignment { target, value, op }));
        }

        if matches!(target.kind, ExpressionKind::Call { .. }) {
            return Ok(StatementKind::Expression(target));
        }

        Err(self.error_here("Expected an assignment or task call."))
    }

    fn parse_declaration(&mut self) -> Result<Declaration, MaxError> {
        self.expect(K::Declare)?;

        let mut modifiers = ModifierSet::default();
        while self.check(K::Modifier) {
            let token = self.consume()?;
            // We check for modifiers...
            match token.value.as_str() {
                "fixed" => {
                    if modifiers.mutability.is_some() {
                        return Err(Self::error_at(&token, "Cannot have more than one mutability modifier on a variable declaration."));
                    }
                    modifiers.mutability = Some(Mutability::Fixed);
                }

                "open" | "static" => {
                    if modifiers.lifecycle.is_some() {
                        return Err(Self::error_at(&token, "Cannot have more than one lifecycle modifier on a variable declaration."));
                    }
                    modifiers.lifecycle = Some(if token.value == "open" { Lifecycle::Open } else { Lifecycle::Static });
                }

                _ => unreachable!("The Lexer only produces modifier tokens for fixed, open and static"),
            }
        }

        let name_token = self.expect(K::Identifier)?;
        let name_span = Span::from(&name_token);
        let name = name_token.value;

        let type_ref = if self.eat(K::As) { Some(self.parse_span_type()?) } else { None };

        self.expect(K::Equals)?;
        let value = self.parse_expression()?;

        let mut resortable = None;
        if self.eat(K::Comma) {
            self.expect(K::Resort)?;
            let resort = self.parse_expression()?;
            if matches!(resort.kind, ExpressionKind::Nil) {
                return Err(self.error_here("Cannot resort a value to nil"));
            }
            resortable = Some(resort);
        }

        Ok(Declaration { name, modifiers, type_ref, value, resortable, name_span })
    }

    fn parse_if(&mut self) -> Result<IfStatement, MaxError> {
        self.expect(K::IfBegin)?;
        let condition = self.parse_expression()?;
        self.expect(K::Then)?;

        let stops = [K::Elif, K::Maybe, K::Else, K::ConcludeStatement];
        let body = self.parse_block(&stops)?;
        let first = ConditionalBranch { condition, body };

        // A list of elifs possible, each with their own conditions and block
        let mut elifs = Vec::new();
        while self.eat(K::Elif) {
            let condition = self.parse_expression()?;
            self.expect(K::Then)?;

            let body = self.parse_block(&stops)?;
            elifs.push(ConditionalBranch { condition, body });
        }

        // There can only be one maybe block and one else block. So they are just Options
        let mut maybe_block = None;
        let mut else_block = None;

        loop {
            let token = self
                .current()
                .cloned()
                .ok_or_else(|| MaxError::EndOfFile(String::from("Expected 'conclude' after if statement.")))?;

            match token.kind {
                K::Maybe => {
                    if maybe_block.is_some() {
                        return Err(Self::error_at(&token, "Cannot have more than one 'maybe' branch"));
                    }
                    self.pos += 1;
                    maybe_block = Some(self.parse_block(&stops)?);
                }
                K::Else => {
                    if else_block.is_some() {
                        return Err(Self::error_at(&token, "Cannot have more than one 'else' branch"));
                    }
                    self.pos += 1;
                    else_block = Some(self.parse_block(&stops)?);
                }
                K::Elif => {
                    return Err(Self::error_at(&token, "Cannot have any 'elif' branches after a 'maybe' or 'else' branch"));
                }
                K::ConcludeStatement => break,
                _ => return Err(Self::error_at(&token, "Unexpected token after if/elif branches")),
            }
        }

        self.expect(K::ConcludeStatement)?;

        Ok(IfStatement {
            first,
            elifs,
            maybe_block: maybe_block.unwrap_or_default(),
            else_block: else_block.unwrap_or_default(),
        })
    }

    /// A while statement is alike running a look with an if statement at the beginning as the loop condition
    fn parse_while(&mut self) -> Result<WhileStatement, MaxError> {
        self.expect(K::While)?;
        let condition = self.parse_expression()?;
        self.expect(K::Do)?;
        let body = self.parse_block(&[K::ConcludeStatement])?;

        self.expect(K::ConcludeStatement)?;
        Ok(WhileStatement { condition, body })
    }

    /// For statements are specific to integer enumerating
    fn parse_for(&mut self) -> Result<ForStatement, MaxError> {
        self.expect(K::For)?;
        let name_token = self.expect(K::Identifier)?;
        // Even in a for statement, we need a name span...
        let name_span = Span::from(&name_token);
        let name = name_token.value;

        self.expect(K::Equals)?;
        let start = self.parse_expression()?;

        self.expect(K::To)?;
        let limit = self.parse_expression()?;

        // Our step is 1 by default
        let step = if self.eat(K::Comma) {
            self.expect(K::Step)?;
            self.parse_expression()?
        } else {
            // Let our span be where the step *would* be 🍕🍕🍕 I got hungry
            let span: Span = Span::from(self.current()
                .unwrap_or_else(|| panic!("Error occurred when attempting to create Span during `parse_list`"))
            );
            Expression::new(ExpressionKind::Integer(1), span)
        };

        self.expect(K::Do)?;
        let body = self.parse_block(&[K::ConcludeStatement])?;
        self.expect(K::ConcludeStatement)?;

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
    fn parse_foreach(&mut self) -> Result<ForeachStatement, MaxError> {
        self.expect(K::Foreach)?;

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
                variables.push(self.expect(K::Identifier)?.value);
                if !self.eat(K::Comma) {
                    break;
                }
            }
            self.expect(K::RightParenthesis)?;
        } else {
            variables.push(self.expect(K::Identifier)?.value);
        }

        self.expect(K::In)?;
        let iterable = self.parse_expression()?;
        self.expect(K::Do)?;
        let body = self.parse_block(&[K::ConcludeStatement])?;
        self.expect(K::ConcludeStatement)?;

        Ok(ForeachStatement { variables, iterable, body })
    }

    fn parse_match(&mut self) -> Result<MatchStatement, MaxError> {
        self.expect(K::Match)?;
        let condition = self.parse_expression()?;
        self.expect(K::Then)?;

        let (cases, default) = self.parse_cases()?;

        Ok(MatchStatement { condition, cases, default })
    }

    /// Parses the cases from a match statement, starting after the 'then'.
    /// This returns the Vec of Cases, as well as an Optional 'Else' case as a tuple
    fn parse_cases(&mut self) -> Result<(Vec<Case>, Option<Vec<Statement>>), MaxError> {
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
            if self.eat(K::Else) {
                // ONLY Conclude is expected at the end. Meaning the 'else' case MUST be the last case
                else_case = Some(self.parse_block(&[K::ConcludeStatement])?);
                self.expect(K::ConcludeStatement)?;
                break;
            }

            self.expect(K::Case)?;
            let expression = self.parse_expression()?;
            self.expect(K::Do)?;
            let body = self.parse_block(&stops)?;

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

    /// Parses parameters, this always begins by expecting a left parenthesis
    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, MaxError> {
        self.expect(K::LeftParenthesis)?;

        let mut parameters = Vec::new();
        if self.eat(K::RightParenthesis) {
            return Ok(parameters);
        }

        loop {
            let token = self.expect(K::Identifier)?;
            let name = token.value;
            // Allow 'as' for type specification
            let type_ref = if self.eat(K::As) { Some(self.parse_span_type()?) } else { None };

            parameters.push(Parameter { type_ref, name });

            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect(K::RightParenthesis)?;

        Ok(parameters)
    }

    fn parse_definition(&mut self) -> Result<TaskDefinition, MaxError> {
        self.expect(K::Define)?;

        let kind_token = self.expect(K::TaskType)?;
        let kind = match kind_token.value.as_str() {
            "function" => TaskKind::Function,
            "coroutine" => TaskKind::Coroutine,
            other => return Err(Self::error_at(&kind_token, format!("Unknown task type '{other}'"))),
        };

        let name_token = self.expect(K::Identifier)?;
        let name_span = Span::from(&name_token);
        let name = name_token.value;

        let parameters =
        // '=>'
        if self.eat(K::EvaluateTo) {
            Vec::new() // No params by default
        } else if self.check(K::With) {
            let with = self.consume()?;
            if kind == TaskKind::Coroutine {
                return Err(Self::error_at(&with, "Coroutines cannot intake parameters."));
            }

            self.parse_parameters()?
        } else {
            return Err(self.error_here("Expected a task name to be followed by '=>' or parameters ('with')"));
        };

        let return_type = if self.eat(K::Return) { Some(self.parse_span_type()?) } else { None };

        self.expect(K::Do)?;
        let body = self.parse_block(&[K::ConcludeStatement])?;
        self.expect(K::ConcludeStatement)?;

        Ok(TaskDefinition { kind, name, parameters, return_type, body, name_span })
    }

    fn parse_enum(&mut self) -> Result<EnumDefinition, MaxError> {
        self.expect(K::Enum)?;
        let ident_token = self.expect(K::Identifier)?;
        let name_span = Span::from(&ident_token);
        let identifier = ident_token.value;

        self.expect(K::Members)?;
        // Enum members parsed in a func to allow recursive sub-members
        let members = self.parse_enum_members()?;
        Ok(EnumDefinition { identifier, members, name_span })
    }

    /// Parses members up to and including their closing `conclude`.
    /// A member may have `$ members ... conclude` (parsed recursively) or `$ OtherEnum`.
    fn parse_enum_members(&mut self) -> Result<Vec<EnumMember>, MaxError> {
        let mut members = Vec::new();
        let mut enumerable = 0;

        while !self.check(K::ConcludeStatement) {
            let name_token = self.expect(K::Identifier)?;
            let span = Span::from(&name_token);
            let name = name_token.value;

            let mut sub_members = Vec::new();
            let mut sub_enum_reference = None;

            if self.eat(K::Dollar) {
                if self.eat(K::Members) {
                    sub_members = self.parse_enum_members()?;
                } else {
                    sub_enum_reference = Some(self.expect(K::Identifier)?.value);
                }
            }

            members.push(EnumMember { name, enumerable, sub_members, sub_enum_reference, span });
            enumerable += 1;

            if !self.eat(K::Comma) {
                break;
            }
        }

        self.expect(K::ConcludeStatement)?;
        Ok(members)
    }

    /// `using module;` or `using module.member.member;`
    fn parse_using(&mut self) -> Result<UsingStatement, MaxError> {
        self.expect(K::Using)?;

        let module_token = self.expect(K::Identifier)?;
        let module_span: Span = Span::from(&module_token);
        let module = module_token.value;

        // Push members in to allow grabbing specific things from a module
        let mut members = Vec::new();
        while !self.check(K::Semicolon) {
            self.expect(K::Dot)?;
            members.push(self.expect(K::Identifier)?.value);
        }
        self.expect(K::Semicolon)?;

        Ok(UsingStatement { module, members, module_span })
    }
}