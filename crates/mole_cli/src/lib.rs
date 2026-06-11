use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs, io,
    ops::Range,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use mole_devtool::ParityLedgerViewModel;
use mole_ledger::LedgerMap;

mod decomp;
mod formatting;
mod frame_data;
mod frame_data_sampler;
mod generated;
mod graph;
mod ledger_map;
mod package;
mod replay;
mod stage_assets;
mod value_sheets;
mod verify;

pub use verify::verification_plan_for_changed_paths;

pub(crate) const SCHEMA_VERSION: u8 = 1;
const EXPECTED_BRANCH: &str = "handoff/rust-rollback-architecture";
const EXPECTED_REMOTE: &str = "https://github.com/Robbiemas/First_Game.git";
pub(crate) const MESSAGE_BOARD_PATH: &str = "MOLE_CLI_AGENT_MESSAGES.md";
const MACRO_PLAN_PATH: &str =
    "docs/superpowers/plans/2026-05-31-human-noticeable-melee-parity-macro-plan.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    command: CliCommand,
    root: PathBuf,
    output: OutputMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    Status,
    Parity,
    ParityGaps,
    Snapshot,
    Agent(AgentCommand),
    Graph(GraphCommand),
    Verify(VerifyCommand),
    Generated(GeneratedCommand),
    Devtool(DevtoolCommand),
    Finish(FinishCommand),
    Replay(ReplayCommand),
    Decomp(DecompCommand),
    FrameData(FrameDataCommand),
    Package(PackageCommand),
    FriendConnect(FriendConnectCommand),
    Doctor,
    Tests,
    Handoff,
    RecommendNext,
    Request(RequestCommand),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentCommand {
    Brief,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphCommand {
    Missing,
    Next,
    Inspect { target: String },
    Layout,
    LayoutSave { write: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VerifyCommand {
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DevtoolCommand {
    Ledger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GeneratedCommand {
    Check,
    WriteValueSheets { write: bool },
    WriteStageAsset { stage: String, write: bool },
    WriteLedgerMap { write: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FinishCommand {
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplayCommand {
    Artifacts,
    Check(ReplayCheckOptions),
    Trace(ReplayTraceOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecompCommand {
    Search(DecompSearchOptions),
    Show(DecompShowOptions),
    Symbol(DecompSearchOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameDataCommand {
    Extract(FrameDataOptions),
    ExtractAll(FrameDataBatchOptions),
    ExportRuntime(FrameDataExportOptions),
    ExportRuntimeAll(FrameDataExportBatchOptions),
    Sample(FrameDataSampleOptions),
    Show(FrameDataOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageCommand {
    FriendPlaytest { verify: bool, dry_run: bool },
    LocalInternetPlaytest { verify: bool, dry_run: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FriendConnectCommand {
    Status,
    Diagnostics { log: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameDataOptions {
    pub character: String,
    pub source_character: Option<String>,
    pub source_state: Option<String>,
    pub state: String,
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameDataExportOptions {
    pub character: String,
    pub state: String,
    pub output: Option<String>,
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameDataBatchOptions {
    pub character: String,
    pub source_character: String,
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameDataExportBatchOptions {
    pub character: String,
    pub output: Option<String>,
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameDataSampleOptions {
    pub character: String,
    pub source_character: Option<String>,
    pub state: String,
    pub frame: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecompSearchOptions {
    pub query: String,
    pub limit: usize,
    pub decomp_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecompShowOptions {
    pub path: String,
    pub line: usize,
    pub context: usize,
    pub decomp_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayCheckOptions {
    pub replay: Option<String>,
    pub inputs: Option<String>,
    pub frames: usize,
    pub include_negative_frames: bool,
    pub mode: ReplayCheckMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayTraceOptions {
    pub inputs: String,
    pub frames: usize,
    pub player_index: usize,
    pub source_frame_start: i32,
    pub source_frame_end: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplayCheckMode {
    MatchStart,
    Seeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RequestCommand {
    List,
    Next,
    Add {
        id: Option<String>,
        title: String,
        request: String,
        context: String,
        expected: String,
    },
    Done {
        id: String,
        result: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputMode {
    Json,
    Text,
    Markdown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GitStatusSummary {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub remote_origin: Option<String>,
    pub expected_branch: String,
    pub expected_remote: String,
    pub branch_ok: bool,
    pub remote_ok: bool,
    pub dirty: bool,
    pub modified: usize,
    pub deleted: usize,
    pub untracked: usize,
    pub renamed: usize,
    pub other: usize,
    pub total_changed: usize,
    pub sample: Vec<String>,
    pub git_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParitySummary {
    pub value_sections: BTreeMap<String, ValueSectionSummary>,
    pub value_total_rows: usize,
    pub value_matches: usize,
    pub value_actionable: usize,
    pub value_derived: usize,
    pub ecb_mapped_motion_states: usize,
    pub ecb_missing_sampled_mappings: usize,
    pub ecb_unmapped_derived_states: Vec<String>,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub graph_status_counts: BTreeMap<String, usize>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValueSectionSummary {
    pub rows: usize,
    pub matches: usize,
    pub actionable: usize,
    pub derived: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GeneratedArtifactStatus {
    pub path: String,
    pub exists: bool,
    pub dirty: bool,
    pub git_status: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MessageBoardRequest {
    pub id: String,
    pub title: String,
    pub request: String,
    pub context: String,
    pub expected_output: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MessageBoardSummary {
    pub path: String,
    pub inbox_count: usize,
    pub completed_count: usize,
    pub inbox: Vec<MessageBoardRequest>,
    pub completed: Vec<MessageBoardRequest>,
    pub errors: Vec<String>,
}

pub fn run_cli(args: &[String]) -> Result<String, String> {
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    let options = parse_args(args, &cwd)?;
    let report = command_report(&options);
    Ok(match options.output {
        OutputMode::Json => serde_json::to_string_pretty(&report).expect("report is serializable"),
        OutputMode::Text => formatting::format_text_report(&report),
        OutputMode::Markdown => formatting::format_markdown_report(&report),
    })
}

pub fn parse_args(args: &[String], cwd: &Path) -> Result<CliOptions, String> {
    let mut root: Option<PathBuf> = None;
    let mut output = OutputMode::Json;
    let mut positional = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => output = OutputMode::Json,
            "--text" => output = OutputMode::Text,
            "--format" => {
                index += 1;
                let Some(format) = args.get(index) else {
                    return Err("--format requires json, text, or markdown".to_string());
                };
                output = match format.as_str() {
                    "json" => OutputMode::Json,
                    "text" => OutputMode::Text,
                    "markdown" => OutputMode::Markdown,
                    _ => return Err("--format requires json, text, or markdown".to_string()),
                };
            }
            "--root" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--root requires a path".to_string());
                };
                root = Some(PathBuf::from(path));
            }
            "-h" | "--help" => positional.push("help".to_string()),
            value => positional.push(value.to_string()),
        }
        index += 1;
    }

    let command = parse_command(&positional)?;
    let root = match root {
        Some(path) => path,
        None => find_project_root(cwd).ok_or_else(|| {
            format!(
                "could not find First_Game project root from {}",
                cwd.display()
            )
        })?,
    };

    Ok(CliOptions {
        command,
        root,
        output,
    })
}

fn parse_command(positional: &[String]) -> Result<CliCommand, String> {
    let Some(command) = positional.first().map(String::as_str) else {
        return Ok(CliCommand::Status);
    };

    match command {
        "status" => ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Status),
        "parity" => {
            if positional.get(1).map(String::as_str) == Some("snapshot") {
                ensure_no_extra_args("parity snapshot", &positional[2..])
                    .map(|()| CliCommand::Snapshot)
            } else if positional.get(1).map(String::as_str) == Some("gaps") {
                ensure_no_extra_args("parity gaps", &positional[2..])
                    .map(|()| CliCommand::ParityGaps)
            } else {
                ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Parity)
            }
        }
        "snapshot" => {
            ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Snapshot)
        }
        "agent" => parse_agent_command(&positional[1..]).map(CliCommand::Agent),
        "graph" => parse_graph_command(&positional[1..]).map(CliCommand::Graph),
        "verify" => parse_verify_command(&positional[1..]).map(CliCommand::Verify),
        "generated" => parse_generated_command(&positional[1..]).map(CliCommand::Generated),
        "devtool" => parse_devtool_command(&positional[1..]).map(CliCommand::Devtool),
        "finish" => parse_finish_command(&positional[1..]).map(CliCommand::Finish),
        "replay" => parse_replay_command(&positional[1..]).map(CliCommand::Replay),
        "decomp" => parse_decomp_command(&positional[1..]).map(CliCommand::Decomp),
        "frame-data" => parse_frame_data_command(&positional[1..]).map(CliCommand::FrameData),
        "package" => parse_package_command(&positional[1..]).map(CliCommand::Package),
        "friend-connect" => {
            parse_friend_connect_command(&positional[1..]).map(CliCommand::FriendConnect)
        }
        "doctor" => ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Doctor),
        "tests" => ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Tests),
        "handoff" => ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Handoff),
        "recommend-next" => {
            ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::RecommendNext)
        }
        "request" => parse_request_command(&positional[1..]).map(CliCommand::Request),
        "help" => ensure_no_extra_args(command, &positional[1..]).map(|()| CliCommand::Help),
        other => Err(format!("unknown mole command: {other}")),
    }
}

fn parse_friend_connect_command(args: &[String]) -> Result<FriendConnectCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("status");
    let rest = subcommand_args(args);
    match subcommand {
        "status" => ensure_no_extra_args("friend-connect status", rest)
            .map(|()| FriendConnectCommand::Status),
        "diagnostics" => parse_friend_connect_diagnostics(rest),
        other => Err(format!("unknown mole friend-connect command: {other}")),
    }
}

fn parse_friend_connect_diagnostics(args: &[String]) -> Result<FriendConnectCommand, String> {
    let mut log = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--log" => {
                log = Some(take_flag_value(args, &mut index, "--log")?);
            }
            other => {
                return Err(format!(
                    "unexpected argument for friend-connect diagnostics: {other}"
                ))
            }
        }
        index += 1;
    }
    Ok(FriendConnectCommand::Diagnostics { log })
}

fn parse_package_command(args: &[String]) -> Result<PackageCommand, String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("package requires a subcommand".to_string());
    };
    match subcommand {
        "friend-playtest" | "local-internet-playtest" => {
            let mut verify = true;
            let mut dry_run = false;
            for arg in &args[1..] {
                match arg.as_str() {
                    "--verify" => verify = true,
                    "--no-verify" => verify = false,
                    "--dry-run" => dry_run = true,
                    other => {
                        return Err(format!(
                            "unexpected argument for package {subcommand}: {other}"
                        ))
                    }
                }
            }
            if subcommand == "friend-playtest" {
                Ok(PackageCommand::FriendPlaytest { verify, dry_run })
            } else {
                Ok(PackageCommand::LocalInternetPlaytest { verify, dry_run })
            }
        }
        other => Err(format!("unknown mole package command: {other}")),
    }
}

fn parse_decomp_command(args: &[String]) -> Result<DecompCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("search");
    let rest = subcommand_args(args);
    match subcommand {
        "search" => parse_decomp_search(rest).map(DecompCommand::Search),
        "symbol" => parse_decomp_search(rest).map(DecompCommand::Symbol),
        "show" => parse_decomp_show(rest).map(DecompCommand::Show),
        other => Err(format!("unknown mole decomp command: {other}")),
    }
}

fn parse_decomp_search(args: &[String]) -> Result<DecompSearchOptions, String> {
    let mut query_parts = Vec::new();
    let mut limit = 20usize;
    let mut decomp_root = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                limit =
                    parse_positive_usize(&take_flag_value(args, &mut index, "--limit")?, "--limit")?
            }
            "--decomp-root" => {
                decomp_root = Some(take_flag_value(args, &mut index, "--decomp-root")?)
            }
            other if other.starts_with("--") => {
                return Err(format!("unexpected argument for decomp search: {other}"))
            }
            other => query_parts.push(other.to_string()),
        }
        index += 1;
    }

    let query = query_parts.join(" ");
    if query.trim().is_empty() {
        return Err("decomp search requires a query".to_string());
    }

    Ok(DecompSearchOptions {
        query,
        limit,
        decomp_root,
    })
}

fn parse_decomp_show(args: &[String]) -> Result<DecompShowOptions, String> {
    let mut path = None;
    let mut line = 1usize;
    let mut context = 20usize;
    let mut decomp_root = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--line" => {
                line =
                    parse_positive_usize(&take_flag_value(args, &mut index, "--line")?, "--line")?
            }
            "--context" => {
                context = parse_positive_usize(
                    &take_flag_value(args, &mut index, "--context")?,
                    "--context",
                )?
            }
            "--decomp-root" => {
                decomp_root = Some(take_flag_value(args, &mut index, "--decomp-root")?)
            }
            other if other.starts_with("--") => {
                return Err(format!("unexpected argument for decomp show: {other}"))
            }
            other => {
                if path.is_some() {
                    return Err(format!("unexpected argument for decomp show: {other}"));
                }
                path = Some(other.to_string());
            }
        }
        index += 1;
    }

    Ok(DecompShowOptions {
        path: path.ok_or_else(|| "decomp show requires a path".to_string())?,
        line,
        context,
        decomp_root,
    })
}

fn parse_frame_data_command(args: &[String]) -> Result<FrameDataCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("show");
    let rest = subcommand_args(args);
    match subcommand {
        "extract" if has_flag(rest, "--all-states") => {
            parse_frame_data_batch_options(rest).map(FrameDataCommand::ExtractAll)
        }
        "extract" => parse_frame_data_options(rest, true).map(FrameDataCommand::Extract),
        "export-runtime" if has_flag(rest, "--all-states") => {
            parse_frame_data_export_batch_options(rest).map(FrameDataCommand::ExportRuntimeAll)
        }
        "export-runtime" => {
            parse_frame_data_export_options(rest).map(FrameDataCommand::ExportRuntime)
        }
        "sample" => parse_frame_data_sample_options(rest).map(FrameDataCommand::Sample),
        "show" => parse_frame_data_options(rest, false).map(FrameDataCommand::Show),
        other => Err(format!("unknown mole frame-data command: {other}")),
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn parse_frame_data_options(
    args: &[String],
    allow_source_character: bool,
) -> Result<FrameDataOptions, String> {
    let mut character = None;
    let mut source_character = None;
    let mut source_state = None;
    let mut state = None;
    let mut write = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--character" => character = Some(take_flag_value(args, &mut index, "--character")?),
            "--source-character" if allow_source_character => {
                source_character = Some(take_flag_value(args, &mut index, "--source-character")?)
            }
            "--source-state" if allow_source_character => {
                source_state = Some(take_flag_value(args, &mut index, "--source-state")?)
            }
            "--state" => state = Some(take_flag_value(args, &mut index, "--state")?),
            "--write" if allow_source_character => write = true,
            other => return Err(format!("unexpected argument for frame-data: {other}")),
        }
        index += 1;
    }
    Ok(FrameDataOptions {
        character: character.ok_or_else(|| "frame-data requires --character <id>".to_string())?,
        source_character,
        source_state,
        state: state.ok_or_else(|| "frame-data requires --state <MotionState>".to_string())?,
        write,
    })
}

fn parse_frame_data_batch_options(args: &[String]) -> Result<FrameDataBatchOptions, String> {
    let mut character = None;
    let mut source_character = None;
    let mut all_states = false;
    let mut write = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--all-states" => all_states = true,
            "--character" => character = Some(take_flag_value(args, &mut index, "--character")?),
            "--source-character" => {
                source_character = Some(take_flag_value(args, &mut index, "--source-character")?)
            }
            "--write" => write = true,
            other => {
                return Err(format!(
                    "unexpected argument for frame-data extract: {other}"
                ))
            }
        }
        index += 1;
    }
    if !all_states {
        return Err("frame-data extract --all-states requires --all-states".to_string());
    }
    Ok(FrameDataBatchOptions {
        character: character.ok_or_else(|| "frame-data requires --character <id>".to_string())?,
        source_character: source_character
            .ok_or_else(|| "frame-data requires --source-character <id>".to_string())?,
        write,
    })
}

fn parse_frame_data_export_options(args: &[String]) -> Result<FrameDataExportOptions, String> {
    let mut character = None;
    let mut state = None;
    let mut output = None;
    let mut write = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--character" => character = Some(take_flag_value(args, &mut index, "--character")?),
            "--state" => state = Some(take_flag_value(args, &mut index, "--state")?),
            "--output" => output = Some(take_flag_value(args, &mut index, "--output")?),
            "--write" => write = true,
            other => {
                return Err(format!(
                    "unexpected argument for frame-data export-runtime: {other}"
                ))
            }
        }
        index += 1;
    }
    Ok(FrameDataExportOptions {
        character: character.ok_or_else(|| "frame-data requires --character <id>".to_string())?,
        state: state.ok_or_else(|| "frame-data requires --state <MotionState>".to_string())?,
        output,
        write,
    })
}

fn parse_frame_data_export_batch_options(
    args: &[String],
) -> Result<FrameDataExportBatchOptions, String> {
    let mut character = None;
    let mut output = None;
    let mut all_states = false;
    let mut write = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--all-states" => all_states = true,
            "--character" => character = Some(take_flag_value(args, &mut index, "--character")?),
            "--output" => output = Some(take_flag_value(args, &mut index, "--output")?),
            "--write" => write = true,
            other => {
                return Err(format!(
                    "unexpected argument for frame-data export-runtime: {other}"
                ))
            }
        }
        index += 1;
    }
    if !all_states {
        return Err("frame-data export-runtime --all-states requires --all-states".to_string());
    }
    Ok(FrameDataExportBatchOptions {
        character: character.ok_or_else(|| "frame-data requires --character <id>".to_string())?,
        output,
        write,
    })
}

fn parse_frame_data_sample_options(args: &[String]) -> Result<FrameDataSampleOptions, String> {
    let mut character = None;
    let mut source_character = None;
    let mut state = None;
    let mut frame = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--character" => character = Some(take_flag_value(args, &mut index, "--character")?),
            "--source-character" => {
                source_character = Some(take_flag_value(args, &mut index, "--source-character")?)
            }
            "--state" => state = Some(take_flag_value(args, &mut index, "--state")?),
            "--frame" => {
                frame = Some(parse_positive_usize(
                    &take_flag_value(args, &mut index, "--frame")?,
                    "--frame",
                )? as u64)
            }
            other => {
                return Err(format!(
                    "unexpected argument for frame-data sample: {other}"
                ))
            }
        }
        index += 1;
    }
    Ok(FrameDataSampleOptions {
        character: character.ok_or_else(|| "frame-data requires --character <id>".to_string())?,
        source_character,
        state: state.ok_or_else(|| "frame-data requires --state <MotionState>".to_string())?,
        frame: frame.ok_or_else(|| "frame-data sample requires --frame <N>".to_string())?,
    })
}

fn parse_agent_command(args: &[String]) -> Result<AgentCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("brief");
    let rest = subcommand_args(args);
    match subcommand {
        "brief" => ensure_no_extra_args("agent brief", rest).map(|()| AgentCommand::Brief),
        other => Err(format!("unknown mole agent command: {other}")),
    }
}

fn parse_graph_command(args: &[String]) -> Result<GraphCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("missing");
    let rest = subcommand_args(args);
    match subcommand {
        "missing" => ensure_no_extra_args("graph missing", rest).map(|()| GraphCommand::Missing),
        "next" => ensure_no_extra_args("graph next", rest).map(|()| GraphCommand::Next),
        "layout" => parse_graph_layout_command(rest),
        "inspect" => {
            let target = rest.join(" ");
            if target.trim().is_empty() {
                Err("graph inspect requires a node id or edge target".to_string())
            } else {
                Ok(GraphCommand::Inspect { target })
            }
        }
        other => Err(format!("unknown mole graph command: {other}")),
    }
}

fn parse_graph_layout_command(args: &[String]) -> Result<GraphCommand, String> {
    if args.first().map(String::as_str) != Some("save") {
        return ensure_no_extra_args("graph layout", args).map(|()| GraphCommand::Layout);
    }

    let mut write = false;
    let mut check = false;
    for arg in &args[1..] {
        match arg.as_str() {
            "--check" => check = true,
            "--write" => write = true,
            other => {
                return Err(format!(
                    "unexpected argument for graph layout save: {other}"
                ))
            }
        }
    }
    if write && check {
        return Err("graph layout save accepts either --check or --write, not both".to_string());
    }
    Ok(GraphCommand::LayoutSave { write })
}

fn parse_verify_command(args: &[String]) -> Result<VerifyCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("changed");
    let rest = subcommand_args(args);
    match subcommand {
        "changed" => ensure_no_extra_args("verify changed", rest).map(|()| VerifyCommand::Changed),
        other => Err(format!("unknown mole verify command: {other}")),
    }
}

fn parse_generated_command(args: &[String]) -> Result<GeneratedCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("check");
    let rest = subcommand_args(args);
    match subcommand {
        "check" => ensure_no_extra_args("generated check", rest).map(|()| GeneratedCommand::Check),
        "write-value-sheets" => {
            let mut write = false;
            for arg in rest {
                match arg.as_str() {
                    "--write" => write = true,
                    other => {
                        return Err(format!(
                            "unexpected argument for generated write-value-sheets: {other}"
                        ))
                    }
                }
            }
            Ok(GeneratedCommand::WriteValueSheets { write })
        }
        "write-stage-asset" => {
            let mut stage = None;
            let mut write = false;
            let mut index = 0;
            while index < rest.len() {
                match rest[index].as_str() {
                    "--stage" => {
                        index += 1;
                        let Some(value) = rest.get(index) else {
                            return Err("--stage requires a stage id".to_string());
                        };
                        stage = Some(value.to_string());
                    }
                    "--write" => write = true,
                    other => {
                        return Err(format!(
                            "unexpected argument for generated write-stage-asset: {other}"
                        ))
                    }
                }
                index += 1;
            }
            Ok(GeneratedCommand::WriteStageAsset {
                stage: stage.ok_or_else(|| "--stage requires a stage id".to_string())?,
                write,
            })
        }
        "write-ledger-map" => {
            let mut write = false;
            for arg in rest {
                match arg.as_str() {
                    "--write" => write = true,
                    other => {
                        return Err(format!(
                            "unexpected argument for generated write-ledger-map: {other}"
                        ))
                    }
                }
            }
            Ok(GeneratedCommand::WriteLedgerMap { write })
        }
        other => Err(format!("unknown mole generated command: {other}")),
    }
}

fn parse_finish_command(args: &[String]) -> Result<FinishCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("check");
    let rest = subcommand_args(args);
    match subcommand {
        "check" => ensure_no_extra_args("finish check", rest).map(|()| FinishCommand::Check),
        other => Err(format!("unknown mole finish command: {other}")),
    }
}

fn parse_devtool_command(args: &[String]) -> Result<DevtoolCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("ledger");
    let rest = subcommand_args(args);
    match subcommand {
        "ledger" => ensure_no_extra_args("devtool ledger", rest).map(|()| DevtoolCommand::Ledger),
        other => Err(format!("unknown mole devtool command: {other}")),
    }
}

fn parse_replay_command(args: &[String]) -> Result<ReplayCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("check");
    let rest = subcommand_args(args);
    match subcommand {
        "artifacts" => {
            ensure_no_extra_args("replay artifacts", rest).map(|()| ReplayCommand::Artifacts)
        }
        "check" => parse_replay_check(rest).map(ReplayCommand::Check),
        "trace" => parse_replay_trace(rest).map(ReplayCommand::Trace),
        other => Err(format!("unknown mole replay command: {other}")),
    }
}

fn parse_replay_check(args: &[String]) -> Result<ReplayCheckOptions, String> {
    let mut replay = None;
    let mut inputs = None;
    let mut frames = 1_800usize;
    let mut include_negative_frames = true;
    let mut mode = ReplayCheckMode::MatchStart;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--replay" => replay = Some(take_flag_value(args, &mut index, "--replay")?),
            "--inputs" => inputs = Some(take_flag_value(args, &mut index, "--inputs")?),
            "--frames" => {
                frames = parse_positive_usize(
                    &take_flag_value(args, &mut index, "--frames")?,
                    "--frames",
                )?
            }
            "--include-negative-frames" => include_negative_frames = true,
            "--no-negative-frames" => include_negative_frames = false,
            "--mode" => {
                mode = match take_flag_value(args, &mut index, "--mode")?.as_str() {
                    "match-start" => ReplayCheckMode::MatchStart,
                    "seeded" => ReplayCheckMode::Seeded,
                    other => {
                        return Err(format!(
                            "--mode requires match-start or seeded, got {other}"
                        ))
                    }
                }
            }
            other => return Err(format!("unexpected argument for replay check: {other}")),
        }
        index += 1;
    }

    match (replay.is_some(), inputs.is_some()) {
        (true, true) => {
            Err("replay check accepts either --replay or --inputs, not both".to_string())
        }
        (false, false) => {
            Err("replay check requires --replay <path> or --inputs <path>".to_string())
        }
        _ => Ok(ReplayCheckOptions {
            replay,
            inputs,
            frames,
            include_negative_frames,
            mode,
        }),
    }
}

fn parse_replay_trace(args: &[String]) -> Result<ReplayTraceOptions, String> {
    let mut inputs = None;
    let mut frames = 1_800usize;
    let mut player_index = 0usize;
    let mut source_frame_start = 0i32;
    let mut source_frame_end = 0i32;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--inputs" => inputs = Some(take_flag_value(args, &mut index, "--inputs")?),
            "--frames" => {
                frames = parse_positive_usize(
                    &take_flag_value(args, &mut index, "--frames")?,
                    "--frames",
                )?
            }
            "--player" => {
                let player = parse_positive_usize(
                    &take_flag_value(args, &mut index, "--player")?,
                    "--player",
                )?;
                if !(1..=2).contains(&player) {
                    return Err("--player must be 1 or 2".to_string());
                }
                player_index = player - 1;
            }
            "--start" => {
                source_frame_start =
                    parse_i32(&take_flag_value(args, &mut index, "--start")?, "--start")?
            }
            "--end" => {
                source_frame_end = parse_i32(&take_flag_value(args, &mut index, "--end")?, "--end")?
            }
            other => return Err(format!("unexpected argument for replay trace: {other}")),
        }
        index += 1;
    }

    if source_frame_end < source_frame_start {
        return Err(
            "replay trace requires --end to be greater than or equal to --start".to_string(),
        );
    }

    Ok(ReplayTraceOptions {
        inputs: inputs.ok_or_else(|| "replay trace requires --inputs <path>".to_string())?,
        frames,
        player_index,
        source_frame_start,
        source_frame_end,
    })
}

fn parse_positive_usize(value: &str, flag: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{flag} must be a positive integer"))
}

fn parse_i32(value: &str, flag: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|_| format!("{flag} must be an integer"))
}

fn subcommand_args(args: &[String]) -> &[String] {
    if args.is_empty() {
        &[]
    } else {
        &args[1..]
    }
}

fn ensure_no_extra_args(command: &str, args: &[String]) -> Result<(), String> {
    if let Some(extra) = args.first() {
        return Err(format!("unexpected argument for {command}: {extra}"));
    }
    Ok(())
}

fn parse_request_command(args: &[String]) -> Result<RequestCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("list");
    let rest = subcommand_args(args);
    match subcommand {
        "list" => ensure_no_extra_args("request list", rest).map(|()| RequestCommand::List),
        "next" => ensure_no_extra_args("request next", rest).map(|()| RequestCommand::Next),
        "add" => parse_request_add(rest),
        "done" => parse_request_done(rest),
        other => Err(format!("unknown mole request command: {other}")),
    }
}

fn parse_request_add(args: &[String]) -> Result<RequestCommand, String> {
    let mut id = None;
    let mut title = None;
    let mut request = None;
    let mut context = String::new();
    let mut expected = String::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--id" => id = Some(take_flag_value(args, &mut index, "--id")?),
            "--title" => title = Some(take_flag_value(args, &mut index, "--title")?),
            "--request" => request = Some(take_flag_value(args, &mut index, "--request")?),
            "--context" => context = take_flag_value(args, &mut index, "--context")?,
            "--expected" => expected = take_flag_value(args, &mut index, "--expected")?,
            other => return Err(format!("unexpected argument for request add: {other}")),
        }
        index += 1;
    }

    Ok(RequestCommand::Add {
        id,
        title: required_request_arg(title, "--title")?,
        request: required_request_arg(request, "--request")?,
        context,
        expected,
    })
}

fn parse_request_done(args: &[String]) -> Result<RequestCommand, String> {
    let mut id = None;
    let mut result = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--id" => id = Some(take_flag_value(args, &mut index, "--id")?),
            "--result" => result = Some(take_flag_value(args, &mut index, "--result")?),
            other => return Err(format!("unexpected argument for request done: {other}")),
        }
        index += 1;
    }

    Ok(RequestCommand::Done {
        id: required_request_arg(id, "--id")?,
        result: required_request_arg(result, "--result")?,
    })
}

fn take_flag_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn required_request_arg(value: Option<String>, flag: &str) -> Result<String, String> {
    value.ok_or_else(|| format!("request command requires {flag}"))
}

pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if is_project_root(candidate) {
            return Some(candidate.to_path_buf());
        }
    }

    let first_game = start.join("First_Game");
    if is_project_root(&first_game) {
        return Some(first_game);
    }

    fs::read_dir(start)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| is_project_root(path))
}

fn is_project_root(path: &Path) -> bool {
    path.join(".git").exists()
        && path.join("Cargo.toml").exists()
        && path.join("docs").join("state_graphs").exists()
}

fn command_report(options: &CliOptions) -> Value {
    match &options.command {
        CliCommand::Status => status_report(&options.root),
        CliCommand::Parity => parity_report(&options.root),
        CliCommand::ParityGaps => parity_gaps_report(&options.root),
        CliCommand::Snapshot => snapshot_report(&options.root),
        CliCommand::Agent(command) => agent_report(&options.root, command),
        CliCommand::Graph(command) => graph::graph_report(&options.root, command),
        CliCommand::Verify(command) => verify_report(&options.root, command),
        CliCommand::Generated(command) => generated::generated_report(&options.root, command),
        CliCommand::Devtool(command) => devtool_report(&options.root, command),
        CliCommand::Finish(command) => finish_report(&options.root, command),
        CliCommand::Replay(command) => replay::replay_report(&options.root, command),
        CliCommand::Decomp(command) => decomp::decomp_report(&options.root, command),
        CliCommand::FrameData(command) => frame_data::frame_data_report(&options.root, command),
        CliCommand::Package(command) => package::package_report(&options.root, command),
        CliCommand::FriendConnect(command) => friend_connect_report(&options.root, command),
        CliCommand::Doctor => doctor_report(&options.root),
        CliCommand::Tests => tests_report(&options.root),
        CliCommand::Handoff => handoff_report(&options.root),
        CliCommand::RecommendNext => recommend_next_report(&options.root),
        CliCommand::Request(command) => request_report(&options.root, command),
        CliCommand::Help => help_report(),
    }
}

fn base_report(command: &str, root: &Path) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "project_root": root.display().to_string(),
    })
}

fn status_report(root: &Path) -> Value {
    let git = git_status_summary(root);
    let mut report = base_report("status", root);
    report["git"] = serde_json::to_value(git).expect("git summary serializes");
    report["artifacts"] = json!(artifact_checks(root));
    report
}

fn parity_report(root: &Path) -> Value {
    let parity = parity_summary(root);
    let ledger_map = ledger_map_summary(root);
    let mut report = base_report("parity", root);
    report["parity"] = serde_json::to_value(parity).expect("parity summary serializes");
    report["ledger_map"] = ledger_map;
    report
}

fn parity_gaps_report(root: &Path) -> Value {
    let mut errors = Vec::new();
    let ledger_path = root.join("docs/state_graphs/parity_ledger_map.json");
    let graph_path = root.join("docs/state_graphs/mole_current_graph.json");
    let ledger = read_json(ledger_path.clone())
        .inspect_err(|error| errors.push(format!("{}: {error}", ledger_path.display())))
        .ok();
    let graph = read_json(graph_path.clone())
        .inspect_err(|error| errors.push(format!("{}: {error}", graph_path.display())))
        .ok();
    let planned_surfaces = ledger
        .as_ref()
        .and_then(|ledger| ledger.get("tabs"))
        .and_then(Value::as_array)
        .map(|tabs| planned_ledger_surfaces(tabs))
        .unwrap_or_default();
    let partial_graph_entries = graph
        .as_ref()
        .map(partial_graph_entries)
        .unwrap_or_default();
    let mut report = base_report("parity gaps", root);
    report["mutated"] = json!(false);
    report["ok"] = json!(errors.is_empty());
    report["ledger_path"] = json!("docs/state_graphs/parity_ledger_map.json");
    report["graph_path"] = json!("docs/state_graphs/mole_current_graph.json");
    report["summary"] = json!({
        "planned_surface_count": planned_surfaces.len(),
        "partial_graph_count": partial_graph_entries.len(),
        "total_gap_count": planned_surfaces.len() + partial_graph_entries.len(),
    });
    report["planned_surfaces"] = json!(planned_surfaces);
    report["partial_graph_entries"] = json!(partial_graph_entries);
    report["recommended_next"] = json!(parity_gap_recommendations(
        report["planned_surfaces"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        report["partial_graph_entries"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    ));
    report["errors"] = json!(errors);
    report
}

fn planned_ledger_surfaces(tabs: &[Value]) -> Vec<Value> {
    tabs.iter()
        .filter(|tab| {
            tab.get("status").and_then(Value::as_str) != Some("Active")
                || tab.get("cli_surface").and_then(Value::as_str) != Some("Active")
                || tab.get("gui_surface").and_then(Value::as_str) != Some("Active")
        })
        .map(|tab| {
            json!({
                "id": tab.get("id").cloned().unwrap_or(Value::Null),
                "label": tab.get("label").cloned().unwrap_or(Value::Null),
                "kind": tab.get("kind").cloned().unwrap_or(Value::Null),
                "status": tab.get("status").cloned().unwrap_or(Value::Null),
                "cli_surface": tab.get("cli_surface").cloned().unwrap_or(Value::Null),
                "gui_surface": tab.get("gui_surface").cloned().unwrap_or(Value::Null),
                "summary": tab.get("summary").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn partial_graph_entries(graph: &Value) -> Vec<Value> {
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|node| node.get("status").and_then(Value::as_str) == Some("partial"))
        .map(|node| {
            json!({
                "kind": "node",
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "label": node.get("label").cloned().unwrap_or(Value::Null),
                "status": node.get("status").cloned().unwrap_or(Value::Null),
                "notes": node.get("notes").cloned().unwrap_or(Value::Null),
                "known_gaps": node.get("known_gaps").cloned().unwrap_or_else(|| json!([])),
            })
        });
    let edges = graph
        .get("edges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|edge| edge.get("status").and_then(Value::as_str) == Some("partial"))
        .map(|edge| {
            let from = edge.get("from").and_then(Value::as_str).unwrap_or("");
            let to = edge.get("to").and_then(Value::as_str).unwrap_or("");
            json!({
                "kind": "edge",
                "id": format!("{from} -> {to}"),
                "from": edge.get("from").cloned().unwrap_or(Value::Null),
                "to": edge.get("to").cloned().unwrap_or(Value::Null),
                "label": edge.get("label").cloned().unwrap_or(Value::Null),
                "status": edge.get("status").cloned().unwrap_or(Value::Null),
                "notes": edge.get("notes").cloned().unwrap_or(Value::Null),
                "known_gaps": edge.get("known_gaps").cloned().unwrap_or_else(|| json!([])),
            })
        });
    nodes.chain(edges).collect()
}

fn parity_gap_recommendations(
    planned_surfaces: &[Value],
    partial_entries: &[Value],
) -> Vec<String> {
    let mut recommendations = Vec::new();
    if let Some(surface) = planned_surfaces.first() {
        let id = surface
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("planned surface");
        recommendations.push(format!(
            "Implement the `{id}` CLI artifact/read model first, then mirror it in the Rust GUI."
        ));
    }
    if let Some(entry) = partial_entries.first() {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("partial graph entry");
        recommendations.push(format!(
            "Use `mole graph inspect \"{id}\" --json` before changing gameplay for the highest-priority partial graph entry."
        ));
    }
    if recommendations.is_empty() {
        recommendations.push("No planned or partial parity gaps detected.".to_string());
    }
    recommendations
}

fn friend_connect_report(root: &Path, command: &FriendConnectCommand) -> Value {
    match command {
        FriendConnectCommand::Status => friend_connect_status_report(root),
        FriendConnectCommand::Diagnostics { log } => friend_connect_diagnostics_report(root, log),
    }
}

fn friend_connect_diagnostics_report(root: &Path, log: &Option<String>) -> Value {
    let mut report = base_report("friend-connect diagnostics", root);
    let Some(log_path) = friend_connect_diagnostics_log_path(root, log) else {
        report["ok"] = json!(false);
        report["error"] = json!("no netplay log found; pass --log <path> or run Friend Connect");
        return report;
    };

    let Ok(text) = fs::read_to_string(&log_path) else {
        report["ok"] = json!(false);
        report["log_path"] = json!(log_path.display().to_string());
        report["error"] = json!("failed to read netplay log");
        return report;
    };

    let mut events = 0u64;
    let mut frame_summaries = 0u64;
    let mut last_frame = None::<u64>;
    let mut max_rollback_corrections = 0u64;
    let mut max_missing_remote_frames = 0u64;
    let mut max_packet_bundle_len = 0u64;
    let mut advance_events = 0u64;
    let mut skip_events = 0u64;
    let mut speed_ppm_min = None::<u64>;
    let mut speed_ppm_max = None::<u64>;
    let mut last_world_checksum = None::<u64>;
    let mut parse_errors = 0u64;

    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            parse_errors = parse_errors.saturating_add(1);
            continue;
        };
        events = events.saturating_add(1);
        if value.get("event").and_then(Value::as_str) == Some("frame_summary") {
            frame_summaries = frame_summaries.saturating_add(1);
        }
        if let Some(frame) = value.get("frame").and_then(Value::as_u64) {
            last_frame = Some(last_frame.map(|last| last.max(frame)).unwrap_or(frame));
        }
        max_rollback_corrections = max_rollback_corrections.max(
            value
                .get("rollback_corrections")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        );
        max_missing_remote_frames = max_missing_remote_frames.max(
            value
                .get("missing_remote_frames")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        );
        max_packet_bundle_len = max_packet_bundle_len.max(
            value
                .get("packet_bundle_len")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        );
        if value
            .get("advance_online_frame")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            advance_events = advance_events.saturating_add(1);
        }
        if value
            .get("skip_online_frame")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            skip_events = skip_events.saturating_add(1);
        }
        if let Some(speed_ppm) = value.get("speed_ppm").and_then(Value::as_u64) {
            speed_ppm_min = Some(
                speed_ppm_min
                    .map(|current| current.min(speed_ppm))
                    .unwrap_or(speed_ppm),
            );
            speed_ppm_max = Some(
                speed_ppm_max
                    .map(|current| current.max(speed_ppm))
                    .unwrap_or(speed_ppm),
            );
        }
        if let Some(checksum) = value.get("world_checksum").and_then(Value::as_u64) {
            last_world_checksum = Some(checksum);
        }
    }

    report["ok"] = json!(parse_errors == 0 && events > 0);
    report["log_path"] = json!(log_path.display().to_string());
    report["events"] = json!(events);
    report["frame_summaries"] = json!(frame_summaries);
    report["parse_errors"] = json!(parse_errors);
    report["last_frame"] = json!(last_frame);
    report["max_rollback_corrections"] = json!(max_rollback_corrections);
    report["max_missing_remote_frames"] = json!(max_missing_remote_frames);
    report["max_packet_bundle_len"] = json!(max_packet_bundle_len);
    report["advance_events"] = json!(advance_events);
    report["skip_events"] = json!(skip_events);
    report["speed_ppm_min"] = json!(speed_ppm_min);
    report["speed_ppm_max"] = json!(speed_ppm_max);
    report["last_world_checksum"] = json!(last_world_checksum);
    report["agent_notes"] = json!([
        "Rollback corrections should increase only when a late remote input differs from prediction.",
        "packet_bundle_len should normally be nonzero during gameplay; Friend Connect sends the recent input window as one Slippi-shaped datagram.",
        "advance_events and skip_events should be rare on stable links and explain visible pacing changes."
    ]);
    report
}

fn friend_connect_diagnostics_log_path(root: &Path, log: &Option<String>) -> Option<PathBuf> {
    if let Some(log) = log {
        let path = PathBuf::from(log);
        return Some(if path.is_absolute() {
            path
        } else {
            root.join(path)
        });
    }

    let log_dir = root.join("logs/netplay");
    fs::read_dir(log_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn friend_connect_status_report(root: &Path) -> Value {
    let playtest_exe = root.join("playtest/MoleGame-FriendPlaytest.exe");
    let local_internet_playtest_exe = root.join("playtest/MoleGame-LocalInternetPlaytest.exe");
    let mut report = base_report("friend-connect status", root);
    report["ok"] = json!(true);
    report["mutated"] = json!(false);
    report["package_ready"] = json!(playtest_exe.is_file());
    report["role_contract"] = json!({
        "mode": "friend-connect singles",
        "assignment_rule": "lobby role is authoritative; sorted peer codes do not decide player slots",
        "host": {
            "role": "code owner",
            "player_slot": 1,
            "network_index": 0,
            "can_start_match": true,
        },
        "joiner": {
            "role": "code connector",
            "player_slot": 2,
            "network_index": 1,
            "can_start_match": false,
        },
        "start_owner": "host/code owner",
    });
    report["controller_contract"] = json!({
        "mode": "single local active controller per Friend Connect client",
        "active_at_launch": false,
        "activation_rule": "the first connected local WUP/GameCube controller that produces non-neutral gameplay input latches as the active local controller",
        "host_mapping": "active local controller drives rollback P1",
        "joiner_mapping": "active local controller drives rollback P2",
        "extra_local_controllers": "ignored for this singles playtest; reserved for future local doubles",
        "wup_missing_or_busy": "nonfatal; runtime keeps both windows open and retries",
    });
    report["netplay_contract"] = json!({
        "default_input_delay_frames": 2,
        "manual_delay_override": "--netplay-delay N",
        "slippi_delay_model": "physical input sampled on match frame F is scheduled and transmitted immediately for game frame F + delay",
        "initial_delay_pads": "on the first online send, neutral pads are queued for the initial delay window before the first real delayed input, matching Slippi handleSendInputs",
        "rollback_window_frames": 7,
        "recent_input_retransmit_frames": 8,
        "recent_input_retain_frames": 128,
        "recent_input_datagram": "recent local inputs are transmitted as one bundled datagram ordered newest-to-oldest, mirroring Slippi SendSlippiPad's queued pad packet instead of sending one UDP datagram per frame",
        "ack_pruning": "acked local inputs are pruned with Slippi's frame < minAckFrame rule; the ack boundary frame remains available for repair",
        "remote_receive_head": "incoming bundled remote pads are copied only while their frames are newer than the current remote head, matching Slippi inputsToCopy = packetNewestFrame - headFrame; overlapping older bytes do not backfill or overwrite rollback history",
        "time_offset_sampling": "a bundled input datagram contributes one CalcTimeOffsetUs-style timing sample from the newest packet frame, not one sample per bundled pad frame",
        "remote_lookahead_stall": "when the latest remote input is outside Slippi's ROLLBACK_MAX_FRAMES lookahead, the runtime resends queued local inputs and does not advance the online match frame",
        "slippi_time_sync": "runtime mirrors Slippi CalcTimeOffsetUs early sync: every 30 online frames through frame 120, an ahead client stalls when trimmed-average offset exceeds 10000us, capped to 5 skipped frames",
        "slippi_dynamic_pacing": "after frame 120, runtime mirrors Slippi shouldAdvanceOnlineFrame m_EmulationSpeed pacing: offset samples adjust frame deadlines from 99.5% to 101.0%, with far-behind advance hints spaced every 5 frames",
        "match_frame_epoch": "match frame 0 begins when Friend Connect gameplay starts; launch-window frames are not used as gameplay packet frames",
        "late_remote_input": "accepted packets older than the current match frame are fed back through rollback confirmation when a snapshot is still available",
        "stale_remote_input": "packets older than the retained rollback snapshot window are ignored for correction instead of crashing the runtime",
    });
    report["signaling_contract"] = json!({
        "supabase_role": "rendezvous and match-start signaling only",
        "gameplay_transport": "direct UDP input packets",
        "start_delivery": "host rebroadcasts match_start so the joiner listener can catch it before game start",
    });
    report["solo_internet_test_contract"] = json!({
        "mode": "visible-host-plus-visible-peer",
        "visible_host_command": "mole_runtime.exe --friend-connect --play --input-trace --netplay-delay 2 --friend-code CODE --auto-start --friend-local-udp 127.0.0.1:41001",
        "visible_peer_command": "mole_runtime.exe --friend-connect --play --input-trace --netplay-delay 2 --friend-code PEER --connect-code CODE --friend-local-udp 127.0.0.1:41002",
        "headless_peer_command": "mole_runtime.exe --friend-connect-headless-peer --connect-code CODE --netplay-delay 2",
        "visible_peer_slot": 2,
        "headless_peer_input": "deterministic neutral input remains available for explicit headless diagnostics",
        "supabase_role": "same setup rendezvous and match-start signaling as Friend Connect",
        "gameplay_transport": "direct UDP input packets over explicit loopback ports for same-machine visual testing",
        "performance_readout": "visible Friend Connect status panels show CPU nHZ headroom before the deterministic 60 Hz cap wait",
        "log_policy": "bounded JSONL role logs under logs/netplay, capped at 5 MB per role/session without per-packet spam",
        "nat_hairpin_note": "same-machine internet testing may expose router NAT hairpin limits; the runtime must log endpoint/UDP reachability instead of silently faking loopback",
    });
    report["artifacts"] = json!({
        "dist_dir": file_status(&root.join("dist/MoleGame-FriendPlaytest")),
        "dist_zip": file_status(&root.join("dist/MoleGame-FriendPlaytest.zip")),
        "dist_exe": file_status(&root.join("dist/MoleGame-FriendPlaytest.exe")),
        "local_internet_dist_exe": file_status(&root.join("dist/MoleGame-LocalInternetPlaytest.exe")),
        "playtest_exe": file_status(&playtest_exe),
        "local_internet_playtest_exe": file_status(&local_internet_playtest_exe),
    });
    report["build_command"] = json!([
        "cargo",
        "run",
        "-p",
        "mole_cli",
        "--",
        "package",
        "friend-playtest",
        "--json"
    ]);
    report
}

fn file_status(path: &Path) -> Value {
    match fs::metadata(path) {
        Ok(metadata) => json!({
            "path": path.display().to_string(),
            "exists": true,
            "is_file": metadata.is_file(),
            "is_dir": metadata.is_dir(),
            "len": if metadata.is_file() { Some(metadata.len()) } else { None },
            "modified_unix_seconds": metadata.modified().ok().and_then(system_time_unix_seconds),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => json!({
            "path": path.display().to_string(),
            "exists": false,
        }),
        Err(error) => json!({
            "path": path.display().to_string(),
            "exists": false,
            "error": error.to_string(),
        }),
    }
}

fn system_time_unix_seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn snapshot_report(root: &Path) -> Value {
    let git = git_status_summary(root);
    let parity = parity_summary(root);
    let ledger_map = ledger_map_summary(root);
    let generated_artifacts = generated_artifact_statuses(root);
    let generated_artifacts_dirty = generated_artifacts.iter().any(|artifact| artifact.dirty);
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "parity snapshot",
        "project_root": root.display().to_string(),
        "git": {
            "branch": git.branch,
            "remote_origin": git.remote_origin,
            "branch_ok": git.branch_ok,
            "remote_ok": git.remote_ok,
            "dirty": git.dirty,
            "total_changed": git.total_changed,
            "generated_artifacts_dirty": generated_artifacts_dirty,
            "error": git.git_error,
        },
        "generated_artifacts": generated_artifacts,
        "value_counts": {
            "total_rows": parity.value_total_rows,
            "matches": parity.value_matches,
            "actionable": parity.value_actionable,
            "derived": parity.value_derived,
            "sections": parity.value_sections,
        },
        "ledger_map": ledger_map,
        "falcon_ecb": {
            "mapped_motion_states": parity.ecb_mapped_motion_states,
            "missing_sampled_mappings": parity.ecb_missing_sampled_mappings,
            "unmapped_derived_states": parity.ecb_unmapped_derived_states,
        },
        "graph": {
            "nodes": parity.graph_nodes,
            "edges": parity.graph_edges,
            "status_counts": parity.graph_status_counts,
        },
        "recommended_next": recommended_next(root),
        "verification_commands": recommended_test_commands(),
        "errors": parity.errors,
    })
}

fn devtool_report(root: &Path, command: &DevtoolCommand) -> Value {
    match command {
        DevtoolCommand::Ledger => devtool_ledger_report(root),
    }
}

fn devtool_ledger_report(root: &Path) -> Value {
    let path = root.join("docs/state_graphs/parity_ledger_map.json");
    match ParityLedgerViewModel::load(&path) {
        Ok(view_model) => json!({
            "schema_version": SCHEMA_VERSION,
            "command": "devtool ledger",
            "project_root": root.display().to_string(),
            "mutated": false,
            "ok": true,
            "surface": view_model.surface,
            "ledger": view_model,
        }),
        Err(error) => json!({
            "schema_version": SCHEMA_VERSION,
            "command": "devtool ledger",
            "project_root": root.display().to_string(),
            "mutated": false,
            "ok": false,
            "surface": "parity_ledger",
            "error": error,
            "ledger": {
                "schema_version": 1,
                "surface": "parity_ledger",
                "registry": {
                    "tab_count": 0,
                    "active_tab_count": 0,
                    "planned_tab_count": 0,
                    "dual_surface": false,
                },
                "tabs": [],
                "all_tabs_dual_surface": false,
            },
        }),
    }
}

fn doctor_report(root: &Path) -> Value {
    let checks = doctor_checks(root);
    let ok = checks.iter().all(|check| check.ok);
    let mut report = base_report("doctor", root);
    report["ok"] = json!(ok);
    report["checks"] = serde_json::to_value(checks).expect("doctor checks serialize");
    report
}

fn tests_report(root: &Path) -> Value {
    let commands = recommended_test_commands();
    let mut report = base_report("tests", root);
    report["commands"] = json!(commands);
    report["notes"] = json!([
        "Run full workspace tests after Rust core or generated table changes.",
        "Run graph/parity Python tests after docs/state_graphs or tools changes.",
        "Cargo accepts one test-name filter per cargo test invocation; run multiple exact tests as separate cargo test commands or use one shared substring/module filter.",
        "Mole CLI is read-only; it reports commands but does not run them."
    ]);
    report
}

fn handoff_report(root: &Path) -> Value {
    let mut report = base_report("handoff", root);
    report["git"] = serde_json::to_value(git_status_summary(root)).expect("git summary serializes");
    report["parity"] = serde_json::to_value(parity_summary(root)).expect("parity serializes");
    report["recommended_next"] = json!(recommended_next(root));
    report["verification_commands"] = json!(recommended_test_commands());
    report
}

fn recommend_next_report(root: &Path) -> Value {
    let mut report = base_report("recommend-next", root);
    report["recommended_next"] = json!(recommended_next(root));
    report
}

fn finish_report(root: &Path, command: &FinishCommand) -> Value {
    match command {
        FinishCommand::Check => finish_check_report(root),
    }
}

fn finish_check_report(root: &Path) -> Value {
    let git = git_status_summary(root);
    let help_catalog = help_catalog_status();
    let doctor_checks = doctor_checks(root);
    let doctor_ok = doctor_checks.iter().all(|check| check.ok);
    let request_summary = message_board_summary(root);
    let (verification, verification_ok) = finish_verification_report(root);
    let ledger_map = ledger_map_summary(root);
    let ledger_map_registry = ledger_map
        .get("registry")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let ledger_map_ok = ledger_map
        .get("loadable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && ledger_map_registry
            .get("dual_surface")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let completion_checks = vec![
        json!({
            "name": "repository branch",
            "ok": git.branch_ok,
            "detail": format!("expected `{}`, got `{}`", EXPECTED_BRANCH, git.branch.as_deref().unwrap_or("unknown")),
        }),
        json!({
            "name": "repository remote",
            "ok": git.remote_ok,
            "detail": format!("expected `{}`", EXPECTED_REMOTE),
        }),
        json!({
            "name": "help catalog",
            "ok": help_catalog.get("ok").and_then(Value::as_bool).unwrap_or(false),
            "detail": "help --json documents the known command surface",
        }),
        json!({
            "name": "doctor",
            "ok": doctor_ok,
            "detail": "required local tools and project artifacts are visible",
        }),
        json!({
            "name": "request inbox",
            "ok": request_summary.inbox_count == 0,
            "detail": format!("{} inbox request(s)", request_summary.inbox_count),
        }),
        json!({
            "name": "verification plan",
            "ok": verification_ok,
            "detail": "changed-file verification plan was generated",
        }),
        json!({
            "name": "ledger map",
            "ok": ledger_map_ok,
            "detail": format!(
                "tab_count {}, dual_surface {}",
                ledger_map_registry.get("tab_count").and_then(Value::as_u64).unwrap_or(0),
                ledger_map_registry.get("dual_surface").and_then(Value::as_bool).unwrap_or(false)
            ),
        }),
    ];
    let completion_ok = completion_checks
        .iter()
        .all(|check| check.get("ok").and_then(Value::as_bool).unwrap_or(false));

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "finish check",
        "project_root": root.display().to_string(),
        "mutated": false,
        "completion_gate": {
            "ok": completion_ok,
            "checks": completion_checks,
        },
        "git": {
            "branch": git.branch,
            "remote_origin": git.remote_origin,
            "branch_ok": git.branch_ok,
            "remote_ok": git.remote_ok,
            "dirty": git.dirty,
            "total_changed": git.total_changed,
            "error": git.git_error,
        },
        "help_catalog": help_catalog,
        "doctor": {
            "ok": doctor_ok,
            "checks": doctor_checks,
        },
        "request_queue": {
            "inbox_count": request_summary.inbox_count,
            "completed_count": request_summary.completed_count,
            "next": request_summary.inbox.first(),
            "errors": request_summary.errors,
        },
        "verification": verification,
    })
}

fn finish_verification_report(root: &Path) -> (Value, bool) {
    let mut errors = Vec::new();
    let changed_paths = match run_git(root, &["status", "--porcelain=v1"]) {
        Ok(status) => verify::changed_paths_from_git_status(&status),
        Err(error) => {
            errors.push(format!("git status --porcelain=v1: {error}"));
            Vec::new()
        }
    };
    let (commands, command_reasons) =
        verify::verification_plan_with_reasons_for_changed_paths(&changed_paths);
    let ok = errors.is_empty() && commands.iter().any(|command| command == "git diff --check");
    (
        json!({
            "changed_paths": changed_paths,
            "commands": commands,
            "command_reasons": command_reasons,
            "errors": errors,
        }),
        ok,
    )
}

fn help_catalog_status() -> Value {
    let expected = expected_command_names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let commands = command_help_catalog();
    let documented = commands
        .as_array()
        .map(|commands| {
            commands
                .iter()
                .filter_map(|command| command.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let missing = expected
        .iter()
        .filter(|expected| !documented.iter().any(|documented| documented == *expected))
        .cloned()
        .collect::<Vec<_>>();
    let extra = documented
        .iter()
        .filter(|documented| !expected.iter().any(|expected| expected == *documented))
        .cloned()
        .collect::<Vec<_>>();

    json!({
        "ok": missing.is_empty(),
        "documented_command_count": documented.len(),
        "expected_commands": expected,
        "documented_commands": documented,
        "missing_commands": missing,
        "extra_commands": extra,
    })
}

fn expected_command_names() -> Vec<&'static str> {
    vec![
        "status",
        "parity",
        "parity gaps",
        "parity snapshot",
        "snapshot",
        "agent brief",
        "graph missing",
        "graph next",
        "graph inspect",
        "graph layout",
        "verify changed",
        "generated check",
        "devtool ledger",
        "finish check",
        "frame-data extract",
        "frame-data export-runtime",
        "frame-data show",
        "package friend-playtest",
        "package local-internet-playtest",
        "friend-connect status",
        "doctor",
        "tests",
        "handoff",
        "recommend-next",
        "request list",
        "request next",
        "request add",
        "request done",
        "help",
    ]
}

fn agent_report(root: &Path, command: &AgentCommand) -> Value {
    match command {
        AgentCommand::Brief => agent_brief_report(root),
    }
}

fn agent_brief_report(root: &Path) -> Value {
    let git = git_status_summary(root);
    let parity = parity_summary(root);
    let ledger_map = ledger_map_summary(root);
    let missing_graph = graph::graph_missing_report(root);
    let graph_next = graph::graph_next_report(root);
    let request_summary = message_board_summary(root);
    let next_request = request_summary.inbox.first().cloned();
    let (macro_plan_exists, compaction_anchor, macro_plan_error) = macro_plan_anchor(root);
    let mut errors = parity.errors.clone();
    if let Some(error) = ledger_map.get("error").and_then(Value::as_str) {
        errors.push(error.to_string());
    }
    errors.extend(request_summary.errors.clone());
    if let Some(error) = macro_plan_error {
        errors.push(error);
    }

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "agent brief",
        "project_root": root.display().to_string(),
        "git": {
            "branch": git.branch,
            "remote_origin": git.remote_origin,
            "branch_ok": git.branch_ok,
            "remote_ok": git.remote_ok,
            "dirty": git.dirty,
            "total_changed": git.total_changed,
            "error": git.git_error,
        },
        "macro_plan": {
            "path": MACRO_PLAN_PATH,
            "exists": macro_plan_exists,
        },
        "compaction_anchor": compaction_anchor,
        "parity": parity,
        "ledger_map": ledger_map,
        "missing_graph": {
            "missing_count": missing_graph.get("missing_count").cloned().unwrap_or(json!(0)),
            "nodes": missing_graph.get("nodes").cloned().unwrap_or_else(|| json!([])),
            "edges": missing_graph.get("edges").cloned().unwrap_or_else(|| json!([])),
            "recommended_next": missing_graph.get("recommended_next").cloned().unwrap_or_else(|| json!([])),
        },
        "graph_next": {
            "ranked_count": graph_next.get("ranked_count").cloned().unwrap_or(json!(0)),
            "omitted_count": graph_next.get("omitted_count").cloned().unwrap_or(json!(0)),
            "ranked_entries": graph_next.get("ranked_entries").cloned().unwrap_or_else(|| json!([])),
        },
        "request_queue": {
            "inbox_count": request_summary.inbox_count,
            "completed_count": request_summary.completed_count,
            "next": next_request,
        },
        "recommended_next": recommended_next(root),
        "verification_commands": recommended_test_commands(),
        "errors": errors,
    })
}

fn macro_plan_anchor(root: &Path) -> (bool, Vec<String>, Option<String>) {
    let path = root.join(MACRO_PLAN_PATH);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return (
                false,
                Vec::new(),
                Some(format!("{}: {error}", path.display())),
            )
        }
    };
    let mut in_anchor = false;
    let mut anchor = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "## Compaction Anchor" {
            in_anchor = true;
            continue;
        }
        if in_anchor && trimmed.starts_with("## ") {
            break;
        }
        if !in_anchor {
            continue;
        }
        if let Some((_number, text)) = trimmed.split_once(". ") {
            if trimmed
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
            {
                anchor.push(text.trim().to_string());
            }
        }
    }
    (true, anchor, None)
}

fn verify_report(root: &Path, command: &VerifyCommand) -> Value {
    match command {
        VerifyCommand::Changed => verify_changed_report(root),
    }
}

fn verify_changed_report(root: &Path) -> Value {
    let mut errors = Vec::new();
    let changed_paths = match run_git(root, &["status", "--porcelain=v1"]) {
        Ok(status) => verify::changed_paths_from_git_status(&status),
        Err(error) => {
            errors.push(format!("git status --porcelain=v1: {error}"));
            Vec::new()
        }
    };
    let (commands, command_reasons) =
        verify::verification_plan_with_reasons_for_changed_paths(&changed_paths);
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "verify changed",
        "project_root": root.display().to_string(),
        "mutated": false,
        "changed_paths": changed_paths,
        "commands": commands,
        "command_reasons": command_reasons,
        "errors": errors,
    })
}

fn request_report(root: &Path, command: &RequestCommand) -> Value {
    match command {
        RequestCommand::List => request_list_report(root),
        RequestCommand::Next => request_next_report(root),
        RequestCommand::Add {
            id,
            title,
            request,
            context,
            expected,
        } => match add_message_board_request(root, id.clone(), title, request, context, expected) {
            Ok(request) => request_mutation_report("request add", root, true, request),
            Err(error) => request_error_report("request add", root, error),
        },
        RequestCommand::Done { id, result } => {
            match complete_message_board_request(root, id, result) {
                Ok(request) => request_mutation_report("request done", root, true, request),
                Err(error) => request_error_report("request done", root, error),
            }
        }
    }
}

fn request_list_report(root: &Path) -> Value {
    let summary = message_board_summary(root);
    let mut report = base_report("request list", root);
    report["message_board"] = json!(MESSAGE_BOARD_PATH);
    report["mutated"] = json!(false);
    report["inbox_count"] = json!(summary.inbox_count);
    report["completed_count"] = json!(summary.completed_count);
    report["inbox"] = serde_json::to_value(summary.inbox).expect("requests serialize");
    report["completed"] = serde_json::to_value(summary.completed).expect("requests serialize");
    report["errors"] = json!(summary.errors);
    report
}

fn request_next_report(root: &Path) -> Value {
    let summary = message_board_summary(root);
    let next = summary.inbox.first().cloned();
    let mut report = base_report("request next", root);
    report["message_board"] = json!(MESSAGE_BOARD_PATH);
    report["mutated"] = json!(false);
    report["has_request"] = json!(next.is_some());
    report["request"] = serde_json::to_value(next).expect("request serializes");
    report["inbox_count"] = json!(summary.inbox_count);
    report["completed_count"] = json!(summary.completed_count);
    report["errors"] = json!(summary.errors);
    report
}

fn request_mutation_report(
    command: &str,
    root: &Path,
    mutated: bool,
    request: MessageBoardRequest,
) -> Value {
    let mut report = base_report(command, root);
    report["message_board"] = json!(MESSAGE_BOARD_PATH);
    report["mutated"] = json!(mutated);
    report["request"] = serde_json::to_value(request).expect("request serializes");
    report
}

fn request_error_report(command: &str, root: &Path, error: String) -> Value {
    let mut report = base_report(command, root);
    report["message_board"] = json!(MESSAGE_BOARD_PATH);
    report["mutated"] = json!(false);
    report["error"] = json!(error);
    report
}

fn help_report() -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "help",
        "summary": "Agent-facing command catalog for Mole CLI.",
        "usage": "mole <command> [--json|--text|--format json|text|markdown] [--root PATH]",
        "default_command": "status",
        "global_flags": {
            "--json": "Emit JSON output. This is the default.",
            "--text": "Emit compact human-readable text output.",
            "--format": "Output format override: json, text, or markdown.",
            "--root": "Project root override.",
            "-h, --help": "Show this command catalog."
        },
        "commands": command_help_catalog(),
        "examples": [
            "cargo run -p mole_cli -- help --json",
            "cargo run -p mole_cli -- status --json",
            "cargo run -p mole_cli -- agent brief --json",
            "cargo run -p mole_cli -- parity snapshot --json",
            "cargo run -p mole_cli -- graph missing --json",
            "cargo run -p mole_cli -- graph next --json",
            "cargo run -p mole_cli -- graph next --format markdown",
            "cargo run -p mole_cli -- graph inspect Dash --json",
            "cargo run -p mole_cli -- graph inspect \"Dash -> Run\" --format markdown",
            "cargo run -p mole_cli -- verify changed --json",
            "cargo run -p mole_cli -- verify changed --format markdown",
            "cargo run -p mole_cli -- generated check --json",
            "cargo run -p mole_cli -- generated check --format markdown",
            "cargo run -p mole_cli -- finish check --json",
            "cargo run -p mole_cli -- finish check --format markdown",
            "cargo run -p mole_cli -- replay check --replay replays\\Game_20260530T214929.slp --frames 1800 --json",
            "cargo run -p mole_cli -- replay check --inputs debug\\slippi\\Game_20260530T214929.inputs.json --mode seeded --json",
            "cargo run -p mole_cli -- replay trace --inputs debug\\slippi\\Game_20260530T214929.inputs.json --player 1 --start 392 --end 402 --format markdown",
            "cargo run -p mole_cli -- decomp search ftCo_Turn_Anim --json",
            "cargo run -p mole_cli -- decomp symbol ftCo_LandingFallSpecial_Enter --format markdown",
            "cargo run -p mole_cli -- decomp show src/melee/ft/chara/ftCommon/ftCo_Turn.c --line 90 --context 24 --json",
            "cargo run -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --write --json",
            "cargo run -p mole_cli -- frame-data extract --all-states --character dolphin_mole --source-character captain --write --json",
            "cargo run -p mole_cli -- frame-data export-runtime --all-states --character dolphin_mole --output crates/mole_runtime/src/generated/source_frame_data.rs --write --json",
            "cargo run -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown",
            "cargo run -p mole_cli -- package friend-playtest --json",
            "cargo run -p mole_cli -- package local-internet-playtest --json",
            "cargo run -p mole_cli -- friend-connect status --json",
            "cargo run -p mole_cli -- friend-connect diagnostics --log logs\\netplay\\visible_host.jsonl --json",
            "cargo run -p mole_cli -- request next --json",
            "cargo run -p mole_cli -- request add --id trace-summary --title \"Trace Summary\" --request \"Add a compact trace summary command.\" --context \"Agents need shorter logs.\" --expected \"JSON summary.\" --json",
            "cargo run -p mole_cli -- request done --id trace-summary --result \"Implemented and verified.\" --json"
        ],
        "ai_contract": {
            "read_only_by_default": true,
            "mutating_commands": ["frame-data extract", "frame-data export-runtime", "package friend-playtest", "package local-internet-playtest", "replay check", "request add", "request done"],
            "no_interactive_prompts": true,
            "stable_json_schema_version": SCHEMA_VERSION,
            "nonzero_exit_on_cli_usage_error": true
        }
    })
}

fn command_help_catalog() -> Value {
    json!([
        {
            "name": "status",
            "usage": "mole status [--json]",
            "purpose": "Summarize git branch, remote, dirty counts, and key artifact presence.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Good first command for re-anchoring any agent in the correct repository."
        },
        {
            "name": "parity",
            "usage": "mole parity [--json]",
            "purpose": "Summarize parity ledgers for value sheets, Falcon ECB coverage, and state graph status counts.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Read-only; does not run generators."
        },
        {
            "name": "parity gaps",
            "usage": "mole parity gaps [--json]",
            "purpose": "List planned parity-ledger surfaces and partial state-graph entries as one agent-facing implementation queue.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use before implementing broad parity work so planned surfaces and partial graph entries come from checked-in artifacts instead of ad-hoc JSON parsing."
        },
        {
            "name": "parity snapshot",
            "usage": "mole parity snapshot [--json]",
            "purpose": "Return compact all-in-one parity context for handoff or context refresh.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["snapshot"],
            "agent_notes": "Preferred command when an agent needs high-signal project context quickly."
        },
        {
            "name": "snapshot",
            "usage": "mole snapshot [--json]",
            "purpose": "Alias for `parity snapshot`.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["parity snapshot"],
            "agent_notes": "Shorter spelling for automation."
        },
        {
            "name": "agent brief",
            "usage": "mole agent brief [--json|--format markdown]",
            "purpose": "Return a compact start-here packet with git status, macro-plan anchor, parity, missing graph entries, ranked graph-next targets, request queue, and verification commands.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["agent"],
            "agent_notes": "Best first command after compaction or handoff when the agent needs high-signal context."
        },
        {
            "name": "graph missing",
            "usage": "mole graph missing [--json]",
            "purpose": "List state graph nodes and edges whose status is missing, including refs and compact recommendations.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["graph"],
            "agent_notes": "Use before bottom-up parity work to avoid ad-hoc graph JSON parsing."
        },
        {
            "name": "graph next",
            "usage": "mole graph next [--json|--format markdown]",
            "purpose": "Rank missing and high-priority partial state-graph entries for source-backed parity work, including score reasons.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use when choosing the next graph-backed parity target."
        },
        {
            "name": "graph inspect",
            "usage": "mole graph inspect <node-id|from->to> [--json|--format markdown]",
            "purpose": "Return source refs, Rust refs, value refs, known gaps, notes, and an audit checklist for one graph node or edge.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use after graph next selects a target so agents do not manually parse graph JSON."
        },
        {
            "name": "graph layout",
            "usage": "mole graph layout [--json|--format markdown]",
            "purpose": "Inspect the two-pane state graph canvas contract: graph files, saved layout coverage, zoom, and validation errors.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use before changing Rust state graph canvas behavior so CLI and GUI consume the same graph/layout facts."
        },
        {
            "name": "graph layout save",
            "usage": "mole graph layout save [--check|--write] [--json|--format markdown]",
            "purpose": "Validate or write the shared state graph layout file used by the Rust devtool canvases.",
            "mutates_workspace": true,
            "writes": ["config/state_graph_layout.json"],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format", "--check", "--write"],
            "aliases": [],
            "agent_notes": "Use --check before committing GUI layout edits; use --write only when intentionally canonicalizing the shared layout file."
        },
        {
            "name": "verify changed",
            "usage": "mole verify changed [--json|--format markdown]",
            "purpose": "Inspect changed files and return the smallest safe verification command set, including why each command was selected.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["verify"],
            "agent_notes": "Use before claiming completion to avoid guessing which tests cover the current diff."
        },
        {
            "name": "generated check",
            "usage": "mole generated check [--json|--format markdown]",
            "purpose": "Check known generated artifacts for missing outputs, missing inputs, stale timestamps, dirty generated outputs, and recommended regeneration commands.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["generated"],
            "agent_notes": "Read-only; use before handoff or after editing extraction inputs/generators so agents do not guess whether generated parity files are current."
        },
        {
            "name": "devtool ledger",
            "usage": "mole devtool ledger [--json|--format markdown]",
            "purpose": "Load the Rust parity ledger map into a GUI-ready view model for the native dev-tool layer.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["devtool"],
            "agent_notes": "Use this when a future Rust GUI needs the same tab model that the CLI already consumes."
        },
        {
            "name": "generated write-value-sheets",
            "usage": "mole generated write-value-sheets [--write] [--json]",
            "purpose": "Write Rust-owned parity value sheets for global, character, physics, combat, and stage-ledger values.",
            "mutates_workspace": true,
            "writes": [
                "docs/state_graphs/value_sheets/global_common_values.json",
                "docs/state_graphs/value_sheets/captain_falcon_values.json",
                "docs/state_graphs/value_sheets/physics_engine_values.json",
                "docs/state_graphs/value_sheets/global_combat_values.json",
                "docs/state_graphs/value_sheets/captain_falcon_combat_values.json",
                "docs/state_graphs/value_sheets/battlefield_stage_values.json"
            ],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--write", "--root", "--json", "--text", "--format"],
            "aliases": ["generated"],
            "agent_notes": "Use this when the checked-in parity sheet schema changes so the repo stays Rust-owned instead of depending on Python generation."
        },
        {
            "name": "generated write-stage-asset",
            "usage": "mole generated write-stage-asset --stage battlefield [--write] [--json]",
            "purpose": "Write a portable extracted stage asset for the named stage, starting with Battlefield as the template stage.",
            "mutates_workspace": true,
            "writes": ["resources/melee/extracted/stages/battlefield_stage.json"],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": ["--stage"],
            "optional_flags": ["--write", "--root", "--json", "--text", "--format"],
            "aliases": ["generated"],
            "agent_notes": "Use this to keep stage extraction structured so later stages can reuse the same pipeline and file shape."
        },
        {
            "name": "generated write-ledger-map",
            "usage": "mole generated write-ledger-map [--write] [--json]",
            "purpose": "Write the canonical dual-surface parity ledger registry for CLI and GUI consumers.",
            "mutates_workspace": true,
            "writes": ["docs/state_graphs/parity_ledger_map.json"],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--write", "--root", "--json", "--text", "--format"],
            "aliases": ["generated"],
            "agent_notes": "Use this when the ledger tab contract changes so both human and agentic surfaces keep reading the same Rust-owned registry."
        },
        {
            "name": "finish check",
            "usage": "mole finish check [--json|--format markdown]",
            "purpose": "Run a read-only completion gate over repository anchor, help catalog freshness, doctor status, request queue, and changed-file verification plan.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["finish"],
            "agent_notes": "Use before handoff, compaction, or claiming a CLI/development-tooling slice is complete."
        },
        {
            "name": "replay check",
            "usage": "mole replay check (--replay PATH|--inputs PATH) [--frames N] [--mode match-start|seeded] [--no-negative-frames] [--json|--text|--format markdown]",
            "purpose": "Export a Slippi replay when needed, replay its game-facing inputs through the Rust core, and report the first state mismatch or significant position drift.",
            "mutates_workspace": true,
            "writes": ["debug/slippi/*.inputs.json", "debug/slippi/*.report.md", "debug/slippi/*.core.report.md"],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": ["--replay or --inputs"],
            "optional_flags": ["--frames", "--mode", "--include-negative-frames", "--no-negative-frames", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Default mode is sequential match-start and includes negative entry frames for raw .slp exports. Use --inputs with an existing exported JSON file to skip Node export."
        },
        {
            "name": "replay artifacts",
            "usage": "mole replay artifacts [--json|--format markdown]",
            "purpose": "List existing Slippi replay files and exported input JSON artifacts for trace/replay selection without mutating the workspace.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use before replay trace or when wiring GUI artifact selectors; returns paths plus available export metadata."
        },
        {
            "name": "replay trace",
            "usage": "mole replay trace --inputs PATH [--player 1|2] [--start N] [--end N] [--frames N] [--json|--format markdown]",
            "purpose": "Replay an existing Slippi input export from match start and return a compact per-frame trace window for one player.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": ["--inputs"],
            "optional_flags": ["--player", "--start", "--end", "--frames", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use after replay check identifies a divergence; includes source frame, input, expected state, actual state, actual motion frame, position, and velocity deltas."
        },
        {
            "name": "decomp search",
            "usage": "mole decomp search <query> [--limit N] [--decomp-root PATH] [--json|--format markdown]",
            "purpose": "Quick search the local decompiled Melee tree and return compact source references for parity agents.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--limit", "--decomp-root", "--root", "--json", "--text", "--format"],
            "aliases": ["decomp"],
            "agent_notes": "Use during replay divergence work before ad-hoc shell searches; each match includes a suggested decomp show command."
        },
        {
            "name": "decomp show",
            "usage": "mole decomp show <relative-path> [--line N] [--context N] [--decomp-root PATH] [--json|--format markdown]",
            "purpose": "Return a bounded numbered excerpt from a decompiled Melee source file.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--line", "--context", "--decomp-root", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use the suggested command from decomp search to quote just the source lines needed for a parity fix."
        },
        {
            "name": "decomp symbol",
            "usage": "mole decomp symbol <name> [--limit N] [--decomp-root PATH] [--json|--format markdown]",
            "purpose": "Find likely decompiled Melee function or symbol definitions, ranking exact definitions before references.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--limit", "--decomp-root", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use when a replay trace names a source function and the agent needs the definition quickly."
        },
        {
            "name": "frame-data extract",
            "usage": "mole frame-data extract --character ID --source-character ID (--state MotionState|--all-states) [--source-state SourceAction] [--write] [--json|--format markdown]",
            "purpose": "Load source-derived move frame data for a target character, preserving raw 3D keyframes and extraction gaps; with --all-states, map every source action with an explicit MotionState match into character-scoped artifacts.",
            "mutates_workspace": true,
            "writes": ["resources/melee/frame_data/<character>/<state>.json when --write is present"],
            "output_modes": ["json", "markdown"],
            "required_flags": ["--character", "--source-character", "--state or --all-states"],
            "optional_flags": ["--source-state", "--all-states", "--write", "--root", "--json", "--format"],
            "aliases": [],
            "agent_notes": "Use for replay-parity attack work such as Captain Falcon Nair assigned to Dolphin Mole, or use --all-states to import a source character's mapped actions without making the pipeline Falcon-specific."
        },
        {
            "name": "frame-data export-runtime",
            "usage": "mole frame-data export-runtime --character ID (--state MotionState|--all-states) [--output PATH] [--write] [--json]",
            "purpose": "Generate runtime frame-data assets from extracted source data. Compact all-states export emits provenance sidecars plus a baked source_frame_capsules.bin action/frame capsule and DownBound hip-pose sidecar consumed by player runtime.",
            "mutates_workspace": true,
            "writes": ["crates/mole_runtime/src/generated/source_frame_data.rs", "crates/mole_runtime/src/generated/source_frame_data/*"],
            "output_modes": ["json"],
            "required_flags": ["--character", "--state or --all-states"],
            "optional_flags": ["--all-states", "--output", "--write", "--root", "--json"],
            "aliases": [],
            "agent_notes": "Use after frame-data extract/show confirms the canonical artifacts; compact --all-states embeds source-shaped Rust assets so runtime does not read raw Melee DAT files or evaluate FigaTree/JObj data during play."
        },
        {
            "name": "frame-data show",
            "usage": "mole frame-data show --character ID --state MotionState [--json|--format markdown]",
            "purpose": "Show an existing move frame data artifact with keyframes, projection metadata, source citations, and gaps.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "markdown"],
            "required_flags": ["--character", "--state"],
            "optional_flags": ["--root", "--json", "--format"],
            "aliases": [],
            "agent_notes": "Use after extraction to cite the canonical artifact that also feeds the Move Keyframes dev-tool tab."
        },
        {
            "name": "package friend-playtest",
            "usage": "mole package friend-playtest [--verify|--no-verify] [--dry-run] [--json]",
            "purpose": "Build the one-file Windows Friend Connect playtest package and, by default, verify that it extracts and the packaged runtime starts without depending on the repo asset path.",
            "mutates_workspace": true,
            "writes": ["dist/MoleGame-FriendPlaytest", "dist/MoleGame-FriendPlaytest.zip", "dist/MoleGame-FriendPlaytest.exe", "dist/MoleGame-LocalInternetPlaytest.exe", "playtest/MoleGame-FriendPlaytest.exe", "playtest/MoleGame-LocalInternetPlaytest.exe"],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--verify", "--no-verify", "--dry-run", "--root", "--json", "--text", "--format"],
            "aliases": ["package"],
            "agent_notes": "Use this instead of manually remembering the PowerShell package script. The default verification catches stale packaged assets and hardcoded development asset roots before sending the exe to a tester."
        },
        {
            "name": "package local-internet-playtest",
            "usage": "mole package local-internet-playtest [--verify|--no-verify] [--dry-run] [--json]",
            "purpose": "Build the secondary one-file Windows launcher that extracts the Friend Connect package, generates a local room code, and starts both the visible host and headless internet peer.",
            "mutates_workspace": true,
            "writes": ["dist/MoleGame-FriendPlaytest", "dist/MoleGame-FriendPlaytest.zip", "dist/MoleGame-FriendPlaytest.exe", "dist/MoleGame-LocalInternetPlaytest.exe", "playtest/MoleGame-FriendPlaytest.exe", "playtest/MoleGame-LocalInternetPlaytest.exe"],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--verify", "--no-verify", "--dry-run", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use this when the solo visible-host plus headless-peer EXE needs to be refreshed after runtime or package changes."
        },
        {
            "name": "friend-connect status",
            "usage": "mole friend-connect status [--json|--text]",
            "purpose": "Report the current Friend Connect role, controller activation, signaling, and playtest artifact contract without mutating the workspace.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["friend-connect"],
            "agent_notes": "Use before packaging or handoff to confirm the host/code owner is P1/start owner, the joiner is P2, and controllers latch only after first real local input."
        },
        {
            "name": "friend-connect diagnostics",
            "usage": "mole friend-connect diagnostics [--log PATH] [--json|--text]",
            "purpose": "Summarize bounded Friend Connect JSONL logs into rollback, packet bundle, checksum, and pacing counters for playtest diagnosis.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--log", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use after a local or friend internet playtest. Without --log it reads the newest logs/netplay/*.jsonl file."
        },
        {
            "name": "doctor",
            "usage": "mole doctor [--json]",
            "purpose": "Check whether expected repo artifacts and helper runtimes are present.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Reports availability; does not install tools or mutate the workspace."
        },
        {
            "name": "tests",
            "usage": "mole tests [--json]",
            "purpose": "List recommended verification commands for the current Mole Game workflow.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Reports commands only; it intentionally does not execute them. Cargo accepts one test-name filter per invocation; run multiple exact tests as separate cargo test commands or use one shared substring/module filter."
        },
        {
            "name": "handoff",
            "usage": "mole handoff [--json|--format markdown]",
            "purpose": "Produce a handoff-oriented summary of git, parity, recommendations, and verification commands.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text", "markdown"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use markdown when pasting into a conversation; use JSON for tools."
        },
        {
            "name": "recommend-next",
            "usage": "mole recommend-next [--json]",
            "purpose": "Return high-priority structural recommendations derived from git and parity summaries.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Recommendations are hints, not proof that gameplay code should change."
        },
        {
            "name": "request list",
            "usage": "mole request list [--json]",
            "purpose": "List Inbox and Completed Notes from the Mole CLI message board.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": ["request"],
            "agent_notes": "Reads MOLE_CLI_AGENT_MESSAGES.md."
        },
        {
            "name": "request next",
            "usage": "mole request next [--json]",
            "purpose": "Return the newest actionable Inbox request for Mole CLI/development-tooling work.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Preferred startup command after reading the message board directly."
        },
        {
            "name": "request add",
            "usage": "mole request add --title TITLE --request TEXT [--id ID] [--context TEXT] [--expected TEXT] [--json]",
            "purpose": "Add a newest-first feature request to the Mole CLI message board Inbox.",
            "mutates_workspace": true,
            "writes": [MESSAGE_BOARD_PATH],
            "output_modes": ["json", "text"],
            "required_flags": ["--title", "--request"],
            "optional_flags": ["--id", "--context", "--expected", "--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Only intended mutation for creating Mole CLI/development-tooling feature requests."
        },
        {
            "name": "request done",
            "usage": "mole request done --id ID --result TEXT [--json]",
            "purpose": "Move a matching Inbox request to Completed Notes and record the result.",
            "mutates_workspace": true,
            "writes": [MESSAGE_BOARD_PATH],
            "output_modes": ["json", "text"],
            "required_flags": ["--id", "--result"],
            "optional_flags": ["--root", "--json", "--text", "--format"],
            "aliases": [],
            "agent_notes": "Use after implementation and verification so the main agent can see completion."
        },
        {
            "name": "help",
            "usage": "mole help [--json]",
            "purpose": "Expose this complete command catalog for AI agents.",
            "mutates_workspace": false,
            "writes": [],
            "output_modes": ["json", "text"],
            "required_flags": [],
            "optional_flags": ["--root", "--json", "--text", "--format", "-h", "--help"],
            "aliases": ["-h", "--help"],
            "agent_notes": "Update this catalog whenever Mole CLI gains or removes a command."
        }
    ])
}

pub fn message_board_summary(root: &Path) -> MessageBoardSummary {
    let path = message_board_path(root);
    match fs::read_to_string(&path) {
        Ok(text) => parse_message_board(&text, MESSAGE_BOARD_PATH.to_string()),
        Err(error) => MessageBoardSummary {
            path: MESSAGE_BOARD_PATH.to_string(),
            inbox_count: 0,
            completed_count: 0,
            inbox: Vec::new(),
            completed: Vec::new(),
            errors: vec![format!("{}: {error}", path.display())],
        },
    }
}

fn parse_message_board(text: &str, path: String) -> MessageBoardSummary {
    let mut errors = Vec::new();
    let inbox = match find_section_range(text, "## Inbox") {
        Some(range) => parse_request_entries(&text[range]),
        None => {
            errors.push("missing ## Inbox section".to_string());
            Vec::new()
        }
    };
    let completed = match find_section_range(text, "## Completed Notes") {
        Some(range) => parse_request_entries(&text[range]),
        None => {
            errors.push("missing ## Completed Notes section".to_string());
            Vec::new()
        }
    };

    MessageBoardSummary {
        path,
        inbox_count: inbox.len(),
        completed_count: completed.len(),
        inbox,
        completed,
        errors,
    }
}

fn add_message_board_request(
    root: &Path,
    id: Option<String>,
    title: &str,
    request: &str,
    context: &str,
    expected: &str,
) -> Result<MessageBoardRequest, String> {
    let path = message_board_path(root);
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let id = id.unwrap_or_else(|| generated_request_id(&text, title));
    let request = MessageBoardRequest {
        id,
        title: title.trim().to_string(),
        request: request.trim().to_string(),
        context: context.trim().to_string(),
        expected_output: expected.trim().to_string(),
        raw: String::new(),
    };
    let entry = format_request_entry(&request, None);
    let text = insert_entry_into_section(&text, "## Inbox", &entry)?;
    fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(MessageBoardRequest {
        raw: entry,
        ..request
    })
}

fn complete_message_board_request(
    root: &Path,
    id: &str,
    result: &str,
) -> Result<MessageBoardRequest, String> {
    let path = message_board_path(root);
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let inbox_range = find_section_range(&text, "## Inbox")
        .ok_or_else(|| "missing ## Inbox section".to_string())?;
    let entries = request_entry_ranges(&text, inbox_range);
    let (request, range) = entries
        .into_iter()
        .find(|(request, _range)| request.id == id)
        .ok_or_else(|| format!("request id not found in Inbox: {id}"))?;

    let completed_entry = format_request_entry(&request, Some(result));
    let mut text_without_request = String::with_capacity(text.len() + completed_entry.len());
    text_without_request.push_str(&text[..range.start]);
    text_without_request.push_str(&text[range.end..]);
    let updated = insert_entry_into_section(
        &text_without_request,
        "## Completed Notes",
        &completed_entry,
    )?;
    fs::write(&path, updated).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(MessageBoardRequest {
        raw: completed_entry,
        ..request
    })
}

fn message_board_path(root: &Path) -> PathBuf {
    root.join(MESSAGE_BOARD_PATH)
}

fn generated_request_id(text: &str, title: &str) -> String {
    let base = slugify(title).unwrap_or_else(|| {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        format!("feature-{seconds}")
    });
    let summary = parse_message_board(text, MESSAGE_BOARD_PATH.to_string());
    let existing_ids = summary
        .inbox
        .iter()
        .chain(summary.completed.iter())
        .map(|request| request.id.as_str())
        .collect::<Vec<_>>();

    if !existing_ids.contains(&base.as_str()) {
        return base;
    }

    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if !existing_ids.contains(&candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search always returns")
}

fn slugify(value: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn format_request_entry(request: &MessageBoardRequest, result: Option<&str>) -> String {
    let mut lines = vec![
        format!("### {} - {}", request.id, request.title),
        String::new(),
        "Request:".to_string(),
        request.request.clone(),
        String::new(),
        "Context:".to_string(),
        request.context.clone(),
        String::new(),
        "Expected output:".to_string(),
        request.expected_output.clone(),
    ];

    if let Some(result) = result {
        lines.extend([
            String::new(),
            "Result:".to_string(),
            result.trim().to_string(),
        ]);
    }

    lines.push(String::new());
    lines.push(String::new());
    lines.join("\n")
}

fn insert_entry_into_section(
    text: &str,
    section_heading: &str,
    entry: &str,
) -> Result<String, String> {
    let section_range = find_section_range(text, section_heading)
        .ok_or_else(|| format!("missing {section_heading} section"))?;
    let section = &text[section_range.clone()];
    let relative_offset = first_request_entry_offset(section)
        .or_else(|| first_fence_offset(section))
        .unwrap_or(section.len());
    let offset = section_range.start + relative_offset;

    let mut updated = String::with_capacity(text.len() + entry.len() + 4);
    updated.push_str(&text[..offset]);
    if !updated.ends_with("\n\n") {
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push('\n');
    }
    updated.push_str(entry);
    if !text[offset..].starts_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&text[offset..]);
    Ok(updated)
}

fn find_section_range(text: &str, heading: &str) -> Option<Range<usize>> {
    let mut offset = 0;
    let mut start = None;
    let mut in_fence = false;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        }

        if !in_fence && trimmed == heading {
            start = Some(offset + line.len());
        } else if start.is_some() && !in_fence && trimmed.starts_with("## ") {
            return start.map(|start| start..offset);
        }

        offset += line.len();
    }

    start.map(|start| start..text.len())
}

fn parse_request_entries(section: &str) -> Vec<MessageBoardRequest> {
    request_entry_ranges(section, 0..section.len())
        .into_iter()
        .map(|(request, _range)| request)
        .collect()
}

fn request_entry_ranges(
    text: &str,
    section_range: Range<usize>,
) -> Vec<(MessageBoardRequest, Range<usize>)> {
    let section = &text[section_range.clone()];
    let mut entries = Vec::new();
    let mut current_start = None;
    let mut current_raw = String::new();
    let mut offset = section_range.start;
    let mut in_fence = false;

    for line in section.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if !in_fence {
                if let Some(start) = current_start {
                    push_request_entry(&mut entries, &current_raw, start..offset);
                    current_raw.clear();
                    current_start = None;
                }
            }
            in_fence = !in_fence;
            offset += line.len();
            continue;
        }

        if !in_fence && trimmed.starts_with("### ") {
            if let Some(start) = current_start {
                push_request_entry(&mut entries, &current_raw, start..offset);
                current_raw.clear();
            }
            current_start = Some(offset);
        }

        if current_start.is_some() {
            current_raw.push_str(line);
        }
        offset += line.len();
    }

    if let Some(start) = current_start {
        push_request_entry(&mut entries, &current_raw, start..offset);
    }

    entries
}

fn push_request_entry(
    entries: &mut Vec<(MessageBoardRequest, Range<usize>)>,
    raw: &str,
    range: Range<usize>,
) {
    if let Some(request) = parse_request_entry(raw) {
        entries.push((request, range));
    }
}

fn parse_request_entry(raw: &str) -> Option<MessageBoardRequest> {
    let heading = raw
        .lines()
        .find_map(|line| line.trim().strip_prefix("### "))?
        .trim();
    let (id, title) = heading
        .split_once(" - ")
        .map(|(id, title)| (id.trim(), title.trim()))
        .unwrap_or((heading, heading));
    Some(MessageBoardRequest {
        id: id.to_string(),
        title: title.to_string(),
        request: labeled_field(raw, "Request:"),
        context: labeled_field(raw, "Context:"),
        expected_output: labeled_field(raw, "Expected output:"),
        raw: raw.trim().to_string(),
    })
}

fn labeled_field(raw: &str, label: &str) -> String {
    let mut capture = false;
    let mut lines = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if capture
            && matches!(
                trimmed,
                "Request:" | "Context:" | "Expected output:" | "Result:"
            )
        {
            break;
        }
        if capture {
            lines.push(line);
        } else if trimmed == label {
            capture = true;
        }
    }

    trim_blank_lines(lines).join("\n")
}

fn trim_blank_lines(lines: Vec<&str>) -> Vec<String> {
    let mut start = 0;
    let mut end = lines.len();
    while start < end && lines[start].trim().is_empty() {
        start += 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[start..end]
        .iter()
        .map(|line| line.trim_end().to_string())
        .collect()
}

fn first_request_entry_offset(section: &str) -> Option<usize> {
    let mut offset = 0;
    let mut in_fence = false;
    for line in section.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence && trimmed.starts_with("### ") {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

fn first_fence_offset(section: &str) -> Option<usize> {
    let mut offset = 0;
    for line in section.split_inclusive('\n') {
        if line.trim().starts_with("```") {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

pub fn git_status_summary(root: &Path) -> GitStatusSummary {
    let remote_origin = run_git(root, &["remote", "get-url", "origin"]).ok();
    let status = match run_git(root, &["status", "--porcelain=v1", "-b"]) {
        Ok(status) => status,
        Err(error) => {
            return GitStatusSummary {
                branch: None,
                upstream: None,
                remote_origin,
                expected_branch: EXPECTED_BRANCH.to_string(),
                expected_remote: EXPECTED_REMOTE.to_string(),
                branch_ok: false,
                remote_ok: false,
                dirty: true,
                modified: 0,
                deleted: 0,
                untracked: 0,
                renamed: 0,
                other: 0,
                total_changed: 0,
                sample: Vec::new(),
                git_error: Some(error),
            };
        }
    };
    parse_git_status(&status, remote_origin)
}

pub fn parse_git_status(status: &str, remote_origin: Option<String>) -> GitStatusSummary {
    let mut branch = None;
    let mut upstream = None;
    let mut modified = 0;
    let mut deleted = 0;
    let mut untracked = 0;
    let mut renamed = 0;
    let mut other = 0;
    let mut sample = Vec::new();

    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let (branch_part, upstream_part) = rest
                .split_once("...")
                .map_or((rest, None), |(left, right)| (left, Some(right)));
            branch = Some(
                branch_part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string(),
            );
            upstream = upstream_part
                .map(|value| value.split_whitespace().next().unwrap_or("").to_string());
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        let status_code = &line[0..2];
        let path = line[3..].to_string();
        if sample.len() < 20 {
            sample.push(format!("{status_code} {path}"));
        }
        if status_code.contains('?') {
            untracked += 1;
        } else if status_code.contains('R') {
            renamed += 1;
        } else if status_code.contains('D') {
            deleted += 1;
        } else if status_code.contains('M') || status_code.contains('A') {
            modified += 1;
        } else {
            other += 1;
        }
    }

    let total_changed = modified + deleted + untracked + renamed + other;
    let remote_ok = remote_origin.as_deref().is_some_and(|remote| {
        remote == EXPECTED_REMOTE || remote == EXPECTED_REMOTE.trim_end_matches(".git")
    });
    let branch_ok = branch.as_deref() == Some(EXPECTED_BRANCH);

    GitStatusSummary {
        branch,
        upstream,
        remote_origin,
        expected_branch: EXPECTED_BRANCH.to_string(),
        expected_remote: EXPECTED_REMOTE.to_string(),
        branch_ok,
        remote_ok,
        dirty: total_changed > 0,
        modified,
        deleted,
        untracked,
        renamed,
        other,
        total_changed,
        sample,
        git_error: None,
    }
}

fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn artifact_checks(root: &Path) -> Vec<DoctorCheck> {
    artifact_paths()
        .into_iter()
        .map(|(name, relative)| {
            let path = root.join(relative);
            DoctorCheck {
                name: name.to_string(),
                ok: path.exists(),
                detail: relative.to_string(),
            }
        })
        .collect()
}

fn artifact_paths() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "mole current graph",
            "docs/state_graphs/mole_current_graph.json",
        ),
        (
            "melee reference graph",
            "docs/state_graphs/melee_reference_graph.json",
        ),
        (
            "value diff report",
            "docs/state_graphs/parity_reports/value_diffs.json",
        ),
        (
            "falcon ecb coverage",
            "docs/state_graphs/parity_reports/falcon_ecb_coverage.json",
        ),
        (
            "falcon generated ecb rust",
            "crates/mole_core/src/generated/falcon_ecb.rs",
        ),
        ("state graph viewer", "tools/state_graph_viewer.py"),
        (
            "slippi replay converter",
            "tools/slippi_replay_to_inputs.cjs",
        ),
        (
            "captain falcon extracted action ecb samples",
            "resources/melee/extracted/captain_falcon_action_ecb_samples.json",
        ),
        (
            "captain falcon extracted profile",
            "resources/melee/extracted/captain_falcon_profile.json",
        ),
    ]
}

pub fn generated_artifact_statuses(root: &Path) -> Vec<GeneratedArtifactStatus> {
    generated_artifact_paths()
        .into_iter()
        .map(|(_name, relative)| generated_artifact_status(root, relative))
        .collect()
}

fn generated_artifact_paths() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "mole current graph",
            "docs/state_graphs/mole_current_graph.json",
        ),
        (
            "melee reference graph",
            "docs/state_graphs/melee_reference_graph.json",
        ),
        (
            "value diff report",
            "docs/state_graphs/parity_reports/value_diffs.json",
        ),
        (
            "falcon ecb coverage",
            "docs/state_graphs/parity_reports/falcon_ecb_coverage.json",
        ),
        (
            "global value sheet",
            "docs/state_graphs/value_sheets/global_common_values.json",
        ),
        (
            "captain falcon value sheet",
            "docs/state_graphs/value_sheets/captain_falcon_values.json",
        ),
        (
            "physics engine value sheet",
            "docs/state_graphs/value_sheets/physics_engine_values.json",
        ),
        (
            "global combat value sheet",
            "docs/state_graphs/value_sheets/global_combat_values.json",
        ),
        (
            "captain falcon combat value sheet",
            "docs/state_graphs/value_sheets/captain_falcon_combat_values.json",
        ),
        (
            "battlefield stage value sheet",
            "docs/state_graphs/value_sheets/battlefield_stage_values.json",
        ),
        (
            "battlefield stage asset",
            "resources/melee/extracted/stages/battlefield_stage.json",
        ),
        (
            "parity ledger map",
            "docs/state_graphs/parity_ledger_map.json",
        ),
        (
            "falcon generated ecb rust",
            "crates/mole_core/src/generated/falcon_ecb.rs",
        ),
        (
            "captain falcon extracted action ecb samples",
            "resources/melee/extracted/captain_falcon_action_ecb_samples.json",
        ),
        (
            "captain falcon extracted profile",
            "resources/melee/extracted/captain_falcon_profile.json",
        ),
        (
            "plco extracted common data",
            "resources/melee/extracted/plco_common_data.json",
        ),
    ]
}

fn generated_artifact_status(root: &Path, relative: &str) -> GeneratedArtifactStatus {
    let exists = root.join(relative).exists();
    match run_git(root, &["status", "--porcelain=v1", "--", relative]) {
        Ok(output) => {
            let git_status = output
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            GeneratedArtifactStatus {
                path: relative.to_string(),
                exists,
                dirty: !git_status.is_empty(),
                git_status,
                error: None,
            }
        }
        Err(error) => GeneratedArtifactStatus {
            path: relative.to_string(),
            exists,
            dirty: false,
            git_status: Vec::new(),
            error: Some(error),
        },
    }
}

pub fn parity_summary(root: &Path) -> ParitySummary {
    let mut errors = Vec::new();
    let value_report = read_json(root.join("docs/state_graphs/parity_reports/value_diffs.json"))
        .inspect_err(|error| errors.push(format!("value_diffs.json: {error}")))
        .ok();
    let ecb_report =
        read_json(root.join("docs/state_graphs/parity_reports/falcon_ecb_coverage.json"))
            .inspect_err(|error| errors.push(format!("falcon_ecb_coverage.json: {error}")))
            .ok();
    let graph = read_json(root.join("docs/state_graphs/mole_current_graph.json"))
        .inspect_err(|error| errors.push(format!("mole_current_graph.json: {error}")))
        .ok();

    let mut value_sections = BTreeMap::new();
    let mut value_total_rows = 0;
    let mut value_matches = 0;
    let mut value_actionable = 0;
    let mut value_derived = 0;

    if let Some(report) = value_report {
        if let Some(sections) = report.get("sections").and_then(Value::as_object) {
            for (name, section) in sections {
                let rows = section
                    .get("rows")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let matches = section
                    .get("rows")
                    .and_then(Value::as_array)
                    .map(|rows| {
                        rows.iter()
                            .filter(|row| {
                                row.get("status").and_then(Value::as_str) == Some("match")
                            })
                            .count()
                    })
                    .unwrap_or(0);
                let actionable = section
                    .get("actionable_rows")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let derived = section
                    .get("derived_rows")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                value_total_rows += rows;
                value_matches += matches;
                value_actionable += actionable;
                value_derived += derived;
                value_sections.insert(
                    name.clone(),
                    ValueSectionSummary {
                        rows,
                        matches,
                        actionable,
                        derived,
                    },
                );
            }
        }
    }

    let ecb_mapped_motion_states = ecb_report
        .as_ref()
        .and_then(|report| report.get("mapped_motion_state_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let ecb_missing_sampled_mappings = ecb_report
        .as_ref()
        .and_then(|report| report.get("missing_sampled_mappings"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let ecb_unmapped_derived_states = ecb_report
        .as_ref()
        .and_then(|report| report.get("unmapped_derived_motion_states"))
        .and_then(Value::as_array)
        .map(|states| {
            states
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let graph_nodes = graph
        .as_ref()
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let graph_edges = graph
        .as_ref()
        .and_then(|graph| graph.get("edges"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let graph_status_counts = graph.as_ref().map(count_graph_statuses).unwrap_or_default();

    ParitySummary {
        value_sections,
        value_total_rows,
        value_matches,
        value_actionable,
        value_derived,
        ecb_mapped_motion_states,
        ecb_missing_sampled_mappings,
        ecb_unmapped_derived_states,
        graph_nodes,
        graph_edges,
        graph_status_counts,
        errors,
    }
}

fn ledger_map_summary(root: &Path) -> Value {
    let path = root.join("docs/state_graphs/parity_ledger_map.json");
    match LedgerMap::load(&path) {
        Ok(ledger_map) => serde_json::to_value(ledger_map).expect("ledger map serializes"),
        Err(error) => json!({
            "schema_version": 1,
            "path": path.display().to_string(),
            "exists": path.exists(),
            "loadable": false,
            "error": error,
            "registry": {
                "tab_count": 0,
                "active_tab_count": 0,
                "planned_tab_count": 0,
                "dual_surface": false,
            },
            "tabs": [],
        }),
    }
}

pub(crate) fn read_json(path: PathBuf) -> Result<Value, io::Error> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn count_graph_statuses(graph: &Value) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for collection in ["nodes", "edges"] {
        if let Some(items) = graph.get(collection).and_then(Value::as_array) {
            for item in items {
                if let Some(status) = item.get("status").and_then(Value::as_str) {
                    *counts.entry(status.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

fn doctor_checks(root: &Path) -> Vec<DoctorCheck> {
    let mut checks = artifact_checks(root);
    checks.extend([
        command_check("git", &["--version"]),
        command_check("cargo", &["--version"]),
        command_check("node", &["--version"]),
        DoctorCheck {
            name: "project python venv".to_string(),
            ok: root.join(".venv/Scripts/python.exe").exists(),
            detail: ".venv/Scripts/python.exe".to_string(),
        },
    ]);
    checks
}

fn command_check(command: &str, args: &[&str]) -> DoctorCheck {
    match Command::new(command).args(args).output() {
        Ok(output) if output.status.success() => DoctorCheck {
            name: format!("{command} available"),
            ok: true,
            detail: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        },
        Ok(output) => DoctorCheck {
            name: format!("{command} available"),
            ok: false,
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        },
        Err(error) => DoctorCheck {
            name: format!("{command} available"),
            ok: false,
            detail: error.to_string(),
        },
    }
}

fn recommended_test_commands() -> Vec<&'static str> {
    vec![
        "cargo test --workspace",
        ".venv\\Scripts\\python.exe -m pytest tests\\test_state_graph_viewer.py tests\\test_launch_inputs.py tests\\test_parity_diff_report.py tests\\test_generate_falcon_ecb_rust.py -q",
        ".venv\\Scripts\\python.exe tools\\state_graph_viewer.py --check",
        "cargo run -p mole_cli -- generated write-value-sheets --write --json",
        "cargo run -p mole_cli -- generated write-stage-asset --stage battlefield --write --json",
        "cargo run -p mole_cli -- generated write-ledger-map --write --json",
        "cargo run -p mole_cli -- devtool ledger --json",
        "cargo run -p mole_devtool",
        "cargo run -p mole_cli -- parity --json",
        "cargo run -p mole_cli -- parity snapshot --json",
    ]
}

fn recommended_next(root: &Path) -> Vec<String> {
    let git = git_status_summary(root);
    let parity = parity_summary(root);
    let ledger_map = ledger_map_summary(root);
    let mut recommendations = Vec::new();

    if !git.branch_ok {
        recommendations.push(format!(
            "Re-anchor branch before editing: expected `{}`, got `{}`.",
            EXPECTED_BRANCH,
            git.branch.as_deref().unwrap_or("unknown")
        ));
    }
    if !git.remote_ok {
        recommendations.push(format!(
            "Re-anchor remote before editing: expected `{}`.",
            EXPECTED_REMOTE
        ));
    }
    if parity.value_actionable > 0 {
        recommendations.push(format!(
            "Resolve {} actionable value parity rows before tuning feel.",
            parity.value_actionable
        ));
    }
    if parity.ecb_missing_sampled_mappings > 0 {
        recommendations.push(format!(
            "Regenerate or map {} missing sampled ECB actions.",
            parity.ecb_missing_sampled_mappings
        ));
    }
    if !parity.ecb_unmapped_derived_states.is_empty() {
        recommendations.push(format!(
            "Replace or explicitly justify unmapped derived states: {}.",
            parity.ecb_unmapped_derived_states.join(", ")
        ));
    }
    if let Some(missing) = parity.graph_status_counts.get("missing") {
        if *missing > 0 {
            recommendations.push(format!(
                "Continue bottom-up decomp parity for {missing} graph entries marked missing."
            ));
        }
    }
    if !ledger_map
        .get("loadable")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        recommendations.push("Regenerate the parity ledger map so Rust can load the owned contract for CLI and future GUI consumers.".to_string());
    } else if !ledger_map
        .get("registry")
        .and_then(Value::as_object)
        .and_then(|registry| registry.get("dual_surface"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        recommendations.push(
            "Restore CLI/GUI dual-surface parity in the ledger map before expanding the dev tool."
                .to_string(),
        );
    }
    if recommendations.is_empty() {
        recommendations.push("No high-priority structural gap detected by Mole CLI.".to_string());
    }
    recommendations
}
