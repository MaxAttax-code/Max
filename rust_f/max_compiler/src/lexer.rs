use crate::{error::MaxError, token::{Token, TokenKind as K}};

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    column: u32,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();

        Lexer {
            chars,
            pos: 0,
            line: 1,
            column: 1
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, MaxError> {
        let mut tokens: Vec<Token> = vec![];

        while self.pos < self.chars.len() {
            let c: char = self.chars[self.pos];
            
            // Whitespace = bye
            if c.is_whitespace() {
                self.advance();
                continue;
            }

            match c {
                '"' => tokens.push(self.read_string()?),

                // A number
                c if c.is_ascii_digit() => tokens.push(self.read_number()),

                // Read a word, underscores included
                c if c.is_alphabetic() || c == '_' => tokens.push(self.read_word()),

                // These all allow '=' after them
                // Where is '/'? Well since slashes may indicate a comment...
                '+' | '-' | '*' | '%' => tokens.push(self.read_arithmetic(c)),

                // It has it's own kind of method
                '/' => {
                    // A '/' may start a comment instead of being a token
                    if let Some(token) = self.read_slash()? {
                        tokens.push(token);
                    }
                }

                // In the end, the leftover tokens are all just symbol kinds
                _ => tokens.push(self.read_symbol(c)?),
            }
        }

        Ok(tokens)
    }

    // ---------------------------- //
    // ----- CURSOR HELPERS ------- //
    // ---------------------------- //

    /// Looks at the character `offset` places ahead. An offset of 0 is the current character.
    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    /// Consumes the current character and returns it. This is the only place that moves `pos`,
    /// so line and column stay correct everywhere.
    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;

        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(c)
    }

    /// Consumes the current character only if it equals `expected`
    fn eat(&mut self, expected: char) -> bool {
        if self.peek_at(0) == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Checks if the upcoming characters match the given text
    fn matches(&self, s: &str) -> bool {
        s.chars().enumerate().all(|(i, ch)| self.peek_at(i) == Some(ch))
    }

    /// The current (line, column). Grab this *before* consuming a token, so the token points at its start.
    fn here(&self) -> (u32, u32) {
        (self.line, self.column)
    }

    fn make_token(kind: K, value: impl Into<String>, start: (u32, u32)) -> Token {
        Token::new(kind, value, start.0, start.1)
    }

    // ---------------------------- //
    // -------- READERS ----------- //
    // ---------------------------- //

    fn read_string(&mut self) -> Result<Token, MaxError> {
        let start = self.here();
        self.advance(); // Opening '"'

        let mut value = String::new();

        loop {
            match self.advance() {
                Some('"') => break, // Closing '"', they're like twins!
                Some(c) => value.push(c),
                None => {
                    return Err(MaxError::Syntax {
                        message: String::from("Unterminated string literal."),
                        line: start.0,
                        column: start.1,
                    });
                }
            }
        }

        Ok(Self::make_token(K::StringLiteral, value, start))
    }

    fn read_number(&mut self) -> Token {
        let start = self.here();
        // It's a string since we are adding digits, not numeric values
        let mut number = String::new();

        // Input digits into this numr currently
        self.read_digits_into(&mut number);

        // A '.' only belongs to the number when a digit follows it.
        // That way `5.abs` lexes as Integer, Dot, Identifier instead of a broken float.
        let mut is_float = false;
        if self.peek_at(0) == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            number.push('.');
            
            self.advance();
            self.read_digits_into(&mut number);
        }

        // Decide... is the cake a lie?
        let kind = if is_float { K::Float } else { K::Integer };
        Self::make_token(kind, number, start)
    }

    fn read_digits_into(&mut self, out: &mut String) {
        while let Some(c) = self.peek_at(0) {
            if !c.is_ascii_digit() {
                break;
            }
            out.push(c);
            self.advance();
        }
    }

    fn read_word(&mut self) -> Token {
        let start = self.here();
        let mut word = String::new();

        // Letters, digits, or underscores
        while let Some(c) = self.peek_at(0) {
            if !(c.is_alphanumeric() || c == '_') {
                break;
            }
            word.push(c);
            self.advance();
        }

        // All the keywords we want here
        let kind = match word.as_str() {
            "using" => K::Using,

            "declare" => K::Declare,
            "resort" => K::Resort,
            "say" => K::Say,
            "define" => K::Define,
            "enum" => K::Enum,

            "function" | "coroutine" => K::TaskType,
            "static" | "open" | "fixed" => K::Modifier,

            "true" | "false" | "possible" => K::Boolean,

            "if" => K::IfBegin,
            "then" => K::Then,
            "elif" => K::Elif,
            "else" => K::Else,
            "maybe" => K::Maybe,
            "conclude" => K::ConcludeStatement,
            "return" => K::Return,
            "yield" => K::Yield,

            "while" => K::While,
            "for" => K::For,
            "foreach" => K::Foreach,
            "do" => K::Do,

            "nil" => K::Nil,
            "as" => K::As,
            "not" => K::Not,
            "and" => K::And,
            "or" => K::Or,
            "to" => K::To,
            "step" => K::Step,
            "of" => K::Of,
            "in" => K::In,
            "with" => K::With,
            "members" => K::Members,

            _ => K::Identifier,
        };

        Self::make_token(kind, word, start)
    }

    /// Handles + - * % and their compound assignment forms (+= -= *= %=)
    fn read_arithmetic(&mut self, op: char) -> Token {
        let start = self.here();
        self.advance(); // The operator itself

        let with_equals = self.eat('=');

        let (kind, text) = match (op, with_equals) {
            ('+', false) => (K::Plus, "+"),
            ('+', true) => (K::PlusEqual, "+="),
            ('-', false) => (K::Minus, "-"),
            ('-', true) => (K::MinusEqual, "-="),
            ('*', false) => (K::Multiply, "*"),
            ('*', true) => (K::MultiplyEqual, "*="),
            ('/', false) => (K::Divide, "/"),
            ('/', true) => (K::DivideEqual, "/="),
            ('%', false) => (K::Modulus, "%"),
            ('%', true) => (K::ModulusEqual, "%="),
            _ => unreachable!("read_arithmetic was called with '{op}'"),
        };

        Self::make_token(kind, text, start)
    }

    /// Handles a '/': a block comment, a line comment, or the divide operators.
    /// Returns None when a comment was skipped.
    fn read_slash(&mut self) -> Result<Option<Token>, MaxError> {
        // 1. Block comment: /// ... \\\
        if self.matches("///") {
            self.skip_block_comment()?;
            return Ok(None);
        }

        // 2. Line comment: //
        if self.matches("//") {
            self.skip_line_comment();
            return Ok(None);
        }

        // 3. Regular division: / or /=
        Ok(Some(self.read_arithmetic('/')))
    }

    /// Handles everything that is one or two fixed characters:
    /// = == =>   < <=   > >=   ! !=   brackets   and punctuation
    fn read_symbol(&mut self, c: char) -> Result<Token, MaxError> {
        let start = self.here();
        self.advance(); // The first character

        let (kind, text) = match c {
            '=' => {
                if self.eat('=') {
                    (K::Comparator, "==")
                } else if self.eat('>') {
                    (K::EvaluateTo, "=>")
                } else {
                    (K::Equals, "=")
                }
            }

            '<' => {
                if self.eat('=') { (K::LesserEqual, "<=") } else { (K::Lesser, "<") }
            }

            '>' => {
                if self.eat('=') { (K::GreaterEqual, ">=") } else { (K::Greater, ">") }
            }

            '!' => {
                if self.eat('=') { (K::NotEqualTo, "!=") } else { (K::Not, "!") }
            }

            '(' => (K::LeftParenthesis, "("),
            ')' => (K::RightParenthesis, ")"),
            '[' => (K::LeftBracket, "["),
            ']' => (K::RightBracket, "]"),
            '{' => (K::LeftCurlyBracket, "{"),
            '}' => (K::RightCurlyBracket, "}"),

            ',' => (K::Comma, ","),
            ':' => (K::Colon, ":"),
            ';' => (K::Semicolon, ";"),
            '.' => (K::Dot, "."),
            '$' => (K::Dollar, "$"),

            _ => {
                return Err(MaxError::Syntax {
                    message: format!("Unknown character: '{c}'"),
                    line: start.0,
                    column: start.1,
                });
            }
        };

        Ok(Self::make_token(kind, text, start))
    }

    // ---------------------------- //
    // -------- COMMENTS ---------- //
    // ---------------------------- //

    fn skip_line_comment(&mut self) {
        // Stop at the newline, and leave it for the whitespace handling
        while let Some(c) = self.peek_at(0) {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), MaxError> {
        for _ in 0..3 {
            self.advance(); // The opening ///
        }

        // Three backslashes close the comment
        while self.pos < self.chars.len() && !self.matches("\\\\\\") {
            self.advance();
        }

        // You must must, like MUST, close the comment
        if self.pos >= self.chars.len() {
            return Err(MaxError::EndOfFile(String::from(
                "Unterminated block comment, please end a block comment with '\\\\\\'",
            )));
        }

        // Advance past the \\\
        for _ in 0..3 {
            self.advance(); // The closing \\\
        }

        Ok(())
    }
}