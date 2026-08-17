use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FEEL_SCHEMA_VERSION: u32 = 1;
const MANIFEST_VERSION: u32 = 2;
const RUN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeelSuite {
    Baseline,
    FirstFiveMinutes,
    QuietObservation,
    InteractionChain,
    BadConditions,
    RelationshipOverTime,
}

#[derive(Debug)]
pub struct FeelOptions<'a> {
    pub suite: FeelSuite,
    pub output: Option<&'a Path>,
    pub game: Option<&'a Path>,
}

#[derive(Debug, Clone, Copy)]
struct Experience {
    id: &'static str,
    scenario: &'static str,
    fake_ai: bool,
    tts_requested: bool,
}

const FIRST_FIVE_MINUTES: Experience = Experience {
    id: "first-five-minutes",
    scenario: "fixtures/scenarios/feel/first-five-minutes.jsonl",
    fake_ai: true,
    tts_requested: true,
};
const QUIET_OBSERVATION: Experience = Experience {
    id: "quiet-observation",
    scenario: "fixtures/scenarios/feel/quiet-observation.jsonl",
    fake_ai: true,
    tts_requested: false,
};
const INTERACTION_CHAIN: Experience = Experience {
    id: "interaction-chain",
    scenario: "fixtures/scenarios/feel/interaction-chain.jsonl",
    fake_ai: true,
    tts_requested: true,
};
const BAD_CONDITIONS: Experience = Experience {
    id: "bad-conditions",
    scenario: "fixtures/scenarios/feel/bad-conditions.jsonl",
    // Deliberately omit the worker so dialogue exercises the authored technical fallback.
    fake_ai: false,
    tts_requested: true,
};
const RELATIONSHIP_OVER_TIME: Experience = Experience {
    id: "relationship-over-time",
    scenario: "fixtures/scenarios/feel/relationship-over-time.jsonl",
    fake_ai: true,
    tts_requested: true,
};
const RELATIONSHIP_OVER_TIME_NO_AI: Experience = Experience {
    id: "relationship-over-time-no-ai",
    scenario: "fixtures/scenarios/feel/relationship-over-time-no-ai.jsonl",
    fake_ai: false,
    tts_requested: false,
};

#[derive(Debug, Deserialize, Serialize)]
struct Marker {
    version: u32,
    name: String,
    playback_ms: u64,
    simulation_ms: u64,
}

#[derive(Debug, Serialize)]
struct Artifact {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct Manifest<'a> {
    version: u32,
    experience: &'a str,
    suite: FeelSuite,
    scenario: &'a str,
    scenario_sha256: String,
    commit: String,
    working_tree_dirty: bool,
    game_binary_sha256: String,
    platform: &'static str,
    build_profile: &'static str,
    fake_ai: bool,
    tts_requested: bool,
    seed: u64,
    video_fps: u32,
    presentation: &'static str,
    audible_mix_captured: bool,
    reference_mix_generated: bool,
    artifacts: Vec<Artifact>,
}

pub fn run(options: FeelOptions<'_>) -> Result<()> {
    preflight_tool("ffmpeg")?;
    preflight_tool("ffprobe")?;
    build_binaries()?;

    let game = options.game.map_or_else(
        || binary_path("beastie-game"),
        |path| {
            ensure!(
                path.is_file(),
                "feel game executable is missing: {}",
                path.display()
            );
            Ok(path.to_path_buf())
        },
    )?;

    let output = match options.output {
        Some(path) => path.to_path_buf(),
        None => default_output_dir()?,
    };
    ensure_new_directory(&output)?;

    let experiences = experiences(options.suite);
    for experience in experiences.iter().copied() {
        run_experience(experience, options.suite, &game, &output)?;
    }
    write_suite_index(&output, &experiences)?;
    eprintln!("feel evidence: {}", output.display());
    Ok(())
}

fn experiences(suite: FeelSuite) -> Vec<Experience> {
    match suite {
        FeelSuite::Baseline => vec![
            FIRST_FIVE_MINUTES,
            QUIET_OBSERVATION,
            INTERACTION_CHAIN,
            BAD_CONDITIONS,
            RELATIONSHIP_OVER_TIME,
            RELATIONSHIP_OVER_TIME_NO_AI,
        ],
        FeelSuite::FirstFiveMinutes => vec![FIRST_FIVE_MINUTES],
        FeelSuite::QuietObservation => vec![QUIET_OBSERVATION],
        FeelSuite::InteractionChain => vec![INTERACTION_CHAIN],
        FeelSuite::BadConditions => vec![BAD_CONDITIONS],
        FeelSuite::RelationshipOverTime => {
            vec![RELATIONSHIP_OVER_TIME, RELATIONSHIP_OVER_TIME_NO_AI]
        }
    }
}

fn run_experience(
    experience: Experience,
    suite: FeelSuite,
    game: &Path,
    output: &Path,
) -> Result<()> {
    let scenario = Path::new(experience.scenario);
    ensure!(
        scenario.is_file(),
        "feel scenario is missing: {}",
        scenario.display()
    );
    let directory = output.join(experience.id);
    fs::create_dir_all(directory.join("captures"))
        .with_context(|| format!("failed to create {}", directory.display()))?;

    let mut command = Command::new(game);
    command
        .arg("--script")
        .arg(scenario)
        .arg("--capture-dir")
        .arg(directory.join("captures"))
        .arg("--feel-dir")
        .arg(&directory);
    if experience.tts_requested {
        command
            .arg("--tts")
            .env("BEASTIE_TTS_WORKER", binary_path("beastie-tts")?)
            .env("BEASTIE_TTS_CACHE_DIR", directory.join("tts-cache"));
    }
    if experience.fake_ai {
        command
            .arg("--fake-ai")
            .env("BEASTIE_AI_WORKER", binary_path("beastie-ai-worker")?)
            .env("BEASTIE_STT_WORKER", binary_path("beastie-stt")?);
    } else {
        command.env_remove("BEASTIE_AI_WORKER");
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to launch feel experience {}", experience.id))?;
    focus_macos_process(child.id());
    wait_for_child(&mut child, experience.id)?;
    verify_video(&directory.join("session.mp4"))?;
    generate_reference_mix(&directory)?;
    let markers = read_markers(&directory.join("markers.jsonl"))?;
    validate_relationship_evidence(&directory, experience, &markers)?;
    generate_filmstrips(&directory, &markers)?;
    write_review(&directory, experience, &markers)?;
    write_manifest(&directory, experience, suite, game)?;
    Ok(())
}

fn build_binaries() -> Result<()> {
    require_success(
        "cargo build beastie-game",
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "--package",
                "beastie-game",
                "--bin",
                "beastie-game",
            ])
            .status()
            .context("failed to build beastie-game")?,
    )?;
    require_success(
        "cargo build feel workers",
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "--package",
                "beastie-ai-worker",
                "--bin",
                "beastie-ai-worker",
                "--bin",
                "beastie-stt",
                "--bin",
                "beastie-tts",
            ])
            .status()
            .context("failed to build feel workers")?,
    )
}

fn binary_path(name: &str) -> Result<PathBuf> {
    Ok(std::env::current_dir()
        .context("failed to locate repository root")?
        .join("target")
        .join("debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
}

fn default_output_dir() -> Result<PathBuf> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock predates the Unix epoch")?
        .as_secs();
    Ok(PathBuf::from("target")
        .join("feel")
        .join(format!("baseline-{seconds}")))
}

fn ensure_new_directory(path: &Path) -> Result<()> {
    if path.exists() {
        let mut entries =
            fs::read_dir(path).with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            entries.next().is_none(),
            "feel output must be new or empty: {}",
            path.display()
        );
    } else {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn preflight_tool(tool: &str) -> Result<()> {
    let status = Command::new(tool)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("{tool} is required for feel evidence"))?;
    require_success(tool, status)
}

fn wait_for_child(child: &mut std::process::Child, name: &str) -> Result<()> {
    let deadline = Instant::now() + RUN_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().context("failed to poll beastie-game")? {
            return require_success(name, status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "feel experience {name} did not finish within {} seconds",
                RUN_TIMEOUT.as_secs()
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn verify_video(path: &Path) -> Result<()> {
    ensure!(
        path.is_file(),
        "feel video was not produced: {}",
        path.display()
    );
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate,nb_frames",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
        .context("failed to inspect feel video")?;
    require_success("ffprobe", output.status)?;
    let description = String::from_utf8(output.stdout).context("ffprobe output was not UTF-8")?;
    ensure!(
        description.contains("width=640"),
        "feel video width is not 640"
    );
    ensure!(
        description.contains("height=360"),
        "feel video height is not 360"
    );
    ensure!(
        description.contains("avg_frame_rate=60/1"),
        "feel video is not 60fps: {description}"
    );
    Ok(())
}

fn generate_reference_mix(directory: &Path) -> Result<()> {
    let duration = video_duration_seconds(&directory.join("session.mp4"))?;
    let ambience = resolve_audio_asset("environment/underwater-loop")?;
    let mut inputs = vec![(ambience, 0_u64, 0.245_f32, true)];
    let trace = File::open(directory.join("audio.jsonl"))?;
    for (line_number, line) in BufReader::new(trace).lines().enumerate() {
        let value: serde_json::Value = serde_json::from_str(&line?)
            .with_context(|| format!("invalid audio trace line {}", line_number + 1))?;
        let playback_ms = value["playback_ms"].as_u64().unwrap_or_default();
        if let Some(cues) = value["cue_ids"].as_array() {
            for cue in cues.iter().filter_map(serde_json::Value::as_str) {
                let path = resolve_audio_asset(cue)?;
                let gain = if cue.starts_with("ui/") {
                    0.315
                } else if cue == "movement/swim-wake" {
                    0.378
                } else {
                    0.49
                };
                inputs.push((path, playback_ms, gain, false));
            }
        }
        if let Some(path) = value["path"].as_str() {
            let speech = directory.join(path);
            ensure!(
                speech.is_file(),
                "recorded speech is missing: {}",
                speech.display()
            );
            inputs.push((speech, playback_ms, 0.7, false));
        }
    }

    let mut command = Command::new("ffmpeg");
    command.args(["-y", "-v", "error"]);
    for (path, _, _, looping) in &inputs {
        if *looping {
            command.args(["-stream_loop", "-1"]);
        }
        command.arg("-i").arg(path);
    }
    let mut filters = Vec::new();
    let mut labels = String::new();
    for (index, (_, delay_ms, gain, _)) in inputs.iter().enumerate() {
        let label = format!("a{index}");
        filters.push(format!(
            "[{index}:a]volume={gain},adelay={delay_ms}:all=1[{label}]"
        ));
        labels.push_str(&format!("[{label}]"));
    }
    filters.push(format!(
        "{labels}amix=inputs={}:normalize=0:dropout_transition=0,alimiter=limit=0.95,atrim=duration={duration}[mix]",
        inputs.len()
    ));
    command
        .arg("-filter_complex")
        .arg(filters.join(";"))
        .args(["-map", "[mix]", "-c:a", "pcm_s16le"])
        .arg(directory.join("reference-mix.wav"));
    require_success(
        "ffmpeg feel reference mix",
        command
            .status()
            .context("failed to generate feel reference mix")?,
    )?;

    require_success(
        "ffmpeg feel reference mux",
        Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-i"])
            .arg(directory.join("session.mp4"))
            .arg("-i")
            .arg(directory.join("reference-mix.wav"))
            .args(["-c:v", "copy", "-c:a", "aac", "-shortest"])
            .arg(directory.join("session-audio-reference.mp4"))
            .status()
            .context("failed to mux feel reference audio")?,
    )?;
    require_success(
        "ffmpeg feel audio overview",
        Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-i"])
            .arg(directory.join("reference-mix.wav"))
            .args([
                "-filter_complex",
                "showwavespic=s=1600x240:split_channels=1:colors=0x77d1cd",
                "-frames:v",
                "1",
            ])
            .arg(directory.join("audio-overview.png"))
            .status()
            .context("failed to render feel audio overview")?,
    )
}

fn resolve_audio_asset(id: &str) -> Result<PathBuf> {
    for source in ["final", "generated"] {
        let path = Path::new("assets")
            .join(source)
            .join("audio")
            .join(format!("{id}.wav"));
        if path.is_file() {
            return Ok(path);
        }
    }
    bail!("feel audio asset is missing: {id}")
}

fn video_duration_seconds(video: &Path) -> Result<String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(video)
        .output()
        .context("failed to read feel video duration")?;
    require_success("ffprobe duration", output.status)?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn read_markers(path: &Path) -> Result<Vec<Marker>> {
    let file = File::open(path)
        .with_context(|| format!("feel markers are missing: {}", path.display()))?;
    let mut markers = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("failed to read marker line {}", index + 1))?;
        let marker = serde_json::from_str::<Marker>(&line)
            .with_context(|| format!("invalid marker line {}", index + 1))?;
        ensure!(
            marker.version == FEEL_SCHEMA_VERSION,
            "unsupported marker version"
        );
        markers.push(marker);
    }
    ensure!(!markers.is_empty(), "feel experience contains no markers");
    Ok(markers)
}

fn validate_relationship_evidence(
    directory: &Path,
    experience: Experience,
    markers: &[Marker],
) -> Result<()> {
    if !experience.id.starts_with("relationship-over-time") {
        return Ok(());
    }
    let duration_ms = video_duration_seconds(&directory.join("session.mp4"))?
        .parse::<f64>()
        .context("feel video duration is not numeric")?
        .mul_add(1_000.0, 0.0) as u64;
    let final_marker = markers
        .last()
        .context("relationship evidence has no final marker")?;
    ensure!(
        duration_ms >= final_marker.playback_ms.saturating_add(1_500),
        "relationship final inspect hold is shorter than 1,500 ms"
    );

    let state = read_jsonl(&directory.join("state.jsonl"))?;
    let expressions = state
        .iter()
        .filter_map(|record| record.get("creature")?.get("relationship_expression"))
        .collect::<Vec<_>>();
    ensure!(
        !expressions.is_empty(),
        "relationship state trace is missing active/recent expression metadata"
    );
    ensure!(
        expressions.iter().all(|expression| {
            expression
                .get("count_active_day")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|count| count <= 4)
        }),
        "relationship beat count exceeded the four-per-day bound"
    );

    let events = read_jsonl(&directory.join("events.jsonl"))?;
    let event_kinds = events
        .iter()
        .flat_map(|record| {
            record
                .get("events")
                .into_iter()
                .flat_map(|events| events.as_array())
        })
        .flat_map(|events| events.iter())
        .filter_map(|event| event.get("kind").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    ensure!(
        event_kinds.contains(&"relationship_beat_started"),
        "relationship scenario produced no authoritative beat-start event"
    );
    ensure!(
        event_kinds.contains(&"relationship_beat_phase_changed"),
        "relationship scenario produced no distinct phase callback"
    );
    if experience.id == "relationship-over-time" {
        ensure!(
            event_kinds.contains(&"relationship_beat_interrupted"),
            "relationship scenario produced no direct interruption/preemption evidence"
        );
        let day_markers = [
            "01-day-two-arrival",
            "02-day-three-arrival",
            "04-day-four-arrival",
        ]
        .into_iter()
        .map(|name| {
            markers
                .iter()
                .find(|marker| marker.name == name)
                .with_context(|| {
                    format!("relationship scenario is missing day callback marker {name}")
                })
        })
        .collect::<Result<Vec<_>>>()?;
        let callbacks = day_markers
            .iter()
            .map(|marker| relationship_signatures_near_marker(&events, marker))
            .collect::<Vec<_>>();
        ensure!(
            callbacks.iter().all(|signatures| !signatures.is_empty()),
            "relationship day marker has no typed callback in its bounded evidence window"
        );
        ensure!(
            callbacks.windows(2).all(|pair| pair[0] != pair[1]),
            "day-2/day-3/day-4 relationship callbacks do not carry distinct authoritative motifs"
        );
        let observable_callbacks = day_markers
            .iter()
            .map(|marker| relationship_observables_near_marker(&state, marker))
            .collect::<Vec<_>>();
        ensure!(
            observable_callbacks
                .iter()
                .all(|signatures| !signatures.is_empty()),
            "relationship day marker has no embodied callback in its bounded playback window"
        );
        ensure!(
            observable_callbacks
                .windows(2)
                .all(|pair| pair[0] != pair[1]),
            "day-2/day-3/day-4 callbacks are not observably distinct in intention, gaze, movement, destination, action, or cue"
        );

        let inputs = read_jsonl(&directory.join("inputs.jsonl"))?;
        let save_index = inputs
            .iter()
            .position(|record| command_name(record) == Some("save"))
            .context("relationship scenario did not save a session")?;
        let load_index = inputs
            .iter()
            .enumerate()
            .skip(save_index.saturating_add(1))
            .find_map(|(index, record)| (command_name(record) == Some("load")).then_some(index))
            .context("relationship scenario did not load after saving")?;
        let save_playback_ms = inputs[save_index]["playback_ms"]
            .as_u64()
            .unwrap_or_default();
        let load_playback_ms = inputs[load_index]["playback_ms"]
            .as_u64()
            .unwrap_or_default();
        let before_save = state
            .iter()
            .rev()
            .find(|record| record["playback_ms"].as_u64().unwrap_or_default() <= save_playback_ms)
            .and_then(relationship_expression)
            .context("relationship state trace has no ledger before save")?;
        let after_load = state
            .iter()
            .find(|record| record["playback_ms"].as_u64().unwrap_or_default() >= load_playback_ms)
            .and_then(relationship_expression)
            .context("relationship state trace has no ledger after load")?;
        let saved_recent = before_save
            .get("recent")
            .and_then(serde_json::Value::as_array)
            .context("relationship ledger before save has no recent expressions")?;
        let loaded_recent = after_load
            .get("recent")
            .and_then(serde_json::Value::as_array)
            .context("relationship ledger after load has no recent expressions")?;
        ensure!(
            !saved_recent.is_empty() && saved_recent == loaded_recent,
            "relationship expression ledger did not survive save/load unchanged"
        );
    } else {
        ensure!(!experience.fake_ai && !experience.tts_requested);
        let inputs = read_jsonl(&directory.join("inputs.jsonl"))?;
        ensure!(
            inputs.iter().all(|record| {
                record
                    .get("input")
                    .and_then(|input| input.get("command"))
                    .and_then(serde_json::Value::as_str)
                    != Some("talk")
            }),
            "AI-off relationship scenario contains a talk command"
        );
        ensure!(
            !read_jsonl(&directory.join("audio.jsonl"))?
                .iter()
                .any(
                    |record| record.get("kind").and_then(serde_json::Value::as_str)
                        == Some("speech")
                ),
            "AI-off relationship scenario unexpectedly recorded speech"
        );
        let return_marker = markers
            .iter()
            .find(|marker| marker.name == "02-familiar-return-without-voice")
            .context("AI-off scenario is missing its familiar return marker")?;
        ensure!(
            !relationship_signatures_near_marker(&events, return_marker).is_empty(),
            "AI-off return produced no authoritative relationship callback"
        );
        ensure!(
            !relationship_observables_near_marker(&state, return_marker).is_empty(),
            "AI-off return produced no observable nonverbal callback"
        );
        let return_events = relationship_events_in_playback_window(&events, return_marker, 8_000);
        ensure!(
            return_events.iter().any(|event| {
                event.get("kind").and_then(serde_json::Value::as_str)
                    == Some("relationship_beat_started")
                    && event
                        .get("value")
                        .and_then(|value| value.get("trigger"))
                        .and_then(|trigger| trigger.get("kind"))
                        .and_then(serde_json::Value::as_str)
                        == Some("player_return")
                    && event
                        .get("value")
                        .and_then(|value| value.get("expression"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|expression| expression != "notice")
            }),
            "AI-off familiar return did not start a mature grounded callback"
        );
        ensure!(
            return_events.iter().any(|event| {
                event.get("kind").and_then(serde_json::Value::as_str)
                    == Some("relationship_beat_completed")
            }) && !return_events.iter().any(|event| {
                event.get("kind").and_then(serde_json::Value::as_str)
                    == Some("relationship_beat_interrupted")
            }),
            "AI-off familiar return did not complete before another action interrupted it"
        );
    }
    Ok(())
}

fn command_name(record: &serde_json::Value) -> Option<&str> {
    record.get("input")?.get("command")?.as_str()
}

fn relationship_expression(record: &serde_json::Value) -> Option<&serde_json::Value> {
    record.get("creature")?.get("relationship_expression")
}

fn relationship_signatures_near_marker(
    records: &[serde_json::Value],
    marker: &Marker,
) -> BTreeSet<String> {
    const CALLBACK_WINDOW_MS: u64 = 2_000;
    records
        .iter()
        .filter(|record| {
            let simulation_ms = record["simulation_ms"].as_u64().unwrap_or_default();
            simulation_ms.abs_diff(marker.simulation_ms) <= CALLBACK_WINDOW_MS
        })
        .flat_map(|record| record["events"].as_array().into_iter().flatten())
        .filter_map(relationship_event_signature)
        .collect()
}

fn relationship_observables_near_marker(
    records: &[serde_json::Value],
    marker: &Marker,
) -> BTreeSet<String> {
    const CALLBACK_WINDOW_MS: u64 = 6_000;
    records
        .iter()
        .filter(|record| {
            let playback_ms = record["playback_ms"].as_u64().unwrap_or_default();
            playback_ms >= marker.playback_ms
                && playback_ms <= marker.playback_ms.saturating_add(CALLBACK_WINDOW_MS)
                && relationship_expression(record)
                    .and_then(|expression| expression.get("active"))
                    .is_some_and(|active| !active.is_null())
        })
        .filter_map(|record| {
            let creature = record.get("creature")?;
            let view = record.get("view")?;
            Some(format!(
                "intention={};gaze={};steering={};destination={};action={};cue={}",
                creature.get("intention")?,
                creature.get("gaze")?,
                creature.get("steering")?,
                creature.get("destination")?,
                creature.get("action")?,
                view.get("cue")?
            ))
        })
        .collect()
}

fn relationship_events_in_playback_window<'a>(
    records: &'a [serde_json::Value],
    marker: &Marker,
    window_ms: u64,
) -> Vec<&'a serde_json::Value> {
    records
        .iter()
        .filter(|record| {
            let playback_ms = record["playback_ms"].as_u64().unwrap_or_default();
            playback_ms.saturating_add(100) >= marker.playback_ms
                && playback_ms <= marker.playback_ms.saturating_add(window_ms)
        })
        .flat_map(|record| record["events"].as_array().into_iter().flatten())
        .filter(|event| {
            event
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| kind.starts_with("relationship_"))
        })
        .collect()
}

fn relationship_event_signature(event: &serde_json::Value) -> Option<String> {
    let kind = event.get("kind")?.as_str()?;
    let value = event.get("value")?;
    match kind {
        "relationship_beat_started" => Some(format!(
            "started:{}:{}:{}",
            value.get("motif")?,
            value.get("expression")?,
            value.get("trigger")?
        )),
        "relationship_beat_phase_changed" => Some(format!(
            "phase:{}:{}",
            value.get("motif")?,
            value.get("to")?
        )),
        "relationship_beat_completed" => Some(format!("completed:{value}")),
        "relationship_beat_interrupted" => Some(format!("interrupted:{value}")),
        _ => None,
    }
}

fn read_jsonl(path: &Path) -> Result<Vec<serde_json::Value>> {
    let file =
        File::open(path).with_context(|| format!("feel trace is missing: {}", path.display()))?;
    BufReader::new(file)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.with_context(|| format!("failed to read trace line {}", index + 1))?;
            serde_json::from_str(&line)
                .with_context(|| format!("invalid JSONL trace line {}", index + 1))
        })
        .collect()
}

fn generate_filmstrips(directory: &Path, markers: &[Marker]) -> Result<()> {
    let filmstrips = directory.join("filmstrips");
    fs::create_dir_all(&filmstrips)?;
    let video = directory.join("session.mp4");
    require_success(
        "ffmpeg overview filmstrip",
        Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-i"])
            .arg(&video)
            .args([
                "-vf",
                "fps=1/2,scale=320:180:flags=neighbor,tile=4x3:padding=2:margin=2",
                "-fps_mode",
                "vfr",
            ])
            .arg(filmstrips.join("overview-%03d.png"))
            .status()
            .context("failed to generate overview filmstrips")?,
    )?;
    for marker in markers {
        let start_ms = marker.playback_ms.saturating_sub(1_000);
        let start = format!("{}.{:03}", start_ms / 1_000, start_ms % 1_000);
        let pattern = filmstrips.join(format!("{}-%03d.png", marker.name));
        require_success(
            "ffmpeg marker filmstrip",
            Command::new("ffmpeg")
                .args(["-y", "-v", "error", "-ss"])
                .arg(start)
                .arg("-i")
                .arg(&video)
                .args([
                    "-t",
                    "2",
                    "-vf",
                    "fps=10,scale=320:180:flags=neighbor,tile=5x4:padding=2:margin=2",
                    "-fps_mode",
                    "vfr",
                ])
                .arg(pattern)
                .status()
                .with_context(|| format!("failed to generate filmstrip for {}", marker.name))?,
        )?;
    }
    Ok(())
}

fn write_review(directory: &Path, experience: Experience, markers: &[Marker]) -> Result<()> {
    let mut report = String::new();
    report.push_str(&format!("# Feel review: {}\n\n", experience.id));
    report.push_str("Status: evidence captured; findings not yet adjudicated.\n\n");
    report.push_str("## Evidence\n\n");
    report.push_str("- Continuous presentation: `session.mp4`\n");
    report.push_str(
        "- Synchronized authored reference audio: `session-audio-reference.mp4` (not a host-output capture)\n",
    );
    report.push_str("- Session-wide reference waveform: `audio-overview.png`\n");
    report.push_str("- Uniform filmstrips: `filmstrips/overview-*.png`\n");
    report.push_str(
        "- Synchronized traces: `inputs.jsonl`, `events.jsonl`, `state.jsonl`, `audio.jsonl`\n\n",
    );
    report.push_str("## Markers\n\n| Playback | Simulation | Marker |\n|---:|---:|---|\n");
    for marker in markers {
        report.push_str(&format!(
            "| {} | {} | `{}` |\n",
            timestamp(marker.playback_ms),
            timestamp(marker.simulation_ms),
            marker.name
        ));
    }
    report.push_str("\n## Findings\n\nNo findings adjudicated yet.\n");
    fs::write(directory.join("review.md"), report)
        .with_context(|| format!("failed to write review for {}", experience.id))
}

fn write_manifest(
    directory: &Path,
    experience: Experience,
    suite: FeelSuite,
    game: &Path,
) -> Result<()> {
    let mut names = vec![
        "session.mp4".to_owned(),
        "session-audio-reference.mp4".to_owned(),
        "reference-mix.wav".to_owned(),
        "inputs.jsonl".to_owned(),
        "events.jsonl".to_owned(),
        "state.jsonl".to_owned(),
        "audio.jsonl".to_owned(),
        "audio-overview.png".to_owned(),
        "markers.jsonl".to_owned(),
        "review.md".to_owned(),
    ];
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("speech-") && name.ends_with(".wav") {
            names.push(name);
        }
    }
    names.sort();
    let mut artifacts = Vec::new();
    for name in names {
        let path = directory.join(&name);
        ensure!(
            path.is_file(),
            "feel artifact is missing: {}",
            path.display()
        );
        artifacts.push(Artifact {
            path: name,
            bytes: fs::metadata(&path)?.len(),
            sha256: sha256_file(&path)?,
        });
    }
    let commit = git_commit()?;
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        experience: experience.id,
        suite,
        scenario: experience.scenario,
        scenario_sha256: sha256_file(Path::new(experience.scenario))?,
        commit,
        working_tree_dirty: working_tree_dirty()?,
        game_binary_sha256: sha256_file(game)?,
        platform: std::env::consts::OS,
        build_profile: "debug",
        fake_ai: experience.fake_ai,
        tts_requested: experience.tts_requested,
        seed: 42,
        video_fps: 60,
        presentation: "640x360 exact 2x logical framebuffer",
        audible_mix_captured: false,
        reference_mix_generated: true,
        artifacts,
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(directory.join("manifest.json"), format!("{json}\n"))?;
    Ok(())
}

fn write_suite_index(output: &Path, experiences: &[Experience]) -> Result<()> {
    let mut index = String::from("# Beastie feel evidence\n\n");
    index.push_str(
        "Review each experience at session, interaction-beat, and individual-frame resolution.\n\n",
    );
    for experience in experiences {
        index.push_str(&format!(
            "- [{}]({}/review.md)\n",
            experience.id, experience.id
        ));
    }
    fs::write(output.join("README.md"), index)?;
    Ok(())
}

fn git_commit() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .context("failed to identify feel commit")?;
    require_success("git rev-parse", output.status)?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn working_tree_dirty() -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("failed to inspect feel worktree")?;
    require_success("git status", output.status)?;
    Ok(!output.stdout.is_empty())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn timestamp(milliseconds: u64) -> String {
    format!(
        "{:02}:{:02}.{:03}",
        milliseconds / 60_000,
        (milliseconds / 1_000) % 60,
        milliseconds % 1_000
    )
}

fn require_success(name: &str, status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("{name} failed with {status}")
    }
}

#[cfg(target_os = "macos")]
fn focus_macos_process(pid: u32) {
    let script = format!(
        "tell application \"System Events\"\nrepeat 20 times\nset matches to every process whose unix id is {pid}\nif (count of matches) > 0 then\nset frontmost of item 1 of matches to true\nreturn\nend if\ndelay 0.1\nend repeat\nend tell"
    );
    let _ = Command::new("osascript").args(["-e", &script]).status();
}

#[cfg(not(target_os = "macos"))]
fn focus_macos_process(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_contains_every_required_experience_in_order() {
        let ids = experiences(FeelSuite::Baseline)
            .into_iter()
            .map(|experience| experience.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "first-five-minutes",
                "quiet-observation",
                "interaction-chain",
                "bad-conditions",
                "relationship-over-time",
                "relationship-over-time-no-ai"
            ]
        );
    }

    #[test]
    fn relationship_suite_keeps_ai_on_and_off_runs_separate() {
        let experiences = experiences(FeelSuite::RelationshipOverTime);
        assert_eq!(experiences.len(), 2);
        assert!(experiences[0].fake_ai && experiences[0].tts_requested);
        assert!(!experiences[1].fake_ai && !experiences[1].tts_requested);
    }

    #[test]
    fn timestamps_are_stable_and_millisecond_precise() {
        assert_eq!(timestamp(0), "00:00.000");
        assert_eq!(timestamp(61_234), "01:01.234");
    }
}
