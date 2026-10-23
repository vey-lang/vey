use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort();
        for entry in entries {
            collect_files(&entry, files)?;
        }
    } else if metadata.is_file() {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn feed(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    *hash ^= 0xff;
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
}

fn main() -> std::io::Result<()> {
    let root = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets the manifest directory"),
    );
    let mut files = vec![root.join("Cargo.toml")];
    for optional in ["Cargo.lock", "build.rs"] {
        let path = root.join(optional);
        if path.is_file() {
            files.push(path);
        }
    }
    collect_files(&root.join("src"), &mut files)?;
    files.sort();
    files.dedup();

    let mut hash = 0xcbf2_9ce4_8422_2325;
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        feed(&mut hash, relative.to_string_lossy().as_bytes());
        feed(&mut hash, &fs::read(path)?);
    }

    for name in [
        "TARGET",
        "PROFILE",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_CFG_TARGET_FEATURE",
        "CARGO_CFG_TARGET_OS",
        "CARGO_CFG_TARGET_ARCH",
        "CARGO_CFG_TARGET_ENV",
        "CARGO_CFG_TARGET_ENDIAN",
        "CARGO_CFG_TARGET_POINTER_WIDTH",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        if let Ok(value) = env::var(name) {
            feed(&mut hash, name.as_bytes());
            feed(&mut hash, value.as_bytes());
        }
    }

    let mut features: Vec<_> = env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_FEATURE_"))
        .collect();
    features.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, value) in features {
        println!("cargo:rerun-if-env-changed={name}");
        feed(&mut hash, name.as_bytes());
        feed(&mut hash, value.as_bytes());
    }

    if let Some(rustc) = env::var_os("RUSTC") {
        if let Ok(output) = Command::new(rustc).arg("--version").output() {
            feed(&mut hash, &output.stdout);
        }
    }

    println!("cargo:rustc-env=VEY_COMPILER_FINGERPRINT={hash:016x}");
    Ok(())
}
