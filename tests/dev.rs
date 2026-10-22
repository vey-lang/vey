use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static USED_PORTS: std::sync::Mutex<Vec<u16>> = std::sync::Mutex::new(Vec::new());

fn temp_dir(label: &str) -> PathBuf {
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
        "vey-dev-{}-{}-{}-{}",
        label,
        std::process::id(),
        nanos,
        index
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn free_port() -> u16 {
    let mut used = USED_PORTS.lock().unwrap();
    loop {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        if !used.contains(&port) {
            used.push(port);
            return port;
        }
    }
}

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn request(port: u16, path: &str) -> Option<(String, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        path
    )
    .ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let status = raw.lines().next()?.trim().to_string();
    let body = raw.split("\r\n\r\n").nth(1)?.to_string();
    Some((status, body))
}

fn wait_for(port: u16, path: &str, expected: &str) -> (String, String) {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Some((status, body)) = request(port, path) {
            if body.contains(expected) {
                return (status, body);
            }
            last = body;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for '{expected}'; last body: {last}");
}

#[test]
fn dev_reports_errors_in_a_readable_page_and_recovers() {
    let dir = temp_dir("error-page");
    fs::write(dir.join("style.css"), "body { margin: 0; }\n").unwrap();
    fs::write(
        dir.join("main.vey"),
        "use \"./stylish.css\"\n\ncomponent App() {\n  main { h1 { \"Hello\" } }\n}\n\nbind(\"#app\", App())\n",
    )
    .unwrap();

    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("dev")
        .arg(&dir)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (status, body) = wait_for(port, "/", "Compile error");
    assert!(status.contains("500"), "status: {status}");
    assert!(body.contains("module not found: ./stylish.css"), "{body}");
    assert!(
        body.contains("did you mean <code>use &quot;./style.css&quot;</code>?"),
        "{body}"
    );
    assert!(body.contains("class=\"code\""), "{body}");
    assert!(body.contains("main.vey"), "{body}");
    assert!(body.contains("watching for changes"), "{body}");
    assert!(!body.contains("Vey could not build this page"), "{body}");
    // The page is black and white, like the rest of Vey.
    assert!(!body.contains("#818cf8"), "{body}");
    assert!(!body.contains("radial-gradient"), "{body}");

    fs::write(
        dir.join("main.vey"),
        "use \"./style.css\"\n\ncomponent App() {\n  main { h1 { \"Hello again\" } }\n}\n\nbind(\"#app\", App())\n",
    )
    .unwrap();

    let (status, body) = wait_for(port, "/", "Hello again");
    assert!(status.contains("200"), "status: {status}");
    assert!(body.contains("body { margin: 0; }"), "{body}");
}

#[test]
fn the_pages_of_a_project_are_served_at_their_own_paths() {
    let dir = temp_dir("pages");
    fs::create_dir_all(dir.join("src/styles")).unwrap();
    fs::create_dir_all(dir.join("src/test")).unwrap();
    fs::write(dir.join("src/styles/site.css"), "body { margin: 0; }\n").unwrap();
    fs::write(
        dir.join("src/main.vey"),
        "use \"./styles/site.css\"\n\ncomponent App() {\n  main { h1 { \"Home\" } }\n}\n\nbind(\"#app\", App())\n",
    )
    .unwrap();
    fs::write(
        dir.join("src/test/main.vey"),
        "use \"../styles/site.css\"\n\ncomponent Test() {\n  main { h1 { \"Test\" } }\n}\n\nbind(\"#app\", Test())\n",
    )
    .unwrap();

    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("dev")
        .arg(&dir)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _server = Server(child);

    // The page is served at the path of the file, without the folder of the
    // pages in front of it.
    let (status, body) = wait_for(port, "/test/main.vey", "Test");
    assert!(status.contains("200"), "status: {status}");
    assert!(body.contains("Test"), "{body}");

    // The folder of the pages is not part of the path.
    let (status, _) = request(port, "/src/test/main.vey").expect("no answer");
    assert!(status.contains("404"), "status: {status}");
}

#[test]
fn the_error_page_shows_the_code_with_colours() {
    let dir = temp_dir("colours");
    fs::write(
        dir.join("main.vey"),
        "use \"./missing.vey\"\nlet count = 42 // a comment\nlet name = \"vey\"\n",
    )
    .unwrap();
    // A file with a similar name, so that the error has a hint.
    fs::write(dir.join("missin.vey"), "export fn value() { return 1 }\n").unwrap();

    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("dev")
        .arg(&dir)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (status, body) = wait_for(port, "/", "Compile error");
    assert!(status.contains("500"), "status: {status}");
    // The code is shown, and the page carries the highlighter of the
    // runtime, so that the colours of an error are the colours of a sample.
    assert!(body.contains("let count = 42"), "the code is not shown");
    assert!(body.contains("a comment"), "the comment is not shown");
    assert!(
        body.contains("data-vey-highlight"),
        "the highlighter is not in the page"
    );
    // The classes of the highlighter are styled by the page.
    for class in [".cm {", ".kw {", ".str {", ".num {"] {
        assert!(body.contains(class), "the class '{}' has no colour", class);
    }
    // The hint is readable.
    assert!(body.contains("<b>Hint</b><span>"), "{body}");
    assert!(body.contains("did you mean"), "{body}");
}

#[test]
fn the_release_server_serves_the_site_and_not_the_sources() {
    let dir = temp_dir("release");
    fs::write(
        dir.join("main.vey"),
        "component App() {\n  main { h1 { \"Home\" } }\n}\n\nbind(\"#app\", App())\n",
    )
    .unwrap();

    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("run")
        .arg(&dir)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _server = Server(child);

    // The site is served.
    let (status, body) = wait_for(port, "/", "Home");
    assert!(status.contains("200"), "status: {status}");
    assert!(!body.contains("__vey_reload"), "{body}");

    // A source file is not.
    let (status, _) = request(port, "/main.vey").expect("no answer");
    assert!(status.contains("404"), "status: {status}");
}

#[test]
fn a_page_that_is_added_while_the_server_runs_is_served() {
    let dir = temp_dir("new-page");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/main.vey"),
        "component App() {\n  main { h1 { \"Home\" } }\n}\n\nbind(\"#app\", App())\n",
    )
    .unwrap();

    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("dev")
        .arg(&dir)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (status, _) = wait_for(port, "/", "Home");
    assert!(status.contains("200"), "status: {status}");

    // A page that is added while the server runs is served as well.
    fs::write(
        dir.join("src/about.vey"),
        "component About() {\n  main { h1 { \"About page\" } }\n}\n\nbind(\"#app\", About())\n",
    )
    .unwrap();

    let (status, body) = wait_for(port, "/about.html", "About page");
    assert!(status.contains("200"), "status: {status}");
    assert!(body.contains("About page"), "{body}");
}
