//! The signatures of the runtime helpers.
//!
//! The types here mirror the runtime in `runtime/`, so that a call to a helper is
//! checked in the same way as a call to a function of the program.

use crate::types::core::{union, FunctionType, ObjectField, Type};

/// A helper that takes a fixed number of arguments.
pub(crate) fn builtin_function(params: Vec<Type>, ret: Type) -> Type {
    let required = params.len();
    builtin_signature(params, required, false, ret)
}

/// A helper whose last arguments may be left out.
pub(crate) fn builtin_optional(params: Vec<Type>, required: usize, ret: Type) -> Type {
    builtin_signature(params, required, false, ret)
}

/// A helper that takes any number of arguments of one type.
pub(crate) fn builtin_rest(param: Type, ret: Type) -> Type {
    builtin_signature(vec![param], 0, true, ret)
}

fn builtin_signature(params: Vec<Type>, required: usize, rest: bool, ret: Type) -> Type {
    Type::Function(FunctionType {
        params,
        required,
        rest,
        ret: Box::new(ret),
        checked: true,
        constraints: Vec::new(),
        type_params: Vec::new(),
    })
}

pub(crate) fn builtin_object(fields: Vec<(&str, Type)>) -> Type {
    Type::Object(
        fields
            .into_iter()
            .map(|(name, ty)| ObjectField {
                name: name.to_string(),
                ty,
                optional: false,
            })
            .collect(),
    )
}

pub(crate) fn builtin_type(name: &str) -> Option<Type> {
    let number = || Type::Number;
    let string = || Type::String;
    let bool_ty = || Type::Bool;
    let any = || Type::Any;
    let list = |item: Type| Type::Array(Box::new(item));
    // The list helpers also accept text, which they read as a list of
    // characters.
    let list_or_text = || union(vec![list(any()), string()]);
    let promise = || Type::Any;

    match name {
        "print" => Some(builtin_rest(any(), Type::Void)),
        "bind" => Some(builtin_function(vec![any(), any()], Type::Void)),
        "input" => Some(builtin_optional(vec![string()], 0, string())),
        "sleep" => Some(builtin_function(vec![number()], promise())),
        "str" => Some(builtin_function(vec![any()], string())),
        "num" | "int" => Some(builtin_function(vec![any()], number())),
        "bool" => Some(builtin_function(vec![any()], bool_ty())),
        "type" => Some(builtin_function(vec![any()], string())),
        "len" => Some(builtin_function(vec![any()], number())),
        "append" => Some(builtin_signature(
            vec![list_or_text(), any()],
            1,
            true,
            list(any()),
        )),
        "slice" => Some(builtin_optional(vec![any(), number(), number()], 1, any())),
        "join" => Some(builtin_optional(
            vec![list_or_text(), string()],
            1,
            string(),
        )),
        "split" => Some(builtin_optional(
            vec![string(), string()],
            1,
            list(string()),
        )),
        "upper" | "lower" | "trim" => Some(builtin_function(vec![string()], string())),
        "replace" => Some(builtin_optional(
            vec![string(), string(), string()],
            2,
            string(),
        )),
        "includes" => Some(builtin_function(vec![any(), any()], bool_ty())),
        "startsWith" => Some(builtin_function(vec![string(), string()], bool_ty())),
        "endsWith" => Some(builtin_function(vec![string(), string()], bool_ty())),
        "find" => Some(builtin_function(vec![list_or_text(), any()], any())),
        "sort" => Some(builtin_optional(
            vec![list_or_text(), any()],
            1,
            list(any()),
        )),
        "map" | "filter" | "flatMap" => {
            Some(builtin_function(vec![list_or_text(), any()], list(any())))
        }
        "reduce" => Some(builtin_optional(
            vec![list_or_text(), any(), any()],
            2,
            any(),
        )),
        "keys" => Some(builtin_function(vec![any()], list(string()))),
        "values" | "entries" => Some(builtin_function(vec![any()], list(any()))),
        "omit" => Some(builtin_function(vec![any(), list(any())], any())),
        "range" => Some(builtin_optional(
            vec![number(), number()],
            1,
            list(number()),
        )),
        "error" => Some(builtin_function(vec![any()], Type::Never)),
        "assert" => Some(builtin_optional(vec![any(), string()], 1, Type::Void)),
        "test" => Some(builtin_function(vec![string(), any()], Type::Void)),
        "all" | "race" => Some(builtin_function(vec![list(any())], promise())),
        "env" => Some(builtin_optional(
            vec![string(), string()],
            1,
            union(vec![string(), Type::Null]),
        )),
        "uuid" => Some(builtin_function(vec![], string())),
        "random" => Some(builtin_optional(vec![number(), number()], 0, number())),
        "clamp" => Some(builtin_function(
            vec![number(), number(), number()],
            number(),
        )),
        "sum" => Some(builtin_function(vec![list(number())], number())),
        "unique" | "reverse" => Some(builtin_function(vec![list_or_text()], list(any()))),
        "html" => Some(builtin_function(vec![any()], string())),
        "raw" => Some(builtin_function(vec![any()], any())),
        "cls" => Some(builtin_rest(any(), string())),
        "server" => Some(builtin_optional(vec![number()], 0, any())),
        "response" => Some(builtin_optional(vec![any(), any(), any()], 1, any())),
        "redirect" => Some(builtin_optional(vec![string(), number()], 1, any())),
        "json" => Some(builtin_object(vec![
            ("parse", builtin_function(vec![string()], any())),
            (
                "stringify",
                builtin_optional(vec![any(), number()], 1, string()),
            ),
        ])),
        "math" => Some(builtin_object(vec![
            ("abs", builtin_function(vec![number()], number())),
            ("ceil", builtin_function(vec![number()], number())),
            ("floor", builtin_function(vec![number()], number())),
            ("round", builtin_function(vec![number()], number())),
            ("min", builtin_rest(number(), number())),
            ("max", builtin_rest(number(), number())),
            ("sqrt", builtin_function(vec![number()], number())),
            ("pow", builtin_function(vec![number(), number()], number())),
            ("random", builtin_function(vec![], number())),
            ("pi", number()),
        ])),
        "time" => Some(builtin_object(vec![
            ("now", builtin_function(vec![], number())),
            ("iso", builtin_function(vec![], string())),
        ])),
        "http" => Some(builtin_object(vec![
            ("get", builtin_optional(vec![string(), any()], 1, promise())),
            (
                "post",
                builtin_optional(vec![string(), any(), any()], 2, promise()),
            ),
        ])),
        "base64" => Some(builtin_object(vec![
            ("encode", builtin_function(vec![string()], string())),
            ("decode", builtin_function(vec![string()], string())),
        ])),
        "url" => Some(builtin_object(vec![
            ("encode", builtin_function(vec![string()], string())),
            ("decode", builtin_function(vec![string()], string())),
            ("parse", builtin_function(vec![string()], any())),
        ])),
        "fs" => Some(builtin_object(vec![
            (
                "read",
                builtin_optional(vec![string(), string()], 1, string()),
            ),
            ("write", builtin_function(vec![string(), any()], Type::Void)),
            (
                "append",
                builtin_function(vec![string(), any()], Type::Void),
            ),
            ("exists", builtin_function(vec![string()], bool_ty())),
            ("list", builtin_function(vec![string()], list(string()))),
            ("remove", builtin_function(vec![string()], Type::Void)),
            ("mkdir", builtin_function(vec![string()], Type::Void)),
        ])),
        "path" => Some(builtin_object(vec![
            ("join", builtin_rest(string(), string())),
            ("dirname", builtin_function(vec![string()], string())),
            ("basename", builtin_function(vec![string()], string())),
            ("extname", builtin_function(vec![string()], string())),
            ("resolve", builtin_rest(string(), string())),
        ])),
        _ => None,
    }
}

pub(crate) fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "any" | "number" | "string" | "bool" | "null" | "void" | "never"
    )
}
