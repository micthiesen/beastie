use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beastie_core::{GameEvent, NamingTarget, NormalizedPosition, ToyId};
use beastie_protocol::{
    MouthTiming, RecognitionOutcome, SpeechInputFailure, TranscriptRecord,
    identity_tts_voice_settings,
};
use beastie_session::{
    CommandEnvelope, GameSession, SESSION_PROTOCOL_VERSION, SessionCommand, SessionError,
    SpokenInputStatus,
};
use beastie_view::{
    BindableAction, BindingLabels, CursorKind, MicrophoneState, RenderPlan, SPEECH_RELEASE_MS,
    UiAction, UiMode, ViewState, audio_plan_for_events, logical_to_world, plan,
};
use ggez::conf::{FullscreenType, WindowMode};
use ggez::event::{Button, EventHandler, GamepadId};
use ggez::graphics::{Canvas, Color, DrawParam, Image, Sampler};
use ggez::input::keyboard::KeyInput;
use ggez::input::mouse::MouseButton;
use ggez::winit::keyboard::{Key, NamedKey};
use ggez::winit::window::CursorIcon;
use ggez::{Context, GameError, GameResult};

use crate::args::Args;
use crate::audio::{AmbientBubbleSchedule, AudioBank, UI_CONFIRM, UI_SELECT, sound_for_cue};
use crate::dialogue::{DialogueManager, WorkerConfig};
use crate::feel::FeelRecorder;
use crate::input::{
    action_at_with_assets, append_text, cursor_at_with_assets, focused_action, move_focus,
    region_at_with_assets,
};
use crate::microphone::{MicrophoneCapture, MicrophoneError, PrivateAudioRoot};
use crate::recognition::{RecognitionManager, RecognitionWorkerConfig, speech_failure};
use crate::renderer::{
    AssetCatalog, Viewport, execute_plan, presentation_rgba, save_presentation_png,
};
use crate::save_store::{LoadedSave, SaveStore};
use crate::scenario::{ScenarioRunner, ScenarioStep};
use crate::settings::{BindingKey, KeyBindings, SettingsStore, TextScale, TextSpeed, UserSettings};
use crate::transcript::TranscriptStore;
use crate::tts::{TtsManager, TtsWorkerConfig};

struct SpeechReveal {
    full_text: String,
    started_at_ms: u64,
}

struct SpeechAnimation {
    started_at_ms: u64,
    timing: MouthTiming,
}

pub struct Game {
    session: GameSession,
    view: ViewState,
    presentation_frame: Image,
    assets: AssetCatalog,
    audio: AudioBank,
    queued_audio: Vec<&'static str>,
    viewport: Viewport,
    dialogue: DialogueManager,
    recognition: RecognitionManager,
    microphone: Option<MicrophoneCapture>,
    stt_audio_root: PrivateAudioRoot,
    recognition_sequence: u64,
    active_recognition: Option<u64>,
    spoken_turn_pending: bool,
    tts: TtsManager,
    save_store: SaveStore,
    settings_store: SettingsStore,
    settings: UserSettings,
    transcripts: TranscriptStore,
    save_enabled: bool,
    scenario: Option<ScenarioRunner>,
    feel: Option<FeelRecorder>,
    capture: Option<CaptureState>,
    smoke_frames: Option<u8>,
    finished_frames: u8,
    stay_open: bool,
    pointer_logical: Option<(i32, i32)>,
    cursor_world: Option<NormalizedPosition>,
    window_settings_dirty: bool,
    speech_reveal: Option<SpeechReveal>,
    bubble_schedule: AmbientBubbleSchedule,
    pending_mouth_timing: Option<MouthTiming>,
    speech_animation: Option<SpeechAnimation>,
    transcript_export_path: PathBuf,
    renaming_with_osk: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureState {
    RenderPending(String),
    ReadbackReady(String),
}

impl Game {
    pub fn new(ctx: &mut Context, args: &Args) -> GameResult<Self> {
        let assets_root = assets_root();
        let save_store = SaveStore::new(ctx.fs.user_config_dir().join("saves").join("main.json"));
        let settings_store = SettingsStore::new(ctx.fs.user_config_dir().join("settings.json"));
        let (settings, settings_message) = if args.feel_dir.is_some() {
            // Feel evidence must not inherit the operator's accessibility, window, audio, or
            // microphone preferences. Comparisons need one explicit, repeatable presentation.
            (UserSettings::default(), None)
        } else {
            match settings_store.load() {
                Ok(settings) => (settings, None),
                Err(_) => (
                    UserSettings::default(),
                    Some("settings were unreadable. using safe defaults.".to_owned()),
                ),
            }
        };
        if args.new_game {
            save_store
                .reset()
                .map_err(|error| GameError::FilesystemError(error.to_string()))?;
        }
        let tts = TtsManager::new(TtsWorkerConfig::discover(
            args.tts && settings.voice_enabled,
            ctx.fs.user_config_dir().join("tts-cache"),
        ));
        let stt_audio_root = PrivateAudioRoot::prepare(ctx.fs.user_config_dir().join("stt-input"))
            .map_err(|error| GameError::FilesystemError(error.to_string()))?;
        let recognition = RecognitionManager::new(RecognitionWorkerConfig::discover(
            args.fake_ai,
            stt_audio_root.path(),
            args.stt_worker.as_deref(),
            &args.stt_backend,
            args.stt_model_dir.as_deref(),
            args.moonshine_engine.as_deref(),
            args.stt_timeout_ms,
        ));
        let (session, load_message, _resumed, save_enabled) = if args.script.is_some() {
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
                    .unwrap_or_else(|| PathBuf::from("target/captures/aquarium-shell"));
                ScenarioRunner::load(path, capture_dir)
            })
            .transpose()
            .map_err(|error| GameError::ConfigError(error.to_string()))?;
        let feel = args
            .feel_dir
            .clone()
            .map(FeelRecorder::create)
            .transpose()
            .map_err(feel_error)?;
        let mut view = ViewState {
            pixel_grid: settings.pixel_grid,
            text_scale: u8::from(matches!(settings.text_scale, TextScale::Large)) + 1,
            reduced_motion: settings.reduced_motion,
            reduced_flashes: settings.reduced_flashes,
            reduced_shake: settings.reduced_shake,
            window_scale: settings.window_scale,
            fullscreen: settings.fullscreen,
            effects_volume: settings.effects_volume,
            speech_volume: settings.speech_volume,
            voice_enabled: settings.voice_enabled,
            subtitles: settings.subtitles,
            text_speed: text_speed_value(settings.text_speed),
            binding_labels: binding_labels(&settings.bindings),
            transcript_enabled: args.transcript || settings.transcript_enabled,
            microphone_enabled: settings.microphone_enabled,
            microphone_state: if settings.microphone_enabled {
                MicrophoneState::Idle
            } else {
                MicrophoneState::Disabled
            },
            ..ViewState::default()
        };
        if let Some(message) = settings_message.or(load_message) {
            view.show_speech(message, session.world().elapsed_ms);
        }
        ctx.gfx
            .window()
            .set_cursor_hittest(true)
            .map_err(|error| GameError::WindowError(error.to_string()))?;
        let (width, height) = ctx.gfx.drawable_size();
        let mut audio = AudioBank::load(&assets_root);
        audio.set_gains(settings.effects_gain(), settings.speech_gain());
        let transcripts = TranscriptStore::new(
            ctx.fs.user_config_dir().join("transcripts/playtest.jsonl"),
            args.transcript || settings.transcript_enabled,
        );
        let transcript_export_path = ctx
            .fs
            .user_config_dir()
            .join("transcripts/beastie-playtest-export.jsonl");
        let bubble_schedule =
            AmbientBubbleSchedule::new(session.world().seed, session.world().elapsed_ms);
        let game = Self {
            session,
            view,
            presentation_frame: Image::new_canvas_image(ctx, 640, 360, 1),
            assets: AssetCatalog::load(ctx, &assets_root),
            audio,
            queued_audio: Vec::new(),
            viewport: Viewport::for_drawable(width, height),
            dialogue: DialogueManager::new(WorkerConfig::from_environment(
                args.fake_ai,
                args.ai_timeout_ms.map_or_else(
                    WorkerConfig::environment_reply_timeout,
                    Duration::from_millis,
                ),
            )),
            recognition,
            microphone: None,
            stt_audio_root,
            recognition_sequence: 0,
            active_recognition: None,
            spoken_turn_pending: false,
            tts,
            save_store,
            settings_store,
            settings,
            transcripts,
            save_enabled,
            scenario,
            feel,
            capture: None,
            smoke_frames: args.smoke.then_some(3),
            finished_frames: 0,
            stay_open: args.stay_open,
            pointer_logical: None,
            cursor_world: None,
            window_settings_dirty: false,
            speech_reveal: None,
            bubble_schedule,
            pending_mouth_timing: None,
            speech_animation: None,
            transcript_export_path,
            renaming_with_osk: false,
        };
        game.apply_window_settings(ctx)?;
        if let Some(destination) = &args.export_transcript {
            game.transcripts
                .export(destination)
                .map_err(|error| GameError::FilesystemError(error.to_string()))?;
        }
        Ok(game)
    }

    fn render_plan(&self) -> RenderPlan {
        plan(self.session.world(), &self.view).0
    }

    fn apply_window_settings(&self, ctx: &mut Context) -> GameResult {
        let mode = if self.settings.fullscreen {
            WindowMode::default()
                .fullscreen_type(FullscreenType::Desktop)
                .resizable(false)
        } else {
            let (width, height) = Viewport::window_dimensions(self.settings.window_scale);
            WindowMode::default()
                .dimensions(width, height)
                .resizable(false)
        };
        ctx.gfx.set_mode(mode)
    }

    fn persist_settings(&self) -> GameResult {
        self.settings_store
            .store(&self.settings)
            .map_err(|error| GameError::FilesystemError(error.to_string()))
    }

    fn toggle_fullscreen(&mut self, ctx: &mut Context) -> GameResult {
        self.settings.fullscreen = !self.settings.fullscreen;
        self.view.fullscreen = self.settings.fullscreen;
        self.apply_window_settings(ctx)?;
        self.persist_settings()
    }

    fn change_window_scale(&mut self, ctx: &mut Context, delta: i8) -> GameResult {
        if self.settings.fullscreen {
            return Ok(());
        }
        self.settings.window_scale = i16::from(self.settings.window_scale)
            .saturating_add(i16::from(delta))
            .clamp(1, 6) as u8;
        self.view.window_scale = self.settings.window_scale;
        self.apply_window_settings(ctx)?;
        self.persist_settings()
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
        self.queued_audio.extend(
            audio_plan_for_events(&observation.events)
                .events
                .into_iter()
                .filter_map(sound_for_cue),
        );
        self.view
            .observe_events(&observation.events, self.session.world().elapsed_ms);
        if let Some(feel) = &mut self.feel {
            feel.record_observation(&observation, self.session.world().elapsed_ms)
                .map_err(feel_error)?;
        }
        let now_ms = self.session.world().elapsed_ms;
        if let Some(status) = observation.spoken_input {
            let resting_microphone_state = self.resting_microphone_state();
            self.spoken_turn_pending =
                apply_spoken_input_status(&mut self.view, status, now_ms, resting_microphone_state);
        } else if observation.events.contains(&GameEvent::TalkIgnored) {
            self.view
                .show_status("Not interested right now.".to_owned(), now_ms, 4_000);
        }
        if let Some(request) = observation.dialogue_request
            && self.dialogue.request(request)
        {
            self.view.pending = true;
            self.view.speech = None;
            self.view.mode = UiMode::Compose;
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

    fn begin_push_to_talk(&mut self) -> GameResult {
        self.view.transcript_status = None;
        if self.microphone.is_some() {
            return Ok(());
        }
        if !self.settings.microphone_enabled {
            self.view.status_message =
                Some("Microphone is off. Enable it in Settings (F5)".to_owned());
            return Ok(());
        }
        if self.spoken_turn_pending {
            self.view.status_message =
                Some("Already heard you. Waiting for a good moment...".to_owned());
            return Ok(());
        }
        if self.recognition.is_pending() {
            self.view.status_message = Some("Still recognizing what you said...".to_owned());
            return Ok(());
        }
        if self.dialogue.is_pending() {
            self.view.status_message = Some("Finish this thought before speaking again".to_owned());
            return Ok(());
        }
        self.apply_command(SessionCommand::SpeechStarted, false)?;
        match MicrophoneCapture::start() {
            Ok(capture) => {
                self.microphone = Some(capture);
                self.view.microphone_state = MicrophoneState::Listening;
                self.view.status_message = None;
            }
            Err(MicrophoneError::Unavailable | MicrophoneError::UnsupportedFormat) => {
                self.apply_command(
                    SessionCommand::SpeechFailed {
                        failure: SpeechInputFailure::MicrophoneUnavailable,
                    },
                    false,
                )?;
                self.view.microphone_state = MicrophoneState::Unavailable;
                self.view.status_message =
                    Some("Microphone unavailable. Text still works".to_owned());
            }
            Err(MicrophoneError::Start(_) | MicrophoneError::Storage(_)) => {
                self.apply_command(
                    SessionCommand::SpeechFailed {
                        failure: SpeechInputFailure::MicrophoneUnavailable,
                    },
                    false,
                )?;
                self.view.microphone_state = MicrophoneState::Error;
                self.view.status_message =
                    Some("Microphone could not start. Text still works".to_owned());
            }
        }
        Ok(())
    }

    fn end_push_to_talk(&mut self) -> GameResult {
        let Some(capture) = self.microphone.take() else {
            return Ok(());
        };
        match capture.finish(self.stt_audio_root.path()) {
            Ok(audio) => {
                self.recognition_sequence = self.recognition_sequence.wrapping_add(1).max(1);
                let request_id = self.recognition_sequence;
                if self.recognition.request(request_id, audio) {
                    self.active_recognition = Some(request_id);
                    self.view.microphone_state = MicrophoneState::Recognizing;
                } else {
                    self.apply_command(
                        SessionCommand::SpeechFailed {
                            failure: SpeechInputFailure::RecognizerUnavailable,
                        },
                        false,
                    )?;
                    self.view.microphone_state = MicrophoneState::Unavailable;
                    self.view.status_message =
                        Some("Speech recognition unavailable. Text still works".to_owned());
                }
            }
            Err(
                MicrophoneError::Unavailable
                | MicrophoneError::UnsupportedFormat
                | MicrophoneError::Start(_),
            ) => {
                self.apply_command(
                    SessionCommand::SpeechFailed {
                        failure: SpeechInputFailure::MicrophoneUnavailable,
                    },
                    false,
                )?;
                self.view.microphone_state = MicrophoneState::Unavailable;
                self.view.status_message =
                    Some("Microphone disconnected. Text still works".to_owned());
            }
            Err(MicrophoneError::Storage(_)) => {
                self.apply_command(
                    SessionCommand::SpeechFailed {
                        failure: SpeechInputFailure::RecognitionFailed,
                    },
                    false,
                )?;
                self.view.microphone_state = MicrophoneState::Error;
                self.view.status_message = Some("Recording could not be prepared".to_owned());
            }
        }
        Ok(())
    }

    fn poll_recognition(&mut self) -> GameResult {
        match self.recognition.try_recv() {
            Ok(completion) => {
                if self.active_recognition != Some(completion.request_id) {
                    return Ok(());
                }
                self.active_recognition = None;
                match completion.outcome {
                    RecognitionOutcome::Recognized { text, confidence } => {
                        self.apply_command(
                            SessionCommand::SpeechCandidate { text, confidence },
                            false,
                        )?;
                        self.apply_command(SessionCommand::SpeechEnded, true)?;
                        if !self.spoken_turn_pending {
                            self.view.status_message = None;
                            self.view.microphone_state = self.resting_microphone_state();
                        }
                    }
                    RecognitionOutcome::NoSpeech {} => {
                        self.apply_command(SessionCommand::SpeechEnded, false)?;
                        self.view.status_message =
                            Some("No speech heard. Hold F1 and try again".to_owned());
                        self.view.microphone_state = self.resting_microphone_state();
                    }
                    RecognitionOutcome::Error { code } => {
                        let unavailable = matches!(
                            code,
                            beastie_protocol::RecognitionErrorCode::BackendUnavailable
                        );
                        self.apply_command(
                            SessionCommand::SpeechFailed {
                                failure: speech_failure(code),
                            },
                            false,
                        )?;
                        self.view.microphone_state = if unavailable {
                            MicrophoneState::Unavailable
                        } else {
                            MicrophoneState::Error
                        };
                        self.view.status_message = Some(if unavailable {
                            "Speech recognition unavailable. Text still works".to_owned()
                        } else {
                            "Speech recognition failed. Hold F1 to try again".to_owned()
                        });
                    }
                }
            }
            Err(TryRecvError::Disconnected) if self.active_recognition.take().is_some() => {
                self.apply_command(
                    SessionCommand::SpeechFailed {
                        failure: SpeechInputFailure::RecognizerUnavailable,
                    },
                    false,
                )?;
                self.view.microphone_state = MicrophoneState::Unavailable;
                self.view.status_message =
                    Some("Speech recognition stopped. Text still works".to_owned());
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        Ok(())
    }

    const fn resting_microphone_state(&self) -> MicrophoneState {
        if self.settings.microphone_enabled {
            MicrophoneState::Idle
        } else {
            MicrophoneState::Disabled
        }
    }

    fn poll_dialogue(&mut self) -> GameResult {
        if !self.dialogue.is_pending() {
            return Ok(());
        }
        match self.dialogue.try_recv_turn() {
            Ok(turn) => {
                self.view.pending = false;
                self.audio.stop_speech();
                if !self.session.accept_dialogue_turn(
                    &turn.request,
                    &turn.reply,
                    turn.retry_count,
                    turn.fallback,
                ) {
                    // The player superseded the authoritative relationship beat while local
                    // inference was pending. Never present, speak, or transcript stale wording.
                    self.pending_mouth_timing = None;
                    self.view.mode = UiMode::Compose;
                    return Ok(());
                }
                self.persist()?;
                let voice = identity_tts_voice_settings(self.session.world());
                let text_speed = effective_dialogue_text_speed(
                    self.settings.text_speed,
                    self.scenario.is_some(),
                );
                if self.settings.voice_enabled {
                    self.pending_mouth_timing = Some(voice.mouth_timing);
                    let _ = self
                        .tts
                        .request(turn.reply.say.clone(), self.session.world());
                }
                self.speech_reveal = show_dialogue_caption(
                    &mut self.view,
                    &turn.reply.say,
                    self.session.world().elapsed_ms,
                    self.settings.subtitles,
                    text_speed,
                );
                if turn.fallback {
                    self.view.show_status(
                        "Local thoughts unavailable. Using a simple response.".to_owned(),
                        self.session.world().elapsed_ms,
                        5_000,
                    );
                }
                let mut transcript = TranscriptRecord::from_turn(
                    self.session.world().elapsed_ms,
                    &turn.request,
                    Some(&turn.reply),
                    turn.backend,
                    turn.latency_ms,
                    turn.fallback,
                    false,
                    Some(voice),
                );
                transcript.apply_dialogue_metadata(
                    turn.retry_count,
                    turn.duplicate_suppressed,
                    turn.fallback_reason,
                );
                self.transcripts
                    .append(&transcript)
                    .map_err(|error| GameError::FilesystemError(error.to_string()))?;
                self.view.mode = UiMode::Compose;
                self.view.focused_region = Some("reaction/laugh".to_owned());
            }
            Err(TryRecvError::Disconnected) => {
                self.view.pending = false;
                let text_speed = effective_dialogue_text_speed(
                    self.settings.text_speed,
                    self.scenario.is_some(),
                );
                self.speech_reveal = show_dialogue_caption(
                    &mut self.view,
                    "too many thought.",
                    self.session.world().elapsed_ms,
                    self.settings.subtitles,
                    text_speed,
                );
            }
            Err(TryRecvError::Empty) => {}
        }
        Ok(())
    }

    fn poll_tts(&mut self) -> GameResult {
        match self.tts.try_recv() {
            Ok(completion) => {
                let _ = completion.request_id;
                if let Some(wav) = completion.wav {
                    if let Some(feel) = &mut self.feel {
                        feel.record_speech(&wav, self.session.world().elapsed_ms)
                            .map_err(feel_error)?;
                    }
                    self.audio.play_speech(wav);
                    if let Some(timing) = self.pending_mouth_timing.take() {
                        self.speech_animation = Some(SpeechAnimation {
                            started_at_ms: self.session.world().elapsed_ms,
                            timing,
                        });
                    }
                }
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        Ok(())
    }

    fn apply_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        if matches!(
            action,
            UiAction::OpenContext(_)
                | UiAction::OpenFoodChoice
                | UiAction::OpenToyChoice
                | UiAction::SelectFood(_)
                | UiAction::DropFood(_)
                | UiAction::Play(_)
                | UiAction::Comfort
                | UiAction::Inspect
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
                self.view.mode = UiMode::Compose;
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
            UiAction::SelectFood(food) => {
                self.view.mode = UiMode::FoodDrop(food);
                self.view.focused_region = None;
            }
            UiAction::DropFood(food) => {
                let position = self
                    .cursor_world
                    .unwrap_or_else(|| NormalizedPosition::new(5_000, 2_500));
                self.apply_command(SessionCommand::DropFood { food, position }, true)?;
                self.close_menu();
            }
            UiAction::Play(toy) => {
                self.apply_command(play_command(toy), true)?;
                self.close_menu();
            }
            UiAction::Inspect => {
                self.apply_command(SessionCommand::Inspect, false)?;
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
                    UiMode::Compose
                };
                self.reset_focus();
            }
            UiAction::FocusCompose => {
                if !matches!(self.view.mode, UiMode::Rename) {
                    self.view.mode = UiMode::Compose;
                }
                self.view.focused_region = Some("compose/input".to_owned());
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
            UiAction::SubmitText => {
                if self.renaming_with_osk {
                    self.submit_name()?;
                } else {
                    self.submit_text()?;
                }
            }
            UiAction::ClearText => {
                self.view.text_buffer.clear();
            }
            UiAction::CancelMode => {
                self.close_menu();
            }
            UiAction::OpenSettings => {
                self.view.mode = UiMode::Settings;
                self.reset_focus();
            }
            UiAction::SetTextScale(scale) => {
                self.view.text_scale = scale.clamp(1, 2);
                self.settings.text_scale = if self.view.text_scale >= 2 {
                    TextScale::Large
                } else {
                    TextScale::Medium
                };
                self.persist_settings()?;
            }
            UiAction::ToggleReducedMotion => {
                self.settings.reduced_motion = !self.settings.reduced_motion;
                self.view.reduced_motion = self.settings.reduced_motion;
                self.persist_settings()?;
            }
            UiAction::ToggleReducedFlashes => {
                self.settings.reduced_flashes = !self.settings.reduced_flashes;
                self.view.reduced_flashes = self.settings.reduced_flashes;
                self.persist_settings()?;
            }
            UiAction::ToggleReducedShake => {
                self.settings.reduced_shake = !self.settings.reduced_shake;
                self.view.reduced_shake = self.settings.reduced_shake;
                self.persist_settings()?;
            }
            UiAction::TogglePixelGrid => {
                self.settings.pixel_grid = !self.settings.pixel_grid;
                self.view.pixel_grid = self.settings.pixel_grid;
                self.persist_settings()?;
            }
            UiAction::CycleWindowScale => {
                self.settings.window_scale = self.settings.window_scale % 6 + 1;
                self.view.window_scale = self.settings.window_scale;
                self.window_settings_dirty = true;
                self.persist_settings()?;
            }
            UiAction::ToggleFullscreen => {
                self.settings.fullscreen = !self.settings.fullscreen;
                self.view.fullscreen = self.settings.fullscreen;
                self.window_settings_dirty = true;
                self.persist_settings()?;
            }
            UiAction::CycleEffectsVolume => {
                self.settings.effects_volume = cycle_volume(self.settings.effects_volume);
                self.view.effects_volume = self.settings.effects_volume;
                self.audio
                    .set_gains(self.settings.effects_gain(), self.settings.speech_gain());
                self.persist_settings()?;
            }
            UiAction::CycleSpeechVolume => {
                self.settings.speech_volume = cycle_volume(self.settings.speech_volume);
                self.view.speech_volume = self.settings.speech_volume;
                self.audio
                    .set_gains(self.settings.effects_gain(), self.settings.speech_gain());
                self.persist_settings()?;
            }
            UiAction::ToggleVoice => {
                self.settings.voice_enabled = !self.settings.voice_enabled;
                self.view.voice_enabled = self.settings.voice_enabled;
                if !self.settings.voice_enabled {
                    self.audio.stop_speech();
                }
                self.persist_settings()?;
            }
            UiAction::ToggleMicrophone => {
                if self.microphone.is_some() {
                    self.end_push_to_talk()?;
                }
                self.settings.microphone_enabled = !self.settings.microphone_enabled;
                self.view.microphone_enabled = self.settings.microphone_enabled;
                self.view.microphone_state = if self.settings.microphone_enabled {
                    MicrophoneState::Idle
                } else if self.recognition.is_pending() {
                    MicrophoneState::Recognizing
                } else {
                    MicrophoneState::Disabled
                };
                self.view.status_message = Some(if self.settings.microphone_enabled {
                    "Microphone enabled. Hold F1 or the mic button to speak".to_owned()
                } else {
                    "Microphone disabled".to_owned()
                });
                self.persist_settings()?;
            }
            UiAction::PushToTalk => self.begin_push_to_talk()?,
            UiAction::ToggleSubtitles => {
                self.settings.subtitles = !self.settings.subtitles;
                self.view.subtitles = self.settings.subtitles;
                if !self.settings.subtitles {
                    self.view.clear_speech();
                    self.speech_reveal = None;
                }
                self.persist_settings()?;
            }
            UiAction::CycleTextSpeed => {
                self.settings.text_speed = cycle_text_speed(self.settings.text_speed);
                self.view.text_speed = text_speed_value(self.settings.text_speed);
                self.persist_settings()?;
            }
            UiAction::OpenBindings => {
                self.view.mode = UiMode::Bindings;
                self.reset_focus();
            }
            UiAction::BeginRebind(action) => {
                self.view.mode = UiMode::Rebinding(action);
                self.view.focused_region = None;
            }
            UiAction::ResetBindings => {
                self.settings.bindings = KeyBindings::default();
                self.view.binding_labels = binding_labels(&self.settings.bindings);
                self.persist_settings()?;
            }
            UiAction::Rename => {
                self.view.text_buffer.clear();
                self.renaming_with_osk = controller;
                self.view.mode = if controller {
                    UiMode::OnScreenKeyboard
                } else {
                    UiMode::Rename
                };
                self.reset_focus();
            }
            UiAction::SubmitName => self.submit_name()?,
            UiAction::OpenDataManagement => {
                self.view.mode = UiMode::DataManagement;
                self.reset_focus();
            }
            UiAction::RecoverBackup => self.recover_backup(),
            UiAction::RequestReset => {
                self.view.mode = UiMode::ConfirmReset;
                self.reset_focus();
            }
            UiAction::ConfirmReset => self.reset_save(),
            UiAction::ToggleTranscript => {
                self.settings.transcript_enabled = !self.view.transcript_enabled;
                self.view.transcript_enabled = self.settings.transcript_enabled;
                self.transcripts
                    .set_enabled(self.settings.transcript_enabled);
                self.view.transcript_status = Some(if self.settings.transcript_enabled {
                    "Recording safe local turns".to_owned()
                } else {
                    "Transcript recording off".to_owned()
                });
                self.persist_settings()?;
            }
            UiAction::ExportTranscript => {
                match self.transcripts.export(&self.transcript_export_path) {
                    Ok(turns) => {
                        self.view.transcript_status = Some(format!(
                            "Exported {turns} turns to {}",
                            self.transcript_export_path.display()
                        ));
                    }
                    Err(error) => {
                        self.view.transcript_status = Some(format!("Export failed: {error}"));
                    }
                }
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

    fn submit_name(&mut self) -> GameResult {
        let name = self.view.text_buffer.trim().to_owned();
        if name.is_empty() || name.chars().count() > 24 || name.chars().any(char::is_control) {
            self.view.status_message = Some("Name must be 1 to 24 visible characters".to_owned());
            return Ok(());
        }
        match self.apply_command(
            SessionCommand::Name {
                target: NamingTarget::Creature,
                name: name.clone(),
            },
            true,
        ) {
            Ok(()) => {
                self.view.text_buffer.clear();
                self.view.status_message = Some(format!("Name set to {name}"));
                self.close_menu();
            }
            Err(error) => {
                self.view.status_message = Some(format!("Couldn't use that name: {error}"));
            }
        }
        Ok(())
    }

    fn recover_backup(&mut self) {
        let result = self
            .save_store
            .load_backup()
            .and_then(|source| {
                source.ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "no backup exists")
                })
            })
            .and_then(|source| {
                GameSession::resume_json(&source, unix_time_ms())
                    .map(|(session, _)| session)
                    .map_err(std::io::Error::other)
            })
            .and_then(|session| {
                self.save_store.promote_backup().and_then(|promoted| {
                    promoted
                        .then_some(session)
                        .ok_or_else(|| std::io::Error::other("backup disappeared"))
                })
            });
        match result {
            Ok(session) => {
                self.session = session;
                self.save_enabled = true;
                self.view.status_message = Some("Recovered the last-good save".to_owned());
            }
            Err(error) => {
                self.view.status_message = Some(format!("Recovery failed: {error}"));
            }
        }
        self.view.mode = UiMode::DataManagement;
        self.reset_focus();
    }

    fn reset_save(&mut self) {
        match self.save_store.reset() {
            Ok(_) => {
                self.session = GameSession::new(42, "Mop");
                self.save_enabled = true;
                self.view.status_message =
                    Some("Save reset. Previous creature kept in the reset backup".to_owned());
                self.view.text_buffer.clear();
            }
            Err(error) => {
                self.view.status_message = Some(format!("Reset failed: {error}"));
            }
        }
        self.view.mode = UiMode::DataManagement;
        self.reset_focus();
    }

    fn apply_confirmed_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        self.apply_ui_action(action, controller)?;
        self.queued_audio.push(UI_CONFIRM);
        Ok(())
    }

    fn close_menu(&mut self) {
        self.renaming_with_osk = false;
        self.view.mode = UiMode::Compose;
        self.view.focused_region = Some("compose/input".to_owned());
    }

    fn clear_speech(&mut self) {
        self.view.clear_speech();
        self.audio.stop_speech();
        self.pending_mouth_timing = None;
        self.speech_animation = None;
        self.speech_reveal = None;
    }

    fn update_speech_reveal(&mut self) {
        let Some(reveal) = &self.speech_reveal else {
            return;
        };
        let elapsed = self
            .session
            .world()
            .elapsed_ms
            .saturating_sub(reveal.started_at_ms);
        let (text, complete) = revealed_text(&reveal.full_text, elapsed, self.settings.text_speed);
        self.view.speech = Some(text);
        if complete {
            self.speech_reveal = None;
        }
    }

    fn activate_binding(&mut self, action: BindableAction) -> GameResult {
        match action {
            BindableAction::PushToTalk => self.begin_push_to_talk(),
            BindableAction::Food => self.apply_confirmed_ui_action(UiAction::OpenFoodChoice, false),
            BindableAction::Play => self.apply_confirmed_ui_action(UiAction::OpenToyChoice, false),
            BindableAction::Comfort => self.apply_confirmed_ui_action(UiAction::Comfort, false),
            BindableAction::Settings => {
                self.apply_confirmed_ui_action(UiAction::OpenSettings, false)
            }
            BindableAction::Cancel => {
                self.close_menu();
                Ok(())
            }
        }
    }

    fn rebind(&mut self, action: BindableAction, key: BindingKey) -> GameResult {
        self.settings.bindings.rebind_swapping(action, key);
        self.view.binding_labels = binding_labels(&self.settings.bindings);
        self.view.mode = UiMode::Bindings;
        self.reset_focus();
        self.persist_settings()
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
            if self.capture.is_some() {
                ctx.gfx.window().request_redraw();
            }
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
            ctx.gfx.window().request_redraw();
            return Ok(());
        };
        match step {
            ScenarioStep::Session(envelope) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_command(&envelope, self.session.world().elapsed_ms)
                        .map_err(feel_error)?;
                }
                let persist = command_requires_persist(&envelope.command);
                self.apply_command(envelope.command, persist)?;
            }
            ScenarioStep::Ui(action) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_ui(action, self.session.world().elapsed_ms)
                        .map_err(feel_error)?;
                }
                self.apply_ui_action(action, false)?;
            }
            ScenarioStep::Capture(name) => {
                self.capture = Some(CaptureState::RenderPending(name));
                ctx.gfx.window().request_redraw();
            }
            ScenarioStep::Marker(name) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_marker(&name, self.session.world().elapsed_ms)
                        .map_err(feel_error)?;
                }
            }
            ScenarioStep::WaitTick { milliseconds } => {
                self.apply_command(SessionCommand::Tick { milliseconds }, false)?;
                ctx.gfx.window().request_redraw();
            }
        }
        Ok(())
    }
}

impl EventHandler for Game {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        if self.window_settings_dirty {
            self.apply_window_settings(ctx)?;
            self.window_settings_dirty = false;
        }
        let frame_delta_ms = ctx
            .time
            .delta()
            .as_millis()
            .clamp(1, 250)
            .try_into()
            .unwrap_or(250);
        if self
            .microphone
            .as_ref()
            .is_some_and(MicrophoneCapture::has_failed)
        {
            self.end_push_to_talk()?;
        }
        self.poll_recognition()?;
        self.poll_dialogue()?;
        self.poll_tts()?;
        if self.scenario.is_some() {
            self.drive_scenario(ctx)?;
        } else {
            let dt_ms = frame_delta_ms;
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
        if self.audio.speech_active() && self.view.speech.is_some() {
            self.view.speech_expires_at_ms = Some(
                self.session
                    .world()
                    .elapsed_ms
                    .saturating_add(SPEECH_RELEASE_MS),
            );
        }
        self.view.expire(self.session.world().elapsed_ms);
        self.update_speech_reveal();
        let had_one_shot = !self.queued_audio.is_empty();
        if let Some(bubble) = self
            .bubble_schedule
            .poll(self.session.world().elapsed_ms, had_one_shot)
        {
            self.queued_audio.push(bubble);
        }
        self.audio.ensure_ambience();
        self.audio
            .update_ducking(frame_delta_ms, !self.queued_audio.is_empty());
        if let Some(feel) = &mut self.feel {
            feel.record_audio(
                &self.queued_audio,
                self.audio.speech_active(),
                self.audio.one_shot_active(),
                self.audio.ambience_duck(),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.audio.play_queued(&mut self.queued_audio);
        if !self.audio.speech_active() {
            self.speech_animation = None;
        }
        self.view.speaking = self.audio.speech_active();
        self.view.mouth_phase = self.speech_animation.as_ref().map_or(0, |animation| {
            mouth_phase(
                animation.timing,
                self.session
                    .world()
                    .elapsed_ms
                    .saturating_sub(animation.started_at_ms),
            )
        });
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        if let Some(name) = take_capture_for_readback(&mut self.capture) {
            let directory = self
                .scenario
                .as_ref()
                .map(|scenario| scenario.capture_dir.as_path())
                .ok_or_else(|| GameError::ConfigError("capture has no scenario".to_owned()))?;
            save_presentation_png(
                ctx,
                &self.presentation_frame,
                &directory.join(format!("{name}.png")),
            )?;
        }

        let render = self.render_plan();
        let mut presentation = Canvas::from_image(
            ctx,
            self.presentation_frame.clone(),
            Color::from_rgb(20, 18, 24),
        );
        execute_plan(ctx, &mut presentation, &render, &self.assets)?;
        presentation.finish(ctx)?;

        if let Some(feel) = &mut self.feel {
            let rgba = presentation_rgba(ctx, &self.presentation_frame)?;
            feel.record_frame(&rgba, self.session.world(), &self.view)
                .map_err(feel_error)?;
        }

        if mark_capture_rendered(&mut self.capture) {
            ctx.gfx.window().request_redraw();
        }

        let (width, height) = ctx.gfx.drawable_size();
        self.viewport = Viewport::for_drawable(width, height);
        let mut frame = Canvas::from_frame(ctx, Color::from_rgb(12, 11, 15));
        frame.set_sampler(Sampler::nearest_clamp());
        frame.draw(
            &self.presentation_frame,
            DrawParam::default()
                .dest([self.viewport.x, self.viewport.y])
                .scale([self.viewport.scale, self.viewport.scale]),
        );
        frame.finish(ctx)?;
        if self.scenario.is_some() && !self.stay_open && self.finished_frames >= 3 {
            if let Some(mut feel) = self.feel.take() {
                feel.finish().map_err(feel_error)?;
            }
            ctx.request_quit();
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
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "mouse_button_down",
                serde_json::json!({"button": format!("{button:?}"), "x": x, "y": y}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.view.controller_active = false;
        if button != MouseButton::Left {
            return Ok(());
        }
        let Some((x, y)) = self.viewport.logical_point(x, y) else {
            return Ok(());
        };
        let render = self.render_plan();
        if let Some(action) = action_at_with_assets(&render, &self.assets, x, y) {
            if action == UiAction::PushToTalk {
                self.begin_push_to_talk()?;
            } else if let UiAction::DropFood(food) = action {
                let position = logical_to_world(x.floor() as i32, y.floor() as i32);
                self.cursor_world = Some(position);
                self.apply_command(SessionCommand::DropFood { food, position }, true)?;
                self.close_menu();
                self.queued_audio.push(UI_CONFIRM);
            } else {
                self.apply_confirmed_ui_action(action, false)?;
            }
        }
        Ok(())
    }

    fn mouse_button_up_event(
        &mut self,
        _ctx: &mut Context,
        button: MouseButton,
        x: f32,
        y: f32,
    ) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "mouse_button_up",
                serde_json::json!({"button": format!("{button:?}"), "x": x, "y": y}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if button == MouseButton::Left {
            self.end_push_to_talk()?;
        }
        Ok(())
    }

    fn mouse_motion_event(
        &mut self,
        ctx: &mut Context,
        x: f32,
        y: f32,
        _dx: f32,
        _dy: f32,
    ) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "mouse_motion",
                serde_json::json!({"x": x, "y": y}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.view.controller_active = false;
        let logical = self
            .viewport
            .logical_point(x, y)
            .map(|(x, y)| (x.floor() as i32, y.floor() as i32));
        self.pointer_logical = logical;
        let render = self.render_plan();
        self.view.hovered_region = logical.and_then(|(x, y)| {
            region_at_with_assets(&render, &self.assets, x as f32, y as f32)
                .map(|region| region.id.clone())
        });
        let cursor = logical.map_or(CursorKind::Default, |(x, y)| {
            cursor_at_with_assets(&render, &self.assets, x as f32, y as f32)
        });
        ctx.gfx.window().set_cursor(match cursor {
            CursorKind::Default => CursorIcon::Default,
            CursorKind::Pointer => CursorIcon::Pointer,
            CursorKind::FoodDrop => CursorIcon::Crosshair,
        });
        let cursor_world = logical
            .filter(|(_, y)| *y < beastie_view::COMPOSE_BAR_TOP)
            .map(|(x, y)| logical_to_world(x, y));
        if cursor_world != self.cursor_world {
            self.cursor_world = cursor_world;
            self.apply_command(
                SessionCommand::Cursor {
                    position: cursor_world,
                },
                false,
            )?;
        }
        Ok(())
    }

    fn mouse_enter_or_leave(&mut self, _ctx: &mut Context, entered: bool) -> GameResult {
        if !entered {
            self.pointer_logical = None;
            self.cursor_world = None;
            self.view.hovered_region = None;
            self.apply_command(SessionCommand::Cursor { position: None }, false)?;
        }
        Ok(())
    }

    fn key_down_event(&mut self, ctx: &mut Context, input: KeyInput, repeated: bool) -> GameResult {
        if let Some(feel) = &mut self.feel {
            let key = match &input.event.logical_key {
                Key::Character(_) => "character".to_owned(),
                Key::Named(named) => format!("{named:?}"),
                _ => "unidentified".to_owned(),
            };
            feel.record_native(
                "key_down",
                serde_json::json!({
                    "key": key,
                    "repeated": repeated,
                    "control": input.mods.control_key(),
                    "super": input.mods.super_key(),
                    "shift": input.mods.shift_key(),
                }),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.view.controller_active = false;
        let key = &input.event.logical_key;
        if let UiMode::Rebinding(action) = self.view.mode {
            if matches!(key, Key::Named(NamedKey::Escape)) {
                self.view.mode = UiMode::Bindings;
                self.reset_focus();
                return Ok(());
            }
            if !repeated && let Some(binding) = binding_key(key) {
                self.rebind(action, binding)?;
            }
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Rename) {
            match key {
                Key::Named(NamedKey::Escape) => self.close_menu(),
                Key::Named(NamedKey::Enter) => self.submit_name()?,
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
        if input.mods.control_key() || input.mods.super_key() {
            match key {
                Key::Character(character) if character.eq_ignore_ascii_case("q") => {
                    self.persist()?;
                    ctx.request_quit();
                    return Ok(());
                }
                Key::Character(character) if character == "+" || character == "=" => {
                    return self.change_window_scale(ctx, 1);
                }
                Key::Character(character) if character == "-" => {
                    return self.change_window_scale(ctx, -1);
                }
                _ => {}
            }
        }
        if !repeated
            && let Some(binding) = binding_key(key)
            && let Some(action) = self.settings.bindings.action_for(binding)
        {
            self.activate_binding(action)?;
            return Ok(());
        }
        if matches!(key, Key::Named(NamedKey::F11)) {
            return self.toggle_fullscreen(ctx);
        }
        if matches!(key, Key::Named(NamedKey::F8)) {
            self.settings.pixel_grid = !self.settings.pixel_grid;
            self.view.pixel_grid = self.settings.pixel_grid;
            return self.persist_settings();
        }
        if matches!(key, Key::Named(NamedKey::F6)) {
            self.settings.voice_enabled = !self.settings.voice_enabled;
            self.view.voice_enabled = self.settings.voice_enabled;
            if !self.settings.voice_enabled {
                self.audio.stop_speech();
            }
            return self.persist_settings();
        }
        if matches!(key, Key::Named(NamedKey::F7)) {
            self.settings.reduced_motion = !self.settings.reduced_motion;
            self.view.reduced_motion = self.settings.reduced_motion;
            return self.persist_settings();
        }
        if matches!(key, Key::Named(NamedKey::F9)) {
            self.settings.reduced_flashes = !self.settings.reduced_flashes;
            self.view.reduced_flashes = self.settings.reduced_flashes;
            return self.persist_settings();
        }
        if matches!(key, Key::Named(NamedKey::F10)) {
            self.settings.reduced_shake = !self.settings.reduced_shake;
            self.view.reduced_shake = self.settings.reduced_shake;
            return self.persist_settings();
        }
        if matches!(key, Key::Named(NamedKey::F12)) {
            self.settings.text_speed = cycle_text_speed(self.settings.text_speed);
            self.view.text_speed = text_speed_value(self.settings.text_speed);
            return self.persist_settings();
        }
        if matches!(self.view.mode, UiMode::Compose)
            && !repeated
            && matches!(key, Key::Named(NamedKey::Tab))
        {
            self.navigate(if input.mods.shift_key() { -1 } else { 1 });
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Compose)
            && self.view.focused_region.as_deref() != Some("compose/input")
            && !repeated
        {
            match key {
                Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown) => self.navigate(1),
                Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => self.navigate(-1),
                Key::Named(NamedKey::Enter | NamedKey::Space) => self.activate_focus(false)?,
                Key::Named(NamedKey::Escape) => {
                    self.view.focused_region = Some("compose/input".to_owned());
                }
                _ => {}
            }
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Compose) {
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.apply_ui_action(UiAction::ClearText, false)?;
                }
                Key::Named(NamedKey::Enter) if !self.view.pending => self.submit_text()?,
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
                self.close_menu();
            }
            Key::Named(NamedKey::Tab | NamedKey::ArrowRight | NamedKey::ArrowDown) => {
                self.navigate(1)
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => self.navigate(-1),
            Key::Named(NamedKey::Enter | NamedKey::Space) => self.activate_focus(false)?,
            _ => {}
        }
        Ok(())
    }

    fn key_up_event(&mut self, _ctx: &mut Context, input: KeyInput) -> GameResult {
        if let Some(feel) = &mut self.feel {
            let key = match &input.event.logical_key {
                Key::Character(_) => "character".to_owned(),
                Key::Named(named) => format!("{named:?}"),
                _ => "unidentified".to_owned(),
            };
            feel.record_native(
                "key_up",
                serde_json::json!({"key": key}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if binding_key(&input.event.logical_key) == Some(self.settings.bindings.push_to_talk)
            || matches!(
                input.event.logical_key,
                Key::Named(NamedKey::Enter | NamedKey::Space)
            )
        {
            self.end_push_to_talk()?;
        }
        Ok(())
    }

    fn gamepad_button_down_event(
        &mut self,
        _ctx: &mut Context,
        button: Button,
        _id: GamepadId,
    ) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "gamepad_button_down",
                serde_json::json!({"button": format!("{button:?}")}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.view.controller_active = true;
        if self.view.focused_region.is_none() {
            self.reset_focus();
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

    fn gamepad_button_up_event(
        &mut self,
        _ctx: &mut Context,
        button: Button,
        _id: GamepadId,
    ) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "gamepad_button_up",
                serde_json::json!({"button": format!("{button:?}")}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if button == Button::South {
            self.end_push_to_talk()?;
        }
        Ok(())
    }

    fn focus_event(&mut self, _ctx: &mut Context, gained: bool) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "window_focus",
                serde_json::json!({"gained": gained}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if !gained {
            self.end_push_to_talk()?;
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

fn load_session(store: &SaveStore) -> (GameSession, Option<String>, bool, bool) {
    let loaded = match store.load_recoverable() {
        Ok(Some(loaded)) => loaded,
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
    let (source, recovered) = match loaded {
        LoadedSave::Primary(source) => (source, false),
        LoadedSave::Backup(source) => (source, true),
    };
    match GameSession::resume_json(&source, unix_time_ms()) {
        Ok((session, _)) if recovered => match store.promote_backup() {
            Ok(true) => (
                session,
                Some("found the spare save. we're okay.".to_owned()),
                true,
                true,
            ),
            _ => (
                session,
                Some("found the spare, but couldn't repair the save box.".to_owned()),
                true,
                false,
            ),
        },
        Ok((session, _)) => (session, Some("you came back.".to_owned()), true, true),
        Err(_) => {
            if !recovered
                && let Ok(Some(backup)) = store.load_backup()
                && let Ok((session, _)) = GameSession::resume_json(&backup, unix_time_ms())
            {
                return match store.promote_backup() {
                    Ok(true) => (
                        session,
                        Some("main save was weird. used the spare.".to_owned()),
                        true,
                        true,
                    ),
                    _ => (
                        session,
                        Some("spare opened, but the save box couldn't be repaired.".to_owned()),
                        true,
                        false,
                    ),
                };
            }
            (
                GameSession::new(42, "Mop"),
                Some("old save smelled wrong. left it alone.".to_owned()),
                false,
                false,
            )
        }
    }
}

fn cycle_volume(volume: u8) -> u8 {
    match volume {
        0..=24 => 25,
        25..=49 => 50,
        50..=74 => 75,
        75..=99 => 100,
        _ => 0,
    }
}

fn cycle_text_speed(speed: TextSpeed) -> TextSpeed {
    match speed {
        TextSpeed::Instant => TextSpeed::Normal,
        TextSpeed::Normal => TextSpeed::Slow,
        TextSpeed::Slow => TextSpeed::Instant,
    }
}

const fn effective_dialogue_text_speed(configured: TextSpeed, scripted: bool) -> TextSpeed {
    if scripted {
        TextSpeed::Instant
    } else {
        configured
    }
}

fn revealed_text(full_text: &str, elapsed_ms: u64, speed: TextSpeed) -> (String, bool) {
    let milliseconds_per_character = match speed {
        TextSpeed::Instant => 0,
        TextSpeed::Normal => 30,
        TextSpeed::Slow => 70,
    };
    if milliseconds_per_character == 0 {
        return (full_text.to_owned(), true);
    }
    let visible = usize::try_from(elapsed_ms / milliseconds_per_character)
        .unwrap_or(usize::MAX)
        .saturating_add(1);
    let text = full_text.chars().take(visible).collect::<String>();
    (text, visible >= full_text.chars().count())
}

fn show_dialogue_caption(
    view: &mut ViewState,
    text: &str,
    elapsed_ms: u64,
    subtitles: bool,
    text_speed: TextSpeed,
) -> Option<SpeechReveal> {
    if !subtitles {
        view.clear_speech();
        return None;
    }
    view.show_speech(text.to_owned(), elapsed_ms);
    (text_speed != TextSpeed::Instant).then(|| SpeechReveal {
        full_text: text.to_owned(),
        started_at_ms: elapsed_ms,
    })
}

fn mouth_phase(timing: MouthTiming, elapsed_ms: u64) -> u8 {
    let syllable_ms = u64::from(timing.syllable_ms.max(1));
    let within = elapsed_ms % syllable_ms;
    if within < u64::from(timing.open_ms) {
        2
    } else if within < u64::from(timing.open_ms.saturating_add(timing.close_ms)) {
        0
    } else {
        1
    }
}

fn mark_capture_rendered(capture: &mut Option<CaptureState>) -> bool {
    let Some(CaptureState::RenderPending(name)) = capture.take() else {
        return false;
    };
    *capture = Some(CaptureState::ReadbackReady(name));
    true
}

fn take_capture_for_readback(capture: &mut Option<CaptureState>) -> Option<String> {
    match capture.take() {
        Some(CaptureState::ReadbackReady(name)) => Some(name),
        pending => {
            *capture = pending;
            None
        }
    }
}

const fn text_speed_value(speed: TextSpeed) -> u8 {
    match speed {
        TextSpeed::Instant => 0,
        TextSpeed::Normal => 1,
        TextSpeed::Slow => 2,
    }
}

fn binding_labels(bindings: &KeyBindings) -> BindingLabels {
    BindingLabels {
        push_to_talk: bindings.push_to_talk.label().to_owned(),
        food: bindings.food.label().to_owned(),
        play: bindings.play.label().to_owned(),
        comfort: bindings.comfort.label().to_owned(),
        settings: bindings.settings.label().to_owned(),
        cancel: bindings.cancel.label().to_owned(),
    }
}

fn binding_key(key: &Key) -> Option<BindingKey> {
    match key {
        Key::Named(NamedKey::Escape) => Some(BindingKey::Escape),
        Key::Named(NamedKey::F1) => Some(BindingKey::F1),
        Key::Named(NamedKey::F2) => Some(BindingKey::F2),
        Key::Named(NamedKey::F3) => Some(BindingKey::F3),
        Key::Named(NamedKey::F4) => Some(BindingKey::F4),
        Key::Named(NamedKey::F5) => Some(BindingKey::F5),
        Key::Named(NamedKey::F6) => Some(BindingKey::F6),
        Key::Named(NamedKey::F7) => Some(BindingKey::F7),
        Key::Named(NamedKey::F8) => Some(BindingKey::F8),
        Key::Named(NamedKey::F9) => Some(BindingKey::F9),
        Key::Named(NamedKey::F10) => Some(BindingKey::F10),
        Key::Named(NamedKey::F11) => Some(BindingKey::F11),
        Key::Named(NamedKey::F12) => Some(BindingKey::F12),
        _ => None,
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

fn feel_error(error: crate::feel::FeelError) -> GameError {
    GameError::FilesystemError(error.to_string())
}

fn play_command(toy: ToyId) -> SessionCommand {
    SessionCommand::Play { toy }
}

fn clears_speech(command: &SessionCommand) -> bool {
    matches!(
        command,
        SessionCommand::Feed { .. }
            | SessionCommand::DropFood { .. }
            | SessionCommand::Play { .. }
            | SessionCommand::Comfort
            | SessionCommand::Tidy
            | SessionCommand::Talk { .. }
    )
}

fn command_requires_persist(command: &SessionCommand) -> bool {
    matches!(
        command,
        SessionCommand::Feed { .. }
            | SessionCommand::DropFood { .. }
            | SessionCommand::Name { .. }
            | SessionCommand::Play { .. }
            | SessionCommand::Comfort
            | SessionCommand::Tidy
            | SessionCommand::Talk { .. }
            | SessionCommand::SpeechEnded
            | SessionCommand::React { .. }
    )
}

const fn spoken_input_is_pending(status: SpokenInputStatus) -> bool {
    matches!(
        status,
        SpokenInputStatus::Listening
            | SpokenInputStatus::CandidateUpdated { .. }
            | SpokenInputStatus::Deferred
    )
}

fn apply_spoken_input_status(
    view: &mut ViewState,
    status: SpokenInputStatus,
    now_ms: u64,
    resting_microphone_state: MicrophoneState,
) -> bool {
    let pending = spoken_input_is_pending(status);
    match status {
        SpokenInputStatus::Listening => {
            view.microphone_state = MicrophoneState::Listening;
            view.clear_status();
        }
        SpokenInputStatus::CandidateUpdated { .. } => {
            view.microphone_state = MicrophoneState::Recognizing;
        }
        SpokenInputStatus::Deferred => {
            view.microphone_state = MicrophoneState::Recognizing;
            view.show_status(
                "Heard you. Waiting for a good moment...".to_owned(),
                now_ms,
                60_000,
            );
        }
        SpokenInputStatus::Submitted => {
            view.microphone_state = resting_microphone_state;
            view.clear_status();
        }
        SpokenInputStatus::AcousticUncertainty { .. } => {
            view.microphone_state = resting_microphone_state;
            view.show_status(
                "Did not catch that. Hold F1 and try again.".to_owned(),
                now_ms,
                5_000,
            );
        }
        SpokenInputStatus::NoCandidate => {
            view.microphone_state = resting_microphone_state;
            view.show_status(
                "No speech heard. Hold F1 and try again.".to_owned(),
                now_ms,
                5_000,
            );
        }
        SpokenInputStatus::Refused => {
            view.microphone_state = resting_microphone_state;
            view.show_status("Not answering right now.".to_owned(), now_ms, 4_000);
        }
        SpokenInputStatus::NotEngaged { .. } => {
            view.microphone_state = resting_microphone_state;
            view.show_status("Not interested right now.".to_owned(), now_ms, 4_000);
        }
        SpokenInputStatus::InfrastructureFailure { failure } => {
            view.microphone_state = match failure {
                SpeechInputFailure::MicrophoneUnavailable
                | SpeechInputFailure::RecognizerUnavailable
                | SpeechInputFailure::UnsupportedLanguage => MicrophoneState::Unavailable,
                SpeechInputFailure::RecognitionFailed => MicrophoneState::Error,
            };
            let message = match failure {
                SpeechInputFailure::MicrophoneUnavailable => {
                    "Microphone unavailable. Text still works."
                }
                SpeechInputFailure::RecognizerUnavailable => {
                    "Speech recognition unavailable. Text still works."
                }
                SpeechInputFailure::RecognitionFailed => {
                    "Speech recognition failed. Hold F1 to try again."
                }
                SpeechInputFailure::UnsupportedLanguage => {
                    "Speech language unsupported. Text still works."
                }
            };
            view.show_status(message.to_owned(), now_ms, 5_000);
        }
    }
    pending
}

#[cfg(test)]
mod tests {
    use beastie_core::{Reaction, ToyId};
    use beastie_session::{
        SESSION_PROTOCOL_VERSION, SESSION_SAVE_VERSION, SessionCommand, SpokenInputStatus,
    };
    use beastie_view::{MicrophoneState, UiAction, ViewState};

    use super::{
        CaptureState, apply_spoken_input_status, clears_speech, command_requires_persist,
        effective_dialogue_text_speed, load_session, mark_capture_rendered, mouth_phase,
        play_command, revealed_text, show_dialogue_caption, spoken_input_is_pending,
        take_capture_for_readback,
    };
    use crate::save_store::SaveStore;
    use crate::settings::TextSpeed;

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
            UiAction::Inspect,
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
        assert!(clears_speech(&SessionCommand::DropFood {
            food: beastie_core::FoodId::Berry,
            position: beastie_core::NormalizedPosition::new(1_000, 2_000),
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
    fn completed_spoken_input_is_persisted_but_transient_recognition_is_not() {
        assert!(!command_requires_persist(&SessionCommand::SpeechStarted));
        assert!(!command_requires_persist(
            &SessionCommand::SpeechCandidate {
                text: "hello".to_owned(),
                confidence: beastie_protocol::AcousticConfidence::new(900).expect("valid"),
            }
        ));
        assert!(command_requires_persist(&SessionCommand::SpeechEnded));
        assert!(!command_requires_persist(&SessionCommand::SpeechFailed {
            failure: beastie_protocol::SpeechInputFailure::RecognitionFailed,
        }));
    }

    #[test]
    fn deferred_spoken_input_stays_busy_until_the_session_submits_it() {
        assert!(spoken_input_is_pending(SpokenInputStatus::Listening));
        assert!(spoken_input_is_pending(
            SpokenInputStatus::CandidateUpdated {
                confidence: beastie_protocol::AcousticConfidence::new(900).expect("valid"),
            }
        ));
        assert!(spoken_input_is_pending(SpokenInputStatus::Deferred));

        for terminal in [
            SpokenInputStatus::Submitted,
            SpokenInputStatus::NoCandidate,
            SpokenInputStatus::Refused,
            SpokenInputStatus::AcousticUncertainty {
                confidence: beastie_protocol::AcousticConfidence::new(500).expect("valid"),
            },
            SpokenInputStatus::NotEngaged {
                attention: beastie_core::SpeechAttention::Ignored,
            },
            SpokenInputStatus::InfrastructureFailure {
                failure: beastie_protocol::SpeechInputFailure::RecognitionFailed,
            },
        ] {
            assert!(!spoken_input_is_pending(terminal));
        }
    }

    #[test]
    fn submitted_speech_clears_the_deferred_message_and_failures_explain_recovery() {
        let mut view = ViewState::default();
        assert!(apply_spoken_input_status(
            &mut view,
            SpokenInputStatus::Deferred,
            1_000,
            MicrophoneState::Idle,
        ));
        assert_eq!(
            view.status_message.as_deref(),
            Some("Heard you. Waiting for a good moment...")
        );

        assert!(!apply_spoken_input_status(
            &mut view,
            SpokenInputStatus::Submitted,
            2_000,
            MicrophoneState::Idle,
        ));
        assert!(view.status_message.is_none());
        assert_eq!(view.microphone_state, MicrophoneState::Idle);

        apply_spoken_input_status(
            &mut view,
            SpokenInputStatus::InfrastructureFailure {
                failure: beastie_protocol::SpeechInputFailure::RecognizerUnavailable,
            },
            3_000,
            MicrophoneState::Idle,
        );
        assert_eq!(view.microphone_state, MicrophoneState::Unavailable);
        assert_eq!(
            view.status_message.as_deref(),
            Some("Speech recognition unavailable. Text still works.")
        );
    }

    #[test]
    fn text_reveal_is_unicode_safe_and_instant_is_an_accessibility_override() {
        assert_eq!(
            revealed_text("héy", 30, TextSpeed::Normal),
            ("hé".to_owned(), false)
        );
        assert_eq!(
            revealed_text("héy", 0, TextSpeed::Instant),
            ("héy".to_owned(), true)
        );
    }

    #[test]
    fn scripted_dialogue_is_fully_visible_at_capture_time() {
        assert_eq!(
            effective_dialogue_text_speed(TextSpeed::Slow, true),
            TextSpeed::Instant
        );
        assert_eq!(
            effective_dialogue_text_speed(TextSpeed::Slow, false),
            TextSpeed::Slow
        );
    }

    #[test]
    fn subtitles_suppress_only_the_visible_dialogue_caption() {
        let mut view = ViewState::default();
        let reveal =
            show_dialogue_caption(&mut view, "audible rude fish", 42, false, TextSpeed::Normal);
        assert!(reveal.is_none());
        assert!(view.speech.is_none());
        assert!(view.speech_expires_at_ms.is_none());

        let reveal =
            show_dialogue_caption(&mut view, "visible rude fish", 43, true, TextSpeed::Normal);
        assert!(reveal.is_some());
        assert_eq!(view.speech.as_deref(), Some("visible rude fish"));
    }

    #[test]
    fn genuine_v2_game_save_loads_continues_and_is_rewritten_as_v3() {
        let directory =
            std::env::temp_dir().join(format!("beastie-v2-game-migration-{}", std::process::id()));
        let path = directory.join("main.json");
        let store = SaveStore::new(path.clone());
        store
            .store(include_str!("../../../fixtures/saves/v2-mvp-session.json"))
            .expect("store historical V2 fixture");

        let (mut session, message, resumed, save_enabled) = load_session(&store);
        assert!(resumed);
        assert!(save_enabled);
        assert_eq!(message.as_deref(), Some("you came back."));
        assert_eq!(session.world().creature.name, "Mop");
        assert_eq!(session.world().creature.memories.len(), 5);
        assert!(!session.world().aquarium.objects.is_empty());

        let before = session.world().elapsed_ms;
        session
            .apply(beastie_session::CommandEnvelope {
                version: SESSION_PROTOCOL_VERSION,
                command: SessionCommand::Tick {
                    milliseconds: 1_000,
                },
            })
            .expect("continue migrated game");
        assert_eq!(session.world().elapsed_ms, before + 1_000);

        let current = session
            .capture(super::unix_time_ms())
            .to_json()
            .expect("serialize current session save");
        store.store(&current).expect("write migrated V3 save");
        let value = serde_json::from_str::<serde_json::Value>(&current).expect("current JSON");
        assert_eq!(value["version"], SESSION_SAVE_VERSION);
        assert_eq!(value["world"]["save_version"], beastie_core::SAVE_VERSION);
        assert!(value["world"].get("room").is_none());
        assert!(value["world"].get("aquarium").is_some());

        let (reloaded, _, reloaded_from_disk, writable) = load_session(&store);
        assert!(reloaded_from_disk);
        assert!(writable);
        assert_eq!(reloaded.world().creature.name, "Mop");
        std::fs::remove_dir_all(directory).expect("clean migration fixture directory");
    }

    #[test]
    fn protocol_mouth_timing_drives_repeatable_open_close_rest_phases() {
        let timing = beastie_protocol::MouthTiming {
            open_ms: 60,
            close_ms: 40,
            syllable_ms: 160,
        };
        assert_eq!(mouth_phase(timing, 0), 2);
        assert_eq!(mouth_phase(timing, 70), 0);
        assert_eq!(mouth_phase(timing, 120), 1);
        assert_eq!(mouth_phase(timing, 160), 2);
    }

    #[test]
    fn scripted_capture_waits_for_submitted_render_before_readback() {
        let mut capture = Some(CaptureState::RenderPending("aquarium".to_owned()));
        assert_eq!(take_capture_for_readback(&mut capture), None);
        assert!(mark_capture_rendered(&mut capture));
        assert_eq!(
            take_capture_for_readback(&mut capture).as_deref(),
            Some("aquarium")
        );
        assert_eq!(capture, None);
    }
}
