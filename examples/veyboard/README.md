# Veyboard

A personal productivity board, written with [Vey](https://vey-lang.dev): a
dashboard for the day, tasks with priorities, due dates and tags, notes that
may be pinned and linked to a task, and a history of what was done. Everything
is kept in one file on the machine that runs it.

The whole application is Vey: the routes, the domain rules, the pages and the
tests. Node only runs the program that `vey build` makes of it.

## What it does

**Dashboard.** How many tasks are open and done, what is due today and in the
next days, what is late, the notes that were touched last, and a small chart of
the last seven days, counted from the activity history. A quick form adds a task
without leaving the page, and a note is one click away from the same card.

**Tasks.** Create, edit, delete, finish and reopen. Every task has a priority
(high, medium, low), an optional due date, and tags. The list may be searched by
title, filtered by status, priority and tag, and sorted by due date, priority or
the moment it was made. A task whose day has gone by is marked as late, in words
as well as in colour. Deleting asks on a page of its own first; nothing is
deleted on a link click alone.

**Notes.** Create, edit, delete, pin, tag, and link to a task. The search looks
at the title and the body. Pinned notes come first, the rest by the moment they
were last written, and every note says when that was.

**Activity.** Each change leaves a line: a task created, finished, reopened,
edited or deleted, a note created, pinned, unpinned, edited or deleted. The page
of the history groups the lines by day and counts the week from them.

## Requirements

- Node 22 or newer.
- The Vey compiler: `npm install -g vey`.
- Nothing else. There is no database and no package of the registry to install.

## Make the project

The project was made with the generator, and the files the generator writes are
still where it put them:

```sh
vey new veyboard
cd veyboard
```

`vey new` writes `vey.toml`, the `package.json`, the `Dockerfile`, the CI
workflow, `public/`, `src/main.vey`, `src/site.vey`, `src/styles/site.css` and
`tests/site.vey`. The starter page was then replaced by the board: `main.vey`
still starts the server, `site.css` still holds the styles, and the tests of the
page grew into the suite in `tests/`.

## Run it

```sh
npm start
```

That builds the project into one program of Node (`server.cjs`) and starts it.
The board is at <http://127.0.0.1:4000>.

The same work by hand:

```sh
vey build --node -o server.cjs
node --enable-source-maps server.cjs
```

Two settings are read from the environment, and both have a sane stand-in:

| Variable | What it is | Default |
| --- | --- | --- |
| `PORT` | the port the server listens on | `4000` |
| `VEYBOARD_DATA` | the folder the board is kept in | `data` |

```sh
PORT=8080 VEYBOARD_DATA=/var/lib/veyboard npm start
```

Run the commands from the project folder: the server hands out
`src/styles/site.css` and the files of `public/` from there.

## Check and test

```sh
npm run check   # vey check
npm test        # vey test
```

The tests are Vey tests. They cover the date arithmetic, the validation of form
fields, the rules of tasks and notes, the store, the pages, and the whole board
over HTTP: a real server is started on a free port, requests are sent through
it, and a second server is started over the same folder to show that the data
survives a restart. A failing test fails `npm test`.

## How the data is kept

`store.vey` owns the file; nothing else in the program reads or writes it. The
file is `veyboard.json` in the data folder, and `veyboard.json.bak` is the file as it
was before the last save.

- **A save is atomic.** The new board is written to `veyboard.json.tmp`, the
  current file is moved to `veyboard.json.bak`, and the temporary file takes its
  place. A save that fails leaves the board as it was, and the handler takes the
  change back out of memory, so the memory and the file never tell different
  stories.
- **A file that does not read is not lost.** If `veyboard.json` is broken, the
  backup is read instead, and the person who opens the board is told on the
  first page that it happened.
- **A record that is wrong is repaired, not thrown away.** A title that is not a
  text becomes `untitled task`, a priority that is not one of the three becomes
  `medium`, a due date that is not a day becomes no date, a link to a task that
  is gone becomes no link. A line of the file that is not a record at all is
  left out, and the rest stay.
- **Ids are made, not counted.** Every task, note and activity line has a UUID,
  so a change never depends on where a record sits in a list.

The folder is `data/` by default, which `.gitignore` keeps out of the
repository. The file is JSON, so it may be read, copied and backed up by hand.

## Security

- `security()` sets the headers of the server, and errors are not sent to the
  browser (`exposeErrors: false`).
- A body larger than 128 KiB is refused before it is read.
- A form that posts from another site is refused with 403; the origin is
  checked against the host of the request.
- Everything that is written into a page goes through the escaping of the
  markup: a task titled `<script>` is a task titled `<script>`.
- Every form field is checked and normalised before it is saved, and a form that
  does not pass is answered again with its own words, with nothing saved.
- The pages an id leads to are found by the id, and an id that is not there is
  a 404, and a method an address does not take is a 405 with an `Allow` header.
- The data folder is a setting of the program, not a part of an address: no
  path of a request reaches the disk.

## Layout

| Path | What it is |
| --- | --- |
| `src/main.vey` | the entry: reads the port and the folder, makes the server, starts it |
| `src/app.vey` | the routes, the middleware, and the whole page around the markup |
| `src/pages.vey` | one component for each page of the board |
| `src/site.vey` | the small parts the pages are built from: the side, a task row, a note card |
| `src/tasks.vey` | the rules of a task: making, revising, finishing, ordering, filtering |
| `src/notes.vey` | the rules of a note: making, revising, pinning, searching |
| `src/activity.vey` | the history: what a line is, and the counts of the last days |
| `src/dates.vey` | calendar days as plain arithmetic, so a test needs no clock |
| `src/format.vey` | days and moments in words |
| `src/validate.vey` | what a form sends, checked and normalised |
| `src/store.vey` | the file: reading, repairing, saving, the backup |
| `src/styles/site.css` | the styles of the board, light and dark |
| `public/` | the files that are served as they are: the marks and `robots.txt` |
| `tests/` | the tests, one file for each part |

The pages are rendered on the server and sent as HTML. A form posts, the handler
changes the board, and the answer is a redirect to the page the form came from,
so a reload never sends a form twice. The message of what was done waits in the
session for that one page.

## Limitations

These are the edges of Vey today, and what the board does about them:

- **The markup has no `body` tag** (`body` is not a tag of the markup, and
  neither is `html` or `head`), so `app.vey` writes the document around the
  markup of the page, and the title of the page goes through `html()` so that it
  is escaped like every other text.
- **A component keeps no value of its own that reads a prop**, because the
  values of a component body are made before its arguments are bound. The pages
  therefore compute what they show in plain functions next to the components,
  which also keeps every page a plain function of its data.
- **`for` is a keyword**, so a label cannot carry `for="…"`. Every field is
  wrapped in its label instead, which is as good for a screen reader.
- **No client-side JavaScript is used.** The board is server-rendered forms and
  links: it works with a script turned off, and every state of it is a page that
  may be linked to.
- **One user, one process.** The board has no accounts, and Node runs one
  request of a server at a time, which is enough for the machine it is for.

## Build it for a server

```sh
vey build --node -o server.cjs
node --enable-source-maps server.cjs
```

`server.cjs` is the whole program in one file. The `Dockerfile` of the
generator does the same in an image: it installs the compiler, builds the
program, and runs it. Point `VEYBOARD_DATA` at a volume so that the board
outlives the container.
