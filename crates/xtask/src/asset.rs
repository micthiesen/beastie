use std::{
    collections::HashSet,
    env, fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, ImageReader, Rgba};
use serde::{Deserialize, Serialize};

const PIXELLAB_API_URL: &str = "https://api.pixellab.ai/v2/create-image-pixflux";
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

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
    /// Maximum number of distinct opaque RGB colors allowed in each candidate.
    #[serde(default = "default_max_colors")]
    max_colors: u32,
    /// Maximum per-channel distance from the nearest declared palette color.
    #[serde(default)]
    palette_tolerance: u8,
    /// Pixel art uses binary alpha. `any` is retained only for legacy references.
    #[serde(default, alias = "alpha")]
    alpha_policy: AlphaPolicy,
    /// Number of source pixels represented by one logical pixel, when known.
    #[serde(default, alias = "pixel_density")]
    native_pixel_density: Option<u32>,
    status: Status,
    path: Option<String>,
    provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum AlphaPolicy {
    #[default]
    Hard,
    Any,
}

const fn default_max_colors() -> u32 {
    32
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

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PixfluxRequest {
    description: String,
    image_size: ImageSize,
    no_background: bool,
    seed: i64,
    color_image: EncodedImage,
    #[serde(skip_serializing_if = "Option::is_none")]
    init_image: Option<EncodedImage>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ImageSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedImage {
    #[serde(rename = "type")]
    #[serde(default = "base64_kind")]
    kind: String,
    base64: String,
    #[serde(default = "png_format")]
    format: String,
}

fn base64_kind() -> String {
    "base64".to_owned()
}

fn png_format() -> String {
    "png".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PixfluxResponse {
    image: EncodedImage,
    #[serde(default)]
    usage: Option<serde_json::Value>,
}

trait PixelLabClient {
    fn create_image(&self, token: &str, request: &PixfluxRequest) -> Result<PixfluxResponse>;
}

struct HttpPixelLabClient;

impl PixelLabClient for HttpPixelLabClient {
    fn create_image(&self, token: &str, request: &PixfluxRequest) -> Result<PixfluxResponse> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(180))
            .timeout_write(Duration::from_secs(30))
            .build();
        let response = agent
            .post(PIXELLAB_API_URL)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Content-Type", "application/json")
            .send_json(request)
            .map_err(|error| anyhow::anyhow!("PixelLab request failed: {error}"))?;
        response
            .into_json::<PixfluxResponse>()
            .context("PixelLab returned malformed JSON")
    }
}

#[derive(Debug)]
struct GenerationReport {
    destination: PathBuf,
    seed: i64,
    prompt: String,
    references: Vec<String>,
    generated_at: String,
    usage: Option<serde_json::Value>,
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

pub(crate) fn check(manifest_path: &Path, require_runtime: bool, verbose: bool) -> Result<()> {
    let source = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read asset manifest {}", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&source)
        .with_context(|| format!("failed to parse asset manifest {}", manifest_path.display()))?;
    let asset_root = manifest_path
        .parent()
        .context("asset manifest must have a parent directory")?;
    validate_manifest(&manifest, asset_root, require_runtime, verbose)?;
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

pub(crate) fn generate(manifest_path: &Path, id: &str, force: bool) -> Result<()> {
    let token = env::var("PIXELLAB_API_TOKEN")
        .context("PIXELLAB_API_TOKEN is required for asset generation")?;
    if token.trim().is_empty() {
        bail!("PIXELLAB_API_TOKEN must not be empty");
    }
    let report = generate_with_client(manifest_path, id, force, &HttpPixelLabClient, &token)?;
    println!("generated {}", report.destination.display());
    println!("PixelLab endpoint: {PIXELLAB_API_URL}");
    println!("PixelLab job: synchronous endpoint (no background job id)");
    if let Some(usage) = &report.usage {
        println!("PixelLab usage: {usage}");
    }
    println!("manifest provenance:");
    println!("  generator = \"pixellab\"");
    println!("  job_id = \"synchronous:create-image-pixflux\"");
    println!("  generated_at = {:?}", report.generated_at);
    println!("  prompt = {:?}", report.prompt);
    println!("  seed = {:?}", report.seed.to_string());
    println!("  references = {:?}", report.references);
    println!(
        "  terms = {:?}",
        format!("PixelLab Terms of Use, accessed {}", report.generated_at)
    );
    Ok(())
}

pub(crate) fn normalize_alpha(manifest_path: &Path, id: &str) -> Result<()> {
    let source = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read asset manifest {}", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&source)
        .with_context(|| format!("failed to parse asset manifest {}", manifest_path.display()))?;
    let asset_root = manifest_path
        .parent()
        .context("asset manifest must have a parent directory")?;
    if id == "all" {
        for asset in manifest.asset.iter().filter(|asset| {
            asset.alpha_policy == AlphaPolicy::Hard
                && asset.transparent
                && asset.status != Status::Reference
        }) {
            normalize_asset_alpha(asset, asset_root)?;
        }
        return Ok(());
    }
    let asset = manifest
        .asset
        .iter()
        .find(|asset| asset.id == id)
        .with_context(|| format!("asset manifest has no asset id {id}"))?;
    normalize_asset_alpha(asset, asset_root)
}

fn normalize_asset_alpha(asset: &Asset, asset_root: &Path) -> Result<()> {
    if asset.status == Status::Reference {
        bail!("reference asset {} cannot be normalized in place", asset.id);
    }
    let palette = read_palette(asset, asset_root)?;
    for frame in 0..asset.frames {
        let path = candidate_path(
            asset_root,
            Source::Generated,
            &asset.id,
            asset.frames,
            frame,
        );
        if !path.is_file() {
            bail!("generated asset frame is missing: {}", path.display());
        }
        let mut rgba = image::open(&path)
            .with_context(|| format!("failed to decode {}", path.display()))?
            .to_rgba8();
        let mut changed = 0_u64;
        for pixel in rgba.pixels_mut() {
            if pixel.0[3] == 0 && pixel.0[..3] != [0, 0, 0] {
                pixel.0[..3].copy_from_slice(&[0, 0, 0]);
                changed += 1;
            }
        }
        if changed == 0 {
            println!("{} already has canonical transparent RGB", path.display());
            continue;
        }
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut bytes, ImageFormat::Png)
            .with_context(|| format!("failed to encode normalized {}", path.display()))?;
        publish_png_frame(asset, asset_root, frame, &bytes.into_inner(), &palette)?;
        println!(
            "normalized {changed} transparent RGB pixel(s) in {}",
            path.display()
        );
    }
    Ok(())
}

fn generate_with_client(
    manifest_path: &Path,
    id: &str,
    force: bool,
    client: &impl PixelLabClient,
    token: &str,
) -> Result<GenerationReport> {
    let source = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read asset manifest {}", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&source)
        .with_context(|| format!("failed to parse asset manifest {}", manifest_path.display()))?;
    let asset_root = manifest_path
        .parent()
        .context("asset manifest must have a parent directory")?;
    let asset = select_generation_asset(&manifest, id)?;
    let provenance = asset
        .provenance
        .as_ref()
        .context("asset generation requires manifest provenance with prompt and numeric seed")?;
    let seed = provenance.seed.parse::<i64>().with_context(|| {
        format!(
            "asset {} provenance seed {:?} is not a numeric PixelLab seed",
            asset.id, provenance.seed
        )
    })?;
    let destination = candidate_path(asset_root, Source::Generated, &asset.id, 1, 0);
    if destination.exists() && !force {
        bail!(
            "refusing to overwrite {}; pass --force to replace it",
            destination.display()
        );
    }

    let palette = read_palette(asset, asset_root)?;
    let color_image = encode_palette_image(&palette)?;
    let init_image = provenance
        .references
        .first()
        .map(|reference| {
            let path = checked_relative_path(asset_root, reference, "reference", &asset.id)?;
            encode_png_file(&path)
        })
        .transpose()?;
    let request = PixfluxRequest {
        description: provenance.prompt.clone(),
        image_size: ImageSize {
            width: asset.width,
            height: asset.height,
        },
        no_background: asset.transparent,
        seed,
        color_image,
        init_image,
    };
    let response = client.create_image(token, &request)?;
    let bytes = canonicalize_transparent_png(&decode_response_image(&response.image)?)?;
    validate_png_bytes(asset, &bytes, &palette)?;
    publish_png(asset, asset_root, &bytes, force, &palette)?;

    Ok(GenerationReport {
        destination,
        seed,
        prompt: provenance.prompt.clone(),
        references: provenance.references.clone(),
        generated_at: utc_date()?,
        usage: response.usage,
    })
}

fn utc_date() -> Result<String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    let days = i64::try_from(seconds / 86_400).context("system date is out of range")?;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn select_generation_asset<'a>(manifest: &'a Manifest, id: &str) -> Result<&'a Asset> {
    if manifest.audio.iter().any(|audio| audio.id == id) {
        bail!("asset {id} is audio; PixelLab image generation does not support audio entries");
    }
    let asset = manifest
        .asset
        .iter()
        .find(|asset| asset.id == id)
        .with_context(|| format!("asset manifest has no entry with id {id}"))?;
    validate_id(&asset.id, &format!("asset {}", asset.id))?;
    if asset.status == Status::Reference {
        bail!("asset {id} is a reference entry and cannot be generated into assets/generated");
    }
    if asset.frames != 1 {
        bail!(
            "asset {id} declares {} frames; this generator supports single-frame entries only",
            asset.frames
        );
    }
    if !(16..=400).contains(&asset.width) || !(16..=400).contains(&asset.height) {
        bail!(
            "asset {id} is {}x{}; PixelLab create-image-pixflux supports each side from 16 through 400",
            asset.width,
            asset.height
        );
    }
    Ok(asset)
}

fn read_palette(asset: &Asset, asset_root: &Path) -> Result<Vec<[u8; 3]>> {
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
        .map(|color| {
            if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                bail!("invalid RGB hex color {color:?}");
            }
            Ok([
                u8::from_str_radix(&color[0..2], 16)?,
                u8::from_str_radix(&color[2..4], 16)?,
                u8::from_str_radix(&color[4..6], 16)?,
            ])
        })
        .collect::<Result<Vec<_>>>()
        .with_context(|| {
            format!(
                "asset {} palette {} must contain one RGB hex color per line",
                asset.id,
                path.display()
            )
        })?;
    if colors.is_empty() {
        bail!(
            "asset {} palette {} must contain one RGB hex color per line",
            asset.id,
            path.display()
        );
    }
    Ok(colors)
}

fn encode_palette_image(colors: &[[u8; 3]]) -> Result<EncodedImage> {
    let width = u32::try_from(colors.len()).context("palette contains too many colors")?;
    let image = ImageBuffer::from_fn(width, 1, |x, _| {
        let [red, green, blue] = colors[x as usize];
        Rgba([red, green, blue, u8::MAX])
    });
    encode_dynamic_image(&DynamicImage::ImageRgba8(image))
}

fn encode_png_file(path: &Path) -> Result<EncodedImage> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read PixelLab reference {}", path.display()))?;
    image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .with_context(|| format!("PixelLab reference {} is not a valid PNG", path.display()))?;
    Ok(EncodedImage {
        kind: "base64".to_owned(),
        base64: BASE64.encode(bytes),
        format: "png".to_owned(),
    })
}

fn encode_dynamic_image(image: &DynamicImage) -> Result<EncodedImage> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .context("failed to encode PixelLab palette image")?;
    Ok(EncodedImage {
        kind: "base64".to_owned(),
        base64: BASE64.encode(bytes.into_inner()),
        format: "png".to_owned(),
    })
}

fn decode_response_image(image: &EncodedImage) -> Result<Vec<u8>> {
    if image.kind != "base64" || !image.format.eq_ignore_ascii_case("png") {
        bail!(
            "PixelLab returned unsupported image encoding type={:?} format={:?}",
            image.kind,
            image.format
        );
    }
    let encoded = image
        .base64
        .strip_prefix("data:image/png;base64,")
        .unwrap_or(&image.base64);
    BASE64
        .decode(encoded)
        .context("PixelLab returned invalid base64 image data")
}

fn validate_png_bytes(asset: &Asset, bytes: &[u8], palette: &[[u8; 3]]) -> Result<()> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .context("PixelLab response is not a valid PNG")?;
    validate_decoded_png(asset, &image, "PixelLab response", palette).map(|_| ())
}

fn canonicalize_transparent_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut rgba = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .context("PixelLab response is not a valid PNG")?
        .to_rgba8();
    let mut changed = false;
    for pixel in rgba.pixels_mut() {
        if pixel.0[3] == 0 && pixel.0[..3] != [0, 0, 0] {
            pixel.0[..3].copy_from_slice(&[0, 0, 0]);
            changed = true;
        }
    }
    if !changed {
        return Ok(bytes.to_vec());
    }
    let mut normalized = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(rgba)
        .write_to(&mut normalized, ImageFormat::Png)
        .context("failed to encode normalized PixelLab response")?;
    Ok(normalized.into_inner())
}

fn publish_png(
    asset: &Asset,
    asset_root: &Path,
    bytes: &[u8],
    force: bool,
    palette: &[[u8; 3]],
) -> Result<PathBuf> {
    let destination = candidate_path(asset_root, Source::Generated, &asset.id, 1, 0);
    if destination.exists() && !force {
        bail!(
            "refusing to overwrite {}; pass --force to replace it",
            destination.display()
        );
    }
    let parent = destination
        .parent()
        .context("generated asset path must have a parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create generated directory {}", parent.display()))?;
    let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .context("generated asset path must have a UTF-8 filename")?;
    let temporary = parent.join(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ));
    let publish_result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("failed to create temporary asset {}", temporary.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("failed to write temporary asset {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync temporary asset {}", temporary.display()))?;
        validate_png(asset, &temporary, "generated temporary", palette)?;
        if destination.exists() && !force {
            bail!(
                "destination appeared during generation: {}",
                destination.display()
            );
        }
        if force {
            atomic_replace(&temporary, &destination)?;
        } else {
            fs::hard_link(&temporary, &destination).with_context(|| {
                format!(
                    "failed to atomically publish {} without overwriting an existing file",
                    destination.display()
                )
            })?;
        }
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    publish_result?;
    Ok(destination)
}

fn publish_png_frame(
    asset: &Asset,
    asset_root: &Path,
    frame: u32,
    bytes: &[u8],
    palette: &[[u8; 3]],
) -> Result<()> {
    let destination = candidate_path(
        asset_root,
        Source::Generated,
        &asset.id,
        asset.frames,
        frame,
    );
    let parent = destination
        .parent()
        .context("generated asset path must have a parent")?;
    let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .context("generated asset path must have a UTF-8 filename")?;
    let temporary = parent.join(format!(
        ".{file_name}.normalize-{}-{sequence}",
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("failed to create {}", temporary.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temporary.display()))?;
        validate_png(asset, &temporary, "normalized temporary", palette)?;
        atomic_replace(&temporary, &destination)
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination).with_context(|| {
        format!(
            "failed to atomically replace {} with {}",
            destination.display(),
            source.display()
        )
    })
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: Both paths are live, NUL-terminated UTF-16 buffers for the duration of the call.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error()).context("failed to atomically replace asset");
    }
    Ok(())
}

fn validate_manifest(
    manifest: &Manifest,
    asset_root: &Path,
    require_runtime: bool,
    verbose: bool,
) -> Result<()> {
    if manifest.version != 1 {
        bail!("asset manifest must declare version = 1");
    }
    if manifest.asset.is_empty() && manifest.audio.is_empty() {
        bail!("asset manifest must declare at least one asset or audio entry");
    }

    let mut ids = HashSet::new();
    for asset in &manifest.asset {
        validate_asset(asset, asset_root, require_runtime, verbose, &mut ids)?;
    }
    for audio in &manifest.audio {
        validate_audio(audio, asset_root, require_runtime, verbose, &mut ids)?;
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
    verbose: bool,
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
    if verbose && let Some(source) = resolved {
        println!("  {} -> {}/{}.wav", audio.id, source.directory(), audio.id);
    }
    Ok(())
}

fn validate_asset(
    asset: &Asset,
    asset_root: &Path,
    require_runtime: bool,
    verbose: bool,
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

    let palette = read_palette(asset, asset_root)?;
    validate_asset_policy(asset)?;
    validate_provenance(asset, asset_root)?;

    if asset.status == Status::Reference {
        let relative = asset
            .path
            .as_deref()
            .context(format!("{label} with status reference must declare path"))?;
        let path = checked_relative_path(asset_root, relative, "reference asset", &asset.id)?;
        let diagnostics = validate_png(asset, &path, "reference", &palette)?;
        if verbose {
            print_asset_diagnostics(asset, "reference", &diagnostics);
        }
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
            let diagnostics =
                validate_png(asset, &generated, Source::Generated.directory(), &palette)?;
            if verbose {
                print_asset_diagnostics(asset, Source::Generated.directory(), &diagnostics);
            }
        }
        if final_exists {
            let diagnostics =
                validate_png(asset, &final_path, Source::Final.directory(), &palette)?;
            if verbose {
                print_asset_diagnostics(asset, Source::Final.directory(), &diagnostics);
            }
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

    if verbose && resolved_complete {
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

fn print_asset_diagnostics(asset: &Asset, source: &str, diagnostics: &AssetDiagnostics) {
    println!(
        "    {} [{}]: colors={}, palette_distance={}, alpha_partial={}, hidden_rgb={}, interpolation_pixels={}, density={}",
        asset.id,
        source,
        diagnostics.unique_opaque_colors,
        diagnostics.max_palette_distance,
        diagnostics.partial_alpha_pixels,
        diagnostics.noncanonical_transparent_rgb_pixels,
        diagnostics.isolated_interpolation_pixels,
        diagnostics
            .native_pixel_density
            .map_or_else(|| "unspecified".to_owned(), |density| density.to_string())
    );
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

fn validate_png(
    asset: &Asset,
    path: &Path,
    source: &str,
    palette: &[[u8; 3]],
) -> Result<AssetDiagnostics> {
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
    validate_decoded_png(asset, &image, &path.display().to_string(), palette)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AssetDiagnostics {
    pub width: u32,
    pub height: u32,
    pub opaque_pixels: u64,
    pub transparent_pixels: u64,
    pub partial_alpha_pixels: u64,
    pub noncanonical_transparent_rgb_pixels: u64,
    pub unique_opaque_colors: usize,
    pub max_palette_distance: u8,
    pub isolated_interpolation_pixels: u64,
    pub native_pixel_density: Option<u32>,
}

fn validate_asset_policy(asset: &Asset) -> Result<()> {
    if asset.max_colors == 0 {
        bail!("asset {} max_colors must be greater than zero", asset.id);
    }
    if asset.native_pixel_density == Some(0) {
        bail!(
            "asset {} native_pixel_density must be greater than zero",
            asset.id
        );
    }
    Ok(())
}

fn validate_decoded_png(
    asset: &Asset,
    image: &DynamicImage,
    candidate: &str,
    palette: &[[u8; 3]],
) -> Result<AssetDiagnostics> {
    if image.dimensions() != (asset.width, asset.height) {
        bail!(
            "asset {} candidate {} is {}x{}, expected {}x{}",
            asset.id,
            candidate,
            image.width(),
            image.height(),
            asset.width,
            asset.height
        );
    }
    let rgba = image.to_rgba8();
    let mut opaque_colors = HashSet::new();
    let mut transparent_pixels = 0_u64;
    let mut partial_alpha_pixels = 0_u64;
    let mut noncanonical_transparent_rgb_pixels = 0_u64;
    let mut max_palette_distance = 0_u8;
    for pixel in rgba.pixels() {
        let alpha = pixel.0[3];
        if alpha == u8::MAX {
            let rgb = [pixel.0[0], pixel.0[1], pixel.0[2]];
            opaque_colors.insert(rgb);
            max_palette_distance = max_palette_distance.max(palette_distance(rgb, palette));
        } else {
            transparent_pixels += 1;
            if alpha == 0 && pixel.0[..3] != [0, 0, 0] {
                noncanonical_transparent_rgb_pixels += 1;
            }
            if alpha != 0 {
                partial_alpha_pixels += 1;
            }
        }
    }
    let has_transparency = transparent_pixels > 0;
    if asset.transparent && !has_transparency {
        bail!(
            "asset {} candidate {} must contain transparent pixels",
            asset.id,
            candidate
        );
    }
    if !asset.transparent && has_transparency {
        bail!(
            "asset {} candidate {} must be fully opaque",
            asset.id,
            candidate
        );
    }
    if asset.alpha_policy == AlphaPolicy::Hard
        && partial_alpha_pixels > 0
        && asset.status != Status::Reference
    {
        bail!(
            "asset {} candidate {} violates hard alpha policy [0,255]: {} partial-alpha pixel(s)",
            asset.id,
            candidate,
            partial_alpha_pixels
        );
    }
    if asset.alpha_policy == AlphaPolicy::Hard
        && noncanonical_transparent_rgb_pixels > 0
        && asset.status != Status::Reference
    {
        bail!(
            "asset {} candidate {} has {} fully transparent pixel(s) with noncanonical RGB; run `cargo xtask asset normalize-alpha {}`",
            asset.id,
            candidate,
            noncanonical_transparent_rgb_pixels,
            asset.id
        );
    }
    if asset.status != Status::Reference && opaque_colors.len() > asset.max_colors as usize {
        bail!(
            "asset {} candidate {} has {} unique opaque colors, exceeds max_colors {}",
            asset.id,
            candidate,
            opaque_colors.len(),
            asset.max_colors
        );
    }
    let isolated = count_isolated_interpolation_pixels(&rgba, palette, asset.palette_tolerance);
    if asset.status != Status::Reference && isolated > 0 {
        bail!(
            "asset {} candidate {} contains {} isolated interpolation/resampling color pixel(s)",
            asset.id,
            candidate,
            isolated
        );
    }
    if asset.status != Status::Reference && max_palette_distance > asset.palette_tolerance {
        bail!(
            "asset {} candidate {} has opaque RGB pixels up to palette distance {}, exceeds palette_tolerance {}",
            asset.id,
            candidate,
            max_palette_distance,
            asset.palette_tolerance
        );
    }
    Ok(AssetDiagnostics {
        width: image.width(),
        height: image.height(),
        opaque_pixels: u64::from(image.width()) * u64::from(image.height()) - transparent_pixels,
        transparent_pixels,
        partial_alpha_pixels,
        noncanonical_transparent_rgb_pixels,
        unique_opaque_colors: opaque_colors.len(),
        max_palette_distance,
        isolated_interpolation_pixels: isolated,
        native_pixel_density: asset.native_pixel_density,
    })
}

fn palette_distance(rgb: [u8; 3], palette: &[[u8; 3]]) -> u8 {
    palette
        .iter()
        .map(|color| {
            rgb.into_iter()
                .zip(color)
                .map(|(left, right)| left.abs_diff(*right))
                .max()
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(u8::MAX)
}

fn count_isolated_interpolation_pixels(
    image: &image::RgbaImage,
    palette: &[[u8; 3]],
    tolerance: u8,
) -> u64 {
    let (width, height) = image.dimensions();
    let mut counts = std::collections::HashMap::<[u8; 3], u32>::new();
    for pixel in image.pixels().filter(|pixel| pixel.0[3] == u8::MAX) {
        let rgb = [pixel.0[0], pixel.0[1], pixel.0[2]];
        *counts.entry(rgb).or_default() += 1;
    }
    let mut isolated = 0_u64;
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            if pixel.0[3] != u8::MAX {
                continue;
            }
            let rgb = [pixel.0[0], pixel.0[1], pixel.0[2]];
            if palette_distance(rgb, palette) <= tolerance || counts[&rgb] > 2 {
                continue;
            }
            let mut neighbors = HashSet::new();
            for (nx, ny) in [
                (x.checked_sub(1), Some(y)),
                (x.checked_add(1), Some(y)),
                (Some(x), y.checked_sub(1)),
                (Some(x), y.checked_add(1)),
            ] {
                let (Some(nx), Some(ny)) = (nx, ny) else {
                    continue;
                };
                if nx >= width || ny >= height {
                    continue;
                }
                let neighbor = image.get_pixel(nx, ny);
                if neighbor.0[3] == u8::MAX {
                    neighbors.insert([neighbor.0[0], neighbor.0[1], neighbor.0[2]]);
                }
            }
            if neighbors.len() >= 2 {
                isolated += 1;
            }
        }
    }
    isolated
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
        cell::RefCell,
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
max_colors = 32
palette_tolerance = 0
alpha_policy = "hard"
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
            let rgb: [u8; 3] = if transparent { [0, 0, 0] } else { [17, 34, 51] };
            let image =
                ImageBuffer::from_pixel(width, height, Rgba([rgb[0], rgb[1], rgb[2], alpha]));
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

        fn generation_manifest(&self, frames: u32, status: &str) -> PathBuf {
            let reference = self.root.join("style/reference.png");
            let reference_image = ImageBuffer::from_pixel(16, 16, Rgba([1_u8, 2, 3, 0]));
            reference_image
                .save_with_format(reference, ImageFormat::Png)
                .expect("reference PNG");
            let path = self.root.join("manifest.toml");
            fs::write(
                &path,
                format!(
                    r#"
version = 1

[[asset]]
id = "creature/test"
kind = "sprite"
width = 16
height = 16
frames = {frames}
transparent = true
palette = "style/main.hex"
max_colors = 32
palette_tolerance = 0
alpha_policy = "hard"
status = "{status}"

[asset.provenance]
generator = "pixellab"
generated_at = "2026-08-15"
prompt = "awkward test creature"
seed = "4242"
terms = "test"
references = ["style/reference.png"]

[[audio]]
id = "audio/test"
sample_rate = 44100
channels = 1
bits_per_sample = 16
status = "planned"
provenance = "docs/audio.md"
"#
                ),
            )
            .expect("generation manifest");
            path
        }
    }

    impl Drop for TestAssets {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove test assets");
        }
    }

    struct MockPixelLab {
        response: RefCell<Option<PixfluxResponse>>,
        request: RefCell<Option<serde_json::Value>>,
        token: RefCell<Option<String>>,
    }

    impl MockPixelLab {
        fn returning(bytes: &[u8]) -> Self {
            Self {
                response: RefCell::new(Some(PixfluxResponse {
                    image: EncodedImage {
                        kind: "base64".to_owned(),
                        base64: format!("data:image/png;base64,{}", BASE64.encode(bytes)),
                        format: "png".to_owned(),
                    },
                    usage: Some(serde_json::json!({"type": "usd", "usd": 0.01})),
                })),
                request: RefCell::new(None),
                token: RefCell::new(None),
            }
        }
    }

    impl PixelLabClient for MockPixelLab {
        fn create_image(&self, token: &str, request: &PixfluxRequest) -> Result<PixfluxResponse> {
            self.request.replace(Some(
                serde_json::to_value(request).expect("serialize captured request"),
            ));
            self.token.replace(Some(token.to_owned()));
            self.response
                .borrow_mut()
                .take()
                .context("mock response already consumed")
        }
    }

    fn png_bytes(transparent: bool, red: u8) -> Vec<u8> {
        let alpha = if transparent { 0 } else { u8::MAX };
        let (red, green, blue) = if transparent {
            (0, 0, 0)
        } else {
            (red, 34, 51)
        };
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            16,
            16,
            Rgba([red, green, blue, alpha]),
        ));
        let mut bytes = Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("encode PNG");
        bytes.into_inner()
    }

    #[test]
    fn final_candidate_takes_precedence_over_generated() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 2, 2, true);
        assets.png(Source::Final, 2, 2, true);
        let manifest = assets.manifest("runtime");

        validate_manifest(&manifest, &assets.root, true, false)
            .expect("final candidate should resolve");
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
        let error = validate_manifest(&assets.manifest("runtime"), &assets.root, true, false)
            .expect_err("all existing candidates should be validated")
            .to_string();

        assert!(error.contains("3x2, expected 2x2"), "{error}");
    }

    #[test]
    fn rejects_wrong_dimensions_and_alpha() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 3, 2, false);
        let error = validate_manifest(&assets.manifest("generated"), &assets.root, false, false)
            .expect_err("invalid image should fail")
            .to_string();

        assert!(error.contains("3x2, expected 2x2"), "{error}");
    }

    #[test]
    fn rejects_missing_expected_transparency() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 2, 2, false);
        let error = validate_manifest(&assets.manifest("generated"), &assets.root, false, false)
            .expect_err("opaque sprite should fail")
            .to_string();

        assert!(error.contains("must contain transparent pixels"), "{error}");
    }

    fn strict_asset(width: u32, height: u32, max_colors: u32, tolerance: u8) -> Asset {
        toml::from_str(&format!(
            r#"
id = "fixture/pixel"
kind = "sprite"
width = {width}
height = {height}
transparent = true
palette = "style/main.hex"
max_colors = {max_colors}
palette_tolerance = {tolerance}
alpha_policy = "hard"
status = "planned"
"#
        ))
        .expect("strict asset")
    }

    fn fixture_image(width: u32, height: u32, pixels: Vec<Rgba<u8>>) -> DynamicImage {
        DynamicImage::ImageRgba8(
            ImageBuffer::from_raw(
                width,
                height,
                pixels.into_iter().flat_map(|pixel| pixel.0).collect(),
            )
            .expect("fixture pixels match image dimensions"),
        )
    }

    #[test]
    fn rejects_bilinear_resampling_color() {
        let assets = TestAssets::new();
        let asset = strict_asset(3, 3, 3, 0);
        let red = Rgba([17, 34, 51, 255]);
        let blue = Rgba([171, 205, 239, 255]);
        let blended = Rgba([94, 119, 145, 255]);
        let mut pixels = vec![red; 9];
        pixels[1] = blue;
        pixels[3] = blue;
        pixels[4] = blended;
        pixels[8] = Rgba([0, 0, 0, 0]);
        let error = validate_decoded_png(
            &asset,
            &fixture_image(3, 3, pixels),
            "bilinear fixture",
            &read_palette(&asset, &assets.root).expect("palette"),
        )
        .expect_err("interpolation color should fail")
        .to_string();
        assert!(
            error.contains("isolated interpolation/resampling"),
            "{error}"
        );
    }

    #[test]
    fn rejects_partial_alpha_fringe_under_hard_policy() {
        let assets = TestAssets::new();
        let asset = strict_asset(2, 2, 4, 0);
        let pixels = vec![
            Rgba([17, 34, 51, 255]),
            Rgba([17, 34, 51, 128]),
            Rgba([0, 0, 0, 0]),
            Rgba([171, 205, 239, 255]),
        ];
        let error = validate_decoded_png(
            &asset,
            &fixture_image(2, 2, pixels),
            "alpha fringe fixture",
            &read_palette(&asset, &assets.root).expect("palette"),
        )
        .expect_err("partial alpha should fail")
        .to_string();
        assert!(error.contains("hard alpha policy [0,255]"), "{error}");
    }

    #[test]
    fn rejects_palette_explosion() {
        let assets = TestAssets::new();
        let asset = strict_asset(2, 2, 2, 255);
        let pixels = vec![
            Rgba([17, 34, 51, 255]),
            Rgba([171, 205, 239, 255]),
            Rgba([18, 35, 52, 255]),
            Rgba([0, 0, 0, 0]),
        ];
        let error = validate_decoded_png(
            &asset,
            &fixture_image(2, 2, pixels),
            "palette explosion fixture",
            &read_palette(&asset, &assets.root).expect("palette"),
        )
        .expect_err("too many colors should fail")
        .to_string();
        assert!(
            error.contains("unique opaque colors") && error.contains("max_colors"),
            "{error}"
        );
    }

    #[test]
    fn rejects_off_palette_pixels_beyond_tolerance() {
        let assets = TestAssets::new();
        let asset = strict_asset(2, 2, 4, 2);
        let pixels = vec![
            Rgba([17, 34, 51, 255]),
            Rgba([90, 90, 90, 255]),
            Rgba([90, 90, 90, 255]),
            Rgba([0, 0, 0, 0]),
        ];
        let error = validate_decoded_png(
            &asset,
            &fixture_image(2, 2, pixels),
            "off-palette fixture",
            &read_palette(&asset, &assets.root).expect("palette"),
        )
        .expect_err("off-palette pixel should fail")
        .to_string();
        assert!(
            error.contains("palette distance") && error.contains("palette_tolerance"),
            "{error}"
        );
    }

    #[test]
    fn clean_hard_edge_fixture_reports_diagnostics() {
        let assets = TestAssets::new();
        let mut asset = strict_asset(2, 2, 2, 0);
        asset.native_pixel_density = Some(1);
        let pixels = vec![
            Rgba([17, 34, 51, 255]),
            Rgba([171, 205, 239, 255]),
            Rgba([0, 0, 0, 0]),
            Rgba([17, 34, 51, 255]),
        ];
        let diagnostics = validate_decoded_png(
            &asset,
            &fixture_image(2, 2, pixels),
            "clean hard-edge fixture",
            &read_palette(&asset, &assets.root).expect("palette"),
        )
        .expect("clean hard-edge image should pass");
        assert_eq!(diagnostics.unique_opaque_colors, 2);
        assert_eq!(diagnostics.partial_alpha_pixels, 0);
        assert_eq!(diagnostics.noncanonical_transparent_rgb_pixels, 0);
        assert_eq!(diagnostics.native_pixel_density, Some(1));
    }

    #[test]
    fn rejects_hidden_rgb_behind_hard_transparency() {
        let assets = TestAssets::new();
        let asset = strict_asset(2, 2, 1, 0);
        let pixels = vec![
            Rgba([17, 34, 51, 255]),
            Rgba([255, 255, 255, 0]),
            Rgba([0, 0, 0, 0]),
            Rgba([17, 34, 51, 255]),
        ];
        let error = validate_decoded_png(
            &asset,
            &fixture_image(2, 2, pixels),
            "hidden RGB fixture",
            &read_palette(&asset, &assets.root).expect("palette"),
        )
        .expect_err("hidden RGB should fail hard-alpha policy")
        .to_string();
        assert!(error.contains("noncanonical RGB"), "{error}");
    }

    #[test]
    fn rejects_inconsistent_animation_frame_dimensions() {
        let assets = TestAssets::new();
        let manifest_path = assets.generation_manifest(2, "planned");
        let manifest: Manifest =
            toml::from_str(&fs::read_to_string(&manifest_path).expect("manifest source"))
                .expect("manifest");
        let asset = &manifest.asset[0];
        let palette = read_palette(asset, &assets.root).expect("palette");
        let frame0 = candidate_path(&assets.root, Source::Generated, &asset.id, 2, 0);
        let frame1 = candidate_path(&assets.root, Source::Generated, &asset.id, 2, 1);
        fs::create_dir_all(frame0.parent().expect("frame parent")).expect("frame directory");
        ImageBuffer::from_pixel(16, 16, Rgba([0_u8, 0, 0, 0]))
            .save_with_format(frame0, ImageFormat::Png)
            .expect("frame 0");
        ImageBuffer::from_pixel(17, 16, Rgba([0_u8, 0, 0, 0]))
            .save_with_format(frame1, ImageFormat::Png)
            .expect("frame 1");
        let error = validate_asset(
            &manifest.asset[0],
            &assets.root,
            false,
            false,
            &mut HashSet::new(),
        )
        .expect_err("mixed animation frame dimensions should fail")
        .to_string();
        assert!(
            error.contains("17x16, expected 16x16"),
            "{error}; palette={palette:?}"
        );
    }

    #[test]
    fn runtime_gate_rejects_non_runtime_assets() {
        let assets = TestAssets::new();
        assets.png(Source::Generated, 2, 2, true);
        let error = validate_manifest(&assets.manifest("generated"), &assets.root, true, false)
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
        let error = validate_manifest(&assets.manifest("runtime"), &assets.root, true, false)
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

    #[test]
    fn generation_selection_rejects_audio_references_and_multiple_frames() {
        let assets = TestAssets::new();
        let multi_path = assets.generation_manifest(4, "planned");
        let source = fs::read_to_string(&multi_path).expect("manifest source");
        let multi: Manifest = toml::from_str(&source).expect("multi-frame manifest");
        let error = select_generation_asset(&multi, "creature/test")
            .expect_err("multi-frame generation should fail")
            .to_string();
        assert!(error.contains("single-frame"), "{error}");
        let error = select_generation_asset(&multi, "audio/test")
            .expect_err("audio generation should fail")
            .to_string();
        assert!(error.contains("does not support audio"), "{error}");

        let reference_path = assets.generation_manifest(1, "reference");
        let source = fs::read_to_string(reference_path).expect("reference manifest source");
        let reference: Manifest = toml::from_str(&source).expect("reference manifest");
        let error = select_generation_asset(&reference, "creature/test")
            .expect_err("reference generation should fail")
            .to_string();
        assert!(error.contains("reference entry"), "{error}");
    }

    #[test]
    fn generation_builds_documented_payload_decodes_response_and_publishes_expected_path() {
        let assets = TestAssets::new();
        let manifest_path = assets.generation_manifest(1, "planned");
        let bytes = png_bytes(true, 17);
        let client = MockPixelLab::returning(&bytes);

        let report = generate_with_client(&manifest_path, "creature/test", false, &client, "token")
            .expect("mock generation");
        assert_eq!(
            report.destination,
            assets.root.join("generated/creature/test.png")
        );
        validate_png(
            &toml::from_str::<Manifest>(&fs::read_to_string(&manifest_path).expect("manifest"))
                .expect("parse manifest")
                .asset[0],
            &report.destination,
            "test generated",
            &read_palette(
                &toml::from_str::<Manifest>(&fs::read_to_string(&manifest_path).expect("manifest"))
                    .expect("parse manifest")
                    .asset[0],
                &assets.root,
            )
            .expect("palette"),
        )
        .expect("published PNG contract");
        assert_eq!(client.token.borrow().as_deref(), Some("token"));
        let request = client.request.borrow();
        let request = request.as_ref().expect("captured request");
        assert_eq!(request["description"], "awkward test creature");
        assert_eq!(
            request["image_size"],
            serde_json::json!({"width": 16, "height": 16})
        );
        assert_eq!(request["no_background"], true);
        assert_eq!(request["seed"], 4242);
        assert_eq!(request["color_image"]["type"], "base64");
        assert_eq!(request["init_image"]["format"], "png");
    }

    #[test]
    fn generation_never_calls_service_or_overwrites_without_force() {
        let assets = TestAssets::new();
        let manifest_path = assets.generation_manifest(1, "planned");
        let original = png_bytes(true, 17);
        let destination = assets.root.join("generated/creature/test.png");
        fs::create_dir_all(destination.parent().expect("destination parent"))
            .expect("generated parent");
        fs::write(&destination, &original).expect("existing candidate");
        let unused_client = MockPixelLab::returning(&png_bytes(true, 99));

        let error = generate_with_client(
            &manifest_path,
            "creature/test",
            false,
            &unused_client,
            "token",
        )
        .expect_err("overwrite should require force")
        .to_string();
        assert!(error.contains("pass --force"), "{error}");
        assert!(unused_client.request.borrow().is_none());
        assert_eq!(fs::read(&destination).expect("original remains"), original);

        let replacement = png_bytes(true, 99);
        let force_client = MockPixelLab::returning(&replacement);
        generate_with_client(
            &manifest_path,
            "creature/test",
            true,
            &force_client,
            "token",
        )
        .expect("forced replacement");
        assert_eq!(
            fs::read(&destination).expect("replacement published"),
            replacement
        );
        let temporary_files = fs::read_dir(destination.parent().expect("destination parent"))
            .expect("generated directory")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(temporary_files, 0);
    }

    #[test]
    fn response_decoder_rejects_non_png_and_invalid_base64() {
        let non_png = EncodedImage {
            kind: "base64".to_owned(),
            base64: BASE64.encode(b"not png"),
            format: "jpeg".to_owned(),
        };
        assert!(decode_response_image(&non_png).is_err());
        let invalid = EncodedImage {
            kind: "base64".to_owned(),
            base64: "%%%".to_owned(),
            format: "png".to_owned(),
        };
        assert!(decode_response_image(&invalid).is_err());
    }
}
