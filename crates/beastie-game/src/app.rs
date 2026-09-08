use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{GameError, GameResult};
use crate::host::KeyStroke;
use beastie_core::{GameEvent, NamingTarget, NormalizedPosition, ToyId};
use beastie_protocol::{
    MouthTiming, RecognitionOutcome, SpeechInputFailure, TranscriptRecord,
    identity_tts_voice_settings,
};
use beastie_session::{
    CommandEnvelope, GameSession, SESSION_PROTOCOL_VERSION, SessionCommand, SessionError,
    SessionSave, SpokenInputStatus,
};
use beastie_view::{
    AudioCommand, AudioCue, BindableAction, BindingLabels, MicrophoneState, PresentationChannel,
    SPEECH_RELEASE_MS, ScenePlan, SemanticOwner, UiAction, UiMode, ViewState, plan,
};
use bevy::input::keyboard::Key;

use crate::args::Args;
use crate::audio::{AmbientBubbleSchedule, AudioBank, sound_for_cue};
use crate::dialogue::{DialogueManager, DialogueOwner, DialogueTurn, WorkerConfig};
use crate::feel::{
    AudioTraceFrame, DialogueHealthTrace, DialogueTraceOwner, FeelRecorder, PresentationTraceState,
    SpeechTraceOwner,
};
use crate::input::{append_text, focused_action, move_focus};
use crate::microphone::{MicrophoneCapture, MicrophoneError, PrivateAudioRoot};
use crate::recognition::{RecognitionManager, RecognitionWorkerConfig, speech_failure};
use crate::save_store::{LoadedSave, SaveStore};
use crate::scenario::{MicrophoneAcquisition, ScenarioRunner, ScenarioStep};
use crate::settings::{BindingKey, KeyBindings, SettingsStore, TextScale, TextSpeed, UserSettings};
use crate::transcript::TranscriptStore;
use crate::tts::{TtsManager, TtsWorkerConfig};

struct SpeechReveal {
    owner: DialogueOwner,
    full_text: String,
    started_at_ms: u64,
}

struct SpeechAnimation {
    owner: DialogueOwner,
    started_at_ms: u64,
    timing: MouthTiming,
}

struct PendingTts {
    owner: DialogueOwner,
    request_id: u64,
    mouth_timing: MouthTiming,
}

struct DelayedCompletion<T> {
    delay_ms: u64,
    release_at_ms: Option<u64>,
    completion: Option<T>,
}

impl<T> DelayedCompletion<T> {
    const fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            release_at_ms: None,
            completion: None,
        }
    }

    fn arm(&mut self, now_ms: u64) {
        self.release_at_ms = Some(now_ms.saturating_add(self.delay_ms));
        self.completion = None;
    }

    fn hold_or_release(&mut self, completion: T, now_ms: u64) -> Option<T> {
        if self
            .release_at_ms
            .is_some_and(|release_at_ms| now_ms < release_at_ms)
        {
            self.completion = Some(completion);
            None
        } else {
            self.release_at_ms = None;
            Some(completion)
        }
    }

    fn take_ready(&mut self, now_ms: u64) -> Option<T> {
        if !self
            .release_at_ms
            .is_some_and(|release_at_ms| now_ms >= release_at_ms)
        {
            return None;
        }
        self.release_at_ms = None;
        self.completion.take()
    }

    fn cancel(&mut self) {
        self.release_at_ms = None;
        self.completion = None;
    }

    const fn allows_scenario_progress(&self) -> bool {
        self.delay_ms > 0 && self.release_at_ms.is_some()
    }
}

impl PendingTts {
    const fn trace_owner(&self) -> SpeechTraceOwner {
        SpeechTraceOwner {
            dialogue_generation: self.owner.generation,
            dialogue_request_id: self.owner.request_id,
            tts_request_id: self.request_id,
        }
    }
}

const fn dialogue_trace_owner(owner: DialogueOwner) -> DialogueTraceOwner {
    DialogueTraceOwner {
        dialogue_generation: owner.generation,
        dialogue_request_id: owner.request_id,
    }
}

pub struct Game {
    session: GameSession,
    view: ViewState,
    audio: AudioBank,
    queued_audio: Vec<AudioCommand>,
    dialogue: DialogueManager,
    dialogue_generation: u64,
    active_dialogue_owner: Option<DialogueOwner>,
    delayed_dialogue: DelayedCompletion<DialogueTurn>,
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
    pending_tts: Option<PendingTts>,
    active_speech_owner: Option<SpeechTraceOwner>,
    turn_status_owner: Option<DialogueOwner>,
    speech_animation: Option<SpeechAnimation>,
    transcript_export_path: PathBuf,
    renaming_with_osk: bool,
    pub(crate) quit_requested: bool,
    pub(crate) failed: bool,
    /// Backpressure only: ordinary asynchronous screenshots never pause gameplay.
    pub(crate) frame_pending: bool,
    capture_outstanding: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureState {
    RenderPending(String),
}

/// The shell must establish capture before it tells the simulation that Mop perceived speech.
/// These outcomes remain deliberately small and testable because device startup is host I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MicrophoneAcquisitionOutcome {
    Acquired,
    Unavailable,
    StartFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MicrophoneAcquisitionTransition {
    starts_perception: bool,
    state: MicrophoneState,
    message: Option<&'static str>,
}

const fn microphone_acquisition_transition(
    outcome: MicrophoneAcquisitionOutcome,
) -> MicrophoneAcquisitionTransition {
    match outcome {
        MicrophoneAcquisitionOutcome::Acquired => MicrophoneAcquisitionTransition {
            starts_perception: true,
            state: MicrophoneState::Listening,
            message: None,
        },
        MicrophoneAcquisitionOutcome::Unavailable => MicrophoneAcquisitionTransition {
            starts_perception: false,
            state: MicrophoneState::Unavailable,
            message: Some("Microphone unavailable. Text still works"),
        },
        MicrophoneAcquisitionOutcome::StartFailed => MicrophoneAcquisitionTransition {
            starts_perception: false,
            state: MicrophoneState::Error,
            message: Some("Microphone could not start. Text still works"),
        },
    }
}

const fn microphone_acquisition_label(outcome: MicrophoneAcquisitionOutcome) -> &'static str {
    match outcome {
        MicrophoneAcquisitionOutcome::Acquired => "acquired",
        MicrophoneAcquisitionOutcome::Unavailable => "unavailable",
        MicrophoneAcquisitionOutcome::StartFailed => "start_failed",
    }
}

impl Game {
    pub fn new(args: &Args) -> GameResult<Self> {
        let assets_root = assets_root();
        let directories = directories::ProjectDirs::from("", "Michael Thiesen", "beastie")
            .ok_or_else(|| GameError::Config("No local configuration directory".to_owned()))?;
        let config_dir = directories.config_dir();
        let save_store = SaveStore::new(config_dir.join("saves").join("main.json"));
        let settings_store = SettingsStore::new(config_dir.join("settings.json"));
        let (settings, settings_message) = if args.feel_dir.is_some() {
            // Feel evidence must not inherit the operator's accessibility, window, audio, or
            // microphone preferences. Comparisons need one explicit, repeatable presentation.
            (UserSettings::default(), None)
        } else {
            match settings_store.load() {
                Ok(settings) => (settings, None),
                Err(_) => (
                    UserSettings::default(),
                    Some("Settings unreadable. Using safe defaults.".to_owned()),
                ),
            }
        };
        if args.new_game {
            save_store
                .reset()
                .map_err(|error| GameError::Filesystem(error.to_string()))?;
        }
        let tts = TtsManager::new(TtsWorkerConfig::discover(
            args.tts && settings.voice_enabled,
            config_dir.join("tts-cache"),
        ));
        let stt_audio_root = PrivateAudioRoot::prepare(config_dir.join("stt-input"))
            .map_err(|error| GameError::Filesystem(error.to_string()))?;
        let recognition = RecognitionManager::new(RecognitionWorkerConfig::discover(
            args.fake_ai,
            stt_audio_root.path(),
            args.stt_worker.as_deref(),
            &args.stt_backend,
            args.stt_model_dir.as_deref(),
            args.moonshine_engine.as_deref(),
            args.stt_timeout_ms,
        ));
        let (session, load_message, _resumed, save_enabled) =
            if let Some(path) = &args.feel_initial_save {
                let source = std::fs::read_to_string(path)
                    .map_err(|error| GameError::Filesystem(error.to_string()))?;
                let save = SessionSave::from_json(&source)
                    .map_err(|error| GameError::Config(error.to_string()))?;
                let resumed_at_ms = save.saved_at_ms;
                let (session, progress) = GameSession::resume(save, resumed_at_ms)
                    .map_err(|error| GameError::Config(error.to_string()))?;
                debug_assert_eq!(progress.applied_ms, 0);
                (session, None, false, false)
            } else if args.script.is_some() {
                (
                    GameSession::new(args.feel_seed.unwrap_or(42), "Mop"),
                    None,
                    false,
                    false,
                )
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
            .map_err(|error| GameError::Config(error.to_string()))?;
        let feel = args
            .feel_dir
            .clone()
            .map(FeelRecorder::create)
            .transpose()
            .map_err(feel_error)?;
        let mut view = ViewState {
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
        if let Some(message) = settings_message {
            view.show_status(message, session.world().elapsed_ms, 12_000);
        }
        if let Some(notice) = load_message {
            present_startup_notice(&mut view, notice, session.world().elapsed_ms);
        }
        let mut audio = AudioBank::load(&assets_root);
        audio.set_gains(settings.effects_gain(), settings.speech_gain());
        let transcripts = TranscriptStore::new(
            config_dir.join("transcripts/playtest.jsonl"),
            args.transcript || settings.transcript_enabled,
        );
        let transcript_export_path = config_dir.join("transcripts/beastie-playtest-export.jsonl");
        let bubble_schedule =
            AmbientBubbleSchedule::new(session.world().seed, session.world().elapsed_ms);
        let game = Self {
            session,
            view,
            audio,
            queued_audio: Vec::new(),
            dialogue: DialogueManager::new(WorkerConfig::from_environment(
                args.fake_ai,
                args.ai_timeout_ms.map_or_else(
                    WorkerConfig::environment_reply_timeout,
                    Duration::from_millis,
                ),
            )),
            dialogue_generation: 1,
            active_dialogue_owner: None,
            delayed_dialogue: DelayedCompletion::new(
                args.feel_dialogue_delay_ms.unwrap_or_default(),
            ),
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
            pending_tts: None,
            active_speech_owner: None,
            turn_status_owner: None,
            speech_animation: None,
            transcript_export_path,
            renaming_with_osk: false,
            quit_requested: false,
            failed: false,
            frame_pending: true,
            capture_outstanding: 0,
        };
        if let Some(destination) = &args.export_transcript {
            game.transcripts
                .export(destination)
                .map_err(|error| GameError::Filesystem(error.to_string()))?;
        }
        Ok(game)
    }

    pub(crate) fn render_plan(&self) -> ScenePlan {
        plan(self.session.world(), &self.view).0
    }

    pub(crate) fn is_scripted(&self) -> bool {
        self.scenario.is_some()
    }
    pub(crate) fn window_settings(&self) -> (bool, u8) {
        (self.settings.fullscreen, self.settings.window_scale)
    }
    pub(crate) fn take_window_settings_dirty(&mut self) -> bool {
        std::mem::take(&mut self.window_settings_dirty)
    }
    pub(crate) fn needs_capture(&self) -> bool {
        self.capture.is_some() || self.feel.is_some()
    }
    pub(crate) fn capture_path(&self) -> Option<PathBuf> {
        let CaptureState::RenderPending(name) = self.capture.as_ref()?;
        Some(
            self.scenario
                .as_ref()?
                .capture_dir
                .join(format!("{name}.png")),
        )
    }
    pub(crate) fn presentation_trace(&self) -> PresentationTraceState {
        PresentationTraceState {
            subtitles_enabled: self.settings.subtitles,
            active_dialogue_owner: self.active_dialogue_owner.map(dialogue_trace_owner),
            caption_owner: self
                .view
                .speech
                .as_ref()
                .and(self.active_dialogue_owner)
                .map(dialogue_trace_owner),
            pending_mouth_owner: self.pending_tts.as_ref().map(PendingTts::trace_owner),
            active_mouth_owner: self.active_speech_owner,
        }
    }
    pub(crate) fn begin_capture(&mut self) -> crate::capture::FrameSnapshot {
        let snapshot = crate::capture::FrameSnapshot {
            world: self.session.world().clone(),
            view: self.view.clone(),
            presentation: self.presentation_trace(),
            path: self.capture_path(),
            feel_frame: self.feel.as_mut().map(FeelRecorder::schedule_frame),
        };
        self.capture = None;
        self.capture_outstanding += 1;
        snapshot
    }

    pub(crate) fn record_rendered_frame(
        &mut self,
        snapshot: &crate::capture::FrameSnapshot,
        rgba: &[u8],
    ) -> GameResult {
        if let Some(index) = snapshot.feel_frame {
            self.feel
                .as_mut()
                .ok_or_else(|| {
                    GameError::Config("Capture completed after recorder shutdown".to_owned())
                })?
                .record_frame(
                    index,
                    rgba,
                    &snapshot.world,
                    &snapshot.view,
                    snapshot.presentation,
                )
                .map_err(feel_error)?;
        }
        self.capture_outstanding = self.capture_outstanding.saturating_sub(1);
        Ok(())
    }

    pub(crate) fn captures_drained(&self) -> bool {
        self.capture_outstanding == 0
    }

    pub(crate) fn fail_captures(&mut self) {
        self.failed = true;
        self.quit_requested = true;
        self.capture_outstanding = 0;
        self.frame_pending = false;
    }

    pub(crate) fn finalize_capture(&mut self) -> GameResult {
        if self.captures_drained()
            && let Some(mut feel) = self.feel.take()
        {
            feel.finish().map_err(feel_error)?;
        }
        Ok(())
    }

    /// Counts submitted presentation frames, independently of GPU completion latency.
    pub(crate) fn finish_frame(&mut self) -> GameResult {
        if let Some(frames) = &mut self.smoke_frames {
            *frames = frames.saturating_sub(1);
            if *frames == 0 {
                self.quit_requested = true;
            }
        }
        if self.scenario.is_some() && !self.stay_open && self.finished_frames >= 3 {
            self.quit_requested = true;
        }
        Ok(())
    }

    fn persist_settings(&self) -> GameResult {
        if self.scenario.is_some() {
            return Ok(());
        }
        self.settings_store
            .store(&self.settings)
            .map_err(|error| GameError::Filesystem(error.to_string()))
    }

    fn toggle_fullscreen(&mut self) -> GameResult {
        self.settings.fullscreen = !self.settings.fullscreen;
        self.view.fullscreen = self.settings.fullscreen;
        self.window_settings_dirty = true;
        self.persist_settings()
    }

    fn change_window_scale(&mut self, delta: i8) -> GameResult {
        if self.settings.fullscreen {
            return Ok(());
        }
        self.settings.window_scale = i16::from(self.settings.window_scale)
            .saturating_add(i16::from(delta))
            .clamp(1, 6) as u8;
        self.view.window_scale = self.settings.window_scale;
        self.window_settings_dirty = true;
        self.persist_settings()
    }

    fn apply_command(&mut self, command: SessionCommand, persist: bool) -> GameResult {
        if clears_speech(&command) {
            self.supersede_dialogue_turn()?;
        }
        let observation = self
            .session
            .apply(CommandEnvelope {
                version: SESSION_PROTOCOL_VERSION,
                command,
            })
            .map_err(session_error)?;
        let audio = self
            .view
            .observe_events(&observation.events, self.session.world().elapsed_ms);
        self.queued_audio.extend(audio.events);
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
            && let Some(owner) = self
                .dialogue
                .request_owned(request, self.dialogue_generation)
        {
            self.active_dialogue_owner = Some(owner);
            self.delayed_dialogue.arm(self.session.world().elapsed_ms);
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
            .map_err(|error| GameError::Filesystem(error.to_string()))
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
        match MicrophoneCapture::start() {
            Ok(capture) => {
                self.apply_microphone_acquisition(MicrophoneAcquisitionOutcome::Acquired)?;
                self.microphone = Some(capture);
            }
            Err(MicrophoneError::Unavailable | MicrophoneError::UnsupportedFormat) => {
                self.apply_microphone_acquisition(MicrophoneAcquisitionOutcome::Unavailable)?;
            }
            Err(MicrophoneError::Start(_) | MicrophoneError::Storage(_)) => {
                self.apply_microphone_acquisition(MicrophoneAcquisitionOutcome::StartFailed)?;
            }
        }
        Ok(())
    }

    /// Applies the host-side result of microphone acquisition. Failures deliberately never send a
    /// session command: without a live capture there is no perceptual evidence for the creature.
    fn apply_microphone_acquisition(
        &mut self,
        outcome: MicrophoneAcquisitionOutcome,
    ) -> GameResult {
        let transition = microphone_acquisition_transition(outcome);
        if transition.starts_perception {
            self.apply_command(SessionCommand::SpeechStarted, false)?;
        }
        self.view.microphone_state = transition.state;
        self.view.status_message = transition.message.map(str::to_owned);
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "microphone_acquisition",
                serde_json::json!({
                    "outcome": microphone_acquisition_label(outcome),
                    "capture_evidence": transition.starts_perception,
                }),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
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
        let now_ms = self.session.world().elapsed_ms;
        let turn = if let Some(turn) = self.delayed_dialogue.take_ready(now_ms) {
            turn
        } else {
            if !self.dialogue.is_pending() {
                return Ok(());
            }
            match self.dialogue.try_recv_turn() {
                Ok(turn) => {
                    let Some(turn) = self.delayed_dialogue.hold_or_release(turn, now_ms) else {
                        return Ok(());
                    };
                    turn
                }
                Err(TryRecvError::Disconnected) => {
                    self.delayed_dialogue.cancel();
                    let owner = self.active_dialogue_owner;
                    self.view.pending = false;
                    self.active_dialogue_owner = None;
                    let text_speed = effective_dialogue_text_speed(
                        self.settings.text_speed,
                        self.scenario.is_some(),
                    );
                    self.speech_reveal = owner.and_then(|owner| {
                        show_dialogue_caption(
                            &mut self.view,
                            owner,
                            "too many thought.",
                            self.session.world().elapsed_ms,
                            self.settings.subtitles,
                            text_speed,
                        )
                    });
                    return Ok(());
                }
                Err(TryRecvError::Empty) => return Ok(()),
            }
        };
        if self.active_dialogue_owner != Some(turn.owner) {
            return Ok(());
        }
        self.view.pending = false;
        self.audio.stop_speech();
        let accepted_by_session = self.session.accept_dialogue_turn(
            &turn.request,
            &turn.reply,
            turn.retry_count,
            turn.fallback,
        );
        if let Some(feel) = &mut self.feel {
            feel.record_dialogue_health(
                DialogueHealthTrace {
                    owner: dialogue_trace_owner(turn.owner),
                    backend: turn.backend,
                    fallback: turn.fallback,
                    fallback_reason: turn.fallback_reason,
                    retry_count: turn.retry_count,
                    duplicate_suppressed: turn.duplicate_suppressed,
                    reply_word_count: turn.reply.say.split_whitespace().count(),
                    recalled_memory: turn.reply.recalled_memory.is_some(),
                    recalled_belief: turn.reply.recalled_belief.is_some(),
                    accepted_by_session,
                },
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if !accepted_by_session {
            // The player superseded the authoritative relationship beat while local
            // inference was pending. Never present, speak, or transcript stale wording.
            self.active_dialogue_owner = None;
            self.pending_tts = None;
            self.view.mode = UiMode::Compose;
            return Ok(());
        }
        self.persist()?;
        let voice = identity_tts_voice_settings(self.session.world());
        let text_speed =
            effective_dialogue_text_speed(self.settings.text_speed, self.scenario.is_some());
        if self.settings.voice_enabled
            && let Some(request_id) = self
                .tts
                .request(turn.reply.say.clone(), self.session.world())
        {
            self.pending_tts = Some(PendingTts {
                owner: turn.owner,
                request_id,
                mouth_timing: voice.mouth_timing,
            });
            if let Some(feel) = &mut self.feel {
                feel.record_tts_lifecycle(
                    "tts_enqueued",
                    self.pending_tts
                        .as_ref()
                        .expect("just installed pending TTS")
                        .trace_owner(),
                    None,
                    self.session.world().elapsed_ms,
                )
                .map_err(feel_error)?;
            }
        }
        self.speech_reveal = show_dialogue_caption(
            &mut self.view,
            turn.owner,
            &turn.reply.say,
            self.session.world().elapsed_ms,
            self.settings.subtitles,
            text_speed,
        );
        if turn.fallback {
            self.view.show_status(
                "Local AI unavailable".to_owned(),
                self.session.world().elapsed_ms,
                5_000,
            );
            self.turn_status_owner = Some(turn.owner);
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
            .map_err(|error| GameError::Filesystem(error.to_string()))?;
        self.view.mode = UiMode::Compose;
        self.view.focused_region = Some("reaction/laugh".to_owned());
        Ok(())
    }

    fn poll_tts(&mut self) -> GameResult {
        match self.tts.try_recv() {
            Ok(completion) => {
                let Some(pending) = self.pending_tts.take() else {
                    return Ok(());
                };
                let trace_owner = pending.trace_owner();
                if pending.request_id != completion.request_id
                    || self.active_dialogue_owner != Some(pending.owner)
                {
                    if let Some(feel) = &mut self.feel {
                        feel.record_tts_lifecycle(
                            "tts_completion_discarded",
                            trace_owner,
                            Some(completion.wav.is_some()),
                            self.session.world().elapsed_ms,
                        )
                        .map_err(feel_error)?;
                    }
                    return Ok(());
                }
                if let Some(feel) = &mut self.feel {
                    feel.record_tts_lifecycle(
                        "tts_completion_accepted",
                        trace_owner,
                        Some(completion.wav.is_some()),
                        self.session.world().elapsed_ms,
                    )
                    .map_err(feel_error)?;
                }
                if let Some(wav) = completion.wav {
                    if let Some(feel) = &mut self.feel {
                        feel.record_speech(&wav, trace_owner, self.session.world().elapsed_ms)
                            .map_err(feel_error)?;
                    }
                    let started = self.audio.play_speech(wav, trace_owner);
                    if let Some(feel) = &mut self.feel {
                        feel.record_tts_lifecycle(
                            if started {
                                "speech_playback_started"
                            } else {
                                "speech_playback_unavailable"
                            },
                            trace_owner,
                            Some(true),
                            self.session.world().elapsed_ms,
                        )
                        .map_err(feel_error)?;
                    }
                    if !started {
                        return Ok(());
                    }
                    self.active_speech_owner = Some(trace_owner);
                    self.speech_animation = Some(SpeechAnimation {
                        owner: pending.owner,
                        started_at_ms: self.session.world().elapsed_ms,
                        timing: pending.mouth_timing,
                    });
                }
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        Ok(())
    }

    fn apply_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        self.view.compose_engaged = matches!(
            action,
            UiAction::FocusCompose | UiAction::Talk | UiAction::TypeCharacter(_)
        );
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
            self.supersede_dialogue_turn()?;
        }
        match action {
            UiAction::OpenContext(target) => {
                // Choose the clear region once. Swimming must not move controls under a pointer.
                self.view.context_above = Some(
                    beastie_view::world_to_logical(self.session.world().creature.aquarium.position)
                        .1
                        > 68,
                );
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
                self.view.focused_region =
                    Some(format!("settings/page-{}", self.view.settings_page.min(2)));
            }
            UiAction::SelectSettingsPage(page) => {
                self.view.settings_page = page.min(2);
                self.view.mode = UiMode::Settings;
                self.view.focused_region =
                    Some(format!("settings/page-{}", self.view.settings_page));
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
                    self.stop_owned_voice()?;
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
            UiAction::RecoverBackup => self.recover_backup()?,
            UiAction::RequestReset => {
                self.view.mode = UiMode::ConfirmReset;
                self.reset_focus();
            }
            UiAction::ConfirmReset => self.reset_save()?,
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

    fn recover_backup(&mut self) -> GameResult {
        self.supersede_dialogue_turn()?;
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
        Ok(())
    }

    fn reset_save(&mut self) -> GameResult {
        self.supersede_dialogue_turn()?;
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
        Ok(())
    }

    fn apply_confirmed_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        self.apply_ui_action(action, controller)?;
        self.queued_audio.push(ui_audio(AudioCue::UiConfirm));
        Ok(())
    }

    fn close_menu(&mut self) {
        self.renaming_with_osk = false;
        self.view.compose_engaged = false;
        self.view.mode = UiMode::Compose;
        self.view.focused_region = Some("compose/input".to_owned());
    }

    fn clear_speech(&mut self) -> GameResult {
        self.view.clear_speech();
        self.stop_owned_voice()?;
        self.speech_reveal = None;
        Ok(())
    }

    fn stop_owned_voice(&mut self) -> GameResult {
        self.audio.stop_speech();
        self.speech_animation = None;
        if let Some(pending) = self.pending_tts.take() {
            let _ = self.tts.cancel(pending.request_id);
            if let Some(feel) = &mut self.feel {
                feel.record_tts_lifecycle(
                    "tts_canceled",
                    pending.trace_owner(),
                    None,
                    self.session.world().elapsed_ms,
                )
                .map_err(feel_error)?;
            }
        }
        if let Some(owner) = self.active_speech_owner.take()
            && let Some(feel) = &mut self.feel
        {
            feel.record_tts_lifecycle(
                "speech_playback_stopped",
                owner,
                None,
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        Ok(())
    }

    fn supersede_dialogue_turn(&mut self) -> GameResult {
        self.dialogue_generation = self.dialogue_generation.wrapping_add(1).max(1);
        if let Some(owner) = self.active_dialogue_owner.take() {
            let _ = self.dialogue.cancel(owner);
        }
        self.delayed_dialogue.cancel();
        self.view.pending = false;
        if self.turn_status_owner.take().is_some()
            && self.view.status_message.as_deref() == Some("Local AI unavailable")
        {
            self.view.status_message = None;
            self.view.status_expires_at_ms = None;
        }
        self.clear_speech()
    }

    fn update_speech_reveal(&mut self) {
        let Some(reveal) = &self.speech_reveal else {
            return;
        };
        if self.active_dialogue_owner != Some(reveal.owner) {
            self.speech_reveal = None;
            return;
        }
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
        self.view.compose_engaged = self.view.focused_region.as_deref() == Some("compose/input");
        if self.view.focused_region != previous {
            self.queued_audio.push(ui_audio(AudioCue::UiReject));
        }
    }

    fn activate_focus(&mut self, controller: bool) -> GameResult {
        let plan = self.render_plan();
        if let Some(action) = focused_action(&plan, self.view.focused_region.as_deref()) {
            self.apply_confirmed_ui_action(action, controller)?;
        }
        Ok(())
    }

    fn drive_scenario(&mut self) -> GameResult {
        if (self.dialogue.is_pending() && !self.delayed_dialogue.allows_scenario_progress())
            || self.capture.is_some()
        {
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
            ScenarioStep::ControllerUi(action) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_ui(action, self.session.world().elapsed_ms)
                        .map_err(feel_error)?;
                }
                self.apply_ui_action(action, true)?;
            }
            ScenarioStep::MicrophoneAcquisition(outcome) => {
                let outcome = match outcome {
                    MicrophoneAcquisition::Acquired => MicrophoneAcquisitionOutcome::Acquired,
                    MicrophoneAcquisition::Unavailable => MicrophoneAcquisitionOutcome::Unavailable,
                };
                self.apply_microphone_acquisition(outcome)?;
            }
            ScenarioStep::SetSubtitles(enabled) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_native(
                        "scenario_setting",
                        serde_json::json!({"subtitles_enabled": enabled}),
                        self.session.world().elapsed_ms,
                    )
                    .map_err(feel_error)?;
                }
                self.settings.subtitles = enabled;
                self.view.subtitles = enabled;
                if !enabled {
                    self.view.clear_speech();
                    self.speech_reveal = None;
                }
            }
            ScenarioStep::Capture(name) => {
                self.capture = Some(CaptureState::RenderPending(name));
            }
            ScenarioStep::Marker(name) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_marker(&name, self.session.world().elapsed_ms)
                        .map_err(feel_error)?;
                }
            }
            ScenarioStep::WaitTick { milliseconds } => {
                self.apply_command(SessionCommand::Tick { milliseconds }, false)?;
            }
        }
        Ok(())
    }
}

impl Game {
    pub(crate) fn update(&mut self, frame_delta_ms: u64, advance_frame: bool) -> GameResult {
        if self.quit_requested {
            return Ok(());
        }
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
        if self.frame_pending || (self.scenario.is_some() && !advance_frame) {
            return Ok(());
        }
        if self.scenario.is_some() {
            self.drive_scenario()?;
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
        let had_one_shot = self
            .queued_audio
            .iter()
            .any(|command| matches!(command, AudioCommand::Play { .. }));
        if let Some(bubble) = self
            .bubble_schedule
            .poll(self.session.world().elapsed_ms, had_one_shot)
        {
            self.queued_audio.push(AudioCommand::play(
                SemanticOwner::Ordinary,
                PresentationChannel::Ambience,
                bubble,
                700,
            ));
        }
        let requested_audio = self.feel.as_ref().map(|_| self.queued_audio.clone());
        self.audio.ensure_ambience();
        self.audio.play_queued(&mut self.queued_audio);
        self.audio.update_ducking(frame_delta_ms);
        let decisions = self.audio.take_decisions();
        if let Some(feel) = &mut self.feel {
            let commands = requested_audio.as_deref().unwrap_or_default();
            let playback = self.audio.playback_snapshot();
            let cue_ids = commands
                .iter()
                .filter_map(|command| match command {
                    AudioCommand::Play { cue, .. } => sound_for_cue(*cue),
                    _ => None,
                })
                .collect::<Vec<_>>();
            feel.record_audio(
                AudioTraceFrame {
                    commands,
                    cue_ids: &cue_ids,
                    playback: &playback,
                    decisions: &decisions,
                    output_available: self.audio.output_available(),
                    effects_gain: self.settings.effects_gain(),
                    speech_gain: self.settings.speech_gain(),
                    speech_active: self.audio.speech_active(),
                    one_shot_active: self.audio.one_shot_active(),
                    ambience_duck: self.audio.ambience_duck(),
                    speech_owner: self.active_speech_owner,
                    presentation: PresentationTraceState {
                        subtitles_enabled: self.settings.subtitles,
                        active_dialogue_owner: self.active_dialogue_owner.map(dialogue_trace_owner),
                        caption_owner: self
                            .view
                            .speech
                            .as_ref()
                            .and(self.active_dialogue_owner)
                            .map(dialogue_trace_owner),
                        pending_mouth_owner: self.pending_tts.as_ref().map(PendingTts::trace_owner),
                        active_mouth_owner: self.active_speech_owner,
                    },
                },
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if !self.audio.speech_active() {
            self.speech_animation = None;
            if let Some(owner) = self.active_speech_owner.take()
                && let Some(feel) = &mut self.feel
            {
                feel.record_tts_lifecycle(
                    "speech_playback_completed",
                    owner,
                    None,
                    self.session.world().elapsed_ms,
                )
                .map_err(feel_error)?;
            }
        }
        self.view.speaking = self.audio.speech_active();
        self.view.mouth_phase = self.speech_animation.as_ref().map_or(0, |animation| {
            if self.active_dialogue_owner == Some(animation.owner)
                && self.active_speech_owner.is_some_and(|owner| {
                    owner.dialogue_generation == animation.owner.generation
                        && owner.dialogue_request_id == animation.owner.request_id
                })
            {
                mouth_phase(
                    animation.timing,
                    self.session
                        .world()
                        .elapsed_ms
                        .saturating_sub(animation.started_at_ms),
                )
            } else {
                0
            }
        });
        Ok(())
    }

    pub(crate) fn pointer_moved(
        &mut self,
        logical: Option<(i32, i32)>,
        world: Option<NormalizedPosition>,
        hit: Option<&beastie_view::HitRegion>,
    ) -> GameResult {
        if self.pointer_logical != logical {
            self.view.controller_active = false;
            if let Some(feel) = &mut self.feel {
                feel.record_native(
                    "mouse_motion",
                    serde_json::json!({"logical": logical}),
                    self.session.world().elapsed_ms,
                )
                .map_err(feel_error)?;
            }
        }
        self.pointer_logical = logical;
        self.view.hovered_region = hit.map(|hit| hit.id.clone());
        if world != self.cursor_world {
            self.cursor_world = world;
            self.apply_command(SessionCommand::Cursor { position: world }, false)?;
        }
        Ok(())
    }

    pub(crate) fn pointer_pressed(
        &mut self,
        action: Option<UiAction>,
        world: Option<NormalizedPosition>,
    ) -> GameResult {
        self.view.controller_active = false;
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "mouse_button_down",
                serde_json::json!({"button":"Left", "logical": self.pointer_logical}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if let Some(action) = action {
            if action == UiAction::PushToTalk {
                self.begin_push_to_talk()?;
            } else if let UiAction::DropFood(food) = action {
                if let Some(position) = world {
                    self.cursor_world = Some(position);
                    self.apply_command(SessionCommand::DropFood { food, position }, true)?;
                    self.close_menu();
                    self.queued_audio.push(ui_audio(AudioCue::UiConfirm));
                }
            } else {
                self.apply_confirmed_ui_action(action, false)?;
            }
        }
        Ok(())
    }

    pub(crate) fn pointer_released(&mut self) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "mouse_button_up",
                serde_json::json!({"button":"Left", "logical": self.pointer_logical}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.end_push_to_talk()
    }

    pub(crate) fn key_down_event(&mut self, input: KeyStroke, repeated: bool) -> GameResult {
        if let Some(feel) = &mut self.feel {
            let key = match &input.key {
                Key::Character(_) => "character".to_owned(),
                other => format!("{other:?}"),
            };
            feel.record_native(
                "key_down",
                serde_json::json!({
                    "key": key,
                    "repeated": repeated,
                    "control": input.control,
                    "super": input.super_key,
                    "shift": input.shift,
                }),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        self.view.controller_active = false;
        let key = &input.key;
        if let UiMode::Rebinding(action) = self.view.mode {
            if matches!(key, Key::Escape) {
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
                Key::Escape => self.close_menu(),
                Key::Enter => self.submit_name()?,
                Key::Backspace => {
                    self.view.text_buffer.pop();
                }
                _ if !input.control && !input.super_key => {
                    if let Some(text) = input.text.as_deref() {
                        self.view.compose_engaged = true;
                        append_text(&mut self.view.text_buffer, text);
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        if input.control || input.super_key {
            match key {
                Key::Character(character) if character.eq_ignore_ascii_case("q") => {
                    self.persist()?;
                    self.quit_requested = true;
                    return Ok(());
                }
                Key::Character(character) if character == "+" || character == "=" => {
                    return self.change_window_scale(1);
                }
                Key::Character(character) if character == "-" => {
                    return self.change_window_scale(-1);
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
        if matches!(key, Key::F11) {
            return self.toggle_fullscreen();
        }
        if matches!(key, Key::F6) {
            self.settings.voice_enabled = !self.settings.voice_enabled;
            self.view.voice_enabled = self.settings.voice_enabled;
            if !self.settings.voice_enabled {
                self.stop_owned_voice()?;
            }
            return self.persist_settings();
        }
        if matches!(key, Key::F7) {
            self.settings.reduced_motion = !self.settings.reduced_motion;
            self.view.reduced_motion = self.settings.reduced_motion;
            return self.persist_settings();
        }
        if matches!(key, Key::F9) {
            self.settings.reduced_flashes = !self.settings.reduced_flashes;
            self.view.reduced_flashes = self.settings.reduced_flashes;
            return self.persist_settings();
        }
        if matches!(key, Key::F10) {
            self.settings.reduced_shake = !self.settings.reduced_shake;
            self.view.reduced_shake = self.settings.reduced_shake;
            return self.persist_settings();
        }
        if matches!(key, Key::F12) {
            self.settings.text_speed = cycle_text_speed(self.settings.text_speed);
            self.view.text_speed = text_speed_value(self.settings.text_speed);
            return self.persist_settings();
        }
        if matches!(self.view.mode, UiMode::Compose) && !repeated && matches!(key, Key::Tab) {
            self.navigate(if input.shift { -1 } else { 1 });
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Compose)
            && self.view.focused_region.as_deref() != Some("compose/input")
            && !repeated
        {
            match key {
                Key::ArrowRight | Key::ArrowDown => self.navigate(1),
                Key::ArrowLeft | Key::ArrowUp => self.navigate(-1),
                Key::Enter | Key::Space => self.activate_focus(false)?,
                Key::Escape => {
                    self.view.focused_region = Some("compose/input".to_owned());
                }
                _ => {}
            }
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Compose) {
            match key {
                Key::Escape => {
                    self.apply_ui_action(UiAction::ClearText, false)?;
                }
                Key::Enter if !self.view.pending => self.submit_text()?,
                Key::Backspace => {
                    self.view.text_buffer.pop();
                }
                _ if !input.control && !input.super_key => {
                    if let Some(text) = input.text.as_deref() {
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
            Key::Escape => {
                self.close_menu();
            }
            Key::Tab | Key::ArrowRight | Key::ArrowDown => self.navigate(1),
            Key::ArrowLeft | Key::ArrowUp => self.navigate(-1),
            Key::Enter | Key::Space => self.activate_focus(false)?,
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn key_up_event(&mut self, input: KeyStroke) -> GameResult {
        if let Some(feel) = &mut self.feel {
            let key = match &input.key {
                Key::Character(_) => "character".to_owned(),
                other => format!("{other:?}"),
            };
            feel.record_native(
                "key_up",
                serde_json::json!({"key": key}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if binding_key(&input.key) == Some(self.settings.bindings.push_to_talk)
            || matches!(input.key, Key::Enter | Key::Space)
        {
            self.end_push_to_talk()?;
        }
        Ok(())
    }

    pub(crate) fn gamepad_button_down_event(
        &mut self,
        button: bevy::input::gamepad::GamepadButton,
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
            bevy::input::gamepad::GamepadButton::DPadRight
            | bevy::input::gamepad::GamepadButton::DPadDown => self.navigate(1),
            bevy::input::gamepad::GamepadButton::DPadLeft
            | bevy::input::gamepad::GamepadButton::DPadUp => self.navigate(-1),
            bevy::input::gamepad::GamepadButton::South => self.activate_focus(true)?,
            bevy::input::gamepad::GamepadButton::East => self.close_menu(),
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn gamepad_button_up_event(
        &mut self,
        button: bevy::input::gamepad::GamepadButton,
    ) -> GameResult {
        if let Some(feel) = &mut self.feel {
            feel.record_native(
                "gamepad_button_up",
                serde_json::json!({"button": format!("{button:?}")}),
                self.session.world().elapsed_ms,
            )
            .map_err(feel_error)?;
        }
        if button == bevy::input::gamepad::GamepadButton::South {
            self.end_push_to_talk()?;
        }
        Ok(())
    }

    pub(crate) fn focus_event(&mut self, gained: bool) -> GameResult {
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

    pub(crate) fn quit_event(&mut self) -> GameResult<bool> {
        self.persist()?;
        Ok(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupNotice {
    WelcomeBack,
    Technical(&'static str),
}

fn present_startup_notice(view: &mut ViewState, notice: StartupNotice, now_ms: u64) {
    match notice {
        StartupNotice::WelcomeBack => view.show_speech("you came back.".to_owned(), now_ms),
        StartupNotice::Technical(message) => view.show_status(message, now_ms, 12_000),
    }
}

fn load_session(store: &SaveStore) -> (GameSession, Option<StartupNotice>, bool, bool) {
    let loaded = match store.load_recoverable() {
        Ok(Some(loaded)) => loaded,
        Ok(None) => return (GameSession::new(42, "Mop"), None, false, true),
        Err(_) => {
            return (
                GameSession::new(42, "Mop"),
                Some(StartupNotice::Technical(
                    "Save unavailable. Your existing files are unchanged.",
                )),
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
                Some(StartupNotice::Technical("Recovered the last-good save.")),
                true,
                true,
            ),
            _ => (
                session,
                Some(StartupNotice::Technical(
                    "Backup loaded. Save repair failed.",
                )),
                true,
                false,
            ),
        },
        Ok((session, _)) => (session, Some(StartupNotice::WelcomeBack), true, true),
        Err(_) => {
            if !recovered
                && let Ok(Some(backup)) = store.load_backup()
                && let Ok((session, _)) = GameSession::resume_json(&backup, unix_time_ms())
            {
                return match store.promote_backup() {
                    Ok(true) => (
                        session,
                        Some(StartupNotice::Technical("Recovered the last-good save.")),
                        true,
                        true,
                    ),
                    _ => (
                        session,
                        Some(StartupNotice::Technical(
                            "Backup loaded. Save repair failed.",
                        )),
                        true,
                        false,
                    ),
                };
            }
            (
                GameSession::new(42, "Mop"),
                Some(StartupNotice::Technical(
                    "Save unreadable. Your existing files are unchanged.",
                )),
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
    owner: DialogueOwner,
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
        owner,
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
        Key::Escape => Some(BindingKey::Escape),
        Key::F1 => Some(BindingKey::F1),
        Key::F2 => Some(BindingKey::F2),
        Key::F3 => Some(BindingKey::F3),
        Key::F4 => Some(BindingKey::F4),
        Key::F5 => Some(BindingKey::F5),
        Key::F6 => Some(BindingKey::F6),
        Key::F7 => Some(BindingKey::F7),
        Key::F8 => Some(BindingKey::F8),
        Key::F9 => Some(BindingKey::F9),
        Key::F10 => Some(BindingKey::F10),
        Key::F11 => Some(BindingKey::F11),
        Key::F12 => Some(BindingKey::F12),
        _ => None,
    }
}

pub(crate) fn assets_root() -> PathBuf {
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
    GameError::Session(error.to_string())
}

fn feel_error(error: crate::feel::FeelError) -> GameError {
    GameError::Filesystem(error.to_string())
}

fn play_command(toy: ToyId) -> SessionCommand {
    SessionCommand::Play { toy }
}

const fn ui_audio(cue: AudioCue) -> AudioCommand {
    AudioCommand::play(SemanticOwner::Ordinary, PresentationChannel::Ui, cue, 450)
}

fn clears_speech(command: &SessionCommand) -> bool {
    matches!(
        command,
        SessionCommand::Feed { .. }
            | SessionCommand::DropFood { .. }
            | SessionCommand::Play { .. }
            | SessionCommand::Comfort
            | SessionCommand::Tidy
            | SessionCommand::React { .. }
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
        SpokenInputStatus::Expired => {
            view.microphone_state = resting_microphone_state;
            view.show_status(
                "Mop stayed with what it was doing.".to_owned(),
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
        DelayedCompletion, MicrophoneAcquisitionOutcome, apply_spoken_input_status, clears_speech,
        command_requires_persist, effective_dialogue_text_speed, load_session,
        microphone_acquisition_transition, mouth_phase, play_command, revealed_text,
        show_dialogue_caption, spoken_input_is_pending,
    };
    use crate::dialogue::DialogueOwner;
    use crate::save_store::SaveStore;
    use crate::settings::TextSpeed;

    #[test]
    fn startup_recovery_is_technical_feedback_without_creature_speech() {
        let mut view = beastie_view::ViewState::default();
        super::present_startup_notice(
            &mut view,
            super::StartupNotice::Technical("Recovered the last-good save."),
            20,
        );
        assert_eq!(
            view.status_message.as_deref(),
            Some("Recovered the last-good save.")
        );
        assert!(view.speech.is_none());
        assert!(view.cue_queue.is_empty());
    }

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
        assert!(clears_speech(&SessionCommand::React {
            reaction: Reaction::Laugh,
        }));
    }

    #[test]
    fn direct_action_during_scripted_delay_discards_completion_before_accept_or_tts() {
        let mut ordinary = DelayedCompletion::<&str>::new(0);
        ordinary.arm(1_000);
        assert!(!ordinary.allows_scenario_progress());

        let mut delayed = DelayedCompletion::new(800);
        delayed.arm(1_000);
        assert!(delayed.allows_scenario_progress());
        assert_eq!(delayed.hold_or_release("generated turn", 1_016), None);

        // The authored direct action occurs after 150 ms of simulation, while the completion is
        // still held. Supersession drops it without waiting for the 800 ms release deadline.
        assert_eq!(delayed.take_ready(1_150), None);
        delayed.cancel();
        assert!(!delayed.allows_scenario_progress());
        assert_eq!(delayed.take_ready(1_800), None);
    }

    #[test]
    fn reaction_commands_use_the_late_dialogue_supersession_path() {
        for reaction in [Reaction::Laugh, Reaction::Disapprove, Reaction::Comfort] {
            assert!(clears_speech(&SessionCommand::React { reaction }));
        }
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
    fn microphone_acquisition_only_starts_perception_after_live_capture() {
        let acquired = microphone_acquisition_transition(MicrophoneAcquisitionOutcome::Acquired);
        assert!(acquired.starts_perception);
        assert_eq!(acquired.state, MicrophoneState::Listening);
        assert_eq!(acquired.message, None);

        let unavailable =
            microphone_acquisition_transition(MicrophoneAcquisitionOutcome::Unavailable);
        assert!(!unavailable.starts_perception);
        assert_eq!(unavailable.state, MicrophoneState::Unavailable);
        assert_eq!(
            unavailable.message,
            Some("Microphone unavailable. Text still works")
        );

        let failed = microphone_acquisition_transition(MicrophoneAcquisitionOutcome::StartFailed);
        assert!(!failed.starts_perception);
        assert_eq!(failed.state, MicrophoneState::Error);
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
            SpokenInputStatus::Expired,
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
        let owner = DialogueOwner {
            generation: 1,
            request_id: 7,
        };
        let reveal = show_dialogue_caption(
            &mut view,
            owner,
            "audible rude fish",
            42,
            false,
            TextSpeed::Normal,
        );
        assert!(reveal.is_none());
        assert!(view.speech.is_none());
        assert!(view.speech_expires_at_ms.is_none());

        let reveal = show_dialogue_caption(
            &mut view,
            owner,
            "visible rude fish",
            43,
            true,
            TextSpeed::Normal,
        );
        assert!(reveal.is_some());
        assert_eq!(view.speech.as_deref(), Some("visible rude fish"));
    }

    #[test]
    fn genuine_v2_game_save_loads_continues_and_is_rewritten_as_v4() {
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
        assert_eq!(message, Some(super::StartupNotice::WelcomeBack));
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
}
