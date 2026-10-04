use std::path::{Path, PathBuf};
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
    SPEECH_RELEASE_MS, ScenePlan, SemanticOwner, UiAction, UiMode, UiTarget, ViewState, plan,
};
use bevy::input::keyboard::Key;
use unicode_segmentation::UnicodeSegmentation;

use crate::args::Args;
use crate::audio::{AmbientBubbleSchedule, AudioBank, sound_for_cue};
use crate::dialogue::{DialogueManager, DialogueOwner, DialogueTurn, WorkerConfig};
use crate::feel::{
    AudioTraceFrame, DialogueHealthTrace, DialogueTraceOwner, FeelRecorder, PresentationTraceState,
    SpeechTraceOwner,
};
use crate::input::{
    MAX_NAME_CHARACTERS, PasteOwner, TextEditTracker, append_bounded_text, append_text,
    delete_view_text, focused_action, focused_text_field, move_focus,
};
use crate::microphone::{MicrophoneCapture, MicrophoneError, PrivateAudioRoot};
use crate::recognition::{RecognitionManager, RecognitionWorkerConfig, speech_failure};
use crate::save_store::{BackupSource, LoadedSave, SaveStore};
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
    owner: SpeechTraceOwner,
    timing: MouthTiming,
}

impl SpeechAnimation {
    fn phase(
        &self,
        active_dialogue_owner: Option<DialogueOwner>,
        active_speech_owner: Option<SpeechTraceOwner>,
        audio: &AudioBank,
    ) -> u8 {
        if active_dialogue_owner
            != Some(DialogueOwner {
                generation: self.owner.dialogue_generation,
                request_id: self.owner.dialogue_request_id,
            })
        {
            return 0;
        }
        let Some(owner) = active_speech_owner else {
            return 0;
        };
        if owner.dialogue_generation != self.owner.dialogue_generation
            || owner.dialogue_request_id != self.owner.dialogue_request_id
            || owner.tts_request_id != self.owner.tts_request_id
        {
            return 0;
        }
        // Playback advances between simulation ticks and stops owning the mouth as soon as
        // its source finishes or is interrupted. No world-clock estimate can preserve both.
        audio.speech_position(owner).map_or(0, |position| {
            mouth_phase(
                self.timing,
                u64::try_from(position.as_millis()).unwrap_or(u64::MAX),
            )
        })
    }
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
    text_edits: TextEditTracker,
    paste_requested: Option<PasteOwner>,
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
    settings_from_title: bool,
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
        let config_dir = local_data_root(args)?;
        let save_store = SaveStore::new(config_dir.join("saves").join("main.json"));
        let settings_store = SettingsStore::new(config_dir.join("settings.json"));
        let (settings, settings_message) = if args.script.is_some() {
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
                (
                    GameSession::first_meeting(fresh_seed(), "Mop"),
                    None,
                    false,
                    true,
                )
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
            mode: if args.script.is_some() {
                UiMode::Compose
            } else {
                UiMode::Title
            },
            focused_region: Some(
                if args.script.is_some() {
                    "compose/input"
                } else {
                    "title/continue"
                }
                .to_owned(),
            ),
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
            text_edits: TextEditTracker::default(),
            paste_requested: None,
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
            settings_from_title: false,
            quit_requested: false,
            failed: false,
            frame_pending: true,
            capture_outstanding: 0,
        };
        if let Some(destination) = &args.export_transcript {
            game.transcripts
                .export(&transcript_export_destination(
                    args,
                    &config_dir,
                    destination,
                )?)
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
            motion: None,
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
                    snapshot.motion.as_ref(),
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
        let saved_at_ms = if self.scenario.is_some() {
            self.session.world().elapsed_ms
        } else {
            unix_time_ms()
        };
        let save = self
            .session
            .capture(saved_at_ms)
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
            self.view.show_status(
                format!(
                    "Microphone is off. Enable it in Settings ({}) > Sound.",
                    self.view.binding_labels.settings
                ),
                self.session.world().elapsed_ms,
                5_000,
            );
            return Ok(());
        }
        if self.spoken_turn_pending {
            self.view.show_status(
                "Already heard you. Waiting for a good moment...",
                self.session.world().elapsed_ms,
                5_000,
            );
            return Ok(());
        }
        if self.recognition.is_pending() {
            self.view.show_status(
                "Still recognizing what you said...",
                self.session.world().elapsed_ms,
                5_000,
            );
            return Ok(());
        }
        if self.dialogue.is_pending() {
            self.view.show_status(
                "A reply is still forming. You can keep caring for your Beastie.",
                self.session.world().elapsed_ms,
                5_000,
            );
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
        self.view.status_expires_at_ms = None;
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
                            self.view.clear_status();
                            self.view.microphone_state = self.resting_microphone_state();
                        }
                    }
                    RecognitionOutcome::NoSpeech {} => {
                        self.apply_command(SessionCommand::SpeechEnded, false)?;
                        self.view.show_status(
                            format!(
                                "No speech heard. Hold {} and try again.",
                                self.view.binding_labels.push_to_talk
                            ),
                            self.session.world().elapsed_ms,
                            5_000,
                        );
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
                        self.view.show_status(
                            if unavailable {
                                "Speech recognition unavailable. Text still works.".to_owned()
                            } else {
                                format!(
                                    "Speech recognition failed. Hold {} to try again.",
                                    self.view.binding_labels.push_to_talk
                                )
                            },
                            self.session.world().elapsed_ms,
                            5_000,
                        );
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
        // Without a local model the creature speaks through its own composed lines by design;
        // only a model that is present but failing deserves a technical notice.
        let designed_voice = turn.request.speech_intent.is_some()
            && matches!(
                turn.fallback_reason,
                None | Some(beastie_protocol::DialogueFallbackReason::WorkerUnavailable)
            );
        if turn.fallback && !designed_voice {
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
        focus_after_dialogue(&mut self.view);
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
                        owner: trace_owner,
                        timing: pending.mouth_timing,
                    });
                }
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        Ok(())
    }

    fn apply_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        self.text_edits.sync(&mut self.view);
        let result = self.handle_ui_action(action, controller);
        self.text_edits.sync(&mut self.view);
        result
    }

    fn handle_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        let was_renaming = name_entry_active(&self.view);
        mark_pressed_action(
            &self.render_plan(),
            &mut self.view,
            action,
            self.session.world().elapsed_ms,
        );
        if action != UiAction::DismissStatus {
            self.view.compose_engaged = matches!(
                action,
                UiAction::FocusCompose | UiAction::Talk | UiAction::TypeCharacter(_)
            );
        }
        if matches!(
            action,
            UiAction::OpenContext(_)
                | UiAction::OpenFoodChoice
                | UiAction::OpenToyChoice
                | UiAction::SelectFood(_)
                | UiAction::Play(_)
                | UiAction::Comfort
                | UiAction::Inspect
                | UiAction::Talk
        ) {
            self.supersede_dialogue_turn()?;
        }
        match action {
            UiAction::OpenTitle => {
                self.end_push_to_talk()?;
                self.settings_from_title = false;
                self.view.mode = UiMode::Title;
                self.view.focused_region = Some("title/continue".to_owned());
            }
            UiAction::Continue => {
                self.settings_from_title = false;
                self.view.mode = UiMode::Compose;
                self.view.focused_region = Some("compose/input".to_owned());
            }
            UiAction::Quit => {
                self.persist()?;
                self.quit_requested = true;
            }
            UiAction::OpenContext(target) => {
                // Choose the clear region once. Swimming must not move controls under a pointer.
                self.view.context_card_anchor = match target {
                    UiTarget::Toy(toy) => Some(beastie_view::toy_context_anchor(
                        self.session.world(),
                        toy,
                        self.view.text_scale,
                    )),
                    _ => None,
                };
                self.view.context_above = Some(
                    beastie_view::world_to_logical(self.session.world().creature.aquarium.position)
                        .1
                        > 74,
                );
                self.view.mode = UiMode::Context(target);
                self.reset_focus();
            }
            UiAction::CloseContext => {
                self.view.context_card_anchor = None;
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
                // Food lands just in front of the creature's face so feeding pays off in about
                // a second, from either the rail or the controller food strip.
                let position = beastie_core::feeding_position(self.session.world());
                self.apply_command(SessionCommand::DropFood { food, position }, true)?;
                self.close_menu();
            }
            UiAction::TapWater => {
                // Only the pointer can tap a point in the water; it supplies the position.
                if let Some(position) = self.cursor_world {
                    self.apply_command(SessionCommand::Tap { position }, true)?;
                }
            }
            UiAction::Play(toy) => {
                self.apply_command(play_command(toy), true)?;
                self.close_menu();
            }
            UiAction::Inspect => {
                open_inspection(self.session.world(), &mut self.view);
                self.reset_focus();
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
                if controller {
                    self.view.renaming_with_osk |= self.view.mode == UiMode::Rename;
                    self.view.mode = UiMode::OnScreenKeyboard;
                    self.reset_focus();
                } else {
                    focus_text_input(&mut self.view);
                }
            }
            UiAction::TypeCharacter(character) => {
                append_view_text(
                    &mut self.view,
                    &character.to_string(),
                    self.session.world().elapsed_ms,
                );
            }
            UiAction::Backspace => {
                delete_view_text(&mut self.view, true);
            }
            UiAction::SubmitText => {
                if self.view.renaming_with_osk {
                    self.submit_name()?;
                } else {
                    self.submit_text()?;
                }
            }
            UiAction::ClearText => {
                self.view.text_buffer.clear();
                self.view.text_selected = false;
            }
            UiAction::CancelMode => {
                self.close_menu();
            }
            UiAction::OpenSettings => {
                self.settings_from_title |= matches!(self.view.mode, UiMode::Title);
                self.view.mode = UiMode::Settings;
                self.view.focused_region =
                    Some(format!("settings/page-{}", self.view.settings_page.min(2)));
            }
            UiAction::SelectSettingsPage(page) => {
                self.settings_from_title |= matches!(self.view.mode, UiMode::Title);
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
                self.view.show_status(
                    if self.settings.microphone_enabled {
                        "Mic on. Hold Speak; release to send."
                    } else {
                        "Microphone disabled"
                    },
                    self.session.world().elapsed_ms,
                    4_000,
                );
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
            // Speech bubbles are no longer paged; older scenario scripts may still ask.
            UiAction::ChangeSpeechPage(_) => {}
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
                self.view.show_status(
                    "Default keys restored",
                    self.session.world().elapsed_ms,
                    3_000,
                );
            }
            UiAction::Rename => {
                self.view.text_buffer.clear();
                self.view.renaming_with_osk = controller;
                self.view.mode = if controller {
                    UiMode::OnScreenKeyboard
                } else {
                    UiMode::Rename
                };
                if controller {
                    self.reset_focus();
                } else {
                    self.view.hovered_region = None;
                    self.view.focused_region = Some("rename/input".to_owned());
                    self.view.compose_engaged = true;
                }
            }
            UiAction::SubmitName => self.submit_name()?,
            UiAction::OpenDataManagement => {
                self.view.mode = UiMode::DataManagement;
                self.reset_focus();
            }
            UiAction::RecoverBackup => self.recover_backup()?,
            UiAction::RequestReset => {
                self.view.clear_status();
                self.view.mode = UiMode::ConfirmReset;
                self.reset_focus();
            }
            UiAction::ConfirmReset => self.reset_save()?,
            UiAction::ToggleTranscript => {
                self.settings.transcript_enabled = !self.view.transcript_enabled;
                self.view.transcript_enabled = self.settings.transcript_enabled;
                self.transcripts
                    .set_enabled(self.settings.transcript_enabled);
                let message = if self.settings.transcript_enabled {
                    "Recording safe local turns".to_owned()
                } else {
                    "Transcript recording off".to_owned()
                };
                self.view.transcript_status = None;
                self.view
                    .show_status(message, self.session.world().elapsed_ms, 12_000);
                self.persist_settings()?;
            }
            UiAction::ExportTranscript => {
                let message = match self.transcripts.export(&self.transcript_export_path) {
                    Ok(turns) => format!(
                        "Exported {turns} {} to {}",
                        if turns == 1 { "turn" } else { "turns" },
                        self.transcript_export_path.display()
                    ),
                    Err(error) => format!("Export failed: {error}"),
                };
                self.view.transcript_status = None;
                self.view
                    .show_status(message, self.session.world().elapsed_ms, 12_000);
            }
            UiAction::DismissStatus => {
                self.view.dismiss_status();
            }
        }
        finish_name_entry_transition(&mut self.view, was_renaming);
        Ok(())
    }

    fn submit_text(&mut self) -> GameResult {
        let text = self.view.text_buffer.trim().to_owned();
        if text.is_empty() || self.view.pending {
            return Ok(());
        }
        self.view.text_buffer.clear();
        self.apply_command(SessionCommand::Talk { text }, true)
    }

    fn submit_name(&mut self) -> GameResult {
        let name = self.view.text_buffer.trim().to_owned();
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARACTERS
            || name.chars().any(char::is_control)
        {
            self.view.show_status(
                "Enter a name using 1–24 characters.",
                self.session.world().elapsed_ms,
                5_000,
            );
            if !self.view.renaming_with_osk {
                focus_text_input(&mut self.view);
            }
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
                self.view.show_status(
                    format!("Name set to {name}"),
                    self.session.world().elapsed_ms,
                    4_000,
                );
                self.close_menu();
            }
            Err(error) => {
                self.view.show_status(
                    format!("Couldn't use that name: {error}"),
                    self.session.world().elapsed_ms,
                    8_000,
                );
            }
        }
        Ok(())
    }

    fn recover_backup(&mut self) -> GameResult {
        self.supersede_dialogue_turn()?;
        let result =
            recover_saved_session(&self.save_store, self.scenario.is_none().then(unix_time_ms));
        match result {
            Ok((session, source)) => {
                self.session = session;
                self.save_enabled = true;
                self.reset_world_presentation();
                self.view.show_status(
                    if source == BackupSource::LastGood {
                        "Recovered the last-good save."
                    } else {
                        "Recovered the creature from before reset."
                    },
                    self.session.world().elapsed_ms,
                    8_000,
                );
            }
            Err(error) => {
                self.view.show_status(
                    format!("Recovery failed: {error}. Current creature unchanged."),
                    self.session.world().elapsed_ms,
                    12_000,
                );
            }
        }
        self.view.mode = UiMode::DataManagement;
        self.reset_focus();
        Ok(())
    }

    fn reset_save(&mut self) -> GameResult {
        self.supersede_dialogue_turn()?;
        match self.save_store.reset() {
            Ok(preserved) => {
                self.session = GameSession::first_meeting(fresh_seed(), "Mop");
                self.save_enabled = true;
                self.reset_world_presentation();
                self.view.show_status(
                    if preserved {
                        "New creature ready. Recover backup restores the previous save."
                    } else {
                        "New creature ready. No previous save was present."
                    },
                    self.session.world().elapsed_ms,
                    8_000,
                );
                self.view.text_buffer.clear();
            }
            Err(error) => {
                self.view.show_status(
                    format!("Reset failed: {error}"),
                    self.session.world().elapsed_ms,
                    12_000,
                );
            }
        }
        self.view.mode = UiMode::DataManagement;
        self.reset_focus();
        Ok(())
    }

    fn reset_world_presentation(&mut self) {
        // A recovered/reset world may have an earlier clock. Presentation deadlines and
        // cues from the displaced creature must not linger in the replacement session.
        self.view.cue_queue.clear();
        self.queued_audio.clear();
        // Drop live capture and cancel work owned by the displaced creature. Merely
        // invalidating its ID would leave new push-to-talk blocked on the old worker.
        self.microphone = None;
        self.active_recognition = None;
        self.recognition.cancel_pending();
        self.spoken_turn_pending = false;
        self.view.microphone_state = self.resting_microphone_state();
        self.view.pressed_region = None;
        self.view.pressed_until_ms = 0;
        self.view.context_card_anchor = None;
        self.view.context_above = None;
        self.view.text_buffer.clear();
        self.view.renaming_with_osk = false;
        self.view.compose_engaged = false;
        self.bubble_schedule =
            AmbientBubbleSchedule::new(self.session.world().seed, self.session.world().elapsed_ms);
    }

    fn apply_confirmed_ui_action(&mut self, action: UiAction, controller: bool) -> GameResult {
        self.apply_ui_action(action, controller)?;
        self.queued_audio.push(ui_audio(AudioCue::UiConfirm));
        Ok(())
    }

    fn close_menu(&mut self) {
        self.text_edits.invalidate(&mut self.view);
        if let UiMode::Rebinding(action) = self.view.mode {
            finish_rebinding(
                self.session.world(),
                &mut self.view,
                &mut self.settings.bindings,
                action,
                None,
            );
            return;
        }
        if self.view.mode == UiMode::Rename || self.view.renaming_with_osk {
            self.view.text_buffer.clear();
        }
        self.view.context_card_anchor = None;
        self.view.renaming_with_osk = false;
        self.view.compose_engaged = false;
        self.view.hovered_region = None;
        if self.settings_from_title || matches!(self.view.mode, UiMode::Title) {
            self.settings_from_title = false;
            self.view.mode = UiMode::Title;
            self.view.focused_region = Some("title/continue".to_owned());
            return;
        }
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
        if matches!(self.view.mode, UiMode::Title)
            && !matches!(action, BindableAction::Settings | BindableAction::Cancel)
        {
            return Ok(());
        }
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
        let swapped = self
            .settings
            .bindings
            .action_for(key)
            .is_some_and(|bound| bound != action);
        finish_rebinding(
            self.session.world(),
            &mut self.view,
            &mut self.settings.bindings,
            action,
            Some(key),
        );
        self.view.show_status(
            if swapped {
                "Key updated. The previous binding was swapped."
            } else {
                "Key updated."
            },
            self.session.world().elapsed_ms,
            4_000,
        );
        self.persist_settings()
    }

    fn reset_focus(&mut self) {
        self.view.focused_region = None;
        let plan = self.render_plan();
        self.view.focused_region = move_focus(&plan, None, 1);
    }

    fn navigate(&mut self, delta: i32) {
        self.text_edits.sync(&mut self.view);
        self.view.hovered_region = None;
        let plan = self.render_plan();
        let previous = self.view.focused_region.clone();
        self.view.focused_region = move_focus(&plan, self.view.focused_region.as_deref(), delta);
        self.view.compose_engaged = matches!(
            self.view.focused_region.as_deref(),
            Some("compose/input" | "rename/input")
        );
        if self.view.focused_region != previous {
            self.queued_audio.push(ui_audio(AudioCue::UiReject));
        }
        self.text_edits.sync(&mut self.view);
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
            ScenarioStep::TextInput(text) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_native(
                        "scenario_setting",
                        serde_json::json!({
                            "text_input_characters": text.chars().count()
                        }),
                        self.session.world().elapsed_ms,
                    )
                    .map_err(feel_error)?;
                }
                if matches!(
                    self.view.mode,
                    UiMode::Compose | UiMode::Rename | UiMode::OnScreenKeyboard
                ) {
                    append_view_text(&mut self.view, &text, self.session.world().elapsed_ms);
                }
            }
            ScenarioStep::UiPreview(preview) => {
                if let Some(feel) = &mut self.feel {
                    feel.record_native("scenario_setting", serde_json::json!({
                        "presentation_only_preview": true,
                        "microphone_state": preview.microphone,
                        "caption_characters": preview.caption.as_ref().map(|text| text.chars().count()),
                        "status_characters": preview.status.as_ref().map(|text| text.chars().count()),
                    }), self.session.world().elapsed_ms).map_err(feel_error)?;
                }
                // Preview data is never sent to GameSession and must not be confused with a
                // speech acquisition, recognized transcript or real dialogue completion.
                if preview.clear {
                    self.view.microphone_enabled = self.settings.microphone_enabled;
                    self.view.microphone_state = self.resting_microphone_state();
                }
                preview.apply(&mut self.view, self.session.world().elapsed_ms);
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
        self.text_edits.sync(&mut self.view);
        let result = self.update_frame(frame_delta_ms, advance_frame);
        self.text_edits.sync(&mut self.view);
        result
    }

    fn update_frame(&mut self, frame_delta_ms: u64, advance_frame: bool) -> GameResult {
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
            animation.phase(
                self.active_dialogue_owner,
                self.active_speech_owner,
                &self.audio,
            )
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
        if matches!(self.view.mode, UiMode::Title)
            && let Some(hit) = hit.filter(|hit| hit.id.starts_with("title/"))
        {
            self.view.focused_region = Some(hit.id.clone());
        }
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
                self.apply_ui_action(action, false)?;
            } else if action == UiAction::TapWater {
                if let Some(position) = world {
                    self.apply_command(SessionCommand::Tap { position }, true)?;
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
        self.text_edits.sync(&mut self.view);
        let result = self.handle_key_down_event(input, repeated);
        self.text_edits.sync(&mut self.view);
        result
    }

    fn handle_key_down_event(&mut self, input: KeyStroke, repeated: bool) -> GameResult {
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
        if repeated {
            repeat_text_input(&mut self.view, &input, self.session.world().elapsed_ms);
            return Ok(());
        }
        let key = &input.key;
        if input.control || input.super_key {
            match text_shortcut(&self.view, &input) {
                Some(TextShortcut::SelectAll) => {
                    self.view.text_selected = !self.view.text_buffer.is_empty();
                    self.view.compose_engaged = true;
                }
                Some(TextShortcut::Paste) => self.paste_requested = self.text_edits.owner(),
                None if self.view.mode != UiMode::Rename => match key {
                    Key::Character(character) if character.eq_ignore_ascii_case("q") => {
                        self.persist()?;
                        self.quit_requested = true;
                    }
                    Key::Character(character) if character == "+" || character == "=" => {
                        return self.change_window_scale(1);
                    }
                    Key::Character(character) if character == "-" => {
                        return self.change_window_scale(-1);
                    }
                    _ => {}
                },
                None => {}
            }
            // Unsupported modified keys cannot become literal input or gameplay bindings.
            return Ok(());
        }
        if let UiMode::Rebinding(action) = self.view.mode {
            if matches!(key, Key::Escape) {
                finish_rebinding(
                    self.session.world(),
                    &mut self.view,
                    &mut self.settings.bindings,
                    action,
                    None,
                );
                return Ok(());
            }
            if let Some(binding) = binding_key(key) {
                self.rebind(action, binding)?;
            }
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Rename) {
            let editing = self.view.focused_region.as_deref() == Some("rename/input");
            match key {
                Key::Escape => self.close_menu(),
                Key::Tab => self.navigate(tab_direction(input.shift)),
                Key::Enter if editing => {
                    self.apply_confirmed_ui_action(UiAction::SubmitName, false)?
                }
                Key::Enter | Key::Space if !editing => self.activate_focus(false)?,
                Key::ArrowRight | Key::ArrowDown if !editing => self.navigate(1),
                Key::ArrowLeft | Key::ArrowUp if !editing => self.navigate(-1),
                Key::Backspace | Key::Delete if editing => {
                    delete_view_text(&mut self.view, *key == Key::Backspace);
                }
                _ if editing && !input.control && !input.super_key => {
                    if let Some(text) = input.text.as_deref() {
                        append_view_text(&mut self.view, text, self.session.world().elapsed_ms);
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        if let Some(binding) = binding_key(key)
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
        if matches!(self.view.mode, UiMode::Compose) && matches!(key, Key::Tab) {
            self.navigate(tab_direction(input.shift));
            return Ok(());
        }
        if matches!(self.view.mode, UiMode::Compose)
            && self.view.focused_region.as_deref() != Some("compose/input")
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
                Key::Enter if !self.view.pending => {
                    self.apply_confirmed_ui_action(UiAction::SubmitText, false)?
                }
                Key::Backspace | Key::Delete => {
                    delete_view_text(&mut self.view, *key == Key::Backspace);
                }
                _ if !input.control && !input.super_key => {
                    if let Some(text) = input.text.as_deref() {
                        append_view_text(&mut self.view, text, self.session.world().elapsed_ms);
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        match key {
            Key::Escape => {
                self.close_menu();
            }
            Key::Tab => self.navigate(tab_direction(input.shift)),
            Key::ArrowRight | Key::ArrowDown => self.navigate(1),
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
            self.text_edits.invalidate(&mut self.view);
            self.paste_requested = None;
            self.end_push_to_talk()?;
            self.persist()?;
        }
        Ok(())
    }

    pub(crate) fn take_paste_request(&mut self) -> Option<PasteOwner> {
        self.paste_requested.take()
    }

    pub(crate) fn apply_clipboard_text(&mut self, owner: PasteOwner, text: Result<String, ()>) {
        apply_clipboard_result(
            &mut self.view,
            &mut self.text_edits,
            owner,
            text,
            self.session.world().elapsed_ms,
        );
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

/// Each new creature is its own: the shell injects a wall-clock seed, and the simulation stays
/// deterministic from there.
fn fresh_seed() -> u64 {
    let mut seed = unix_time_ms() ^ (u64::from(std::process::id()) << 32);
    seed ^= seed >> 33;
    seed = seed.wrapping_mul(0xff51_afd7_ed55_8ccd);
    seed ^ (seed >> 33)
}

fn load_session(store: &SaveStore) -> (GameSession, Option<StartupNotice>, bool, bool) {
    let loaded = match store.load_recoverable() {
        Ok(Some(loaded)) => loaded,
        Ok(None) => {
            return (
                GameSession::first_meeting(fresh_seed(), "Mop"),
                None,
                false,
                true,
            );
        }
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
    let text = full_text.graphemes(true).take(visible).collect::<String>();
    (text, visible >= full_text.graphemes(true).count())
}

fn focus_after_dialogue(view: &mut ViewState) {
    // A reply never takes the keyboard: the player can always keep talking.
    if view.mode == UiMode::Compose {
        view.focused_region = Some("compose/input".to_owned());
    }
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

fn finish_rebinding(
    world: &beastie_core::WorldState,
    view: &mut ViewState,
    bindings: &mut KeyBindings,
    action: BindableAction,
    key: Option<BindingKey>,
) {
    if let Some(key) = key {
        bindings.rebind_swapping(action, key);
    }
    view.binding_labels = binding_labels(bindings);
    view.mode = UiMode::Bindings;
    // Recover the row from its semantic action, including after pointer-opened capture.
    view.focused_region = plan(world, view)
        .0
        .hit_regions
        .into_iter()
        .find(|hit| hit.enabled && hit.action == UiAction::BeginRebind(action))
        .map(|hit| hit.id);
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

/// Every scripted side effect belongs to the evidence bundle, including explicit reset,
/// recovery, settings and transcript actions. Native play retains its platform directory.
fn local_data_root(args: &Args) -> GameResult<PathBuf> {
    if args.script.is_some() {
        return Ok(args
            .feel_dir
            .as_deref()
            .or(args.capture_dir.as_deref())
            .unwrap_or_else(|| Path::new("target/captures/aquarium-shell"))
            .join(".local-data"));
    }
    directories::ProjectDirs::from("", "Michael Thiesen", "beastie")
        .map(|directories| directories.config_dir().to_owned())
        .ok_or_else(|| GameError::Config("No local configuration directory".to_owned()))
}

fn recover_saved_session(
    store: &SaveStore,
    now_ms: Option<u64>,
) -> std::io::Result<(GameSession, BackupSource)> {
    let mut failure = std::io::Error::new(std::io::ErrorKind::NotFound, "no backup exists");
    let mut pending_reset = false;
    for source in [
        BackupSource::BeforeReset,
        BackupSource::LastGoodBeforeReset,
        BackupSource::LastGood,
    ] {
        // A sibling last-good copy belongs only to its pending reset. Once the reset is
        // recovered, later Recover actions must use the ordinary automatic backup again.
        if source == BackupSource::LastGoodBeforeReset && !pending_reset {
            continue;
        }
        let bytes = match store.load_backup_source(source) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(error) => {
                // NotFound is normalized to None by SaveStore. Any other read failure
                // still belongs to a pending reset whose independent sibling may work.
                pending_reset |= source == BackupSource::BeforeReset;
                failure = error;
                continue;
            }
        };
        pending_reset |= source == BackupSource::BeforeReset;
        let session = match SessionSave::from_json(&bytes).and_then(|save| {
            // Captures replay only authored simulation time, regardless of host load.
            let resume_at_ms = now_ms.unwrap_or(save.saved_at_ms);
            GameSession::resume(save, resume_at_ms)
        }) {
            Ok((session, _)) => session,
            Err(error) => {
                failure = std::io::Error::other(error);
                continue;
            }
        };
        if !store.promote_backup_source(source)? {
            return Err(std::io::Error::other("backup disappeared"));
        }
        return Ok((session, source));
    }
    Err(failure)
}

fn transcript_export_destination(
    args: &Args,
    root: &Path,
    requested: &Path,
) -> GameResult<PathBuf> {
    if args.script.is_none() {
        return Ok(requested.to_owned());
    }
    let name = requested
        .file_name()
        .ok_or_else(|| GameError::Config("Transcript export needs a file name".to_owned()))?;
    Ok(root.join("exports").join(name))
}

fn open_inspection(world: &beastie_core::WorldState, view: &mut ViewState) {
    let target = match view.mode {
        UiMode::Context(target) | UiMode::Inspect(target) => target,
        _ => UiTarget::Creature,
    };
    view.mode = UiMode::Inspect(target);
    let head_x = beastie_view::world_to_logical(world.creature.aquarium.position).0;
    view.context_card_anchor = Some((if head_x >= 160 { 7 } else { 173 }, 31));
    view.hovered_region = None;
    view.focused_region = None;
}

fn focus_text_input(view: &mut ViewState) {
    view.text_selected = false;
    let region = if view.mode == UiMode::Rename {
        "rename/input"
    } else {
        view.mode = UiMode::Compose;
        // Clicking the message field leaves the controller's separate naming flow.
        view.renaming_with_osk = false;
        "compose/input"
    };
    view.focused_region = Some(region.to_owned());
    view.compose_engaged = true;
}

fn append_view_text(view: &mut ViewState, text: &str, now_ms: u64) {
    if !text.chars().any(|character| !character.is_control()) {
        return;
    }
    view.compose_engaged = true;
    if view.text_selected {
        view.text_buffer.clear();
        view.text_selected = false;
    }
    if matches!(view.mode, UiMode::Rename) || view.renaming_with_osk {
        if append_bounded_text(&mut view.text_buffer, text, MAX_NAME_CHARACTERS) {
            view.show_status("Names can be up to 24 characters.", now_ms, 4_000);
        }
    } else {
        append_text(&mut view.text_buffer, text);
    }
}

fn repeat_text_input(view: &mut ViewState, input: &KeyStroke, now_ms: u64) {
    let editing = focused_text_field(view).is_some();
    if !editing || input.control || input.super_key {
        return;
    }
    match input.key {
        Key::Backspace | Key::Delete => delete_view_text(view, input.key == Key::Backspace),
        Key::Character(_) | Key::Space => {
            if let Some(text) = input.text.as_deref() {
                append_view_text(view, text, now_ms);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextShortcut {
    SelectAll,
    Paste,
}

fn text_shortcut(view: &ViewState, input: &KeyStroke) -> Option<TextShortcut> {
    let primary = if cfg!(target_os = "macos") {
        input.super_key && !input.control
    } else {
        input.control && !input.super_key
    };
    if !primary || focused_text_field(view).is_none() {
        return None;
    }
    let Key::Character(character) = &input.key else {
        return None;
    };
    match character.to_ascii_lowercase().as_str() {
        "a" => Some(TextShortcut::SelectAll),
        "v" => Some(TextShortcut::Paste),
        _ => None,
    }
}

fn apply_clipboard_result(
    view: &mut ViewState,
    edits: &mut TextEditTracker,
    owner: PasteOwner,
    text: Result<String, ()>,
    now_ms: u64,
) {
    edits.sync(view);
    if !edits.accepts(owner) {
        return;
    }
    match text {
        Ok(text) => append_view_text(view, &text, now_ms),
        Err(()) => view.show_status(
            "Clipboard unavailable. Your draft is unchanged.",
            now_ms,
            4_000,
        ),
    }
    edits.sync(view);
}

const fn tab_direction(shift: bool) -> i32 {
    if shift { -1 } else { 1 }
}

fn mark_pressed_action(plan: &ScenePlan, view: &mut ViewState, action: UiAction, now_ms: u64) {
    let matching = |hit: &&beastie_view::HitRegion| hit.enabled && hit.action == action;
    let hit = plan
        .hit_regions
        .iter()
        .filter(matching)
        .find(|hit| {
            view.hovered_region.as_deref() == Some(&hit.id)
                || view.focused_region.as_deref() == Some(&hit.id)
        })
        .or_else(|| plan.hit_regions.iter().find(matching));
    if let Some(hit) = hit {
        view.pressed_region = Some(hit.id.clone());
        view.pressed_until_ms = now_ms.saturating_add(100);
    }
}

fn name_entry_active(view: &ViewState) -> bool {
    view.mode == UiMode::Rename || (view.mode == UiMode::OnScreenKeyboard && view.renaming_with_osk)
}

fn finish_name_entry_transition(view: &mut ViewState, was_renaming: bool) {
    if was_renaming && !name_entry_active(view) {
        // Care and utility controls remain usable from the dialog. Leaving it must never
        // turn a creature-name draft into a chat message or leave the controller in name mode.
        view.text_buffer.clear();
        view.renaming_with_osk = false;
    }
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
                format!(
                    "Did not catch that. Hold {} and try again.",
                    view.binding_labels.push_to_talk
                ),
                now_ms,
                5_000,
            );
        }
        SpokenInputStatus::NoCandidate => {
            view.microphone_state = resting_microphone_state;
            view.show_status(
                format!(
                    "No speech heard. Hold {} and try again.",
                    view.binding_labels.push_to_talk
                ),
                now_ms,
                5_000,
            );
        }
        SpokenInputStatus::Expired => {
            view.microphone_state = resting_microphone_state;
            view.show_status(
                "Your Beastie stayed with what it was doing.".to_owned(),
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
                    "Microphone unavailable. Text still works.".to_owned()
                }
                SpeechInputFailure::RecognizerUnavailable => {
                    "Speech recognition unavailable. Text still works.".to_owned()
                }
                SpeechInputFailure::RecognitionFailed => {
                    format!(
                        "Speech recognition failed. Hold {} to try again.",
                        view.binding_labels.push_to_talk
                    )
                }
                SpeechInputFailure::UnsupportedLanguage => {
                    "Speech language unsupported. Text still works.".to_owned()
                }
            };
            view.show_status(message, now_ms, 5_000);
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
    use beastie_view::{MicrophoneState, UiAction, UiMode, UiTarget, ViewState};

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
    fn leaving_name_entry_through_care_never_turns_the_name_into_a_chat_draft() {
        for old_mode in [UiMode::Rename, UiMode::OnScreenKeyboard] {
            for next_mode in [
                UiMode::FoodChoice,
                UiMode::Context(UiTarget::Toy(ToyId::Ball)),
                UiMode::Settings,
                UiMode::Compose,
            ] {
                let mut view = ViewState {
                    mode: old_mode,
                    renaming_with_osk: old_mode == UiMode::OnScreenKeyboard,
                    text_buffer: "Fern".into(),
                    ..Default::default()
                };
                let was_renaming = super::name_entry_active(&view);
                view.mode = next_mode;
                super::finish_name_entry_transition(&mut view, was_renaming);
                assert!(view.text_buffer.is_empty());
                assert!(!view.renaming_with_osk);
            }
        }
        let mut view = ViewState {
            mode: UiMode::FoodChoice,
            text_buffer: "hello".into(),
            ..Default::default()
        };
        super::finish_name_entry_transition(&mut view, false);
        assert_eq!(
            view.text_buffer, "hello",
            "ordinary chat drafts survive care navigation"
        );
    }

    #[test]
    fn scripted_local_stores_and_cli_export_are_confined_to_evidence() {
        use clap::Parser;
        use std::path::Path;
        for (arguments, expected) in [
            (
                vec!["game", "--script", "capture.jsonl"],
                "target/captures/aquarium-shell/.local-data",
            ),
            (
                vec![
                    "game",
                    "--script",
                    "capture.jsonl",
                    "--capture-dir",
                    "/tmp/beastie-ui-isolation",
                ],
                "/tmp/beastie-ui-isolation/.local-data",
            ),
            (
                vec![
                    "game",
                    "--script",
                    "capture.jsonl",
                    "--capture-dir",
                    "/tmp/stills",
                    "--feel-dir",
                    "/tmp/beastie-feel-isolation",
                ],
                "/tmp/beastie-feel-isolation/.local-data",
            ),
        ] {
            let args = crate::args::Args::parse_from(arguments);
            let root = super::local_data_root(&args).unwrap();
            assert_eq!(root, Path::new(expected));
            let export = super::transcript_export_destination(
                &args,
                &root,
                Path::new("/private/player/export.jsonl"),
            )
            .unwrap();
            assert_eq!(export, root.join("exports/export.jsonl"));
        }
        let args = crate::args::Args::parse_from(["game"]);
        let requested = Path::new("/tmp/explicit-player-export.jsonl");
        assert_eq!(
            super::transcript_export_destination(&args, Path::new("unused"), requested).unwrap(),
            requested
        );
    }

    fn recovery_test_path(label: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir()
            .join(format!(
                "beastie-ui-recovery-{}-{}-{label}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ))
            .join("main.json")
    }

    fn named_save(name: &str) -> String {
        beastie_session::GameSession::new(42, name)
            .capture(100)
            .to_json()
            .unwrap()
    }

    #[test]
    fn scripted_reset_recovery_and_export_leave_unrelated_player_files_untouched() {
        use clap::Parser;
        let scratch = recovery_test_path("isolated-io")
            .parent()
            .unwrap()
            .to_owned();
        let player = scratch.join("player");
        std::fs::create_dir_all(&player).unwrap();
        for file in ["main.json", "settings.json", "playtest.jsonl"] {
            std::fs::write(player.join(file), "unrelated player data").unwrap();
        }
        let capture = scratch.join("evidence");
        let args = crate::args::Args::parse_from([
            "game",
            "--script",
            "capture.jsonl",
            "--capture-dir",
            capture.to_str().unwrap(),
        ]);
        let root = super::local_data_root(&args).unwrap();
        let saves = SaveStore::new(root.join("saves/main.json"));
        saves.store(&named_save("Fern")).unwrap();
        saves.reset().unwrap();
        saves.store(&named_save("New")).unwrap();
        assert_eq!(
            super::recover_saved_session(&saves, None)
                .unwrap()
                .0
                .world()
                .creature
                .name,
            "Fern"
        );
        let export =
            super::transcript_export_destination(&args, &root, &player.join("playtest.jsonl"))
                .unwrap();
        let transcripts =
            crate::transcript::TranscriptStore::new(root.join("transcripts/playtest.jsonl"), true);
        let source = root.join("transcripts/playtest.jsonl");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, "partial source record").unwrap();
        assert_eq!(transcripts.export(&export).unwrap(), 0);
        assert!(export.starts_with(&root));
        assert_ne!(export, source);
        assert_eq!(
            std::fs::read_to_string(source).unwrap(),
            "partial source record"
        );
        for file in ["main.json", "settings.json", "playtest.jsonl"] {
            assert_eq!(
                std::fs::read_to_string(player.join(file)).unwrap(),
                "unrelated player data"
            );
        }
        std::fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn unreadable_reset_primary_still_uses_its_valid_sibling() {
        let path = recovery_test_path("unreadable-reset");
        let store = SaveStore::new(path.clone());
        store.store(&named_save("Fern")).unwrap();
        store.store(&named_save("Damaged")).unwrap();
        store.reset().unwrap();
        let reset = path.with_extension("json.reset");
        std::fs::remove_file(&reset).unwrap();
        std::fs::create_dir(&reset).unwrap();
        let (session, source) = super::recover_saved_session(&store, None).unwrap();
        assert_eq!(source, crate::save_store::BackupSource::LastGoodBeforeReset);
        assert_eq!(session.world().creature.name, "Fern");
        assert!(reset.is_dir(), "unreadable evidence stays available");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn reset_new_save_and_recovery_restore_the_previous_creature_once() {
        use crate::save_store::BackupSource;
        let path = recovery_test_path("roundtrip");
        let store = SaveStore::new(path.clone());
        store.store(&named_save("Older")).unwrap();
        store.store(&named_save("Fern")).unwrap();
        assert!(store.reset().unwrap());
        store.store(&named_save("New")).unwrap();
        store.store(&named_save("Newest")).unwrap();
        let (recovered, source) = super::recover_saved_session(&store, Some(100)).unwrap();
        assert_eq!(source, BackupSource::BeforeReset);
        assert_eq!(recovered.world().creature.name, "Fern");
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.reset-recovered")).unwrap(),
            named_save("Fern")
        );
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.corrupt")).unwrap(),
            named_save("Newest")
        );
        let (next, source) = super::recover_saved_session(&store, Some(100)).unwrap();
        assert_eq!(source, BackupSource::LastGood);
        assert_eq!(next.world().creature.name, "New");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn reset_preserves_corrupt_primary_and_recovers_its_last_good_copy() {
        use crate::save_store::BackupSource;
        let path = recovery_test_path("corrupt");
        let store = SaveStore::new(path.clone());
        store.store(&named_save("Fern")).unwrap();
        store.store("broken primary").unwrap();
        store.reset().unwrap();
        store.store(&named_save("New")).unwrap();
        let (recovered, source) = super::recover_saved_session(&store, Some(100)).unwrap();
        assert_eq!(source, BackupSource::LastGoodBeforeReset);
        assert_eq!(recovered.world().creature.name, "Fern");
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.reset")).unwrap(),
            "broken primary"
        );
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.reset-good-recovered")).unwrap(),
            named_save("Fern")
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_reset_falls_back_to_valid_ordinary_backup_without_losing_evidence() {
        use crate::save_store::BackupSource;
        let path = recovery_test_path("fallback");
        let store = SaveStore::new(path.clone());
        store.store("broken reset").unwrap();
        store.reset().unwrap();
        store.store(&named_save("Fern")).unwrap();
        store.store(&named_save("New")).unwrap();
        let (recovered, source) = super::recover_saved_session(&store, Some(100)).unwrap();
        assert_eq!(source, BackupSource::LastGood);
        assert_eq!(recovered.world().creature.name, "Fern");
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.reset")).unwrap(),
            "broken reset"
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn failed_recovery_keeps_pending_reset_and_current_save_for_retry() {
        let path = recovery_test_path("failure");
        let store = SaveStore::new(path.clone());
        store.store(&named_save("Fern")).unwrap();
        store.reset().unwrap();
        store.store(&named_save("New")).unwrap();
        std::fs::create_dir(path.with_extension("json.corrupt")).unwrap();
        assert!(super::recover_saved_session(&store, Some(100)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), named_save("New"));
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.reset")).unwrap(),
            named_save("Fern")
        );
        std::fs::remove_dir(path.with_extension("json.corrupt")).unwrap();
        let (recovered, _) = super::recover_saved_session(&store, Some(100)).unwrap();
        assert_eq!(recovered.world().creature.name, "Fern");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rename_keyboard_and_controller_use_the_same_unicode_limit() {
        use beastie_view::UiMode;
        for (mode, renaming_with_osk) in [(UiMode::Rename, false), (UiMode::OnScreenKeyboard, true)]
        {
            let mut view = ViewState {
                mode,
                renaming_with_osk,
                ..ViewState::default()
            };
            super::append_view_text(&mut view, &"水".repeat(30), 10);
            assert_eq!(view.text_buffer, "水".repeat(24));
            assert_eq!(view.status_expires_at_ms, Some(4_010));
            assert!(view.compose_engaged);
        }
        let mut view = ViewState::default();
        super::append_view_text(&mut view, &"a".repeat(600), 10);
        assert_eq!(view.text_buffer.chars().count(), 512);
    }

    #[test]
    fn native_text_shortcuts_belong_only_to_the_focused_message_or_name_field() {
        use bevy::input::keyboard::Key;
        let mut stroke = crate::host::KeyStroke {
            key: Key::Character("a".into()),
            text: Some("a".to_owned()),
            control: !cfg!(target_os = "macos"),
            super_key: cfg!(target_os = "macos"),
            shift: false,
        };
        for (mode, focus, expected) in [
            (UiMode::Compose, "compose/input", true),
            (UiMode::Rename, "rename/input", true),
            (UiMode::Compose, "compose/send", false),
            (UiMode::Rename, "rename/submit", false),
            (UiMode::OnScreenKeyboard, "keyboard/a", false),
        ] {
            let view = ViewState {
                mode,
                focused_region: Some(focus.to_owned()),
                text_buffer: "hello".to_owned(),
                ..ViewState::default()
            };
            stroke.key = Key::Character("a".into());
            assert_eq!(
                super::text_shortcut(&view, &stroke),
                expected.then_some(super::TextShortcut::SelectAll)
            );
            stroke.key = Key::Character("v".into());
            assert_eq!(
                super::text_shortcut(&view, &stroke),
                expected.then_some(super::TextShortcut::Paste)
            );
            stroke.key = Key::Character("z".into());
            assert_eq!(super::text_shortcut(&view, &stroke), None);
        }
        stroke.key = Key::Character("a".into());
        stroke.control = cfg!(target_os = "macos");
        stroke.super_key = !cfg!(target_os = "macos");
        assert_eq!(super::text_shortcut(&ViewState::default(), &stroke), None);
    }

    #[test]
    fn selected_text_is_replaced_by_typing_and_cleared_by_either_delete_key() {
        let mut view = ViewState {
            text_buffer: "hello".to_owned(),
            text_selected: true,
            ..ViewState::default()
        };
        super::append_view_text(&mut view, "Mop", 0);
        assert_eq!(view.text_buffer, "Mop");
        assert!(!view.text_selected);
        for backspace in [false, true] {
            view.text_buffer = "Mop".to_owned();
            view.text_selected = true;
            crate::input::delete_view_text(&mut view, backspace);
            assert!(view.text_buffer.is_empty());
            assert!(!view.text_selected);
        }
        view.text_buffer = "Me\u{301}".to_owned();
        crate::input::delete_view_text(&mut view, true);
        assert_eq!(view.text_buffer, "M");
        crate::input::delete_view_text(&mut view, false);
        assert_eq!(view.text_buffer, "M");
    }

    #[test]
    fn selected_paste_is_unicode_bounded_and_failure_preserves_selection() {
        use crate::input::TextEditTracker;
        for (mode, focus, limit) in [
            (UiMode::Compose, "compose/input", 512),
            (UiMode::Rename, "rename/input", 24),
        ] {
            let mut view = ViewState {
                mode,
                focused_region: Some(focus.to_owned()),
                text_buffer: "old draft".to_owned(),
                ..ViewState::default()
            };
            let mut edits = TextEditTracker::default();
            edits.sync(&mut view);
            view.text_selected = true;
            edits.sync(&mut view);
            let owner = edits.owner().unwrap();
            super::apply_clipboard_result(&mut view, &mut edits, owner, Err(()), 0);
            assert_eq!(view.text_buffer, "old draft");
            assert!(view.text_selected);
            let pasted = format!("{}e\u{301}\nmore", "é".repeat(limit - 1));
            super::apply_clipboard_result(&mut view, &mut edits, owner, Ok(pasted), 1);
            assert_eq!(view.text_buffer, "é".repeat(limit - 1));
            assert!(!view.text_selected);
            assert!(!view.pending);
        }
    }

    #[test]
    fn delayed_paste_cannot_follow_edits_navigation_submission_or_window_focus_loss() {
        use crate::input::TextEditTracker;
        for change in 0..6 {
            let mut view = ViewState {
                text_buffer: "draft".to_owned(),
                ..ViewState::default()
            };
            let mut edits = TextEditTracker::default();
            edits.sync(&mut view);
            let owner = edits.owner().unwrap();
            match change {
                0 => super::append_view_text(&mut view, " changed", 0),
                1 => view.focused_region = Some("compose/send".to_owned()),
                2 => {
                    view.mode = UiMode::Rename;
                    view.focused_region = Some("rename/input".to_owned());
                }
                3 => {
                    view.mode = UiMode::Settings;
                    edits.sync(&mut view);
                    view.mode = UiMode::Compose;
                }
                4 => {
                    view.text_buffer.clear();
                    edits.sync(&mut view);
                    view.text_buffer = "draft".to_owned();
                }
                _ => edits.invalidate(&mut view),
            }
            let before = view.text_buffer.clone();
            super::apply_clipboard_result(
                &mut view,
                &mut edits,
                owner,
                Ok("wrong destination".to_owned()),
                10,
            );
            assert_eq!(view.text_buffer, before, "case {change}");
        }
    }

    #[test]
    fn key_repeats_edit_only_the_focused_field_and_never_activate_controls() {
        use crate::host::KeyStroke;
        use bevy::input::keyboard::Key;
        let stroke = |key, text: Option<&str>| KeyStroke {
            key,
            text: text.map(str::to_owned),
            control: false,
            super_key: false,
            shift: false,
        };
        for (mode, focus, editable) in [
            (UiMode::Compose, "compose/input", true),
            (UiMode::Compose, "compose/send", false),
            (UiMode::Compose, "compose/settings", false),
            (UiMode::Rename, "rename/input", true),
            (UiMode::Rename, "rename/submit", false),
            (UiMode::OnScreenKeyboard, "keyboard/a", false),
            (UiMode::Title, "title/continue", false),
        ] {
            let mut view = ViewState {
                mode,
                focused_region: Some(focus.to_owned()),
                text_buffer: "abc".to_owned(),
                ..ViewState::default()
            };
            super::repeat_text_input(&mut view, &stroke(Key::Character("x".into()), Some("x")), 0);
            assert_eq!(view.text_buffer, if editable { "abcx" } else { "abc" });
            super::repeat_text_input(&mut view, &stroke(Key::Backspace, None), 0);
            assert_eq!(view.text_buffer, "abc");
            for key in [Key::Enter, Key::Tab, Key::F6, Key::F11] {
                let before = view.clone();
                super::repeat_text_input(&mut view, &stroke(key, None), 0);
                assert_eq!(view, before);
            }
            let mut modified = stroke(Key::Character("x".into()), Some("x"));
            modified.super_key = true;
            let before = view.clone();
            super::repeat_text_input(&mut view, &modified, 0);
            assert_eq!(view, before);
        }
    }

    #[test]
    fn rename_focus_stays_local_and_reverse_tab_traverses_its_controls() {
        use beastie_view::UiMode;
        let world = beastie_core::WorldState::new(42, "Mop");
        let mut view = ViewState {
            mode: UiMode::Rename,
            text_buffer: "Fern".to_owned(),
            ..ViewState::default()
        };
        super::focus_text_input(&mut view);
        assert_eq!(view.mode, UiMode::Rename);
        assert_eq!(view.focused_region.as_deref(), Some("rename/input"));
        let scene = beastie_view::plan(&world, &view).0;
        let next = crate::input::move_focus(
            &scene,
            view.focused_region.as_deref(),
            super::tab_direction(false),
        );
        let back = crate::input::move_focus(&scene, next.as_deref(), super::tab_direction(true));
        assert_eq!(back.as_deref(), Some("rename/input"));
        for id in ["rename/input", "rename/submit", "rename/cancel"] {
            assert!(
                crate::input::focused_action(&scene, Some(id)).is_some(),
                "{id}"
            );
        }
    }

    #[test]
    fn inspection_retains_selected_target_without_changing_world() {
        use beastie_view::{UiMode, UiTarget};
        let world = beastie_core::WorldState::new(42, "Mop");
        let before = world.clone();
        for target in [
            UiTarget::Creature,
            UiTarget::Toy(ToyId::Bell),
            UiTarget::Cave,
            UiTarget::OpenWater,
            UiTarget::Plant(1),
            UiTarget::FoodObject(1),
        ] {
            let mut view = ViewState {
                mode: UiMode::Context(target),
                ..ViewState::default()
            };
            super::open_inspection(&world, &mut view);
            assert_eq!(view.mode, UiMode::Inspect(target));
            assert!(!beastie_view::plan(&world, &view).0.text.is_empty());
            let anchor = view.context_card_anchor;
            let mut moved = world.clone();
            moved.creature.aquarium.position = beastie_core::NormalizedPosition::new(0, 5_000);
            let first = beastie_view::plan(&moved, &view).0;
            moved.creature.aquarium.position = beastie_core::NormalizedPosition::new(10_000, 5_000);
            let second = beastie_view::plan(&moved, &view).0;
            assert_eq!(view.context_card_anchor, anchor);
            let close_bounds = |scene: &beastie_view::ScenePlan| {
                scene
                    .hit_regions
                    .iter()
                    .find(|hit| hit.id == "inspect/close")
                    .unwrap()
                    .rect
            };
            assert_eq!(close_bounds(&first), close_bounds(&second));
        }
        assert_eq!(world, before);
    }

    #[test]
    fn pressed_receipt_only_belongs_to_an_enabled_matching_action() {
        let world = beastie_core::WorldState::new(42, "Mop");
        let mut view = ViewState::default();
        let scene = beastie_view::plan(&world, &view).0;
        super::mark_pressed_action(&scene, &mut view, UiAction::PushToTalk, 10);
        assert!(view.pressed_region.is_none());
        super::mark_pressed_action(&scene, &mut view, UiAction::OpenSettings, 10);
        assert_eq!(view.pressed_region.as_deref(), Some("compose/settings"));
        assert_eq!(view.pressed_until_ms, 110);
    }

    #[test]
    fn microphone_recovery_hints_use_current_bindings_and_never_a_fixed_name() {
        let mut view = ViewState::default();
        view.binding_labels.push_to_talk = "F8".to_owned();
        for status in [
            SpokenInputStatus::NoCandidate,
            SpokenInputStatus::AcousticUncertainty {
                confidence: beastie_protocol::AcousticConfidence::new(500).unwrap(),
            },
            SpokenInputStatus::InfrastructureFailure {
                failure: beastie_protocol::SpeechInputFailure::RecognitionFailed,
            },
        ] {
            apply_spoken_input_status(&mut view, status, 10, MicrophoneState::Idle);
            let message = view.status_message.as_deref().unwrap();
            assert!(message.contains("F8"));
            assert!(!message.contains("F1"));
        }
        apply_spoken_input_status(
            &mut view,
            SpokenInputStatus::Expired,
            10,
            MicrophoneState::Idle,
        );
        assert!(!view.status_message.as_deref().unwrap().contains("Mop"));
    }

    #[test]
    fn completed_and_canceled_rebinding_restore_the_originating_row() {
        use crate::settings::{BindingKey, KeyBindings};
        use beastie_view::{BindableAction, UiMode};

        let world = beastie_core::WorldState::new(7, "Mop");
        for action in [
            BindableAction::PushToTalk,
            BindableAction::Food,
            BindableAction::Play,
            BindableAction::Comfort,
            BindableAction::Settings,
            BindableAction::Cancel,
        ] {
            for accepted in [false, true] {
                let mut bindings = KeyBindings::default();
                let original = bindings.clone();
                let mut view = ViewState {
                    mode: UiMode::Rebinding(action),
                    focused_region: None,
                    ..ViewState::default()
                };
                super::finish_rebinding(
                    &world,
                    &mut view,
                    &mut bindings,
                    action,
                    accepted.then_some(BindingKey::F6),
                );
                assert_eq!(view.mode, UiMode::Bindings);
                assert_eq!(
                    crate::input::focused_action(
                        &beastie_view::plan(&world, &view).0,
                        view.focused_region.as_deref(),
                    ),
                    Some(UiAction::BeginRebind(action)),
                );
                assert_eq!(
                    bindings.key_for(action),
                    if accepted {
                        BindingKey::F6
                    } else {
                        original.key_for(action)
                    },
                );
                if !accepted {
                    assert_eq!(bindings, original);
                }
                assert!(!bindings.has_conflict());
                assert_eq!(view.binding_labels, super::binding_labels(&bindings));
            }
        }
    }

    #[test]
    fn occupied_rebinding_keeps_the_swap_and_focuses_the_edited_action() {
        use crate::settings::{BindingKey, KeyBindings};
        use beastie_view::{BindableAction, UiMode};

        let world = beastie_core::WorldState::new(7, "Mop");
        let mut bindings = KeyBindings::default();
        let mut view = ViewState {
            mode: UiMode::Rebinding(BindableAction::Food),
            ..ViewState::default()
        };
        super::finish_rebinding(
            &world,
            &mut view,
            &mut bindings,
            BindableAction::Food,
            Some(BindingKey::F3),
        );
        assert_eq!(bindings.food, BindingKey::F3);
        assert_eq!(bindings.play, BindingKey::F2);
        assert!(!bindings.has_conflict());
        assert_eq!(view.binding_labels.food, "F3");
        assert_eq!(view.binding_labels.play, "F2");
        assert_eq!(
            crate::input::focused_action(
                &beastie_view::plan(&world, &view).0,
                view.focused_region.as_deref(),
            ),
            Some(UiAction::BeginRebind(BindableAction::Food)),
        );
    }

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
            UiAction::TapWater,
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
        assert_eq!(
            revealed_text("e\u{301}👨‍👩‍👧‍👦!", 0, TextSpeed::Normal),
            ("e\u{301}".to_owned(), false)
        );
        assert_eq!(
            revealed_text("e\u{301}👨‍👩‍👧‍👦!", 30, TextSpeed::Normal),
            ("e\u{301}👨‍👩‍👧‍👦".to_owned(), false)
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
    fn reply_never_steals_keyboard_focus_and_preserves_other_pages() {
        let owner = DialogueOwner {
            generation: 1,
            request_id: 7,
        };
        let mut view = ViewState::default();
        for (at, subtitles) in [(42, false), (43, true)] {
            view.text_buffer = "ball".to_owned();
            show_dialogue_caption(&mut view, owner, "hello", at, subtitles, TextSpeed::Instant);
            super::focus_after_dialogue(&mut view);
            // The player can keep typing through a visible reply without a click.
            assert_eq!(view.focused_region.as_deref(), Some("compose/input"));
            assert_eq!(
                crate::input::focused_text_field(&view),
                Some(crate::input::TextField::Message)
            );
            assert_eq!(view.text_buffer, "ball");
        }

        for (mode, focus) in [
            (UiMode::Settings, "settings/subtitles"),
            (UiMode::Rename, "rename/input"),
            (UiMode::FoodChoice, "food/berry"),
        ] {
            view.mode = mode;
            view.focused_region = Some(focus.to_owned());
            view.text_buffer = "unfinished draft".to_owned();
            for subtitles in [false, true] {
                show_dialogue_caption(
                    &mut view,
                    owner,
                    "another thought",
                    44,
                    subtitles,
                    TextSpeed::Instant,
                );
                super::focus_after_dialogue(&mut view);
                assert_eq!(view.mode, mode);
                assert_eq!(view.focused_region.as_deref(), Some(focus));
                assert_eq!(view.text_buffer, "unfinished draft");
            }
        }
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

    #[test]
    fn owned_audio_articulates_between_simulation_ticks_and_closes_when_finished() {
        let owner = crate::feel::SpeechTraceOwner {
            dialogue_generation: 3,
            dialogue_request_id: 7,
            tts_request_id: 11,
        };
        let dialogue = Some(DialogueOwner {
            generation: owner.dialogue_generation,
            request_id: owner.dialogue_request_id,
        });
        let animation = super::SpeechAnimation {
            owner,
            timing: beastie_protocol::MouthTiming {
                open_ms: 60,
                close_ms: 40,
                syllable_ms: 160,
            },
        };
        let (mut audio, mut source) = crate::audio::test_speech_playback(owner, 1_000);
        let mut session = beastie_session::GameSession::new(42, "Mop");
        let fixed_tick_ms = session.world().elapsed_ms;
        let mut frame_ms = 0;
        for (milliseconds, expected_phase) in [(20, 2), (60, 0), (50, 1), (50, 2)] {
            session
                .apply(beastie_session::CommandEnvelope {
                    version: SESSION_PROTOCOL_VERSION,
                    command: SessionCommand::Tick { milliseconds },
                })
                .expect("advance one presentation frame");
            source.by_ref().take(milliseconds as usize).for_each(drop);
            frame_ms += milliseconds;
            if frame_ms < beastie_core::SIMULATION_TICK_MS {
                // The mouth moves on audio time even before the simulation steps.
                assert_eq!(session.world().elapsed_ms, fixed_tick_ms);
            }
            assert_eq!(
                animation.phase(dialogue, Some(owner), &audio),
                expected_phase
            );
        }

        assert_eq!(animation.phase(None, Some(owner), &audio), 0);
        assert_eq!(animation.phase(dialogue, None, &audio), 0);
        for stale in [
            crate::feel::SpeechTraceOwner {
                dialogue_generation: 4,
                ..owner
            },
            crate::feel::SpeechTraceOwner {
                dialogue_request_id: 8,
                ..owner
            },
            crate::feel::SpeechTraceOwner {
                tts_request_id: 12,
                ..owner
            },
        ] {
            assert_eq!(animation.phase(dialogue, Some(stale), &audio), 0);
            let (other_audio, _source) = crate::audio::test_speech_playback(stale, 1_000);
            assert_eq!(animation.phase(dialogue, Some(owner), &other_audio), 0);
        }
        assert_eq!(
            animation.phase(
                Some(DialogueOwner {
                    generation: owner.dialogue_generation + 1,
                    request_id: owner.dialogue_request_id,
                }),
                Some(owner),
                &audio,
            ),
            0
        );

        source.by_ref().take(1_001).for_each(drop);
        assert!(!audio.speech_active());
        assert_eq!(animation.phase(dialogue, Some(owner), &audio), 0);
        audio.stop_speech();
        assert_eq!(animation.phase(dialogue, Some(owner), &audio), 0);
        let (empty, mut source) = crate::audio::test_speech_playback(owner, 0);
        let _ = source.next();
        assert_eq!(animation.phase(dialogue, Some(owner), &empty), 0);
    }
}
