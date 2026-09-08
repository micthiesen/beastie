//! Opt-in native acceptance measurements. Wall-frame intervals include CPU work,
//! GPU backpressure, presentation pacing and captures; they are not GPU timings.
use std::{fs, path::PathBuf};

use bevy::{
    app::AppExit,
    prelude::*,
    render::{render_resource::ShaderType, renderer::RenderAdapterInfo},
};
use serde::Serialize;

use crate::ray_scene::{BvhNode, GpuInstance, RayScene, Triangle};

const WARMUP_FRAMES: u64 = 30;

pub struct RayStatsPlugin(pub Option<PathBuf>);

impl Plugin for RayStatsPlugin {
    fn build(&self, app: &mut App) {
        if let Some(path) = &self.0 {
            app.insert_resource(FrameMeasurements {
                path: path.clone(),
                ..default()
            })
            .add_systems(Last, record_and_finish);
        }
    }
}

#[derive(Resource, Default)]
struct FrameMeasurements {
    path: PathBuf,
    frames_seen: u64,
    wall_frame_ms: Vec<f64>,
    peak_scene: SceneSize,
    written: bool,
}

#[derive(Default, Serialize)]
struct SceneSize {
    triangle_arena_count: u64,
    blas_node_arena_count: u64,
    instance_count: u64,
    tlas_node_count: u64,
    geometry_arena_bytes: u64,
    instance_and_tlas_data_bytes: u64,
}

impl SceneSize {
    fn include(&mut self, scene: &RayScene) {
        self.triangle_arena_count = self
            .triangle_arena_count
            .max(u64::from(scene.triangle_count));
        self.blas_node_arena_count = self.blas_node_arena_count.max(u64::from(scene.node_count));
        self.instance_count = self.instance_count.max(scene.instances.len() as u64);
        self.tlas_node_count = self.tlas_node_count.max(scene.tlas_nodes.len() as u64);
        self.geometry_arena_bytes = self.geometry_arena_bytes.max(
            u64::from(scene.triangle_count) * Triangle::min_size().get()
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
    percentile_method: &'static str,
    warmup_frames: u64,
    total_frames: u64,
    measured_frames: usize,
    measured_wall_elapsed_ms: f64,
    wall_frame_p50_ms: Option<f64>,
    wall_frame_p95_ms: Option<f64>,
    wall_frame_p99_ms: Option<f64>,
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
        schema_version: 1,
        measurement: "CPU wall-frame intervals, including GPU backpressure, pacing and capture overhead; not GPU pass timings",
        percentile_method: "nearest rank",
        warmup_frames: WARMUP_FRAMES.min(measurements.frames_seen),
        total_frames: measurements.frames_seen,
        measured_frames: sorted.len(),
        measured_wall_elapsed_ms: sorted.iter().sum(),
        wall_frame_p50_ms: percentile(&sorted, 50),
        wall_frame_p95_ms: percentile(&sorted, 95),
        wall_frame_p99_ms: percentile(&sorted, 99),
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
    use super::percentile;

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
