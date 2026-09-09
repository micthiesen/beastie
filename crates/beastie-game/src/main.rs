mod app;
mod appearance;
mod args;
mod audio;
mod body;
mod capture;
mod creature;

mod dialogue;
mod environment;
mod error;
mod feel;
mod glyphs;
mod host;
mod input;
mod microphone;
mod motion_trace;
mod process;
mod ray_scene;
mod ray_stats;
#[cfg(test)]
mod ray_validation;
mod raytrace;
mod recognition;
mod renderer;
mod save_store;
mod scenario;
mod settings;
mod transcript;
mod tts;
mod voxel;

use app::Game;
use args::Args;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode, WindowPlugin};
use clap::Parser;

fn main() -> bevy::app::AppExit {
    let args = Args::parse();
    let game = match Game::new(&args) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("Beastie could not start: {error}");
            return bevy::app::AppExit::error();
        }
    };
    let (fullscreen, scale) = game.window_settings();
    let frame = renderer::SceneFrame {
        plan: game.render_plan(),
    };
    App::new()
        .insert_resource(host::HostInputPolicy::from_script_only(
            args.feel_script_only,
        ))
        .insert_resource(appearance::RenderAppearance {
            treatment: args.surface_treatment.unwrap_or_default(),
            study: args.lighting_study.unwrap_or_default(),
        })
        .insert_resource(frame)
        .insert_non_send(game)
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Beastie".to_owned(),
                        resolution: bevy::window::WindowResolution::new(
                            640 * u32::from(scale),
                            360 * u32::from(scale),
                        )
                        .with_scale_factor_override(1.0),
                        resizable: false,
                        present_mode: if args.render_uncapped {
                            bevy::window::PresentMode::AutoNoVsync
                        } else {
                            default()
                        },
                        mode: if fullscreen {
                            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                        } else {
                            WindowMode::Windowed
                        },
                        ..default()
                    }),
                    close_when_requested: false,
                    ..default()
                })
                .disable::<bevy::pbr::PbrPlugin>()
                .disable::<bevy::light::LightPlugin>(),
        )
        // StandardMaterial is CPU-side scene data for the compute ray renderer.
        .init_asset::<StandardMaterial>()
        .add_plugins((
            host::HostPlugin,
            renderer::RendererPlugin,
            ray_scene::RayScenePlugin,
            ray_stats::RayStatsPlugin(args.render_report.clone(), args.render_uncapped),
            raytrace::RayTracePlugin,
            capture::CapturePlugin,
        ))
        .run()
}
