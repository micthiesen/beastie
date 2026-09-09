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
    sand: [u8; 3],
}
const TANK: TankArt = TankArt {
    back_low: [8, 31, 39],
    back_high: [13, 57, 68],
    frame: [27, 42, 39],
    sand: [185, 164, 116],
};

#[derive(Clone, Copy)]
struct FrondArt {
    root: Vec3,
    height: f32,
    bend: f32,
    color: [u8; 3],
}
// A tall right group balances the low shelter on the left. The central water stays clear.
const REAR_FRONDS: [FrondArt; 11] = [
    FrondArt {
        root: Vec3::new(-6.88, -1.83, -1.45),
        height: 4.65,
        bend: 1.12,
        color: [63, 102, 54],
    },
    FrondArt {
        root: Vec3::new(-6.70, -1.83, -1.10),
        height: 3.25,
        bend: -0.43,
        color: [87, 119, 62],
    },
    FrondArt {
        root: Vec3::new(6.67, -1.83, -1.65),
        height: 4.15,
        bend: -1.12,
        color: [69, 111, 61],
    },
    FrondArt {
        root: Vec3::new(6.83, -1.83, -1.28),
        height: 3.55,
        bend: 0.78,
        color: [77, 113, 57],
    },
    FrondArt {
        root: Vec3::new(6.48, -1.83, -0.90),
        height: 2.65,
        bend: -0.61,
        color: [94, 130, 67],
    },
    FrondArt {
        root: Vec3::new(-7.35, -1.83, -1.7),
        height: 3.60,
        bend: -0.14,
        color: [44, 84, 57],
    },
    FrondArt {
        root: Vec3::new(7.24, -1.83, -1.85),
        height: 4.85,
        bend: -0.40,
        color: [47, 86, 54],
    },
    FrondArt {
        root: Vec3::new(5.94, -1.83, -1.30),
        height: 2.04,
        bend: -0.27,
        color: [83, 117, 64],
    },
    FrondArt {
        root: Vec3::new(-7.10, -1.83, -1.15),
        height: 2.90,
        bend: 0.78,
        color: [74, 112, 58],
    },
    FrondArt {
        root: Vec3::new(-6.56, -1.83, -1.95),
        height: 4.05,
        bend: -0.35,
        color: [45, 85, 56],
    },
    FrondArt {
        root: Vec3::new(6.56, -1.83, -1.00),
        height: 3.28,
        bend: 0.68,
        color: [78, 117, 56],
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
    frame_material: Handle<StandardMaterial>,
) {
    commands.insert_resource(ClearColor(Color::srgb_u8(7, 18, 25)));
    commands.spawn((
        Mesh3d(meshes.add(appearance.mesh(backdrop_mesh()))),
        MeshMaterial3d(background.clone()),
        Transform::default(),
    ));
    let mut tank = Geometry::default();
    tank.cuboid(
        Vec3::new(0.0, -2.2, -0.3),
        Vec3::new(15.94, 0.32, 4.8),
        [23, 37, 37],
    );
    // The rear wall structure ends behind one continuous front perimeter. The
    // four front members meet at their ends, without overlapping trim faces or
    // staggered front planes that read as stray lines at the bottom corners.
    for side in [-1.0, 1.0] {
        tank.cuboid(
            Vec3::new(side * 7.88, 1.30, -0.2),
            Vec3::new(0.18, 7.05, 4.7),
            TANK.frame,
        );
        tank.cuboid(
            Vec3::new(side * 7.87, 1.47, 2.225),
            Vec3::new(0.20, 7.66, 0.15),
            TANK.frame,
        );
    }
    // An opaque hood reaches above both camera crops. No water or clipped rear
    // posts should peek over the front crossbar.
    tank.cuboid(
        Vec3::new(0.0, 4.97, 2.225),
        Vec3::new(15.54, 0.66, 0.15),
        TANK.frame,
    );
    tank.cuboid(
        Vec3::new(0.0, -2.20, 2.225),
        Vec3::new(15.54, 0.32, 0.15),
        TANK.frame,
    );
    // The cabinet continues the sill's front plane without a projecting lip.
    tank.cuboid(
        Vec3::new(0.0, -3.035, 2.225),
        Vec3::new(15.94, 1.35, 0.15),
        TANK.frame,
    );
    // The substrate and rear ridge are one filled lattice. Shared faces disappear,
    // leaving terraces in the silhouette instead of dark channels around each grain.
    commands.spawn((
        Mesh3d(
            meshes.add(appearance.mesh(substrate_model().mesh_with_flat_normals(
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
    let garden = garden_mesh(appearance);
    commands.spawn((
        Mesh3d(meshes.add(appearance.mesh(garden))),
        MeshMaterial3d(solid.clone()),
        Transform::default(),
    ));
    let mut lamps = Geometry::default();
    for x in [-6.9, -4.2, 4.2, 6.9] {
        // A dark socket surrounds each warm diffuser. The lens sits behind its
        // front lip instead of hanging below a flat brown block.
        tank.cuboid(
            Vec3::new(x, 4.82, 2.32),
            Vec3::new(0.58, 0.22, 0.04),
            [35, 43, 37],
        );
        for side in [-1.0, 1.0] {
            tank.cuboid(
                Vec3::new(x + side * 0.255, 4.82, 2.365),
                Vec3::new(0.07, 0.22, 0.05),
                [49, 51, 39],
            );
            tank.cuboid(
                Vec3::new(x, 4.82 + side * 0.085, 2.365),
                Vec3::new(0.44, 0.05, 0.05),
                [49, 51, 39],
            );
        }
        lamps.cuboid(
            Vec3::new(x, 4.82, 2.345),
            Vec3::new(0.44, 0.12, 0.01),
            [255, 220, 146],
        );
    }
    commands.spawn((
        Mesh3d(meshes.add(lamps.mesh())),
        MeshMaterial3d(background.clone()),
        Transform::default(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(appearance.mesh(tank.mesh()))),
        MeshMaterial3d(frame_material),
        Transform::default(),
    ));
}

/// Distant decor keeps exact voxel occupancy, folds and authored color edges.
/// Its subpixel bevel bands otherwise split every face into nine quads, wasting
/// most of the aquarium BLAS budget. Two close foreground rocks retain bevels.
fn garden_mesh(appearance: crate::appearance::RenderAppearance) -> Mesh {
    let scenery_style = match appearance.treatment {
        crate::appearance::SurfaceTreatment::Separated => crate::voxel::SurfaceStyle::Separated,
        _ => crate::voxel::SurfaceStyle::Sharp,
    };
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
        garden_stone(&mut garden, center, radius, color, scenery_style);
    }
    // Layered corner outcrops are scenery, with clear water between their silhouettes.
    for (center, radius, color) in [
        (
            Vec3::new(-7.28, -0.65, -1.85),
            Vec3::new(0.64, 1.30, 0.44),
            [66, 88, 79],
        ),
        (
            Vec3::new(-6.18, -0.35, -1.94),
            Vec3::new(0.58, 1.62, 0.35),
            [56, 80, 75],
        ),
        (
            Vec3::new(-7.02, -1.52, 0.55),
            Vec3::new(0.76, 0.41, 0.48),
            [94, 110, 92],
        ),
        (
            Vec3::new(7.12, -0.36, -1.50),
            Vec3::new(0.71, 1.72, 0.55),
            [67, 89, 76],
        ),
        (
            Vec3::new(6.13, -0.90, -1.65),
            Vec3::new(0.69, 1.15, 0.49),
            [70, 96, 83],
        ),
        (
            Vec3::new(7.15, -1.49, 0.76),
            Vec3::new(0.67, 0.43, 0.43),
            [99, 110, 89],
        ),
    ] {
        let style = if center.z > 0.4 {
            appearance.style()
        } else {
            scenery_style
        };
        garden_stone(&mut garden, center, radius, color, style);
    }
    for frond in REAR_FRONDS {
        garden_frond(
            &mut garden,
            frond.root,
            frond.height,
            frond.bend,
            frond.color,
            scenery_style,
        );
    }
    // Short growth and a few low stones join the tall compositions to the sand.
    // They stay on the outer banks, away from toy and creature swimming space.
    for (x, z, height, bend) in [
        (-7.35, 0.90, 0.36, 0.13),
        (-6.42, 0.78, 0.27, -0.14),
        (-5.90, -1.3, 0.49, 0.18),
        (5.20, -1.25, 0.42, -0.14),
        (6.72, 1.10, 0.28, 0.12),
        (7.34, 0.45, 0.44, -0.20),
    ] {
        for stem in 0..3 {
            garden_frond(
                &mut garden,
                Vec3::new(x + stem as f32 * 0.07, -1.80, z),
                height * (0.65 + stem as f32 * 0.17),
                bend * (stem as f32 - 0.6),
                [71, 111, 62],
                scenery_style,
            );
        }
    }
    for (center, radius) in [
        (Vec3::new(-6.47, -1.59, 0.52), Vec3::new(0.32, 0.29, 0.30)),
        (Vec3::new(5.57, -1.57, -0.65), Vec3::new(0.37, 0.35, 0.29)),
        (Vec3::new(6.50, -1.72, 0.75), Vec3::new(0.24, 0.18, 0.23)),
    ] {
        garden_stone(&mut garden, center, radius, [103, 116, 91], scenery_style);
    }
    garden
}

/// One solid slab with interpolated vertex colors leaves the water continuous.
/// Its bounds match the former overlapping strips; only their color steps disappear.
fn backdrop_mesh() -> Mesh {
    let mut back = Geometry::default();
    back.cuboid(
        Vec3::new(0.0, 1.325, -2.6),
        Vec3::new(15.95, 6.905, 0.3),
        TANK.back_low,
    );
    let mut mesh = back.mesh();
    let low = Color::srgb_u8(TANK.back_low[0], TANK.back_low[1], TANK.back_low[2])
        .to_linear()
        .to_f32_array();
    let high = Color::srgb_u8(TANK.back_high[0], TANK.back_high[1], TANK.back_high[2])
        .to_linear()
        .to_f32_array();
    let colors: Vec<[f32; 4]> = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .expect("backdrop positions")
        .as_float3()
        .expect("three-dimensional backdrop")
        .iter()
        .map(|position| {
            let t = ((position[1] + 2.1275) / 6.905).clamp(0.0, 1.0);
            std::array::from_fn(|axis| low[axis] + (high[axis] - low[axis]) * t)
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
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
    let phase = center.x * 1.73 + center.z * 2.31;
    let layer_rows = if radius.y > 0.8 { 3 } else { 2 };
    for y in -extent.y..=extent.y {
        let height = (y + extent.y) as f32 / (2 * extent.y).max(1) as f32;
        let tier = (y + extent.y).div_euclid(layer_rows) as f32;
        // Many shallow weathered courses, a domed cap and round broken corners
        // avoid square columns. Every course remains filled against its neighbours.
        let cap = 1.0 - ((height - 0.78).max(0.0) / 0.22).powi(2) * 0.48;
        let shelf = (phase + tier * 1.63).sin() * 0.065;
        let width = (1.0 - height * 0.27 + shelf) * cap;
        let shift_x = (phase + tier * 0.7).sin() * height * 0.17;
        let shift_z = (phase * 0.7 + tier * 0.9).cos() * height * 0.12;
        for x in -extent.x..=extent.x {
            for z in -extent.z..=extent.z {
                let nx = x as f32 * cell / radius.x - shift_x;
                let nz = z as f32 * cell / radius.z - shift_z;
                let radial = (nx * nx + nz * nz).sqrt();
                let chipped = ((x.div_euclid(2) * 7 + z.div_euclid(2) * 3 + tier as i32)
                    .rem_euclid(7) as f32
                    - 3.0)
                    * 0.022;
                if radial <= width + chipped {
                    let lip = (y + extent.y).rem_euclid(layer_rows) == layer_rows - 1;
                    let tone = (tier as i16 % 3) * 2 - 3 + if lip { 7 } else { 0 };
                    stone.set([x, y, z], shade(color, tone));
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
    let phase = root.x * 2.7 + root.z * 4.1 + height;
    for y in 0..=rows {
        let t = y as f32 / rows as f32;
        let envelope = (t * std::f32::consts::PI).sin().max(0.0);
        let sway = (t * 4.2 + phase).sin() * envelope * height.min(3.0) * 0.085;
        let center_x = ((bend * t.powf(1.8) + sway) / cell).round() as i32;
        let center_z = ((t * 4.2 + phase).sin() * envelope * 0.18 / cell).round() as i32;
        // Narrow tapered ribbons twist gradually through depth. Their folded faces
        // receive different light without disconnected strips or a painted outline.
        let half_width = (0.45 + envelope.powf(0.65) * 2.7).round() as i32;
        let twist = (t * 4.5 + phase).sin() * 0.9;
        for x in -half_width..=half_width {
            let edge = x.abs() == half_width;
            if edge && half_width > 1 && (y + (phase.abs() * 3.0) as i32).rem_euclid(11) == 0 {
                continue;
            }
            let fold = (x as f32 * twist - x.abs() as f32 * 0.34).round() as i32;
            let tone = if x == 0 {
                11
            } else if edge {
                5
            } else {
                -2
            };
            for depth in 0..=1 {
                leaf.set(
                    [center_x + x, y, center_z + fold - depth],
                    shade(color, tone),
                );
            }
        }
    }
    append_voxels(mesh, &leaf, cell, root, style);
}

/// Filled column height and bank blend shared by meshing and contact placement.
fn substrate_column(x: i32, z: i32) -> (i32, f32) {
    let world_x = x as f32 * 0.10;
    let rear = ((-z + 2) as f32 / 23.0).clamp(0.0, 1.0);
    let side = ((world_x.abs() - 4.2) / 3.6).clamp(0.0, 1.0);
    let side = side * side * (3.0 - 2.0 * side);
    let bank = side * rear * rear;
    let height = 2 + (bank * (3.6 + (world_x * 0.8).sin() * 0.4)).round() as i32;
    (height, bank)
}

/// Top of the actual sand voxel beneath a presentation position, in world units.
/// Out-of-bed positions clamp to the nearest bank; gameplay anchors remain unchanged.
pub(crate) fn substrate_surface_height(x: f32, z: f32) -> f32 {
    let x = (x / 0.10).round().clamp(-78.0, 78.0) as i32;
    let z = (z / 0.10).round().clamp(-21.0, 20.0) as i32;
    let (height, _) = substrate_column(x, z);
    -2.0 + (height as f32 - 0.5) * 0.10
}

/// Integer occupancy makes the floor continuous even where terrace heights differ.
fn substrate_model() -> VoxelModel {
    let mut model = VoxelModel::default();
    for x in -78..=78 {
        for z in -21..=20 {
            let world_x = x as f32 * 0.10;
            let (height, bank) = substrate_column(x, z);
            let tone = ((world_x * 0.72 + z as f32 * 0.12).sin() * 1.5).round() as i16;
            let color = std::array::from_fn(|axis| {
                let sand = TANK.sand[axis] as f32;
                let bank_color = [137.0, 143.0, 105.0][axis];
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
    mesh.merge(
        &model
            .mesh_with_flat_normals(cell, style)
            .translated_by(offset),
    )
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
        .mesh_with_flat_normals(0.055, style),
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
            shape.mesh_with_flat_normals(0.052, style)
        }
        ObjectKind::Toy(ToyId::Sock) => sock_model().mesh_with_flat_normals(0.043, style),
        ObjectKind::Cave => shelter_model().mesh_with_flat_normals(0.075, style),
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
            for z in -18_i32..=-5 {
                // A semicircular crown meets upright jambs at the spring line.
                // The old tall ellipse made the shelter look like a pointed hutch.
                let front = z >= -7;
                let outer_radius: f32 = match z {
                    -5 => 16.0,
                    -7..=-6 => 17.0,
                    -9..=-8 | -14..=-13 => 16.5,
                    _ => 16.0,
                };
                let inner_radius: f32 = if z == -5 {
                    14.5
                } else if front {
                    14.0
                } else {
                    15.0
                };
                let roof_y = y.max(0) as f32;
                let outer =
                    (x as f32 / outer_radius).powi(2) + (roof_y / outer_radius).powi(2) <= 1.0;
                let doorway = (x as f32 / inner_radius).powi(2) + (roof_y / 14.0).powi(2) < 1.0;
                if outer && (!doorway || z <= -17) {
                    // Recessed depth courses and a narrow rolled front lip make
                    // the terracotta read as a thick fired object, not a flat arch.
                    let base = if doorway {
                        [49, 43, 35]
                    } else if z == -5 {
                        [198, 126, 77]
                    } else if front {
                        [183, 106, 62]
                    } else if y >= 0 {
                        [174, 99, 60]
                    } else {
                        [147, 83, 51]
                    };
                    let tone = if doorway {
                        0
                    } else {
                        (y.div_euclid(4).rem_euclid(3) - 1) as i16 * 2
                    };
                    shape.set([x, y, z], shade(base, tone));
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
    shape.mesh_with_flat_normals(0.045, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_scenery_keeps_a_bounded_triangle_budget() {
        let mesh = garden_mesh(crate::appearance::RenderAppearance::default());
        let triangles = mesh.indices().expect("indexed scenery").len() / 3;
        assert!(
            triangles <= 180_000,
            "scenery has {triangles} triangles; retain sharp subpixel decor surfaces"
        );
    }

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
