mod app;
mod args;
mod audio;
mod dialogue;
mod input;
mod process;
mod renderer;
mod save_store;
mod scenario;
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
                .dimensions(960.0, 540.0)
                .min_dimensions(320.0, 180.0)
                .resizable(true),
        )
        .build()?;
    let game = Game::new(&mut ctx, &args)?;
    event::run(ctx, event_loop, game)
}
