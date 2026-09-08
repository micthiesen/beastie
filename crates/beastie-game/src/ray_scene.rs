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
    pub revision: u64,
    pub triangles: Arc<Vec<Triangle>>,
    pub nodes: Arc<Vec<BvhNode>>,
}

#[derive(Resource, Clone, Default, ExtractResource)]
pub struct RayScene {
    pub geometry: Vec<GeometryChunk>,
    pub triangle_count: u32,
    pub node_count: u32,
    pub instances: Vec<GpuInstance>,
    pub tlas_nodes: Vec<BvhNode>,
    pub geometry_revision: u64,
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
    triangle_count: u32,
    node_count: u32,
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
    ),
>;

fn extract_scene(
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    query: SceneInstances,
    mut cache: ResMut<MeshCache>,
    mut scene: ResMut<RayScene>,
) {
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
        }) {
            blas.packed_root = old.packed_root;
            blas.packed_triangles = old.packed_triangles;
            blas.triangle_capacity = old.triangle_capacity;
            blas.node_capacity = old.node_capacity;
        } else {
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
        let live_nodes: u32 = cache.meshes.values().map(|mesh| mesh.node_capacity).sum();
        // Reclaim removed/replaced allocations before dead space can grow without
        // bound. Most frames update only the small effects mesh in its old slot.
        if cache.triangle_count > live_triangles.saturating_mul(2).saturating_add(1024)
            || cache.node_count > live_nodes.saturating_mul(2).saturating_add(1024)
        {
            cache.revision = cache.revision.wrapping_add(1);
            let revision = cache.revision;
            let mut triangle_offset = 0;
            let mut node_offset = 0;
            for blas in cache.meshes.values_mut() {
                blas.packed_triangles = triangle_offset;
                blas.packed_root = node_offset;
                triangle_offset += blas.triangle_capacity;
                node_offset += blas.node_capacity;
                pack_chunk(blas, revision);
            }
            cache.triangle_count = triangle_offset;
            cache.node_count = node_offset;
        }
        scene.geometry = cache
            .meshes
            .values()
            .filter_map(|blas| blas.chunk.clone())
            .collect();
        scene.triangle_count = cache.triangle_count;
        scene.node_count = cache.node_count;
        scene.geometry_revision = scene.geometry_revision.wrapping_add(1);
    }
    let mut instances = Vec::new();
    let mut bounds = Vec::new();
    for (mesh, material, transform, visibility, overlay) in &query {
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
            pad1: 0,
            pad2: 0,
        });
    }
    let (nodes, order) = build_bvh(&bounds, 2);
    scene.instances = order.into_iter().map(|index| instances[index]).collect();
    scene.tlas_nodes = nodes;
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
    Some(MeshBlas {
        triangles: Arc::new(order.into_iter().map(|index| triangles[index]).collect()),
        nodes,
        bounds,
        packed_root: 0,
        packed_triangles: 0,
        triangle_capacity: 0,
        node_capacity: 0,
        chunk: None,
    })
}

/// Leaves refer to contiguous ranges in the returned permutation. Internal
/// children are allocated together so the shader derives right as left + 1.
///
/// The domain below bounds both primitive and node indices: even one primitive
/// per leaf creates at most `2 * primitive_count - 1` nodes, which fits in u32.
/// Every split divides the primitive count at its median, so the longest path
/// has at most `ceil(log2(primitive_count)) + 1 <= 32` nodes. Depth-first shader
/// traversal keeps at most one pending sibling per ancestor plus the current
/// node, hence at most 32 entries. WGSL descends directly into the near child
/// and stacks only far siblings, so its 32-slot stacks need at most 31 entries.
/// This bound holds for coincident centroids too: they still split by count.
fn build_bvh(bounds: &[Bounds], leaf_size: usize) -> (Vec<BvhNode>, Vec<usize>) {
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
    let mut order: Vec<_> = (0..bounds.len()).collect();
    let mut nodes = vec![BvhNode::default()];
    split_node(bounds, &mut order, 0, leaf_size, &mut nodes, 0);
    (nodes, order)
}

fn split_node(
    bounds: &[Bounds],
    order: &mut [usize],
    offset: usize,
    leaf_size: usize,
    nodes: &mut Vec<BvhNode>,
    node_index: usize,
) {
    let total = order
        .iter()
        .fold(Bounds::empty(), |total, index| total.union(bounds[*index]));
    nodes[node_index] = BvhNode {
        min: total.min,
        max: total.max,
        first: offset as u32,
        count: order.len() as u32,
    };
    if order.len() <= leaf_size {
        return;
    }
    let centers = order.iter().fold(Bounds::empty(), |total, index| {
        total.point(bounds[*index].center())
    });
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
        bounds[*left].center()[axis].total_cmp(&bounds[*right].center()[axis])
    });
    let child = nodes.len();
    nodes.extend([BvhNode::default(); 2]);
    nodes[node_index].first = child as u32;
    nodes[node_index].count = 0;
    let (left, right) = order.split_at_mut(middle);
    split_node(bounds, left, offset, leaf_size, nodes, child);
    split_node(bounds, right, offset + middle, leaf_size, nodes, child + 1);
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
        assert_eq!(BvhNode::min_size().get(), 32);
        assert_eq!(GpuInstance::min_size().get(), 176);
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
            assert!(depth < 32, "shader traversal stack limit");
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
