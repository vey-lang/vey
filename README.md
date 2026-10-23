<p align="center">
  <img src="logo.svg" width="84" height="84" alt="Vey sparkle logo">
</p>

# Vey

Vey is a small web language with its own syntax, compiler and runtime. The compiler is written in Rust and ships as a single binary. It emits JavaScript, so the same `.vey` source runs on Node and in the browser. Types are optional and checked at compile time.

Browsers only execute JavaScript. That is why the output is JavaScript while the toolchain itself is not.

The changes of every release are in the [changelog](CHANGELOG.md). What the
language is still missing, and the order in which the gaps are closed, is in
the [roadmap](ROADMAP.md).

## Build

```sh
cargo build --release
./target/release/vey help
```

## Install

Vey is one binary. The script puts it in `~/.local/bin`:

```sh
curl -fsSL https://veylang.tech/install.sh | sh
```

The site of the language is at [veylang.tech](https://veylang.tech), and it is
written in Vey. Its source is in `examples/veylang`.

A whole application, Veyboard — tasks, notes and a dashboard, kept in a file —
is in `examples/veyboard`. It was made with `vey new`, and its README says how
to run it and how it is built.

## Commands

```sh
vey dev                          # build, serve, and build again on a change
vey run                          # build and serve the way a host does
vey test                         # run .vey files under tests/
vey build                        # the site in .dist/, or the program of a server
vey build app.vey --node -o app.cjs  # standalone node script
vey new myapp                    # project scaffold
vey check                        # check the whole project
```

`vey dev` is the command for the work: it watches the files of the project
and builds it again when one of them changes. A project of pages is served
with a reload of the page in the browser. A project of a server is built, run,
and built and started again; the pages it serves carry a small listener that
writes them again when the build changes, and a note in the corner while the
server is starting again. A build that does not pass stops the server, so an
answer never comes from code that the files no longer say, and the reason is
printed in the terminal.

`vey run` is the same project the way a host has it: a site is built once and
served, and a server is built and run as the program it is. Neither a watcher
nor a reload is in either.

The kind of a project is `kind` in `vey.toml`: `page`, `server` or `lib`. A
project that does not write the line is read from its entry: a program that
starts a server is a server, and everything else is a site of pages.

A program that is not a page — a script — is built and run with node:

```sh
vey build script.vey --node -o script.cjs
node --enable-source-maps script.cjs
```

The flag makes a runtime error point to the line of the `.vey` file.

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
  index.js        # the script of the page
  404.html
  robots.txt      # from public/
```

The script of a page is written in a file of its own, so that the browser
keeps it and the page of the site stays small.

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
    main.vey         the entry: the page is put on the screen
    site.vey         the component of the page and its styles
    styles/
      site.css       the styles of the site
  public/
    logo.svg         the mark of the site
    favicon.svg      the mark as a favicon
    robots.txt
  tests/
    site.vey         the tests of the page
  package.json       the scripts of npm
  README.md
  .gitignore
```

The styles of a page may also be written in a `style { }` block, next to the
markup they belong to. A page of an unknown address is `src/404.vey`; without
it, Vey writes a page of its own.

`npm run dev`, `npm run build` and `npm test` call the same commands as
`vey dev`, `vey build` and `vey test`.

## Development

```sh
vey dev
vey dev --port 3000 --host 0.0.0.0
```

`vey dev` watches the project and builds it again when a file changes: the
entry, every `.vey` file it imports (including modules in other folders),
`.css` files, and the files of `public/`. A page that is added or taken away is
noticed as well. Other files in the project are left alone, so a note or a log
file does not cause a build. A change of the content is noticed even when the
time and the size of the file stay the same.

A project of pages is served with a reload of the page in the browser.
Compile errors open a styled page with the file, the source line, a caret under
the failing column, and a hint when Vey can guess the fix (for example
`use "../style.css"` for a stylesheet in the parent folder). The page
disappears after the next successful save, so the browser can stay open while
iterating. The same diagnostic is printed by `vey build` in the terminal.

While the work is going on, any `.vey` file of the project can be previewed by
visiting its path, for example `http://localhost:4000/about.vey`. The release
server serves the site that was built and nothing else.

## Servers

A project whose entry starts a server is a server. It is built into one
program of Node and run:

```sh
vey dev        # build, run, and start again when a file changes
vey run        # build and run, the way a host has it
vey build      # -> .dist/server.cjs
node --enable-source-maps .dist/server.cjs
```

`vey dev` builds the program into `.vey/dev/server.cjs`, runs it with Node, and
watches the project. When a file changes the program is stopped, built again
and started again. A program that does not build leaves the server stopped,
and the mistake is printed in the terminal, so an answer never comes from code
that the files no longer say. The build only restarts when the build is done,
and a tool that saves a file twice does not start two servers.

The pages of a program that is worked on carry `/__vey/dev/client.js`. It
listens to `/__vey/dev/events` and writes the page again when the build
changes, and a small note says that the server is starting again while it is.
The listener and the two routes of it are only in the program that `vey dev`
runs: a program built or run for a host has neither, and `vey run` serves the
same HTML without them.

The program listens where its own `server()` call says. The port and the host
of the settings are given to it as `PORT` and `HOST`, so a program that reads
them follows the command line:

```vey
let port = int(env("PORT", "4000"))
let host = env("HOST", "127.0.0.1")

let app = server(port, { host: host })
```

```sh
vey dev --port 4100 --host 0.0.0.0
```

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

The tests may be chosen with a filter, so that only the ones whose name holds
the text are run:

```sh
vey test --filter "adds"
```

There are matchers for the things a test says most of the time:
`assertEquals`, `assertNotEquals`, `assertDeepEquals`, `assertContains`,
`assertNotContains`, `assertThrows` and `assertMatch`. Each of them takes
the values and a message that says what the test was about.

The number of the tests that passed and of the ones that failed is told when
the tests are done.

## Checks

`vey check` rejects duplicate declarations, reassignment of constants and imports, `break` or `continue` outside a loop, `return` outside a function, assignment to an unknown variable, the use of an unknown variable, the use of a value before it is declared and type mismatches in annotated code. The same checks run before every build.

When a name is not known, the error names the one that is closest to it:

```
main.vey:2:7: unknown variable 'nam'

  print(nam)
        ^

  hint: did you mean 'name'?
```

The compiler also gives warnings, which do not stop a build: a name that is
written and never read, a name that hides another one, and a name that is
close to another one.

## Settings

The settings of a project are written in `vey.toml` in the folder of the
project. The options of the command line are stronger than the file, so that
a program may be run in another way for a moment.

```toml
# The port that the server listens on.
port = 4000
# The host that the server listens on.
host = "127.0.0.1"
# The title of the site.
title = "My site"
# The folder the site is written to.
out = ".dist"

[lint]
# The length of a line that is still easy to read.
maxLine = 100
```

> **Note:** minification is not implemented. If `[build].minify = true` is set,
> `vey build` and `vey deploy` stop with an explicit diagnostic rather than
> silently writing unminified output. This also covers Node builds. Omit the
> setting (or set it to `false`) until a safe minifier is available.

## The tools

| Command | What it does |
| --- | --- |
| `vey fmt` | write the files in the usual shape |
| `vey lint` | report the smells of a program |
| `vey repl` | try the language in a prompt |
| `vey explain <file>:<line>:<column>` | say the type of an expression |
| `vey docs` | write the documentation of a program |
| `vey migrate` | bring a program to this version of the language |
| `vey deploy` | build the site and say where it is |
| `vey lsp` | the language server |
| `vey add <package>` | add a package of npm to the project |

Every command has a help of its own:

```sh
vey build --help
```

The build is cached in `.vey/`, so a project that did not change is built
again in no time. The modules of a project are written in parallel.

The language server (`vey lsp`) tells the editor what is wrong with a file,
the type of the expression under the cursor, the names that are declared in
a file, where a name is declared, where it is used, and how it may be
changed.

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
The word is refused, so that a program cannot have a function that waits and
one that does not without saying so.

A template holds the parts of a text, and the values that are woven into it:

```vey
let name = "Vey"
print(`Hello, {name}!`)
```
<!-- check -->

A value may be cast to another type with `as`, and its type may be checked
with `is`:

```vey
let value: any = "a text"
let text = value as string

let n: any = 42
if n is number {
  print("a number")
}
```
<!-- check -->

The type of an arrow may be written as well:

```vey
let double = (n: number): number => n * 2
```
<!-- check -->

An arrow may name its type parameters, the way a function does:

```vey
let identity = <T>(value: T) => value
```
<!-- check -->

The arguments of a call may be given by name, and the ones that are given
in order fill the parameters that are left:

```vey
fn draw(width: number, height: number, color: string) -> string {
  return color + " " + str(width) + "x" + str(height)
}

print(draw(height=20, width=10, color="red"))
```
<!-- check -->

### The shapes of a program

A map keeps the keys that are not texts, and a set holds the values it has
seen once:

```vey
let seen = Map()
seen.set(1, "one")
seen.set(2, "two")
print(seen.get(1))

let names = Set()
names.add("Ada")
names.add("Ada")
print(names.size)
```
<!-- check -->

A function that may fail says so in its type, and `?` hands the failure on:

```vey
fn readNumber(text: string) -> Result<number, string> {
  if text == "" {
    return err("the text is empty")
  }
  return ok(num(text))
}

fn double(text: string) -> Result<number, string> {
  // The `?` gives the failure of the call back to the caller.
  let value = readNumber(text)?
  return ok(value * 2)
}

print(double("21").value)
```
<!-- check -->

A function that hands its values back one by one is a generator, and a loop
may walk it:

```vey
fn lines(words: [string]) -> string {
  for word in words {
    yield word
  }
}

for line in lines(["one", "two"]) {
  print(line)
}
```
<!-- check -->

A piece of code that runs when the block is over is written with `defer`:

```vey
fn work() -> string {
  // The piece of code runs when the function is left.
  defer print("the work is over")
  return "the work is done"
}

print(work())
```
<!-- check -->

A type may have functions of its own, and they are given with `impl`:

```vey
type Config = { port: number }

impl Config {
  fn url() -> string {
    return "http://localhost:" + str(self.port)
  }
}

let config = Config({ port: 3000 })
print(config.url())
```
<!-- check -->

A value may be taken apart with `if let`, `while let` and `let else`:

```vey
let names = ["Ada", "Grace"]

if let name = names[0] {
  print(name)
}

fn first(names: [string]) -> string {
  // The value has to be there, or the block runs.
  let found = names[0] else {
    return "there is no name"
  }
  return found
}

print(first(["Ada"]))
```
<!-- check -->

A name may be public or private, so that a library keeps its helpers to
itself:

```vey
pub fn open() -> string { return "open" }
priv fn helper() -> number { return 1 }
```

A file may say which version of the language it was written for, and the
doc comments of a file are the documentation of it:

```vey
// vey: 0.7

/// The greeting of a name.
export fn greet(name: string) -> string {
  return "Hello, " + name
}
```

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
- `Signal<T>` / `Store<T>` the state of a page (see [A page that lives](#a-page-that-lives))

Computed access follows the declared shape: lists accept numeric indices and
dictionaries accept keys of their declared key type. Fixed objects and tuples
require literal property names and literal, in-range tuple indices; an `unknown`
value must be narrowed before computed access. Use a dictionary or list when
the key or index is only known at runtime.

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

A checked Vey module is a namespace of known exports, not a dynamic dictionary. A computed module member must use a string literal, such as `math["area"]`; a nonliteral key is rejected because its export signature cannot be checked. For dynamic keys, export a dictionary and index that value instead.

### Packages of npm

A package of npm is used with the `npm:` prefix. The package is loaded when the
program runs, so it has to be installed in the project:

```sh
npm install marked
```

```vey
use "npm:marked" as marked

let page = marked.parse("# Hello")
print(page)
```

A package is not checked, because Vey does not know what is inside it. Its
members are of the type `any`.

Packages of npm need Node, so they may only be used in a program and not in a
page of a site. A page that uses one is reported when the site is built.

The `require` helper loads a package without an import, which is handy for one
that is only used in one place:

```vey
let marked = require("marked")
print(marked.parse("# Hello"))
```

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

- A tag is the name of a tag of HTML, with optional `.class` and `#id` selectors: `div.card`, `main#root`, `img.avatar`, `div.nav-row`. A name that is not a tag of HTML is reported.
- Attributes use `name=value`: `class="row"`, `href="/home"`, `click=fn() { ... }`, `disabled=true`. A value may also be written in braces: `class={name}`.
- Children go inside braces. Use a string or an expression directly for text.
- `if` and `for` work inside markup.
- Components start with an uppercase letter and receive attributes as parameters. `Card { ... }` and `Card() { ... }` are equivalent.
- A component of another module is written `module.Card`.
- `children` is passed automatically when a component has child nodes.
- `style`, `script`, `head`, `title`, `meta` and `link` are regular HTML tags inside markup.
- The tags of SVG are tags as well, so a drawing may be written: `svg`, `path`, `circle`, `g`, `defs`, `linearGradient` and the rest.
- The name of an attribute may have parts: `data-value`, `xml:lang`, `xlink:href`.
- The props of a component may be spread into it: `Card ...props`.
- A component may have values of its own, which are kept when the page is written again:

```vey
component Counter(label) {
  // The value belongs to this component, so every counter of a page has
  // its own.
  let count = signal(0)
  button click=fn() {
    count.set(count.value + 1)
  } { "{label}: {count.value}" }
}
```

- `Boundary` catches the mistakes of the components under it, so that one broken component does not take the whole page away:

```vey
component Fallback() {
  div { "Something went wrong." }
}

component App() {
  Boundary fallback=Fallback {
    Risky()
  }
}
```

- A component may give a value to the ones under it, so that it does not have to be given to each of them:

```vey
component Leaf() {
  let theme = inject("theme")
  div { "the theme is {theme}" }
}

component App() {
  let theme = signal("dark")
  provide("theme", theme)
  Leaf()
}
```

- A part of the page may be written in another place of it, so that a dialogue or a menu is not cut by the box it belongs to:

```vey
component Dialogue(text) {
  div.card { text }
}

component App() {
  div.page {
    "the page"
    // The dialogue is written in the element `#side`, and it follows the
    // state of the page the way the rest of it does.
    portal(Dialogue("Are you sure?"), "#side", "dialogue")
  }
}
```

- A component may have slots, so that the caller may put its content in the place it wants:

```vey
component Card(title, body) {
  div.card {
    h2 { title }
    // The content of the slot is there when the caller gives nothing.
    slot name="body" { p { "There is nothing here yet." } }
  }
}

component App() {
  Card("Hello") {
    p slot="body" { "The content of the caller." }
  }
}
```

Event attributes map to DOM handlers: `click`, `dblclick`, `change`, `input`, `submit`, `load`, `focus`, `blur`, `keydown`, `keyup`, `keypress`, `mouseover`, `mouseout`, `wheel`, `context-menu`, `pointer-down`, `pointer-up`, `pointer-move`, `pointer-enter`, `pointer-leave`, `touch-start`, `touch-end`, `touch-move`, `drag-start`, `drag-end`, `drag-over`, `drop`.

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

### A page that lives

A value that tells when it changes is a `signal`. A state that is shared
between the parts of a page is a `store`, and the work that follows a change
is an `effect`.

```vey
let count = signal(0)

effect(fn() {
  print("the count is {count.value}")
})

count.set(1)
```

A signal or a store that is not written again is a `const`, so that the type
of its value is kept and a change of another type is said:

```vey
const count = signal(0)

let shown: number = count.value
count.set(1)
```
<!-- check -->

`count.set("text")` is a mistake then: the checker says *argument 1 expects
number, found string*. A value that is a `let` stays dynamic, the way every
`let` does.

The type may also be written by hand, which is how a signal and a store are
named when they are passed around:

```vey
let count: Signal<number> = signal(0)
let shared: Store<{ a: number }> = store({ a: 1 })
```
<!-- check -->

Two values of a signal are compared the way its options say, and not always
with `===`:

```vey
const same = signal(0, { equals: fn(a, b) { return a == b } })
const deep = signal({ a: 1 }, { equals: "deep" })
```
<!-- check -->

A value that follows other values is a `computed`. It is called again when
one of the values it read has changed.

```vey
let first = signal(1)
let second = signal(2)
let total = computed(fn() { return first.value + second.value })

print(total.value)
first.set(10)
print(total.value)
```

`mount(selector, node)` writes a page and changes it in place, so that a list
may grow, shrink and be put in another order. An item of a list may carry a
`key`, which is how the page knows that it is the same item.

```vey
let page = mount("#app", App())
page.update(App())
```

When the page is given as a function, the page follows the state it reads:
the values that the markup reads are remembered, and the page is written
again when one of them changes.

```vey
let count = signal(0)

component App() {
  main.page {
    h1 { "Clicked {count.value} times" }
    button click=fn() { count.set(count.value + 1) } { "click" }
  }
}

mount("#app", App)
```

The view of a route may also be a function, and the page of the route
follows the state it reads as well.

```vey
route("/users/{id}", fn(params) {
  h1 { "User {params.id}" }
})
```

When the server has already written the page, the browser takes it over
instead of putting it out again, so that the visitor does not see the page
twice.

The routes of the browser are written with `route`, and another address is
shown with `navigate`. The parameters of the route that is shown now are in
`routeParams()`.

```vey
component UserPage(params) {
  h1 { "User {params.id}" }
}

route("/users/{id}", fn(params) {
  UserPage(params)
})

navigate("/users/7")
```

The value of a field may be a signal. The field shows the value of the
signal, and the signal takes the value of the field when the visitor
writes in it.

```vey
let name = signal("Ada")

component App() {
  input value=name placeholder="your name" { }
}
```

A node may say hello when it comes and goodbye when it goes. The work of
`enter` is done after the node is in the page, and the work of `leave` is
done before the node is taken away. When the work of `leave` gives a
promise, the node waits for it, so a transition may be seen.

```vey
component App() {
  div {
    if visible() {
      section.card leave=fn(node) {
        node.classList.add("leaving")
        return sleep(300)
      } {
        "Goodbye"
      }
    }
  }
}
```

`transition` writes that work for a fade, so the same node may come in and go
out without the work of both parts being written by hand. The `enter` and
`leave` of the value are given to the node, and either of them may be given
on its own.

```vey
let fade = transition(200)

component App() {
  div {
    if visible() {
      section.card enter=fade.enter leave=fade.leave { "Hello" }
    }
  }
}
```

A form holds the values of its fields together with what is wrong with them.
`check` is given the values and gives back the fields that are wrong, so
`valid` follows the fields and `submit` calls its function only when the form
may be sent.

```vey
let signup = form({ name: "", mail: "" }, { check: fn(values) {
  let found = {}
  if values.name == "" { found.name = "a name is needed" }
  if !includes(values.mail, "@") { found.mail = "the mail is not right" }
  return found
} })

component App() {
  form {
    input value=signup.values.value.name placeholder="your name" { }
    input value=signup.values.value.mail placeholder="your mail" { }
    button click=fn() {
      signup.submit(fn(values) { print("sent to {values.mail}") })
    } { "Send" }
    p { signup.errors.value.name }
  }
}
```

The events of one's own are attached with `on`, and a listener that watches
the children of a node is attached with `delegate`. The work that is done
when a node is there is written with `onMount`, the work that is done when
it is taken away with `onDestroy`, and a node may be held with `ref()`.

```vey
let input = ref()

on("#app", "click", fn(event) {
  print("clicked")
})

delegate("#app", "click", ".item", fn(event, node) {
  print(node.textContent)
})
```

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

Routes are looked at before the files of a folder, so a route wins against a
file of the same name: only a request that no route takes is served from
`public/` (and only a `GET` or a `HEAD` asks for a file).

Unknown addresses try `public/404.html` first and return it with status 404.
A custom handler takes priority over the page of Vey:

```vey
app.notFound(fn(req) {
  return response("<h1>Nothing at {req.path}</h1>", 404)
})
```

Without either, Vey writes a page of its own for a browser and a line of text
for a program, which is what the `Accept` header says. The same is true of
the other mistakes:

| The answer | When |
| --- | --- |
| `400` | the body says it is JSON and is not |
| `405` | the path is a route of another method; `Allow` names the methods it takes |
| `413` | the body is bigger than `bodyLimit` (or the limit of a route) |
| `500` | a handler throws, or a file of the server cannot be read |

The reason of a `500` is written in the log of the server, not in the page,
because it may name a file or a password. `server(port, { exposeErrors: true })`
puts it in the page as well, for the machine where the program is worked on.

Route patterns may end with `*` to capture the rest of the path in
`req.params.splat`:

```vey
app.get('/files/*', fn(req) {
  return { file: req.params.splat }
})
```

Route patterns use single quotes, so `{id}` is treated as a path parameter instead of string interpolation.

A parameter may be left out with `{id?}`, and it may hold a constraint with
`{id:int}`, which is one of `int`, `number`, `uuid`, `slug` and `word`, or a
pattern of its own. The routes are looked at in the order they were written,
so the first one that fits is the one that answers.

```vey
app.get('/users/{id?}', fn(req) {
  return { id: req.params.id }
})
```

Handler results:

- string: HTML
- object or array: JSON
- `response(body, status, headers)`: explicit response
- `redirect(url, status)`: redirect

The request object contains `path`, `method`, `params`, `query`, `headers`, `body`, `text` and `cookies`. It also holds `get(header)`, which is the value of a mark of the request, `ip`, `host` and `protocol`.

The body of a request is read as JSON or as the fields of a form, so
`req.body.name` holds the name of a field of a form. A form of the kind
`multipart/form-data` is read as well: the file of a field is in
`req.files`, with its `filename`, its `contentType`, its `size` and its
`bytes`.

### The middle pieces of the server

A piece of the server is a function of the request, of the answer and of the
next one. It is put in the server with `use`, and it runs before the routes.

```vey
app.use(fn(req, res, next) {
  print("{req.method} {req.path}")
  next()
})

app.use("/admin", fn(req, res, next) {
  res.set("X-Admin", "yes")
  next()
})
```

The pieces that are ready are `logger()`, `cors()`, `compress()`,
`errorHandler(fn)` and `bodyLimit(bytes)`. The pieces `security()`,
`rateLimit(options)` and `csrf(options)` keep a browser from doing things it
should not, say how many requests a visitor may make, and check that a
request that changes something comes from the page. A route may have a piece
of its own, which is written after the handler of the route.

The answer of the server may set a cookie with `res.cookie(name, value)`, and
the sessions live from one request to the next.

The server takes some options:

```vey
let app = server(3000, {
  // The biggest body that is read. A body that is bigger is refused
  // before it is read.
  bodyLimit: 1024 * 1024,
  // Whether a proxy is in front of the server. The heads that a proxy
  // writes are only read when this is true, because a visitor may write
  // them as well.
  trustProxy: false,
  // Whether the reason of a mistake is told to the visitor. It may tell
  // something about the machine of the server, so it is not told unless
  // this is true.
  exposeErrors: false,
  // The time that a request may take, before it is refused.
  timeout: 30000,
  // The time that the requests that are running may take when the
  // server stops.
  shutdownTimeout: 10000
})
```

The files of a folder are served before the routes, so that a program may
put a file of its own in front of them. A part of a file may be asked for,
so that a big file may be played without being read whole:

```vey
app.static("public", { index: "index.html", maxAge: 3600 })
```

A mistake of the server itself is given to the program with
`app.onError(fn)`, and `app.lastError()` says whether there was one.

The work that is done when the program is told to stop is given with
`onShutdown(fn)`, so that the server may take the requests that are running
before it goes:

```vey
let app = server(3000)
onShutdown(fn() { app.stop() })
```

```vey
app.get("/login", fn(req, res) {
  session.start()
  session.set("user", "Ada")
  res.cookie("seen", "yes")
  return { ok: true }
})
```

The files that are served carry an `ETag`, a `Last-Modified` and a
`Cache-Control`, and they are not sent again when the visitor has them.

A socket of a visitor is answered with `ws`, and the events of the server
are sent with `sse`.

```vey
app.ws("/chat", fn(socket) {
  socket.onMessage(fn(text) {
    socket.send("echo: {text}")
  })
})

app.sse("/events", fn(req, stream) {
  stream.send("hello")
})
```

A `HEAD` is answered like a `GET` without the body of the answer, and an
`OPTIONS` is answered with the methods of the address. The body of a request
has a limit, and `stop()` waits for the requests that are running.

## Runtime

| Name | Description |
| --- | --- |
| `print(...)` | write to the console |
| `input(prompt)` | read a line from stdin |
| `len(value)` | string, list or object length |
| `str(value)` / `num(value)` / `int(value)` / `bool(value)` | the value as text, number or yes or no |
| `type(value)` | runtime type name |
| `append(list, ...items)` | new list with items added |
| `slice(value, start, end)` | slice a string or list |
| `join(list, sep)` / `split(text, sep)` | list and text conversion |
| `map`, `filter`, `reduce`, `find`, `sort` | list helpers |
| `flatMap(list, fn)` | map a list and flatten the result |
| `upper(text)`, `lower(text)`, `trim(text)` | the text in another shape |
| `replace(text, search, next)`, `includes(text, part)` | the parts of a text |
| `startsWith(text, prefix)`, `endsWith(text, suffix)` | the ends of a text |
| `text.capitalize`, `text.slug`, `text.camel`, `text.kebab` | the shape of a name |
| `text.padStart`, `text.padEnd`, `text.repeat`, `text.reverse`, `text.words` | the size of a text |
| `text.dedent(text)` | a text with the indent that all of its lines share taken away |
| `date.now()`, `date.iso()`, `date.format(value, pattern)` | the date and the time |
| `date.parts(value)`, `date.daysBetween(a, b)`, `date.addDays(value, days)` | the parts of a date |
| `query.parse(text)`, `query.stringify(object)` | the query of an address |
| `cookie.parse(header)`, `cookie.stringify(object)` | the cookies of a request |
| `json.parse`, `json.stringify` | JSON |
| `math.round`, `math.floor`, `math.random`, `math.pi` | math |
| `time.now()`, `time.iso()` | time |
| `http.get(url)`, `http.post(url, body)` | HTTP client, with the answer of `http.fetch` and the body already read |
| `sleep(ms)` | promise based delay |
| `html(node)` | render a component tree to HTML |
| `raw(html)` | mark trusted HTML so `html()` does not escape it |
| `bind(selector, node)` | mount into the DOM |
| `title(text)` | the title of the page, in the tab of the browser |
| `clipboard.write(text)` | put a text on the clipboard of the browser |
| `cls(...names)` | the class names of an element |
| `server(port)` | a server of the program |
| `assert(condition, message)` | throw when a condition is false |
| `assertEquals(actual, expected)` / `assertNotEquals(actual, expected)` / `assertDeepEquals(actual, expected)` | the matchers of the tests |
| `assertContains(text, part)` / `assertNotContains(text, part)` / `assertMatch(text, pattern)` / `assertThrows(fn)` | the matchers of the tests |
| `assertEquals(actual, expected, message)` | throw when the two values are not the same |
| `assertNotEquals(actual, expected, message)` | throw when the two values are the same |
| `assertDeepEquals(actual, expected, message)` | throw when the two values are not the same, deeply |
| `assertContains(text, part, message)` | throw when the text does not hold the part |
| `assertNotContains(text, part, message)` | throw when the text holds the part |
| `assertThrows(fn, message)` | throw when the code does not throw |
| `assertMatch(text, pattern, message)` | throw when the text does not match the pattern |
| `regex` | a pattern of a text, with its own match, replace and split |
| `intl` | the numbers, the dates and the names of a language |
| `bytes` | the bytes of a text, in base64 or in hexadecimal |
| `deepCopy` | a copy of a value that holds nothing of it |
| `signal` | a value that tells when it changes |
| `store` | a state that is shared by its name |
| `effect` | the work that follows a change of a value |
| `mount` | put a page in a node, and change it in place |
| `route` | a page of the browser, with its own path |
| `navigate` | go to another page of the browser |
| `currentRoute` | the page of the browser that is shown |
| `routeParams` | the names of the path of the page |
| `on` | listen to an event of a node |
| `delegate` | listen to an event of the children of a node |
| `onMount` | the work that runs when a page is shown |
| `onDestroy` | the work that runs when a page is taken away |
| `ref` | a name for a node of the page |
| `setTimeout` | run a piece of code after a while |
| `clearTimeout` | stop a piece of code that was waiting |
| `setInterval` | run a piece of code again and again |
| `clearInterval` | stop a piece of code that ran again and again |
| `requestAnimationFrame` | run a piece of code when the page is drawn |
| `cancelAnimationFrame` | stop a piece of code that waited for the page |
| `fetch` | ask a server for something |
| `Request` | a request that is sent |
| `Response` | the answer of a server |
| `Headers` | the names of a request or of an answer |
| `AbortController` | a way to end a request |
| `storage` | the storage of the browser, that stays |
| `sessionStorage` | the storage of the browser, that goes with the tab |
| `history` | the pages the browser has been at |
| `Blob` | a piece of binary data |
| `File` | a file that a visitor chooses |
| `FileReader` | read a file into the memory |
| `FormData` | the parts of a form that is sent |
| `WebSocket` | a connection that stays open |
| `EventSource` | the events of a server, as they come |
| `matchMedia` | the size of the screen of the visitor |
| `IntersectionObserver` | watch what is on the screen |
| `ResizeObserver` | watch the size of a node |
| `crypto` | the random numbers, the signatures and the keys of Node |
| `zlib` | the compression of Node |
| `logger` | a middle piece that writes down the requests |
| `cors` | a middle piece that lets other sites call the server |
| `compress` | a middle piece that makes the answers smaller |
| `errorHandler` | a middle piece that catches the mistakes |
| `bodyLimit` | a middle piece that says how big a request may be |
| `security` | a middle piece that writes the marks that keep a browser from doing things it should not |
| `rateLimit` | a middle piece that says how many requests a visitor may make |
| `csrf` | a middle piece that checks that a request that changes something comes from the page |
| `session` | the sessions of the server |
| `Map` | a map whose keys are not texts |
| `Set` | a set of the values it has seen once |
| `ok` | the answer of a function that worked |
| `err` | the answer of a function that failed |
| `todo` | throw, because the work is not finished |
| `unimplemented` | throw, because the work is not written |
| `ok(value)` | the answer of a function that worked |
| `err(error)` | the answer of a function that failed |
| `todo(message)` | throw, because the work is not finished |
| `unimplemented(message)` | throw, because the work is not written |
| `test(name, fn)` | run a test block |
| `error(message)` | throw an error |
| `all(list)` / `race(list)` | promise combinators |
| `env(name, fallback)` | environment variable, Node only |
| `require(name)` | a package of npm, Node only |
| `hash(value)` | the fingerprint of a value, Node only |
| `highlight(code)` | the code as HTML, with the parts of the language coloured |
| `process.cwd()`, `process.argv()`, `process.platform()`, `process.exit(code)` | the process, Node only |
| `uuid()` / `random(min, max)` / `clamp(value, min, max)` | identifiers and numbers |
| `sum(list)` / `unique(list)` / `reverse(list)` | list helpers |
| `Map()` / `Set()` | a map of many entries, and a set of them |
| `keys(object)`, `values(object)`, `entries(object)` | the parts of an object |
| `omit(object, keys)` | the object without the keys |
| `base64.encode(value)` / `base64.decode(value)` | base64 |
| `url.encode(value)` / `url.decode(value)` / `url.parse(value)` | URL helpers |
| `regex(pattern, flags?)` | a pattern that searches a text, with `test`, `match`, `matchAll`, `replace`, `replaceAll` and `split` |
| `intl.formatNumber(value, locale?)` / `intl.formatDate(value, options?)` / `intl.formatTime(value, options?)` | the way a number, a date and a time are written |
| `intl.dayName(index, locale?)` / `intl.monthName(index, locale?)` / `intl.relativeTime(value, unit, locale?)` | the names of the days and of the months |
| `bytes.from(text, encoding?)` / `bytes.toBase64(bytes)` / `bytes.fromBase64(text)` / `bytes.concat(list)` / `bytes.length(bytes)` / `bytes.hex(bytes)` | the bytes of a text, as a list of numbers |
| `text.encode(value)` / `text.decode(bytes)` | the bytes of a text |
| `deepCopy(value)` | a copy of a value that holds nothing of the value it came from |
| `signal(value, options?)` / `store(name, initial, options?)` / `effect(fn)` / `computed(fn)` | the state of a page, and the work that follows it |
| `Signal<T>` / `Store<T>` | the type of a signal and of a store, which hold the type of their value |
| `form(initial, options?)` | the values of a form with `values`, `errors`, `valid`, `dirty`, `set`, `replace`, `reset`, `check` and `submit` |
| `signal.peek()` / `signal.subscribe(fn)` / `signal.set(v)` / `signal.update(fn)` | reading and writing a value without listening to it, and listening to it |
| `store.set(key, value)` / `store.replace(state)` / `store.update(fn)` | changing one field of a state, the whole of it, or both |
| `deepEquals(a, b)` | whether two values have the same content |
| `mount(target, node)` | write a page in the browser and change it in place |
| `route(path, view)` / `navigate(url)` / `currentRoute()` / `routeParams()` | the routes of the browser |
| `on(target, event, fn)` / `delegate(target, event, selector, fn)` | the events of one's own |
| `onMount(fn)` / `onDestroy(fn)` / `ref()` | the life of a node, and a way to hold it |
| `transition(duration, options?)` | the `enter` and `leave` work of a node that comes in and goes out little by little |
| `Boundary` | the component that catches the mistakes of the ones under it |
| `provide(name, value)` / `inject(name)` | a value that the components under the one that gives it may read |
| `portal(content, target, key?)` | a part of the page that is written in another element, such as a dialogue or a menu |
| `setTimeout(fn, ms)` / `clearTimeout(id)` / `setInterval(fn, ms)` / `clearInterval(id)` | the timers of the browser |
| `requestAnimationFrame(fn)` / `cancelAnimationFrame(id)` | the frames of the browser |
| `http.fetch(url, options?)` / `http.Request` / `http.Response` / `http.Headers` / `http.AbortController` | a request of the browser, with a time and a retry |
| `storage.get(key)` / `storage.set(key, value)` / `storage.remove(key)` / `storage.clear()` / `storage.keys()` | the notes that stay after the browser is closed |
| `sessionStorage.get(key)` / `sessionStorage.set(key, value)` | the notes that stay until the tab is closed |
| `history.push(state, title, url)` / `history.replace(state, title, url)` / `history.back()` / `history.forward()` / `history.go(n)` / `history.state()` | the address of the page |
| `Blob(parts)` / `File(parts, name)` / `FileReader()` / `FormData()` | the files of a visitor |
| `WebSocket(url)` / `EventSource(url)` | the sockets of the browser |
| `matchMedia(query)` / `IntersectionObserver(fn)` / `ResizeObserver(fn)` | the screen of the browser |
| `fs.read`, `fs.write`, `fs.append`, `fs.exists`, `fs.list`, `fs.remove`, `fs.mkdir` | file system, Node only |
| `fs.readStream(path)` / `fs.writeStream(path, value)` | the files as they come, Node only |
| `path.join`, `path.dirname`, `path.basename`, `path.extname`, `path.resolve` | paths, Node only |
| `process.spawn(command, args?)` / `process.run(command, args?)` | another program, Node only |
| `crypto.randomBytes(count)` / `crypto.sign(value, key)` / `crypto.verify(value, signature, key)` / `crypto.keyPair()` | the bytes and the signatures, Node only |
| `zlib.gzip(value)` / `zlib.gunzip(bytes)` / `zlib.deflate(value)` / `zlib.inflate(bytes)` | the files that are smaller, Node only |
| `app.use(fn)` / `app.use(path, fn)` | the middle pieces of the server |
| `logger()` / `cors()` / `compress()` / `errorHandler(fn)` / `bodyLimit(bytes)` | the middle pieces that are ready |
| `security(options?)` / `rateLimit(options?)` / `csrf(options?)` | the marks that keep a browser from doing things it should not, the limit of the requests of a visitor and the check that a request comes from the page |
| `session.start()` / `session.regenerate()` / `session.get(key)` / `session.set(key, value)` / `session.end()` / `session.id()` | the sessions of the server |
| `onShutdown(fn)` | the work that is done when the program is told to stop |

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
  core.rs            the type representation
  rules.rs           the rules that relate two types
  builtins.rs        the signatures of the runtime helpers
  checker.rs         the checker that walks a program
  stmts.rs           the statements
  exprs.rs           the expressions
  patterns.rs        the patterns and the narrowing
  resolve.rs         the resolution of the types
  explain.rs         the type of an expression at a place
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
  cache.rs           the cache of the build
src/runtime.rs       joins the parts of the runtime
src/runtime/         the runtime shared by node and the browser
  index.js           the frame that creates the runtime
  shared.js          the helpers that the other parts share
  node.js            the file system, the paths, the environment and npm
  core.js            the console, the conversions and the tests
  text.js            the helpers for text
  collections.js     the helpers for lists and objects
  map.js             the maps and the sets
  enum.js            the values of the enums of the language
  data.js            JSON, math, time, base64 and URLs
  date.js            the date and the time
  web.js             the query of an address and the cookies
  regex.js           the pattern that searches a text
  intl.js            the languages and the zones
  bytes.js           the bytes of a text
  state.js           the state of a page
  dom.js             the markup and the browser
  mount.js           the difference between two trees
  events.js          the events of one's own
  history.js         the address of the page
  router.js          the routes of the browser
  timers.js          the timers of the browser
  storage.js         the notes of the browser
  files.js           the files of a visitor
  socket.js          the sockets of the browser
  observe.js         the screen of the browser
  http.js            the HTTP client
  highlight.js       the colouring of the code
  session.js         the sessions of the server
  middleware.js      the middle pieces of the server
  server.js          the server of a program
src/sourcemap/       the source map
  mod.rs             the map
  vlq.rs             the encoding of the map
src/cli/             the commands
  json.rs            the reading and the writing of JSON
  add.rs             adding a package of npm to a project
  args.rs            command line parsing
  config.rs          the settings of a project
  program.rs         project program discovery and configuration
  project.rs         project kind and entry-point handling
  run.rs             run, test and check
  build.rs           the build of the site
  site.rs            static sites
  dev.rs             development server
  scaffold.rs        new projects
  entry.rs           finding the entry file
  error.rs           build errors with a source frame
  output.rs          turning a bundle into runnable output
  fmt.rs             the shape of the files
  lint.rs            the smells of a program
  repl.rs            the prompt
  explain.rs         the type of an expression
  docs.rs            the documentation of a program
  migrate.rs         bringing a program to this version
  deploy.rs          building the site for a host
  lsp.rs             the language server
src/builtins.rs      the names the runtime provides
src/suggest.rs       suggestions for names that are close to a known one
src/warn.rs          the warnings of the compiler
tests/               integration tests
  lang.rs            the language
  types.rs           the type checker
  regressions.rs     bugs that used to give wrong results
  runtime.rs         the runtime
  server.rs          the http server
  cli.rs             the commands
  dom.rs             rendering in a browser
  dev.rs             the development server
  readme.rs          the examples of this file
```
