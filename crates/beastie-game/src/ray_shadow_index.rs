//! Retained shadow-only LOD, prepared off-thread with exact geometry while pending.
use crate::{ray_scene::RayScene, ray_static_maps::encoded};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use wgpu::util::DeviceExt;

type MeshKey = (usize, u32);
fn mesh_key(chunk: &crate::ray_scene::GeometryChunk, tolerance: f32) -> MeshKey {
    (Arc::as_ptr(&chunk.triangles) as usize, tolerance.to_bits())
}
struct CachedShadowMesh {
    // Own the immutable source so allocator pointer reuse cannot alias a key.
    source: Arc<Vec<crate::ray_scene::Triangle>>,
    positions: Vec<Vec4>,
    original: Vec<u32>,
    simplified: Vec<u32>,
}
impl CachedShadowMesh {
    fn bytes(&self) -> usize {
        self.positions.capacity() * 16
            + (self.original.capacity() + self.simplified.capacity()) * 4
            + self.source.capacity() * std::mem::size_of::<crate::ray_scene::Triangle>()
    }
}
#[derive(Default)]
pub struct ShadowMeshCache {
    entries: HashMap<MeshKey, (u64, Arc<CachedShadowMesh>)>,
    clock: u64,
    #[cfg(test)]
    builds: usize,
}
impl ShadowMeshCache {
    fn get(
        &mut self,
        chunk: &crate::ray_scene::GeometryChunk,
        tolerance: f32,
    ) -> Arc<CachedShadowMesh> {
        self.clock += 1;
        let key = mesh_key(chunk, tolerance);
        if let Some((used, mesh)) = self.entries.get_mut(&key) {
            *used = self.clock;
            return mesh.clone();
        }
        let mut positions = Vec::new();
        let mut original = Vec::new();
        append_chunk(&chunk.triangles, &mut positions, &mut original);
        let (simplified, _) = simplify_positions(&positions, &original, tolerance);
        let mesh = Arc::new(CachedShadowMesh {
            source: chunk.triangles.clone(),
            positions,
            original,
            simplified,
        });
        self.entries.insert(key, (self.clock, mesh.clone()));
        #[cfg(test)]
        {
            self.builds += 1;
        }
        mesh
    }
    fn prune_sources(&mut self, chunks: &[crate::ray_scene::GeometryChunk]) {
        let sources: HashSet<_> = chunks
            .iter()
            .map(|c| Arc::as_ptr(&c.triangles) as usize)
            .collect();
        let active = self
            .entries
            .keys()
            .filter(|key| sources.contains(&key.0))
            .copied()
            .collect();
        self.prune(&active);
    }
    fn prune(&mut self, active: &HashSet<MeshKey>) {
        const RETAIN_BYTES: usize = 32 * 1024 * 1024;
        const RETAIN_MESHES: usize = 128;
        let mut bytes: usize = self.entries.values().map(|(_, mesh)| mesh.bytes()).sum();
        let mut inactive: Vec<_> = self
            .entries
            .iter()
            .filter(|(key, _)| !active.contains(key))
            .map(|(key, (used, _))| (*used, *key))
            .collect();
        inactive.sort_unstable();
        for (_, key) in inactive {
            if bytes <= RETAIN_BYTES && self.entries.len() <= RETAIN_MESHES {
                break;
            }
            if let Some((_, mesh)) = self.entries.remove(&key) {
                bytes -= mesh.bytes();
            }
        }
    }
}

pub struct ShadowIndex {
    pub positions: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub ranges: HashMap<u32, std::ops::Range<u32>>,
    pub(crate) original_ranges: HashMap<u32, std::ops::Range<u32>>,
    #[cfg(test)]
    pub lod_world_error: f32,
    #[cfg(test)]
    pub original_triangles: usize,
    #[cfg(test)]
    pub vertices: usize,
    #[cfg(test)]
    pub triangles: usize,
    pub bytes: u64,
}
impl ShadowIndex {
    /// Conservative world-error scaling fallback, checked at draw time so no
    /// animated transform can reuse an under-budgeted LOD or force a rebuild.
    pub fn range_for(&self, instance: &crate::ray_scene::GpuInstance) -> std::ops::Range<u32> {
        if within_lod_scale(instance.world_from_local) {
            self.ranges[&instance.root].clone()
        } else {
            self.original_ranges[&instance.root].clone()
        }
    }

    /// Transform and TLAS ordering changes do not change object-local positions.
    /// Mesh membership and revisions do, so retain only those inputs in the key.
    pub fn key(scene: &RayScene) -> Vec<(u32, u64)> {
        let roots: HashSet<_> = crate::ray_shadow_maps::dynamic_casters(scene)
            .into_iter()
            .map(|i| scene.instances[i].root)
            .collect();
        let mut key: Vec<_> = scene
            .geometry
            .iter()
            .filter(|c| roots.contains(&c.node_offset))
            .map(|c| (c.node_offset, c.revision))
            .collect();
        key.sort_unstable();
        key
    }

    #[cfg(test)]
    pub fn new(device: &wgpu::Device, scene: &RayScene) -> Self {
        Self::new_with_cache(device, scene, &mut ShadowMeshCache::default())
    }
    #[cfg(test)]
    pub fn new_with_cache(
        device: &wgpu::Device,
        scene: &RayScene,
        cache: &mut ShadowMeshCache,
    ) -> Self {
        let chunks = selected_chunks(scene);
        let prepared = PreparedIndex::new(&chunks, cache);
        cache.prune_sources(&chunks);
        prepared.upload(device)
    }
}

fn selected_chunks(scene: &RayScene) -> Vec<crate::ray_scene::GeometryChunk> {
    let roots: HashSet<_> = crate::ray_shadow_maps::dynamic_casters(scene)
        .into_iter()
        .map(|i| scene.instances[i].root)
        .collect();
    scene
        .geometry
        .iter()
        .filter(|c| roots.contains(&c.node_offset))
        .cloned()
        .collect()
}

pub struct PreparedIndex {
    positions: Vec<u8>,
    indices: Vec<u8>,
    ranges: HashMap<u32, std::ops::Range<u32>>,
    original_ranges: HashMap<u32, std::ops::Range<u32>>,
    #[cfg(test)]
    lod_world_error: f32,
    #[cfg(test)]
    original_triangles: usize,
    #[cfg(test)]
    vertices: usize,
    #[cfg(test)]
    triangles: usize,
}
impl PreparedIndex {
    fn new(chunks: &[crate::ray_scene::GeometryChunk], cache: &mut ShadowMeshCache) -> Self {
        let mut positions = Vec::<Vec4>::new();
        let mut indices = Vec::<u32>::new();
        let mut ranges = HashMap::new();
        let mut original_ranges = HashMap::new();
        // Absolute simplifier metric budget in world units. The transform guard
        // uses a conservative Frobenius bound; visual/motion oracles validate the
        // metric approximation, which is not a Hausdorff error certificate.
        let tolerance = 0.002;
        let mut selected_triangles = 0;
        let mut original_triangles = 0;
        for chunk in chunks {
            let first = indices.len() as u32;
            let vertex_base = positions.len() as u32;
            let mesh = cache.get(chunk, tolerance);
            positions.extend_from_slice(&mesh.positions);
            indices.extend(mesh.original.iter().map(|i| i + vertex_base));
            let original = first..indices.len() as u32;
            original_triangles += mesh.original.len() / 3;
            original_ranges.insert(chunk.node_offset, original.clone());
            let simplified = &mesh.simplified;
            if simplified.len() < mesh.original.len() {
                let start = indices.len() as u32;
                indices.extend(simplified.iter().map(|i| i + vertex_base));
                ranges.insert(chunk.node_offset, start..indices.len() as u32);
            } else {
                ranges.insert(chunk.node_offset, original);
            }
            selected_triangles += simplified.len() / 3;
        }
        #[cfg(test)]
        let vertex_count = positions.len();
        #[cfg(test)]
        let triangle_count = selected_triangles;
        #[cfg(not(test))]
        let _ = (selected_triangles, original_triangles);
        // Bindings remain legal during an empty-dynamic-scene transition.
        if positions.is_empty() {
            positions.push(Vec4::ZERO);
        }
        if indices.is_empty() {
            indices.push(0);
        }
        Self {
            positions: encoded(&positions),
            indices: encoded(&indices),
            ranges,
            original_ranges,
            #[cfg(test)]
            lod_world_error: tolerance,
            #[cfg(test)]
            original_triangles,
            #[cfg(test)]
            vertices: vertex_count,
            #[cfg(test)]
            triangles: triangle_count,
        }
    }
    pub fn upload(self, device: &wgpu::Device) -> ShadowIndex {
        let position_bytes = self.positions;
        let index_bytes = self.indices;
        let bytes = (position_bytes.len() + index_bytes.len()) as u64;
        let positions = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indexed shadow positions"),
            contents: &position_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shadow indices"),
            contents: &index_bytes,
            usage: wgpu::BufferUsages::INDEX,
        });
        ShadowIndex {
            positions,
            indices,
            ranges: self.ranges,
            original_ranges: self.original_ranges,
            #[cfg(test)]
            lod_world_error: self.lod_world_error,
            #[cfg(test)]
            original_triangles: self.original_triangles,
            #[cfg(test)]
            vertices: self.vertices,
            #[cfg(test)]
            triangles: self.triangles,
            bytes,
        }
    }
}

type SceneKey = Vec<(u32, u64)>;
type JobResult = (SceneKey, PreparedIndex, ShadowMeshCache);
#[derive(Default)]
pub struct ShadowPreparation {
    cache: ShadowMeshCache,
    job: Option<Task<JobResult>>,
    #[cfg(test)]
    launched: usize,
}
impl ShadowPreparation {
    /// Poll even when maps are disabled. Never wait on a running worker.
    pub fn poll(&mut self, current: Option<&SceneKey>, scene: &RayScene) -> Option<PreparedIndex> {
        let (key, prepared, cache) = block_on(poll_once(self.job.as_mut()?))?;
        self.job = None;
        self.cache = cache;
        self.cache.prune_sources(&if current.is_some() {
            selected_chunks(scene)
        } else {
            Vec::new()
        });
        (current == Some(&key)).then_some(prepared)
    }
    pub fn request(&mut self, scene: &RayScene, key: SceneKey) {
        if self.job.is_some() {
            return;
        }
        let chunks = selected_chunks(scene);
        let mut cache = std::mem::take(&mut self.cache);
        self.job = Some(AsyncComputeTaskPool::get().spawn(async move {
            let prepared = PreparedIndex::new(&chunks, &mut cache);
            (key, prepared, cache)
        }));
        #[cfg(test)]
        {
            self.launched += 1;
        }
    }
}

/// Weld only bit-identical positions after the same a+edge reconstruction as WGSL.
/// Starting a fresh map per chunk keeps independently versioned meshes separable.
fn append_chunk(
    triangles: &[crate::ray_scene::Triangle],
    positions: &mut Vec<Vec4>,
    indices: &mut Vec<u32>,
) {
    let mut unique = HashMap::<[u32; 3], u32>::new();
    for triangle in triangles {
        let t = triangle.geometry(0);
        for p in [t.a, t.a + t.e1.truncate(), t.a + t.e2.truncate()] {
            let index = *unique
                .entry(p.to_array().map(f32::to_bits))
                .or_insert_with(|| {
                    let index = positions.len() as u32;
                    positions.push(p.extend(1.0));
                    index
                });
            indices.push(index);
        }
    }
}

// Frobenius norm bounds the largest singular value, including shear/nonuniform
// scale. A fixed cap avoids transform-driven LOD regeneration. Larger or invalid
// transforms draw the retained original range instead.
const LOD_SCALE_CAP: f32 = 4.0;
fn within_lod_scale(matrix: Mat4) -> bool {
    let squared = matrix.x_axis.truncate().length_squared()
        + matrix.y_axis.truncate().length_squared()
        + matrix.z_axis.truncate().length_squared();
    squared.is_finite() && squared <= LOD_SCALE_CAP * LOD_SCALE_CAP
}
fn simplify_positions(positions: &[Vec4], indices: &[u32], world_error: f32) -> (Vec<u32>, f32) {
    if world_error == 0.0 || indices.is_empty() {
        return (indices.to_vec(), 0.0);
    }
    let bytes = encoded(&positions.to_vec());
    let vertices = meshopt::VertexDataAdapter::new(&bytes, 16, 0).expect("vec4 positions");
    let mut error = 0.0;
    let simplified = meshopt::simplify(
        indices,
        &vertices,
        0,
        world_error / LOD_SCALE_CAP,
        meshopt::SimplifyOptions::LockBorder | meshopt::SimplifyOptions::ErrorAbsolute,
        Some(&mut error),
    );
    assert!(error.is_finite() && error * LOD_SCALE_CAP <= world_error * 1.00001);
    assert!(
        simplified.len().is_multiple_of(3)
            && simplified.iter().all(|i| (*i as usize) < positions.len())
    );
    if simplified.is_empty() {
        return (indices.to_vec(), 0.0);
    }
    (simplified, error * LOD_SCALE_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ray_scene::Triangle;

    #[test]
    fn indexed_geometry_key_reuses_transforms_but_invalidates_mesh_edits_and_membership() {
        let (mut scene, _) = crate::ray_benchmark::scene_snapshot();
        let key = ShadowIndex::key(&scene);
        assert!(!key.is_empty());
        scene.instances.reverse();
        for instance in &mut scene.instances {
            instance.world_from_local =
                Mat4::from_translation(Vec3::ONE) * instance.world_from_local;
        }
        assert_eq!(key, ShadowIndex::key(&scene));
        let root = key[0].0;
        scene
            .geometry
            .iter_mut()
            .find(|c| c.node_offset == root)
            .unwrap()
            .revision += 1;
        assert_ne!(key, ShadowIndex::key(&scene));
        scene
            .geometry
            .iter_mut()
            .find(|c| c.node_offset == root)
            .unwrap()
            .revision -= 1;
        scene.instances.retain(|i| i.root != root);
        assert_ne!(key, ShadowIndex::key(&scene));
    }

    #[test]
    fn indexed_shadow_stream_preserves_triangle_order_and_reconstructed_bits() {
        let triangle = |a: Vec3, b: Vec3, c: Vec3| Triangle {
            a: a.extend(1.0),
            b: b.extend(1.0),
            c: c.extend(1.0),
            n0: Vec4::ZERO,
            n1: Vec4::ZERO,
            n2: Vec4::ZERO,
            c0: Vec4::ZERO,
            c1: Vec4::ZERO,
            c2: Vec4::ZERO,
        };
        let triangles = [
            triangle(Vec3::ZERO, Vec3::X, Vec3::Y),
            triangle(Vec3::Y, Vec3::X, Vec3::ONE),
            triangle(Vec3::splat(100.0), Vec3::splat(0.00003), Vec3::Z),
        ];
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        append_chunk(&triangles, &mut positions, &mut indices);
        assert!(positions.len() < indices.len());
        for (t, indexed) in triangles.iter().zip(indices.as_chunks::<3>().0.iter()) {
            let g = t.geometry(0);
            let expected = [g.a, g.a + g.e1.truncate(), g.a + g.e2.truncate()];
            for (p, i) in expected.into_iter().zip(indexed) {
                assert_eq!(
                    p.to_array().map(f32::to_bits),
                    positions[*i as usize]
                        .truncate()
                        .to_array()
                        .map(f32::to_bits)
                );
            }
        }
        let first_vertices = positions.len();
        append_chunk(&triangles[..1], &mut positions, &mut indices);
        assert!(
            indices[indices.len() - 3..]
                .iter()
                .all(|i| *i as usize >= first_vertices)
        );
    }
}

#[test]
fn lod_scale_guard_includes_shear_and_nonuniform_transforms() {
    assert!(within_lod_scale(Mat4::IDENTITY));
    assert!(within_lod_scale(Mat4::from_rotation_y(1.4)));
    assert!(!within_lod_scale(Mat4::from_scale(Vec3::new(
        4.0, 1.0, 1.0
    ))));
    let shear = Mat4::from_cols(Vec4::X, Vec4::new(4.0, 1.0, 0.0, 0.0), Vec4::Z, Vec4::W);
    assert!(!within_lod_scale(shear));
}
#[test]
fn retained_shadow_meshes_survive_relocation_and_invalidate_geometry_edits() {
    let (scene, _) = crate::ray_benchmark::scene_snapshot();
    let chunk = scene
        .geometry
        .iter()
        .filter(|c| !c.triangles.is_empty())
        .min_by_key(|c| c.triangles.len())
        .unwrap();
    let mut cache = ShadowMeshCache::default();
    let first = cache.get(chunk, 0.002);
    cache.prune(&HashSet::new());
    let returned = cache.get(chunk, 0.002);
    assert!(Arc::ptr_eq(&first, &returned));
    assert_eq!(cache.builds, 1);
    assert!(
        first.bytes()
            >= first.source.capacity() * std::mem::size_of::<crate::ray_scene::Triangle>()
    );
    let mut relocated = chunk.clone();
    relocated.node_offset += 100000;
    relocated.revision += 100000;
    assert!(Arc::ptr_eq(&first, &cache.get(&relocated, 0.002)));
    assert_eq!(
        cache.builds, 1,
        "arena relocation must not repeat simplification"
    );
    let mut revised = chunk.clone();
    revised.revision += 100000;
    let mut triangles = (*revised.triangles).clone();
    triangles[0].a.x += 0.001;
    revised.triangles = Arc::new(triangles);
    let changed = cache.get(&revised, 0.002);
    assert_eq!(cache.builds, 2);
    assert_ne!(first.positions, changed.positions);
    for index in 0..140 {
        let mut alias = chunk.clone();
        alias.node_offset += index + 100000;
        alias.triangles = Arc::new((*chunk.triangles).clone());
        cache.get(&alias, 0.0);
    }
    let active = HashSet::from([mesh_key(chunk, 0.002)]);
    cache.prune(&active);
    assert!(cache.entries.len() <= 128);
    assert!(cache.entries.contains_key(active.iter().next().unwrap()));
    assert!(Arc::ptr_eq(&first, &cache.get(chunk, 0.002)));
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    fn finished_job(key: SceneKey, builds: usize) -> ShadowPreparation {
        let job = AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new).spawn(async move {
            let mut cache = ShadowMeshCache::default();
            let prepared = PreparedIndex::new(&[], &mut cache);
            cache.builds = builds;
            (key, prepared, cache)
        });
        while !job.is_finished() {
            std::thread::yield_now();
        }
        ShadowPreparation {
            cache: ShadowMeshCache::default(),
            job: Some(job),
            launched: 1,
        }
    }
    #[test]
    fn stale_and_disabled_results_never_upload_but_keep_cpu_cache() {
        let a = vec![(1, 1)];
        let b = vec![(2, 2)];
        let mut preparation = finished_job(a.clone(), 7);
        assert!(preparation.poll(Some(&b), &RayScene::default()).is_none());
        assert_eq!(preparation.cache.builds, 7);
        assert!(preparation.job.is_none());
        let mut preparation = finished_job(a.clone(), 9);
        assert!(preparation.poll(None, &RayScene::default()).is_none());
        assert_eq!(preparation.cache.builds, 9);
        let mut preparation = finished_job(a.clone(), 11);
        assert!(preparation.poll(Some(&a), &RayScene::default()).is_some());
        assert_eq!(preparation.cache.builds, 11);
    }
    #[test]
    fn only_one_job_runs_while_scene_membership_changes() {
        let (release, wait) = std::sync::mpsc::channel();
        let job = Some(
            AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new).spawn(async move {
                wait.recv().unwrap();
                let mut cache = ShadowMeshCache::default();
                (vec![(1, 1)], PreparedIndex::new(&[], &mut cache), cache)
            }),
        );
        let mut preparation = ShadowPreparation {
            job,
            launched: 1,
            ..Default::default()
        };
        preparation.request(&RayScene::default(), vec![(2, 2)]);
        assert_eq!(preparation.launched, 1);
        assert!(
            preparation
                .poll(Some(&vec![(2, 2)]), &RayScene::default())
                .is_none()
        );
        release.send(()).unwrap();
        while !preparation.job.as_ref().unwrap().is_finished() {
            std::thread::yield_now();
        }
        assert!(
            preparation
                .poll(Some(&vec![(2, 2)]), &RayScene::default())
                .is_none()
        );
        preparation.request(&RayScene::default(), vec![]);
        while !preparation.job.as_ref().unwrap().is_finished() {
            std::thread::yield_now();
        }
        assert!(
            preparation
                .poll(Some(&vec![]), &RayScene::default())
                .is_some()
        );
    }
}
