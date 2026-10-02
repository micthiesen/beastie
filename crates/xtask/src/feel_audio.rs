//! Offline reconstruction of accepted runtime playback, never a claimed host recording.
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const RATE: usize = 44_100;

#[derive(Clone, Debug, Deserialize)]
struct Source {
    playback_id: u64,
    asset_sha256: String,
    gain: f32,
    looping: bool,
}
#[derive(Deserialize)]
struct Frame {
    playback_ms: u64,
    playback_schema: u32,
    playback: Vec<Source>,
    output_available: bool,
    #[serde(default)]
    decisions: Vec<Decision>,
}

#[derive(Deserialize)]
struct Decision {
    outcome: String,
    playback_id: Option<u64>,
    started: Option<Source>,
}

pub fn reconstruct(directory: &Path, duration: f64) -> Result<()> {
    ensure!(
        duration.is_finite() && (0.0..=3600.0).contains(&duration),
        "invalid reference duration"
    );
    let mut frames = Vec::new();
    for line in BufReader::new(fs::File::open(directory.join("audio.jsonl"))?).lines() {
        let value: serde_json::Value = serde_json::from_str(&line?)?;
        if value["kind"] == "cues" {
            let frame: Frame = serde_json::from_value(value).context(
                "audio trace lacks post-arbitration playback; recapture with current game",
            )?;
            ensure!(frame.playback_schema == 1, "unsupported playback schema");
            ensure!(
                frame.output_available || frame.playback.is_empty(),
                "unavailable output cannot have active playback"
            );
            ensure!(
                frames
                    .last()
                    .is_none_or(|previous: &Frame| previous.playback_ms <= frame.playback_ms),
                "audio trace time moved backwards"
            );
            frames.push(frame);
        }
    }
    ensure!(!frames.is_empty(), "missing playback frames");
    validate_start_coverage(&frames)?;
    let mut assets = BTreeMap::new();
    for source in frames.iter().flat_map(|frame| &frame.playback) {
        if assets.contains_key(&source.asset_sha256) {
            continue;
        }
        let path = verified_asset_path(directory, &source.asset_sha256)?;
        let output = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args([
                "-f",
                "f32le",
                "-acodec",
                "pcm_f32le",
                "-ac",
                "1",
                "-ar",
                "44100",
                "pipe:1",
            ])
            .output()
            .context("decode retained reference source")?;
        ensure!(
            output.status.success(),
            "could not decode retained audio {}",
            path.display()
        );
        ensure!(output.stdout.len() % 4 == 0, "partial reference PCM sample");
        let samples: Vec<f32> = output
            .stdout
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect();
        ensure!(
            !samples.is_empty() && samples.iter().all(|sample| sample.is_finite()),
            "invalid retained audio PCM"
        );
        assets.insert(source.asset_sha256.clone(), samples);
    }
    let mixed = render(&frames, &assets, (duration * RATE as f64).ceil() as usize)?;
    let peak = mixed.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    let clipping_samples = mixed.iter().filter(|sample| sample.abs() > 1.0).count();
    fs::write(
        directory.join("audio-reconstruction.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": 1,
            "mode": "recorded_playback_reconstruction",
            "host_output_captured": false,
            "timing": "frame-clock playback starts/stops and piecewise-constant recorded gains; device buffering is not captured",
            "sample_rate": RATE,
            "output_available_frames": frames.iter().filter(|f| f.output_available).count(),
            "output_unavailable_frames": frames.iter().filter(|f| !f.output_available).count(),
            "retained_assets": assets.len(),
            "pre_limiter_peak": peak,
            "clipping_samples": clipping_samples,
            "limiter": "none; clipping rejects the evidence"
        }))?,
    )?;
    ensure!(
        clipping_samples == 0,
        "reference audio clips at peak {peak}; see audio-reconstruction.json"
    );
    write_pcm(&directory.join("reference-mix.wav"), &mixed)
}

fn verified_asset_path(directory: &Path, hash: &str) -> Result<std::path::PathBuf> {
    ensure!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid audio asset hash"
    );
    let path = directory.join(format!("audio-{hash}.wav"));
    let bytes =
        fs::read(&path).with_context(|| format!("missing retained audio {}", path.display()))?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == hash,
        "retained audio hash mismatch"
    );
    Ok(path)
}

/// A frame snapshot cannot infer a sub-frame source's lifetime. Reject that evidence rather than
/// silently dropping an accepted player. Its start metadata and bytes remain available to inspect.
fn validate_start_coverage(frames: &[Frame]) -> Result<()> {
    let represented: std::collections::BTreeSet<u64> = frames
        .iter()
        .flat_map(|frame| frame.playback.iter().map(|source| source.playback_id))
        .collect();
    let mut starts = std::collections::BTreeSet::new();
    for decision in frames.iter().flat_map(|frame| &frame.decisions) {
        if decision.outcome == "started" {
            let source = decision
                .started
                .as_ref()
                .context("playback start lacks retained source metadata")?;
            ensure!(
                decision.playback_id == Some(source.playback_id),
                "playback start identity mismatch"
            );
            ensure!(
                starts.insert(source.playback_id),
                "playback identity started more than once"
            );
            ensure!(
                represented.contains(&source.playback_id),
                "accepted playback {} started and ended between snapshots; frame-clock reconstruction is insufficient, retained source {}",
                source.playback_id,
                source.asset_sha256
            );
        }
    }
    Ok(())
}

fn sample_at(ms: u64) -> usize {
    (ms as u128 * RATE as u128 / 1000) as usize
}

fn render(frames: &[Frame], assets: &BTreeMap<String, Vec<f32>>, count: usize) -> Result<Vec<f32>> {
    let mut mixed = vec![0.0; count];
    let mut identities: BTreeMap<u64, (usize, String, bool)> = BTreeMap::new();
    let mut ended = std::collections::BTreeSet::new();
    let mut previous = std::collections::BTreeSet::new();
    for (index, frame) in frames.iter().enumerate() {
        let start = sample_at(frame.playback_ms).min(count);
        let end = frames
            .get(index + 1)
            .map_or(count, |next| sample_at(next.playback_ms).min(count));
        let mut active = std::collections::BTreeSet::new();
        for source in &frame.playback {
            ensure!(
                source.gain.is_finite() && (0.0..=1.0).contains(&source.gain),
                "invalid playback gain"
            );
            ensure!(
                active.insert(source.playback_id),
                "duplicate active playback identity"
            );
            ensure!(
                !ended.contains(&source.playback_id),
                "completed playback identity was reused"
            );
            let (origin, hash, looping) = identities.entry(source.playback_id).or_insert((
                start,
                source.asset_sha256.clone(),
                source.looping,
            ));
            ensure!(
                *hash == source.asset_sha256 && *looping == source.looping,
                "playback source changed under same identity"
            );
            let samples = assets.get(hash).context("missing playback asset")?;
            ensure!(!samples.is_empty(), "empty playback asset");
            for (offset, target) in mixed[start..end].iter_mut().enumerate() {
                let position = start + offset - *origin;
                let sample = if *looping {
                    samples[position % samples.len()]
                } else {
                    samples.get(position).copied().unwrap_or(0.0)
                };
                *target += sample * source.gain;
            }
        }
        ended.extend(previous.difference(&active).copied());
        previous = active;
    }
    Ok(mixed)
}

fn write_pcm(path: &Path, samples: &[f32]) -> Result<()> {
    let mut file = std::io::BufWriter::new(fs::File::create(path)?);
    let length = u32::try_from(samples.len() * 2)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + length).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&(RATE as u32).to_le_bytes())?;
    file.write_all(&(RATE as u32 * 2).to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&length.to_le_bytes())?;
    for sample in samples {
        file.write_all(&((sample * 32767.0).round() as i16).to_le_bytes())?;
    }
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(id: u64, gain: f32) -> Source {
        Source {
            playback_id: id,
            asset_sha256: "test".into(),
            gain,
            looping: true,
        }
    }
    fn frame(ms: u64, playback: Vec<Source>) -> Frame {
        Frame {
            playback_ms: ms,
            playback_schema: 1,
            playback,
            output_available: true,
            decisions: Vec::new(),
        }
    }
    #[test]
    fn cancellation_mute_duck_overlap_and_missing_output_reconstruct_exactly() {
        let frames = vec![
            frame(0, vec![source(1, 0.4), source(2, 0.2)]),
            frame(10, vec![source(1, 0.1)]),
            frame(20, vec![source(1, 0.0)]),
            Frame {
                output_available: false,
                ..frame(30, vec![])
            },
        ];
        let assets = BTreeMap::from([("test".into(), vec![1.0; 10])]);
        let mix = render(&frames, &assets, sample_at(40)).expect("render");
        assert!(
            mix[..sample_at(10)]
                .iter()
                .all(|v| (*v - 0.6).abs() < 0.00001)
        );
        assert!(mix[sample_at(10)..sample_at(20)].iter().all(|v| *v == 0.1));
        assert!(mix[sample_at(20)..].iter().all(|v| *v == 0.0));
    }
    #[test]
    fn source_offset_survives_gain_changes_and_stops_at_asset_end() {
        let mut first = source(1, 1.0);
        first.looping = false;
        let frames = vec![
            frame(0, vec![first.clone()]),
            frame(1, vec![Source { gain: 0.5, ..first }]),
        ];
        let assets = BTreeMap::from([("test".into(), (0..88).map(|v| v as f32 / 100.0).collect())]);
        let mix = render(&frames, &assets, 100).expect("render");
        assert_eq!(mix[43], 0.43);
        assert_eq!(mix[44], 0.22);
        assert_eq!(mix[88], 0.0);
    }
    #[test]
    fn missing_assets_and_reused_playback_ids_are_rejected() {
        assert!(render(&[frame(0, vec![source(1, 1.0)])], &BTreeMap::new(), 100).is_err());
        let assets = BTreeMap::from([("test".into(), vec![1.0])]);
        assert!(
            render(
                &[
                    frame(0, vec![source(1, 1.0)]),
                    frame(1, vec![]),
                    frame(2, vec![source(1, 1.0)])
                ],
                &assets,
                100
            )
            .is_err()
        );
    }
    #[test]
    fn between_snapshot_playback_is_rejected_instead_of_silently_omitted() {
        let mut frames = vec![frame(0, vec![])];
        frames[0].decisions.push(Decision {
            outcome: "started".into(),
            playback_id: Some(7),
            started: Some(source(7, 1.0)),
        });
        assert!(validate_start_coverage(&frames).is_err());
        frames[0].playback.push(source(7, 1.0));
        validate_start_coverage(&frames).expect("represented start");
    }
    #[test]
    fn retained_source_verification_rejects_tampering_and_unsafe_hashes() {
        let directory = std::env::temp_dir().join(format!(
            "beastie-audio-hash-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&directory).expect("scratch");
        let hash = format!("{:x}", Sha256::digest(b"original"));
        let path = directory.join(format!("audio-{hash}.wav"));
        assert!(verified_asset_path(&directory, &hash).is_err());
        assert!(verified_asset_path(&directory, "../../escape").is_err());
        fs::write(&path, b"original").expect("source");
        assert_eq!(
            verified_asset_path(&directory, &hash).expect("verified"),
            path
        );
        fs::write(&path, b"changed").expect("tamper");
        assert!(verified_asset_path(&directory, &hash).is_err());
        fs::remove_dir_all(directory).expect("remove scratch");
    }
}
