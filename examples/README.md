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
