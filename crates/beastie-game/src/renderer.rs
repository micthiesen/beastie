//! Bevy's native 3D presentation. Every non-text mark is solid colored geometry.
use std::collections::BTreeMap;

use beastie_core::{FoodId, NormalizedPosition, ToyId};
use beastie_view::{
    HitRegion, HitShape, IconKind, ObjectKind, PresentationCueKind, ScenePlan, UiTarget,
};
use bevy::{camera::ScalingMode, prelude::*, window::PrimaryWindow};

use crate::{
    host::HostSet,
    voxel::{Geometry, VoxelModel},
};

pub const LOGICAL_WIDTH: f32 = 320.0;
pub const LOGICAL_HEIGHT: f32 = 180.0;
pub const PRESENTATION_WIDTH: f32 = 1280.0;
pub const PRESENTATION_HEIGHT: f32 = 720.0;
const UNITS: f32 = 20.0;

#[derive(Resource)]
pub struct SceneFrame {
    pub plan: ScenePlan,
}
#[derive(Component)]
pub struct TankCamera;
#[derive(Component)]
struct WorldObject(u64);
#[derive(Component)]
struct UiGeometry;
#[derive(Component)]
struct UiText;
#[derive(Component)]
struct EffectGeometry;
#[derive(Component)]
struct Bubble(usize);
#[derive(Resource)]
struct Palette {
    solid: Handle<StandardMaterial>,
    ui: Handle<StandardMaterial>,
}
#[derive(Resource, Default)]
struct ObjectMeshes(BTreeMap<String, Handle<Mesh>>);
#[derive(Resource, Default)]
struct EffectMesh {
    handle: Option<Handle<Mesh>>,
    entity: Option<Entity>,
}
#[derive(Resource, Default)]
struct UiCache {
    rects: Vec<beastie_view::RectCommand>,
    icons: Vec<beastie_view::IconCommand>,
    text: Vec<beastie_view::TextCommand>,
    width: f32,
    height: f32,
    geometry: Option<Handle<Mesh>>,
    text_entities: BTreeMap<(String, usize), (Entity, Entity)>,
}
#[derive(Resource)]
struct UiFont(Handle<Font>);

pub struct RendererPlugin;
impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjectMeshes>()
            .init_resource::<EffectMesh>()
            .init_resource::<UiCache>()
            .add_systems(Startup, (setup, crate::creature::setup_creature))
            .add_systems(
                Update,
                (
                    crate::creature::animate_creature,
                    sync_objects,
                    sync_ui,
                    sync_effects,
                    animate_bubbles,
                )
                    .chain()
                    .after(HostSet::Publish),
            );
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}
impl Viewport {
    pub fn for_drawable(width: f32, height: f32) -> Self {
        let scale = (width / LOGICAL_WIDTH)
            .min(height / LOGICAL_HEIGHT)
            .max(0.001);
        Self {
            x: (width - LOGICAL_WIDTH * scale) / 2.0,
            y: (height - LOGICAL_HEIGHT * scale) / 2.0,
            scale,
        }
    }
    pub fn logical_point(self, x: f32, y: f32) -> Option<(f32, f32)> {
        let point = ((x - self.x) / self.scale, (y - self.y) / self.scale);
        (point.0 >= 0.0 && point.1 >= 0.0 && point.0 < LOGICAL_WIDTH && point.1 < LOGICAL_HEIGHT)
            .then_some(point)
    }
}

pub fn world_position(position: NormalizedPosition) -> Vec3 {
    // Keep fractional normalized motion through the final 3D transform.
    Vec3::new(
        -6.6 + position.x as f32 * 13.2 / 10_000.0,
        3.4 - position.y as f32 * 4.55 / 10_000.0,
        0.0,
    )
}
fn logical_position(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new((x - 160.0) / UNITS, (90.0 - y) / UNITS, z)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fonts: ResMut<Assets<Font>>,
) {
    let solid = materials.add(StandardMaterial {
        perceptual_roughness: 0.84,
        ..default()
    });
    let ui = materials.add(StandardMaterial {
        unlit: true,
        ..default()
    });
    let bubble = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.70, 0.72),
        metallic: 0.3,
        perceptual_roughness: 0.25,
        ..default()
    });
    commands.spawn((
        TankCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 16.0,
                min_height: 9.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 24.0).looking_at(Vec3::ZERO, Vec3::Y),
        AmbientLight {
            color: Color::srgb(0.60, 0.80, 0.88),
            brightness: 550.0,
            ..default()
        },
    ));
    commands.insert_resource(ClearColor(Color::srgb_u8(7, 18, 25)));
    commands.spawn((
        DirectionalLight {
            illuminance: 4200.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.0, 7.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.40, 0.85, 0.95),
            intensity: 150_000.0,
            range: 30.0,
            ..default()
        },
        Transform::from_xyz(6.0, 4.0, 5.0),
    ));
    let mut back = Geometry::default();
    // Layered teal back wall, brass-dark structural frame, and a stepped stone bed.
    for row in 0..26 {
        let t = row as f32 / 25.0;
        let color = [
            12 + (t * 10.0) as u8,
            38 + (t * 21.0) as u8,
            48 + (t * 21.0) as u8,
        ];
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
            [40, 66, 68],
        );
        tank.cuboid(
            Vec3::new(side * 7.75, 1.1, 2.1),
            Vec3::new(0.045, 6.65, 0.08),
            [155, 149, 109],
        );
    }
    tank.cuboid(
        Vec3::new(0.0, 4.35, -0.2),
        Vec3::new(16.0, 0.2, 4.7),
        [38, 68, 69],
    );
    tank.cuboid(
        Vec3::new(0.0, 4.21, 2.1),
        Vec3::new(15.65, 0.055, 0.07),
        [181, 179, 125],
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
                [92 + hash as u8 * 2, 102 + hash as u8 * 2, 83 + hash as u8],
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
    for (x, height, bend, color) in [
        (-7.0, 1.9, 0.36, [43, 93, 82]),
        (-6.60, 1.4, -0.32, [50, 109, 89]),
        (-6.30, 1.65, 0.28, [61, 115, 95]),
        (6.60, 2.25, -0.44, [37, 91, 85]),
        (6.93, 1.62, 0.36, [58, 117, 96]),
        (6.07, 1.35, -0.26, [49, 103, 91]),
    ] {
        garden_frond(&mut garden, Vec3::new(x, -1.9, -1.30), height, bend, color);
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
    let cube = meshes.add(Cuboid::default());
    for index in 0..14 {
        commands.spawn((
            Bubble(index),
            Mesh3d(cube.clone()),
            MeshMaterial3d(bubble.clone()),
            Transform::default(),
        ));
    }
    let font = std::fs::read(
        crate::app::assets_root().join("generated/ui/atkinson-hyperlegible-next-medium.ttf"),
    )
    .ok()
    .map(Font::from_bytes)
    .map(|font| fonts.add(font))
    .unwrap_or_default();
    commands.insert_resource(UiFont(font));
    commands.insert_resource(Palette { solid, ui });
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
        let center = root + Vec3::new(bend * t * t + (t * 4.5).sin() * 0.045, t * height, t * 0.10);
        let width = 0.065 + (std::f32::consts::PI * t).sin() * 0.085;
        shape.cuboid(center, Vec3::new(width, 0.083, 0.055), color);
        if segment > 3 && segment % 5 == 0 {
            let side = if segment % 10 == 0 { -1.0 } else { 1.0 };
            for leaf in 1..=3 {
                shape.cuboid(
                    center + Vec3::new(side * leaf as f32 * 0.055, leaf as f32 * 0.025, 0.015),
                    Vec3::new(0.075, 0.040, 0.050),
                    color.map(|channel| channel.saturating_add(9)),
                );
            }
        }
    }
}

fn object_key(kind: ObjectKind) -> String {
    format!("{kind:?}")
}
fn object_mesh(kind: ObjectKind) -> Mesh {
    match kind {
        ObjectKind::Food(food) => {
            let color = match food {
                FoodId::Berry => [205, 80, 101],
                FoodId::Mushroom => [191, 166, 121],
                FoodId::Pellet => [143, 119, 79],
            };
            VoxelModel::ellipsoid([3, 3, 3], |p| if p[1] == 3 { [83, 140, 99] } else { color })
                .mesh(0.055)
        }
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
            geometry.mesh()
        }
        ObjectKind::Cave => {
            let mut shape = VoxelModel::default();
            for x in -12_i32..=12 {
                for y in -13..17 {
                    for z in -6..=4 {
                        let outer =
                            y < 0 || (x as f32 / 12.0).powi(2) + (y as f32 / 17.0).powi(2) <= 1.0;
                        let doorway = x.abs() < 7 && y < 11 && z > -5;
                        if outer && !doorway {
                            let shade = (x * 17 + y * 31 + z * 7_i32).rem_euclid(13) as u8;
                            let color = if x.abs() < 7 && y < 11 && z <= -5 {
                                [19 + shade / 3, 35 + shade / 3, 36 + shade / 3]
                            } else {
                                [64 + shade, 90 + shade, 88 + shade]
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

/// Use the same continuous anchor for drawing, effects, and volume picking.
pub(crate) fn object_position(object: &beastie_view::ObjectScene, scene: &ScenePlan) -> Vec3 {
    if object.carried {
        return crate::creature::head_position(scene) + Vec3::new(0.0, -0.34, 0.65);
    }
    let fraction = scene.simulation_remainder_ms.min(999) as f32 / 1000.0;
    world_position(NormalizedPosition::new(
        object
            .position
            .x
            .saturating_add((object.velocity.x as f32 * fraction).round() as i32),
        object
            .position
            .y
            .saturating_add((object.velocity.y as f32 * fraction).round() as i32),
    ))
}

fn presented_object_position(
    object: &beastie_view::ObjectScene,
    scene: &ScenePlan,
    motion: &crate::creature::CreatureMotion,
) -> Vec3 {
    if object.carried {
        motion.position(scene) + Vec3::new(0.0, -0.34, 0.65)
    } else {
        object_position(object, scene)
    }
}

fn sync_objects(
    mut commands: Commands,
    frame: Res<SceneFrame>,
    palette: Res<Palette>,
    motion: Res<crate::creature::CreatureMotion>,
    mut cache: ResMut<ObjectMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut objects: Query<(Entity, &WorldObject, &mut Transform)>,
) {
    for (entity, object, mut transform) in &mut objects {
        let Some(plan) = frame.plan.objects.iter().find(|p| p.id == object.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = presented_object_position(plan, &frame.plan, &motion);
        let time = frame
            .plan
            .elapsed_ms
            .saturating_add(frame.plan.simulation_remainder_ms) as f32
            / 1000.0;
        transform.rotation = Quat::IDENTITY;
        if matches!(plan.kind, ObjectKind::Plant) && !frame.plan.reduced_motion {
            transform.rotation = Quat::from_rotation_z((time / 1.9 + plan.id as f32).sin() * 0.055);
        }
        if plan.kind == ObjectKind::Toy(ToyId::Ball) {
            transform.rotation = Quat::from_rotation_z(-transform.translation.x * 1.2);
        }
        if plan.kind == ObjectKind::Toy(ToyId::Bell)
            && !frame.plan.reduced_motion
            && !frame.plan.reduced_shake
            && let Some(cue) = frame.plan.effects.iter().find(|cue| {
                cue.target == UiTarget::Toy(ToyId::Bell)
                    && cue.cue == PresentationCueKind::BellStrike
            })
        {
            let elapsed =
                cue.elapsed_ms
                    .saturating_add(frame.plan.simulation_remainder_ms) as f32
                    / 1000.0;
            transform.rotation =
                Quat::from_rotation_z((elapsed * 18.0).sin() * (-elapsed * 1.5).exp() * 0.3);
        }
        if plan.carried {
            transform.rotation = Quat::from_rotation_z(-0.15);
        }
    }
    for object in &frame.plan.objects {
        if objects
            .iter()
            .any(|(_, candidate, _)| candidate.0 == object.id)
        {
            continue;
        }
        let mesh = cache
            .0
            .entry(object_key(object.kind))
            .or_insert_with(|| meshes.add(object_mesh(object.kind)))
            .clone();
        commands.spawn((
            WorldObject(object.id),
            Mesh3d(mesh),
            MeshMaterial3d(palette.solid.clone()),
            Transform::from_translation(presented_object_position(object, &frame.plan, &motion)),
        ));
    }
}

fn animate_bubbles(frame: Res<SceneFrame>, mut bubbles: Query<(&Bubble, &mut Transform)>) {
    for (bubble, mut transform) in &mut bubbles {
        let t = if frame.plan.reduced_motion {
            bubble.0 as f32
        } else {
            frame
                .plan
                .elapsed_ms
                .saturating_add(frame.plan.simulation_remainder_ms) as f32
                / 1000.0
        };
        let i = bubble.0 as f32;
        let y = (t * 0.13 + i * 0.73).rem_euclid(6.0) - 1.7;
        transform.translation = Vec3::new(
            -7.1 + (i * 2.73).rem_euclid(14.1) + (t * 0.3 + i).sin() * 0.12,
            y,
            -1.4,
        );
        transform.scale = Vec3::splat(0.025 + (i % 4.0) * 0.007);
        transform.rotation = Quat::from_rotation_z(t * 0.2 + i);
    }
}

fn icon_geometry(kind: IconKind, center: Vec3, shape: &mut Geometry) {
    let color = [189, 211, 191];
    let cell = 0.055;
    let pattern: &[&str] = match kind {
        IconKind::Pearl => &["  xx  ", " xxxx ", "xxxxxx", "xxxxxx", " xxxx ", "  xx  "],
        IconKind::Microphone => &[
            "  xx  ", "  xx  ", "x xx x", "x xx x", " xxxx ", "  xx  ", " xxxx ",
        ],
        IconKind::Food => &["  x   ", " xxx  ", "xxxxx ", "xxxxx ", " xxx  "],
        IconKind::Settings => &[" x  x ", "xxxxxx", "xx  xx", "xx  xx", "xxxxxx", " x  x "],
        IconKind::Send => &[
            "x     ", "xxx   ", "xxxxx ", "xxxxxx", "xxxxx ", "xxx   ", "x     ",
        ],
    };
    for (row, line) in pattern.iter().enumerate() {
        for (column, byte) in line.bytes().enumerate() {
            if byte == b'x' {
                shape.cuboid(
                    center + Vec3::new(column as f32 * cell, -(row as f32) * cell, 0.0),
                    Vec3::splat(cell * 0.95),
                    color,
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextBox {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl TextBox {
    fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.w).min(other.x + other.w);
        let bottom = (self.y + self.h).min(other.y + other.h);
        (right > x && bottom > y).then_some(Self {
            x,
            y,
            w: right - x,
            h: bottom - y,
        })
    }
    fn subtract(self, cover: Self) -> Vec<Self> {
        let Some(overlap) = self.intersection(cover) else {
            return vec![self];
        };
        [
            Self {
                x: self.x,
                y: self.y,
                w: self.w,
                h: overlap.y - self.y,
            },
            Self {
                x: self.x,
                y: overlap.y + overlap.h,
                w: self.w,
                h: self.y + self.h - overlap.y - overlap.h,
            },
            Self {
                x: self.x,
                y: overlap.y,
                w: overlap.x - self.x,
                h: overlap.h,
            },
            Self {
                x: overlap.x + overlap.w,
                y: overlap.y,
                w: self.x + self.w - overlap.x - overlap.w,
                h: overlap.h,
            },
        ]
        .into_iter()
        .filter(|part| part.w > 0.01 && part.h > 0.01)
        .collect()
    }
}
fn text_box(rect: beastie_view::Rect) -> TextBox {
    TextBox {
        x: rect.x as f32,
        y: rect.y as f32,
        w: rect.w as f32,
        h: rect.h as f32,
    }
}

/// Every text box belongs to a panel or button. Neighboring labels constrain columns/rows
/// inside a panel; native glyph wrapping must never borrow the remainder of the window.
fn text_content_bounds(text: &beastie_view::TextCommand, plan: &ScenePlan) -> TextBox {
    let container = plan
        .rects
        .iter()
        .filter(|rect| {
            !rect.outline
                && rect.layer <= text.layer
                && rect.rect.w >= 8
                && rect.rect.h >= 6
                && rect.rect.contains(text.x, text.y)
        })
        .min_by_key(|rect| rect.rect.w * rect.rect.h)
        .map(|rect| text_box(rect.rect))
        .unwrap_or(TextBox {
            x: 0.0,
            y: 0.0,
            w: LOGICAL_WIDTH,
            h: LOGICAL_HEIGHT,
        });
    let x = text.x as f32;
    let y = text.y as f32;
    let mut right = container.x + container.w - 3.0;
    let mut bottom = if text.id.starts_with("settings/") && text.id.ends_with("-value") {
        container.y + container.h
    } else {
        container.y + container.h - 2.0
    };
    // A settings label is followed by its value button, which is a rect rather than
    // another text command. Constrain the label to that control or large text can
    // incorrectly borrow the whole modal width (for example, "Save & data").
    for rect in &plan.rects {
        if rect.outline
            || rect.rect.x <= text.x
            || rect.rect.y > text.y
            || rect.rect.y + rect.rect.h <= text.y
        {
            continue;
        }
        right = right.min(rect.rect.x as f32 - 3.0);
        if text.id.starts_with("settings/") && text.id.ends_with("-name") {
            // The final settings row has no following label to establish a bottom
            // edge, so its panel would otherwise make the label much taller than
            // every other row at the large accessibility size.
            bottom = bottom.min(rect.rect.y as f32 + rect.rect.h as f32 + 4.0);
        } else if text.id.starts_with("settings/") && text.id.ends_with("-value") {
            // Keep value text inside the button's actual background. The text starts
            // one unit below the outer edge, and the background supplies the lower edge.
            bottom = bottom.min(rect.rect.y as f32 + rect.rect.h as f32);
        }
    }
    for next in &plan.text {
        if next.id == text.id || next.layer != text.layer {
            continue;
        }
        if next.y == text.y && next.x > text.x {
            right = right.min(next.x as f32 - 3.0);
        }
        if next.y > text.y && (next.x as f32) < right && next.x >= text.x {
            bottom = bottom.min(next.y as f32 - 1.0);
        }
    }
    TextBox {
        x,
        y,
        w: (right - x).max(1.0),
        h: (bottom - y).max(1.0),
    }
}

fn visible_text_boxes(
    text: &beastie_view::TextCommand,
    bounds: TextBox,
    plan: &ScenePlan,
) -> Vec<TextBox> {
    let mut visible = vec![bounds];
    for cover in plan
        .rects
        .iter()
        .filter(|rect| !rect.outline && rect.layer > text.layer)
    {
        visible = visible
            .into_iter()
            .flat_map(|part| part.subtract(text_box(cover.rect)))
            .collect();
        if visible.is_empty() {
            break;
        }
    }
    visible
}

fn fitting_font_size(text: &beastie_view::TextCommand, bounds: TextBox) -> f32 {
    let requested = 8.0 * f32::from(text.scale);
    let mut size = requested.min(bounds.h / 1.2);
    // Atkinson's mean advance is about half its em. A conservative width estimate keeps
    // small controls legible without silently drawing their label across the next control.
    let longest = text
        .text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f32;
    if text.id == "speech/text" {
        for _ in 0..16 {
            let columns = (bounds.w / (size * 0.56)).floor().max(1.0);
            let lines = (text.text.chars().count() as f32 / columns).ceil().max(1.0);
            if lines * size * 1.2 <= bounds.h {
                break;
            }
            size *= 0.92;
        }
    } else {
        size = size.min(bounds.w / (longest * 0.56).max(1.0));
    }
    size.max(1.0)
}

#[derive(bevy::ecs::system::SystemParam)]
struct UiSystem<'w, 's> {
    commands: Commands<'w, 's>,
    frame: Res<'w, SceneFrame>,
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
    palette: Res<'w, Palette>,
    font: Res<'w, UiFont>,
    cache: ResMut<'w, UiCache>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

fn sync_ui(mut ui: UiSystem) {
    let geometry_changed =
        ui.cache.rects != ui.frame.plan.rects || ui.cache.icons != ui.frame.plan.icons;
    if geometry_changed {
        let mut shape = Geometry::default();
        for rect in &ui.frame.plan.rects {
            let r = rect.rect;
            let center = logical_position(
                r.x as f32 + r.w as f32 * 0.5,
                r.y as f32 + r.h as f32 * 0.5,
                8.0 + rect.layer as f32 * 0.002,
            );
            let color = [rect.color[0], rect.color[1], rect.color[2]];
            if rect.outline {
                let w = r.w as f32 / UNITS;
                let h = r.h as f32 / UNITS;
                for y in [-h * 0.5, h * 0.5] {
                    shape.cuboid(center + Vec3::Y * y, Vec3::new(w, 0.028, 0.02), color);
                }
                for x in [-w * 0.5, w * 0.5] {
                    shape.cuboid(center + Vec3::X * x, Vec3::new(0.028, h, 0.02), color);
                }
            } else {
                shape.cuboid(
                    center,
                    Vec3::new(r.w as f32 / UNITS, r.h as f32 / UNITS, 0.02),
                    color,
                );
            }
        }
        for icon in &ui.frame.plan.icons {
            icon_geometry(
                icon.kind,
                logical_position(
                    icon.x as f32 + 1.0,
                    icon.y as f32 + 1.0,
                    8.05 + icon.layer as f32 * 0.002,
                ),
                &mut shape,
            );
        }
        let replacement = shape.mesh();
        if let Some(handle) = ui.cache.geometry.clone() {
            if let Some(mut mesh) = ui.meshes.get_mut(&handle) {
                *mesh = replacement;
            }
        } else {
            let mesh = ui.meshes.add(replacement);
            let material = ui.palette.ui.clone();
            ui.commands.spawn((
                UiGeometry,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::default(),
            ));
            ui.cache.geometry = Some(mesh);
        }
    }
    let text_changed = geometry_changed
        || ui.cache.text != ui.frame.plan.text
        || ui.cache.width != ui.window.width()
        || ui.cache.height != ui.window.height();
    if text_changed {
        let viewport = Viewport::for_drawable(ui.window.width(), ui.window.height());
        let mut retained = BTreeMap::new();
        let labels = ui.frame.plan.text.clone();
        for text in labels {
            let bounds = text_content_bounds(&text, &ui.frame.plan);
            let font_size = fitting_font_size(&text, bounds) * viewport.scale;
            for (index, visible) in visible_text_boxes(&text, bounds, &ui.frame.plan)
                .into_iter()
                .enumerate()
            {
                let key = (text.id.clone(), index);
                let (parent, child) = if let Some(entities) = ui.cache.text_entities.remove(&key) {
                    entities
                } else {
                    let parent = ui.commands.spawn(UiText).id();
                    let child = ui.commands.spawn_empty().id();
                    ui.commands.entity(parent).add_child(child);
                    (parent, child)
                };
                ui.commands.entity(parent).insert((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(viewport.x + visible.x * viewport.scale),
                        top: px(viewport.y + visible.y * viewport.scale),
                        width: px(visible.w * viewport.scale),
                        height: px(visible.h * viewport.scale),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    GlobalZIndex(i32::from(text.layer)),
                ));
                let font = ui.font.0.clone();
                ui.commands.entity(child).insert((
                    Text::new(&text.text),
                    TextFont {
                        font: font.into(),
                        font_size: bevy::text::FontSize::Px(font_size),
                        ..default()
                    },
                    TextColor(Color::srgb_u8(228, 232, 207)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px((bounds.x - visible.x) * viewport.scale),
                        top: px((bounds.y - visible.y) * viewport.scale),
                        width: px(bounds.w * viewport.scale),
                        ..default()
                    },
                ));
                retained.insert(key, (parent, child));
            }
        }
        for (_, (parent, _)) in std::mem::take(&mut ui.cache.text_entities) {
            ui.commands.entity(parent).despawn();
        }
        ui.cache.text_entities = retained;
        ui.cache.text = ui.frame.plan.text.clone();
        ui.cache.width = ui.window.width();
        ui.cache.height = ui.window.height();
    }
    ui.cache.rects = ui.frame.plan.rects.clone();
    ui.cache.icons = ui.frame.plan.icons.clone();
}

fn sync_effects(
    mut commands: Commands,
    frame: Res<SceneFrame>,
    palette: Res<Palette>,
    motion: Res<crate::creature::CreatureMotion>,
    mut mesh: ResMut<EffectMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut shape = Geometry::default();
    for effect in &frame.plan.effects {
        let elapsed = effect
            .elapsed_ms
            .saturating_add(frame.plan.simulation_remainder_ms) as f32
            / 1000.0;
        let position = match effect.target {
            UiTarget::Creature => motion.position(&frame.plan),
            target => frame
                .plan
                .objects
                .iter()
                .find(|object| match object.kind {
                    ObjectKind::Toy(toy) => target == UiTarget::Toy(toy),
                    ObjectKind::Food(_) => target == UiTarget::FoodObject(object.id),
                    ObjectKind::Plant => target == UiTarget::Plant(object.id),
                    ObjectKind::Cave => target == UiTarget::Cave,
                })
                .map_or_else(
                    || world_position(effect.position),
                    |object| presented_object_position(object, &frame.plan, &motion),
                ),
        };
        let travel = if frame.plan.reduced_motion {
            0.0
        } else {
            elapsed.min(1.5)
        };
        match effect.cue {
            PresentationCueKind::Wake => {
                if !frame.plan.reduced_motion {
                    let direction = if frame.plan.creature.facing == beastie_core::Facing::Left {
                        1.0
                    } else {
                        -1.0
                    };
                    let time = frame
                        .plan
                        .elapsed_ms
                        .saturating_add(frame.plan.simulation_remainder_ms)
                        as f32
                        / 1000.0;
                    for index in 0..3 {
                        let phase = (time * 0.8 + index as f32 / 3.0).fract();
                        shape.cuboid(
                            position
                                + Vec3::new(
                                    direction * (0.85 + phase * 0.6),
                                    (phase * 5.0 + index as f32).sin() * 0.08,
                                    -0.25,
                                ),
                            Vec3::splat(0.035 * (1.0 - phase).max(0.2)),
                            [89, 139, 144],
                        );
                    }
                }
            }
            PresentationCueKind::Affection | PresentationCueKind::Delight => {
                let center = position + Vec3::new(0.0, 0.8 + travel * 0.22, 0.7);
                for (row, line) in [
                    " xx xx ", "xxxxxxx", "xxxxxxx", " xxxxx ", "  xxx  ", "   x   ",
                ]
                .iter()
                .enumerate()
                {
                    for (column, byte) in line.bytes().enumerate() {
                        if byte == b'x' {
                            shape.cuboid(
                                center
                                    + Vec3::new(
                                        (column as f32 - 3.0) * 0.045,
                                        -(row as f32) * 0.045,
                                        0.0,
                                    ),
                                Vec3::splat(0.044),
                                [233, 130, 117],
                            );
                        }
                    }
                }
            }
            PresentationCueKind::Crumbs
            | PresentationCueKind::Spit
            | PresentationCueKind::SandPuff
            | PresentationCueKind::BottomForage => {
                let sand = matches!(
                    effect.cue,
                    PresentationCueKind::SandPuff | PresentationCueKind::BottomForage
                );
                for index in 0..if frame.plan.reduced_flashes { 3 } else { 7 } {
                    let angle = index as f32 * 2.4;
                    let offset = Vec3::new(
                        angle.sin() * (0.15 + travel * 0.35),
                        if sand {
                            -0.2 + angle.cos() * 0.07 + travel * 0.15
                        } else {
                            -0.2 - travel * 0.3 + angle.cos() * 0.12
                        },
                        0.6,
                    );
                    let color = if sand {
                        [148, 154, 117]
                    } else {
                        [214, 147, 90]
                    };
                    shape.cuboid(
                        position + offset,
                        Vec3::splat((0.06 - travel * 0.02).max(0.025)),
                        color,
                    );
                }
            }
            PresentationCueKind::Sleep
            | PresentationCueKind::CaveShelter
            | PresentationCueKind::OpenWaterDrift
            | PresentationCueKind::Comfort => {}
            _ => {
                for index in 0..3 {
                    let angle = (index as f32 - 1.0) * 0.6;
                    let center =
                        position + Vec3::new(angle.sin() * 0.5, 0.7 + angle.cos() * 0.18, 0.6);
                    shape.cuboid(center, Vec3::new(0.04, 0.10, 0.045), [187, 216, 181]);
                }
            }
        }
    }
    for hit in &frame.plan.hit_regions {
        let highlighted = match hit.target {
            Some(UiTarget::Creature) => {
                frame.plan.creature.highlight != beastie_view::Highlight::None
            }
            Some(target) => frame.plan.objects.iter().any(|object| {
                object.highlight != beastie_view::Highlight::None
                    && match object.kind {
                        ObjectKind::Toy(toy) => target == UiTarget::Toy(toy),
                        ObjectKind::Cave => target == UiTarget::Cave,
                        ObjectKind::Plant => target == UiTarget::Plant(object.id),
                        ObjectKind::Food(_) => target == UiTarget::FoodObject(object.id),
                    }
            }),
            None => false,
        };
        if !highlighted {
            continue;
        }
        let center = if hit.target == Some(UiTarget::Creature) {
            motion.position(&frame.plan)
        } else if let Some(object) = frame.plan.objects.iter().find(|object| match object.kind {
            ObjectKind::Toy(toy) => hit.target == Some(UiTarget::Toy(toy)),
            ObjectKind::Cave => hit.target == Some(UiTarget::Cave),
            ObjectKind::Plant => hit.target == Some(UiTarget::Plant(object.id)),
            ObjectKind::Food(_) => hit.target == Some(UiTarget::FoodObject(object.id)),
        }) {
            presented_object_position(object, &frame.plan, &motion)
        } else {
            continue;
        };
        let radius = if hit.target == Some(UiTarget::Creature) {
            0.78
        } else {
            0.56
        };
        for side in [-1.0, 1.0] {
            for vertical in [-1.0, 1.0] {
                let p = center + Vec3::new(side * radius, vertical * radius * 0.7, 1.0);
                shape.cuboid(p, Vec3::new(0.13, 0.03, 0.04), [165, 210, 185]);
                shape.cuboid(
                    p + Vec3::new(side * 0.05, -vertical * 0.05, 0.0),
                    Vec3::new(0.03, 0.13, 0.04),
                    [165, 210, 185],
                );
            }
        }
    }
    let next = shape.mesh();
    // Empty meshes have no allocator slots. Keep the last allocation hidden instead of asking
    // the renderer to upload zero vertex/index buffers on every idle frame.
    if next.count_vertices() == 0 {
        if let Some(entity) = mesh.entity {
            commands.entity(entity).insert(Visibility::Hidden);
        }
        return;
    }
    if let Some(handle) = &mesh.handle {
        if let Some(mut current) = meshes.get_mut(handle) {
            *current = next;
        }
        if let Some(entity) = mesh.entity {
            commands.entity(entity).insert(Visibility::Visible);
        }
    } else {
        let handle = meshes.add(next);
        let entity = commands
            .spawn((
                EffectGeometry,
                Mesh3d(handle.clone()),
                MeshMaterial3d(palette.ui.clone()),
                Transform::default(),
                bevy::light::NotShadowCaster,
            ))
            .id();
        mesh.handle = Some(handle);
        mesh.entity = Some(entity);
    }
}

pub fn pointer_world(
    camera: &Camera,
    transform: &GlobalTransform,
    cursor: Vec2,
) -> Option<NormalizedPosition> {
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let distance = -ray.origin.z / ray.direction.z;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let point = ray.get_point(distance);
    if !(-6.6..=6.6).contains(&point.x) || !(-1.15..=3.4).contains(&point.y) {
        return None;
    }
    Some(NormalizedPosition::new(
        ((point.x + 6.6) * 10_000.0 / 13.2).round() as i32,
        ((3.4 - point.y) * 10_000.0 / 4.55).round() as i32,
    ))
}

pub fn pick(
    plan: &ScenePlan,
    camera: &Camera,
    transform: &GlobalTransform,
    cursor: Vec2,
    motion: &crate::creature::CreatureMotion,
) -> Option<HitRegion> {
    let size = camera.logical_viewport_size()?;
    let (x, y) = Viewport::for_drawable(size.x, size.y).logical_point(cursor.x, cursor.y)?;
    if let Some(hit) = plan.hit_regions.iter().rev().find(|hit| {
        hit.enabled && matches!(hit.shape, HitShape::Rect) && hit.rect.contains(x as i32, y as i32)
    }) {
        return Some(hit.clone());
    }
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let mut selected: Option<(f32, UiTarget)> = None;
    let mut consider = |target: UiTarget, center: Vec3, radius: Vec3| {
        if let Some(distance) = ray_ellipsoid(ray.origin, *ray.direction, center, radius)
            && selected.is_none_or(|(near, _)| distance < near)
        {
            selected = Some((distance, target));
        }
    };
    consider(
        UiTarget::Creature,
        motion.position(plan),
        Vec3::new(0.75, 0.65, 0.55),
    );
    for object in &plan.objects {
        let (target, radius) = match object.kind {
            ObjectKind::Food(_) => (UiTarget::FoodObject(object.id), Vec3::splat(0.22)),
            ObjectKind::Toy(toy) => (UiTarget::Toy(toy), Vec3::splat(0.42)),
            ObjectKind::Cave => (UiTarget::Cave, Vec3::new(0.95, 0.85, 0.55)),
            ObjectKind::Plant => (UiTarget::Plant(object.id), Vec3::new(0.5, 0.8, 0.4)),
        };
        consider(
            target,
            presented_object_position(object, plan, motion),
            radius,
        );
    }
    let target = selected
        .map(|(_, target)| target)
        .or_else(|| pointer_world(camera, transform, cursor).map(|_| UiTarget::OpenWater))?;
    plan.hit_regions
        .iter()
        .rev()
        .find(|hit| hit.enabled && hit.shape == HitShape::World(target))
        .cloned()
}
fn ray_ellipsoid(origin: Vec3, direction: Vec3, center: Vec3, radius: Vec3) -> Option<f32> {
    let o = (origin - center) / radius;
    let d = direction / radius;
    let a = d.length_squared();
    let b = o.dot(d);
    let c = o.length_squared() - 1.0;
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let near = (-b - discriminant.sqrt()) / a;
    let far = (-b + discriminant.sqrt()) / a;
    if near >= 0.0 {
        Some(near)
    } else if far >= 0.0 {
        Some(far)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picking_uses_volume_instead_of_a_rectangular_silhouette() {
        assert!(
            ray_ellipsoid(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z, Vec3::ZERO, Vec3::ONE).is_some()
        );
        assert!(
            ray_ellipsoid(Vec3::new(0.9, 0.9, 5.0), Vec3::NEG_Z, Vec3::ZERO, Vec3::ONE).is_none()
        );
    }
    #[test]
    fn fractional_positions_survive_projection() {
        assert!(
            world_position(NormalizedPosition::new(5001, 5000)).x
                > world_position(NormalizedPosition::new(5000, 5000)).x
        );
    }
    #[test]
    fn letterbox_is_excluded_from_ui_input() {
        let viewport = Viewport::for_drawable(1600.0, 720.0);
        assert!(viewport.logical_point(0.0, 360.0).is_none());
        assert_eq!(viewport.logical_point(800.0, 360.0), Some((160.0, 90.0)));
    }
}

#[cfg(test)]
mod object_motion_tests {
    use super::*;

    #[test]
    fn moving_objects_advance_between_authoritative_ticks() {
        let mut scene = beastie_view::plan(
            &beastie_core::WorldState::new(9, "Motion"),
            &beastie_view::ViewState::default(),
        )
        .0;
        let mut object = scene.objects[0].clone();
        object.position = NormalizedPosition::new(4000, 4000);
        object.velocity = beastie_core::NormalizedVelocity { x: 200, y: 100 };
        scene.simulation_remainder_ms = 0;
        let start = object_position(&object, &scene);
        scene.simulation_remainder_ms = 500;
        let halfway = object_position(&object, &scene);
        assert!(halfway.x > start.x && halfway.y < start.y);
        assert!(halfway.distance(world_position(NormalizedPosition::new(4100, 4050))) < 0.0001);
    }

    #[test]
    fn carried_object_is_visible_in_front_of_the_head() {
        let scene = beastie_view::plan(
            &beastie_core::WorldState::new(9, "Motion"),
            &beastie_view::ViewState::default(),
        )
        .0;
        let mut object = scene.objects[0].clone();
        object.carried = true;
        let relative = object_position(&object, &scene) - crate::creature::head_position(&scene);
        assert!(relative.z > 0.55 && relative.y < 0.0);
    }
}

#[cfg(test)]
mod ui_layout_tests {
    use super::*;

    #[test]
    fn caption_wraps_inside_its_panel_on_both_sides() {
        for x in [1500, 8500] {
            let mut world = beastie_core::WorldState::new(3, "Mop");
            world.creature.aquarium.position.x = x;
            let scene = beastie_view::plan(
                &world,
                &beastie_view::ViewState {
                    speech: Some(
                        "I remember the berry, and I am watching you very carefully today.".into(),
                    ),
                    ..default()
                },
            )
            .0;
            let text = scene
                .text
                .iter()
                .find(|text| text.id == "speech/text")
                .unwrap();
            let panel = scene
                .rects
                .iter()
                .find(|rect| rect.id == "speech/panel")
                .unwrap();
            let bounds = text_content_bounds(text, &scene);
            assert!(bounds.x + bounds.w < (panel.rect.x + panel.rect.w) as f32);
            assert!(bounds.y + bounds.h < (panel.rect.y + panel.rect.h) as f32);
            assert!(bounds.w > 40.0 && bounds.h > 6.0);
        }
    }

    #[test]
    fn partial_modal_overlap_clips_even_when_text_anchor_is_outside() {
        let world = beastie_core::WorldState::new(3, "Mop");
        let mut scene = beastie_view::plan(&world, &beastie_view::ViewState::default()).0;
        scene.rects.clear();
        let text = beastie_view::TextCommand {
            id: "lower".into(),
            text: "Underlapping text".into(),
            x: 10,
            y: 20,
            layer: 10,
            scale: 1,
        };
        let bounds = TextBox {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 20.0,
        };
        let cover = beastie_view::Rect {
            x: 50,
            y: 15,
            w: 20,
            h: 30,
        };
        scene.rects.push(beastie_view::RectCommand {
            id: "modal".into(),
            rect: cover,
            color: [0, 0, 0, 255],
            layer: 20,
            outline: false,
        });
        assert!(!cover.contains(text.x, text.y));
        let visible = visible_text_boxes(&text, bounds, &scene);
        assert_eq!(visible.len(), 2);
        assert!(
            visible
                .iter()
                .all(|part| part.intersection(text_box(cover)).is_none())
        );
        assert_eq!(
            visible.iter().map(|part| part.w * part.h).sum::<f32>(),
            1600.0
        );
    }

    #[test]
    fn all_modal_labels_have_bounded_fitting_at_normal_and_large_size() {
        let world = beastie_core::WorldState::new(3, "Mop");
        for scale in [1, 2] {
            for mode in [
                beastie_view::UiMode::Settings,
                beastie_view::UiMode::Bindings,
                beastie_view::UiMode::FoodChoice,
                beastie_view::UiMode::DataManagement,
            ] {
                let scene = beastie_view::plan(
                    &world,
                    &beastie_view::ViewState {
                        text_scale: scale,
                        mode,
                        ..default()
                    },
                )
                .0;
                for text in &scene.text {
                    let bounds = text_content_bounds(text, &scene);
                    let size = fitting_font_size(text, bounds);
                    assert!(bounds.w > 0.0 && bounds.h > 0.0);
                    assert!(size.is_finite());
                    assert!(size * 1.2 <= bounds.h + 0.001, "{} height", text.id);
                    assert!(bounds.x + bounds.w <= LOGICAL_WIDTH);
                    assert!(bounds.y + bounds.h <= LOGICAL_HEIGHT);
                }
            }
        }
    }

    #[test]
    fn large_settings_labels_stop_before_their_value_controls() {
        let world = beastie_core::WorldState::new(3, "Mop");
        let scene = beastie_view::plan(
            &world,
            &beastie_view::ViewState {
                mode: beastie_view::UiMode::Settings,
                text_scale: 2,
                ..default()
            },
        )
        .0;
        let label = scene
            .text
            .iter()
            .find(|text| text.id == "settings/data-name")
            .expect("Save & data label");
        let bounds = text_content_bounds(label, &scene);
        let label_size = fitting_font_size(label, bounds);
        let other = scene
            .text
            .iter()
            .find(|text| text.id == "settings/effects-volume-name")
            .expect("another settings label");
        let other_size = fitting_font_size(other, text_content_bounds(other, &scene));
        let value = scene
            .text
            .iter()
            .find(|text| text.id == "settings/data-value")
            .expect("Save & data value");
        let value_bounds = text_content_bounds(value, &scene);
        let value_size = fitting_font_size(value, value_bounds);
        assert!(bounds.w < 100.0);
        assert!((label_size - other_size).abs() < 0.01);
        assert!(
            value_size >= 8.0,
            "value size {value_size}, bounds {value_bounds:?}"
        );
    }

    #[test]
    fn settings_value_text_stays_inside_its_button_at_large_size() {
        let world = beastie_core::WorldState::new(3, "Mop");
        let scene = beastie_view::plan(
            &world,
            &beastie_view::ViewState {
                mode: beastie_view::UiMode::Settings,
                text_scale: 2,
                ..default()
            },
        )
        .0;
        for value in scene
            .text
            .iter()
            .filter(|text| text.id.starts_with("settings/") && text.id.ends_with("-value"))
        {
            let bounds = text_content_bounds(value, &scene);
            let button = scene
                .rects
                .iter()
                .find(|rect| {
                    rect.id
                        == format!(
                            "settings/{}-background",
                            value
                                .id
                                .strip_prefix("settings/")
                                .unwrap()
                                .strip_suffix("-value")
                                .unwrap()
                        )
                })
                .expect("value button background");
            assert!(bounds.y >= button.rect.y as f32);
            assert!(bounds.y + bounds.h <= (button.rect.y + button.rect.h) as f32);
        }
    }
}
