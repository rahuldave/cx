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

#[test]
fn repeated_in_and_out_paths_are_up_to_date_on_the_second_run() {
    let temp = TempDir::new().expect("tempdir");
    write(&temp.path().join("input.txt"), "alpha\n");

    let args = [
        "--in",
        "input.txt",
        "--in",
        "input.txt",
        "--out",
        "output.txt",
        "--out",
        "output.txt",
        "--",
        "sh",
        "-c",
        "cat input.txt > output.txt && printf run >> runs.txt",
    ];

    assert!(cx(&temp, &args).status.success());
    let second = cx(&temp, &args);
    assert!(
        second.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("up-to-date: output.txt\n"),
        "stderr was: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(read(&temp.path().join("runs.txt")), "run");

    // The line with each path once is the same line, so it shares the record.
    let single = cx(
        &temp,
        &[
            "--in",
            "input.txt",
            "--out",
            "output.txt",
            "--",
            "sh",
            "-c",
            "cat input.txt > output.txt && printf run >> runs.txt",
        ],
    );
    assert!(single.status.success());
    assert_eq!(read(&temp.path().join("runs.txt")), "run");
}

#[test]
fn failed_command_passes_its_exit_code_through() {
    let temp = TempDir::new().expect("tempdir");

    let output = cx(
        &temp,
        &["--out", "output.txt", "--", "sh", "-c", "exit 200"],
    );

    assert_eq!(output.status.code(), Some(200));
    assert!(!temp.path().join(".cx").join("state.json").exists());
}

#[cfg(unix)]
#[test]
fn command_stopped_by_a_signal_gives_128_plus_the_signal() {
    let temp = TempDir::new().expect("tempdir");

    let term = cx(
        &temp,
        &["--out", "output.txt", "--", "sh", "-c", "kill -TERM $$"],
    );
    assert_eq!(term.status.code(), Some(128 + 15));

    let kill = cx(
        &temp,
        &["--out", "output.txt", "--", "sh", "-c", "kill -KILL $$"],
    );
    assert_eq!(kill.status.code(), Some(128 + 9));
}

#[test]
fn touched_input_with_the_same_content_keeps_the_hash_and_takes_the_new_time() {
    let temp = TempDir::new().expect("tempdir");
    let input = temp.path().join("input.txt");
    write(&input, "alpha\n");

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

    let state_path = temp.path().join(".cx").join("state.json");
    let stamp = |state: &str| -> serde_json::Value {
        let state: serde_json::Value = serde_json::from_str(state).expect("state json");
        state["output.txt"]["inputs"]["input.txt"].clone()
    };
    let before = stamp(&read(&state_path));

    let file = fs::File::options().write(true).open(&input).expect("open");
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
        .expect("set mtime");
    drop(file);

    let output = cx(&temp, &args);
    assert!(output.status.success());
    assert_eq!(read(&temp.path().join("runs.txt")), "run");

    let after = stamp(&read(&state_path));
    assert_eq!(after["hash"], before["hash"]);
    assert_eq!(after["mtime_ns"], 1_000_000u64 * 1_000_000_000);
}

#[test]
fn help_declares_out_as_required() {
    let temp = TempDir::new().expect("tempdir");

    let output = cx(&temp, &["--help"]);
    let help = String::from_utf8_lossy(&output.stdout);

    assert!(
        help.contains("cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND [ARG]..."),
        "help was: {help}"
    );
}
