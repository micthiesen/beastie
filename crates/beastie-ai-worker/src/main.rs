use std::io;
use std::path::PathBuf;
use std::time::Duration;

use beastie_ai_worker::{FixtureBackend, LlamaCppBackend, LlamaCppConfig, run_jsonl};
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendKind {
    Fixture,
    LlamaCpp,
}

#[derive(Debug, Parser)]
#[command(about = "Offline JSONL dialogue worker")]
struct Args {
    /// Dialogue implementation. Fixture remains the deterministic default.
    #[arg(long, env = "BEASTIE_AI_BACKEND", value_enum, default_value_t = BackendKind::Fixture)]
    backend: BackendKind,

    /// GGUF model passed to llama-cli. Required by the llama-cpp backend.
    #[arg(long, env = "BEASTIE_AI_MODEL")]
    model: Option<PathBuf>,

    /// llama.cpp command-line executable.
    #[arg(long, env = "BEASTIE_LLAMA_CLI", default_value = "llama-cli")]
    llama_cli: PathBuf,

    /// Hard timeout for each llama-cli attempt.
    #[arg(long, env = "BEASTIE_AI_TIMEOUT_MS", default_value_t = 30_000)]
    timeout_ms: u64,

    /// Maximum stdout accepted from each llama-cli attempt.
    #[arg(long, env = "BEASTIE_AI_MAX_OUTPUT_BYTES", default_value_t = 16_384)]
    max_output_bytes: usize,

    /// Force llama.cpp off GPU and op-offload paths.
    #[arg(long, env = "BEASTIE_AI_CPU_ONLY")]
    cpu_only: bool,

    /// Extra argument passed to llama-cli before Beastie's fixed arguments.
    #[arg(long = "llama-arg", allow_hyphen_values = true)]
    llama_args: Vec<std::ffi::OsString>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let stdin = io::stdin();
    let stdout = io::stdout();

    match args.backend {
        BackendKind::Fixture => run_jsonl(stdin.lock(), stdout.lock(), &mut FixtureBackend)?,
        BackendKind::LlamaCpp => {
            let model = args.model.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--model or BEASTIE_AI_MODEL is required for --backend llama-cpp",
                )
            })?;
            let mut backend = LlamaCppBackend::new(LlamaCppConfig {
                executable: args.llama_cli,
                model,
                timeout: Duration::from_millis(args.timeout_ms),
                max_output_bytes: args.max_output_bytes,
                cpu_only: args.cpu_only,
                extra_args: args.llama_args,
            });
            run_jsonl(stdin.lock(), stdout.lock(), &mut backend)?;
        }
    }
    Ok(())
}
