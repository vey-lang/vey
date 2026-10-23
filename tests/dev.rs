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
        // A development server starts a program of Node. The program is in
        // the group of the command, so the group is ended with it.
        #[cfg(unix)]
        let _ = Command::new("kill")
            .arg("-s")
            .arg("KILL")
            .arg("--")
            .arg(format!("-{}", self.0.id()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The command of a development server, in a group of its own so that the
/// program it starts is ended with it.
fn dev_command(dir: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vey"));
    command
        .arg("dev")
        .arg(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

/// The status line of a request, when the server answers at all.
fn status_of(port: u16, path: &str) -> Option<String> {
    request(port, path).map(|(status, _)| status)
}

/// The first event of the events of the development server.
fn event_version(port: u16) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(
        stream,
        "GET /__vey/dev/events HTTP/1.1\r\nHost: 127.0.0.1\r\n\
         Accept: text/event-stream\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut raw = String::new();
    let mut buffer = [0u8; 2048];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                raw.push_str(&String::from_utf8_lossy(&buffer[..read]));
                if let Some(line) = raw.lines().find(|line| line.starts_with("data: ")) {
                    return Some(line[6..].to_string());
                }
            }
            Err(_) => break,
        }
    }
    None
}

/// Wait until the server stops answering, which is how a program that does
/// not build is seen.
fn wait_until_stopped(port: u16, path: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if status_of(port, path).is_none() {
            // A build takes a moment, and the server is started again after
            // it. The answer has to be gone for a while to be gone.
            thread::sleep(Duration::from_millis(400));
            if status_of(port, path).is_none() {
                return;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("the server still answers after a build that does not pass");
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

/// The program of a server, with a heading of its own.
const SERVER_PROGRAM: &str = r#"let port = int(env("PORT", "4000"))
let host = env("HOST", "127.0.0.1")
let app = server(port, { host: host })
app.get("/", fn() {
  return response("<html><body><h1>{heading}</h1></body></html>", 200, { "Content-Type": "text/html; charset=utf-8" })
})
app.start()
"#;

/// Write the program of a server into a folder, and say what it is when
/// the folder does not say.
fn write_server(dir: &std::path::Path, heading: &str, kind: bool) {
    fs::create_dir_all(dir.join("src")).unwrap();
    if kind {
        fs::write(dir.join("vey.toml"), "kind = \"server\"\n").unwrap();
    }
    fs::write(
        dir.join("src/main.vey"),
        SERVER_PROGRAM.replace("{heading}", heading),
    )
    .unwrap();
}

/// The command that runs a project the way a host would.
fn run_command(dir: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vey"));
    command
        .arg("run")
        .arg(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

#[test]
fn a_server_project_is_run_and_built_again_when_it_changes() {
    let dir = temp_dir("server-dev");
    write_server(&dir, "First", true);
    let port = free_port();
    let child = dev_command(&dir)
        .arg("--port")
        .arg(port.to_string())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (status, body) = wait_for(port, "/", "First");
    assert!(status.contains("200"), "status: {status}");
    // The page of a program that is worked on listens for the next build.
    assert!(
        body.contains("/__vey/dev/client.js"),
        "the page does not listen for the next build: {body}"
    );
    let client = request(port, "/__vey/dev/client.js").expect("the client of the dev server");
    assert!(client.0.contains("200"), "{}", client.0);
    assert!(client.1.contains("EventSource"), "{}", client.1);
    let first_version = event_version(port).expect("the version of the first build");
    assert!(!first_version.is_empty());

    // A change builds the program again, and the page learns that the
    // build has changed.
    write_server(&dir, "Second", true);
    let (_, body) = wait_for(port, "/", "Second");
    assert!(body.contains("/__vey/dev/client.js"), "{body}");
    let second_version = event_version(port).expect("the version of the second build");
    assert_ne!(
        first_version, second_version,
        "the page is not told that the build changed"
    );

    // A program that does not build stops the server, so that an answer
    // never comes from code that the files no longer say.
    fs::write(dir.join("src/main.vey"), "let = = =").unwrap();
    wait_until_stopped(port, "/");

    // Fixing it starts the server again.
    write_server(&dir, "Third", true);
    let (_, body) = wait_for(port, "/", "Third");
    assert!(body.contains("Third"), "{body}");
}

#[test]
fn a_project_that_does_not_say_its_kind_is_read_from_its_entry() {
    let dir = temp_dir("server-guess");
    // No `vey.toml`: the program starts a server, so it is one.
    write_server(&dir, "Guessed", false);
    let port = free_port();
    let child = dev_command(&dir)
        .arg("--port")
        .arg(port.to_string())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (_, body) = wait_for(port, "/", "Guessed");
    assert!(body.contains("__vey_dev_client") || body.contains("/__vey/dev/client.js"));
}

#[test]
fn a_server_project_is_run_without_the_development_client_for_a_host() {
    let dir = temp_dir("server-release");
    write_server(&dir, "Release", true);
    let port = free_port();
    let child = run_command(&dir)
        .arg("--port")
        .arg(port.to_string())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (status, body) = wait_for(port, "/", "Release");
    assert!(status.contains("200"), "status: {status}");
    // Nothing of the development server is in a program that runs for a
    // host: no listener in the page, and no route of the work.
    assert!(!body.contains("__vey/dev"), "{body}");
    let (status, _) = request(port, "/__vey/dev/client.js").expect("no answer");
    assert!(status.contains("404"), "status: {status}");
}

#[cfg(unix)]
#[test]
fn the_program_of_a_server_is_ended_with_the_command() {
    let dir = temp_dir("server-signal");
    write_server(&dir, "Signalled", true);
    let port = free_port();
    let mut child = dev_command(&dir)
        .arg("--port")
        .arg(port.to_string())
        .spawn()
        .unwrap();

    wait_for(port, "/", "Signalled");
    // The terminal sends the signal to the group of the foreground
    // command, so that is what is sent here.
    let _ = Command::new("kill")
        .arg("-s")
        .arg("INT")
        .arg("--")
        .arg(format!("-{}", child.id()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut exited = false;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            exited = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if !exited {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert!(exited, "the command did not stop on SIGINT");

    // The program it started is stopped with it.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && status_of(port, "/").is_some() {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        status_of(port, "/").is_none(),
        "the program still answers after the command is stopped"
    );
}


#[test]
fn dev_reloads_the_title_when_project_config_changes() {
    let dir = temp_dir("config-title-reload");
    fs::write(
        dir.join("main.vey"),
        "component App() { main { h1 { \"Hello\" } } }\nbind(\"#app\", App())\n",
    )
    .unwrap();
    fs::write(dir.join("vey.toml"), "title = \"Old title\"\n").unwrap();

    let port = free_port();
    let child = dev_command(&dir)
        .arg("--port")
        .arg(port.to_string())
        .spawn()
        .unwrap();
    let _server = Server(child);

    let (status, _) = wait_for(port, "/", "<title>Old title</title>");
    assert!(status.contains("200"), "status: {status}");

    fs::write(dir.join("vey.toml"), "title = \"New title\"\n").unwrap();
    let (status, body) = wait_for(port, "/", "<title>New title</title>");
    assert!(status.contains("200"), "status: {status}");
    assert!(body.contains("Hello"), "{body}");
}
