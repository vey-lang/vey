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
    // A value of an enum is the same as one of its variants.
    if let (Type::Enum(enum_name, variant), Type::EnumUnion(other)) = (left, right) {
        return enum_name == other || variant.is_empty();
    }
    if let (Type::EnumUnion(other), Type::Enum(enum_name, variant)) = (left, right) {
        return enum_name == other || variant.is_empty();
    }
    if let (Type::Enum(name, variant), Type::Enum(other, other_variant)) = (left, right) {
        return name == other && variant == other_variant;
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

/// Whether the values of a type have an order.
///
/// Only numbers have one. A value that may be nothing is fine, because it is
/// never greater than another value.
pub(crate) fn orderable(ty: &Type) -> bool {
    match ty {
        Type::Any | Type::Var(_) | Type::Never | Type::Number | Type::Null => true,
        Type::Union(items) => items.iter().all(orderable),
        _ => false,
    }
}

/// The state of a check that opens the recursive aliases it meets.
///
/// A type that refers to itself cannot be written out, so it is opened one
/// step at a time, only where the check reaches it. A pair of types that
/// the check is already inside of says nothing new when it comes again, so
/// it is taken to fit: the check then goes round the type instead of for
/// ever.
#[derive(Default)]
pub(crate) struct Opening<'a> {
    /// The shape of every alias of the program, when they may be opened.
    aliases: Option<&'a HashMap<String, Type>>,
    /// The declared parameter names of generic aliases.
    type_params: Option<&'a HashMap<String, Vec<String>>>,
    /// The pairs of types that the check is inside of.
    assumed: Vec<(Type, Type)>,
}

impl<'a> Opening<'a> {
    /// An opening that may use the shapes of the aliases of the program.
    pub(crate) fn new(
        aliases: &'a HashMap<String, Type>,
        type_params: &'a HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            aliases: Some(aliases),
            type_params: Some(type_params),
            assumed: Vec::new(),
        }
    }

    /// The shape of an alias, when it is one that may be opened.
    fn shape(&self, ty: &Type) -> Option<Type> {
        let Type::Recursion(name, args) = ty else {
            return None;
        };
        let shape = self.aliases?.get(name)?;
        let Some(params) = self.type_params?.get(name) else {
            return Some(shape.clone());
        };
        if params.len() != args.len() {
            return Some(shape.clone());
        }
        let subst = params.iter().cloned().zip(args.iter().cloned()).collect();
        Some(substitute(shape, &subst))
    }
}

pub(crate) fn assignable(source: &Type, target: &Type) -> bool {
    assignable_in(source, target, &mut Opening::default())
}

/// Whether a value of one type fits another, opening the aliases that the
/// check meets on its way.
pub(crate) fn assignable_with(source: &Type, target: &Type, opening: &mut Opening) -> bool {
    assignable_in(source, target, opening)
}

fn invariantly_assignable_in(source: &Type, target: &Type, opening: &mut Opening) -> bool {
    assignable_in(source, target, opening) && assignable_in(target, source, opening)
}

fn assignable_in(source: &Type, target: &Type, opening: &mut Opening) -> bool {
    // A type that refers to itself is opened one step at a time, and the
    // pair is remembered while it is being checked. A pair that comes again
    // is the same question as before, so it is taken to fit.
    if matches!(source, Type::Recursion(_, _)) || matches!(target, Type::Recursion(_, _)) {
        let open_source = opening.shape(source);
        let open_target = opening.shape(target);
        // Without a shape the open part is taken to fit anything, as it has
        // no written form.
        if (matches!(source, Type::Recursion(_, _)) && open_source.is_none())
            || (matches!(target, Type::Recursion(_, _)) && open_target.is_none())
        {
            return true;
        }
        let pair = (source.clone(), target.clone());
        if opening.assumed.contains(&pair) {
            return true;
        }
        opening.assumed.push(pair);
        let result = assignable_in(
            open_source.as_ref().unwrap_or(source),
            open_target.as_ref().unwrap_or(target),
            opening,
        );
        opening.assumed.pop();
        return result;
    }

    // The open part of a recursive alias is checked loosely, because it
    // cannot be written out.
    if matches!(source, Type::Any | Type::Recursion(_, _))
        || matches!(target, Type::Any | Type::Recursion(_, _))
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
            return sources.iter().all(|item| {
                targets
                    .iter()
                    .any(|target| assignable_in(item, target, opening))
            });
        }
        return sources
            .iter()
            .all(|item| assignable_in(item, target, opening));
    }

    if let Type::Union(targets) = target {
        return targets
            .iter()
            .any(|item| assignable_in(source, item, opening));
    }

    match (source, target) {
        (Type::Array(source), Type::Array(target)) => {
            invariantly_assignable_in(source, target, opening)
        }
        (Type::Tuple(source), Type::Tuple(target)) => {
            source.len() == target.len()
                && source
                    .iter()
                    .zip(target.iter())
                    .all(|(source, target)| invariantly_assignable_in(source, target, opening))
        }
        // Arrays and tuples are both mutable at runtime. An array's length
        // is not known, so it cannot promise the positions of a tuple.
        (Type::Array(_), Type::Tuple(_)) => false,
        (Type::Tuple(source), Type::Array(target)) => source
            .iter()
            .all(|item| invariantly_assignable_in(item, target, opening)),
        (Type::Dict(source_key, source_value), Type::Dict(target_key, target_value)) => {
            invariantly_assignable_in(source_key, target_key, opening)
                && invariantly_assignable_in(source_value, target_value, opening)
        }
        (Type::Object(source), Type::Object(target)) => {
            object_assignable_exact_in(source, target, opening)
        }
        (Type::Named(_, source), Type::Object(target)) => {
            object_assignable_in(source, target, opening)
        }
        (Type::Object(source), Type::Named(_, target)) => {
            object_assignable_exact_in(source, target, opening)
        }
        (Type::Named(_, source), Type::Named(_, target)) => {
            object_assignable_in(source, target, opening)
        }
        // An interface is a promise, not a shape: a value may have more
        // than it asks for.
        (Type::Interface(_, source), Type::Object(target)) => {
            object_assignable_in(source, target, opening)
        }
        // A value that is written out is read as the interface it is given
        // to, so a field that the interface does not have is a mistake.
        (Type::Object(source), Type::Interface(_, target)) => {
            object_assignable_exact_in(source, target, opening)
        }
        (Type::Named(_, source), Type::Interface(_, target)) => {
            object_assignable_in(source, target, opening)
        }
        (Type::Interface(_, source), Type::Named(_, target)) => {
            object_assignable_in(source, target, opening)
        }
        (Type::Interface(_, source), Type::Interface(_, target)) => {
            object_assignable_in(source, target, opening)
        }
        (Type::Object(_), Type::AnyObject) => true,
        (Type::Named(_, _), Type::AnyObject) => true,
        (Type::Interface(_, _), Type::AnyObject) => true,
        (Type::Array(_), Type::AnyObject) => true,
        (Type::Tuple(_), Type::AnyObject) => true,
        (Type::Dict(_, _), Type::AnyObject) => true,
        (Type::Enum(_, _), Type::AnyObject) => true,
        (Type::EnumUnion(_), Type::AnyObject) => true,
        (Type::Function(_), Type::AnyObject) => true,
        (Type::Promise(_), Type::AnyObject) => true,
        (Type::Result(_, _), Type::AnyObject) => true,
        (Type::Function(source), Type::Function(target)) => {
            function_assignable_in(source, target, opening)
        }
        (Type::Promise(source), Type::Promise(target)) => assignable_in(source, target, opening),
        (Type::Result(source_ok, source_err), Type::Result(target_ok, target_err)) => {
            assignable_in(source_ok, target_ok, opening)
                && assignable_in(source_err, target_err, opening)
        }
        (Type::Enum(e1, v1), Type::Enum(e2, v2)) => e1 == e2 && v1 == v2,
        (Type::Enum(e, _), Type::EnumUnion(e2)) => e == e2,
        (Type::EnumUnion(e), Type::EnumUnion(e2)) => e == e2,
        (Type::EnumNamespace(e), Type::EnumNamespace(e2)) => e == e2,
        _ => false,
    }
}

fn object_assignable_in(
    source: &[ObjectField],
    target: &[ObjectField],
    opening: &mut Opening,
) -> bool {
    for target_field in target {
        match source
            .iter()
            .find(|source_field| source_field.name == target_field.name)
        {
            Some(source_field) => {
                // Fields can be written through an alias, so their type and
                // optionality must be the same in both views.
                if source_field.optional != target_field.optional {
                    return false;
                }
                if !invariantly_assignable_in(&source_field.ty, &target_field.ty, opening) {
                    return false;
                }
            }
            None if target_field.optional => {}
            None => return false,
        }
    }
    true
}

/// Whether an object has exactly the fields of the target.
///
/// A field that the target does not have is a mistake, because the value
/// would be read as something it is not.
fn object_assignable_exact_in(
    source: &[ObjectField],
    target: &[ObjectField],
    opening: &mut Opening,
) -> bool {
    if !object_assignable_in(source, target, opening) {
        return false;
    }
    for source_field in source {
        if !target
            .iter()
            .any(|target_field| target_field.name == source_field.name)
        {
            return false;
        }
    }
    true
}

fn function_assignable_in(
    source: &FunctionType,
    target: &FunctionType,
    opening: &mut Opening,
) -> bool {
    if function_has_vars(source) {
        if let Some(instantiated) = instantiate_function(source, target) {
            if !function_has_vars(&instantiated) {
                return function_assignable_in(&instantiated, target, opening);
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
        // The source has to be able to take the arguments that the target
        // will pass.
        if source.params.len() < target.required {
            return false;
        }
    }
    // A variadic target may pass more arguments than the source takes.
    // JavaScript ignores the extra ones, so the source stays usable.

    for (source_param, target_param) in source.params.iter().zip(target.params.iter()) {
        if !assignable_in(target_param, source_param, opening) {
            return false;
        }
    }
    assignable_in(&source.ret, &target.ret, opening)
}

pub(crate) fn function_has_vars(function: &FunctionType) -> bool {
    function.params.iter().any(type_has_var) || type_has_var(&function.ret)
}

pub(crate) fn type_has_var(ty: &Type) -> bool {
    match ty {
        Type::Var(_) => true,
        Type::Recursion(_, args) => args.iter().any(type_has_var),
        Type::Array(inner) => type_has_var(inner),
        Type::Tuple(items) => items.iter().any(type_has_var),
        Type::Dict(key, value) => type_has_var(key) || type_has_var(value),
        Type::Promise(inner) => type_has_var(inner),
        Type::Result(ok, err) => type_has_var(ok) || type_has_var(err),
        Type::Object(fields) => fields.iter().any(|field| type_has_var(&field.ty)),
        Type::Named(_, fields) => fields.iter().any(|field| type_has_var(&field.ty)),
        Type::Interface(_, fields) => fields.iter().any(|field| type_has_var(&field.ty)),
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
        Type::Recursion(name, args) => {
            Type::Recursion(name.clone(), args.iter().map(erase_type_vars).collect())
        }
        Type::Array(inner) => Type::Array(Box::new(erase_type_vars(inner))),
        Type::Tuple(items) => {
            Type::Tuple(items.iter().map(erase_type_vars).collect())
        }
        Type::Dict(key, value) => Type::Dict(
            Box::new(erase_type_vars(key)),
            Box::new(erase_type_vars(value)),
        ),
        Type::Promise(inner) => Type::Promise(Box::new(erase_type_vars(inner))),
        Type::Result(ok, err) => Type::Result(
            Box::new(erase_type_vars(ok)),
            Box::new(erase_type_vars(err)),
        ),
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
        Type::Interface(name, fields) => Type::Interface(
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
            param_names: function.param_names.clone(),
            defaults: function.defaults.clone(),
            loose: function.loose,
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
        Type::Recursion(name, args) => Type::Recursion(
            name.clone(),
            args.iter()
                .map(|arg| substitute_bound(arg, subst))
                .collect(),
        ),
        Type::Array(inner) => Type::Array(Box::new(substitute_bound(inner, subst))),
        Type::Tuple(items) => {
            Type::Tuple(items.iter().map(|item| substitute_bound(item, subst)).collect())
        }
        Type::Dict(key, value) => Type::Dict(
            Box::new(substitute_bound(key, subst)),
            Box::new(substitute_bound(value, subst)),
        ),
        Type::Promise(inner) => Type::Promise(Box::new(substitute_bound(inner, subst))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_bound(ok, subst)),
            Box::new(substitute_bound(err, subst)),
        ),
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
        Type::Interface(name, fields) => Type::Interface(
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
            param_names: function.param_names.clone(),
            defaults: function.defaults.clone(),
            loose: function.loose,
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
        Type::Recursion(_, args) => args.iter().any(has_type_var),
        Type::Array(inner) => has_type_var(inner),
        Type::Tuple(items) => items.iter().any(has_type_var),
        Type::Dict(key, value) => has_type_var(key) || has_type_var(value),
        Type::Promise(inner) => has_type_var(inner),
        Type::Result(ok, err) => has_type_var(ok) || has_type_var(err),
        Type::Object(fields) => fields.iter().any(|field| has_type_var(&field.ty)),
        Type::Named(_, fields) => fields.iter().any(|field| has_type_var(&field.ty)),
        Type::Interface(_, fields) => fields.iter().any(|field| has_type_var(&field.ty)),
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
        Type::Recursion(_, args) => {
            for arg in args {
                collect_type_vars(arg, out);
            }
        }
        Type::Array(inner) => collect_type_vars(inner, out),
        Type::Tuple(items) => {
            for item in items {
                collect_type_vars(item, out);
            }
        }
        Type::Dict(key, value) => {
            collect_type_vars(key, out);
            collect_type_vars(value, out);
        }
        Type::Promise(inner) => collect_type_vars(inner, out),
        Type::Result(ok, err) => {
            collect_type_vars(ok, out);
            collect_type_vars(err, out);
        }
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
        Type::Interface(_, fields) => {
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
        Type::Recursion(_, args) => args.iter().any(|arg| type_contains_var(arg, name)),
        Type::Array(inner) => type_contains_var(inner, name),
        Type::Tuple(items) => items.iter().any(|item| type_contains_var(item, name)),
        Type::Dict(key, value) => {
            type_contains_var(key, name) || type_contains_var(value, name)
        }
        Type::Promise(inner) => type_contains_var(inner, name),
        Type::Result(ok, err) => {
            type_contains_var(ok, name) || type_contains_var(err, name)
        }
        Type::Object(fields) => fields
            .iter()
            .any(|field| type_contains_var(&field.ty, name)),
        Type::Named(_, fields) => fields
            .iter()
            .any(|field| type_contains_var(&field.ty, name)),
        Type::Interface(_, fields) => fields
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

    if let (
        Type::Recursion(actual_name, actual_args),
        Type::Recursion(expected_name, expected_args),
    ) = (actual, expected)
    {
        if actual_name == expected_name && actual_args.len() != expected_args.len() {
            return false;
        }
        if actual_args.len() == expected_args.len() {
            return actual_args
                .iter()
                .zip(expected_args)
                .all(|(actual, expected)| unify_types(actual, expected, subst));
        }
    }

    if matches!(actual, Type::Any | Type::Recursion(_, _))
        || matches!(expected, Type::Any | Type::Recursion(_, _))
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
        (Type::Tuple(actual), Type::Tuple(expected)) => {
            if actual.len() != expected.len() {
                return false;
            }
            actual
                .iter()
                .zip(expected.iter())
                .all(|(actual, expected)| unify_types(actual, expected, subst))
        }
        (Type::Dict(actual_key, actual_value), Type::Dict(expected_key, expected_value)) => {
            unify_types(actual_key, expected_key, subst)
                && unify_types(actual_value, expected_value, subst)
        }
        (Type::Promise(actual), Type::Promise(expected)) => unify_types(actual, expected, subst),
        (Type::Result(actual_ok, actual_err), Type::Result(expected_ok, expected_err)) => {
            unify_types(actual_ok, expected_ok, subst)
                && unify_types(actual_err, expected_err, subst)
        }
        (Type::Object(actual), Type::Object(expected))
        | (Type::Named(_, actual), Type::Object(expected))
        | (Type::Object(actual), Type::Named(_, expected))
        | (Type::Named(_, actual), Type::Named(_, expected))
        | (Type::Interface(_, actual), Type::Object(expected))
        | (Type::Object(actual), Type::Interface(_, expected))
        | (Type::Named(_, actual), Type::Interface(_, expected))
        | (Type::Interface(_, actual), Type::Named(_, expected))
        | (Type::Interface(_, actual), Type::Interface(_, expected)) => {
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
                Some(Type::Var(bound)) if bound == name => ty.clone(),
                Some(bound) => {
                    seen.push(name.clone());
                    let result = substitute_with(bound, subst, seen);
                    seen.pop();
                    result
                }
                None => Type::Any,
            }
        }
        Type::Recursion(name, args) => Type::Recursion(
            name.clone(),
            args.iter()
                .map(|arg| substitute_with(arg, subst, seen))
                .collect(),
        ),
        Type::Array(inner) => Type::Array(Box::new(substitute_with(inner, subst, seen))),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| substitute_with(item, subst, seen))
                .collect(),
        ),
        Type::Dict(key, value) => Type::Dict(
            Box::new(substitute_with(key, subst, seen)),
            Box::new(substitute_with(value, subst, seen)),
        ),
        Type::Promise(inner) => Type::Promise(Box::new(substitute_with(inner, subst, seen))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_with(ok, subst, seen)),
            Box::new(substitute_with(err, subst, seen)),
        ),
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
        Type::Interface(name, fields) => Type::Interface(
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
            param_names: function.param_names.clone(),
            defaults: function.defaults.clone(),
            loose: function.loose,
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

#[cfg(test)]
mod tests {
    use super::{has_type_var, type_has_var};
    use crate::types::core::Type;

    #[test]
    fn recursive_placeholder_arguments_count_as_type_variables() {
        let ty = Type::Recursion("Tree".into(), vec![Type::Var("T".into())]);
        assert!(type_has_var(&ty));
        assert!(has_type_var(&ty));
    }
}
