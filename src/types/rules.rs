//! The rules that relate two types.

use std::collections::HashMap;

use crate::types::core::{union, FunctionType, ObjectField, Type};

/// Whether two values of these types may be the same.
pub(crate) fn comparable(left: &Type, right: &Type) -> bool {
    if matches!(left, Type::Any | Type::Var(_) | Type::Never)
        || matches!(right, Type::Any | Type::Var(_) | Type::Never)
    {
        return true;
    }
    if left == right {
        return true;
    }
    if let Type::Union(items) = left {
        if items.iter().any(|item| comparable(item, right)) {
            return true;
        }
    }
    if let Type::Union(items) = right {
        if items.iter().any(|item| comparable(left, item)) {
            return true;
        }
    }
    // Null is the same as the absence of a value.
    if matches!(left, Type::Null) {
        return holds_null(right);
    }
    if matches!(right, Type::Null) {
        return holds_null(left);
    }
    false
}

/// Whether the type may hold null.
fn holds_null(ty: &Type) -> bool {
    match ty {
        Type::Null => true,
        Type::Union(items) => items.iter().any(holds_null),
        _ => false,
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
