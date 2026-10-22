//! The types that may be written in a program.

use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Name(String, Pos),
    Qualified(String, String, Pos),
    Generic(String, Vec<TypeExpr>, Pos),
    Array(Box<TypeExpr>, Pos),
    /// A list of values of different types, such as `[number, string]`.
    Tuple(Vec<TypeExpr>, Pos),
    /// A dictionary, such as `{[string]: number}`.
    Dict(Box<TypeExpr>, Box<TypeExpr>, Pos),
    Object(Vec<TypeField>, Pos),
    Optional(Box<TypeExpr>, Pos),
    Union(Vec<TypeExpr>, Pos),
    Function(Vec<TypeExpr>, Box<TypeExpr>, bool, Pos),
}

#[derive(Debug, Clone)]
pub struct TypeField {
    pub name: String,
    pub ty: TypeExpr,
    pub optional: bool,
    pub pos: Pos,
}

impl TypeExpr {
    pub fn pos(&self) -> Pos {
        match self {
            Self::Name(_, pos)
            | Self::Qualified(_, _, pos)
            | Self::Generic(_, _, pos)
            | Self::Array(_, pos)
            | Self::Tuple(_, pos)
            | Self::Dict(_, _, pos)
            | Self::Object(_, pos)
            | Self::Optional(_, pos)
            | Self::Union(_, pos)
            | Self::Function(_, _, _, pos) => *pos,
        }
    }
}
