//! The cache of the build.
//!
//! A build reads every file of the project and compiles it. When nothing
//! has changed, the result of the last build is taken from the cache, and
//! the project is built again in no time.

use std::fs;
use std::path::{Path, PathBuf};

use crate::compiler::{Bundle, SourceFile, StyleFile};
use crate::sourcemap::Segment;
use crate::warn::Warning;

/// The folder the cache is written in.
const CACHE_FOLDER: &str = ".vey";

/// The name of the file the cache is written in.
const CACHE_FILE: &str = "cache.bin";

/// The part of a bundle that is written to the cache.
pub struct Cached {
    pub code: String,
    pub sources: Vec<SourceFile>,
    pub mappings: Vec<Segment>,
    pub styles: Vec<StyleFile>,
    /// The paths of the modules, so that the watcher may know them.
    pub modules: Vec<PathBuf>,
    /// The warnings of the program.
    pub warnings: Vec<Warning>,
}

/// The number of a piece of text.
///
/// The number is not a secret and does not have to be strong: it says
/// whether the sources of the project are the same as they were.
pub fn hash(parts: &[&str]) -> u64 {
    let mut value: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.as_bytes() {
            value ^= *byte as u64;
            value = value.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // The end of a part, so that two parts are not one.
        value ^= 0xff;
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    value
}

/// Whether a bundle may be built from the cache.
///
/// The key includes the compiler build, sources and styles. It is stable for
/// identical compiler sources, unlike the executable's modification time.
pub fn key(bundle_sources: &[SourceFile], styles: &[StyleFile], version: &str) -> u64 {
    key_with_fingerprint(
        bundle_sources,
        styles,
        version,
        env!("VEY_COMPILER_FINGERPRINT"),
    )
}

fn key_with_fingerprint(
    bundle_sources: &[SourceFile],
    styles: &[StyleFile],
    version: &str,
    fingerprint: &str,
) -> u64 {
    let mut parts: Vec<String> =
        Vec::with_capacity(bundle_sources.len() * 2 + styles.len() * 2 + 3);
    parts.push(version.to_string());
    parts.push(fingerprint.to_string());
    for source in bundle_sources {
        parts.push(source.path.clone());
        parts.push(source.content.clone());
    }
    // A stylesheet is a file of its own, so a change in it is a change in
    // the project.
    for style in styles {
        parts.push(style.path.display().to_string());
        parts.push(style.content.clone());
    }
    let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
    hash(&borrowed)
}

/// Put a bundle that was built into the cache.
pub fn store(folder: &Path, bundle: &Bundle) {
    let cached = Cached {
        code: bundle.code.clone(),
        sources: bundle.sources.clone(),
        mappings: bundle.mappings.clone(),
        styles: bundle.styles.clone(),
        modules: bundle
            .modules
            .iter()
            .map(|module| module.path.clone())
            .collect(),
        warnings: bundle.warnings.clone(),
    };
    let k = key(&cached.sources, &cached.styles, env!("CARGO_PKG_VERSION"));
    if let Err(err) = write_folder(folder, k, &cached) {
        eprintln!("cannot write the cache: {}", err);
    }
}

/// Read a bundle from the cache.
///
/// The key is the number of the sources, so that a cache of another project
/// is never used.
pub fn read(folder: &Path, key: u64) -> Option<Cached> {
    let path = folder.join(CACHE_FOLDER).join(CACHE_FILE);
    let bytes = fs::read(path).ok()?;
    let mut input = &bytes[..];

    let stored_key = read_count(&mut input)? as u64;
    if stored_key != key {
        return None;
    }
    let code = read_text(&mut input)?;
    let sources_len = read_count(&mut input)?;
    let mut sources = Vec::with_capacity(sources_len);
    for _ in 0..sources_len {
        let path = read_text(&mut input)?;
        let content = read_text(&mut input)?;
        sources.push(SourceFile { path, content });
    }

    let mappings_len = read_count(&mut input)?;
    let mut mappings = Vec::with_capacity(mappings_len);
    for _ in 0..mappings_len {
        let mut values = [0u64; 5];
        for value in values.iter_mut() {
            if input.len() < 8 {
                return None;
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&input[..8]);
            *value = u64::from_le_bytes(bytes);
            input = &input[8..];
        }
        mappings.push(Segment {
            generated_line: values[0] as usize,
            generated_column: values[1] as usize,
            source: values[2] as usize,
            source_line: values[3] as usize,
            source_column: values[4] as usize,
        });
    }

    let styles_len = read_count(&mut input)?;
    let mut styles = Vec::with_capacity(styles_len);
    for _ in 0..styles_len {
        let path = read_text(&mut input)?;
        let content = read_text(&mut input)?;
        styles.push(StyleFile {
            path: PathBuf::from(path),
            content,
        });
    }

    let modules_len = read_count(&mut input)?;
    let mut modules = Vec::with_capacity(modules_len);
    for _ in 0..modules_len {
        let path = read_text(&mut input)?;
        modules.push(PathBuf::from(path));
    }

    let warnings_len = read_count(&mut input)?;
    let mut warnings = Vec::with_capacity(warnings_len);
    for _ in 0..warnings_len {
        let message = read_text(&mut input)?;
        let file = read_text(&mut input)?;
        let line = read_count(&mut input)?;
        let col = read_count(&mut input)?;
        let hint = if input.is_empty() {
            None
        } else {
            let has_hint = input[0] == 1;
            input = &input[1..];
            if has_hint {
                Some(read_text(&mut input)?)
            } else {
                None
            }
        };
        warnings.push(Warning {
            message,
            file,
            line,
            col,
            hint,
        });
    }

    Some(Cached {
        code,
        sources,
        mappings,
        styles,
        modules,
        warnings,
    })
}

/// The number of a piece of data, so that the parts may be read back.
fn read_count(input: &mut &[u8]) -> Option<usize> {
    if input.len() < 8 {
        return None;
    }
    let mut len = [0u8; 8];
    len.copy_from_slice(&input[..8]);
    let len = u64::from_le_bytes(len) as usize;
    *input = &input[8..];
    Some(len)
}

/// Write a bundle to the cache.
fn write_folder(folder: &Path, key: u64, cached: &Cached) -> std::io::Result<()> {
    let path = folder.join(CACHE_FOLDER).join(CACHE_FILE);
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&key.to_le_bytes());
    write_text(&mut out, &cached.code);

    write_count(&mut out, cached.sources.len());
    for source in &cached.sources {
        write_text(&mut out, &source.path);
        write_text(&mut out, &source.content);
    }

    write_count(&mut out, cached.mappings.len());
    for segment in &cached.mappings {
        for value in [
            segment.generated_line,
            segment.generated_column,
            segment.source,
            segment.source_line,
            segment.source_column,
        ] {
            out.extend_from_slice(&(value as u64).to_le_bytes());
        }
    }

    write_count(&mut out, cached.styles.len());
    for style in &cached.styles {
        write_text(&mut out, &style.path.display().to_string());
        write_text(&mut out, &style.content);
    }

    write_count(&mut out, cached.modules.len());
    for module in &cached.modules {
        write_text(&mut out, &module.display().to_string());
    }

    write_count(&mut out, cached.warnings.len());
    for warning in &cached.warnings {
        write_text(&mut out, &warning.message);
        write_text(&mut out, &warning.file);
        write_count(&mut out, warning.line);
        write_count(&mut out, warning.col);
        match &warning.hint {
            Some(hint) => {
                out.push(1);
                write_text(&mut out, hint);
            }
            None => out.push(0),
        }
    }

    fs::create_dir_all(folder.join(CACHE_FOLDER))?;
    fs::write(path, out)
}

/// Write a piece of data as a u64 little-endian to out.
fn write_count(out: &mut Vec<u8>, count: usize) {
    out.extend_from_slice(&(count as u64).to_le_bytes());
}

/// Write a piece of text.
fn write_text(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u64).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// Read a piece of text.
fn read_text(input: &mut &[u8]) -> Option<String> {
    if input.len() < 8 {
        return None;
    }
    let mut len = [0u8; 8];
    len.copy_from_slice(&input[..8]);
    let len = u64::from_le_bytes(len) as usize;
    if input.len() < 8 + len {
        return None;
    }
    let text = String::from_utf8(input[8..8 + len].to_vec()).ok()?;
    *input = &input[8 + len..];
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::{key_with_fingerprint, SourceFile, StyleFile};
    use std::path::PathBuf;

    #[test]
    fn compiler_fingerprint_invalidates_cache_without_changing_project_inputs() {
        let sources = vec![SourceFile {
            path: "main.vey".to_string(),
            content: "print(1)".to_string(),
        }];
        let styles = vec![StyleFile {
            path: PathBuf::from("site.css"),
            content: "body { color: black }".to_string(),
        }];

        let first = key_with_fingerprint(&sources, &styles, "0.7.0", "compiler-a");
        let same_build = key_with_fingerprint(&sources, &styles, "0.7.0", "compiler-a");
        let changed_build = key_with_fingerprint(&sources, &styles, "0.7.0", "compiler-b");

        assert_eq!(first, same_build);
        assert_ne!(first, changed_build);
    }
}
