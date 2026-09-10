//! Opt-in native acceptance measurements. Wall-frame intervals include CPU work,
//! GPU backpressure, presentation pacing and captures; they are not GPU timings.
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

use bevy::{
    app::AppExit,
    platform::time::Instant,
    prelude::*,
    render::{render_resource::ShaderType, renderer::RenderAdapterInfo},
};
use serde::Serialize;

use crate::ray_scene::{BvhNode, GeometryTriangle, GpuInstance, RayScene, TriangleSurface};

const WARMUP_FRAMES: u64 = 30;

pub struct RayStatsPlugin(pub Option<PathBuf>, pub bool);

impl Plugin for RayStatsPlugin {
    fn build(&self, app: &mut App) {
        if let Some(path) = &self.0 {
            // Bevy's diagnostic frame pool grows when readbacks fall behind.
            // Our bounded timestamp slots provide all GPU measurements below.
            let gpu = ComputeGpuTiming::default();
            app.insert_resource(gpu.clone());
            if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
                render_app.insert_resource(gpu);
            }
            app.insert_resource(FrameMeasurements {
                path: path.clone(),
                uncapped: self.1,
                startup_started: Some(Instant::now()),
                probe: app
                    .world()
                    .get_resource::<crate::args::RenderProbe>()
                    .copied()
                    .unwrap_or_default(),
                ..default()
            })
            .add_systems(Last, (collect_compute_timings, record_and_finish).chain());
        }
    }
}

#[derive(Resource, Default)]
struct FrameMeasurements {
    path: PathBuf,
    uncapped: bool,
    probe: crate::args::RenderProbe,
    compute_gpu: PassMeasurements,
    gpu_passes: BTreeMap<&'static str, PassMeasurements>,
    cpu_phases: BTreeMap<&'static str, PassMeasurements>,
    gpu_geometry_allocated_bytes: u64,
    gpu_framebuffer_nominal_bytes: u64,
    shadow_maps: Option<ShadowMapReport>,
    viewport_pixels: Option<[u32; 2]>,
    startup_started: Option<Instant>,
    renderer_ready_ms: Option<f64>,
    frames_seen: u64,
    wall_frame_ms: Vec<f64>,
    peak_scene: SceneSize,
    written: bool,
}

/// Opt-in pass-boundary timestamps also work on Metal, where Bevy intentionally
/// suppresses encoder timestamp writes. Twelve reusable slots (four for each of
/// compute, primary visibility and dynamic shadows) bound readback memory;
/// if the GPU falls behind, skip instrumentation instead of stalling rendering.
#[derive(Resource, Clone, Default)]
pub(crate) struct ComputeGpuTiming {
    slots: Arc<Mutex<Vec<GpuTimingSlot>>>,
    completed: Arc<Mutex<Vec<GpuSample>>>,
    cpu_completed: Arc<Mutex<Vec<CpuSample>>>,
    geometry_bytes: Arc<AtomicU64>,
    framebuffer_bytes: Arc<AtomicU64>,
    shadow_maps: Arc<Mutex<Option<ShadowMapReport>>>,
    viewport: Arc<AtomicU64>,
}

#[derive(Clone, Serialize)]
struct ShadowMapReport {
    enabled: bool,
    reason: &'static str,
    resolution: u32,
    texture_allocated_bytes: u64,
    uniform_allocated_bytes: u64,
    static_map_bytes: u64,
    static_map_resolution: u32,
    indexed_shadow_bytes: u64,
    dynamic_atlas: bool,
    depth_bits: u64,
}

struct GpuSample {
    name: &'static str,
    milliseconds: f64,
}

struct CpuSample {
    name: &'static str,
    milliseconds: f64,
}

/// Owns its queue reference so the measured system can mutate its other inputs.
/// Constructed only with an opt-in report resource; Drop also covers early returns.
#[must_use]
pub(crate) struct CpuPhaseSpan {
    name: &'static str,
    started: Instant,
    completed: Arc<Mutex<Vec<CpuSample>>>,
}

impl Drop for CpuPhaseSpan {
    fn drop(&mut self) {
        let milliseconds = self.started.elapsed().as_secs_f64() * 1000.0;
        self.completed.lock().unwrap().push(CpuSample {
            name: self.name,
            milliseconds,
        });
    }
}

#[derive(Clone)]
pub(crate) struct GpuTimingSlot {
    pub queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    completion_fence: wgpu::Buffer,
    name: &'static str,
    // 0 free, 1 recording/waiting for frame completion, 2 ready to resolve,
    // 3 waiting for query readback. A query set is never reused before state 0.
    state: Arc<AtomicU8>,
}

impl ComputeGpuTiming {
    pub(crate) fn observe_shadow_maps(
        &self,
        enabled: bool,
        reason: &'static str,
        resolution: u32,
        texture_allocated_bytes: u64,
        uniform_allocated_bytes: u64,
    ) {
        *self.shadow_maps.lock().unwrap() = Some(ShadowMapReport {
            enabled,
            reason,
            resolution,
            texture_allocated_bytes,
            uniform_allocated_bytes,
            static_map_bytes: 0,
            static_map_resolution: 0,
            indexed_shadow_bytes: 0,
            dynamic_atlas: false,
            depth_bits: crate::ray_static_maps::DEPTH_BYTES * 8,
        });
    }

    pub(crate) fn observe_shadow_storage(
        &self,
        static_map_bytes: u64,
        static_map_resolution: u32,
        indexed_shadow_bytes: u64,
        dynamic_atlas: bool,
    ) {
        if let Some(report) = self.shadow_maps.lock().unwrap().as_mut() {
            report.static_map_bytes = static_map_bytes;
            report.static_map_resolution = static_map_resolution;
            report.indexed_shadow_bytes = indexed_shadow_bytes;
            report.dynamic_atlas = dynamic_atlas;
        }
    }
    pub(crate) fn cpu_span(&self, name: &'static str) -> CpuPhaseSpan {
        CpuPhaseSpan {
            name,
            started: Instant::now(),
            completed: self.cpu_completed.clone(),
        }
    }

    pub(crate) fn observe_scene(&self, geometry_bytes: u64, size: UVec2) {
        self.geometry_bytes
            .fetch_max(geometry_bytes, Ordering::Relaxed);
        self.viewport.store(
            (u64::from(size.x) << 32) | u64::from(size.y),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn observe_framebuffers(&self, nominal_bytes: u64) {
        self.framebuffer_bytes
            .fetch_max(nominal_bytes, Ordering::Relaxed);
    }

    pub(crate) fn begin(
        &self,
        device: &bevy::render::renderer::RenderDevice,
    ) -> Option<GpuTimingSlot> {
        self.begin_named(device, "compute")
    }

    pub(crate) fn begin_named(
        &self,
        device: &bevy::render::renderer::RenderDevice,
        name: &'static str,
    ) -> Option<GpuTimingSlot> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let mut slots = self.slots.lock().unwrap();
        if let Some(slot) = slots.iter().find(|slot| {
            slot.name == name
                && slot
                    .state
                    .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
        }) {
            return Some(slot.clone());
        }
        if slots.len() == 12 || slots.iter().filter(|slot| slot.name == name).count() == 4 {
            return None;
        }
        let device = device.wgpu_device();
        let slot = GpuTimingSlot {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("aquarium compute timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("aquarium timestamp resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("aquarium timestamp readback"),
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            completion_fence: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("aquarium timestamp completion fence"),
                size: 4,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            name,
            state: Arc::new(AtomicU8::new(1)),
        };
        slots.push(slot.clone());
        Some(slot)
    }

    pub(crate) fn finish(
        &self,
        slot: GpuTimingSlot,
        encoder: &mut wgpu::CommandEncoder,
        _period_ns: f32,
    ) {
        // Metal resolves stage counters too early in the same submission, often
        // returning zero or previous-frame timestamps. This map callback runs
        // only after the submission containing the measured pass completes.
        encoder.clear_buffer(&slot.completion_fence, 0, None);
        let fence = slot.completion_fence.clone();
        encoder.map_buffer_on_submit(
            &slot.completion_fence,
            wgpu::MapMode::Read,
            ..,
            move |result| {
                if result.is_ok() {
                    fence.unmap();
                    slot.state.store(2, Ordering::Release);
                } else {
                    slot.state.store(0, Ordering::Release);
                }
            },
        );
    }

    /// Call before acquiring this frame's slots, even when every slot is busy.
    /// Resolving only completed prior submissions avoids Metal's stale counters
    /// without waiting for the GPU or altering ordinary production scheduling.
    pub(crate) fn resolve_completed(&self, encoder: &mut wgpu::CommandEncoder, period_ns: f32) {
        let slots = self.slots.lock().unwrap();
        for slot in slots.iter() {
            if slot
                .state
                .compare_exchange(2, 3, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                continue;
            }
            encoder.resolve_query_set(&slot.queries, 0..2, &slot.resolve, 0);
            encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, 16);
            let slot = slot.clone();
            let readback = slot.readback.clone();
            let completed = self.completed.clone();
            encoder.map_buffer_on_submit(&slot.readback, wgpu::MapMode::Read, .., move |result| {
                if result.is_ok() {
                    let bytes = readback.slice(..).get_mapped_range();
                    let start = u64::from_le_bytes(bytes[..8].try_into().unwrap());
                    let end = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                    let milliseconds = if start > 0 && end > start {
                        (end - start) as f64 * f64::from(period_ns) / 1e6
                    } else {
                        0.0
                    };
                    completed.lock().unwrap().push(GpuSample {
                        name: slot.name,
                        milliseconds,
                    });
                    drop(bytes);
                    readback.unmap();
                }
                slot.state.store(0, Ordering::Release);
            });
        }
    }
}

fn collect_compute_timings(
    gpu: Res<ComputeGpuTiming>,
    mut measurements: ResMut<FrameMeasurements>,
) {
    measurements.gpu_framebuffer_nominal_bytes = gpu.framebuffer_bytes.load(Ordering::Relaxed);
    measurements.shadow_maps = gpu.shadow_maps.lock().unwrap().clone();
    measurements.gpu_geometry_allocated_bytes = gpu.geometry_bytes.load(Ordering::Relaxed);
    let viewport = gpu.viewport.load(Ordering::Relaxed);
    measurements.viewport_pixels =
        (viewport != 0).then_some([(viewport >> 32) as u32, viewport as u32]);
    for sample in gpu.cpu_completed.lock().unwrap().drain(..) {
        measurements
            .cpu_phases
            .entry(sample.name)
            .or_default()
            .include_value(sample.milliseconds);
    }
    for sample in gpu.completed.lock().unwrap().drain(..) {
        measurements
            .gpu_passes
            .entry(sample.name)
            .or_default()
            .include_value(sample.milliseconds);
        if sample.name == "compute" {
            measurements.compute_gpu.include_value(sample.milliseconds);
        }
    }
}

/// Each queued CPU or GPU sample is consumed once; pass and wall counts can differ.
#[derive(Default)]
struct PassMeasurements {
    samples_seen: u64,
    milliseconds: Vec<f64>,
}

impl PassMeasurements {
    fn include_value(&mut self, value: f64) {
        self.samples_seen += 1;
        if self.samples_seen > WARMUP_FRAMES && value.is_finite() && value > 0.0 {
            self.milliseconds.push(value);
        }
    }

    fn report(&self) -> PassReport {
        let mut sorted = self.milliseconds.clone();
        sorted.sort_by(f64::total_cmp);
        PassReport {
            available: !sorted.is_empty(),
            samples_seen: self.samples_seen,
            warmup_samples: WARMUP_FRAMES.min(self.samples_seen),
            measured_samples: sorted.len(),
            p50_ms: percentile(&sorted, 50),
            p95_ms: percentile(&sorted, 95),
            p99_ms: percentile(&sorted, 99),
            max_ms: sorted.last().copied(),
        }
    }
}

#[derive(Serialize)]
struct PassReport {
    available: bool,
    samples_seen: u64,
    warmup_samples: u64,
    measured_samples: usize,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
    max_ms: Option<f64>,
}

#[derive(Default, Serialize)]
struct SceneSize {
    live_triangle_count: u64,
    live_blas_node_count: u64,
    triangle_arena_count: u64,
    blas_node_arena_count: u64,
    surface_record_count: u64,
    surface_arena_count: u64,
    instance_count: u64,
    tlas_node_count: u64,
    geometry_arena_bytes: u64,
    instance_and_tlas_data_bytes: u64,
}

impl SceneSize {
    fn include(&mut self, scene: &RayScene) {
        self.live_triangle_count = self.live_triangle_count.max(
            scene
                .geometry
                .iter()
                .map(|chunk| chunk.triangles.len() as u64)
                .sum(),
        );
        self.live_blas_node_count = self.live_blas_node_count.max(
            scene
                .geometry
                .iter()
                .map(|chunk| chunk.nodes.len() as u64)
                .sum(),
        );
        self.triangle_arena_count = self
            .triangle_arena_count
            .max(u64::from(scene.triangle_count));
        self.blas_node_arena_count = self.blas_node_arena_count.max(u64::from(scene.node_count));
        self.surface_record_count = self.surface_record_count.max(
            scene
                .geometry
                .iter()
                .map(|chunk| chunk.surfaces.len() as u64)
                .sum(),
        );
        self.surface_arena_count = self.surface_arena_count.max(u64::from(scene.surface_count));
        self.instance_count = self.instance_count.max(scene.instances.len() as u64);
        self.tlas_node_count = self.tlas_node_count.max(scene.tlas_nodes.len() as u64);
        self.geometry_arena_bytes = self.geometry_arena_bytes.max(
            u64::from(scene.triangle_count) * GeometryTriangle::min_size().get()
                + u64::from(scene.surface_count) * TriangleSurface::min_size().get()
                + u64::from(scene.node_count) * BvhNode::min_size().get(),
        );
        self.instance_and_tlas_data_bytes = self.instance_and_tlas_data_bytes.max(
            scene.instances.len() as u64 * GpuInstance::min_size().get()
                + scene.tlas_nodes.len() as u64 * BvhNode::min_size().get(),
        );
    }
}

#[derive(Serialize)]
struct FrameReport<'a> {
    schema_version: u32,
    measurement: &'static str,
    presentation_uncapped: bool,
    render_probe: crate::args::RenderProbe,
    render_probe_measurement: &'static str,
    ray_pass_measurement: &'static str,
    ray_passes: BTreeMap<String, PassReport>,
    compute_gpu: PassReport,
    gpu_passes: BTreeMap<&'static str, PassReport>,
    cpu_phase_measurement: &'static str,
    cpu_phases: BTreeMap<&'static str, PassReport>,
    gpu_geometry_allocated_bytes: u64,
    gpu_framebuffer_nominal_bytes: u64,
    shadow_maps: Option<ShadowMapReport>,
    viewport_pixels: Option<[u32; 2]>,
    percentile_method: &'static str,
    warmup_frames: u64,
    total_frames: u64,
    measured_frames: usize,
    measured_wall_elapsed_ms: f64,
    wall_frame_p50_ms: Option<f64>,
    wall_frame_p95_ms: Option<f64>,
    wall_frame_p99_ms: Option<f64>,
    wall_frame_max_ms: Option<f64>,
    wall_frames_over_33_ms: usize,
    wall_frame_ms: &'a [f64],
    renderer_ready_ms: Option<f64>,
    peak_scene: &'a SceneSize,
    adapter: Option<String>,
    backend: Option<String>,
}

fn record_and_finish(
    time: Res<Time<Real>>,
    ready: Res<crate::raytrace::RayReady>,
    scene: Res<RayScene>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut measurements: ResMut<FrameMeasurements>,
    mut exits: MessageReader<AppExit>,
) {
    if measurements.written || !ready.get() {
        return;
    }
    if let Some(started) = measurements.startup_started.take() {
        measurements.renderer_ready_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
    }
    measurements.frames_seen += 1;
    measurements.peak_scene.include(&scene);
    let milliseconds = time.delta_secs_f64() * 1000.0;
    if measurements.frames_seen > WARMUP_FRAMES && milliseconds.is_finite() && milliseconds > 0.0 {
        measurements.wall_frame_ms.push(milliseconds);
    }
    if exits.read().next().is_none() {
        return;
    }
    measurements.written = true;
    let mut sorted = measurements.wall_frame_ms.clone();
    sorted.sort_by(f64::total_cmp);
    let report = FrameReport {
        schema_version: 3,
        measurement: "CPU wall-frame intervals, including GPU backpressure, pacing and capture overhead; not GPU pass timings",
        presentation_uncapped: measurements.uncapped,
        render_probe: measurements.probe,
        render_probe_measurement: "Startup-specialized diagnostic shader; ablations change pictures. Timing differences are non-additive because compiler optimization, control flow and GPU occupancy also change.",
        compute_gpu: measurements.compute_gpu.report(),
        gpu_passes: measurements
            .gpu_passes
            .iter()
            .map(|(&name, samples)| (name, samples.report()))
            .collect(),
        cpu_phase_measurement: "Per-invocation CPU elapsed time including allocations and early returns; first 30 invocations per phase excluded. Phases may run concurrently, so their durations should not be summed as frame time.",
        cpu_phases: measurements
            .cpu_phases
            .iter()
            .map(|(&name, samples)| (name, samples.report()))
            .collect(),
        gpu_geometry_allocated_bytes: measurements.gpu_geometry_allocated_bytes,
        gpu_framebuffer_nominal_bytes: measurements.gpu_framebuffer_nominal_bytes,
        shadow_maps: measurements.shadow_maps.clone(),
        viewport_pixels: measurements.viewport_pixels,
        ray_pass_measurement: "Legacy Bevy ray_passes diagnostics are disabled and unavailable: their unbounded frame query pool is redundant with our bounded custom collection. ray_passes remains empty for JSON compatibility. compute_gpu aliases gpu_passes.compute. Custom named GPU passes use separate query sets and resolve after submission completion; first 30 samples per pass are excluded and invalid/zero timestamps are discarded. Pass durations can overlap and percentiles must not be summed. Presentation and readbacks still in flight at exit are excluded.",
        ray_passes: BTreeMap::new(),
        percentile_method: "nearest rank",
        warmup_frames: WARMUP_FRAMES.min(measurements.frames_seen),
        total_frames: measurements.frames_seen,
        measured_frames: sorted.len(),
        measured_wall_elapsed_ms: sorted.iter().sum(),
        wall_frame_p50_ms: percentile(&sorted, 50),
        wall_frame_p95_ms: percentile(&sorted, 95),
        wall_frame_p99_ms: percentile(&sorted, 99),
        wall_frame_max_ms: sorted.last().copied(),
        wall_frames_over_33_ms: sorted.iter().filter(|&&ms| ms > 33.0).count(),
        wall_frame_ms: &measurements.wall_frame_ms,
        renderer_ready_ms: measurements.renderer_ready_ms,
        peak_scene: &measurements.peak_scene,
        adapter: adapter.as_ref().map(|info| info.name.clone()),
        backend: adapter.as_ref().map(|info| format!("{:?}", info.backend)),
    };
    let write_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = measurements
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(&measurements.path, serde_json::to_vec_pretty(&report)?)?;
        Ok(())
    })();
    if let Err(error) = write_result {
        error!(path = %measurements.path.display(), %error, "Could not write renderer acceptance report");
    }
}

fn percentile(sorted: &[f64], percentage: usize) -> Option<f64> {
    let rank = (sorted.len() * percentage).div_ceil(100);
    sorted.get(rank.saturating_sub(1)).copied()
}

#[cfg(test)]
mod tests {
    use super::{PassMeasurements, WARMUP_FRAMES, percentile};

    #[test]
    fn cpu_phase_guard_records_early_returns_once_without_a_gpu() {
        fn measured(timing: &super::ComputeGpuTiming, early: bool) {
            let _span = timing.cpu_span("test_phase");
            if early {
                return;
            }
            std::hint::black_box(1 + 2);
        }
        let timing = super::ComputeGpuTiming::default();
        measured(&timing, true);
        measured(&timing, false);
        let samples = timing.cpu_completed.lock().unwrap();
        assert_eq!(samples.len(), 2);
        assert!(
            samples
                .iter()
                .all(|sample| sample.name == "test_phase" && sample.milliseconds >= 0.0)
        );
    }

    #[test]
    fn reporting_uses_bounded_custom_collection_without_bevy_query_pool() {
        let mut app = bevy::prelude::App::new();
        app.add_plugins(super::RayStatsPlugin(
            Some("unused-report.json".into()),
            false,
        ));
        assert!(app.world().contains_resource::<super::ComputeGpuTiming>());
        assert!(!app.is_plugin_added::<bevy::render::diagnostic::RenderDiagnosticsPlugin>());
    }

    #[test]
    fn independent_pass_samples_apply_warmup_and_reject_invalid_values() {
        let mut samples = PassMeasurements::default();
        for frame in 0..WARMUP_FRAMES + 2 {
            samples.include_value(frame as f64);
        }
        assert_eq!(samples.samples_seen, 32);
        assert_eq!(samples.milliseconds, [30.0, 31.0]);
        assert_eq!(samples.report().p95_ms, Some(31.0));
        samples.include_value(f64::NAN);
        assert_eq!(samples.report().measured_samples, 2);
    }

    #[test]
    fn suppressed_or_invalid_gpu_timestamps_are_not_reported_as_fast_frames() {
        let mut samples = PassMeasurements::default();
        for _ in 0..WARMUP_FRAMES + 10 {
            samples.include_value(0.0);
        }
        samples.include_value(f64::NAN);
        samples.include_value(-1.0);
        let report = samples.report();
        assert!(!report.available);
        assert_eq!(report.p50_ms, None);
        samples.include_value(8.5);
        assert!(samples.report().available);
        assert_eq!(samples.report().p50_ms, Some(8.5));
    }

    #[test]
    fn nearest_rank_preserves_tail_outliers_and_handles_short_runs() {
        assert_eq!(percentile(&[], 95), None);
        assert_eq!(percentile(&[12.0], 99), Some(12.0));
        let mut samples = vec![16.0; 99];
        samples.push(90.0);
        assert_eq!(percentile(&samples, 50), Some(16.0));
        assert_eq!(percentile(&samples, 99), Some(16.0));
        assert_eq!(percentile(&samples, 100), Some(90.0));
    }
}
