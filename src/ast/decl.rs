//! The declarations of a program.

use crate::ast::expr::{Expr, Pattern};
use crate::ast::markup::MarkupNode;
use crate::ast::pattern::BindingPattern;
use crate::ast::stmt::Stmt;
use crate::ast::types::TypeExpr;
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Stmt>,
    /// The doc comments of the file, in the order they were written.
    pub docs: Vec<DocComment>,
    /// The version of the language the file was written for.
    pub version: Option<String>,
    /// The names the program keeps to itself.
    pub private: Vec<String>,
}

/// A comment that says what a piece of the program does.
#[derive(Debug, Clone)]
pub struct DocComment {
    pub text: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub body: Vec<Stmt>,
    /// The position of the `{` that opens the block.
    pub start: Pos,
    /// The position of the `}` that closes the block.
    pub end: Pos,
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
    /// The values of the component, which are kept when the page is
    /// written again.
    pub body: Vec<Stmt>,
    pub children: Vec<MarkupNode>,
    pub exported: bool,
    pub pos: Pos,
    /// The `{` that opens the component body.
    pub body_start: Pos,
    /// The `}` that closes the component body.
    pub body_end: Pos,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub test: Expr,
    pub then_block: Block,
    pub else_branch: Option<Box<Stmt>>,
    pub pos: Pos,
}

/// An `if` that takes a value apart.
#[derive(Debug, Clone)]
pub struct IfLetStmt {
    pub pattern: Pattern,
    pub value: Expr,
    pub then_block: Block,
    pub else_branch: Option<Box<Stmt>>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub test: Expr,
    pub body: Block,
    /// The name of the loop, so that a `break` may name it.
    pub label: Option<String>,
    pub pos: Pos,
}

/// A `while` that takes a value apart.
#[derive(Debug, Clone)]
pub struct WhileLetStmt {
    pub pattern: Pattern,
    pub value: Expr,
    pub body: Block,
    pub pos: Pos,
}

/// A value that has to be there.
#[derive(Debug, Clone)]
pub struct LetElseStmt {
    pub pattern: BindingPattern,
    pub value: Expr,
    /// The block that runs when the value is not there.
    pub otherwise: Block,
    pub pos: Pos,
}

/// A piece of code that runs when the block is left.
#[derive(Debug, Clone)]
pub struct DeferStmt {
    pub value: Expr,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub iter: Expr,
    pub body: Block,
    /// The name of the loop, so that a `break` may name it.
    pub label: Option<String>,
    /// The step of the loop, when it walks a range.
    pub step: Option<Expr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct RepeatStmt {
    pub count: Expr,
    pub body: Block,
    /// The name of the loop, so that a `break` may name it.
    pub label: Option<String>,
    pub pos: Pos,
}

/// A value that is thrown.
#[derive(Debug, Clone)]
pub struct ThrowStmt {
    pub value: Expr,
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
    /// The position of an explicitly written alias; inferred aliases have no token.
    pub alias_pos: Option<Pos>,
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

/// A type with a name and a shape of its own.
///
/// An interface says what a value has to have, without saying what it is.
/// A value may have more than the interface asks for.
#[derive(Debug, Clone)]
pub struct InterfaceDecl {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub members: Vec<InterfaceMember>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum InterfaceMember {
    Field(InterfaceField),
    Method(InterfaceMethod),
}

#[derive(Debug, Clone)]
pub struct InterfaceField {
    pub name: String,
    pub ty: TypeExpr,
    pub optional: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: TypeExpr,
    pub pos: Pos,
}

/// The functions of a type.
///
/// An `impl` block gives a type functions of its own, so that a program of
/// many types does not have a file of free functions.
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub type_name: String,
    pub methods: Vec<ImplMethod>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ImplMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Block,
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
