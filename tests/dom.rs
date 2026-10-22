use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("vey-dom-{}-{}", std::process::id(), nanos));
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
}, 50)
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
