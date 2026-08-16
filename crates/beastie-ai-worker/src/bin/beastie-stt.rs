use std::io;
use std::path::PathBuf;
use std::time::Duration;

use beastie_ai_worker::stt::{
    FixtureSttBackend, MoonshineBackend, MoonshineConfig, SttBackend, run_stt_jsonl,
};
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Backend {
    Fixture,
    Moonshine,
}

#[derive(Debug, Parser)]
#[command(about = "Bounded offline STT JSONL worker")]
struct Args {
    /// Recognition implementation. Fixture remains deterministic and model-free.
    #[arg(long, env = "BEASTIE_STT_BACKEND", value_enum, default_value_t = Backend::Fixture)]
    backend: Backend,

    /// Directory containing only `<sha256>.wav` recognition inputs.
    #[arg(long, env = "BEASTIE_STT_AUDIO_ROOT")]
    audio_root: PathBuf,

    /// Moonshine Voice v0.1.2 Tiny Streaming arch 2 model directory.
    #[arg(long, env = "BEASTIE_STT_MODEL_DIR")]
    model_dir: Option<PathBuf>,

    /// Beastie's persistent Moonshine sidecar executable.
    #[arg(
        long,
        env = "BEASTIE_MOONSHINE_ENGINE",
        default_value = "beastie-moonshine-engine"
    )]
    moonshine_engine: PathBuf,

    /// Hard deadline for each recognition result.
    #[arg(long, env = "BEASTIE_STT_TIMEOUT_MS", default_value_t = 15_000)]
    timeout_ms: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut backend: Box<dyn SttBackend> = match args.backend {
        Backend::Fixture => {
            if args.model_dir.is_some() {
                return Err("--model-dir is only valid with --backend moonshine".into());
            }
            Box::new(FixtureSttBackend)
        }
        Backend::Moonshine => {
            let model_dir = args
                .model_dir
                .ok_or("--model-dir is required with --backend moonshine")?;
            Box::new(MoonshineBackend::new(MoonshineConfig {
                executable: args.moonshine_engine,
                model_dir,
                audio_root: args.audio_root.clone(),
                timeout: Duration::from_millis(args.timeout_ms),
            }))
        }
    };
    run_stt_jsonl(
        io::stdin().lock(),
        io::stdout().lock(),
        &args.audio_root,
        backend.as_mut(),
    )?;
    Ok(())
}
