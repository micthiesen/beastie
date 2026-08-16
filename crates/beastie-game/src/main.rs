mod app;
mod args;
mod audio;
mod dialogue;
mod input;
mod microphone;
mod process;
mod recognition;
mod renderer;
mod save_store;
mod scenario;
mod settings;
mod transcript;
mod tts;

use app::Game;
use args::Args;
use clap::Parser;
use ggez::conf::{WindowMode, WindowSetup};
use ggez::event;
use ggez::{ContextBuilder, GameResult};

fn main() -> GameResult {
    let args = Args::parse();
    let (mut ctx, event_loop) = ContextBuilder::new("beastie", "Michael Thiesen")
        .window_setup(WindowSetup::default().title("Beastie"))
        .window_mode(
            WindowMode::default()
                .dimensions(1280.0, 720.0)
                .resizable(false),
        )
        .build()?;
    let game = Game::new(&mut ctx, &args)?;
    event::run(ctx, event_loop, game)
}
