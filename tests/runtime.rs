use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn cx(temp: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cx"))
        .args(args)
        .current_dir(temp.path())
        .output()
        .expect("run cx")
}

fn write(path: &Path, body: &str) {
    fs::write(path, body).expect("write file");
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read file")
}

#[test]
fn requires_at_least_one_output() {
    let temp = TempDir::new().expect("tempdir");
    write(&temp.path().join("input.txt"), "alpha");

    let output = cx(
        &temp,
        &["--in", "input.txt", "--", "sh", "-c", "cat input.txt"],
    );

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("at least one --out"),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn first_run_executes_and_second_run_is_up_to_date() {
    let temp = TempDir::new().expect("tempdir");
    write(&temp.path().join("input.txt"), "alpha\n");

    let args = [
        "--in",
        "input.txt",
        "--out",
        "output.txt",
        "--",
        "sh",
        "-c",
        "cat input.txt > output.txt && printf run >> runs.txt",
    ];

    let first = cx(&temp, &args);
    assert!(
        first.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(read(&temp.path().join("output.txt")), "alpha\n");
    assert_eq!(read(&temp.path().join("runs.txt")), "run");

    let second = cx(&temp, &args);
    assert!(
        second.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("up-to-date: output.txt"),
        "stderr was: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(read(&temp.path().join("runs.txt")), "run");
}

#[test]
fn same_content_input_write_does_not_rerun() {
    let temp = TempDir::new().expect("tempdir");
    write(&temp.path().join("input.txt"), "alpha\n");

    let args = [
        "--in",
        "input.txt",
        "--out",
        "output.txt",
        "--",
        "sh",
        "-c",
        "cat input.txt > output.txt && printf run >> runs.txt",
    ];

    assert!(cx(&temp, &args).status.success());
    write(&temp.path().join("input.txt"), "alpha\n");

    let output = cx(&temp, &args);
    assert!(
        output.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read(&temp.path().join("runs.txt")), "run");
}

#[test]
fn changed_input_content_reruns_command() {
    let temp = TempDir::new().expect("tempdir");
    write(&temp.path().join("input.txt"), "alpha\n");

    let args = [
        "--in",
        "input.txt",
        "--out",
        "output.txt",
        "--",
        "sh",
        "-c",
        "cat input.txt > output.txt && printf run >> runs.txt",
    ];

    assert!(cx(&temp, &args).status.success());
    write(&temp.path().join("input.txt"), "beta\n");

    let output = cx(&temp, &args);
    assert!(
        output.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read(&temp.path().join("output.txt")), "beta\n");
    assert_eq!(read(&temp.path().join("runs.txt")), "runrun");
}
