//! Integration tests: drive the real binary end to end.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dir2dir"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dir2dir");

    // Always take and drop stdin so the child sees EOF even when we pass none.
    if let Some(input) = stdin {
        let mut handle = child.stdin.take().expect("stdin");
        handle.write_all(input.as_bytes()).expect("write stdin");
    } else {
        drop(child.stdin.take());
    }

    child.wait_with_output().expect("wait for dir2dir")
}

fn write(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(path, content).expect("write");
}

fn populate_source(source: &Path) {
    write(&source.join("greeting"), b"Hello, world!\n");
    write(&source.join("dir/subfile"), b"Content.\n");
    write(&source.join("run.sh"), b"#!/bin/sh\necho Howdy!\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(source.join("run.sh"), fs::Permissions::from_mode(0o755))
            .expect("chmod");
        std::os::unix::fs::symlink("/etc/hostname", source.join("hostname-link")).expect("symlink");
    }
}

#[test]
fn copy_materializes_the_tree() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let output = run(
        &[
            "copy",
            source.path().to_str().unwrap(),
            destination.path().to_str().unwrap(),
        ],
        None,
    );
    assert!(
        output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    assert_eq!(
        fs::read_to_string(destination.path().join("greeting")).expect("read"),
        "Hello, world!\n"
    );
    assert_eq!(
        fs::read_to_string(destination.path().join("dir/subfile")).expect("read"),
        "Content.\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(destination.path().join("run.sh"))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
        assert!(
            fs::symlink_metadata(destination.path().join("hostname-link"))
                .expect("stat")
                .is_symlink()
        );
    }
}

#[test]
fn copy_is_idempotent_and_reports_no_changes() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let src = source.path().to_str().unwrap();
    let dst = destination.path().to_str().unwrap();
    assert!(run(&["copy", src, dst], None).status.success());
    let second = run(&["copy", src, dst], None);

    assert!(second.status.success());
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("0 created, 0 overwritten, 0 extraneous"),
        "second run must be a no-op, got: {stdout}"
    );
}

#[test]
fn dry_run_leaves_the_destination_alone() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let output = run(
        &[
            "copy",
            source.path().to_str().unwrap(),
            destination.path().to_str().unwrap(),
            "--dry-run",
        ],
        None,
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("create     greeting"), "got: {stdout}");
    assert!(
        fs::read_dir(destination.path()).expect("readdir").count() == 0,
        "dry-run must not create anything"
    );
}

#[test]
fn prune_deletes_extraneous_paths() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let src = source.path().to_str().unwrap();
    let dst = destination.path().to_str().unwrap();
    assert!(run(&["copy", src, dst], None).status.success());

    write(&destination.path().join("stale"), b"old\n");

    let kept = run(&["copy", src, dst], None);
    let stdout = String::from_utf8_lossy(&kept.stdout);
    assert!(stdout.contains("extraneous"), "got: {stdout}");
    assert!(destination.path().join("stale").exists());

    let pruned = run(&["copy", src, dst, "--prune"], None);
    assert!(pruned.status.success());
    assert!(!destination.path().join("stale").exists());
}

#[test]
fn to_json_pipes_into_from_json() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let exported = run(&["to-json", source.path().to_str().unwrap()], None);
    assert!(exported.status.success());
    let document = String::from_utf8_lossy(&exported.stdout).to_string();

    // json2dir-compat shapes on the wire
    assert!(document.contains("\"script\""), "got: {document}");
    assert!(document.contains("\"link\""), "got: {document}");

    let imported = run(
        &["from-json", destination.path().to_str().unwrap()],
        Some(&document),
    );
    assert!(
        imported.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&imported.stderr)
    );

    assert_eq!(
        fs::read_to_string(destination.path().join("greeting")).expect("read"),
        "Hello, world!\n"
    );
    assert_eq!(
        fs::read_to_string(destination.path().join("dir/subfile")).expect("read"),
        "Content.\n"
    );
}

#[test]
fn from_json_accepts_the_json2dir_example() {
    let destination = tempfile::tempdir().expect("destination");
    let output = run(
        &["from-json", destination.path().to_str().unwrap()],
        Some(
            r##"{
  "greeting": "Hello, world!",
  "dir": { "subfile": "Content.\n", "subdir": {} },
  "symlink": ["link", "target path"],
  "script": ["script", "#!/bin/sh\necho Howdy!"]
}"##,
        ),
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(destination.path().join("greeting")).expect("read"),
        "Hello, world!"
    );
    assert!(destination.path().join("dir/subdir").is_dir());
    #[cfg(unix)]
    assert!(fs::symlink_metadata(destination.path().join("symlink"))
        .expect("stat")
        .is_symlink());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(destination.path().join("script"))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
    }
}

#[test]
fn binary_content_survives_the_json_roundtrip() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    write(&source.path().join("blob"), &[0x00, 0xff, 0xfe, 0x81]);

    let src = source.path().to_str().unwrap();
    let dst = destination.path().to_str().unwrap();

    let exported = run(&["to-json", src], None);
    assert!(exported.status.success());
    let document = String::from_utf8_lossy(&exported.stdout);
    assert!(document.contains("\"b64\""), "got: {document}");

    let imported = run(&["from-json", dst], Some(&document));
    assert!(imported.status.success());

    assert_eq!(
        fs::read(destination.path().join("blob")).expect("read"),
        vec![0x00, 0xff, 0xfe, 0x81]
    );
}

#[test]
fn diff_reports_differences_with_diff_exit_codes() {
    let left = tempfile::tempdir().expect("left");
    let right = tempfile::tempdir().expect("right");

    let a = left.path().to_str().unwrap();
    let b = right.path().to_str().unwrap();

    write(&left.path().join("same"), b"x\n");
    write(&right.path().join("same"), b"x\n");

    let identical = run(&["diff", a, b], None);
    assert!(identical.status.success());

    write(&right.path().join("extra"), b"y\n");
    let different = run(&["diff", a, b], None);
    assert_eq!(different.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&different.stdout);
    assert!(stdout.contains("create     extra"), "got: {stdout}");
}

#[test]
fn errors_are_reported_not_panicked() {
    let destination = tempfile::tempdir().expect("destination");

    let output = run(
        &[
            "copy",
            "/nonexistent-dir2dir-source",
            destination.path().to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("error:"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run(
        &["from-json", destination.path().to_str().unwrap()],
        Some("not json"),
    );
    assert_eq!(output.status.code(), Some(1));
}
