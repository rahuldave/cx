//! The cx binary: runs `cx::run_env` and exits with its code.

use std::io::Write;

/// Runs cx with the arguments and the current folder of the process, and exits with the code
/// of `cx::run_env`. `std::process::exit` takes an `i32`, so the code of a failed command
/// keeps all the bits that the system allows.
fn main() {
    let code = cx::run_env();
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}
