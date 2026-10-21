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
    color-scheme: light dark;
    --bg: #ffffff;
    --fg: #0b0b0b;
    --muted: #6e6e6e;
    --line: #e3e3e3;
    --soft: #f6f6f6;
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #0b0b0b;
      --fg: #f4f4f4;
      --muted: #8d8d8d;
      --line: #262626;
      --soft: #141414;
    }
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    min-height: 100vh;
    padding: 3rem 1.5rem;
    background: var(--bg);
    color: var(--fg);
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    line-height: 1.6;
    -webkit-font-smoothing: antialiased;
  }
  main { max-width: 46rem; margin: 0 auto; }
  .top {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
  }
  .mark { width: 16px; height: 16px; }
  .kicker {
    margin: 2.5rem 0 0;
    color: var(--muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.6875rem;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }
  h1 {
    margin: 1rem 0 0;
    font-family: ui-serif, Georgia, Cambria, "Times New Roman", serif;
    font-size: clamp(1.75rem, 6vw, 2.5rem);
    font-weight: 400;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }
  .sub { margin: 1rem 0 0; max-width: 34rem; color: var(--muted); }
  .location {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    margin-top: 1.5rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8125rem;
  }
  .file { max-width: 100%; overflow-wrap: anywhere; color: var(--muted); }
  .pos { color: var(--fg); }
  .message {
    margin-top: 2rem;
    padding: 1rem 1.25rem;
    border: 1px solid var(--line);
    border-left: 3px solid var(--fg);
    border-radius: 0 0.5rem 0.5rem 0;
    background: var(--soft);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.875rem;
    overflow-wrap: anywhere;
  }
  .code {
    margin: 1.25rem 0 0;
    padding: 0.75rem 0;
    overflow-x: auto;
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    background: var(--soft);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8125rem;
    line-height: 1.7;
  }
  .line { display: flex; gap: 1rem; padding: 0 1rem; white-space: pre; }
  .line.error { background: var(--bg); }
  .no {
    flex: none;
    width: 3ch;
    color: var(--muted);
    text-align: right;
    user-select: none;
  }
  .text { color: var(--fg); }
  .caret { color: var(--fg); font-weight: 700; }
  .hint {
    margin-top: 1.25rem;
    padding: 0.875rem 1rem;
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    color: var(--muted);
    font-size: 0.875rem;
  }
  .hint b { color: var(--fg); font-weight: 600; }
  code {
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--soft);
    color: var(--fg);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.92em;
  }
  footer {
    margin-top: 2.5rem;
    padding-top: 1.25rem;
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 0.8125rem;
  }
</style>
</head>
<body>
<main>
  <div class="top">
    <svg class="mark" viewBox="0 0 32 32" aria-hidden="true"><path d="M16 2C16 10 22 16 30 16 22 16 16 22 16 30 16 22 10 16 2 16 10 16 16 10 16 2Z" fill="currentColor"/></svg>
    <span>Vey dev</span>
  </div>
  <p class="kicker">Compile error</p>
  <h1>This page could not be built.</h1>
  <p class="sub">Vey stopped at the first problem. Fix it and save the file, the page reloads on its own.</p>
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
        out.push_str("<footer>Vey dev &middot; watching for changes</footer></main></body></html>");
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
