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
fn help_names_conditional_execution() {
    let temp = TempDir::new().expect("tempdir");

    let output = cx(&temp, &["--help"]);

    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Conditional eXecution"),
        "stdout was: {}",
        String::from_utf8_lossy(&output.stdout)
    );
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

fn assert_prints_version(flag: &str) {
    let temp = TempDir::new().expect("tempdir");
    // A broken Justfile proves the flag answers before cx reads project files.
    write(
        &temp.path().join("Justfile"),
        "this is not valid just {{{\n",
    );

    let output = cx(&temp, &[flag]);

    assert!(output.status.success(), "status was: {:?}", output.status);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("cx {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(
        output.stderr.is_empty(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn long_version_flag_prints_version() {
    assert_prints_version("--version");
}

#[test]
fn short_version_flag_prints_version() {
    assert_prints_version("-V");
}

#[test]
fn existing_options_still_work_beside_version_flag() {
    let temp = TempDir::new().expect("tempdir");
    write(&temp.path().join("input.txt"), "alpha\n");

    // --version after -- belongs to COMMAND, not to cx.
    let run = cx(
        &temp,
        &[
            "--in",
            "input.txt",
            "--out",
            "output.txt",
            "--",
            "sh",
            "-c",
            "printf %s \"$0\" > output.txt",
            "--version",
        ],
    );
    assert!(
        run.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(read(&temp.path().join("output.txt")), "--version");

    let help = cx(&temp, &["-h"]);
    assert!(help.status.success());
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.starts_with(&format!("cx {}\n", env!("CARGO_PKG_VERSION"))));
    assert!(help_text.contains("cx --version"));

    // The version flag counts only as the first argument.
    let late = cx(&temp, &["--in", "input.txt", "--version"]);
    assert!(!late.status.success());
    assert!(String::from_utf8_lossy(&late.stderr).contains("unknown cx option: --version"));

    let unknown = cx(&temp, &["--bogus"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown cx option: --bogus"));
}
