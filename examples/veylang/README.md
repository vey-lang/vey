# veylang.tech

The site of Vey, written in Vey.

| Path | What it is |
| --- | --- |
| `src/main.vey` | the home page |
| `src/language.vey` | the guide of the language |
| `src/library.vey` | the runtime that every program carries |
| `src/install.vey` | how to install Vey |
| `src/site.vey` | the parts that every page shares |
| `src/samples.vey` | the samples of the code |
| `src/styles/site.css` | the styles of the site |
| `public/install.sh` | the script that installs Vey with one command |
| `public/logo.svg` | the mark of the language |

## Build

```sh
vey build . -o .dist
```

The site is a folder of files that any host can serve.

## The pages

Every page is a component that is put in the page of the browser:

```vey
component App() {
  main {
    site.Header("language")
    section.prose {
      div.shell {
        h2 { "The language" }
      }
    }
    site.Footer()
  }
}

bind("#app", App())
```

The name of the page that is open is given to the header, so that it can mark
the link of that page.

## The samples of the code

The samples are written once, in `src/samples.vey`, and the pages only put
them in their place. They are kept in text that is not interpolated, so the
braces of the code stay as they are:

```vey
export const HELLO = text.dedent('let name = "Vey"

fn greet(who) {
  return "Hello, " + who
}')
```
