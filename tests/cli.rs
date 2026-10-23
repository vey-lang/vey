//! Tests for the command line tools.

mod common;

use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::temp_dir;

fn vey() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vey"))
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

const PAGE: &str = "component App() {\n  main.page { h1 { \"Hi\" } }\n}\n\nbind(\"#app\", App())\n";

#[test]
fn a_build_refuses_to_write_into_a_directory_that_holds_the_project() {
    let dir = temp_dir("build-guard");
    let project = dir.join("parent/project");
    write(&project.join("main.vey"), PAGE);
    write(&dir.join("parent/precious.txt"), "keep me\n");

    // The parent of the project holds a file that must not be lost.
    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("parent"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("contains the project"), "{error}");

    // The file is still there and nothing was written.
    assert!(dir.join("parent/precious.txt").is_file());
    assert!(!dir.join("parent/index.html").exists());
}

#[test]
fn a_build_refuses_to_write_into_a_parent_of_the_project() {
    let dir = temp_dir("build-guard-parent");
    let project = dir.join("a/b/project");
    write(&project.join("main.vey"), PAGE);
    write(&dir.join("a/b/precious.txt"), "keep me\n");

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("a"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(dir.join("a/b/precious.txt").is_file());
    assert!(project.join("main.vey").is_file());
}

#[test]
fn a_build_writes_into_an_ordinary_folder() {
    let dir = temp_dir("build-ok");
    let project = dir.join("project");
    write(&project.join("main.vey"), PAGE);

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.join("out/index.html").is_file());
    assert!(dir.join("out/404.html").is_file());
}

#[test]
fn a_link_in_the_public_folder_does_not_leak_files() {
    let dir = temp_dir("symlink");
    let project = dir.join("project");
    write(&project.join("main.vey"), PAGE);
    write(&dir.join("secret.txt"), "top secret\n");

    let public = project.join("public");
    fs::create_dir_all(&public).unwrap();
    std::os::unix::fs::symlink(dir.join("secret.txt"), public.join("leak.txt")).unwrap();

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.join("out/leak.txt").exists());
}

#[test]
fn a_project_in_a_src_folder_keeps_its_public_folder() {
    let dir = temp_dir("src-project");
    let project = dir.join("project");
    write(&project.join("src/main.vey"), PAGE);
    write(&project.join("public/robots.txt"), "User-agent: *\n");

    let output = vey()
        .arg("build")
        .arg(project.join("src/main.vey"))
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The public folder of the project is copied, not the one of `src`.
    assert!(dir.join("out/robots.txt").is_file());
}

#[test]
fn code_that_is_nested_too_deeply_is_reported() {
    let dir = temp_dir("deep");
    let file = dir.join("main.vey");
    let source = format!("let x = {}1{}\n", "(".repeat(500), ")".repeat(500));
    write(&file, &source);

    let output = vey().arg("check").arg(&file).output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("nested too deeply"), "{error}");
}

#[test]
fn a_nested_type_that_is_too_deep_is_reported() {
    let dir = temp_dir("deep-type");
    let file = dir.join("main.vey");
    let source = format!("let x: {}number{} = []\n", "[".repeat(500), "]".repeat(500));
    write(&file, &source);

    let output = vey().arg("check").arg(&file).output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("nested too deeply"), "{error}");
}

#[test]
fn a_new_project_can_be_built() {
    let dir = temp_dir("new-project");
    let project = dir.join("app");

    let output = vey().arg("new").arg(&project).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.join("out/index.html").is_file());
    assert!(dir.join("out/404.html").is_file());
}

#[test]
fn every_page_of_a_project_becomes_a_page_of_the_site() {
    let dir = temp_dir("many-pages");
    let project = dir.join("project");
    write(&project.join("src/main.vey"), PAGE);
    write(
        &project.join("src/about.vey"),
        "component About() {\n  main { h1 { \"About\" } }\n}\n\nbind(\"#app\", About())\n",
    );
    write(
        &project.join("src/docs/index.vey"),
        "component Docs() {\n  main { h1 { \"Docs\" } }\n}\n\nbind(\"#app\", Docs())\n",
    );
    // A file that is imported is a module and does not become a page.
    write(
        &project.join("src/shared.vey"),
        "export fn shared() { return 1 }\n",
    );
    write(
        &project.join("src/uses.vey"),
        "use \"./shared.vey\" as shared\n\ncomponent Uses() {\n  main { h1 { \"Uses\" } }\n}\n\nbind(\"#app\", Uses())\n",
    );

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(dir.join("out/index.html").is_file());
    assert!(dir.join("out/about.html").is_file());
    assert!(dir.join("out/docs/index.html").is_file());
    assert!(dir.join("out/uses.html").is_file());
    // A module is not a page.
    assert!(!dir.join("out/shared.html").exists());
}

#[test]
fn a_page_that_cannot_be_built_does_not_stop_the_others() {
    let dir = temp_dir("broken-page");
    let project = dir.join("project");
    write(&project.join("src/main.vey"), PAGE);
    write(
        &project.join("src/bad.vey"),
        "component Bad() {\n  main { h1 { missing } }\n}\n\nbind(\"#app\", Bad())\n",
    );

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();

    // The other pages are written, and the broken one is reported.
    assert!(dir.join("out/index.html").is_file());
    assert!(dir.join("out/404.html").is_file());
    assert!(!dir.join("out/bad.html").exists());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("cannot build bad.html"), "{error}");
    assert!(error.contains("unknown variable 'missing'"), "{error}");
}

#[test]
fn a_check_of_a_file_looks_at_that_file_only() {
    let dir = temp_dir("check-one");
    let project = dir.join("project");
    write(&project.join("good.vey"), PAGE);
    // A broken file next to it does not matter when the file is named.
    write(
        &project.join("broken.vey"),
        "component Broken() {\n  main { h1 { missing } }\n}\n",
    );

    let output = vey()
        .arg("check")
        .arg(project.join("good.vey"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 page"));
}

#[test]
fn a_check_of_a_project_looks_at_every_page() {
    let dir = temp_dir("check-all");
    let project = dir.join("project");
    write(&project.join("src/main.vey"), PAGE);
    write(
        &project.join("src/broken.vey"),
        "component Broken() {\n  main { h1 { missing } }\n}\n",
    );

    let output = vey().arg("check").arg(&project).output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("unknown variable 'missing'"), "{error}");
}

#[test]
fn a_page_may_not_take_the_place_of_the_home_of_the_site() {
    let dir = temp_dir("page-clash");
    let project = dir.join("project");
    write(&project.join("src/main.vey"), PAGE);
    write(
        &project.join("src/index.vey"),
        "component Index() {\n  main { h1 { \"Index\" } }\n}\n\nbind(\"#app\", Index())\n",
    );

    let output = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("would take the place"), "{error}");

    // The home of the site is still the page of the entry. Its script is in
    // a file of its own.
    let home = std::fs::read_to_string(dir.join("out/index.js")).unwrap();
    assert!(home.contains("Hi"), "{home}");
    let page = std::fs::read_to_string(dir.join("out/index.html")).unwrap();
    assert!(page.contains("index.js"), "{page}");
}

#[test]
fn an_option_that_is_not_known_is_reported() {
    let output = vey().arg("build").arg("--nonsense").output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("unknown option '--nonsense'"), "{error}");
}

#[test]
fn an_option_without_a_value_is_reported() {
    let output = vey().arg("build").arg("--port").output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("needs a value"), "{error}");
}

#[test]
fn a_port_that_is_not_a_number_is_reported() {
    let output = vey().arg("dev").arg("--port").arg("abc").output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("is not a port"), "{error}");
}

#[test]
fn formatter_ignores_braces_inside_multiline_block_comments() {
    let dir = temp_dir("fmt-block-comment");
    let file = dir.join("main.vey");
    write(
        &file,
        "fn main() {\n  /* comment\n  }\n  */\n  let value = 1\n}\n",
    );

    let output = vey().arg("fmt").arg("--write").arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let formatted = fs::read_to_string(&file).unwrap();
    assert!(formatted.contains("  let value = 1\n"), "{formatted}");
    assert!(formatted.ends_with("}\n"), "{formatted}");

    let output = vey().arg("fmt").arg("--write").arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(file).unwrap(), formatted);
}

#[test]
fn migrate_refuses_to_downgrade_a_newer_file_version() {
    let dir = temp_dir("migrate-newer-version");
    let file = dir.join("main.vey");
    let source = "// vey: 0.99.0\nfn main() {\n  print(1)\n}\n";
    write(&file, source);

    let output = vey().arg("migrate").arg("--write").arg(&file).output().unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("newer than this compiler"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(file).unwrap(), source);
}

#[test]
fn migrate_updates_an_older_version_and_is_idempotent() {
    let dir = temp_dir("migrate-older-version");
    let file = dir.join("main.vey");
    write(&file, "// vey: 0.6.0\nfn main() {\n  print(1)\n}\n");

    let output = vey().arg("migrate").arg("--write").arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let migrated = fs::read_to_string(&file).unwrap();
    assert!(
        migrated.starts_with(&format!("// vey: {}\n", env!("CARGO_PKG_VERSION"))),
        "{migrated}"
    );

    let output = vey().arg("migrate").arg("--write").arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(file).unwrap(), migrated);
}

#[test]
fn test_watch_restarts_when_an_imported_module_changes() {
    let dir = temp_dir("test-watch-module");
    write(
        &dir.join("tests/main.vey"),
        "use \"./helper.vey\" as helper\ntest(\"module\", fn() {\n  assert(helper.value() == 2)\n})\n",
    );
    write(
        &dir.join("tests/helper.vey"),
        "export fn value() -> number { return 2 }\n",
    );

    let mut child = vey()
        .arg("test")
        .arg("--watch")
        .arg("tests/main.vey")
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if send.send(line).is_err() {
                break;
            }
        }
    });

    let mut output = String::new();
    let initial_deadline = Instant::now() + Duration::from_secs(10);
    while !output.contains("test(s) passed") && Instant::now() < initial_deadline {
        if let Ok(line) = receive.recv_timeout(Duration::from_millis(100)) {
            output.push_str(&line);
            output.push('\n');
        }
    }
    let initial_run = output.contains("test(s) passed");

    let mut watched_import = false;
    if initial_run {
        write(
            &dir.join("tests/helper.vey"),
            "export fn value() -> number { return 3 }\n",
        );
        let reload_deadline = Instant::now() + Duration::from_secs(5);
        while output.matches("test tests/main.vey").count() < 2
            && Instant::now() < reload_deadline
        {
            if let Ok(line) = receive.recv_timeout(Duration::from_millis(100)) {
                output.push_str(&line);
                output.push('\n');
            }
        }
        watched_import = output.matches("test tests/main.vey").count() >= 2;
    }

    let _ = child.kill();
    let _ = child.wait();
    assert!(initial_run, "the initial test run did not finish:\n{output}");
    assert!(watched_import, "changing an imported module did not rerun tests:\n{output}");
}

#[test]
fn test_watch_restarts_when_a_broken_import_is_fixed() {
    let dir = temp_dir("test-watch-broken-module");
    write(
        &dir.join("tests/main.vey"),
        "use \"./helper.vey\" as helper\ntest(\"module\", fn() {\n  assert(helper.value() == 2)\n})\n",
    );
    write(
        &dir.join("tests/helper.vey"),
        "export fn value() -> number { return \"broken\" }\n",
    );

    let mut child = vey()
        .arg("test")
        .arg("--watch")
        .arg("tests/main.vey")
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (send, receive) = mpsc::channel();
    let (error_send, error_receive) = mpsc::channel();
    thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if send.send(line).is_err() {
                break;
            }
        }
    });
    thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if error_send.send(line).is_err() {
                break;
            }
        }
    });

    let mut output = String::new();
    let mut errors = String::new();
    let initial_deadline = Instant::now() + Duration::from_secs(10);
    while !errors.contains("expected number, found string") && Instant::now() < initial_deadline {
        if let Ok(line) = receive.recv_timeout(Duration::from_millis(50)) {
            output.push_str(&line);
            output.push('\n');
        }
        if let Ok(line) = error_receive.try_recv() {
            errors.push_str(&line);
            errors.push('\n');
        }
    }
    let initial_failure = errors.contains("expected number, found string");

    let mut fixed_import = false;
    if initial_failure {
        write(
            &dir.join("tests/helper.vey"),
            "export fn value() -> number { return 2 }\n",
        );
        let reload_deadline = Instant::now() + Duration::from_secs(5);
        while (output.matches("test tests/main.vey").count() < 2
            || !output.contains("1 test(s) passed"))
            && Instant::now() < reload_deadline
        {
            if let Ok(line) = receive.recv_timeout(Duration::from_millis(100)) {
                output.push_str(&line);
                output.push('\n');
            }
        }
        fixed_import = output.matches("test tests/main.vey").count() >= 2
            && output.contains("1 test(s) passed");
    }

    let _ = child.kill();
    let _ = child.wait();
    assert!(initial_failure, "the initial imported-module error was not reported:\n{errors}");
    assert!(fixed_import, "fixing the imported module did not rerun and pass tests:\n{output}");
}

#[test]
fn a_page_that_uses_a_package_of_npm_is_reported() {
    let dir = temp_dir("npm-page");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/main.vey"),
        "use \"npm:marked\" as marked\n\ncomponent App() {\n  main { h1 { \"Home\" } }\n}\n",
    )
    .unwrap();

    let output = vey().arg("build").arg(&dir).output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("only available in Node"), "{error}");
}

#[cfg(unix)]
#[test]
fn add_fails_when_npm_cannot_install_the_package() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("add-npm-error");
    let project = dir.join("project");
    let bin = dir.join("bin");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::write(project.join("package.json"), "{\n  \"name\": \"test\"\n}\n").unwrap();

    let npm = bin.join("npm");
    fs::write(&npm, "#!/bin/sh\nexit 17\n").unwrap();
    let mut permissions = fs::metadata(&npm).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&npm, permissions).unwrap();

    let output = vey()
        .arg("add")
        .arg("missing-package")
        .current_dir(&project)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("npm could not install missing-package"), "{error}");

    let missing_bin = dir.join("missing-bin");
    fs::create_dir_all(&missing_bin).unwrap();
    let output = vey()
        .arg("add")
        .arg("missing-package")
        .current_dir(&project)
        .env("PATH", &missing_bin)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("cannot start npm"), "{error}");
}

#[cfg(unix)]
#[test]
fn add_writes_a_package_name_that_already_appears_in_a_string_value() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("add-json-value");
    let project = dir.join("project");
    let bin = dir.join("bin");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        project.join("package.json"),
        r#"{"name":"app","description":"left-pad","devDependencies":{"existing":"1.0"}}"#,
    )
    .unwrap();

    let npm = bin.join("npm");
    fs::write(&npm, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&npm).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&npm, permissions).unwrap();

    let output = vey()
        .arg("add")
        .arg("left-pad")
        .current_dir(&project)
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = Command::new("node")
        .arg("-e")
        .arg(
            "const m = JSON.parse(require('node:fs').readFileSync(process.argv[1], 'utf8')); if (m.dependencies?.['left-pad'] !== 'latest' || m.description !== 'left-pad' || m.devDependencies?.existing !== '1.0') process.exit(1)",
        )
        .arg(project.join("package.json"))
        .output()
        .unwrap();
    assert!(
        manifest.status.success(),
        "{}",
        String::from_utf8_lossy(&manifest.stderr)
    );
}

#[cfg(unix)]
#[test]
fn add_rejects_invalid_package_json_without_changing_it() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("add-invalid-json");
    let project = dir.join("project");
    let bin = dir.join("bin");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let invalid = "{\n  \"name\": \"app\",\n";
    fs::write(project.join("package.json"), invalid).unwrap();

    let npm = bin.join("npm");
    fs::write(&npm, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&npm).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&npm, permissions).unwrap();

    let output = vey()
        .arg("add")
        .arg("left-pad")
        .current_dir(&project)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("package.json is not valid JSON"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(project.join("package.json")).unwrap(), invalid);
}

#[test]
fn the_site_of_the_language_builds() {
    // The site of the language is not always there, so the test is only
    // run when it is.
    if !Path::new("examples/veylang").is_dir() {
        return;
    }
    let dir = temp_dir("veylang");
    let output = vey()
        .arg("build")
        .arg("examples/veylang")
        .arg("-o")
        .arg(dir.join("dist"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for page in [
        "index.html",
        "language.html",
        "library.html",
        "install.html",
    ] {
        let path = dir.join("dist").join(page);
        assert!(path.is_file(), "{} is missing", page);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Vey"), "{}", page);
    }

    // Every page is also written under the name of its source.
    for page in ["language.vey", "library.vey", "install.vey"] {
        let path = dir.join("dist").join(page);
        assert!(path.is_file(), "{} is missing", page);
    }

    // The script that installs Vey is served with the site.
    let script = dir.join("dist").join("install.sh");
    assert!(script.is_file(), "the install script is missing");
    let text = fs::read_to_string(&script).unwrap();
    assert!(text.contains("install"), "{}", text);
}

#[test]
fn a_build_keeps_the_files_that_it_did_not_write() {
    let dir = temp_dir("build-keeps");
    let project = dir.join("project");
    write(&project.join("main.vey"), PAGE);

    // A folder that holds a file of the one who made it is not emptied.
    let output = dir.join("out");
    write(&output.join("precious.txt"), "keep me\n");
    let built = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(!built.status.success());
    let error = String::from_utf8_lossy(&built.stderr).to_string();
    assert!(error.contains("--force"), "{error}");
    assert!(output.join("precious.txt").is_file());
    assert!(!output.join("index.html").exists());

    // With the mark of the one who asks for it, the folder is emptied.
    let forced = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(&output)
        .arg("--force")
        .output()
        .unwrap();
    assert!(forced.status.success());
    assert!(output.join("index.html").is_file());
}

#[test]
fn a_build_keeps_a_file_that_is_added_to_the_folder_of_the_build() {
    let dir = temp_dir("build-keeps-two");
    let project = dir.join("project");
    write(&project.join("main.vey"), PAGE);
    let output = dir.join("out");

    // The first build writes the site.
    let first = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(first.status.success());

    // A file that is put in the folder of the build belongs to the one
    // who put it there, so it stays.
    write(&output.join("mine.txt"), "mine\n");
    let second = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(second.status.success());
    assert!(output.join("mine.txt").is_file());
    assert!(output.join("index.html").is_file());
}

/// A message of the language server, written in the shape it wants.
fn lsp_message(id: i64, method: &str, params: &str) -> String {
    let body = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"{}\",\"params\":{}}}",
        id, method, params
    );
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

#[test]
fn the_language_server_answers_the_places_of_the_names() {
    let dir = temp_dir("lsp");
    let file = dir.join("main.vey");
    let source = "fn helper() -> number {\n  return 1\n}\n\nfn main() {\n  let x = helper()\n  print(x)\n  x = x + 1\n  if true {\n    let x = 2\n    print(x)\n  }\n}\n\nfn other() {\n  let x = 3\n  print(x)\n}\nfn unicode() {\n  let 𐐀x = 1\n  print(𐐀x)\n}\n";
    write(&file, source);

    let uri = format!("file://{}", file.display());
    let mut child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        // The file is opened, and the places of the name are asked for.
        let open = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            uri,
            serde_escape(source)
        );
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
            .unwrap();
        stdin.write_all(
            lsp_message(
                1,
                "textDocument/definition",
                &format!(
                    "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":5,\"character\":10}}}}",
                    uri
                ),
            )
            .as_bytes(),
        ).unwrap();
        stdin.write_all(
            lsp_message(
                2,
                "textDocument/references",
                &format!(
                    "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":5,\"character\":10}}}}",
                    uri
                ),
            )
            .as_bytes(),
        ).unwrap();
        stdin
            .write_all(
                lsp_message(
                    3,
                    "textDocument/documentSymbol",
                    &format!("{{\"textDocument\":{{\"uri\":\"{}\"}}}}", uri),
                )
                .as_bytes(),
            )
            .unwrap();
        for (id, line, character, new_name) in [
            (4, 5, 10, "calculate"),
            (5, 6, 8, "outer"),
            (6, 9, 8, "inner"),
            (7, 15, 6, "sibling"),
            (8, 20, 9, "unicodeName"),
        ] {
            stdin.write_all(
                lsp_message(
                    id,
                    "textDocument/rename",
                    &format!(
                        "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":{},\"character\":{}}},\"newName\":\"{}\"}}",
                        uri, line, character, new_name
                    ),
                )
                .as_bytes(),
            ).unwrap();
        }
        stdin.flush().unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "LSP exited unsuccessfully");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let check = Command::new("node")
        .arg("-e")
        .arg(
            r#"
const assert = require("node:assert/strict");
const bytes = Buffer.from(process.argv[1], "utf8");
const messages = [];
let offset = 0;
while (offset < bytes.length) {
    const separator = bytes.indexOf("\r\n\r\n", offset);
    assert.notEqual(separator, -1, "missing LSP header separator");
    const headers = bytes.toString("ascii", offset, separator);
    const match = headers.match(/(?:^|\r\n)Content-Length:\s*(\d+)/i);
    assert.ok(match, `missing Content-Length in ${headers}`);
    const bodyStart = separator + 4;
    const bodyEnd = bodyStart + Number(match[1]);
    assert.ok(bodyEnd <= bytes.length, "truncated LSP message");
    messages.push(JSON.parse(bytes.subarray(bodyStart, bodyEnd).toString("utf8")));
    offset = bodyEnd;
}
function resultFor(id) {
    const matches = messages.filter(message => message.id === id);
    assert.equal(matches.length, 1, `expected exactly one response for id ${id}`);
    assert.equal(matches[0].jsonrpc, "2.0");
    assert.ok(!Object.hasOwn(matches[0], "error"), JSON.stringify(matches[0]));
    return matches[0].result;
}
const definition = resultFor(1);
assert.equal(definition.uri, process.argv[2]);
assert.equal(definition.range.start.line, 0);
const references = resultFor(2);
assert.equal(references.length, 2);
assert.deepEqual(references.map(item => item.range.start.line).sort(), [0, 5]);
assert.ok(references.every(item => item.uri === process.argv[2]));
const editFor = id => resultFor(id).changes[process.argv[2]];
assert.deepEqual(editFor(4).map(edit => [
    edit.range.start.line, edit.range.start.character,
    edit.range.end.character, edit.newText
]), [[0, 3, 9, "calculate"], [5, 10, 16, "calculate"]]);
assert.deepEqual(editFor(5).map(edit => [
    edit.range.start.line, edit.range.start.character,
    edit.range.end.character, edit.newText
]), [[5, 6, 7, "outer"], [6, 8, 9, "outer"], [7, 2, 3, "outer"], [7, 6, 7, "outer"]]);
assert.deepEqual(editFor(6).map(edit => [
    edit.range.start.line, edit.range.start.character,
    edit.range.end.character, edit.newText
]), [[9, 8, 9, "inner"], [10, 10, 11, "inner"]]);
assert.deepEqual(editFor(7).map(edit => [
    edit.range.start.line, edit.range.start.character,
    edit.range.end.character, edit.newText
]), [[15, 6, 7, "sibling"], [16, 8, 9, "sibling"]]);
assert.deepEqual(editFor(8).map(edit => [
    edit.range.start.line, edit.range.start.character,
    edit.range.end.character, edit.newText
]), [[19, 6, 9, "unicodeName"], [20, 8, 11, "unicodeName"]]);
const names = resultFor(3).map(symbol => symbol.name);
assert.ok(names.includes("helper"), JSON.stringify(names));
assert.ok(names.includes("main"), JSON.stringify(names));
"#,
        )
        .arg(&text)
        .arg(&uri)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}\nLSP output:\n{text}",
        String::from_utf8_lossy(&check.stderr)
    );
}

/// The text of a source, as it is written in a message of the server.
fn serde_escape(source: &str) -> String {
    let mut out = String::from("\"");
    for c in source.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[test]
fn the_language_server_says_the_mistakes_of_a_used_module() {
    let dir = temp_dir("lsp-module").join("café");
    let helper = dir.join("helper.vey");
    write(
        &helper,
        "export fn bad() -> number {\n  return \"text\"\n}\n",
    );
    let file = dir.join("main.vey");
    let source = "use \"./helper.vey\" as h\n\nfn main() {\n  print(h.bad())\n}\n";
    write(&file, source);

    let uri = format!("file://{}", file.display()).replace('é', "%C3%A9");
    let mut child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        let open = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            uri,
            serde_escape(source)
        );
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
            .unwrap();
        stdin.write_all(
            lsp_message(
                1,
                "textDocument/rename",
                &format!(
                    "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":3,\"character\":8}},\"newName\":\"helper\"}}",
                    uri
                ),
            )
            .as_bytes(),
        ).unwrap();
        stdin.flush().unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    // The mistake of the module that is used is said, and it is said for
    // that file.
    let reported = text
        .split("publishDiagnostics")
        .find(|message| message.contains("helper.vey"))
        .unwrap_or_else(|| panic!("the module is not named:\n{text}"));
    assert!(reported.contains("expected number, found string"), "{text}");
    let helper_uri = format!("file://{}", helper.display()).replace('é', "%C3%A9");
    assert!(
        reported.contains(&format!("\"uri\":\"{helper_uri}\"")),
        "{text}"
    );
    assert_eq!(text.matches("\"newText\":\"helper\"").count(), 2, "{text}");
    assert!(
        text.contains("\"line\":0,\"character\":22},\"end\":{\"line\":0,\"character\":23"),
        "the explicit alias range must select only `h`: {text}"
    );
}

/// The container of a new project runs what the Dockerfile builds, and the
/// continuous integration runs a test that exists.
#[test]
fn the_templates_of_a_new_project_build_what_they_run() {
    let dir = temp_dir("new-templates");

    // The page: the container serves the built site with Vey itself, so the
    // file that the container runs is not a file no build writes.
    let page = dir.join("page-app");
    let output = vey()
        .arg("new")
        .arg(&page)
        .arg("--template")
        .arg("page")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let docker = fs::read_to_string(page.join("Dockerfile")).unwrap();
    assert!(docker.contains("RUN vey build"), "{docker}");
    assert!(docker.contains("\"vey\", \"run\""), "{docker}");
    // The site is reached from outside of the container, so the server
    // listens on every address and not on the machine itself.
    assert!(docker.contains("\"0.0.0.0\""), "{docker}");
    assert!(!docker.contains("server.cjs"), "{docker}");

    // The server: the file the container runs is the file that the Dockerfile
    // builds, and it is built where nothing else is written.
    let server = dir.join("server-app");
    let output = vey()
        .arg("new")
        .arg(&server)
        .arg("--template")
        .arg("server")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let docker = fs::read_to_string(server.join("Dockerfile")).unwrap();
    assert!(docker.contains("RUN vey build --node -o server.cjs"), "{docker}");
    assert!(docker.contains("\"server.cjs\""), "{docker}");
    assert!(!docker.contains(".dist/server.cjs"), "{docker}");
    // The server starts when the program runs, so the container answers.
    let source = fs::read_to_string(server.join("src/main.vey")).unwrap();
    assert!(source.contains("app.start()"), "{source}");

    for project in [&page, &server] {
        for command in ["check", "test"] {
            let output = vey()
                .arg(command)
                .current_dir(project)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "vey {command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // The library: it has no page to serve and no server to run, so it has no
    // container, and its continuous integration names the file it has.
    let lib = dir.join("lib-app");
    let output = vey()
        .arg("new")
        .arg(&lib)
        .arg("--template")
        .arg("lib")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!lib.join("Dockerfile").exists());
    let workflow = fs::read_to_string(lib.join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("- run: vey check src/lib.vey"), "{workflow}");
    for command in ["check", "test"] {
        let output = if command == "check" {
            vey()
                .arg("check")
                .arg("src/lib.vey")
                .current_dir(&lib)
                .output()
                .unwrap()
        } else {
            vey().arg(command).current_dir(&lib).output().unwrap()
        };
        assert!(
            output.status.success(),
            "vey {command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
