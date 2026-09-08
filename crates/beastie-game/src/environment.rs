//! Tank art and reusable object definitions. These shapes never alter simulation or picking.
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
    stone: [u8; 3],
    moss: [u8; 3],
    key_illuminance: f32,
    fill_intensity: f32,
}
const TANK: TankArt = TankArt {
    back_low: [13, 37, 47],
    back_high: [26, 66, 76],
    frame: [38, 64, 67],
    brass: [164, 156, 113],
    sand: [112, 119, 93],
    stone: [67, 94, 94],
    moss: [67, 116, 86],
    key_illuminance: 3200.0,
    fill_intensity: 115_000.0,
};

#[derive(Clone, Copy)]
struct FrondArt {
    root: Vec3,
    height: f32,
    bend: f32,
    color: [u8; 3],
}
#[allow(clippy::approx_constant)] // Authored spatial coordinates, not angles.
const REAR_FRONDS: [FrondArt; 8] = [
    FrondArt {
        root: Vec3::new(-7.05, -1.9, -1.65),
        height: 2.3,
        bend: 0.48,
        color: [35, 78, 75],
    },
    FrondArt {
        root: Vec3::new(-6.72, -1.9, -1.40),
        height: 1.8,
        bend: -0.32,
        color: [43, 94, 80],
    },
    FrondArt {
        root: Vec3::new(-6.28, -1.9, -1.05),
        height: 1.5,
        bend: 0.3,
        color: [57, 111, 89],
    },
    FrondArt {
        root: Vec3::new(-7.30, -1.9, -0.72),
        height: 1.1,
        bend: 0.24,
        color: [65, 120, 91],
    },
    FrondArt {
        root: Vec3::new(6.85, -1.9, -1.62),
        height: 2.7,
        bend: -0.5,
        color: [33, 78, 77],
    },
    FrondArt {
        root: Vec3::new(6.52, -1.9, -1.30),
        height: 2.05,
        bend: 0.36,
        color: [43, 98, 85],
    },
    FrondArt {
        root: Vec3::new(6.05, -1.9, -1.05),
        height: 1.42,
        bend: -0.25,
        color: [57, 111, 88],
    },
    FrondArt {
        root: Vec3::new(7.15, -1.9, -0.65),
        height: 1.05,
        bend: -0.32,
        color: [69, 121, 93],
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
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.89, 0.74),
            illuminance: TANK.key_illuminance,
            shadow_depth_bias: 0.06,
            shadow_normal_bias: 2.5,
            shadow_maps_enabled: appearance.shadows(),
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 15.0,
            maximum_distance: 35.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-3.0, 7.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.40, 0.85, 0.95),
            intensity: TANK.fill_intensity,
            range: 30.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(6.0, 4.0, 5.0),
    ));
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
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
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
    // The substrate and rear ridge are one filled lattice. Shared faces disappear,
    // leaving terraces in the silhouette instead of dark channels around each grain.
    commands.spawn((
        bevy::light::NotShadowCaster,
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
            Vec3::new(-6.75, -1.67, -0.9),
            Vec3::new(0.75, 0.30, 0.48),
            [77, 103, 103],
        ),
        (
            Vec3::new(-6.16, -1.76, -0.55),
            Vec3::new(0.39, 0.17, 0.35),
            [111, 122, 109],
        ),
        (
            Vec3::new(-5.86, -1.79, 0.18),
            Vec3::new(0.23, 0.13, 0.25),
            [125, 132, 110],
        ),
        (
            Vec3::new(6.56, -1.55, -1.0),
            Vec3::new(0.73, 0.47, 0.43),
            [72, 97, 101],
        ),
        (
            Vec3::new(5.95, -1.74, -0.36),
            Vec3::new(0.48, 0.23, 0.38),
            [108, 119, 111],
        ),
        (
            Vec3::new(5.48, -1.79, 0.17),
            Vec3::new(0.25, 0.13, 0.23),
            [133, 139, 113],
        ),
        (
            Vec3::new(-1.35, -1.83, 0.4),
            Vec3::new(0.17, 0.10, 0.16),
            [140, 139, 109],
        ),
        (
            Vec3::new(2.38, -1.82, -0.1),
            Vec3::new(0.21, 0.11, 0.18),
            [117, 126, 103],
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
        );
    }
    // Joined shelves frame the scene; their stepped outer contour is intentional.
    for side in [-1.0, 1.0] {
        append_voxels(
            &mut garden,
            &shelf_model(),
            0.05,
            Vec3::new(side * 7.05, -1.8, -1.90),
            appearance.style(),
        );
    }
    commands.spawn((
        bevy::light::NotShadowCaster,
        Mesh3d(meshes.add(appearance.mesh(garden))),
        MeshMaterial3d(solid.clone()),
        Transform::default(),
    ));
    commands.spawn((
        bevy::light::NotShadowCaster,
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

fn garden_frond(mesh: &mut Mesh, root: Vec3, height: f32, bend: f32, color: [u8; 3]) {
    let mut shape = Geometry::default();
    let segments = (height / 0.075).ceil() as u32;
    for segment in 0..segments {
        let t = segment as f32 / segments as f32;
        let center = root
            + Vec3::new(
                bend * t * t + (t * 4.5).sin() * 0.045,
                t * height,
                (t * 3.2).sin() * 0.16,
            );
        let width = 0.065 + (std::f32::consts::PI * t).sin() * 0.085;
        shape.cuboid(center, Vec3::new(width, 0.083, 0.055), color);
        if segment > 3 && segment % 5 == 0 {
            let side = if segment % 10 == 0 { -1.0 } else { 1.0 };
            for leaf in 1..=3 {
                shape.cuboid(
                    center
                        + Vec3::new(
                            side * leaf as f32 * 0.055,
                            leaf as f32 * 0.025,
                            side * leaf as f32 * 0.025,
                        ),
                    Vec3::new(0.075, 0.040, 0.050),
                    color.map(|channel| channel.saturating_add(9)),
                );
            }
        }
    }
    mesh.merge(&shape.mesh())
        .expect("scenery meshes share colored triangle attributes");
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

fn shelf_model() -> VoxelModel {
    let mut model = VoxelModel::default();
    for y in 0..13 {
        let step = y / 3;
        let half_width = 12 - step * 2;
        let half_depth = 5 - step / 2;
        for x in -half_width..=half_width {
            for z in -half_depth..=half_depth {
                model.set([x, y, z], shade(TANK.stone, step as i16 * 2 - 4));
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
                [240, 197, 120]
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
            shape.set([0, 8, 0], [103, 103, 81]);
            shape.mesh_with_style(0.052, style)
        }
        ObjectKind::Toy(ToyId::Sock) => {
            let mut geometry = Geometry::default();
            geometry.cuboid(
                Vec3::new(0.0, 0.12, 0.0),
                Vec3::new(0.28, 0.48, 0.2),
                [159, 137, 168],
            );
            geometry.cuboid(
                Vec3::new(0.15, -0.12, 0.0),
                Vec3::new(0.55, 0.22, 0.22),
                [183, 159, 181],
            );
            geometry.cuboid(
                Vec3::new(0.0, 0.3, 0.0),
                Vec3::new(0.3, 0.07, 0.22),
                [220, 196, 183],
            );
            for y in [-0.02, 0.08, 0.18] {
                geometry.cuboid(
                    Vec3::new(0.0, y, 0.111),
                    Vec3::new(0.285, 0.032, 0.025),
                    [111, 100, 134],
                );
            }
            geometry.cuboid(
                Vec3::new(0.35, -0.12, 0.0),
                Vec3::new(0.16, 0.225, 0.225),
                [127, 111, 145],
            );
            geometry.cuboid(
                Vec3::new(-0.07, -0.12, 0.113),
                Vec3::new(0.14, 0.14, 0.026),
                [217, 183, 155],
            );
            geometry.mesh()
        }
        ObjectKind::Cave => {
            let mut shape = VoxelModel::default();
            for x in -12_i32..=12 {
                for y in -13_i32..17 {
                    for z in -10_i32..=4 {
                        let outer =
                            y < 0 || (x as f32 / 12.0).powi(2) + (y as f32 / 17.0).powi(2) <= 1.0;
                        let doorway = x.abs() < 7 && y < 11 && z > -9;
                        if outer && !doorway {
                            let shade = ((x.div_euclid(5) + y.div_euclid(7) + z.div_euclid(6))
                                .rem_euclid(3)
                                * 3) as u8;
                            let color = if x.abs() < 7 && y < 11 && z <= -9 {
                                [19 + shade / 3, 35 + shade / 3, 36 + shade / 3]
                            } else {
                                let top = y > 10
                                    && z > -3
                                    && (x.div_euclid(5) + z.div_euclid(4)).rem_euclid(3) < 2;
                                if top {
                                    TANK.moss.map(|c| c.saturating_add(shade))
                                } else {
                                    TANK.stone.map(|c| c.saturating_add(shade))
                                }
                            };
                            shape.set([x, y - 6, z], color);
                        }
                    }
                }
            }
            shape.mesh_with_style(0.075, style)
        }
        ObjectKind::Plant => {
            let mut shape = Geometry::default().mesh();
            // At the authoritative plant center, y=-1.31 reaches the substrate. Narrow
            // curved ribbons rise from that root rather than floating as a leaf lattice.
            for stem in 0..7 {
                let angle = stem as f32 * 1.35;
                let height = 1.35 + (stem % 3) as f32 * 0.17;
                let bend = angle.sin() * 0.38;
                garden_frond(
                    &mut shape,
                    Vec3::new(angle.sin() * 0.10, -1.31, angle.cos() * 0.15),
                    height,
                    bend,
                    [55 + stem * 3, 119 + stem * 4, 92 + stem * 2],
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
