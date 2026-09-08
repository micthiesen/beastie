use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use beastie_view::{AudioCommand, AudioCue, PresentationChannel, SemanticOwner};
use rodio::Player;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source};

pub const UI_SELECT: &str = "ui/select";
pub const UI_CONFIRM: &str = "ui/confirm";
const CREATURE_MRR: &str = "creature/mrr";
const CREATURE_ANNOYED: &str = "creature/annoyed";
const CREATURE_SLEEP: &str = "creature/sleep";
const UNDERWATER_LOOP: &str = "environment/underwater-loop";
const BUBBLES_1: &str = "environment/bubbles-1";
const BUBBLES_2: &str = "environment/bubbles-2";
const CAVE_SETTLE: &str = "environment/cave-settle";
const SAND_DISTURB: &str = "environment/sand-disturb";
const SWIM_WAKE: &str = "movement/swim-wake";
const FOOD_DROP: &str = "food/drop-sink";
const FOOD_EAT: &str = "food/eat";
const FOOD_REJECT: &str = "food/spit-reject";
const TOY_IMPACT: &str = "object/toy-impact";
const BALL_NUDGE: &str = "object/ball-nudge";
const BELL_RING: &str = "object/bell-ring";
const SOCK_RUSTLE: &str = "object/sock-rustle";
const CREATURE_AFFECTION: &str = "creature/affection";
const CREATURE_WAKE: &str = "creature/wake";
const CREATURE_CURIOUS: &str = "creature/curious";

const SOUND_IDS: &[&str] = &[
    UI_SELECT,
    UI_CONFIRM,
    CREATURE_MRR,
    CREATURE_ANNOYED,
    CREATURE_SLEEP,
    UNDERWATER_LOOP,
    BUBBLES_1,
    BUBBLES_2,
    CAVE_SETTLE,
    SAND_DISTURB,
    SWIM_WAKE,
    FOOD_DROP,
    FOOD_EAT,
    FOOD_REJECT,
    TOY_IMPACT,
    BALL_NUDGE,
    BELL_RING,
    SOCK_RUSTLE,
    CREATURE_AFFECTION,
    "creature/surprise",
    "creature/curious",
    "creature/sad",
    CREATURE_WAKE,
];

/// Post-arbitration source state. Bytes are retained separately by the recorder, by hash.
#[derive(Clone, serde::Serialize)]
pub struct PlaybackSnapshot {
    pub playback_id: u64,
    pub asset_sha256: String,
    pub source: &'static str,
    pub role: MixRole,
    pub owner: Option<SemanticOwner>,
    pub speech_owner: Option<crate::feel::SpeechTraceOwner>,
    pub channel: Option<PresentationChannel>,
    pub gain: f32,
    pub looping: bool,
    #[serde(skip)]
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MixRole {
    Bed,
    Ambience,
    Interface,
    Physical,
    Creature,
    Speech,
}

#[derive(serde::Serialize)]
pub struct PlaybackDecision {
    pub outcome: &'static str,
    pub playback_id: Option<u64>,
    pub source: &'static str,
    pub started: Option<PlaybackSnapshot>,
}

pub struct AudioBank {
    output: Option<MixerDeviceSink>,
    sounds: BTreeMap<&'static str, Arc<[u8]>>,
    speech: Option<ActivePlayback>,
    one_shots: Vec<ActivePlayback>,
    ambience: Option<ActivePlayback>,
    effects_gain: f32,
    speech_gain: f32,
    ambience_duck: f32,
    duck_envelope: GainEnvelope,
    creature_envelope: GainEnvelope,
    next_playback_id: u64,
    decisions: Vec<PlaybackDecision>,
}

struct PlaybackRequest {
    source: &'static str,
    bytes: Arc<[u8]>,
    role: MixRole,
    owner: Option<SemanticOwner>,
    channel: Option<PresentationChannel>,
    base_gain: f32,
    looping: bool,
}

struct ActivePlayback {
    player: Player,
    state: PlaybackSnapshot,
    base_gain: f32,
}

/// A bounded linear envelope: repeated frames do not restart its attack/release.
#[derive(Clone, Copy)]
struct GainEnvelope {
    value: f32,
    start: f32,
    target: f32,
    elapsed_ms: u64,
    duration_ms: u64,
}
impl Default for GainEnvelope {
    fn default() -> Self {
        Self {
            value: 1.0,
            start: 1.0,
            target: 1.0,
            elapsed_ms: 0,
            duration_ms: 0,
        }
    }
}
impl GainEnvelope {
    fn advance(&mut self, target: f32, delta_ms: u64, attack_ms: u64, release_ms: u64) -> f32 {
        if self.target != target {
            self.start = self.value;
            self.duration_ms = if target < self.value {
                attack_ms
            } else {
                release_ms
            };
            self.target = target;
            self.elapsed_ms = 0;
        }
        self.elapsed_ms = self
            .elapsed_ms
            .saturating_add(delta_ms)
            .min(self.duration_ms);
        self.value = if self.duration_ms == 0 {
            target
        } else {
            self.start + (target - self.start) * self.elapsed_ms as f32 / self.duration_ms as f32
        };
        self.value
    }
}

impl AudioBank {
    #[must_use]
    pub fn load(assets_root: &Path) -> Self {
        let mut output = DeviceSinkBuilder::open_default_sink().ok();
        if let Some(output) = &mut output {
            output.log_on_drop(false);
        }
        Self {
            output,
            sounds: load_sounds(assets_root),
            speech: None,
            one_shots: Vec::new(),
            ambience: None,
            effects_gain: 0.7,
            speech_gain: 0.7,
            ambience_duck: 1.0,
            duck_envelope: GainEnvelope::default(),
            creature_envelope: GainEnvelope::default(),
            next_playback_id: 1,
            decisions: Vec::new(),
        }
    }

    fn start(&mut self, request: PlaybackRequest) -> Option<ActivePlayback> {
        let PlaybackRequest {
            source,
            bytes,
            role,
            owner,
            channel,
            base_gain,
            looping,
        } = request;
        self.decisions.push(PlaybackDecision {
            outcome: "requested",
            playback_id: None,
            source,
            started: None,
        });
        let Some(output) = &self.output else {
            self.decisions.push(PlaybackDecision {
                outcome: "discarded_no_output",
                playback_id: None,
                source,
                started: None,
            });
            return None;
        };
        let Ok(decoder) = Decoder::try_from(Cursor::new(Arc::clone(&bytes))) else {
            self.decisions.push(PlaybackDecision {
                outcome: "discarded_decode",
                playback_id: None,
                source,
                started: None,
            });
            return None;
        };
        // Set gain before append, so new sources never leak a full-volume first block.
        let player = Player::connect_new(output.mixer());
        let gain = self.gain_for(role, base_gain);
        player.set_volume(gain);
        if looping {
            player.append(decoder.repeat_infinite());
        } else {
            player.append(decoder);
        }
        let playback_id = self.next_playback_id;
        self.next_playback_id += 1;
        use sha2::{Digest, Sha256};
        let active = ActivePlayback {
            player,
            base_gain,
            state: PlaybackSnapshot {
                playback_id,
                asset_sha256: format!("{:x}", Sha256::digest(&bytes)),
                source,
                role,
                owner,
                speech_owner: None,
                channel,
                gain,
                looping,
                bytes,
            },
        };
        self.decisions.push(PlaybackDecision {
            outcome: "started",
            playback_id: Some(playback_id),
            source,
            started: Some(active.state.clone()),
        });
        Some(active)
    }

    fn gain_for(&self, role: MixRole, base: f32) -> f32 {
        base * match role {
            MixRole::Speech => self.speech_gain,
            MixRole::Bed => self.effects_gain * self.ambience_duck,
            MixRole::Creature => self.effects_gain * self.creature_envelope.value,
            _ => self.effects_gain,
        }
    }

    pub fn ensure_ambience(&mut self) {
        if self.ambience.is_some() || self.output.is_none() {
            return;
        }
        if let Some(bytes) = self.sounds.get(UNDERWATER_LOOP).cloned() {
            self.ambience = self.start(PlaybackRequest {
                source: UNDERWATER_LOOP,
                bytes,
                role: MixRole::Bed,
                owner: None,
                channel: None,
                base_gain: 0.35,
                looping: true,
            });
        }
    }

    pub fn set_gains(&mut self, effects: f32, speech: f32) {
        self.effects_gain = effects.clamp(0.0, 1.0);
        self.speech_gain = speech.clamp(0.0, 1.0);
        self.apply_gains();
    }

    pub fn play_queued(&mut self, queued: &mut Vec<AudioCommand>) {
        let requested = queued.clone();
        reduce_audio_commands(queued);
        let mut remaining = queued.clone();
        for command in requested {
            if let AudioCommand::Play { cue, .. } = command {
                if let Some(index) = remaining.iter().position(|candidate| *candidate == command) {
                    remaining.remove(index);
                } else if let Some(source) = sound_for_cue(cue) {
                    self.decisions.push(PlaybackDecision {
                        outcome: "discarded_arbitration",
                        playback_id: None,
                        source,
                        started: None,
                    });
                }
            }
        }
        for command in queued.drain(..) {
            match command {
                AudioCommand::CancelOwner { owner } => {
                    self.cancel_where(|sound| sound.state.owner == Some(owner))
                }
                AudioCommand::CancelLowerPriority { owner, channel } => {
                    self.cancel_where(|sound| {
                        sound.state.channel == Some(channel)
                            && sound
                                .state
                                .owner
                                .is_some_and(|other| owner_priority(other) < owner_priority(owner))
                    })
                }
                AudioCommand::Play {
                    owner,
                    channel,
                    cue,
                    gain_milli,
                } => {
                    let Some(source) = sound_for_cue(cue) else {
                        continue;
                    };
                    if source == SWIM_WAKE
                        && self
                            .one_shots
                            .iter()
                            .any(|s| s.state.source == source && !s.player.empty())
                    {
                        self.decisions.push(PlaybackDecision {
                            outcome: "discarded_rate_limit",
                            playback_id: None,
                            source,
                            started: None,
                        });
                        continue;
                    }
                    let Some(bytes) = self.sounds.get(source).cloned() else {
                        self.decisions.push(PlaybackDecision {
                            outcome: "discarded_missing_asset",
                            playback_id: None,
                            source,
                            started: None,
                        });
                        continue;
                    };
                    if matches!(source, BUBBLES_1 | BUBBLES_2)
                        && self
                            .one_shots
                            .iter()
                            .filter(|s| matches!(s.state.source, BUBBLES_1 | BUBBLES_2))
                            .count()
                            >= 2
                        && let Some(index) = self
                            .one_shots
                            .iter()
                            .position(|s| matches!(s.state.source, BUBBLES_1 | BUBBLES_2))
                    {
                        let sound = self.one_shots.remove(index);
                        sound.player.stop();
                        self.decisions.push(PlaybackDecision {
                            outcome: "cancelled_overlap",
                            playback_id: Some(sound.state.playback_id),
                            source: sound.state.source,
                            started: None,
                        });
                    }
                    if let Some(sound) = self.start(PlaybackRequest {
                        source,
                        bytes,
                        role: role_for_cue(cue),
                        owner: Some(owner),
                        channel: Some(channel),
                        base_gain: f32::from(gain_milli.min(1_000)) / 1_000.0,
                        looping: false,
                    }) {
                        self.one_shots.push(sound);
                    }
                }
            }
        }
    }

    fn cancel_where(&mut self, predicate: impl Fn(&ActivePlayback) -> bool) {
        self.one_shots.retain(|sound| {
            if predicate(sound) {
                sound.player.stop();
                self.decisions.push(PlaybackDecision {
                    outcome: "cancelled",
                    playback_id: Some(sound.state.playback_id),
                    source: sound.state.source,
                    started: None,
                });
                false
            } else {
                true
            }
        });
    }

    pub fn play_speech(&mut self, wav: Arc<[u8]>, owner: crate::feel::SpeechTraceOwner) -> bool {
        self.stop_speech();
        self.speech = self.start(PlaybackRequest {
            source: "speech",
            bytes: wav,
            role: MixRole::Speech,
            owner: None,
            channel: None,
            base_gain: 1.0,
            looping: false,
        });
        if let Some(sound) = &mut self.speech {
            sound.state.speech_owner = Some(owner);
            if let Some(started) = self
                .decisions
                .last_mut()
                .and_then(|decision| decision.started.as_mut())
            {
                started.speech_owner = Some(owner);
            }
        }
        self.speech.is_some()
    }

    pub fn stop_speech(&mut self) {
        if let Some(sound) = self.speech.take() {
            sound.player.stop();
            self.decisions.push(PlaybackDecision {
                outcome: "cancelled",
                playback_id: Some(sound.state.playback_id),
                source: sound.state.source,
                started: None,
            });
        }
    }

    pub fn update_ducking(&mut self, delta_ms: u64) {
        self.one_shots.retain(|sound| {
            if sound.player.empty() {
                self.decisions.push(PlaybackDecision {
                    outcome: "completed",
                    playback_id: Some(sound.state.playback_id),
                    source: sound.state.source,
                    started: None,
                });
                false
            } else {
                true
            }
        });
        if self.speech.as_ref().is_some_and(|s| s.player.empty())
            && let Some(sound) = self.speech.take()
        {
            self.decisions.push(PlaybackDecision {
                outcome: "completed",
                playback_id: Some(sound.state.playback_id),
                source: sound.state.source,
                started: None,
            });
        }
        let speech_active = self.speech_active() && self.speech_gain > 0.0;
        let creature_active = self.effects_gain > 0.0
            && self
                .one_shots
                .iter()
                .any(|s| s.state.role == MixRole::Creature && s.base_gain > 0.0);
        let target = duck_gain(speech_active, creature_active);
        let release_ms = if self.duck_envelope.target == duck_gain(true, false) {
            260
        } else {
            180
        };
        self.ambience_duck = self.duck_envelope.advance(
            target,
            delta_ms,
            if speech_active { 45 } else { 35 },
            release_ms,
        );
        self.creature_envelope.advance(
            if speech_active { 0.562_341 } else { 1.0 },
            delta_ms,
            45,
            260,
        );
        self.apply_gains();
    }

    fn apply_gains(&mut self) {
        if let Some(sound) = &mut self.speech {
            sound.state.gain = self.speech_gain;
            sound.player.set_volume(sound.state.gain);
        }
        if let Some(sound) = &mut self.ambience {
            sound.state.gain = 0.35 * self.effects_gain * self.ambience_duck;
            sound.player.set_volume(sound.state.gain);
        }
        for sound in &mut self.one_shots {
            sound.state.gain = sound.base_gain
                * self.effects_gain
                * if sound.state.role == MixRole::Creature {
                    self.creature_envelope.value
                } else {
                    1.0
                };
            sound.player.set_volume(sound.state.gain);
        }
    }

    pub fn playback_snapshot(&self) -> Vec<PlaybackSnapshot> {
        self.ambience
            .iter()
            .chain(self.speech.iter())
            .chain(self.one_shots.iter())
            .map(|s| s.state.clone())
            .collect()
    }
    pub fn take_decisions(&mut self) -> Vec<PlaybackDecision> {
        std::mem::take(&mut self.decisions)
    }
    pub const fn output_available(&self) -> bool {
        self.output.is_some()
    }
    #[must_use]
    pub fn speech_active(&self) -> bool {
        self.speech.as_ref().is_some_and(|s| !s.player.empty())
    }
    #[must_use]
    pub const fn ambience_duck(&self) -> f32 {
        self.ambience_duck
    }
    #[must_use]
    pub fn one_shot_active(&self) -> bool {
        !self.one_shots.is_empty()
    }
}

const fn role_for_cue(cue: AudioCue) -> MixRole {
    match cue {
        AudioCue::Bubble | AudioCue::BubbleAlternate => MixRole::Ambience,
        AudioCue::UiReject | AudioCue::UiConfirm => MixRole::Interface,
        AudioCue::Mrr
        | AudioCue::Annoyed
        | AudioCue::Sleep
        | AudioCue::Affection
        | AudioCue::Wake
        | AudioCue::Curious => MixRole::Creature,
        _ => MixRole::Physical,
    }
}

const fn owner_priority(owner: SemanticOwner) -> u8 {
    match owner {
        SemanticOwner::Ordinary => 0,
        SemanticOwner::PrivateLife(_) => 1,
        SemanticOwner::StandaloneRelationship(_) => 1,
        SemanticOwner::ActionRelationship(_) => 2,
        SemanticOwner::DirectOutcome => 3,
    }
}

pub fn reduce_audio_commands(commands: &mut Vec<AudioCommand>) {
    let mut reduced = Vec::with_capacity(commands.len());
    for command in commands.drain(..) {
        match command {
            AudioCommand::CancelOwner { owner } => {
                reduced.retain(|queued| {
                    !matches!(queued, AudioCommand::Play { owner: queued_owner, .. } if *queued_owner == owner)
                });
                reduced.push(command);
            }
            AudioCommand::CancelLowerPriority { owner, channel } => {
                reduced.retain(|queued| {
                    !matches!(queued, AudioCommand::Play { owner: queued_owner, channel: queued_channel, .. } if *queued_channel == channel && owner_priority(*queued_owner) < owner_priority(owner))
                });
                reduced.push(command);
            }
            AudioCommand::Play { .. } => reduced.push(command),
        }
    }
    *commands = reduced;
}

#[derive(Debug, Clone)]
pub struct AmbientBubbleSchedule {
    seed: u64,
    sequence: u64,
    next_at_ms: u64,
    last_variant: Option<bool>,
    repeat_count: u8,
}

impl AmbientBubbleSchedule {
    #[must_use]
    pub fn new(seed: u64, now_ms: u64) -> Self {
        let mut schedule = Self {
            seed,
            sequence: 0,
            next_at_ms: now_ms,
            last_variant: None,
            repeat_count: 0,
        };
        schedule.schedule_after(now_ms);
        schedule
    }

    pub fn poll(&mut self, now_ms: u64, busy: bool) -> Option<AudioCue> {
        if now_ms < self.next_at_ms {
            return None;
        }
        if busy {
            self.next_at_ms = now_ms.saturating_add(1_000);
            return None;
        }
        let mixed = splitmix64(
            self.seed.rotate_left(17) ^ self.sequence.wrapping_mul(0xD1B5_4A32_D192_ED03),
        );
        let candidate = mixed.is_multiple_of(2);
        let variant = if self.last_variant == Some(candidate) && self.repeat_count >= 2 {
            !candidate
        } else {
            candidate
        };
        if self.last_variant == Some(variant) {
            self.repeat_count = self.repeat_count.saturating_add(1);
        } else {
            self.last_variant = Some(variant);
            self.repeat_count = 1;
        }
        let sound = if variant {
            AudioCue::Bubble
        } else {
            AudioCue::BubbleAlternate
        };
        self.schedule_after(now_ms);
        Some(sound)
    }

    fn schedule_after(&mut self, now_ms: u64) {
        let mixed = splitmix64(self.seed ^ self.sequence.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        self.sequence = self.sequence.wrapping_add(1);
        self.next_at_ms = now_ms.saturating_add(6_000 + mixed % 12_001);
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn duck_gain(speech_active: bool, one_shot_active: bool) -> f32 {
    if speech_active {
        0.446_684
    } else if one_shot_active {
        0.630_957
    } else {
        1.0
    }
}

fn load_sounds(assets_root: &Path) -> BTreeMap<&'static str, Arc<[u8]>> {
    SOUND_IDS
        .iter()
        .filter_map(|id| {
            let bytes = ["final", "generated"].into_iter().find_map(|source| {
                fs::read(
                    assets_root
                        .join(source)
                        .join("audio")
                        .join(format!("{id}.wav")),
                )
                .ok()
            })?;
            let bytes = Arc::<[u8]>::from(bytes);
            Decoder::try_from(Cursor::new(Arc::clone(&bytes)))
                .ok()
                .map(|_| (*id, bytes))
        })
        .collect()
}

#[must_use]
pub const fn sound_for_cue(cue: AudioCue) -> Option<&'static str> {
    match cue {
        AudioCue::AquariumHum => Some(UNDERWATER_LOOP),
        AudioCue::Bubble => Some(BUBBLES_1),
        AudioCue::SwimWake => Some(SWIM_WAKE),
        AudioCue::Wake => Some(CREATURE_WAKE),
        AudioCue::FoodDrop => Some(FOOD_DROP),
        AudioCue::FoodEat => Some(FOOD_EAT),
        AudioCue::FoodReject => Some(FOOD_REJECT),
        AudioCue::Sand => Some(SAND_DISTURB),
        AudioCue::ToyImpact => Some(TOY_IMPACT),
        AudioCue::BallNudge => Some(BALL_NUDGE),
        AudioCue::BellRing => Some(BELL_RING),
        AudioCue::SockRustle => Some(SOCK_RUSTLE),
        AudioCue::Affection => Some(CREATURE_AFFECTION),
        AudioCue::Curious => Some(CREATURE_CURIOUS),
        AudioCue::Mrr => Some(CREATURE_MRR),
        AudioCue::Annoyed => Some(CREATURE_ANNOYED),
        AudioCue::Sleep => Some(CREATURE_SLEEP),
        AudioCue::UiReject => Some(UI_SELECT),
        AudioCue::UiConfirm => Some(UI_CONFIRM),
        AudioCue::BubbleAlternate => Some(BUBBLES_2),
        AudioCue::CaveSettle => Some(CAVE_SETTLE),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn repository_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn semantic_cues_resolve_only_to_authored_audio() {
        assert_eq!(sound_for_cue(AudioCue::UiReject), Some(UI_SELECT));
        assert_eq!(sound_for_cue(AudioCue::FoodReject), Some(FOOD_REJECT));
        assert_eq!(sound_for_cue(AudioCue::Affection), Some(CREATURE_AFFECTION));
        assert_eq!(sound_for_cue(AudioCue::ToyImpact), Some(TOY_IMPACT));
        assert_eq!(sound_for_cue(AudioCue::BallNudge), Some(BALL_NUDGE));
        assert_eq!(sound_for_cue(AudioCue::BellRing), Some(BELL_RING));
        assert_eq!(sound_for_cue(AudioCue::SockRustle), Some(SOCK_RUSTLE));
        assert_eq!(sound_for_cue(AudioCue::Sleep), Some(CREATURE_SLEEP));
        assert_eq!(sound_for_cue(AudioCue::Curious), Some(CREATURE_CURIOUS));
        assert_eq!(sound_for_cue(AudioCue::CaveSettle), Some(CAVE_SETTLE));
    }

    #[test]
    fn queued_relationship_cancellation_preserves_ui_and_physical_audio() {
        let relationship = SemanticOwner::ActionRelationship(9);
        let mut commands = vec![
            AudioCommand::play(
                relationship,
                PresentationChannel::CreatureVoice,
                AudioCue::Mrr,
                450,
            ),
            AudioCommand::play(
                SemanticOwner::Ordinary,
                PresentationChannel::Ui,
                AudioCue::UiConfirm,
                450,
            ),
            AudioCommand::play(
                SemanticOwner::DirectOutcome,
                PresentationChannel::Physical,
                AudioCue::FoodEat,
                700,
            ),
            AudioCommand::CancelOwner {
                owner: relationship,
            },
        ];
        reduce_audio_commands(&mut commands);
        assert!(!commands.iter().any(|command| matches!(
            command,
            AudioCommand::Play { owner, .. } if *owner == relationship
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            AudioCommand::Play {
                channel: PresentationChannel::Ui,
                ..
            }
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            AudioCommand::Play {
                channel: PresentationChannel::Physical,
                ..
            }
        )));
    }

    #[test]
    fn direct_outcome_removes_only_lower_priority_creature_voice() {
        let mut commands = vec![
            AudioCommand::play(
                SemanticOwner::StandaloneRelationship(
                    beastie_core::RelationshipMotifKey::PlayerReturns,
                ),
                PresentationChannel::CreatureVoice,
                AudioCue::Affection,
                600,
            ),
            AudioCommand::play(
                SemanticOwner::Ordinary,
                PresentationChannel::Ambience,
                AudioCue::Bubble,
                700,
            ),
            AudioCommand::CancelLowerPriority {
                owner: SemanticOwner::DirectOutcome,
                channel: PresentationChannel::CreatureVoice,
            },
        ];
        reduce_audio_commands(&mut commands);
        assert_eq!(
            commands
                .iter()
                .filter(|command| matches!(command, AudioCommand::Play { .. }))
                .count(),
            1
        );
        assert!(commands.iter().any(|command| matches!(
            command,
            AudioCommand::Play {
                channel: PresentationChannel::Ambience,
                ..
            }
        )));
    }

    #[test]
    fn authored_audio_decodes_without_opening_an_audio_device() {
        assert_eq!(load_sounds(&repository_assets()).len(), SOUND_IDS.len());
        assert!(load_sounds(Path::new("missing-assets")).is_empty());
    }

    #[test]
    fn ambient_bubbles_are_deterministic_sparse_and_avoid_long_repeats() {
        let mut left = AmbientBubbleSchedule::new(42, 0);
        let mut right = AmbientBubbleSchedule::new(42, 0);
        let mut emitted = Vec::new();
        for now_ms in (0_u64..=60_000).step_by(250) {
            let a = left.poll(now_ms, false);
            let b = right.poll(now_ms, false);
            assert_eq!(a, b);
            if let Some(sound) = a {
                emitted.push((now_ms, sound));
            }
        }
        assert!(emitted.len() >= 3);
        assert!(
            emitted
                .windows(2)
                .all(|pair| pair[1].0.saturating_sub(pair[0].0) >= 6_000)
        );
        assert!(
            emitted
                .windows(3)
                .all(|run| { !(run[0].1 == run[1].1 && run[1].1 == run[2].1) })
        );
        assert!(emitted.windows(2).any(|pair| pair[0].1 == pair[1].1));
        assert!(emitted.windows(2).any(|pair| pair[0].1 != pair[1].1));
    }

    #[test]
    fn busy_audio_postpones_bubbles_and_ducking_uses_authored_decibels() {
        let mut schedule = AmbientBubbleSchedule::new(9, 0);
        assert_eq!(schedule.poll(20_000, true), None);
        assert_eq!(schedule.poll(20_500, false), None);
        assert!(schedule.poll(21_000, false).is_some());
        assert!((duck_gain(false, true) - 0.630_957).abs() < 0.000_001);
        assert!((duck_gain(true, false) - 0.446_684).abs() < 0.000_001);
        assert_eq!(duck_gain(false, false), 1.0);

        let mut envelope = GainEnvelope::default();
        let attack = envelope.advance(duck_gain(true, false), 17, 45, 260);
        assert!(attack < 1.0 && attack > duck_gain(true, false));
        let settled = envelope.advance(duck_gain(true, false), 28, 45, 260);
        assert_eq!(settled, duck_gain(true, false));
        let release = envelope.advance(1.0, 17, 45, 260);
        assert!(release > settled && release < 1.0);
        assert_eq!(envelope.advance(1.0, 243, 45, 260), 1.0);
    }
    fn silent_bank() -> AudioBank {
        AudioBank {
            output: None,
            sounds: load_sounds(&repository_assets()),
            speech: None,
            one_shots: Vec::new(),
            ambience: None,
            effects_gain: 0.7,
            speech_gain: 0.7,
            ambience_duck: 1.0,
            duck_envelope: GainEnvelope::default(),
            creature_envelope: GainEnvelope::default(),
            next_playback_id: 1,
            decisions: Vec::new(),
        }
    }

    #[test]
    fn semantic_mix_keeps_bubbles_ui_and_physical_cues_out_of_voice_priority() {
        let mut bank = silent_bank();
        bank.creature_envelope.value = 0.562_341;
        for cue in [
            AudioCue::Bubble,
            AudioCue::BubbleAlternate,
            AudioCue::UiConfirm,
            AudioCue::UiReject,
            AudioCue::FoodDrop,
            AudioCue::FoodEat,
            AudioCue::FoodReject,
            AudioCue::ToyImpact,
        ] {
            assert_ne!(role_for_cue(cue), MixRole::Creature);
            assert_eq!(bank.gain_for(role_for_cue(cue), 0.7), 0.7 * 0.7);
        }
        assert_eq!(
            bank.gain_for(role_for_cue(AudioCue::Affection), 0.7),
            0.7 * 0.7 * 0.562_341
        );
    }

    #[test]
    fn discarded_commands_and_missing_output_do_not_start_or_duck() {
        let mut bank = silent_bank();
        let mut commands = vec![
            AudioCommand::play(
                SemanticOwner::Ordinary,
                PresentationChannel::CreatureVoice,
                AudioCue::Affection,
                700,
            ),
            AudioCommand::CancelOwner {
                owner: SemanticOwner::Ordinary,
            },
        ];
        bank.play_queued(&mut commands);
        bank.update_ducking(100);
        assert_eq!(bank.ambience_duck(), 1.0);
        assert!(
            bank.take_decisions()
                .iter()
                .any(|d| d.outcome == "discarded_arbitration")
        );
        bank.play_queued(&mut vec![AudioCommand::play(
            SemanticOwner::Ordinary,
            PresentationChannel::CreatureVoice,
            AudioCue::Affection,
            700,
        )]);
        bank.update_ducking(100);
        assert_eq!(bank.ambience_duck(), 1.0);
        assert!(bank.playback_snapshot().is_empty());
        assert!(
            bank.take_decisions()
                .iter()
                .any(|d| d.outcome == "discarded_no_output")
        );
        bank.sounds.clear();
        bank.play_queued(&mut vec![AudioCommand::play(
            SemanticOwner::Ordinary,
            PresentationChannel::Physical,
            AudioCue::FoodEat,
            700,
        )]);
        assert!(
            bank.take_decisions()
                .iter()
                .any(|d| d.outcome == "discarded_missing_asset")
        );
    }

    #[test]
    fn vocal_duck_finishes_at_authored_attack_and_release_boundaries() {
        let mut envelope = GainEnvelope::default();
        let target = duck_gain(false, true);
        assert!(envelope.advance(target, 17, 35, 180) > target);
        assert!(envelope.advance(target, 17, 35, 180) > target);
        assert_eq!(envelope.advance(target, 1, 35, 180), target);
        for _ in 0..10 {
            envelope.advance(1.0, 17, 35, 180);
        }
        assert!(envelope.value < 1.0);
        assert_eq!(envelope.advance(1.0, 10, 35, 180), 1.0);
    }
}
