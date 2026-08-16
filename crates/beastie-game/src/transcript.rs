use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use beastie_protocol::TranscriptRecord;

#[derive(Debug, Clone)]
pub struct TranscriptStore {
    path: PathBuf,
    enabled: bool,
}

impl TranscriptStore {
    #[must_use]
    pub fn new(path: PathBuf, enabled: bool) -> Self {
        Self { path, enabled }
    }

    pub fn append(&self, record: &TranscriptRecord) -> io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, record).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        file.sync_all()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Rewrites only records that still validate as the protocol's bounded,
    /// player-text-free transcript type. A damaged partial final line is
    /// ignored so a power loss cannot prevent exporting earlier turns.
    pub fn export(&self, destination: &Path) -> io::Result<usize> {
        let source = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return write_export(destination, &[]);
            }
            Err(error) => return Err(error),
        };
        let mut records = Vec::new();
        for line in BufReader::new(source).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(record) = serde_json::from_str::<TranscriptRecord>(&line) {
                records.push(record);
            }
        }
        write_export(destination, &records)
    }
}

fn write_export(destination: &Path, records: &[TranscriptRecord]) -> io::Result<usize> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = destination.with_extension("jsonl.tmp");
    let mut file = File::create(&temporary)?;
    for record in records {
        serde_json::to_writer(&mut file, record).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
    }
    file.sync_all()?;
    drop(file);
    let backup = destination.with_extension("jsonl.bak");
    let had_existing = destination.exists();
    if had_existing {
        remove_if_present(&backup)?;
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(&temporary, destination) {
        if had_existing {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error);
    }
    Ok(records.len())
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use std::collections::BTreeSet;

    use beastie_protocol::{
        DialogueConstraints, DialogueContext, DialogueRequest, Gesture, Idiolect, PROTOCOL_VERSION,
        TranscriptBackend, TranscriptRecord, constrained_fallback_reply,
    };

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "beastie-transcript-{}-{}-{name}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn record() -> TranscriptRecord {
        let mut request = DialogueRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            creature_name: "Mop".to_owned(),
            mood: "fine".to_owned(),
            known_concepts: BTreeSet::new(),
            candidate_memories: Vec::new(),
            candidate_beliefs: Vec::new(),
            idiolect: Idiolect::default(),
            desired_social_act: None,
            input_rejection: None,
            context: DialogueContext::default(),
            interpretation: Default::default(),
            player_said: "private player words".to_owned(),
            constraints: DialogueConstraints {
                max_words: 4,
                allowed_gestures: BTreeSet::from([Gesture::None]),
            },
        };
        let reply = constrained_fallback_reply(&request);
        request.player_said = "must never be persisted".to_owned();
        TranscriptRecord::from_turn(
            42,
            &request,
            Some(&reply),
            TranscriptBackend::Fixture,
            3,
            false,
            false,
            None,
        )
    }

    #[test]
    fn disabled_store_writes_nothing() {
        let path = path("disabled.jsonl");
        TranscriptStore::new(path.clone(), false)
            .append(&record())
            .expect("disabled append");
        assert!(!path.exists());
    }

    #[test]
    fn export_omits_player_text_and_skips_partial_tail() {
        let source = path("source.jsonl");
        let export = path("export.jsonl");
        let store = TranscriptStore::new(source.clone(), true);
        store.append(&record()).expect("append");
        OpenOptions::new()
            .append(true)
            .open(&source)
            .expect("open")
            .write_all(b"{partial")
            .expect("damage tail");
        assert_eq!(store.export(&export).expect("export"), 1);
        assert_eq!(store.export(&export).expect("repeat export"), 1);
        let output = fs::read_to_string(&export).expect("read export");
        assert!(!output.contains("private player words"));
        assert!(!output.contains("must never be persisted"));
        assert_eq!(output.lines().count(), 1);
        fs::remove_file(source).expect("cleanup source");
        fs::remove_file(&export).expect("cleanup export");
        fs::remove_file(export.with_extension("jsonl.bak")).expect("cleanup backup");
    }
}
