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
        "runtime script failed:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn inherited_names_are_not_treated_as_void_tags() {
    let text = run_runtime(
        "void-tags",
        r#"
console.log(runtime.html(runtime.__h("constructor", null, [])));
console.log(runtime.html(runtime.__h("br", null, [])));
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
const el = browser.__h("p", null, [browser.raw("<strong>x</strong>")]);
console.log(el.children.length, el.children[0].nodeType);
"#,
    );
    // Raw markup becomes a fragment, never a text node with "[object Object]".
    assert!(text.contains("1 11"), "{text}");
    assert!(!text.contains("[object Object]"), "{text}");
}

#[test]
fn test_summary_installs_only_one_exit_listener() {
    let text = run_runtime(
        "test-summary-listener",
        r#"
const proc = require("node:process");
const before = proc.listenerCount("exit");
for (let i = 0; i < 12; i++) runtime.test("test " + i, () => {});
console.log(proc.listenerCount("exit") - before);
"#,
    );
    assert!(text.lines().any(|line| line == "1"), "{text}");
}

#[test]
fn styles_use_the_same_key_format_on_both_paths() {
    let text = run_runtime(
        "style-keys",
        r#"
console.log(runtime.html(runtime.__h("div", { style: { backgroundColor: "red", "font-size": "12px" } }, [])));
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
    assert!(
        text.contains("<span class=\"tag\">section</span>"),
        "{text}"
    );
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
    assert!(
        text.contains("[[\"1-2\",\"1\",\"2\"],[\"3-4\",\"3\",\"4\"]]"),
        "{text}"
    );
    assert!(text.contains("34-12"), "{text}");
    assert!(text.contains("x x"), "{text}");
    assert!(text.contains("[\"a\",\"b\",\"c\"]"), "{text}");
}

#[test]
fn regex_supports_named_groups_and_function_replacements() {
    let text = run_runtime(
        "regex-named-groups",
        r#"
const re = runtime.regex("(?<first>\\w+)-(?<second>\\w+)");
const found = re.match("one-two");
console.log(JSON.stringify(found.groups));
console.log(re.replace("one-two", (whole, first, second, offset, input, named) => named.second + ":" + named.first));
"#,
    );
    assert!(text.contains(r#"{"first":"one","second":"two"}"#), "{text}");
    assert!(text.contains("two:one"), "{text}");
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
fn date_helpers_use_calendar_days_and_timezones() {
    let text = run_runtime(
        "date-timezone",
        r#"
const instant = "2024-03-10T06:30:00Z";
console.log(JSON.stringify(runtime.date.parts(instant, { timeZone: "America/New_York" })));
console.log(runtime.date.format(instant, "YYYY-MM-DD HH:mm", { timeZone: "America/New_York" }));
console.log(runtime.date.daysBetween("2024-03-10T23:00:00-05:00", "2024-03-11T01:00:00-04:00"));
"#,
    );
    assert!(
        text.contains(
            r#"{"year":2024,"month":3,"day":10,"hour":1,"minute":30,"second":0,"weekday":0}"#
        ),
        "{text}"
    );
    assert!(text.contains("2024-03-10 01:30"), "{text}");
    assert!(text.contains("1"), "{text}");
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
fn http_supports_put_patch_delete_and_head() {
    let text = run_runtime(
        "http-methods",
        r#"
const calls = [];
globalThis.fetch = (url, options) => {
  calls.push({ method: options.method, body: options.body || null, type: options.headers["content-type"] || null });
  return Promise.resolve({
    status: 200,
    statusText: "OK",
    url,
    headers: { get: () => "text/plain" },
    text: () => Promise.resolve("ok")
  });
};
Promise.all([
  runtime.http.put("/put", { value: 1 }),
  runtime.http.patch("/patch", { value: 2 }),
  runtime.http.delete("/delete"),
  runtime.http.head("/head")
]).then(() => console.log(JSON.stringify(calls)));
"#,
    );
    assert!(text.contains(r#""method":"PUT""#), "{text}");
    assert!(text.contains(r#""method":"PATCH""#), "{text}");
    assert!(text.contains(r#""method":"DELETE""#), "{text}");
    assert!(text.contains(r#""method":"HEAD""#), "{text}");
    assert!(
        text.contains(r#""body":"{\"value\":1}","type":"application/json""#),
        "{text}"
    );
    assert!(
        text.contains(r#""body":"{\"value\":2}","type":"application/json""#),
        "{text}"
    );
}

#[test]
fn http_timeout_keeps_working_with_an_external_abort_signal() {
    let text = run_runtime(
        "http-abort-timeout",
        r#"
const external = runtime.http.AbortController();
globalThis.fetch = (url, options) => {
  console.log(options.signal !== external.signal);
  return new Promise((resolve, reject) => {
    options.signal.addEventListener("abort", () => reject(new Error("aborted")));
  });
};
runtime.http.fetch("/slow", { signal: external.signal, timeout: 1000 }).catch(err => console.log(err.message));
external.abort();
"#,
    );
    assert!(text.contains("true"), "{text}");
    assert!(text.contains("aborted"), "{text}");
}

#[test]
fn http_timeout_is_not_retried_after_its_deadline() {
    let text = run_runtime(
        "http-timeout-no-retry",
        r#"
let calls = 0;
globalThis.fetch = (url, options) => {
  calls++;
  return new Promise((resolve, reject) => {
    options.signal.addEventListener("abort", () => reject(new Error("request timed out")));
  });
};
runtime.http.fetch("/slow", { timeout: 10, retries: 3, retryDelay: 0 }).catch(err => {
  console.log(JSON.stringify({ calls, message: err.message }));
});
"#,
    );
    assert!(
        text.contains(r#"{"calls":1,"message":"request timed out"}"#),
        "{text}"
    );
}

#[test]
fn http_external_abort_does_not_retry_a_pending_request() {
    let text = run_runtime(
        "http-abort-no-retry",
        r#"
const external = runtime.http.AbortController();
let calls = 0;
globalThis.fetch = (url, options) => {
  calls++;
  return new Promise((resolve, reject) => {
    options.signal.addEventListener("abort", () => reject(new Error("caller cancelled")));
  });
};
const pending = runtime.http.fetch("/cancel", {
  signal: external.signal,
  retries: 3,
  retryDelay: 0
});
external.abort();
pending.then(
  () => console.log("unexpected success"),
  err => console.log(JSON.stringify({ calls, message: err.message }))
);
"#,
    );
    assert!(
        text.contains(r#"{"calls":1,"message":"caller cancelled"}"#),
        "{text}"
    );
}

#[test]
fn http_preaborted_signal_skips_fetch_attempts() {
    let text = run_runtime(
        "http-pre-aborted",
        r#"
const external = runtime.http.AbortController();
external.abort(new Error("already cancelled"));
let calls = 0;
globalThis.fetch = () => {
  calls++;
  return Promise.reject(new Error("should not be called"));
};
runtime.http.fetch("/cancel", {
  signal: external.signal,
  retries: 2,
  retryDelay: 0
}).catch(err => console.log(JSON.stringify({ calls, message: err.message })));
"#,
    );
    assert!(
        text.contains(r#"{"calls":0,"message":"already cancelled"}"#),
        "{text}"
    );
}

#[test]
fn http_abort_during_server_error_backoff_stops_retries() {
    let text = run_runtime(
        "http-abort-server-backoff",
        r#"
const external = runtime.http.AbortController();
let calls = 0;
const busy = {
  status: 503,
  statusText: "Busy",
  url: "/busy",
  headers: { get: () => "text/plain" },
  text: () => Promise.resolve("busy")
};
globalThis.fetch = () => { calls++; return Promise.resolve(busy); };
const pending = runtime.http.fetch("/busy", {
  signal: external.signal,
  retries: 2,
  retryDelay: 100
});
setTimeout(() => external.abort(new Error("caller cancelled")), 5);
pending.then(
  () => console.log("unexpected success"),
  err => console.log(JSON.stringify({ calls, message: err.message }))
);
"#,
    );
    assert!(
        text.contains(r#"{"calls":1,"message":"caller cancelled"}"#),
        "{text}"
    );
}

#[test]
fn http_abort_during_network_error_backoff_stops_retries() {
    let text = run_runtime(
        "http-abort-network-backoff",
        r#"
const external = runtime.http.AbortController();
let calls = 0;
globalThis.fetch = () => { calls++; return Promise.reject(new Error("temporary network error")); };
const pending = runtime.http.fetch("/offline", {
  signal: external.signal,
  retries: 2,
  retryDelay: 100
});
setTimeout(() => external.abort(new Error("caller cancelled")), 5);
pending.then(
  () => console.log("unexpected success"),
  err => console.log(JSON.stringify({ calls, message: err.message }))
);
"#,
    );
    assert!(
        text.contains(r#"{"calls":1,"message":"caller cancelled"}"#),
        "{text}"
    );
}

#[test]
fn http_removes_external_abort_listener_when_request_settles() {
    let text = run_runtime(
        "http-abort-listener-cleanup",
        r#"
const external = {
  aborted: false,
  addEventListener(event, listener) { this.listener = listener; },
  removeEventListener(event, listener) { this.removed = this.listener === listener; }
};
let calls = 0;
globalThis.fetch = () => {
  calls++;
  return Promise.resolve({
    status: 200,
    statusText: "OK",
    url: "/ok",
    headers: { get: () => "text/plain" },
    text: () => Promise.resolve("ok")
  });
};
runtime.http.fetch("/ok", { signal: external, retries: 1 }).then(() => {
  console.log(JSON.stringify({ calls, removed: external.removed }));
});
"#,
    );
    assert!(text.contains(r#"{"calls":1,"removed":true}"#), "{text}");
}

#[test]
fn a_store_can_update_a_nested_path_without_losing_siblings() {
    let text = run_runtime(
        "store-nested",
        r#"
const state = runtime.store({ user: { name: "Ada", age: 36 }, theme: "dark" });
state.set("user.name", "Grace");
console.log(JSON.stringify(state.peek()));
"#,
    );
    assert!(
        text.contains(r#"{"user":{"name":"Grace","age":36},"theme":"dark"}"#),
        "{text}"
    );
}

#[test]
fn batch_runs_an_effect_once_and_untrack_does_not_add_a_dependency() {
    let text = run_runtime(
        "batch-untrack",
        r#"
const first = runtime.signal(0);
const second = runtime.signal(0);
let runs = 0;
runtime.effect(() => {
  runs += 1;
  first.value;
  runtime.untrack(() => second.value);
});
runtime.batch(() => {
  first.set(1);
  first.set(2);
  second.set(1);
});
console.log(runs);
second.set(2);
console.log(runs);
"#,
    );
    assert!(text.contains("2\n2"), "{text}");
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
const first = browser.__h("ul", { class: "list" }, [
  browser.__h("li", { key: "a" }, ["a"]),
  browser.__h("li", { key: "b" }, ["b"])
]);
const page = browser.mount("#app", first);
const list = dom.root.childNodes[0];
const item = list.childNodes[0];
page.update(browser.__h("ul", { class: "list" }, [
  browser.__h("li", { key: "b" }, ["b!"]),
  browser.__h("li", { key: "a" }, ["a!"])
]));
console.log(dom.root.childNodes[0] === list);
console.log(list.childNodes[0] === list.childNodes[1]);
console.log(dom.root.textContent);
console.log(item.getAttribute("key"));
"##,
    );
    // The list is the same node, and the item with the key "b" comes first.
    assert!(text.contains("true"), "{text}");
    assert!(text.contains("b!a!"), "{text}");
}

#[test]
fn mount_update_switches_between_static_and_reactive_pages_and_releases_old_dependencies() {
    let text = run_browser(
        "mount-update-transitions",
        r##"
(async function () {
  const first = browser.signal("one");
  const second = browser.signal("two");
  let secondSubscriptions = 0;
  const subscribeSecond = second.subscribe;
  second.subscribe = function (listener) {
    const unsubscribe = subscribeSecond.call(second, listener);
    secondSubscriptions += 1;
    let active = true;
    return function () {
      if (!active) return;
      active = false;
      secondSubscriptions -= 1;
      unsubscribe();
    };
  };
  const page = browser.mount("#app", browser.__h("p", { title: "initial" }, ["static"]));

  let firstRuns = 0;
  page.update(() => {
    firstRuns += 1;
    return browser.__h("p", {}, ["first " + first.value]);
  });
  console.log("to-reactive", dom.root.textContent, firstRuns);

  first.set("changed");
  await new Promise(resolve => queueMicrotask(resolve));
  console.log("reactive-update", dom.root.textContent, firstRuns);

  page.destroy();
  let initialRuns = 0;
  const livePage = browser.mount("#app", () => {
    initialRuns += 1;
    return browser.__h("p", {}, ["first " + first.value]);
  });
  console.log("initial-reactive", dom.root.textContent, initialRuns);

  let secondRuns = 0;
  livePage.update(() => {
    secondRuns += 1;
    return browser.__h("p", {}, ["second " + second.value]);
  });
  first.set("stale");
  await new Promise(resolve => queueMicrotask(resolve));
  console.log("old-dependency", dom.root.textContent, secondRuns);

  second.set("fresh");
  await new Promise(resolve => queueMicrotask(resolve));
  console.log("new-dependency", dom.root.textContent, secondRuns);

  const staticProps = { title: "held" };
  livePage.update(browser.__h("p", staticProps, ["static again"]));
  console.log("static-unsubscribed", secondSubscriptions);
  staticProps.title = "changed";
  second.set("unused");
  await new Promise(resolve => queueMicrotask(resolve));
  console.log("function-to-static", dom.root.textContent, dom.root.childNodes[0].getAttribute("title"), secondRuns);
})().catch(error => { console.error(error); process.exitCode = 1; });
"##,
    );

    assert!(text.contains("to-reactive first one 1"), "{text}");
    assert!(text.contains("reactive-update first changed 2"), "{text}");
    assert!(text.contains("initial-reactive first changed 1"), "{text}");
    assert!(text.contains("old-dependency second two 1"), "{text}");
    assert!(text.contains("new-dependency second fresh 2"), "{text}");
    assert!(text.contains("static-unsubscribed 0"), "{text}");
    assert!(
        text.contains("function-to-static static again held 2"),
        "{text}"
    );
}

#[test]
fn mount_update_transitions_work_in_headless_chromium_when_available() {
    let chromium = std::env::var("VEY_CHROMIUM").ok().or_else(|| {
        [
            "chromium",
            "chromium-browser",
            "google-chrome",
            "google-chrome-stable",
        ]
        .into_iter()
        .find(|name| {
            Command::new(name)
                .arg("--version")
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        })
        .map(str::to_owned)
    });
    let Some(chromium) = chromium else {
        eprintln!("Skipping real-browser mount test: Chromium is not installed.");
        return;
    };

    let dir = temp_dir("mount-update-chromium");
    fs::write(dir.join("runtime.js"), vey::runtime::source()).unwrap();
    let page = dir.join("index.html");
    fs::write(
        &page,
        r##"<!doctype html>
<div id="app"></div>
<pre id="result"></pre>
<script src="runtime.js"></script>
<script>
const browser = createVeyRuntime(window, null);
const flushEffects = () => new Promise(resolve => queueMicrotask(resolve));
(async function () {
  const first = browser.signal("one");
  const second = browser.signal("two");
  let secondSubscriptions = 0;
  const subscribeSecond = second.subscribe;
  second.subscribe = function (listener) {
    const unsubscribe = subscribeSecond.call(second, listener);
    secondSubscriptions += 1;
    let active = true;
    return function () {
      if (!active) return;
      active = false;
      secondSubscriptions -= 1;
      unsubscribe();
    };
  };

  const staticPage = browser.mount("#app", browser.__h("p", {}, ["static"]));
  let firstRuns = 0;
  staticPage.update(() => {
    firstRuns += 1;
    return browser.__h("p", {}, ["first " + first.value]);
  });
  const staticToReactiveInitial = [document.querySelector("#app").textContent, firstRuns];
  first.set("changed");
  await flushEffects();
  const staticToReactiveUpdate = [document.querySelector("#app").textContent, firstRuns];
  staticPage.destroy();

  let initialRuns = 0;
  const reactivePage = browser.mount("#app", () => {
    initialRuns += 1;
    return browser.__h("p", {}, ["first " + first.value]);
  });
  let secondRuns = 0;
  reactivePage.update(() => {
    secondRuns += 1;
    return browser.__h("p", {}, ["second " + second.value]);
  });
  first.set("stale");
  await flushEffects();
  const staleDependency = [document.querySelector("#app").textContent, secondRuns];
  second.set("fresh");
  await flushEffects();
  const newDependency = [document.querySelector("#app").textContent, secondRuns];

  const props = { title: "held" };
  reactivePage.update(browser.__h("p", props, ["static again"]));
  const subscriptionsAfterStaticUpdate = secondSubscriptions;
  props.title = "changed";
  second.set("unused");
  await flushEffects();
  const staticResult = [
    document.querySelector("#app").textContent,
    document.querySelector("#app p").getAttribute("title"),
    secondRuns,
    secondSubscriptions
  ];

  document.querySelector("#result").textContent = JSON.stringify({
    staticToReactiveInitial,
    staticToReactiveUpdate,
    initialRuns,
    staleDependency,
    newDependency,
    subscriptionsAfterStaticUpdate,
    staticResult
  });
})().catch(error => {
  document.querySelector("#result").textContent = "ERROR: " + String(error && error.stack || error);
});
</script>"##,
    )
    .unwrap();

    let address = format!("file://{}", page.canonicalize().unwrap().display());
    let output = Command::new(chromium)
        .args([
            "--headless",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--allow-file-access-from-files",
            "--dump-dom",
        ])
        .arg(address)
        .output()
        .expect("failed to start Chromium");
    assert!(
        output.status.success(),
        "Chromium failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let rendered = String::from_utf8_lossy(&output.stdout);
    for expected in [
        r#"["first changed",2]"#,
        r#"["second two",1]"#,
        r#"["second fresh",2]"#,
        r#""subscriptionsAfterStaticUpdate":0"#,
        r#"["static again","held",2,0]"#,
    ] {
        assert!(
            rendered.contains(expected),
            "Missing {expected:?}: {rendered}"
        );
    }
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
browser.mount("#app", browser.__h("p", { class: "hello" }, ["hi"]));
console.log(dom.root.childNodes.length);
console.log(dom.root.childNodes[0] === server);
"##,
    );
    assert!(text.contains("1"), "{text}");
    assert!(text.contains("true"), "{text}");
}

#[test]
fn the_events_of_one_s_own_are_attached_to_a_node() {
    let text = run_browser(
        "events",
        r##"
browser.mount("#app", browser.__h("div", {}, [browser.__h("span", { class: "item" }, ["x"])]));
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
const page = browser.mount("#app", browser.__h("p", {}, ["x"]));
page.destroy();
console.log(mounted, destroyed);
const input = browser.ref();
browser.mount("#app", browser.__h("input", { ref: input }, []));
console.log(input.value.tagName);
"##,
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
  return browser.__h("p", {}, ["user " + params.id]);
});
browser.mount("#app", browser.__h("p", {}, ["start"]));
browser.navigate("/users/7");
console.log(JSON.stringify(seen));
console.log(dom.root.textContent);
console.log(browser.routeParams().id, browser.currentRoute().path);
"##,
    );
    assert!(text.contains("[\"7\"]"), "{text}");
    assert!(text.contains("user 7"), "{text}");
    assert!(text.contains("7 /users/7"), "{text}");
}

#[test]
fn optional_route_parameters_backtrack_before_literals_required_parameters_and_wildcards() {
    let text = run_browser(
        "optional-middle-routes",
        r##"
browser.route("/users/{id?:int}/posts/{slug:word}", (params) => {
  return browser.__h("p", {}, [
    (params.id === undefined ? "no id" : "id " + params.id) + " slug " + params.slug
  ]);
});
browser.route("/assets/{version?:int}/static/*", (params) => {
  return browser.__h("p", {}, [
    (params.version === undefined ? "no version" : "version " + params.version) + " files " + params.splat
  ]);
});
browser.route("/events/{id?}/{postId}", (params) => {
  return browser.__h("p", {}, [
    (params.id === undefined ? "no id" : "id " + params.id) + " post " + params.postId
  ]);
});
browser.route("/logs/{version?:int}/*", (params) => {
  return browser.__h("p", {}, [
    (params.version === undefined ? "no version" : "version " + params.version) + " files " + params.splat
  ]);
});
browser.mount("#app", browser.__h("p", {}, ["start"]));

browser.navigate("/users/posts/hello");
console.log("user-skip", JSON.stringify(browser.routeParams()), dom.root.textContent);
browser.navigate("/users/7/posts/hello");
console.log("user-consume", JSON.stringify(browser.routeParams()), dom.root.textContent);
browser.navigate("/assets/static/css/app.css");
console.log("wildcard-skip", JSON.stringify(browser.routeParams()), dom.root.textContent);
browser.navigate("/assets/2/static/js/app.js");
console.log("wildcard-consume", JSON.stringify(browser.routeParams()), dom.root.textContent);
browser.navigate("/events/42");
console.log("required-parameter", JSON.stringify(browser.routeParams()), dom.root.textContent);
browser.navigate("/events/1/42");
console.log("optional-and-required", JSON.stringify(browser.routeParams()), dom.root.textContent);
browser.navigate("/logs/2/js/app.js");
console.log("wildcard-greedy", JSON.stringify(browser.routeParams()), dom.root.textContent);
"##,
    );

    assert!(
        text.contains("user-skip {\"slug\":\"hello\"} no id slug hello"),
        "{text}"
    );
    assert!(
        text.contains("user-consume {\"id\":\"7\",\"slug\":\"hello\"} id 7 slug hello"),
        "{text}"
    );
    assert!(
        text.contains("wildcard-skip {\"splat\":\"css/app.css\"} no version files css/app.css"),
        "{text}"
    );
    assert!(
        text.contains("wildcard-consume {\"version\":\"2\",\"splat\":\"js/app.js\"} version 2 files js/app.js"),
        "{text}"
    );
    assert!(
        text.contains("required-parameter {\"postId\":\"42\"} no id post 42"),
        "{text}"
    );
    assert!(
        text.contains("optional-and-required {\"id\":\"1\",\"postId\":\"42\"} id 1 post 42"),
        "{text}"
    );
    assert!(
        text.contains(
            "wildcard-greedy {\"version\":\"2\",\"splat\":\"js/app.js\"} version 2 files js/app.js"
        ),
        "{text}"
    );
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
fn middleware_paths_accept_route_parameters_and_wildcards() {
    let text = run_runtime(
        "middleware-patterns",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.use("/v/{version}", (req, res, next) => { res.set("X-Version", "yes"); next(); });
app.use("/assets/*", (req, res, next) => { res.set("X-Assets", "yes"); next(); });
app.get("/v/{version}/page", () => "version");
app.get("/assets/file.txt", () => "asset");
app.start(() => {
  const get = path => new Promise(resolve => {
    http.get("http://127.0.0.1:" + app.port() + path, res => {
      res.resume();
      res.on("end", () => resolve([res.statusCode, res.headers["x-version"] || null, res.headers["x-assets"] || null]));
    });
  });
  Promise.all([get("/v/1/page"), get("/assets/file.txt")]).then(async answers => {
    console.log(JSON.stringify(answers));
    await app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(
        text.contains(r#"[[200,"yes",null],[200,null,"yes"]]"#),
        "{text}"
    );
}

#[test]
fn optional_middle_parameters_match_in_server_routes() {
    let text = run_runtime(
        "optional-server-route",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/users/{id?:int}/posts/{slug:word}", req => {
  return (req.params.id === undefined ? "none" : req.params.id) + ":" + req.params.slug;
});
app.start(() => {
  const get = path => new Promise(resolve => {
    http.get("http://127.0.0.1:" + app.port() + path, res => {
      let body = "";
      res.on("data", chunk => body += chunk);
      res.on("end", () => resolve([res.statusCode, body]));
    });
  });
  Promise.all([get("/users/posts/hello"), get("/users/7/posts/hello")]).then(async answers => {
    console.log(JSON.stringify(answers));
    await app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(
        text.contains(r#"[[200,"none:hello"],[200,"7:hello"]]"#),
        "{text}"
    );
}

#[test]
fn response_can_send_files_and_downloads_asynchronously() {
    let text = run_runtime(
        "response-files",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const file = nodePath.join(require("node:os").tmpdir(), "vey-response-file-" + process.pid);
nodeFs.writeFileSync(file, "file body");
const app = runtime.server(0);
app.get("/file", (req, res) => res.sendFile(file));
app.get("/download", (req, res) => res.download(file, "report.txt"));
app.get("/missing", (req, res) => res.sendFile(file + ".missing"));
app.start(() => {
  const get = path => new Promise(resolve => {
    http.get({ hostname: "127.0.0.1", port: app.port(), path, headers: { Accept: "application/json" } }, res => {
      let body = "";
      res.on("data", chunk => body += chunk);
      res.on("end", () => resolve({ status: res.statusCode, body, disposition: res.headers["content-disposition"] || null }));
    });
  });
  Promise.all([get("/file"), get("/download"), get("/missing")]).then(async answers => {
    console.log(JSON.stringify(answers));
    nodeFs.unlinkSync(file);
    await app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(
        text.contains(r#"{"status":200,"body":"file body","disposition":null}"#),
        "{text}"
    );
    assert!(text.contains(r#"{"status":200,"body":"file body","disposition":"attachment; filename=\"report.txt\""}"#), "{text}");
    assert!(text.contains(r#"{"status":404,"body":"404 Nothing here.\nThe link may be broken, or the page may have moved.","disposition":null}"#), "{text}");
}

#[test]
fn send_file_head_preserves_length_and_closes_descriptor() {
    let text = run_runtime(
        "sendfile-head-close",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const file = nodePath.join(require("node:os").tmpdir(), "vey-sendfile-head-" + process.pid);
nodeFs.writeFileSync(file, "head");
const originalClose = nodeFs.close;
let closed = false;
nodeFs.close = function(fd, callback) {
  return originalClose.call(nodeFs, fd, err => {
    if (!err) closed = true;
    if (callback) callback(err);
  });
};
const app = runtime.server(0);
app.get("/file", (req, res) => res.sendFile(file));
app.start(() => {
  const request = http.request({
    host: "127.0.0.1",
    port: app.port(),
    path: "/file",
    method: "HEAD"
  }, res => {
    let bodyLength = 0;
    res.on("data", chunk => { bodyLength += chunk.length; });
    res.on("end", async () => {
      await new Promise(resolve => setTimeout(resolve, 10));
      nodeFs.close = originalClose;
      console.log(JSON.stringify({
        status: res.statusCode,
        length: res.headers["content-length"],
        bodyLength,
        closed
      }));
      nodeFs.unlinkSync(file);
      await app.stop();
      process.exit(0);
    });
  });
  request.end();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":200,"length":"4","bodyLength":0,"closed":true}"#)
    );
}

#[test]
fn send_file_streams_the_file_it_opened_before_a_path_swap() {
    let text = run_runtime(
        "sendfile-open-race",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const parent = nodeFs.mkdtempSync(nodePath.join(require("node:os").tmpdir(), "vey-sendfile-race-"));
const file = nodePath.join(parent, "asset.txt");
const secret = nodePath.join(parent, "secret.txt");
nodeFs.writeFileSync(file, "public");
nodeFs.writeFileSync(secret, "secret");
const originalCreateReadStream = nodeFs.createReadStream;
let swapped = false;
const app = runtime.server(0);
app.get("/file", (req, res) => res.sendFile(file));
app.start(() => {
  nodeFs.createReadStream = function(filename, options) {
    if (!swapped && nodePath.resolve(String(filename)) === nodePath.resolve(file)) {
      nodeFs.unlinkSync(file);
      nodeFs.symlinkSync(secret, file);
      swapped = true;
    }
    return originalCreateReadStream.call(nodeFs, filename, options);
  };
  http.get("http://127.0.0.1:" + app.port() + "/file", res => {
    let body = "";
    res.on("data", chunk => body += chunk);
    res.on("end", async () => {
      nodeFs.createReadStream = originalCreateReadStream;
      console.log(JSON.stringify({
        status: res.statusCode,
        body,
        length: res.headers["content-length"],
        swapped
      }));
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":200,"body":"public","length":"6","swapped":true}"#)
    );
}

#[test]
fn streamed_files_keep_session_cookies_and_sanitize_download_names() {
    let text = run_runtime(
        "sendfile-session-download-name",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const file = nodePath.join(require("node:os").tmpdir(), "vey-sendfile-session-" + process.pid);
nodeFs.writeFileSync(file, "file body");
const app = runtime.server(0);
app.get("/session-file", (req, res) => {
  runtime.session.start();
  runtime.session.set("user", "Ada");
  return res.sendFile(file);
});
app.get("/download", (req, res) => res.download(file, "bad\t\u0000name.txt"));
app.get("/unicode-download", (req, res) => res.download(file, "café.txt"));
app.start(() => {
  const get = path => new Promise(resolve => {
    http.get("http://127.0.0.1:" + app.port() + path, res => {
      let body = "";
      res.on("data", chunk => body += chunk);
      res.on("end", () => resolve({
        status: res.statusCode,
        body,
        cookies: res.headers["set-cookie"] || [],
        disposition: res.headers["content-disposition"] || null
      }));
    });
  });
  Promise.all([get("/session-file"), get("/download"), get("/unicode-download")]).then(async answers => {
    console.log(JSON.stringify({
      session: answers[0],
      download: answers[1],
      unicode: answers[2]
    }));
    nodeFs.unlinkSync(file);
    await app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(
        text.contains(r#""status":200,"body":"file body","cookies":["vey.sid="#),
        "{text}"
    );
    assert!(
        text.contains(r#""disposition":"attachment; filename=\"bad__name.txt\""#),
        "{text}"
    );
    assert!(
        text.contains(
            r#""disposition":"attachment; filename=\"caf_.txt\"; filename*=UTF-8''caf%C3%A9.txt""#
        ),
        "{text}"
    );
}

#[test]
fn send_file_closes_a_late_open_after_request_timeout() {
    let text = run_runtime(
        "sendfile-late-open",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const file = nodePath.join(require("node:os").tmpdir(), "vey-sendfile-late-" + process.pid);
nodeFs.writeFileSync(file, "file body");
const originalOpen = nodeFs.open;
const originalClose = nodeFs.close;
const originalCreateReadStream = nodeFs.createReadStream;
let activeResponse;
let closed = false;
let streamed = false;
let resolveClosed;
const closedPromise = new Promise(resolve => { resolveClosed = resolve; });
nodeFs.open = function(filename, flags, callback) {
  if (nodePath.resolve(String(filename)) === nodePath.resolve(file)) {
    return originalOpen.call(nodeFs, filename, flags, (err, fd) => {
      if (!err && activeResponse && !activeResponse.isFinished()) {
        activeResponse.status(408).send("timed out");
      }
      setTimeout(() => callback(err, fd), 25);
    });
  }
  return originalOpen.call(nodeFs, filename, flags, callback);
};
nodeFs.close = function(fd, callback) {
  return originalClose.call(nodeFs, fd, err => {
    if (!err) {
      closed = true;
      resolveClosed();
    }
    if (callback) callback(err);
  });
};
nodeFs.createReadStream = function() {
  streamed = true;
  return originalCreateReadStream.apply(nodeFs, arguments);
};
const app = runtime.server(0);
app.get("/file", (req, res) => {
  activeResponse = res;
  return res.sendFile(file);
});
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/file", res => {
    res.resume();
    res.on("end", async () => {
      await Promise.race([
        closedPromise,
        new Promise(resolve => setTimeout(resolve, 1000))
      ]);
      nodeFs.open = originalOpen;
      nodeFs.close = originalClose;
      nodeFs.createReadStream = originalCreateReadStream;
      console.log(JSON.stringify({ status: res.statusCode, streamed, closed }));
      nodeFs.unlinkSync(file);
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":408,"streamed":false,"closed":true}"#)
    );
}

#[test]
fn send_file_source_stops_when_the_client_disconnects() {
    let text = run_runtime(
        "sendfile-client-disconnect",
        r#"
const net = require("node:net");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const file = nodePath.join(require("node:os").tmpdir(), "vey-sendfile-large-" + process.pid);
const size = 64 * 1024 * 1024;
nodeFs.writeFileSync(file, Buffer.alloc(size, 97));
const originalCreateReadStream = nodeFs.createReadStream;
let bytesRead = 0;
let streamClosed = false;
let completed = false;
let socket;
let timer;
const app = runtime.server(0, { shutdownTimeout: 500 });
app.get("/file", (req, res) => res.sendFile(file));
app.start(() => {
  nodeFs.createReadStream = function(filename, options) {
    const stream = originalCreateReadStream.call(nodeFs, filename,
      Object.assign({}, options, { highWaterMark: 8192 }));
    stream.on("data", chunk => { bytesRead += chunk.length; });
    stream.on("close", () => {
      streamClosed = true;
      finish();
    });
    return stream;
  };
  socket = net.connect(app.port(), "127.0.0.1");
  socket.on("connect", () => {
    socket.write("GET /file HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
  });
  socket.on("data", () => socket.destroy());
  socket.on("error", () => { });
  timer = setTimeout(finish, 3000);
  function finish() {
    if (completed) return;
    completed = true;
    clearTimeout(timer);
    nodeFs.createReadStream = originalCreateReadStream;
    const stoppedEarly = bytesRead < size;
    console.log(JSON.stringify({ streamClosed, stoppedEarly }));
    socket.destroy();
    app.stop().then(() => {
      nodeFs.unlinkSync(file);
      process.exit(0);
    });
  }
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"streamClosed":true,"stoppedEarly":true}"#)
    );
}

#[test]
fn send_file_obeys_response_backpressure() {
    let text = run_runtime(
        "sendfile-backpressure",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const nodePath = require("node:path");
const file = nodePath.join(require("node:os").tmpdir(), "vey-sendfile-backpressure-" + process.pid);
const size = 128 * 1024;
nodeFs.writeFileSync(file, Buffer.alloc(size, 120));
const originalCreateReadStream = nodeFs.createReadStream;
const originalWrite = http.ServerResponse.prototype.write;
let blocked = false;
let blockedChunks = 0;
let forced = false;
const app = runtime.server(0);
app.get("/file", (req, res) => res.sendFile(file));
app.start(() => {
  nodeFs.createReadStream = function(filename, options) {
    const stream = originalCreateReadStream.call(nodeFs, filename,
      Object.assign({}, options, { highWaterMark: 8192 }));
    stream.on("data", () => { if (blocked) blockedChunks++; });
    return stream;
  };
  http.ServerResponse.prototype.write = function(chunk) {
    const result = originalWrite.apply(this, arguments);
    if (!forced && Buffer.isBuffer(chunk)) {
      forced = true;
      blocked = true;
      const response = this;
      Object.defineProperty(response, "writableNeedDrain", {
        configurable: true,
        writable: true,
        value: true
      });
      setTimeout(() => {
        blocked = false;
        response.writableNeedDrain = false;
        response.emit("drain");
      }, 25);
      return false;
    }
    return result;
  };
  http.get("http://127.0.0.1:" + app.port() + "/file", res => {
    let body = Buffer.alloc(0);
    res.on("data", chunk => { body = Buffer.concat([body, chunk]); });
    res.on("end", async () => {
      nodeFs.createReadStream = originalCreateReadStream;
      http.ServerResponse.prototype.write = originalWrite;
      console.log(JSON.stringify({ status: res.statusCode, length: body.length, blockedChunks, forced }));
      nodeFs.unlinkSync(file);
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":200,"length":131072,"blockedChunks":0,"forced":true}"#)
    );
}

#[test]
fn response_exposes_the_request_that_is_being_handled() {
    let text = run_runtime(
        "response-request",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/check", (req, res) => ({
  samePath: res.req.path === req.path,
  sameMethod: res.req.method === "GET",
  sameObject: res.req === req
}));
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/check", res => {
    let body = "";
    res.on("data", chunk => body += chunk);
    res.on("end", async () => {
      console.log(body);
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert!(
        text.contains(r#"{"samePath":true,"sameMethod":true,"sameObject":true}"#),
        "{text}"
    );
}

#[test]
fn server_runs_shutdown_callbacks_for_sigterm() {
    let text = run_runtime(
        "server-shutdown-signal",
        r#"
const app = runtime.server(0);
let received = null;
app.onShutdown(signal => { received = signal; });
app.start(() => {
  process.emit("SIGTERM");
  setTimeout(() => {
    console.log(received);
    process.exit(received === "SIGTERM" ? 0 : 1);
  }, 50);
});
"#,
    );
    assert!(text.contains("SIGTERM"), "{text}");
}

#[test]
fn cors_answers_preflight_with_the_configured_headers() {
    let text = run_runtime(
        "cors-preflight",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.use(runtime.cors({
  origin: "https://app.example",
  credentials: true,
  methods: "GET, OPTIONS",
  maxAge: 300
}));
app.get("/resource", () => "ok");
app.start(() => {
  const request = http.request("http://127.0.0.1:" + app.port() + "/resource", {
    method: "OPTIONS",
    headers: {
      Origin: "https://app.example",
      "Access-Control-Request-Method": "GET",
      "Access-Control-Request-Headers": "X-Token"
    }
  }, (res) => {
    res.resume();
    res.on("end", async () => {
      console.log(JSON.stringify({
        status: res.statusCode,
        origin: res.headers["access-control-allow-origin"],
        credentials: res.headers["access-control-allow-credentials"],
        methods: res.headers["access-control-allow-methods"],
        headers: res.headers["access-control-allow-headers"],
        maxAge: res.headers["access-control-max-age"],
        vary: res.headers.vary
      }));
      await app.stop();
      process.exit(0);
    });
  });
  request.end();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"status":204,"origin":"https://app.example","credentials":"true","methods":"GET, OPTIONS","headers":"X-Token","maxAge":"300","vary":"Origin"}"#
        )
    );
}

#[test]
fn gzip_compression_sets_vary_and_returns_decodable_bytes() {
    let text = run_runtime(
        "gzip-compress",
        r#"
const http = require("node:http");
const zlib = require("node:zlib");
const app = runtime.server(0);
app.use(runtime.compress({ threshold: 1 }));
app.get("/", () => "compress this response body");
app.start(() => {
  http.get({
    host: "127.0.0.1",
    port: app.port(),
    path: "/",
    headers: { "Accept-Encoding": "gzip" }
  }, (res) => {
    const chunks = [];
    res.on("data", (chunk) => chunks.push(chunk));
    res.on("end", async () => {
      const compressed = Buffer.concat(chunks);
      const decoded = zlib.gunzipSync(compressed).toString("utf8");
      console.log(JSON.stringify({
        status: res.statusCode,
        encoding: res.headers["content-encoding"],
        vary: res.headers.vary,
        length: res.headers["content-length"] || null,
        decoded
      }));
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"status":200,"encoding":"gzip","vary":"Accept-Encoding","length":null,"decoded":"compress this response body"}"#
        )
    );
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
fn a_prototype_named_session_cookie_cannot_pollute_object_prototypes() {
    let text = run_runtime(
        "session-prototype-pollution",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/", () => {
  runtime.session.set("veyPolluted", true);
  return {
    value: runtime.session.get("veyPolluted"),
    polluted: Object.prototype.veyPolluted === true
  };
});
app.start(() => {
  http.get({
    host: "127.0.0.1",
    port: app.port(),
    path: "/",
    headers: { Cookie: "vey.sid=__proto__" }
  }, (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", async () => {
      console.log(body);
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"value":true,"polluted":false}"#)
    );
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
fn the_cookie_of_a_session_is_there_when_the_handler_ends_the_answer() {
    let text = run_runtime(
        "session-cookie",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/login", (req, res) => {
  runtime.session.start();
  runtime.session.set("user", "Ada");
  // The answer ends here, so the cookie of the session must still be
  // written.
  res.send({ user: runtime.session.get("user") });
});
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/login", (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(body);
      console.log((res.headers["set-cookie"] || []).some((value) => value.startsWith("vey.sid=")));
      app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert!(text.contains("\"user\":\"Ada\""), "{text}");
    assert!(text.contains("true"), "{text}");
}

#[test]
fn malformed_json_body_is_rejected_before_the_route_runs() {
    let text = run_runtime(
        "invalid-json-body",
        r#"
const http = require("node:http");
const app = runtime.server(0);
let handled = false;
app.post("/", (req) => {
  handled = true;
  return req.body;
});
app.start(() => {
  const request = http.request({
    host: "127.0.0.1",
    port: app.port(),
    path: "/",
    method: "POST",
    headers: { "Content-Type": "application/json", "Accept": "application/json" }
  }, (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", async () => {
      console.log(JSON.stringify({ status: res.statusCode, body, handled }));
      await app.stop();
      process.exit(0);
    });
  });
  request.end("{\"name\":");
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"status":400,"body":"400 That request could not be read.\nThe body of the request is not valid JSON.","handled":false}"#
        )
    );
}

#[test]
fn session_cookie_security_flags_follow_the_trusted_request_protocol() {
    let text = run_runtime(
        "session-cookie-flags",
        r#"
const http = require("node:http");
async function requestCookie(trustProxy) {
  const app = runtime.server(0, { trustProxy });
  app.get("/login", () => {
    runtime.session.start();
    return "ok";
  });
  await new Promise((resolve) => app.start(resolve));
  const cookie = await new Promise((resolve) => {
    http.get({
      host: "127.0.0.1",
      port: app.port(),
      path: "/login",
      headers: { "X-Forwarded-Proto": "https" }
    }, (res) => {
      res.resume();
      res.on("end", () => resolve((res.headers["set-cookie"] || []).find(value => value.startsWith("vey.sid="))));
    });
  });
  await app.stop();
  return cookie;
}
(async () => {
  const untrusted = await requestCookie(false);
  const trusted = await requestCookie(true);
  const common = ["Path=/", "HttpOnly", "SameSite=Lax"];
  if (!untrusted || !trusted || common.some(flag => !untrusted.includes(flag) || !trusted.includes(flag))) {
    throw new Error(JSON.stringify({ untrusted, trusted }));
  }
  if (untrusted.includes("Secure") || !trusted.includes("Secure")) {
    throw new Error(JSON.stringify({ untrusted, trusted }));
  }
  console.log("session cookie flags verified");
  process.exit(0);
})();
"#,
    );
    assert_eq!(text.lines().last(), Some("session cookie flags verified"));
}

#[test]
fn sessions_expire_and_are_limited_by_configuration() {
    let text = run_runtime(
        "session-limits",
        r#"
const http = require("node:http");
const app = runtime.server(0);
runtime.session.configure({ ttl: 25, idleTimeout: 0, maxSessions: 1 });
app.get("/start", () => { runtime.session.set("value", "kept"); return runtime.session.get("value"); });
app.get("/check", () => runtime.session.get("value") || "missing");
function request(path, cookie) {
  return new Promise(resolve => {
    const opts = { hostname: "127.0.0.1", port: app.port(), path, headers: cookie ? { Cookie: cookie } : {} };
    http.get(opts, res => {
      let body = "";
      res.on("data", chunk => body += chunk);
      res.on("end", () => resolve({ body, cookie: res.headers["set-cookie"] ? res.headers["set-cookie"][0].split(";")[0] : cookie }));
    });
  });
}
app.start(async () => {
  const first = await request("/start");
  const second = await request("/start");
  const limited = await request("/check", first.cookie);
  await new Promise(resolve => setTimeout(resolve, 35));
  const expired = await request("/check", second.cookie);
  console.log(JSON.stringify([first.body, limited.body, expired.body]));
  await app.stop();
  process.exit(0);
});
"#,
    );
    assert!(text.contains(r#"["kept","missing","missing"]"#), "{text}");
}

#[test]
fn node_storage_provides_a_ttl_key_value_store() {
    let text = run_runtime(
        "node-storage",
        r#"
runtime.storage.clear();
runtime.storage.configure({ ttl: 25 });
runtime.storage.set("answer", 42);
runtime.storage.set("other", "yes");
console.log(runtime.storage.get("answer"), JSON.stringify(runtime.storage.keys()));
runtime.storage.remove("other");
setTimeout(() => {
  console.log(runtime.storage.get("answer"), JSON.stringify(runtime.storage.keys()));
}, 35);
"#,
    );
    assert!(text.contains("42 [\"answer\",\"other\"]"), "{text}");
    assert!(text.contains("null []"), "{text}");
}

#[test]
fn request_validation_checks_body_query_and_params() {
    let text = run_runtime(
        "request-validation",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.post("/users/{id}", req => ({ name: req.body.name, id: req.params.id }), runtime.validate({
  body: { type: "object", fields: { name: { type: "string", required: true, min: 2 } } },
  query: { fields: { active: { enum: ["yes", "no"] } } },
  params: { fields: { id: { type: "string", pattern: "^[0-9]+$" } } }
}));
function request(body, path) {
  return new Promise(resolve => {
    const data = JSON.stringify(body);
    const req = http.request({ hostname: "127.0.0.1", port: app.port(), path, method: "POST", headers: { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(data) } }, res => {
      let text = "";
      res.on("data", chunk => text += chunk);
      res.on("end", () => resolve([res.statusCode, text]));
    });
    req.end(data);
  });
}
app.start(async () => {
  const valid = await request({ name: "Ada" }, "/users/7?active=yes");
  const invalid = await request({ name: "A" }, "/users/no?active=maybe");
  console.log(JSON.stringify([valid[0], invalid[0], valid[1].includes("Ada"), invalid[1]]));
  await app.stop();
  process.exit(0);
});
"#,
    );
    assert!(text.contains(r#"[200,400,true,"#), "{text}");
}

#[test]
fn async_fs_operations_do_not_require_sync_helpers() {
    let text = run_runtime(
        "fs-async",
        r#"
const nodeFs = require("node:fs");
const path = require("node:path");
const root = nodeFs.mkdtempSync(path.join(require("node:os").tmpdir(), "vey-fs-async-"));
const first = path.join(root, "first.txt");
const second = path.join(root, "second.txt");
(async () => {
  await runtime.fs.writeAsync(first, "hello");
  await runtime.fs.appendAsync(first, " world");
  const value = await runtime.fs.readAsync(first);
  const info = await runtime.fs.stat(first);
  await runtime.fs.copy(first, second);
  await runtime.fs.rename(second, path.join(root, "renamed.txt"));
  console.log(value, info.size, (await runtime.fs.listAsync(root)).length, await runtime.fs.realpath(first) === nodeFs.realpathSync(first));
  nodeFs.rmSync(root, { recursive: true, force: true });
})();
"#,
    );
    assert!(text.contains("hello world 11 2 true"), "{text}");
}

#[test]
fn path_helpers_cover_normalization_and_parsing() {
    let text = run_runtime(
        "path-helpers",
        r#"
console.log(
  runtime.path.relative("/tmp/project/src", "/tmp/project/lib/index.js"),
  runtime.path.normalize("/tmp/project/../project/file.txt"),
  runtime.path.isAbsolute("/tmp/file"),
  runtime.path.parse("/tmp/file.txt").name,
  runtime.path.parse("/tmp/file.txt").ext,
  runtime.path.sep
);
"#,
    );
    assert!(
        text.contains("../lib/index.js /tmp/project/file.txt true file .txt /"),
        "{text}"
    );
}

#[test]
fn path_exposes_posix_and_win32_namespaces() {
    let text = run_runtime(
        "path-namespaces",
        r#"
console.log(
  runtime.path.posix.join("a", "b"),
  runtime.path.posix.sep,
  runtime.path.win32.join("a", "b"),
  runtime.path.win32.sep,
  runtime.path.win32.parse("C:\\\\work\\\\file.txt").root
);
"#,
    );
    assert!(text.contains("a/b"), "{text}");
    assert!(text.contains("a\\b"), "{text}");
    assert!(text.contains("C:\\"), "{text}");
}

#[test]
fn a_body_that_is_too_large_is_refused_before_it_is_read() {
    let text = run_runtime(
        "body-limit",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.use(runtime.bodyLimit(16));
app.post("/", (req) => ({ size: req.text.length }));
app.start(() => {
  const request = http.request({
    host: "127.0.0.1",
    port: app.port(),
    path: "/",
    method: "POST",
    headers: { "Content-Type": "text/plain", "Content-Length": "64", "Connection": "keep-alive" }
  }, (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(res.statusCode);
      console.log(body);
      console.log(res.headers.connection);
      app.stop();
      process.exit(0);
    });
  });
  request.end("x".repeat(64));
});
"#,
    );
    assert!(text.contains("413"), "{text}");
    assert!(text.contains("too large"), "{text}");
    assert!(text.lines().any(|line| line == "close"), "{text}");
}

#[test]
fn a_chunked_body_that_is_too_large_gets_a_413_response() {
    let text = run_runtime(
        "chunked-body-limit",
        r#"
const http = require("node:http");
const app = runtime.server(0);
let handled = false;
app.use(runtime.bodyLimit(16));
app.post("/", (req) => {
  handled = true;
  return { size: req.text.length };
});
app.start(() => {
  const request = http.request({
    host: "127.0.0.1",
    port: app.port(),
    path: "/",
    method: "POST",
    headers: {
      "Content-Type": "text/plain",
      "Accept": "application/json",
      "Transfer-Encoding": "chunked"
    }
  }, (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(JSON.stringify({ status: res.statusCode, body, handled }));
      process.exit(0);
    });
  });
  request.on("error", (error) => {
    console.log(JSON.stringify({ status: null, error: error.code, handled }));
    process.exit(0);
  });
  request.write("x".repeat(64));
  request.end();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"status":413,"body":"413 That was too large.\nThe body of the request is bigger than this server takes.","handled":false}"#
        )
    );
}

#[test]
fn an_oversized_incomplete_chunked_body_gets_413_and_closes_the_connection() {
    let text = run_runtime(
        "incomplete-chunked-body",
        r#"
const net = require("node:net");
const app = runtime.server(0);
app.use(runtime.bodyLimit(16));
app.post("/", () => "handled");
app.start(() => {
  const socket = net.connect(app.port(), "127.0.0.1");
  let data = "";
  let finished = false;
  let timer;
  function responseDetails() {
    const head = data.split("\r\n\r\n")[0];
    const status = (head.match(/^HTTP\/1\.1\s+(\d+)/) || [])[1] || null;
    const connection = (head.match(/(?:^|\r\n)connection:\s*([^\r\n]+)/i) || [])[1] || null;
    return { status, connection };
  }
  function finish(closed) {
    if (finished) return;
    finished = true;
    clearTimeout(timer);
    console.log(JSON.stringify(Object.assign(responseDetails(), { closed })));
    socket.destroy();
    app.stop().then(() => process.exit(0));
  }
  socket.on("connect", () => {
    socket.write(
      "POST / HTTP/1.1\r\n" +
      "Host: 127.0.0.1\r\n" +
      "Transfer-Encoding: chunked\r\n" +
      "Connection: keep-alive\r\n\r\n" +
      "40\r\n" + "x".repeat(64) + "\r\n"
    );
  });
  socket.on("data", chunk => { data += chunk.toString("latin1"); });
  socket.on("error", () => {});
  socket.on("close", () => finish(true));
  timer = setTimeout(() => finish(false), 1000);
});
"#,
    );
    assert!(
        text.contains(r#"{"status":"413","connection":"close","closed":true}"#),
        "{text}"
    );
}

#[test]
fn an_explicit_options_route_handles_the_request() {
    let text = run_runtime(
        "options-route",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.route("OPTIONS", "/resource", (req, res) => {
  res.status(202);
  res.set("X-Options-Route", "handled");
  return { route: "explicit" };
});
app.start(() => {
  const request = http.request(
    "http://127.0.0.1:" + app.port() + "/resource",
    { method: "OPTIONS" },
    (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", async () => {
        console.log(JSON.stringify({
          status: res.statusCode,
          route: res.headers["x-options-route"],
          body
        }));
        await app.stop();
        process.exit(0);
      });
    }
  );
  request.end();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":202,"route":"handled","body":"{\"route\":\"explicit\"}"}"#)
    );
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
fn response_can_write_headers_and_keep_request_locals() {
    let text = run_runtime(
        "response-headers",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.get("/", (req, res) => {
  res.locals.answer = "hello";
  res.writeHead(201, { "X-Answer": res.locals.answer });
  res.write(res.locals.answer);
  res.end();
});
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/", (res) => {
    let body = "";
    res.on("data", chunk => body += chunk);
    res.on("end", async () => {
      console.log(JSON.stringify({ status: res.statusCode, header: res.headers["x-answer"], body }));
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":201,"header":"hello","body":"hello"}"#)
    );
}

#[test]
fn static_files_do_not_follow_symlinks_outside_the_root() {
    let text = run_runtime(
        "static-symlink",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const parent = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-static-link-"));
const root = path.join(parent, "public");
const secret = path.join(parent, "secret.txt");
nodeFs.mkdirSync(root);
nodeFs.writeFileSync(secret, "private data");
nodeFs.symlinkSync(secret, path.join(root, "secret.txt"));
const assets = path.join(root, "assets");
nodeFs.mkdirSync(assets);
nodeFs.symlinkSync(secret, path.join(assets, "index.html"));
const app = runtime.server(0);
app.static(root);
app.start(() => {
  const get = (pathname) => new Promise((resolve) => {
    http.get("http://127.0.0.1:" + app.port() + pathname, (res) => {
      res.resume();
      res.on("end", () => resolve(res.statusCode));
    });
  });
  (async () => {
    const direct = await get("/secret.txt");
    const index = await get("/assets/");
    const traversal = await get("/%2e%2e/secret.txt");
    console.log(JSON.stringify({ direct, index, traversal }));
    await app.stop();
    process.exit(0);
  })();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"direct":403,"index":403,"traversal":403}"#)
    );
}

#[test]
fn static_file_open_rechecks_the_resolved_target() {
    let text = run_runtime(
        "static-symlink-open-race",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const parent = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-static-open-race-"));
const root = path.join(parent, "public");
const asset = path.join(root, "asset.txt");
const secret = path.join(parent, "secret.txt");
nodeFs.mkdirSync(root);
nodeFs.writeFileSync(asset, "public data");
nodeFs.writeFileSync(secret, "private data");
// The root may be reached through a symbolic link, as it is on macOS, so the
// swap is keyed by the real path that the server resolves.
const realAsset = path.join(nodeFs.realpathSync(root), "asset.txt");
const originalRealpathSync = nodeFs.realpathSync;
let swapped = false;
nodeFs.realpathSync = function(filename, options) {
  const resolved = originalRealpathSync.call(nodeFs, filename, options);
  if (!swapped && path.resolve(String(filename)) === path.resolve(realAsset)) {
    nodeFs.unlinkSync(asset);
    nodeFs.symlinkSync(secret, asset);
    swapped = true;
  }
  return resolved;
};
const app = runtime.server(0);
app.static(root);
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/asset.txt", { headers: { Accept: "application/json" } }, res => {
    let body = "";
    res.on("data", chunk => body += chunk);
    res.on("end", async () => {
      nodeFs.realpathSync = originalRealpathSync;
      console.log(JSON.stringify({ status: res.statusCode, body, swapped }));
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"status":403,"body":"403 That file is not served.\nThis path is outside the folder of the files.","swapped":true}"#
        )
    );
}

#[test]
fn static_file_streaming_uses_the_file_checked_for_containment() {
    let text = run_runtime(
        "static-symlink-race",
        r#"
const net = require("node:net");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const parent = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-static-race-"));
const root = path.join(parent, "public");
const asset = path.join(root, "asset.txt");
const secret = path.join(parent, "secret.txt");
nodeFs.mkdirSync(root);
nodeFs.writeFileSync(asset, "public data");
nodeFs.writeFileSync(secret, "private data");
const originalCreateReadStream = nodeFs.createReadStream;
const app = runtime.server(0);
app.static(root);
app.start(() => {
  const socket = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0);
  let swapped = false;
  let finished = false;
  let timer;
  function finish() {
    if (finished) return;
    finished = true;
    clearTimeout(timer);
    nodeFs.createReadStream = originalCreateReadStream;
    const raw = data.toString("latin1");
    const split = raw.indexOf("\r\n\r\n");
    const head = split < 0 ? raw : raw.slice(0, split);
    const body = split < 0 ? "" : raw.slice(split + 4);
    const status = (head.match(/^HTTP\/1\.1\s+(\d+)/) || [])[1] || null;
    console.log(JSON.stringify({ status, body, swapped }));
    socket.destroy();
    app.stop().then(() => process.exit(0));
  }
  nodeFs.createReadStream = function(filename, options) {
    if (!swapped) {
      nodeFs.unlinkSync(asset);
      nodeFs.symlinkSync(secret, asset);
      swapped = true;
    }
    return originalCreateReadStream.call(nodeFs, filename, options);
  };
  socket.on("connect", () => {
    socket.write("GET /asset.txt HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
  });
  socket.on("data", chunk => { data = Buffer.concat([data, chunk]); });
  socket.on("error", finish);
  socket.on("end", finish);
  timer = setTimeout(finish, 1000);
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"status":"200","body":"public data","swapped":true}"#)
    );
}

#[test]
fn static_range_open_rejects_a_swapped_outside_symlink() {
    let text = run_runtime(
        "static-range-open-race",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const parent = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-static-range-race-"));
const root = path.join(parent, "public");
const asset = path.join(root, "asset.txt");
const secret = path.join(parent, "secret.txt");
nodeFs.mkdirSync(root);
nodeFs.writeFileSync(asset, "public data");
nodeFs.writeFileSync(secret, "private data");
// The real path is the one the server opens when the temporary folder is
// behind a symbolic link.
const realAsset = path.join(nodeFs.realpathSync(root), "asset.txt");
const originalOpenSync = nodeFs.openSync;
let swapped = false;
const app = runtime.server(0);
app.static(root);
app.start(() => {
  nodeFs.openSync = function(filename, flags, mode) {
    if (!swapped && path.resolve(String(filename)) === path.resolve(realAsset)) {
      nodeFs.unlinkSync(asset);
      nodeFs.symlinkSync(secret, asset);
      swapped = true;
    }
    return originalOpenSync.call(nodeFs, filename, flags, mode);
  };
  http.get("http://127.0.0.1:" + app.port() + "/asset.txt", {
    headers: { Range: "bytes=0-5", Accept: "application/json" }
  }, res => {
    let body = "";
    res.on("data", chunk => body += chunk);
    res.on("end", async () => {
      nodeFs.openSync = originalOpenSync;
      console.log(JSON.stringify({ status: res.statusCode, body, swapped }));
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"status":403,"body":"403 That file is not served.\nThis path is outside the folder of the files.","swapped":true}"#
        )
    );
}

#[test]
fn head_and_range_requests_for_static_files_preserve_headers_and_bytes() {
    let text = run_runtime(
        "static-head-range",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const root = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-static-range-"));
nodeFs.writeFileSync(path.join(root, "data.txt"), "0123456789");
const app = runtime.server(0);
app.static(root);
app.start(() => {
  const ask = (method, headers) => new Promise((resolve) => {
    const request = http.request(
      "http://127.0.0.1:" + app.port() + "/data.txt",
      { method, headers },
      (res) => {
        let body = "";
        res.on("data", (chunk) => { body += chunk; });
        res.on("end", () => resolve({
          status: res.statusCode,
          length: res.headers["content-length"],
          range: res.headers["content-range"] || null,
          body
        }));
      }
    );
    request.end();
  });
  (async () => {
    const head = await ask("HEAD", {});
    const range = await ask("GET", { Range: "bytes=2-5" });
    console.log(JSON.stringify({ head, range }));
    await app.stop();
    process.exit(0);
  })();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"head":{"status":200,"length":"10","range":null,"body":""},"range":{"status":206,"length":"4","range":"bytes 2-5/10","body":"2345"}}"#
        )
    );
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
fn error_handlers_are_isolated_between_server_instances() {
    let text = run_runtime(
        "isolated-error-handlers",
        r#"
const http = require("node:http");
const first = runtime.server(0);
const second = runtime.server(0);
first.use(runtime.errorHandler((error, req, res) => res.send("first:" + error.message)));
second.use(runtime.errorHandler((error, req, res) => res.send("second:" + error.message)));
first.get("/boom", () => new Promise((resolve, reject) => {
  setTimeout(() => reject(new Error("A")), 40);
}));
second.get("/boom", () => { throw new Error("B"); });
function ask(app) {
  return new Promise((resolve) => {
    http.get("http://127.0.0.1:" + app.port() + "/boom", (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", () => resolve(body));
    });
  });
}
Promise.all([
  new Promise((resolve) => first.start(resolve)),
  new Promise((resolve) => second.start(resolve))
]).then(async () => {
  const firstBody = ask(first);
  const secondBody = ask(second);
  console.log(JSON.stringify(await Promise.all([firstBody, secondBody])));
  await first.stop();
  await second.stop();
  process.exit(0);
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"["first:A","second:B"]"#),
        "{text}"
    );
}

#[test]
fn error_handlers_can_pass_errors_to_the_next_handler() {
    let text = run_runtime(
        "error-handler-chain",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.use(runtime.errorHandler((error, req, res, next) => next(new Error("forwarded:" + error.message))));
app.use(runtime.errorHandler((error, req, res) => res.send(error.message)));
app.get("/boom", () => { throw new Error("original"); });
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/boom", res => {
    let body = "";
    res.on("data", chunk => body += chunk);
    res.on("end", async () => {
      console.log(body);
      await app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert!(text.contains("forwarded:original"), "{text}");
}

#[test]
fn custom_not_found_and_error_handlers_control_their_responses() {
    let text = run_runtime(
        "not-found-error-handlers",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.notFound((req) => "missing: " + req.path);
app.error((error, req, res) => {
  res.status(503);
  res.send({ message: error.message, path: req.path });
});
app.get("/boom", () => { throw new Error("route failed"); });
function ask(pathname) {
  return new Promise((resolve) => {
    http.get("http://127.0.0.1:" + app.port() + pathname, (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", () => resolve({ status: res.statusCode, body }));
    });
  });
}
app.start(() => {
  (async () => {
    const missing = await ask("/missing");
    const failed = await ask("/boom");
    console.log(JSON.stringify({ missing, failed }));
    await app.stop();
    process.exit(0);
  })();
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(
            r#"{"missing":{"status":404,"body":"missing: /missing"},"failed":{"status":503,"body":"{\"message\":\"route failed\",\"path\":\"/boom\"}"}}"#
        )
    );
}

#[test]
fn a_port_conflict_is_reported_without_crashing_the_process() {
    let text = run_runtime(
        "port-conflict",
        r#"
const net = require("node:net");
const blocker = net.createServer();
blocker.listen(0, () => {
  const app = runtime.server(blocker.address().port);
  app.onError((error) => {
    console.log(JSON.stringify({ code: error.code, lastError: app.lastError().code }));
    blocker.close(() => process.exit(0));
  });
  app.start(() => {
    console.log("unexpected server start");
    process.exit(1);
  });
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"code":"EADDRINUSE","lastError":"EADDRINUSE"}"#)
    );
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
fn stopping_after_an_sse_client_disconnects_completes() {
    let text = run_runtime(
        "sse-disconnect-stop",
        r#"
const http = require("node:http");
const app = runtime.server(0, { shutdownTimeout: 250 });
app.sse("/events", (req, stream) => stream.send("ready"));
app.start(() => {
  const request = http.get("http://127.0.0.1:" + app.port() + "/events", (res) => {
    res.once("data", () => {
      res.destroy();
      res.once("close", async () => {
        const stopped = await Promise.race([
          app.stop().then(() => true),
          new Promise((resolve) => setTimeout(() => resolve(false), 500))
        ]);
        console.log(stopped);
        process.exit(0);
      });
    });
  });
  request.on("error", () => {});
});
"#,
    );
    assert_eq!(text.lines().last(), Some("true"), "{text}");
}

#[test]
fn sse_stream_supports_ids_retry_and_heartbeats() {
    let text = run_runtime(
        "sse-features",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.sse("/events", (req, stream) => {
  stream.retry(1500);
  stream.send("hello", "42");
  stream.heartbeat(10);
});
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/events", res => {
    let body = "";
    res.on("data", chunk => {
      body += chunk;
      if (body.includes(": heartbeat")) {
        res.destroy();
        app.stop().then(() => { console.log(body); process.exit(0); });
      }
    });
  });
});
"#,
    );
    assert!(text.contains("retry: 1500"), "{text}");
    assert!(text.contains("id: 42"), "{text}");
    assert!(text.contains(": heartbeat"), "{text}");
}

#[test]
fn sse_stops_writing_when_the_client_is_backpressured() {
    let text = run_runtime(
        "sse-backpressure",
        r#"
const http = require("node:http");
const originalWrite = http.ServerResponse.prototype.write;
let sseWrites = 0;
http.ServerResponse.prototype.write = function (chunk, ...args) {
  if (this.req && this.req.url === "/events") {
    sseWrites++;
    return false;
  }
  return originalWrite.call(this, chunk, ...args);
};
const app = runtime.server(0, { shutdownTimeout: 250 });
app.sse("/events", (req, stream) => {
  for (let i = 0; i < 100; i++) stream.send("event-" + i);
});
app.start(() => {
  const request = http.get("http://127.0.0.1:" + app.port() + "/events", (res) => {
    res.on("error", () => {});
    res.resume();
  });
  request.on("error", () => {});
  setTimeout(async () => {
    await app.stop();
    console.log(sseWrites);
    process.exit(0);
  }, 50);
});
setTimeout(() => process.exit(2), 1500);
"#,
    );
    assert_eq!(text.lines().last(), Some("1"), "{text}");
}

#[test]
fn sse_rejects_an_oversized_single_frame() {
    let text = run_runtime(
        "sse-frame-size",
        r#"
const http = require("node:http");
const originalWrite = http.ServerResponse.prototype.write;
let sseWrites = 0;
http.ServerResponse.prototype.write = function (chunk, ...args) {
  if (this.req && this.req.url === "/events") sseWrites++;
  return originalWrite.call(this, chunk, ...args);
};
const app = runtime.server(0, { shutdownTimeout: 250 });
app.sse("/events", (req, stream) => {
  stream.send("x".repeat(1024 * 1024 + 1));
  stream.send("after oversized event");
});
app.start(() => {
  const request = http.get("http://127.0.0.1:" + app.port() + "/events", (res) => {
    res.on("error", () => {});
    res.resume();
  });
  request.on("error", () => {});
  setTimeout(async () => {
    await app.stop();
    console.log(sseWrites);
    process.exit(0);
  }, 50);
});
setTimeout(() => process.exit(2), 1500);
"#,
    );
    assert_eq!(text.lines().last(), Some("0"), "{text}");
}

#[test]
fn websocket_64_bit_lengths_are_checked_before_reading_the_payload() {
    let text = run_runtime(
        "websocket-64-bit-length",
        r#"
const net = require("node:net");
const crypto = require("node:crypto");
const app = runtime.server(0);
let handled = false;
app.ws("/echo", (socket) => {
  socket.onMessage(() => { handled = true; });
});
app.start(() => {
  const client = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0);
  let upgraded = false;
  client.on("connect", () => {
    client.write([
      "GET /echo HTTP/1.1",
      "Host: 127.0.0.1:" + app.port(),
      "Upgrade: websocket",
      "Connection: Upgrade",
      "Sec-WebSocket-Key: " + crypto.randomBytes(16).toString("base64"),
      "Sec-WebSocket-Version: 13",
      "",
      ""
    ].join("\r\n"));
  });
  client.on("data", (chunk) => {
    data = Buffer.concat([data, chunk]);
    if (!upgraded) {
      const end = data.indexOf("\r\n\r\n");
      if (end < 0) return;
      if (!data.toString("ascii", 0, end).startsWith("HTTP/1.1 101")) throw new Error("upgrade failed");
      upgraded = true;
      data = data.slice(end + 4);
      const frame = Buffer.alloc(10);
      frame[0] = 0x81;
      frame[1] = 0xff;
      frame.writeUInt32BE(1, 2);
      frame.writeUInt32BE(0, 6);
      client.write(frame);
    }
    if (data.length >= 2 && (data[0] & 0x0f) === 0x8) {
      console.log(JSON.stringify({ close: true, handled }));
      client.destroy();
      app.stop().then(() => process.exit(0));
    }
  });
  setTimeout(() => process.exit(2), 1000);
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"close":true,"handled":false}"#),
        "{text}"
    );
}

#[test]
fn websocket_text_fragments_are_delivered_as_one_message() {
    let text = run_runtime(
        "websocket-fragments",
        r#"
const net = require("node:net");
const crypto = require("node:crypto");
const app = runtime.server(0);
app.ws("/echo", (socket) => {
  socket.onMessage((message) => socket.send("echo:" + message));
});
function clientFrame(fin, opcode, text) {
  const payload = Buffer.from(text, "utf8");
  const mask = Buffer.from([9, 3, 7, 1]);
  const frame = Buffer.alloc(2 + 4 + payload.length);
  frame[0] = (fin ? 0x80 : 0) | opcode;
  frame[1] = 0x80 | payload.length;
  mask.copy(frame, 2);
  for (let i = 0; i < payload.length; i++) frame[6 + i] = payload[i] ^ mask[i % 4];
  return frame;
}
app.start(() => {
  const client = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0);
  let upgraded = false;
  let replies = [];
  client.on("connect", () => {
    const key = crypto.randomBytes(16).toString("base64");
    client.write([
      "GET /echo HTTP/1.1",
      "Host: 127.0.0.1:" + app.port(),
      "Upgrade: websocket",
      "Connection: Upgrade",
      "Sec-WebSocket-Key: " + key,
      "Sec-WebSocket-Version: 13",
      "",
      ""
    ].join("\r\n"));
  });
  client.on("data", (chunk) => {
    if (replies.length > 0) return;
    data = Buffer.concat([data, chunk]);
    if (!upgraded) {
      const end = data.indexOf("\r\n\r\n");
      if (end < 0) return;
      const headers = data.toString("ascii", 0, end);
      if (!headers.startsWith("HTTP/1.1 101")) throw new Error(headers);
      upgraded = true;
      data = data.slice(end + 4);
      client.write(Buffer.concat([
        clientFrame(false, 0x1, "hel"),
        clientFrame(true, 0x0, "lo")
      ]));
    }
    while (data.length >= 2) {
      const opcode = data[0] & 0x0f;
      const length = data[1] & 0x7f;
      const headerLength = length < 126 ? 2 : 4;
      if (data.length < headerLength + length) return;
      if (opcode === 0x1) {
        replies.push(data.subarray(headerLength, headerLength + length).toString("utf8"));
      }
      data = data.slice(headerLength + length);
      if (replies.length > 0) {
        (async () => {
          client.end();
          await app.stop();
          console.log(JSON.stringify(replies));
          process.exit(0);
        })();
        return;
      }
    }
  });
  setTimeout(() => process.exit(2), 1000);
});
"#,
    );
    assert_eq!(text.lines().last(), Some(r#"["echo:hello"]"#), "{text}");
}

#[test]
fn websocket_binary_frames_stay_binary() {
    let text = run_runtime(
        "websocket-binary",
        r#"
const net = require("node:net");
const crypto = require("node:crypto");
const app = runtime.server(0);
app.ws("/binary", socket => socket.onMessage(message => {
  if (!Buffer.isBuffer(message)) return socket.close();
  socket.send(message);
}));
function frame(opcode, payload) {
  const mask = Buffer.from([1, 2, 3, 4]);
  const out = Buffer.alloc(6 + payload.length);
  out[0] = 0x80 | opcode;
  out[1] = 0x80 | payload.length;
  mask.copy(out, 2);
  for (let i = 0; i < payload.length; i++) out[6 + i] = payload[i] ^ mask[i % 4];
  return out;
}
app.start(() => {
  const client = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0);
  let upgraded = false;
  client.on("connect", () => {
    const key = crypto.randomBytes(16).toString("base64");
    client.write(["GET /binary HTTP/1.1", "Host: 127.0.0.1:" + app.port(), "Upgrade: websocket", "Connection: Upgrade", "Sec-WebSocket-Key: " + key, "Sec-WebSocket-Version: 13", "", ""].join("\r\n"));
  });
  client.on("data", chunk => {
    data = Buffer.concat([data, chunk]);
    if (!upgraded) {
      const end = data.indexOf("\r\n\r\n");
      if (end < 0) return;
      upgraded = true;
      data = data.slice(end + 4);
      client.write(frame(0x2, Buffer.from([7, 8, 9])));
    }
    if (data.length >= 5 && (data[0] & 0x0f) === 0x2) {
      console.log(data[1] === 3 && data[2] === 7 && data[3] === 8 && data[4] === 9);
      client.end();
      app.stop().then(() => process.exit(0));
    }
  });
  setTimeout(() => process.exit(2), 1000);
});
"#,
    );
    assert!(text.contains("true"), "{text}");
}

#[test]
fn websocket_close_carries_code_and_reason() {
    let text = run_runtime(
        "websocket-close",
        r#"
const net = require("node:net");
const crypto = require("node:crypto");
const app = runtime.server(0);
app.ws("/close", socket => socket.onMessage(() => socket.close(1008, "policy")));
function frame(text) {
  const payload = Buffer.from(text);
  const mask = Buffer.from([1, 2, 3, 4]);
  const out = Buffer.alloc(6 + payload.length);
  out[0] = 0x81; out[1] = 0x80 | payload.length; mask.copy(out, 2);
  for (let i = 0; i < payload.length; i++) out[6 + i] = payload[i] ^ mask[i % 4];
  return out;
}
app.start(() => {
  const client = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0); let upgraded = false;
  client.on("connect", () => {
    const key = crypto.randomBytes(16).toString("base64");
    client.write(["GET /close HTTP/1.1", "Host: 127.0.0.1:" + app.port(), "Upgrade: websocket", "Connection: Upgrade", "Sec-WebSocket-Key: " + key, "Sec-WebSocket-Version: 13", "", ""].join("\r\n"));
  });
  client.on("data", chunk => {
    data = Buffer.concat([data, chunk]);
    if (!upgraded) {
      const end = data.indexOf("\r\n\r\n"); if (end < 0) return;
      upgraded = true; data = data.slice(end + 4); client.write(frame("x"));
    }
    if (data.length >= 10 && (data[0] & 0x0f) === 8) {
      console.log(data.readUInt16BE(2) === 1008 && data.slice(4, 10).toString() === "policy");
      client.end(); app.stop().then(() => process.exit(0));
    }
  });
  setTimeout(() => process.exit(2), 1000);
});
"#,
    );
    assert!(text.contains("true"), "{text}");
}

#[test]
fn websocket_fragmented_messages_obey_the_total_size_limit() {
    let text = run_runtime(
        "websocket-fragment-limit",
        r#"
const net = require("node:net");
const crypto = require("node:crypto");
const app = runtime.server(0, { maxFrame: 5 });
let handled = false;
app.ws("/echo", (socket) => {
  socket.onMessage(() => { handled = true; });
});
function clientFrame(fin, opcode, text) {
  const payload = Buffer.from(text, "utf8");
  const mask = Buffer.from([9, 3, 7, 1]);
  const frame = Buffer.alloc(6 + payload.length);
  frame[0] = (fin ? 0x80 : 0) | opcode;
  frame[1] = 0x80 | payload.length;
  mask.copy(frame, 2);
  for (let i = 0; i < payload.length; i++) frame[6 + i] = payload[i] ^ mask[i % 4];
  return frame;
}
app.start(() => {
  const client = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0);
  let upgraded = false;
  client.on("connect", () => {
    client.write([
      "GET /echo HTTP/1.1",
      "Host: 127.0.0.1:" + app.port(),
      "Upgrade: websocket",
      "Connection: Upgrade",
      "Sec-WebSocket-Key: " + crypto.randomBytes(16).toString("base64"),
      "Sec-WebSocket-Version: 13",
      "",
      ""
    ].join("\r\n"));
  });
  client.on("data", (chunk) => {
    data = Buffer.concat([data, chunk]);
    if (!upgraded) {
      const end = data.indexOf("\r\n\r\n");
      if (end < 0) return;
      if (!data.toString("ascii", 0, end).startsWith("HTTP/1.1 101")) throw new Error("upgrade failed");
      upgraded = true;
      data = data.slice(end + 4);
      client.write(Buffer.concat([
        clientFrame(false, 0x1, "1234"),
        clientFrame(true, 0x0, "5678")
      ]));
    }
    if (data.length >= 2 && (data[0] & 0x0f) === 0x8) {
      console.log(JSON.stringify({ close: true, handled }));
      client.destroy();
      app.stop().then(() => process.exit(0));
    }
  });
  setTimeout(() => process.exit(2), 1000);
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"close":true,"handled":false}"#),
        "{text}"
    );
}

#[test]
fn websocket_unmasked_client_frames_are_rejected() {
    let text = run_runtime(
        "websocket-unmasked",
        r#"
const net = require("node:net");
const crypto = require("node:crypto");
const app = runtime.server(0);
let handled = false;
app.ws("/echo", (socket) => {
  socket.onMessage(() => { handled = true; });
});
app.start(() => {
  const client = net.connect(app.port(), "127.0.0.1");
  let data = Buffer.alloc(0);
  let upgraded = false;
  client.on("connect", () => {
    client.write([
      "GET /echo HTTP/1.1",
      "Host: 127.0.0.1:" + app.port(),
      "Upgrade: websocket",
      "Connection: Upgrade",
      "Sec-WebSocket-Key: " + crypto.randomBytes(16).toString("base64"),
      "Sec-WebSocket-Version: 13",
      "",
      ""
    ].join("\r\n"));
  });
  client.on("data", (chunk) => {
    data = Buffer.concat([data, chunk]);
    if (!upgraded) {
      const end = data.indexOf("\r\n\r\n");
      if (end < 0) return;
      if (!data.toString("ascii", 0, end).startsWith("HTTP/1.1 101")) throw new Error("upgrade failed");
      upgraded = true;
      data = data.slice(end + 4);
      client.write(Buffer.from([0x81, 0x01, 0x78]));
    }
    if (data.length >= 2 && (data[0] & 0x0f) === 0x8) {
      console.log(JSON.stringify({ close: true, handled }));
      client.destroy();
      app.stop().then(() => process.exit(0));
    }
  });
  setTimeout(() => process.exit(2), 1000);
});
"#,
    );
    assert_eq!(
        text.lines().last(),
        Some(r#"{"close":true,"handled":false}"#),
        "{text}"
    );
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

#[test]
fn the_lists_of_the_runtime_are_the_lists_of_the_compiler() {
    // The highlighter of the runtime has its own list of the words and of
    // the tags of the language. They are copies, so a test keeps them in
    // step with the compiler.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let lexer = fs::read_to_string(root.join("src/lexer/mod.rs")).unwrap();
    // The words of the language are the ones of `is_keyword`.
    let body = lexer
        .split("pub fn is_keyword")
        .nth(1)
        .and_then(|rest| rest.split_once('{'))
        .map(|(_, body)| body)
        .unwrap_or("");
    let body = body.split('}').next().unwrap_or("");
    let mut words: Vec<String> = Vec::new();
    for word in body.split('"').skip(1).step_by(2) {
        if !word.is_empty() && !words.contains(&word.to_string()) {
            words.push(word.to_string());
        }
    }
    words.push("type".to_string());
    words.sort();

    let html = fs::read_to_string(root.join("src/parser/html.rs")).unwrap();
    let mut tags: Vec<String> = Vec::new();
    for line in html.lines() {
        let line = line.trim().trim_end_matches(',');
        if !line.starts_with('"') || !line.ends_with('"') {
            continue;
        }
        let tag = line.trim_matches('"');
        if !tag.is_empty() && !tags.contains(&tag.to_string()) {
            tags.push(tag.to_string());
        }
    }
    tags.sort();

    let runtime = fs::read_to_string(root.join("src/runtime/highlight.js")).unwrap();
    let runtime_words = list_of(&runtime, "var keywords = [", "]");
    let runtime_tags = list_of(&runtime, "var htmlTags = (", ")");

    assert_eq!(
        runtime_words, words,
        "the list of the words of the runtime has drifted"
    );
    assert_eq!(
        runtime_tags, tags,
        "the list of the tags of the runtime has drifted"
    );
}

/// The words of a list of the runtime, read from its source.
fn list_of(source: &str, start: &str, end: &str) -> Vec<String> {
    let Some(index) = source.find(start) else {
        panic!("the list '{}' is not there", start);
    };
    let rest = &source[index + start.len()..];
    let body = &rest[..rest.find(end).unwrap_or(0)];
    let mut out: Vec<String> = Vec::new();
    for piece in body.split('"').skip(1).step_by(2) {
        // A list of the runtime may be written in parts.
        for word in piece.split(|c: char| c == ',' || c.is_whitespace()) {
            let word = word.trim();
            if word.is_empty() {
                continue;
            }
            if !out.contains(&word.to_string()) {
                out.push(word.to_string());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_server_has_the_pieces_that_keep_a_browser_from_doing_what_it_should_not() {
    let text = run_runtime(
        "security",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.use(runtime.security());
app.get("/", (req) => "hello");
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/", (res) => {
    console.log(res.headers["x-content-type-options"]);
    console.log(res.headers["x-frame-options"]);
    console.log(!!res.headers["content-security-policy"]);
    app.stop();
    process.exit(0);
  });
});
"#,
    );
    assert!(text.contains("nosniff"), "{text}");
    assert!(text.contains("SAMEORIGIN"), "{text}");
    assert!(text.contains("true"), "{text}");
}

#[test]
fn the_requests_of_a_visitor_are_limited() {
    let text = run_runtime(
        "rate-limit",
        r#"
const http = require("node:http");
const app = runtime.server(0);
app.use(runtime.rateLimit({ limit: 2 }));
app.get("/", (req) => "hello");
app.start(() => {
  const ask = () => new Promise((resolve) => {
    http.get("http://127.0.0.1:" + app.port() + "/", (res) => {
      res.on("data", () => {});
      res.on("end", () => resolve(res.statusCode));
    });
  });
  (async () => {
    console.log(await ask());
    console.log(await ask());
    console.log(await ask());
    app.stop();
    process.exit(0);
  })();
});
"#,
    );
    assert!(text.contains("200"), "{text}");
    assert!(text.contains("429"), "{text}");
}

#[test]
fn forwarded_ip_and_protocol_are_used_only_when_proxy_is_trusted() {
    let text = run_runtime(
        "trust-proxy",
        r#"
const http = require("node:http");
function listen(trustProxy) {
  return new Promise((resolve) => {
    const app = runtime.server(0, { trustProxy });
    app.get("/", (req) => ({ ip: req.ip, protocol: req.protocol }));
    app.start(() => resolve(app));
  });
}
function ask(app) {
  return new Promise((resolve) => {
    http.get({
      host: "127.0.0.1",
      port: app.port(),
      path: "/",
      headers: {
        "X-Forwarded-For": "203.0.113.42, 10.0.0.1",
        "X-Forwarded-Proto": "https, http"
      }
    }, (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", () => resolve(JSON.parse(body)));
    });
  });
}
(async () => {
  const untrusted = await listen(false);
  const untrustedRequest = await ask(untrusted);
  await untrusted.stop();
  const trusted = await listen(true);
  const trustedRequest = await ask(trusted);
  await trusted.stop();
  if (untrustedRequest.ip === "203.0.113.42" || untrustedRequest.protocol !== "http") {
    throw new Error("untrusted forwarded headers were used");
  }
  if (trustedRequest.ip !== "203.0.113.42" || trustedRequest.protocol !== "https") {
    throw new Error("trusted forwarded headers were ignored");
  }
  console.log("proxy trust behavior verified");
  process.exit(0);
})();
"#,
    );
    assert_eq!(text.lines().last(), Some("proxy trust behavior verified"));
}

#[test]
fn a_request_that_takes_too_long_is_refused() {
    let text = run_runtime(
        "timeout",
        r#"
const http = require("node:http");
const app = runtime.server(0, { timeout: 50 });
app.get("/", (req) => new Promise(() => {}));
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/", (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(res.statusCode);
      app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert!(text.contains("408"), "{text}");
}

#[test]
fn a_route_wins_against_a_file_of_the_same_name() {
    let text = run_runtime(
        "static-first",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const folder = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-static-"));
nodeFs.writeFileSync(path.join(folder, "a.txt"), "the file");
const app = runtime.server(0);
app.get("/a.txt", () => "the route");
app.static(folder);
app.start(() => {
  http.get("http://127.0.0.1:" + app.port() + "/a.txt", (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(body);
      app.stop();
      process.exit(0);
    });
  });
});
"#,
    );
    assert!(
        text.lines().last().is_some_and(|line| line == "the route"),
        "{text}"
    );
}

#[test]
fn a_part_of_a_file_may_be_asked_for() {
    let text = run_runtime(
        "range",
        r#"
const http = require("node:http");
const nodeFs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const folder = nodeFs.mkdtempSync(path.join(os.tmpdir(), "vey-range-"));
nodeFs.writeFileSync(path.join(folder, "a.txt"), "0123456789");
const app = runtime.server(0);
app.static(folder);
app.start(() => {
  const request = http.request({
    host: "127.0.0.1",
    port: app.port(),
    path: "/a.txt",
    headers: { Range: "bytes=2-5" }
  }, (res) => {
    let body = "";
    res.on("data", (chunk) => { body += chunk; });
    res.on("end", () => {
      console.log(res.statusCode);
      console.log(body);
      console.log(res.headers["content-range"]);
      app.stop();
      process.exit(0);
    });
  });
  request.end();
});
"#,
    );
    assert!(text.contains("206"), "{text}");
    assert!(text.contains("2345"), "{text}");
    assert!(text.contains("bytes 2-5/10"), "{text}");
}
