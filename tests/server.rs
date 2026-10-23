//! Tests for the HTTP server of the runtime.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use common::temp_dir;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        // Kill the isolated process group before reaping its leader. Reaping
        // first releases its PID/PGID and can let a later signal hit an
        // unrelated process group if the identifier is reused.
        #[cfg(unix)]
        let _ = Command::new("kill")
            .arg("-s")
            .arg("KILL")
            .arg("--")
            .arg(format!("-{}", self.0.id()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        #[cfg(not(unix))]
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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

fn wait_for(port: u16, path: &str) -> (String, String) {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Some((status, body)) = request(port, path) {
            if !body.is_empty() {
                return (status, body);
            }
            last = body;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {path}; last body: {last}");
}

/// Start a server from `source`, with `{port}` replaced by a free port.
fn start_server(source: &str) -> (Server, u16) {
    let dir = temp_dir("server");
    let port = free_port();
    std::fs::write(
        dir.join("main.vey"),
        source.replace("{port}", &port.to_string()),
    )
    .unwrap();
    // The program is run with node, the way a script runs.
    let script = common::build_script(&dir.join("main.vey"));
    let mut command = Command::new("node");
    command
        .arg("--enable-source-maps")
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // The server runs in a group of its own, so that the node process under
    // the compiler can be ended with it.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command.spawn().unwrap();
    (Server(child), port)
}

const ROUTES: &str = "let app = server({port})\n\
     app.get('/users/{id}', fn(req) { return { id: req.params.id } })\n\
     app.get('/files/*', fn(req) { return { file: req.params.splat } })\n\
     app.start()\n\
     await sleep(10000)\n\
     app.stop()\n";

#[test]
fn a_route_returns_its_handler_result() {
    let (server, port) = start_server(ROUTES);
    let _ = server;
    let (_, body) = wait_for(port, "/users/abc");
    assert!(body.contains("\"id\":\"abc\""), "{body}");
}

#[test]
fn route_parameters_are_decoded() {
    let (server, port) = start_server(ROUTES);
    let _ = server;
    let (_, body) = wait_for(port, "/users/a%20b");
    assert!(body.contains("\"id\":\"a b\""), "{body}");

    let (_, body) = wait_for(port, "/files/x%20y/z");
    assert!(body.contains("\"file\":\"x y/z\""), "{body}");
}

#[test]
fn a_malformed_path_does_not_take_the_server_down() {
    let (server, port) = start_server(ROUTES);
    let _ = server;
    let (_, _) = wait_for(port, "/users/abc");
    // A broken percent encoding used to throw inside the request handler.
    let _ = request(port, "/users/%");
    let _ = request(port, "/%");
    let (_, body) = wait_for(port, "/users/abc");
    assert!(body.contains("\"id\":\"abc\""), "{body}");
}

/// Send a request with a body, and wait for the server to be there.
fn post(port: u16, path: &str, body: &str, content_type: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let request = format!(
                "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\
                 Content-Type: {content_type}\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if write!(stream, "{request}").is_ok() {
                let mut raw = String::new();
                if stream.read_to_string(&mut raw).is_ok() && !raw.is_empty() {
                    return raw;
                }
                last = raw;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out sending to {path}; last body: {last}");
}

#[test]
fn the_body_of_a_form_is_read() {
    let (server, port) = start_server(
        "let app = server({port})\n\
         app.post('/form', fn(req) { return { name: req.body.name } })\n\
         app.start()\n\
         await sleep(10000)\n\
         app.stop()\n",
    );
    let _ = server;

    let raw = post(
        port,
        "/form",
        "name=Ada&x=1",
        "application/x-www-form-urlencoded",
    );
    assert!(raw.contains("\"name\":\"Ada\""), "{raw}");
}
