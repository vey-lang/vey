pub mod ast;
pub mod builtins;
pub mod check;
pub mod codegen;
pub mod compiler;
pub mod lexer;
pub mod parser;
pub mod sourcemap;
pub mod types;

use std::fmt;

#[derive(Debug, Clone)]
pub struct VeyError {
    pub message: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub hint: Option<String>,
}

impl VeyError {
    pub fn new(
        message: impl Into<String>,
        file: impl Into<String>,
        line: usize,
        col: usize,
    ) -> Self {
        Self {
            message: message.into(),
            file: file.into(),
            line,
            col,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for VeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.col, self.message
        )
    }
}

impl std::error::Error for VeyError {}

pub fn format_error(err: &VeyError, src: &str) -> String {
    let mut out = format!("{}:{}:{}: {}", err.file, err.line, err.col, err.message);
    if let Some(line) = src.lines().nth(err.line.saturating_sub(1)) {
        out.push_str("\n\n  ");
        out.push_str(line);
        out.push_str("\n  ");
        out.push_str(&" ".repeat(err.col.saturating_sub(1)));
        out.push('^');
    }
    if let Some(hint) = &err.hint {
        out.push_str("\n\n  hint: ");
        out.push_str(hint);
    }
    out
}
