use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use beastie_core::{SeededRandom, WorldState, step};
use beastie_protocol::{DialogueReply, DialogueRequest, validate_reply, validate_request};
use beastie_view::plan;
use ggez::conf::{WindowMode, WindowSetup};
use ggez::event::{self, EventHandler};
use ggez::graphics::{Canvas, Color, DrawMode, DrawParam, Mesh, Rect, Text};
use ggez::{Context, ContextBuilder, GameResult};

struct Game {
    world: WorldState,
    rng: SeededRandom,
    smoke_frames: Option<u8>,
    speech: Option<String>,
    speech_receiver: Option<Receiver<Option<String>>>,
}

impl Game {
    fn new(smoke: bool, fake_ai: bool) -> Self {
        let world = WorldState::new(42, "Mop");
        let speech_receiver = fake_ai.then(|| {
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let _ = sender.send(request_fixture_dialogue());
            });
            receiver
        });
        Self {
            rng: SeededRandom::new(world.seed),
            world,
            smoke_frames: smoke.then_some(3),
            speech: None,
            speech_receiver,
        }
    }
}

impl EventHandler for Game {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        let dt_ms = ctx.time.delta().as_millis().try_into().unwrap_or(u64::MAX);
        step(&mut self.world, &[], dt_ms, &mut self.rng);
        if let Some(receiver) = &self.speech_receiver {
            match receiver.try_recv() {
                Ok(speech) => {
                    self.speech = speech;
                    self.speech_receiver = None;
                }
                Err(TryRecvError::Disconnected) => self.speech_receiver = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(frames) = &mut self.smoke_frames {
            *frames = frames.saturating_sub(1);
            if *frames == 0 {
                ctx.request_quit();
            }
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let (render, _) = plan(&self.world, self.speech.as_deref());
        let mut canvas = Canvas::from_frame(ctx, Color::from_rgb(29, 25, 34));
        canvas.set_screen_coordinates(Rect::new(0.0, 0.0, 320.0, 180.0));
        execute_render_plan(ctx, &mut canvas, &render)?;
        canvas.finish(ctx)
    }
}

fn execute_render_plan(
    ctx: &mut Context,
    canvas: &mut Canvas,
    render: &beastie_view::RenderPlan,
) -> GameResult {
    let mut sprites = render.sprites.iter().collect::<Vec<_>>();
    sprites.sort_by_key(|sprite| sprite.layer);
    for sprite in sprites {
        let mesh = if sprite.id == "room/background" {
            Mesh::new_rectangle(
                ctx,
                DrawMode::fill(),
                Rect::new(sprite.x as f32, sprite.y as f32, 320.0, 180.0),
                Color::from_rgb(77, 59, 65),
            )?
        } else if sprite.id.starts_with("creature/") {
            Mesh::new_circle(
                ctx,
                DrawMode::fill(),
                [sprite.x as f32, sprite.y as f32],
                16.0,
                0.5,
                Color::from_rgb(188, 207, 142),
            )?
        } else {
            continue;
        };
        canvas.draw(&mesh, DrawParam::default());
    }
    for command in &render.text {
        canvas.draw(
            &Text::new(command.text.as_str()),
            DrawParam::default().dest([command.x as f32, command.y as f32]),
        );
    }
    Ok(())
}

fn request_fixture_dialogue() -> Option<String> {
    let worker = std::env::var_os("BEASTIE_AI_WORKER")?;
    let request_text = include_str!("../../../fixtures/dialogue/berry-memory.json").trim();
    let request: DialogueRequest = serde_json::from_str(request_text).ok()?;
    validate_request(&request).ok()?;
    let mut child = Command::new(worker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    writeln!(stdin, "{request_text}").ok()?;
    drop(stdin);
    let stdout = child.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut reply_line = String::new();
        let result = BufReader::new(stdout)
            .read_line(&mut reply_line)
            .ok()
            .map(|_| reply_line);
        let _ = sender.send(result);
    });
    let reply_line = match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(Some(reply_line)) => reply_line,
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return None;
        }
    };
    let mut worker_succeeded = false;
    for _ in 0..20 {
        match child.try_wait() {
            Ok(Some(status)) => {
                worker_succeeded = status.success();
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return None;
            }
        }
    }
    if !worker_succeeded {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        return None;
    }
    reader.join().ok()?;
    let reply: DialogueReply = serde_json::from_str(&reply_line).ok()?;
    validate_reply(&request, reply).ok().map(|reply| reply.say)
}

fn main() -> GameResult {
    let smoke = std::env::args().any(|argument| argument == "--smoke");
    let fake_ai = std::env::args().any(|argument| argument == "--fake-ai");
    let (ctx, event_loop) = ContextBuilder::new("beastie", "Michael Thiesen")
        .window_setup(WindowSetup::default().title("Beastie"))
        .window_mode(
            WindowMode::default()
                .dimensions(960.0, 540.0)
                .resizable(true),
        )
        .build()?;
    event::run(ctx, event_loop, Game::new(smoke, fake_ai))
}
