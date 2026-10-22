# Roadmap

What the language is missing, and the order in which the gaps are closed.

This file is written from the source in `src/`. Every item names the files
that hold the gap, so that the work can start at the place where the change
belongs. The items are grouped in the order of the phases at the end of the
file: the things that lie come first, then the type system, then the parts
of the language that are not there yet, then the surface, then the library,
then the two runtimes, then the tools.

A short note on the shape of the language today: the compiler is a single
binary, the runtime is small and readable, the types are optional and erased
at the end, and the markup has no closing tags. What is missing is mostly
what a language needs to *grow up* with: a type system that keeps its
promises, the parts of a language that a program reaches for every day, a
browser runtime that can hold state, a server with the middle pieces of a
web framework, and the tools that a team needs around a compiler.

---

The work is in the phases at the end of this file. The boxes are the state
of it: a box that is full is done, and the ones that are empty are what is
left. The parts that are done are in the history of the repository, one
commit for each phase.

## 0. The state of the language

The table is the state of the language before the work of this file
began. What is there now is in the history of the repository, one commit
for each phase, and the gaps that are left are in part 14.

| Part | Then | Now |
| --- | --- | --- |
| Lexer | 40 lines of operators, decimal numbers, `//` comments | the marks of the bitwise family, the numbers of four bases, the `_` between the digits, the escapes `\u` and `\x`, the block comments, the doc comments, the templates |
| Parser | expressions, statements, markup, types | the end of a statement, the tuples, the dictionaries, the interfaces, the `throw`, the `?`, the `defer`, the `yield`, the labelled loops, the patterns that take a value apart, the casts |
| Types | 15 type kinds, structural objects, generics on `fn` | `Promise`, `Result`, `Option`, `unknown`, the tuples, the dictionaries, the interfaces, the narrowing, the types that are guessed |
| Checker | names, scopes, a few ordering rules | the unused names, the shadowing, the unreachable code, the order of every name |
| Codegen | plain JavaScript, one bundle, one inline source map | the columns of the source map, the script of every page in a file of its own |
| Runtime (browser) | `h`, `html`, `raw`, `bind`, `cls`, `clipboard`, `title` | the state, the difference between two trees, the routes, the events, the timers, the fetch, the storage, the history, the observers |
| Runtime (server) | routes, a body, cookies, a static folder | the middle pieces, the sessions, the cookies of the answer, the cache of the files, the sockets, the limit of the body |
| Standard library | text, lists, dates, JSON, the server | the maps, the sets, the regex, the numbers of the world, the bytes, the deep copy |
| CLI | `run`, `dev`, `test`, `build`, `new`, `check` | `fmt`, `lint`, `repl`, `explain`, `docs`, `migrate`, `deploy`, `lsp`, `add` |

--- | --- | --- |
| Lexer | 40 lines of operators, decimal numbers, `//` comments | no `;`, no hex, no block comments, no escapes beyond four |
| Parser | expressions, statements, markup, types | no ASI, no tuples, no interfaces, no `throw` |
| Types | 15 type kinds, structural objects, generics on `fn` | `await` and every unannotated call fall back to `any` |
| Checker | names, scopes, a few ordering rules | no unused names, no shadowing, no unreachable code |
| Codegen | plain JavaScript, one bundle, one inline source map | the source map loses every closure, the columns are lost |
| Runtime (browser) | `h`, `html`, `raw`, `bind`, `cls`, `clipboard`, `title` | it renders once; there is no state and no update |
| Runtime (server) | routes, a body, cookies, a static folder | no middleware, no sessions, no caching, no sockets |
| Standard library | text, lists, dates, JSON, the server | no `Map`, no `Set`, no regex, no timers, no `fetch` |
| CLI | `run`, `dev`, `test`, `build`, `new`, `check` | no `fmt`, no `lint`, no `repl`, no language server |

---

## 1. Truth first: the places where the code lies

These are not missing features. They are places where the compiler, the
checker or the runtime says something that is not true, and they cost more
than a missing feature because the reader trusts them.

- [x] **The `ctx` helpers are assigned in the wrong place.** In four parts of
      the runtime the `ctx.*` lines ended up inside a function body instead
      of at the end of the part: `src/runtime/data.js:39-43` (inside
      `url.parse`), `src/runtime/http.js:17-25` (inside the `then` of the
      request), `src/runtime/server.js:159-161` (inside `makeRequest`) and
      `src/runtime/dom.js:42-51` (inside the branch of `h` without a
      document). Nothing reads them, so the effect is dead code with a
      misleading indentation. *Done when:* every `ctx.*` sits at the end of
      its part, and the indentation of the four files is straight.

- [x] **`vey build` returns 0 when a page fails.** The failures are printed
      to stderr but the exit code is still 0 (`src/cli/build.rs:47-67`), so a
      pipeline that builds a site never notices a broken page. *Done when:*
      the command returns 1 when a page could not be built.

- [x] **A `match` with a guard is called exhaustive when it is not.**
      The checker inserts the variant of an arm into the covered set even
      when the arm has a guard (`src/types/exprs.rs:296`), while the
      wildcard is only noticed when there is no guard
      (`src/types/exprs.rs:299-308`). A match whose only `Size.Large` arm is
      guarded passes the check and then falls through to `undefined` at
      runtime (`src/codegen/exprs.rs:391-393`). *Done when:* an arm with a
      guard does not cover its variant, and the checker says so.

- [x] **The source map loses every closure.** `capture()` throws the line
      table of the body away (`src/codegen/mod.rs:153-167`), so the code
      inside an arrow, a `fn` expression or a `match` body is not in the map.
      A runtime error there points at the wrong line, which breaks the
      promise of the README (line 170). *Done when:* the lines of a closure
      are in the map.

- [x] **The source map has no columns.** The generated column and the
      source column are always 0 (`src/sourcemap/mod.rs:35-38`), so a
      debugger can only point at a line. *Done when:* the caret of the error
      page lands on the word that is wrong.

- [x] **The pages of a `tests/` folder are built as pages of the site.**
      `is_ignored_dir` knows `tests` and `examples`
      (`src/cli/entry.rs:97-110`), but `collect_vey_files` does not
      (`src/cli/entry.rs:177-200`), so a project with its entry at the root
      tries to build `tests/*.vey` as a page. *Done when:* both walk the
      same list of folders.

- [x] **The `;` is a token that the parser never wants.** The lexer reads it
      (`src/lexer/mod.rs:241`) and the parser reports "unexpected token ';'"
      (`let x = 1;` fails). Either the semicolon is a separator or it is not
      a token. *Done when:* the two agree.

- [x] **The rules for the bitwise operators are dead.** The type checker
      knows `& | ^ << >>` (`src/types/exprs.rs:644-651`) but the lexer has
      no such marks (`src/lexer/mod.rs:75-78`), so `1 & 2` is a syntax
      error and the rules can never run. *Done when:* the marks exist or the
      rules are gone.

- [x] **`input()` writes to the ambient `process`.** The runtime is careful
      not to trust the ambient `process` (`src/runtime/index.js:14-17`) and
      asks for the module of Node instead, but `input` uses the global
      (`src/runtime/core.js:18`). *Done when:* it asks for the module.

- [x] **A generic call is parsed by copying the parser.**
      `src/parser/exprs.rs:200-215` clones the whole parser to try the
      generic form and restores it when it fails, so `a < b > (c)` may be
      read as a generic call. *Done when:* the parser looks ahead instead of
      copying itself.

- [x] **`src/cli/highlight.rs:36` keeps a flag that is thrown away**
      (`let _ = done;`). *Done when:* the flag says something or is gone.

- [x] **The enum of a module is described twice.** The checker keeps the
      variants of an enum (`src/types/checker.rs:125-189`) and the code
      generator rebuilds them (`src/codegen/js.rs:104-121`). *Done when:*
      there is one description.

---

## 2. The type system that keeps its promises

The types are optional and erased, which is the right shape for a small
language. The problem is not that they are optional, it is that a program
*with* types is not checked in many places, and the checker stays quiet
where it should speak.

### 2.1 The places where everything becomes `any`

- [x] **`await` has no type.** There is no `Promise` in the language
      (`src/types/builtins.rs:58`); the type of an `await` is `any`
      (`src/types/exprs.rs:371-374`). *Done when:* `await` has the type of
      what it waits for, and `Promise<T>` is a type of the language.

- [x] **A call to a function without types is not checked.** The arity and
      the arguments are only checked when the function has a type on a
      parameter or on the return (`src/types/exprs.rs:429-431` and
      `src/types/checker.rs:404`). The same happens to every call with a
      spread (`src/types/exprs.rs:429`). *Done when:* the arguments of
      every call are checked, and the types of a function are inferred when
      they are not written.

- [x] **A call on a union is not checked** (`src/types/exprs.rs:567`) and
      the optional call `?.()` is not checked at all
      (`src/types/exprs.rs:139-143`).

- [x] **A name that is not known is `any`, without an error**
      (`src/types/exprs.rs:32-35`), and the member of a module that is not
      known is `any`, without an error (`src/types/exprs.rs:666-679`). A
      typo in `ui.Card` or `math.PIe` is silent.

- [x] **A `let` without a type stays dynamic.** The language is gradual, so
      a value without a type may be anything later on. A value with a type
      is checked, and the type of a function is guessed from the way it is
      used. (`src/types/stmts.rs:113`) and the
      type of a `const` is only guessed for the simple shapes
      (`src/types/helpers.rs:186-197`).

- [x] **`and`, `or`, `+` and the bitwise marks do not check their
      operands** (`src/types/exprs.rs:117-127`, `:583-593`, `:644-651`).

- [x] **A spread of something that is not a list, or an object with a
      spread, throws the whole type away** (`src/types/exprs.rs:40-45`,
      `:72-87`).

- [x] **`any` fits everywhere**, in both directions: that is what makes the
      language gradual. The type that does *not* fit is `unknown`, which
      asks for the type to be checked before it is used.
      (`src/types/rules.rs:61-65`), so one `any` in a program turns the
      checking off for everything it touches.

### 2.2 The types that do not exist yet

- [x] **No interface, trait or protocol.** The only shapes are the object
      type and the alias (`src/ast/types.rs:5-15`).
- [x] **No tuple and no dictionary type.** `{[string]: number}` cannot be
      written (`src/parser/types.rs:40-127`).
- [x] **No `unknown`, no `object`, no `symbol`, no `bigint`.**
- [x] **A cast (`as`) and a check of a type (`is`) are there.** There is no
      `satisfies`, no `typeof`, no `keyof` or
      `infer` in a type.**
- [x] **No generic function expression.** The generics are on the `fn` of a
      declaration (`src/ast/decl.rs:63-79`), not on an arrow
      (`src/ast/expr.rs:88-89`).
- [x] **The constraint of a type alias is parsed and never checked**
      (`src/parser/mod.rs:411-445` against `src/types/resolve.rs:140-171`).
      The constraint of a function is checked
      (`src/types/exprs.rs:546-562`).

### 2.3 The checks that are too soft

- [x] **A condition is barely checked.** Only a number and a text are
      refused (`src/types/stmts.rs:326-334`); a list, an object or a
      function pass as a condition. *Done when:* a condition must be a
      boolean.

- [x] **`repeat n { }` does not check that `n` is a number**
      (`src/types/stmts.rs:254-257`).

- [x] **An object with a field too much is accepted**
      (`src/types/rules.rs:101-120` only walks the fields of the target).

- [x] **The narrowing knows two shapes only:** `x == null` and
      `type(x) == "kind"` (`src/types/patterns.rs:13-82`). The truth of a
      value (`if user { }`), `!= undefined` and `instanceof` do not narrow.
      The kind `undefined` cannot be narrowed at all
      (`src/types/core.rs:253-285`).

- [x] **The exhaustiveness of a `match` is only checked for the enums of
      the program.** A `match` on `number | string` is not checked
      (`src/types/exprs.rs:327-364`).

- [x] **The enums of another module are not checked at all.** The
      environment of a module keeps no enum
      (`src/types/checker.rs:26-35`), so `t.Status.Nope` is not reported
      and a `match` on a foreign enum is not checked for exhaustiveness.
      The README (line 414) says the fields are checked.

- [x] **The arguments of a component are checked only when the component has
      no `children` and no rest parameter** (`src/types/stmts.rs:517-532`),
      and the arguments in excess are ignored
      (`src/types/stmts.rs:506-509`).

- [x] **A type that is recursive is loose in both directions**
      (`src/types/rules.rs:61-63`), so the open end of
      `type Node = { value: number, next: Node | null }` takes anything.

- [x] **A type variable that cannot be bound becomes `any` without a word**
      (`src/types/rules.rs:192-233`).

---

## 3. The checker of the names

- [x] **No unused names.** Nothing in the program reports a name that is
      written and never read: no unused variable, no unused import, no
      unused parameter, no unused `catch` binding.
- [x] **No shadowing.** A name may hide another name in the same block
      without a word (`src/check/scopes.rs:81-86`).
- [x] **No unreachable code.** The code after a `return`, a `break` or a
      `continue` is not reported.
- [x] **The order of the names is only checked for `let` and `const` at the
      top level** (`src/check/mod.rs:101-106`, `:354-364`); a function or a
      component may be used before it is written.
- [x] **The names of a module are not checked.** `ui.Card` and `math.PIe`
      are only checked as an object and an index
      (`src/check/mod.rs:415-420`).
- [x] **A `break` outside a loop is not found in every case.** The counter
      is reset for every function (`src/check/mod.rs:243-247`), so a
      `break` inside a `match` inside a loop passes and becomes a mistake
      in the emitted JavaScript.
- [x] **The types and the enums are not names for the checker**
      (`src/check/mod.rs:222-223`), so two types with the same name are
      only caught by the type checker.

---

## 4. The parts of the language that are not there yet

These are not mistakes; they are the parts of a language that a program
reaches for every day and that Vey does not have. Each of them is small on
its own and large in the life of a program.

### 4.1 The shapes of a program

- [x] **There is no `Map` and no `Set`.** The only shapes are the list and
      the object (`src/ast/expr.rs`), so the keys of a map are always
      texts and a set is an object with `true` in it. *Done when:* the
      runtime has `Map` and `Set` with the same shape as the language.

- [x] **There is no `Result` and no `Option`.** A failure is only an
      exception (`src/runtime/core.js:82-87`), so a function that may fail
      cannot say so in its type, and every caller has to know the shape of
      the answer. *Done when:* `Result<T, E>` and `Option<T>` are types of
      the language, with `?` to hand the failure on.

- [x] **There is no `yield`, so there are no iterators.** A function that
      hands back values one by one has to build a list first
      (`src/ast/expr.rs`), so a file of a million lines is read into the
      memory before the first line is used. *Done when:* `yield` writes a
      generator and `for x in generator` reads it.

- [x] **There is no `defer`.** A program that opens a file, a lock or a
      connection has no way to close it when the block is left
      (`src/ast/stmt.rs`), so the cleanup is written by hand at every exit.

- [x] **There is no `pub` and no `priv`.** Everything a module exports is
      for everyone (`src/ast/decl.rs`), and `export` is all or nothing: a
      library cannot keep a helper of its own to itself.

- [x] **There are no methods.** A type has no fields and no functions of its
      own; the helpers are free functions in a namespace
      (`src/runtime/collections.rs`). A program of many types has a file of
      free functions.

- [x] **There is no `if let` and no `while let`.** The `match` does the
      work, but the two forms that read well when there is one case are not
      there.

- [x] **There is no `let else`.** A value that must be there is written as
      a `match` with an arm that fails.

- [x] **The `for` has no step and does not walk the keys of an object**
      (`src/parser/mod.rs`). There is no `for i in 0..10 by 2`.

- [x] **There are no labelled loops**, so a `break` in a loop inside a loop
      breaks the inner one only, and there is no way to say otherwise.

### 4.2 The words of a program

- [x] **There are no doc comments.** `///` is not a comment
      (`src/lexer/mod.rs:167-174` reads only `//`), so the docs of a
      library live somewhere else than the code.

- [x] **There is no version of the language in a file.** A file that is
      written for one compiler may be read by another without a word; there
      is no `// vey: 0.7` that says what it was written for.

- [x] **There are no warnings.** The compiler speaks only when something is
      wrong. A name that is never used, a branch that is never taken, a
      value that is always the same and a `catch` that is empty are all
      silent.

- [x] **There is no `todo` and no `unimplemented` in the language.** A
      piece of work that is not finished is a comment today.

---

## 5. The surface of the language

### 5.1 The lexer

- [x] **The numbers are decimal only** (`src/lexer/mod.rs:257-312`): no
      `0x`, `0b`, `0o`, no `_` between the digits, no `n` at the end, no
      `.5` at the start.
- [x] **The escapes are four:** `\n`, `\t`, `\r`, `\0`
      (`src/lexer/strings.rs:36-43`). `\uXXXX`, `\xNN`, `\'` and `\"` are
      not read; `\u` becomes a `u`.
- [x] **There are no block comments** outside a `style { }` block
      (`src/lexer/strings.rs:117-125`).
- [x] **There is no `!`, no `&`, no `^`, no `<<`, no `>>`** in the marks
      (`src/lexer/mod.rs:75-78`). `not` is there, the bitwise family is not.
- [x] **There is no regex and no template string.**

### 5.2 The parser

- [x] **There is no end of a statement.** The newlines are plain spaces
      (`src/lexer/mod.rs:159-178`), so `let a = 1` under a line that begins
      with `-2` is `1 - 2`, and `f(x)` under `(y)` is `f(x)(y)`. *Done
      when:* the end of a line ends a statement, or the semicolon is
      wanted.

- [x] **The assignment marks are few:** `= += -= *= /= %=`
      (`src/parser/mod.rs:17`). There is no `**=`, `&&=`, `||=`, `??=`.

- [x] **There is no `throw`** (only the `error()` helper), no `switch`, no
      labelled `break`/`continue`, no `do while`, no class, no struct, no
      `impl`, no decorator, no macro.

- [x] **The patterns of a `match` are few** (`src/ast/expr.rs:21-37`,
      `src/parser/patterns.rs:9-124`): a value, a range, a name, a variant.
      There is no list, no object, no prefix of a text, no `as`.

- [x] **The markup knows four things:** an element, an expression, an `if`
      and a `for` (`src/ast/markup.rs:6-12`). There is no `match` in the
      markup, no `key`, no slot, no spread of the children, no index in a
      `for`, and a `<` in the text is a mistake. One `id` per element.

- [x] **The type of an arrow function cannot be written**
      (`src/parser/exprs.rs:355-415` reads only the types of the
      parameters).

- [x] **The named arguments live only in the markup**; `f(a=1)` is a
      mistake anywhere else (`src/parser/exprs.rs:262-268`).

- [x] **The `async` keyword is refused** (`src/parser/mod.rs:140-150`),
      which is right, but nothing says so in the language itself.

### 5.3 The shape of the types

- [x] **No tuple, no dictionary, no interface** (see 2.2).

---

## 6. The compiler and the code generator

- [x] **The lines of the closures are lost** (see 1).
- [x] **The `await` is found by looking at the text.** A default value with
      an `await` in it (`src/codegen/mod.rs:191-194`,
      `src/codegen/awaits.rs:5-41`), a `match` with an `await` in the guard
      or in the subject (`src/codegen/exprs.rs:399-403`) and the defaults of
      the parameters of an arrow (`src/codegen/exprs.rs:166-169`) are not
      seen, so the emitted function is not `async` and the JavaScript is a
      mistake.

- [x] **The events are twelve** (`src/codegen/js.rs:44-57`): there is no
      `keypress`, `wheel`, `pointer*`, `touch*`, `drag*`, `contextmenu`,
      no events of one's own and no delegation.

- [x] **The attributes are thin.** The `class` and the `id` are put
      together by hand (`src/codegen/markup.rs:59-87`); there is no list of
      the marks that are true when they are there, no care for `data-*` and
      `aria-*`, and no difference between a `style` that is a text and a
      `style` that is an object.

- [x] **The lists of the markup have no key.** A `for` becomes a
      `V.flatMap` (`src/codegen/markup.rs:32-39`), so the items of a list
      have no name of their own.

- [x] **The `use` of a module is lifted above everything**
      (`src/codegen/mod.rs:129-139`), even when the parser allows a `use`
      inside a block.

- [x] **A `match` that is not exhaustive gives `undefined`**
      (`src/codegen/exprs.rs:391-393`).

- [x] **A name of the program is only made safe with a `_`**; a variable
      called `json` hides the runtime for the whole scope.

- [x] **There is no cache of the build.** Every build reads every module and
      compiles it again (`src/compiler/mod.rs:48-120`), and the modules are
      compiled one after the other. *Done when:* the modules that did not
      change are taken from `.vey/cache`, and the modules are compiled in
      parallel.

---

## 7. The library that a program needs

The runtime is small on purpose. What is missing here is not a lot of
helpers, it is the handful of helpers that every program reaches for in its
first week.

### 7.1 The common part

- [x] **No `Map` and no `Set`** in the runtime either
      (`src/runtime/collections.js`).
- [x] **No regex.** A text is searched with `includes` and cut with
      `split` (`src/runtime/text.js`); there is no `match`, no `replaceAll`,
      no `at`, no `trimStart`/`trimEnd`.
- [x] **No `Intl`.** A number and a date are written the way JavaScript
      writes them (`src/runtime/date.js:40-57` knows six marks of a date);
      there is no language, no zone, no name of a day.
- [x] **No binary.** There is no `Buffer` and no `Uint8Array` helper; the
      base64 of the runtime (`src/runtime/data.js`) is all there is.
- [x] **No `TextEncoder` and no `TextDecoder`** for the bytes of a text.
- [x] **No `structuredClone` and no deep copy**, so a copy of an object
      shares what is inside it.

### 7.2 The browser

- [x] **No `fetch` with a `Request`, a `Response` and `Headers`.** There
      are `http.get` and `http.post` (`src/runtime/http.js:30-44`), so the
      body, the marks and the time of a request are all out of reach.
- [x] **No way to end a request** (`AbortController`), no time, no retry.
- [x] **No `URL` object**, only `url.parse` with a plain object
      (`src/runtime/data.js`).
- [x] **No `Blob`, no `File`, no `FileReader`, no `FormData`**, so a file
      that a visitor chooses never reaches the server.
- [x] **No `WebSocket` and no `EventSource`** on the client.
- [x] **No `localStorage` and no `sessionStorage`.**
- [x] **No `history`**, so a page of a single-page program cannot change
      the address of the browser.
- [x] **No timers.** There is `sleep(ms)` (`src/runtime/core.js:35-37`),
      which waits; there is no `setTimeout`, no `setInterval` and no
      `requestAnimationFrame`, so a page cannot draw, poll or wait for
      anything.
- [x] **No `matchMedia`, no `IntersectionObserver`, no `ResizeObserver`,**
      so a page cannot react to the size of the screen.

### 7.3 The server

- [x] **No `child_process`.** A program cannot run another program
      (`src/runtime/node.js`), so a build script, a watcher of a process and
      a call to `git` are all outside the language.
- [x] **No streams.** A file is read whole into the memory
      (`src/runtime/node.js:68-70`).
- [x] **No `crypto`.** There is `hash` and `uuid`
      (`src/runtime/node.js:41-51`), but no random bytes, no signature, no
      key.
- [x] **No `zlib`**, so a file that is served is not compressed.

---

## 8. The runtime of the browser

The runtime of the browser is seven helpers: `h`, `html`, `raw`, `bind`,
`cls`, `clipboard.write` and `title`. A page is written once and never
changes. These are the pieces that a page needs to live.

- [x] **There is no state.** `bind()` writes the children of its target
      once (`src/runtime/dom.js:156-168`); there is no signal, no store, no
      effect, no way to say that a value has changed and to have the page
      follow.
- [x] **There is no difference.** `h()` builds the real nodes at once
      (`src/runtime/dom.js:35-40`) and `html()` builds a text. Nothing
      compares two trees, so there is no update of a list, no order that
      changes, no `key`.
- [x] **There is no route in the browser.** The only matcher is on the
      server (`src/runtime/server.js:10-38`). There is no history, no
      parameters of a route in the page, no layouts that hold each other.
- [x] **There is no water.** The page of the server is not taken over by
      the page of the browser; `bind()` would replace all of it.
- [x] **There are no forms.** The server reads a form that is
      `application/x-www-form-urlencoded` or JSON
      (`src/runtime/server.js:84-102`); `multipart/form-data` is not read
      at all. There is no state of a form, no check, no file.
- [x] **There is no event system** beyond the twelve marks of the markup:
      no delegation, no events of one's own, no `onMount`, no `onDestroy`,
      no reference to a node.

---

## 9. The runtime of the server

The server has nine methods (`src/runtime/server.js:237-268`). The middle
of a web framework is missing.

- [x] **There is no middle piece.** No `app.use(...)`, no middle piece that
      catches the mistakes, no log of the requests, no CORS, no
      compression, no limit of the requests.
- [x] **There are no sessions and no way to set a cookie.** The `cookie`
      helper reads and writes a text (`src/runtime/web.js:42-69`), but the
      answer of the server has no `Set-Cookie`.
- [x] **The files that are served have no cache.** `serveStatic` sets the
      kind of the file and nothing else (`src/runtime/server.js:133-154`):
      no `ETag`, no `Last-Modified`, no `Cache-Control`.
- [x] **There are no sockets and no events from the server** (no
      WebSocket, no SSE), no HTTPS, no `HEAD`, no `OPTIONS`.
- [x] **The body of a request has no limit**; it is all read into the
      memory (`src/runtime/server.js:84-102`).
- [x] **The request is thin** (`src/runtime/server.js:163-172`): no
      `req.get(header)`, no `req.ip`, no `req.host`, no `req.protocol`.
- [x] **The routes are simple** (`src/runtime/server.js:10-38`): a segment
      that is the same or a `{param}`, and a `*` at the end. No optional
      parameter, no constraint, no middle piece of its own, no order.
- [x] **The server does not stop well.** There is a `stop()`
      (`src/runtime/server.js:255-267`), but a request that is running is
      not waited for.

---

## 10. The tools

- [x] **There is no `vey fmt` and no `vey lint`.**
- [x] **There is no `vey repl`.** A language that is learned in a minute
      should be tried in a minute; today a program is written in a file,
      built and run.
- [x] **There is no `vey explain`.** There is no way to ask the compiler
      what the type of an expression is.
- [x] **The tests may be watched,** and `dev` watches by asking the
      files every 300 ms (`src/cli/dev.rs:143-159`, `:518-585`) instead of
      waiting for the system to say that something changed.
- [x] **The reload of the page is a script that is put in every page**
      (`src/cli/dev.rs:404-413`); there is no socket, and the CSS is not
      reloaded on its own.
- [x] **The tests cannot be chosen** (`src/cli/run.rs:46-132`): no filter,
      no name, no watch, no measure of the cover. There is `assert` and
      nothing else: no matchers, no mocks, no files of fixtures, no
      pictures of a result.
- [x] **There is no `vey docs`, no `vey deploy`, no language server, no
      `vey add` for a package.**
- [x] **There is no `--help` for a command.** `vey build --help` says
      "unknown option" (`src/cli/args.rs:54-56`).
- [x] **`--force` is read by every command and used by one**
      (`src/cli/args.rs:40`, `src/cli/scaffold.rs:36`).
- [x] **The help does not name `--force`, the `--host` of `dev`, or the
      entry of `run` and `dev`** (`src/cli/output.rs:142-175`).
- [x] **The bundle is one file with everything.** There is no shaking of the
      trees, no minifier and no split of the code that is only used on one
      page (`src/compiler/mod.rs:120-172`).
- [x] **There is no migration of the old programs.** When the language
      changes, there is no `vey migrate` that takes a program of the older
      version to the newer one.
- [x] **The scaffold has one shape only** (`src/cli/scaffold.rs`); there is
      no template for a program with a server, for a library or for a
      single page.
- [x] **There is a recipe for the continuous integration;** the bench is
      still missing.
      a new project has no Dockerfile and no workflow.

---

## 11. The words of the mistakes

- [x] **The suggestion is too near** for a short name: the limit is
      `max(len / 3, 2)` (`src/suggest.rs:30`), so for a name of two or three
      letters almost everything is close enough.
- [x] **Only the distance of Levenshtein is used** (`src/suggest.rs:4-50`):
      the two letters that are swapped are not seen, and the case is not
      ignored.
- [x] **Only an unknown name and an unknown type get a suggestion**
      (`src/check/mod.rs:349`, `src/types/resolve.rs:97`); a member, a
      component or a variant that is not known get nothing.

---

## 12. A program of Vey cannot do these things today

The short list that the rest of this file is made of.

```vey
// A map of many entries, with keys that are not texts.
let seen = Map()
seen.set(user.id, user)

// A function that may fail, said so in its type.
fn readConfig(path: string) -> Result<Config, Error>

// A file that is read line by line, not all at once.
fn lines(path: string) -> Iterator<string>

// A file that is closed when the block is over.
let file = open("data.txt")
defer file.close()

// A type with its own functions.
impl Config {
  fn port(): number { return 8080 }
}

// A helper of a library that the outside world cannot see.
fn parseHeader(text: string): Header

// A warning that a name is never read.
let unused = 1
```

---

## 13. The road

The phases are in the order in which they should be done. Each phase ends
with something that a reader of the language can use.

### Phase 0 — Say what is true  *(the small fixes, one afternoon)*

The items of the first part. Nothing here is a feature; after it, the
compiler and the runtime stop saying things that are not so.

- [x] The `ctx` helpers of the four parts of the runtime.
- [x] The exit code of `vey build`.
- [x] The exhaustiveness of a `match` with a guard.
- [x] The lines of the closures in the source map.
- [x] The `tests/` folder in the pages of a site.
- [x] The `;` and the dead rules of the bitwise marks.

### Phase 1 — A type system that speaks  *(the heart of the language)*

- [x] `Promise<T>` and the type of an `await`.
- [x] The check of the calls, also for the functions without types.
- [x] The names of the modules, the members of a module, the variants of a
      foreign enum.
- [x] The narrowing: the truth of a value, `undefined`, `instanceof`.
- [x] The arguments of a component, in every case.
- [x] The condition must be a boolean.
- [x] The unused names and the shadowing.
- [x] The tuples, the dictionaries and the interfaces.

### Phase 2 — The parts of the language  *(what a program reaches for)*

- [x] `Map` and `Set`.
- [x] `Result` and `Option`, with `?`.
- [x] `yield` and the iterators.
- [x] `defer`.
- [x] `pub` and `priv`.
- [x] The methods of a type (`impl`).
- [x] `if let`, `while let` and `let else`.
- [x] The doc comments and the version of the language in a file.
- [x] The warnings.

### Phase 3 — The surface of the language  *(what is written every day)*

- [x] The end of a statement (the semicolon or the end of the line).
- [x] The numbers of the other bases, the `_` between the digits.
- [x] The escapes `\u` and `\x`, the block comments.
- [x] The missing marks: `!`, `&`, `|`, `^`, `<<`, `>>`, `**=`, `??=`.
- [x] The patterns of a `match`: the lists, the objects, the prefixes.
- [x] The markup: `key`, the slots, the `match`, the `<` in the text.
- [x] The `throw`, the labelled `break`.

### Phase 4 — A library that a program can lean on  *(the runtime)*

- [x] The timers of the browser: `setTimeout`, `setInterval`,
      `requestAnimationFrame`.
- [x] The `fetch` of the browser with `Request`, `Response` and `Headers`,
      and a way to end it.
- [x] `localStorage`, the `history` of the browser.
- [x] The `Blob`, the `File` and the `FormData` of the browser, and the
      `multipart` of the server.
- [x] `child_process` and the streams of Node, `crypto`, `zlib`.
- [x] The regex and the `Intl` of the common part.
- [x] The deep copy and the `TextEncoder`.

### Phase 5 — A page that lives  *(the runtime of the browser)*

- [x] The state: a signal, a store, and the page that follows.
- [x] The difference between two trees, so that a list may change.
- [x] The routes in the browser.
- [x] The events of one's own, the delegation, the life of a node.
- [x] The water between the page of the server and the page of the browser.

### Phase 6 — A server with the middle pieces  *(the runtime of the server)*

- [x] The middle pieces, the log of the requests, the CORS.
- [x] The sessions and the cookies of the answer.
- [x] The cache of the files that are served.
- [x] The sockets and the events from the server.
- [x] The limit of the body of a request.
- [x] The routes with their own middle piece and their own order.

### Phase 7 — The tools around the compiler  *(the work of a team)*

- [x] `vey fmt` and `vey lint`.
- [x] `vey repl` and `vey explain`.
- [x] The cache of the build and the modules in parallel.
- [x] The watch of the files by the system, not by a question every 300 ms.
- [x] The filter of the tests, the matchers and the measure of the cover.
- [x] The language server.
- [x] `vey docs`, `vey deploy` and `vey migrate`.
- [x] The templates of the new projects, the bench and the recipe of the
      continuous integration.

---

## 14. The gaps that the code still has

This part was written after reading the whole of `src/` from the top to the
bottom, once the work of the phases above was done. Every item says where
the gap lives, so that the work can start there. A box that is full is a
gap that is closed.

### 14.1 The compiler

- [x] **The `await` finder did not look inside a `throw`.** A `throw await
      f()` put an `await` in a function that was not async, and the whole
      bundle was not valid JavaScript. (`src/codegen/awaits.rs`)
- [x] **The `await` finder did not look at the guard of a `match` arm.**
      (`src/codegen/awaits.rs`)
- [x] **A component whose markup waits was not async.** The markup may hold
      any expression, so `{await load()}` broke the bundle.
      (`src/codegen/mod.rs`, `src/codegen/awaits.rs`)
- [x] **The `defer` of a block ran when the function was left, and not when
      the block was left.** The README promises the block.
      (`src/codegen/mod.rs`)
- [x] **The cache of the build did not notice that the compiler itself had
      changed,** so a new compiler read the cache of the old one and wrote
      the old code. (`src/compiler/cache.rs`)
- [x] **The constructor of an `impl` block was not exported,** so the users
      of a library could not make a value of its type. (`src/compiler/loader.rs`,
      `src/types/checker.rs`)
- [x] **A `let ... else` throws the destructuring away.** Every name of the
      pattern is bound to the whole value. (`src/codegen/mod.rs`)
- [x] **The names of a list or an object pattern of an `if let` or a
      `while let` are never declared,** so the program is refused with
      "unknown variable". (`src/check/mod.rs`, `src/types/stmts.rs`)
- [x] **The names of a list or an object pattern of a `match` are never
      declared,** so they are of the type `any`.
      (`src/types/exprs.rs`, `src/types/stmts.rs`)
- [x] **The names of a list or an object pattern of a `match` are not in the
      scope of the code generator,** so a name that hides a helper of the
      runtime is not the one that is bound. (`src/codegen/exprs.rs`)
- [x] **The arguments that are given by name are not handled in the path of
      a call with type arguments,** so the marker of the named arguments is
      counted as an argument. (`src/types/exprs.rs`, `src/codegen/exprs.rs`)
- [x] **A `match` on an enum of another module binds the wrong field,** so
      the value of the field is `undefined`. (`src/codegen/exprs.rs`)
- [x] **A `for ... by` with a destructuring pattern writes JavaScript that
      cannot be parsed,** and a negative step never ends.
      (`src/codegen/mod.rs`)
- [x] **A broken interpolation becomes literal text** instead of an error.
      (`src/parser/strings.rs`)
- [x] **Every error of the code generator points at line 1, column 1.**
      (`src/codegen/mod.rs`)
- [x] **The `vey explain` walker skips the casts, the functions and the
      markup,** so it answers "there is no expression at that place".
      (`src/types/explain.rs`)
- [x] **The guard of a `match` in the markup is tried before the names it
      uses exist.** (`src/codegen/markup.rs`)
- [x] **A `defer` at the top level of a file is taken,** although the error
      that says it is not allowed can never fire. (`src/codegen/mod.rs`)
- [x] **The wildcard pattern has no place,** so an error in it is reported
      at the top of the file. (`src/ast/expr.rs`, `src/parser/patterns.rs`)
- [x] **Two copies of "does this piece of code yield?" disagree,** so a
      function that yields inside a `throw` is typed as a generator and
      written as an ordinary one. (`src/codegen/awaits.rs`,
      `src/types/helpers.rs`)
- [x] **The helper that looks for a `defer` inside an expression always
      says no,** so the next person who writes one gets nothing.
      (`src/codegen/awaits.rs`)
- [ ] **The checker says a type of another module may be implemented, but
      only the types of the program itself are known.** (`src/types/checker.rs`)
- [x] **The comment about the order of the names is old:** the names of the
      functions are now checked like the names of the values.
      (`src/check/mod.rs`)
- [x] **The branch that reads `() => ...` can never be reached.**
      (`src/parser/exprs.rs`)
- [x] **A `?` at the top level of a module ends the program without a
      word.** (`src/codegen/mod.rs`)
- [x] **A `try` of the program catches the failure of `?` as a wrapper
      error,** so the reason of the failure is lost and the failure is not
      handed on. (`src/codegen/mod.rs`, `src/runtime/core.js`)
- [x] **The line count of a wrapped module is computed and thrown away.**
      (`src/compiler/mod.rs`)
- [x] **`pub let { a } = x` exports nothing, without a word.**
      (`src/parser/mod.rs`)
- [ ] **Nothing is shaken out of the bundle:** every export of every module
      is written, whether it is used or not. (`src/compiler/loader.rs`)

### 14.2 The type checker

- [x] **A value of an enum could not be compared with one of its
      variants.** (`src/types/rules.rs`)
- [x] **A parameter that was compared with `null` was given the type
      `null`,** so every call to it was refused. (`src/types/helpers.rs`)
- [x] **A `throw` was not an exit in every place,** so a function that
      only throws was said not to return. (`src/types/helpers.rs`)
- [x] **The enum of another module was looked for in an order that
      changed from one run to the next,** so the same program could be
      checked two ways. (`src/types/exprs.rs`)
- [x] **Every member of an interface was `any`:** an interface was not
      checked at all. (`src/types/exprs.rs`)
- [x] **The tuple type could not be used:** a list of values is never a
      tuple, and a tuple does not fit a list. (`src/types/rules.rs`,
      `src/types/exprs.rs`)
- [x] **`Promise<T>` and `Result<T, E>` are not in the rule that says
      whether a value fits,** and the helpers that hand a promise back are
      typed `any`. (`src/types/rules.rs`, `src/types/builtins.rs`)
- [x] **`object` does not take a list, a dictionary, an enum or a
      function.** (`src/types/rules.rs`)
- [ ] **The recursive part of a type is only opened up in one of the three
      places,** so an assignment and a call are not checked like a `let`.
      (`src/types/stmts.rs`, `src/types/exprs.rs`)
- [ ] **The type of a parameter that is used inside another function is
      lost or given to the wrong parameter.** (`src/types/stmts.rs`)
- [x] **A type parameter is not assignable to anything,** so a value of a
      known type cannot be given to a variable of the type parameter.
      (`src/types/rules.rs`)
- [x] **`is` does not narrow,** so the type of the value is not known in
      the branch. (`src/types/exprs.rs`)
- [x] **The exhaustiveness of a `match` on a `bool` is not checked.**
      (`src/types/exprs.rs`)
- [x] **The literal patterns of a `match` are not compared with the type
      of the subject,** so a pattern of the wrong type is taken.
      (`src/types/exprs.rs`)
- [x] **The bindings of an enum pattern of an `if let` get the type of the
      whole subject** and not the type of the field. (`src/types/stmts.rs`)
- [ ] **A function written as an expression ignores the types that were
      guessed,** so the same code is checked in one form and not in the
      other. (`src/types/checker.rs`, `src/types/stmts.rs`)
- [ ] **A type argument that is given by hand is dropped when the type
      parameter is not used,** so the constraint of the parameter is never
      checked. (`src/types/exprs.rs`)
- [ ] **The substitutions throw away the names of the parameters,** so a
      generic function cannot take its arguments by name.
      (`src/types/rules.rs`)
- [ ] **An interface is missing from the substitution, the unification and
      the erasure.** (`src/types/rules.rs`)
- [x] **The type parameters of an interface are parsed and then thrown
      away,** so an interface with a type parameter does not work.
      (`src/types/checker.rs`, `src/types/resolve.rs`)
- [x] **The methods of an `impl` block are not known to the other
      modules.** (`src/types/checker.rs`, `src/types/exprs.rs`)
- [ ] **The types that are guessed for a parameter are put together
      instead of being held to,** so a parameter that is used in two ways
      takes the union of the two. (`src/types/stmts.rs`)
- [x] **`??` does not take `undefined` out of the type.**
      (`src/types/core.rs`)
- [x] **The condition of a ternary is not checked to be a bool.**
      (`src/types/exprs.rs`)
- [x] **`+` never fails:** a bool and a number are added without a word.
      (`src/types/exprs.rs`)
- [x] **A function of no arguments satisfies a type of one argument.**
      (`src/types/rules.rs`)
- [x] **A parameter that comes after a parameter with a value is taken as
      optional.** (`src/types/checker.rs`)
- [ ] **The opening of a recursive type grows with the number of its
      fields,** so a type with six of them is opened six times over.
      (`src/types/checker.rs`)
- [x] **The name of an import wins over a local name of its own.**
      (`src/types/exprs.rs`)
- [ ] **A narrowing inside a closure changes the type of the variable of
      the function around it.** (`src/types/checker.rs`)
- [x] **The step of a `for` loop is never checked.** (`src/types/stmts.rs`)
- [x] **A generator without a return type has the type `() -> [void]`.**
      (`src/types/stmts.rs`)
- [x] **`vey explain` gives the type `any` for a name inside a function,**
      because the checker of the explanation does not walk into the
      functions. (`src/types/explain.rs`)
- [x] **`unknown` fits nothing but `any` and `object`,** so it cannot be
      used as the type of a value that is not known yet.
      (`src/types/rules.rs`)
- [x] **The type of the values of a dictionary is not used** for
      `d["key"]`. (`src/types/exprs.rs`)
- [x] **The parts of a tuple are not taken apart by their place,** so every
      name of the tuple has the union of all of them.
      (`src/types/patterns.rs`)
- [x] **The value that a name of a `let` has by default is not checked**
      against the type of the name. (`src/types/patterns.rs`)
- [x] **An `await` of a union is `any`.** (`src/types/exprs.rs`)
- [ ] **The smaller gaps:** `undefined` is not a helper of the language,
      `symbol` and `bigint` have no members, the name of a `catch` is
      always `any`, a `while` does not narrow, a comparison of a member
      does not narrow, a dictionary and a tuple cannot be walked through,
      `&&` and `||` are typed `bool` and not the value, an unknown operator
      is typed `any`, and a `yield` inside a cast is not seen.

### 14.3 The runtime and the tools

- [x] **The types of the helper objects did not have the members that the
      runtime has,** so `http.fetch`, `fs.readStream`, `process.spawn`,
      `text.encode` and `Map().toList()` were type errors although the
      README writes them. (`src/types/builtins.rs`)
- [x] **The signatures of the helpers did not match the runtime:** the
      marks of a regex, the name of a store, the options of a server, the
      arguments of the history, the options of a blob and of a file, the
      options of a time, the letter of a padding, the first value of a
      signal and the element that `bind` gives back were all wrong.
      (`src/types/builtins.rs`)
- [x] **The warning that a name hides a helper of the runtime was said for
      every parameter,** so it was said for a name that is used most of the
      time. (`src/check/mod.rs`)
- [x] **The middle piece that catches the mistakes never fires,** because
      the server catches them before it. (`src/runtime/middleware.js`,
      `src/runtime/server.js`)
- [x] **The list of the middle pieces is a dead object.** Nothing reads it
      and no program can reach it. (`src/runtime/middleware.js`)
- [x] **Two helpers of the runtime cannot be called by a program,** and one
      of them has the name of a word of the language. (`src/runtime/core.js`)
- [x] **`env()` fails silently in the browser** and gives a wrong answer
      without a word. (`src/runtime/node.js`)
- [x] **`title()` does nothing outside the browser,** without a word.
      (`src/runtime/dom.js`)
- [x] **The helpers that are only in Node name the module and not the
      helper,** so the reader sees "node:fs is only available in Node" for
      a call to `fs.read`. (`src/runtime/node.js`)
- [x] **`--force` does nothing in `vey build`,** although the help says it
      does. (`src/cli/build.rs`)
- [x] **A test file that is named and is not there is ignored,** and the
      command says that there is no test and succeeds. (`src/cli/run.rs`)
- [x] **The folder of the tests is invisible** to `vey lint`, `vey fmt`,
      `vey docs`, `vey migrate` and `vey check`. (`src/cli/entry.rs`)
- [x] **The help of the command has no arm for `vey lsp`,** so it falls
      back to the general help. (`src/cli/output.rs`)
- [x] **`vey fmt` and `vey migrate` write several files one after the
      other,** without a name between them. (`src/cli/fmt.rs`,
      `src/cli/migrate.rs`)
- [x] **The report of an error swallows a file that cannot be read,** so
      the frame of the code is missing. (`src/cli/output.rs`)
- [x] **The general help does not name `--target` and `--write`.**
      (`src/cli/output.rs`)
- [x] **The list of the tags of HTML is copied into the runtime and has
      drifted:** `title` is missing. (`src/runtime/highlight.js`,
      `src/parser/html.rs`)
- [x] **The list of the words of the language is copied into the runtime
      and has drifted:** `pub`, `priv`, `throw`, `yield`, `defer` and `is`
      are missing, and `type` is there. (`src/runtime/highlight.js`,
      `src/lexer/mod.rs`)
- [ ] **There are two highlighters of the same language,** and they colour
      it differently. (`src/cli/highlight.rs`, `src/runtime/highlight.js`)
- [x] **There are two escapers of HTML,** and the one of the tools does
      not escape the apostrophe. (`src/runtime/shared.js`,
      `src/cli/output.rs`)
- [x] **There are two readers of cookies,** and they give different
      answers for the same head. (`src/runtime/server.js`,
      `src/runtime/web.js`)
- [ ] **`http.get` and `fetch` give two shapes for the same thing,** and
      both are typed `any`, so nothing notices. (`src/runtime/http.js`)
- [x] **The list of the parts of the runtime in the README is missing
      `map.js`.** (`README.md`)
- [ ] **The list of the helpers of the runtime lives in four places**
      that have to be written together, and it has already drifted.
      (`src/builtins.rs`, `src/types/builtins.rs`, `README.md`,
      `src/cli/lsp.rs`)
- [ ] **`bind()` and `mount()` are two ways to put a page on the screen,**
      and only one of them is known to the router. (`src/runtime/dom.js`,
      `src/runtime/mount.js`, `src/runtime/router.js`)
- [x] **`http.get` refuses with a plain error** where `fetch` refuses with
      an error of the language, so the browser shows a stack.
      (`src/runtime/http.js`)
- [x] **The server always says that it listens on `localhost`,** even when
      it listens on another host. (`src/runtime/server.js`)


---

## The promises of the README that the code does not keep

Every promise of the README is kept now. The gaps that are left are in
part 14, and the ones that are about the words of the README are:

| The README says | The code does |
| --- | --- |
| the enums of another module have their fields checked | they are checked, and the fields of a variant of another module are bound to the right name (14.1) |
| the checker narrows the unions in the simple conditions | it narrows the truth of a value, `undefined` and `instanceof` (14.2) |
| a `match` on an enum is checked for exhaustiveness | it is, also for an enum of another module, but not for a `bool` (14.2) |
| the checker checks the calls | it does, also for a function without types, but the types of a function written as an expression are not guessed (14.2) |
| the source map points at the file and the line | it does, and at the column as well |
| `vey check` refuses a value that is used before it is written | it refuses the values and the functions of the top level |
| the list of the parts of the runtime | `map.js` is missing from the list in the README (14.3) |
