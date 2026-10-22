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
#[derive(Clone)]
pub(crate) struct Binding {
    pub(crate) kind: BindingKind,
    pub(crate) pos: Pos,
    /// Whether the name was read.
    ///
    /// A name that is written and never read is not used, and the compiler
    /// says so. The mark is shared by every copy of the name, so that a
    /// read in one place is seen in every other.
    pub(crate) used: std::rc::Rc<std::cell::Cell<bool>>,
}

impl Binding {
    pub(crate) fn read(&self) {
        self.used.set(true);
    }
}

/// The names of a program, in the scopes they belong to.
pub(crate) struct Scopes {
    scopes: Vec<HashMap<String, std::rc::Rc<Binding>>>,
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
            .cloned();
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
            scope.insert(
                name.to_string(),
                std::rc::Rc::new(Binding {
                    kind,
                    pos,
                    used: std::rc::Rc::new(std::cell::Cell::new(false)),
                }),
            );
        }
        Ok(())
    }

    /// The name, when it is in scope.
    pub(crate) fn resolve(&self, name: &str) -> Option<std::rc::Rc<Binding>> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    /// The name, when it is in scope, and mark it as read.
    pub(crate) fn read(&self, name: &str) -> Option<std::rc::Rc<Binding>> {
        let binding = self.resolve(name)?;
        binding.read();
        Some(binding)
    }

    /// The name that a new one would hide.
    ///
    /// A name that hides another one is allowed, but the compiler says so.
    pub(crate) fn shadowed(&self, name: &str) -> Option<std::rc::Rc<Binding>> {
        let mut scopes = self.scopes.iter().rev();
        let first = scopes.next()?;
        if first.contains_key(name) {
            return None;
        }
        scopes.find_map(|scope| scope.get(name).cloned())
    }

    /// The names of the scope that is about to be left.
    pub(crate) fn take_scope(&mut self) -> Vec<(String, std::rc::Rc<Binding>)> {
        match self.scopes.pop() {
            Some(scope) => scope.into_iter().collect(),
            None => Vec::new(),
        }
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
