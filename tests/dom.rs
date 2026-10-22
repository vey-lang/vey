use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    // The folder of a test has a name of its own, so that two tests that
    // run at the same time do not write in the same place. The clock of
    // the machine is not enough, because it may say the same time twice.
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let index = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "vey-dom-{}-{}-{}",
        std::process::id(),
        nanos,
        index
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn renders_in_a_dom_and_rerenders() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let title = "Hello from Vey"
let count = 0

component App() {
  main.page {
    h1 { title }
    button.btn click=fn() {
      count = count + 1
      bind("#app", App())
    } { "Clicked {count} times" }
  }
}

bind("#app", App())
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_TEST_OK"));
}

#[test]
fn the_page_follows_the_state_it_reads() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let count = signal(0)

component App() {
  main.page {
    h1 { "Clicked {count.value} times" }
  }
}

mount("#app", App)

setTimeout(fn() {
  count.set(1)
}, 10)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-signal-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-signal-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_SIGNAL_TEST_OK"));
}

#[test]
fn a_node_may_say_goodbye_before_it_is_taken_away() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let show = signal(true)

component App() {
  div {
    if show.value {
      span leave=fn(node) {
        print("GOODBYE")
        return sleep(10)
      } { "here" }
    }
  }
}

mount("#app", App)

setTimeout(fn() {
  show.set(false)
}, 200)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-leave-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-leave-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("DOM_LEAVE_TEST_OK"), "{text}");
    assert!(text.contains("GOODBYE"), "{text}");
}

#[test]
fn the_value_of_a_field_and_the_state_are_one() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let name = signal("Ada")

component App() {
  input value=name { }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-field-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-field-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_FIELD_TEST_OK"));
}

#[test]
fn the_route_of_the_page_is_shown_even_when_it_is_written_first() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
component UserPage(params) {
  h1 { "User {params.id}" }
}

route("/users/{id}", fn(params) {
  return UserPage(params)
})

component App() {
  main { "the app" }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-route-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-route-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_ROUTE_TEST_OK"));
}

#[test]
fn the_work_of_a_node_is_not_written_in_the_page() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let mounts = 0

component App() {
  onMount(fn() { mounts = mounts + 1 })
  div { "the page" }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    // The work of the node is done, but it is not written in the page.
    let harness = dir.join("dom-effect-test.cjs");
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root } = require("./dom-fake.cjs");
require(bundle);
setTimeout(() => {
  const text = root.textContent;
  if (text !== "the page") throw new Error("the page is not right: " + text);
  if (text.includes("function")) throw new Error("the work was written in the page");
  console.log("DOM_EFFECT_TEST_OK");
}, 150);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_EFFECT_TEST_OK"));
}

#[test]
fn many_changes_write_the_page_once() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let count = signal(0)

component App() {
  print("written")
  div { "{count.value}" }
}

mount("#app", App)
count.set(1)
count.set(2)
count.set(3)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-batch-test.cjs");
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root } = require("./dom-fake.cjs");
require(bundle);
setTimeout(() => {
  console.log("TEXT:" + root.textContent);
}, 150);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("TEXT:3"), "{text}");
    // The page is written once at the beginning and once for the three
    // changes, and not three times more.
    let written = text.matches("written").count();
    assert_eq!(written, 2, "{text}");
}

#[test]
fn the_work_of_a_node_is_done_when_it_is_taken_away() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let show = signal(true)

component Child() {
  onMount(fn() { print("MOUNTED") })
  onDestroy(fn() { print("DESTROYED") })
  span { "child" }
}

component App() {
  if show.value {
    Child()
  } else {
    p { "gone" }
  }
}

mount("#app", App)

setTimeout(fn() {
  show.set(false)
}, 100)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-destroy-test.cjs");
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root } = require("./dom-fake.cjs");
require(bundle);
setTimeout(() => {
  console.log("TEXT:" + root.textContent);
}, 300);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The work of the node is done once, when it comes and when it goes.
    assert_eq!(text.matches("MOUNTED").count(), 1, "{text}");
    assert_eq!(text.matches("DESTROYED").count(), 1, "{text}");
    assert!(text.contains("TEXT:gone"), "{text}");
}

#[test]
fn a_component_keeps_the_values_of_its_own() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
component Counter(label) {
  let count = signal(0)
  button click=fn() {
    count.set(count.value + 1)
  } { "{label}: {count.value}" }
}

component App() {
  div {
    Counter("a")
    Counter("b")
  }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-state-test.cjs");
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root } = require("./dom-fake.cjs");
require(bundle);
setTimeout(() => {
  const buttons = root.children[0].children;
  console.log("BEFORE:" + buttons.map((b) => b.textContent).join(","));
  buttons[0].listeners.click[0]();
  setTimeout(() => {
    console.log("AFTER:" + buttons.map((b) => b.textContent).join(","));
    buttons[1].listeners.click[0]();
    setTimeout(() => {
      console.log("LAST:" + buttons.map((b) => b.textContent).join(","));
    }, 100);
  }, 100);
}, 100);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Every component has its own value, and it keeps it when the page is
    // written again.
    assert!(text.contains("BEFORE:a: 0,b: 0"), "{text}");
    assert!(text.contains("AFTER:a: 1,b: 0"), "{text}");
    assert!(text.contains("LAST:a: 1,b: 1"), "{text}");
}

#[test]
fn a_slot_takes_the_content_of_the_caller_or_the_content_of_its_own() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
component Card(title, body) {
  div.card {
    h2 { title }
    slot name="body" { p { "There is nothing here yet." } }
  }
}

component App() {
  div {
    Card("With") {
      p slot="body" { "Given content" }
    }
    Card("Without")
  }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-slot-test.cjs");
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root } = require("./dom-fake.cjs");
require(bundle);
setTimeout(() => {
  console.log("TEXT:" + root.textContent);
}, 150);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The content of the caller takes the place of the content of the
    // slot, and the content of the slot is there when there is none.
    assert!(
        text.contains("TEXT:WithGiven contentWithoutThere is nothing here yet."),
        "{text}"
    );
}

#[test]
fn a_portal_is_written_in_another_place_and_follows_the_page() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let label = signal("first")
let count = signal(2)
let shown = signal(true)

component Box(text) {
  div.box { text }
}

component App() {
  div {
    "page"
    if shown.value {
      portal(Box("{label.value} {count.value}"), "#side", "box")
    }
  }
}

mount("#app", App)

setTimeout(fn() {
  label.set("second")
  count.set(3)
}, 20)

setTimeout(fn() {
  shown.set(false)
}, 60)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-portal-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-portal-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_PORTAL_TEST_OK"));
}

#[test]
fn the_head_the_shared_values_and_the_fallback_are_written() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
component Fallback() {
  div { "the fallback" }
}

component Leaf() {
  let theme = inject("theme")
  div { "theme {theme}" }
}

component Broken() {
  error("it broke")
}

component App() {
  title { "A page of Vey" }
  meta name="description" content="a description"
  div {
    provide("theme", "dark")
    Leaf()
    Boundary fallback=Fallback {
      Broken()
    }
  }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-head-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-head-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_HEAD_TEST_OK"));
}

#[test]
fn a_node_may_come_in_and_go_out_little_by_little() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let show = signal(true)
let fade = transition(100)

component App() {
  div { "page" }
  if show.value {
    span enter=fade.enter leave=fade.leave { "here" }
  }
}

mount("#app", App)

setTimeout(fn() {
  show.set(false)
}, 150)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-transition-test.cjs");
    fs::write(&harness, include_str!("fixtures/dom-transition-test.cjs")).unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DOM_TRANSITION_TEST_OK"));
}

#[test]
fn the_place_that_was_scrolled_is_kept_when_the_page_is_written_again() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let count = signal(0)

component App() {
  main.page {
    h1 { "Clicked {count.value} times" }
    p { "the rest of the page" }
  }
}

mount("#app", App)

setTimeout(fn() {
  count.set(1)
}, 10)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-scroll-test.cjs");
    // The visitor scrolls the page, and the page is written again when the
    // signal changes. The place they were in must stay the same.
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root, waitFor } = require("./dom-fake.cjs");

require(bundle);

waitFor(
  () => root.textContent.includes("Clicked 0 times"),
  () => "the page was not written",
  () => {
    root.scrollTop = 40;
    setTimeout(() => {
      waitFor(
        () => root.textContent.includes("Clicked 1 times"),
        () => "the page was not written again",
        () => console.log("SCROLL:" + root.scrollTop)
      );
    }, 20);
  }
);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(text.contains("SCROLL:40"), "{}", text);
}

#[test]
fn the_field_the_visitor_is_in_keeps_the_focus_when_a_list_changes() {
    let dir = temp_dir();
    let app = dir.join("app.vey");
    fs::write(
        &app,
        r##"
let items = signal(["one", "two", "three"])

component App() {
  main.page {
    ul.list {
      for item in items.value {
        li key=item { input.field value=item }
      }
    }
    button.go click=fn() {
      items.set(["three", "one", "two"])
    } { "reorder" }
  }
}

mount("#app", App)
"##,
    )
    .unwrap();

    let bundle = dir.join("app.cjs");
    let build = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("-o")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let harness = dir.join("dom-focus-test.cjs");
    fs::write(
        &harness,
        r##"
const bundle = process.argv[2];
const { root, find, document } = require("./dom-fake.cjs");

require(bundle);

setTimeout(() => {
  const field = find(root, "input");
  const button = find(root, "button");
  field.focus();
  button.listeners.click[0]();
  setTimeout(() => {
    console.log("FOCUS_KEPT:" + (document.activeElement === field));
  }, 100);
}, 100);
"##,
    )
    .unwrap();
    fs::write(
        dir.join("dom-fake.cjs"),
        include_str!("fixtures/dom-fake.cjs"),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(text.contains("FOCUS_KEPT:true"), "{}", text);
}
