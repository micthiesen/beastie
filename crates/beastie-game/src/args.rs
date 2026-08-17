use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Beastie native game shell")]
pub struct Args {
    /// Use the deterministic fixture-backed dialogue worker.
    #[arg(long)]
    pub fake_ai: bool,
    /// Capture-only simulation-time delay before fixture dialogue acceptance.
    #[arg(
        long,
        hide = true,
        requires_all = ["fake_ai", "script", "feel_dir"],
        value_parser = parse_feel_dialogue_delay_ms
    )]
    pub feel_dialogue_delay_ms: Option<u64>,
    /// Watchdog for one reply from the long-lived outer AI worker.
    #[arg(long)]
    pub ai_timeout_ms: Option<u64>,
    /// Development override for the isolated local STT worker.
    #[arg(long)]
    pub stt_worker: Option<PathBuf>,
    /// Local STT backend used by a development worker.
    #[arg(
        long,
        value_parser = ["parakeet", "moonshine"],
        default_value = "parakeet"
    )]
    pub stt_backend: String,
    /// Development override for the Moonshine model directory.
    #[arg(long, requires = "stt_worker")]
    pub stt_model_dir: Option<PathBuf>,
    /// Development override for the persistent Moonshine engine.
    #[arg(long, requires = "stt_worker")]
    pub moonshine_engine: Option<PathBuf>,
    /// Watchdog for one reply from the STT worker.
    #[arg(long)]
    pub stt_timeout_ms: Option<u64>,
    /// Enable environment-configured TTS; complete packaged bundles enable automatically.
    #[arg(long)]
    pub tts: bool,
    /// Exit after a few rendered frames.
    #[arg(long)]
    pub smoke: bool,
    /// Replay semantic commands through a fresh, non-persistent visible game.
    #[arg(long)]
    pub script: Option<PathBuf>,
    /// Directory for named 640x360 presentation captures.
    #[arg(long, requires = "script")]
    pub capture_dir: Option<PathBuf>,
    /// Directory for a 60 fps feel-review evidence bundle.
    #[arg(long, requires = "script")]
    pub feel_dir: Option<PathBuf>,
    /// Validated deterministic starting state used only by scripted feel evidence.
    #[arg(
        long,
        hide = true,
        requires_all = ["script", "feel_dir"],
        conflicts_with = "new_game"
    )]
    pub feel_initial_save: Option<PathBuf>,
    /// Keep a completed visible scenario open for native input inspection.
    #[arg(long, requires = "script")]
    pub stay_open: bool,
    /// Ignore an existing save and start a new creature.
    #[arg(long)]
    pub new_game: bool,
    /// Opt in to privacy-safe local playtest transcript recording. Player text is never stored.
    #[arg(long)]
    pub transcript: bool,
    /// Export validated privacy-safe transcript records to this JSONL file.
    #[arg(long)]
    pub export_transcript: Option<PathBuf>,
}

fn parse_feel_dialogue_delay_ms(value: &str) -> Result<u64, String> {
    let milliseconds = value
        .parse::<u64>()
        .map_err(|_| "feel dialogue delay must be an integer".to_owned())?;
    if !(1..=5_000).contains(&milliseconds) {
        return Err("feel dialogue delay must be within 1..=5000 ms".to_owned());
    }
    Ok(milliseconds)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Args;

    #[test]
    fn feel_directory_requires_a_script() {
        assert!(Args::try_parse_from(["beastie-game", "--feel-dir", "target/feel/run"]).is_err());
        assert!(
            Args::try_parse_from([
                "beastie-game",
                "--script",
                "fixtures/scenarios/aquarium-v1-visible.jsonl",
                "--feel-dir",
                "target/feel/run",
            ])
            .is_ok()
        );
    }

    #[test]
    fn feel_initial_save_requires_the_full_evidence_path() {
        assert!(
            Args::try_parse_from([
                "beastie-game",
                "--feel-initial-save",
                "fixtures/saves/feel/trusted-berry.json",
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "beastie-game",
                "--script",
                "fixtures/scenarios/feel/relationship-breadth/trusted-berry.jsonl",
                "--feel-dir",
                "target/feel/run",
                "--feel-initial-save",
                "fixtures/saves/feel/trusted-berry.json",
            ])
            .is_ok()
        );
    }

    #[test]
    fn feel_dialogue_delay_is_bounded_and_capture_only() {
        let complete = [
            "beastie-game",
            "--fake-ai",
            "--script",
            "fixtures/scenarios/feel/dialogue-races.jsonl",
            "--feel-dir",
            "target/feel/run",
            "--feel-dialogue-delay-ms",
            "800",
        ];
        assert!(Args::try_parse_from(complete).is_ok());
        assert!(
            Args::try_parse_from([
                "beastie-game",
                "--fake-ai",
                "--feel-dialogue-delay-ms",
                "800",
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "beastie-game",
                "--fake-ai",
                "--script",
                "fixtures/scenarios/feel/dialogue-races.jsonl",
                "--feel-dir",
                "target/feel/run",
                "--feel-dialogue-delay-ms",
                "5001",
            ])
            .is_err()
        );
    }
}
