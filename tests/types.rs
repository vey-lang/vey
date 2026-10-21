//! Tests for the type checker.

mod common;

use common::run_err;

fn check_ok(label: &str, source: &str) {
    let output = common::run_source(label, source);
    assert!(
        output.status.success(),
        "expected success, got:\n{}",
        common::stderr(&output)
    );
}

fn check_err(label: &str, source: &str) -> String {
    run_err(label, source)
}

#[test]
fn a_rest_parameter_takes_many_arguments() {
    check_ok(
        "rest-call",
        r#"
fn total(...nums: [number]) -> number {
  return 0
}
let t = total(1, 2, 3)
"#,
    );
}

#[test]
fn a_generic_type_is_not_bound_by_a_nullable_argument() {
    check_ok(
        "generic-union",
        r#"
fn pick<T>(maybe: T | null, value: T) -> T {
  return value
}
let n: number = pick(null, 1)
"#,
    );
}

#[test]
fn a_shadowed_name_keeps_the_type_of_the_outer_binding() {
    check_ok(
        "shadowing",
        r#"
fn f(x: number) -> number {
  for x in ["a"] {
    print(x)
  }
  return x
}
"#,
    );
    check_ok(
        "shadowing-outer",
        r#"
let x = 1
for x in ["a", "b"] { print(x) }
let y: number = x
"#,
    );
}

#[test]
fn a_narrowing_survives_an_early_return() {
    check_ok(
        "narrow-after-return",
        r#"
fn f(x: number | null) -> number {
  if x == null {
    return 0
  }
  return x - 1
}
"#,
    );
}

#[test]
fn a_function_may_not_fall_off_the_end() {
    let error = check_err(
        "fall-off-end",
        r#"
fn f(x: number) -> number {
  if x > 0 {
    return 1
  }
}
"#,
    );
    assert!(error.contains("must return"), "{error}");
}

#[test]
fn a_function_with_a_return_type_on_every_path_is_accepted() {
    check_ok(
        "returns-everywhere",
        r#"
fn f(x: number) -> number {
  if x > 0 {
    return 1
  } else {
    return 2
  }
}
"#,
    );
}

#[test]
fn a_generic_alias_can_be_used_from_another_module() {
    let dir = common::temp_dir("generic-module");
    std::fs::write(dir.join("types.vey"), "export type Box<T> = { value: T }\n").unwrap();
    std::fs::write(
        dir.join("app.vey"),
        r#"
use "./types.vey" as t
let b: t.Box<number> = { value: 2 }
print(b.value)
"#,
    )
    .unwrap();
    let output = common::run_file(&dir.join("app.vey"));
    assert!(output.status.success(), "{}", common::stderr(&output));
    assert!(common::stdout(&output).contains('2'));
}

#[test]
fn an_unknown_name_in_an_alias_is_reported() {
    let error = check_err("alias-unknown", "type Alias = { v: Nope }\n");
    assert!(error.contains("unknown type 'Nope'"), "{error}");
}

#[test]
fn an_alias_does_not_capture_the_type_parameters_of_its_user() {
    let error = check_err(
        "alias-capture",
        "type Alias = { v: T }\nfn f<T>(x: Alias) -> void { print(x.v) }\n",
    );
    assert!(error.contains("unknown type 'T'"), "{error}");
}

#[test]
fn a_broken_exported_alias_reports_the_real_error() {
    let dir = common::temp_dir("broken-export");
    std::fs::write(dir.join("bad.vey"), "export type Bad = { v: Nope }\n").unwrap();
    std::fs::write(
        dir.join("app.vey"),
        "use \"./bad.vey\" as t\nlet x: t.Bad = { v: 1 }\n",
    )
    .unwrap();
    let output = common::run_file(&dir.join("app.vey"));
    assert!(!output.status.success());
    let error = common::stderr(&output);
    assert!(error.contains("unknown type 'Nope'"), "{error}");
}

#[test]
fn a_function_with_an_optional_parameter_fits_a_shorter_type() {
    check_ok(
        "optional-assign",
        r#"
fn withDefault(a: number, b: number = 1) -> number { return a }
let g: (number) -> number = withDefault
"#,
    );
}

#[test]
fn a_fixed_function_fits_a_variadic_type() {
    check_ok(
        "variadic-assign",
        r#"
fn apply(f: (...number) -> number) -> number { return f(1, 2, 3) }
let t = apply((a: number, b: number, c: number) => a + b + c)
"#,
    );
}

#[test]
fn a_type_check_does_not_narrow_to_an_impossible_type() {
    let error = check_err(
        "unsound-narrowing",
        r#"
fn f(s: string) -> void {
  if type(s) == "number" {
    let x: number = s
    print(x)
  }
}
"#,
    );
    assert!(error.contains("expected number"), "{error}");
}

#[test]
fn a_component_needs_its_required_arguments() {
    let error = check_err(
        "component-missing",
        r#"
component Card(title: string, count: number) { h2 { title } }
component App() {
  Card { "no title" }
}
"#,
    );
    assert!(error.contains("missing the argument 'title'"), "{error}");
}

#[test]
fn a_component_with_all_arguments_is_accepted() {
    check_ok(
        "component-arguments",
        r#"
component Card(title: string, count: number = 1) { h2 { title } }
component App() {
  Card title="x" count=2
}
"#,
    );
}

#[test]
fn a_builtin_call_is_checked() {
    let error = check_err("builtin-argument", "let a = math.round(\"hello\")\n");
    assert!(error.contains("expects number"), "{error}");

    let error = check_err("builtin-count", "let a = len()\n");
    assert!(error.contains("expected at least 1 argument"), "{error}");
}

#[test]
fn a_variadic_builtin_takes_many_arguments() {
    check_ok(
        "builtin-variadic",
        "let d = path.join(\"a\", \"b\", \"c\")\nlet e = math.min(1, 2, 3)\n",
    );
}

#[test]
fn an_optional_builtin_argument_may_be_left_out() {
    check_ok(
        "builtin-optional",
        r#"
let a = range(3)
let b = join(["a"])
let c = json.stringify({ x: 1 })
let d = env("HOME")
"#,
    );
}

#[test]
fn a_recursive_alias_is_reported() {
    let error = check_err(
        "recursive-alias",
        "type Node = { value: number, next: Node | null }\nlet n: Node = { value: 1, next: null }\n",
    );
    assert!(error.contains("recursive type alias"), "{error}");
}

#[test]
fn a_mismatch_is_reported() {
    let error = check_err("mismatch", "let x: number = \"str\"\n");
    assert!(error.contains("expected number"), "{error}");
}
