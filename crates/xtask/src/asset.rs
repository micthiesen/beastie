//! Offline contract for the only file-backed runtime assets: sound and text fonts.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    audio: Vec<Audio>,
    font: Vec<Font>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Audio {
    id: String,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    status: Status,
    provenance: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Status {
    Planned,
    Generated,
    Runtime,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Font {
    path: String,
    license: String,
}

pub(crate) fn check(manifest_path: &Path, require_runtime: bool, verbose: bool) -> Result<()> {
    let manifest: Manifest = toml::from_str(&fs::read_to_string(manifest_path)?)
        .context("invalid sound/font asset manifest")?;
    ensure!(
        manifest.version == 2,
        "unsupported sound/font manifest version"
    );
    let root = manifest_path
        .parent()
        .context("manifest has no parent")?
        .canonicalize()?;
    let repository = root.parent().context("asset directory has no parent")?;
    reject_images(&root)?;
    ensure!(
        !manifest.audio.is_empty() && !manifest.font.is_empty(),
        "sound/font manifest is empty"
    );
    let mut ids = HashSet::new();
    for audio in &manifest.audio {
        validate_id(&audio.id, "audio")?;
        ensure!(
            audio.id.starts_with("audio/"),
            "sound id must start with audio/"
        );
        ensure!(ids.insert(&audio.id), "duplicate sound id {}", audio.id);
        ensure!(
            !require_runtime || audio.status == Status::Runtime,
            "sound {} must be runtime",
            audio.id
        );
        checked_file(repository, &audio.provenance)?;
        if audio.status == Status::Generated {
            ensure!(
                root.join("generated")
                    .join(format!("{}.wav", audio.id))
                    .is_file(),
                "missing generated sound {}",
                audio.id
            );
        }
        let mut found = false;
        for source in ["final", "generated"] {
            let path = root.join(source).join(format!("{}.wav", audio.id));
            if path.is_file() {
                validate_wav(audio, &path)?;
                found = true;
                if verbose {
                    println!("  {}", path.display());
                }
            }
        }
        ensure!(
            found || audio.status == Status::Planned,
            "missing sound {}",
            audio.id
        );
    }
    for font in &manifest.font {
        let path = checked_file(&root, &font.path)?;
        let bytes = fs::read(&path)?;
        ensure!(
            bytes.len() >= 12 && (&bytes[..4] == b"\0\x01\0\0" || &bytes[..4] == b"OTTO"),
            "invalid OpenType font {}",
            path.display()
        );
        let tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        ensure!(
            tables > 0 && bytes.len() >= 12 + tables * 16,
            "truncated font table directory"
        );
        for table in bytes[12..12 + tables * 16].as_chunks::<16>().0.iter() {
            let offset = u32::from_be_bytes(table[8..12].try_into()?) as usize;
            let length = u32::from_be_bytes(table[12..16].try_into()?) as usize;
            ensure!(
                offset
                    .checked_add(length)
                    .is_some_and(|end| end <= bytes.len()),
                "truncated font table"
            );
        }
        let license = checked_file(&root, &font.license)?;
        ensure!(
            !fs::read_to_string(license)?.trim().is_empty(),
            "font license is empty"
        );
    }
    Ok(())
}

fn checked_file(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_id(relative, "asset path")?;
    let path = root.join(relative);
    ensure!(path.is_file(), "missing asset {}", path.display());
    ensure!(
        path.canonicalize()?.starts_with(root.canonicalize()?),
        "asset escapes root"
    );
    Ok(path)
}

fn reject_images(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(
            !kind.is_symlink(),
            "runtime asset symlinks are not supported"
        );
        if kind.is_dir() {
            reject_images(&entry.path())?;
        } else if let Some(extension) = entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
        {
            ensure!(
                !matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ktx2" | "dds"
                ),
                "runtime image assets are forbidden: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn validate_id(id: &str, label: &str) -> Result<()> {
    if id.is_empty()
        || id.split('/').any(|segment| {
            segment.is_empty()
                || segment.starts_with('.')
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        })
    {
        bail!("{label} has an invalid id; use safe slash-separated path segments");
    }
    Ok(())
}

fn validate_wav(audio: &Audio, path: &Path) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("failed to read WAV {}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!(
            "audio asset {} candidate {} is not RIFF/WAVE",
            audio.id,
            path.display()
        );
    }
    let mut offset = 12_usize;
    let mut format = None;
    let mut has_data = false;
    while offset + 8 <= bytes.len() {
        let chunk = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("four bytes"),
        ) as usize;
        let data_start = offset + 8;
        let data_end = data_start
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .with_context(|| format!("audio asset {} has a truncated WAV chunk", audio.id))?;
        if chunk == b"fmt " {
            if size < 16 {
                bail!("audio asset {} has an invalid WAV fmt chunk", audio.id);
            }
            format = Some((
                u16::from_le_bytes(
                    bytes[data_start..data_start + 2]
                        .try_into()
                        .expect("two bytes"),
                ),
                u16::from_le_bytes(
                    bytes[data_start + 2..data_start + 4]
                        .try_into()
                        .expect("two bytes"),
                ),
                u32::from_le_bytes(
                    bytes[data_start + 4..data_start + 8]
                        .try_into()
                        .expect("four bytes"),
                ),
                u16::from_le_bytes(
                    bytes[data_start + 14..data_start + 16]
                        .try_into()
                        .expect("two bytes"),
                ),
            ));
        } else if chunk == b"data" {
            has_data = size > 0;
        }
        offset = data_end + (size & 1);
    }
    let (encoding, channels, sample_rate, bits_per_sample) =
        format.context(format!("audio asset {} has no WAV fmt chunk", audio.id))?;
    if !has_data {
        bail!("audio asset {} has no non-empty WAV data chunk", audio.id);
    }
    if encoding != 1
        || channels != audio.channels
        || sample_rate != audio.sample_rate
        || bits_per_sample != audio.bits_per_sample
    {
        bail!(
            "audio asset {} candidate {} is encoding {encoding}, {channels}ch, {sample_rate}Hz, {bits_per_sample}-bit; expected PCM, {}ch, {}Hz, {}-bit",
            audio.id,
            path.display(),
            audio.channels,
            audio.sample_rate,
            audio.bits_per_sample
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_sound_and_fonts_satisfy_offline_contract() {
        check(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/manifest.toml"),
            true,
            false,
        )
        .unwrap();
    }

    #[test]
    fn legacy_sprite_manifest_is_rejected() {
        assert!(
            toml::from_str::<Manifest>(
                "version = 2\naudio = []\nfont = []\n[[asset]]\nid = 'creature'\n"
            )
            .is_err()
        );
    }

    #[test]
    fn runtime_image_is_rejected_even_when_unlisted() {
        let root =
            std::env::temp_dir().join(format!("beastie-forbidden-image-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("unused.PNG"), b"not an image").unwrap();
        let result = reject_images(&root);
        fs::remove_dir_all(&root).unwrap();
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("runtime image assets are forbidden")
        );
    }

    #[test]
    fn asset_paths_cannot_escape_root() {
        for path in ["../secret", "/secret", "audio//file", "audio/../../secret"] {
            assert!(validate_id(path, "asset path").is_err());
        }
    }
}
