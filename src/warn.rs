//! The warnings of the compiler.
//!
//! A warning is a thing that is not a mistake, but that a reader of the
//! program would want to know about. The compiler collects them while it
//! checks, and the command that ran it prints them.

#[derive(Clone, Debug)]
pub struct Warning {
    pub message: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub hint: Option<String>,
}

impl Warning {
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

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: warning: {}",
            self.file, self.line, self.col, self.message
        )
    }
}

/// The warnings of a program, in the order they were found.
pub type Warnings = Vec<Warning>;
