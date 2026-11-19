//! The cx binary: runs `cx::run_env` and exits with its code.

use std::process::ExitCode;

/// Runs cx with the arguments and the current folder of the process.
fn main() -> ExitCode {
    cx::run_env()
}
