//! Tests for the command line tools.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

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
