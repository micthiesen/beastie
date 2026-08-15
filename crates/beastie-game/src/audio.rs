use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use beastie_core::{GameEvent, SpeechAttention};
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
    "object/toy-impact",
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
    ambience: Option<Player>,
    effects_gain: f32,
    speech_gain: f32,
    ambience_duck: f32,
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

    pub fn play_queued(&self, queued: &mut Vec<&'static str>) {
        let Some(output) = &self.output else {
            queued.clear();
            return;
        };
        for id in queued.drain(..) {
            let Some(bytes) = self.sounds.get(id) else {
                continue;
            };
            if let Ok(player) = rodio::play(output.mixer(), Cursor::new(Arc::clone(bytes))) {
                let base = if id.starts_with("ui/") { 0.45 } else { 0.7 };
                player.set_volume(base * self.effects_gain);
                player.detach();
            }
        }
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

    pub fn update_ducking(&mut self, one_shot_active: bool) {
        let speech_active = self.speech_active();
        self.ambience_duck = duck_gain(speech_active, one_shot_active);
        if let Some(player) = &self.ambience {
            player.set_volume(0.35 * self.effects_gain * self.ambience_duck);
        }
    }

    #[must_use]
    pub fn speech_active(&self) -> bool {
        self.speech.as_ref().is_some_and(|player| !player.empty())
    }
}

#[derive(Debug, Clone)]
pub struct AmbientBubbleSchedule {
    seed: u64,
    sequence: u64,
    next_at_ms: u64,
}

impl AmbientBubbleSchedule {
    #[must_use]
    pub fn new(seed: u64, now_ms: u64) -> Self {
        let mut schedule = Self {
            seed,
            sequence: 0,
            next_at_ms: now_ms,
        };
        schedule.schedule_after(now_ms);
        schedule
    }

    pub fn poll(&mut self, now_ms: u64, busy: bool) -> Option<&'static str> {
        if now_ms < self.next_at_ms {
            return None;
        }
        if busy {
            self.next_at_ms = now_ms.saturating_add(1_000);
            return None;
        }
        let sound = if self.sequence.is_multiple_of(2) {
            BUBBLES_1
        } else {
            BUBBLES_2
        };
        self.schedule_after(now_ms);
        Some(sound)
    }

    fn schedule_after(&mut self, now_ms: u64) {
        let mixed = splitmix64(self.seed ^ self.sequence.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        self.sequence = self.sequence.wrapping_add(1);
        self.next_at_ms = now_ms.saturating_add(4_000 + mixed % 9_001);
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
pub fn sound_for_event(event: &GameEvent) -> Option<&'static str> {
    match event {
        GameEvent::FoodDropped { .. } => Some(FOOD_DROP),
        GameEvent::FoodDropRejected(_) => Some(FOOD_REJECT),
        GameEvent::FoodConsumed(_) => Some(FOOD_EAT),
        GameEvent::FoodRejected(_) => Some(FOOD_REJECT),
        GameEvent::FoodSettled(_) => Some(SAND_DISTURB),
        GameEvent::ToyRejected(_) => Some(CREATURE_ANNOYED),
        GameEvent::SleepStarted => Some(CREATURE_SLEEP),
        GameEvent::SleepEnded => Some(CREATURE_WAKE),
        GameEvent::Comforted => Some(CREATURE_AFFECTION),
        GameEvent::ActionPhaseChanged {
            to: beastie_core::ActionPhase::Approach,
            ..
        } => Some(SWIM_WAKE),
        GameEvent::NonverbalAct(beastie_core::NonverbalAct::LeanAgainstPlayer) => {
            Some(CREATURE_MRR)
        }
        GameEvent::FoodExpired(_) => Some(BUBBLES_1),
        GameEvent::SpeechPerceived(SpeechAttention::Glanced | SpeechAttention::Attended) => {
            Some(CREATURE_CURIOUS)
        }
        GameEvent::NonverbalAct(_) => Some(CAVE_SETTLE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use beastie_core::{FoodId, GameEvent};

    use super::*;

    fn repository_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn state_events_map_only_to_authored_one_shots() {
        assert_eq!(
            sound_for_event(&GameEvent::FoodDropRejected(
                beastie_core::FoodDropRejectionReason::AquariumFull,
            )),
            Some(FOOD_REJECT)
        );
        assert_eq!(
            sound_for_event(&GameEvent::FoodRejected(FoodId::Berry)),
            Some(FOOD_REJECT)
        );
        assert_eq!(
            sound_for_event(&GameEvent::Comforted),
            Some(CREATURE_AFFECTION)
        );
        assert_eq!(
            sound_for_event(&GameEvent::SleepStarted),
            Some(CREATURE_SLEEP)
        );
        assert_eq!(sound_for_event(&GameEvent::NeedChanged), None);
        assert_eq!(
            sound_for_event(&GameEvent::SpeechPerceived(SpeechAttention::Attended)),
            Some(CREATURE_CURIOUS)
        );
        assert_eq!(
            sound_for_event(&GameEvent::SpeechPerceived(SpeechAttention::Ignored)),
            None
        );
    }

    #[test]
    fn authored_audio_decodes_without_opening_an_audio_device() {
        assert_eq!(load_sounds(&repository_assets()).len(), SOUND_IDS.len());
        assert!(load_sounds(Path::new("missing-assets")).is_empty());
    }

    #[test]
    fn ambient_bubbles_are_deterministic_rate_limited_and_alternating() {
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
        assert!(emitted.len() >= 4);
        assert!(
            emitted
                .windows(2)
                .all(|pair| pair[1].0.saturating_sub(pair[0].0) >= 4_000)
        );
        assert!(emitted.windows(2).all(|pair| pair[0].1 != pair[1].1));
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
    }
}
