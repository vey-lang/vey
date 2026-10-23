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
fn a_check_narrows_a_member_of_a_value() {
    check_ok(
        "narrow-member",
        r#"
type User = { name: string | null }
fn takes(s: string) -> string { return s }
fn show(user: User) {
  if user.name != null {
    print(takes(user.name))
  }
  if user.name {
    print(takes(user.name))
  }
  if user.name == null {
    return
  }
  print(takes(user.name))
}
"#,
    );
    let error = check_err(
        "narrow-member-off",
        r#"
type User = { name: string | null }
fn takes(s: string) -> string { return s }
fn show(user: User) {
  print(takes(user.name))
}
"#,
    );
    assert!(error.contains("string | null"), "{error}");
}

#[test]
fn a_narrowing_of_a_member_reaches_inside_a_closure() {
    check_ok(
        "narrow-member-closure",
        r#"
type User = { name: string | null }
fn takes(s: string) -> string { return s }
fn show(user: User) {
  if user.name != null {
    let read = fn() { return takes(user.name) }
    print(read())
  }
}
"#,
    );
}

#[test]
fn a_write_takes_away_the_narrowing_of_a_member() {
    let error = check_err(
        "narrow-member-written",
        r#"
type User = { name: string | null }
fn takes(s: string) -> string { return s }
fn show(user: User) {
  if user.name != null {
    user.name = null
    print(takes(user.name))
  }
}
"#,
    );
    assert!(error.contains("string | null"), "{error}");
    // A write to the value itself takes the narrowing of a member away too.
    let error = check_err(
        "narrow-member-root-written",
        r#"
type User = { name: string | null }
fn takes(s: string) -> string { return s }
fn show(user: User, other: User) {
  if user.name != null {
    user = other
    print(takes(user.name))
  }
}
"#,
    );
    assert!(error.contains("string | null"), "{error}");
}

#[test]
fn a_chain_of_members_is_narrowed_by_its_path() {
    check_ok(
        "narrow-member-chain",
        r#"
type Address = { city: string | null }
type User = { address: Address }
fn takes(s: string) -> string { return s }
fn show(user: User) {
  if user.address.city != null {
    print(takes(user.address.city))
  }
}
"#,
    );
}

#[test]
fn a_wrong_value_inside_a_recursive_type_is_found() {
    let error = check_err(
        "recursive-wrong",
        r#"
type Tree = { value: number, children: [Tree] }
let bad: Tree = { value: 1, children: [{ value: "no", children: [] }] }
"#,
    );
    assert!(error.contains("expected Tree"), "{error}");

    // The value is wrong deep inside, and the type is opened as far as the
    // check goes.
    let error = check_err(
        "recursive-wrong-deep",
        r#"
type Tree = { value: number, children: [Tree] }
let bad: Tree = { value: 1, children: [{ value: 2, children: [{ value: 3, children: [{ value: 4, children: [{ value: 5, children: [{ value: 6, children: [{ value: "no", children: [] }] }] }] }] }] }] }
"#,
    );
    assert!(error.contains("expected Tree"), "{error}");
}

#[test]
fn a_recursive_type_is_opened_where_the_check_needs_it() {
    // A type with eight fields that refer to itself used to be opened six
    // times over, which took seconds. It is now opened where the check
    // reaches it.
    let start = std::time::Instant::now();
    check_ok(
        "recursive-wide",
        r#"
type Wide = { a: Wide | null, b: Wide | null, c: Wide | null, d: Wide | null, e: Wide | null, f: Wide | null, g: Wide | null, h: Wide | null }
fn take(w: Wide) -> number { return 0 }
fn make() -> Wide {
  let w: Wide = { a: null, b: null, c: null, d: null, e: null, f: null, g: null, h: null }
  return w
}
print(take(make()))
"#,
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 2,
        "the check of a recursive type took {:?}",
        elapsed
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
fn a_return_in_finally_satisfies_the_function_return_type() {
    check_ok(
        "finally-return",
        r#"
fn f() -> number {
  try {
    print("body")
  } finally {
    return 7
  }
}
let answer: number = f()
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
fn an_earlier_argument_contextualizes_a_later_generic_callback() {
    check_ok(
        "generic-callback-inference",
        r#"
fn choose<T, U>(x: T, f: (T) -> U) -> U { return f(x) }
let n: number = choose(1, fn(x) { return x + 1 })
"#,
    );
}

#[test]
fn a_generic_interface_can_be_used_from_another_module() {
    let dir = common::temp_dir("generic-interface-module");
    std::fs::write(
        dir.join("types.vey"),
        "export interface Box<T: number> { value: T }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.vey"),
        "use \"./types.vey\" as t\nlet b: t.Box<number> = { value: 2 }\n",
    )
    .unwrap();
    let valid = common::check_file(&dir.join("app.vey"));
    assert!(valid.status.success(), "{}", common::stderr(&valid));

    std::fs::write(
        dir.join("app.vey"),
        "use \"./types.vey\" as t\nlet b: t.Box<number> = { value: \"wrong\" }\n",
    )
    .unwrap();
    let invalid = common::check_file(&dir.join("app.vey"));
    assert!(!invalid.status.success(), "mismatched interface field was accepted");

    std::fs::write(
        dir.join("app.vey"),
        "use \"./types.vey\" as t\nlet b: t.Box<string> = { value: \"wrong\" }\n",
    )
    .unwrap();
    let invalid_argument = common::check_file(&dir.join("app.vey"));
    assert!(!invalid_argument.status.success(), "the interface constraint was ignored");
    assert!(common::stderr(&invalid_argument).contains("expects number"));
}

#[test]
fn an_imported_recursive_alias_checks_fields_after_recursive_edges() {
    let dir = common::temp_dir("recursive-imported-alias");
    std::fs::write(
        dir.join("types.vey"),
        "export type Tree = { value: number, children: [Tree] }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("valid.vey"),
        "use \"./types.vey\" as t\nlet good: t.Tree = { value: 1, children: [{ value: 2, children: [] }] }\n",
    )
    .unwrap();
    let valid = common::check_file(&dir.join("valid.vey"));
    assert!(valid.status.success(), "{}", common::stderr(&valid));

    std::fs::write(
        dir.join("app.vey"),
        r#"
use "./types.vey" as t
let bad: t.Tree = { value: 1, children: [{ value: 2, children: [{ value: 3, children: [{ value: "wrong", children: [] }] }] }] }
"#,
    )
    .unwrap();
    let invalid = common::check_file(&dir.join("app.vey"));
    assert!(!invalid.status.success(), "invalid nested value was accepted");
    assert!(common::stderr(&invalid).contains("expected t.Tree"));
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
fn a_bounded_generic_member_uses_the_bound_type() {
    check_ok(
        "bounded-member-object",
        "fn get<T: { x: number }>(v: T) -> number { return v.x }\nlet n: number = get({ x: 1 })\n",
    );
    check_ok(
        "bounded-member-interface",
        "interface HasX { x: number }\nfn get<T: HasX>(v: T) -> number { return v.x }\nlet n: number = get({ x: 1 })\n",
    );
    check_ok(
        "bounded-member-nested",
        "fn get<T: { child: { n: number } }>(v: T) -> number { return v.child.n }\nlet n: number = get({ child: { n: 1 } })\n",
    );
    check_ok(
        "bounded-member-chain",
        "fn get<U: { x: number }, T: U>(v: T) -> number { return v.x }\nlet n: number = get({ x: 1 })\n",
    );
    check_ok(
        "bounded-member-chain-explicit",
        "interface HasX { x: number }\nfn get<U: HasX, T: U>(v: T) -> number { return v.x }\nlet n: number = get<HasX, HasX>({ x: 1 })\n",
    );

    let error = check_err(
        "bounded-member-wrong-return",
        "fn get<T: { x: number }>(v: T) -> string { return v.x }\nlet s: string = get({ x: 1 })\n",
    );
    assert!(error.contains("expected string, found number"), "{error}");

    let chain_error = check_err(
        "bounded-member-chain-wrong-argument",
        "fn get<U: { x: number }, T: U>(v: T) -> number { return v.x }\nlet n: number = get({ x: \"bad\" })\n",
    );
    assert!(
        chain_error.contains("type argument 'T' expects"),
        "{chain_error}"
    );

    let wrong_context = check_err(
        "bounded-member-chain-incompatible-return-context",
        "fn get<U: { x: number }, T: U>(v: T) -> U { return v }\nlet n: number = get({ x: 1 })\n",
    );
    assert!(
        wrong_context.contains("expected number, found { x: number }"),
        "{wrong_context}"
    );
}

#[test]
fn an_unbounded_or_missing_generic_member_is_rejected() {
    let missing = check_err(
        "bounded-member-missing",
        "fn get<T: { x: number }>(v: T) -> number { return v.nope }\nlet n: number = get({ x: 1 })\n",
    );
    assert!(missing.contains("has no field 'nope'"), "{missing}");

    let unbounded = check_err(
        "unbounded-member",
        "fn get<T>(v: T) -> number { return v.x }\nlet n: number = get({ x: 1 })\n",
    );
    assert!(unbounded.contains("has no field 'x'"), "{unbounded}");

    let recursive = check_err(
        "recursive-member-bound",
        "fn get<T: U, U: T>(v: T) -> number { return v.x }\n",
    );
    assert!(recursive.contains("has no field 'x'"), "{recursive}");
}

#[test]
fn a_bounded_generic_array_index_keeps_its_element_type() {
    check_ok(
        "bounded-array-index",
        "fn get<T: [number]>(v: T) -> number { return v[0] }\nlet n: number = get([1])\n",
    );
    check_ok(
        "bounded-nested-array-index",
        "fn get<T: { items: [number] }>(v: T) -> number { return v.items[0] }\n",
    );
    check_ok(
        "bounded-union-array-index",
        "fn get<T: [number] | [string]>(v: T) -> number | string { return v[0] }\n",
    );

    let wrong_array = check_err(
        "bounded-array-index-wrong-return",
        "fn get<T: [number]>(v: T) -> string { return v[0] }\n",
    );
    assert!(
        wrong_array.contains("expected string, found number"),
        "{wrong_array}"
    );

    let wrong_union = check_err(
        "bounded-union-array-index-wrong-return",
        "fn get<T: [number] | [string]>(v: T) -> bool { return v[0] }\n",
    );
    assert!(wrong_union.contains("expected bool"), "{wrong_union}");

    let recursive = check_err(
        "recursive-index-bound",
        "fn get<T: U, U: T>(v: T) -> number { return v[0] }\n",
    );
    assert!(recursive.contains("cannot be indexed"), "{recursive}");
}

#[test]
fn a_bounded_generic_dictionary_index_checks_key_and_value_types() {
    check_ok(
        "bounded-dictionary-index",
        "fn get<T: {[string]: number}>(v: T) -> number { return v[\"key\"] }\n",
    );

    let wrong_value = check_err(
        "bounded-dictionary-index-wrong-return",
        "fn get<T: {[string]: number}>(v: T) -> string { return v[\"key\"] }\n",
    );
    assert!(
        wrong_value.contains("expected string, found number"),
        "{wrong_value}"
    );

    let wrong_key = check_err(
        "bounded-dictionary-index-wrong-key",
        "fn get<T: {[string]: number}>(v: T) -> number { return v[1] }\n",
    );
    assert!(
        wrong_key.contains("expected string, found number"),
        "{wrong_key}"
    );

    let unbounded = check_err(
        "unbounded-generic-index",
        "fn get<T>(v: T) -> number { return v[0] }\n",
    );
    assert!(unbounded.contains("no indexable constraint"), "{unbounded}");
}

#[test]
fn a_generic_alias_accepts_a_type_parameter_with_a_matching_bound() {
    check_ok(
        "bounded-generic-alias-argument",
        "type Box<T: number> = [T]\nfn first<U: number>(v: Box<U>) -> U { return v[0] }\nlet n: number = first([1])\n",
    );

    let error = check_err(
        "bounded-generic-alias-incompatible-argument",
        "type Box<T: number> = [T]\nfn first<U: string>(v: Box<U>) -> U { return v[0] }\n",
    );
    assert!(error.contains("type 'Box' expects number"), "{error}");
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
fn a_generic_parameter_is_not_a_concrete_return_type() {
    check_ok(
        "generic-identity-return",
        "fn id<T>(value: T) -> T { return value }\nlet n: number = id(1)\n",
    );
    check_ok(
        "bounded-generic-return",
        "fn as_number<T: number>(value: T) -> number { return value }\nprint(as_number(1))\n",
    );
    check_ok(
        "bounded-generic-operation",
        "fn add<T: number>(value: T) -> number { return value + 1 }\nprint(add(1))\n",
    );
    check_ok(
        "bounded-generic-list",
        "fn numbers<T: number>(values: [T]) -> [number] { return values }\nlet values: [number] = numbers([1])\n",
    );
    let error = check_err(
        "unbounded-generic-operation",
        "fn add<T>(value: T) -> number { return value + 1 }\nprint(add(1))\n",
    );
    assert!(error.contains("operator '+'"), "{error}");
    let error = check_err(
        "generic-concrete-return",
        "fn lie<T>(value: T) -> number { return value }\nlet n: number = lie(\"not a number\")\n",
    );
    assert!(error.contains("expected number"), "{error}");
}

#[test]
fn mutable_arrays_and_tuples_do_not_allow_covariant_aliases() {
    let error = check_err(
        "mutable-array-covariance",
        r#"
type Animal = { name: string }
type Dog = { name: string, bark: number }
let dogs: [Dog] = [{ name: "Fido", bark: 1 }]
let animals: [Animal] = dogs
animals[0] = { name: "Felix" }
let bark: number = dogs[0].bark
"#,
    );
    assert!(error.contains("expected [Animal]"), "{error}");

    let error = check_err(
        "mutable-tuple-to-array-covariance",
        r#"
type Animal = { name: string }
type Dog = { name: string, bark: number }
let dogs: [Dog, Dog] = [{ name: "Fido", bark: 1 }, { name: "Rex", bark: 2 }]
let animals: [Animal] = dogs
animals[0] = { name: "Felix" }
let bark: number = dogs[0].bark
"#,
    );
    assert!(error.contains("expected [Animal]"), "{error}");

    let error = check_err(
        "mutable-array-to-tuple-covariance",
        r#"
type Animal = { name: string }
type Dog = { name: string, bark: number }
let dogs: [Dog] = [{ name: "Fido", bark: 1 }]
let animals: [Animal, Animal] = dogs
animals[0] = { name: "Felix" }
let bark: number = dogs[0].bark
"#,
    );
    assert!(error.contains("expected [Animal, Animal]"), "{error}");

    let error = check_err(
        "mutable-tuple-covariance",
        r#"
type Animal = { name: string }
type Dog = { name: string, bark: number }
let dogs: [Dog, Dog] = [{ name: "Fido", bark: 1 }, { name: "Rex", bark: 2 }]
let animals: [Animal, Animal] = dogs
animals[0] = { name: "Felix" }
let bark: number = dogs[0].bark
"#,
    );
    assert!(error.contains("expected [Animal, Animal]"), "{error}");

    check_ok(
        "mutable-array-same-type-alias",
        "let values: [number] = [1]\nlet alias: [number] = values\nalias[0] = 2\n",
    );
    check_ok(
        "mutable-tuple-to-array-same-type-alias",
        "let pair: [number, number] = [1, 2]\nlet values: [number] = pair\nvalues[0] = 3\n",
    );
}

#[test]
fn mutable_objects_and_dictionaries_do_not_allow_covariant_aliases() {
    let error = check_err(
        "mutable-object-field-covariance",
        r#"
let narrow: { x: number } = { x: 1 }
let wide: { x: number | string } = narrow
wide.x = "oops"
let n: number = narrow.x
"#,
    );
    assert!(error.contains("number | string"), "{error}");

    let error = check_err(
        "mutable-named-field-covariance",
        r#"
type Narrow = { x: number }
type Wide = { x: number | string }
let narrow: Narrow = { x: 1 }
let wide: Wide = narrow
wide.x = "bad"
let n: number = narrow.x
"#,
    );
    assert!(error.contains("Wide"), "{error}");

    let error = check_err(
        "mutable-interface-field-covariance",
        r#"
interface Narrow { x: number }
interface Wide { x: number | string }
let narrow: Narrow = { x: 1 }
let wide: Wide = narrow
wide.x = "bad"
let n: number = narrow.x
"#,
    );
    assert!(error.contains("Wide"), "{error}");

    let error = check_err(
        "mutable-dictionary-value-covariance",
        r#"
let narrow: {[string]: number} = { x: 1 }
let wide: {[string]: number | string} = narrow
let key: string = "x"
wide[key] = "bad"
let n: number = narrow[key]
"#,
    );
    assert!(error.contains("number | string"), "{error}");

    let error = check_err(
        "mutable-optional-field-covariance",
        r#"
let narrow: { x: number } = { x: 1 }
let wide: { x?: number } = narrow
wide.x = null
let n: number = narrow.x
"#,
    );
    assert!(error.contains("x?: number"), "{error}");

    check_ok(
        "mutable-object-same-field-alias",
        "let first: { x: number } = { x: 1 }\nlet second: { x: number } = first\nsecond.x = 2\nlet n: number = first.x\n",
    );
    check_ok(
        "mutable-dictionary-same-value-alias",
        "let first: {[string]: number} = { x: 1 }\nlet second: {[string]: number} = first\nsecond[\"x\"] = 2\nlet n: number = first[\"x\"]\n",
    );
}

#[test]
fn fresh_literals_take_contextual_types_without_widening_existing_aliases() {
    check_ok(
        "fresh-object-literal-field-widening",
        "let value: { x: number | string } = { x: 1 }\nvalue.x = \"ok\"\n",
    );
    check_ok(
        "fresh-array-literal-element-widening",
        "let values: [number | string] = [1]\nvalues[0] = \"ok\"\n",
    );
    check_ok(
        "fresh-tuple-literal-element-widening",
        "let pair: [number | string, number | string] = [1, \"x\"]\npair[0] = \"ok\"\n",
    );
    check_ok(
        "fresh-nested-object-literal-widening",
        "let value: { child: { x: number | string } } = { child: { x: 1 } }\nvalue.child.x = \"ok\"\n",
    );

    let error = check_err(
        "fresh-wrapper-does-not-widen-aliased-object",
        r#"
let child: { x: number } = { x: 1 }
let parent: { child: { x: number | string } } = { child: child }
parent.child.x = "oops"
let n: number = child.x
"#,
    );
    assert!(error.contains("expected"), "{error}");

    let error = check_err(
        "fresh-wrapper-does-not-widen-aliased-array",
        "let child: [number] = [1]\nlet parent: [[number | string]] = [child]\nparent[0][0] = \"oops\"\nlet n: number = child[0]\n",
    );
    assert!(error.contains("expected"), "{error}");
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
fn a_literal_computed_call_to_another_module_is_checked() {
    let dir = common::temp_dir("module-computed-member");
    std::fs::write(
        dir.join("lib.vey"),
        "export fn add(a: number, b: number) -> number {\n  return a + b\n}\n",
    )
    .unwrap();
    let app = dir.join("app.vey");
    std::fs::write(
        &app,
        "use \"./lib.vey\" as lib\nlet n: number = lib[\"add\"](1, 2)\n",
    )
    .unwrap();
    let output = common::check_file(&app);
    assert!(output.status.success(), "{}", common::stderr(&output));

    std::fs::write(&app, "use \"./lib.vey\" as lib\nlib[\"add\"](\"one\", 2)\n").unwrap();
    let output = common::check_file(&app);
    assert!(!output.status.success());
    let error = common::stderr(&output);
    assert!(error.contains("argument 1 expects number"), "{error}");
}

#[test]
fn a_missing_literal_computed_export_is_rejected() {
    let dir = common::temp_dir("module-computed-missing");
    std::fs::write(dir.join("lib.vey"), "export let count: number = 1\n").unwrap();
    std::fs::write(
        dir.join("app.vey"),
        "use \"./lib.vey\" as lib\nlet value = lib[\"missing\"]\n",
    )
    .unwrap();
    let output = common::check_file(&dir.join("app.vey"));
    assert!(!output.status.success());
    let error = common::stderr(&output);
    assert!(error.contains("module 'lib' has no 'missing'"), "{error}");
}

#[test]
fn a_local_import_shadow_only_hides_the_module_inside_its_scope() {
    let dir = common::temp_dir("module-shadow-scope");
    std::fs::write(
        dir.join("lib.vey"),
        "export fn add(a: number) -> number { return a }\n",
    )
    .unwrap();
    let app = dir.join("app.vey");
    for access in ["lib.add(\"wrong\")", "lib[\"add\"](\"wrong\")"] {
        std::fs::write(
            &app,
            format!(
                "use \"./lib.vey\" as lib\nif true {{\n  let lib = {{ add: \"not a function\" }}\n  let local: string = lib.add\n  let computed: string = lib[\"add\"]\n}}\n{}\n",
                access
            ),
        )
        .unwrap();
        let output = common::check_file(&app);
        assert!(
            !output.status.success(),
            "{access} bypassed the module signature"
        );
        let error = common::stderr(&output);
        assert!(error.contains("argument 1 expects number"), "{error}");
    }
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

#[test]
fn a_cast_is_only_possible_between_types_that_are_near() {
    let error = check_err(
        "cast-impossible",
        "let value: number = 1\nlet text = value as string\n",
    );
    assert!(error.contains("expected string, found number"), "{error}");
}

#[test]
fn the_type_of_an_arrow_is_checked() {
    let error = check_err(
        "arrow-type",
        "let wrong = (n: number): string => n * 2\n",
    );
    assert!(error.contains("expected string, found number"), "{error}");
}

#[test]
fn a_match_of_a_union_needs_a_name_or_a_wildcard() {
    let error = check_err(
        "union-not-exhaustive",
        "fn f(value: number | string) -> string {\n  return match value {\n    1 => \"one\"\n  }\n}\n",
    );
    assert!(error.contains("not exhaustive"), "{error}");
}

#[test]
fn an_arrow_may_name_its_type_parameters() {
    check_ok(
        "generic-arrow",
        "let identity = <T>(value: T) => value\nlet n: number = identity(42)\nlet s: string = identity(\"text\")\n",
    );
}

#[test]
fn a_function_that_waits_gives_back_a_promise() {
    // The type of the value is the type that is written, and the function
    // gives back a promise of it.
    check_ok(
        "async-return",
        r#"
fn load() -> number {
  await sleep(10)
  return 1
}
let n: number = await load()
"#,
    );
    // The promise may also be written by hand.
    check_ok(
        "async-return-written",
        r#"
fn load() -> Promise<number> {
  await sleep(10)
  return 1
}
let n: number = await load()
"#,
    );
}

#[test]
fn the_value_of_an_await_is_the_value_that_will_be_there() {
    let error = check_err(
        "async-value",
        r#"
fn load() -> number {
  await sleep(10)
  return 1
}
let n: string = await load()
"#,
    );
    assert!(error.contains("expected string, found number"), "{error}");
}

#[test]
fn awaiting_a_non_promise_keeps_its_type() {
    check_ok(
        "await-non-promise-valid",
        "fn load() -> string { return await \"ready\" }\nfn main() { let value: string = await load() }\n",
    );
    let error = check_err(
        "await-non-promise-invalid",
        "fn load() -> number { return await \"not a number\" }\n",
    );
    assert!(error.contains("expected number, found string"), "{error}");
}

#[test]
fn the_helpers_that_wait_give_back_a_promise() {
    let error = check_err(
        "async-sleep",
        r#"
fn main() {
  let n: number = await sleep(10)
  print(n)
}
"#,
    );
    assert!(error.contains("expected number, found void"), "{error}");

    check_ok(
        "async-http",
        r#"
fn main() {
  let answer = await http.get("https://example.com")
  let status: number = answer.status
  print(status)
}
"#,
    );
}

#[test]
fn an_arrow_that_waits_gives_back_a_promise() {
    check_ok(
        "async-arrow",
        r#"
fn main() {
  let load = () => await sleep(10)
  let n: void = await load()
  print(n)
}
"#,
    );
}

#[test]
fn a_union_of_the_variants_of_an_enum_may_be_taken_apart() {
    check_ok(
        "union-exhaustive",
        r#"
enum Color {
  Red
  Green
  Blue
}

fn name(c: Color | null) -> string {
  return match c {
    Color.Red => "red"
    Color.Green => "green"
    Color.Blue => "blue"
    null => "none"
  }
}
"#,
    );
    // A variant that is not there is named.
    let error = check_err(
        "union-missing",
        r#"
enum Color {
  Red
  Green
  Blue
}

fn name(c: Color | null) -> string {
  return match c {
    Color.Red => "red"
    Color.Green => "green"
    null => "none"
  }
}
"#,
    );
    assert!(error.contains("missing: Color.Blue"), "{error}");
}

#[test]
fn a_type_argument_that_is_given_by_hand_is_checked() {
    // A type parameter that is settled by the value that is returned is
    // not generic any more, so a type argument that does not fit is a
    // mistake and not something that is ignored.
    let error = check_err(
        "settled-type-argument",
        r#"
fn parse<T>(text: string) -> T { return num(text) }
let n: string = parse<string>("42")
"#,
    );
    assert!(error.contains("type argument 'T' expects number, found string"), "{error}");
}

#[test]
fn a_callback_knows_the_type_of_what_it_receives() {
    check_ok(
        "contextual-callback",
        r#"
fn each(items: [string], f: (string) -> void) {
  for item in items { f(item) }
}
fn main() {
  each(["a", "b"], fn(item) {
    let n: number = item.length
    print(n)
  })
}
"#,
    );
    let error = check_err(
        "contextual-callback-wrong",
        r#"
fn each(items: [string], f: (string) -> void) {
  for item in items { f(item) }
}
fn main() {
  each(["a"], fn(item) {
    let n: number = item
    print(n)
  })
}
"#,
    );
    assert!(error.contains("expected number, found string"), "{error}");
}

#[test]
fn a_dictionary_gives_the_type_of_its_values() {
    check_ok(
        "dict-member",
        r#"
fn f(d: {[string]: number}) -> number {
  return d["a"] + d.b
}
"#,
    );
    // The key of a dictionary has to be of the type that it says.
    let error = check_err(
        "dict-key",
        r#"
fn f(d: {[string]: number}) -> number {
  return d[true]
}
"#,
    );
    assert!(error.contains("expected string, found bool"), "{error}");
}

#[test]
fn a_value_of_an_unknown_type_is_narrowed_by_is() {
    check_ok(
        "is-unknown",
        r#"
fn show(value: unknown) -> string {
  if value is string { return value }
  if value is symbol { return "a symbol" }
  if value is bigint { return "a bigint" }
  if value is void { return "nothing" }
  return "another one"
}
"#,
    );
    // A kind that the value cannot be is a mistake, not a silent no.
    let error = check_err(
        "is-impossible",
        r#"
fn show(text: string) -> bool {
  return text is number
}
"#,
    );
    assert!(error.contains("expected number, found string"), "{error}");
}

#[test]
fn an_unknown_value_needs_object_narrowing_before_use_as_object() {
    check_ok(
        "unknown-object-narrowed",
        "fn needs_object(value: object) {}\nfn use_value(value: unknown) { if value is object { needs_object(value) } }\n",
    );
    let error = check_err(
        "unknown-object-unchecked",
        "fn needs_object(value: object) {}\nfn use_value(value: unknown) { needs_object(value) }\n",
    );
    assert!(error.contains("expects object, found unknown"), "{error}");
}

#[test]
fn a_constant_of_a_value_that_may_be_nothing_keeps_that() {
    check_ok(
        "const-union",
        r#"
type User = { name: string }
fn find() -> User | null { return null }
const user = find()
if user != null { print(user.name) }
"#,
    );
    // The type is kept, so a member that the value does not have is a
    // mistake.
    let error = check_err(
        "const-union-member",
        r#"
type User = { name: string }
fn find() -> User | null { return null }
const user = find()
print(user.age)
"#,
    );
    assert!(error.contains("has no field 'age'"), "{error}");
}

#[test]
fn a_field_that_an_interface_does_not_have_is_named() {
    let error = check_err(
        "interface-extra-field",
        r#"
interface Point { x: number }
fn show(point: Point) -> number { return point.x }
show({ x: 1, y: 2 })
"#,
    );
    assert!(error.contains("'y' is not a field of Point"), "{error}");
}

#[test]
fn a_component_may_be_held_in_a_name() {
    check_ok(
        "component-alias",
        r#"
component Card(title) {
  div.card { title }
}

let Box = Card

component App() {
  let Also = Card
  div {
    Box("one")
    Also("two")
  }
}
"#,
    );
}

#[test]
fn the_question_mark_needs_a_result() {
    for source in [
        "fn f(value: number) -> number {\n  let n = value?\n  return n\n}\n",
        "fn f(value: string | null) -> string {\n  let text = value?\n  return text\n}\n",
        "fn f(value: unknown) -> number {\n  let n = value?\n  return 1\n}\n",
    ] {
        let error = check_err("question-mark-needs-result", source);
        assert!(error.contains("needs a Result"), "{error}");
    }

    check_ok(
        "question-mark-result-union",
        "fn a() -> Result<number, string> { return ok(1) }\n\
         fn b(flag: bool) -> Result<number, string> {\n\
           let value = match flag {\n\
             true => a()\n\
             false => ok(2)\n\
           }\n\
           return ok(value? + 1)\n\
         }\n",
    );
}

#[test]
fn a_signal_is_typed_by_the_value_it_is_given() {    check_ok(
        "signal-typed",
        r#"
const count = signal(0)
count.set(1)
let shown: number = count.value
let back: number = count.update(fn(value) { return value + 1 })
let read: number = count.peek()
count.subscribe(fn(value) { print(value) })
"#,
    );
    let error = check_err(
        "signal-wrong-set",
        r#"
const count = signal(0)
count.set("text")
"#,
    );
    assert!(
        error.contains("argument 1 expects number, found string"),
        "{error}"
    );
}

#[test]
fn a_signal_may_be_given_its_type_argument() {
    check_ok(
        "signal-explicit",
        r#"
let count: Signal<number> = signal(0)
count.set(1)
"#,
    );
    let error = check_err(
        "signal-explicit-wrong",
        r#"
let count: Signal<number> = signal("text")
"#,
    );
    assert!(error.contains("Signal"), "{error}");
}

#[test]
fn a_signal_takes_the_way_two_values_are_compared() {
    check_ok(
        "signal-options",
        r#"
const count = signal(0, { equals: fn(a, b) { return a == b } })
const deep = signal({ a: 1 }, { equals: "deep" })
print(count.value, deep.value)
"#,
    );
}

#[test]
fn a_store_is_typed_by_the_state_it_is_given() {
    check_ok(
        "store-typed",
        r#"
const state = store({ a: 1 })
let a: number = state.value.a
state.set("a", 2)
state.replace({ a: 3 })
state.update(fn(now) { return { a: now.a + 1 } })
let read: number = state.peek().a
"#,
    );
    let error = check_err(
        "store-wrong-replace",
        r#"
const state = store({ a: 1 })
state.replace({ a: "text" })
"#,
    );
    assert!(
        error.contains("argument 1 expects { a: number }, found { a: string }"),
        "{error}"
    );
}

#[test]
fn a_store_may_be_asked_for_by_its_name_alone() {
    check_ok(
        "store-by-name",
        r#"
const shared = store("shared", { a: 1 })
let a: number = shared.value.a
const later = store("shared")
print(later.value)
"#,
    );
}

#[test]
fn a_computed_keeps_the_type_of_the_value_it_returns() {
    check_ok(
        "computed-typed",
        r#"
const count = signal(1)
const total = computed(fn() { return count.value + 1 })
let shown: number = total.value
total.dispose()
"#,
    );
    let error = check_err(
        "computed-wrong-value",
        r#"
const total = computed(fn() { return "text" })
let shown: number = total.value
"#,
    );
    assert!(error.contains("expected number, found string"), "{error}");
}
