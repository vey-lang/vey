<p align="center">
  <img src="logo.svg" width="84" height="84" alt="Vey sparkle logo">
</p>

# Vey

Vey is a small web language with its own syntax, compiler and runtime. The compiler is written in Rust and ships as a single binary. It emits JavaScript, so the same `.vey` source runs on Node and in the browser. Types are optional and checked at compile time.

Browsers only execute JavaScript. That is why the output is JavaScript while the toolchain itself is not.

The changes of every release are in the [changelog](CHANGELOG.md).

## Build

```sh
cargo build --release
./target/release/vey help
```

## Commands

```sh
vey run                          # serve the site
vey dev                          # serve the site with a live reload
vey test                         # run .vey files under tests/
vey build                        # static site in .dist/
vey build app.vey --node -o app.cjs  # standalone node script
vey new myapp                    # project scaffold
vey check                        # check the whole project
```

`vey run` serves the site the way a host would: it is built once and then
served, without a watcher and without the reload. `vey dev` is the same
server for the work: it watches the files of the project, rebuilds when one
of them changes and reloads the page in the browser.

A program that is not a page — a script — is built and run with node:

```sh
vey build script.vey --node -o script.cjs
node script.cjs
```

## Static sites

`vey build` compiles the entry file, renders `404.vey` when it exists and copies
`public/` into a folder that can be uploaded as is:

```sh
vey build          # -> .dist/
vey build -o site  # -> site/
```

```
.dist/
  index.html
  404.html
  robots.txt      # from public/
```

The page title comes from the project directory, or from `--title "My site"`.
If there is no `404.vey`, Vey writes a styled default 404 page.

`main.vey` is the entry of a project. It is found in `src/` or in the project
itself, so the commands work without arguments inside a project. When run one
level above a project, the only matching entry in a subdirectory is used;
multiple matches must be passed explicitly.

Every other page of the project becomes a page of the site. The name of a
page follows the file it comes from, so `src/about.vey` becomes `about.html`
and `src/docs/index.vey` becomes `docs/index.html`. A file that is imported
by another page is a module and does not become a page, and a page may not
take the place of the home of the site.

The build folder is emptied before it is written. Vey refuses to write into
the project itself or into a folder that holds it, so that a wrong `-o` cannot
delete the sources. A link in `public/` that points outside the project is left
out, so that the site never holds files of the machine.

## New project

```sh
vey new myapp
```

The project that is written is small on purpose:

```
myapp/
  src/
    main.vey         the page
    styles/
      site.css       the styles of the site
  public/
    logo.svg         the mark of the site
    favicon.svg      the mark as a favicon
    robots.txt
  package.json       the scripts of npm
  README.md
  .gitignore
```

The styles of a page may also be written in a `style { }` block, next to the
markup they belong to. A page of an unknown address is `src/404.vey`; without
it, Vey writes a page of its own.

`npm run dev`, `npm run build` and `npm test` call the same commands as
`vey dev`, `vey build` and `vey test`.

## Servers

```sh
vey run
vey run --port 3000 --host 0.0.0.0
vey dev
```

`vey run` serves the built site the way a host does. `vey dev` is the same
server for the work: it rebuilds the site when the entry, any imported `.vey`
or `.css` file (including modules in other folders) or a `public/` file
changes, and reloads the browser. Other files in the project are left alone, so
a note or a log file does not cause a reload. A change of the content is
noticed even when the time and the size of the file stay the same. Unknown
paths serve `404.html` with a `404` status.
Compile errors open a styled page with the file, the source line, a caret under
the failing column, and a hint when Vey can guess the fix (for example
`use "../style.css"` for a stylesheet in the parent folder). The page
disappears after the next successful save, so the browser can stay open while
iterating. The same diagnostic is printed by `vey build` in the terminal.

Any `.vey` file of the project can be previewed by visiting its path, for
example `http://localhost:4000/about.vey`.

## Styles

CSS is written in `.css` files or in `style { ... }` blocks. Both end up in the
built page, so there is no extra request and no build step to configure.

```vey
use "./style.css"

style {
  body { margin: 0; background: #0b1020; color: white }
  .card:hover { border-color: #7c8cff }
  @media (max-width: 600px) { .card { padding: 8px } }
}
```

`style` blocks are raw CSS: braces, `@media` rules, `url(...)` values and
comments survive untouched. Styles from imported modules and `style` blocks
are collected in dependency order, so shared styles load before app styles.

## Debugging

Built scripts contain an inline source map, so a runtime error points to the
original `.vey` file and line, including errors inside imported modules. Run
them with the Node flag:

```sh
node --enable-source-maps app.cjs
```

## Testing

`vey test` runs every `.vey` file under `tests/`. Pass a file or directory to choose another target.

```vey
fn add(a: number, b: number) -> number {
  return a + b
}

test("adds numbers", fn() {
  assert(add(2, 3) == 5, "sum should be 5")
})
```
<!-- check -->

`test(name, fn)` runs immediately. A test that contains `await` returns a promise, and the runner keeps the process alive until it settles. `assert(condition, message)` throws when the condition is false. A failed test marks the process as failed.

## Checks

`vey check` rejects duplicate declarations, reassignment of constants and imports, `break` or `continue` outside a loop, `return` outside a function, assignment to an unknown variable, the use of an unknown variable, the use of a value before it is declared and type mismatches in annotated code. The same checks run before every build.

When a name is not known, the error names the one that is closest to it:

```
main.vey:2:7: unknown variable 'nam'

  print(nam)
        ^

  hint: did you mean 'name'?
```

## Language

```vey
let name = "Vey"
const version = 0.6

fn greet(who) {
  return "Hello, " + who
}

print(greet("world"))
print("Version {version}")

let double = n => n * 2
let numbers = range(1, 6)
print(numbers |> map(double))
```
<!-- check -->

Strings in double quotes interpolate `{expr}`. Single quoted strings are literal.

A brace only starts an interpolation when it holds a single expression. Anything
else, such as a JSON object, stays literal text:

```vey
print("{\"a\": 1}")   // {"a": 1}
print("{}")           // {}
print("value {name}") // the value of `name`
```

A function that uses `await` is async on its own; there is no `async` keyword.

### Values

```vey
let items = [1, 2, 3]
let user = { name: "Ada", age: 36 }
let nothing = null
let flag = true
```
<!-- check -->

Member access supports optional chaining and nullish fallback:

```vey
let city = user?.address?.city ?? "unknown"
```

Spread works in lists, objects and calls:

```vey
let combined = [0, ...items]
let merged = { ...defaults, active: true }
print(add(...args))
```

### Types

Types are optional. Annotations are checked at compile time and erased from the output. Unannotated `let` bindings, object constants and list constants stay dynamic, so existing code keeps working. Simple `const` values keep their inferred type.

```vey
type User = {
  name: string
  age?: number
}

type Id = number | string

fn greet(user: User) -> string {
  return "Hello, " + user.name
}

fn add(a: number, b: number) -> number {
  return a + b
}

let ada: User = { name: "Ada", age: 36 }
print(greet(ada))
print(add(2, 3))
```
<!-- check -->

Built-in types are `any`, `number`, `string`, `bool`, `null`, `void` and `never`.

Type forms:

- `[T]` list
- `{ name: string, age?: number }` object
- `A | B` union
- `T?` optional value
- `(number, string) -> bool` function
- `(...number) -> number` rest parameters

The fields of an object may be written one under the other, with or without a
comma. The same goes for the fields of an object value.

Functions and type aliases can be generic:

```vey
fn identity<T>(value: T) -> T {
  return value
}

type Box<T> = { value: T }
type Maybe<T> = T | null

let numberValue: number = identity(1)
let textValue: string = identity("Vey")
let box: Box<number> = { value: 2 }
let missing: Maybe<string> = null
```
<!-- check -->

Type arguments are inferred at call sites. A call may also name them:

```vey
fn parse<T>(text: string) -> T {
  return num(text)
}

let number = parse<number>("42")
```
<!-- check -->

A type parameter may have a constraint, which every type argument has to
satisfy:

```vey
fn total<T: number>(list: [T]) -> number {
  return 0
}

total([1, 2])   // fine
total(["a"])   // type argument 'T' expects number, found string
```

A type parameter that only the return value determines is settled by that
value, so the function is not generic after all.

Annotations work on `let`, `const`, function parameters, return types, `for` bindings, destructuring patterns, function expressions and arrow parameters.

```vey
let parse: (string) -> number = (text: string) => num(text)
let values: [number] = [1, 2, 3]

for value: number in values {
  print(value * 2)
}
```
<!-- check -->

The checker verifies assignments, return values, calls, object fields, list elements and component attributes. It narrows unions in simple conditions:

```vey
fn nameOf(user: User | null) -> string {
  if user != null {
    return user.name
  }
  return "none"
}
```

The narrowing also holds for the code after the check, as long as the branch
that does not match leaves the function:

```vey
fn age(user: User | null) -> number {
  if user == null {
    return 0
  }
  return user.age
}
```

`type(value)` checks narrow `string`, `number`, `boolean`, `list`, `object`, `function` and `enum` branches. A check only narrows to a type that the value can actually have.

The calls to the runtime helpers are checked as well, so `math.round("x")` and
`len()` are reported. A function with a return type has to return a value on
every path, and a component needs a value for each of its parameters that has
no default.

Enums are declared with `enum Name { Variant, Variant(field: Type) }`. Unit variants are values, payload variants are constructors, and `match` on an enum value checks exhaustiveness unless a wildcard arm is present.

```vey
enum Color {
  Red
  Green
  Blue
}

enum Shape {
  Circle(radius: number)
  Rect(width: number, height: number)
}

fn area(shape: Shape) -> number {
  return match shape {
    Shape.Circle(r) => r * r,
    Shape.Rect(w, h) => w * h,
  }
}
```
<!-- check -->

Types are module local. Exported type aliases and enums can be used from other modules as `module.TypeName`, and their fields are type-checked. A generic alias can be used from another module as well, with its type arguments filled in:

```vey
// types.vey
export type Box<T> = { value: T }

// app.vey
use "./types.vey" as t

let box: t.Box<number> = { value: 2 }
```

An alias may refer to itself, which is how a linked list or a tree is
written:

```vey
type Node = { value: number, next: Node | null }

let list: Node = { value: 1, next: { value: 2, next: null } }
```
<!-- check -->

The part of the alias that is still open is checked loosely, because it cannot
be written out. The other fields are checked as usual.

The functions and constants of another module keep their signature, so a call
to them is checked in the same way as a local one.

Unit variants print as `Color.Red`; payload variants print as `Circle(radius=2.5)`. `type(value)` returns `"enum"` for enum values.

### Destructuring

```vey
let { name, meta: { age }, tags: [first, ...rest] } = user
let [head, second] = items
const { title } = post

for { id, label } in records {
  print("{id}: {label}")
}
```

Object patterns support renamed keys, nested patterns, string keys and a rest binding. Array patterns support nested patterns and a rest binding. Destructuring requires a value, and an exported declaration must bind a single name.

A name may have a value that is used when there is none:

```vey
let user = { name: "Ada" }
let items = [1, 2]

let { name, greeting = "Hello" } = user
let [head, second = 0] = items

print(greeting + " " + name)
print(head)
print(second)
```
<!-- check -->

A parameter may take the argument apart as well:

```vey
fn greet({ name, greeting = "Hello" }) {
  return greeting + " " + name
}

greet({ name: "Ada" })
greet({ name: "Ada", greeting: "Hi" })
```
<!-- check -->

A component reads its props the same way:

```vey
component Card({ title, subtitle }) {
  section { h2 { title } p { subtitle } }
}
```
<!-- check -->

### Control flow

```vey
if count > 10 {
  print("large")
} else if count > 5 {
  print("medium")
} else {
  print("small")
}

for item in items {
  print(item)
}

repeat 3 {
  print("tick")
}

while count > 0 {
  count -= 1
}
```

`range(a, b)` produces `a` up to `b - 1`. `range(n)` produces `0` up to `n - 1`. The `..` operator does the same:

```vey
for i in 0..10 {
  print(i)
}
```
<!-- check -->

### Pattern matching

```vey
let label = match score {
  0 => "zero"
  1 | 2 => "small"
  3..9 => "medium"
  _ => "large"
}
```

A name takes the value of the match, and a condition narrows the arm down:

```vey
let label = match score {
  n if n > 100 => "a lot"
  n if n > 10 => "some"
  _ => "few"
}
```

A variant of an enum may take its fields apart as well:

```vey
enum Size {
  Small
  Large(weight: number)
}

let size = Size.Large(30)

let label = match size {
  Size.Large(weight) if weight > 10 => "heavy"
  Size.Large(weight) => "light"
  Size.Small => "small"
}

print(label)
```
<!-- check -->

### Pipes and arrows

```vey
let result = items
  |> filter(n => n % 2 == 1)
  |> map(n => n * 10)
```

`value |> fn(a)` becomes `fn(value, a)`.

### Errors

```vey
try {
  json.parse(text)
} catch err {
  print("bad json: {err.message}")
}
```

A `finally` block always runs, with or without an error. It may stand on its own
or follow a `catch`:

```vey
try {
  save(text)
} catch err {
  print("could not save: {err.message}")
} finally {
  print("done")
}
```

### Modules

```vey
// math.vey
export const PI = 3.14159

export fn area(radius) {
  return PI * radius ** 2
}
```
<!-- check -->

```vey
// app.vey
use "./math.vey" as math

print(math.area(2))
```

Imports are resolved at compile time. The compiler bundles every module into one output file. Circular imports are reported as errors.

Exported type aliases and enums are available as `module.TypeName` in annotations:

```vey
// types.vey
export type User = { name: string, age: number }
export enum Status { Active, Inactive }

// app.vey
use "./types.vey" as t

let user: t.User = { name: "Ada", age: 30 }
let status: t.Status = t.Status.Active
```


## Components

Vey does not use JSX. Markup is written with tag selectors, without angle brackets or closing tags.

```vey
component Card(title, children) {
  section.card {
    h2 { title }
    p { "A card body." }
    children
  }
}

component App() {
  main.page {
    h1 { "Hello" }
    Card title="Counter" {
      button.btn click=fn() {
        print("clicked")
      } { "Click me" }
    }
  }
}
```
<!-- check -->

Rules:

- A tag is a name with optional `.class` and `#id` selectors: `div.card`, `main#root`, `img.avatar`, `div.nav-row`.
- Attributes use `name=value`: `class="row"`, `href="/home"`, `click=fn() { ... }`, `disabled=true`.
- Children go inside braces. Use a string or an expression directly for text.
- `if` and `for` work inside markup.
- Components start with an uppercase letter and receive attributes as parameters. `Card { ... }` and `Card() { ... }` are equivalent.
- A component of another module is written `module.Card`.
- `children` is passed automatically when a component has child nodes.
- `style`, `script`, `head`, `title`, `meta` and `link` are regular HTML tags inside markup.

Event attributes map to DOM handlers: `click`, `dblclick`, `change`, `input`, `submit`, `load`, `focus`, `blur`, `keydown`, `keyup`, `mouseover`, `mouseout`.

### Passing arguments to a component

A component takes its parameters in order. In markup the arguments can also be
named, which is handy when a parameter has a default value:

```vey
component Card(title, subtitle = "none") {
  section.card {
    h2 { title }
    p { subtitle }
  }
}

component App() {
  Card("Counter")            // positional
  Card("Counter", "live")    // positional
  Card title="Counter"       // named
}
```
<!-- check -->

Named arguments only work in markup. Everywhere else the arguments are
positional.

A component of another module is used with the name of the module in front of
it:

```vey
// ui.vey
export component Card(title: string) {
  section { h2 { title } }
}

// app.vey
use "./ui.vey" as ui

component App() {
  ui.Card title="Counter"
}
```

The name of a module starts with a small letter and the name of a component
with a big one, so that `div.card` stays a tag with a class.

### Browser

```vey
bind("#app", App())
```

`bind(selector, node)` renders a component into the page.

### Server side rendering

The same component can be rendered to HTML on the server:

```vey
print(html(App()))
```

Text children are escaped. Wrap trusted markup with `raw()`:

```vey
p { raw("<strong>Trusted</strong>") }
```

## Server

```vey
let app = server(3000)

app.get("/", fn(req) {
  return "<h1>Hello</h1>"
})

app.get('/users/{id}', fn(req) {
  return { id: req.params.id, query: req.query }
})

app.post("/data", fn(req) {
  return response({ received: req.body }, 201)
})

app.static("public")
app.start()
```
<!-- check -->

Unknown routes automatically try `public/404.html` and return it with status
404. A custom handler takes priority:

```vey
app.notFound(fn(req) {
  return response("<h1>Nothing at {req.path}</h1>", 404)
})
```

Route patterns may end with `*` to capture the rest of the path in
`req.params.splat`:

```vey
app.get('/files/*', fn(req) {
  return { file: req.params.splat }
})
```

Route patterns use single quotes, so `{id}` is treated as a path parameter instead of string interpolation.

Handler results:

- string: HTML
- object or array: JSON
- `response(body, status, headers)`: explicit response
- `redirect(url, status)`: redirect

The request object contains `path`, `method`, `params`, `query`, `headers`, `body`, `text` and `cookies`.

## Runtime

| Name | Description |
| --- | --- |
| `print(...)` | write to the console |
| `input(prompt)` | read a line from stdin |
| `len(value)` | string, list or object length |
| `append(list, ...items)` | new list with items added |
| `slice(value, start, end)` | slice a string or list |
| `join(list, sep)` / `split(text, sep)` | list and text conversion |
| `map`, `filter`, `reduce`, `find`, `sort` | list helpers |
| `keys`, `values`, `entries`, `omit` | the parts of an object |
| `startsWith(text, prefix)`, `endsWith(text, suffix)` | the ends of a text |
| `json.parse`, `json.stringify` | JSON |
| `math.round`, `math.floor`, `math.random`, `math.pi` | math |
| `time.now()`, `time.iso()` | time |
| `http.get(url)`, `http.post(url, body)` | HTTP client |
| `sleep(ms)` | promise based delay |
| `html(node)` | render a component tree to HTML |
| `raw(html)` | mark trusted HTML so `html()` does not escape it |
| `bind(selector, node)` | mount into the DOM |
| `type(value)` | runtime type name |
| `assert(condition, message)` | throw when a condition is false |
| `test(name, fn)` | run a test block |
| `all(list)` / `race(list)` | promise combinators |
| `env(name, fallback)` | environment variable, Node only |
| `uuid()` / `random(min, max)` / `clamp(value, min, max)` | identifiers and numbers |
| `sum(list)` / `unique(list)` / `reverse(list)` | list helpers |
| `base64.encode(value)` / `base64.decode(value)` | base64 |
| `url.encode(value)` / `url.decode(value)` / `url.parse(value)` | URL helpers |
| `fs.read`, `fs.write`, `fs.append`, `fs.exists`, `fs.list`, `fs.remove`, `fs.mkdir` | file system, Node only |
| `path.join`, `path.dirname`, `path.basename`, `path.extname`, `path.resolve` | paths, Node only |

A few details worth knowing:

- `sort(list)` orders numbers by value and everything else as text. Pass a
  comparison function to sort in another order.
- `random()` without arguments returns a fraction between 0 and 1, while
  `random(min, max)` returns an integer in that range.
- `reduce(list, fn)` needs an initial value when the list is empty.
- `json.stringify` always returns text, so a value it cannot encode becomes
  the text `undefined`.
- `url.parse` accepts a relative path and resolves it against a local origin.

## Project layout

```
src/main.rs          CLI entry point
src/lib.rs           the library of the compiler
src/ast/             the shape of a program
  decl.rs            the declarations
  expr.rs            the expressions
  stmt.rs            the statements
  pattern.rs         the patterns
  types.rs           the type expressions
  markup.rs          the markup
src/lexer/           the tokenizer
  mod.rs             the tokens
  strings.rs         the reading of string literals
src/parser/          the parser
  mod.rs             the parser and the statements
  exprs.rs           the expressions
  patterns.rs        the binding patterns and the match patterns
  types.rs           the type expressions
  strings.rs         the string literals
  markup.rs          the tag syntax
  html.rs            the HTML tags
src/check/           the semantic checks
  mod.rs             the checks
  scopes.rs          the scopes and the names
src/types/           the gradual type checker
  core.rs            the type representation and the rules between types
  builtins.rs        the signatures of the runtime helpers
  checker.rs         the checker that walks a program
  stmts.rs           the statements
  exprs.rs           the expressions
  patterns.rs        the patterns and the narrowing
  resolve.rs         the resolution of the types
  helpers.rs         the helpers of the checker
src/codegen/         the JavaScript output
  mod.rs             the statements
  exprs.rs           the expressions
  markup.rs          the markup
  js.rs              the JavaScript helpers
  awaits.rs          finding out whether code uses `await`
src/compiler/        the module graph
  mod.rs             the bundle
  loader.rs          the loading of the modules
src/sourcemap/       the source map
  mod.rs             the map
  vlq.rs             the encoding of the map
src/cli/             the commands
  args.rs            command line parsing
  run.rs             run, test and check
  build.rs           build and web
  site.rs            static sites
  dev.rs             development server
  scaffold.rs        new projects
  entry.rs           finding the entry file
  error.rs           build errors with a source frame
  highlight.rs       the colouring of the code in the error page
  output.rs          turning a bundle into runnable output
src/builtins.rs      the names the runtime provides
src/suggest.rs       suggestions for names that are close to a known one
runtime.js           runtime shared by node and the browser
tests/               integration tests
  lang.rs            the language
  types.rs           the type checker
  regressions.rs     bugs that used to give wrong results
  runtime.rs         the runtime
  server.rs          the http server
  cli.rs             the commands
  dom.rs             rendering in a browser
  dev.rs             the development server
```
