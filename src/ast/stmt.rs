//! The statements of a program.

use crate::ast::decl::*;
use crate::ast::expr::*;
use crate::ast::pattern::BindingPattern;
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub target: Expr,
    pub op: String,
    pub value: Expr,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Var(VarDecl),
    Fn(FnDecl),
    Component(ComponentDecl),
    Enum(EnumDecl),
    Interface(InterfaceDecl),
    Impl(ImplDecl),
    If(IfStmt),
    IfLet(IfLetStmt),
    While(WhileStmt),
    WhileLet(WhileLetStmt),
    LetElse(LetElseStmt),
    Defer(DeferStmt),
    For(ForStmt),
    Repeat(RepeatStmt),
    Return(ReturnStmt),
    Break(Option<String>, Pos),
    Continue(Option<String>, Pos),
    Throw(ThrowStmt),
    Try(TryStmt),
    Use(UseStmt),
    Style(StyleDecl),
    Type(TypeDecl),
    Expr(ExprStmt),
    Assign(AssignStmt),
    Block(Block, Pos),
}

impl Stmt {
    pub fn pos(&self) -> Pos {
        match self {
            Stmt::Var(s) => s.pos,
            Stmt::Fn(s) => s.pos,
            Stmt::Component(s) => s.pos,
            Stmt::Enum(s) => s.pos,
            Stmt::Interface(s) => s.pos,
            Stmt::Impl(s) => s.pos,
            Stmt::If(s) => s.pos,
            Stmt::IfLet(s) => s.pos,
            Stmt::While(s) => s.pos,
            Stmt::WhileLet(s) => s.pos,
            Stmt::LetElse(s) => s.pos,
            Stmt::Defer(s) => s.pos,
            Stmt::For(s) => s.pos,
            Stmt::Repeat(s) => s.pos,
            Stmt::Return(s) => s.pos,
            Stmt::Break(_, p) => *p,
            Stmt::Continue(_, p) => *p,
            Stmt::Throw(s) => s.pos,
            Stmt::Try(s) => s.pos,
            Stmt::Use(s) => s.pos,
            Stmt::Style(s) => s.pos,
            Stmt::Type(s) => s.pos,
            Stmt::Expr(s) => s.pos,
            Stmt::Assign(s) => s.pos,
            Stmt::Block(_, p) => *p,
        }
    }

    pub fn exported_name(&self) -> Option<(&str, bool)> {
        match self {
            Stmt::Var(s) => match &s.pattern {
                BindingPattern::Name { name, .. } => Some((name.as_str(), s.exported)),
                _ => None,
            },
            Stmt::Fn(s) => Some((s.name.as_str(), s.exported)),
            Stmt::Component(s) => Some((s.name.as_str(), s.exported)),
            Stmt::Enum(s) => Some((s.name.as_str(), s.exported)),
            Stmt::Interface(s) => Some((s.name.as_str(), s.exported)),
            // types are compile-time only; not exported as runtime values
            _ => None,
        }
    }
}
