use std::io;
use std::path::PathBuf;

use beastie_ai_worker::tts::{SherpaKittenSynthesizer, run_tts_jsonl};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "EXPERIMENTAL GPL-BLOCKED native Kitten TTS smoke tool")]
struct Args {
    /// Extracted kitten-nano-en-v0_8-int8 directory.
    #[arg(long, env = "BEASTIE_TTS_MODEL_DIR")]
    model_dir: PathBuf,

    /// Directory containing only content-hashed WAV cache entries.
    #[arg(long, env = "BEASTIE_TTS_CACHE_DIR")]
    cache_dir: PathBuf,

    #[arg(long, default_value_t = 2)]
    threads: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "LICENSE BLOCKER: this experimental build statically embeds GPLv3 espeak-ng; do not distribute it"
    );
    let args = Args::parse();
    let mut synthesizer = SherpaKittenSynthesizer::load(&args.model_dir, args.threads)?;
    run_tts_jsonl(
        io::stdin().lock(),
        io::stdout().lock(),
        &args.cache_dir,
        &mut synthesizer,
    )
    .map_err(Into::into)
}
