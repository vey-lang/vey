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
    let output = common::check_file(&dir.join("app.vey"));
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
fn a_recursive_alias_may_be_used() {
    check_ok(
        "recursive-alias",
        r#"
type Node = { value: number, next: Node | null }

let a: Node = { value: 1, next: null }
let b: Node = { value: 2, next: { value: 3, next: null } }
print(a.value)
print(b.next.value)
"#,
    );
}

#[test]
fn a_recursive_alias_still_checks_its_other_fields() {
    let error = check_err(
        "recursive-alias-field",
        "type Node = { value: number, next: Node | null }\nlet a: Node = { value: \"x\", next: null }\n",
    );
    assert!(error.contains("expected Node"), "{error}");
}

#[test]
fn a_mismatch_is_reported() {
    let error = check_err("mismatch", "let x: number = \"str\"\n");
    assert!(error.contains("expected number"), "{error}");
}

#[test]
fn a_type_parameter_may_have_a_constraint() {
    check_ok(
        "constraint-ok",
        "fn total<T: number>(list: [T]) -> number {\n  return 0\n}\nprint(total([1, 2]))\n",
    );
    let error = check_err(
        "constraint-broken",
        "fn total<T: number>(list: [T]) -> number {\n  return 0\n}\nprint(total([\"a\"]))\n",
    );
    assert!(
        error.contains("type argument 'T' expects number"),
        "{error}"
    );
}

#[test]
fn a_constraint_may_be_a_union() {
    check_ok(
        "constraint-union",
        "fn pick<T: number | string>(v: T) -> T {\n  return v\n}\nprint(pick(1))\nprint(pick(\"a\"))\n",
    );
}

#[test]
fn a_call_may_name_its_type_arguments() {
    check_ok(
        "explicit-type-argument",
        "fn id<T>(v: T) -> T {\n  return v\n}\nprint(id<number>(1))\n",
    );
    let error = check_err(
        "explicit-type-argument-count",
        "fn id<T>(v: T) -> T {\n  return v\n}\nprint(id<number, string>(1))\n",
    );
    assert!(error.contains("expected 1 type argument"), "{error}");
}

#[test]
fn a_comparison_is_not_a_call_with_type_arguments() {
    check_ok(
        "comparison",
        "let a = 1\nlet b = 2\nprint(a < b)\nprint(a > b)\n",
    );
}

#[test]
fn a_generic_return_takes_the_type_of_the_value() {
    check_ok(
        "generic-return",
        "fn parse<T>(text: string) -> T {\n  return num(text)\n}\nlet n: number = parse(\"42\")\n",
    );
    let error = check_err(
        "generic-return-wrong",
        "fn parse<T>(text: string) -> T {\n  return num(text)\n}\nlet n: string = parse(\"42\")\n",
    );
    assert!(error.contains("expected string, found number"), "{error}");
}

#[test]
fn a_call_to_another_module_is_checked() {
    let dir = common::temp_dir("module-signature");
    std::fs::write(
        dir.join("lib.vey"),
        "export fn add(a: number, b: number) -> number {\n  return a + b\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.vey"),
        "use \"./lib.vey\" as lib\nlet n: string = lib.add(1, 2)\n",
    )
    .unwrap();
    let output = common::check_file(&dir.join("app.vey"));
    assert!(!output.status.success());
    let error = common::stderr(&output);
    assert!(error.contains("expected string, found number"), "{error}");
}

#[test]
fn a_call_to_another_module_with_a_wrong_argument_is_checked() {
    let dir = common::temp_dir("module-argument");
    std::fs::write(
        dir.join("lib.vey"),
        "export fn add(a: number, b: number) -> number {\n  return a + b\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.vey"),
        "use \"./lib.vey\" as lib\nlib.add(1, \"two\")\n",
    )
    .unwrap();
    let output = common::check_file(&dir.join("app.vey"));
    assert!(!output.status.success());
    let error = common::stderr(&output);
    assert!(error.contains("argument 2 expects number"), "{error}");
}

#[test]
fn a_call_to_another_module_with_too_few_arguments_is_checked() {
    let dir = common::temp_dir("module-arity");
    std::fs::write(
        dir.join("lib.vey"),
        "export fn add(a: number, b: number) -> number {\n  return a + b\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.vey"),
        "use \"./lib.vey\" as lib\nlib.add(1)\n",
    )
    .unwrap();
    let output = common::check_file(&dir.join("app.vey"));
    assert!(!output.status.success());
    let error = common::stderr(&output);
    assert!(error.contains("expected at least 2 arguments"), "{error}");
}

#[test]
fn a_value_of_the_language_has_no_fields() {
    let error = check_err("field-of-a-number", "let n: number = 5\nprint(n.field)\n");
    assert!(
        error.contains("type number has no field 'field'"),
        "{error}"
    );

    let error = check_err("field-of-a-list", "let l: [number] = [1]\nprint(l.field)\n");
    assert!(error.contains("has no field 'field'"), "{error}");
}

#[test]
fn a_value_of_the_language_cannot_be_called() {
    let error = check_err("call-a-number", "let n: number = 5\nprint(n())\n");
    assert!(
        error.contains("value of type number is not callable"),
        "{error}"
    );
}

#[test]
fn only_a_list_can_be_walked_through() {
    let error = check_err(
        "walk-a-number",
        "fn f(x: number) {\n  for i in x { print(i) }\n}\n",
    );
    assert!(error.contains("cannot walk through a number"), "{error}");
}

#[test]
fn a_rest_parameter_is_a_list_inside_the_function() {
    check_ok(
        "rest-inside",
        r#"
fn sum(...values: number) -> number {
  let total: number = 0
  for value in values {
    total += value
  }
  return total
}
print(sum(1, 2, 3))
"#,
    );
}

#[test]
fn a_closure_has_to_fit_the_type_it_is_given() {
    check_ok(
        "closure-fits",
        "let f: (number) -> number = fn(a: number) { return a }\n",
    );

    let error = check_err(
        "closure-result",
        "let f: () -> number = fn() { return \"x\" }\n",
    );
    assert!(
        error.contains("expected () -> number, found () -> string"),
        "{error}"
    );

    let error = check_err(
        "closure-parameter",
        "let f: (number) -> number = fn(a: string) { return 1 }\n",
    );
    assert!(error.contains("expected (number) -> number"), "{error}");
}

#[test]
fn a_condition_has_to_be_a_bool() {
    let error = check_err("condition-number", "if 5 { print(1) }\n");
    assert!(error.contains("the condition must be a bool"), "{error}");

    let error = check_err(
        "condition-text",
        "let s: string = \"a\"\nif s { print(1) }\n",
    );
    assert!(error.contains("the condition must be a bool"), "{error}");

    check_ok(
        "condition-bool",
        "let b: bool = true\nif b { print(1) }\nlet n = 0\nwhile n < 1 { n += 1 }\n",
    );
}

#[test]
fn two_values_of_different_types_are_never_the_same() {
    let error = check_err(
        "compare-different",
        "let n: number = 1\nif n == \"a\" { print(1) }\n",
    );
    assert!(
        error.contains("number and string are never the same"),
        "{error}"
    );

    check_ok(
        "compare-same",
        "let n: number = 1\nlet s: string = \"a\"\nprint(n == 1)\nprint(s == \"a\")\n",
    );
}

#[test]
fn a_comparison_of_numbers_needs_numbers() {
    let error = check_err("order-different", "let n: number = 1\nprint(n < \"a\")\n");
    assert!(error.contains("requires numbers"), "{error}");
}

#[test]
fn a_helper_of_the_runtime_is_not_a_field() {
    let error = check_err(
        "string-field",
        "let s: string = \"text\"\nprint(s.upper())\n",
    );
    assert!(
        error.contains("type string has no field 'upper'"),
        "{error}"
    );
    assert!(error.contains("use upper(...) instead"), "{error}");
}

#[test]
fn a_value_that_may_be_nothing_can_be_compared() {
    check_ok(
        "compare-maybe-nothing",
        "let x: number | null = 5\nif x > 10 { print(\"big\") }\nif x == null { print(\"none\") }\n",
    );

    let error = check_err(
        "compare-text",
        "let x: number | string = 5\nif x > 10 { print(\"big\") }\n",
    );
    assert!(error.contains("requires numbers"), "{error}");
}

#[test]
fn a_component_has_no_argument_that_is_not_its_own() {
    let error = check_err(
        "unknown-argument",
        "component Card(title) {\n  section { h2 { title } }\n}\ncomponent App() {\n  Card titel=\"x\"\n}\n",
    );
    assert!(error.contains("has no argument 'titel'"), "{error}");
}
