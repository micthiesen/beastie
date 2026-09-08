//! Tank art and reusable object definitions. Shapes preserve authoritative anchors;
//! scenery picking uses the same meshes.
use crate::voxel::{Geometry, VoxelModel};
use beastie_core::{FoodId, ToyId};
use beastie_view::ObjectKind;
use bevy::prelude::*;

/// Palette stays quieter and cooler than the creature and the interactive toys.
/// Depth is expressed with solid stepped forms, never transparent picture planes.
#[derive(Clone, Copy)]
struct TankArt {
    back_low: [u8; 3],
    back_high: [u8; 3],
    frame: [u8; 3],
    brass: [u8; 3],
    sand: [u8; 3],
}
const TANK: TankArt = TankArt {
    back_low: [13, 37, 47],
    back_high: [26, 66, 76],
    frame: [38, 64, 67],
    brass: [164, 156, 113],
    sand: [112, 119, 93],
};

#[derive(Clone, Copy)]
struct FrondArt {
    root: Vec3,
    height: f32,
    bend: f32,
    color: [u8; 3],
}
// A tall right group balances the low shelter on the left. The central water stays clear.
const REAR_FRONDS: [FrondArt; 5] = [
    FrondArt {
        root: Vec3::new(-6.88, -1.83, -1.45),
        height: 1.30,
        bend: -0.30,
        color: [49, 93, 77],
    },
    FrondArt {
        root: Vec3::new(-6.70, -1.83, -1.10),
        height: 0.85,
        bend: 0.42,
        color: [68, 112, 86],
    },
    FrondArt {
        root: Vec3::new(6.67, -1.83, -1.65),
        height: 2.65,
        bend: -0.65,
        color: [56, 105, 89],
    },
    FrondArt {
        root: Vec3::new(6.83, -1.83, -1.28),
        height: 2.08,
        bend: 0.38,
        color: [54, 105, 85],
    },
    FrondArt {
        root: Vec3::new(6.48, -1.83, -0.90),
        height: 1.32,
        bend: -0.61,
        color: [79, 126, 92],
    },
];

fn shade(color: [u8; 3], amount: i16) -> [u8; 3] {
    color.map(|channel| (i16::from(channel) + amount).clamp(0, 255) as u8)
}

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    solid: Handle<StandardMaterial>,
    appearance: crate::appearance::RenderAppearance,
    background: Handle<StandardMaterial>,
) {
    commands.insert_resource(ClearColor(Color::srgb_u8(7, 18, 25)));
    let mut back = Geometry::default();
    // Layered teal back wall, brass-dark structural frame, and a stepped stone bed.
    for row in 0..26 {
        let t = row as f32 / 25.0;
        let color = std::array::from_fn(|axis| {
            (TANK.back_low[axis] as f32 * (1.0 - t) + TANK.back_high[axis] as f32 * t) as u8
        });
        back.cuboid(
            Vec3::new(0.0, -2.0 + row as f32 * 0.25, -2.6),
            Vec3::new(15.95, 0.255, 0.3),
            color,
        );
    }
    commands.spawn((
        Mesh3d(meshes.add(appearance.mesh(back.mesh()))),
        MeshMaterial3d(background),
        Transform::default(),
    ));
    let mut tank = Geometry::default();
    tank.cuboid(
        Vec3::new(0.0, -2.2, -0.3),
        Vec3::new(16.0, 0.32, 4.8),
        [63, 73, 68],
    );
    for side in [-1.0, 1.0] {
        tank.cuboid(
            Vec3::new(side * 7.88, 1.1, -0.2),
            Vec3::new(0.18, 6.65, 4.7),
            TANK.frame,
        );
        tank.cuboid(
            Vec3::new(side * 7.75, 1.1, 2.1),
            Vec3::new(0.045, 6.65, 0.08),
            TANK.brass,
        );
    }
    tank.cuboid(
        Vec3::new(0.0, 4.35, -0.2),
        Vec3::new(16.0, 0.2, 4.7),
        TANK.frame,
    );
    tank.cuboid(
        Vec3::new(0.0, 4.21, 2.1),
        Vec3::new(15.65, 0.055, 0.07),
        shade(TANK.brass, 15),
    );
    // A solid cabinet apron remains visible beneath the bed as the compact care
    // controls recede. It is part of the tank, so no empty background gap is exposed.
    tank.cuboid(
        Vec3::new(0.0, -2.95, 2.15),
        Vec3::new(16.0, 1.35, 0.30),
        [61, 65, 58],
    );
    tank.cuboid(
        Vec3::new(0.0, -2.29, 2.20),
        Vec3::new(16.0, 0.065, 0.34),
        [129, 125, 99],
    );
    // The substrate and rear ridge are one filled lattice. Shared faces disappear,
    // leaving terraces in the silhouette instead of dark channels around each grain.
    commands.spawn((
        Mesh3d(
            meshes.add(appearance.mesh(substrate_model().mesh_with_style(
                0.10,
                match appearance.treatment {
                    crate::appearance::SurfaceTreatment::Separated => {
                        crate::voxel::SurfaceStyle::Separated
                    }
                    _ => crate::voxel::SurfaceStyle::Sharp,
                },
            ))),
        ),
        MeshMaterial3d(solid.clone()),
        Transform::from_xyz(0.0, -2.0, 0.0),
    ));
    // Sparse rear clusters frame the creature's open swimming space. These are scenery,
    // not extra simulation objects: the familiar cave and plant remain the only targets.
    let mut garden = Geometry::default().mesh();
    for (center, radius, color) in [
        (
            Vec3::new(-6.85, -1.70, -0.85),
            Vec3::new(0.72, 0.22, 0.45),
            [88, 108, 102],
        ),
        (
            Vec3::new(-6.20, -1.77, -0.45),
            Vec3::new(0.47, 0.15, 0.35),
            [118, 127, 110],
        ),
        (
            Vec3::new(-5.90, -1.82, 0.08),
            Vec3::new(0.20, 0.09, 0.19),
            [143, 141, 115],
        ),
        (
            Vec3::new(6.80, -1.72, -0.95),
            Vec3::new(0.54, 0.24, 0.36),
            [83, 105, 99],
        ),
        (
            Vec3::new(6.16, -1.80, -0.56),
            Vec3::new(0.27, 0.12, 0.23),
            [117, 129, 109],
        ),
    ] {
        garden_stone(&mut garden, center, radius, color, appearance.style());
    }
    for frond in REAR_FRONDS {
        garden_frond(
            &mut garden,
            frond.root,
            frond.height,
            frond.bend,
            frond.color,
            appearance.style(),
        );
    }
    commands.spawn((
        Mesh3d(meshes.add(appearance.mesh(garden))),
        MeshMaterial3d(solid.clone()),
        Transform::default(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(appearance.mesh(tank.mesh()))),
        MeshMaterial3d(solid.clone()),
        Transform::default(),
    ));
}

fn garden_stone(
    shape: &mut Mesh,
    center: Vec3,
    radius: Vec3,
    color: [u8; 3],
    style: crate::voxel::SurfaceStyle,
) {
    let cell = 0.10;
    let extent = (radius / cell).ceil().as_ivec3();
    let mut stone = VoxelModel::default();
    for x in -extent.x..=extent.x {
        for y in -extent.y..=extent.y {
            for z in -extent.z..=extent.z {
                let offset = Vec3::new(x as f32, y as f32, z as f32) * cell;
                if (offset / radius).length_squared() <= 1.0 {
                    // Broad strata keep the form readable without a checkerboard of cells.
                    stone.set([x, y, z], shade(color, (y / 2).clamp(-2, 2) as i16));
                }
            }
        }
    }
    append_voxels(shape, &stone, cell, center, style);
}

/// One filled leaf lattice gives a broad silhouette without overlapping strip seams.
fn garden_frond(
    mesh: &mut Mesh,
    root: Vec3,
    height: f32,
    bend: f32,
    color: [u8; 3],
    style: crate::voxel::SurfaceStyle,
) {
    let cell = 0.045;
    let rows = (height / cell).ceil() as i32;
    let mut leaf = VoxelModel::default();
    for y in 0..=rows {
        let t = y as f32 / rows as f32;
        let center_x = (bend * t * t / cell).round() as i32;
        let center_z = ((t * std::f32::consts::PI).sin() * 0.12 / cell).round() as i32;
        let half_width = (1.0
            + (t * std::f32::consts::PI).sin() * 3.0 * (height / 1.5).clamp(0.7, 1.3))
        .round() as i32;
        for x in -half_width..=half_width {
            // A quiet midrib and folded edge communicate a leaf, not a painted stripe.
            let fold = i32::from(x.abs() == half_width && half_width > 1);
            for depth in 0..=1 {
                leaf.set(
                    [center_x + x, y, center_z - fold - depth],
                    shade(color, if x == 0 { 7 } else { 0 }),
                );
            }
            if fold > 0 {
                leaf.set([center_x + x, y, center_z], color);
            }
        }
    }
    append_voxels(mesh, &leaf, cell, root, style);
}

/// Integer occupancy makes the floor continuous even where terrace heights differ.
fn substrate_model() -> VoxelModel {
    let mut model = VoxelModel::default();
    for x in -78..=78 {
        for z in -21..=20 {
            let world_x = x as f32 * 0.10;
            let rear = ((-z + 2) as f32 / 23.0).clamp(0.0, 1.0);
            let side = ((world_x.abs() - 4.2) / 3.6).clamp(0.0, 1.0);
            let side = side * side * (3.0 - 2.0 * side);
            // Raised banks belong at the rear corners. Keep a broad, continuous
            // central bed so terrace highlights cannot stripe the toys and face.
            let bank = side * rear * rear;
            let height = 2 + (bank * (3.6 + (world_x * 0.8).sin() * 0.4)).round() as i32;
            let tone = ((world_x * 0.72 + z as f32 * 0.12).sin() * 1.5).round() as i16;
            let color = std::array::from_fn(|axis| {
                let sand = TANK.sand[axis] as f32;
                let bank_color = [96.0, 111.0, 95.0][axis];
                (sand + (bank_color - sand) * bank * 0.7).round() as u8
            });
            for y in 0..height {
                model.set([x, y, z], shade(color, tone));
            }
        }
    }
    model
}

fn append_voxels(
    mesh: &mut Mesh,
    model: &VoxelModel,
    cell: f32,
    offset: Vec3,
    style: crate::voxel::SurfaceStyle,
) {
    mesh.merge(&model.mesh_with_style(cell, style).translated_by(offset))
        .expect("voxel meshes share colored triangle attributes");
}

pub(crate) fn object_mesh(
    kind: ObjectKind,
    appearance: crate::appearance::RenderAppearance,
) -> Mesh {
    let style = appearance.style();
    match kind {
        ObjectKind::Food(food) => food_mesh(food, style),
        ObjectKind::Toy(ToyId::Ball) => VoxelModel::ellipsoid([7, 7, 7], |p| {
            if p[1].abs() < 2 {
                [210, 181, 132]
            } else if p[0] > 1 {
                [111, 177, 163]
            } else {
                [212, 119, 92]
            }
        })
        .mesh_with_style(0.055, style),
        ObjectKind::Toy(ToyId::Bell) => {
            let mut shape = VoxelModel::default();
            for y in 0..12 {
                let r = 7 - y / 3;
                for x in -r..=r {
                    for z in -r..=r {
                        if x * x + z * z <= r * r && (x * x + z * z >= (r - 2) * (r - 2) || y > 8) {
                            shape.set([x, y - 5, z], [202, 158, 72]);
                        }
                    }
                }
            }
            for x in -7_i32..=7 {
                for z in -7_i32..=7 {
                    if (35..=53).contains(&(x * x + z * z)) {
                        shape.set([x, -5, z], [230, 187, 100]);
                    }
                }
            }
            for y in -7..=-4 {
                shape.set([0, y, 0], [112, 89, 59]);
            }
            // Open suspension loop makes the bell read as an object built to ring.
            for x in -2_i32..=2 {
                for y in 7_i32..=11 {
                    if x.abs() == 2 || y == 7 || y == 11 {
                        shape.set([x, y, 0], [179, 135, 65]);
                    }
                }
            }
            // A small cork float is part of this toy, not an attachment to the tank.
            // It makes the bell's existing suspended position credible even in older saves.
            for x in -5_i32..=5 {
                for y in 12_i32..=15 {
                    for z in -3_i32..=3 {
                        if x * x + z * z <= 29 {
                            shape.set(
                                [x, y, z],
                                if y == 13 {
                                    [121, 94, 60]
                                } else {
                                    [185, 155, 103]
                                },
                            );
                        }
                    }
                }
            }
            shape.mesh_with_style(0.052, style)
        }
        ObjectKind::Toy(ToyId::Sock) => sock_model().mesh_with_style(0.043, style),
        ObjectKind::Cave => shelter_model().mesh_with_style(0.075, style),
        ObjectKind::Plant => {
            let mut shape = Geometry::default().mesh();
            // Broad leaves grow from the authoritative root, with a varied fan silhouette.
            for stem in 0..5 {
                let angle = stem as f32 * 1.35;
                let height = 1.10 + (stem % 3) as f32 * 0.23;
                let bend = angle.sin() * 0.45;
                garden_frond(
                    &mut shape,
                    Vec3::new(angle.sin() * 0.10, -1.31, angle.cos() * 0.15),
                    height,
                    bend,
                    [60 + stem * 4, 115 + stem * 4, 83 + stem * 2],
                    style,
                );
            }
            garden_stone(
                &mut shape,
                Vec3::new(0.0, -1.26, 0.0),
                Vec3::new(0.26, 0.09, 0.25),
                [108, 121, 95],
                style,
            );
            shape
        }
    }
}

/// The entrance surrounds the animal's full head and crown at its canonical rest anchor.
/// The barrel sits behind the interaction plane so a trailing body can peek out naturally.
fn shelter_model() -> VoxelModel {
    let mut shape = VoxelModel::default();
    for x in -17_i32..=17 {
        for y in -19_i32..=17 {
            for z in -18_i32..=-6 {
                let roof_y = (y + 6).max(0) as f32;
                let lip = z >= -7;
                let outer_radius = if lip { 17.0 } else { 16.0 };
                let outer = (x as f32 / outer_radius).powi(2) + (roof_y / 23.0).powi(2) <= 1.0;
                let opening_y = (y + 7).max(0) as f32;
                let inner_radius = if lip { 14.0 } else { 15.0 };
                let doorway = (x as f32 / inner_radius).powi(2) + (opening_y / 21.0).powi(2) < 1.0;
                if outer && (!doorway || z <= -17) {
                    let color = if doorway {
                        [51, 47, 39]
                    } else if lip {
                        [167, 116, 83]
                    } else if y > 0 {
                        [143, 98, 72]
                    } else {
                        [129, 89, 67]
                    };
                    shape.set([x, y, z], color);
                }
            }
        }
    }
    shape
}

fn sock_model() -> VoxelModel {
    let mut shape = VoxelModel::default();
    for x in -4_i32..=11 {
        for y in -6_i32..=10 {
            for z in -3_i32..=3 {
                let leg = x.abs() <= 3 && y >= -3 && x * x + z * z <= 16;
                let toe = ((x - 3) as f32 / 8.0).powi(2)
                    + ((y + 3) as f32 / 3.2).powi(2)
                    + (z as f32 / 3.5).powi(2)
                    <= 1.0;
                if leg || toe {
                    let color = if y >= 8 {
                        [209, 192, 169]
                    } else if x > 7 || x < -1 && y < -1 {
                        [123, 113, 136]
                    } else if y == 5 || y == 6 {
                        [171, 156, 176]
                    } else {
                        [148, 136, 158]
                    };
                    // Cloth bends under its own weight instead of standing like a boot.
                    // The cuff remains near the contact anchor in resting and carried poses.
                    let bend = ((10 - y) as f32 * 0.38).sin() * 3.0;
                    shape.set([x + bend.round() as i32, y - 8, z], color);
                }
            }
        }
    }
    shape
}

/// Food silhouettes stay distinct even without color: berry cluster, cap/stem, pellet.
fn food_mesh(food: FoodId, style: crate::voxel::SurfaceStyle) -> Mesh {
    let mut shape = VoxelModel::default();
    match food {
        FoodId::Berry => {
            shape = VoxelModel::ellipsoid([3, 3, 3], |p| {
                if p[0] < 0 && p[1] > 0 && p[2] > 1 {
                    [228, 115, 128]
                } else {
                    [191, 67, 92]
                }
            });
            for x in -2_i32..=2 {
                shape.set([x, 3, 0], [74, 126, 79]);
            }
            shape.set([0, 4, 0], [106, 139, 83]);
        }
        FoodId::Mushroom => {
            for x in -4_i32..=4 {
                for z in -4_i32..=4 {
                    for y in 0..=3 {
                        if x * x + z * z + y * y <= 18 {
                            shape.set(
                                [x, y, z],
                                if y == 0 {
                                    [148, 104, 79]
                                } else {
                                    [202, 150, 107]
                                },
                            );
                        }
                    }
                }
            }
            for y in -3..0 {
                for x in -1..=1 {
                    for z in -1..=1 {
                        shape.set([x, y, z], [219, 197, 151]);
                    }
                }
            }
        }
        FoodId::Pellet => {
            for x in -2..=2 {
                for y in -2..=2 {
                    for z in -2..=2 {
                        shape.set(
                            [x, y, z],
                            if y == 2 {
                                [177, 148, 91]
                            } else {
                                [134, 107, 64]
                            },
                        );
                    }
                }
            }
        }
    }
    shape.mesh_with_style(0.045, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substrate_has_a_complete_top_and_no_internal_base_walls() {
        let mesh = substrate_model().mesh(0.10);
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
        let mut top_area = 0.0_f64;
        for triangle in indices.chunks_exact(3) {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|index| Vec3::from(positions[index]));
            let normal = Vec3::from(normals[triangle[0]]);
            if normal.y > 0.99 {
                top_area += (b - a).cross(c - a).length() as f64 * 0.5;
            }
            let center = (a + b + c) / 3.0;
            if normal.y.abs() < 0.01 && center.y < 0.10 {
                assert!(
                    (center.x.abs() - 7.85).abs() < 0.001
                        || (center.z + 2.15).abs() < 0.001
                        || (center.z - 2.05).abs() < 0.001,
                    "unexpected internal substrate wall at {center:?}"
                );
            }
        }
        assert!((top_area - 15.7 * 4.2).abs() < 0.001);
    }
}
