//! The gradual type checker.
//!
//! Types are optional in Vey. The checker only looks at code that carries
//! annotations and reports the mismatches it finds; everything else keeps
//! working dynamically.

pub mod builtins;
pub mod checker;
pub mod core;
mod explain;
mod exprs;
pub mod helpers;
mod patterns;
mod resolve;
mod rules;
mod stmts;

pub use checker::{check, ModuleTypeEnv};
pub use core::{FunctionType, ObjectField, Type};
pub use explain::explain;
