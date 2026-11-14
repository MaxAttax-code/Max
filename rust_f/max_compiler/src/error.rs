use std::fmt::{self};

use crate::ast::Span;

#[derive(Debug, PartialEq, Default)]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Info,
    Hint
}

#[derive(Debug, Default)]
pub struct MaxDiagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub span: Span
}

impl MaxDiagnostic {
    pub fn new(severity: Severity, code: &'static str, message: String, span: Span) -> Self {
        MaxDiagnostic { severity, code, message, span }
    }
}

impl fmt::Display for MaxDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Max {:?}: {}; at line {}, column {}", self.severity, self.message, self.span.start_line, self.span.start_column)
    }
}

impl std::error::Error for MaxDiagnostic {}

pub struct RuntimeError {
    pub message: String
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}