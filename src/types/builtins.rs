//! The signatures of the runtime helpers.
//!
//! The types here mirror the runtime in `src/runtime/`, so that a call to a helper is
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

/// A helper with type parameters of its own.
///
/// `loose` says that a type parameter the arguments do not settle stands for
/// `any`, instead of being a mistake. The helpers of the runtime are called
/// without the value that would tell their type (`store("name")`), and that
/// call has to keep working.
pub(crate) fn builtin_generic(
    type_params: &[&str],
    params: Vec<Type>,
    required: usize,
    ret: Type,
    loose: bool,
) -> Type {
    let count = params.len();
    Type::Function(FunctionType {
        params,
        required,
        rest: false,
        ret: Box::new(ret),
        checked: true,
        constraints: Vec::new(),
        type_params: type_params.iter().map(|name| name.to_string()).collect(),
        param_names: Vec::new(),
        defaults: vec![false; count],
        loose,
    })
}

fn builtin_signature(params: Vec<Type>, required: usize, rest: bool, ret: Type) -> Type {
    let count = params.len();
    Type::Function(FunctionType {
        params,
        required,
        rest,
        ret: Box::new(ret),
        checked: true,
        constraints: Vec::new(),
        type_params: Vec::new(),
        param_names: Vec::new(),
        defaults: vec![false; count],
        loose: false,
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

/// A type parameter, by its name.
fn type_var(name: &str) -> Type {
    Type::Var(name.to_string())
}

/// The options of a signal and of a store: the way two values are asked
/// whether they are the same.
fn state_options() -> Type {
    Type::Object(vec![ObjectField {
        name: "equals".to_string(),
        ty: union(vec![
            builtin_function(vec![Type::Any, Type::Any], Type::Bool),
            Type::String,
        ]),
        optional: true,
    }])
}

/// A function of no arguments.
fn thunk(ret: Type) -> Type {
    builtin_function(Vec::new(), ret)
}

/// The values of a signal: what it holds, and the ways it is read, changed
/// and listened to.
///
/// The shape is a type of its own, so that a signal that is a `const` keeps
/// the type of the value it was given.
pub(crate) fn signal_type(value: Type) -> Type {
    Type::Interface("Signal".to_string(), signal_fields(value))
}

/// The values of a store: the state it holds, and the ways it is read,
/// changed and listened to.
pub(crate) fn store_type(state: Type) -> Type {
    Type::Interface("Store".to_string(), store_fields(state))
}

fn signal_fields(value: Type) -> Vec<ObjectField> {
    let fields = vec![
        ("value", value.clone()),
        ("set", builtin_function(vec![value.clone()], value.clone())),
        (
            "update",
            builtin_function(
                vec![builtin_function(vec![value.clone()], value.clone())],
                value.clone(),
            ),
        ),
        ("peek", thunk(value.clone())),
        (
            "subscribe",
            builtin_function(vec![builtin_function(vec![value], Type::Void)], thunk(Type::Void)),
        ),
        ("toString", thunk(Type::String)),
    ];
    object_fields(fields)
}

fn store_fields(state: Type) -> Vec<ObjectField> {
    let fields = vec![
        ("value", state.clone()),
        // One field of the state is changed by its name; the value of a
        // field is not the state itself, so it stays `any`.
        ("set", builtin_function(vec![Type::String, Type::Any], state.clone())),
        ("replace", builtin_function(vec![state.clone()], state.clone())),
        (
            "update",
            builtin_function(
                vec![builtin_function(vec![state.clone()], state.clone())],
                state.clone(),
            ),
        ),
        ("peek", thunk(state.clone())),
        (
            "subscribe",
            builtin_function(vec![builtin_function(vec![state], Type::Void)], thunk(Type::Void)),
        ),
        ("toString", thunk(Type::String)),
    ];
    object_fields(fields)
}

/// The fields of a shape, by name and type.
fn object_fields(fields: Vec<(&str, Type)>) -> Vec<ObjectField> {
    fields
        .into_iter()
        .map(|(name, ty)| ObjectField {
            name: name.to_string(),
            ty,
            optional: false,
        })
        .collect()
}

/// The values of a signal that a `computed` makes: the same, and the way the
/// work that follows the values is let go.
fn computed_type(value: Type) -> Type {
    let Type::Interface(name, mut fields) = signal_type(value) else {
        return Type::Any;
    };
    fields.push(ObjectField {
        name: "dispose".to_string(),
        ty: thunk(Type::Void),
        optional: false,
    });
    Type::Interface(name, fields)
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
    // A value that will be there later.
    let promise = |inner: Type| Type::Promise(Box::new(inner));
    let promise_any = || promise(any());

    match name {
        "print" => Some(builtin_rest(any(), Type::Void)),
        "undefined" => Some(Type::Undefined),
        "bind" => Some(builtin_function(vec![any(), any()], any())),
        "input" => Some(builtin_optional(vec![string()], 0, string())),
        "sleep" => Some(builtin_function(vec![number()], promise(Type::Void))),
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
        "text" => Some(builtin_object(vec![
            ("capitalize", builtin_function(vec![string()], string())),
            ("slug", builtin_function(vec![string()], string())),
            ("camel", builtin_function(vec![string()], string())),
            ("kebab", builtin_function(vec![string()], string())),
            (
                "padStart",
                builtin_optional(vec![string(), number(), string()], 2, string()),
            ),
            (
                "padEnd",
                builtin_optional(vec![string(), number(), string()], 2, string()),
            ),
            (
                "repeat",
                builtin_function(vec![string(), number()], string()),
            ),
            ("reverse", builtin_function(vec![string()], string())),
            ("words", builtin_function(vec![string()], list(string()))),
            ("dedent", builtin_function(vec![string()], string())),
            ("encode", builtin_function(vec![string()], list(number()))),
            ("decode", builtin_function(vec![list(number())], string())),
        ])),
        "date" => Some(builtin_object(vec![
            ("now", builtin_function(vec![], number())),
            ("iso", builtin_function(vec![], string())),
            ("parts", builtin_function(vec![any()], any())),
            (
                "format",
                builtin_optional(vec![any(), string()], 1, string()),
            ),
            (
                "daysBetween",
                builtin_function(vec![any(), any()], number()),
            ),
            ("addDays", builtin_function(vec![any(), number()], any())),
        ])),
        "query" => Some(builtin_object(vec![
            ("parse", builtin_function(vec![any()], any())),
            ("stringify", builtin_function(vec![any()], string())),
        ])),
        "cookie" => Some(builtin_object(vec![
            ("parse", builtin_function(vec![any()], any())),
            ("stringify", builtin_function(vec![any()], string())),
        ])),
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
        "assertEquals" => Some(builtin_optional(
            vec![any(), any(), string()],
            2,
            Type::Void,
        )),
        "assertNotEquals" => Some(builtin_optional(
            vec![any(), any(), string()],
            2,
            Type::Void,
        )),
        "assertDeepEquals" => Some(builtin_optional(
            vec![any(), any(), string()],
            2,
            Type::Void,
        )),
        "assertContains" => Some(builtin_optional(
            vec![any(), any(), string()],
            2,
            Type::Void,
        )),
        "assertNotContains" => Some(builtin_optional(
            vec![any(), any(), string()],
            2,
            Type::Void,
        )),
        "assertThrows" => Some(builtin_optional(vec![any(), string()], 1, Type::Void)),
        "assertMatch" => Some(builtin_optional(vec![any(), any(), string()], 2, Type::Void)),
        "test" => Some(builtin_function(vec![string(), any()], Type::Void)),
        "all" => Some(builtin_function(vec![list(any())], promise(list(any())))),
        "race" => Some(builtin_function(vec![list(any())], promise_any())),
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
        "ok" => Some(builtin_function(vec![any()], any())),
        "err" => Some(builtin_function(vec![any()], any())),
        "todo" | "unimplemented" => Some(builtin_optional(vec![string()], 0, Type::Never)),
        "sum" => Some(builtin_function(vec![list(number())], number())),
        "unique" | "reverse" => Some(builtin_function(vec![list_or_text()], list(any()))),
        "html" => Some(builtin_function(vec![any()], string())),
        "raw" => Some(builtin_function(vec![any()], any())),
        "cls" => Some(builtin_rest(any(), string())),
        "server" => Some(builtin_optional(vec![number(), any()], 0, any())),
        "require" => Some(builtin_function(vec![string()], any())),
        "hash" => Some(builtin_function(vec![any()], string())),
        "highlight" => Some(builtin_function(vec![string()], string())),
        "title" => Some(builtin_function(vec![any()], Type::Void)),
        "clipboard" => Some(builtin_object(vec![(
            "write",
            builtin_function(vec![string()], promise(Type::Void)),
        )])),
        "onShutdown" => Some(builtin_function(vec![any()], any())),
        "process" => Some(builtin_object(vec![
            ("cwd", builtin_function(vec![], string())),
            ("argv", builtin_function(vec![], list(string()))),
            ("platform", builtin_function(vec![], string())),
            ("exit", builtin_function(vec![number()], Type::Void)),
            ("spawn", builtin_function(vec![string(), any()], any())),
            ("run", builtin_function(vec![string(), any()], any())),
        ])),
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
            ("get", builtin_optional(vec![string(), any()], 1, promise(response_type()))),
            (
                "post",
                builtin_optional(vec![string(), any(), any()], 2, promise(response_type())),
            ),
            ("fetch", builtin_optional(vec![string(), any()], 1, promise(response_type()))),
            ("Request", builtin_optional(vec![string(), any()], 1, any())),
            ("Response", builtin_optional(vec![any(), any()], 1, any())),
            ("Headers", builtin_optional(vec![any()], 0, any())),
            ("AbortController", builtin_function(vec![], any())),
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
            ("readStream", builtin_function(vec![string()], any())),
            ("writeStream", builtin_function(vec![string(), any()], any())),
        ])),
        "path" => Some(builtin_object(vec![
            ("join", builtin_rest(string(), string())),
            ("dirname", builtin_function(vec![string()], string())),
            ("basename", builtin_function(vec![string()], string())),
            ("extname", builtin_function(vec![string()], string())),
            ("resolve", builtin_rest(string(), string())),
        ])),
        "Map" => Some(builtin_optional(vec![any()], 0, map_type())),
        "Set" => Some(builtin_optional(vec![any()], 0, set_type())),
        // The library of the browser and of the server.
        "regex" => Some(builtin_optional(vec![string(), string()], 1, any())),
        "intl" => Some(builtin_object(vec![
            (
                "formatNumber",
                builtin_optional(vec![number(), string()], 1, string()),
            ),
            (
                "formatDate",
                builtin_optional(vec![any(), any()], 1, string()),
            ),
            (
                "formatTime",
                builtin_optional(vec![any(), any()], 1, string()),
            ),
            (
                "dayName",
                builtin_optional(vec![number(), string()], 1, string()),
            ),
            (
                "monthName",
                builtin_optional(vec![number(), string()], 1, string()),
            ),
            (
                "relativeTime",
                builtin_optional(vec![number(), string(), string()], 2, string()),
            ),
        ])),
        "bytes" => Some(builtin_object(vec![
            (
                "from",
                builtin_optional(vec![string(), string()], 1, list(number())),
            ),
            ("toBase64", builtin_function(vec![list(number())], string())),
            ("fromBase64", builtin_function(vec![string()], list(number()))),
            ("concat", builtin_function(vec![list(any())], list(number()))),
            ("length", builtin_function(vec![list(number())], number())),
            ("hex", builtin_function(vec![list(number())], string())),
            ("text", builtin_function(vec![list(number())], string())),
        ])),
        "deepCopy" => Some(builtin_function(vec![any()], any())),
        // The state of a page: typed by the value it is given, so that a
        // wrong `set` is said and the fields of a store are known.
        "signal" => Some(builtin_generic(
            &["T"],
            vec![type_var("T"), state_options()],
            0,
            signal_type(type_var("T")),
            true,
        )),
        "store" => Some(builtin_generic(
            &["T"],
            vec![
                // The state may be given on its own, or after a name that
                // shares it between the parts of a page.
                union(vec![Type::String, type_var("T")]),
                union(vec![state_options(), type_var("T")]),
                state_options(),
            ],
            0,
            store_type(type_var("T")),
            true,
        )),
        // The values of a form, and what is wrong with them.
        "form" => Some(builtin_optional(vec![any(), any()], 0, any())),
        "effect" => Some(builtin_function(vec![any()], any())),
        "computed" => Some(builtin_generic(
            &["T"],
            vec![builtin_function(Vec::new(), type_var("T"))],
            1,
            computed_type(type_var("T")),
            true,
        )),
        "mount" => Some(builtin_function(vec![any(), any()], any())),
        "route" => Some(builtin_function(vec![string(), any()], any())),
        "navigate" => Some(builtin_function(vec![string()], any())),
        "currentRoute" => Some(builtin_function(vec![], any())),
        "routeParams" => Some(builtin_function(vec![], any())),
        "on" => Some(builtin_function(vec![any(), string(), any()], any())),
        // The component that catches the mistakes of the ones under it.
        "Boundary" => Some(builtin_function(vec![any()], any())),
        // The values that the components share.
        "provide" => Some(builtin_function(vec![string(), any()], any())),
        "inject" => Some(builtin_function(vec![string()], any())),
        // A value that is written in another place of the page.
        "portal" => Some(builtin_optional(vec![any(), any(), string()], 2, any())),
        // The way a node comes to the page and goes from it.
        "transition" => Some(builtin_optional(vec![number(), any()], 0, any())),
        "delegate" => Some(builtin_function(
            vec![any(), string(), string(), any()],
            any(),
        )),
        "onMount" | "onDestroy" => Some(builtin_function(vec![any()], any())),
        "ref" => Some(builtin_function(vec![], any())),
        "setTimeout" | "setInterval" => Some(builtin_optional(
            vec![any(), number()],
            1,
            any(),
        )),
        "clearTimeout" | "clearInterval" | "cancelAnimationFrame" => {
            Some(builtin_function(vec![any()], Type::Void))
        }
        "requestAnimationFrame" => Some(builtin_function(vec![any()], number())),
        "fetch" => Some(builtin_optional(vec![string(), any()], 1, promise(response_type()))),
        "Request" => Some(builtin_optional(vec![string(), any()], 1, any())),
        "Response" => Some(builtin_optional(vec![any(), any()], 1, any())),
        "Headers" => Some(builtin_optional(vec![any()], 0, any())),
        "AbortController" => Some(builtin_function(vec![], any())),
        "storage" | "sessionStorage" => Some(builtin_object(vec![
            ("get", builtin_function(vec![string()], any())),
            ("set", builtin_function(vec![string(), any()], Type::Void)),
            ("remove", builtin_function(vec![string()], Type::Void)),
            ("clear", builtin_function(vec![], Type::Void)),
            ("keys", builtin_function(vec![], list(string()))),
        ])),
        "history" => Some(builtin_object(vec![
            (
                "push",
                builtin_optional(vec![any(), string(), string()], 0, Type::Void),
            ),
            (
                "replace",
                builtin_optional(vec![any(), string(), string()], 0, Type::Void),
            ),
            ("back", builtin_function(vec![], Type::Void)),
            ("forward", builtin_function(vec![], Type::Void)),
            ("go", builtin_optional(vec![number()], 0, Type::Void)),
            ("state", builtin_function(vec![], any())),
        ])),
        "Blob" => Some(builtin_optional(vec![any(), any()], 1, any())),
        "File" => Some(builtin_optional(vec![any(), string(), any()], 1, any())),
        "FileReader" => Some(builtin_function(vec![], any())),
        "FormData" => Some(builtin_function(vec![], any())),
        "WebSocket" => Some(builtin_optional(vec![string(), any()], 1, any())),
        "EventSource" => Some(builtin_optional(vec![string(), any()], 1, any())),
        "matchMedia" => Some(builtin_function(vec![string()], any())),
        "IntersectionObserver" | "ResizeObserver" => {
            Some(builtin_optional(vec![any(), any()], 1, any()))
        }
        "crypto" => Some(builtin_object(vec![
            ("randomBytes", builtin_function(vec![number()], list(number()))),
            ("sign", builtin_function(vec![any(), string()], string())),
            (
                "verify",
                builtin_function(vec![any(), string(), string()], bool_ty()),
            ),
            ("keyPair", builtin_function(vec![], any())),
        ])),
        "zlib" => Some(builtin_object(vec![
            ("gzip", builtin_function(vec![any()], list(number()))),
            ("gunzip", builtin_function(vec![any()], string())),
            ("deflate", builtin_function(vec![any()], list(number()))),
            ("inflate", builtin_function(vec![any()], string())),
        ])),
        "logger" | "cors" | "compress" | "errorHandler" | "security"
        | "rateLimit" | "csrf" => Some(builtin_optional(vec![any()], 0, any())),
        "bodyLimit" => Some(builtin_function(vec![number()], any())),
        "session" => Some(builtin_object(vec![
            ("start", builtin_function(vec![], string())),
            ("get", builtin_function(vec![string()], any())),
            ("set", builtin_function(vec![string(), any()], any())),
            ("end", builtin_function(vec![], Type::Void)),
            ("id", builtin_function(vec![], any())),
        ])),
        _ => None,
    }
}

/// The type of the answer of a request.
///
/// The answer of a request has the marks of it, the text of it and the
/// value that is in it.
fn response_type() -> Type {
    builtin_object(vec![
        ("status", Type::Number),
        ("statusText", Type::String),
        ("ok", Type::Bool),
        ("url", Type::String),
        ("redirected", Type::Bool),
        ("body", Type::Any),
        ("headers", Type::Any),
        ("text", builtin_function(vec![], Type::Promise(Box::new(Type::String)))),
        ("json", builtin_function(vec![], Type::Promise(Box::new(Type::Any)))),
        ("clone", builtin_function(vec![], Type::Any)),
    ])
}

/// The type of a map of the runtime.
fn map_type() -> Type {
    let any = Type::Any;
    let list = Type::Array(Box::new(Type::Any));
    builtin_object(vec![
        ("get", builtin_function(vec![any.clone()], any.clone())),
        ("set", builtin_function(vec![any.clone(), any.clone()], any.clone())),
        ("has", builtin_function(vec![any.clone()], Type::Bool)),
        ("delete", builtin_function(vec![any.clone()], Type::Bool)),
        ("clear", builtin_function(vec![], any.clone())),
        ("keys", builtin_function(vec![], list.clone())),
        ("values", builtin_function(vec![], list.clone())),
        ("entries", builtin_function(vec![], list)),
        ("forEach", builtin_function(vec![any.clone()], Type::Void)),
        ("size", Type::Number),
        ("toList", builtin_function(vec![], Type::Array(Box::new(Type::Any)))),
    ])
}

/// The type of a set of the runtime.
fn set_type() -> Type {
    let any = Type::Any;
    let list = Type::Array(Box::new(Type::Any));
    builtin_object(vec![
        ("add", builtin_function(vec![any.clone()], any.clone())),
        ("has", builtin_function(vec![any.clone()], Type::Bool)),
        ("delete", builtin_function(vec![any.clone()], Type::Bool)),
        ("clear", builtin_function(vec![], any.clone())),
        ("values", builtin_function(vec![], list)),
        ("forEach", builtin_function(vec![any.clone()], Type::Void)),
        ("size", Type::Number),
        ("toList", builtin_function(vec![], Type::Array(Box::new(Type::Any)))),
    ])
}

pub(crate) fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "any"
            | "unknown"
            | "number"
            | "string"
            | "bool"
            | "null"
            | "undefined"
            | "void"
            | "never"
            | "symbol"
            | "bigint"
            | "object"
    )
}
