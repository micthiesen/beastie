use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Beastie native game shell")]
pub struct Args {
    /// Use the deterministic fixture-backed dialogue worker.
    #[arg(long)]
    pub fake_ai: bool,
    /// Exit after a few rendered frames.
    #[arg(long)]
    pub smoke: bool,
    /// Replay semantic commands through a fresh, non-persistent visible game.
    #[arg(long)]
    pub script: Option<PathBuf>,
    /// Directory for named logical-framebuffer captures.
    #[arg(long, requires = "script")]
    pub capture_dir: Option<PathBuf>,
    /// Keep a completed visible scenario open for native input inspection.
    #[arg(long, requires = "script")]
    pub stay_open: bool,
    /// Ignore an existing save and start a new creature.
    #[arg(long)]
    pub new_game: bool,
}
