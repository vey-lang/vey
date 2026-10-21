//! The markup of a component.

use crate::ast::expr::Expr;
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub enum MarkupNode {
    Element(Element),
    Expr(Expr),
    If(MIf),
    For(MFor),
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
    pub iter: Expr,
    pub body: Vec<MarkupNode>,
}
