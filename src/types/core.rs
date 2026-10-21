//! The type representation and the rules that relate two types.

use std::collections::HashMap;

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

pub(crate) fn assignable(source: &Type, target: &Type) -> bool {
    // The open part of a recursive alias is checked loosely, because it
    // cannot be written out.
    if matches!(source, Type::Any | Type::Recursion(_))
        || matches!(target, Type::Any | Type::Recursion(_))
    {
        return true;
    }
    if matches!(source, Type::Never) {
        return true;
    }
    if source == target {
        return true;
    }

    if let Type::Union(sources) = source {
        if let Type::Union(targets) = target {
            return sources
                .iter()
                .all(|item| targets.iter().any(|target| assignable(item, target)));
        }
        return sources.iter().all(|item| assignable(item, target));
    }

    if let Type::Union(targets) = target {
        return targets.iter().any(|item| assignable(source, item));
    }

    match (source, target) {
        (Type::Array(source), Type::Array(target)) => assignable(source, target),
        (Type::Object(source), Type::Object(target)) => object_assignable(source, target),
        (Type::Named(_, source), Type::Object(target)) => object_assignable(source, target),
        (Type::Object(source), Type::Named(_, target)) => object_assignable(source, target),
        (Type::Named(_, source), Type::Named(_, target)) => object_assignable(source, target),
        (Type::Function(source), Type::Function(target)) => function_assignable(source, target),
        (Type::Enum(e1, v1), Type::Enum(e2, v2)) => e1 == e2 && v1 == v2,
        (Type::Enum(e, _), Type::EnumUnion(e2)) => e == e2,
        (Type::EnumUnion(e), Type::EnumUnion(e2)) => e == e2,
        (Type::EnumNamespace(e), Type::EnumNamespace(e2)) => e == e2,
        _ => false,
    }
}

pub(crate) fn object_assignable(source: &[ObjectField], target: &[ObjectField]) -> bool {
    for target_field in target {
        match source
            .iter()
            .find(|source_field| source_field.name == target_field.name)
        {
            Some(source_field) => {
                if source_field.optional && !target_field.optional {
                    return false;
                }
                if !assignable(&source_field.ty, &target_field.ty) {
                    return false;
                }
            }
            None if target_field.optional => {}
            None => return false,
        }
    }
    true
}

pub(crate) fn function_assignable(source: &FunctionType, target: &FunctionType) -> bool {
    if function_has_vars(source) {
        if let Some(instantiated) = instantiate_function(source, target) {
            if !function_has_vars(&instantiated) {
                return function_assignable(&instantiated, target);
            }
        }
        return false;
    }

    if !target.rest {
        // The target always passes the same number of arguments, so the
        // source has to be able to take them. A source with fewer
        // parameters is fine as long as the missing ones have a default.
        if source.required > target.params.len() {
            return false;
        }
    }
    // A variadic target may pass more arguments than the source takes.
    // JavaScript ignores the extra ones, so the source stays usable.

    for (source_param, target_param) in source.params.iter().zip(target.params.iter()) {
        if !assignable(target_param, source_param) {
            return false;
        }
    }
    assignable(&source.ret, &target.ret)
}

pub(crate) fn function_has_vars(function: &FunctionType) -> bool {
    function.params.iter().any(type_has_var) || type_has_var(&function.ret)
}

pub(crate) fn type_has_var(ty: &Type) -> bool {
    match ty {
        Type::Var(_) => true,
        Type::Array(inner) => type_has_var(inner),
        Type::Object(fields) => fields.iter().any(|field| type_has_var(&field.ty)),
        Type::Named(_, fields) => fields.iter().any(|field| type_has_var(&field.ty)),
        Type::Function(function) => function_has_vars(function),
        Type::Union(items) => items.iter().any(type_has_var),
        _ => false,
    }
}

pub(crate) fn instantiate_function(
    source: &FunctionType,
    target: &FunctionType,
) -> Option<FunctionType> {
    if source.rest != target.rest || source.params.len() != target.params.len() {
        return None;
    }
    let mut subst = HashMap::new();
    for (source_param, target_param) in source.params.iter().zip(target.params.iter()) {
        if !unify_types(target_param, source_param, &mut subst) {
            return None;
        }
    }
    if !unify_types(&target.ret, &source.ret, &mut subst) {
        return None;
    }
    match substitute(&Type::Function(source.clone()), &subst) {
        Type::Function(function) => Some(function),
        _ => None,
    }
}

/// Replace the type variables that are still free with `any`.
///
/// A type parameter that could not be inferred stays unknown.
pub(crate) fn erase_type_vars(ty: &Type) -> Type {
    match ty {
        Type::Var(_) => Type::Any,
        Type::Array(inner) => Type::Array(Box::new(erase_type_vars(inner))),
        Type::Object(fields) => Type::Object(
            fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: erase_type_vars(&field.ty),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Named(name, fields) => Type::Named(
            name.clone(),
            fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: erase_type_vars(&field.ty),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Function(function) => Type::Function(FunctionType {
            params: function.params.iter().map(erase_type_vars).collect(),
            required: function.required,
            rest: function.rest,
            ret: Box::new(erase_type_vars(&function.ret)),
            checked: function.checked,
            constraints: function
                .constraints
                .iter()
                .map(|(name, constraint)| (name.clone(), erase_type_vars(constraint)))
                .collect(),
            type_params: function.type_params.clone(),
        }),
        Type::Union(items) => union(items.iter().map(erase_type_vars).collect()),
        other => other.clone(),
    }
}

/// Replace the type variables that are bound and leave the others alone.
///
/// Unlike `substitute` this keeps a type variable that has no binding, so
/// that a generic type stays generic.
pub(crate) fn substitute_bound(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Var(name) => match subst.get(name) {
            Some(bound) => bound.clone(),
            None => ty.clone(),
        },
        Type::Array(inner) => Type::Array(Box::new(substitute_bound(inner, subst))),
        Type::Object(fields) => Type::Object(
            fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: substitute_bound(&field.ty, subst),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Named(name, fields) => Type::Named(
            name.clone(),
            fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: substitute_bound(&field.ty, subst),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Function(function) => Type::Function(FunctionType {
            params: function
                .params
                .iter()
                .map(|param| substitute_bound(param, subst))
                .collect(),
            required: function.required,
            rest: function.rest,
            ret: Box::new(substitute_bound(&function.ret, subst)),
            checked: function.checked,
            constraints: function
                .constraints
                .iter()
                .map(|(name, constraint)| (name.clone(), substitute_bound(constraint, subst)))
                .collect(),
            type_params: function.type_params.clone(),
        }),
        Type::Union(items) => union(
            items
                .iter()
                .map(|item| substitute_bound(item, subst))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Whether the type still holds a type variable.
pub(crate) fn has_type_var(ty: &Type) -> bool {
    match ty {
        Type::Var(_) => true,
        Type::Array(inner) => has_type_var(inner),
        Type::Object(fields) => fields.iter().any(|field| has_type_var(&field.ty)),
        Type::Named(_, fields) => fields.iter().any(|field| has_type_var(&field.ty)),
        Type::Function(function) => {
            function.params.iter().any(has_type_var) || has_type_var(&function.ret)
        }
        Type::Union(items) => items.iter().any(has_type_var),
        _ => false,
    }
}

/// The type variables of a type, in the order they appear.
pub(crate) fn collect_type_vars(ty: &Type, out: &mut Vec<String>) {
    match ty {
        Type::Var(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Type::Array(inner) => collect_type_vars(inner, out),
        Type::Object(fields) => {
            for field in fields {
                collect_type_vars(&field.ty, out);
            }
        }
        Type::Named(_, fields) => {
            for field in fields {
                collect_type_vars(&field.ty, out);
            }
        }
        Type::Function(function) => {
            for param in &function.params {
                collect_type_vars(param, out);
            }
            collect_type_vars(&function.ret, out);
        }
        Type::Union(items) => {
            for item in items {
                collect_type_vars(item, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn type_contains_var(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Var(other) => other == name,
        Type::Array(inner) => type_contains_var(inner, name),
        Type::Object(fields) => fields
            .iter()
            .any(|field| type_contains_var(&field.ty, name)),
        Type::Function(function) => {
            function
                .params
                .iter()
                .any(|param| type_contains_var(param, name))
                || type_contains_var(&function.ret, name)
        }
        Type::Union(items) => items.iter().any(|item| type_contains_var(item, name)),
        _ => false,
    }
}

pub(crate) fn unify_types(
    actual: &Type,
    expected: &Type,
    subst: &mut HashMap<String, Type>,
) -> bool {
    if let Type::Var(name) = expected {
        if let Some(bound) = subst.get(name).cloned() {
            return unify_types(actual, &bound, subst);
        }
        if let Type::Var(other) = actual {
            if name == other {
                return true;
            }
            if let Some(bound) = subst.get(other).cloned() {
                return unify_types(&bound, expected, subst);
            }
        }
        if type_contains_var(actual, name) {
            return false;
        }
        subst.insert(name.clone(), actual.clone());
        return true;
    }

    if let Type::Var(name) = actual {
        if let Some(bound) = subst.get(name).cloned() {
            return unify_types(&bound, expected, subst);
        }
        return false;
    }

    if matches!(actual, Type::Any | Type::Recursion(_))
        || matches!(expected, Type::Any | Type::Recursion(_))
    {
        return true;
    }
    if matches!(actual, Type::Never) || actual == expected {
        return true;
    }

    if let Type::Union(targets) = expected {
        // A concrete member of the union is tried before a type variable, so
        // that `T | null` binds `null` to `null` and leaves `T` free for the
        // other arguments.
        let mut deferred: Vec<&Type> = Vec::new();
        for target in targets {
            if matches!(target, Type::Var(_)) {
                deferred.push(target);
                continue;
            }
            let mut trial = subst.clone();
            if unify_types(actual, target, &mut trial) {
                *subst = trial;
                return true;
            }
        }
        for target in deferred {
            let mut trial = subst.clone();
            if unify_types(actual, target, &mut trial) {
                *subst = trial;
                return true;
            }
        }
        if let Type::Union(actuals) = actual {
            return actuals
                .iter()
                .all(|item| unify_types(item, expected, subst));
        }
        return false;
    }

    if let Type::Union(actuals) = actual {
        return actuals
            .iter()
            .all(|item| unify_types(item, expected, subst));
    }

    match (actual, expected) {
        (Type::Array(actual), Type::Array(expected)) => unify_types(actual, expected, subst),
        (Type::Object(actual), Type::Object(expected))
        | (Type::Named(_, actual), Type::Object(expected))
        | (Type::Object(actual), Type::Named(_, expected))
        | (Type::Named(_, actual), Type::Named(_, expected)) => {
            for expected_field in expected {
                match actual
                    .iter()
                    .find(|field| field.name == expected_field.name)
                {
                    Some(actual_field) => {
                        if actual_field.optional && !expected_field.optional {
                            return false;
                        }
                        if !unify_types(&actual_field.ty, &expected_field.ty, subst) {
                            return false;
                        }
                    }
                    None if expected_field.optional => {}
                    None => return false,
                }
            }
            true
        }
        (Type::Function(actual), Type::Function(expected)) => {
            if actual.rest != expected.rest
                || actual.params.len() != expected.params.len()
                || actual.required > expected.required
            {
                return false;
            }
            for (actual_param, expected_param) in actual.params.iter().zip(expected.params.iter()) {
                if !unify_types(actual_param, expected_param, subst) {
                    return false;
                }
            }
            unify_types(&actual.ret, &expected.ret, subst)
        }
        _ => false,
    }
}

pub(crate) fn substitute(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    substitute_with(ty, subst, &mut Vec::new())
}

pub(crate) fn substitute_with(
    ty: &Type,
    subst: &HashMap<String, Type>,
    seen: &mut Vec<String>,
) -> Type {
    match ty {
        Type::Var(name) => {
            if seen.iter().any(|item| item == name) {
                return Type::Any;
            }
            match subst.get(name) {
                Some(bound) => {
                    seen.push(name.clone());
                    let result = substitute_with(bound, subst, seen);
                    seen.pop();
                    result
                }
                None => Type::Any,
            }
        }
        Type::Array(inner) => Type::Array(Box::new(substitute_with(inner, subst, seen))),
        Type::Object(fields) => Type::Object(
            fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: substitute_with(&field.ty, subst, seen),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Named(name, fields) => Type::Named(
            name.clone(),
            fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: substitute_with(&field.ty, subst, seen),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Function(function) => Type::Function(FunctionType {
            params: function
                .params
                .iter()
                .map(|param| substitute_with(param, subst, seen))
                .collect(),
            required: function.required,
            rest: function.rest,
            ret: Box::new(substitute_with(&function.ret, subst, seen)),
            checked: function.checked,
            constraints: function
                .constraints
                .iter()
                .map(|(name, constraint)| (name.clone(), substitute_with(constraint, subst, seen)))
                .collect(),
            type_params: function.type_params.clone(),
        }),
        Type::Union(items) => union(
            items
                .iter()
                .map(|item| substitute_with(item, subst, seen))
                .collect(),
        ),
        other => other.clone(),
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
