use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "vey-dev-{}-{}-{}",
        label,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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
