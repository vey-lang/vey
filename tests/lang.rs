use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "vey-test-{}-{}-{}-{}",
        label,
        std::process::id(),
        nanos,
        index
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_file(path: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("run")
        .arg(path)
        .output()
        .expect("failed to start vey")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn check_error(source: &str, label: &str, expected: &str) {
    let dir = temp_dir(label);
    let file = dir.join("main.vey");
    fs::write(&file, source).unwrap();

    let output = run_file(&file);
    assert!(!output.status.success(), "expected a failure");
    let text = stderr(&output);
    assert!(text.contains(expected), "{}", text);
}

#[test]
fn runs_core_language() {
    let dir = temp_dir("core");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
let numbers = range(1, 5)

fn double(n) {
  return n * 2
}

let doubled = numbers |> map(double)
print(doubled)

let label = match 4 {
  1 | 2 => "small"
  3..9 => "medium"
  _ => "large"
}
print(label)

let squares = numbers |> map(n => n ** 2)
print(squares)

let total = reduce(numbers, fn(a, b) { return a + b }, 0)
print("total {total}")

let person = { name: "Ada", age: 36 }
print(person.name)
print(person["age"])
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("[ 2, 4, 6, 8 ]"));
    assert!(text.contains("medium"));
    assert!(text.contains("[ 1, 4, 9, 16 ]"));
    assert!(text.contains("total 10"));
    assert!(text.contains("Ada"));
    assert!(text.contains("36"));
}

#[test]
fn imports_modules() {
    let dir = temp_dir("modules");
    fs::write(
        dir.join("math.vey"),
        r#"
export const PI = 3.14159

export fn area(radius) {
  return PI * radius ** 2
}
"#,
    )
    .unwrap();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r#"
use "./math.vey" as math
print("area {math.area(2)}")
print("pi {math.PI}")
"#,
    )
    .unwrap();

    let output = run_file(&app);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("area 12.56636"));
    assert!(text.contains("pi 3.14159"));
}

#[test]
fn finds_an_entry_in_a_subdirectory() {
    let dir = temp_dir("nested-entry");
    let project = dir.join("myapp");
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::write(
        project.join("main.vey"),
        r##"component App() {
  main { h1 { "Nested entry" } }
}

bind("#app", App())
"##,
    )
    .unwrap();
    fs::write(
        project.join("tests/main.vey"),
        "test(\"ignored by discovery\", fn() { assert(true) })\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    let html = fs::read_to_string(project.join("dist/index.html")).unwrap();
    assert!(html.contains("Nested entry"), "{}", html);
}

#[test]
fn resolves_modules_in_other_folders() {
    let dir = temp_dir("external-modules");
    let shared = dir.join("shared");
    let app = dir.join("app");
    fs::create_dir_all(&shared).unwrap();
    fs::create_dir_all(&app).unwrap();
    fs::write(
        shared.join("lib.vey"),
        r#"export fn greeting() -> string {
  return "From elsewhere"
}
"#,
    )
    .unwrap();

    let main = app.join("main.vey");
    fs::write(
        &main,
        r##"use "../shared/lib.vey" as lib

component App() {
  main { h1 { lib.greeting() } }
}

bind("#app", App())
"##,
    )
    .unwrap();

    let output_path = app.join("index.html");
    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("web")
        .arg(&main)
        .arg("-o")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    let html = fs::read_to_string(&output_path).unwrap();
    assert!(html.contains("From elsewhere"), "{}", html);
}

#[test]
fn renders_components_and_markup() {
    let dir = temp_dir("markup");
    let file = dir.join("view.vey");
    fs::write(
        &file,
        r#"
let count = 2

component Card(heading, children) {
  section.card {
    h2 { heading }
    children
  }
}

component App() {
  main.page {
    Card heading="Counter" {
      p { "body" }
      if count > 1 {
        span { "many" }
      }
    }
    ul {
      for item in ["a", "b"] {
        li { item }
      }
    }
  }
}

print(html(App()))
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains(
        "<section class=\"card\"><h2>Counter</h2><p>body</p><span>many</span></section>"
    ));
    assert!(text.contains("<ul><li>a</li><li>b</li></ul>"));
}

#[test]
fn escapes_text_and_allows_raw_html() {
    let dir = temp_dir("escape");
    let file = dir.join("view.vey");
    fs::write(
        &file,
        r##"
component X() {
  div {
    p { "<b>text</b>" }
    p { raw("<b>trusted</b>") }
  }
}

print(html(X()))
"##,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("&lt;b&gt;text&lt;/b&gt;"));
    assert!(text.contains("<b>trusted</b>"));
}

#[test]
fn reports_syntax_errors() {
    let dir = temp_dir("errors");
    let file = dir.join("bad.vey");
    fs::write(&file, "let x = \n").unwrap();

    let output = run_file(&file);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(text.contains("bad.vey"));
    assert!(text.contains("unexpected token"));
}

#[test]
fn reports_missing_modules() {
    let dir = temp_dir("missing");
    let file = dir.join("app.vey");
    fs::write(&file, "use \"./nope.vey\" as nope\n").unwrap();

    let output = run_file(&file);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("module not found"));
}

#[test]
fn maps_runtime_errors_to_source() {
    let dir = temp_dir("sourcemap");
    let file = dir.join("crash.vey");
    fs::write(
        &file,
        "let value = null\nprint(\"before\")\nprint(value.name)\n",
    )
    .unwrap();

    let output = run_file(&file);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(text.contains("crash.vey:3"), "{}", text);
}

#[test]
fn rejects_invalid_control_flow() {
    check_error("break\n", "break", "break outside a loop");
    check_error("continue\n", "continue", "continue outside a loop");
    check_error("return 1\n", "return", "return outside a function");
    check_error(
        "for n in [1] {\n  fn inner() {\n    break\n  }\n}\n",
        "nested-break",
        "break outside a loop",
    );
}

#[test]
fn rejects_const_and_unknown_assignments() {
    check_error(
        "const count = 1\ncount = 2\n",
        "const-assign",
        "cannot assign to const 'count'",
    );
    check_error(
        "missing = 1\n",
        "unknown-assign",
        "cannot assign to unknown variable 'missing'",
    );
    check_error(
        "fn work() {}\nwork = 1\n",
        "function-assign",
        "cannot assign to function 'work'",
    );
    check_error(
        "fn set() {\n  value = 1\n}\nconst value = 0\n",
        "forward-const",
        "cannot assign to const 'value'",
    );
}

#[test]
fn accepts_forward_references() {
    let dir = temp_dir("forward");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
fn increment() {
  count = count + 1
}

let count = 0
increment()
print(count)
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "1");
}

#[test]
fn rejects_duplicate_declarations() {
    check_error(
        "let value = 1\nlet value = 2\n",
        "duplicate",
        "duplicate declaration 'value'",
    );
    check_error(
        "fn work(value) {\n  let value = 2\n}\n",
        "duplicate-param",
        "duplicate declaration 'value'",
    );
}

#[test]
fn accepts_loops_and_shadowing() {
    let dir = temp_dir("control-flow-ok");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
let total = 0

for n in range(1, 5) {
  if n == 2 {
    continue
  }
  if n == 4 {
    break
  }
  let doubled = n * 2
  total = total + doubled
}

fn inner() {
  let total = 10
  return total
}

print(total)
print(inner())
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "8\n10");
}

#[test]
fn destructures_objects_and_arrays() {
    let dir = temp_dir("destructure");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
let user = { name: "Ada", meta: { age: 36, city: "London" }, tags: ["math", "code", "music"] }
let { name, meta: { age, city }, tags: [first, ...rest] } = user
print(name)
print(age)
print(city)
print(first)
print(join(rest, ","))

let { a, ...others } = { a: 1, b: 2, c: 3 }
print(a)
print(others.b + others.c)

let { "first-name": firstName } = { "first-name": "Grace" }
print(firstName)

const { title } = { title: "Vey" }
print(title)

let [head, second] = [1, 2, 3]
print(head + second)

for { id, label } in [{ id: 1, label: "one" }, { id: 2, label: "two" }] {
  print("{id}:{label}")
}

for [left, right] in [[1, 2], [3, 4]] {
  print(left + right)
}
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "Ada\n36\nLondon\nmath\ncode,music\n1\n5\nGrace\nVey\n3\n1:one\n2:two\n3\n7"
    );
}

#[test]
fn rejects_invalid_destructuring() {
    check_error(
        "let { name }\n",
        "destructure-no-value",
        "destructuring requires a value",
    );
    check_error(
        "export let { name } = { name: \"Ada\" }\n",
        "destructure-export",
        "export requires a single name",
    );
    check_error(
        "let { value, other: value } = { value: 1, other: 2 }\n",
        "destructure-duplicate",
        "duplicate declaration 'value'",
    );
}

#[test]
fn reports_circular_imports() {
    let dir = temp_dir("cycle");
    fs::write(dir.join("a.vey"), "use \"./b.vey\" as b\nprint(\"a\")\n").unwrap();
    fs::write(dir.join("b.vey"), "use \"./a.vey\" as a\nprint(\"b\")\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("check")
        .arg(dir.join("a.vey"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("circular import"));
}

#[test]
fn builds_html_page() {
    let dir = temp_dir("web");
    let file = dir.join("page.vey");
    fs::write(
        &file,
        r##"
component App() {
  main {
    h1 { "Hello" }
  }
}

bind("#app", App())
"##,
    )
    .unwrap();

    let output_path = dir.join("index.html");
    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("web")
        .arg(&file)
        .arg("-o")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    let html = fs::read_to_string(&output_path).unwrap();
    assert!(html.contains("<div id=\"app\"></div>"));
    assert!(html.contains("createVeyRuntime"));
    assert!(html.contains("V.h"));
}

#[test]
fn inlines_styles_from_blocks_and_css_files() {
    let dir = temp_dir("styles");
    fs::write(
        dir.join("theme.css"),
        ".from-file { color: teal; }\nbody { margin: 0; }\n",
    )
    .unwrap();
    let file = dir.join("page.vey");
    fs::write(
        &file,
        r##"
use "./theme.css"

style {
  .from-block { color: gold; }
  @media (max-width: 600px) {
    .from-block { color: red; }
  }
  .icon { background: url(https://example.com/a.png); }
}

component App() {
  main.nav-row {
    h1 { "Styled" }
  }
}

bind("#app", App())
"##,
    )
    .unwrap();

    let output_path = dir.join("index.html");
    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("web")
        .arg(&file)
        .arg("-o")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    let html = fs::read_to_string(&output_path).unwrap();
    assert!(html.contains("data-vey"), "{}", html);
    assert!(html.contains(".from-file"), "{}", html);
    assert!(html.contains(".from-block"), "{}", html);
    assert!(html.contains("@media (max-width: 600px)"), "{}", html);
    assert!(html.contains("url(https://example.com/a.png)"), "{}", html);
}

#[test]
fn builds_a_deployable_site() {
    let dir = temp_dir("site");
    fs::write(
        dir.join("index.vey"),
        r##"
component App() {
  main { h1 { "Home page" } }
}

bind("#app", App())
"##,
    )
    .unwrap();
    fs::write(
        dir.join("404.vey"),
        r##"
component Missing() {
  main { h1 { "Not found here" } }
}

bind("#app", Missing())
"##,
    )
    .unwrap();
    fs::create_dir_all(dir.join("public/assets")).unwrap();
    fs::write(dir.join("public/robots.txt"), "User-agent: *\nAllow: /\n").unwrap();
    fs::write(dir.join("public/assets/app.svg"), "<svg></svg>").unwrap();
    fs::write(dir.join("public/index.html"), "PUBLIC INDEX").unwrap();
    fs::write(dir.join("public/404.html"), "PUBLIC 404").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg("-o")
        .arg("site")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    let index = fs::read_to_string(dir.join("site/index.html")).unwrap();
    assert!(index.contains("Home page"), "{}", index);
    assert!(!index.contains("PUBLIC INDEX"), "{}", index);
    let missing = fs::read_to_string(dir.join("site/404.html")).unwrap();
    assert!(missing.contains("Not found here"), "{}", missing);
    assert!(!missing.contains("PUBLIC 404"), "{}", missing);
    assert!(dir.join("site/robots.txt").is_file());
    assert!(dir.join("site/assets/app.svg").is_file());
}

#[test]
fn new_project_builds_and_curate_is_gone() {
    let dir = temp_dir("new");
    let project = dir.join("myapp");

    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("new")
        .arg(&project)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    for file in [
        "main.vey",
        "404.vey",
        "style.css",
        "tests/main.vey",
        "public/robots.txt",
        "public/logo.svg",
        "public/favicon.svg",
    ] {
        assert!(project.join(file).is_file(), "missing {}", file);
    }

    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(build.status.success(), "{}", stderr(&build));
    assert!(!project.join("index.vey").exists());
    let index = fs::read_to_string(project.join("dist/index.html")).unwrap();
    assert!(index.contains("Welcome to"), "{}", index);
    assert!(index.contains("main.vey"), "{}", index);
    assert!(index.contains("favicon.svg"), "{}", index);
    for removed in [
        "A small language",
        "Clicked",
        "See features",
        "What you get",
        "Built from",
    ] {
        assert!(
            !index.contains(removed),
            "unexpected '{}' in landing page",
            removed
        );
    }

    let landing = fs::read_to_string(project.join("main.vey")).unwrap();
    assert!(landing.contains("width=\"76\""), "{}", landing);
    let css = fs::read_to_string(project.join("style.css")).unwrap();
    assert!(!css.contains("animation"), "logo must not animate: {}", css);
    assert!(
        !css.contains("@keyframes"),
        "logo must not animate: {}",
        css
    );

    let logo = fs::read_to_string(project.join("public/logo.svg")).unwrap();
    assert_eq!(logo.matches("<path").count(), 1, "{}", logo);
    assert!(
        !logo.contains("<rect"),
        "logo must not have a box: {}",
        logo
    );
    assert!(project.join("dist/logo.svg").is_file());
    assert!(project.join("dist/404.html").is_file());

    let curate = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("curate")
        .arg("oops")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!curate.status.success());
    assert!(stderr(&curate).contains("removed"), "{}", stderr(&curate));
}

#[test]
fn accepts_gradual_types() {
    let dir = temp_dir("types-ok");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
type User = { name: string, age?: number }
type Id = number | string

fn label(id: Id) -> string {
  if type(id) == "string" {
    return id
  }
  return str(id)
}

fn greet(name: string = "world") -> string {
  return "Hello, " + name
}

fn sum(...values: number) -> number {
  let total: number = 0
  for value in values {
    total = total + value
  }
  return total
}

let ada: User = { name: "Ada" }
let grace: User = { name: "Grace", age: 40 }
print(ada.name)
print(ada.age ?? 0)
print(grace.age ?? 0)
print(label(7))
print(label("x"))
print(greet())
print(sum(1, 2, 3))

let numbers: [number] = [1, 2, 3]
for n: number in numbers {
  let doubled: number = n * 2
  print(doubled)
}

let { name }: User = ada
print(name)

let [first, ...rest]: [number] = [9, 8, 7]
print(first)
print(rest)

let double: (number) -> number = (n: number) => n * 2
let maybe: (number) -> number = fn(n: number) -> number { return n + 1 }
print(double(4))
print(maybe(4))

fn nameOf(user: User | null) -> string {
  if user != null {
    return user.name
  }
  return "none"
}
print(nameOf(ada))
print(nameOf(null))

component Badge(text: string) {
  span.badge { text }
}

component Header() {
  Badge text="Vey"
}

component UserCard(user: User | null) {
  if user != null {
    button click=fn() {
      user = null
    } { user.name }
  }
}

print(html(Header()))
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "Ada\n0\n40\n7\nx\nHello, world\n6\n2\n4\n6\nAda\n9\n[ 8, 7 ]\n8\n5\nAda\nnone\n<span class=\"badge\">Vey</span>"
    );
}

#[test]
fn supports_generics() {
    let dir = temp_dir("generics");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
fn identity<T>(value: T) -> T {
  return value
}

type Box<T> = { value: T }
type Maybe<T> = T | null

fn first<T>(items: [T]) -> T? {
  if len(items) == 0 {
    return null
  }
  return items[0]
}

fn pair<A, B>(left: A, right: B) -> { left: A, right: B } {
  return { left: left, right: right }
}

fn apply<T, U>(value: T, callback: (T) -> U) -> U {
  return callback(value)
}

let numberValue: number = identity(1)
let stringValue: string = identity("x")
let box: Box<number> = { value: 2 }
let missing: Maybe<string> = null
let found: number | null = first([3, 4])
let pairValue: { left: number, right: string } = pair(1, "a")
let applied: number = apply(2, (n: number) => n + 1)
let id: (number) -> number = identity

print(numberValue)
print(stringValue)
print(box.value)
print(missing ?? "none")
print(found ?? 0)
print(pairValue.left)
print(pairValue.right)
print(applied)
print(id(3))
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "1\nx\n2\nnone\n3\n1\na\n3\n3");
}

#[test]
fn supports_enums() {
    let dir = temp_dir("enums");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
enum Color {
  Red
  Green
  Blue
}

enum Shape {
  Circle(radius: number)
  Rect(width: number, height: number)
}

fn colorName(color: Color) -> string {
  return match color {
    Color.Red => "red",
    Color.Green => "green",
    Color.Blue => "blue",
  }
}

fn area(shape: Shape) -> number {
  return match shape {
    Shape.Circle(r) => r * r,
    Shape.Rect(w, h) => w * h,
  }
}

let color = Color.Green
let shape = Shape.Circle(2.5)

print(colorName(color))
print(area(shape))
print(Color.Red)
print(Shape.Rect(3, 4))
print(type(color))
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "green\n6.25\nColor.Red\nRect(width=3, height=4)\nenum"
    );
}

#[test]
fn rejects_type_mismatches() {
    check_error(
        "let count: number = \"many\"\n",
        "type-literal",
        "expected number, found string",
    );
    check_error(
        "fn bad() -> number {\n  return \"x\"\n}\n",
        "type-return",
        "expected number, found string",
    );
    check_error(
        "fn add(a: number, b: number) -> number {\n  return a + b\n}\nadd(1, \"2\")\n",
        "type-argument",
        "argument 2 expects number, found string",
    );
    check_error(
        "type User = { name: string }\nlet user: User = { name: 1 }\n",
        "type-object",
        "expected { name: string }, found { name: number }",
    );
    check_error(
        "let user: { name: string } = { name: \"Ada\" }\nprint(user.age)\n",
        "type-field",
        "has no field 'age'",
    );
    check_error(
        "let value: number = 1\nvalue = \"x\"\n",
        "type-assign",
        "expected number, found string",
    );
    check_error(
        "fn only(a: number) -> number {\n  return a\n}\nonly()\n",
        "type-arity",
        "expected at least 1 argument, got 0",
    );
    check_error(
        "fn value() -> number {\n  let x = 1\n}\n",
        "type-missing-return",
        "function must return number",
    );
    check_error(
        "type Node = { next: Node }\nlet node: Node = { next: null }\n",
        "type-recursive",
        "recursive type alias 'Node'",
    );
    check_error(
        "let f: (number) -> number = (n: string) => n\n",
        "type-function",
        "expected (number) -> number, found (string) -> string",
    );
    check_error(
        "let values: [string] = [\"a\"]\nfor value: number in values {\n  print(value)\n}\n",
        "type-for",
        "expected number, found string",
    );
    check_error(
        "component Card(title: string) {\n  h2 { title }\n}\n\ncomponent App() {\n  Card title=123 { \"x\" }\n}\n\nprint(html(App()))\n",
        "type-component-prop",
        "expected string, found number",
    );
    check_error(
        "fn identity<T>(value: T) -> T {\n  return value\n}\nlet value: string = identity(1)\n",
        "type-generic-argument",
        "expected string, found number",
    );
    check_error(
        "fn bad<T>() -> T {\n  return 1\n}\n",
        "type-generic-return",
        "expected T, found number",
    );
    check_error(
        "type Box<T> = { value: T }\nlet box: Box = { value: 1 }\n",
        "type-generic-arity",
        "type 'Box' expects 1 type argument, got 0",
    );
    check_error(
        "type Box<T> = { value: T }\nlet box: Box<number> = { value: \"x\" }\n",
        "type-generic-field",
        "expected { value: number }, found { value: string }",
    );
    check_error(
        "fn bad<T, T>(value: T) -> T {\n  return value\n}\n",
        "type-generic-duplicate",
        "duplicate type parameter 'T'",
    );
    check_error(
        "enum Color { Red, Green, Blue }\nlet c: Color = 5\n",
        "type-enum",
        "expected Color, found number",
    );
    check_error(
        "enum Shape { Circle(radius: number) }\nlet x = Shape.Circle(\"bad\")\n",
        "type-enum-ctor",
        "argument 1 expects number, found string",
    );
    check_error(
        "enum Color { Red, Green, Blue }\nfn bad(c: Color) -> string {\n  return match c {\n    Color.Red => 'red',\n  }\n}\n",
        "type-enum-exhaustive",
        "match on 'Color' is not exhaustive, missing: Green, Blue",
    );
}

#[test]
fn supports_spread() {
    let dir = temp_dir("spread");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
let base = [1, 2]
let more = [0, ...base, 3]
print(more)
print(join(more, ","))

let defaults = { a: 1, b: 2 }
let merged = { ...defaults, b: 9, c: 3 }
print(merged.a)
print(merged.b)
print(merged.c)

fn add(a, b, c) { return a + b + c }
let args = [1, 2, 3]
print(add(...args))

let numbers: [number] = [1, 2]
let typedMore: [number] = [0, ...numbers, 3]
print(typedMore)

type Point = { x: number, y: number }
let origin: Point = { x: 0, y: 0 }
let shifted: Point = { ...origin, x: 2 }
print(shifted.x)
print(shifted.y)

fn sum(...values: number) -> number {
  let total: number = 0
  for value in values {
    total = total + value
  }
  return total
}
print(sum(...numbers))
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "[ 0, 1, 2, 3 ]\n0,1,2,3\n1\n9\n3\n6\n[ 0, 1, 2, 3 ]\n2\n0\n3"
    );
}

#[test]
fn supports_standard_helpers() {
    let dir = temp_dir("stdlib");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
let encoded: string = base64.encode("hello")
let decoded: string = base64.decode(encoded)
let encodedUrl: string = url.encode("a b")
let decodedUrl: string = url.decode(encodedUrl)
let values: [any] = unique([1, 1, 2])
let reversed: [any] = reverse([1, 2, 3])
let total: number = sum([1, 2, 3])
let bounded: number = clamp(12, 0, 10)
let pathText: string = path.join("a", "b")
print(encoded)
print(decoded)
print(encodedUrl)
print(decodedUrl)
print(values)
print(reversed)
print(total)
print(bounded)
print(pathText)
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "aGVsbG8=\nhello\na%20b\na b\n[ 1, 2 ]\n[ 3, 2, 1 ]\n6\n10\na/b"
    );
}

#[test]
fn runs_vey_tests() {
    let dir = temp_dir("test-runner");
    let passing = dir.join("passing.vey");
    fs::write(
        &passing,
        r#"
test("passes", fn() { assert(1 + 1 == 2) })
test("waits", fn() {
  await sleep(5)
  assert(true)
})
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("test")
        .arg(&passing)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("ok passes"), "{}", text);
    assert!(text.contains("ok waits"), "{}", text);
    assert!(text.contains("1 file(s) passed"), "{}", text);

    let failing = dir.join("failing.vey");
    fs::write(
        &failing,
        "test(\"fails\", fn() { assert(false, \"nope\") })\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("test")
        .arg(&failing)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(text.contains("fail fails"), "{}", text);
    assert!(text.contains("nope"), "{}", text);
}

#[test]
fn keeps_unannotated_const_dynamic() {
    let dir = temp_dir("const-dynamic");
    let file = dir.join("main.vey");
    fs::write(
        &file,
        r#"
const obj = { a: 1 }
obj.b = 2
const items = [1]
items.push(2)
print(obj.b)
print(items)
"#,
    )
    .unwrap();

    let output = run_file(&file);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "2\n[ 1, 2 ]");
}
