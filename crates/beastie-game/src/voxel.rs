//! Colored solid geometry. Interior voxel faces are omitted before upload.

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};

/// Rendering treatments share the same authored occupancy and colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceStyle {
    #[default]
    Sharp,
    Beveled,
    /// Deliberate construction-block treatment, for visual comparisons.
    Separated,
}

#[derive(Default)]
pub struct VoxelModel {
    cells: BTreeMap<[i32; 3], [u8; 3]>,
}

impl VoxelModel {
    pub fn set(&mut self, point: [i32; 3], color: [u8; 3]) {
        self.cells.insert(point, color);
    }

    pub fn ellipsoid(radii: [i32; 3], color: impl Fn([i32; 3]) -> [u8; 3]) -> Self {
        let mut model = Self::default();
        for x in -radii[0]..=radii[0] {
            for y in -radii[1]..=radii[1] {
                for z in -radii[2]..=radii[2] {
                    let p = [x, y, z];
                    let distance: f32 = (0..3)
                        .map(|axis| (p[axis] as f32 / (radii[axis] as f32 + 0.5)).powi(2))
                        .sum();
                    if distance <= 1.0 {
                        model.set(p, color(p));
                    }
                }
            }
        }
        model
    }

    #[cfg(test)]
    pub fn mesh(&self, cell_size: f32) -> Mesh {
        self.mesh_with_style(cell_size, SurfaceStyle::Sharp)
    }

    pub fn mesh_with_style(&self, cell_size: f32, style: SurfaceStyle) -> Mesh {
        self.build_mesh(cell_size, style, false)
    }

    /// Reduce planar bevel subdivisions while retaining every boundary edge.
    /// Use only when normals remain flat: later position-dependent normal or
    /// color generation can depend on the removed interior vertices.
    pub fn mesh_with_flat_normals(&self, cell_size: f32, style: SurfaceStyle) -> Mesh {
        self.build_mesh(cell_size, style, true)
    }

    fn build_mesh(&self, cell_size: f32, style: SurfaceStyle, reduce_flat_faces: bool) -> Mesh {
        assert!(cell_size.is_finite() && cell_size > 0.0);
        let mut geometry = Geometry::default();
        if style == SurfaceStyle::Separated {
            for (&point, &color) in &self.cells {
                geometry.cuboid(
                    Vec3::from_array(point.map(|v| v as f32 * cell_size)),
                    Vec3::splat(cell_size * 0.88),
                    color,
                );
            }
            return geometry.mesh();
        }
        let mut boundary = Boundary::default();
        for (&point, &color) in &self.cells {
            for (axis, sign) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1)] {
                let mut neighbor = point;
                neighbor[axis] += sign;
                if !self.cells.contains_key(&neighbor) {
                    boundary.face(point, axis, sign, color, style);
                }
            }
        }
        boundary
            .geometry(cell_size, style, reduce_flat_faces)
            .mesh()
    }
}

// Integer face coordinates are shared before conversion to world space. In particular,
// (cell * size) +/- half_size cannot introduce different rounding on neighboring faces.
// Bevels occupy only a narrow band beside shape edges, never coplanar color boundaries.
const LATTICE: i64 = 1_000;
const HALF_CELL: i64 = LATTICE / 2;
const BEVEL_INNER: i64 = 380;
type Point = [i64; 3];

#[derive(Default)]
struct Boundary {
    vertices: BTreeMap<Point, BoundaryVertex>,
    quads: Vec<([Point; 4], [u8; 3])>,
}

#[derive(Default)]
struct BoundaryVertex {
    neighbors: BTreeSet<Point>,
    normals: BTreeSet<(usize, i32)>,
}

impl Boundary {
    fn face(
        &mut self,
        cell: [i32; 3],
        axis: usize,
        sign: i32,
        color: [u8; 3],
        style: SurfaceStyle,
    ) {
        let steps: &[i64] = if style == SurfaceStyle::Beveled {
            &[-HALF_CELL, -BEVEL_INNER, BEVEL_INNER, HALF_CELL]
        } else {
            &[-HALF_CELL, HALF_CELL]
        };
        let a = (axis + 1) % 3;
        let b = (axis + 2) % 3;
        for us in steps.windows(2) {
            for vs in steps.windows(2) {
                let mut quad = [
                    (us[0], vs[0]),
                    (us[1], vs[0]),
                    (us[1], vs[1]),
                    (us[0], vs[1]),
                ]
                .map(|(u, v)| {
                    let mut p = cell.map(|c| i64::from(c) * LATTICE);
                    p[axis] += i64::from(sign) * HALF_CELL;
                    p[a] += u;
                    p[b] += v;
                    p
                });
                if sign < 0 {
                    quad.reverse();
                }
                for i in 0..4 {
                    let vertex = self.vertices.entry(quad[i]).or_default();
                    vertex.normals.insert((axis, sign));
                    vertex.neighbors.insert(quad[(i + 1) % 4]);
                    vertex.neighbors.insert(quad[(i + 3) % 4]);
                }
                self.quads.push((quad, color));
            }
        }
    }

    fn geometry(self, cell_size: f32, style: SurfaceStyle, reduce_flat_faces: bool) -> Geometry {
        let positions: BTreeMap<_, _> = self
            .vertices
            .iter()
            .map(|(&point, vertex)| {
                let mut position = point.map(|p| p as f64);
                if style == SurfaceStyle::Beveled && vertex.normals.len() > 1 {
                    // One simultaneous relaxation of the connected surface creates real narrow
                    // chamfers. Restrict displacement to crease-normal axes so an uneven grid
                    // cannot slide a vertex along an otherwise straight edge. Concave corners
                    // use the very same shared position, preventing cracks and overlapping caps.
                    for (axis, coordinate) in position.iter_mut().enumerate() {
                        if vertex
                            .normals
                            .iter()
                            .any(|&(normal_axis, _)| normal_axis == axis)
                        {
                            *coordinate =
                                vertex.neighbors.iter().map(|p| p[axis] as f64).sum::<f64>()
                                    / vertex.neighbors.len() as f64;
                        }
                    }
                }
                (
                    point,
                    Vec3::from_array(
                        position.map(|p| (p * f64::from(cell_size) / LATTICE as f64) as f32),
                    ),
                )
            })
            .collect();
        let mut geometry = Geometry::default();
        let face_quads = if style == SurfaceStyle::Beveled { 9 } else { 1 };
        for face in self.quads.chunks_exact(face_quads) {
            if reduce_flat_faces
                && style == SurfaceStyle::Beveled
                && let Some(triangles) = flat_face_triangles(face, &positions)
            {
                for triangle in triangles {
                    geometry.triangle(triangle, face[0].1);
                }
                continue;
            }
            for &(quad, color) in face {
                let points = quad.map(|p| positions[&p]);
                if style == SurfaceStyle::Sharp {
                    geometry.quad(points, color);
                } else {
                    geometry.triangle([points[0], points[1], points[2]], color);
                    geometry.triangle([points[0], points[2], points[3]], color);
                }
            }
        }
        geometry
    }
}

/// Ear clipping preserves the collinear perimeter vertices that neighboring
/// bevel patches share. A simple two-triangle quad would leave unmatched edges.
fn flat_face_triangles(
    face: &[([Point; 4], [u8; 3])],
    positions: &BTreeMap<Point, Vec3>,
) -> Option<Vec<[Vec3; 3]>> {
    let first = positions[&face[0].0[0]];
    let axis = (0..3).find(|&axis| {
        face.iter()
            .flat_map(|(quad, _)| quad)
            .all(|p| positions[p][axis] == first[axis])
    })?;
    let mut edges = BTreeMap::new();
    for (quad, _) in face {
        for i in 0..4 {
            let from = quad[i];
            let to = quad[(i + 1) % 4];
            if edges.remove(&(to, from)).is_none() {
                edges.insert((from, to), ());
            }
        }
    }
    let start = edges.first_key_value()?.0.0;
    let mut point = start;
    let mut polygon = Vec::with_capacity(edges.len());
    loop {
        polygon.push(positions[&point]);
        let next = edges.keys().find(|&&(from, _)| from == point)?.1;
        edges.remove(&(point, next));
        point = next;
        if point == start {
            break;
        }
    }
    if !edges.is_empty() || polygon.len() != 12 {
        return None;
    }
    let a = (axis + 1) % 3;
    let b = (axis + 2) % 3;
    let cross = |p: Vec3, q: Vec3, r: Vec3| {
        (f64::from(q[a]) - f64::from(p[a])) * (f64::from(r[b]) - f64::from(p[b]))
            - (f64::from(q[b]) - f64::from(p[b])) * (f64::from(r[a]) - f64::from(p[a]))
    };
    let normal = face[0].0.map(|p| positions[&p]);
    let sign = cross(normal[0], normal[1], normal[2]).signum();
    let mut triangles = Vec::with_capacity(10);
    while polygon.len() > 3 {
        let ear = (0..polygon.len()).find(|&i| {
            let previous = (i + polygon.len() - 1) % polygon.len();
            let next = (i + 1) % polygon.len();
            let [p, q, r] = [polygon[previous], polygon[i], polygon[next]];
            cross(p, q, r) * sign > 0.0
                && polygon.iter().enumerate().all(|(j, &test)| {
                    j == previous
                        || j == i
                        || j == next
                        || cross(p, q, test) * sign < 0.0
                        || cross(q, r, test) * sign < 0.0
                        || cross(r, p, test) * sign < 0.0
                })
        })?;
        triangles.push([
            polygon[(ear + polygon.len() - 1) % polygon.len()],
            polygon[ear],
            polygon[(ear + 1) % polygon.len()],
        ]);
        polygon.remove(ear);
    }
    if cross(polygon[0], polygon[1], polygon[2]) * sign <= 0.0 {
        return None;
    }
    triangles.push([polygon[0], polygon[1], polygon[2]]);
    Some(triangles)
}

/// A single mesh assembled from colored blocks, useful for scenery and raised interface panels.
#[derive(Default)]
pub struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Geometry {
    pub fn cuboid(&mut self, center: Vec3, size: Vec3, color: [u8; 3]) {
        for (axis, sign) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1)] {
            self.face(center, size, axis, sign, color);
        }
    }

    fn face(&mut self, center: Vec3, size: Vec3, axis: usize, sign: i32, color: [u8; 3]) {
        let a = (axis + 1) % 3;
        let b = (axis + 2) % 3;
        let mut normal = Vec3::ZERO;
        normal[axis] = sign as f32;
        let base = self.positions.len() as u32;
        let rgba = Color::srgb_u8(color[0], color[1], color[2])
            .to_linear()
            .to_f32_array();
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let mut position = center;
            position[axis] += sign as f32 * size[axis] * 0.5;
            position[a] += u * size[a] * 0.5;
            position[b] += v * size[b] * 0.5;
            self.positions.push(position.to_array());
            self.normals.push(normal.to_array());
            self.colors.push(rgba);
        }
        let winding = if sign > 0 {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        self.indices.extend(winding.map(|index| base + index));
    }

    fn quad(&mut self, positions: [Vec3; 4], color: [u8; 3]) {
        let base = self.positions.len() as u32;
        let normal = (positions[1] - positions[0])
            .cross(positions[2] - positions[0])
            .normalize();
        let rgba = Color::srgb_u8(color[0], color[1], color[2])
            .to_linear()
            .to_f32_array();
        self.positions.extend(positions.map(|p| p.to_array()));
        self.normals.extend([normal.to_array(); 4]);
        self.colors.extend([rgba; 4]);
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    }

    pub(crate) fn triangle(&mut self, positions: [Vec3; 3], color: [u8; 3]) {
        let base = self.positions.len() as u32;
        let normal = (positions[1] - positions[0])
            .cross(positions[2] - positions[0])
            .normalize();
        let rgba = Color::srgb_u8(color[0], color[1], color[2])
            .to_linear()
            .to_f32_array();
        self.positions.extend(positions.map(|p| p.to_array()));
        self.normals.extend([normal.to_array(); 3]);
        self.colors.extend([rgba; 3]);
        self.indices.extend([base, base + 1, base + 2]);
    }

    pub fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            // The compute renderer packs its own geometry buffers from CPU
            // meshes. Bevy's raster mesh upload would duplicate this storage.
            RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_reduction_preserves_closed_boundaries_normals_and_colors() {
        let mut model = VoxelModel::default();
        for x in -3..=3 {
            for z in -3..=3 {
                for y in 0..if z < 0 { 3 } else { 1 } {
                    model.set(
                        [x, y, z],
                        if x < 1 { [180, 150, 90] } else { [90, 130, 80] },
                    );
                }
            }
        }
        let original = model.mesh_with_style(0.13, SurfaceStyle::Beveled);
        let reduced = model.mesh_with_flat_normals(0.13, SurfaceStyle::Beveled);
        assert!(reduced.indices().unwrap().len() < original.indices().unwrap().len());
        assert_closed_and_valid(&reduced);
        let positions = |mesh: &Mesh| {
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap()
                .to_vec()
        };
        let original_positions = positions(&original);
        assert!(
            positions(&reduced)
                .iter()
                .all(|p| original_positions.contains(p))
        );

        // Compare area on every oriented colored plane, including all unchanged
        // bevel facets. Retriangulation must not change appearance attributes.
        let planes = |mesh: &Mesh| {
            let positions = positions(mesh);
            let normals = mesh
                .attribute(Mesh::ATTRIBUTE_NORMAL)
                .unwrap()
                .as_float3()
                .unwrap();
            let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
            else {
                panic!("colors")
            };
            let mut result = BTreeMap::<([u32; 3], u32, [u32; 4]), f64>::new();
            let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
            for tri in indices.as_chunks::<3>().0.iter() {
                let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from(positions[i]));
                let mut normal = Vec3::from(normals[tri[0]]);
                if normal.to_array().iter().filter(|&&v| v != 0.0).count() == 1 {
                    // f32 normalization can yield 0.99999994 on an axis. Larger
                    // triangles change that rounding, not the supporting plane.
                    // Bound the normal error explicitly before canonicalizing
                    // this exact axis direction for the plane-area comparison.
                    let axis_normal = normal.map(|v| if v == 0.0 { 0.0 } else { v.signum() });
                    assert!((normal - axis_normal).abs().max_element() <= f32::EPSILON);
                    normal = axis_normal;
                }
                let canonical = |v: f32| if v == 0.0 { 0 } else { v.to_bits() };
                let key = (
                    normal.to_array().map(canonical),
                    canonical(normal.dot(a)),
                    colors[tri[0]].map(canonical),
                );
                *result.entry(key).or_default() += f64::from((b - a).cross(c - a).length()) * 0.5;
            }
            result
        };
        let original_planes = planes(&original);
        let reduced_planes = planes(&reduced);
        assert_eq!(original_planes.len(), reduced_planes.len());
        for (key, area) in original_planes {
            let reduced_area = reduced_planes[&key];
            assert!(
                (area - reduced_area).abs() < 1e-6,
                "plane area changed: normal {:?}, distance {}, color {:?}, {area} -> {reduced_area}",
                key.0.map(f32::from_bits),
                f32::from_bits(key.1),
                key.2.map(f32::from_bits)
            );
        }
    }

    #[test]
    fn nonplanar_bevel_faces_and_other_styles_keep_their_triangles() {
        let mut model = VoxelModel::default();
        model.set([0, 0, 0], [150, 100, 80]);
        for style in [
            SurfaceStyle::Sharp,
            SurfaceStyle::Beveled,
            SurfaceStyle::Separated,
        ] {
            let original = model.mesh_with_style(0.13, style);
            let reduced = model.mesh_with_flat_normals(0.13, style);
            assert_eq!(original.indices().unwrap(), reduced.indices().unwrap());
            for attribute in [
                Mesh::ATTRIBUTE_POSITION,
                Mesh::ATTRIBUTE_NORMAL,
                Mesh::ATTRIBUTE_COLOR,
            ] {
                assert_eq!(
                    original.attribute(attribute).unwrap(),
                    reduced.attribute(attribute).unwrap()
                );
            }
        }
    }

    #[test]
    fn adjacent_voxels_remove_their_shared_faces() {
        let mut model = VoxelModel::default();
        model.set([0, 0, 0], [255; 3]);
        model.set([1, 0, 0], [255; 3]);
        let mesh = model.mesh(1.0);
        assert_eq!(mesh.count_vertices(), 40);
        assert_eq!(mesh.indices().unwrap().len(), 60);
    }

    #[test]
    fn face_winding_matches_outward_normals() {
        let mut model = VoxelModel::default();
        model.set([0, 0, 0], [255; 3]);
        let mesh = model.mesh(1.0);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        let indices: Vec<usize> = mesh.indices().unwrap().iter().collect();
        for triangle in indices.as_chunks::<3>().0.iter() {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from(positions[i]));
            assert!((b - a).cross(c - a).dot(Vec3::from(normals[triangle[0]])) > 0.0);
        }
    }
    fn assert_closed_and_valid(mesh: &Mesh) {
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        let mut edges = BTreeMap::<([u32; 3], [u32; 3]), usize>::new();
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        let mut volume = 0.0;
        for triangle in indices.as_chunks::<3>().0.iter() {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from(positions[i]));
            let cross = (b - a).cross(c - a);
            assert!(
                cross.length_squared() > 1e-12,
                "degenerate face at {a:?}, {b:?}, {c:?}"
            );
            for &i in triangle {
                let normal = Vec3::from(normals[i]);
                assert!(normal.is_finite());
                assert!((normal.length() - 1.0).abs() < 1e-5);
                assert!(cross.dot(normal) > 0.0);
            }
            volume += a.dot(b.cross(c)) / 6.0;
            for (from, to) in [(a, b), (b, c), (c, a)] {
                // Both sides of every edge must use bit-identical world coordinates.
                let key = [from, to]
                    .map(|p| p.to_array().map(|v| if v == 0.0 { 0 } else { v.to_bits() }));
                *edges.entry((key[0], key[1])).or_default() += 1;
            }
        }
        assert!(volume > 0.0);
        for (&(from, to), &count) in &edges {
            assert_eq!(count, 1, "duplicate oriented edge");
            assert_eq!(edges.get(&(to, from)), Some(&1), "surface has an open edge");
        }
    }

    #[test]
    fn bevels_keep_convex_concave_and_color_boundaries_closed() {
        for cells in [
            vec![[0, 0, 0]],
            vec![[0, 0, 0], [1, 0, 0]],
            vec![[0, 0, 0], [1, 0, 0], [0, 1, 0]],
            vec![[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]],
        ] {
            let mut model = VoxelModel::default();
            for (index, cell) in cells.into_iter().enumerate() {
                model.set(cell, [index as u8 * 50, 100, 200]);
            }
            for style in [
                SurfaceStyle::Sharp,
                SurfaceStyle::Beveled,
                SurfaceStyle::Separated,
            ] {
                assert_closed_and_valid(&model.mesh_with_style(0.13, style));
            }
        }
    }

    #[test]
    fn bevels_preserve_coplanar_surfaces_across_color_changes() {
        let mut model = VoxelModel::default();
        for x in -2..=2 {
            for y in -2..=2 {
                model.set([x, y, 0], if x < 1 { [255; 3] } else { [120; 3] });
            }
        }
        let mesh = model.mesh_with_style(1.0, SurfaceStyle::Beveled);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        let mut seam_vertices = 0;
        for (p, n) in positions.iter().zip(normals) {
            if p[0] == 0.5 && p[1].abs() < 1.0 && p[2] > 0.0 {
                assert_eq!(p[2], 0.5);
                assert_eq!(*n, [0.0, 0.0, 1.0]);
                seam_vertices += 1;
            }
        }
        assert!(seam_vertices > 0);
    }

    #[test]
    fn bevels_add_real_facets_with_a_bounded_silhouette_change() {
        let mut model = VoxelModel::default();
        model.set([0, 0, 0], [255; 3]);
        let mesh = model.mesh_with_style(1.0, SurfaceStyle::Beveled);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(
            normals
                .iter()
                .any(|n| n.iter().filter(|v| v.abs() > 0.1).count() > 1)
        );
        assert!(positions.iter().all(|p| p.iter().all(|v| v.abs() <= 0.5)));
        assert!(positions.contains(&[0.5, 0.38, 0.38]));
        assert!(
            positions
                .iter()
                .any(|p| p.iter().all(|v| (v.abs() - 0.46).abs() < 1e-6))
        );
    }
    #[test]
    fn stepped_ellipsoids_remain_closed_after_beveling() {
        for radii in [[2, 2, 1], [5, 4, 3], [7, 6, 4]] {
            let model = VoxelModel::ellipsoid(radii, |_| [160, 100, 200]);
            assert_closed_and_valid(&model.mesh_with_style(0.13, SurfaceStyle::Beveled));
        }
    }
    #[test]
    fn long_terrace_creases_do_not_divot_at_cell_boundaries() {
        let mut model = VoxelModel::default();
        // A long upper shelf meeting a lower flat bed exercises both convex and
        // concave horizontal creases without introducing real contour turns.
        for x in -12..=12 {
            for z in -3..=3 {
                for y in 0..if z < 0 { 3 } else { 1 } {
                    model.set([x, y, z], [100; 3]);
                }
            }
        }
        let mesh = model.mesh_with_style(0.1, SurfaceStyle::Beveled);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        let mut profiles = BTreeMap::<i32, BTreeSet<(i32, i32)>>::new();
        for (p, n) in positions.iter().zip(normals) {
            if p[0].abs() < 0.9 {
                // Extruded faces must have no normal component along the straight
                // crease. A boundary divot would tilt these faces toward +/- X.
                assert!(n[0].abs() < 1e-5, "tilted face at {p:?}: {n:?}");
                profiles
                    .entry((p[0] * 100_000.0).round() as i32)
                    .or_default()
                    .insert((
                        (p[1] * 100_000.0).round() as i32,
                        (p[2] * 100_000.0).round() as i32,
                    ));
            }
        }
        let reference = profiles.values().next().unwrap();
        assert!(profiles.len() > 20);
        assert!(profiles.values().all(|profile| profile == reference));
        assert_closed_and_valid(&mesh);
    }
}
