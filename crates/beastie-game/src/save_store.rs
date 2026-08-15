use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::fs::File;

#[derive(Debug, Clone)]
pub struct SaveStore {
    path: PathBuf,
}

impl SaveStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> io::Result<Option<String>> {
        match fs::read_to_string(&self.path) {
            Ok(save) => Ok(Some(save)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let backup = self.path.with_extension("json.bak");
                match fs::read_to_string(backup) {
                    Ok(save) => Ok(Some(save)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
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
        remove_if_present(&backup)?;
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow the epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "beastie-save-store-{}-{unique}",
                std::process::id()
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
            store.load().expect("load should succeed").as_deref(),
            Some("second")
        );
        assert!(!path.with_extension("json.tmp").exists());
        assert!(!path.with_extension("json.bak").exists());
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
            store.load().expect("backup load should succeed").as_deref(),
            Some("last-good")
        );
        fs::remove_dir_all(directory).expect("test directory should be removable");
    }
}
