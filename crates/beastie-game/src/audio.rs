use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use beastie_core::GameEvent;
#[cfg(feature = "experimental-gpl-tts")]
use rodio::Player;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink};

pub const UI_SELECT: &str = "ui/select";
pub const UI_CONFIRM: &str = "ui/confirm";
const CREATURE_MRR: &str = "creature/mrr";
const CREATURE_ANNOYED: &str = "creature/annoyed";
const CREATURE_SLEEP: &str = "creature/sleep";

const SOUND_IDS: &[&str] = &[
    UI_SELECT,
    UI_CONFIRM,
    CREATURE_MRR,
    CREATURE_ANNOYED,
    CREATURE_SLEEP,
];

pub struct AudioBank {
    output: Option<MixerDeviceSink>,
    sounds: BTreeMap<&'static str, Arc<[u8]>>,
    #[cfg(feature = "experimental-gpl-tts")]
    speech: Option<Player>,
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
            #[cfg(feature = "experimental-gpl-tts")]
            speech: None,
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
                player.set_volume(if id.starts_with("ui/") { 0.32 } else { 0.48 });
                player.detach();
            }
        }
    }

    #[cfg(feature = "experimental-gpl-tts")]
    pub fn play_speech(&mut self, wav: Arc<[u8]>) {
        self.stop_speech();
        let Some(output) = &self.output else {
            return;
        };
        if let Ok(player) = rodio::play(output.mixer(), Cursor::new(wav)) {
            player.set_volume(0.48);
            self.speech = Some(player);
        }
    }

    #[cfg(feature = "experimental-gpl-tts")]
    pub fn stop_speech(&mut self) {
        if let Some(player) = self.speech.take() {
            player.stop();
        }
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
        GameEvent::FoodRejected(_) | GameEvent::ToyRejected(_) => Some(CREATURE_ANNOYED),
        GameEvent::SleepStarted => Some(CREATURE_SLEEP),
        GameEvent::Comforted => Some(CREATURE_MRR),
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
            sound_for_event(&GameEvent::FoodRejected(FoodId::Berry)),
            Some(CREATURE_ANNOYED)
        );
        assert_eq!(sound_for_event(&GameEvent::Comforted), Some(CREATURE_MRR));
        assert_eq!(
            sound_for_event(&GameEvent::SleepStarted),
            Some(CREATURE_SLEEP)
        );
        assert_eq!(sound_for_event(&GameEvent::NeedChanged), None);
    }

    #[test]
    fn authored_audio_decodes_without_opening_an_audio_device() {
        assert_eq!(load_sounds(&repository_assets()).len(), SOUND_IDS.len());
        assert!(load_sounds(Path::new("missing-assets")).is_empty());
    }
}
