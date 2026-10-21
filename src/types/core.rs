//! The type representation and the rules that relate two types.

use crate::ast::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Any,
    Var(String),
    Number,
    String,
    Bool,
    Null,
    Void,
    Never,
    Array(Box<Type>),
    Object(Vec<ObjectField>),
    Function(FunctionType),
    Union(Vec<Type>),
    Enum(String, String),
    EnumUnion(String),
    EnumNamespace(String),
    /// A reference to a type alias that refers back to itself.
    ///
    /// The alias cannot be written out, so the part that is still open is
    /// checked loosely.
    Recursion(String),
    /// An object type that has a name, such as a type alias.
    Named(String, Vec<ObjectField>),
}

#[derive(Clone)]
pub(crate) struct Alias {
    pub(crate) params: Vec<TypeParam>,
    pub(crate) body: TypeExpr,
}

#[derive(Clone)]
pub(crate) struct EnumInfo {
    pub(crate) variants: Vec<EnumVariantInfo>,
}

#[derive(Clone)]
pub(crate) struct EnumVariantInfo {
    pub(crate) name: String,
    pub(crate) fields: Vec<EnumFieldInfo>,
}

#[derive(Clone)]
pub(crate) struct EnumFieldInfo {
    pub(crate) name: String,
    pub(crate) ty: Option<TypeExpr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectField {
    pub(crate) name: String,
    pub(crate) ty: Type,
    pub(crate) optional: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionType {
    pub(crate) params: Vec<Type>,
    pub(crate) required: usize,
    pub(crate) rest: bool,
    pub(crate) ret: Box<Type>,
    pub(crate) checked: bool,
    /// The types the type parameters have to satisfy, by name.
    pub(crate) constraints: Vec<(String, Type)>,
    /// The names of the type parameters, in the order they were declared.
    pub(crate) type_params: Vec<String>,
}

impl Type {
    pub(crate) fn display(&self) -> String {
        match self {
            Self::Any => "any".to_string(),
            Self::Var(name) => name.clone(),
            Self::Number => "number".to_string(),
            Self::String => "string".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Null => "null".to_string(),
            Self::Void => "void".to_string(),
            Self::Never => "never".to_string(),
            Self::Array(inner) => format!("[{}]", inner.display()),
            Self::Object(fields) => {
                if fields.is_empty() {
                    return "{}".to_string();
                }
                let parts = fields
                    .iter()
                    .map(|field| {
                        if field.optional {
                            format!("{}?: {}", field.name, field.ty.without_null().display())
                        } else {
                            format!("{}: {}", field.name, field.ty.display())
                        }
                    })
                    .collect::<Vec<_>>();
                format!("{{ {} }}", parts.join(", "))
            }
            Self::Function(function) => {
                let mut params = Vec::new();
                for (index, param) in function.params.iter().enumerate() {
                    if function.rest && index + 1 == function.params.len() {
                        params.push(format!("...{}", param.display()));
                    } else {
                        params.push(param.display());
                    }
                }
                format!("({}) -> {}", params.join(", "), function.ret.display())
            }
            Self::Union(items) => items
                .iter()
                .map(Type::display)
                .collect::<Vec<_>>()
                .join(" | "),
            Self::Enum(name, variant) => format!("{}.{}", name, variant),
            Self::EnumUnion(name) => name.clone(),
            Self::EnumNamespace(name) => format!("enum {}", name),
            Self::Recursion(name) => name.clone(),
            Self::Named(name, _) => name.clone(),
        }
    }

    pub(crate) fn without_null(&self) -> Type {
        match self {
            Self::Null => Self::Never,
            Self::Union(items) => union(
                items
                    .iter()
                    .filter(|item| !matches!(item, Self::Null))
                    .cloned()
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

pub(crate) fn union(types: Vec<Type>) -> Type {
    let mut out = Vec::new();
    for ty in types {
        match ty {
            Type::Union(items) => {
                for item in items {
                    push_unique(&mut out, item);
                }
            }
            Type::Never => {}
            other => push_unique(&mut out, other),
        }
    }

    if out.is_empty() {
        Type::Never
    } else if out.len() == 1 {
        out.remove(0)
    } else if out.iter().any(|item| matches!(item, Type::Any)) {
        Type::Any
    } else {
        Type::Union(out)
    }
}

pub(crate) fn push_unique(out: &mut Vec<Type>, ty: Type) {
    if !out.contains(&ty) {
        out.push(ty);
    }
}

pub(crate) fn element_type(ty: &Type) -> Type {
    match ty {
        Type::Array(inner) => (**inner).clone(),
        Type::String => Type::String,
        Type::Any => Type::Any,
        Type::Union(items) => union(items.iter().map(element_type).collect()),
        _ => Type::Any,
    }
}

/// The type of a single value of a rest parameter.
///
/// A rest parameter is written as a list, for example `...names: [string]`,
/// but it stands for the type of one value.
pub(crate) fn rest_element_type(ty: &Type) -> Type {
    match ty {
        Type::Array(inner) => (**inner).clone(),
        other => other.clone(),
    }
}

pub(crate) fn project_field(ty: &Type, name: &str) -> Type {
    match ty {
        Type::Object(fields) => fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.ty.clone())
            .unwrap_or(Type::Any),
        Type::Named(_, fields) => fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.ty.clone())
            .unwrap_or(Type::Any),
        Type::Union(items) => union(items.iter().map(|item| project_field(item, name)).collect()),
        Type::Any => Type::Any,
        _ => Type::Any,
    }
}

pub(crate) fn object_rest_type(ty: &Type, bound: &[BindingField]) -> Type {
    match ty {
        Type::Object(fields) => {
            let remaining = fields
                .iter()
                .filter(|field| !bound.iter().any(|item| item.key == field.name))
                .cloned()
                .collect();
            Type::Object(remaining)
        }
        Type::Named(name, fields) => {
            let remaining = fields
                .iter()
                .filter(|field| !bound.iter().any(|item| item.key == field.name))
                .cloned()
                .collect();
            Type::Named(name.clone(), remaining)
        }
        Type::Any => Type::Any,
        Type::Union(items) => union(
            items
                .iter()
                .map(|item| object_rest_type(item, bound))
                .collect(),
        ),
        _ => Type::Any,
    }
}

pub(crate) fn string_literal(expr: &Expr) -> Option<&str> {
    if let Expr::String(parts, _) = expr {
        if parts.len() == 1 && parts[0].expr.is_none() {
            return Some(&parts[0].text);
        }
    }
    None
}

/// The type that a `type(value) == "kind"` check narrows to.
///
/// The narrowed type has to be part of the declared type, otherwise the
/// check could never be true and narrowing to it would be unsound.
pub(crate) fn type_from_kind(kind: &str, original: &Type) -> Option<Type> {
    let target = match kind {
        "string" => Type::String,
        "number" => Type::Number,
        "boolean" => Type::Bool,
        "null" => Type::Null,
        "list" => match original {
            Type::Array(_) => original.clone(),
            _ => Type::Array(Box::new(Type::Any)),
        },
        "function" => match original {
            Type::Function(_) => original.clone(),
            _ => return None,
        },
        "object" => match original {
            Type::Object(_) | Type::Named(_, _) => original.clone(),
            _ => return None,
        },
        "enum" => match original {
            Type::Enum(_, _) | Type::EnumUnion(_) => original.clone(),
            _ => return None,
        },
        _ => return None,
    };

    match original {
        // An unknown value could be anything.
        Type::Any => Some(target),
        Type::Union(items) if items.contains(&target) => Some(target),
        other if *other == target => Some(target),
        _ => None,
    }
}

pub(crate) fn remove_type(ty: &Type, target: &Type) -> Type {
    match ty {
        Type::Union(items) => union(
            items
                .iter()
                .filter(|item| *item != target)
                .cloned()
                .collect(),
        ),
        other if other == target => Type::Never,
        other => other.clone(),
    }
}
