//! The expressions of a program.

use crate::ast::decl::{Block, EnumBinding, Param};
use crate::ast::types::TypeExpr;
use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub struct StrPart {
    pub text: String,
    pub expr: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub patterns: Vec<Pattern>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(Expr),
    Range(Expr, Expr),
    Wildcard,
    Enum {
        enum_name: String,
        variant: String,
        bindings: Vec<EnumBinding>,
        pos: Pos,
    },
}

#[derive(Debug, Clone)]
pub enum MemberProp {
    Name(String),
    Computed(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum ArrowBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Debug, Clone)]
pub enum ObjectProp {
    Field(String, Expr),
    Spread(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64, Pos),
    String(Vec<StrPart>, Pos),
    Bool(bool, Pos),
    Null(Pos),
    Ident(String, Pos),
    Array(Vec<Expr>, Pos),
    Object(Vec<ObjectProp>, Pos),
    Unary(String, Box<Expr>, Pos),
    Binary(String, Box<Expr>, Box<Expr>, Pos),
    Logical(String, Box<Expr>, Box<Expr>, Pos),
    Nullish(Box<Expr>, Box<Expr>, Pos),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>, Pos),
    Call(Box<Expr>, Vec<Expr>, bool, Pos),
    /// A call with explicit type arguments, such as `parse<number>(text)`.
    CallGeneric(Box<Expr>, Vec<TypeExpr>, Vec<Expr>, bool, Pos),
    Member(Box<Expr>, MemberProp, bool, Pos),
    Fn(Vec<Param>, Block, Option<TypeExpr>, Pos),
    Arrow(Vec<Param>, ArrowBody, Pos),
    Range(Box<Expr>, Box<Expr>, Pos),
    Match(Box<Expr>, Vec<MatchArm>, Pos),
    Await(Box<Expr>, Pos),
    Spread(Box<Expr>, Pos),
}

impl Expr {
    pub fn pos(&self) -> Pos {
        match self {
            Expr::Number(_, p)
            | Expr::String(_, p)
            | Expr::Bool(_, p)
            | Expr::Null(p)
            | Expr::Ident(_, p)
            | Expr::Array(_, p)
            | Expr::Object(_, p)
            | Expr::Unary(_, _, p)
            | Expr::Binary(_, _, _, p)
            | Expr::Logical(_, _, _, p)
            | Expr::Nullish(_, _, p)
            | Expr::Ternary(_, _, _, p)
            | Expr::Call(_, _, _, p)
            | Expr::CallGeneric(_, _, _, _, p)
            | Expr::Member(_, _, _, p)
            | Expr::Fn(_, _, _, p)
            | Expr::Arrow(_, _, p)
            | Expr::Range(_, _, p)
            | Expr::Match(_, _, p)
            | Expr::Await(_, p)
            | Expr::Spread(_, p) => *p,
        }
    }
}
