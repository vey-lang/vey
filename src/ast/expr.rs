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
    /// The condition that has to hold as well.
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(Expr),
    Range(Expr, Expr),
    Wildcard,
    /// A name that takes the value of the match.
    Binding {
        name: String,
        pos: Pos,
    },
    Enum {
        enum_name: String,
        variant: String,
        bindings: Vec<EnumBinding>,
        pos: Pos,
    },
    /// A list of patterns, such as `[1, name]`.
    List(Vec<Pattern>, Pos),
    /// The fields of an object, such as `{ name, age }`.
    Object(Vec<(String, Pattern)>, Pos),
    /// A text that begins with something, such as `"Vey "..`.
    Prefix(Expr, Pos),
}

impl Pattern {
    /// The place of the pattern.
    pub fn pos(&self) -> Pos {
        match self {
            Self::Literal(expr) => expr.pos(),
            Self::Range(start, _) => start.pos(),
            Self::Wildcard => Pos::new(0, 0),
            Self::Binding { pos, .. } => *pos,
            Self::Enum { pos, .. } => *pos,
            Self::List(_, pos) => *pos,
            Self::Object(_, pos) => *pos,
            Self::Prefix(_, pos) => *pos,
        }
    }

    /// The names that the pattern takes apart.
    pub fn bindings(&self) -> Vec<(String, Pos)> {
        let mut out = Vec::new();
        self.collect_bindings(&mut out);
        out
    }

    fn collect_bindings(&self, out: &mut Vec<(String, Pos)>) {
        match self {
            Self::Binding { name, pos } => out.push((name.clone(), *pos)),
            Self::Enum { bindings, .. } => {
                for binding in bindings {
                    out.push((binding.name.clone(), binding.pos));
                }
            }
            Self::List(items, _) => {
                for item in items {
                    item.collect_bindings(out);
                }
            }
            Self::Object(fields, _) => {
                for (_, pattern) in fields {
                    pattern.collect_bindings(out);
                }
            }
            _ => {}
        }
    }
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
    Arrow(Vec<Param>, ArrowBody, Option<TypeExpr>, Pos),
    Range(Box<Expr>, Box<Expr>, Pos),
    Match(Box<Expr>, Vec<MatchArm>, Pos),
    Await(Box<Expr>, Pos),
    /// A value that hands its failure on.
    Try(Box<Expr>, Pos),
    /// A value that a generator hands back one by one.
    Yield(Box<Expr>, Pos),
    /// A value that is used as another type.
    As(Box<Expr>, TypeExpr, Pos),
    /// A check of the type of a value.
    Is(Box<Expr>, TypeExpr, Pos),
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
            | Expr::Arrow(_, _, _, p)
            | Expr::Range(_, _, p)
            | Expr::Match(_, _, p)
            | Expr::Await(_, p)
            | Expr::Try(_, p)
            | Expr::Yield(_, p)
            | Expr::As(_, _, p)
            | Expr::Is(_, _, p)
            | Expr::Spread(_, p) => *p,
        }
    }
}
