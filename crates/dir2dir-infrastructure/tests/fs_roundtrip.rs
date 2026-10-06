//! End-to-end tests over the public API: scan → plan → apply → rescan.

use std::fs;
use std::path::Path;

use dir2dir_application::ports::{ApplyOptions, TreeSink, TreeSource};
use dir2dir_application::{CopyDir, CopyOptions};
use dir2dir_infrastructure::{FsSink, FsSource};

fn write(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(path, content).expect("write");
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// A source tree exercising every node kind: nested dirs, text, binary,
/// executables, an empty directory and a symlink.
fn populate_source(source: &Path) {
    write(&source.join("greeting"), b"Hello, world!\n");
    write(&source.join("dir/subfile"), b"Content.\n");
    fs::create_dir_all(source.join("dir/subdir")).expect("mkdir");
    write(&source.join("binary"), &[0x00, 0xff, 0xfe, 0x81, 0x7f]);
    write(&source.join("run.sh"), b"#!/bin/sh\necho Howdy!\n");
    make_executable(&source.join("run.sh"));
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/hostname", source.join("hostname-link")).expect("symlink");
}

#[test]
fn roundtrips_through_the_model() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let source_tree = FsSource.read(source.path()).expect("scan source");

    let plan = FsSink.plan(destination.path(), &source_tree).expect("plan");
    assert_eq!(plan.counts(), (5, 0, 0)); // greeting, dir/…, binary, run.sh, hostname-link

    FsSink
        .apply(
            destination.path(),
            &source_tree,
            &plan,
            &ApplyOptions::default(),
        )
        .expect("apply");

    let destination_tree = FsSource.read(destination.path()).expect("scan destination");
    assert_eq!(source_tree, destination_tree);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(destination.path().join("run.sh"))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755, "the executable bit must survive the copy");
    }
    assert_eq!(
        fs::read(destination.path().join("binary")).expect("read binary"),
        vec![0x00, 0xff, 0xfe, 0x81, 0x7f],
        "binary content must survive the copy byte-for-byte"
    );
}

#[test]
fn second_run_is_a_no_op() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let run = |dry_run: bool| {
        CopyDir::new(&FsSource, &FsSink)
            .execute(
                source.path(),
                destination.path(),
                &CopyOptions {
                    dry_run,
                    prune: false,
                },
            )
            .expect("copy")
    };

    let first = run(false);
    assert!(!first.is_empty());

    let second = run(false);
    assert!(
        second.is_empty(),
        "an unchanged tree must produce an empty plan"
    );
}

#[test]
fn dry_run_touches_nothing() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let plan = CopyDir::new(&FsSource, &FsSink)
        .execute(
            source.path(),
            destination.path(),
            &CopyOptions {
                dry_run: true,
                prune: false,
            },
        )
        .expect("dry-run copy");

    assert!(!plan.is_empty());
    assert!(
        fs::read_dir(destination.path()).expect("readdir").count() == 0,
        "dry-run must not create anything"
    );
}

#[test]
fn only_changed_nodes_are_rewritten() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");

    write(&source.path().join("stable"), b"stable\n");
    write(&source.path().join("volatile"), b"v1\n");

    let copy = || {
        CopyDir::new(&FsSource, &FsSink)
            .execute(source.path(), destination.path(), &CopyOptions::default())
            .expect("copy")
    };

    copy();
    write(&source.path().join("volatile"), b"v2\n");
    let plan = copy();

    let overwritten: Vec<String> = plan.overwritten().map(|p| p.to_string()).collect();
    assert_eq!(
        overwritten,
        ["volatile"],
        "only the changed node is planned"
    );
    assert_eq!(
        fs::read(destination.path().join("stable")).expect("read"),
        b"stable\n"
    );
    assert_eq!(
        fs::read(destination.path().join("volatile")).expect("read"),
        b"v2\n"
    );
}

#[test]
fn prune_deletes_extraneous_paths() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");
    populate_source(source.path());

    let run = |prune: bool| {
        CopyDir::new(&FsSource, &FsSink)
            .execute(
                source.path(),
                destination.path(),
                &CopyOptions {
                    dry_run: false,
                    prune,
                },
            )
            .expect("copy")
    };

    run(false);
    write(&destination.path().join("extraneous/stale"), b"old\n");
    fs::write(destination.path().join("loose-end"), b"old\n").expect("write");

    let plan = run(false);
    let extraneous: Vec<String> = plan.extraneous().map(|p| p.to_string()).collect();
    assert_eq!(extraneous, ["extraneous", "loose-end"]);
    assert!(
        destination.path().join("loose-end").exists(),
        "without --prune extraneous files stay"
    );

    run(true);
    assert!(
        !destination.path().join("loose-end").exists(),
        "--prune must delete extraneous files"
    );
    assert!(
        !destination.path().join("extraneous").exists(),
        "--prune must delete extraneous directories"
    );
}

#[test]
fn never_writes_through_a_preexisting_symlink() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");

    // Attacker pre-seeds destination/x -> canary outside the destination.
    let canary = tempfile::tempdir().expect("canary");
    let canary_file = canary.path().join("authorized_keys");
    fs::write(&canary_file, b"precious\n").expect("write");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&canary_file, destination.path().join("x")).expect("symlink");

    write(&source.path().join("x"), b"attacker payload\n");

    CopyDir::new(&FsSource, &FsSink)
        .execute(source.path(), destination.path(), &CopyOptions::default())
        .expect("copy");

    assert_eq!(
        fs::read(&canary_file).expect("read canary"),
        b"precious\n",
        "the sink must never write through a pre-existing symlink"
    );
    assert_eq!(
        fs::read(destination.path().join("x")).expect("read x"),
        b"attacker payload\n",
        "x must now be a regular file with the copied content"
    );
    #[cfg(unix)]
    assert!(!fs::symlink_metadata(destination.path().join("x"))
        .expect("stat x")
        .is_symlink());
}

#[test]
fn kind_transitions_are_handled() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir().expect("destination");

    // file -> directory
    write(&source.path().join("a"), b"file\n");
    CopyDir::new(&FsSource, &FsSink)
        .execute(source.path(), destination.path(), &CopyOptions::default())
        .expect("copy");

    fs::remove_file(source.path().join("a")).expect("rm");
    write(&source.path().join("a/child"), b"now a directory\n");
    CopyDir::new(&FsSource, &FsSink)
        .execute(source.path(), destination.path(), &CopyOptions::default())
        .expect("copy");
    assert!(destination.path().join("a/child").is_file());

    // directory -> file
    fs::remove_file(source.path().join("a/child")).expect("rm");
    fs::remove_dir(source.path().join("a")).expect("rmdir");
    write(&source.path().join("a"), b"back to a file\n");
    CopyDir::new(&FsSource, &FsSink)
        .execute(source.path(), destination.path(), &CopyOptions::default())
        .expect("copy");
    assert_eq!(
        fs::read(destination.path().join("a")).expect("read"),
        b"back to a file\n"
    );
}

#[test]
fn empty_source_materializes_the_destination_root() {
    let source = tempfile::tempdir().expect("source");
    let destination = tempfile::tempdir()
        .expect("destination")
        .path()
        .join("does-not-exist-yet");

    CopyDir::new(&FsSource, &FsSink)
        .execute(source.path(), &destination, &CopyOptions::default())
        .expect("copy");

    assert!(destination.is_dir());
}
