//! Offline release staging checks.
//!
//! This module deliberately does not know how to build a Rust binary. The caller supplies the
//! already-built game and worker paths, and this code stages only files whose provenance is in
//! the repository (or the explicitly supplied model). No network client, shell, or deletion is
//! involved, so a package build cannot silently fetch weights or leave a stale TTS binary behind.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const INSTALL_SIZE_BUDGET: u64 = 1_500_000_000;
const SHA256_HEX_LEN: usize = 64;
const ESPEAK_SOURCE_NAME: &str = "espeak-ng-1.52.0.tar.gz";
#[cfg(not(test))]
const ESPEAK_SOURCE_BYTES: u64 = 17_739_803;
#[cfg(not(test))]
const ESPEAK_SOURCE_SHA256: &str =
    "bb4338102ff3b49a81423da8a1a158b420124b055b60fa76cfb4b18677130a23";
#[cfg(test)]
const ESPEAK_SOURCE_BYTES: u64 = 22;
#[cfg(test)]
const ESPEAK_SOURCE_SHA256: &str =
    "eb1a20133f0683368b8fe8524740c1ed1042e839a23e850a1c57d7a24a032129";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

impl Platform {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }

    /// Stable human-facing artifact name used by release automation.
    pub const fn artifact_name(self) -> &'static str {
        match self {
            Self::Macos => "Beastie-macos.dmg",
            Self::Windows => "Beastie-windows-setup.exe",
            Self::Linux => "Beastie-linux.AppImage",
        }
    }

    fn game_name(self) -> &'static str {
        match self {
            Self::Macos => "beastie",
            Self::Windows => "beastie.exe",
            Self::Linux => "beastie",
        }
    }

    const fn worker_name(self) -> &'static str {
        match self {
            Self::Macos => "beastie-ai-worker",
            Self::Windows => "beastie-ai-worker.exe",
            Self::Linux => "beastie-ai-worker",
        }
    }

    const fn tts_worker_name(self) -> &'static str {
        match self {
            Self::Windows => "beastie-tts.exe",
            Self::Macos | Self::Linux => "beastie-tts",
        }
    }

    const fn espeak_name(self) -> &'static str {
        match self {
            Self::Windows => "espeak-ng.exe",
            Self::Macos | Self::Linux => "espeak-ng",
        }
    }

    const fn runtime_name(self) -> &'static str {
        match self {
            Self::Windows => "llama-server.exe",
            Self::Macos | Self::Linux => "llama-server",
        }
    }
}

#[derive(Debug)]
pub struct PackageOptions<'a> {
    pub destination: &'a Path,
    pub platform: Platform,
    pub game: &'a Path,
    pub worker: &'a Path,
    pub tts_worker: Option<&'a Path>,
    pub espeak: Option<&'a Path>,
    pub espeak_data: Option<&'a Path>,
    pub espeak_license: Option<&'a Path>,
    pub espeak_source: Option<&'a Path>,
    /// Release packages require speech. Development-only staging may opt out explicitly.
    pub require_tts: bool,
    /// The warm local inference runtime and any dynamic libraries it needs.
    /// A worker binary alone is not considered a complete release runtime.
    pub runtime: &'a [PathBuf],
    pub model: &'a Path,
    pub model_license: &'a Path,
    pub model_card: &'a Path,
    pub model_id: &'a str,
    pub repository_root: &'a Path,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageReport {
    pub platform: String,
    pub network: bool,
    #[serde(default)]
    pub release_complete: bool,
    pub total_bytes: u64,
    pub size_budget_bytes: u64,
    pub files: BTreeMap<String, FileRecord>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct FileRecord {
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
struct ModelManifest {
    version: u32,
    selection: Selection,
    candidate: Vec<ModelCandidate>,
}

#[derive(Debug, Deserialize)]
struct Selection {
    dialogue: DialogueSelection,
}

#[derive(Debug, Deserialize)]
struct DialogueSelection {
    preferred_candidate: String,
}

#[derive(Debug, Deserialize)]
struct ModelCandidate {
    id: String,
    #[serde(default)]
    upstream_revision: String,
    #[serde(default)]
    quantization_revision: String,
    #[serde(default)]
    source_revision: String,
    license: String,
    #[serde(default)]
    license_sha256: String,
    #[serde(default)]
    model_card_sha256: String,
    file: String,
    bytes: u64,
    sha256: String,
    tracked: bool,
}

/// Stage a release package and write `package-manifest.json` into it.
pub fn build(options: PackageOptions<'_>) -> Result<PackageReport> {
    validate_options(&options)?;
    let manifest = load_model_manifest(&options.repository_root.join("models/manifest.toml"))?;
    let model = select_model(&manifest, options.model_id)?;
    if model.license.trim().is_empty() {
        bail!("model {} has no license in models/manifest.toml", model.id);
    }
    if model.tracked {
        bail!(
            "model {} is tracked in git; release weights must remain external to the repository",
            model.id
        );
    }
    validate_model_file(options.model, model)?;
    validate_provenance_file(
        options.model_license,
        &model.license_sha256,
        "model license",
    )?;
    validate_provenance_file(options.model_card, &model.model_card_sha256, "model card")?;
    for (label, revision) in [
        ("upstream", &model.upstream_revision),
        ("quantization", &model.quantization_revision),
        ("source", &model.source_revision),
    ] {
        if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("model {} has an invalid {label} revision", model.id);
        }
    }
    let platform_root = options.destination.join(options.platform.as_str());
    validate_destination(&platform_root)?;
    fs::create_dir_all(&platform_root).with_context(|| {
        format!(
            "failed to create package destination {}",
            platform_root.display()
        )
    })?;
    let mut staging = StagingGuard::new(platform_root.clone());
    copy_binary(
        options.game,
        &platform_root.join(options.platform.game_name()),
        "game",
    )?;
    copy_binary(
        options.worker,
        &platform_root.join(options.platform.worker_name()),
        "AI worker",
    )?;
    if options.runtime.is_empty() {
        bail!("required local inference runtime is absent: provide llama-server and its libraries");
    }
    if !options.runtime.iter().any(|path| {
        path.file_name()
            .is_some_and(|name| name == options.platform.runtime_name())
    }) {
        bail!(
            "runtime files do not include the required {} executable",
            options.platform.runtime_name()
        );
    }
    if !options
        .runtime
        .iter()
        .any(|path| path.file_name().is_some_and(|name| name == "LICENSE"))
    {
        bail!("runtime files do not include the required llama.cpp LICENSE");
    }
    let runtime_destination = platform_root.join("runtime");
    let mut runtime_names = BTreeSet::new();
    for runtime in options.runtime {
        let name = runtime
            .file_name()
            .context("runtime path must name a file")?;
        if !runtime_names.insert(name.to_owned()) {
            bail!(
                "runtime files contain duplicate basename: {}",
                name.to_string_lossy()
            );
        }
        if name.to_string_lossy().contains("sherpa-onnx")
            || name == options.platform.espeak_name()
            || name == "espeak-ng-data"
            || name == "espeak-ng-COPYING"
            || name == ESPEAK_SOURCE_NAME
        {
            bail!(
                "reserved or GPL-linked TTS runtime must use explicit package inputs: {}",
                runtime.display()
            );
        }
        copy_required_file(
            runtime,
            &runtime_destination.join(name),
            "inference runtime",
        )?;
    }
    if tts_inputs_complete(&options)? {
        copy_binary(
            options.tts_worker.expect("complete TTS inputs"),
            &platform_root.join(options.platform.tts_worker_name()),
            "TTS worker",
        )?;
        copy_binary(
            options.espeak.expect("complete TTS inputs"),
            &runtime_destination.join(options.platform.espeak_name()),
            "eSpeak NG executable",
        )?;
        copy_required_directory(
            options.espeak_data.expect("complete TTS inputs"),
            &runtime_destination.join("espeak-ng-data"),
            "eSpeak NG data",
        )?;
        copy_required_file(
            options.espeak_license.expect("complete TTS inputs"),
            &runtime_destination.join("espeak-ng-COPYING"),
            "eSpeak NG GPLv3 license",
        )?;
        copy_required_file(
            options.espeak_source.expect("complete TTS inputs"),
            &runtime_destination.join(ESPEAK_SOURCE_NAME),
            "eSpeak NG corresponding source",
        )?;
    }

    let assets_source = options.repository_root.join("assets");
    let assets_destination = platform_root.join("assets");
    copy_required_file(
        &assets_source.join("manifest.toml"),
        &assets_destination.join("manifest.toml"),
        "asset manifest",
    )?;
    copy_optional_directory(
        &assets_source.join("generated"),
        &assets_destination.join("generated"),
    )?;
    copy_optional_directory(
        &assets_source.join("final"),
        &assets_destination.join("final"),
    )?;
    copy_required_directory(
        &assets_source.join("licenses"),
        &assets_destination.join("licenses"),
        "asset licenses",
    )?;

    let models_destination = platform_root.join("models");
    copy_required_file(
        &options.repository_root.join("models/manifest.toml"),
        &models_destination.join("manifest.toml"),
        "model manifest",
    )?;
    copy_required_file(
        options.model,
        &models_destination.join(&model.file),
        "dialogue model",
    )?;
    copy_required_file(
        options.model_license,
        &models_destination.join("LICENSE"),
        "model license",
    )?;
    copy_required_file(
        options.model_card,
        &models_destination.join("README.md"),
        "model card",
    )?;
    copy_required_file(
        &options.repository_root.join("THIRD_PARTY_NOTICES"),
        &platform_root.join("THIRD_PARTY_NOTICES"),
        "third-party notices",
    )?;
    copy_required_file(
        &options.repository_root.join("LICENSE"),
        &platform_root.join("LICENSE"),
        "Beastie license",
    )?;

    let files = inventory(&platform_root)?;
    let total_bytes = files.values().try_fold(0_u64, |total, record| {
        total
            .checked_add(record.bytes)
            .context("package size overflow")
    })?;
    if total_bytes > INSTALL_SIZE_BUDGET {
        bail!(
            "package {} is {} bytes, above the {} byte installed-size budget",
            platform_root.display(),
            total_bytes,
            INSTALL_SIZE_BUDGET
        );
    }
    let report = PackageReport {
        platform: options.platform.as_str().to_owned(),
        network: false,
        release_complete: tts_inputs_complete(&options)?,
        total_bytes,
        size_budget_bytes: INSTALL_SIZE_BUDGET,
        files,
    };
    let manifest_path = platform_root.join("package-manifest.json");
    let json =
        serde_json::to_string_pretty(&report).context("failed to encode package manifest")?;
    fs::write(&manifest_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;
    println!(
        "package {} staged: {} files, {} bytes, artifact {}, network disabled",
        options.platform.as_str(),
        report.files.len(),
        report.total_bytes,
        options.platform.artifact_name()
    );
    staging.commit();
    Ok(report)
}

/// Check an existing staged package without changing it.
pub fn check(
    destination: &Path,
    platform: Platform,
    require_release_complete: bool,
) -> Result<PackageReport> {
    let root = destination.join(platform.as_str());
    let manifest_path = root.join("package-manifest.json");
    let source = fs::read_to_string(&manifest_path)
        .with_context(|| format!("missing package manifest {}", manifest_path.display()))?;
    let report: PackageReport = serde_json::from_str(&source)
        .with_context(|| format!("invalid package manifest {}", manifest_path.display()))?;
    if report.platform != platform.as_str() {
        bail!(
            "package manifest platform is {}, expected {}",
            report.platform,
            platform.as_str()
        );
    }
    if report.network {
        bail!("package manifest enables network access; Beastie releases must be offline");
    }
    if require_release_complete && !report.release_complete {
        bail!("package is development-only because the required release TTS bundle is absent");
    }
    if report.size_budget_bytes != INSTALL_SIZE_BUDGET {
        bail!("package manifest has an unexpected installed-size budget");
    }
    for required in [
        platform.game_name(),
        platform.worker_name(),
        "assets/manifest.toml",
        "assets/licenses/atkinson-hyperlegible-next-OFL.txt",
        "models/manifest.toml",
        "models/LICENSE",
        "models/README.md",
        "THIRD_PARTY_NOTICES",
        "LICENSE",
    ] {
        if !root.join(required).is_file() {
            bail!("package is missing required file: {required}");
        }
    }
    validate_executable(&root.join(platform.game_name()), platform, "packaged game")?;
    validate_executable(
        &root.join(platform.worker_name()),
        platform,
        "packaged AI worker",
    )?;
    let packaged_model_manifest = load_model_manifest(&root.join("models/manifest.toml"))?;
    let packaged_model_id = packaged_model_manifest
        .selection
        .dialogue
        .preferred_candidate
        .clone();
    let packaged_model = select_model(&packaged_model_manifest, &packaged_model_id)?;
    validate_model_file(
        &root.join("models").join(&packaged_model.file),
        packaged_model,
    )?;
    validate_provenance_file(
        &root.join("models/LICENSE"),
        &packaged_model.license_sha256,
        "packaged model license",
    )?;
    validate_provenance_file(
        &root.join("models/README.md"),
        &packaged_model.model_card_sha256,
        "packaged model card",
    )?;
    let runtime = root.join("runtime");
    if !runtime.join(platform.runtime_name()).is_file() {
        bail!(
            "package is missing required inference executable: runtime/{}",
            platform.runtime_name()
        );
    }
    if !runtime.join("LICENSE").is_file() {
        bail!("package is missing required llama.cpp license: runtime/LICENSE");
    }
    validate_executable(
        &runtime.join(platform.runtime_name()),
        platform,
        "packaged llama-server",
    )?;
    let has_tts = validate_packaged_tts(&root, platform)?;
    if has_tts != report.release_complete {
        bail!("package release-complete flag does not match its TTS contents");
    }
    for path in report.files.keys() {
        let file = root.join(path);
        let record = report.files.get(path).expect("key from map");
        let actual = file_record(&file)
            .with_context(|| format!("packaged file missing or unreadable: {}", file.display()))?;
        if actual.bytes != record.bytes || actual.sha256 != record.sha256 {
            bail!("packaged file hash or size changed: {path}");
        }
    }
    let mut actual_files = inventory(&root)?;
    actual_files.remove("package-manifest.json");
    if actual_files != report.files {
        bail!("package contents differ from package-manifest.json (stale or unexpected file)");
    }
    let actual_total = actual_files
        .into_iter()
        .filter(|(path, _)| path != "package-manifest.json")
        .try_fold(0_u64, |total, (_, record)| {
            total
                .checked_add(record.bytes)
                .context("package size overflow")
        })?;
    if actual_total != report.total_bytes {
        bail!(
            "package manifest total is {} bytes, actual package is {} bytes",
            report.total_bytes,
            actual_total
        );
    }
    if actual_total > INSTALL_SIZE_BUDGET {
        bail!("package exceeds installed-size budget: {actual_total} bytes");
    }
    for forbidden in ["experimental-gpl-tts", "sherpa-onnx"] {
        if report.files.keys().any(|path| path.contains(forbidden)) {
            bail!("GPL-blocked TTS artifact is present in package: {forbidden}");
        }
    }
    Ok(report)
}

fn validate_options(options: &PackageOptions<'_>) -> Result<()> {
    for (label, path) in [
        ("game", options.game),
        ("AI worker", options.worker),
        ("model", options.model),
    ] {
        if !path.is_file() {
            bail!("required {label} runtime is absent: {}", path.display());
        }
    }
    if options.runtime.is_empty() {
        bail!("required local inference runtime is absent: provide llama-server and its libraries");
    }
    validate_executable(options.game, options.platform, "game")?;
    validate_executable(options.worker, options.platform, "AI worker")?;
    if options.model_id.trim().is_empty() {
        bail!("--model-id must not be empty");
    }
    validate_runtime_inputs(options)?;
    let has_tts = tts_inputs_complete(options)?;
    if options.require_tts && !has_tts {
        bail!(
            "release package requires local TTS; provide all five TTS inputs or use --development-package"
        );
    }
    if !options.repository_root.is_dir() {
        bail!(
            "repository root is absent: {}",
            options.repository_root.display()
        );
    }
    Ok(())
}

fn tts_inputs_complete(options: &PackageOptions<'_>) -> Result<bool> {
    let present = [
        options.tts_worker.is_some(),
        options.espeak.is_some(),
        options.espeak_data.is_some(),
        options.espeak_license.is_some(),
        options.espeak_source.is_some(),
    ];
    let count = present.into_iter().filter(|value| *value).count();
    if count == 0 {
        return Ok(false);
    }
    if count != present.len() {
        bail!(
            "TTS package is incomplete: provide --tts-worker, --espeak, --espeak-data, --espeak-license, and --espeak-source together"
        );
    }
    for (label, path) in [
        ("TTS worker", options.tts_worker.expect("checked present")),
        (
            "eSpeak NG executable",
            options.espeak.expect("checked present"),
        ),
        (
            "eSpeak NG GPLv3 license",
            options.espeak_license.expect("checked present"),
        ),
        (
            "eSpeak NG corresponding source",
            options.espeak_source.expect("checked present"),
        ),
    ] {
        if !path.is_file() {
            bail!("required {label} is absent: {}", path.display());
        }
    }
    validate_executable(
        options.tts_worker.expect("checked present"),
        options.platform,
        "TTS worker",
    )?;
    validate_executable(
        options.espeak.expect("checked present"),
        options.platform,
        "eSpeak NG executable",
    )?;
    let data = options.espeak_data.expect("checked present");
    if !data.is_dir() || !directory_contains_file(data)? {
        bail!(
            "required eSpeak NG data directory is absent or empty: {}",
            data.display()
        );
    }
    if options
        .espeak_license
        .expect("checked present")
        .metadata()
        .context("failed to inspect eSpeak NG GPLv3 license")?
        .len()
        == 0
    {
        bail!("required eSpeak NG GPLv3 license is empty");
    }
    let source_record = file_record(options.espeak_source.expect("checked present"))?;
    if source_record.bytes != ESPEAK_SOURCE_BYTES || source_record.sha256 != ESPEAK_SOURCE_SHA256 {
        bail!("eSpeak NG corresponding source does not match the pinned 1.52.0 archive");
    }
    Ok(true)
}

fn validate_packaged_tts(root: &Path, platform: Platform) -> Result<bool> {
    let worker = root.join(platform.tts_worker_name());
    let espeak = root.join("runtime").join(platform.espeak_name());
    let data = root.join("runtime/espeak-ng-data");
    let license = root.join("runtime/espeak-ng-COPYING");
    let source = root.join("runtime").join(ESPEAK_SOURCE_NAME);
    let present = [
        worker.is_file(),
        espeak.is_file(),
        data.is_dir(),
        license.is_file(),
        source.is_file(),
    ];
    let count = present.into_iter().filter(|value| *value).count();
    if count == 0 {
        return Ok(false);
    }
    if count != present.len() || !directory_contains_file(&data)? || license.metadata()?.len() == 0
    {
        bail!(
            "packaged TTS is incomplete: require sibling {}, runtime/{}, runtime/espeak-ng-data/**, runtime/espeak-ng-COPYING, and runtime/{ESPEAK_SOURCE_NAME}",
            platform.tts_worker_name(),
            platform.espeak_name()
        );
    }
    let source_record = file_record(&source)?;
    if source_record.bytes != ESPEAK_SOURCE_BYTES || source_record.sha256 != ESPEAK_SOURCE_SHA256 {
        bail!("packaged eSpeak NG corresponding source does not match the pinned 1.52.0 archive");
    }
    validate_executable(&worker, platform, "packaged TTS worker")?;
    validate_executable(&espeak, platform, "packaged eSpeak NG executable")?;
    Ok(true)
}

fn validate_runtime_inputs(options: &PackageOptions<'_>) -> Result<()> {
    if !options.runtime.iter().any(|path| {
        path.file_name()
            .is_some_and(|name| name == options.platform.runtime_name())
    }) {
        bail!(
            "runtime files do not include the required {} executable",
            options.platform.runtime_name()
        );
    }
    if !options
        .runtime
        .iter()
        .any(|path| path.file_name().is_some_and(|name| name == "LICENSE"))
    {
        bail!("runtime files do not include the required llama.cpp LICENSE");
    }
    let server = options
        .runtime
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == options.platform.runtime_name())
        })
        .expect("required server checked above");
    validate_executable(server, options.platform, "llama-server")?;
    let mut names = BTreeSet::new();
    for path in options.runtime {
        if !path.is_file() {
            bail!("required inference runtime is absent: {}", path.display());
        }
        let name = path.file_name().context("runtime path must name a file")?;
        if !names.insert(name.to_owned()) {
            bail!(
                "runtime files contain duplicate basename: {}",
                name.to_string_lossy()
            );
        }
    }
    Ok(())
}

fn validate_executable(path: &Path, platform: Platform, label: &str) -> Result<()> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open {label} executable {}", path.display()))?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)
        .with_context(|| format!("{label} executable is too short: {}", path.display()))?;
    let valid = match platform {
        Platform::Windows => magic.starts_with(b"MZ"),
        Platform::Linux => magic == *b"\x7fELF",
        Platform::Macos => matches!(
            magic,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        ),
    };
    if !valid {
        bail!(
            "{label} executable does not match the {} binary format: {}",
            platform.as_str(),
            path.display()
        );
    }
    Ok(())
}

struct StagingGuard {
    platform_root: PathBuf,
    armed: bool,
}

impl StagingGuard {
    fn new(platform_root: PathBuf) -> Self {
        Self {
            platform_root,
            armed: true,
        }
    }

    fn commit(&mut self) {
        self.armed = false;
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.platform_root);
        }
    }
}

fn validate_destination(destination: &Path) -> Result<()> {
    if destination.exists() {
        let mut entries = fs::read_dir(destination).with_context(|| {
            format!(
                "failed to inspect package destination {}",
                destination.display()
            )
        })?;
        if entries.next().is_some() {
            bail!(
                "package destination must be new or empty to prevent stale release files: {}",
                destination.display()
            );
        }
    }
    Ok(())
}

fn load_model_manifest(path: &Path) -> Result<ModelManifest> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read model manifest {}", path.display()))?;
    let manifest: ModelManifest = toml::from_str(&source)
        .with_context(|| format!("failed to parse model manifest {}", path.display()))?;
    if manifest.version != 1 {
        bail!("model manifest must declare version = 1");
    }
    Ok(manifest)
}

fn select_model<'a>(manifest: &'a ModelManifest, requested: &str) -> Result<&'a ModelCandidate> {
    let preferred = &manifest.selection.dialogue.preferred_candidate;
    if requested != preferred {
        bail!("model {requested} is not the selected dialogue candidate {preferred}");
    }
    manifest
        .candidate
        .iter()
        .find(|candidate| candidate.id == requested)
        .with_context(|| format!("selected model {requested} is absent from models/manifest.toml"))
}

fn validate_model_file(path: &Path, expected: &ModelCandidate) -> Result<()> {
    let record = file_record(path)?;
    if record.bytes != expected.bytes {
        bail!(
            "model {} has {} bytes, manifest requires {} bytes",
            path.display(),
            record.bytes,
            expected.bytes
        );
    }
    if expected.sha256.len() != SHA256_HEX_LEN
        || !expected.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("model {} has an invalid manifest SHA-256", expected.id);
    }
    if record.sha256 != expected.sha256 {
        bail!(
            "model {} SHA-256 mismatch: got {}, expected {}",
            path.display(),
            record.sha256,
            expected.sha256
        );
    }
    Ok(())
}

fn validate_provenance_file(path: &Path, expected_sha256: &str, label: &str) -> Result<()> {
    if expected_sha256.len() != SHA256_HEX_LEN
        || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("{label} has an invalid manifest SHA-256");
    }
    let actual = file_record(path)?;
    if actual.sha256 != expected_sha256 {
        bail!(
            "{label} SHA-256 mismatch for {}: got {}, expected {}",
            path.display(),
            actual.sha256,
            expected_sha256
        );
    }
    Ok(())
}

fn copy_binary(source: &Path, destination: &Path, label: &str) -> Result<()> {
    copy_required_file(source, destination, label)
}

fn copy_required_file(source: &Path, destination: &Path, label: &str) -> Result<()> {
    let file_type = fs::symlink_metadata(source)
        .with_context(|| format!("failed to inspect {label} {}", source.display()))?
        .file_type();
    if file_type.is_symlink() {
        let target = source
            .canonicalize()
            .with_context(|| format!("failed to resolve {label} {}", source.display()))?;
        let parent = source
            .parent()
            .context("symlinked package input has no parent")?
            .canonicalize()
            .context("failed to resolve symlinked package input parent")?;
        if target.parent() != Some(parent.as_path()) {
            bail!(
                "required {label} symlink must resolve to a sibling file: {}",
                source.display()
            );
        }
    }
    if !source.is_file() {
        bail!("required {label} is absent: {}", source.display());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy {label} from {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn copy_optional_directory(source: &Path, destination: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect asset source {}", source.display()));
        }
    };
    if metadata.file_type().is_symlink() {
        bail!("asset source must not be a symlink: {}", source.display());
    }
    if !metadata.is_dir() {
        bail!("asset source is not a directory: {}", source.display());
    }
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry.context("failed to read asset directory entry")?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .context("failed to inspect asset directory entry")?
            .is_symlink()
        {
            bail!(
                "asset source must not be a symlink: {}",
                source_path.display()
            );
        }
        if source_path.is_dir() {
            copy_optional_directory(&source_path, &destination_path)?;
        } else {
            copy_required_file(&source_path, &destination_path, "asset")?;
        }
    }
    Ok(())
}

fn copy_required_directory(source: &Path, destination: &Path, label: &str) -> Result<()> {
    if fs::symlink_metadata(source)
        .with_context(|| format!("failed to inspect {label} directory {}", source.display()))?
        .file_type()
        .is_symlink()
    {
        bail!(
            "required {label} directory must not be a symlink: {}",
            source.display()
        );
    }
    if !source.is_dir() {
        bail!("required {label} directory is absent: {}", source.display());
    }
    if !directory_contains_file(source)? {
        bail!("required {label} directory is empty: {}", source.display());
    }
    copy_optional_directory(source, destination)
}

fn directory_contains_file(directory: &Path) -> Result<bool> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read directory {}", directory.display()))?
    {
        let entry = entry.context("failed to read directory entry")?;
        let file_type = entry
            .file_type()
            .context("failed to inspect directory entry")?;
        if file_type.is_file() {
            return Ok(true);
        }
        if file_type.is_dir() && directory_contains_file(&entry.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inventory(root: &Path) -> Result<BTreeMap<String, FileRecord>> {
    let mut files = BTreeMap::new();
    inventory_directory(root, root, &mut files)?;
    Ok(files)
}

fn inventory_directory(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, FileRecord>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
    {
        let entry = entry.context("failed to read package directory entry")?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .context("failed to inspect package directory entry")?;
        if file_type.is_symlink() {
            bail!("package contains unsupported symlink: {}", path.display());
        }
        if file_type.is_dir() {
            inventory_directory(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .context("package path escaped destination")?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, file_record(&path)?);
        } else {
            bail!(
                "package contains unsupported special file: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn file_record(path: &Path) -> Result<FileRecord> {
    let mut file =
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::default();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .context("file size overflow")?;
        hasher.update(&buffer[..read]);
    }
    Ok(FileRecord {
        bytes,
        sha256: hasher.finish(),
    })
}

#[derive(Clone, Copy)]
struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffered: 0,
            length: 0,
        }
    }
}

impl Sha256 {
    fn update(&mut self, mut bytes: &[u8]) {
        self.length = self.length.saturating_add(bytes.len() as u64);
        if self.buffered != 0 {
            let needed = 64 - self.buffered;
            if bytes.len() < needed {
                self.buffer[self.buffered..self.buffered + bytes.len()].copy_from_slice(bytes);
                self.buffered += bytes.len();
                return;
            }
            self.buffer[self.buffered..].copy_from_slice(&bytes[..needed]);
            let block = self.buffer;
            self.compress(block);
            self.buffered = 0;
            bytes = &bytes[needed..];
        }
        while bytes.len() >= 64 {
            self.compress(bytes[..64].try_into().expect("64-byte block"));
            bytes = &bytes[64..];
        }
        self.buffer[..bytes.len()].copy_from_slice(bytes);
        self.buffered = bytes.len();
    }

    fn finish(mut self) -> String {
        let bit_length = self.length.saturating_mul(8);
        self.buffer[self.buffered] = 0x80;
        self.buffered += 1;
        if self.buffered > 56 {
            self.buffer[self.buffered..].fill(0);
            let block = self.buffer;
            self.compress(block);
            self.buffered = 0;
        }
        self.buffer[self.buffered..56].fill(0);
        self.buffer[56..].copy_from_slice(&bit_length.to_be_bytes());
        let block = self.buffer;
        self.compress(block);
        let mut output = String::with_capacity(SHA256_HEX_LEN);
        for word in self.state {
            output.push_str(&format!("{word:08x}"));
        }
        output
    }

    fn compress(&mut self, block: [u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut words = [0_u32; 64];
        for (index, chunk) in block.chunks_exact(4).enumerate().take(16) {
            words[index] = u32::from_be_bytes(chunk.try_into().expect("four-byte word"));
        }
        for index in 16..64 {
            let x = words[index - 15];
            let y = words[index - 2];
            let small_sigma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let small_sigma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            words[index] = words[index - 16]
                .wrapping_add(small_sigma0)
                .wrapping_add(words[index - 7])
                .wrapping_add(small_sigma1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sigma1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestPackage {
        root: PathBuf,
        destination: PathBuf,
        game: PathBuf,
        worker: PathBuf,
        tts_worker: PathBuf,
        espeak: PathBuf,
        espeak_data: PathBuf,
        espeak_license: PathBuf,
        espeak_source: PathBuf,
        model: PathBuf,
        model_license: PathBuf,
        model_card: PathBuf,
        runtime: Vec<PathBuf>,
    }

    impl Drop for TestPackage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn temporary_root(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "beastie-packaging-{label}-{}-{now}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temporary package root should be created");
        root
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("temporary parent should be created");
        }
        fs::write(path, bytes).expect("temporary file should be written");
    }

    fn test_package(label: &str) -> TestPackage {
        let root = temporary_root(label);
        write_file(
            &root.join("models/manifest.toml"),
            br#"version = 1

[selection.dialogue]
preferred_candidate = "test-model"

[[candidate]]
id = "test-model"
upstream_revision = "1111111111111111111111111111111111111111"
quantization_revision = "2222222222222222222222222222222222222222"
source_revision = "1111111111111111111111111111111111111111"
license = "Apache-2.0"
license_sha256 = "a098d20414682e24cee9b629c4ce804c487c434d7ab92df8f17b18c52bfe1baf"
model_card_sha256 = "1768200bb6bba3ddcc1f2c0596bc5f95bf17bf92b2ed34abdb72cf3a04d19733"
file = "test.gguf"
bytes = 4
sha256 = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
tracked = false
"#,
        );
        write_file(&root.join("assets/manifest.toml"), b"version = 1\n");
        write_file(
            &root.join("assets/licenses/atkinson-hyperlegible-next-OFL.txt"),
            b"OFL license fixture\n",
        );
        write_file(&root.join("THIRD_PARTY_NOTICES"), b"test notices\n");
        write_file(&root.join("LICENSE"), b"MIT license\n");
        let game = root.join("inputs/game");
        let worker = root.join("inputs/worker");
        let tts_worker = root.join("inputs/beastie-tts");
        let espeak = root.join("inputs/espeak-ng");
        let espeak_data = root.join("inputs/espeak-ng-data");
        let espeak_license = root.join("inputs/espeak-ng-COPYING");
        let espeak_source = root.join("inputs/espeak-ng-1.52.0.tar.gz");
        let model = root.join("inputs/model.gguf");
        let model_license = root.join("inputs/model-LICENSE");
        let model_card = root.join("inputs/model-README.md");
        let macho = b"\xcf\xfa\xed\xfe fixture";
        write_file(&game, macho);
        write_file(&worker, macho);
        write_file(&tts_worker, macho);
        write_file(&espeak, macho);
        write_file(&espeak_data.join("voices/en"), b"voice data");
        write_file(&espeak_license, b"GPLv3 license\n");
        write_file(&espeak_source, b"source archive fixture");
        write_file(&model, b"test");
        write_file(&model_license, b"model license\n");
        write_file(&model_card, b"model card\n");
        let llama_server = root.join("inputs/llama-server");
        let runtime_library = root.join("inputs/libllama.dylib");
        let runtime_license = root.join("inputs/LICENSE");
        write_file(&llama_server, macho);
        write_file(&runtime_library, b"library");
        write_file(&runtime_license, b"MIT license\n");
        TestPackage {
            destination: root.join("dist"),
            root,
            game,
            worker,
            tts_worker,
            espeak,
            espeak_data,
            espeak_license,
            espeak_source,
            model,
            model_license,
            model_card,
            runtime: vec![llama_server, runtime_library, runtime_license],
        }
    }

    fn options<'a>(package: &'a TestPackage, runtime: &'a [PathBuf]) -> PackageOptions<'a> {
        PackageOptions {
            destination: &package.destination,
            platform: Platform::Macos,
            game: &package.game,
            worker: &package.worker,
            tts_worker: Some(&package.tts_worker),
            espeak: Some(&package.espeak),
            espeak_data: Some(&package.espeak_data),
            espeak_license: Some(&package.espeak_license),
            espeak_source: Some(&package.espeak_source),
            require_tts: true,
            runtime,
            model: &package.model,
            model_license: &package.model_license,
            model_card: &package.model_card,
            model_id: "test-model",
            repository_root: &package.root,
        }
    }

    #[test]
    fn sha256_matches_known_empty_digest() {
        let mut hasher = Sha256::default();
        hasher.update(b"");
        assert_eq!(
            hasher.finish(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_matches_known_sentence_digest() {
        let mut hasher = Sha256::default();
        hasher.update(b"The quick brown fox jumps over the lazy dog");
        assert_eq!(
            hasher.finish(),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn platform_layout_is_explicit() {
        assert_eq!(Platform::Macos.game_name(), "beastie");
        assert_eq!(Platform::Windows.worker_name(), "beastie-ai-worker.exe");
        assert_eq!(Platform::Linux.as_str(), "linux");
    }

    #[test]
    fn platform_artifact_names_are_stable() {
        assert_eq!(Platform::Macos.artifact_name(), "Beastie-macos.dmg");
        assert_eq!(
            Platform::Windows.artifact_name(),
            "Beastie-windows-setup.exe"
        );
        assert_eq!(Platform::Linux.artifact_name(), "Beastie-linux.AppImage");
    }

    #[test]
    fn missing_runtime_is_a_clear_failure() {
        let options = PackageOptions {
            destination: Path::new("target/package-test"),
            platform: Platform::Macos,
            game: Path::new("Cargo.toml"),
            worker: Path::new("Cargo.toml"),
            tts_worker: None,
            espeak: None,
            espeak_data: None,
            espeak_license: None,
            espeak_source: None,
            require_tts: true,
            runtime: &[],
            model: Path::new("Cargo.toml"),
            model_license: Path::new("Cargo.toml"),
            model_card: Path::new("Cargo.toml"),
            model_id: "qwen3.5-0.8b-q4_0",
            repository_root: Path::new("."),
        };
        let error = validate_options(&options).expect_err("missing runtime should fail");
        assert!(
            error
                .to_string()
                .contains("local inference runtime is absent")
        );
    }

    #[test]
    fn parses_the_checked_in_model_manifest() {
        let manifest_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/manifest.toml");
        let manifest = load_model_manifest(&manifest_path).expect("checked-in manifest parses");
        assert_eq!(manifest.version, 1);
        assert_eq!(
            manifest.selection.dialogue.preferred_candidate,
            "qwen3.5-0.8b-q4_0"
        );
        let selected = select_model(&manifest, "qwen3.5-0.8b-q4_0")
            .expect("preferred model should be declared");
        assert_eq!(selected.license, "Apache-2.0");
        assert_eq!(selected.bytes, 563_036_064);
        assert_eq!(selected.sha256.len(), SHA256_HEX_LEN);
    }

    #[test]
    fn runtime_without_llama_server_is_rejected() {
        let package = test_package("missing-server");
        let runtime = vec![package.runtime[1].clone(), package.runtime[2].clone()];
        let error = build(options(&package, &runtime)).expect_err("runtime must include server");
        assert!(
            error
                .to_string()
                .contains("required llama-server executable")
        );
    }

    #[test]
    fn wrong_platform_executable_is_rejected() {
        let package = test_package("wrong-platform-binary");
        write_file(&package.game, b"MZ\0\0 windows fixture");
        let error = build(options(&package, &package.runtime))
            .expect_err("a Windows executable must not enter a macOS package");
        assert!(error.to_string().contains("macos binary format"));
    }

    #[test]
    fn executable_format_sanity_covers_every_release_platform() {
        let root = temporary_root("binary-formats");
        let windows = root.join("beastie.exe");
        let linux = root.join("beastie-linux");
        let macos = root.join("beastie-macos");
        write_file(&windows, b"MZ\0\0 fixture");
        write_file(&linux, b"\x7fELF fixture");
        write_file(&macos, b"\xcf\xfa\xed\xfe fixture");

        validate_executable(&windows, Platform::Windows, "Windows fixture")
            .expect("PE magic should pass the Windows sanity gate");
        validate_executable(&linux, Platform::Linux, "Linux fixture")
            .expect("ELF magic should pass the Linux sanity gate");
        validate_executable(&macos, Platform::Macos, "macOS fixture")
            .expect("Mach-O magic should pass the macOS sanity gate");
        assert!(validate_executable(&windows, Platform::Linux, "mismatch").is_err());
    }

    #[test]
    fn platform_package_preserves_sibling_platforms() {
        let package = test_package("platform-siblings");
        let windows_marker = package.destination.join("windows/existing-package.txt");
        write_file(&windows_marker, b"keep me");

        build(options(&package, &package.runtime))
            .expect("macOS staging should allow an existing Windows sibling");

        assert_eq!(
            fs::read(&windows_marker).expect("sibling package should remain readable"),
            b"keep me"
        );
        assert!(
            package
                .destination
                .join("macos/package-manifest.json")
                .is_file()
        );
    }

    #[test]
    fn tts_inputs_must_be_complete_or_absent() {
        let package = test_package("partial-tts-input");
        let mut package_options = options(&package, &package.runtime);
        package_options.espeak_license = None;
        let error = build(package_options).expect_err("partial TTS input must fail");
        assert!(error.to_string().contains("TTS package is incomplete"));

        let package = test_package("absent-tts-input");
        let mut package_options = options(&package, &package.runtime);
        package_options.tts_worker = None;
        package_options.espeak = None;
        package_options.espeak_data = None;
        package_options.espeak_license = None;
        package_options.espeak_source = None;
        package_options.require_tts = false;
        let report = build(package_options).expect("fully absent optional TTS should stage");
        assert!(!report.files.contains_key("beastie-tts"));
        assert!(!report.release_complete);
        check(&package.destination, Platform::Macos, false)
            .expect("explicit development-package check should allow missing TTS");
        let error = check(&package.destination, Platform::Macos, true)
            .expect_err("release check must require TTS");
        assert!(error.to_string().contains("development-only"));
    }

    #[test]
    fn package_check_rejects_incomplete_tts_bundle() {
        let package = test_package("partial-tts-package");
        let report =
            build(options(&package, &package.runtime)).expect("complete TTS package should stage");
        assert!(report.files.contains_key("beastie-tts"));
        assert!(report.files.contains_key("runtime/espeak-ng"));
        assert!(
            report
                .files
                .contains_key("runtime/espeak-ng-data/voices/en")
        );
        assert!(report.files.contains_key("runtime/espeak-ng-COPYING"));
        assert!(report.files.contains_key("runtime/espeak-ng-1.52.0.tar.gz"));
        check(&package.destination, Platform::Macos, true)
            .expect("complete packaged TTS should pass validation");
        fs::remove_file(package.destination.join("macos/runtime/espeak-ng-COPYING")).unwrap();
        let error = check(&package.destination, Platform::Macos, true)
            .expect_err("partial packaged TTS must fail before launch");
        assert!(error.to_string().contains("packaged TTS is incomplete"));
    }

    #[test]
    fn runtime_without_llama_license_is_rejected() {
        let package = test_package("missing-runtime-license");
        let runtime = vec![package.runtime[0].clone(), package.runtime[1].clone()];
        let error = build(options(&package, &runtime)).expect_err("runtime must include license");
        assert!(error.to_string().contains("required llama.cpp LICENSE"));
    }

    #[cfg(unix)]
    #[test]
    fn safe_sibling_runtime_symlink_is_accepted() {
        use std::os::unix::fs::symlink;

        let package = test_package("safe-link");
        let link = package.root.join("inputs/llama-server");
        let sibling_target = package.root.join("inputs/llama-server-real");
        fs::rename(&package.runtime[0], &sibling_target)
            .expect("runtime source should be renamed beside its link");
        symlink(&sibling_target, &link).expect("sibling symlink should be created");
        let runtime = vec![link, package.runtime[1].clone(), package.runtime[2].clone()];
        let report = build(options(&package, &runtime)).expect("safe sibling link should stage");
        assert!(report.files.contains_key("runtime/llama-server"));
    }

    #[cfg(unix)]
    #[test]
    fn escaping_runtime_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let package = test_package("escaping-link");
        let outside = temporary_root("escaping-target");
        let outside_server = outside.join("llama-server");
        write_file(&outside_server, b"\xcf\xfa\xed\xfe outside");
        let link = package.root.join("inputs/llama-server");
        fs::remove_file(&package.runtime[0]).expect("runtime source should be removed for link");
        symlink(&outside_server, &link).expect("escaping symlink should be created");
        let runtime = vec![link, package.runtime[1].clone(), package.runtime[2].clone()];
        let error = build(options(&package, &runtime)).expect_err("escaping link must fail");
        assert!(error.to_string().contains("must resolve to a sibling file"));
        fs::remove_dir_all(outside).expect("outside test directory should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_asset_root_is_rejected() {
        use std::os::unix::fs::symlink;

        let package = test_package("symlinked-asset-root");
        let outside = temporary_root("external-assets");
        write_file(&outside.join("room/background.png"), b"not packaged");
        symlink(&outside, package.root.join("assets/generated"))
            .expect("asset-root symlink should be created");

        let error = build(options(&package, &package.runtime))
            .expect_err("a symlinked asset root must not be followed");
        assert!(
            error
                .to_string()
                .contains("asset source must not be a symlink")
        );
        fs::remove_dir_all(outside).expect("outside fixture should be removed");
    }

    #[test]
    fn package_check_rejects_tampered_file_hash() {
        let package = test_package("tamper");
        build(options(&package, &package.runtime)).expect("test package should stage");
        let packaged_game = package.destination.join("macos/beastie");
        let mut file = OpenOptions::new()
            .append(true)
            .open(&packaged_game)
            .expect("packaged game should exist");
        file.write_all(b"tampered")
            .expect("packaged game should be writable in test");
        let error = check(&package.destination, Platform::Macos, true)
            .expect_err("tampered package must fail integrity check");
        assert!(error.to_string().contains("hash or size changed"));
    }

    #[test]
    fn package_check_requires_the_runtime_shape() {
        let package = test_package("check-runtime-shape");
        build(options(&package, &package.runtime)).expect("test package should stage");
        fs::remove_file(package.destination.join("macos/runtime/llama-server"))
            .expect("packaged server should be removable");
        let error = check(&package.destination, Platform::Macos, true)
            .expect_err("package without its server must fail");
        assert!(error.to_string().contains("inference executable"));

        let package = test_package("check-runtime-license");
        build(options(&package, &package.runtime)).expect("test package should stage");
        fs::remove_file(package.destination.join("macos/runtime/LICENSE"))
            .expect("packaged license should be removable");
        let error = check(&package.destination, Platform::Macos, true)
            .expect_err("package without the llama.cpp license must fail");
        assert!(error.to_string().contains("llama.cpp license"));
    }

    #[test]
    fn package_check_requires_corresponding_espeak_source() {
        let package = test_package("check-espeak-source");
        build(options(&package, &package.runtime)).expect("test package should stage");
        fs::remove_file(
            package
                .destination
                .join("macos/runtime/espeak-ng-1.52.0.tar.gz"),
        )
        .expect("packaged source should be removable");
        let error = check(&package.destination, Platform::Macos, true)
            .expect_err("eSpeak package without corresponding source must fail");
        assert!(error.to_string().contains("packaged TTS is incomplete"));
    }

    #[cfg(unix)]
    #[test]
    fn package_check_rejects_symlinked_files_and_directories() {
        use std::os::unix::fs::symlink;

        let package = test_package("symlinked-package-entry");
        build(options(&package, &package.runtime)).expect("test package should stage");
        let root = package.destination.join("macos");
        let outside = package.root.join("outside");
        write_file(&outside, b"must not be followed");

        fs::remove_file(root.join("LICENSE")).expect("license should be removable");
        symlink(&outside, root.join("LICENSE")).expect("file symlink should be created");
        let error = inventory(&root).expect_err("package inventory must reject symlinked files");
        assert!(error.to_string().contains("unsupported symlink"));

        fs::remove_file(root.join("LICENSE")).expect("file symlink should be removable");
        write_file(&root.join("LICENSE"), b"MIT license\n");
        let outside_dir = package.root.join("outside-dir");
        write_file(&outside_dir.join("escape.txt"), b"must not be followed");
        fs::remove_dir_all(root.join("assets")).expect("assets should be removable");
        symlink(&outside_dir, root.join("assets")).expect("directory symlink should be created");
        let error =
            inventory(&root).expect_err("package inventory must reject symlinked directories");
        assert!(error.to_string().contains("unsupported symlink"));
    }

    #[test]
    fn failed_staging_leaves_the_destination_retryable() {
        let package = test_package("retry-after-failure");
        let asset_manifest = package.root.join("assets/manifest.toml");
        fs::remove_file(&asset_manifest).expect("fixture manifest should be removable");
        build(options(&package, &package.runtime)).expect_err("missing assets should fail staging");
        assert!(
            !package.destination.join("macos").exists(),
            "failed platform staging directory should be cleaned"
        );

        write_file(&asset_manifest, b"version = 1\n");
        build(options(&package, &package.runtime))
            .expect("the same destination should accept a corrected retry");
    }
}
