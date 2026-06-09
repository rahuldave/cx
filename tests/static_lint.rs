use std::fs;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn cx(temp: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cx"))
        .args(args)
        .current_dir(temp.path())
        .output()
        .expect("run cx")
}

fn write_justfile(temp: &TempDir, body: &str) {
    fs::write(temp.path().join("Justfile"), body).expect("write Justfile");
}

#[test]
fn graph_extracts_linewise_cx_calls_from_just_dump() {
    let temp = TempDir::new().expect("tempdir");
    write_justfile(
        &temp,
        r#"
build part:
    cx --in data/{{part}}.txt --out out/{{part}}.txt -- sh -c 'cat "$1" > "$2"' _ data/{{part}}.txt out/{{part}}.txt
"#,
    );

    let output = cx(&temp, &["graph"]);
    assert!(
        output.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let graph: Value = serde_json::from_slice(&output.stdout).expect("graph json");
    let lines = graph["lines"].as_array().expect("lines array");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["recipe"], "build");
    assert_eq!(lines[0]["inputs"][0], "data/{{part}}.txt");
    assert_eq!(lines[0]["outputs"][0], "out/{{part}}.txt");
    assert!(graph["violations"].as_array().unwrap().is_empty());
}

#[test]
fn lint_rejects_cx_line_without_output() {
    let temp = TempDir::new().expect("tempdir");
    write_justfile(
        &temp,
        r#"
build:
    cx --in input.txt -- sh -c 'cat input.txt'
"#,
    );

    let output = cx(&temp, &["lint"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("at least one --out"),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
