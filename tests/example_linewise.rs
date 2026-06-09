use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination dir");

    for entry in fs::read_dir(source).expect("read source dir") {
        let entry = entry.expect("dir entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).expect("copy file");
        }
    }
}

fn run_just(example: &Path) -> Output {
    let cx_bin = PathBuf::from(env!("CARGO_BIN_EXE_cx"));
    let cx_dir = cx_bin.parent().expect("cx bin parent");
    let path = format!(
        "{}:{}",
        cx_dir.display(),
        env::var("PATH").unwrap_or_default()
    );

    Command::new("just")
        .arg("build")
        .current_dir(example)
        .env("PATH", path)
        .output()
        .expect("run just build")
}

#[test]
fn linewise_example_runs_with_just_and_then_skips_cx_lines() {
    let temp = TempDir::new().expect("tempdir");
    let example = temp.path().join("linewise");
    copy_dir(Path::new("examples/linewise"), &example);

    let first = run_just(&example);
    assert!(
        first.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        fs::read_to_string(example.join("dist/message.txt")).expect("read output"),
        "Result: HELLO FROM CX\n"
    );

    let second = run_just(&example);
    assert!(
        second.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("up-to-date: build/message.upper"));
    assert!(stderr.contains("up-to-date: dist/message.txt"));
}
