use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SaveStore {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadedSave {
    Primary(String),
    Backup(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSource {
    BeforeReset,
    LastGoodBeforeReset,
    LastGood,
}

impl BackupSource {
    const fn extension(self) -> &'static str {
        match self {
            Self::BeforeReset => "json.reset",
            Self::LastGoodBeforeReset => "json.reset.bak",
            Self::LastGood => "json.bak",
        }
    }

    const fn recovered_extension(self) -> Option<&'static str> {
        match self {
            Self::BeforeReset => Some("json.reset-recovered"),
            Self::LastGoodBeforeReset => Some("json.reset-good-recovered"),
            Self::LastGood => None,
        }
    }
}

impl SaveStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load_recoverable(&self) -> io::Result<Option<LoadedSave>> {
        match fs::read_to_string(&self.path) {
            Ok(save) => Ok(Some(LoadedSave::Primary(save))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let backup = self.path.with_extension("json.bak");
                match fs::read_to_string(backup) {
                    Ok(save) => Ok(Some(LoadedSave::Backup(save))),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    pub fn load_backup(&self) -> io::Result<Option<String>> {
        self.load_backup_source(BackupSource::LastGood)
    }

    pub fn load_backup_source(&self, source: BackupSource) -> io::Result<Option<String>> {
        match fs::read_to_string(self.path.with_extension(source.extension())) {
            Ok(save) => Ok(Some(save)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Promotes the last-good backup while preserving the displaced primary as
    /// a visible `.corrupt` generation for support or manual recovery.
    pub fn promote_backup(&self) -> io::Result<bool> {
        self.promote_backup_source(BackupSource::LastGood)
    }

    /// Caller validates the selected bytes before promotion. A reset restore point is consumed
    /// only after the primary is installed, while its bytes remain in a recovered generation.
    pub fn promote_backup_source(&self, source: BackupSource) -> io::Result<bool> {
        let backup = self.path.with_extension(source.extension());
        if !backup.is_file() {
            return Ok(false);
        }
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let displaced = self.path.with_extension("json.corrupt");
        let temporary = self.path.with_extension("json.recovery.tmp");
        let recovered = source
            .recovered_extension()
            .map(|extension| self.path.with_extension(extension));
        if let Some(recovered) = &recovered {
            remove_if_present(recovered)?;
        }
        remove_if_present(&displaced)?;
        remove_if_present(&temporary)?;
        let had_primary = self.path.exists();
        if had_primary {
            fs::rename(&self.path, &displaced)?;
        }
        if let Err(error) = fs::copy(&backup, &temporary)
            .and_then(|_| File::open(&temporary)?.sync_all())
            .and_then(|_| fs::rename(&temporary, &self.path))
        {
            remove_if_present(&temporary)?;
            if had_primary {
                let _ = fs::rename(&displaced, &self.path);
            }
            return Err(error);
        }
        if let Err(error) = sync_directory(parent).and_then(|()| {
            if let Some(recovered) = &recovered {
                fs::rename(&backup, recovered)?;
                if let Err(error) = sync_directory(parent) {
                    let _ = fs::rename(recovered, &backup);
                    return Err(error);
                }
            }
            Ok(())
        }) {
            // Recovery failure leaves the pending source available for another attempt.
            let _ = fs::remove_file(&self.path);
            if had_primary {
                let _ = fs::rename(&displaced, &self.path);
            }
            return Err(error);
        }
        Ok(true)
    }

    /// Keeps both current and last-good generations as a pending UI restore point.
    /// Resetting again before a replacement save exists preserves that restore point.
    pub fn reset(&self) -> io::Result<bool> {
        let reset = self.path.with_extension("json.reset");
        let reset_good = self.path.with_extension("json.reset.bak");
        let last_good = self.path.with_extension("json.bak");
        if !self.path.exists() && !last_good.exists() {
            remove_if_present(&self.path.with_extension("json.tmp"))?;
            return Ok(reset.is_file());
        }
        remove_if_present(&reset)?;
        remove_if_present(&reset_good)?;
        let mut moved = false;
        if self.path.exists() {
            fs::rename(&self.path, &reset)?;
            moved = true;
        }
        remove_if_present(&self.path.with_extension("json.tmp"))?;
        if last_good.exists() {
            // The primary may be corrupt. Retain the independent last-good generation too.
            fs::rename(last_good, if moved { reset_good } else { reset })?;
            moved = true;
        }
        Ok(moved)
    }

    pub fn store(&self, save: &str) -> io::Result<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temporary = self.path.with_extension("json.tmp");
        let backup = self.path.with_extension("json.bak");

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(save.as_bytes())?;
        file.sync_all()?;
        drop(file);

        let had_existing = self.path.exists();
        if had_existing {
            remove_if_present(&backup)?;
            fs::rename(&self.path, &backup)?;
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            if had_existing {
                let _ = fs::rename(&backup, &self.path);
            }
            return Err(error);
        }
        // Keep one last-good generation visible for recovery and support.
        sync_directory(parent)
    }
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow the epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "beastie-save-store-{}-{unique}-{}",
                std::process::id(),
                TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ))
            .join("save.json")
    }

    #[test]
    fn replaces_a_save_without_leaving_temporary_files() {
        let path = test_path();
        let directory = path.parent().expect("test path has a parent").to_owned();
        let store = SaveStore::new(path.clone());
        store.store("first").expect("first write should succeed");
        store.store("second").expect("replacement should succeed");

        assert_eq!(
            store.load_recoverable().expect("load should succeed"),
            Some(LoadedSave::Primary("second".to_owned()))
        );
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(
            fs::read_to_string(path.with_extension("json.bak")).expect("backup"),
            "first"
        );
        fs::remove_dir_all(directory).expect("test directory should be removable");
    }

    #[test]
    fn loads_backup_if_replacement_was_interrupted() {
        let path = test_path();
        let directory = path.parent().expect("test path has a parent").to_owned();
        fs::create_dir_all(&directory).expect("test directory should be created");
        fs::write(path.with_extension("json.bak"), "last-good")
            .expect("backup fixture should be written");
        fs::write(path.with_extension("json.tmp"), "unfinished")
            .expect("temporary fixture should be written");

        let store = SaveStore::new(path);
        assert_eq!(
            store
                .load_recoverable()
                .expect("backup load should succeed"),
            Some(LoadedSave::Backup("last-good".to_owned()))
        );
        fs::remove_dir_all(directory).expect("test directory should be removable");
    }

    #[test]
    fn reset_is_recoverable_and_removes_incomplete_generations() {
        let path = test_path();
        let directory = path.parent().expect("test path has a parent").to_owned();
        let store = SaveStore::new(path.clone());
        store.store("current").expect("store");
        fs::write(path.with_extension("json.tmp"), "partial").expect("temporary");
        assert!(store.reset().expect("reset"));
        assert!(!path.exists());
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(
            fs::read_to_string(path.with_extension("json.reset")).expect("reset generation"),
            "current"
        );
        assert!(store.reset().expect("reset without a replacement save"));
        assert_eq!(
            store
                .load_backup_source(BackupSource::BeforeReset)
                .unwrap()
                .as_deref(),
            Some("current")
        );
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn backup_promotion_preserves_both_recovered_and_displaced_generations() {
        let path = test_path();
        let directory = path.parent().expect("test path has a parent").to_owned();
        fs::create_dir_all(&directory).expect("directory");
        fs::write(&path, "broken-current").expect("primary");
        fs::write(path.with_extension("json.bak"), "last-good").expect("backup");
        let store = SaveStore::new(path.clone());
        assert!(store.promote_backup().expect("promote"));
        assert_eq!(fs::read_to_string(&path).expect("primary"), "last-good");
        assert_eq!(
            fs::read_to_string(path.with_extension("json.corrupt")).expect("displaced"),
            "broken-current"
        );
        assert_eq!(
            fs::read_to_string(path.with_extension("json.bak")).expect("backup retained"),
            "last-good"
        );
        fs::remove_dir_all(directory).expect("cleanup");
    }
}
