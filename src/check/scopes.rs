//! The scopes of a program and the names in them.

use std::collections::HashMap;

use crate::ast::VarKind;
use crate::lexer::Pos;
use crate::VeyError;

/// The kind of a name, which says what may be done with it.
#[derive(Clone, Copy)]
pub(crate) enum BindingKind {
    Let,
    Const,
    Param,
    Function,
    Component,
    Import,
}

impl BindingKind {
    pub(crate) fn immutable(self) -> bool {
        matches!(
            self,
            Self::Const | Self::Function | Self::Component | Self::Import
        )
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Function => "function",
            Self::Component => "component",
            Self::Import => "import",
            Self::Let | Self::Param => "variable",
        }
    }

    pub(crate) fn from_var(kind: VarKind) -> Self {
        match kind {
            VarKind::Let => Self::Let,
            VarKind::Const => Self::Const,
        }
    }
}

/// A name that is in scope.
#[derive(Clone, Copy)]
pub(crate) struct Binding {
    pub(crate) kind: BindingKind,
    pub(crate) pos: Pos,
}

/// The names of a program, in the scopes they belong to.
pub(crate) struct Scopes {
    scopes: Vec<HashMap<String, Binding>>,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub(crate) fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(crate) fn pop(&mut self) {
        self.scopes.pop();
    }

    /// Add a name, and report it when it is already taken.
    pub(crate) fn declare(
        &mut self,
        name: &str,
        kind: BindingKind,
        pos: Pos,
        file: &str,
    ) -> Result<(), VeyError> {
        let previous = self
            .scopes
            .last()
            .and_then(|scope| scope.get(name))
            .copied();
        if let Some(previous) = previous {
            return Err(VeyError::new(
                format!(
                    "duplicate declaration '{}' (first declared on line {})",
                    name, previous.pos.line
                ),
                file,
                pos.line,
                pos.col,
            ));
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Binding { kind, pos });
        }
        Ok(())
    }

    /// The name, when it is in scope.
    pub(crate) fn resolve(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    /// How many scopes there are.
    pub(crate) fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// The names that are in scope, used to suggest a similar one.
    pub(crate) fn known_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for scope in &self.scopes {
            for name in scope.keys() {
                names.push(name.as_str());
            }
        }
        names
    }
}
