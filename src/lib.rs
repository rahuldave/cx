//! cx: conditional execution for linewise `just` recipe lines.
//!
//! `cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND [ARG]...` runs COMMAND only
//! when the inputs, the outputs or the command line changed since the last successful run, and
//! keeps the records in `.cx/state.json`. `cx graph` and `cx lint` read
//! `just --dump --dump-format json` and report the cx lines of the Justfile.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::UNIX_EPOCH;

/// Runs cx with ARGS (without the program name) in the folder CWD, and returns the exit code.
///
/// With no argument, a first argument that is not UTF-8, `--help` or `-h`, it prints the
/// help, and with `--version` or `-V` the version. `graph` and `lint` run the static
/// analysis of the Justfile. Any other first argument starts a runtime invocation.
///
/// The code is 0 on success and 1 on an error of cx. When the command of a cx line fails, it
/// is the code of the command, as `command_exit_code` gives it. The code is an `i32`, not an
/// `ExitCode`, because an `ExitCode` holds only 8 bits. Some systems, such as Windows, give a
/// command a wider code. Pass the code to `std::process::exit`.
pub fn run<I, S>(args: I, cwd: &Path) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();

    match args.first().and_then(|arg| arg.to_str()) {
        None | Some("--help") | Some("-h") => {
            print_help();
            SUCCESS
        }
        Some("--version") | Some("-V") => {
            println!("cx {}", env!("CARGO_PKG_VERSION"));
            SUCCESS
        }
        Some("graph") => run_static(StaticMode::Graph, cwd),
        Some("lint") => run_static(StaticMode::Lint, cwd),
        Some(_) => match RuntimeInvocation::parse(args) {
            Ok(invocation) => run_runtime(invocation, cwd),
            Err(message) => fail(message),
        },
    }
}

/// The exit code of a success.
const SUCCESS: i32 = 0;

/// The exit code of an error of cx, and of a `cx lint` that finds a violation.
const FAILURE: i32 = 1;

/// The parsed arguments of a runtime invocation, which has the form
/// `cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND`.
#[derive(Debug, Clone)]
struct RuntimeInvocation {
    /// The paths of the `--in` options, in the order given, each path once.
    inputs: Vec<String>,
    /// The paths of the `--out` options, in the order given, each path once. At least one.
    outputs: Vec<String>,
    /// The command after `--` and its arguments. Not empty.
    command: Vec<OsString>,
}

impl RuntimeInvocation {
    /// Parses the arguments after the program name.
    ///
    /// It drops a path that a second `--in` gives again, and a path that a second `--out`
    /// gives again. The record of the state keeps each path once, so a repeated path would
    /// make the line stale on each run.
    ///
    /// # Errors
    ///
    /// Returns a message in each of these cases:
    ///
    /// - an option that is not `--in`, `--out` or `--`;
    /// - a bare argument before `--`;
    /// - an option with no value;
    /// - an option or path that is not UTF-8;
    /// - no `--out`;
    /// - no command after `--`.
    fn parse(args: Vec<OsString>) -> Result<Self, String> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut command = Vec::new();
        let mut index = 0;

        while index < args.len() {
            let token = args[index]
                .to_str()
                .ok_or_else(|| "cx arguments must be valid UTF-8 before --".to_string())?;

            match token {
                "--in" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "--in requires a PATH operand".to_string())?;
                    push_unique(&mut inputs, os_string_to_string(value, "--in PATH")?);
                }
                "--out" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "--out requires a PATH operand".to_string())?;
                    push_unique(&mut outputs, os_string_to_string(value, "--out PATH")?);
                }
                "--" => {
                    command.extend(args[index + 1..].iter().cloned());
                    break;
                }
                other if other.starts_with("--") => {
                    return Err(format!("unknown cx option: {other}"));
                }
                other => {
                    return Err(format!("unexpected argument before -- COMMAND: {other}"));
                }
            }

            index += 1;
        }

        if outputs.is_empty() {
            return Err("cx requires at least one --out PATH".to_string());
        }

        if command.is_empty() {
            return Err("cx requires -- COMMAND [ARG]...".to_string());
        }

        Ok(Self {
            inputs,
            outputs,
            command,
        })
    }

    /// The key of the record in the state: the output paths, sorted and joined with newlines.
    fn key(&self) -> String {
        let mut outputs = self.outputs.clone();
        outputs.sort();
        outputs.join("\n")
    }

    /// The SHA-256 hex digest of the sorted inputs, the sorted outputs and the command.
    ///
    /// A change to the inputs or the command makes the stored record stale. A change to the
    /// outputs selects another record. The order of the `--in` and `--out` options does not
    /// count.
    fn identity_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cx-runtime-v1\0");

        for input in sorted(&self.inputs) {
            hasher.update(b"--in\0");
            hasher.update(input.as_bytes());
            hasher.update(b"\0");
        }

        for output in sorted(&self.outputs) {
            hasher.update(b"--out\0");
            hasher.update(output.as_bytes());
            hasher.update(b"\0");
        }

        hasher.update(b"--\0");
        for arg in &self.command {
            hasher.update(arg.as_encoded_bytes());
            hasher.update(b"\0");
        }

        format!("{:x}", hasher.finalize())
    }
}

/// The stamp of one input file at the last run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileStamp {
    /// The modification time, in nanoseconds since the Unix epoch.
    mtime_ns: u64,
    /// The size in bytes.
    size: u64,
    /// The SHA-256 hex digest of the content.
    hash: String,
}

/// The record of the last run or up-to-date check for one set of outputs, stored under the
/// output key. Two cx lines with the same outputs share one record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StalenessRecord {
    /// The identity hash of the invocation, from `RuntimeInvocation::identity_hash`.
    command_hash: String,
    /// The stamp of each input path.
    inputs: BTreeMap<String, FileStamp>,
    /// The SHA-256 hex digest of the content of each output file, by path.
    outputs: BTreeMap<String, String>,
}

/// The content of `.cx/state.json`: one staleness record for each output key.
type State = BTreeMap<String, StalenessRecord>;

/// The subcommand of the static analysis.
enum StaticMode {
    /// `cx graph`: print the static graph as JSON.
    Graph,
    /// `cx lint`: print each violation and fail when there is one.
    Lint,
}

/// The result of the static analysis of a Justfile, which `cx graph` prints as JSON.
#[derive(Debug, Serialize)]
struct StaticGraph {
    /// The path of the Justfile, as `just --dump` gives it.
    source: Option<String>,
    /// The cx lines that parsed.
    lines: Vec<StaticCxLine>,
    /// The cx lines that broke a rule.
    violations: Vec<LintViolation>,
}

/// One valid cx line of a linewise recipe. The paths and the command are the shell words of
/// the body line as cx renders it back from the dump. An interpolation is not evaluated: it
/// becomes `{{...}}` text, which can differ from the Justfile source.
#[derive(Debug, Serialize)]
struct StaticCxLine {
    /// The name of the recipe.
    recipe: String,
    /// The number of the body line in the recipe, from 1.
    line: usize,
    /// The body line, rendered back to text.
    body: String,
    /// The paths of the `--in` options.
    inputs: Vec<String>,
    /// The paths of the `--out` options.
    outputs: Vec<String>,
    /// The command after `--` and its arguments.
    command: Vec<String>,
    /// The SHA-256 hex digest of the rendered body line.
    command_hash: String,
}

/// The options of a cx line that `parse_static_tokens` read, before the rules are checked.
struct StaticCxArgs {
    /// The paths of the `--in` options.
    inputs: Vec<String>,
    /// The paths of the `--out` options. It can be empty here.
    outputs: Vec<String>,
    /// The command after `--`. It can be empty here.
    command: Vec<String>,
}

/// A cx line that breaks a rule, as `cx lint` reports it.
#[derive(Debug, Serialize)]
struct LintViolation {
    /// The name of the recipe.
    recipe: String,
    /// The number of the body line in the recipe, from 1.
    line: usize,
    /// What is wrong with the line.
    message: String,
}

/// The parts of the JSON of `just --dump --dump-format json` that cx reads.
#[derive(Debug, Deserialize)]
struct JustDump {
    /// The path of the Justfile.
    source: Option<String>,
    /// The recipes, by name.
    recipes: BTreeMap<String, JustRecipe>,
}

/// The parts of one recipe of the dump that cx reads.
#[derive(Debug, Deserialize)]
struct JustRecipe {
    /// The attributes of the recipe, such as `[script]`.
    #[serde(default)]
    attributes: Vec<Value>,
    /// The body lines. Each line is a list of fragments: text, or an interpolation.
    #[serde(default)]
    body: Vec<Vec<Value>>,
    /// True when the recipe starts with a `#!` line.
    #[serde(default)]
    shebang: bool,
}

/// Runs a runtime invocation, and prints the error and fails when it gives one.
fn run_runtime(invocation: RuntimeInvocation, cwd: &Path) -> i32 {
    match run_runtime_inner(invocation, cwd) {
        Ok(code) => code,
        Err(error) => fail(error.to_string()),
    }
}

/// Runs `cx graph` or `cx lint`, and prints the error and fails when it gives one.
fn run_static(mode: StaticMode, cwd: &Path) -> i32 {
    match run_static_inner(mode, cwd) {
        Ok(code) => code,
        Err(error) => fail(error.to_string()),
    }
}

/// Builds the static graph, then prints it as JSON for `graph`, or prints each violation for
/// `lint`. `lint` returns a failure code when there is a violation.
///
/// # Errors
///
/// Returns the errors of `build_static_graph`, and `CxError::StaticJson` when the graph cannot
/// be serialized.
fn run_static_inner(mode: StaticMode, cwd: &Path) -> Result<i32, CxError> {
    let graph = build_static_graph(cwd)?;

    match mode {
        StaticMode::Graph => {
            let json = serde_json::to_string_pretty(&graph).map_err(CxError::StaticJson)?;
            println!("{json}");
            Ok(SUCCESS)
        }
        StaticMode::Lint if graph.violations.is_empty() => Ok(SUCCESS),
        StaticMode::Lint => {
            for violation in &graph.violations {
                eprintln!(
                    "lint error: {}:{}: {}",
                    violation.recipe, violation.line, violation.message
                );
            }
            Ok(FAILURE)
        }
    }
}

/// Runs `just --dump --dump-format json` in CWD and collects the cx lines and violations.
///
/// In a shebang recipe, or a recipe with the attribute `[script]` or `[script(...)]`, each line
/// with a word `cx` is a violation. In a linewise recipe, a line whose first shell word, after
/// an optional `@`, is `cx` gives a cx line or a violation.
///
/// # Errors
///
/// Returns `CxError::JustSpawn` when `just` cannot start, `CxError::JustDump` when it exits
/// with a failure, and `CxError::StaticJson` when its output is not a valid dump.
fn build_static_graph(cwd: &Path) -> Result<StaticGraph, CxError> {
    let output = Command::new("just")
        .args(["--dump", "--dump-format", "json"])
        .current_dir(cwd)
        .output()
        .map_err(CxError::JustSpawn)?;

    if !output.status.success() {
        return Err(CxError::JustDump {
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let dump: JustDump = serde_json::from_slice(&output.stdout).map_err(CxError::StaticJson)?;
    let mut lines = Vec::new();
    let mut violations = Vec::new();

    for (recipe_name, recipe) in &dump.recipes {
        let is_script_form = recipe.shebang || has_script_attribute(&recipe.attributes);

        for (index, fragments) in recipe.body.iter().enumerate() {
            let line_number = index + 1;
            let body = render_body_fragments(fragments);

            if is_script_form {
                if body.split_whitespace().any(|token| token == "cx") {
                    violations.push(LintViolation {
                        recipe: recipe_name.clone(),
                        line: line_number,
                        message: "cx is not supported inside script recipes; use a linewise recipe"
                            .to_string(),
                    });
                }
                continue;
            }

            match parse_static_cx_line(recipe_name, line_number, &body) {
                StaticLineParse::NotCx => {}
                StaticLineParse::Line(line) => lines.push(line),
                StaticLineParse::Violation(violation) => violations.push(violation),
            }
        }
    }

    Ok(StaticGraph {
        source: dump.source,
        lines,
        violations,
    })
}

/// The result of reading one recipe body line.
enum StaticLineParse {
    /// The first shell word of the line, after an optional `@`, is not `cx`.
    NotCx,
    /// A valid cx line.
    Line(StaticCxLine),
    /// A cx line that breaks a rule.
    Violation(LintViolation),
}

/// Reads one body line of a linewise recipe.
///
/// Leading whitespace and one `@` are dropped, and the line is split by shell rules. A line
/// that does not split is a violation when its first word is `cx`, as `first_word_is_cx`
/// finds it. When the first word is `cx`, the options are read. A bad option, no `--out` or
/// no command is a violation.
fn parse_static_cx_line(recipe: &str, line: usize, body: &str) -> StaticLineParse {
    let trimmed = body.trim_start();
    let without_quiet = trimmed
        .strip_prefix('@')
        .map(str::trim_start)
        .unwrap_or(trimmed);

    let tokens = match shell_words::split(without_quiet) {
        Ok(tokens) => tokens,
        Err(error) => {
            if first_word_is_cx(without_quiet) {
                return StaticLineParse::Violation(LintViolation {
                    recipe: recipe.to_string(),
                    line,
                    message: format!("failed to parse cx line: {error}"),
                });
            }
            return StaticLineParse::NotCx;
        }
    };

    if tokens.first().map(String::as_str) != Some("cx") {
        return StaticLineParse::NotCx;
    }

    match parse_static_tokens(&tokens[1..]) {
        Ok(args) => {
            if args.outputs.is_empty() {
                return StaticLineParse::Violation(LintViolation {
                    recipe: recipe.to_string(),
                    line,
                    message: "cx requires at least one --out PATH".to_string(),
                });
            }
            if args.command.is_empty() {
                return StaticLineParse::Violation(LintViolation {
                    recipe: recipe.to_string(),
                    line,
                    message: "cx requires -- COMMAND [ARG]...".to_string(),
                });
            }

            StaticLineParse::Line(StaticCxLine {
                recipe: recipe.to_string(),
                line,
                body: body.to_string(),
                inputs: args.inputs,
                outputs: args.outputs,
                command: args.command,
                command_hash: hash_text(body),
            })
        }
        Err(message) => StaticLineParse::Violation(LintViolation {
            recipe: recipe.to_string(),
            line,
            message,
        }),
    }
}

/// True when the first shell word of TEXT is `cx`, for a line that does not split as a whole.
///
/// The first word ends at the first space, tab or newline outside quotes, as `shell_words`
/// splits words. So the text before the first such character holds the whole first word, or
/// the start of a quoted word that goes on past it. The function is true when that text splits
/// into the one word `cx`. So `cxfoo 'open` and `cx-tool 'open` are not cx lines, and
/// `cx --out 'open` and `"cx" --out 'open` are.
fn first_word_is_cx(text: &str) -> bool {
    let head = text.split([' ', '\t', '\n']).next().unwrap_or("");
    matches!(shell_words::split(head).as_deref(), Ok([word]) if word == "cx")
}

/// Reads the options of a static cx line, the words after `cx`. It does not check that
/// `--out` or the command is present.
///
/// # Errors
///
/// Returns a message on an option that is not `--in`, `--out` or `--`, on a bare word before
/// `--`, and on an option with no value.
fn parse_static_tokens(tokens: &[String]) -> Result<StaticCxArgs, String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut command = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        match tokens[index].as_str() {
            "--in" => {
                index += 1;
                let value = tokens
                    .get(index)
                    .ok_or_else(|| "--in requires a PATH operand".to_string())?;
                inputs.push(value.clone());
            }
            "--out" => {
                index += 1;
                let value = tokens
                    .get(index)
                    .ok_or_else(|| "--out requires a PATH operand".to_string())?;
                outputs.push(value.clone());
            }
            "--" => {
                command.extend_from_slice(&tokens[index + 1..]);
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown cx option: {other}"));
            }
            other => {
                return Err(format!("unexpected argument before -- COMMAND: {other}"));
            }
        }

        index += 1;
    }

    Ok(StaticCxArgs {
        inputs,
        outputs,
        command,
    })
}

/// Renders the fragments of one body line of the dump back to text.
fn render_body_fragments(fragments: &[Value]) -> String {
    fragments.iter().map(render_body_fragment).collect()
}

/// Renders one fragment: text as it is, and an interpolation in `{{ }}`.
fn render_body_fragment(fragment: &Value) -> String {
    match fragment {
        Value::String(text) => text.clone(),
        Value::Array(parts) => format!("{{{{{}}}}}", render_expression(parts)),
        other => other.to_string(),
    }
}

/// Renders the expression of an interpolation: a single node by itself, or the parts joined
/// with spaces.
fn render_expression(parts: &[Value]) -> String {
    match parts {
        [Value::Array(node)] => render_expression_node(node),
        _ => parts
            .iter()
            .map(render_expression_part)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Renders one expression node: a variable as its name, any other node as its parts joined
/// with spaces.
fn render_expression_node(node: &[Value]) -> String {
    match node {
        [Value::String(kind), Value::String(name)] if kind == "variable" => name.clone(),
        _ => node
            .iter()
            .map(render_expression_part)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Renders one part of an expression: text as it is, a list as a nested expression.
fn render_expression_part(part: &Value) -> String {
    match part {
        Value::String(text) => text.clone(),
        Value::Array(parts) => render_expression(parts),
        other => other.to_string(),
    }
}

/// Decides whether the cx line is stale, runs the command when it is, and saves the record.
///
/// The line is stale when it has no record, its identity hash changed, an input hash changed,
/// an output is missing or its hash changed, or the record holds a different number of paths
/// than the line gives. `RuntimeInvocation::parse` keeps each path once, so a repeated `--in`
/// or `--out` path does not change the count. When it is up to date, the record is refreshed
/// and the command does not run. When the command fails, the code of `command_exit_code` is
/// returned, and the state is not changed.
///
/// # Errors
///
/// Returns an error when the state cannot be read, parsed or written, when an input cannot be
/// stamped, when an output cannot be hashed, or when the command cannot start. Returns
/// `CxError::MissingOutput` when the command succeeds but an output does not exist.
fn run_runtime_inner(invocation: RuntimeInvocation, cwd: &Path) -> Result<i32, CxError> {
    let state_path = cwd.join(".cx").join("state.json");
    let mut state = load_state(&state_path)?;
    let key = invocation.key();
    let identity = invocation.identity_hash();
    let previous = state.get(&key);

    let input_stamps = collect_input_stamps(cwd, &invocation.inputs, previous)?;
    let output_hashes = collect_existing_output_hashes(cwd, &invocation.outputs)?;
    let mut stale = previous.is_none();

    if previous
        .map(|record| record.command_hash.as_str() != identity.as_str())
        .unwrap_or(false)
    {
        stale = true;
    }

    if let Some(previous) = previous {
        for input in &invocation.inputs {
            let Some(current) = input_stamps.get(input) else {
                stale = true;
                continue;
            };
            if previous.inputs.get(input).map(|old| old.hash.as_str())
                != Some(current.hash.as_str())
            {
                stale = true;
            }
        }

        if previous.inputs.len() != invocation.inputs.len() {
            stale = true;
        }

        for output in &invocation.outputs {
            match output_hashes.get(output) {
                Some(hash) if previous.outputs.get(output) == Some(hash) => {}
                _ => stale = true,
            }
        }

        if previous.outputs.len() != invocation.outputs.len() {
            stale = true;
        }
    }

    if !stale {
        eprintln!("up-to-date: {}", invocation.outputs.join(", "));
        let refreshed = StalenessRecord {
            command_hash: identity,
            inputs: input_stamps,
            outputs: output_hashes,
        };
        state.insert(key, refreshed);
        save_state(&state_path, &state)?;
        return Ok(SUCCESS);
    }

    let status = Command::new(&invocation.command[0])
        .args(&invocation.command[1..])
        .current_dir(cwd)
        .status()
        .map_err(|source| CxError::CommandSpawn {
            command: display_os(&invocation.command[0]),
            source,
        })?;

    if !status.success() {
        return Ok(command_exit_code(status));
    }

    let output_hashes = collect_required_output_hashes(cwd, &invocation.outputs)?;
    let record = StalenessRecord {
        command_hash: identity,
        inputs: input_stamps,
        outputs: output_hashes,
    };
    state.insert(key, record);
    save_state(&state_path, &state)?;

    Ok(SUCCESS)
}

/// Returns the exit code that cx gives for a command that failed with STATUS.
///
/// When the command exited, it is the code of the command, with all its bits. Unix keeps only
/// 8 bits of a code, and other systems, such as Windows, keep more. When a signal stopped the
/// command, it is 128 plus the number of the signal, as a shell gives it. For example, it is
/// 143 for `SIGTERM`. When the status gives neither, it is 1.
fn command_exit_code(status: ExitStatus) -> i32 {
    exit_code_from_parts(status.code(), exit_signal(&status))
}

/// Returns the exit code of a failed command from its CODE and the SIGNAL that stopped it.
///
/// A code of 0 becomes 1, so that a failure never gives a success.
fn exit_code_from_parts(code: Option<i32>, signal: Option<i32>) -> i32 {
    match (code, signal) {
        (Some(0), _) => FAILURE,
        (Some(code), _) => code,
        (None, Some(signal)) => 128 + signal,
        (None, None) => FAILURE,
    }
}

/// Returns the number of the signal that stopped the process of STATUS, on Unix.
#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

/// Returns no signal, because systems other than Unix do not stop a process with a signal.
#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

/// Runs cx with the arguments and the current folder of the process, and returns the exit
/// code of `run`.
pub fn run_env() -> i32 {
    run(
        env::args_os().skip(1),
        &env::current_dir().unwrap_or_else(|_| ".".into()),
    )
}

/// Prints the version and the usage.
fn print_help() {
    println!(
        "cx {}\nConditional eXecution for linewise just commands.\n\nUSAGE:\n    cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND [ARG]...\n    cx graph\n    cx lint\n    cx --version",
        env!("CARGO_PKG_VERSION")
    );
}

/// Stamps each input file.
///
/// When the modification time and size equal the stamp in PREVIOUS, the old stamp is kept and
/// the file is not read. Otherwise the file is hashed.
///
/// # Errors
///
/// Returns `CxError::InputMetadata` when the metadata of an input cannot be read, for example
/// because it is missing, the errors of `PartialStamp::from_metadata`, and the errors of
/// `hash_file`.
fn collect_input_stamps(
    cwd: &Path,
    inputs: &[String],
    previous: Option<&StalenessRecord>,
) -> Result<BTreeMap<String, FileStamp>, CxError> {
    let mut stamps = BTreeMap::new();

    for input in inputs {
        let path = resolve(cwd, input);
        let metadata = fs::metadata(&path).map_err(|source| CxError::InputMetadata {
            path: input.clone(),
            source,
        })?;
        let partial = PartialStamp::from_metadata(&metadata)?;

        let stamp = match previous.and_then(|record| record.inputs.get(input)) {
            Some(old) if old.mtime_ns == partial.mtime_ns && old.size == partial.size => {
                old.clone()
            }
            _ => FileStamp {
                mtime_ns: partial.mtime_ns,
                size: partial.size,
                hash: hash_file(&path)?,
            },
        };

        stamps.insert(input.clone(), stamp);
    }

    Ok(stamps)
}

/// Hashes each output file that exists, and skips the missing ones.
///
/// # Errors
///
/// Returns the errors of `hash_file`, for example for an output that is a folder.
fn collect_existing_output_hashes(
    cwd: &Path,
    outputs: &[String],
) -> Result<BTreeMap<String, String>, CxError> {
    let mut hashes = BTreeMap::new();

    for output in outputs {
        let path = resolve(cwd, output);
        if path.exists() {
            hashes.insert(output.clone(), hash_file(&path)?);
        }
    }

    Ok(hashes)
}

/// Hashes each output file.
///
/// # Errors
///
/// Returns `CxError::MissingOutput` for the first output that does not exist, and the errors of
/// `hash_file`.
fn collect_required_output_hashes(
    cwd: &Path,
    outputs: &[String],
) -> Result<BTreeMap<String, String>, CxError> {
    let mut hashes = BTreeMap::new();

    for output in outputs {
        let path = resolve(cwd, output);
        if !path.exists() {
            return Err(CxError::MissingOutput {
                path: output.clone(),
            });
        }
        hashes.insert(output.clone(), hash_file(&path)?);
    }

    Ok(hashes)
}

/// The modification time and size of a file, before its hash.
#[derive(Debug)]
struct PartialStamp {
    /// The modification time, in nanoseconds since the Unix epoch.
    mtime_ns: u64,
    /// The size in bytes.
    size: u64,
}

impl PartialStamp {
    /// Reads the modification time and size from METADATA.
    ///
    /// # Errors
    ///
    /// Returns `CxError::FileTime` when the time is not available, and `CxError::BeforeUnixEpoch`
    /// when it is before the Unix epoch.
    fn from_metadata(metadata: &fs::Metadata) -> Result<Self, CxError> {
        let modified = metadata.modified().map_err(CxError::FileTime)?;
        let duration = modified
            .duration_since(UNIX_EPOCH)
            .map_err(|_| CxError::BeforeUnixEpoch)?;
        let mtime_ns = duration
            .as_secs()
            .saturating_mul(1_000_000_000)
            .saturating_add(u64::from(duration.subsec_nanos()));

        Ok(Self {
            mtime_ns,
            size: metadata.len(),
        })
    }
}

/// Reads the state file at PATH. A missing file gives an empty state.
///
/// # Errors
///
/// Returns `CxError::StateRead` when the file exists but cannot be read, and
/// `CxError::StateJson` when it is not a valid state.
fn load_state(path: &Path) -> Result<State, CxError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(CxError::StateJson),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(State::default()),
        Err(error) => Err(CxError::StateRead {
            path: path.to_path_buf(),
            source: error,
        }),
    }
}

/// Writes the state to PATH.
///
/// It writes a temporary file in the `tmp` folder next to PATH, then renames it to PATH.
///
/// # Errors
///
/// Returns `CxError::StatePath` when PATH has no parent folder, `CxError::StateJson` when the
/// state cannot be serialized, and `CxError::StateWrite` when the folder, the temporary file or
/// the rename fails.
fn save_state(path: &Path, state: &State) -> Result<(), CxError> {
    let state_dir = path.parent().ok_or(CxError::StatePath)?;
    let tmp_dir = state_dir.join("tmp");
    fs::create_dir_all(&tmp_dir).map_err(|source| CxError::StateWrite {
        path: tmp_dir.clone(),
        source,
    })?;

    let tmp_path = tmp_dir.join(format!("state.json.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(state).map_err(CxError::StateJson)?;

    {
        let mut file = fs::File::create(&tmp_path).map_err(|source| CxError::StateWrite {
            path: tmp_path.clone(),
            source,
        })?;
        file.write_all(&bytes)
            .map_err(|source| CxError::StateWrite {
                path: tmp_path.clone(),
                source,
            })?;
    }

    fs::rename(&tmp_path, path).map_err(|source| CxError::StateWrite {
        path: path.to_path_buf(),
        source,
    })
}

/// The SHA-256 hex digest of the content of the file at PATH, read in blocks of 8 KiB.
///
/// # Errors
///
/// Returns `CxError::FileOpen` when the file cannot be opened, and `CxError::FileRead` when it
/// cannot be read.
fn hash_file(path: &Path) -> Result<String, CxError> {
    let mut file = fs::File::open(path).map_err(|source| CxError::FileOpen {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let read = file.read(&mut buffer).map_err(|source| CxError::FileRead {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// The SHA-256 hex digest of TEXT.
fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// PATH resolved against CWD. An absolute PATH stays as it is.
fn resolve(cwd: &Path, path: &str) -> PathBuf {
    cwd.join(path)
}

/// Adds VALUE to the end of VALUES when VALUES does not hold it yet.
fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

/// References to VALUES, sorted.
fn sorted(values: &[String]) -> Vec<&String> {
    let mut values: Vec<&String> = values.iter().collect();
    values.sort();
    values
}

/// Converts an argument to a String.
///
/// # Errors
///
/// Returns a message that names CONTEXT when the argument is not UTF-8.
fn os_string_to_string(value: &OsString, context: &str) -> Result<String, String> {
    value
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{context} must be valid UTF-8"))
}

/// Converts an OS string to text, and replaces the parts that are not UTF-8.
fn display_os(value: &OsString) -> String {
    value.to_string_lossy().into_owned()
}

/// True when one of ATTRIBUTES is `[script]` or `[script(...)]`.
///
/// The dump gives `[script]` as `{"script": null}` and `[script("sh")]` as
/// `{"script": {...}}`. A bare string `"script"` also counts. Another attribute does not
/// count, even when its argument holds the word, such as `[group("scripts")]`.
fn has_script_attribute(attributes: &[Value]) -> bool {
    attributes.iter().any(|attribute| match attribute {
        Value::String(name) => name == "script",
        Value::Object(map) => map.contains_key("script"),
        _ => false,
    })
}

/// Prints MESSAGE with the prefix `cx: ` on stderr, and returns a failure.
fn fail(message: String) -> i32 {
    eprintln!("cx: {message}");
    FAILURE
}

/// An error of cx. Its `Display` gives the message that cx prints.
#[derive(Debug)]
enum CxError {
    /// The modification time of a file is before the Unix epoch.
    BeforeUnixEpoch,
    /// The command of a cx line could not start.
    CommandSpawn {
        /// The program name of the command.
        command: String,
        /// The error of the spawn.
        source: io::Error,
    },
    /// A file could not be opened for hashing.
    FileOpen {
        /// The path of the file.
        path: PathBuf,
        /// The error of the open.
        source: io::Error,
    },
    /// A file could not be read for hashing.
    FileRead {
        /// The path of the file.
        path: PathBuf,
        /// The error of the read.
        source: io::Error,
    },
    /// The modification time of a file is not available.
    FileTime(io::Error),
    /// The metadata of an input file could not be read, for example because it is missing.
    InputMetadata {
        /// The input path as the `--in` option gave it.
        path: String,
        /// The error of the metadata call.
        source: io::Error,
    },
    /// `just --dump --dump-format json` exited with a failure.
    JustDump {
        /// The stderr of just.
        stderr: String,
    },
    /// `just` could not start.
    JustSpawn(io::Error),
    /// The command succeeded but did not make an output file.
    MissingOutput {
        /// The output path as the `--out` option gave it.
        path: String,
    },
    /// The state could not be parsed or serialized as JSON.
    StateJson(serde_json::Error),
    /// The state path has no parent folder.
    StatePath,
    /// The state file could not be read.
    StateRead {
        /// The path of the state file.
        path: PathBuf,
        /// The error of the read.
        source: io::Error,
    },
    /// The state file or its temporary file could not be written.
    StateWrite {
        /// The path that could not be written: the `tmp` folder, the temporary file or the
        /// state file.
        path: PathBuf,
        /// The error of the write.
        source: io::Error,
    },
    /// The dump of just could not be parsed, or the graph could not be serialized, as JSON.
    StaticJson(serde_json::Error),
}

impl std::fmt::Display for CxError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeUnixEpoch => write!(formatter, "file timestamp is before the Unix epoch"),
            Self::CommandSpawn { command, source } => {
                write!(formatter, "failed to run command {command:?}: {source}")
            }
            Self::FileOpen { path, source } => {
                write!(formatter, "failed to open {}: {source}", path.display())
            }
            Self::FileRead { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::FileTime(source) => write!(formatter, "failed to read file timestamp: {source}"),
            Self::InputMetadata { path, source } => {
                write!(formatter, "failed to stat input {path:?}: {source}")
            }
            Self::JustDump { stderr } => {
                write!(formatter, "just --dump --dump-format json failed: {stderr}")
            }
            Self::JustSpawn(source) => {
                write!(
                    formatter,
                    "failed to run just --dump --dump-format json: {source}"
                )
            }
            Self::MissingOutput { path } => {
                write!(
                    formatter,
                    "command succeeded but did not create output {path:?}"
                )
            }
            Self::StateJson(source) => write!(formatter, "failed to parse cx state: {source}"),
            Self::StatePath => write!(formatter, "state path has no parent directory"),
            Self::StateRead { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::StateWrite { path, source } => {
                write!(formatter, "failed to write {}: {source}", path.display())
            }
            Self::StaticJson(source) => {
                write!(formatter, "failed to parse static graph JSON: {source}")
            }
        }
    }
}

impl std::error::Error for CxError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_keeps_every_bit_of_the_code() {
        assert_eq!(exit_code_from_parts(Some(3), None), 3);
        assert_eq!(exit_code_from_parts(Some(256), None), 256);
        assert_eq!(exit_code_from_parts(Some(-1073741819), None), -1073741819);
    }

    #[test]
    fn exit_code_of_a_signal_is_128_plus_the_signal() {
        assert_eq!(exit_code_from_parts(None, Some(15)), 143);
        assert_eq!(exit_code_from_parts(None, Some(9)), 137);
    }

    #[test]
    fn exit_code_of_a_failure_is_never_0() {
        assert_eq!(exit_code_from_parts(Some(0), None), 1);
        assert_eq!(exit_code_from_parts(None, None), 1);
    }

    #[test]
    fn parse_keeps_each_path_once() {
        let args = [
            "--in", "a", "--in", "b", "--in", "a", "--out", "o", "--out", "o", "--", "x",
        ];
        let invocation =
            RuntimeInvocation::parse(args.iter().map(OsString::from).collect()).expect("parse");
        assert_eq!(invocation.inputs, ["a", "b"]);
        assert_eq!(invocation.outputs, ["o"]);
    }

    #[test]
    fn first_word_is_cx_reads_the_shell_word() {
        assert!(first_word_is_cx("cx --out 'open"));
        assert!(first_word_is_cx("\"cx\" --out 'open"));
        assert!(!first_word_is_cx("cxfoo 'open"));
        assert!(!first_word_is_cx("cx-tool 'open"));
        assert!(!first_word_is_cx("'cx foo' 'open"));
        assert!(!first_word_is_cx("\"cx 'open"));
        assert!(!first_word_is_cx(""));
        assert!(first_word_is_cx("cx\t--out 'open"));
        // A no-break space is part of a shell word, so the word is "cx\u{a0}x", not cx.
        assert!(!first_word_is_cx("cx\u{a0}x 'open"));
    }

    #[test]
    fn script_attribute_is_matched_by_name() {
        let attributes = |text: &str| -> Vec<Value> { serde_json::from_str(text).unwrap() };
        assert!(has_script_attribute(&attributes(r#"[{"script": null}]"#)));
        assert!(has_script_attribute(&attributes(
            r#"[{"script": {"command": "sh"}}]"#
        )));
        assert!(has_script_attribute(&attributes(r#"["script"]"#)));
        assert!(!has_script_attribute(&attributes(
            r#"[{"group": "scripts"}]"#
        )));
        assert!(!has_script_attribute(&attributes(r#"[{"doc": "script"}]"#)));
        assert!(!has_script_attribute(&attributes(r#"["no-script"]"#)));
        assert!(!has_script_attribute(&attributes(
            r#"[null, 1, ["script"]]"#
        )));
    }
}
