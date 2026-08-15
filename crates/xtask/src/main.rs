use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use anyhow::{Context, Result, bail};
use beastie_core::{ACTIVE_DAY_MS, GameEvent, MemoryId, SeededRandom, WorldState, step};
use beastie_protocol::{DialogueReply, DialogueRequest, Gesture, PROTOCOL_VERSION, validate_reply};
use beastie_session::{
    GameSession, MAX_COMMAND_BYTES, Observation, SESSION_PROTOCOL_VERSION, SessionError,
};
use clap::{Parser, Subcommand};
use serde::Serialize;

const BERRY_GRUDGE_SCENARIO: &str = "fixtures/scenarios/berry-grudge.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Beastie developer tasks")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    /// Run every headless verification gate.
    Verify,
    /// Run the game shell with fixture-backed AI.
    Dev {
        #[arg(long)]
        fake_ai: bool,
        #[arg(long)]
        smoke: bool,
    },
    /// Run the deterministic simulation without rendering.
    Sim {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 3)]
        days: u32,
    },
    /// Drive a headless game session with versioned JSONL commands.
    Play {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long)]
        scenario: Option<PathBuf>,
        #[arg(long)]
        fake_ai: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Task::Verify => verify(),
        Task::Dev { fake_ai, smoke } => dev(fake_ai, smoke),
        Task::Sim { seed, days } => sim(seed, days),
        Task::Play {
            seed,
            scenario,
            fake_ai,
        } => play(seed, scenario.as_deref(), fake_ai),
    }
}

fn verify() -> Result<()> {
    verify_manifest("assets/manifest.toml")?;
    verify_manifest("models/manifest.toml")?;
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["test", "--workspace", "--locked"])?;
    run("cargo", &["build", "--workspace", "--locked"])?;
    replay_scenario(BERRY_GRUDGE_SCENARIO)
}

fn dev(fake_ai: bool, smoke: bool) -> Result<()> {
    if !fake_ai {
        bail!("only --fake-ai is available until a local model backend is integrated");
    }
    run(
        "cargo",
        &["build", "--package", "beastie-ai-worker", "--locked"],
    )?;
    let worker = std::env::current_dir()
        .context("failed to locate repository root")?
        .join("target")
        .join("debug")
        .join(format!("beastie-ai-worker{}", std::env::consts::EXE_SUFFIX));
    let mut arguments = vec![
        "run",
        "--package",
        "beastie-game",
        "--locked",
        "--",
        "--fake-ai",
    ];
    if smoke {
        arguments.push("--smoke");
    }
    let status = Command::new("cargo")
        .args(arguments)
        .env("BEASTIE_AI_WORKER", worker)
        .status()
        .context("failed to start cargo")?;
    require_success("cargo", status)
}

fn sim(seed: u64, days: u32) -> Result<()> {
    let mut world = WorldState::new(seed, "Mop");
    let mut rng = SeededRandom::new(seed);
    let minutes_per_day = ACTIVE_DAY_MS / 60_000;
    let minutes = u64::from(days) * minutes_per_day;
    for _ in 0..minutes {
        step(&mut world, &[], 60_000, &mut rng);
    }
    serde_json::to_writer_pretty(std::io::stdout(), &world)
        .context("failed to write simulation result")?;
    Ok(())
}

fn play(seed: u64, scenario: Option<&Path>, fake_ai: bool) -> Result<()> {
    if !fake_ai {
        bail!("--fake-ai is required until a local model backend is integrated");
    }
    let mut output = io::stdout().lock();
    match scenario {
        Some(path) => {
            let file = File::open(path)
                .with_context(|| format!("failed to open scenario {}", path.display()))?;
            process_commands(BufReader::new(file), &mut output, seed).map(|_| ())
        }
        None => process_commands(io::stdin().lock(), &mut output, seed).map(|_| ()),
    }
}

fn replay_scenario(path: &str) -> Result<()> {
    let file = File::open(path).with_context(|| format!("failed to open scenario {path}"))?;
    let report = process_commands(BufReader::new(file), io::sink(), 1)?;
    if report.accepted == 0 {
        bail!("scenario {path} contains no commands");
    }
    if report.rejected > 0 {
        bail!(
            "scenario {path} rejected {} of {} commands",
            report.rejected,
            report.accepted + report.rejected
        );
    }
    if !report.grounded_reply || !report.food_rejected {
        bail!("scenario {path} did not produce grounded recall and berry rejection");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PlayReport {
    accepted: usize,
    rejected: usize,
    grounded_reply: bool,
    food_rejected: bool,
}

fn process_commands(
    mut input: impl BufRead,
    mut output: impl Write,
    seed: u64,
) -> Result<PlayReport> {
    let mut session = GameSession::new(seed, "Mop");
    let mut report = PlayReport::default();
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        let read = input
            .read_until(b'\n', &mut bytes)
            .context("failed to read session command")?;
        if read == 0 {
            break;
        }
        trim_line_ending(&mut bytes);
        if bytes.is_empty() {
            continue;
        }
        let result = if bytes.len() > MAX_COMMAND_BYTES {
            Err(AdapterError::new(
                "command_too_large",
                format!("command exceeds {MAX_COMMAND_BYTES} bytes"),
            ))
        } else {
            process_command(&mut session, &bytes)
        };
        match result {
            Ok(observation) => {
                report.grounded_reply |= observation.recalled_memory.is_some();
                report.food_rejected |= observation
                    .observation
                    .events
                    .iter()
                    .any(|event| matches!(event, GameEvent::FoodRejected(_)));
                serde_json::to_writer(&mut output, &observation)
                    .context("failed to encode session observation")?;
                report.accepted += 1;
            }
            Err(error) => {
                serde_json::to_writer(&mut output, &ErrorOutput::from(error))
                    .context("failed to encode session error")?;
                report.rejected += 1;
            }
        }
        output
            .write_all(b"\n")
            .context("failed to terminate JSONL output")?;
        output.flush().context("failed to flush JSONL output")?;
    }
    Ok(report)
}

fn trim_line_ending(bytes: &mut Vec<u8>) {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
}

fn process_command(
    session: &mut GameSession,
    bytes: &[u8],
) -> std::result::Result<PlayOutput, AdapterError> {
    let line = std::str::from_utf8(bytes)
        .map_err(|error| AdapterError::new("invalid_utf8", error.to_string()))?;
    let command = GameSession::parse_command(line).map_err(AdapterError::from_session)?;
    let observation = session.apply(command).map_err(AdapterError::from_session)?;
    let (speech, recalled_memory) = observation
        .dialogue_request
        .as_ref()
        .map(fixture_reply)
        .transpose()?
        .map_or((None, None), |reply| {
            (Some(reply.say), reply.recalled_memory)
        });
    Ok(PlayOutput {
        observation,
        accepted: true,
        speech,
        recalled_memory,
    })
}

fn fixture_reply(request: &DialogueRequest) -> std::result::Result<DialogueReply, AdapterError> {
    let memory = request.candidate_memories.first();
    let reply = DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say: memory.map_or_else(
            || "hm. no old thought.".to_owned(),
            |_| "yes. old thing remains.".to_owned(),
        ),
        gesture: if request
            .constraints
            .allowed_gestures
            .contains(&Gesture::LookPlayer)
        {
            Gesture::LookPlayer
        } else {
            request
                .constraints
                .allowed_gestures
                .iter()
                .next()
                .copied()
                .unwrap_or(Gesture::None)
        },
        recalled_memory: memory.map(|candidate| candidate.id),
    };
    validate_reply(request, reply).map_err(|error| {
        AdapterError::new(
            "fixture_reply_invalid",
            format!("fixture AI failed: {error}"),
        )
    })
}

#[derive(Debug, Serialize)]
struct PlayOutput {
    #[serde(flatten)]
    observation: Observation,
    accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    speech: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recalled_memory: Option<MemoryId>,
}

#[derive(Debug)]
struct AdapterError {
    code: &'static str,
    message: String,
}

impl AdapterError {
    fn new(code: &'static str, message: String) -> Self {
        Self { code, message }
    }

    fn from_session(error: SessionError) -> Self {
        let code = match error {
            SessionError::Version(_) => "unsupported_version",
            SessionError::CommandTooLarge => "command_too_large",
            SessionError::Json(_) => "malformed_command",
            SessionError::Advance(_) => "invalid_advance",
            SessionError::TalkTooLong => "talk_too_long",
            SessionError::NoCheckpoint => "no_checkpoint",
            SessionError::Save(_) => "save_failed",
            SessionError::State(_) => "invalid_state",
            SessionError::Dialogue(_) => "invalid_dialogue_request",
        };
        Self::new(code, error.to_string())
    }
}

#[derive(Debug, Serialize)]
struct ErrorOutput {
    version: u32,
    accepted: bool,
    error: ErrorDetail,
}

impl From<AdapterError> for ErrorOutput {
    fn from(error: AdapterError) -> Self {
        Self {
            version: SESSION_PROTOCOL_VERSION,
            accepted: false,
            error: ErrorDetail {
                code: error.code,
                message: error.message,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

fn run(program: &str, arguments: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(arguments)
        .status()
        .with_context(|| format!("failed to start {program}"))?;
    require_success(program, status)
}

fn require_success(program: &str, status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("{program} exited with {status}")
    }
}

fn verify_manifest(path: &str) -> Result<()> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("failed to read manifest {path}"))?;
    let value: toml::Value =
        toml::from_str(&source).with_context(|| format!("failed to parse manifest {path}"))?;
    if value.get("version").and_then(toml::Value::as_integer) != Some(1) {
        bail!("manifest {path} must declare version = 1");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_commands_emit_errors_and_do_not_stop_the_stream() {
        let input = b"not json\n{\"version\":1,\"command\":\"inspect\"}\n";
        let mut output = Vec::new();
        let report = process_commands(&input[..], &mut output, 42).expect("stream should run");
        let lines = String::from_utf8(output).expect("output should be UTF-8");
        let values = lines
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSON"))
            .collect::<Vec<_>>();

        assert_eq!(report.accepted, 1);
        assert_eq!(report.rejected, 1);
        assert_eq!(values[0]["accepted"], false);
        assert_eq!(values[0]["error"]["code"], "malformed_command");
        assert_eq!(values[1]["accepted"], true);
        assert_eq!(values[1]["sequence"], 1);
    }

    #[test]
    fn talk_output_is_grounded_in_an_offered_memory() {
        let input = include_bytes!("../../../fixtures/scenarios/berry-grudge.jsonl");
        let mut output = Vec::new();
        let report = process_commands(&input[..], &mut output, 1).expect("scenario should run");
        let lines = String::from_utf8(output).expect("output should be UTF-8");
        let talk = lines
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSON"))
            .find(|value| value.get("speech").is_some())
            .expect("scenario should talk");

        assert_eq!(report.rejected, 0);
        assert!(report.grounded_reply);
        assert!(report.food_rejected);
        let recalled = talk["recalled_memory"].clone();
        assert!(
            talk["dialogue_request"]["candidate_memories"]
                .as_array()
                .expect("candidate list")
                .iter()
                .any(|memory| memory["id"] == recalled)
        );
    }

    #[test]
    fn simulation_day_uses_the_accelerated_active_day() {
        assert_eq!(ACTIVE_DAY_MS / 60_000, 15);
    }
}
