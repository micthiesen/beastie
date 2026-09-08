//! Colored solid geometry. Interior voxel faces are omitted before upload.

use std::collections::BTreeMap;

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};

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

    pub fn mesh(&self, cell_size: f32) -> Mesh {
        let mut geometry = Geometry::default();
        for (&point, &color) in &self.cells {
            for (axis, sign) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1)] {
                let mut neighbor = point;
                neighbor[axis] += sign;
                if self.cells.contains_key(&neighbor) {
                    continue;
                }
                let center = Vec3::from_array(point.map(|v| v as f32 * cell_size));
                geometry.face(center, Vec3::splat(cell_size), axis, sign, color);
            }
        }
        geometry.mesh()
    }
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

    pub fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
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
        for triangle in indices.chunks_exact(3) {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from(positions[i]));
            assert!((b - a).cross(c - a).dot(Vec3::from(normals[triangle[0]])) > 0.0);
        }
    }
}
