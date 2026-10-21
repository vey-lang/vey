//! The shape of a program.
//!
//! The tree is split over a few files: `decl` holds the declarations,
//! `expr` the expressions, `stmt` the statements, `pattern` the patterns,
//! `types` the type expressions and `markup` the markup.

mod decl;
mod expr;
mod markup;
mod pattern;
mod stmt;
mod types;

pub use decl::*;
pub use expr::*;
pub use markup::*;
pub use pattern::*;
pub use stmt::*;
pub use types::*;
