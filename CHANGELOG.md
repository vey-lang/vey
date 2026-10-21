# Changelog

## 0.7.0

The toolchain changed the most in this release.

- `vey run` serves the built site the way a host would serve it. `vey dev` is
  the same server for the work: it watches the files and reloads the page.
- `vey check` checks the whole project when it is given a folder, and only the
  file it is given when it is given a file.
- `vey new` writes a clean project: `src/main.vey`, `src/styles/site.css`,
  `public/`, `package.json`, `README.md` and `.gitignore`.
- Every page of a project becomes a page of the site. A page that cannot be
  built is reported and the other pages are still written.
- A page may not take the place of the home of the site.

The language grew a few things:

- A `try` may have a `finally` block, with or without a `catch`.
- A name in a pattern may have a value that is used when there is none:
  `let { name, greeting = "Hello" } = user`.
- The fields of an object may be written one under the other, in a type, in a
  value and in a pattern.
- A match may have a condition (`n if n > 10 => ...`) and a name may take the
  value of the match (`n => ...`).
- The value of an attribute may be written in braces: `class={name}`.
- The examples of the readme are checked by the tests, so the documentation
  cannot drift away from the language.
- A generic function may have a constraint (`fn first<T: number>(...)`) and a
  call may name its type arguments.
- A type alias may refer to itself.
- A parameter may take the argument apart.

The checker learned to:

- Report a comparison of values that can never be the same.
- Report a walk through something that is not a list, in the markup as well.
- Report a return type that has no arrow, and a component that has one.
- Point to the helper of the runtime when a field is missing.
- Report a tag that is not a tag of HTML.
- Report an argument that a component does not have.

## 0.6.0

- The first release of the compiler, the type checker, the runtime and the
  command line.
