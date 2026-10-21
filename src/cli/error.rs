//! Build errors with a source frame for the terminal and the browser.
use std::fs;
use std::path::Path;

use vey::VeyError;

use crate::cli::output::escape_html;

#[derive(Clone)]
pub(crate) struct BuildError {
    file: String,
    line: usize,
    col: usize,
    message: String,
    hint: Option<String>,
    source: Option<String>,
}

pub(crate) fn compact_path(path: &str) -> String {
    let path = Path::new(path);
    let cwd = std::env::current_dir().ok();
    if let Some(cwd) = cwd {
        if let Ok(relative) = path.strip_prefix(&cwd) {
            let text = relative.display().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    path.display().to_string()
}

impl BuildError {
    pub(crate) fn from_vey(err: &VeyError) -> Self {
        let source = fs::read_to_string(&err.file).ok();
        Self {
            file: compact_path(&err.file),
            line: err.line,
            col: err.col,
            message: err.message.clone(),
            hint: err.hint.clone(),
            source,
        }
    }

    pub(crate) fn simple(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            file: String::new(),
            line: 0,
            col: 0,
            message: message.clone(),
            hint: None,
            source: None,
        }
    }

    pub(crate) fn source_line(&self) -> Option<&str> {
        if self.line == 0 {
            return None;
        }
        self.source.as_ref()?.lines().nth(self.line - 1)
    }

    pub(crate) fn code_frame(&self) -> Option<String> {
        let source = self.source.as_ref()?;
        if self.line == 0 {
            return None;
        }
        let lines: Vec<&str> = source.lines().collect();
        if lines.is_empty() {
            return None;
        }
        let target = self.line.min(lines.len());
        let start = target.saturating_sub(2).max(1);
        let end = (target + 2).min(lines.len());

        let mut out = String::from("<pre class=\"code\"><code>");
        for number in start..=end {
            let class = if number == target {
                "line error"
            } else {
                "line"
            };
            out.push_str(&format!(
                "<span class=\"{}\"><span class=\"no\">{}</span><span class=\"text\">{}</span></span>",
                class,
                number,
                escape_html(lines[number - 1])
            ));
            if number == target {
                out.push_str(&format!(
                    "<span class=\"line error\"><span class=\"no\"></span><span class=\"text\"><span class=\"caret\">{}^</span></span></span>",
                    " ".repeat(self.col.saturating_sub(1))
                ));
            }
        }
        out.push_str("</code></pre>");
        Some(out)
    }

    pub(crate) fn render_terminal(&self) -> String {
        let mut out = format!("error: {}", self.message);
        if !self.file.is_empty() {
            out.push_str(&format!("\n  --> {}:{}:{}", self.file, self.line, self.col));
            if let Some(line) = self.source_line() {
                let number = self.line.to_string();
                let pad = " ".repeat(number.len());
                out.push_str(&format!("\n{pad} |"));
                out.push_str(&format!("\n{number} | {line}"));
                out.push_str(&format!(
                    "\n{pad} | {}^",
                    " ".repeat(self.col.saturating_sub(1))
                ));
            }
        }
        if let Some(hint) = &self.hint {
            out.push_str(&format!("\n  = hint: {}", hint));
        }
        out
    }

    pub(crate) fn render_html(&self) -> String {
        let mut out = String::from(
            r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Build failed · Vey</title>
<style>
  :root {
    color-scheme: dark;
    --bg: #0f172a;
    --border: #2a3550;
    --text: #e2e8f0;
    --muted: #94a3b8;
    --accent: #818cf8;
    --danger: #f87171;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 32px 20px;
    background:
      radial-gradient(620px 320px at 50% -8%, rgba(129, 140, 248, 0.14), transparent 70%),
      var(--bg);
    color: var(--text);
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  .panel { width: min(780px, 100%); }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin: 0 2px 16px;
    color: var(--muted);
    font-size: 13px;
  }
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    color: var(--text);
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .mark { width: 18px; height: 18px; }
  .card {
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 18px;
    background: rgba(22, 33, 58, 0.72);
    box-shadow: 0 28px 90px rgba(0, 0, 0, 0.38);
  }
  .head { padding: 26px 26px 0; }
  .eyebrow {
    margin: 0 0 10px;
    color: var(--danger);
    font-size: 11.5px;
    font-weight: 650;
    letter-spacing: 0.15em;
    text-transform: uppercase;
  }
  h1 {
    margin: 0;
    font-size: clamp(21px, 3vw, 25px);
    font-weight: 600;
    letter-spacing: -0.025em;
  }
  .sub {
    margin: 10px 0 0;
    color: var(--muted);
    font-size: 13.5px;
    line-height: 1.55;
  }
  .location {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
    font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .file {
    max-width: 100%;
    padding: 6px 10px;
    overflow-wrap: anywhere;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--muted);
    background: rgba(15, 23, 42, 0.55);
  }
  .pos { color: var(--accent); }
  .body { padding: 20px 26px 26px; }
  .message {
    padding: 14px 16px;
    border-left: 3px solid var(--danger);
    border-radius: 0 10px 10px 0;
    background: rgba(248, 113, 113, 0.07);
    color: #fecaca;
    font: 13.5px/1.65 ui-monospace, SFMono-Regular, Menlo, monospace;
    overflow-wrap: anywhere;
  }
  .code {
    margin: 16px 0 0;
    padding: 12px 0;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: #0b1222;
    font: 13px/1.75 ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .line {
    display: flex;
    gap: 14px;
    padding: 0 16px;
    white-space: pre;
  }
  .line.error { background: rgba(248, 113, 113, 0.09); }
  .no {
    flex: none;
    width: 2ch;
    color: #475569;
    text-align: right;
    user-select: none;
  }
  .text { color: #cbd5e1; }
  .caret { color: var(--danger); }
  .hint {
    display: flex;
    gap: 10px;
    margin-top: 16px;
    padding: 12px 14px;
    border: 1px solid rgba(129, 140, 248, 0.32);
    border-radius: 10px;
    background: rgba(129, 140, 248, 0.08);
    color: #c7d2fe;
    font-size: 13.5px;
    line-height: 1.55;
  }
  .hint b { flex: none; color: #e0e7ff; font-weight: 600; }
  code {
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(15, 23, 42, 0.55);
    color: #e0e7ff;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.92em;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 16px 2px 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #34d399;
    box-shadow: 0 0 10px rgba(52, 211, 153, 0.6);
  }
</style>
</head>
<body>
<main class="panel">
  <div class="top">
    <span class="brand">
      <svg class="mark" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2c.7 5.3 3.9 8.5 9.2 9.2-5.3.7-8.5 3.9-9.2 9.2-.7-5.3-3.9-8.5-9.2-9.2C8.1 10.5 11.3 7.3 12 2Z" fill="#818cf8"/></svg>
      Vey dev
    </span>
    <span>Build failed</span>
  </div>
  <section class="card" role="alert">
    <div class="head">
      <p class="eyebrow">Compile error</p>
      <h1>This page could not be built.</h1>
      <p class="sub">Vey stopped at the first problem. Fix it and save the file — the page reloads on its own.</p>
"##,
        );

        if !self.file.is_empty() {
            out.push_str("<div class=\"location\">");
            out.push_str(&format!(
                "<span class=\"file\">{}</span>",
                escape_html(&self.file)
            ));
            if self.line > 0 {
                out.push_str(&format!(
                    "<span class=\"pos\">{}:{}</span>",
                    self.line, self.col
                ));
            }
            out.push_str("</div>");
        }

        out.push_str("</div><div class=\"body\">");
        out.push_str(&format!(
            "<div class=\"message\">{}</div>",
            escape_html(&self.message)
        ));
        if let Some(frame) = self.code_frame() {
            out.push_str(&frame);
        }
        if let Some(hint) = &self.hint {
            out.push_str(&format!(
                "<div class=\"hint\"><b>Hint</b><span>{}</span></div>",
                inline_code(hint)
            ));
        }
        out.push_str(
            "</div></section><div class=\"footer\"><span class=\"dot\"></span><span>Watching for changes. Save the file to rebuild.</span></div></main></body></html>",
        );
        out
    }
}

impl From<String> for BuildError {
    fn from(message: String) -> Self {
        BuildError::simple(message)
    }
}

impl From<&str> for BuildError {
    fn from(message: &str) -> Self {
        BuildError::simple(message)
    }
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render_terminal())
    }
}

pub(crate) fn inline_code(text: &str) -> String {
    let mut out = String::new();
    for (index, part) in text.split('`').enumerate() {
        if index % 2 == 1 {
            out.push_str("<code>");
            out.push_str(&escape_html(part));
            out.push_str("</code>");
        } else {
            out.push_str(&escape_html(part));
        }
    }
    out
}
