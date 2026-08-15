use std::io;
use std::path::PathBuf;

#[cfg(feature = "experimental-gpl-tts")]
use beastie_ai_worker::tts::SherpaKittenSynthesizer;
use beastie_ai_worker::tts::{EspeakNgSynthesizer, TtsSynthesizer, run_tts_jsonl};
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Backend {
    Espeak,
    SherpaKitten,
}

#[derive(Debug, Parser)]
#[command(about = "Bounded offline TTS JSONL worker")]
struct Args {
    /// TTS implementation. eSpeak NG runs as a separately installed process.
    #[arg(long, env = "BEASTIE_TTS_BACKEND", default_value = "espeak")]
    backend: Backend,

    /// eSpeak NG executable used by the release-capable process backend.
    #[arg(long, env = "BEASTIE_ESPEAK_NG", default_value = "espeak-ng")]
    espeak: PathBuf,

    /// Directory named espeak-ng-data for a bundled eSpeak NG runtime.
    #[arg(long, env = "BEASTIE_ESPEAK_DATA")]
    espeak_data: Option<PathBuf>,

    /// Base eSpeak NG voice; speaker IDs 1..=7 select its male variants.
    #[arg(long, env = "BEASTIE_ESPEAK_VOICE", default_value = "en-us")]
    voice: String,

    /// Extracted kitten-nano-en-v0_8-int8 directory.
    #[arg(long, env = "BEASTIE_TTS_MODEL_DIR")]
    model_dir: Option<PathBuf>,

    /// Directory containing only content-hashed WAV cache entries.
    #[arg(long, env = "BEASTIE_TTS_CACHE_DIR")]
    cache_dir: PathBuf,

    #[arg(long, default_value_t = 2)]
    threads: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut synthesizer: Box<dyn TtsSynthesizer> = match args.backend {
        Backend::Espeak => Box::new(EspeakNgSynthesizer::new(
            args.espeak,
            args.voice,
            args.espeak_data,
        )?),
        Backend::SherpaKitten => {
            #[cfg(feature = "experimental-gpl-tts")]
            {
                eprintln!(
                    "LICENSE BLOCKER: this build statically embeds GPLv3 espeak-ng; do not distribute it"
                );
                let model_dir = args
                    .model_dir
                    .ok_or("--model-dir is required for sherpa-kitten")?;
                Box::new(SherpaKittenSynthesizer::load(&model_dir, args.threads)?)
            }
            #[cfg(not(feature = "experimental-gpl-tts"))]
            {
                let _ = (args.model_dir, args.threads);
                return Err(
                    "sherpa-kitten requires the non-distributable experimental-gpl-tts feature"
                        .into(),
                );
            }
        }
    };
    run_tts_jsonl(
        io::stdin().lock(),
        io::stdout().lock(),
        &args.cache_dir,
        synthesizer.as_mut(),
    )
    .map_err(Into::into)
}
