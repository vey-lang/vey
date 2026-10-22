//! The markup of a component.

use crate::ast::expr::{Expr, Pattern};
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub enum MarkupNode {
    Element(Element),
    Expr(Expr),
    If(MIf),
    For(MFor),
    Match(MMatch),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub tag: String,
    pub component: bool,
    pub classes: Vec<String>,
    pub id: Option<String>,
    pub attrs: Vec<(String, Expr)>,
    /// Positional arguments of a component call, e.g. `Card("title")`.
    pub args: Vec<Expr>,
    /// The name of the slot the element goes in, when it is not the default
    /// one.
    pub slot: Option<String>,
    pub children: Vec<MarkupNode>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct MIf {
    pub test: Expr,
    pub then_nodes: Vec<MarkupNode>,
    pub else_nodes: Vec<MarkupNode>,
}

#[derive(Debug, Clone)]
pub struct MFor {
    pub name: String,
    /// The place of the item in the list, when it is wanted.
    pub index: Option<String>,
    pub iter: Expr,
    /// The name that tells the items of the list apart, so that a list may
    /// change without the page being built again.
    pub key: Option<Expr>,
    pub body: Vec<MarkupNode>,
}

/// A `match` in the markup.
#[derive(Debug, Clone)]
pub struct MMatch {
    pub subject: Expr,
    pub arms: Vec<MMatchArm>,
}

#[derive(Debug, Clone)]
pub struct MMatchArm {
    pub patterns: Vec<Pattern>,
    pub guard: Option<Expr>,
    pub body: Vec<MarkupNode>,
}
