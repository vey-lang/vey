//! The `vey deploy` command.
//!
//! The command builds the site and says where it is, so that a host may
//! serve it. With a target, the site is copied into a folder that the host
//! already watches.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::cli::args::parse_args;
use crate::cli::build::OUTPUT_FOLDER;
use crate::cli::entry::entry_from;
use crate::cli::output::print_command_help;

const DEPLOY_MANIFEST: &str = ".vey-deploy-manifest";
const DEPLOY_MANIFEST_HEADER: &str = "VEY-DEPLOY-MANIFEST-1";
const MAX_DEPLOY_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DEPLOY_FILES: usize = 1_000_000;

pub(crate) fn cmd_deploy(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("deploy");
        return 0;
    }

    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey deploy [entry.vey|directory]");
            return 1;
        }
    };

    let root = crate::cli::entry::project_root(&entry);
    let output = parsed
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(OUTPUT_FOLDER));

    let build =
        crate::cli::build::build_site(&entry, &output, parsed.title.as_deref(), parsed.force);
    if build != 0 {
        return build;
    }

    // The site may be copied into a folder that a host watches.
    if let Some(target) = &parsed.target {
        let target = PathBuf::from(target);
        if let Err(err) = copy_folder(&output, &target) {
            eprintln!("cannot copy the site into {}: {}", target.display(), err);
            return 1;
        }
        println!("the site is in {}", target.display());
        return 0;
    }

    println!("the site is in {}", output.display());
    println!("serve that folder with any host of static files.");
    0
}

/// Copy the built site without removing files that do not belong to Vey.
///
/// The target manifest records paths from an earlier deploy. It is validated
/// before use and only those recorded files can be removed as stale.
fn copy_folder(from: &Path, to: &Path) -> Result<(), io::Error> {
    if !from.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the site was not built",
        ));
    }

    let from = fs::canonicalize(from)?;
    let target_path = crate::cli::site::absolute(to);
    if paths_overlap(&from, &target_path) {
        return Err(invalid_input(
            "the deployment target and build output may not contain one another",
        ));
    }
    fs::create_dir_all(&target_path)?;
    let target = fs::canonicalize(&target_path)?;
    if paths_overlap(&from, &target) {
        return Err(invalid_input(
            "the deployment target and build output may not contain one another",
        ));
    }

    let previous = read_deploy_manifest(&target)?;
    let mut files = Vec::new();
    collect_files(&from, &from, &mut files)?;
    files.sort_by(|a: &(PathBuf, PathBuf), b| a.0.cmp(&b.0));

    let current = files
        .iter()
        .map(|(relative, _)| relative.clone())
        .collect::<HashSet<_>>();
    if current.iter().any(|path| is_deploy_manifest(path)) {
        return Err(invalid_input(format!(
            "the build uses the reserved {} path",
            DEPLOY_MANIFEST
        )));
    }

    // Check all conflicts before removing any stale Vey files or copying new
    // files, so an unmanaged host file cannot be partially overwritten.
    for (relative, _) in &files {
        preflight_target(&target, relative, &previous, &current)?;
    }

    let mut stale = previous
        .difference(&current)
        .cloned()
        .collect::<Vec<PathBuf>>();
    stale.sort();
    for relative in stale {
        remove_owned_file(&target, &relative)?;
    }

    for (relative, source) in &files {
        ensure_parent_directories(&target, relative)?;
        let destination = target.join(relative);
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(invalid_input(format!(
                    "refusing to write through symbolic link {}",
                    destination.display()
                )));
            }
            Ok(metadata) if metadata.is_file() => {
                if !previous.contains(relative) {
                    return Err(invalid_input(format!(
                        "refusing to overwrite unmanaged file {}",
                        destination.display()
                    )));
                }
            }
            Ok(_) => {
                return Err(invalid_input(format!(
                    "refusing to replace non-file {}",
                    destination.display()
                )));
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        copy_file_atomically(source, &destination)?;
    }

    write_deploy_manifest(&target, &current)?;
    Ok(())
}

fn paths_overlap(first: &Path, second: &Path) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(invalid_input(format!(
                "refusing to deploy symbolic link {}",
                source.display()
            )));
        }
        if file_type.is_dir() {
            collect_files(root, &source, files)?;
        } else if file_type.is_file() {
            let relative = source
                .strip_prefix(root)
                .map_err(|_| invalid_input("a build file is outside the build output"))?
                .to_path_buf();
            validate_relative_path(&relative)?;
            files.push((relative, source));
            if files.len() > MAX_DEPLOY_FILES {
                return Err(invalid_input("the build has too many files to deploy"));
            }
        } else {
            return Err(invalid_input(format!(
                "refusing to deploy unsupported file type {}",
                source.display()
            )));
        }
    }
    Ok(())
}

fn preflight_target(
    target: &Path,
    relative: &Path,
    previous: &HashSet<PathBuf>,
    current: &HashSet<PathBuf>,
) -> io::Result<()> {
    validate_relative_path(relative)?;
    let mut parent = PathBuf::new();
    if let Some(relative_parent) = relative.parent() {
        for component in relative_parent.components() {
            parent.push(component.as_os_str());
            let path = target.join(&parent);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(invalid_input(format!(
                        "refusing to traverse symbolic link {}",
                        path.display()
                    )));
                }
                Ok(metadata) if metadata.is_dir() => {}
                Ok(metadata)
                    if metadata.is_file()
                        && previous.contains(&parent)
                        && !current.contains(&parent) => {}
                Ok(_) => {
                    return Err(invalid_input(format!(
                        "a non-directory blocks deployment at {}",
                        path.display()
                    )));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }

    let destination = target.join(relative);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(invalid_input(format!(
            "refusing to replace symbolic link {}",
            destination.display()
        ))),
        Ok(metadata) if metadata.is_file() && previous.contains(relative) => Ok(()),
        Ok(metadata) if metadata.is_file() => Err(invalid_input(format!(
            "refusing to overwrite unmanaged file {}",
            destination.display()
        ))),
        Ok(_) => Err(invalid_input(format!(
            "refusing to replace non-file {}",
            destination.display()
        ))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn ensure_parent_directories(root: &Path, relative: &Path) -> io::Result<()> {
    let mut current = root.to_path_buf();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(invalid_input(format!(
                        "refusing to traverse symbolic link {}",
                        current.display()
                    )));
                }
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    return Err(invalid_input(format!(
                        "a non-directory blocks deployment at {}",
                        current.display()
                    )));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => fs::create_dir(&current)?,
                Err(err) => return Err(err),
            }
        }
    }
    Ok(())
}

fn remove_owned_file(root: &Path, relative: &Path) -> io::Result<()> {
    validate_relative_path(relative)?;
    let mut current = root.to_path_buf();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
                Ok(_) => return Ok(()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    }

    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() || metadata.file_type().is_symlink() => {
            fs::remove_file(&path)?;
        }
        Ok(_) => return Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    }

    let mut folder = relative.parent();
    while let Some(relative_folder) = folder {
        if relative_folder.as_os_str().is_empty() {
            break;
        }
        let path = root.join(relative_folder);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                if fs::remove_dir(&path).is_err() {
                    break;
                }
            }
            _ => break,
        }
        folder = relative_folder.parent();
    }
    Ok(())
}

fn read_deploy_manifest(target: &Path) -> io::Result<HashSet<PathBuf>> {
    let manifest = target.join(DEPLOY_MANIFEST);
    let metadata = match fs::symlink_metadata(&manifest) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(err) => return Err(err),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid_input(format!(
            "{} is not a regular deployment manifest",
            manifest.display()
        )));
    }
    if metadata.len() > MAX_DEPLOY_MANIFEST_BYTES {
        return Err(invalid_input("the deployment manifest is too large"));
    }

    let text = fs::read_to_string(&manifest)?;
    let mut lines = text.lines();
    if lines.next() != Some(DEPLOY_MANIFEST_HEADER) {
        return Err(invalid_input(format!(
            "{} is not a Vey deployment manifest; refusing to modify the target",
            manifest.display()
        )));
    }

    let mut paths = HashSet::new();
    for line in lines {
        let path = decode_manifest_path(line)?;
        if is_deploy_manifest(&path) || !paths.insert(path) {
            return Err(invalid_input(
                "the deployment manifest has a reserved or duplicate path",
            ));
        }
    }
    Ok(paths)
}

fn write_deploy_manifest(target: &Path, files: &HashSet<PathBuf>) -> io::Result<()> {
    let mut entries = files
        .iter()
        .map(|path| encode_manifest_path(path))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();

    let mut text = String::from(DEPLOY_MANIFEST_HEADER);
    text.push('\n');
    for entry in entries {
        text.push_str(&entry);
        text.push('\n');
    }
    write_file_atomically(&target.join(DEPLOY_MANIFEST), text.as_bytes())
}

fn encode_manifest_path(path: &Path) -> io::Result<String> {
    validate_relative_path(path)?;
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(invalid_input("the deployment path is not relative"));
        };
        let name = name
            .to_str()
            .ok_or_else(|| invalid_input("deployment paths must be valid UTF-8"))?;
        components.push(name);
    }
    let normalized = components.join("/");
    let hex = b"0123456789abcdef";
    let mut encoded = String::with_capacity(normalized.len() * 2);
    for byte in normalized.as_bytes() {
        encoded.push(hex[(byte >> 4) as usize] as char);
        encoded.push(hex[(byte & 0x0f) as usize] as char);
    }
    Ok(encoded)
}

fn decode_manifest_path(encoded: &str) -> io::Result<PathBuf> {
    if encoded.is_empty() || encoded.len() % 2 != 0 {
        return Err(invalid_input(
            "the deployment manifest contains an invalid path",
        ));
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let high = hex_value(pair[0])
            .ok_or_else(|| invalid_input("the deployment manifest contains invalid hex"))?;
        let low = hex_value(pair[1])
            .ok_or_else(|| invalid_input("the deployment manifest contains invalid hex"))?;
        bytes.push((high << 4) | low);
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| invalid_input("the deployment manifest path is not UTF-8"))?;
    let path = PathBuf::from(text);
    validate_relative_path(&path)?;
    Ok(path)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn validate_relative_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_input(
            "the deployment manifest contains an unsafe relative path",
        ));
    }
    Ok(())
}

fn is_deploy_manifest(path: &Path) -> bool {
    path.components().count() == 1
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(DEPLOY_MANIFEST))
}

fn write_file_atomically(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| invalid_input("a deployment file has no parent directory"))?;
    let (temporary, mut file) = create_temporary_file(parent)?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if let Err(err) = result {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }
    replace_with_temporary(&temporary, destination)
}

fn copy_file_atomically(source: &Path, destination: &Path) -> io::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| invalid_input("a deployment file has no parent directory"))?;
    let mut source_file = File::open(source)?;
    let permissions = source_file.metadata()?.permissions();
    let (temporary, mut output_file) = create_temporary_file(parent)?;
    let result = io::copy(&mut source_file, &mut output_file)
        .and_then(|_| output_file.sync_all())
        .and_then(|_| fs::set_permissions(&temporary, permissions));
    drop(output_file);
    if let Err(err) = result {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }
    replace_with_temporary(&temporary, destination)
}

fn create_temporary_file(directory: &Path) -> io::Result<(PathBuf, File)> {
    static NEXT_TEMPORARY: AtomicUsize = AtomicUsize::new(0);
    for _ in 0..1000 {
        let index = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(".vey-deploy-tmp-{}-{}", std::process::id(), index));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a temporary deployment file",
    ))
}

fn replace_with_temporary(temporary: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if fs::symlink_metadata(destination).is_ok() {
        if let Err(err) = fs::remove_file(destination) {
            let _ = fs::remove_file(temporary);
            return Err(err);
        }
    }

    match fs::rename(temporary, destination) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(temporary);
            Err(err)
        }
    }
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
