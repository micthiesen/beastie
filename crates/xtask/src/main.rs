use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use beastie_core::{ACTIVE_DAY_MS, GameEvent, MemoryId, SeededRandom, WorldState, step};
use beastie_protocol::{
    DialogueReply, DialogueRequest, Gesture, constrained_fallback_reply, validate_reply,
};
use beastie_session::{
    GameSession, MAX_COMMAND_BYTES, Observation, SESSION_PROTOCOL_VERSION, SessionError,
    SpokenInputStatus,
};
use clap::{Parser, Subcommand};
use serde::Serialize;

mod asset;
mod dialogue_eval;
mod feel;
mod feel_audio;
mod packaging;
mod store_assets;
mod stt_eval;
mod target_hygiene;

const BERRY_GRUDGE_SCENARIO: &str = "fixtures/scenarios/berry-grudge.jsonl";
const SPOKEN_INPUT_SCENARIO: &str = "fixtures/scenarios/spoken-input-foundation.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Beastie developer tasks")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Task {
    /// Run every headless verification gate.
    Verify,
    /// Validate offline sound and font assets.
    Asset {
        #[command(subcommand)]
        command: AssetTask,
    },
    /// Build or validate the checked-in Steam store art from its approved source image.
    StoreAssets {
        #[command(subcommand)]
        command: StoreAssetsTask,
    },
    /// Score the checked-in dialogue corpus with fixtures or a local worker.
    Dialogue {
        #[command(subcommand)]
        command: DialogueTask,
    },
    /// Score the checked-in speech corpus or benchmark one persistent local recognizer.
    Stt {
        #[command(subcommand)]
        command: SttTask,
    },
    /// Capture synchronized evidence for subjective play-feel review.
    Feel {
        #[arg(long, value_enum, default_value = "baseline")]
        suite: feel::FeelSuite,
        /// Run one named experience from the selected suite in an isolated game process.
        #[arg(long)]
        experience: Option<String>,
        /// New or empty evidence directory. Defaults beneath target/feel.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Exact game executable to capture, useful for release builds or macOS launch-path issues.
        #[arg(long)]
        game: Option<PathBuf>,
        /// Deadline for the first completed framebuffer and state record.
        #[arg(long, default_value_t = 10_000)]
        startup_timeout_ms: u64,
        /// Wall-clock deadline per attempt, including startup (up to 7,200,000 ms).
        /// Defaults to the authored scenario duration plus 60 seconds.
        #[arg(long)]
        run_timeout_ms: Option<u64>,
        /// Fresh-process retries after a live process produces no first frame.
        #[arg(long, default_value_t = 2)]
        startup_retries: u8,
    },
    /// Run the game shell with fixture or environment-configured local AI.
    Dev {
        /// Optimized interactive build with assertions; use dev for unoptimized debugging.
        #[arg(long, default_value = "dev-perf", value_parser = ["dev", "dev-perf"])]
        profile: String,
        #[arg(long)]
        fake_ai: bool,
        /// Outer worker watchdog in milliseconds.
        #[arg(long)]
        ai_timeout_ms: Option<u64>,
        /// Real local recognizer used with --stt-model-dir.
        #[arg(long, value_parser = ["parakeet", "moonshine"], default_value = "parakeet")]
        stt_backend: String,
        /// Enable real local STT with this model directory.
        #[arg(long, conflicts_with = "fake_ai")]
        stt_model_dir: Option<PathBuf>,
        /// Persistent local Moonshine engine used with --stt-model-dir.
        #[arg(long, requires = "stt_model_dir", conflicts_with = "fake_ai")]
        moonshine_engine: Option<PathBuf>,
        /// Watchdog for one local STT result in milliseconds.
        #[arg(long)]
        stt_timeout_ms: Option<u64>,
        /// Opt into GPL-blocked Kitten TTS instead of the release eSpeak backend.
        #[arg(long, requires = "tts_cache_dir")]
        tts_model_dir: Option<PathBuf>,
        /// Enable offline TTS with this cache directory.
        #[arg(long)]
        tts_cache_dir: Option<PathBuf>,
        /// eSpeak NG executable for the release-capable external-process backend.
        #[arg(long, requires = "tts_cache_dir")]
        tts_espeak: Option<PathBuf>,
        #[arg(long)]
        smoke: bool,
        #[arg(long)]
        script: Option<PathBuf>,
        #[arg(long, requires = "script")]
        capture_dir: Option<PathBuf>,
        #[arg(long, requires = "script")]
        stay_open: bool,
        #[arg(long)]
        new_game: bool,
    },
    /// Stage or check an offline release package.
    Package {
        /// Package root containing one platform subdirectory.
        #[arg(long, default_value = "dist")]
        destination: PathBuf,
        /// Target package layout. Defaults to the current host.
        #[arg(long, value_enum)]
        platform: Option<PackagePlatform>,
        /// Check an existing package instead of staging one.
        #[arg(long)]
        check: bool,
        /// Permit a development-only package without required release TTS and/or STT.
        #[arg(long)]
        development_package: bool,
        /// Already-built game executable.
        #[arg(long, required_unless_present = "check")]
        game: Option<PathBuf>,
        /// Already-built Beastie AI worker executable.
        #[arg(long, required_unless_present = "check")]
        worker: Option<PathBuf>,
        /// Already-built Beastie TTS worker. Provide all five TTS inputs or none.
        #[arg(long)]
        tts_worker: Option<PathBuf>,
        /// Separately distributed eSpeak NG executable.
        #[arg(long)]
        espeak: Option<PathBuf>,
        /// eSpeak NG data directory copied as runtime/espeak-ng-data.
        #[arg(long)]
        espeak_data: Option<PathBuf>,
        /// GPLv3 license file copied as runtime/espeak-ng-COPYING.
        #[arg(long)]
        espeak_license: Option<PathBuf>,
        /// Exact eSpeak NG 1.52.0 corresponding-source archive.
        #[arg(long)]
        espeak_source: Option<PathBuf>,
        /// Already-built Beastie STT worker.
        #[arg(long)]
        stt_worker: Option<PathBuf>,
        /// Optional external Moonshine engine for lightweight fallback packages.
        #[arg(long)]
        stt_engine: Option<PathBuf>,
        /// Optional external STT engine dynamic runtime library. Repeatable.
        #[arg(long)]
        stt_runtime: Vec<PathBuf>,
        /// Optional external STT runtime license snapshot.
        #[arg(long)]
        stt_runtime_license: Option<PathBuf>,
        /// Optional external STT runtime third-party notices.
        #[arg(long)]
        stt_runtime_notices: Option<PathBuf>,
        /// Directory containing exactly the selected STT model components.
        #[arg(long)]
        stt_model_dir: Option<PathBuf>,
        /// Selected STT model license snapshot.
        #[arg(long)]
        stt_model_license: Option<PathBuf>,
        /// Selected STT model card snapshot.
        #[arg(long)]
        stt_model_card: Option<PathBuf>,
        /// Selected local GGUF file.
        #[arg(long, required_unless_present = "check")]
        model: Option<PathBuf>,
        /// Exact upstream license snapshot for the selected model revision.
        #[arg(long, required_unless_present = "check")]
        model_license: Option<PathBuf>,
        /// Exact upstream model-card snapshot for the selected model revision.
        #[arg(long, required_unless_present = "check")]
        model_card: Option<PathBuf>,
        /// llama-server executable and each dynamic runtime library. Repeatable.
        #[arg(long, required_unless_present = "check")]
        runtime: Vec<PathBuf>,
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

#[derive(Debug, Subcommand)]
enum DialogueTask {
    /// Evaluate deterministic fixtures, or opt into a real llama.cpp worker run.
    Eval {
        /// Path to the Beastie AI worker. Enables a real-model run.
        #[arg(long)]
        worker: Option<PathBuf>,
        /// GGUF model passed to the real worker. Required with --worker.
        #[arg(long, requires = "worker")]
        model: Option<PathBuf>,
        /// llama-cli executable passed through to the real worker.
        #[arg(long, requires = "worker")]
        llama_cli: Option<PathBuf>,
        /// Per-request worker timeout in milliseconds.
        #[arg(long, requires = "worker")]
        timeout_ms: Option<u64>,
        /// Maximum bytes accepted from one model generation.
        #[arg(long, requires = "worker")]
        max_output_bytes: Option<usize>,
        /// Force llama.cpp onto the portable CPU-only path.
        #[arg(long, requires = "worker")]
        cpu_only: bool,
        /// Extra argument passed through to llama-cli. Repeatable.
        #[arg(long, requires = "worker", allow_hyphen_values = true)]
        llama_arg: Vec<String>,
        /// Safe report filename stem under evals/reports.
        #[arg(long, default_value = "local-model", requires = "worker")]
        label: String,
    },
}

#[derive(Debug, Subcommand)]
enum SttTask {
    /// Download and verify the selected local STT model for development.
    Setup,
    /// Evaluate deterministic replies, or opt into a real local worker run.
    Eval {
        /// Path to the beastie-stt executable. Enables a real-runtime run.
        #[arg(long)]
        worker: Option<PathBuf>,
        /// Local recognition backend.
        #[arg(
            long,
            value_parser = ["moonshine", "parakeet"],
            default_value = "moonshine",
            requires = "worker"
        )]
        backend: String,
        /// Directory containing the selected local model components.
        #[arg(long, requires = "worker")]
        model_dir: Option<PathBuf>,
        /// Moonshine transcriber engine, required only by that backend.
        #[arg(long, requires = "worker")]
        moonshine_engine: Option<PathBuf>,
        /// Extra worker argument. Repeatable.
        #[arg(long, requires = "worker", allow_hyphen_values = true)]
        worker_arg: Vec<String>,
        /// Safe report filename suffix under evals/reports.
        #[arg(long, default_value = "moonshine-local", requires = "worker")]
        label: String,
    },
}

#[derive(Debug, Subcommand)]
enum AssetTask {
    /// Validate sound and font files, metadata, and runtime readiness.
    Check {
        /// Print diagnostics for every resolved asset candidate.
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Debug, Subcommand)]
enum StoreAssetsTask {
    /// Deterministically generate all required Steam art files offline.
    Build,
    /// Validate dimensions, formats, title treatment, and source provenance.
    Check,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PackagePlatform {
    Macos,
    Windows,
    Linux,
}

impl PackagePlatform {
    fn host() -> Result<Self> {
        match std::env::consts::OS {
            "macos" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            other => bail!("unsupported package host platform: {other}"),
        }
    }
}

impl From<PackagePlatform> for packaging::Platform {
    fn from(value: PackagePlatform) -> Self {
        match value {
            PackagePlatform::Macos => Self::Macos,
            PackagePlatform::Windows => Self::Windows,
            PackagePlatform::Linux => Self::Linux,
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Task::Verify => verify(),
        Task::Asset {
            command: AssetTask::Check { verbose },
        } => asset::check(Path::new("assets/manifest.toml"), true, verbose),
        Task::StoreAssets {
            command: StoreAssetsTask::Build,
        } => store_assets::build(Path::new(".")),
        Task::StoreAssets {
            command: StoreAssetsTask::Check,
        } => store_assets::check(Path::new(".")),
        Task::Dialogue {
            command:
                DialogueTask::Eval {
                    worker,
                    model,
                    llama_cli,
                    timeout_ms,
                    max_output_bytes,
                    cpu_only,
                    llama_arg,
                    label,
                },
        } => dialogue_eval::run(dialogue_eval::EvalOptions {
            worker,
            model,
            llama_cli,
            timeout_ms,
            max_output_bytes,
            cpu_only,
            llama_args: llama_arg,
            label,
        }),
        Task::Stt {
            command: SttTask::Setup,
        } => stt_eval::setup(),
        Task::Stt {
            command:
                SttTask::Eval {
                    worker,
                    backend,
                    model_dir,
                    moonshine_engine,
                    worker_arg,
                    label,
                },
        } => stt_eval::run(stt_eval::EvalOptions {
            worker,
            backend,
            model_dir,
            moonshine_engine,
            worker_args: worker_arg,
            label,
        }),
        Task::Feel {
            suite,
            experience,
            output,
            game,
            startup_timeout_ms,
            run_timeout_ms,
            startup_retries,
        } => feel::run(feel::FeelOptions {
            suite,
            experience: experience.as_deref(),
            output: output.as_deref(),
            game: game.as_deref(),
            startup_timeout_ms,
            run_timeout_ms,
            startup_retries,
        }),
        Task::Dev {
            profile,
            fake_ai,
            ai_timeout_ms,
            stt_backend,
            stt_model_dir,
            moonshine_engine,
            stt_timeout_ms,
            tts_model_dir,
            tts_cache_dir,
            tts_espeak,
            smoke,
            script,
            capture_dir,
            stay_open,
            new_game,
        } => dev(DevOptions {
            profile: &profile,
            fake_ai,
            ai_timeout_ms,
            stt_backend: &stt_backend,
            stt_model_dir: stt_model_dir.as_deref(),
            moonshine_engine: moonshine_engine.as_deref(),
            stt_timeout_ms,
            tts_model_dir: tts_model_dir.as_deref(),
            tts_cache_dir: tts_cache_dir.as_deref(),
            tts_espeak: tts_espeak.as_deref(),
            smoke,
            script: script.as_deref(),
            capture_dir: capture_dir.as_deref(),
            stay_open,
            new_game,
        }),
        Task::Package {
            destination,
            platform,
            check,
            development_package,
            game,
            worker,
            tts_worker,
            espeak,
            espeak_data,
            espeak_license,
            espeak_source,
            stt_worker,
            stt_engine,
            stt_runtime,
            stt_runtime_license,
            stt_runtime_notices,
            stt_model_dir,
            stt_model_license,
            stt_model_card,
            model,
            model_license,
            model_card,
            runtime,
        } => package(PackageCommandOptions {
            destination: &destination,
            platform: platform.unwrap_or(PackagePlatform::host()?).into(),
            check_only: check,
            require_release_complete: !development_package,
            game: game.as_deref(),
            worker: worker.as_deref(),
            tts_worker: tts_worker.as_deref(),
            espeak: espeak.as_deref(),
            espeak_data: espeak_data.as_deref(),
            espeak_license: espeak_license.as_deref(),
            espeak_source: espeak_source.as_deref(),
            stt_worker: stt_worker.as_deref(),
            stt_engine: stt_engine.as_deref(),
            stt_runtime: &stt_runtime,
            stt_runtime_license: stt_runtime_license.as_deref(),
            stt_runtime_notices: stt_runtime_notices.as_deref(),
            stt_model_dir: stt_model_dir.as_deref(),
            stt_model_license: stt_model_license.as_deref(),
            stt_model_card: stt_model_card.as_deref(),
            model: model.as_deref(),
            model_license: model_license.as_deref(),
            model_card: model_card.as_deref(),
            runtime: &runtime,
        }),
        Task::Sim { seed, days } => sim(seed, days),
        Task::Play {
            seed,
            scenario,
            fake_ai,
        } => play(seed, scenario.as_deref(), fake_ai),
    }
}

struct PackageCommandOptions<'a> {
    destination: &'a Path,
    platform: packaging::Platform,
    check_only: bool,
    require_release_complete: bool,
    game: Option<&'a Path>,
    worker: Option<&'a Path>,
    tts_worker: Option<&'a Path>,
    espeak: Option<&'a Path>,
    espeak_data: Option<&'a Path>,
    espeak_license: Option<&'a Path>,
    espeak_source: Option<&'a Path>,
    stt_worker: Option<&'a Path>,
    stt_engine: Option<&'a Path>,
    stt_runtime: &'a [PathBuf],
    stt_runtime_license: Option<&'a Path>,
    stt_runtime_notices: Option<&'a Path>,
    stt_model_dir: Option<&'a Path>,
    stt_model_license: Option<&'a Path>,
    stt_model_card: Option<&'a Path>,
    model: Option<&'a Path>,
    model_license: Option<&'a Path>,
    model_card: Option<&'a Path>,
    runtime: &'a [PathBuf],
}

fn package(options: PackageCommandOptions<'_>) -> Result<()> {
    let PackageCommandOptions {
        destination,
        platform,
        check_only,
        require_release_complete,
        game,
        worker,
        tts_worker,
        espeak,
        espeak_data,
        espeak_license,
        espeak_source,
        stt_worker,
        stt_engine,
        stt_runtime,
        stt_runtime_license,
        stt_runtime_notices,
        stt_model_dir,
        stt_model_license,
        stt_model_card,
        model,
        model_license,
        model_card,
        runtime,
    } = options;
    if check_only {
        packaging::check(destination, platform, require_release_complete)?;
        return Ok(());
    }
    packaging::build(packaging::PackageOptions {
        destination,
        platform,
        game: game.context("--game is required when staging a package")?,
        worker: worker.context("--worker is required when staging a package")?,
        tts_worker,
        espeak,
        espeak_data,
        espeak_license,
        espeak_source,
        stt_worker,
        stt_engine,
        stt_runtime,
        stt_runtime_license,
        stt_runtime_notices,
        stt_model_dir,
        stt_model_license,
        stt_model_card,
        require_tts: require_release_complete,
        require_stt: require_release_complete,
        runtime,
        model: model.context("--model is required when staging a package")?,
        model_license: model_license
            .context("--model-license is required when staging a package")?,
        model_card: model_card.context("--model-card is required when staging a package")?,
        model_id: "qwen3.5-0.8b-q4_0",
        repository_root: Path::new("."),
    })?;
    Ok(())
}

fn verify() -> Result<()> {
    target_hygiene::maintain(Path::new("."))?;
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
    asset::check(Path::new("assets/manifest.toml"), true, false)?;
    verify_manifest("models/manifest.toml")?;
    dialogue_eval::verify_fixtures()?;
    stt_eval::verify_fixtures()?;
    replay_spoken_input_scenario(SPOKEN_INPUT_SCENARIO)?;
    store_assets::check(Path::new("."))?;
    run("cargo", &["build", "--workspace", "--locked"])?;
    replay_scenario(BERRY_GRUDGE_SCENARIO, true, false)?;
    replay_scenario("fixtures/scenarios/aquarium-v1.jsonl", true, false)
}

struct DevOptions<'a> {
    profile: &'a str,
    fake_ai: bool,
    ai_timeout_ms: Option<u64>,
    stt_backend: &'a str,
    stt_model_dir: Option<&'a Path>,
    moonshine_engine: Option<&'a Path>,
    stt_timeout_ms: Option<u64>,
    tts_model_dir: Option<&'a Path>,
    tts_cache_dir: Option<&'a Path>,
    tts_espeak: Option<&'a Path>,
    smoke: bool,
    script: Option<&'a Path>,
    capture_dir: Option<&'a Path>,
    stay_open: bool,
    new_game: bool,
}

fn dev(options: DevOptions<'_>) -> Result<()> {
    let DevOptions {
        profile,
        fake_ai,
        ai_timeout_ms,
        stt_backend,
        stt_model_dir,
        moonshine_engine,
        stt_timeout_ms,
        tts_model_dir,
        tts_cache_dir,
        tts_espeak,
        smoke,
        script,
        capture_dir,
        stay_open,
        new_game,
    } = options;
    if stt_backend == "moonshine" && stt_model_dir.is_some() && moonshine_engine.is_none() {
        bail!("--moonshine-engine is required with --stt-backend moonshine");
    }
    let worker = if fake_ai {
        None
    } else {
        run(
            "cargo",
            &[
                "build",
                "--package",
                "beastie-ai-worker",
                "--bin",
                "beastie-ai-worker",
                "--locked",
            ],
        )?;
        Some(
            std::env::current_dir()
                .context("failed to locate repository root")?
                .join("target")
                .join("debug")
                .join(format!("beastie-ai-worker{}", std::env::consts::EXE_SUFFIX)),
        )
    };
    let stt_worker = if fake_ai || stt_model_dir.is_some() {
        run(
            "cargo",
            &[
                "build",
                "--package",
                "beastie-ai-worker",
                "--bin",
                "beastie-stt",
                "--locked",
            ],
        )?;
        Some(
            std::env::current_dir()?
                .join("target")
                .join("debug")
                .join(format!("beastie-stt{}", std::env::consts::EXE_SUFFIX)),
        )
    } else {
        None
    };
    let tts_worker = if tts_cache_dir.is_some() {
        let mut arguments = vec!["build", "--package", "beastie-ai-worker"];
        if tts_model_dir.is_some() {
            eprintln!(
                "LICENSE BLOCKER: experimental TTS statically embeds GPLv3 espeak-ng; do not distribute this build"
            );
            arguments.extend(["--features", "experimental-gpl-tts"]);
        }
        arguments.extend(["--bin", "beastie-tts", "--locked"]);
        run("cargo", &arguments)?;
        Some(
            std::env::current_dir()?
                .join("target")
                .join("debug")
                .join(format!("beastie-tts{}", std::env::consts::EXE_SUFFIX)),
        )
    } else {
        None
    };
    run(
        "cargo",
        &[
            "build",
            "--package",
            "beastie-game",
            "--bin",
            "beastie-game",
            "--profile",
            profile,
            "--locked",
        ],
    )?;
    let game = std::env::current_dir()
        .context("failed to locate repository root")?
        .join("target")
        .join(if profile == "dev" { "debug" } else { profile })
        .join(format!("beastie-game{}", std::env::consts::EXE_SUFFIX));
    let mut command = Command::new(game);
    if fake_ai {
        command.arg("--fake-ai");
    }
    if let Some(timeout) = ai_timeout_ms {
        command.arg("--ai-timeout-ms").arg(timeout.to_string());
    }
    if let (Some(stt_worker), Some(model_dir)) = (stt_worker.as_ref(), stt_model_dir) {
        command
            .arg("--stt-worker")
            .arg(stt_worker)
            .arg("--stt-backend")
            .arg(stt_backend)
            .arg("--stt-model-dir")
            .arg(model_dir);
        if stt_backend == "moonshine" {
            command
                .arg("--moonshine-engine")
                .arg(moonshine_engine.expect("validated Moonshine engine"));
        }
    }
    if let Some(timeout) = stt_timeout_ms {
        command.arg("--stt-timeout-ms").arg(timeout.to_string());
    }
    if let (Some(tts_worker), Some(cache_dir)) = (tts_worker, tts_cache_dir) {
        command
            .arg("--tts")
            .env("BEASTIE_TTS_WORKER", tts_worker)
            .env("BEASTIE_TTS_CACHE_DIR", cache_dir);
        if let Some(espeak) = tts_espeak {
            command.env("BEASTIE_ESPEAK_NG", espeak);
        }
        if let Some(model_dir) = tts_model_dir {
            command
                .env("BEASTIE_TTS_BACKEND", "sherpa-kitten")
                .env("BEASTIE_TTS_MODEL_DIR", model_dir);
        }
    }
    if smoke {
        command.arg("--smoke");
    }
    if let Some(script) = script {
        command.arg("--script").arg(script);
    }
    if let Some(capture_dir) = capture_dir {
        command.arg("--capture-dir").arg(capture_dir);
    }
    if stay_open {
        command.arg("--stay-open");
    }
    if new_game {
        command.arg("--new-game");
    }
    if let Some(worker) = worker {
        command.env("BEASTIE_AI_WORKER", worker);
    }
    if fake_ai && let Some(stt_worker) = stt_worker {
        command.env("BEASTIE_STT_WORKER", stt_worker);
    }
    let mut child = command.spawn().context("failed to start beastie-game")?;
    if !smoke {
        focus_macos_process(child.id());
    }
    if !stay_open && (smoke || script.is_some()) {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = child.try_wait().context("failed to poll beastie-game")? {
                return require_success("beastie-game", status);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!(
                    "beastie-game did not finish within 120 seconds; on macOS run the command from a focused foreground terminal if Metal did not acquire a drawable"
                );
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    let status = child.wait().context("failed to wait for beastie-game")?;
    require_success("beastie-game", status)
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
    let report = match scenario {
        Some(path) => {
            let file = File::open(path)
                .with_context(|| format!("failed to open scenario {}", path.display()))?;
            process_commands(BufReader::new(file), &mut output, seed)
        }
        None => process_commands(io::stdin().lock(), &mut output, seed),
    }?;
    reject_adapter_errors(report)
}

fn replay_scenario(path: &str, expect_grounded_reply: bool, expect_refusal: bool) -> Result<()> {
    let file = File::open(path).with_context(|| format!("failed to open scenario {path}"))?;
    let report = process_commands(BufReader::new(file), io::sink(), 99)?;
    if report.accepted == 0 {
        bail!("scenario {path} contains no commands");
    }
    reject_adapter_errors(report)?;
    if !report.food_consumed
        || report.grounded_reply != expect_grounded_reply
        || report.refused != expect_refusal
    {
        bail!(
            "scenario {path} did not produce its expected consumption, grounding, and refusal arc"
        );
    }
    Ok(())
}

fn replay_spoken_input_scenario(path: &str) -> Result<()> {
    let file = File::open(path).with_context(|| format!("failed to open scenario {path}"))?;
    let mut session = GameSession::new(99, "Mop");
    let mut listening = 0;
    let mut candidate_updates = 0;
    let mut submitted = 0;
    let mut uncertainty = 0;
    let mut infrastructure_failures = 0;
    let mut dialogue_requests = 0;

    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("failed to read {path}:{}", index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let envelope = GameSession::parse_command(&line)
            .with_context(|| format!("invalid command in {path}:{}", index + 1))?;
        let observation = session
            .apply(envelope)
            .with_context(|| format!("failed command in {path}:{}", index + 1))?;
        dialogue_requests += usize::from(observation.dialogue_request.is_some());
        match observation.spoken_input {
            Some(SpokenInputStatus::Listening) => listening += 1,
            Some(SpokenInputStatus::CandidateUpdated { .. }) => candidate_updates += 1,
            Some(SpokenInputStatus::Submitted) => submitted += 1,
            Some(SpokenInputStatus::AcousticUncertainty { .. }) => uncertainty += 1,
            Some(SpokenInputStatus::InfrastructureFailure { .. }) => {
                infrastructure_failures += 1;
            }
            Some(SpokenInputStatus::Refused) => {}
            Some(SpokenInputStatus::Deferred)
            | Some(SpokenInputStatus::NotEngaged { .. })
            | Some(SpokenInputStatus::Expired)
            | Some(SpokenInputStatus::NoCandidate)
            | None => {}
        }
    }

    if (
        listening,
        candidate_updates,
        submitted,
        uncertainty,
        infrastructure_failures,
    ) != (3, 2, 1, 1, 1)
        || dialogue_requests != 1
    {
        bail!(
            "spoken-input scenario produced unexpected lifecycle counts: listening={listening}, candidates={candidate_updates}, submitted={submitted}, uncertainty={uncertainty}, failures={infrastructure_failures}, dialogue={dialogue_requests}"
        );
    }
    println!("spoken-input replay passed: {path}");
    Ok(())
}

fn reject_adapter_errors(report: PlayReport) -> Result<()> {
    if report.rejected == 0 {
        return Ok(());
    }
    bail!(
        "play encountered {} adapter error(s) across {} command(s)",
        report.rejected,
        report.accepted + report.rejected
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PlayReport {
    accepted: usize,
    rejected: usize,
    grounded_reply: bool,
    food_consumed: bool,
    refused: bool,
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
                report.food_consumed |= observation
                    .observation
                    .events
                    .iter()
                    .any(|event| matches!(event, GameEvent::FoodConsumed(_)));
                report.refused |= observation
                    .observation
                    .events
                    .iter()
                    .any(|event| matches!(event, GameEvent::UtteranceRefused));
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
    let mut reply = constrained_fallback_reply(request);
    if request
        .constraints
        .allowed_gestures
        .contains(&Gesture::LookPlayer)
    {
        reply.gesture = Gesture::LookPlayer;
    }
    reply.recalled_memory = memory.map(|candidate| candidate.id);
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
            SessionError::Tick(_) => "invalid_tick",
            SessionError::Resume(_) => "invalid_resume",
            SessionError::TalkTooLong => "talk_too_long",
            SessionError::EmptySpeechCandidate => "empty_speech_candidate",
            SessionError::SpeechAlreadyStarted => "speech_already_started",
            SessionError::SpeechNotStarted => "speech_not_started",
            SessionError::UtteranceBusy => "utterance_busy",
            SessionError::NoCheckpoint => "no_checkpoint",
            SessionError::SaveVersion(_)
            | SessionError::LegacySave(_)
            | SessionError::RequestId => "save_failed",
            SessionError::State(_) => "invalid_state",
            SessionError::Dialogue(_) => "invalid_dialogue_request",
            SessionError::DialogueHistory => "invalid_dialogue_history",
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
    fn feel_cli_keeps_run_timeout_optional_and_accepts_an_override() {
        for (arguments, expected) in [
            (vec!["xtask", "feel"], None),
            (
                vec!["xtask", "feel", "--run-timeout-ms", "900000"],
                Some(900_000),
            ),
        ] {
            let cli = Cli::try_parse_from(arguments).unwrap();
            let Task::Feel { run_timeout_ms, .. } = cli.command else {
                panic!("expected feel command");
            };
            assert_eq!(run_timeout_ms, expected);
        }
    }

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
        let input = include_bytes!("../../../fixtures/scenarios/aquarium-v1.jsonl");
        let mut output = Vec::new();
        let report = process_commands(&input[..], &mut output, 99).expect("scenario should run");
        let lines = String::from_utf8(output).expect("output should be UTF-8");
        let talk = lines
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSON"))
            .find(|value| value.get("speech").is_some())
            .expect("scenario should talk");

        assert_eq!(report.rejected, 0);
        assert!(report.grounded_reply);
        assert!(report.food_consumed);
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
    fn fixture_reply_obeys_hatch_word_limit() {
        let mut session = GameSession::new(42, "Mop");
        let command =
            GameSession::parse_command(r#"{"version":1,"command":"talk","text":"hello"}"#)
                .expect("talk command should parse");
        let request = session
            .apply(command)
            .expect("talk should apply")
            .dialogue_request
            .expect("hatch talk should request dialogue");

        assert_eq!(request.constraints.max_words, 3);
        let reply = fixture_reply(&request).expect("fixture reply should validate");
        assert!(reply.say.split_whitespace().count() <= 3);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn adapter_errors_make_play_fail_after_emitting_jsonl() {
        let mut output = Vec::new();
        let report = process_commands(b"not json\n".as_slice(), &mut output, 42)
            .expect("adapter should emit its error JSONL");

        assert_eq!(report.rejected, 1);
        assert!(reject_adapter_errors(report).is_err());
        assert!(reject_adapter_errors(PlayReport::default()).is_ok());
    }

    #[test]
    fn spoken_input_order_errors_have_stable_adapter_codes() {
        let input = concat!(
            "{\"version\":1,\"command\":\"speech_started\"}\n",
            "{\"version\":1,\"command\":\"speech_started\"}\n",
            "{\"version\":1,\"command\":\"speech_failed\",\"failure\":\"recognition_failed\"}\n",
            "{\"version\":1,\"command\":\"speech_candidate\",\"text\":\"\",\"confidence\":900}\n",
            "{\"version\":1,\"command\":\"speech_ended\"}\n",
        );
        let mut output = Vec::new();
        let report = process_commands(input.as_bytes(), &mut output, 42).expect("stream runs");
        let codes = String::from_utf8(output)
            .expect("UTF-8")
            .lines()
            .filter_map(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .expect("JSON")
                    .pointer("/error/code")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();

        assert_eq!(report.rejected, 3);
        assert_eq!(
            codes,
            [
                "speech_already_started",
                "empty_speech_candidate",
                "speech_not_started"
            ]
        );
    }

    #[test]
    fn simulation_day_uses_the_accelerated_active_day() {
        assert_eq!(ACTIVE_DAY_MS / 60_000, 15);
    }
}
