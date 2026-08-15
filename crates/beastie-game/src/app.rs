use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beastie_core::ToyId;
use beastie_session::{
    CommandEnvelope, GameSession, SESSION_PROTOCOL_VERSION, SessionCommand, SessionError,
};
use beastie_view::{RenderPlan, UiAction, UiMode, ViewState, plan};
use ggez::event::{Button, EventHandler, GamepadId};
use ggez::graphics::{Canvas, Color, DrawParam, Image, Sampler};
use ggez::input::keyboard::KeyInput;
use ggez::input::mouse::MouseButton;
use ggez::winit::keyboard::{Key, NamedKey};
use ggez::{Context, GameError, GameResult};

use crate::args::Args;
use crate::audio::{AudioBank, UI_CONFIRM, UI_SELECT, sound_for_event};
use crate::dialogue::{DialogueManager, WorkerConfig};
use crate::input::{action_at, append_text, focused_action, move_focus};
use crate::renderer::{AssetCatalog, Viewport, execute_plan, save_logical_png};
use crate::save_store::SaveStore;
use crate::scenario::{ScenarioRunner, ScenarioStep};
use crate::tts::{TtsManager, TtsWorkerConfig};

struct CaptureRequest {
    name: String,
    ready_to_read: bool,
}

pub struct Game {
    session: GameSession,
    view: ViewState,
    logical_frame: Image,
    assets: AssetCatalog,
    audio: AudioBank,
    queued_audio: Vec<&'static str>,
    viewport: Viewport,
    dialogue: DialogueManager,
    tts: TtsManager,
    save_store: SaveStore,
    save_enabled: bool,
    scenario: Option<ScenarioRunner>,
    capture: Option<CaptureRequest>,
    smoke_frames: Option<u8>,
    finished_frames: u8,
    stay_open: bool,
}

impl Game {
    pub fn new(ctx: &mut Context, args: &Args) -> GameResult<Self> {
        let assets_root = assets_root();
        let save_store = SaveStore::new(ctx.fs.user_config_dir().join("saves").join("main.json"));
        let tts = TtsManager::new(TtsWorkerConfig::discover(
            args.tts,
            ctx.fs.user_config_dir().join("tts-cache"),
        ));
        let (session, load_message, resumed, save_enabled) = if args.script.is_some() {
            (GameSession::new(42, "Mop"), None, false, false)
        } else if args.new_game {
            (GameSession::new(42, "Mop"), None, false, true)
        } else {
            load_session(&save_store)
        };
        let scenario = args
            .script
            .as_deref()
            .map(|path| {
                let capture_dir = args
                    .capture_dir
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("target/captures/room-shell"));
                ScenarioRunner::load(path, capture_dir)
            })
            .transpose()
            .map_err(|error| GameError::ConfigError(error.to_string()))?;
        let mut view = ViewState::default();
        if let Some(message) = load_message {
            view.show_speech(message, session.world().elapsed_ms);
        }
        ctx.gfx
            .window()
            .set_cursor_hittest(true)
            .map_err(|error| GameError::WindowError(error.to_string()))?;
        let (width, height) = ctx.gfx.drawable_size();
        let mut game = Self {
            session,
            view,
            logical_frame: Image::new_canvas_image(ctx, 320, 180, 1),
            assets: AssetCatalog::load(ctx, &assets_root),
            audio: AudioBank::load(&assets_root),
            queued_audio: Vec::new(),
            viewport: Viewport::for_drawable(width, height),
            dialogue: DialogueManager::new(WorkerConfig::from_environment(
                args.fake_ai,
                args.ai_timeout_ms.map_or_else(
                    WorkerConfig::environment_reply_timeout,
                    Duration::from_millis,
                ),
            )),
            tts,
            save_store,
            save_enabled,
            scenario,
            capture: None,
            smoke_frames: args.smoke.then_some(3),
            finished_frames: 0,
            stay_open: args.stay_open,
        };
        if resumed {
            game.persist()?;
        }
        Ok(game)
    }

    fn render_plan(&self) -> RenderPlan {
        plan(self.session.world(), &self.view).0
    }

    fn apply_command(&mut self, command: SessionCommand, persist: bool) -> GameResult {
        if clears_speech(&command) {
            self.clear_speech();
        }
        let observation = self
            .session
            .apply(CommandEnvelope {
                version: SESSION_PROTOCOL_VERSION,
                command,
            })
            .map_err(session_error)?;
        self.queued_audio
            .extend(observation.events.iter().filter_map(sound_for_event));
        self.view
            .observe_events(&observation.events, self.session.world().elapsed_ms);
        if let Some(request) = observation.dialogue_request
            && self.dialogue.request(request)
        {
            self.view.pending = true;
            self.view.speech = None;
            self.view.mode = UiMode::Idle;
        }
        if persist {
            self.persist()?;
        }
        Ok(())
    }

    fn persist(&mut self) -> GameResult {
        if !self.save_enabled {
            return Ok(());
        }
        let save = self
            .session
            .capture(unix_time_ms())
            .to_json()
            .map_err(session_error)?;
        self.save_store
            .store(&save)
            .map_err(|error| GameError::FilesystemError(error.to_string()))
    }

    fn poll_dialogue(&mut self) {
        if !self.dialogue.is_pending() {
            return;
        }
        match self.dialogue.try_recv() {
            Ok(reply) => {
                self.view.pending = false;
                self.audio.stop_speech();
                let _ = self.tts.request(reply.say.clone(), self.session.world());
                self.view
                    .show_speech(reply.say, self.session.world().elapsed_ms);
                self.view.mode = UiMode::Idle;
                self.view.focused_region = Some("reaction/laugh".to_owned());
            }
            Err(TryRecvError::Disconnected) => {
                self.view.pending = false;
                self.view.show_speech(
                    "too many thought.".to_owned(),
                    self.session.world().elapsed_ms,
                );
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn poll_tts(&mut self) {
        match self.tts.try_recv() {
            Ok(completion) => {
                let _ = completion.request_id;
                if let Some(wav) = completion.wav {
                    self.audio.play_speech(wav);
                }
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
    }

    fn apply_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        if matches!(
            action,
            UiAction::OpenContext(_)
                | UiAction::OpenFoodChoice
                | UiAction::OpenToyChoice
                | UiAction::Feed(_)
                | UiAction::Play(_)
                | UiAction::Tidy
                | UiAction::Comfort
                | UiAction::Talk
        ) {
            self.clear_speech();
        }
        match action {
            UiAction::OpenContext(target) => {
                self.view.mode = UiMode::Context(target);
                self.reset_focus();
            }
            UiAction::CloseContext => {
                self.view.mode = UiMode::Idle;
                self.reset_focus();
            }
            UiAction::OpenFoodChoice => {
                self.view.mode = UiMode::FoodChoice;
                self.reset_focus();
            }
            UiAction::OpenToyChoice => {
                self.view.mode = UiMode::ToyChoice;
                self.reset_focus();
            }
            UiAction::Feed(food) => {
                self.apply_command(SessionCommand::Feed { food }, true)?;
                self.close_menu();
            }
            UiAction::Play(toy) => {
                self.apply_command(play_command(toy), true)?;
                self.close_menu();
            }
            UiAction::Tidy => {
                self.apply_command(SessionCommand::Tidy, true)?;
                self.close_menu();
            }
            UiAction::Comfort => {
                self.apply_command(SessionCommand::Comfort, true)?;
                self.close_menu();
            }
            UiAction::Talk => {
                self.view.mode = if controller {
                    UiMode::OnScreenKeyboard
                } else {
                    UiMode::TextEntry
                };
                self.view.text_buffer.clear();
                self.reset_focus();
            }
            UiAction::React(reaction) => {
                self.apply_command(SessionCommand::React { reaction }, true)?;
                self.clear_speech();
                self.close_menu();
            }
            UiAction::TypeCharacter(character) => {
                append_text(&mut self.view.text_buffer, &character.to_string());
            }
            UiAction::Backspace => {
                self.view.text_buffer.pop();
            }
            UiAction::SubmitText => self.submit_text()?,
            UiAction::CancelText => {
                self.view.text_buffer.clear();
                self.close_menu();
            }
        }
        Ok(())
    }

    fn submit_text(&mut self) -> GameResult {
        let text = self.view.text_buffer.trim().to_owned();
        if text.is_empty() {
            return Ok(());
        }
        self.view.text_buffer.clear();
        self.apply_command(SessionCommand::Talk { text }, true)
    }

    fn apply_confirmed_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        self.apply_ui_action(action, controller)?;
        self.queued_audio.push(UI_CONFIRM);
        Ok(())
    }

    fn close_menu(&mut self) {
        self.view.mode = UiMode::Idle;
        self.view.focused_region = None;
    }

    fn clear_speech(&mut self) {
        self.view.speech = None;
        self.view.speech_expires_at_ms = None;
        self.audio.stop_speech();
    }

    fn reset_focus(&mut self) {
        self.view.focused_region = None;
        let plan = self.render_plan();
        self.view.focused_region = move_focus(&plan, None, 1);
    }

    fn navigate(&mut self, delta: i32) {
        let plan = self.render_plan();
        let previous = self.view.focused_region.clone();
        self.view.focused_region = move_focus(&plan, self.view.focused_region.as_deref(), delta);
        if self.view.focused_region != previous {
            self.queued_audio.push(UI_SELECT);
        }
    }

    fn activate_focus(&mut self, controller: bool) -> GameResult {
        let plan = self.render_plan();
        if let Some(action) = focused_action(&plan, self.view.focused_region.as_deref()) {
            self.apply_confirmed_ui_action(action, controller)?;
        }
        Ok(())
    }

    fn drive_scenario(&mut self, ctx: &mut Context) -> GameResult {
        if self.dialogue.is_pending() || self.capture.is_some() {
            return Ok(());
        }
        let Some(scenario) = &mut self.scenario else {
            return Ok(());
        };
        let Some(step) = scenario.next() else {
            if self.stay_open {
                return Ok(());
            }
            self.finished_frames = self.finished_frames.saturating_add(1);
            if self.finished_frames >= 3 {
                ctx.request_quit();
            }
            return Ok(());
        };
        match step {
            ScenarioStep::Session(envelope) => {
                if clears_speech(&envelope.command) {
                    self.clear_speech();
                }
                let persist = matches!(
                    &envelope.command,
                    SessionCommand::Feed { .. }
                        | SessionCommand::Play { .. }
                        | SessionCommand::Comfort
                        | SessionCommand::Tidy
                        | SessionCommand::Talk { .. }
                        | SessionCommand::React { .. }
                );
                let observation = self.session.apply(envelope).map_err(session_error)?;
                self.queued_audio
                    .extend(observation.events.iter().filter_map(sound_for_event));
                self.view
                    .observe_events(&observation.events, self.session.world().elapsed_ms);
                if let Some(request) = observation.dialogue_request
                    && self.dialogue.request(request)
                {
                    self.view.pending = true;
                    self.view.speech = None;
                }
                if persist {
                    self.persist()?;
                }
            }
            ScenarioStep::Capture(name) => {
                self.capture = Some(CaptureRequest {
                    name,
                    ready_to_read: false,
                });
            }
        }
        Ok(())
    }
}

impl EventHandler for Game {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        let speech_was_visible = self.view.speech.is_some();
        self.poll_dialogue();
        self.poll_tts();
        if self.scenario.is_some() {
            self.drive_scenario(ctx)?;
        } else {
            let dt_ms = ctx
                .time
                .delta()
                .as_millis()
                .min(250)
                .try_into()
                .unwrap_or(250);
            if dt_ms > 0 {
                self.apply_command(
                    SessionCommand::Tick {
                        milliseconds: dt_ms,
                    },
                    false,
                )?;
            }
        }
        if let Some(frames) = &mut self.smoke_frames {
            *frames = frames.saturating_sub(1);
            if *frames == 0 {
                ctx.request_quit();
            }
        }
        self.view.expire(self.session.world().elapsed_ms);
        if speech_was_visible && self.view.speech.is_none() {
            self.audio.stop_speech();
        }
        self.audio.play_queued(&mut self.queued_audio);
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        if let Some(capture) = &self.capture
            && capture.ready_to_read
        {
            let directory = self
                .scenario
                .as_ref()
                .map(|scenario| scenario.capture_dir.as_path())
                .ok_or_else(|| GameError::ConfigError("capture has no scenario".to_owned()))?;
            save_logical_png(
                ctx,
                &self.logical_frame,
                &directory.join(format!("{}.png", capture.name)),
            )?;
            self.capture = None;
        }

        let render = self.render_plan();
        let mut logical =
            Canvas::from_image(ctx, self.logical_frame.clone(), Color::from_rgb(20, 18, 24));
        execute_plan(ctx, &mut logical, &render, &self.assets)?;
        logical.finish(ctx)?;

        let (width, height) = ctx.gfx.drawable_size();
        self.viewport = Viewport::for_drawable(width, height);
        let mut frame = Canvas::from_frame(ctx, Color::from_rgb(12, 11, 15));
        frame.set_sampler(Sampler::nearest_clamp());
        frame.draw(
            &self.logical_frame,
            DrawParam::default()
                .dest([self.viewport.x, self.viewport.y])
                .scale([self.viewport.scale, self.viewport.scale]),
        );
        frame.finish(ctx)?;
        if let Some(capture) = &mut self.capture {
            capture.ready_to_read = true;
        }
        Ok(())
    }

    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut Context,
        button: MouseButton,
        x: f32,
        y: f32,
    ) -> GameResult {
        if button != MouseButton::Left || self.view.pending {
            return Ok(());
        }
        let Some((x, y)) = self.viewport.logical_point(x, y) else {
            return Ok(());
        };
        let render = self.render_plan();
        if let Some(action) = action_at(&render, x, y) {
            self.apply_confirmed_ui_action(action, false)?;
        }
        Ok(())
    }

    fn key_down_event(&mut self, ctx: &mut Context, input: KeyInput, repeated: bool) -> GameResult {
        if self.view.pending {
            return Ok(());
        }
        let key = &input.event.logical_key;
        if matches!(self.view.mode, UiMode::TextEntry) {
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.apply_ui_action(UiAction::CancelText, false)?;
                }
                Key::Named(NamedKey::Enter) => self.submit_text()?,
                Key::Named(NamedKey::Backspace) => {
                    self.view.text_buffer.pop();
                }
                _ if !input.mods.control_key() && !input.mods.super_key() => {
                    if let Some(text) = input.event.text.as_deref() {
                        append_text(&mut self.view.text_buffer, text);
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        if repeated {
            return Ok(());
        }
        match key {
            Key::Named(NamedKey::Escape) => {
                if self.view.mode == UiMode::Idle {
                    self.persist()?;
                    ctx.request_quit();
                } else {
                    self.close_menu();
                }
            }
            Key::Named(NamedKey::Tab | NamedKey::ArrowRight | NamedKey::ArrowDown) => {
                self.navigate(1)
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => self.navigate(-1),
            Key::Named(NamedKey::Enter | NamedKey::Space) => self.activate_focus(false)?,
            Key::Character(character) => {
                if let Some(action) = idle_shortcut(self.view.mode, character) {
                    self.apply_confirmed_ui_action(action, false)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn gamepad_button_down_event(
        &mut self,
        _ctx: &mut Context,
        button: Button,
        _id: GamepadId,
    ) -> GameResult {
        if self.view.pending {
            return Ok(());
        }
        match button {
            Button::DPadRight | Button::DPadDown => self.navigate(1),
            Button::DPadLeft | Button::DPadUp => self.navigate(-1),
            Button::South => self.activate_focus(true)?,
            Button::East => self.close_menu(),
            _ => {}
        }
        Ok(())
    }

    fn focus_event(&mut self, _ctx: &mut Context, gained: bool) -> GameResult {
        if !gained {
            self.persist()?;
        }
        Ok(())
    }

    fn quit_event(&mut self, _ctx: &mut Context) -> GameResult<bool> {
        self.persist()?;
        Ok(false)
    }

    fn resize_event(&mut self, _ctx: &mut Context, width: f32, height: f32) -> GameResult {
        self.viewport = Viewport::for_drawable(width, height);
        Ok(())
    }
}

fn idle_shortcut(mode: UiMode, character: &str) -> Option<UiAction> {
    if mode != UiMode::Idle {
        return None;
    }
    if character.eq_ignore_ascii_case("f") {
        Some(UiAction::OpenFoodChoice)
    } else if character.eq_ignore_ascii_case("p") {
        Some(UiAction::OpenToyChoice)
    } else if character.eq_ignore_ascii_case("c") {
        Some(UiAction::Comfort)
    } else if character.eq_ignore_ascii_case("t") {
        Some(UiAction::Talk)
    } else {
        None
    }
}

fn load_session(store: &SaveStore) -> (GameSession, Option<String>, bool, bool) {
    let source = match store.load() {
        Ok(Some(source)) => source,
        Ok(None) => return (GameSession::new(42, "Mop"), None, false, true),
        Err(_) => {
            return (
                GameSession::new(42, "Mop"),
                Some("save box would not open.".to_owned()),
                false,
                false,
            );
        }
    };
    match GameSession::resume_json(&source, unix_time_ms()) {
        Ok((session, _)) => (session, Some("you came back.".to_owned()), true, true),
        Err(_) => (
            GameSession::new(42, "Mop"),
            Some("old save smelled wrong. left it alone.".to_owned()),
            false,
            false,
        ),
    }
}

fn assets_root() -> PathBuf {
    if let Some(path) = std::env::var_os("BEASTIE_ASSETS") {
        return PathBuf::from(path);
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        let packaged = directory.join("assets");
        if packaged.is_dir() {
            return packaged;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

fn session_error(error: SessionError) -> GameError {
    GameError::CustomError(error.to_string())
}

fn play_command(toy: ToyId) -> SessionCommand {
    SessionCommand::Play { toy }
}

fn clears_speech(command: &SessionCommand) -> bool {
    matches!(
        command,
        SessionCommand::Feed { .. }
            | SessionCommand::Play { .. }
            | SessionCommand::Comfort
            | SessionCommand::Tidy
            | SessionCommand::Talk { .. }
    )
}

#[cfg(test)]
mod tests {
    use beastie_core::{Reaction, ToyId};
    use beastie_session::SessionCommand;
    use beastie_view::{UiAction, UiMode};

    use super::{clears_speech, idle_shortcut, play_command};

    #[test]
    fn toy_ui_actions_map_to_typed_play_commands() {
        assert_eq!(
            play_command(ToyId::Ball),
            SessionCommand::Play { toy: ToyId::Ball }
        );
        assert_eq!(
            play_command(ToyId::Bell),
            SessionCommand::Play { toy: ToyId::Bell }
        );
        assert_eq!(
            play_command(ToyId::Sock),
            SessionCommand::Play { toy: ToyId::Sock }
        );
    }

    #[test]
    fn primary_ui_actions_remain_distinct() {
        let mappings = [
            UiAction::Play(ToyId::Ball),
            UiAction::Play(ToyId::Bell),
            UiAction::Play(ToyId::Sock),
            UiAction::Tidy,
            UiAction::Comfort,
            UiAction::Talk,
            UiAction::React(Reaction::Laugh),
        ];
        assert_eq!(mappings.len(), 7);
    }

    #[test]
    fn unrelated_player_commands_clear_visible_speech_but_ticks_do_not() {
        assert!(clears_speech(&SessionCommand::Feed {
            food: beastie_core::FoodId::Berry,
        }));
        assert!(clears_speech(&SessionCommand::Play { toy: ToyId::Ball }));
        assert!(clears_speech(&SessionCommand::Comfort));
        assert!(clears_speech(&SessionCommand::Tidy));
        assert!(clears_speech(&SessionCommand::Talk {
            text: "hello".to_owned(),
        }));
        assert!(!clears_speech(&SessionCommand::Tick {
            milliseconds: 1_000,
        }));
        assert!(!clears_speech(&SessionCommand::React {
            reaction: Reaction::Laugh,
        }));
    }

    #[test]
    fn direct_shortcuts_do_not_bypass_open_modals() {
        assert_eq!(
            idle_shortcut(UiMode::Idle, "F"),
            Some(UiAction::OpenFoodChoice)
        );
        for mode in [
            UiMode::Context(beastie_view::UiTarget::Bowl),
            UiMode::FoodChoice,
            UiMode::ToyChoice,
            UiMode::OnScreenKeyboard,
        ] {
            assert_eq!(idle_shortcut(mode, "f"), None);
            assert_eq!(idle_shortcut(mode, "t"), None);
        }
    }
}
