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
use crate::cli::entry::{collect_files, default_title, entry_from, project_root};
use crate::cli::error::BuildError;
use crate::cli::output::html_document;
use crate::cli::site::{default_404_page, site_files};

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
    /// The title of the site.
    pub(crate) title: String,
    /// Files imported from outside the project root. They are not covered by
    /// the directory snapshot, so the watcher tracks them explicitly.
    pub(crate) watch: Vec<PathBuf>,
}

pub(crate) fn cmd_dev(args: &[String]) -> i32 {
    let parsed = parse_args(args);
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey dev [entry.vey] [--port 4000]");
            return 1;
        }
    };

    let root = project_root(&entry);
    let host = parsed
        .host
        .clone()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parsed.port.unwrap_or(DEFAULT_PORT);
    let title = parsed
        .title
        .clone()
        .unwrap_or_else(|| default_title(&entry));

    let state = Arc::new(Mutex::new(DevState::default()));
    {
        let mut guard = lock(&state);
        guard.root = root.canonicalize().unwrap_or_else(|_| root.clone());
        guard.title = title.clone();
    }
    if let Err(err) = refresh_dev(&entry, &title, &state) {
        eprintln!("{}", err);
        eprintln!("waiting for changes...");
    }

    let listener = match TcpListener::bind((host.as_str(), port)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("cannot listen on {}:{}: {}", host, port, err);
            return 1;
        }
    };

    println!(
        "Vey dev server running at http://{}:{}",
        if host == "0.0.0.0" {
            "localhost"
        } else {
            &host
        },
        port
    );
    println!("entry: {}", entry.display());
    println!("watching for changes, press Ctrl+C to stop");

    let watcher_state = Arc::clone(&state);
    let watcher_entry = entry.clone();
    let watcher_title = title.clone();
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
    let result = site_files(entry, Some(title), &mut watch);
    let mut guard = lock(state);
    guard.version += 1;
    match result {
        Ok(files) => {
            let count = files.len();
            guard.files = files;
            guard.error = None;
            guard.watch = watch;
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

    let (status, content_type, body) = {
        let mut guard = lock(&state);
        respond(&mut guard, path)
    };

    write_response(&mut stream, status, content_type, &body, head);
}

pub(crate) fn respond(state: &mut DevState, path: &str) -> (u16, &'static str, Vec<u8>) {
    if path == "/__vey/version" {
        return (
            200,
            "text/plain; charset=utf-8",
            state.version.to_string().into_bytes(),
        );
    }

    if path.ends_with(".vey") {
        if let Some(page) = render_vey_page(state, path) {
            return page;
        }
    }

    if let Some(err) = &state.error {
        if accepts_html(path) {
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
        let body = if content_type.starts_with("text/html") {
            inject_reload(&String::from_utf8_lossy(bytes), state.version).into_bytes()
        } else {
            bytes.clone()
        };
        return (200, content_type, body);
    }

    let nested = format!("{}/index.html", key);
    if let Some(bytes) = state.files.get(&nested) {
        let body = inject_reload(&String::from_utf8_lossy(bytes), state.version).into_bytes();
        return (200, "text/html; charset=utf-8", body);
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

    let root = state.root.clone();
    let file = root.join(&key).canonicalize().ok()?;
    if !file.starts_with(&root) || !file.is_file() {
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
            let favicon = root.join("public/favicon.svg").is_file();
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
    let snippet = format!(
        "<script>(function(){{var v={};setInterval(function(){{fetch('/__vey/version',{{cache:'no-store'}}).then(function(r){{return r.text()}}).then(function(t){{if(t!==String(v)){{v=t;location.reload()}}}}).catch(function(){{}})}},600)}})();</script>",
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
) {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len()
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
