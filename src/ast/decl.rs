//! The declarations of a program.

use crate::ast::expr::Expr;
use crate::ast::markup::MarkupNode;
use crate::ast::pattern::BindingPattern;
use crate::ast::stmt::Stmt;
use crate::ast::types::TypeExpr;
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Param {
    /// The name of the parameter, or a pattern that takes it apart.
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub default: Option<Expr>,
    pub rest: bool,
    pub pos: Pos,
}

impl Param {
    /// The name of the parameter, when it is a simple one.
    pub fn name(&self) -> Option<&str> {
        match &self.pattern {
            BindingPattern::Name { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The names the parameter binds.
    pub fn bindings(&self) -> Vec<(&str, Pos)> {
        self.pattern.bindings()
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub kind: VarKind,
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub init: Option<Expr>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    Let,
    Const,
}

/// A type parameter of a function or a type alias.
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    /// The type the parameter has to satisfy, for example `T: number`.
    pub constraint: Option<TypeExpr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Block,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ComponentDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub children: Vec<MarkupNode>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub test: Expr,
    pub then_block: Block,
    pub else_branch: Option<Box<Stmt>>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub test: Expr,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub iter: Expr,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct RepeatStmt {
    pub count: Expr,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct TryStmt {
    pub body: Block,
    pub name: Option<String>,
    pub catch: Option<Block>,
    /// The block that always runs, with or without an error.
    pub finally: Option<Block>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct UseStmt {
    pub path: String,
    pub alias: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct StyleDecl {
    pub css: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub ty: TypeExpr,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<EnumField>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumField {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumBinding {
    pub field: Option<String>,
    pub name: String,
    pub pos: Pos,
}
