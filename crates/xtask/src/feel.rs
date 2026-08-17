use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use beastie_core::{
    FoodId, RelationshipExpressionMode, RelationshipMotifKey, RelationshipSubject,
    SemanticDestination, ToyId,
};
use beastie_session::SessionSave;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FEEL_SCHEMA_VERSION: u32 = 1;
const MANIFEST_VERSION: u32 = 3;
const RUN_TIMEOUT_GRACE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeelSuite {
    Baseline,
    FirstFiveMinutes,
    QuietObservation,
    InteractionChain,
    BadConditions,
    RelationshipOverTime,
    RelationshipBreadth,
}

#[derive(Debug)]
pub struct FeelOptions<'a> {
    pub suite: FeelSuite,
    pub experience: Option<&'a str>,
    pub output: Option<&'a Path>,
    pub game: Option<&'a Path>,
}

#[derive(Debug, Clone, Copy)]
struct Experience {
    id: &'static str,
    scenario: &'static str,
    initial_save: Option<&'static str>,
    required_motifs: &'static [RequiredMotifMarker],
    fake_ai: bool,
    tts_requested: bool,
}

#[derive(Debug, Clone, Copy)]
struct RequiredMotifMarker {
    name: &'static str,
    motif: RelationshipMotifKey,
    subject: RelationshipSubject,
    mode: RelationshipExpressionMode,
    minimum_evidence: usize,
}

const FIRST_FIVE_MINUTES: Experience = Experience {
    id: "first-five-minutes",
    scenario: "fixtures/scenarios/feel/first-five-minutes.jsonl",
    initial_save: None,
    required_motifs: &[],
    fake_ai: true,
    tts_requested: true,
};
const QUIET_OBSERVATION: Experience = Experience {
    id: "quiet-observation",
    scenario: "fixtures/scenarios/feel/quiet-observation.jsonl",
    initial_save: None,
    required_motifs: &[],
    fake_ai: true,
    tts_requested: false,
};
const INTERACTION_CHAIN: Experience = Experience {
    id: "interaction-chain",
    scenario: "fixtures/scenarios/feel/interaction-chain.jsonl",
    initial_save: None,
    required_motifs: &[],
    fake_ai: true,
    tts_requested: true,
};
const BAD_CONDITIONS: Experience = Experience {
    id: "bad-conditions",
    scenario: "fixtures/scenarios/feel/bad-conditions.jsonl",
    initial_save: None,
    required_motifs: &[],
    // Deliberately omit the worker so dialogue exercises the authored technical fallback.
    fake_ai: false,
    tts_requested: true,
};
const RELATIONSHIP_OVER_TIME: Experience = Experience {
    id: "relationship-over-time",
    scenario: "fixtures/scenarios/feel/relationship-over-time.jsonl",
    initial_save: None,
    required_motifs: &[],
    fake_ai: true,
    tts_requested: true,
};
const RELATIONSHIP_OVER_TIME_NO_AI: Experience = Experience {
    id: "relationship-over-time-no-ai",
    scenario: "fixtures/scenarios/feel/relationship-over-time-no-ai.jsonl",
    initial_save: None,
    required_motifs: &[],
    fake_ai: false,
    tts_requested: false,
};
const TRUSTED_BERRY_MARKERS: &[RequiredMotifMarker] = &[RequiredMotifMarker {
    name: "trusted-berry",
    motif: RelationshipMotifKey::TrustedFood(FoodId::Berry),
    subject: RelationshipSubject::Food(FoodId::Berry),
    mode: RelationshipExpressionMode::ActionBound,
    minimum_evidence: 2,
}];
const MUSHROOM_GRUDGE_MARKERS: &[RequiredMotifMarker] = &[RequiredMotifMarker {
    name: "mushroom-grudge",
    motif: RelationshipMotifKey::FoodGrudge(FoodId::Mushroom),
    subject: RelationshipSubject::Food(FoodId::Mushroom),
    mode: RelationshipExpressionMode::ActionBound,
    minimum_evidence: 3,
}];
const FAMILIAR_CAVE_MARKERS: &[RequiredMotifMarker] = &[RequiredMotifMarker {
    name: "familiar-cave",
    motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
    subject: RelationshipSubject::Place(SemanticDestination::Cave),
    mode: RelationshipExpressionMode::Standalone,
    minimum_evidence: 1,
}];
const FAMILIAR_PLANT_MARKERS: &[RequiredMotifMarker] = &[RequiredMotifMarker {
    name: "familiar-plant",
    motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Plant),
    subject: RelationshipSubject::Place(SemanticDestination::Plant),
    mode: RelationshipExpressionMode::Standalone,
    minimum_evidence: 1,
}];
const FAMILIAR_BALL_MARKERS: &[RequiredMotifMarker] = &[RequiredMotifMarker {
    name: "familiar-ball",
    motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Toy(ToyId::Ball)),
    subject: RelationshipSubject::Place(SemanticDestination::Toy(ToyId::Ball)),
    mode: RelationshipExpressionMode::Standalone,
    minimum_evidence: 1,
}];

const TRUSTED_BERRY: Experience = Experience {
    id: "trusted-berry",
    scenario: "fixtures/scenarios/feel/relationship-breadth/trusted-berry.jsonl",
    initial_save: Some("fixtures/saves/feel/trusted-berry.json"),
    required_motifs: TRUSTED_BERRY_MARKERS,
    fake_ai: false,
    tts_requested: false,
};
const MUSHROOM_GRUDGE: Experience = Experience {
    id: "mushroom-grudge",
    scenario: "fixtures/scenarios/feel/relationship-breadth/mushroom-grudge.jsonl",
    initial_save: Some("fixtures/saves/feel/mushroom-grudge.json"),
    required_motifs: MUSHROOM_GRUDGE_MARKERS,
    fake_ai: false,
    tts_requested: false,
};
const FAMILIAR_CAVE: Experience = Experience {
    id: "familiar-cave",
    scenario: "fixtures/scenarios/feel/relationship-breadth/familiar-cave.jsonl",
    initial_save: Some("fixtures/saves/feel/familiar-cave.json"),
    required_motifs: FAMILIAR_CAVE_MARKERS,
    fake_ai: false,
    tts_requested: false,
};
const FAMILIAR_PLANT: Experience = Experience {
    id: "familiar-plant",
    scenario: "fixtures/scenarios/feel/relationship-breadth/familiar-plant.jsonl",
    initial_save: Some("fixtures/saves/feel/familiar-plant.json"),
    required_motifs: FAMILIAR_PLANT_MARKERS,
    fake_ai: false,
    tts_requested: false,
};
const FAMILIAR_BALL: Experience = Experience {
    id: "familiar-ball",
    scenario: "fixtures/scenarios/feel/relationship-breadth/familiar-ball.jsonl",
    initial_save: Some("fixtures/saves/feel/familiar-ball.json"),
    required_motifs: FAMILIAR_BALL_MARKERS,
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
    initial_save: Option<&'a str>,
    initial_save_sha256: Option<String>,
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

    let experiences = match options.experience {
        Some(id) => {
            let selected = experiences(options.suite)
                .into_iter()
                .filter(|experience| experience.id == id)
                .collect::<Vec<_>>();
            ensure!(
                selected.len() == 1,
                "feel suite {:?} has no experience named {id}",
                options.suite
            );
            selected
        }
        None => experiences(options.suite),
    };
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
        FeelSuite::RelationshipBreadth => vec![
            TRUSTED_BERRY,
            MUSHROOM_GRUDGE,
            FAMILIAR_CAVE,
            FAMILIAR_PLANT,
            FAMILIAR_BALL,
        ],
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
    let initial_save = experience
        .initial_save
        .map(Path::new)
        .map(|path| {
            let source = fs::read_to_string(path)
                .with_context(|| format!("feel initial save is missing: {}", path.display()))?;
            let save = SessionSave::from_json(&source)
                .with_context(|| format!("feel initial save is invalid: {}", path.display()))?;
            Ok::<_, anyhow::Error>((path, save))
        })
        .transpose()?;
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
    if let Some((path, _)) = &initial_save {
        command.arg("--feel-initial-save").arg(path);
    }
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

    let runtime_stderr_path = directory.join("runtime-stderr.log");
    let runtime_stderr = File::create(&runtime_stderr_path)
        .with_context(|| format!("failed to create {}", runtime_stderr_path.display()))?;
    command.stderr(Stdio::from(runtime_stderr));

    let expected_duration = Duration::from_millis(authored_scenario_duration_ms(scenario)?);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to launch feel experience {}", experience.id))?;
    wait_for_child(
        &mut child,
        experience.id,
        expected_duration.saturating_add(RUN_TIMEOUT_GRACE),
    )?;
    let runtime_stderr = fs::read_to_string(&runtime_stderr_path)
        .with_context(|| format!("failed to read {}", runtime_stderr_path.display()))?;
    ensure!(
        !runtime_stderr.contains("Error on EventHandler"),
        "feel experience {} stopped on a game update error; see {}",
        experience.id,
        runtime_stderr_path.display()
    );
    verify_video(&directory.join("session.mp4"))?;
    verify_scenario_duration(scenario, &directory.join("session.mp4"))?;
    generate_reference_mix(&directory)?;
    let markers = read_markers(&directory.join("markers.jsonl"))?;
    validate_required_motifs(&directory, experience, &markers)?;
    validate_relationship_breadth_causality(&directory, experience)?;
    validate_relationship_evidence(&directory, experience, &markers)?;
    generate_filmstrips(&directory, &markers)?;
    write_review(&directory, experience, &markers)?;
    write_manifest(&directory, experience, suite, game)?;
    Ok(())
}

fn verify_scenario_duration(scenario: &Path, video: &Path) -> Result<()> {
    let expected_ms = authored_scenario_duration_ms(scenario)?;
    let actual_ms = video_duration_seconds(video)?
        .parse::<f64>()
        .context("feel video duration is not numeric")?
        .mul_add(1_000.0, 0.0) as u64;
    ensure!(
        actual_ms.saturating_add(100) >= expected_ms,
        "feel video ended early: authored scenario requires {expected_ms} ms, recorded {actual_ms} ms"
    );
    Ok(())
}

fn authored_scenario_duration_ms(scenario: &Path) -> Result<u64> {
    BufReader::new(File::open(scenario)?)
        .lines()
        .try_fold(0_u64, |total, line| -> Result<u64> {
            let value: serde_json::Value = serde_json::from_str(&line?)?;
            let command = value
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let duration = match command {
                "wait" | "inspect_hold" => value
                    .get("milliseconds")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                _ => 0,
            };
            Ok(total.saturating_add(duration))
        })
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

fn wait_for_child(child: &mut std::process::Child, name: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().context("failed to poll beastie-game")? {
            return require_success(name, status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "feel experience {name} did not finish within {} seconds",
                timeout.as_secs()
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

fn validate_required_motifs(
    directory: &Path,
    experience: Experience,
    markers: &[Marker],
) -> Result<()> {
    if experience.required_motifs.is_empty() {
        return Ok(());
    }
    let events = read_jsonl(&directory.join("events.jsonl"))?;
    let state = read_jsonl(&directory.join("state.jsonl"))?;
    for required in experience.required_motifs {
        let matching_markers = markers
            .iter()
            .filter(|marker| marker.name == required.name)
            .collect::<Vec<_>>();
        ensure!(
            matching_markers.len() == 1,
            "required motif marker {} must occur exactly once",
            required.name
        );
        let marker = matching_markers[0];
        let motif = serde_json::to_value(required.motif)?;
        let subject = serde_json::to_value(required.subject)?;
        let event = events
            .iter()
            .filter(|record| {
                record["playback_ms"]
                    .as_u64()
                    .unwrap_or_default()
                    .abs_diff(marker.playback_ms)
                    <= 2_000
            })
            .flat_map(|record| record["events"].as_array().into_iter().flatten())
            .find(|event| required_event(event, required.mode, &motif, &subject))
            .with_context(|| {
                format!(
                    "required marker {} has no exact {:?} authoritative event",
                    required.name, required.mode
                )
            })?;
        let event_evidence = event
            .get("value")
            .and_then(|value| value.get("evidence"))
            .and_then(serde_json::Value::as_array)
            .context("required relationship event has no evidence")?;
        ensure!(
            event_evidence.len() >= required.minimum_evidence,
            "required marker {} has too little authoritative evidence",
            required.name
        );
        ensure!(
            state.iter().any(|record| {
                let playback_ms = record["playback_ms"].as_u64().unwrap_or_default();
                playback_ms >= marker.playback_ms
                    && playback_ms <= marker.playback_ms.saturating_add(6_000)
                    && required_state_context(record, required.mode).is_some_and(|context| {
                        context.get("motif") == Some(&motif)
                            && context.get("subject") == Some(&subject)
                            && context
                                .get("evidence")
                                .and_then(serde_json::Value::as_array)
                                == Some(event_evidence)
                    })
            }),
            "required marker {} has no contemporaneous state with matching motif, subject, mode, and evidence",
            required.name
        );
    }
    Ok(())
}

fn required_event(
    event: &serde_json::Value,
    mode: RelationshipExpressionMode,
    motif: &serde_json::Value,
    subject: &serde_json::Value,
) -> bool {
    let expected_kind = match mode {
        RelationshipExpressionMode::ActionBound => "action_relationship_started",
        RelationshipExpressionMode::Standalone => "relationship_beat_started",
    };
    event.get("kind").and_then(serde_json::Value::as_str) == Some(expected_kind)
        && event.get("value").and_then(|value| value.get("motif")) == Some(motif)
        && event.get("value").and_then(|value| value.get("subject")) == Some(subject)
}

fn required_state_context(
    record: &serde_json::Value,
    mode: RelationshipExpressionMode,
) -> Option<&serde_json::Value> {
    let context = match mode {
        RelationshipExpressionMode::ActionBound => {
            record.get("creature")?.get("action")?.get("relationship")?
        }
        RelationshipExpressionMode::Standalone => record
            .get("creature")?
            .get("relationship_expression")?
            .get("active")?,
    };
    context.is_object().then_some(context)
}

fn validate_relationship_breadth_causality(directory: &Path, experience: Experience) -> Result<()> {
    let (expected_outcome, expected_direct_event) = match experience.id {
        "trusted-berry" => (Some("consumed"), Some("food_consumed")),
        "mushroom-grudge" => (Some("rejected"), Some("food_rejected")),
        "familiar-cave" | "familiar-plant" | "familiar-ball" => (None, None),
        _ => return Ok(()),
    };
    let events = read_jsonl(&directory.join("events.jsonl"))?;
    let state = read_jsonl(&directory.join("state.jsonl"))?;

    if let (Some(outcome), Some(direct_event)) = (expected_outcome, expected_direct_event) {
        let started_at = event_playback_ms(&events, "action_relationship_started")
            .context("relationship food experience has no start event")?;
        let direct_at = event_playback_ms(&events, direct_event)
            .with_context(|| format!("relationship food experience has no {direct_event}"))?;
        let resolved_at = events
            .iter()
            .find_map(|record| {
                let playback_ms = record.get("playback_ms")?.as_u64()?;
                record
                    .get("events")?
                    .as_array()?
                    .iter()
                    .any(|event| {
                        event.get("kind").and_then(serde_json::Value::as_str)
                            == Some("action_relationship_resolved")
                            && event
                                .get("value")
                                .and_then(|value| value.get("outcome"))
                                .and_then(serde_json::Value::as_str)
                                == Some(outcome)
                    })
                    .then_some(playback_ms)
            })
            .with_context(|| format!("relationship food experience has no {outcome} resolution"))?;
        ensure!(
            started_at < direct_at && started_at < resolved_at,
            "relationship recognition did not precede its authoritative food outcome"
        );
        ensure!(
            state.iter().any(|record| {
                let playback_ms = record["playback_ms"].as_u64().unwrap_or_default();
                playback_ms >= resolved_at
                    && playback_ms <= resolved_at.saturating_add(1_000)
                    && record
                        .get("creature")
                        .and_then(|creature| creature.get("action"))
                        .and_then(|action| action.get("food_outcome"))
                        .and_then(serde_json::Value::as_str)
                        == Some(outcome)
            }),
            "relationship food outcome was not retained through action recovery"
        );
        return Ok(());
    }

    let act_at = events
        .iter()
        .find_map(|record| {
            let playback_ms = record.get("playback_ms")?.as_u64()?;
            record
                .get("events")?
                .as_array()?
                .iter()
                .any(|event| {
                    event.get("kind").and_then(serde_json::Value::as_str)
                        == Some("relationship_beat_phase_changed")
                        && event
                            .get("value")
                            .and_then(|value| value.get("to"))
                            .and_then(serde_json::Value::as_str)
                            == Some("act")
                })
                .then_some(playback_ms)
        })
        .context("familiar-place experience never reached Act")?;
    let required = experience
        .required_motifs
        .first()
        .context("familiar-place experience has no required subject")?;
    let destination = serde_json::to_value(match required.subject {
        RelationshipSubject::Place(destination) => destination,
        _ => bail!("familiar-place experience has a non-place subject"),
    })?;
    ensure!(
        state.iter().any(|record| {
            record["playback_ms"].as_u64() == Some(act_at)
                && record["creature"]["steering"].as_str() == Some("hover")
                && record["creature"]["gaze"] == destination
                && record["creature"]["last_arrived_destination"] == destination
                && record["simulation_ms"]
                    .as_u64()
                    .is_some_and(|simulation_ms| {
                        record["creature"]["settled_until_ms"]
                            .as_u64()
                            .is_some_and(|settled_until_ms| simulation_ms < settled_until_ms)
                    })
                && record["creature"]["relationship_expression"]["active"]["phase"].as_str()
                    == Some("act")
        }),
        "familiar-place Act was not contemporaneously anchored by exact gaze and hover"
    );
    Ok(())
}

fn event_playback_ms(records: &[serde_json::Value], kind: &str) -> Option<u64> {
    records.iter().find_map(|record| {
        record
            .get("events")?
            .as_array()?
            .iter()
            .any(|event| event.get("kind").and_then(serde_json::Value::as_str) == Some(kind))
            .then(|| record.get("playback_ms")?.as_u64())
            .flatten()
    })
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
    let initial_save = experience
        .initial_save
        .map(Path::new)
        .map(|path| {
            let source = fs::read_to_string(path)?;
            let save = SessionSave::from_json(&source)?;
            Ok::<_, anyhow::Error>((path, save))
        })
        .transpose()?;
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
        initial_save: experience.initial_save,
        initial_save_sha256: initial_save
            .as_ref()
            .map(|(path, _)| sha256_file(path))
            .transpose()?,
        commit,
        working_tree_dirty: working_tree_dirty()?,
        game_binary_sha256: sha256_file(game)?,
        platform: std::env::consts::OS,
        build_profile: "debug",
        fake_ai: experience.fake_ai,
        tts_requested: experience.tts_requested,
        seed: initial_save
            .as_ref()
            .map_or(42, |(_, save)| save.world.seed),
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

#[cfg(test)]
mod tests {
    use beastie_core::{GameEvent, NormalizedPosition};
    use beastie_session::{CommandEnvelope, SESSION_PROTOCOL_VERSION, SessionCommand};

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
    fn relationship_breadth_is_a_separate_nonverbal_taste_pass() {
        let experiences = experiences(FeelSuite::RelationshipBreadth);
        assert_eq!(experiences.len(), 5);
        assert_eq!(
            experiences
                .iter()
                .map(|experience| experience.id)
                .collect::<Vec<_>>(),
            [
                "trusted-berry",
                "mushroom-grudge",
                "familiar-cave",
                "familiar-plant",
                "familiar-ball"
            ]
        );
        assert!(experiences.iter().all(|experience| {
            experience.initial_save.is_some()
                && !experience.required_motifs.is_empty()
                && !experience.fake_ai
                && !experience.tts_requested
        }));
    }

    #[test]
    fn relationship_scenario_duration_includes_wait_and_final_hold() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for experience in experiences(FeelSuite::RelationshipBreadth) {
            let duration = authored_scenario_duration_ms(&root.join(experience.scenario))
                .expect("scenario duration");
            let expected = if experience.id.starts_with("familiar-") {
                12_000
            } else {
                16_000
            };
            assert_eq!(duration, expected, "{}", experience.id);
        }
    }

    #[test]
    fn relationship_breadth_starting_saves_are_valid_and_derive_the_required_motif() {
        for experience in experiences(FeelSuite::RelationshipBreadth) {
            let path = experience.initial_save.expect("starting save");
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path);
            let source = fs::read_to_string(fixture).expect("fixture exists");
            let save = SessionSave::from_json(&source).expect("fixture validates");
            assert_eq!(save.saved_at_ms, save.world.elapsed_ms);
            assert!(save.world.creature.aquarium.action.is_none());
            assert!(save.world.creature.relationship_expression.active.is_none());
            let motifs = beastie_core::derive_relationship_motifs(&save.world);
            for required in experience.required_motifs {
                assert!(motifs.iter().any(|motif| {
                    motif.key == required.motif && motif.evidence.len() >= required.minimum_evidence
                }));
            }
        }
    }

    #[test]
    fn relationship_breadth_scenarios_start_the_exact_required_context() {
        for experience in experiences(FeelSuite::RelationshipBreadth) {
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(experience.initial_save.expect("starting save"));
            let source = fs::read_to_string(fixture).expect("fixture exists");
            let save = SessionSave::from_json(&source).expect("fixture validates");
            let resumed_at_ms = save.saved_at_ms;
            let (mut session, progress) =
                beastie_session::GameSession::resume(save, resumed_at_ms).expect("fixture resumes");
            assert_eq!(progress.applied_ms, 0);
            let required = experience.required_motifs[0];
            let command = match required.subject {
                RelationshipSubject::Food(food) => SessionCommand::DropFood {
                    food,
                    position: NormalizedPosition::new(5_000, 3_000),
                },
                RelationshipSubject::Place(_) => SessionCommand::Tick {
                    milliseconds: 1_000,
                },
                RelationshipSubject::Toy(_) | RelationshipSubject::Player => {
                    panic!("breadth fixture uses food or place subjects")
                }
            };
            let observation = session
                .apply(CommandEnvelope {
                    version: SESSION_PROTOCOL_VERSION,
                    command,
                })
                .expect("fixture scenario begins");
            match required.mode {
                RelationshipExpressionMode::ActionBound => {
                    assert!(observation.events.iter().any(|event| matches!(
                        event,
                        GameEvent::ActionRelationshipStarted { motif, subject, evidence, .. }
                            if *motif == required.motif
                                && *subject == required.subject
                                && evidence.len() >= required.minimum_evidence
                    )));
                    let context = session
                        .world()
                        .creature
                        .aquarium
                        .action
                        .as_ref()
                        .and_then(|action| action.relationship.as_ref())
                        .expect("action relationship context");
                    assert_eq!(context.motif, required.motif);
                    assert_eq!(context.subject, required.subject);
                }
                RelationshipExpressionMode::Standalone => {
                    assert!(observation.events.iter().any(|event| matches!(
                        event,
                        GameEvent::RelationshipBeatStarted { motif, subject, evidence, .. }
                            if *motif == required.motif
                                && *subject == required.subject
                                && evidence.len() >= required.minimum_evidence
                    )));
                    let beat = session
                        .world()
                        .creature
                        .relationship_expression
                        .active
                        .as_ref()
                        .expect("standalone relationship beat");
                    assert_eq!(beat.motif, required.motif);
                    assert_eq!(beat.subject, Some(required.subject));
                }
            }
        }
    }

    #[test]
    fn required_motif_events_reject_wrong_subject_and_mode() {
        let required = TRUSTED_BERRY_MARKERS[0];
        let motif = serde_json::to_value(required.motif).unwrap();
        let subject = serde_json::to_value(required.subject).unwrap();
        let event = serde_json::json!({
            "kind": "action_relationship_started",
            "value": {
                "motif": motif,
                "subject": subject,
                "evidence": [{"evidence_kind": "memory", "id": 1}]
            }
        });
        assert!(required_event(
            &event,
            RelationshipExpressionMode::ActionBound,
            &motif,
            &subject
        ));
        assert!(!required_event(
            &event,
            RelationshipExpressionMode::Standalone,
            &motif,
            &subject
        ));
        assert!(!required_event(
            &event,
            RelationshipExpressionMode::ActionBound,
            &motif,
            &serde_json::to_value(RelationshipSubject::Food(FoodId::Mushroom)).unwrap()
        ));
    }

    #[test]
    fn timestamps_are_stable_and_millisecond_precise() {
        assert_eq!(timestamp(0), "00:00.000");
        assert_eq!(timestamp(61_234), "01:01.234");
    }
}
