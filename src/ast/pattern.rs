//! The patterns that take a value apart.

use crate::ast::Expr;
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub enum BindingPattern {
    Name {
        name: String,
        /// The value that is used when there is none.
        default: Option<Expr>,
        pos: Pos,
    },
    Array {
        items: Vec<BindingPattern>,
        rest: Option<String>,
        pos: Pos,
    },
    Object {
        fields: Vec<BindingField>,
        rest: Option<String>,
        pos: Pos,
    },
}

#[derive(Debug, Clone)]
pub struct BindingField {
    pub key: String,
    pub pattern: BindingPattern,
    pub pos: Pos,
}

impl BindingPattern {
    pub fn bindings(&self) -> Vec<(&str, Pos)> {
        let mut names = Vec::new();
        self.collect_bindings(&mut names);
        names
    }

    fn collect_bindings<'a>(&'a self, names: &mut Vec<(&'a str, Pos)>) {
        match self {
            Self::Name { name, pos, .. } => names.push((name.as_str(), *pos)),
            Self::Array { items, rest, pos } => {
                for item in items {
                    item.collect_bindings(names);
                }
                if let Some(rest) = rest {
                    names.push((rest.as_str(), *pos));
                }
            }
            Self::Object { fields, rest, pos } => {
                for field in fields {
                    field.pattern.collect_bindings(names);
                }
                if let Some(rest) = rest {
                    names.push((rest.as_str(), *pos));
                }
            }
        }
    }
}
