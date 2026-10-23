//! The `vey dev` development server.
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use vey::compiler::compile_project;

use crate::cli::args::parse_args;
use crate::cli::config::Config;
use crate::cli::entry::{collect_files, collect_folders, default_title, entry_from, project_root};
use crate::cli::error::BuildError;
use crate::cli::output::{html_document, print_command_help};
use crate::cli::site::{default_404_page, site_files};

/// The folder the pages of a project live in.
///
/// A project that keeps its pages in `src` has them in that folder, so the
/// server serves them at the same paths as the built site.
fn pages_root(entry: &Path) -> PathBuf {
    let path = match entry.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    // The path is resolved, so that it can be compared with the resolved
    // path of a file.
    path.canonicalize().unwrap_or(path)
}

/// The port the development server listens on.
pub(crate) const DEFAULT_PORT: u16 = 4000;

/// Take the lock of the state.
///
/// A panic while the lock was held would otherwise make every later request
/// fail, so the state is used even then.
fn lock(state: &Arc<Mutex<DevState>>) -> std::sync::MutexGuard<'_, DevState> {
    state.lock().unwrap_or_else(|err| err.into_inner())
}

#[derive(Default)]
pub(crate) struct DevState {
    pub(crate) files: HashMap<String, Vec<u8>>,
    pub(crate) error: Option<BuildError>,
    pub(crate) version: u64,
    pub(crate) root: PathBuf,
    /// The folder the pages of the project live in.
    pub(crate) pages: PathBuf,
    /// Whether the files are watched and the page reloads.
    pub(crate) live: bool,
    /// The title of the site.
    pub(crate) title: String,
    /// Files imported from outside the project root. They are not covered by
    /// the directory snapshot, so the watcher tracks them explicitly.
    pub(crate) watch: Vec<PathBuf>,
}

pub(crate) fn cmd_dev(args: &[String]) -> i32 {
    let parsed = match parse_args("dev", args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("dev");
        return 0;
    }
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey dev [entry.vey] [--port 4000]");
            return 1;
        }
    };

    // The settings of the project may say where the server listens and
    // what the title of the site is.
    let config = Config::find(&project_root(&entry));
    let host = parsed
        .host
        .clone()
        .or_else(|| config.host.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parsed.port.or(config.port).unwrap_or(DEFAULT_PORT);
    let title = parsed
        .title
        .clone()
        .or_else(|| config.title.clone())
        .unwrap_or_else(|| default_title(&entry));

    serve(&entry, &title, &host, port, true)
}

/// Serve a project.
///
/// With `live` the files are watched, the page reloads when they change and a
/// page that cannot be built shows the error. Without it the site is served
/// the way a host would serve it.
pub(crate) fn serve(entry: &Path, title: &str, host: &str, port: u16, live: bool) -> i32 {
    let root = project_root(entry);
    let state = Arc::new(Mutex::new(DevState {
        live,
        ..DevState::default()
    }));
    {
        let mut guard = lock(&state);
        guard.root = root.canonicalize().unwrap_or_else(|_| root.clone());
        guard.pages = pages_root(entry);
        guard.title = title.to_string();
    }
    if let Err(err) = refresh_dev(entry, title, &state) {
        eprintln!("{}", err);
        if live {
            eprintln!("waiting for changes...");
        } else {
            return 1;
        }
    }

    let listener = match TcpListener::bind((host, port)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("cannot listen on {}:{}: {}", host, port, err);
            return 1;
        }
    };

    println!(
        "Vey {} server running at http://{}:{}",
        if live { "dev" } else { "release" },
        if host == "0.0.0.0" { "localhost" } else { host },
        port
    );
    println!("entry: {}", entry.display());
    if live {
        println!("watching for changes, press Ctrl+C to stop");
    } else {
        println!("press Ctrl+C to stop");
    }

    if live {
        let watcher_state = Arc::clone(&state);
        let watcher_entry = entry.to_path_buf();
        let watcher_title = title.to_string();
        let watcher_root = root.canonicalize().unwrap_or_else(|_| root.clone());
        thread::spawn(move || {
            let mut previous = dev_snapshot(&watcher_root, &watcher_entry, &watcher_state);
            loop {
                thread::sleep(Duration::from_millis(300));
                let current = dev_snapshot(&watcher_root, &watcher_entry, &watcher_state);
                if current == previous {
                    continue;
                }
                thread::sleep(Duration::from_millis(80));
                let current = dev_snapshot(&watcher_root, &watcher_entry, &watcher_state);
                let started = Instant::now();
                match refresh_dev(&watcher_entry, &watcher_title, &watcher_state) {
                    Ok(_) => println!("rebuilt the site in {} ms", started.elapsed().as_millis()),
                    Err(err) => eprintln!("{}", err),
                }
                previous = current;
            }
        });
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || handle_connection(stream, state));
            }
            Err(err) => {
                eprintln!("connection failed: {}", err);
            }
        }
    }

    0
}

pub(crate) fn refresh_dev(
    entry: &Path,
    title: &str,
    state: &Arc<Mutex<DevState>>,
) -> Result<usize, BuildError> {
    let mut watch = Vec::new();
    let result = site_files(entry, Some(title), &mut watch, false);
    let mut guard = lock(state);
    guard.version += 1;
    match result {
        Ok(output) => {
            let count = output.files.len();
            guard.files = output.files;
            guard.error = None;
            guard.watch = watch;
            // A page that cannot be built does not stop the others.
            for (name, err) in output.failed {
                eprintln!("cannot build {}: {}", name, err);
            }
            Ok(count)
        }
        Err(err) => {
            for path in watch {
                if !guard.watch.contains(&path) {
                    guard.watch.push(path);
                }
            }
            guard.error = Some(err.clone());
            Err(err)
        }
    }
}

pub(crate) fn handle_connection(mut stream: TcpStream, state: Arc<Mutex<DevState>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut request = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n")
                    || request.len() > 64 * 1024
                {
                    break;
                }
            }
            Err(_) => return,
        }
    }

    let text = String::from_utf8_lossy(&request);
    let Some(first_line) = text.lines().next() else {
        return;
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    let head = method.eq_ignore_ascii_case("HEAD");

    // The page listens for the changes, so that it reloads itself without
    // asking every moment.
    if path == "/__vey/events" {
        let _ = stream.set_read_timeout(None);
        let headers = "HTTP/1.1 200 OK\r\n\
                   Content-Type: text/event-stream\r\n\
                   Cache-Control: no-store\r\n\
                   Connection: keep-alive\r\n\r\n";
        if stream.write_all(headers.as_bytes()).is_err() {
            return;
        }
        let mut last = u64::MAX;
        loop {
            let version = lock(&state).version;
            if version != last {
                last = version;
                if stream
                    .write_all(format!("data: {}\n\n", version).as_bytes())
                    .is_err()
                {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(200));
        }
        return;
    }

    let (status, content_type, body) = {
        let mut guard = lock(&state);
        respond(&mut guard, path)
    };

    let live = state.lock().map(|guard| guard.live).unwrap_or(false);
    write_response(&mut stream, status, content_type, &body, head, live);
}

pub(crate) fn respond(state: &mut DevState, path: &str) -> (u16, &'static str, Vec<u8>) {
    if path == "/__vey/version" {
        return (
            200,
            "text/plain; charset=utf-8",
            state.version.to_string().into_bytes(),
        );
    }

    // A source file is only previewed while the work is going on. A release
    // server serves what was built.
    if state.live && path.ends_with(".vey") {
        if let Some(page) = render_vey_page(state, path) {
            return page;
        }
    }

    if let Some(err) = &state.error {
        if state.live && accepts_html(path) {
            let body = err.render_html();
            return (
                500,
                "text/html; charset=utf-8",
                inject_reload(&body, state.version).into_bytes(),
            );
        }
    }

    let decoded = percent_decode(path);
    if decoded.split('/').any(|part| part == "..") {
        return missing(state);
    }

    let mut key = decoded.trim_start_matches('/').to_string();
    if key.is_empty() || key.ends_with('/') {
        key.push_str("index.html");
    }

    if let Some(bytes) = state.files.get(&key) {
        let content_type = content_type_for(&key);
        let body = if state.live && content_type.starts_with("text/html") {
            inject_reload(&String::from_utf8_lossy(bytes), state.version).into_bytes()
        } else {
            bytes.clone()
        };
        return (200, content_type, body);
    }

    let nested = format!("{}/index.html", key);
    if let Some(bytes) = state.files.get(&nested) {
        let body = if state.live {
            inject_reload(&String::from_utf8_lossy(bytes), state.version).into_bytes()
        } else {
            bytes.clone()
        };
        return (200, "text/html; charset=utf-8", body);
    }

    // A path without a name may still be a page, the way a host that serves
    // "/about" for "/about.html" does it.
    if !key.contains('.') {
        let page = format!("{}.html", key);
        if let Some(bytes) = state.files.get(&page) {
            let body = if state.live {
                inject_reload(&String::from_utf8_lossy(bytes), state.version).into_bytes()
            } else {
                bytes.clone()
            };
            return (200, "text/html; charset=utf-8", body);
        }
    }

    missing(state)
}

/// Preview any `.vey` file by visiting its path in the dev server. This keeps
/// the entry point at `/` while still letting `/about.vey` or `/test.vey`
/// render locally.
pub(crate) fn render_vey_page(
    state: &mut DevState,
    path: &str,
) -> Option<(u16, &'static str, Vec<u8>)> {
    let key = percent_decode(path).trim_start_matches('/').to_string();
    if key.split('/').any(|part| part == "..") {
        return None;
    }

    let pages = state.pages.clone();
    let file = pages.join(&key).canonicalize().ok()?;
    if !file.starts_with(&pages) || !file.is_file() {
        return None;
    }

    match compile_project(&file) {
        Ok(bundle) => {
            // The files of the preview are watched as well, but the list is
            // kept short so that it does not grow with every request.
            for path in std::iter::once(file.clone())
                .chain(std::iter::once(bundle.entry_path.clone()))
                .chain(bundle.modules.iter().map(|module| module.path.clone()))
                .chain(bundle.styles.iter().map(|style| style.path.clone()))
            {
                if !state.watch.contains(&path) {
                    state.watch.push(path);
                }
            }
            let favicon = state.root.join("public/favicon.svg").is_file();
            let body = html_document(&bundle, &state.title, favicon);
            Some((
                200,
                "text/html; charset=utf-8",
                inject_reload(&body, state.version).into_bytes(),
            ))
        }
        Err(err) => {
            let path = PathBuf::from(&err.file);
            if !state.watch.contains(&path) {
                state.watch.push(path);
            }
            let body = BuildError::from_vey(&err).render_html();
            Some((
                500,
                "text/html; charset=utf-8",
                inject_reload(&body, state.version).into_bytes(),
            ))
        }
    }
}

pub(crate) fn missing(state: &DevState) -> (u16, &'static str, Vec<u8>) {
    let body = state
        .files
        .get("404.html")
        .map(|bytes| String::from_utf8_lossy(bytes).to_string())
        .unwrap_or_else(|| default_404_page("Vey"));
    (
        404,
        "text/html; charset=utf-8",
        inject_reload(&body, state.version).into_bytes(),
    )
}

pub(crate) fn accepts_html(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        None => true,
        Some(extension) => {
            extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
        }
    }
}

pub(crate) fn inject_reload(html: &str, version: u64) -> String {
    // The page listens to the server, and reloads itself when the number
    // changes. The stylesheets are changed in place, so that the state of
    // the page is kept.
    let snippet = format!(
            "<script>(function(){{var v={};var source=new EventSource('/__vey/events');source.onmessage=function(event){{if(event.data===String(v))return;v=event.data;var links=document.querySelectorAll('link[rel=\"stylesheet\"]');if(!links.length){{location.reload();return;}}var left=links.length;for(var i=0;i<links.length;i++){{(function(link){{var fresh=link.cloneNode();fresh.href=link.href.split('?')[0]+'?v='+v;fresh.onload=function(){{if(--left===0)location.reload();}};link.parentNode.replaceChild(fresh,link);}})(links[i]);}}}};}})();</script>",
            version
        );
    match html.rfind("</body>") {
        Some(index) => format!("{}{}{}", &html[..index], snippet, &html[index..]),
        None => format!("{}{}", html, snippet),
    }
}

pub(crate) fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    head: bool,
    live: bool,
) {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    // The development server never lets the browser keep a page, because it
    // changes while it is worked on. The release server asks the browser to
    // check the page again, so that a new build is seen at once.
    let cache = if live {
        "Cache-Control: no-store\r\n"
    } else if content_type.starts_with("text/html") {
        "Cache-Control: no-cache\r\n"
    } else {
        ""
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len(),
        cache
    );
    let _ = stream.write_all(header.as_bytes());
    if !head {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}

pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = |byte: u8| -> Option<u8> {
                match byte {
                    b'0'..=b'9' => Some(byte - b'0'),
                    b'a'..=b'f' => Some(byte - b'a' + 10),
                    b'A'..=b'F' => Some(byte - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub(crate) fn content_type_for(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        // A page may also be written under the name of its source.
        "vey" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "woff2" => "font/woff2",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

/// The state of a file, used to notice that it changed.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FileState {
    pub(crate) path: PathBuf,
    /// The time the file was changed.
    pub(crate) modified: u64,
    /// A number that depends on the content of the file.
    pub(crate) content: u64,
}

/// The state of the files that the server cares about.
pub(crate) fn dev_snapshot(
    root: &Path,
    entry: &Path,
    state: &Arc<Mutex<DevState>>,
) -> Vec<FileState> {
    let watch = lock(state).watch.clone();

    let mut paths: Vec<PathBuf> = vec![entry.to_path_buf()];
    // The folder of the pages is watched as well, so that a page that is
    // added or taken away is noticed. The folders in it are watched too, so
    // that a page that is added in one of them is noticed.
    let pages = lock(state).pages.clone();
    if pages.is_dir() {
        if !paths.contains(&pages) {
            paths.push(pages.clone());
        }
        let mut folders = Vec::new();
        collect_folders(&pages, &mut folders);
        for folder in folders {
            if !paths.contains(&folder) {
                paths.push(folder);
            }
        }
    }
    // The files of the project that the build reads.
    for path in &watch {
        if !paths.contains(path) {
            paths.push(path.clone());
        }
    }
    // The public folder is served as it is.
    let public = root.join("public");
    if public.is_dir() {
        let mut files = Vec::new();
        collect_files(&public, &mut files);
        for path in files {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }

    let mut out: Vec<FileState> = paths.iter().map(|path| file_state(path)).collect();
    out.sort_by(|left, right| left.path.cmp(&right.path));
    out
}

pub(crate) fn file_state(path: &Path) -> FileState {
    let metadata = fs::metadata(path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    // The content is hashed as well, because a file may be replaced while
    // keeping its time and its size.
    let content = content_hash(path);
    FileState {
        path: path.to_path_buf(),
        modified,
        content,
    }
}

/// A number that depends on the content of the file.
fn content_hash(path: &Path) -> u64 {
    let Ok(bytes) = fs::read(path) else {
        return 0;
    };
    // A simple hash, so that a change of the content is always noticed.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::inject_reload;

    #[test]
        fn injected_reload_script_reloads_when_the_version_changes() {
        let html = inject_reload("<html><body></body></html>", 1);
        let script = html
            .split_once("<script>")
            .and_then(|(_, html)| html.split_once("</script>"))
            .map(|(script, _)| script)
            .expect("reload script should be injected");
                let harness = r#"
const assert = require("node:assert/strict");
let messageHandler;
let links = [];
let reloads = 0;
class EventSource {
    constructor(url) { assert.equal(url, "/__vey/events"); }
    set onmessage(handler) { messageHandler = handler; }
}
const document = { querySelectorAll: () => links };
const location = { reload: () => { reloads += 1; } };
new Function("EventSource", "document", "location", process.argv[1])(
    EventSource, document, location
);
assert.equal(typeof messageHandler, "function");
messageHandler({ data: "1" });
assert.equal(reloads, 0, "the initial version must not reload");
messageHandler({ data: "2" });
assert.equal(reloads, 1, "a changed version without stylesheets must reload");
let replacement;
const parent = {
    replaceChild(fresh, old) {
        assert.equal(old, stylesheet);
        replacement = fresh;
    }
};
const stylesheet = {
    href: "http://localhost/site.css?theme=dark",
    parentNode: parent,
    cloneNode() { return { href: this.href, parentNode: this.parentNode }; }
};
links = [stylesheet];
messageHandler({ data: "3" });
assert.equal(reloads, 1, "reload waits for the refreshed stylesheet");
assert.equal(replacement.href, "http://localhost/site.css?v=3");
replacement.onload();
assert.equal(reloads, 2, "the page reloads after the stylesheet loads");
messageHandler({ data: "3" });
assert.equal(reloads, 2, "a duplicate version must not reload again");
"#;
        let output = std::process::Command::new("node")
            .arg("-e")
                        .arg(harness)
            .arg(script)
            .output()
            .expect("Node.js should be available for the JavaScript tests");

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
