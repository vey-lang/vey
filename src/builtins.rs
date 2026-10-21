/// The names that the runtime provides.
///
/// They are always in scope, so the checker does not report them as unknown.
pub const BUILTINS: &[&str] = &[
    "print", "input", "sleep", "str", "num", "int", "bool", "type", "len", "append", "slice",
    "join", "split", "upper", "lower", "trim", "replace", "includes", "find", "sort", "map",
    "filter", "reduce", "keys", "values", "entries", "range", "flatMap", "error", "html", "raw",
    "bind", "server", "response", "redirect", "cls", "json", "math", "time", "http", "assert",
    "test", "all", "race", "env", "uuid", "random", "clamp", "sum", "unique", "reverse", "base64",
    "url", "fs", "path",
];

/// Whether the runtime provides a helper with this name.
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}
