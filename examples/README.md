# Examples

Small programs that show what Vey can do.

A file that is a page is built into a site:

```sh
vey build examples/page.vey --node -o /tmp/page.cjs
```

A file that is a script is built and run with node:

```sh
vey build examples/hello.vey --node -o /tmp/hello.cjs
node --enable-source-maps /tmp/hello.cjs
```

| File | What it shows |
| --- | --- |
| `hello.vey` | the basics: functions, lists, pipes and `match` |
| `math.vey` | a module with the math helpers |
| `enums.vey` | enums and the matching of their variants |
| `generics.vey` | generic functions and type aliases |
| `typed.vey` | the types that are checked |
| `module-app.vey` | a module that is imported by another file |
| `page.vey` | a page with components |
| `render.vey` | a page that is rendered to HTML |
| `server.vey` | a server with a route |
| `npm-package.vey` | a package of npm (`npm install marked` first) |
| `veylang/` | the site of the language, written in Vey |
| `veyboard/` | a whole application: a board of tasks and notes, with a server and a file |

A folder is an example that is a project of its own. `veylang/` is the site of
the language, and `veyboard/` is an application made with `vey new`: a
dashboard, tasks and notes kept in a file, with its own server, styles and
tests. Each folder has a README that says how to run it and what it shows.
