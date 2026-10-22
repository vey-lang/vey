//! Tests for the runtime that ship with the compiler.
//!
//! The runtime is loaded straight into Node so that behaviour which is not
//! reachable from Vey source, such as the internal element factory, can be
//! checked as well.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use common::temp_dir;

/// Load the runtime and evaluate `script`.
///
/// The script can use `runtime` for the Node flavoured runtime and `RUNTIME`
/// for the path of the runtime source.
fn run_runtime(label: &str, script: &str) -> String {
    let dir = temp_dir(label);
    // The runtime is split into parts, so it is put together here.
    let runtime = dir.join("runtime.js");
    fs::write(&runtime, vey::runtime::source()).unwrap();
    let harness = dir.join("harness.cjs");
    fs::write(
        &harness,
        format!(
            "const fs = require('node:fs');\n\
             const RUNTIME = process.argv[2];\n\
             const __veySource = fs.readFileSync(RUNTIME, 'utf8');\n\
             global.RUNTIME = RUNTIME;\n\
             const createVeyRuntime = new Function(__veySource + '\\nreturn createVeyRuntime;')();\n\
             const runtime = createVeyRuntime(globalThis, require);\n\
             {}\n",
            script
        ),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(runtime)
        .output()
        .expect("failed to start node");
    assert!(
        output.status.success(),
        "runtime script failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn inherited_names_are_not_treated_as_void_tags() {
    let text = run_runtime(
        "void-tags",
        r#"
console.log(runtime.html(runtime.h("constructor", null, [])));
console.log(runtime.html(runtime.h("br", null, [])));
"#,
    );
    assert!(text.contains("<constructor></constructor>"), "{text}");
    assert!(text.contains("<br>"), "{text}");
}

#[test]
fn raw_html_survives_the_dom_path() {
    let text = run_runtime(
        "raw-dom",
        r#"
const fakeDoc = {
  createElement(tag) {
    const el = {
      nodeType: 1,
      tagName: tag.toUpperCase(),
      children: [],
      innerHTML: "",
      appendChild(child) { this.children.push(child); return child; },
      setAttribute() {}
    };
    // A real <template> holds its parsed content in `.content`.
    el.content = { nodeType: 11, children: [], appendChild(c) { this.children.push(c); return c; } };
    return el;
  },
  createTextNode(text) { return { nodeType: 3, textContent: String(text) }; },
  createDocumentFragment() {
    return { nodeType: 11, children: [], appendChild(c) { this.children.push(c); return c; } };
  }
};
const browser = new Function(require('node:fs').readFileSync(RUNTIME, 'utf8') + '\nreturn createVeyRuntime;')()({ document: fakeDoc, console }, null);
const el = browser.h("p", null, [browser.raw("<strong>x</strong>")]);
console.log(el.children.length, el.children[0].nodeType);
"#,
    );
    // Raw markup becomes a fragment, never a text node with "[object Object]".
    assert!(text.contains("1 11"), "{text}");
    assert!(!text.contains("[object Object]"), "{text}");
}

#[test]
fn styles_use_the_same_key_format_on_both_paths() {
    let text = run_runtime(
        "style-keys",
        r#"
console.log(runtime.html(runtime.h("div", { style: { backgroundColor: "red", "font-size": "12px" } }, [])));
"#,
    );
    assert!(text.contains("background-color: red"), "{text}");
    assert!(text.contains("font-size: 12px"), "{text}");
}

#[test]
fn malformed_percent_encoding_does_not_throw() {
    let text = run_runtime(
        "safe-decode",
        r#"
console.log(runtime.url.decode("%"));
"#,
    );
    assert!(text.contains("%"), "{text}");
}

#[test]
fn node_only_helpers_report_a_clear_error_in_the_browser() {
    let text = run_runtime(
        "browser-guard",
        r#"
const browser = new Function(require('node:fs').readFileSync(RUNTIME, 'utf8') + '\nreturn createVeyRuntime;')()({ console }, null);
try { browser.fs.read("x"); } catch (err) { console.log(err.message); }
"#,
    );
    assert!(text.contains("only available in Node"), "{text}");
}

#[test]
fn every_helper_of_the_checker_is_in_the_runtime() {
    let runtime = vey::runtime::source();
    let builtins = fs::read_to_string("src/builtins.rs").expect("the list of helpers is missing");

    let mut names = Vec::new();
    for line in builtins.lines() {
        let line = line.trim();
        let Some(start) = line.find('"') else {
            continue;
        };
        let rest = &line[start + 1..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        let name = &rest[..end];
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            names.push(name.to_string());
        }
    }
    assert!(!names.is_empty(), "no helpers were found");

    for name in names {
        assert!(
            runtime.contains(&format!("\n    {}:", name)),
            "the runtime does not have '{}'",
            name
        );
    }
}

#[test]
fn a_comment_of_the_code_stops_at_the_end_of_the_line() {
    let text = run_runtime(
        "highlight-comment",
        r#"
console.log(runtime.highlight("// a comment\nlet x = 1"));
"#,
    );
    assert!(
        text.contains("<span class=\"cm\">// a comment</span>"),
        "{text}"
    );
    assert!(text.contains("<span class=\"kw\">let</span>"), "{text}");
}

#[test]
fn the_tags_of_the_markup_are_coloured() {
    let text = run_runtime(
        "highlight-tags",
        r#"
console.log(runtime.highlight("section.card {\n  h2 { title }\n}"));
"#,
    );
    assert!(text.contains("<span class=\"tag\">section</span>"), "{text}");
    assert!(text.contains("<span class=\"cls\">card</span>"), "{text}");
    assert!(text.contains("<span class=\"tag\">h2</span>"), "{text}");
}

#[test]
fn every_part_of_the_runtime_is_joined() {
    let source = vey::runtime::source();
    let parts = vey::runtime::parts();
    for part in &parts {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/runtime")
            .join(part);
        assert!(path.is_file(), "the part {} is missing", part);
    }
    // Every file of the folder is a part of the runtime.
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/runtime");
    for entry in fs::read_dir(&folder).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".js") {
            continue;
        }
        assert!(
            parts.contains(&name.as_str()),
            "the part {} is not joined into the runtime",
            name
        );
    }
    // The runtime is one function that the program calls.
    assert!(
        source.contains("function createVeyRuntime("),
        "the runtime is broken"
    );
    assert!(
        source.trim_end().ends_with("})();"),
        "the runtime is broken"
    );
}

/// Run a script in the browser, with the small DOM of the tests.
fn run_browser(label: &str, script: &str) -> String {
    let dom = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake-dom.cjs");
    let script = format!(
        "const dom = require(\"{}\");\n\
         const browser = createVeyRuntime(dom.view, null);\n\
         {}\n",
        dom.to_string_lossy(),
        script
    );
    run_runtime(label, &script)
}

#[test]
fn the_regex_finds_and_changes_a_text() {
    let text = run_runtime(
        "regex",
        r#"
const re = runtime.regex("(\\d+)-(\\d+)");
console.log(re.test("12-34"), re.test("nope"));
console.log(JSON.stringify(re.match("12-34")));
console.log(JSON.stringify(re.match("nope")));
console.log(JSON.stringify(re.matchAll("1-2 3-4")));
console.log(re.replace("12-34", "$2-$1"));
console.log(re.replaceAll("1-2 3-4", "x"));
console.log(JSON.stringify(runtime.regex("\\d+").split("a1b22c")));
"#,
    );
    assert!(text.contains("true false"), "{text}");
    assert!(text.contains("[\"12-34\",\"12\",\"34\"]"), "{text}");
    assert!(text.contains("null"), "{text}");
    assert!(text.contains("[[\"1-2\",\"1\",\"2\"],[\"3-4\",\"3\",\"4\"]]"), "{text}");
    assert!(text.contains("34-12"), "{text}");
    assert!(text.contains("x x"), "{text}");
    assert!(text.contains("[\"a\",\"b\",\"c\"]"), "{text}");
}

#[test]
fn the_intl_writes_a_number_and_a_date() {
    let text = run_runtime(
        "intl",
        r#"
console.log(runtime.intl.formatNumber(1234.5, "en-US"));
console.log(runtime.intl.dayName(0, "en-US"), runtime.intl.dayName(1, "en-US"));
console.log(runtime.intl.monthName(0, "en-US"), runtime.intl.monthName(11, "en-US"));
console.log(runtime.intl.formatDate("2024-03-05T10:20:00Z", { locale: "en-US", year: "numeric", month: "long", day: "numeric", timeZone: "UTC" }));
console.log(runtime.intl.relativeTime(-1, "day", "en-US"));
"#,
    );
    assert!(text.contains("1,234.5"), "{text}");
    assert!(text.contains("Sunday Monday"), "{text}");
    assert!(text.contains("January December"), "{text}");
    assert!(text.contains("March"), "{text}");
    assert!(text.contains("yesterday"), "{text}");
}

#[test]
fn the_bytes_of_a_text_are_a_list_of_numbers() {
    let text = run_runtime(
        "bytes",
        r#"
console.log(JSON.stringify(runtime.bytes.from("hi")));
console.log(runtime.bytes.toBase64([104, 105]));
console.log(JSON.stringify(runtime.bytes.fromBase64("aGk=")));
console.log(JSON.stringify(runtime.bytes.concat([[1, 2], [3]])));
console.log(runtime.bytes.length([1, 2, 3]));
console.log(runtime.bytes.hex([0, 255, 16]));
console.log(JSON.stringify(runtime.text.encode("hé")));
console.log(runtime.text.decode([104, 195, 169]));
"#,
    );
    assert!(text.contains("[104,105]"), "{text}");
    assert!(text.contains("aGk="), "{text}");
    assert!(text.contains("[1,2,3]"), "{text}");
    assert!(text.contains("00ff10"), "{text}");
    assert!(text.contains("[104,195,169]"), "{text}");
    assert!(text.contains("hé"), "{text}");
}

#[test]
fn a_deep_copy_holds_nothing_of_the_value_it_came_from() {
    let text = run_runtime(
        "deep-copy",
        r#"
const original = { a: 1, list: [1, { b: 2 }] };
const copy = runtime.deepCopy(original);
copy.list[1].b = 3;
console.log(copy.list[1].b, original.list[1].b);
console.log(copy.list === original.list);
const circle = { name: "a" };
circle.self = circle;
console.log(runtime.deepCopy(circle).self === runtime.deepCopy(circle).self);
"#,
    );
    assert!(text.contains("3 2"), "{text}");
    assert!(text.contains("false"), "{text}");
}

#[test]
fn a_signal_tells_when_its_value_changes() {
    let text = run_runtime(
        "signal",
        r#"
const count = runtime.signal(0);
const seen = [];
const off = count.subscribe((value) => seen.push(value));
count.set(1);
count.set(1);
count.value = 2;
off();
count.set(3);
console.log(JSON.stringify(seen));
console.log(count.value, count.peek());
"#,
    );
    assert!(text.contains("[1,2]"), "{text}");
    assert!(text.contains("3 3"), "{text}");
}

#[test]
fn a_store_is_shared_by_its_name() {
    let text = run_runtime(
        "store",
        r#"
const one = runtime.store("count", { n: 0 });
const two = runtime.store("count");
one.set("n", 5);
console.log(one === two, two.peek().n);
const own = runtime.store({ n: 1 });
own.set("n", 2);
console.log(own.peek().n, runtime.store("count").peek().n);
"#,
    );
    assert!(text.contains("true 5"), "{text}");
    assert!(text.contains("2 5"), "{text}");
}

#[test]
fn an_effect_runs_when_a_value_it_reads_changes() {
    let text = run_runtime(
        "effect",
        r#"
const count = runtime.signal(0);
let runs = 0;
const stop = runtime.effect(() => {
  runs += 1;
  count.value;
});
count.set(1);
console.log(runs);
stop();
count.set(2);
console.log(runs);
"#,
    );
    assert!(text.contains("2"), "{text}");
}

#[test]
fn a_page_is_changed_in_place_and_its_list_is_put_in_another_order() {
    let text = run_browser(
        "mount",
        r##"
const first = browser.h("ul", { class: "list" }, [
  browser.h("li", { key: "a" }, ["a"]),
  browser.h("li", { key: "b" }, ["b"])
]);
const page = browser.mount("#app", first);
const list = dom.root.childNodes[0];
const item = list.childNodes[0];
page.update(browser.h("ul", { class: "list" }, [
  browser.h("li", { key: "b" }, ["b!"]),
  browser.h("li", { key: "a" }, ["a!"])
]));
console.log(dom.root.childNodes[0] === list);
console.log(list.childNodes[0] === list.childNodes[1]);
console.log(dom.root.textContent);
console.log(item.getAttribute("key"));
"##
    );
    // The list is the same node, and the item with the key "b" comes first.
    assert!(text.contains("true"), "{text}");
    assert!(text.contains("b!a!"), "{text}");
}

#[test]
fn the_page_of_the_server_is_taken_over_by_the_browser() {
    let text = run_browser(
        "water",
        r##"
const server = dom.document.createElement("p");
server.setAttribute("class", "hello");
server.appendChild(dom.document.createTextNode("hi"));
dom.root.appendChild(server);
browser.mount("#app", browser.h("p", { class: "hello" }, ["hi"]));
console.log(dom.root.childNodes.length);
console.log(dom.root.childNodes[0] === server);
"##
    );
    assert!(text.contains("1"), "{text}");
    assert!(text.contains("true"), "{text}");
}

#[test]
fn the_events_of_one_s_own_are_attached_to_a_node() {
    let text = run_browser(
        "events",
        r##"
browser.mount("#app", browser.h("div", {}, [browser.h("span", { class: "item" }, ["x"])]));
let seen = 0;
browser.on("#app", "click", () => { seen += 1; });
browser.delegate("#app", "click", ".item", () => { seen += 10; });
const span = dom.root.childNodes[0].childNodes[0];
for (const listener of dom.root.listeners.click) listener({ target: span });
console.log(seen);
let mounted = 0;
let destroyed = 0;
browser.onMount(() => { mounted += 1; });
browser.onDestroy(() => { destroyed += 1; });
const page = browser.mount("#app", browser.h("p", {}, ["x"]));
page.destroy();
console.log(mounted, destroyed);
const input = browser.ref();
browser.mount("#app", browser.h("input", { ref: input }, []));
console.log(input.value.tagName);
"##
    );
    assert!(text.contains("11"), "{text}");
    assert!(text.contains("1 1"), "{text}");
    assert!(text.contains("INPUT"), "{text}");
}

#[test]
fn the_routes_of_the_browser_show_another_page() {
    let text = run_browser(
        "router",
        r##"
const seen = [];
browser.route("/users/{id}", (params) => {
  seen.push(params.id);
  return browser.h("p", {}, ["user " + params.id]);
});
browser.mount("#app", browser.h("p", {}, ["start"]));
browser.navigate("/users/7");
console.log(JSON.stringify(seen));
console.log(dom.root.textContent);
console.log(browser.routeParams().id, browser.currentRoute().path);
"##
    );
    assert!(text.contains("[\"7\"]"), "{text}");
    assert!(text.contains("user 7"), "{text}");
    assert!(text.contains("7 /users/7"), "{text}");
}

#[test]
fn the_notes_of_the_browser_are_kept() {
    let text = run_browser(
        "storage",
        r#"
browser.storage.set("a", "1");
console.log(browser.storage.get("a"));
browser.storage.remove("a");
console.log(browser.storage.get("a"));
browser.sessionStorage.set("b", "2");
console.log(browser.sessionStorage.get("b"));
browser.history.push({ n: 1 }, "", "/next");
console.log(browser.history.state().n, dom.view.location.pathname);
"#,
    );
    assert!(text.contains("1"), "{text}");
    assert!(text.contains("null"), "{text}");
    assert!(text.contains("2"), "{text}");
    assert!(text.contains("1 /next"), "{text}");
}

#[test]
fn the_server_has_middle_pieces() {
    let text = run_runtime(
        "middleware",
        r#"
const http = require("node:http");
const app = runtime.server(0);
const seen = [];
app.use((req, res, next) => { seen.push(req.path); next(); });
app.use("/admin", (req, res, next) => { res.set("X-Admin", "yes"); next(); });
app.get("/", (req) => "hello");
app.get("/own", (req) => "own", (req, res, next) => { res.set("X-Own", "yes"); next(); });
app.start(() => {
  const get = (path) => new Promise((resolve) => {
    http.get("http://127.0.0.1:" + app.port() + path, (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", () => resolve({ status: res.statusCode, headers: res.headers, body }));
    });
  });
  get("/").then(async (answer) => {
    console.log(answer.status, answer.body);
    console.log(JSON.stringify(seen));
    const own = await get("/own");
    console.log(own.headers["x-own"]);
    const admin = await get("/admin");
    console.log(admin.headers["x-admin"]);
    await app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(text.contains("200 hello"), "{text}");
    assert!(text.contains("[\"/\"]"), "{text}");
    assert!(text.contains("yes"), "{text}");
}

#[test]
fn the_server_reads_a_form_with_a_file() {
    let text = run_runtime(
        "multipart",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.post("/form", (req) => ({ name: req.body.name, file: req.files.file.text }));
app.start(() => {
  const boundary = "----veytest";
  const body = [
    "--" + boundary,
    'Content-Disposition: form-data; name="name"',
    "",
    "Grace",
    "--" + boundary,
    'Content-Disposition: form-data; name="file"; filename="a.txt"',
    "Content-Type: text/plain",
    "",
    "the file",
    "--" + boundary + "--",
    ""
  ].join("\r\n");
  const request = http.request("http://127.0.0.1:" + app.port() + "/form", {
    method: "POST",
    headers: { "Content-Type": "multipart/form-data; boundary=" + boundary }
  }, (res) => {
    let text = "";
    res.on("data", (chunk) => { text += chunk; });
    res.on("end", () => { console.log(text); app.stop(); process.exit(0); });
  });
  request.end(body);
});
"#,
    );
    assert!(text.contains("\"name\":\"Grace\""), "{text}");
    assert!(text.contains("the file"), "{text}");
}

#[test]
fn the_answer_of_the_server_may_set_a_cookie_and_a_session() {
    let text = run_runtime(
        "session",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/login", (req, res) => {
  runtime.session.start();
  runtime.session.set("user", "Ada");
  res.cookie("seen", "yes");
  return { user: runtime.session.get("user") };
});
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/login", (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(body);
      console.log(res.headers["set-cookie"].length);
      console.log(res.headers["set-cookie"].some((value) => value.startsWith("vey.sid=")));
      app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert!(text.contains("\"user\":\"Ada\""), "{text}");
    assert!(text.contains("2"), "{text}");
    assert!(text.contains("true"), "{text}");
}

#[test]
fn a_head_and_an_options_are_answered() {
    let text = run_runtime(
        "head-options",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/", (req) => "hello");
app.start(() => {
  const ask = (method) => new Promise((resolve) => {
    const request = http.request("http://127.0.0.1:" + app.port() + "/", { method }, (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", () => resolve({ status: res.statusCode, headers: res.headers, body }));
    });
    request.end();
  });
  ask("HEAD").then(async (head) => {
    console.log(head.status, head.body.length, head.headers["content-length"]);
    const options = await ask("OPTIONS");
    console.log(options.status, options.headers["allow"]);
    await app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(text.contains("200 0 5"), "{text}");
    assert!(text.contains("204"), "{text}");
    assert!(text.contains("GET"), "{text}");
}

#[test]
fn the_files_that_are_served_have_a_cache() {
    let text = run_runtime(
        "cache",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const folder = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-cache-"));
nodeFs.writeFileSync(path.join(folder, "a.txt"), "hello file");
const app = runtime.server(0);
app.static(folder);
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/a.txt", (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(body);
      console.log(!!res.headers["etag"], !!res.headers["last-modified"], !!res.headers["cache-control"]);
      const again = http.request("http://127.0.0.1:" + app.port() + "/a.txt", {
        headers: { "If-None-Match": res.headers["etag"] }
      }, (second) => {
        second.on("data", () => {});
        second.on("end", () => { console.log(second.statusCode); app.stop(); process.exit(0); });
      });
      again.end();
    });
  });
});
"#,
    );
    assert!(text.contains("hello file"), "{text}");
    assert!(text.contains("true true true"), "{text}");
    assert!(text.contains("304"), "{text}");
}

#[test]
fn the_body_of_a_request_has_a_limit() {
    let text = run_runtime(
        "body-limit",
        r#"
const http = require("node:http");
const app = runtime.server(0, { bodyLimit: 10 });
app.post("/x", (req) => "ok");
app.start(() => {
  const request = http.request("http://127.0.0.1:" + app.port() + "/x", { method: "POST" }, (res) => {
    res.on("data", () => {});
    res.on("end", () => { console.log(res.statusCode); app.stop(); process.exit(0); });
  });
  request.end("01234567890123456789");
});
"#,
    );
    assert!(text.contains("413"), "{text}");
}

#[test]
fn the_server_waits_for_the_request_that_is_running() {
    let text = run_runtime(
        "stop",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/slow", (req) => new Promise((resolve) => {
  setTimeout(() => resolve("slow"), 200);
}));
app.start(() => {
  let finished = false;
  http.get("http://127.0.0.1:" + app.port() + "/slow", (res) => {
    res.on("data", () => {});
    res.on("end", () => { finished = true; });
  });
  setTimeout(async () => {
    await app.stop();
    console.log(finished);
    process.exit(0);
  }, 50);
});
"#,
    );
    assert!(text.contains("true"), "{text}");
}

#[test]
fn the_socket_and_the_events_of_the_server_are_answered() {
    let text = run_runtime(
        "sockets",
        r##"
const http = require("node:http");
const app = runtime.server(0);
app.ws("/chat", (socket) => {
  socket.onMessage((text) => { socket.send("echo: " + text); });
});
app.sse("/events", (req, stream) => {
  stream.send("first");
  stream.event("tick", "second");
});
app.start(() => {
  const socket = new WebSocket("ws://127.0.0.1:" + app.port() + "/chat");
  socket.onopen = () => socket.send("hello");
  socket.onmessage = (event) => {
    console.log(event.data);
    const request = http.get("http://127.0.0.1:" + app.port() + "/events", (res) => {
      let text = "";
      res.on("data", (chunk) => {
        text += chunk.toString("utf8");
        for (const line of text.split("\n")) {
          if (line.startsWith("data: ")) console.log(line.slice(6));
        }
      });
      res.on("end", () => {});
    });
    request.on("error", () => {});
    setTimeout(async () => {
      socket.close();
      request.destroy();
      await app.stop();
      process.exit(0);
    }, 200);
  };
});
"##,
    );
    assert!(text.contains("echo: hello"), "{text}");
    assert!(text.contains("first"), "{text}");
    assert!(text.contains("second"), "{text}");
}

#[test]
fn the_crypto_and_the_zlib_of_node_work() {
    let text = run_runtime(
        "crypto",
        r#"
console.log(runtime.crypto.randomBytes(4).length);
const signature = runtime.crypto.sign("hello", "secret");
console.log(runtime.crypto.verify("hello", signature, "secret"));
console.log(runtime.crypto.verify("other", signature, "secret"));
const pair = runtime.crypto.keyPair();
const signed = runtime.crypto.sign("hello", pair.privateKey);
console.log(runtime.crypto.verify("hello", signed, pair.publicKey));
const packed = runtime.zlib.gzip("hello hello");
console.log(runtime.zlib.gunzip(packed));
console.log(runtime.zlib.inflate(runtime.zlib.deflate("hello")));
"#,
    );
    assert!(text.contains("4"), "{text}");
    assert!(text.contains("true"), "{text}");
    assert!(text.contains("false"), "{text}");
    assert!(text.contains("hello hello"), "{text}");
}

#[test]
fn another_program_may_be_run_and_a_file_may_be_streamed() {
    let text = run_runtime(
        "child-process",
        r#"
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
(async () => {
  const result = await runtime.process.run("node", ["-e", "console.log('hi from node')"]);
  console.log(result.code, result.stdout.includes("hi from node"));
  const child = runtime.process.spawn("node", ["-e", "console.log('spawned')"]);
  let seen = "";
  child.on("data", (text) => { seen += text; });
  const waited = await child.wait();
  console.log(waited.code, seen.includes("spawned"));
  const folder = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-stream-"));
  const file = path.join(folder, "a.txt");
  await runtime.fs.writeStream(file, "the file of the stream");
  console.log(nodeFs.readFileSync(file, "utf8"));
  const stream = runtime.fs.readStream(file);
  console.log(await stream.read());
})();
"#,
    );
    assert!(text.contains("0 true"), "{text}");
    assert!(text.contains("the file of the stream"), "{text}");
}

