//! Regression tests for bugs that used to produce wrong results silently.

mod common;

use common::{assert_prints, run_err, run_ok};

#[test]
fn components_receive_positional_arguments() {
    assert_prints(
        "component-args",
        r#"
component App(show) {
  if show { p { "yes" } } else { p { "no" } }
}

print(html(App(true)))
print(html(App(false)))
"#,
        "<p>yes</p>",
    );
    assert_prints(
        "component-args-false",
        r#"
component App(show) {
  if show { p { "yes" } } else { p { "no" } }
}

print(html(App(false)))
"#,
        "<p>no</p>",
    );
}

#[test]
fn components_keep_named_attributes_in_markup() {
    assert_prints(
        "component-named",
        r#"
component Card(title, children) {
  section.card { h2 { title } children }
}

component App() {
  Card title="Hi" { p { "body" } }
}

print(html(App()))
"#,
        r#"<section class="card"><h2>Hi</h2><p>body</p></section>"#,
    );
}

#[test]
fn components_accept_positional_arguments_in_markup() {
    assert_prints(
        "component-markup-args",
        r#"
component Item(text) { li { text } }

component List(items) {
  ul { for it in items { Item(it) } }
}

print(html(List(["a", "b"])))
"#,
        "<ul><li>a</li><li>b</li></ul>",
    );
}

#[test]
fn components_use_default_values() {
    assert_prints(
        "component-defaults",
        r#"
component C(a, b = 5) { p { "{a} {b}" } }

print(html(C(1)))
print(html(C(1, 2)))
"#,
        "<p>1 5</p>",
    );
}

#[test]
fn markup_loops_render_their_body() {
    assert_prints(
        "markup-loop",
        r#"
component L(items) { ul { for x in items { li { x } } } }
print(html(L([1, 2])))
"#,
        "<ul><li>1</li><li>2</li></ul>",
    );
}

#[test]
fn await_inside_match_compiles_to_valid_javascript() {
    let text = run_ok(
        "await-match",
        r#"
fn pick(x) {
  return match x {
    1 => await sleep(1),
    _ => 0
  }
}
print("ok")
"#,
    );
    assert!(text.contains("ok"), "{text}");
}

#[test]
fn async_is_reported_as_unsupported() {
    let error = run_err(
        "async-keyword",
        r#"
async fn work() { return 1 }
"#,
    );
    assert!(error.contains("'async' is not supported"), "{error}");
    assert!(error.contains("already async"), "{error}");
}

#[test]
fn strings_keep_json_objects_literal() {
    let text = run_ok("json-string", r#"print("{\"b\":2}")"#);
    assert!(text.contains(r#"{"b":2}"#), "{text}");
}

#[test]
fn empty_braces_stay_literal() {
    let text = run_ok("empty-braces", r#"print("{}")"#);
    assert!(text.contains("{}"), "{text}");
}

#[test]
fn an_unclosed_brace_stays_literal() {
    let text = run_ok("unclosed-brace", r#"print("{")"#);
    assert!(text.contains("{"), "{text}");
}

#[test]
fn interpolations_still_work() {
    let text = run_ok("interpolation", "let a = 5\nprint(\"v={a}\")");
    assert!(text.contains("v=5"), "{text}");
}

#[test]
fn a_broken_interpolation_is_reported() {
    let error = run_err("broken-interpolation", r#"print("{ 1 + }")"#);
    assert!(!error.is_empty(), "{error}");
}

#[test]
fn sort_orders_numbers_numerically() {
    let text = run_ok("sort-numbers", r#"print(sort([10, 9, 100, 2]))"#);
    assert!(text.contains("[ 2, 9, 10, 100 ]"), "{text}");
}

#[test]
fn sort_orders_strings_alphabetically() {
    let text = run_ok("sort-strings", r#"print(sort(["b", "a", "c"]))"#);
    assert!(text.contains("[ 'a', 'b', 'c' ]"), "{text}");
}

#[test]
fn json_stringify_always_returns_text() {
    let text = run_ok(
        "json-undefined",
        "let missing\nprint(json.stringify(missing))",
    );
    assert!(text.contains("undefined"), "{text}");
}

#[test]
fn url_parse_accepts_relative_urls() {
    let text = run_ok("url-relative", r#"print(url.parse("/a?b=1").pathname)"#);
    assert!(text.contains("/a"), "{text}");
}

#[test]
fn reduce_reports_an_empty_list() {
    let error = run_err("reduce-empty", r#"reduce([], fn(a, b) { return a + b })"#);
    assert!(error.contains("initial value"), "{error}");
}

#[test]
fn replace_without_a_value_removes_the_match() {
    let text = run_ok(
        "replace-empty",
        "let missing\nprint(replace(\"a-b\", \"-\", missing))",
    );
    assert!(text.contains("ab"), "{text}");
}

#[test]
fn random_without_arguments_returns_a_fraction() {
    let text = run_ok("random-float", r#"print(random() < 1)"#);
    assert!(text.contains("true"), "{text}");
}

#[test]
fn random_with_a_range_returns_an_integer() {
    let text = run_ok("random-range", r#"print(random(5, 5))"#);
    assert!(text.contains("5"), "{text}");
}

#[test]
fn flat_map_flattens_a_single_level() {
    let text = run_ok(
        "flatmap",
        r#"print(flatMap([1, 2], fn(x) { return [x, x * 10] }))"#,
    );
    assert!(text.contains("[ 1, 10, 2, 20 ]"), "{text}");
}

#[test]
fn enum_internals_stay_hidden() {
    let text = run_ok(
        "enum-keys",
        r#"
enum Color { Red, Green }
print(keys(Color.Red))
"#,
    );
    assert!(text.contains("[]"), "{text}");
}

#[test]
fn list_helpers_accept_text() {
    let text = run_ok("list-helpers", r#"print(map("ab", fn(c) { return c }))"#);
    assert!(text.contains("[ 'a', 'b' ]"), "{text}");
}

#[test]
fn str_handles_cycles() {
    let text = run_ok(
        "str-cycle",
        r#"
let a = {}
a.self = a
print(type(str(a)))
"#,
    );
    assert!(text.contains("string"), "{text}");
}

#[test]
fn named_arguments_are_only_supported_in_markup() {
    let error = run_err(
        "named-args",
        r#"
component C(title) { p { title } }
print(html(C(title="x")))
"#,
    );
    assert!(error.contains("named arguments"), "{error}");
}

#[test]
fn an_unknown_variable_is_reported_before_running() {
    let error = run_err("unknown-variable", "print(missing)\n");
    assert!(error.contains("unknown variable 'missing'"), "{error}");
}

#[test]
fn an_interpolated_name_that_is_unknown_suggests_a_literal_string() {
    let error = run_err("unknown-interpolation", "print(\"{missing}\")\n");
    assert!(error.contains("unknown variable 'missing'"), "{error}");
    assert!(
        error.contains("single quoted strings are literal"),
        "{error}"
    );
}

#[test]
fn a_name_that_is_known_later_is_accepted() {
    let text = run_ok(
        "forward-reference",
        "fn f() { return g() }\nfn g() { return 1 }\nprint(f())\n",
    );
    assert!(text.contains("1"), "{text}");
}

#[test]
fn an_unknown_variable_suggests_a_similar_name() {
    let error = run_err("suggest-variable", "let name = 1\nprint(nam)\n");
    assert!(error.contains("unknown variable 'nam'"), "{error}");
    assert!(error.contains("did you mean 'name'?"), "{error}");
}

#[test]
fn an_unknown_type_suggests_a_similar_name() {
    let error = run_err(
        "suggest-type",
        "type User = { name: string }\nlet user: Users = { name: \"x\" }\n",
    );
    assert!(error.contains("unknown type 'Users'"), "{error}");
    assert!(error.contains("did you mean 'User'?"), "{error}");
}

#[test]
fn an_unknown_name_without_a_similar_one_has_no_hint() {
    let error = run_err("no-suggestion", "print(zzzzzzzzzzzz)\n");
    assert!(error.contains("unknown variable"), "{error}");
    assert!(!error.contains("did you mean"), "{error}");
}

#[test]
fn a_value_may_not_be_used_before_it_is_declared() {
    let error = run_err("used-before-declared", "print(value)\nlet value = 1\n");
    assert!(error.contains("used before it is declared"), "{error}");
}

#[test]
fn a_function_may_use_a_value_that_is_declared_later() {
    let text = run_ok(
        "forward-in-function",
        "fn increment() {\n  count = count + 1\n}\n\nlet count = 0\nincrement()\nprint(count)\n",
    );
    assert!(text.contains("1"), "{text}");
}

#[test]
fn a_parameter_may_take_the_argument_apart() {
    let text = run_ok(
        "destructured-parameter",
        "fn add({ a, b }) { return a + b }\nprint(add({ a: 1, b: 2 }))\n",
    );
    assert!(text.contains("3"), "{text}");
}

#[test]
fn a_parameter_may_take_a_list_apart() {
    let text = run_ok(
        "destructured-list-parameter",
        "fn add([a, b]) { return a + b }\nprint(add([1, 2]))\n",
    );
    assert!(text.contains("3"), "{text}");
}

#[test]
fn an_arrow_may_take_the_argument_apart() {
    let text = run_ok(
        "destructured-arrow",
        "let first = ({ name }) => name\nprint(first({ name: \"Ada\" }))\n",
    );
    assert!(text.contains("Ada"), "{text}");
}

#[test]
fn a_component_may_take_its_props_apart() {
    let text = run_ok(
        "destructured-component",
        r#"
component Card({ title, subtitle }) {
  section { h2 { title } p { subtitle } }
}
component App() {
  Card title="T" subtitle="S"
}
print(html(App()))
"#,
    );
    assert!(
        text.contains("<section><h2>T</h2><p>S</p></section>"),
        "{text}"
    );
}
