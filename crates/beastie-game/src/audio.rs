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

pub struct AudioBank {
    output: Option<MixerDeviceSink>,
    sounds: BTreeMap<&'static str, Arc<[u8]>>,
    speech: Option<Player>,
    one_shots: Vec<ActiveOneShot>,
    ambience: Option<Player>,
    effects_gain: f32,
    speech_gain: f32,
    ambience_duck: f32,
}

struct ActiveOneShot {
    id: &'static str,
    player: Player,
    base_gain: f32,
    owner: SemanticOwner,
    channel: PresentationChannel,
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
        }
    }

    pub fn ensure_ambience(&mut self) {
        if self.ambience.is_some() {
            return;
        }
        let (Some(output), Some(bytes)) = (&self.output, self.sounds.get(UNDERWATER_LOOP)) else {
            return;
        };
        let Ok(decoder) = Decoder::try_from(Cursor::new(Arc::clone(bytes))) else {
            return;
        };
        let player = Player::connect_new(output.mixer());
        player.set_volume(0.35 * self.effects_gain * self.ambience_duck);
        player.append(decoder.repeat_infinite());
        self.ambience = Some(player);
    }

    pub fn set_gains(&mut self, effects: f32, speech: f32) {
        self.effects_gain = effects.clamp(0.0, 1.0);
        self.speech_gain = speech.clamp(0.0, 1.0);
        if let Some(player) = &self.speech {
            player.set_volume(self.speech_gain);
        }
        if let Some(player) = &self.ambience {
            player.set_volume(0.35 * self.effects_gain * self.ambience_duck);
        }
    }

    pub fn play_queued(&mut self, queued: &mut Vec<AudioCommand>) {
        reduce_audio_commands(queued);
        let mut plays = Vec::new();
        for command in queued.drain(..) {
            match command {
                AudioCommand::CancelOwner { owner } => {
                    self.cancel_where(|sound| sound.owner == owner);
                }
                AudioCommand::CancelLowerPriority { owner, channel } => {
                    self.cancel_where(|sound| {
                        sound.channel == channel
                            && owner_priority(sound.owner) < owner_priority(owner)
                    });
                }
                AudioCommand::Play { .. } => plays.push(command),
            }
        }
        let Some(output) = &self.output else {
            return;
        };
        for command in plays {
            if let AudioCommand::Play {
                owner,
                channel,
                cue,
                gain_milli,
            } = command
            {
                let Some(id) = sound_for_cue(cue) else {
                    continue;
                };
                if id == SWIM_WAKE
                    && self
                        .one_shots
                        .iter()
                        .any(|sound| sound.id == SWIM_WAKE && !sound.player.empty())
                {
                    continue;
                }
                let Some(bytes) = self.sounds.get(id) else {
                    continue;
                };
                if let Ok(player) = rodio::play(output.mixer(), Cursor::new(Arc::clone(bytes))) {
                    let base = f32::from(gain_milli.min(1_000)) / 1_000.0;
                    player.set_volume(base * self.effects_gain);
                    self.one_shots.push(ActiveOneShot {
                        id,
                        player,
                        base_gain: base,
                        owner,
                        channel,
                    });
                }
            }
        }
    }

    fn cancel_where(&mut self, predicate: impl Fn(&ActiveOneShot) -> bool) {
        self.one_shots.retain(|sound| {
            if predicate(sound) {
                sound.player.stop();
                false
            } else {
                true
            }
        });
    }

    pub fn play_speech(&mut self, wav: Arc<[u8]>) {
        self.stop_speech();
        let Some(output) = &self.output else {
            return;
        };
        if let Ok(player) = rodio::play(output.mixer(), Cursor::new(wav)) {
            player.set_volume(self.speech_gain);
            self.speech = Some(player);
        }
    }

    pub fn stop_speech(&mut self) {
        if let Some(player) = self.speech.take() {
            player.stop();
        }
    }

    pub fn update_ducking(&mut self, delta_ms: u64, queued_one_shot: bool) {
        self.one_shots.retain(|sound| !sound.player.empty());
        let speech_active = self.speech_active();
        let one_shot_active = queued_one_shot || !self.one_shots.is_empty();
        let target = duck_gain(speech_active, one_shot_active);
        let transition_ms = if target < self.ambience_duck {
            if speech_active { 45 } else { 35 }
        } else if self.ambience_duck <= duck_gain(true, false) + 0.001 {
            260
        } else {
            180
        };
        self.ambience_duck = approach_gain(self.ambience_duck, target, delta_ms, transition_ms);
        if let Some(player) = &self.ambience {
            player.set_volume(0.35 * self.effects_gain * self.ambience_duck);
        }
        let speech_effect_duck = if speech_active { 0.562_341 } else { 1.0 };
        for sound in &self.one_shots {
            sound
                .player
                .set_volume(sound.base_gain * self.effects_gain * speech_effect_duck);
        }
    }

    #[must_use]
    pub fn speech_active(&self) -> bool {
        self.speech.as_ref().is_some_and(|player| !player.empty())
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

fn approach_gain(current: f32, target: f32, delta_ms: u64, transition_ms: u64) -> f32 {
    if delta_ms >= transition_ms || transition_ms == 0 {
        return target;
    }
    current + (target - current) * delta_ms as f32 / transition_ms as f32
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

        let attack = approach_gain(1.0, duck_gain(true, false), 17, 35);
        assert!(attack < 1.0 && attack > duck_gain(true, false));
        let settled = approach_gain(attack, duck_gain(true, false), 35, 35);
        assert_eq!(settled, duck_gain(true, false));
        let release = approach_gain(settled, 1.0, 17, 180);
        assert!(release > settled && release < 1.0);
    }
}
