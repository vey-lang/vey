//! Tests for the command line tools.

mod common;

use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{stderr, temp_dir};

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
fn a_build_rejects_unsupported_minification_in_site_and_node_modes() {
    let dir = temp_dir("build-minify-unsupported");
    let project = dir.join("project");
    let entry = project.join("main.vey");
    write(&entry, PAGE);
    write(&project.join("vey.toml"), "[build]\nminify = true\n");

    let site_output = dir.join("site");
    let site = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(&site_output)
        .output()
        .unwrap();
    assert_eq!(site.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&site.stderr).contains("[build].minify = true is not supported"),
        "{}",
        String::from_utf8_lossy(&site.stderr)
    );
    assert!(!site_output.exists());

    let node_output = dir.join("server.cjs");
    let node = vey()
        .arg("build")
        .arg(&entry)
        .arg("--node")
        .arg("-o")
        .arg(&node_output)
        .output()
        .unwrap();
    assert_eq!(node.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&node.stderr).contains("[build].minify = true is not supported"),
        "{}",
        String::from_utf8_lossy(&node.stderr)
    );
    assert!(!node_output.exists());

    let target = dir.join("host");
    let deploy = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert_eq!(deploy.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&deploy.stderr)
            .contains("[build].minify = true is not supported"),
        "{}",
        String::from_utf8_lossy(&deploy.stderr)
    );
    assert!(!target.exists());
    assert!(!project.join(".dist").exists());

    write(&project.join("vey.toml"), "[build]\nminify = false\n");
    let default = vey()
        .arg("build")
        .arg(&project)
        .arg("-o")
        .arg(&site_output)
        .output()
        .unwrap();
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    assert!(site_output.join("index.html").is_file());
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
fn an_unknown_new_project_template_does_not_create_the_target() {
    let dir = temp_dir("new-project-invalid-template");
    let project = dir.join("app");
    let output = vey()
        .arg("new")
        .arg(&project)
        .arg("--template")
        .arg("unknown")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("there is no template named 'unknown'"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!project.exists(), "invalid input created the target directory");
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
fn lint_reads_the_settings_of_the_project_that_holds_the_file() {
    let dir = temp_dir("lint-config-root");
    let project = dir.join("project");
    let other = dir.join("elsewhere");
    write(
        &project.join("vey.toml"),
        "[lint]\nmaxLine = 20\n",
    );
    let file = project.join("src/main.vey");
    write(
        &file,
        "fn main() {\n  let a_rather_long_name = 1\n  print(a_rather_long_name)\n}\n",
    );
    fs::create_dir_all(&other).unwrap();

    // The command is run away from the project, so the settings can only be
    // found from the file that is linted.
    let output = vey()
        .current_dir(&other)
        .arg("lint")
        .arg(&file)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success(), "{text}");
    assert!(text.contains("characters long"), "{text}");
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
    let output = vey().arg("build").arg("--title").output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(error.contains("needs a value"), "{error}");
}

#[test]
fn an_option_of_another_command_is_reported() {
    for (command, option) in [
        ("build", "--port"),
        ("lint", "--write"),
        ("check", "--node"),
        ("run", "--target"),
        ("test", "--force"),
    ] {
        let output = vey()
            .arg(command)
            .arg(option)
            .arg("3000")
            .output()
            .unwrap();
        assert!(!output.status.success(), "vey {command} accepted {option}");
        let error = String::from_utf8_lossy(&output.stderr).to_string();
        assert!(
            error.contains(&format!("unknown option '{}'", option)),
            "{error}"
        );
        assert!(error.contains(&format!("vey {command}")), "{error}");
    }
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
fn migrate_dry_run_reports_changes_without_writing() {
    let dir = temp_dir("migrate-dry-run-changes");
    let file = dir.join("main.vey");
    let source = "// vey: 0.6.0\nfn main() {\n  print(1)\n}\n";
    write(&file, source);

    let output = vey().arg("migrate").arg(&file).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains(&format!("// vey: {}", env!("CARGO_PKG_VERSION"))),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("1 file(s) are not of this version."),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(file).unwrap(), source);
}

#[test]
fn migrate_dry_run_succeeds_when_nothing_needs_migration() {
    let dir = temp_dir("migrate-dry-run-current");
    let file = dir.join("main.vey");
    let source = format!(
        "// vey: {}\nfn main() {{\n  print(1)\n}}\n",
        env!("CARGO_PKG_VERSION")
    );
    write(&file, &source);

    let output = vey().arg("migrate").arg(&file).output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(file).unwrap(), source);
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

#[test]
fn deploy_removes_only_stale_vey_files_and_keeps_host_files() {
    let dir = temp_dir("deploy-stale");
    let project = dir.join("project");
    let target = dir.join("host");
    write(&project.join("main.vey"), PAGE);
    write(
        &project.join("about.vey"),
        "component About() { main { h1 { \"About\" } } }\nbind(\"#app\", About())\n",
    );

    let first = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(target.join("about.html").is_file());
    assert!(target.join("about.js").is_file());

    write(&target.join("host-owned.txt"), "keep at the host\n");
    write(&target.join("assets/host-owned.txt"), "also keep this\n");
    fs::remove_file(project.join("about.vey")).unwrap();
    write(&project.join("main.vey"), &PAGE.replace("Hi", "Updated"));

    let second = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert!(second.status.success(), "{}", stderr(&second));
    assert!(!target.join("about.html").exists());
    assert!(!target.join("about.js").exists());
    assert!(!target.join("about.vey").exists());
    assert!(target.join("host-owned.txt").is_file());
    assert!(target.join("assets/host-owned.txt").is_file());
    assert!(fs::read_to_string(target.join("index.js"))
        .unwrap()
        .contains("Updated"));
}

#[test]
fn deploy_refuses_to_overwrite_an_unmanaged_target_file() {
    let dir = temp_dir("deploy-collision");
    let project = dir.join("project");
    let target = dir.join("host");
    write(&project.join("main.vey"), PAGE);
    write(&target.join("index.html"), "host-owned index\n");

    let output = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(target.join("index.html")).unwrap(),
        "host-owned index\n"
    );
}

#[test]
fn deploy_rejects_a_target_inside_the_build_output() {
    let dir = temp_dir("deploy-overlap");
    let project = dir.join("project");
    let output_dir = dir.join("built");
    let target = output_dir.join("host");
    write(&project.join("main.vey"), PAGE);

    let output = vey()
        .arg("deploy")
        .arg(&project)
        .arg("-o")
        .arg(&output_dir)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!target.exists());
    assert!(output_dir.join("index.html").is_file());
}

#[cfg(unix)]
#[test]
fn deploy_replaces_managed_files_without_mutating_hard_link_aliases() {
    let dir = temp_dir("deploy-hardlink");
    let project = dir.join("project");
    let target = dir.join("host");
    let protected = dir.join("protected.txt");
    write(&project.join("main.vey"), PAGE);

    let first = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert!(first.status.success(), "{}", stderr(&first));

    write(&protected, "external data\n");
    fs::remove_file(target.join("index.js")).unwrap();
    fs::hard_link(&protected, target.join("index.js")).unwrap();
    write(&project.join("main.vey"), &PAGE.replace("Hi", "Updated"));

    let second = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert!(second.status.success(), "{}", stderr(&second));
    assert_eq!(fs::read_to_string(protected).unwrap(), "external data\n");
    assert!(fs::read_to_string(target.join("index.js"))
        .unwrap()
        .contains("Updated"));
}

#[test]
fn deploy_rejects_a_manifest_path_that_escapes_the_target() {
    let dir = temp_dir("deploy-manifest-traversal");
    let project = dir.join("project");
    let target = dir.join("host");
    write(&project.join("main.vey"), PAGE);
    write(&dir.join("precious.txt"), "must remain\n");
    write(
        &target.join(".vey-deploy-manifest"),
        "VEY-DEPLOY-MANIFEST-1\n2e2e2f70726563696f75732e747874\n",
    );

    let output = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(dir.join("precious.txt")).unwrap(),
        "must remain\n"
    );
    assert!(fs::read_to_string(target.join(".vey-deploy-manifest"))
        .unwrap()
        .contains("2e2e2f"));
}

#[cfg(unix)]
#[test]
fn deploy_does_not_follow_a_target_symlink_parent() {
    use std::os::unix::fs::symlink;

    let dir = temp_dir("deploy-symlink");
    let project = dir.join("project");
    let target = dir.join("host");
    let outside = dir.join("outside");
    write(&project.join("main.vey"), PAGE);
    write(&project.join("public/assets/app.svg"), "<svg>site</svg>");
    write(&outside.join("app.svg"), "host-owned asset");
    fs::create_dir_all(&target).unwrap();
    symlink(&outside, target.join("assets")).unwrap();

    let output = vey()
        .arg("deploy")
        .arg(&project)
        .arg("--target")
        .arg(&target)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(outside.join("app.svg")).unwrap(),
        "host-owned asset"
    );
}

/// A message of the language server, written in the shape it wants.
fn lsp_message(id: i64, method: &str, params: &str) -> String {
    let body = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"{}\",\"params\":{}}}",
        id, method, params
    );
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn lsp_notification(method: &str, params: &str) -> String {
    let body = format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"{}\",\"params\":{}}}",
        method, params
    );
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn wait_for_lsp_exit(child: &mut std::process::Child, expected_success: bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                assert_eq!(status.success(), expected_success, "LSP status: {status}");
                return;
            }
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the LSP did not exit while its input stream remained open");
            }
        }
    }
}

#[test]
fn the_language_server_terminates_on_exit_notification() {
    let mut child = vey()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(lsp_message(1, "initialize", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_message(2, "shutdown", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_notification("exit", "{}").as_bytes())
            .unwrap();
        stdin.flush().unwrap();
    }
    // Keep stdin open: exit, not EOF, must terminate the process.
    wait_for_lsp_exit(&mut child, true);

    let mut child = vey()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(lsp_notification("exit", "{}").as_bytes())
        .unwrap();
    child.stdin.as_mut().unwrap().flush().unwrap();
    wait_for_lsp_exit(&mut child, false);
}

#[test]
fn the_language_server_returns_parse_error_for_invalid_json() {
    let body = "{]";
    let mut child = vey()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
        .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text.contains("\"error\":{\"code\":-32700,\"message\":\"Parse error\"}"),
        "{text}"
    );
}

#[test]
fn the_language_server_returns_parse_error_for_an_incomplete_frame() {
    let mut child = vey()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"Content-Length: 8\r\n\r\n{}")
        .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text.contains("\"error\":{\"code\":-32700,\"message\":\"Parse error\"}"),
        "{text}"
    );
}

#[test]
fn the_language_server_reports_unknown_requests_and_ignores_unknown_notifications() {
    let mut child = vey()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(lsp_message(1, "initialize", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_message(2, "vey/notAMethod", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_notification("vey/notAMethod", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_message(3, "shutdown", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_notification("exit", "{}").as_bytes())
            .unwrap();
        stdin.flush().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    assert_eq!(
        text.matches("Method not found: vey/notAMethod").count(),
        1,
        "{text}"
    );
    assert!(
        text.contains(
            "\"id\":2,\"error\":{\"code\":-32601,\"message\":\"Method not found: vey/notAMethod\"}"
        ),
        "{text}"
    );
}

#[test]
fn the_language_server_answers_requests_with_invalid_params() {
    let dir = temp_dir("lsp-invalid-params");
    let file = dir.join("main.vey");
    let source = "let value = 1\n";
    write(&file, source);
    let uri = format!("file://{}", file.display());
    let mut child = vey()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(lsp_message(1, "initialize", "{}").as_bytes())
            .unwrap();
        let open = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            uri,
            serde_escape(source)
        );
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_message(2, "textDocument/hover", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_message(3, "textDocument/documentSymbol", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(
                lsp_message(
                    4,
                    "textDocument/rename",
                    &format!(
                        "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":0,\"character\":4}}}}",
                        uri
                    ),
                )
                .as_bytes(),
            )
            .unwrap();
        stdin
            .write_all(lsp_message(5, "shutdown", "{}").as_bytes())
            .unwrap();
        stdin
            .write_all(lsp_notification("exit", "{}").as_bytes())
            .unwrap();
        stdin.flush().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    for id in [2, 3, 4] {
        assert!(
            text.contains(&format!(
                "\"id\":{},\"error\":{{\"code\":-32602,\"message\":\"Invalid params\"}}",
                id
            )),
            "missing correlated invalid-params response for id {id}:\n{text}"
        );
    }
    assert_eq!(text.matches("\"code\":-32602").count(), 3, "{text}");
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
fn completion_offers_the_names_that_are_in_scope_at_the_cursor() {
    let dir = temp_dir("lsp-completion");
    let file = dir.join("main.vey");
    let source = r#"fn outer(first: number) {
  let outer_name = first
  if first > 0 {
    let inner_name = outer_name
    print(inner_name)
  }
  let sibling = 1
  print(sibling)
}

fn other(second: number) {
  let other_name = second
  print(other_name)
}

fn main() {
  let top = 1
  print(top)
}
"#;
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
        let open = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            uri,
            serde_escape(source)
        );
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
            .unwrap();
        for (id, line, character) in [(1, 4, 10), (2, 7, 8), (3, 12, 10)] {
            stdin
                .write_all(
                    lsp_message(
                        id,
                        "textDocument/completion",
                        &format!(
                            "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":{},\"character\":{}}}}}",
                            uri, line, character
                        ),
                    )
                    .as_bytes(),
                )
                .unwrap();
        }
        stdin.flush().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();

    let result = |id: i64| {
        let marker = format!("\"id\":{},\"result\":[", id);
        let start = text.find(&marker).unwrap_or_else(|| panic!("no answer {id}:\n{text}")) + marker.len();
        let end = text[start..].find(']').map(|end| start + end).unwrap();
        &text[start..end]
    };
    let inner = result(1);
    for name in ["inner_name", "outer_name", "first", "outer", "main", "len"] {
        assert!(inner.contains(&format!("\"label\":\"{}\"", name)), "{name} missing:\n{inner}");
    }
    for name in ["sibling", "other_name", "second", "top"] {
        assert!(!inner.contains(&format!("\"label\":\"{}\"", name)), "{name} leaked:\n{inner}");
    }

    let later = result(2);
    assert!(later.contains("\"label\":\"sibling\""), "{later}");
    assert!(!later.contains("\"label\":\"inner_name\""), "{later}");

    let other = result(3);
    for name in ["other_name", "second", "other", "main"] {
        assert!(other.contains(&format!("\"label\":\"{}\"", name)), "{name} missing:\n{other}");
    }
    for name in ["outer_name", "inner_name", "sibling", "top"] {
        assert!(!other.contains(&format!("\"label\":\"{}\"", name)), "{name} leaked:\n{other}");
    }
}

#[test]
fn completion_offers_the_bindings_of_arrow_and_markup_scopes() {
    let dir = temp_dir("lsp-completion-scopes");
    let file = dir.join("main.vey");
    let source = r#"fn main() {
  let double = (amount: number) => amount * 2
  print(double(2))
}

fn label(value: number) -> number {
  match value {
    0 => 1
    n => n + 1
  }
}

component App() {
  let items = ["one", "two"]
  main {
    ul {
      for item in items {
        li { item }
      }
    }
  }
}
"#;
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
        let open = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            uri,
            serde_escape(source)
        );
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
            .unwrap();
        for (id, line, character) in [(1, 1, 37), (2, 8, 9), (3, 17, 14)] {
            stdin
                .write_all(
                    lsp_message(
                        id,
                        "textDocument/completion",
                        &format!(
                            "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":{},\"character\":{}}}}}",
                            uri, line, character
                        ),
                    )
                    .as_bytes(),
                )
                .unwrap();
        }
        stdin.flush().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();

    let result = |id: i64| {
        let marker = format!("\"id\":{},\"result\":[", id);
        let start = text
            .find(&marker)
            .unwrap_or_else(|| panic!("no answer {id}:\n{text}"))
            + marker.len();
        let end = text[start..].find(']').map(|end| start + end).unwrap();
        &text[start..end]
    };
    let has = |answer: &str, name: &str| answer.contains(&format!("\"label\":\"{}\"", name));

    let arrow = result(1);
    for name in ["amount", "double", "main"] {
        assert!(has(arrow, name), "{name} missing:\n{arrow}");
    }
    for name in ["item", "items", "n"] {
        assert!(!has(arrow, name), "{name} leaked:\n{arrow}");
    }

    let arm = result(2);
    for name in ["n", "value", "label", "main"] {
        assert!(has(arm, name), "{name} missing:\n{arm}");
    }
    for name in ["amount", "item", "items"] {
        assert!(!has(arm, name), "{name} leaked:\n{arm}");
    }

    let loop_body = result(3);
    for name in ["item", "items", "App", "main"] {
        assert!(has(loop_body, name), "{name} missing:\n{loop_body}");
    }
    for name in ["amount", "n", "value"] {
        assert!(!has(loop_body, name), "{name} leaked:\n{loop_body}");
    }
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
    // The container builds the program the way `npm run build` does, and
    // runs the file that build writes.
    assert!(docker.contains("RUN vey build"), "{docker}");
    assert!(docker.contains(".dist/server.cjs"), "{docker}");
    // The server of a container is reached from outside of it.
    assert!(docker.contains("HOST=0.0.0.0"), "{docker}");
    // The server starts when the program runs, so the container answers,
    // and it listens where the settings of the machine say.
    let source = fs::read_to_string(server.join("src/main.vey")).unwrap();
    assert!(source.contains("app.start()"), "{source}");
    assert!(source.contains("env(\"PORT\""), "{source}");
    assert!(source.contains("server(port"), "{source}");
    // A project says what it is instead of leaving it to a guess.
    let config = fs::read_to_string(server.join("vey.toml")).unwrap();
    assert!(config.contains("kind = \"server\""), "{config}");
    let page_config = fs::read_to_string(page.join("vey.toml")).unwrap();
    assert!(page_config.contains("kind = \"page\""), "{page_config}");
    // The development command of either project is the same command.
    for project in [&page, &server] {
        let package = fs::read_to_string(project.join("package.json")).unwrap();
        assert!(package.contains("\"dev\": \"vey dev\""), "{package}");
    }
    let server_package = fs::read_to_string(server.join("package.json")).unwrap();
    assert!(server_package.contains("\"start\": \"vey run\""), "{server_package}");
    assert!(server_package.contains("\"build\": \"vey build\""), "{server_package}");

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

    // The build of either project writes what its container runs.
    let output = vey().arg("build").current_dir(&page).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(page.join(".dist/index.html").is_file());
    let output = vey().arg("build").current_dir(&server).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let built = fs::read_to_string(server.join(".dist/server.cjs")).unwrap();
    assert!(built.contains("createVeyRuntime"), "the program is not built");

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
    // A library has nothing to serve, and `vey dev` says so instead of
    // building a page that is not there.
    let output = vey()
        .arg("dev")
        .arg("src/lib.vey")
        .current_dir(&lib)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("library"), "{message}");
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

#[test]
fn the_language_server_resolves_cross_file_members_and_local_type_symbols() {
    let dir = temp_dir("lsp-cross-file");
    let math = dir.join("math.vey");
    let main = dir.join("main.vey");
    let other = dir.join("other.vey");
    let other_main = dir.join("other-main.vey");
    let math_source =
        "export fn area() { return 1 }\nexport component Card(title: string) { h2 { title } }\nexport type Box<T> = { value: T }\nexport enum Color { Red, Blue }\nlet primary = Color.Red\nenum Shape { Red, Blue }\nlet shape = Shape.Red\n";
    let main_source = "use \"./math.vey\" as m\nprint(\"😀\", m.area(), m[\"area\"](), m[\"ar\\x65a\"]())\nfn decoy(m) { print(m.area(), m[\"area\"]) }\nlet item: m.Box<number> = { value: 1 }\ncomponent App() {\n  m.Card title=\"Card\"\n}\nlet color: m.Color = m.Color.Red\ntype Box<T> = { value: T }\nlet Box = 2\nlet localBox: Box<number> = { value: 2 }\nprint(Box)\nlet matched = match color { m.Color.Red => 1, _ => 0 }\n";
    let other_source =
        "export fn area() { return 2 }\nexport component Card(title: string) { h2 { title } }\nexport type Box<T> = { value: T }\nexport enum Color { Red, Blue }\nlet primary = Color.Red\nenum Shape { Red, Blue }\nlet shape = Shape.Red\n";
    let other_main_source = "use \"./other.vey\" as m\nlet otherItem: m.Box<number> = { value: 2 }\nprint(m.area())\ncomponent Another() {\n  m.Card title=\"Other\"\n}\nlet otherColor: m.Color = m.Color.Red\nlet otherVariant = m.Color.Red\n";
    write(&math, math_source);
    write(&main, main_source);
    write(&other, other_source);
    write(&other_main, other_main_source);
    let math_uri = format!("file://{}", math.display());
    let main_uri = format!("file://{}", main.display());
    let other_uri = format!("file://{}", other.display());
    let other_main_uri = format!("file://{}", other_main.display());

    let mut child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for (uri, source) in [
            (&math_uri, math_source),
            (&main_uri, main_source),
            (&other_uri, other_source),
            (&other_main_uri, other_main_source),
        ] {
            let open = format!(
                "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
                uri,
                serde_escape(source)
            );
            stdin
                .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
                .unwrap();
        }
        for (id, method, uri, line, character, extra) in [
            (1, "textDocument/definition", &main_uri, 1, 14, ""),
            (
                2,
                "textDocument/references",
                &math_uri,
                0,
                13,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                3,
                "textDocument/rename",
                &math_uri,
                0,
                13,
                ",\"newName\":\"measure\"",
            ),
            (
                4,
                "textDocument/references",
                &main_uri,
                1,
                14,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                5,
                "textDocument/rename",
                &main_uri,
                1,
                14,
                ",\"newName\":\"measure\"",
            ),
            (6, "textDocument/definition", &main_uri, 5, 4, ""),
            (
                7,
                "textDocument/references",
                &math_uri,
                1,
                17,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                8,
                "textDocument/rename",
                &math_uri,
                1,
                17,
                ",\"newName\":\"Panel\"",
            ),
            (
                9,
                "textDocument/references",
                &main_uri,
                5,
                4,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                10,
                "textDocument/rename",
                &main_uri,
                5,
                4,
                ",\"newName\":\"Panel\"",
            ),
            (11, "textDocument/definition", &main_uri, 3, 12, ""),
            (
                12,
                "textDocument/references",
                &math_uri,
                2,
                12,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                13,
                "textDocument/rename",
                &math_uri,
                2,
                12,
                ",\"newName\":\"Container\"",
            ),
            (
                14,
                "textDocument/references",
                &main_uri,
                3,
                12,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                15,
                "textDocument/rename",
                &main_uri,
                3,
                12,
                ",\"newName\":\"Container\"",
            ),
            (16, "textDocument/definition", &main_uri, 7, 13, ""),
            (
                17,
                "textDocument/references",
                &math_uri,
                3,
                12,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                18,
                "textDocument/rename",
                &math_uri,
                3,
                12,
                ",\"newName\":\"Hue\"",
            ),
            (
                19,
                "textDocument/references",
                &main_uri,
                7,
                13,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                20,
                "textDocument/rename",
                &main_uri,
                7,
                13,
                ",\"newName\":\"Hue\"",
            ),
            (21, "textDocument/definition", &main_uri, 10, 14, ""),
            (
                22,
                "textDocument/references",
                &main_uri,
                8,
                5,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                23,
                "textDocument/rename",
                &main_uri,
                8,
                5,
                ",\"newName\":\"LocalBox\"",
            ),
            (
                24,
                "textDocument/references",
                &main_uri,
                10,
                14,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                25,
                "textDocument/rename",
                &main_uri,
                10,
                14,
                ",\"newName\":\"LocalBox\"",
            ),
            (26, "textDocument/definition", &main_uri, 8, 23, ""),
            (
                27,
                "textDocument/references",
                &main_uri,
                8,
                9,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                28,
                "textDocument/rename",
                &main_uri,
                8,
                23,
                ",\"newName\":\"Value\"",
            ),
            (
                29,
                "textDocument/definition",
                &main_uri,
                9,
                4,
                "",
            ),
            (
                30,
                "textDocument/references",
                &main_uri,
                9,
                4,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                31,
                "textDocument/rename",
                &main_uri,
                9,
                4,
                ",\"newName\":\"Amount\"",
            ),
            (32, "textDocument/definition", &main_uri, 7, 29, ""),
            (
                33,
                "textDocument/references",
                &math_uri,
                3,
                20,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                34,
                "textDocument/rename",
                &math_uri,
                3,
                20,
                ",\"newName\":\"Crimson\"",
            ),
            (
                35,
                "textDocument/references",
                &main_uri,
                7,
                29,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                36,
                "textDocument/rename",
                &main_uri,
                7,
                29,
                ",\"newName\":\"Crimson\"",
            ),
            (37, "textDocument/definition", &main_uri, 12, 36, ""),
            (
                38,
                "textDocument/references",
                &main_uri,
                12,
                36,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                39,
                "textDocument/rename",
                &main_uri,
                12,
                36,
                ",\"newName\":\"Crimson\"",
            ),
            (40, "textDocument/definition", &main_uri, 1, 25, ""),
            (
                41,
                "textDocument/references",
                &main_uri,
                1,
                25,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                42,
                "textDocument/rename",
                &main_uri,
                1,
                25,
                ",\"newName\":\"measure\"",
            ),
            (43, "textDocument/definition", &main_uri, 1, 38, ""),
            (
                44,
                "textDocument/references",
                &main_uri,
                1,
                38,
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (
                45,
                "textDocument/rename",
                &main_uri,
                1,
                38,
                ",\"newName\":\"measure\"",
            ),
        ] {
            let params = format!(
                "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":{},\"character\":{}}}{}}}",
                uri, line, character, extra
            );
            stdin
                .write_all(lsp_message(id, method, &params).as_bytes())
                .unwrap();
        }
        stdin.flush().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
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
    assert.notEqual(separator, -1);
    const headers = bytes.toString("ascii", offset, separator);
    const length = Number(headers.match(/Content-Length:\s*(\d+)/i)[1]);
    const start = separator + 4;
    messages.push(JSON.parse(bytes.subarray(start, start + length).toString("utf8")));
    offset = start + length;
}
const result = id => {
    const response = messages.find(message => message.id === id);
    assert.ok(response && !response.error, JSON.stringify(response));
    return response.result;
};
const mathUri = process.argv[2];
const mainUri = process.argv[3];
const definition = result(1);
assert.equal(definition.uri, mathUri);
assert.deepEqual([definition.range.start.line, definition.range.start.character], [0, 10]);
for (const id of [2, 4]) {
    const references = result(id);
    assert.equal(references.length, 4, JSON.stringify(references));
    assert.deepEqual(references.map(item => item.uri).sort(), [mainUri, mainUri, mainUri, mathUri].sort());
    assert.deepEqual(references.filter(item => item.uri === mainUri).map(item => item.range.start), [
        { line: 1, character: 14 },
        { line: 1, character: 25 },
        { line: 1, character: 38 },
    ]);
}
for (const id of [3, 5]) {
    const changes = result(id).changes;
    assert.deepEqual(Object.keys(changes).sort(), [mainUri, mathUri].sort());
    assert.deepEqual(changes[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[0, 10, 14, "measure"]]);
    assert.deepEqual(changes[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[1, 14, 18, "measure"], [1, 25, 29, "measure"], [1, 38, 45, "measure"]]);
}
const computedDefinition = result(40);
assert.equal(computedDefinition.uri, mathUri);
assert.deepEqual([computedDefinition.range.start.line, computedDefinition.range.start.character], [0, 10]);
assert.equal(result(41).length, 4);
const computedRename = result(42).changes;
assert.deepEqual(computedRename[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[1, 14, 18, "measure"], [1, 25, 29, "measure"], [1, 38, 45, "measure"]]);
const escapedDefinition = result(43);
assert.equal(escapedDefinition.uri, mathUri);
const escapedReferences = result(44);
assert.equal(escapedReferences.length, 4);
const escapedRename = result(45).changes;
assert.deepEqual(escapedRename[mainUri].map(edit => [edit.range.start.character, edit.range.end.character, edit.newText]), [[14, 18, "measure"], [25, 29, "measure"], [38, 45, "measure"]]);
const componentDefinition = result(6);
assert.equal(componentDefinition.uri, mathUri);
assert.deepEqual([componentDefinition.range.start.line, componentDefinition.range.start.character], [1, 17]);
for (const id of [7, 9]) {
    const references = result(id);
    assert.equal(references.length, 2, JSON.stringify(references));
    assert.deepEqual(references.map(item => item.uri).sort(), [mainUri, mathUri].sort());
    assert.deepEqual(references.find(item => item.uri === mainUri).range.start, { line: 5, character: 4 });
}
for (const id of [8, 10]) {
    const changes = result(id).changes;
    assert.deepEqual(Object.keys(changes).sort(), [mainUri, mathUri].sort());
    assert.deepEqual(changes[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[1, 17, 21, "Panel"]]);
    assert.deepEqual(changes[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[5, 4, 8, "Panel"]]);
}
const typeDefinition = result(11);
assert.equal(typeDefinition.uri, mathUri);
assert.deepEqual([typeDefinition.range.start.line, typeDefinition.range.start.character], [2, 12]);
for (const id of [12, 14]) {
    const references = result(id);
    assert.equal(references.length, 2, JSON.stringify(references));
    assert.deepEqual(references.map(item => item.uri).sort(), [mainUri, mathUri].sort());
    assert.deepEqual(references.find(item => item.uri === mainUri).range.start, { line: 3, character: 12 });
}
for (const id of [13, 15]) {
    const changes = result(id).changes;
    assert.deepEqual(Object.keys(changes).sort(), [mainUri, mathUri].sort());
    assert.deepEqual(changes[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[2, 12, 15, "Container"]]);
    assert.deepEqual(changes[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[3, 12, 15, "Container"]]);
}
const enumDefinition = result(16);
assert.equal(enumDefinition.uri, mathUri);
assert.deepEqual([enumDefinition.range.start.line, enumDefinition.range.start.character], [3, 12]);
for (const id of [17, 19]) {
    const references = result(id);
    assert.equal(references.length, 5, JSON.stringify(references));
    assert.deepEqual(references.map(item => item.uri).sort(), [mainUri, mainUri, mainUri, mathUri, mathUri].sort());
    assert.deepEqual(references.filter(item => item.uri === mainUri).map(item => item.range.start), [
        { line: 7, character: 13 },
        { line: 7, character: 23 },
        { line: 12, character: 30 },
    ]);
}
for (const id of [18, 20]) {
    const changes = result(id).changes;
    assert.deepEqual(Object.keys(changes).sort(), [mainUri, mathUri].sort());
    assert.deepEqual(changes[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[3, 12, 17, "Hue"], [4, 14, 19, "Hue"]]);
    assert.deepEqual(changes[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[7, 13, 18, "Hue"], [7, 23, 28, "Hue"], [12, 30, 35, "Hue"]]);
}
const localTypeDefinition = result(21);
assert.equal(localTypeDefinition.uri, mainUri);
assert.deepEqual([localTypeDefinition.range.start.line, localTypeDefinition.range.start.character], [8, 5]);
for (const id of [22, 24]) {
    const references = result(id);
    assert.equal(references.length, 2, JSON.stringify(references));
    assert.deepEqual(references.map(item => item.range.start), [
        { line: 8, character: 5 },
        { line: 10, character: 14 },
    ]);
}
for (const id of [23, 25]) {
    const changes = result(id).changes;
    assert.deepEqual(Object.keys(changes), [mainUri]);
    assert.deepEqual(changes[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [
        [8, 5, 8, "LocalBox"],
        [10, 14, 17, "LocalBox"],
    ]);
}
const typeParameterDefinition = result(26);
assert.equal(typeParameterDefinition.uri, mainUri);
assert.deepEqual([typeParameterDefinition.range.start.line, typeParameterDefinition.range.start.character], [8, 9]);
const typeParameterReferences = result(27);
assert.equal(typeParameterReferences.length, 2);
assert.deepEqual(typeParameterReferences.map(item => item.range.start), [
    { line: 8, character: 9 },
    { line: 8, character: 23 },
]);
const typeParameterRename = result(28).changes;
assert.deepEqual(typeParameterRename[mainUri].map(edit => [edit.range.start.character, edit.range.end.character, edit.newText]), [
    [9, 10, "Value"],
    [23, 24, "Value"],
]);
const valueDefinition = result(29);
assert.deepEqual([valueDefinition.range.start.line, valueDefinition.range.start.character], [9, 4]);
const valueReferences = result(30);
assert.equal(valueReferences.length, 2);
assert.deepEqual(valueReferences.map(item => item.range.start), [
    { line: 9, character: 4 },
    { line: 11, character: 6 },
]);
const valueRename = result(31).changes;
assert.deepEqual(valueRename[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [
    [9, 4, 7, "Amount"],
    [11, 6, 9, "Amount"],
]);
const variantDefinition = result(32);
assert.equal(variantDefinition.uri, mathUri);
assert.deepEqual([variantDefinition.range.start.line, variantDefinition.range.start.character], [3, 20]);
const expectedVariantReferences = [
    [mathUri, 3, 20],
    [mathUri, 4, 20],
    [mainUri, 7, 29],
    [mainUri, 12, 36],
].sort((a, b) => a[0].localeCompare(b[0]) || a[1] - b[1] || a[2] - b[2]);
for (const id of [33, 35, 38]) {
    const references = result(id);
    assert.equal(references.length, 4, JSON.stringify(references));
    const actual = references.map(item => [item.uri, item.range.start.line, item.range.start.character])
        .sort((a, b) => a[0].localeCompare(b[0]) || a[1] - b[1] || a[2] - b[2]);
    assert.deepEqual(actual, expectedVariantReferences);
}
for (const id of [34, 36, 39]) {
    const changes = result(id).changes;
    assert.deepEqual(Object.keys(changes).sort(), [mainUri, mathUri].sort());
    const edits = uri => changes[uri]
        .map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText])
        .sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    assert.deepEqual(edits(mathUri), [[3, 20, 23, "Crimson"], [4, 20, 23, "Crimson"]]);
    assert.deepEqual(edits(mainUri), [[7, 29, 32, "Crimson"], [12, 36, 39, "Crimson"]]);
}
"#,
        )
        .arg(&text)
        .arg(&math_uri)
        .arg(&main_uri)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}\nLSP output:\n{text}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn the_language_server_navigates_to_unopened_imported_modules() {
    let dir = temp_dir("lsp-unopened-import");
    let math = dir.join("math.vey");
    let main = dir.join("main.vey");
    let other = dir.join("other.vey");
    let added_root = dir.with_file_name(format!(
        "{}-added",
        dir.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&added_root).unwrap();
    let replacement = added_root.join("replacement.vey");
    let math_source = "export type area = { value: number }\nfn helper() {\n  let area = 9\n  print(area)\n}\nexport fn area() -> number { return 1 }\n";
    let main_source = "use \"./math.vey\" as m\nlet typed: m.area = { value: 1 }\nm.area()\n";
    let other_source = "use \"./math.vey\" as x\nx.area()\n";
    let replacement_source = format!(
        "use \"../{}/math.vey\" as n\nn.area()\n",
        dir.file_name().unwrap().to_string_lossy()
    );
    write(&math, math_source);
    write(&main, main_source);
    write(&other, other_source);
    write(&replacement, &replacement_source);
    let math_uri = format!("file://{}", math.display());
    let main_uri = format!("file://{}", main.display());
    let other_uri = format!("file://{}", other.display());
    let added_root_uri = format!("file://{}", added_root.display());
    let replacement_uri = format!("file://{}", replacement.display());

    let mut child = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        let root_uri = format!("file://{}", dir.display());
        let initialize = format!(
            "{{\"rootUri\":{},\"capabilities\":{{}}}}",
            serde_escape(&root_uri)
        );
        stdin
            .write_all(lsp_message(0, "initialize", &initialize).as_bytes())
            .unwrap();
        let open = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            main_uri,
            serde_escape(main_source)
        );
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n{}", open.len(), open).as_bytes())
            .unwrap();
        for (id, method, extra) in [
            (1, "textDocument/definition", ""),
            (
                2,
                "textDocument/references",
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (3, "textDocument/rename", ",\"newName\":\"measure\""),
        ] {
            let params = format!(
                "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":2,\"character\":2}}{}}}",
                main_uri, extra
            );
            stdin
                .write_all(lsp_message(id, method, &params).as_bytes())
                .unwrap();
        }
        let references_without_declaration = format!(
            "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":2,\"character\":2}},\"context\":{{\"includeDeclaration\":false}}}}",
            main_uri
        );
        stdin
            .write_all(
                lsp_message(
                    8,
                    "textDocument/references",
                    &references_without_declaration,
                )
                .as_bytes(),
            )
            .unwrap();
        let type_definition = format!(
            "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":1,\"character\":13}}}}",
            main_uri
        );
        stdin
            .write_all(lsp_message(6, "textDocument/definition", &type_definition).as_bytes())
            .unwrap();
        let type_rename = format!(
            "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":1,\"character\":13}},\"newName\":\"Region\"}}",
            main_uri
        );
        stdin
            .write_all(lsp_message(7, "textDocument/rename", &type_rename).as_bytes())
            .unwrap();
        let root_change = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"workspace/didChangeWorkspaceFolders\",\"params\":{{\"event\":{{\"removed\":[{{\"uri\":{}}}],\"added\":[{{\"uri\":{}}}]}}}}}}",
            serde_escape(&root_uri),
            serde_escape(&added_root_uri)
        );
        stdin
            .write_all(
                format!(
                    "Content-Length: {}\r\n\r\n{}",
                    root_change.len(),
                    root_change
                )
                .as_bytes(),
            )
            .unwrap();
        for (id, method, extra) in [
            (
                9,
                "textDocument/references",
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (10, "textDocument/rename", ",\"newName\":\"afterUpdate\""),
            (11, "textDocument/rename", ",\"newName\":\"return\""),
            (12, "textDocument/rename", ",\"newName\":\"bad name\""),
        ] {
            let params = format!(
                "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":2,\"character\":2}}{}}}",
                main_uri, extra
            );
            stdin
                .write_all(lsp_message(id, method, &params).as_bytes())
                .unwrap();
        }
        let open_math = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{}\",\"languageId\":\"vey\",\"version\":1,\"text\":{}}}}}}}",
            math_uri,
            serde_escape(math_source)
        );
        stdin
            .write_all(
                format!("Content-Length: {}\r\n\r\n{}", open_math.len(), open_math).as_bytes(),
            )
            .unwrap();
        for (id, method, extra) in [
            (
                4,
                "textDocument/references",
                ",\"context\":{\"includeDeclaration\":true}",
            ),
            (5, "textDocument/rename", ",\"newName\":\"localArea\""),
        ] {
            let params = format!(
                "{{\"textDocument\":{{\"uri\":\"{}\"}},\"position\":{{\"line\":2,\"character\":7}}{}}}",
                math_uri, extra
            );
            stdin
                .write_all(lsp_message(id, method, &params).as_bytes())
                .unwrap();
        }
        stdin.flush().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
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
    assert.notEqual(separator, -1);
    const headers = bytes.toString("ascii", offset, separator);
    const length = Number(headers.match(/Content-Length:\s*(\d+)/i)[1]);
    const start = separator + 4;
    messages.push(JSON.parse(bytes.subarray(start, start + length).toString("utf8")));
    offset = start + length;
}
const result = id => {
    const response = messages.find(message => message.id === id);
    assert.ok(response && !response.error, JSON.stringify(response));
    return response.result;
};
const mainUri = process.argv[2];
const mathUri = process.argv[3];
const otherUri = process.argv[4];
const replacementUri = process.argv[5];
const initialized = result(0);
assert.equal(initialized.capabilities.workspace.workspaceFolders.supported, true);
assert.equal(initialized.capabilities.workspace.workspaceFolders.changeNotifications, true);
const definition = result(1);
assert.equal(definition.uri, mathUri);
assert.deepEqual([definition.range.start.line, definition.range.start.character], [5, 10]);
const references = result(2);
assert.equal(references.length, 3, JSON.stringify(references));
assert.deepEqual(references.map(item => item.uri).sort(), [mainUri, mathUri, otherUri].sort());
const referencesWithoutDeclaration = result(8);
assert.deepEqual(referencesWithoutDeclaration.map(item => item.uri).sort(), [mainUri, otherUri].sort());
const changes = result(3).changes;
assert.deepEqual(Object.keys(changes).sort(), [mainUri, mathUri, otherUri].sort());
assert.deepEqual(changes[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[5, 10, 14, "measure"]]);
assert.deepEqual(changes[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[2, 2, 6, "measure"]]);
assert.deepEqual(changes[otherUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[1, 2, 6, "measure"]]);
const typeDefinition = result(6);
assert.equal(typeDefinition.uri, mathUri);
assert.deepEqual([typeDefinition.range.start.line, typeDefinition.range.start.character], [0, 12]);
const typeChanges = result(7).changes;
assert.deepEqual(Object.keys(typeChanges).sort(), [mainUri, mathUri].sort());
assert.deepEqual(typeChanges[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[0, 12, 16, "Region"]]);
assert.deepEqual(typeChanges[mainUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [[1, 13, 17, "Region"]]);
const updatedReferences = result(9);
assert.deepEqual(updatedReferences.map(item => item.uri).sort(), [mainUri, mathUri, replacementUri].sort());
const updatedChanges = result(10).changes;
assert.deepEqual(Object.keys(updatedChanges).sort(), [mainUri, mathUri, replacementUri].sort());
assert.deepEqual(updatedChanges[mathUri].map(edit => edit.newText), ["afterUpdate"]);
assert.deepEqual(updatedChanges[mainUri].map(edit => edit.newText), ["afterUpdate"]);
assert.deepEqual(updatedChanges[replacementUri].map(edit => edit.newText), ["afterUpdate"]);
assert.equal(result(11), null);
assert.equal(result(12), null);
const localReferences = result(4);
assert.deepEqual(localReferences.map(item => [item.uri, item.range.start.line, item.range.start.character]), [
    [mathUri, 2, 6],
    [mathUri, 3, 8],
]);
const localChanges = result(5).changes;
assert.deepEqual(Object.keys(localChanges), [mathUri]);
assert.deepEqual(localChanges[mathUri].map(edit => [edit.range.start.line, edit.range.start.character, edit.range.end.character, edit.newText]), [
    [2, 6, 10, "localArea"],
    [3, 8, 12, "localArea"],
]);
"#,
        )
        .arg(&text)
        .arg(&main_uri)
        .arg(&math_uri)
        .arg(&other_uri)
        .arg(&replacement_uri)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}\nLSP output:\n{text}",
        String::from_utf8_lossy(&check.stderr)
    );
}
