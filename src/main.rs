use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        None | Some("--help") | Some("-h") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("graph") | Some("lint") => {
            eprintln!("cx is scaffolded; implement this subcommand after accepting cx-spec.md.");
            ExitCode::FAILURE
        }
        Some(_) => {
            eprintln!("cx is scaffolded; see cx-spec.md and AGENTS.md for next steps.");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "cx 0.1.0\n\nUSAGE:\n    cx [--in PATH]... [--out PATH]... -- COMMAND [ARG]...\n    cx graph\n    cx lint\n\nThis repository is a Rust scaffold. See cx-spec.md for the preliminary proposal."
    );
}
