//! The type representation and the rules that relate two types.

use crate::ast::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Any,
    /// A value whose shape is not known. It is not the same as `any`: a
    /// value of this type cannot be used where a number is expected.
    Unknown,
    Var(String),
    Number,
    String,
    Bool,
    Null,
    /// The type of a value that is not there.
    Undefined,
    Void,
    Never,
    Symbol,
    BigInt,
    Array(Box<Type>),
    /// A list of values of different types, such as `[number, string]`.
    Tuple(Vec<Type>),
    /// A dictionary, such as `{[string]: number}`.
    Dict(Box<Type>, Box<Type>),
    Object(Vec<ObjectField>),
    /// Any value that is not a number, a text or a yes or a no.
    AnyObject,
    /// A type with a name of its own, such as an interface.
    Interface(String, Vec<ObjectField>),
    Function(FunctionType),
    Union(Vec<Type>),
    Enum(String, String),
    EnumUnion(String),
    EnumNamespace(String),
    /// A reference to a type alias that refers back to itself.
    ///
    /// The alias cannot be written out, so the part that is still open is
    /// checked loosely.
    Recursion(String, Vec<Type>),
    /// An object type that has a name, such as a type alias.
    Named(String, Vec<ObjectField>),
    /// A value that will be there later, such as the result of a request.
    Promise(Box<Type>),
    /// A value that may be there, such as the answer of a function that may
    /// fail.
    Result(Box<Type>, Box<Type>),
}

#[derive(Clone)]
pub(crate) struct Alias {
    pub(crate) params: Vec<TypeParam>,
    pub(crate) body: TypeExpr,
}

#[derive(Clone)]
pub struct EnumInfo {
    pub variants: Vec<EnumVariantInfo>,
}

/// The calls whose arguments are given by name.
///
/// The key is the place of the call, and the value is the arguments in the
/// order of the parameters.
/// The calls whose arguments are given by name, and the order of the
/// arguments.
///
/// A parameter that is not given has no place in the order, so that the
/// arguments after it do not move.
pub type ResolvedCalls =
    std::collections::HashMap<(usize, usize), Vec<Option<crate::ast::Expr>>>;

/// The members of an interface.
#[derive(Clone)]
pub struct InterfaceInfo {
    pub members: Vec<crate::ast::InterfaceMember>,
    /// The type parameters of the interface, such as the `T` of
    /// `interface Box<T>`.
    pub params: Vec<crate::ast::TypeParam>,
}

/// The functions of a type.
#[derive(Clone)]
pub struct ImplInfo {
    pub methods: Vec<crate::ast::ImplMethod>,
}

#[derive(Clone)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: Vec<EnumFieldInfo>,
}

#[derive(Clone)]
pub struct EnumFieldInfo {
    pub name: String,
    pub ty: Option<TypeExpr>,
}

impl EnumInfo {
    /// The variant of the enum with this name.
    pub fn variant(&self, name: &str) -> Option<&EnumVariantInfo> {
        self.variants.iter().find(|variant| variant.name == name)
    }

    /// The names of the fields of a variant, in the order they were written.
    pub fn field_names(&self, variant: &str) -> Vec<String> {
        self.variant(variant)
            .map(|variant| variant.fields.iter().map(|field| field.name.clone()).collect())
            .unwrap_or_default()
    }
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
    /// The names of the parameters, so that the arguments that are given by
    /// name may be matched.
    pub(crate) param_names: Vec<String>,
    /// The parameters that have a value of their own, so that a caller may
    /// leave them out.
    pub(crate) defaults: Vec<bool>,
    /// Whether a type parameter that could not be inferred stands for `any`
    /// instead of being a mistake.
    ///
    /// The helpers of the runtime are typed loosely this way: `store("name")`
    /// says nothing about the state, and that is not worth a refusal. A
    /// function of the program is checked strictly, because there the answer
    /// of the call is wanted.
    pub(crate) loose: bool,
}

impl Type {
    pub(crate) fn display(&self) -> String {
        match self {
            Self::Any => "any".to_string(),
            Self::Unknown => "unknown".to_string(),
            Self::Var(name) => name.clone(),
            Self::Number => "number".to_string(),
            Self::String => "string".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Null => "null".to_string(),
            Self::Undefined => "undefined".to_string(),
            Self::Void => "void".to_string(),
            Self::Never => "never".to_string(),
            Self::Symbol => "symbol".to_string(),
            Self::BigInt => "bigint".to_string(),
            Self::Array(inner) => format!("[{}]", inner.display()),
            Self::Tuple(items) => {
                let parts = items
                    .iter()
                    .map(|item| item.display())
                    .collect::<Vec<_>>();
                format!("[{}]", parts.join(", "))
            }
            Self::Dict(key, value) => {
                format!("{{[{}]: {}}}", key.display(), value.display())
            }
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
            Self::AnyObject => "object".to_string(),
            Self::Interface(name, _) => name.clone(),
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
            Self::Recursion(name, args) if !args.is_empty() => format!(
                "{}<{}>",
                name,
                args.iter()
                    .map(Type::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Recursion(name, _) => name.clone(),
            Self::Named(name, _) => name.clone(),
            Self::Promise(inner) => format!("Promise<{}>", inner.display()),
            Self::Result(ok, err) => {
                format!("Result<{}, {}>", ok.display(), err.display())
            }
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

/// A type without the values that are never there.
///
/// `a or b` gives `a` only when it is there, so the part of `a` that is
/// nothing is the part that gives `b`.
pub(crate) fn without_nullish(ty: &Type) -> Type {
    match ty {
        Type::Null | Type::Undefined | Type::Void => Type::Never,
        Type::Union(items) => union(
            items
                .iter()
                .map(without_nullish)
                .filter(|item| !matches!(item, Type::Never))
                .collect(),
        ),
        other => other.clone(),
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
        Type::Tuple(items) => union(items.iter().cloned().collect()),
        // A loop over a dictionary gives its keys.
        Type::Dict(key, _) => (**key).clone(),
        // A loop over an object gives its keys as well.
        Type::Object(_) | Type::Named(_, _) | Type::Interface(_, _) => Type::String,
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
        Type::Interface(_, fields) => fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.ty.clone())
            .unwrap_or(Type::Any),
        // Every key of a dictionary has the same type of value.
        Type::Dict(_, value) => (**value).clone(),
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
        Type::Interface(name, fields) => {
            let remaining = fields
                .iter()
                .filter(|field| !bound.iter().any(|item| item.key == field.name))
                .cloned()
                .collect();
            Type::Interface(name.clone(), remaining)
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
        "undefined" => Type::Undefined,
        "list" => match original {
            Type::Array(_) => original.clone(),
            _ => Type::Array(Box::new(Type::Any)),
        },
        "function" => match original {
            Type::Function(_) => original.clone(),
            _ => return None,
        },
        "object" => match original {
            Type::Object(_) | Type::Named(_, _) | Type::AnyObject => original.clone(),
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

/// The type of a value without the parts that may be nothing.
pub(crate) fn without_empty(ty: &Type) -> Type {
    match ty {
        Type::Null | Type::Undefined => Type::Never,
        Type::Union(items) => union(
            items
                .iter()
                .filter(|item| !matches!(item, Type::Null | Type::Undefined))
                .cloned()
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Whether an expression is a value that is not there.
pub(crate) fn is_empty(expr: &Expr) -> bool {
    match expr {
        Expr::Null(_) => true,
        Expr::Ident(name, _) => name == "undefined",
        _ => false,
    }
}
