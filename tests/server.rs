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

/// A request of a method that is named, with the headers that are named.
fn raw_request(port: u16, method: &str, path: &str, accept: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: {accept}\r\n\
         Content-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    Some(raw)
}

/// Start a server from `source`, with the files of `files` beside it.
fn start_project(source: &str, files: &[(&str, &str)]) -> (Server, u16) {
    let dir = temp_dir("server");
    let port = free_port();
    for (name, content) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
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
        // The program is run in its own folder, so that the files it names
        // relative to itself are found.
        .current_dir(&dir)
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

/// Start a server from `source`, with `{port}` replaced by a free port.
fn start_server(source: &str) -> (Server, u16) {
    start_project(source, &[])
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

#[test]
fn a_method_that_is_not_taken_is_a_405_with_the_methods_it_takes() {
    let (_server, port) = start_server(
        "let app = server({port})\n\
         app.get('/only-get', fn() { return \"got\" })\n\
         app.post('/only-post', fn() { return \"posted\" })\n\
         app.start()\n",
    );
    wait_for(port, "/only-get");

    let raw = raw_request(port, "GET", "/only-post", "*/*").expect("no answer");
    assert!(raw.starts_with("HTTP/1.1 405"), "{raw}");
    assert!(
        raw.to_lowercase().contains("\r\nallow: post"),
        "the answer does not say what the address takes: {raw}"
    );
    assert!(raw.contains("<!doctype html>"), "a browser gets a page: {raw}");

    let raw = raw_request(port, "POST", "/only-get", "application/json").expect("no answer");
    assert!(raw.starts_with("HTTP/1.1 405"), "{raw}");
    let allow = raw.to_lowercase();
    assert!(allow.contains("\r\nallow: get"), "{raw}");
    assert!(allow.contains("head"), "{raw}");
    // A program gets a line of text, not a page.
    assert!(!raw.contains("<!doctype html>"), "{raw}");
}

#[test]
fn a_route_wins_against_a_file_of_the_same_name() {
    let (_server, port) = start_project(
        "let app = server({port})\n\
         app.get('/', fn() { return \"home\" })\n\
         app.get('/asset.txt', fn() { return \"the route\" })\n\
         app.static('public')\n\
         app.start()\n",
        &[
            ("public/asset.txt", "the file"),
            ("public/only-file.txt", "the file"),
        ],
    );
    wait_for(port, "/");

    let (_, body) = request(port, "/asset.txt").expect("no answer");
    assert!(body.contains("the route"), "the file hid the route: {body}");
    // A file that no route takes is served.
    let (_, body) = request(port, "/only-file.txt").expect("no answer");
    assert!(body.contains("the file"), "{body}");
}

#[test]
fn a_missing_address_is_a_page_for_a_browser_and_a_line_for_a_program() {
    let (_server, port) = start_server(
        "let app = server({port})\n\
         app.get('/', fn() { return \"home\" })\n\
         app.start()\n",
    );
    wait_for(port, "/");

    let raw = raw_request(port, "GET", "/missing", "text/html").expect("no answer");
    assert!(raw.starts_with("HTTP/1.1 404"), "{raw}");
    assert!(raw.contains("<!doctype html>"), "{raw}");
    assert!(raw.contains("Nothing here."), "{raw}");

    let raw = raw_request(port, "GET", "/missing", "application/json").expect("no answer");
    assert!(raw.starts_with("HTTP/1.1 404"), "{raw}");
    assert!(!raw.contains("<!doctype html>"), "{raw}");
    assert!(raw.contains("404 Nothing here."), "{raw}");
}

#[test]
fn a_mistake_of_a_handler_is_not_told_to_the_visitor() {
    let (_server, port) = start_server(
        "let app = server({port})\n\
         app.get('/', fn() { return \"home\" })\n\
         app.get('/boom', fn() { error(\"the password is hunter2 at /etc/secret\") })\n\
         app.start()\n",
    );
    wait_for(port, "/");

    let raw = raw_request(port, "GET", "/boom", "text/html").expect("no answer");
    assert!(raw.starts_with("HTTP/1.1 500"), "{raw}");
    assert!(!raw.contains("hunter2"), "{raw}");
    assert!(!raw.contains("/etc/secret"), "{raw}");
    assert!(raw.contains("Something went wrong."), "{raw}");
}

#[test]
fn a_body_that_is_too_large_is_a_413() {
    let (_server, port) = start_server(
        "let app = server({port}, { bodyLimit: 32 })\n\
         app.post('/form', fn(req) { return \"taken\" })\n\
         app.start()\n",
    );
    // The server may take a moment to be there.
    thread::sleep(Duration::from_millis(300));
    let raw = post(port, "/form", &"x".repeat(100), "text/plain");
    assert!(raw.starts_with("HTTP/1.1 413"), "{raw}");
}

#[test]
fn a_custom_handler_of_a_missing_address_still_answers() {
    let (_server, port) = start_server(
        "let app = server({port})\n\
         app.get('/', fn() { return \"home\" })\n\
         app.notFound(fn(req) { return \"nothing at \" + req.path })\n\
         app.start()\n",
    );
    wait_for(port, "/");

    let raw = raw_request(port, "GET", "/gone", "text/html").expect("no answer");
    assert!(raw.starts_with("HTTP/1.1 404"), "{raw}");
    assert!(raw.contains("nothing at /gone"), "{raw}");
}
