use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::UNIX_EPOCH;

pub fn run<I, S>(args: I, cwd: &Path) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();

    match args.first().and_then(|arg| arg.to_str()) {
        None | Some("--help") | Some("-h") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("graph") => run_static(StaticMode::Graph, cwd),
        Some("lint") => run_static(StaticMode::Lint, cwd),
        Some(_) => match RuntimeInvocation::parse(args) {
            Ok(invocation) => run_runtime(invocation, cwd),
            Err(message) => fail(message),
        },
    }
}

#[derive(Debug, Clone)]
struct RuntimeInvocation {
    inputs: Vec<String>,
    outputs: Vec<String>,
    command: Vec<OsString>,
}

impl RuntimeInvocation {
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
                    inputs.push(os_string_to_string(value, "--in PATH")?);
                }
                "--out" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "--out requires a PATH operand".to_string())?;
                    outputs.push(os_string_to_string(value, "--out PATH")?);
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

    fn key(&self) -> String {
        let mut outputs = self.outputs.clone();
        outputs.sort();
        outputs.join("\n")
    }

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileStamp {
    mtime_ns: u64,
    size: u64,
    hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StalenessRecord {
    command_hash: String,
    inputs: BTreeMap<String, FileStamp>,
    outputs: BTreeMap<String, String>,
}

type State = BTreeMap<String, StalenessRecord>;

enum StaticMode {
    Graph,
    Lint,
}

#[derive(Debug, Serialize)]
struct StaticGraph {
    source: Option<String>,
    lines: Vec<StaticCxLine>,
    violations: Vec<LintViolation>,
}

#[derive(Debug, Serialize)]
struct StaticCxLine {
    recipe: String,
    line: usize,
    body: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    command: Vec<String>,
    command_hash: String,
}

struct StaticCxArgs {
    inputs: Vec<String>,
    outputs: Vec<String>,
    command: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LintViolation {
    recipe: String,
    line: usize,
    message: String,
}

#[derive(Debug, Deserialize)]
struct JustDump {
    source: Option<String>,
    recipes: BTreeMap<String, JustRecipe>,
}

#[derive(Debug, Deserialize)]
struct JustRecipe {
    #[serde(default)]
    attributes: Vec<Value>,
    #[serde(default)]
    body: Vec<Vec<Value>>,
    #[serde(default)]
    shebang: bool,
}

fn run_runtime(invocation: RuntimeInvocation, cwd: &Path) -> ExitCode {
    match run_runtime_inner(invocation, cwd) {
        Ok(code) => code,
        Err(error) => fail(error.to_string()),
    }
}

fn run_static(mode: StaticMode, cwd: &Path) -> ExitCode {
    match run_static_inner(mode, cwd) {
        Ok(code) => code,
        Err(error) => fail(error.to_string()),
    }
}

fn run_static_inner(mode: StaticMode, cwd: &Path) -> Result<ExitCode, CxError> {
    let graph = build_static_graph(cwd)?;

    match mode {
        StaticMode::Graph => {
            let json = serde_json::to_string_pretty(&graph).map_err(CxError::StaticJson)?;
            println!("{json}");
            Ok(ExitCode::SUCCESS)
        }
        StaticMode::Lint if graph.violations.is_empty() => Ok(ExitCode::SUCCESS),
        StaticMode::Lint => {
            for violation in &graph.violations {
                eprintln!(
                    "lint error: {}:{}: {}",
                    violation.recipe, violation.line, violation.message
                );
            }
            Ok(ExitCode::FAILURE)
        }
    }
}

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

enum StaticLineParse {
    NotCx,
    Line(StaticCxLine),
    Violation(LintViolation),
}

fn parse_static_cx_line(recipe: &str, line: usize, body: &str) -> StaticLineParse {
    let trimmed = body.trim_start();
    let without_quiet = trimmed
        .strip_prefix('@')
        .map(str::trim_start)
        .unwrap_or(trimmed);

    let tokens = match shell_words::split(without_quiet) {
        Ok(tokens) => tokens,
        Err(error) => {
            if without_quiet.starts_with("cx") {
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

fn render_body_fragments(fragments: &[Value]) -> String {
    fragments.iter().map(render_body_fragment).collect()
}

fn render_body_fragment(fragment: &Value) -> String {
    match fragment {
        Value::String(text) => text.clone(),
        Value::Array(parts) => format!("{{{{{}}}}}", render_expression(parts)),
        other => other.to_string(),
    }
}

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

fn render_expression_part(part: &Value) -> String {
    match part {
        Value::String(text) => text.clone(),
        Value::Array(parts) => render_expression(parts),
        other => other.to_string(),
    }
}

fn run_runtime_inner(invocation: RuntimeInvocation, cwd: &Path) -> Result<ExitCode, CxError> {
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
        return Ok(ExitCode::SUCCESS);
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
        return Ok(ExitCode::from(status.code().unwrap_or(1) as u8));
    }

    let output_hashes = collect_required_output_hashes(cwd, &invocation.outputs)?;
    let record = StalenessRecord {
        command_hash: identity,
        inputs: input_stamps,
        outputs: output_hashes,
    };
    state.insert(key, record);
    save_state(&state_path, &state)?;

    Ok(ExitCode::SUCCESS)
}

pub fn run_env() -> ExitCode {
    run(
        env::args_os().skip(1),
        &env::current_dir().unwrap_or_else(|_| ".".into()),
    )
}

fn print_help() {
    println!(
        "cx 0.1.0\nConditional eXecution for linewise just commands.\n\nUSAGE:\n    cx [--in PATH]... [--out PATH]... -- COMMAND [ARG]...\n    cx graph\n    cx lint"
    );
}

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
            Some(old) => {
                let hash = hash_file(&path)?;
                FileStamp {
                    mtime_ns: partial.mtime_ns,
                    size: partial.size,
                    hash,
                }
                .reuse_hash_if_same(old)
            }
            None => FileStamp {
                mtime_ns: partial.mtime_ns,
                size: partial.size,
                hash: hash_file(&path)?,
            },
        };

        stamps.insert(input.clone(), stamp);
    }

    Ok(stamps)
}

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

#[derive(Debug)]
struct PartialStamp {
    mtime_ns: u64,
    size: u64,
}

impl PartialStamp {
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

impl FileStamp {
    fn reuse_hash_if_same(self, old: &FileStamp) -> Self {
        if self.hash == old.hash {
            Self {
                hash: old.hash.clone(),
                ..self
            }
        } else {
            self
        }
    }
}

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

fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn resolve(cwd: &Path, path: &str) -> PathBuf {
    cwd.join(path)
}

fn sorted(values: &[String]) -> Vec<&String> {
    let mut values: Vec<&String> = values.iter().collect();
    values.sort();
    values
}

fn os_string_to_string(value: &OsString, context: &str) -> Result<String, String> {
    value
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{context} must be valid UTF-8"))
}

fn display_os(value: &OsString) -> String {
    value.to_string_lossy().into_owned()
}

fn has_script_attribute(attributes: &[Value]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.to_string().contains("script"))
}

fn fail(message: String) -> ExitCode {
    eprintln!("cx: {message}");
    ExitCode::FAILURE
}

#[derive(Debug)]
enum CxError {
    BeforeUnixEpoch,
    CommandSpawn { command: String, source: io::Error },
    FileOpen { path: PathBuf, source: io::Error },
    FileRead { path: PathBuf, source: io::Error },
    FileTime(io::Error),
    InputMetadata { path: String, source: io::Error },
    JustDump { stderr: String },
    JustSpawn(io::Error),
    MissingOutput { path: String },
    StateJson(serde_json::Error),
    StatePath,
    StateRead { path: PathBuf, source: io::Error },
    StateWrite { path: PathBuf, source: io::Error },
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
