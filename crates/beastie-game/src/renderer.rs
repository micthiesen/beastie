//! Native 3D presentation. World forms, controls and lettering are ray-traced geometry.
use std::collections::{BTreeMap, HashMap, HashSet};

use beastie_core::{NormalizedPosition, ToyId};
use beastie_view::{
    HitRegion, HitShape, IconKind, ObjectKind, PresentationCueKind, ScenePlan, UiTarget,
};
use bevy::{camera::ScalingMode, prelude::*, window::PrimaryWindow};

use crate::{environment::object_mesh, host::HostSet, voxel::Geometry};

pub const LOGICAL_WIDTH: f32 = 320.0;
pub const LOGICAL_HEIGHT: f32 = 180.0;
pub const PRESENTATION_WIDTH: f32 = 1280.0;
pub const PRESENTATION_HEIGHT: f32 = 720.0;
const UNITS: f32 = 20.0;
const CAMERA_PITCH: f32 = -12.0 * std::f32::consts::PI / 180.0;

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
    plant: Handle<StandardMaterial>,
    rubber: Handle<StandardMaterial>,
    cloth: Handle<StandardMaterial>,
    brass: Handle<StandardMaterial>,
    food: Handle<StandardMaterial>,
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
    text_geometry: Option<Handle<Mesh>>,
}

pub struct RendererPlugin;
impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjectMeshes>()
            .init_resource::<EffectMesh>()
            .init_resource::<SceneryPicking>()
            .init_resource::<UiCache>()
            .init_resource::<crate::glyphs::Lettering>()
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
            )
            .add_systems(
                PostUpdate,
                sync_scenery_picking
                    .after(bevy::asset::AssetEventSystems)
                    .after(bevy::transform::TransformSystems::Propagate)
                    .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
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
    appearance: Res<crate::appearance::RenderAppearance>,
) {
    let solid = materials.add(appearance.surface(crate::appearance::SurfaceMaterial::Stone));
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
        Msaa::Off,
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        bevy::render::camera::CameraRenderGraph::new(crate::raytrace::RayTracing),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 16.0,
                min_height: 9.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH))
            .with_translation(Quat::from_rotation_x(CAMERA_PITCH) * Vec3::new(0.0, 0.0, 24.0)),
    ));
    crate::environment::setup(
        &mut commands,
        &mut meshes,
        solid.clone(),
        *appearance,
        ui.clone(),
    );
    let cube = meshes.add(Cuboid::default());
    for index in 0..14 {
        commands.spawn((
            Bubble(index),
            Mesh3d(cube.clone()),
            MeshMaterial3d(bubble.clone()),
            Transform::default(),
        ));
    }
    use crate::appearance::SurfaceMaterial;
    commands.insert_resource(Palette {
        solid,
        ui,
        plant: materials.add(appearance.surface(SurfaceMaterial::Plant)),
        rubber: materials.add(appearance.surface(SurfaceMaterial::Rubber)),
        cloth: materials.add(appearance.surface(SurfaceMaterial::Cloth)),
        brass: materials.add(appearance.surface(SurfaceMaterial::Brass)),
        food: materials.add(appearance.surface(SurfaceMaterial::Food)),
    });
}

fn object_key(kind: ObjectKind) -> String {
    format!("{kind:?}")
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

#[allow(clippy::too_many_arguments)] // Independent Bevy resources and component access.
fn sync_objects(
    mut commands: Commands,
    frame: Res<SceneFrame>,
    palette: Res<Palette>,
    motion: Res<crate::creature::CreatureMotion>,
    appearance: Res<crate::appearance::RenderAppearance>,
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
            .or_insert_with(|| meshes.add(appearance.mesh(object_mesh(object.kind, *appearance))))
            .clone();
        commands.spawn((
            WorldObject(object.id),
            Mesh3d(mesh),
            MeshMaterial3d(match object.kind {
                ObjectKind::Food(_) => palette.food.clone(),
                ObjectKind::Toy(ToyId::Ball) => palette.rubber.clone(),
                ObjectKind::Toy(ToyId::Bell) => palette.brass.clone(),
                ObjectKind::Toy(ToyId::Sock) => palette.cloth.clone(),
                ObjectKind::Plant => palette.plant.clone(),
                ObjectKind::Cave => palette.solid.clone(),
            }),
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

fn icon_mesh(kind: IconKind, center: Vec3, color: [u8; 3]) -> Mesh {
    let cell = 0.074;
    let pattern: &[&str] = match kind {
        IconKind::Microphone => &[
            "  xx  ", "  xx  ", "x xx x", "x xx x", " xxxx ", "  xx  ", " xxxx ",
        ],
        IconKind::Food => &["  x   ", " xxx  ", "xxxxx ", "xxxxx ", " xxx  "],
        IconKind::Settings => &[" x  x ", "xxxxxx", "xx  xx", "xx  xx", "xxxxxx", " x  x "],
        IconKind::Send => &[
            "x     ", "xxx   ", "xxxxx ", "xxxxxx", "xxxxx ", "xxx   ", "x     ",
        ],
    };
    let mut model = crate::voxel::VoxelModel::default();
    for (row, line) in pattern.iter().enumerate() {
        for (column, byte) in line.bytes().enumerate() {
            if byte == b'x' {
                model.set([column as i32, -(row as i32), 0], color);
            }
        }
    }
    model
        .mesh_with_style(cell, crate::voxel::SurfaceStyle::Sharp)
        .translated_by(
            center + Vec3::new(-2.5 * cell, (pattern.len() as f32 * 0.5 - 0.5) * cell, 0.0),
        )
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
    let _ = plan;
    text_box(text.bounds.unwrap_or(beastie_view::Rect {
        x: text.x,
        y: text.y,
        w: 300 - text.x,
        h: 180 - text.y,
    }))
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
    let requested = text.role.size(text.scale >= 2);
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
    lettering: ResMut<'w, crate::glyphs::Lettering>,
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
                    shape.cuboid(center + Vec3::Y * y, Vec3::new(w, 0.014, 0.02), color);
                }
                for x in [-w * 0.5, w * 0.5] {
                    shape.cuboid(center + Vec3::X * x, Vec3::new(0.014, h, 0.02), color);
                }
            } else {
                // Calm continuous faces let spacing and typography establish hierarchy.
                // Surface details are authored by the view, not added to every rectangle.
                shape.cuboid(
                    center,
                    Vec3::new(r.w as f32 / UNITS, r.h as f32 / UNITS, 0.02),
                    color,
                );
            }
        }
        let mut replacement = shape.mesh();
        for icon in &ui.frame.plan.icons {
            replacement
                .merge(&icon_mesh(
                    icon.kind,
                    logical_position(
                        icon.x as f32,
                        icon.y as f32,
                        8.05 + icon.layer as f32 * 0.002,
                    ),
                    if ui
                        .frame
                        .plan
                        .hit_regions
                        .iter()
                        .find(|h| h.id == icon.id.replace("ui/button-", "compose/"))
                        .is_some_and(|h| !h.enabled)
                    {
                        [88, 114, 118]
                    } else {
                        [216, 219, 185]
                    },
                ))
                .expect("UI meshes share colored triangle attributes");
        }
        if let Some(handle) = ui.cache.geometry.clone() {
            if let Some(mut mesh) = ui.meshes.get_mut(&handle) {
                *mesh = replacement;
            }
        } else {
            let mesh = ui.meshes.add(replacement);
            let material = ui.palette.ui.clone();
            ui.commands.spawn((
                UiGeometry,
                crate::ray_scene::RayOverlay,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH)),
            ));
            ui.cache.geometry = Some(mesh);
        }
    }
    let text_changed = geometry_changed
        || ui.cache.text != ui.frame.plan.text
        || ui.cache.width != ui.window.width()
        || ui.cache.height != ui.window.height();
    if text_changed {
        let mut lettering_mesh = crate::glyphs::LetterMesh::default();
        let labels = ui.frame.plan.text.clone();
        for text in labels {
            let bounds = text_content_bounds(&text, &ui.frame.plan);
            let to_glyph_bounds = |b: TextBox| crate::glyphs::Bounds {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
            };
            let clips: Vec<_> = visible_text_boxes(&text, bounds, &ui.frame.plan)
                .into_iter()
                .map(to_glyph_bounds)
                .collect();
            ui.lettering.append(
                &mut lettering_mesh,
                crate::glyphs::Label {
                    text: &text.text,
                    bounds: to_glyph_bounds(bounds),
                    clips: &clips,
                    size: fitting_font_size(&text, bounds),
                    centered: text.role.centered(),
                    color: if text.muted {
                        [101, 128, 130]
                    } else {
                        text.role.color()
                    },
                    z: 8.08 + text.layer as f32 * 0.002,
                },
            );
        }
        let replacement = lettering_mesh.mesh();
        if let Some(handle) = ui.cache.text_geometry.clone() {
            if let Some(mut mesh) = ui.meshes.get_mut(&handle) {
                *mesh = replacement;
            }
        } else {
            let mesh = ui.meshes.add(replacement);
            ui.commands.spawn((
                UiText,
                crate::ray_scene::RayOverlay,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(ui.palette.ui.clone()),
                Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH)),
            ));
            ui.cache.text_geometry = Some(mesh);
        }
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

/// Hollow scenery and articulated toy silhouettes use exact presented geometry. Each
/// shared mesh is decoded once; per-object bounds reject unrelated pointer rays cheaply.
#[derive(Resource, Default)]
pub(crate) struct SceneryPicking {
    meshes: HashMap<AssetId<Mesh>, PickMesh>,
    instances: HashMap<u64, (AssetId<Mesh>, Mat4)>,
}

impl SceneryPicking {
    fn hit(&self, id: u64, origin: Vec3, direction: Vec3) -> Option<f32> {
        let (mesh, inverse) = self.instances.get(&id)?;
        self.meshes.get(mesh)?.hit(
            inverse.transform_point3(origin),
            inverse.transform_vector3(direction),
        )
    }
}

struct PickMesh {
    min: Vec3,
    max: Vec3,
    triangles: Vec<[Vec3; 3]>,
}

impl PickMesh {
    fn from_mesh(mesh: &Mesh) -> Option<Self> {
        if mesh.primitive_topology()
            != bevy::render::render_resource::PrimitiveTopology::TriangleList
        {
            return None;
        }
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
        let indices: Vec<usize> = mesh.indices().map_or_else(
            || (0..positions.len()).collect(),
            |indices| indices.iter().collect(),
        );
        let mut result = Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
            triangles: Vec::new(),
        };
        for indices in indices.chunks_exact(3) {
            let [Some(a), Some(b), Some(c)] = [
                positions.get(indices[0]),
                positions.get(indices[1]),
                positions.get(indices[2]),
            ] else {
                continue;
            };
            let vertices = [Vec3::from(*a), Vec3::from(*b), Vec3::from(*c)];
            if !vertices.iter().all(|point| point.is_finite())
                || (vertices[1] - vertices[0])
                    .cross(vertices[2] - vertices[0])
                    .length_squared()
                    < 1e-20
            {
                continue;
            }
            for point in vertices {
                result.min = result.min.min(point);
                result.max = result.max.max(point);
            }
            result.triangles.push(vertices);
        }
        (!result.triangles.is_empty()).then_some(result)
    }

    fn hit(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        if !origin.is_finite() || !direction.is_finite() || direction.length_squared() == 0.0 {
            return None;
        }
        let mut near = 0.0_f32;
        let mut far = f32::INFINITY;
        for axis in 0..3 {
            if direction[axis].abs() < 1e-12 {
                if origin[axis] < self.min[axis] || origin[axis] > self.max[axis] {
                    return None;
                }
            } else {
                let a = (self.min[axis] - origin[axis]) / direction[axis];
                let b = (self.max[axis] - origin[axis]) / direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return None;
                }
            }
        }
        let mut closest: Option<f32> = None;
        for &[a, b, c] in &self.triangles {
            let edge1 = b - a;
            let edge2 = c - a;
            let cross = direction.cross(edge2);
            let determinant = edge1.dot(cross);
            // Double-sided, like the production ray tracer. Keep local direction
            // unnormalized so the parameter still compares against world-space targets.
            if determinant.abs() < 1e-12 {
                continue;
            }
            let delta = origin - a;
            let u = delta.dot(cross) / determinant;
            if !(-1e-5..=1.00001).contains(&u) {
                continue;
            }
            let q = delta.cross(edge1);
            let v = direction.dot(q) / determinant;
            if v < -1e-5 || u + v > 1.00001 {
                continue;
            }
            let distance = edge2.dot(q) / determinant;
            if distance >= 0.0 && closest.is_none_or(|previous| distance < previous) {
                closest = Some(distance);
            }
        }
        closest
    }
}

type SceneryInstances<'w, 's> = Query<
    'w,
    's,
    (
        &'static WorldObject,
        &'static Mesh3d,
        &'static GlobalTransform,
        Option<&'static InheritedVisibility>,
    ),
>;

fn sync_scenery_picking(
    frame: Res<SceneFrame>,
    meshes: Res<Assets<Mesh>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    objects: SceneryInstances,
    mut cache: ResMut<SceneryPicking>,
) {
    for event in events.read() {
        if let AssetEvent::Modified { id } | AssetEvent::Removed { id } = event {
            cache.meshes.remove(id);
        }
    }
    cache.instances.clear();
    let mut used = HashSet::new();
    for (object, mesh, transform, visibility) in &objects {
        if visibility.is_some_and(|visibility| !visibility.get())
            || !frame.plan.objects.iter().any(|plan| {
                plan.id == object.0
                    && matches!(
                        plan.kind,
                        ObjectKind::Plant | ObjectKind::Cave | ObjectKind::Toy(_)
                    )
            })
        {
            continue;
        }
        let inverse = transform.to_matrix().inverse();
        if !inverse.is_finite() {
            continue;
        }
        let Some(asset) = meshes.get(mesh.id()) else {
            continue;
        };
        used.insert(mesh.id());
        if let std::collections::hash_map::Entry::Vacant(entry) = cache.meshes.entry(mesh.id())
            && let Some(geometry) = PickMesh::from_mesh(asset)
        {
            entry.insert(geometry);
        }
        cache.instances.insert(object.0, (mesh.id(), inverse));
    }
    cache.meshes.retain(|id, _| used.contains(id));
}

pub fn pick(
    plan: &ScenePlan,
    camera: &Camera,
    transform: &GlobalTransform,
    cursor: Vec2,
    motion: &crate::creature::CreatureMotion,
    scenery: &SceneryPicking,
) -> Option<HitRegion> {
    let size = camera.logical_viewport_size()?;
    let (x, y) = Viewport::for_drawable(size.x, size.y).logical_point(cursor.x, cursor.y)?;
    if let Some(hit) = plan.hit_regions.iter().rev().find(|hit| {
        (hit.enabled || hit.id == "compose/microphone")
            && matches!(hit.shape, HitShape::Rect)
            && hit.rect.contains(x as i32, y as i32)
    }) {
        return Some(hit.clone());
    }
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let mut selected: Option<(f32, UiTarget)> = None;
    let mut consider = |target: UiTarget, distance: Option<f32>| {
        if let Some(distance) = distance
            && selected.is_none_or(|(near, _)| distance < near)
        {
            selected = Some((distance, target));
        }
    };
    consider(
        UiTarget::Creature,
        ray_ellipsoid(
            ray.origin,
            *ray.direction,
            motion.position(plan),
            Vec3::new(0.75, 0.65, 0.55),
        ),
    );
    // The visible animal includes its articulated body, not just its face. These are
    // the preceding presented transforms, so input cannot race the next animation step.
    for (transform, radii) in motion.body_pick_volumes() {
        consider(
            UiTarget::Creature,
            ray_transformed_ellipsoid(ray.origin, *ray.direction, transform, *radii),
        );
    }
    for object in &plan.objects {
        let scenery_target = match object.kind {
            ObjectKind::Plant => Some(UiTarget::Plant(object.id)),
            ObjectKind::Cave => Some(UiTarget::Cave),
            ObjectKind::Toy(toy) => Some(UiTarget::Toy(toy)),
            _ => None,
        };
        if let Some(target) = scenery_target {
            consider(target, scenery.hit(object.id, ray.origin, *ray.direction));
            continue;
        }
        let (target, radius, offset) = match object.kind {
            ObjectKind::Food(_) => (
                UiTarget::FoodObject(object.id),
                Vec3::splat(0.22),
                Vec3::ZERO,
            ),
            ObjectKind::Cave | ObjectKind::Plant | ObjectKind::Toy(_) => {
                unreachable!("mesh picking handled above")
            }
        };
        consider(
            target,
            ray_ellipsoid(
                ray.origin,
                *ray.direction,
                presented_object_position(object, plan, motion) + offset,
                radius,
            ),
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
fn ray_transformed_ellipsoid(
    origin: Vec3,
    direction: Vec3,
    transform: &Transform,
    radii: Vec3,
) -> Option<f32> {
    let inverse = transform.compute_affine().inverse();
    // Do not normalize the transformed direction: its ray parameter remains world distance.
    ray_ellipsoid(
        inverse.transform_point3(origin),
        inverse.transform_vector3(direction),
        Vec3::ZERO,
        radii,
    )
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
    fn scenery_picking_leaves_plant_gaps_and_cave_openings_empty() {
        let appearance = crate::appearance::RenderAppearance::default();
        let plant = PickMesh::from_mesh(&object_mesh(ObjectKind::Plant, appearance)).unwrap();
        let gap = Vec3::new(0.0, 0.20, 2.0);
        // The former single ellipsoid invented an opaque surface between the leaves.
        assert!(
            ray_ellipsoid(
                gap,
                Vec3::NEG_Z,
                Vec3::new(0.055, -0.5225, 0.03),
                Vec3::new(0.60, 0.85, 0.28)
            )
            .is_some()
        );
        assert!(plant.hit(gap, Vec3::NEG_Z).is_none());
        assert!(plant.hit(Vec3::new(0.20, 0.20, 2.0), Vec3::NEG_Z).is_some());
        // Leaf backs are pickable too, with no culling assumption in the input path.
        assert!(plant.hit(Vec3::new(0.20, 0.20, -2.0), Vec3::Z).is_some());
        let cave = PickMesh::from_mesh(&object_mesh(ObjectKind::Cave, appearance)).unwrap();
        let doorway = cave.hit(Vec3::new(0.0, -0.60, 2.0), Vec3::NEG_Z).unwrap();
        let lip = cave.hit(Vec3::new(1.15, -0.60, 2.0), Vec3::NEG_Z).unwrap();
        assert!(
            doorway > 3.0 && lip < 2.6,
            "doorway must reach the recessed back, not an invented front surface"
        );
    }

    #[test]
    fn shelter_opening_clears_the_head_and_crown_at_the_rest_anchor() {
        let cave = PickMesh::from_mesh(&object_mesh(
            ObjectKind::Cave,
            crate::appearance::RenderAppearance::default(),
        ))
        .unwrap();
        // These rays cross the visible resting head/crown, not just the old tiny doorway.
        // They must reach the back of the shelter instead of hitting a lip through the face.
        for (x, y) in [(-0.65, 0.0), (0.65, 0.0), (0.0, 0.8), (0.0, -0.5)] {
            let depth = cave.hit(Vec3::new(x, y, 2.0), Vec3::NEG_Z).unwrap();
            assert!(
                depth > 3.0,
                "shelter covers the animal at ({x}, {y}): {depth}"
            );
        }
    }

    #[test]
    fn toy_mesh_picking_follows_carried_rotation_and_preserves_bell_loop_gap() {
        let bell =
            PickMesh::from_mesh(&object_mesh(ObjectKind::Toy(ToyId::Bell), default())).unwrap();
        assert!(bell.hit(Vec3::new(0.0, 0.47, 2.0), Vec3::NEG_Z).is_none());
        assert!(bell.hit(Vec3::new(0.104, 0.47, 2.0), Vec3::NEG_Z).is_some());

        let plan = beastie_view::plan(
            &beastie_core::WorldState::new(9, "Pick"),
            &beastie_view::ViewState::default(),
        )
        .0;
        let sock_id = plan
            .objects
            .iter()
            .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
            .unwrap()
            .id;
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<SceneryPicking>()
            .insert_resource(SceneFrame { plan })
            .add_message::<AssetEvent<Mesh>>()
            .add_systems(Update, sync_scenery_picking);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(object_mesh(ObjectKind::Toy(ToyId::Sock), default()));
        let transform =
            Transform::from_xyz(2.0, 1.0, 0.65).with_rotation(Quat::from_rotation_z(-0.15));
        let matrix = transform.to_matrix();
        app.world_mut().spawn((
            WorldObject(sock_id),
            Mesh3d(mesh),
            GlobalTransform::from(transform),
            InheritedVisibility::VISIBLE,
        ));
        app.update();
        let cache = app.world().resource::<SceneryPicking>();
        let cuff = matrix.transform_point3(Vec3::new(0.0, 0.043, 2.0));
        assert!(cache.hit(sock_id, cuff, Vec3::NEG_Z).is_some());
        let empty = matrix.transform_point3(Vec3::new(-0.5, 0.3, 2.0));
        assert!(cache.hit(sock_id, empty, Vec3::NEG_Z).is_none());
    }

    #[test]
    fn scenery_picking_caches_meshes_and_preserves_transformed_world_distance() {
        let plan = beastie_view::plan(
            &beastie_core::WorldState::new(9, "Pick"),
            &beastie_view::ViewState::default(),
        )
        .0;
        let plant_id = plan
            .objects
            .iter()
            .find(|object| object.kind == ObjectKind::Plant)
            .unwrap()
            .id;
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<SceneryPicking>()
            .insert_resource(SceneFrame { plan })
            .add_message::<AssetEvent<Mesh>>()
            .add_systems(Update, sync_scenery_picking);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(2.0, 2.0, 2.0));
        let transform = Transform::from_xyz(2.0, 3.0, 4.0)
            .with_rotation(Quat::from_rotation_y(0.3))
            .with_scale(Vec3::new(1.2, 0.7, 2.0));
        let world_matrix = transform.to_matrix();
        let origin = world_matrix.transform_point3(Vec3::new(0.0, 0.0, 3.0));
        let direction = world_matrix.transform_vector3(Vec3::NEG_Z).normalize();
        let entity = app
            .world_mut()
            .spawn((
                WorldObject(plant_id),
                Mesh3d(mesh.clone()),
                GlobalTransform::from(transform),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.update();
        let cache = app.world().resource::<SceneryPicking>();
        assert!((cache.hit(plant_id, origin, direction).unwrap() - 4.0).abs() < 1e-5);
        let triangles = cache.meshes[&mesh.id()].triangles.as_ptr();
        app.update();
        assert_eq!(
            app.world().resource::<SceneryPicking>().meshes[&mesh.id()]
                .triangles
                .as_ptr(),
            triangles
        );
        // Replacing the asset must invalidate its decoded geometry, even when the handle stays.
        *app.world_mut()
            .resource_mut::<Assets<Mesh>>()
            .get_mut(mesh.id())
            .unwrap() = Cuboid::new(2.0, 2.0, 1.0).mesh().build();
        app.world_mut()
            .write_message(AssetEvent::<Mesh>::Modified { id: mesh.id() });
        app.update();
        assert!(
            (app.world()
                .resource::<SceneryPicking>()
                .hit(plant_id, origin, direction)
                .unwrap()
                - 5.0)
                .abs()
                < 1e-5
        );
        // The last presented transform, including plant sway, owns the pick location.
        app.world_mut()
            .entity_mut(entity)
            .insert(GlobalTransform::from_translation(Vec3::new(20.0, 0.0, 0.0)));
        app.update();
        assert!(
            app.world()
                .resource::<SceneryPicking>()
                .hit(plant_id, origin, direction)
                .is_none()
        );
        app.world_mut()
            .entity_mut(entity)
            .insert(InheritedVisibility::HIDDEN);
        app.update();
        let cache = app.world().resource::<SceneryPicking>();
        assert!(cache.instances.is_empty() && cache.meshes.is_empty());
    }

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
    fn articulated_pick_preserves_distance_through_rotation_and_nonuniform_scale() {
        let transform = Transform::from_xyz(2.0, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::new(2.0, 1.0, 0.5));
        let distance =
            ray_transformed_ellipsoid(Vec3::new(2.0, 1.5, 5.0), Vec3::NEG_Z, &transform, Vec3::ONE)
                .unwrap();
        assert!((distance - (5.0 - 0.5 * (1.0_f32 - 0.75 * 0.75).sqrt())).abs() < 0.0001);
        assert!(
            ray_transformed_ellipsoid(Vec3::new(3.5, 0.0, 5.0), Vec3::NEG_Z, &transform, Vec3::ONE)
                .is_none()
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
            role: beastie_view::TextRole::Body,
            bounds: None,
            muted: false,
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
                settings_page: 2,
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
            .find(|text| text.id == "settings/bindings-name")
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
            value_size >= beastie_view::TextRole::Control.size(true) - 0.01,
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
