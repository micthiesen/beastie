use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextScale {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSpeed {
    Instant,
    Normal,
    Slow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKey {
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl BindingKey {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Escape => "Esc",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyBindings {
    pub food: BindingKey,
    pub play: BindingKey,
    pub comfort: BindingKey,
    pub settings: BindingKey,
    pub cancel: BindingKey,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            food: BindingKey::F2,
            play: BindingKey::F3,
            comfort: BindingKey::F4,
            settings: BindingKey::F5,
            cancel: BindingKey::Escape,
        }
    }
}

impl KeyBindings {
    #[must_use]
    pub fn has_conflict(&self) -> bool {
        let bindings = [
            self.food,
            self.play,
            self.comfort,
            self.settings,
            self.cancel,
        ];
        bindings
            .iter()
            .enumerate()
            .any(|(index, binding)| bindings[index + 1..].contains(binding))
    }

    pub fn key_for(&self, action: beastie_view::BindableAction) -> BindingKey {
        match action {
            beastie_view::BindableAction::Food => self.food,
            beastie_view::BindableAction::Play => self.play,
            beastie_view::BindableAction::Comfort => self.comfort,
            beastie_view::BindableAction::Settings => self.settings,
            beastie_view::BindableAction::Cancel => self.cancel,
        }
    }

    pub fn set_key(&mut self, action: beastie_view::BindableAction, key: BindingKey) {
        match action {
            beastie_view::BindableAction::Food => self.food = key,
            beastie_view::BindableAction::Play => self.play = key,
            beastie_view::BindableAction::Comfort => self.comfort = key,
            beastie_view::BindableAction::Settings => self.settings = key,
            beastie_view::BindableAction::Cancel => self.cancel = key,
        }
    }

    pub fn action_for(&self, key: BindingKey) -> Option<beastie_view::BindableAction> {
        use beastie_view::BindableAction;
        [
            BindableAction::Food,
            BindableAction::Play,
            BindableAction::Comfort,
            BindableAction::Settings,
            BindableAction::Cancel,
        ]
        .into_iter()
        .find(|action| self.key_for(*action) == key)
    }

    pub fn rebind_swapping(&mut self, action: beastie_view::BindableAction, key: BindingKey) {
        let previous = self.key_for(action);
        if let Some(other) = self.action_for(key)
            && other != action
        {
            self.set_key(other, previous);
        }
        self.set_key(action, key);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserSettings {
    pub version: u32,
    /// Preferred integer logical scale while windowed. The OS may provide a
    /// larger drawable on HiDPI screens; viewport calculation remains based on
    /// drawable pixels so the final image is never fractionally sampled.
    pub window_scale: u8,
    pub fullscreen: bool,
    pub effects_volume: u8,
    pub speech_volume: u8,
    pub voice_enabled: bool,
    pub transcript_enabled: bool,
    pub subtitles: bool,
    pub text_scale: TextScale,
    pub text_speed: TextSpeed,
    pub bindings: KeyBindings,
    pub reduced_motion: bool,
    pub reduced_flashes: bool,
    pub reduced_shake: bool,
    pub pixel_grid: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            window_scale: 3,
            fullscreen: false,
            effects_volume: 70,
            speech_volume: 70,
            voice_enabled: true,
            transcript_enabled: false,
            subtitles: true,
            text_scale: TextScale::Medium,
            text_speed: TextSpeed::Normal,
            bindings: KeyBindings::default(),
            reduced_motion: false,
            reduced_flashes: false,
            reduced_shake: false,
            pixel_grid: false,
        }
    }
}

impl UserSettings {
    fn sanitize(mut self) -> Self {
        self.version = SETTINGS_VERSION;
        self.window_scale = self.window_scale.clamp(1, 6);
        self.effects_volume = self.effects_volume.min(100);
        self.speech_volume = self.speech_volume.min(100);
        if self.bindings.has_conflict() {
            self.bindings = KeyBindings::default();
        }
        self
    }

    #[must_use]
    pub fn effects_gain(&self) -> f32 {
        f32::from(self.effects_volume) / 100.0
    }

    #[must_use]
    pub fn speech_gain(&self) -> f32 {
        f32::from(self.speech_volume) / 100.0
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> io::Result<UserSettings> {
        match read_settings(&self.path) {
            Ok(settings) => Ok(settings),
            Err(primary_error)
                if matches!(
                    primary_error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidData
                ) =>
            {
                match read_settings(&self.path.with_extension("json.bak")) {
                    Ok(settings) => Ok(settings),
                    Err(backup_error) if backup_error.kind() == io::ErrorKind::NotFound => {
                        if primary_error.kind() == io::ErrorKind::NotFound {
                            Ok(UserSettings::default())
                        } else {
                            Err(primary_error)
                        }
                    }
                    Err(backup_error) => Err(backup_error),
                }
            }
            Err(error) => Err(error),
        }
    }

    pub fn store(&self, settings: &UserSettings) -> io::Result<()> {
        let json =
            serde_json::to_vec_pretty(&settings.clone().sanitize()).map_err(io::Error::other)?;
        atomic_replace(&self.path, &json)
    }
}

fn read_settings(path: &Path) -> io::Result<UserSettings> {
    let source = fs::read_to_string(path)?;
    serde_json::from_str::<UserSettings>(&source)
        .map(UserSettings::sanitize)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn atomic_replace(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    let backup = path.with_extension("json.bak");
    let had_existing = path.exists();
    if had_existing {
        remove_if_present(&backup)?;
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        return Err(error);
    }
    sync_directory(parent)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "beastie-settings-{}-{}.json",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn missing_settings_use_accessible_defaults() {
        let settings = SettingsStore::new(path()).load().expect("defaults");
        assert_eq!(settings, UserSettings::default());
        assert!(settings.subtitles);
    }

    #[test]
    fn persisted_values_are_bounded() {
        let path = path();
        let store = SettingsStore::new(path.clone());
        let settings = UserSettings {
            window_scale: 99,
            effects_volume: 200,
            speech_volume: 255,
            ..UserSettings::default()
        };
        store.store(&settings).expect("store");
        store.store(&settings).expect("repeat store");
        let loaded = store.load().expect("load");
        assert_eq!(loaded.window_scale, 6);
        assert_eq!(loaded.effects_volume, 100);
        assert_eq!(loaded.speech_volume, 100);
        assert!(!path.with_extension("json.tmp").exists());
        fs::remove_file(&path).expect("cleanup");
        fs::remove_file(path.with_extension("json.bak")).expect("cleanup backup");
    }

    #[test]
    fn subtitle_preference_round_trips_and_defaults_on() {
        let path = path();
        let store = SettingsStore::new(path.clone());
        let settings = UserSettings {
            subtitles: false,
            ..UserSettings::default()
        };
        store.store(&settings).expect("store subtitles preference");
        assert!(!store.load().expect("load subtitles preference").subtitles);
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn malformed_settings_are_reported_instead_of_silently_reset() {
        let path = path();
        fs::write(&path, b"not json").expect("fixture");
        assert_eq!(
            SettingsStore::new(path.clone())
                .load()
                .expect_err("malformed")
                .kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(&path).expect("cleanup");
        fs::remove_file(path.with_extension("json.bak")).ok();
    }

    #[test]
    fn malformed_primary_recovers_valid_backup() {
        let path = path();
        let store = SettingsStore::new(path.clone());
        let expected = UserSettings {
            subtitles: false,
            effects_volume: 37,
            ..UserSettings::default()
        };
        store.store(&expected).expect("store primary");
        store.store(&expected).expect("create backup");
        fs::write(&path, b"truncated").expect("corrupt primary");

        assert_eq!(store.load().expect("recover backup"), expected);

        fs::remove_file(&path).expect("cleanup primary");
        fs::remove_file(path.with_extension("json.bak")).expect("cleanup backup");
    }

    #[test]
    fn conflicting_bindings_reset_to_safe_non_printable_defaults() {
        let path = path();
        let store = SettingsStore::new(path.clone());
        let mut settings = UserSettings::default();
        settings.bindings.food = BindingKey::F3;
        settings.bindings.play = BindingKey::F3;
        store.store(&settings).expect("store");
        assert_eq!(store.load().expect("load").bindings, KeyBindings::default());
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rebinding_swaps_an_occupied_key_without_creating_conflicts() {
        let mut bindings = KeyBindings::default();
        bindings.rebind_swapping(beastie_view::BindableAction::Food, BindingKey::F3);
        assert_eq!(bindings.food, BindingKey::F3);
        assert_eq!(bindings.play, BindingKey::F2);
        assert!(!bindings.has_conflict());
    }
}
