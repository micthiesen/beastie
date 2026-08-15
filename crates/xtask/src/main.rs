use std::process::{Command, ExitStatus};

use anyhow::{Context, Result, bail};
use beastie_core::{SeededRandom, WorldState, step};
use clap::{Parser, Subcommand};

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
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Task::Verify => verify(),
        Task::Dev { fake_ai, smoke } => dev(fake_ai, smoke),
        Task::Sim { seed, days } => sim(seed, days),
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
    run("cargo", &["build", "--workspace", "--locked"])
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
    let minutes = u64::from(days) * 24 * 60;
    for _ in 0..minutes {
        step(&mut world, &[], 60_000, &mut rng);
    }
    serde_json::to_writer_pretty(std::io::stdout(), &world)
        .context("failed to write simulation result")?;
    Ok(())
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
