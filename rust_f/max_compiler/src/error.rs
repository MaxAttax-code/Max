use std::fmt::{self};

#[derive(Debug)]
pub enum MaxError {
    Syntax { message: String, line: u32, column: u32 },
    EndOfFile(String),
    Logic(String),
    Compile { message: String, line: u32, column: u32 }
}

impl fmt::Display for MaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaxError::Syntax {message, line, column} => write!(f, "Syntax error: {message} at line {line}, column {column}"),
            MaxError::EndOfFile(m) => write!(f, "{m}"),
            MaxError::Logic(m) => write!(f, "{m}"),
            MaxError::Compile {message, line, column} => write!(f, "Error during compiling: {message} at line {line}, column {column}")
        }
    }
}

impl std::error::Error for MaxError {}