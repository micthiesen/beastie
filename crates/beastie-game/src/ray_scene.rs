//! CPU acceleration structures shared by the software GPU ray tracer.
//!
//! Mesh triangles stay in local space. Only the small instance hierarchy is rebuilt
//! when objects move; mesh acceleration structures are cached until assets change.
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy::{
    camera::visibility::VisibilitySystems,
    mesh::VertexAttributeValues,
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_resource::{PrimitiveTopology, ShaderType},
    },
    transform::TransformSystems,
};

#[derive(Component)]
pub struct RayOverlay;

/// Explicit immutable environment eligibility. All other entities remain dynamic.
#[derive(Component)]
pub struct RayStatic;

#[derive(Clone, PartialEq)]
struct StaticSignature {
    entity: u64,
    index: u32,
    packed_triangles: u32,
    mesh: AssetId<Mesh>,
    transform: [u32; 16],
    material: [u32; 8],
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct Triangle {
    pub a: Vec4,
    pub b: Vec4,
    pub c: Vec4,
    pub n0: Vec4,
    pub n1: Vec4,
    pub n2: Vec4,
    pub c0: Vec4,
    pub c1: Vec4,
    pub c2: Vec4,
}

/// Intersection rays read only positions and precomputed edges. Surface normals
/// and colors remain lossless in a separate array with the same triangle index.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct GeometryTriangle {
    pub a: Vec3,
    pub surface: u32,
    pub e1: Vec4,
    pub e2: Vec4,
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct TriangleSurface {
    pub n0: Vec4,
    pub n1: Vec4,
    pub n2: Vec4,
    pub c0: Vec4,
    pub c1: Vec4,
    pub c2: Vec4,
}

impl Triangle {
    pub fn geometry(&self, surface: u32) -> GeometryTriangle {
        GeometryTriangle {
            a: self.a.truncate(),
            surface,
            e1: (self.b.truncate() - self.a.truncate()).extend(0.0),
            e2: (self.c.truncate() - self.a.truncate()).extend(0.0),
        }
    }

    pub fn surface(&self) -> TriangleSurface {
        TriangleSurface {
            n0: self.n0,
            n1: self.n1,
            n2: self.n2,
            c0: self.c0,
            c1: self.c1,
            c2: self.c2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct BvhNode {
    pub min: Vec3,
    pub first: u32,
    pub max: Vec3,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct GpuInstance {
    pub world_from_local: Mat4,
    pub local_from_world: Mat4,
    /// Roughness, metallic, unlit, overlay. Overlay geometry is never a shadow caster.
    pub material: Vec4,
    pub tint: Vec4,
    pub root: u32,
    pub transmission: f32,
    pub pad1: u32,
    pub pad2: u32,
}

/// Stable arena allocation. Upload a chunk only when its revision changes.
/// A buffer allocation or compaction requires uploading all current chunks.
#[derive(Clone)]
pub struct GeometryChunk {
    pub triangle_offset: u32,
    pub node_offset: u32,
    pub surface_offset: u32,
    pub surfaces: Arc<Vec<TriangleSurface>>,
    pub surface_indices: Arc<Vec<u32>>,
    pub revision: u64,
    pub triangles: Arc<Vec<Triangle>>,
    pub nodes: Arc<Vec<BvhNode>>,
}

#[derive(Resource, Clone, ExtractResource)]
pub struct RayScene {
    pub geometry: Vec<GeometryChunk>,
    pub triangle_count: u32,
    pub node_count: u32,
    pub surface_count: u32,
    pub instances: Vec<GpuInstance>,
    pub tlas_nodes: Vec<BvhNode>,
    /// Auxiliary roots share the primary instance array. MAX denotes no members.
    pub world_root: u32,
    pub shadow_root: u32,
    pub static_shadow_root: u32,
    pub dynamic_shadow_root: u32,
    pub static_revision: u64,
    pub static_world_root: u32,
    pub dynamic_world_root: u32,
    pub static_instances: [u32; 16],
    pub geometry_revision: u64,
}

impl Default for RayScene {
    fn default() -> Self {
        Self {
            geometry: Vec::new(),
            triangle_count: 0,
            node_count: 0,
            surface_count: 0,
            instances: Vec::new(),
            tlas_nodes: Vec::new(),
            world_root: u32::MAX,
            shadow_root: u32::MAX,
            static_shadow_root: u32::MAX,
            dynamic_shadow_root: u32::MAX,
            static_revision: 0,
            static_world_root: u32::MAX,
            dynamic_world_root: u32::MAX,
            static_instances: [u32::MAX; 16],
            geometry_revision: 0,
        }
    }
}

pub struct RayScenePlugin;

impl Plugin for RayScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RayScene>()
            .init_resource::<MeshCache>()
            .add_systems(
                PostUpdate,
                extract_scene
                    .after(bevy::asset::AssetEventSystems)
                    .after(TransformSystems::Propagate)
                    .after(VisibilitySystems::VisibilityPropagate),
            );
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

impl Bounds {
    fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    fn point(mut self, point: Vec3) -> Self {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
        self
    }

    fn union(self, other: Self) -> Self {
        self.point(other.min).point(other.max)
    }

    fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    fn transformed(self, matrix: Mat4) -> Self {
        let mut result = Self::empty();
        for x in [self.min.x, self.max.x] {
            for y in [self.min.y, self.max.y] {
                for z in [self.min.z, self.max.z] {
                    result = result.point(matrix.transform_point3(Vec3::new(x, y, z)));
                }
            }
        }
        result
    }
}

struct MeshBlas {
    triangles: Arc<Vec<Triangle>>,
    surfaces: Arc<Vec<TriangleSurface>>,
    surface_indices: Arc<Vec<u32>>,
    packed_surfaces: u32,
    surface_capacity: u32,
    nodes: Vec<BvhNode>,
    bounds: Bounds,
    packed_root: u32,
    packed_triangles: u32,
    triangle_capacity: u32,
    node_capacity: u32,
    chunk: Option<GeometryChunk>,
}

#[derive(Resource, Default)]
struct MeshCache {
    meshes: HashMap<AssetId<Mesh>, MeshBlas>,
    static_snapshot: Vec<StaticSignature>,
    triangle_count: u32,
    node_count: u32,
    surface_count: u32,
    revision: u64,
}

type SceneInstances<'w, 's> = Query<
    'w,
    's,
    (
        &'static Mesh3d,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static GlobalTransform,
        Option<&'static InheritedVisibility>,
        Option<&'static RayOverlay>,
        Option<&'static RayStatic>,
        Entity,
    ),
>;

fn extract_scene(
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    query: SceneInstances,
    mut cache: ResMut<MeshCache>,
    mut scene: ResMut<RayScene>,
    timing: Option<Res<crate::ray_stats::ComputeGpuTiming>>,
) {
    let _span = timing
        .as_ref()
        .map(|timing| timing.cpu_span("extract_scene"));
    let modified: HashSet<_> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } | AssetEvent::Removed { id } => Some(*id),
            _ => None,
        })
        .collect();
    let used: HashSet<_> = query.iter().map(|(mesh, ..)| mesh.id()).collect();
    let previous_count = cache.meshes.len();
    cache
        .meshes
        .retain(|id, _| used.contains(id) && meshes.contains(*id));
    let mut geometry_changed = cache.meshes.len() != previous_count;
    for id in used {
        if cache.meshes.contains_key(&id) && !modified.contains(&id) {
            continue;
        }
        let old = cache.meshes.remove(&id);
        geometry_changed |= old.is_some();
        let Some(mesh) = meshes.get(id) else {
            continue;
        };
        let Some(mut blas) = mesh_blas(mesh) else {
            continue;
        };
        if let Some(old) = old.filter(|old| {
            old.triangle_capacity >= blas.triangles.len() as u32
                && old.node_capacity >= blas.nodes.len() as u32
                && old.surface_capacity >= blas.surfaces.len() as u32
        }) {
            blas.packed_surfaces = old.packed_surfaces;
            blas.surface_capacity = old.surface_capacity;
            blas.packed_root = old.packed_root;
            blas.packed_triangles = old.packed_triangles;
            blas.triangle_capacity = old.triangle_capacity;
            blas.node_capacity = old.node_capacity;
        } else {
            blas.packed_surfaces = cache.surface_count;
            blas.surface_capacity = (blas.surfaces.len() as u32).next_power_of_two();
            cache.surface_count += blas.surface_capacity;
            blas.packed_root = cache.node_count;
            blas.packed_triangles = cache.triangle_count;
            blas.triangle_capacity = (blas.triangles.len() as u32).next_power_of_two();
            blas.node_capacity = (blas.nodes.len() as u32).next_power_of_two();
            cache.node_count += blas.node_capacity;
            cache.triangle_count += blas.triangle_capacity;
        }
        cache.revision = cache.revision.wrapping_add(1);
        pack_chunk(&mut blas, cache.revision);
        cache.meshes.insert(id, blas);
        geometry_changed = true;
    }
    if geometry_changed {
        let live_triangles: u32 = cache
            .meshes
            .values()
            .map(|mesh| mesh.triangle_capacity)
            .sum();
        let live_surfaces: u32 = cache
            .meshes
            .values()
            .map(|mesh| mesh.surface_capacity)
            .sum();
        let live_nodes: u32 = cache.meshes.values().map(|mesh| mesh.node_capacity).sum();
        // Reclaim removed/replaced allocations before dead space can grow without
        // bound. Most frames update only the small effects mesh in its old slot.
        if cache.triangle_count > live_triangles.saturating_mul(2).saturating_add(1024)
            || cache.node_count > live_nodes.saturating_mul(2).saturating_add(1024)
            || cache.surface_count > live_surfaces.saturating_mul(2).saturating_add(1024)
        {
            cache.revision = cache.revision.wrapping_add(1);
            let revision = cache.revision;
            let mut triangle_offset = 0;
            let mut node_offset = 0;
            let mut surface_offset = 0;
            for blas in cache.meshes.values_mut() {
                blas.packed_surfaces = surface_offset;
                surface_offset += blas.surface_capacity;
                blas.packed_triangles = triangle_offset;
                blas.packed_root = node_offset;
                triangle_offset += blas.triangle_capacity;
                node_offset += blas.node_capacity;
                pack_chunk(blas, revision);
            }
            cache.triangle_count = triangle_offset;
            cache.node_count = node_offset;
            cache.surface_count = surface_offset;
        }
        scene.geometry = cache
            .meshes
            .values()
            .filter_map(|blas| blas.chunk.clone())
            .collect();
        scene.triangle_count = cache.triangle_count;
        scene.node_count = cache.node_count;
        scene.surface_count = cache.surface_count;
        scene.geometry_revision = scene.geometry_revision.wrapping_add(1);
    }
    let mut instances = Vec::new();
    let mut bounds = Vec::new();
    let mut stationary = Vec::new();
    for (mesh, material, transform, visibility, overlay, ray_static, entity) in &query {
        if visibility.is_some_and(|visible| !visible.get()) {
            continue;
        }
        let Some(blas) = cache.meshes.get(&mesh.id()) else {
            continue;
        };
        let Some(material) = materials.get(&material.0) else {
            continue;
        };
        let tint = material.base_color.to_linear().to_vec4();
        if tint.w <= 0.0 {
            continue;
        }
        let matrix = transform.to_matrix();
        if !matrix.is_finite() || matrix.determinant().abs() < 1e-12 {
            continue;
        }
        let inverse = matrix.inverse();
        if !inverse.is_finite() {
            continue;
        }
        if ray_static.is_some() && overlay.is_none() {
            stationary.push(StaticSignature {
                entity: entity.to_bits(),
                index: entity.index_u32(),
                packed_triangles: blas.packed_triangles,
                mesh: mesh.id(),
                transform: matrix.to_cols_array().map(f32::to_bits),
                material: [
                    material.perceptual_roughness.to_bits(),
                    material.metallic.to_bits(),
                    u32::from(material.unlit),
                    material.diffuse_transmission.to_bits(),
                    tint.x.to_bits(),
                    tint.y.to_bits(),
                    tint.z.to_bits(),
                    tint.w.to_bits(),
                ],
            });
        }
        bounds.push(blas.bounds.transformed(matrix));
        instances.push(GpuInstance {
            world_from_local: matrix,
            local_from_world: inverse,
            material: Vec4::new(
                material.perceptual_roughness,
                material.metallic,
                u8::from(material.unlit) as f32,
                u8::from(overlay.is_some()) as f32,
            ),
            tint,
            root: blas.packed_root,
            transmission: material.diffuse_transmission.clamp(0.0, 1.0),
            pad1: u32::from(ray_static.is_some() && overlay.is_none()),
            pad2: entity.index_u32(),
        });
    }
    stationary.sort_by_key(|entry| entry.entity);
    for instance in instances.iter_mut().filter(|i| i.pad1 != 0) {
        instance.pad1 = stationary
            .iter()
            .position(|entry| entry.index == instance.pad2)
            .unwrap() as u32
            + 1;
    }
    if cache.static_snapshot != stationary
        || stationary
            .iter()
            .any(|entry| modified.contains(&entry.mesh))
    {
        scene.static_revision = scene.static_revision.wrapping_add(1);
        cache.static_snapshot = stationary;
    }
    let (nodes, order) = build_bvh_strategy(&bounds, 2, false);
    scene.instances = order.iter().map(|&index| instances[index]).collect();
    scene.tlas_nodes = nodes;
    let mut world = Vec::new();
    let mut static_world = Vec::new();
    let mut dynamic_world = Vec::new();
    scene.static_instances = [u32::MAX; 16];
    let mut shadow = Vec::new();
    let mut static_shadow = Vec::new();
    let mut dynamic_shadow = Vec::new();
    for (index, &source) in order.iter().enumerate() {
        let material = scene.instances[index].material;
        if material.w <= 0.5 {
            world.push((index, bounds[source]));
            let slot = scene.instances[index].pad1;
            if slot != 0 {
                static_world.push((index, bounds[source]));
                if slot <= 16 {
                    scene.static_instances[slot as usize - 1] = index as u32;
                }
            } else {
                dynamic_world.push((index, bounds[source]));
            }
            if material.z <= 0.5 {
                shadow.push((index, bounds[source]));
                if scene.instances[index].pad1 != 0 {
                    static_shadow.push((index, bounds[source]));
                } else {
                    dynamic_shadow.push((index, bounds[source]));
                }
            }
        }
    }
    scene.world_root = append_filtered_tlas(&mut scene.tlas_nodes, &world);
    scene.static_world_root = append_filtered_tlas(&mut scene.tlas_nodes, &static_world);
    scene.dynamic_world_root = append_filtered_tlas(&mut scene.tlas_nodes, &dynamic_world);
    scene.shadow_root = append_filtered_tlas(&mut scene.tlas_nodes, &shadow);
    scene.static_shadow_root = append_filtered_tlas(&mut scene.tlas_nodes, &static_shadow);
    scene.dynamic_shadow_root = append_filtered_tlas(&mut scene.tlas_nodes, &dynamic_shadow);
}

/// Singleton leaves let each auxiliary tree retain the primary instance IDs,
/// regardless of its own spatial permutation. Only internal child offsets move.
fn append_filtered_tlas(nodes: &mut Vec<BvhNode>, members: &[(usize, Bounds)]) -> u32 {
    if members.is_empty() {
        return u32::MAX;
    }
    let bounds: Vec<_> = members.iter().map(|(_, bounds)| *bounds).collect();
    let (mut auxiliary, order) = build_bvh_strategy(&bounds, 1, false);
    let root = u32::try_from(nodes.len()).expect("TLAS root must fit shader indices");
    u32::try_from(nodes.len() + auxiliary.len()).expect("TLAS nodes must fit shader indices");
    for node in &mut auxiliary {
        if node.count == 0 {
            node.first += root;
        } else {
            node.first = u32::try_from(members[order[node.first as usize]].0)
                .expect("TLAS instance must fit shader indices");
        }
    }
    nodes.extend(auxiliary);
    root
}

fn pack_chunk(blas: &mut MeshBlas, revision: u64) {
    let nodes = blas
        .nodes
        .iter()
        .map(|node| BvhNode {
            first: node.first
                + if node.count == 0 {
                    blas.packed_root
                } else {
                    blas.packed_triangles
                },
            ..*node
        })
        .collect();
    blas.chunk = Some(GeometryChunk {
        triangle_offset: blas.packed_triangles,
        surface_offset: blas.packed_surfaces,
        surfaces: blas.surfaces.clone(),
        surface_indices: blas.surface_indices.clone(),
        node_offset: blas.packed_root,
        revision,
        triangles: blas.triangles.clone(),
        nodes: Arc::new(nodes),
    });
}

fn mesh_blas(mesh: &Mesh) -> Option<MeshBlas> {
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
    else {
        return None;
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(values)) => Some(values),
        _ => None,
    };
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(values)) => Some(values),
        _ => None,
    };
    let indices: Vec<_> = mesh.indices().map_or_else(
        || (0..positions.len()).collect(),
        |indices| indices.iter().collect(),
    );
    let mut triangles = Vec::new();
    let mut bounds = Vec::new();
    for indices in indices.chunks_exact(3) {
        let [Some(a), Some(b), Some(c)] = [
            positions.get(indices[0]),
            positions.get(indices[1]),
            positions.get(indices[2]),
        ] else {
            continue;
        };
        let [a, b, c] = [Vec3::from(*a), Vec3::from(*b), Vec3::from(*c)];
        let normal = (b - a).cross(c - a);
        if !a.is_finite() || !b.is_finite() || !c.is_finite() || normal.length_squared() < 1e-20 {
            continue;
        }
        let face_normal = normal.normalize();
        let vertex_normal = |index| {
            normals
                .and_then(|values| values.get(index))
                .map(|value| Vec3::from(*value))
                .filter(|value| value.is_finite())
                .and_then(Vec3::try_normalize)
                .unwrap_or(face_normal)
                .extend(0.0)
        };
        let color = |index| {
            colors
                .and_then(|values| values.get(index))
                .map(|value| Vec4::from(*value))
                .filter(|value| value.is_finite())
                .unwrap_or(Vec4::ONE)
        };
        triangles.push(Triangle {
            a: a.extend(0.0),
            b: b.extend(0.0),
            c: c.extend(0.0),
            n0: vertex_normal(indices[0]),
            n1: vertex_normal(indices[1]),
            n2: vertex_normal(indices[2]),
            c0: color(indices[0]),
            c1: color(indices[1]),
            c2: color(indices[2]),
        });
        bounds.push(Bounds::empty().point(a).point(b).point(c));
    }
    if triangles.is_empty() {
        return None;
    }
    let (nodes, order) = build_bvh(&bounds, 4);
    let bounds = Bounds {
        min: nodes[0].min,
        max: nodes[0].max,
    };
    let triangles: Vec<_> = order.into_iter().map(|index| triangles[index]).collect();
    let (surfaces, surface_indices) = surface_palette(&triangles);
    Some(MeshBlas {
        triangles: Arc::new(triangles),
        surfaces: Arc::new(surfaces),
        surface_indices: Arc::new(surface_indices),
        packed_surfaces: 0,
        surface_capacity: 0,
        nodes,
        bounds,
        packed_root: 0,
        packed_triangles: 0,
        triangle_capacity: 0,
        node_capacity: 0,
        chunk: None,
    })
}

/// Lossless, per-mesh surface dictionary. Keys preserve every authored bit,
/// including signed zero; geometry and interpolated normals/colors do not change.
fn surface_palette(triangles: &[Triangle]) -> (Vec<TriangleSurface>, Vec<u32>) {
    let mut dictionary = HashMap::<[u32; 24], u32>::new();
    let mut surfaces = Vec::new();
    let mut indices = Vec::with_capacity(triangles.len());
    for triangle in triangles {
        let surface = triangle.surface();
        let mut key = [0; 24];
        for (output, vector) in key.chunks_exact_mut(4).zip([
            surface.n0, surface.n1, surface.n2, surface.c0, surface.c1, surface.c2,
        ]) {
            output.copy_from_slice(&vector.to_array().map(f32::to_bits));
        }
        let index = *dictionary.entry(key).or_insert_with(|| {
            let index = u32::try_from(surfaces.len()).expect("surface index must fit u32");
            surfaces.push(surface);
            index
        });
        indices.push(index);
    }
    (surfaces, indices)
}

/// Leaves refer to contiguous ranges in the returned permutation. Internal
/// children are allocated together so the shader derives right as left + 1.
///
/// BLAS trees have at most23 edges, TLAS trees15. Each accepted SAH split
/// reserves enough remaining depth for median fallback, even for adversarial
/// centroid spacing. The matching shader stacks have24 and16 entries. TLAS
/// singleton trees support32,768 instances; voxels share instanced meshes.
fn build_bvh(bounds: &[Bounds], leaf_size: usize) -> (Vec<BvhNode>, Vec<usize>) {
    build_bvh_strategy(bounds, leaf_size, true)
}

fn build_bvh_strategy(
    bounds: &[Bounds],
    leaf_size: usize,
    sah: bool,
) -> (Vec<BvhNode>, Vec<usize>) {
    assert!(
        leaf_size > 0,
        "BVH leaves must contain at least one primitive"
    );
    assert!(
        bounds.len() <= u32::MAX as usize / 2,
        "BVH primitive and node indices must fit the shader's u32 domain"
    );
    if bounds.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let max_depth = if sah { BLAS_MAX_DEPTH } else { TLAS_MAX_DEPTH };
    assert!(
        bounds.len() <= leaf_size.saturating_mul(1usize << max_depth),
        "BVH primitive count exceeds the bounded traversal capacity"
    );
    let mut order: Vec<_> = (0..bounds.len()).collect();
    let mut builder = BvhBuilder {
        bounds,
        leaf_size,
        sah,
        max_depth,
        nodes: vec![BvhNode::default()],
    };
    builder.split(&mut order, 0, 0, 0);
    (builder.nodes, order)
}

const SAH_BINS: usize = 12;
const BLAS_MAX_DEPTH: u32 = 23;
const TLAS_MAX_DEPTH: u32 = 15;

#[derive(Clone, Copy)]
struct SahBin {
    bounds: Bounds,
    count: usize,
}

impl SahBin {
    fn empty() -> Self {
        Self {
            bounds: Bounds::empty(),
            count: 0,
        }
    }

    fn include(mut self, bounds: Bounds, count: usize) -> Self {
        if count != 0 {
            self.bounds = self.bounds.union(bounds);
            self.count += count;
        }
        self
    }

    fn cost(self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let extent = (self.bounds.max - self.bounds.min).as_dvec3();
        (extent.x * extent.y + extent.y * extent.z + extent.z * extent.x) * self.count as f64
    }
}

struct BvhBuilder<'a> {
    bounds: &'a [Bounds],
    leaf_size: usize,
    sah: bool,
    max_depth: u32,
    nodes: Vec<BvhNode>,
}

impl BvhBuilder<'_> {
    fn split(&mut self, order: &mut [usize], offset: usize, node_index: usize, depth: u32) {
        let total = order.iter().fold(Bounds::empty(), |total, index| {
            total.union(self.bounds[*index])
        });
        self.nodes[node_index] = BvhNode {
            min: total.min,
            max: total.max,
            first: offset as u32,
            count: order.len() as u32,
        };
        if order.len() <= self.leaf_size {
            return;
        }
        assert!(
            depth < self.max_depth,
            "BVH split exhausted shader stack budget"
        );
        let centers = order.iter().fold(Bounds::empty(), |total, index| {
            total.point(self.bounds[*index].center())
        });
        let split = self
            .sah
            .then(|| self.sah_split(order, centers, depth))
            .flatten();
        let middle = if let Some((axis, boundary)) = split {
            let mut middle = 0;
            for index in 0..order.len() {
                if bin_index(
                    self.bounds[order[index]].center()[axis],
                    centers.min[axis],
                    centers.max[axis],
                ) <= boundary
                {
                    order.swap(index, middle);
                    middle += 1;
                }
            }
            middle
        } else {
            let extent = centers.max - centers.min;
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            let middle = order.len() / 2;
            order.select_nth_unstable_by(middle, |left, right| {
                self.bounds[*left].center()[axis].total_cmp(&self.bounds[*right].center()[axis])
            });
            middle
        };
        let child = self.nodes.len();
        self.nodes.extend([BvhNode::default(); 2]);
        self.nodes[node_index].first = child as u32;
        self.nodes[node_index].count = 0;
        let (left, right) = order.split_at_mut(middle);
        self.split(left, offset, child, depth + 1);
        self.split(right, offset + middle, child + 1, depth + 1);
    }

    fn sah_split(&self, order: &[usize], centers: Bounds, depth: u32) -> Option<(usize, usize)> {
        // A child has 30-depth remaining edges. Median splitting can accommodate
        // at most leaf_size * 2^(30-depth) primitives in that remaining budget.
        let child_capacity = self
            .leaf_size
            .saturating_mul(1usize << (self.max_depth - depth - 1));
        let mut best = None;
        let mut best_cost = f64::INFINITY;
        for axis in 0..3 {
            if centers.max[axis] <= centers.min[axis] {
                continue;
            }
            let mut bins = [SahBin::empty(); SAH_BINS];
            for &index in order {
                let bin = bin_index(
                    self.bounds[index].center()[axis],
                    centers.min[axis],
                    centers.max[axis],
                );
                bins[bin] = bins[bin].include(self.bounds[index], 1);
            }
            let mut suffix = [SahBin::empty(); SAH_BINS];
            let mut accumulated = SahBin::empty();
            for bin in (0..SAH_BINS).rev() {
                accumulated = accumulated.include(bins[bin].bounds, bins[bin].count);
                suffix[bin] = accumulated;
            }
            let mut left = SahBin::empty();
            for boundary in 0..SAH_BINS - 1 {
                left = left.include(bins[boundary].bounds, bins[boundary].count);
                let right = suffix[boundary + 1];
                if left.count == 0
                    || right.count == 0
                    || left.count > child_capacity
                    || right.count > child_capacity
                {
                    continue;
                }
                let cost = left.cost() + right.cost();
                if cost < best_cost {
                    best_cost = cost;
                    best = Some((axis, boundary));
                }
            }
        }
        best
    }
}

fn bin_index(value: f32, min: f32, max: f32) -> usize {
    (((f64::from(value) - f64::from(min)) / (f64::from(max) - f64::from(min)) * SAH_BINS as f64)
        as usize)
        .min(SAH_BINS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;

    fn triangle_mesh() -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
        )
    }

    fn triangle_hit(triangle: &Triangle, origin: Vec3, direction: Vec3) -> Option<f32> {
        let a = triangle.a.truncate();
        let edge1 = triangle.b.truncate() - a;
        let edge2 = triangle.c.truncate() - a;
        let p = direction.cross(edge2);
        let determinant = edge1.dot(p);
        if determinant.abs() < 1e-7 {
            return None;
        }
        let inv = determinant.recip();
        let s = origin - a;
        let u = s.dot(p) * inv;
        let q = s.cross(edge1);
        let v = direction.dot(q) * inv;
        let t = edge2.dot(q) * inv;
        (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
    }

    #[test]
    fn storage_struct_strides_match_shader_arrays() {
        assert_eq!(Triangle::min_size().get(), 144);
        assert_eq!(GeometryTriangle::min_size().get(), 48);
        assert_eq!(TriangleSurface::min_size().get(), 96);
        assert_eq!(BvhNode::min_size().get(), 32);
        assert_eq!(GpuInstance::min_size().get(), 176);
    }

    #[test]
    fn split_triangle_preserves_edges_and_all_surface_attributes() {
        let blas = mesh_blas(&triangle_mesh()).unwrap();
        for tri in blas.triangles.iter() {
            let geometry = tri.geometry(0);
            let surface = tri.surface();
            assert_eq!(geometry.a, tri.a.truncate());
            assert_eq!(geometry.e1.truncate(), tri.b.truncate() - tri.a.truncate());
            assert_eq!(geometry.e2.truncate(), tri.c.truncate() - tri.a.truncate());
            assert_eq!(
                [
                    surface.n0, surface.n1, surface.n2, surface.c0, surface.c1, surface.c2
                ],
                [tri.n0, tri.n1, tri.n2, tri.c0, tri.c1, tri.c2]
            );
        }
    }

    #[test]
    fn surface_dictionary_preserves_exact_attributes_and_ignores_positions() {
        let triangle = mesh_blas(&triangle_mesh()).unwrap().triangles[0];
        let mut moved = triangle;
        moved.a.x += 7.0;
        let mut changed = triangle;
        changed.c1.y += 0.125;
        let mut signed_zero = triangle;
        signed_zero.n0.w = -0.0;
        let triangles = [triangle, moved, changed, signed_zero, changed];
        let (surfaces, indices) = surface_palette(&triangles);
        assert_eq!(indices, [0, 0, 1, 2, 1]);
        assert_eq!(surfaces.len(), 3);
        for (triangle, index) in triangles.iter().zip(indices) {
            let actual = surfaces[index as usize];
            let expected = triangle.surface();
            for (a, b) in [
                actual.n0, actual.n1, actual.n2, actual.c0, actual.c1, actual.c2,
            ]
            .into_iter()
            .zip([
                expected.n0,
                expected.n1,
                expected.n2,
                expected.c0,
                expected.c1,
                expected.c2,
            ]) {
                assert_eq!(
                    a.to_array().map(f32::to_bits),
                    b.to_array().map(f32::to_bits)
                );
            }
        }
    }

    #[test]
    fn affine_local_rays_preserve_world_hit_distance() {
        let blas = mesh_blas(&triangle_mesh()).expect("triangle");
        for scale in [
            Vec3::ONE,
            Vec3::new(0.2, 3.0, 1.7),
            Vec3::new(-2.0, 0.4, 0.7),
        ] {
            let matrix = Mat4::from_scale_rotation_translation(
                scale,
                Quat::from_rotation_y(0.42) * Quat::from_rotation_x(-0.3),
                Vec3::new(3.0, 1.0, -4.0),
            );
            let inverse = matrix.inverse();
            let mut world = blas.triangles[0];
            world.a = matrix.transform_point3(world.a.truncate()).extend(0.0);
            world.b = matrix.transform_point3(world.b.truncate()).extend(0.0);
            world.c = matrix.transform_point3(world.c.truncate()).extend(0.0);
            let center = matrix.transform_point3(Vec3::ZERO);
            let direction = matrix.transform_vector3(Vec3::NEG_Z).normalize();
            let origin = center - direction * 7.0;
            let expected = triangle_hit(&world, origin, direction).expect("world hit");
            // Do not normalize the transformed direction: the t parameter must
            // remain in world units even with nonuniform or reflected scale.
            let actual = triangle_hit(
                &blas.triangles[0],
                inverse.transform_point3(origin),
                inverse.transform_vector3(direction),
            )
            .expect("local hit");
            assert!((expected - actual).abs() < 0.0001);
            let bounds = blas.bounds.transformed(matrix);
            for vertex in [world.a, world.b, world.c] {
                assert!(vertex.truncate().cmpge(bounds.min).all());
                assert!(vertex.truncate().cmple(bounds.max).all());
            }
        }
    }

    #[test]
    fn bvh_covers_each_primitive_once_and_bounds_children() {
        let bounds: Vec<_> = (0..10003)
            .map(|i| {
                let point = Vec3::new((i % 13) as f32, (i % 57) as f32, (i % 101) as f32);
                Bounds::empty().point(point).point(point + Vec3::splat(0.1))
            })
            .collect();
        let (nodes, order) = build_bvh(&bounds, 4);
        let mut visited = vec![false; bounds.len()];
        let mut stack = vec![(0, 0)];
        while let Some((index, depth)) = stack.pop() {
            assert!(depth <= BLAS_MAX_DEPTH, "shader traversal stack limit");
            let node = nodes[index];
            if node.count > 0 {
                for position in node.first..node.first + node.count {
                    let primitive = order[position as usize];
                    assert!(!visited[primitive]);
                    visited[primitive] = true;
                    assert!(bounds[primitive].min.cmpge(node.min).all());
                    assert!(bounds[primitive].max.cmple(node.max).all());
                }
            } else {
                for child in [node.first as usize, node.first as usize + 1] {
                    assert!(nodes[child].min.cmpge(node.min).all());
                    assert!(nodes[child].max.cmple(node.max).all());
                    stack.push((child, depth + 1));
                }
            }
        }
        assert!(visited.into_iter().all(|value| value));
    }

    fn bounds_hit(bounds: Bounds, origin: Vec3, direction: Vec3, limit: f32) -> Option<f32> {
        let mut near = 0.0001_f32;
        let mut far = limit;
        for axis in 0..3 {
            if direction[axis].abs() < 1e-10 {
                if origin[axis] < bounds.min[axis] || origin[axis] > bounds.max[axis] {
                    return None;
                }
            } else {
                let a = (bounds.min[axis] - origin[axis]) / direction[axis];
                let b = (bounds.max[axis] - origin[axis]) / direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if far < near {
                    return None;
                }
            }
        }
        (near < limit).then_some(near)
    }

    // Exercise the shader's near-first hierarchy algorithm against primitive
    // bounds directly, counting every node and primitive slab test as work.
    fn hierarchy_hit(
        bounds: &[Bounds],
        nodes: &[BvhNode],
        order: &[usize],
        origin: Vec3,
        direction: Vec3,
    ) -> (Option<f32>, usize) {
        let mut closest = 1e30_f32;
        let mut stack = vec![(0, 0.0)];
        let mut work = 0;
        while let Some((index, entry)) = stack.pop() {
            if entry >= closest {
                continue;
            }
            let node = nodes[index];
            if node.count != 0 {
                for position in node.first..node.first + node.count {
                    work += 1;
                    if let Some(hit) =
                        bounds_hit(bounds[order[position as usize]], origin, direction, closest)
                    {
                        closest = hit;
                    }
                }
            } else {
                let mut children = Vec::new();
                for index in [node.first as usize, node.first as usize + 1] {
                    work += 1;
                    let child = nodes[index];
                    if let Some(entry) = bounds_hit(
                        Bounds {
                            min: child.min,
                            max: child.max,
                        },
                        origin,
                        direction,
                        closest,
                    ) {
                        children.push((index, entry));
                    }
                }
                children.sort_by(|a, b| b.1.total_cmp(&a.1));
                stack.extend(children);
            }
        }
        ((closest < 1e30).then_some(closest), work)
    }

    #[test]
    fn sah_traversal_matches_brute_force_for_parallel_inside_and_oblique_rays() {
        let bounds: Vec<_> = (0..257)
            .map(|i| {
                let point = Vec3::new(
                    (i % 13) as f32 - 6.0,
                    (i % 17) as f32 - 8.0,
                    (i % 11) as f32 - 5.0,
                );
                Bounds::empty()
                    .point(point)
                    .point(point + Vec3::new(0.2, 0.7, 0.4))
            })
            .collect();
        let (nodes, order) = build_bvh(&bounds, 4);
        for i in 0..1024 {
            let origin = Vec3::new(
                (i % 19) as f32 - 9.0,
                (i % 23) as f32 - 11.0,
                (i % 29) as f32 - 14.0,
            );
            for direction in [
                Vec3::Z,
                Vec3::NEG_X,
                Vec3::Y,
                Vec3::new(-0.0, 1e-12, 1.0),
                Vec3::new(0.3, -0.2, 0.9).normalize(),
            ] {
                let expected = bounds
                    .iter()
                    .filter_map(|bounds| bounds_hit(*bounds, origin, direction, 1e30))
                    .min_by(f32::total_cmp);
                let actual = hierarchy_hit(&bounds, &nodes, &order, origin, direction).0;
                assert_eq!(actual, expected, "ray {origin:?} {direction:?}");
            }
        }
    }

    #[test]
    fn sah_skips_dense_clusters_more_efficiently_than_median() {
        let mut bounds: Vec<_> = (0..127)
            .map(|i| {
                let point = Vec3::new(
                    (i % 7) as f32 * 0.1,
                    (i % 11) as f32 * 0.1,
                    (i % 13) as f32 * 0.1,
                );
                Bounds::empty()
                    .point(point)
                    .point(point + Vec3::splat(0.15))
            })
            .collect();
        bounds.push(
            Bounds::empty()
                .point(Vec3::new(999.5, -0.5, -0.5))
                .point(Vec3::new(1000.5, 0.5, 0.5)),
        );
        let (sah_nodes, sah_order) = build_bvh(&bounds, 4);
        let (median_nodes, median_order) = build_bvh_strategy(&bounds, 4, false);
        let mut sah_work = 0;
        let mut median_work = 0;
        for i in 0..128 {
            let origin = Vec3::new(1000.0, i as f32 / 128.0 - 0.5, -10.0);
            let sah = hierarchy_hit(&bounds, &sah_nodes, &sah_order, origin, Vec3::Z);
            let median = hierarchy_hit(&bounds, &median_nodes, &median_order, origin, Vec3::Z);
            assert_eq!(sah.0, Some(9.5));
            assert_eq!(sah.0, median.0);
            sah_work += sah.1;
            median_work += median.1;
        }
        assert!(
            sah_work * 2 < median_work,
            "SAH {sah_work} vs median {median_work} slab tests"
        );
    }

    #[test]
    fn sah_rejects_unbalanced_split_when_only_one_depth_remains() {
        let mut bounds = vec![Bounds::empty().point(Vec3::ZERO).point(Vec3::ONE); 7];
        bounds.push(
            Bounds::empty()
                .point(Vec3::splat(100.0))
                .point(Vec3::splat(101.0)),
        );
        let mut builder = BvhBuilder {
            bounds: &bounds,
            leaf_size: 4,
            sah: true,
            max_depth: BLAS_MAX_DEPTH,
            nodes: vec![BvhNode::default()],
        };
        let centers = bounds.iter().fold(Bounds::empty(), |total, bounds| {
            total.point(bounds.center())
        });
        let mut order: Vec<_> = (0..8).collect();
        assert!(
            builder
                .sah_split(&order, centers, BLAS_MAX_DEPTH - 1)
                .is_none()
        );
        builder.split(&mut order, 0, 0, BLAS_MAX_DEPTH - 1);
        assert_eq!(builder.nodes.len(), 3);
        assert_eq!(builder.nodes[1].count, 4);
        assert_eq!(builder.nodes[2].count, 4);
    }

    #[test]
    fn stationary_shadow_revision_tracks_only_eligible_scene_changes() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<MeshCache>()
            .init_resource::<RayScene>()
            .add_message::<AssetEvent<Mesh>>()
            .add_systems(Update, extract_scene);
        let fixed_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(triangle_mesh());
        let moving_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(triangle_mesh());
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let fixed = app
            .world_mut()
            .spawn((
                RayStatic,
                Mesh3d(fixed_mesh.clone()),
                MeshMaterial3d(material.clone()),
                GlobalTransform::IDENTITY,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let moving = app
            .world_mut()
            .spawn((
                Mesh3d(moving_mesh.clone()),
                MeshMaterial3d(material.clone()),
                GlobalTransform::from_translation(Vec3::X),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.update();
        let revision = app.world().resource::<RayScene>().static_revision;
        assert!(revision > 0);
        {
            let scene = app.world().resource::<RayScene>();
            let fixed_leaf = scene.tlas_nodes[scene.static_shadow_root as usize];
            let moving_leaf = scene.tlas_nodes[scene.dynamic_shadow_root as usize];
            assert_eq!(scene.instances[fixed_leaf.first as usize].pad1, 1);
            assert_eq!(scene.instances[moving_leaf.first as usize].pad1, 0);
        }
        // Move across the static instance to permute the primary TLAS order.
        app.world_mut()
            .entity_mut(moving)
            .insert(GlobalTransform::from_translation(-Vec3::X));
        app.world_mut().write_message(AssetEvent::Modified {
            id: moving_mesh.id(),
        });
        app.update();
        assert_eq!(app.world().resource::<RayScene>().static_revision, revision);
        app.world_mut().entity_mut(moving).insert(RayOverlay);
        app.update();
        assert_eq!(app.world().resource::<RayScene>().static_revision, revision);
        app.world_mut()
            .entity_mut(fixed)
            .insert(GlobalTransform::from_translation(Vec3::Y));
        app.update();
        assert_eq!(
            app.world().resource::<RayScene>().static_revision,
            revision + 1
        );
        app.world_mut()
            .resource_mut::<Assets<Mesh>>()
            .get_mut(&fixed_mesh)
            .unwrap()
            .insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 3]);
        app.world_mut().write_message(AssetEvent::Modified {
            id: fixed_mesh.id(),
        });
        app.update();
        assert_eq!(
            app.world().resource::<RayScene>().static_revision,
            revision + 2
        );
        app.world_mut()
            .entity_mut(fixed)
            .insert(InheritedVisibility::HIDDEN);
        app.update();
        assert_eq!(
            app.world().resource::<RayScene>().static_revision,
            revision + 3
        );
        assert_eq!(
            app.world().resource::<RayScene>().static_shadow_root,
            u32::MAX
        );
        app.world_mut()
            .entity_mut(fixed)
            .insert(InheritedVisibility::VISIBLE);
        app.update();
        assert_eq!(
            app.world().resource::<RayScene>().static_revision,
            revision + 4
        );
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&material)
            .unwrap()
            .unlit = true;
        app.update();
        assert_eq!(
            app.world().resource::<RayScene>().static_revision,
            revision + 5
        );
        assert_eq!(
            app.world().resource::<RayScene>().static_shadow_root,
            u32::MAX
        );
        app.world_mut().entity_mut(fixed).remove::<RayStatic>();
        app.update();
        assert_eq!(
            app.world().resource::<RayScene>().static_revision,
            revision + 6
        );
    }

    #[test]
    fn moving_instances_reuse_geometry_and_asset_edits_update_only_their_chunk() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<MeshCache>()
            .init_resource::<RayScene>()
            .add_message::<AssetEvent<Mesh>>()
            .add_systems(Update, extract_scene);
        let first = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(triangle_mesh());
        let second = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(triangle_mesh());
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let entity = app
            .world_mut()
            .spawn((
                Mesh3d(first.clone()),
                MeshMaterial3d(material.clone()),
                GlobalTransform::IDENTITY,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((
                Mesh3d(second.clone()),
                MeshMaterial3d(material.clone()),
                GlobalTransform::IDENTITY,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.update();
        let before = app.world().resource::<RayScene>().clone();
        assert_eq!(before.instances.len(), 2);
        app.world_mut()
            .entity_mut(entity)
            .insert(GlobalTransform::from_translation(Vec3::X));
        app.update();
        let moved = app.world().resource::<RayScene>();
        assert_eq!(moved.geometry_revision, before.geometry_revision);
        assert!(
            moved
                .instances
                .iter()
                .any(|instance| instance.world_from_local.w_axis.x == 1.0)
        );
        for old in &before.geometry {
            let new = moved
                .geometry
                .iter()
                .find(|new| new.node_offset == old.node_offset)
                .unwrap();
            assert!(Arc::ptr_eq(&new.triangles, &old.triangles));
            assert!(Arc::ptr_eq(&new.nodes, &old.nodes));
        }
        let root = app.world().resource::<MeshCache>().meshes[&first.id()].packed_root;
        app.world_mut()
            .resource_mut::<Assets<Mesh>>()
            .get_mut(&first)
            .unwrap()
            .insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vec![[-2.0, -1.0, 0.0], [2.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
            );
        app.world_mut()
            .write_message(AssetEvent::Modified { id: first.id() });
        app.update();
        let edited = app.world().resource::<RayScene>();
        assert_ne!(edited.geometry_revision, before.geometry_revision);
        assert_eq!(
            app.world().resource::<MeshCache>().meshes[&first.id()].packed_root,
            root
        );
        assert_eq!(
            edited
                .geometry
                .iter()
                .filter(|new| before
                    .geometry
                    .iter()
                    .any(|old| old.node_offset == new.node_offset && old.revision == new.revision))
                .count(),
            1
        );
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&material)
            .unwrap()
            .base_color = Color::NONE;
        app.update();
        assert!(app.world().resource::<RayScene>().instances.is_empty());
        app.world_mut().despawn(entity);
        app.world_mut().despawn(other);
        app.update();
        assert!(app.world().resource::<MeshCache>().meshes.is_empty());
        assert!(app.world().resource::<RayScene>().geometry.is_empty());
    }

    #[test]
    fn auxiliary_tlas_preserves_instance_ids_and_ray_categories() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<MeshCache>()
            .init_resource::<RayScene>()
            .add_message::<AssetEvent<Mesh>>()
            .add_systems(Update, extract_scene);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(triangle_mesh());
        let lit = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let unlit = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                unlit: true,
                ..default()
            });
        for (point, material, overlay) in [
            (Vec3::new(0.0, 0.0, 3.0), lit.clone(), true),
            (Vec3::new(0.0, 0.0, 2.0), unlit.clone(), false),
            (Vec3::new(0.0, 0.0, 1.0), lit.clone(), false),
            (Vec3::new(5.0, 0.0, 1.0), lit.clone(), false),
            (Vec3::new(-5.0, 0.0, 2.0), unlit.clone(), false),
            (Vec3::new(10.0, 0.0, 3.0), lit.clone(), true),
        ] {
            let mut entity = app.world_mut().spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                GlobalTransform::from_translation(point),
                InheritedVisibility::VISIBLE,
            ));
            if overlay {
                entity.insert(RayOverlay);
            }
        }
        app.update();
        let scene = app.world().resource::<RayScene>();
        assert_eq!(
            scene.instances.len(),
            6,
            "auxiliary trees never duplicate instances"
        );
        let members = |root| {
            let mut result = Vec::new();
            let mut stack = vec![root];
            while let Some(index) = stack.pop() {
                let node = scene.tlas_nodes[index as usize];
                if node.count == 0 {
                    stack.extend([node.first, node.first + 1]);
                } else {
                    result.extend(node.first..node.first + node.count);
                }
            }
            result.sort_unstable();
            result
        };
        assert_eq!(members(0), (0..6).collect::<Vec<_>>());
        let world: Vec<_> = scene
            .instances
            .iter()
            .enumerate()
            .filter_map(|(index, instance)| (instance.material.w <= 0.5).then_some(index as u32))
            .collect();
        let shadow: Vec<_> = scene
            .instances
            .iter()
            .enumerate()
            .filter_map(|(index, instance)| {
                (instance.material.w <= 0.5 && instance.material.z <= 0.5).then_some(index as u32)
            })
            .collect();
        assert_eq!(world.len(), 4);
        assert_eq!(shadow.len(), 2);
        assert_eq!(members(scene.world_root), world);
        assert_eq!(members(scene.shadow_root), shadow);
        let triangle = mesh_blas(&triangle_mesh()).unwrap().triangles[0];
        let hit = |root| {
            let origin = Vec3::new(0.0, 0.0, 10.0);
            let direction = Vec3::NEG_Z;
            let mut closest = 100.0;
            let mut stack = vec![root];
            while let Some(index) = stack.pop() {
                let node = scene.tlas_nodes[index as usize];
                if bounds_hit(
                    Bounds {
                        min: node.min,
                        max: node.max,
                    },
                    origin,
                    direction,
                    closest,
                )
                .is_none()
                {
                    continue;
                }
                if node.count == 0 {
                    stack.extend([node.first, node.first + 1]);
                } else {
                    for index in node.first..node.first + node.count {
                        let inverse = scene.instances[index as usize].local_from_world;
                        if let Some(t) = triangle_hit(
                            &triangle,
                            inverse.transform_point3(origin),
                            inverse.transform_vector3(direction),
                        ) {
                            closest = closest.min(t);
                        }
                    }
                }
            }
            closest
        };
        assert_eq!(hit(0), 7.0, "primary sees overlay");
        assert_eq!(
            hit(scene.world_root),
            8.0,
            "world sees unlit geometry behind overlay"
        );
        assert_eq!(
            hit(scene.shadow_root),
            9.0,
            "shadow skips overlay and unlit geometry"
        );
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&lit)
            .unwrap()
            .unlit = true;
        app.update();
        assert_eq!(app.world().resource::<RayScene>().shadow_root, u32::MAX);
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&lit)
            .unwrap()
            .base_color = Color::NONE;
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&unlit)
            .unwrap()
            .base_color = Color::NONE;
        app.update();
        let empty = app.world().resource::<RayScene>();
        assert!(empty.tlas_nodes.is_empty());
        assert_eq!(empty.world_root, u32::MAX);
        assert_eq!(empty.shadow_root, u32::MAX);
        assert_eq!(RayScene::default().world_root, u32::MAX);
        assert_eq!(RayScene::default().shadow_root, u32::MAX);
    }

    #[test]
    #[should_panic(expected = "BVH leaves must contain at least one primitive")]
    fn zero_sized_leaves_are_rejected_before_splitting() {
        let _ = build_bvh(&[Bounds::empty().point(Vec3::ZERO)], 0);
    }

    #[test]
    fn empty_and_degenerate_meshes_have_no_acceleration_structure() {
        assert!(build_bvh(&[], 4).0.is_empty());
        let empty = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        );
        assert!(mesh_blas(&empty).is_none());
        let degenerate = empty.with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0; 3]; 3]);
        assert!(mesh_blas(&degenerate).is_none());
        let nonfinite = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[f32::NAN; 3]; 3]);
        assert!(mesh_blas(&nonfinite).is_none());
    }
}
