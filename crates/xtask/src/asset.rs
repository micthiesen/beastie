use std::{collections::HashSet, fs, path::Path};

use anyhow::{Context, Result, bail};
use image::{GenericImageView, ImageReader};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    #[serde(default)]
    asset: Vec<Asset>,
    #[serde(default)]
    audio: Vec<Audio>,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    id: String,
    kind: String,
    width: u32,
    height: u32,
    #[serde(default = "one")]
    frames: u32,
    transparent: bool,
    palette: String,
    status: Status,
    path: Option<String>,
    provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Status {
    Planned,
    Generated,
    Runtime,
    Reference,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    generator: String,
    #[serde(default)]
    job_id: String,
    generated_at: String,
    prompt: String,
    seed: String,
    terms: String,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    human_modifications: String,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum Source {
    Final,
    Generated,
}

impl Source {
    fn directory(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::Generated => "generated",
        }
    }
}

pub(crate) fn check(manifest_path: &Path, require_runtime: bool) -> Result<()> {
    let source = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read asset manifest {}", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&source)
        .with_context(|| format!("failed to parse asset manifest {}", manifest_path.display()))?;
    let asset_root = manifest_path
        .parent()
        .context("asset manifest must have a parent directory")?;
    validate_manifest(&manifest, asset_root, require_runtime)?;
    validate_runtime_sprite_catalog(&manifest, asset_root)?;
    println!(
        "asset check passed: {} declared asset(s), runtime gate {}",
        manifest.asset.len() + manifest.audio.len(),
        if require_runtime {
            "enabled"
        } else {
            "disabled"
        }
    );
    Ok(())
}

fn validate_manifest(manifest: &Manifest, asset_root: &Path, require_runtime: bool) -> Result<()> {
    if manifest.version != 1 {
        bail!("asset manifest must declare version = 1");
    }
    if manifest.asset.is_empty() && manifest.audio.is_empty() {
        bail!("asset manifest must declare at least one asset or audio entry");
    }

    let mut ids = HashSet::new();
    for asset in &manifest.asset {
        validate_asset(asset, asset_root, require_runtime, &mut ids)?;
    }
    for audio in &manifest.audio {
        validate_audio(audio, asset_root, require_runtime, &mut ids)?;
    }
    Ok(())
}

fn validate_runtime_sprite_catalog(manifest: &Manifest, asset_root: &Path) -> Result<()> {
    let catalog_path = asset_root.join("runtime-sprites.txt");
    let source = fs::read_to_string(&catalog_path).with_context(|| {
        format!(
            "failed to read runtime sprite catalog {}",
            catalog_path.display()
        )
    })?;
    let catalog = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<HashSet<_>>();
    let declared = manifest
        .asset
        .iter()
        .filter(|asset| asset.status == Status::Runtime)
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    if catalog != declared {
        bail!(
            "runtime sprite catalog and manifest disagree: catalog={catalog:?}, manifest={declared:?}"
        );
    }
    Ok(())
}

fn validate_audio(
    audio: &Audio,
    asset_root: &Path,
    require_runtime: bool,
    ids: &mut HashSet<String>,
) -> Result<()> {
    let label = format!("audio asset {}", audio.id);
    validate_id(&audio.id, &label)?;
    if !ids.insert(audio.id.clone()) {
        bail!("asset manifest contains duplicate id {}", audio.id);
    }
    if audio.status == Status::Reference {
        bail!("{label} cannot use status reference");
    }
    let repository_root = asset_root
        .parent()
        .context("assets directory must have a repository parent")?;
    checked_relative_path(repository_root, &audio.provenance, "provenance", &audio.id)?;

    let generated = audio_candidate_path(asset_root, Source::Generated, &audio.id);
    let final_path = audio_candidate_path(asset_root, Source::Final, &audio.id);
    let generated_exists = generated.is_file();
    let final_exists = final_path.is_file();
    if generated_exists {
        validate_wav(audio, &generated)?;
    }
    if final_exists {
        validate_wav(audio, &final_path)?;
    }
    let resolved = resolve_source(final_exists, generated_exists);
    match audio.status {
        Status::Planned => {}
        Status::Generated if !generated_exists => {
            bail!("{label} has status generated but its generated WAV is missing");
        }
        Status::Generated => {}
        Status::Runtime if resolved.is_none() => {
            bail!("{label} has status runtime but has no generated or final WAV");
        }
        Status::Runtime => {}
        Status::Reference => unreachable!(),
    }
    if require_runtime && audio.status != Status::Runtime {
        bail!("{label} must have status runtime for the final gate");
    }
    if let Some(source) = resolved {
        println!("  {} -> {}/{}.wav", audio.id, source.directory(), audio.id);
    }
    Ok(())
}

fn validate_asset(
    asset: &Asset,
    asset_root: &Path,
    require_runtime: bool,
    ids: &mut HashSet<String>,
) -> Result<()> {
    let label = format!("asset {}", asset.id);
    validate_id(&asset.id, &label)?;
    if !ids.insert(asset.id.clone()) {
        bail!("asset manifest contains duplicate id {}", asset.id);
    }
    if asset.kind.trim().is_empty() {
        bail!("{label} must declare a non-empty kind");
    }
    if asset.width == 0 || asset.height == 0 || asset.frames == 0 {
        bail!("{label} dimensions and frame count must be greater than zero");
    }

    validate_palette(asset, asset_root)?;
    validate_provenance(asset, asset_root)?;

    if asset.status == Status::Reference {
        let relative = asset
            .path
            .as_deref()
            .context(format!("{label} with status reference must declare path"))?;
        let path = checked_relative_path(asset_root, relative, "reference asset", &asset.id)?;
        validate_png(asset, &path, "reference")?;
        return Ok(());
    }
    if asset.path.is_some() {
        bail!("{label} may only declare path when status is reference");
    }

    let mut generated_complete = true;
    let mut resolved_complete = true;
    let mut resolved_sources = HashSet::new();
    for frame in 0..asset.frames {
        let generated = candidate_path(
            asset_root,
            Source::Generated,
            &asset.id,
            asset.frames,
            frame,
        );
        let final_path = candidate_path(asset_root, Source::Final, &asset.id, asset.frames, frame);
        let generated_exists = generated.is_file();
        let final_exists = final_path.is_file();
        generated_complete &= generated_exists;

        // Validate every candidate, even though final wins at runtime. A stale malformed
        // generated source should not silently remain in the asset history.
        if generated_exists {
            validate_png(asset, &generated, Source::Generated.directory())?;
        }
        if final_exists {
            validate_png(asset, &final_path, Source::Final.directory())?;
        }

        match resolve_source(final_exists, generated_exists) {
            Some(source) => {
                resolved_sources.insert(source);
            }
            None => resolved_complete = false,
        }
    }

    match asset.status {
        Status::Planned => {}
        Status::Generated if !generated_complete => {
            bail!("{label} has status generated but one or more generated PNGs are missing");
        }
        Status::Generated => {}
        Status::Runtime if !resolved_complete => {
            bail!(
                "{label} has status runtime but one or more frames have no generated or final PNG"
            );
        }
        Status::Runtime => {}
        Status::Reference => unreachable!("references return before runtime resolution"),
    }
    if require_runtime && asset.status != Status::Runtime {
        bail!(
            "{label} has status {:?}; the final gate requires status runtime",
            asset.status
        );
    }

    if resolved_complete {
        let resolution = if resolved_sources.len() == 1 {
            resolved_sources
                .iter()
                .next()
                .expect("one resolved source")
                .directory()
        } else {
            "mixed final/generated"
        };
        println!("  {} -> {resolution} ({} frame(s))", asset.id, asset.frames);
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

fn validate_palette(asset: &Asset, asset_root: &Path) -> Result<()> {
    let path = checked_relative_path(asset_root, &asset.palette, "palette", &asset.id)?;
    let contents = fs::read_to_string(&path).with_context(|| {
        format!(
            "asset {} failed to read palette {}",
            asset.id,
            path.display()
        )
    })?;
    let colors = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(';'))
        .map(|line| line.strip_prefix('#').unwrap_or(line))
        .collect::<Vec<_>>();
    if colors.is_empty()
        || colors
            .iter()
            .any(|color| color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        bail!(
            "asset {} palette {} must contain one RGB hex color per line",
            asset.id,
            path.display()
        );
    }
    Ok(())
}

fn validate_provenance(asset: &Asset, asset_root: &Path) -> Result<()> {
    if asset.status == Status::Planned && asset.provenance.is_none() {
        return Ok(());
    }
    let provenance = asset.provenance.as_ref().with_context(|| {
        format!(
            "asset {} with status {:?} must declare provenance",
            asset.id, asset.status
        )
    })?;
    for (name, value) in [
        ("generator", provenance.generator.as_str()),
        ("generated_at", provenance.generated_at.as_str()),
        ("prompt", provenance.prompt.as_str()),
        ("seed", provenance.seed.as_str()),
        ("terms", provenance.terms.as_str()),
    ] {
        if value.trim().is_empty() {
            bail!("asset {} provenance.{name} must not be empty", asset.id);
        }
    }
    if !is_iso_date(&provenance.generated_at) {
        bail!(
            "asset {} provenance.generated_at must use YYYY-MM-DD",
            asset.id
        );
    }
    for reference in &provenance.references {
        checked_relative_path(asset_root, reference, "reference", &asset.id)?;
    }
    let _ = &provenance.human_modifications;
    let _ = &provenance.job_id;
    Ok(())
}

fn checked_relative_path(
    asset_root: &Path,
    relative: &str,
    kind: &str,
    id: &str,
) -> Result<std::path::PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("asset {id} {kind} path must stay inside assets: {relative}");
    }
    let path = asset_root.join(relative_path);
    if !path.is_file() {
        bail!("asset {id} {kind} is missing: {}", path.display());
    }
    Ok(path)
}

fn candidate_path(
    asset_root: &Path,
    source: Source,
    id: &str,
    frames: u32,
    frame: u32,
) -> std::path::PathBuf {
    let name = if frames == 1 {
        format!("{id}.png")
    } else {
        format!("{id}-{frame}.png")
    };
    asset_root.join(source.directory()).join(name)
}

fn resolve_source(final_exists: bool, generated_exists: bool) -> Option<Source> {
    if final_exists {
        Some(Source::Final)
    } else if generated_exists {
        Some(Source::Generated)
    } else {
        None
    }
}

fn audio_candidate_path(asset_root: &Path, source: Source, id: &str) -> std::path::PathBuf {
    asset_root
        .join(source.directory())
        .join(format!("{id}.wav"))
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

fn validate_png(asset: &Asset, path: &Path, source: &str) -> Result<()> {
    let reader = ImageReader::open(path)
        .with_context(|| format!("failed to open {source} candidate {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("failed to detect image format for {}", path.display()))?;
    if reader.format() != Some(image::ImageFormat::Png) {
        bail!(
            "asset {} candidate {} is not a PNG",
            asset.id,
            path.display()
        );
    }
    let image = reader.decode().with_context(|| {
        format!(
            "failed to decode asset {} candidate {}",
            asset.id,
            path.display()
        )
    })?;
    if image.dimensions() != (asset.width, asset.height) {
        bail!(
            "asset {} candidate {} is {}x{}, expected {}x{}",
            asset.id,
            path.display(),
            image.width(),
            image.height(),
            asset.width,
            asset.height
        );
    }
    let has_transparency = image.to_rgba8().pixels().any(|pixel| pixel.0[3] < u8::MAX);
    if asset.transparent && !has_transparency {
        bail!(
            "asset {} candidate {} must contain transparent pixels",
            asset.id,
            path.display()
        );
    }
    if !asset.transparent && has_transparency {
        bail!(
            "asset {} candidate {} must be fully opaque",
            asset.id,
            path.display()
        );
    }
    Ok(())
}

const fn one() -> u32 {
    1
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use image::{ImageBuffer, ImageFormat, Rgba};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestAssets {
        root: std::path::PathBuf,
    }

    impl TestAssets {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "beastie-asset-check-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("generated")).expect("generated directory");
            fs::create_dir_all(root.join("final")).expect("final directory");
            fs::create_dir_all(root.join("style")).expect("style directory");
            fs::write(root.join("style/main.hex"), "#112233\n#abcdef\n").expect("palette");
            Self { root }
        }

        fn manifest(&self, status: &str) -> Manifest {
            toml::from_str(&format!(
                r#"
version = 1

[[asset]]
id = "creature.idle"
kind = "sprite"
width = 2
height = 2
transparent = true
palette = "style/main.hex"
status = "{status}"

[asset.provenance]
generator = "test"
generated_at = "2026-08-15"
prompt = "test sprite"
seed = "42"
terms = "test fixture"
"#
            ))
            .expect("manifest")
        }

        fn png(&self, source: Source, width: u32, height: u32, transparent: bool) {
            let alpha = if transparent { 0 } else { 255 };
            let image = ImageBuffer::from_pixel(width, height, Rgba([17_u8, 34, 51, alpha]));
            image
                .save_with_format(
                    candidate_path(&self.root, source, "creature.idle", 1, 0),
                    ImageFormat::Png,
                )
                .expect("png");
        }

        fn wav(&self, channels: u16) -> std::path::PathBuf {
            let path = self.root.join("generated/test.wav");
            let data_size = 2_u32 * u32::from(channels);
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"RIFF");
            bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
            bytes.extend_from_slice(b"WAVEfmt ");
            bytes.extend_from_slice(&16_u32.to_le_bytes());
            bytes.extend_from_slice(&1_u16.to_le_bytes());
            bytes.extend_from_slice(&channels.to_le_bytes());
            bytes.extend_from_slice(&44_100_u32.to_le_bytes());
            bytes.extend_from_slice(&(44_100_u32 * u32::from(channels) * 2).to_le_bytes());
            bytes.extend_from_slice(&(channels * 2).to_le_bytes());
            bytes.extend_from_slice(&16_u16.to_le_bytes());
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&data_size.to_le_bytes());
            bytes.resize(bytes.len() + data_size as usize, 0);
            fs::write(&path, bytes).expect("wav");
            path
        }
    }

    impl Drop for TestAssets {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove test assets");
        }
    }

    #[test]
    fn final_candidate_takes_precedence_over_generated() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 2, 2, true);
        assets.png(Source::Final, 2, 2, true);
        let manifest = assets.manifest("runtime");

        validate_manifest(&manifest, &assets.root, true).expect("final candidate should resolve");
        assert_eq!(
            resolve_source(
                candidate_path(&assets.root, Source::Final, "creature.idle", 1, 0).is_file(),
                candidate_path(&assets.root, Source::Generated, "creature.idle", 1, 0).is_file(),
            ),
            Some(Source::Final)
        );
    }

    #[test]
    fn final_candidate_does_not_hide_an_invalid_generated_candidate() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 3, 2, true);
        assets.png(Source::Final, 2, 2, true);
        let error = validate_manifest(&assets.manifest("runtime"), &assets.root, true)
            .expect_err("all existing candidates should be validated")
            .to_string();

        assert!(error.contains("3x2, expected 2x2"), "{error}");
    }

    #[test]
    fn rejects_wrong_dimensions_and_alpha() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 3, 2, false);
        let error = validate_manifest(&assets.manifest("generated"), &assets.root, false)
            .expect_err("invalid image should fail")
            .to_string();

        assert!(error.contains("3x2, expected 2x2"), "{error}");
    }

    #[test]
    fn rejects_missing_expected_transparency() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 2, 2, false);
        let error = validate_manifest(&assets.manifest("generated"), &assets.root, false)
            .expect_err("opaque sprite should fail")
            .to_string();

        assert!(error.contains("must contain transparent pixels"), "{error}");
    }

    #[test]
    fn runtime_gate_rejects_non_runtime_assets() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 2, 2, true);
        let error = validate_manifest(&assets.manifest("generated"), &assets.root, true)
            .expect_err("generated-only status should fail final gate")
            .to_string();

        assert!(
            error.contains("final gate requires status runtime"),
            "{error}"
        );
    }

    #[test]
    fn missing_palette_and_provenance_are_rejected() {
        let assets = TestAssets::new();
        fs::remove_file(assets.root.join("style/main.hex")).expect("remove palette");
        let error = validate_manifest(&assets.manifest("runtime"), &assets.root, true)
            .expect_err("missing palette should fail")
            .to_string();

        assert!(error.contains("palette is missing"), "{error}");
    }

    #[test]
    fn validates_pcm_wav_contract() {
        let assets = TestAssets::new();
        let path = assets.wav(1);
        let audio = Audio {
            id: "audio/test".to_owned(),
            sample_rate: 44_100,
            channels: 1,
            bits_per_sample: 16,
            status: Status::Runtime,
            provenance: "docs/audio-direction.md".to_owned(),
        };

        validate_wav(&audio, &path).expect("mono PCM16 44.1k WAV should pass");
    }

    #[test]
    fn rejects_wrong_wav_channels() {
        let assets = TestAssets::new();
        let path = assets.wav(2);
        let audio = Audio {
            id: "audio/test".to_owned(),
            sample_rate: 44_100,
            channels: 1,
            bits_per_sample: 16,
            status: Status::Runtime,
            provenance: "docs/audio-direction.md".to_owned(),
        };
        let error = validate_wav(&audio, &path)
            .expect_err("stereo WAV should fail mono contract")
            .to_string();

        assert!(error.contains("2ch"), "{error}");
    }
}
