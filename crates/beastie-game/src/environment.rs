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
    key_illuminance: 4200.0,
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
) {
    commands.insert_resource(ClearColor(Color::srgb_u8(7, 18, 25)));
    commands.spawn((
        DirectionalLight {
            illuminance: TANK.key_illuminance,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.0, 7.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.40, 0.85, 0.95),
            intensity: TANK.fill_intensity,
            range: 30.0,
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
        Mesh3d(meshes.add(back.mesh())),
        MeshMaterial3d(solid.clone()),
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
    for x in 0..98 {
        for z in 0..18 {
            let hash = (x * 31 + z * 73 + x * z * 7) % 19;
            let height = 0.06 + hash as f32 * 0.004;
            tank.cuboid(
                Vec3::new(
                    -7.7 + x as f32 * 0.158,
                    -1.97 + height * 0.5,
                    -2.0 + z as f32 * 0.22,
                ),
                Vec3::new(0.154, height, 0.21),
                shade(TANK.sand, hash as i16 - 8),
            );
        }
    }
    for column in 0..72 {
        let x = -7.6 + column as f32 * 0.212;
        let ridge = 0.42 + ((x * 0.62).sin() * 0.5 + 0.5) * 0.27;
        for depth in 0..7 {
            let z = -2.0 + depth as f32 * 0.20;
            let height = ridge * (1.0 - depth as f32 / 9.0);
            let shade = ((column * 19 + depth * 11) % 17) as u8;
            tank.cuboid(
                Vec3::new(x, -1.9 + height * 0.5, z),
                Vec3::new(0.209, height, 0.198),
                [79 + shade, 99 + shade, 84 + shade],
            );
        }
    }
    // Sparse rear clusters frame the creature's open swimming space. These are scenery,
    // not extra simulation objects: the familiar cave and plant remain the only targets.
    let mut garden = Geometry::default();
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
        garden_stone(&mut garden, center, radius, color);
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
    // Three receding shelves create an oblique depth cue with a straight-on camera.
    // Keep the middle water column empty so scenery cannot compete with the face.
    for side in [-1.0, 1.0] {
        for step in 0..5 {
            let width = 1.15 - step as f32 * 0.14;
            garden.cuboid(
                Vec3::new(
                    side * (7.25 - step as f32 * 0.12),
                    -1.72 + step as f32 * 0.12,
                    -1.90,
                ),
                Vec3::new(width, 0.22, 0.55 - step as f32 * 0.055),
                shade(TANK.stone, step * 3 - 6),
            );
        }
    }
    commands.spawn((
        bevy::light::NotShadowCaster,
        Mesh3d(meshes.add(garden.mesh())),
        MeshMaterial3d(solid.clone()),
        Transform::default(),
    ));
    commands.spawn((
        bevy::light::NotShadowCaster,
        Mesh3d(meshes.add(tank.mesh())),
        MeshMaterial3d(solid.clone()),
        Transform::default(),
    ));
}

fn garden_stone(shape: &mut Geometry, center: Vec3, radius: Vec3, color: [u8; 3]) {
    let cell = 0.10;
    let extent = (radius / cell).ceil().as_ivec3();
    for x in -extent.x..=extent.x {
        for y in -extent.y..=extent.y {
            for z in -extent.z..=extent.z {
                let offset = Vec3::new(x as f32, y as f32, z as f32) * cell;
                if (offset / radius).length_squared() <= 1.0 {
                    let variation = (x * 13 + y * 7 + z * 17).rem_euclid(7) as u8;
                    shape.cuboid(
                        center + offset,
                        Vec3::splat(cell),
                        color.map(|channel| channel.saturating_add(variation)),
                    );
                }
            }
        }
    }
}

fn garden_frond(shape: &mut Geometry, root: Vec3, height: f32, bend: f32, color: [u8; 3]) {
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
}

pub(crate) fn object_mesh(kind: ObjectKind) -> Mesh {
    match kind {
        ObjectKind::Food(food) => food_mesh(food),
        ObjectKind::Toy(ToyId::Ball) => VoxelModel::ellipsoid([7, 7, 7], |p| {
            if p[1].abs() < 2 {
                [240, 197, 120]
            } else if p[0] > 1 {
                [111, 177, 163]
            } else {
                [212, 119, 92]
            }
        })
        .mesh(0.055),
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
            shape.mesh(0.052)
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
                for y in -13..17 {
                    for z in -10..=4 {
                        let outer =
                            y < 0 || (x as f32 / 12.0).powi(2) + (y as f32 / 17.0).powi(2) <= 1.0;
                        let doorway = x.abs() < 7 && y < 11 && z > -9;
                        if outer && !doorway {
                            let shade = (x * 17 + y * 31 + z * 7_i32).rem_euclid(13) as u8;
                            let color = if x.abs() < 7 && y < 11 && z <= -9 {
                                [19 + shade / 3, 35 + shade / 3, 36 + shade / 3]
                            } else {
                                let top = y > 10 && z > -3 && (x * 7 + z * 3).rem_euclid(9) < 4;
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
            shape.mesh(0.075)
        }
        ObjectKind::Plant => {
            let mut shape = Geometry::default();
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
            );
            shape.mesh()
        }
    }
}

/// Food silhouettes stay distinct even without color: berry cluster, cap/stem, pellet.
fn food_mesh(food: FoodId) -> Mesh {
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
    shape.mesh(0.045)
}
