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
const PLAY_HEIGHT: f32 = 8.7;
const PLAY_LIFT: f32 = 0.15;
const CAMERA_PITCH: f32 = -12.0 * std::f32::consts::PI / 180.0;
const UI_LAYER_STRIDE: f32 = 0.025;
const ICON_DEPTH: f32 = 0.008;

#[derive(Clone, Copy)]
enum OverlayPart {
    Face,
    Rim,
    Icon,
    Selection,
    Text,
}

/// Every primitive occupies only its own semantic layer. In particular, a lower
/// miniature's nearest voxel face must stay behind a higher panel's background.
fn overlay_depth(layer: i16, part: OverlayPart) -> f32 {
    let offset = match part {
        OverlayPart::Face => 0.001,
        OverlayPart::Rim => 0.003,
        OverlayPart::Icon => 0.012,
        OverlayPart::Selection => 0.018,
        OverlayPart::Text => 0.020,
    };
    8.0 + layer as f32 * UI_LAYER_STRIDE + offset
}

#[derive(Resource)]
pub struct SceneFrame {
    pub plan: ScenePlan,
}
#[derive(Component)]
pub struct TankCamera;
#[derive(Component)]
struct WorldObject(u64);

const OBJECT_TRANSFER_MS: u64 = 200;

#[derive(Clone, Copy)]
struct ObjectTransfer {
    started_ms: u64,
    from: Transform,
}

/// Presentation-only carry transitions. Simulation contact and ownership remain immediate.
#[derive(Component)]
struct ObjectPresentation {
    carried: bool,
    last_ms: u64,
    title_screen: bool,
    previous: Transform,
    current: Transform,
    transfer: Option<ObjectTransfer>,
}

impl ObjectPresentation {
    fn new(object: &beastie_view::ObjectScene, scene: &ScenePlan, pose: Transform) -> Self {
        Self {
            carried: object.carried,
            last_ms: scene
                .elapsed_ms
                .saturating_add(scene.simulation_remainder_ms),
            title_screen: scene.title_screen,
            previous: pose,
            current: pose,
            transfer: None,
        }
    }

    fn advance(
        &mut self,
        object: &beastie_view::ObjectScene,
        scene: &ScenePlan,
        target: Transform,
        rendered: Transform,
    ) -> Transform {
        let now = scene
            .elapsed_ms
            .saturating_add(scene.simulation_remainder_ms);
        if now < self.last_ms
            || now.saturating_sub(self.last_ms) > 1_000
            || target.translation.distance(rendered.translation) > 3.0
            || scene.title_screen
            || scene.title_screen != self.title_screen
        {
            // Loads, clock discontinuities and title staging never replay an old pickup.
            *self = Self::new(object, scene, target);
            return target;
        }
        self.previous = rendered;
        if object.carried != self.carried {
            // Restart from the pose actually shown, including release during pickup.
            self.transfer = Some(ObjectTransfer {
                started_ms: now,
                from: self.previous,
            });
        }
        self.carried = object.carried;
        self.last_ms = now;
        self.current = target;
        if let Some(transfer) = self.transfer {
            let fraction =
                now.saturating_sub(transfer.started_ms) as f32 / OBJECT_TRANSFER_MS as f32;
            if fraction == 0.0 {
                self.current = transfer.from;
            } else if fraction >= 1.0 {
                self.transfer = None;
            } else {
                // Essential continuity also remains with reduced motion. No arc, bounce
                // or decorative rotation is added; the endpoint follows the current mouth.
                let blend = fraction * fraction * (3.0 - 2.0 * fraction);
                self.current.translation =
                    transfer.from.translation.lerp(target.translation, blend);
                self.current.rotation = transfer.from.rotation.slerp(target.rotation, blend);
                self.current.scale = transfer.from.scale.lerp(target.scale, blend);
            }
        }
        self.current
    }
}

#[derive(Component)]
struct UiGeometry;
#[derive(Component)]
struct UiText;
#[derive(Component)]
struct EffectGeometry;
#[derive(Component)]
struct Bubble(usize);
#[derive(Component)]
struct TitleLogo;
#[derive(Resource)]
struct Palette {
    solid: Handle<StandardMaterial>,
    ui: Handle<StandardMaterial>,
    panel: Handle<StandardMaterial>,
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
    shape: EffectShape,
    visible: bool,
}

/// Cache exact authored cuboid inputs before expanding vertices or notifying
/// mesh observers. Equal samples retain their BLAS and GPU geometry allocation.
#[derive(Default, PartialEq)]
struct EffectShape(Vec<(Vec3, Vec3, [u8; 3])>);

impl EffectShape {
    fn cuboid(&mut self, center: Vec3, size: Vec3, color: [u8; 3]) {
        self.0.push((center, size, color));
    }

    fn mesh(&self) -> Mesh {
        let mut geometry = Geometry::default();
        for &(center, size, color) in &self.0 {
            geometry.cuboid(center, size, color);
        }
        geometry.mesh()
    }
}

#[derive(Resource, Default)]
struct UiCache {
    rects: Vec<beastie_view::RectCommand>,
    icons: Vec<beastie_view::IconCommand>,
    text: Vec<beastie_view::TextCommand>,
    text_clips: Vec<Vec<TextBox>>,
    icon_colors: Vec<[u8; 3]>,
    icon_templates: Vec<(IconKind, [u8; 3], Handle<Mesh>)>,
    icon_slots: Vec<Entity>,
    icon_entity: Option<Entity>,
    icons_visible: bool,
    geometry: Option<Handle<Mesh>>,
    text_parent: Option<Entity>,
    text_slots: Vec<TextSlot>,
    text_epoch: u64,
    text_initialized: bool,
    panel_geometry: Option<Handle<Mesh>>,
    panel_entity: Option<Entity>,
}

/// Retain recent inactive labels across dialogs, without keeping unbounded
/// user text history. Each semantic slot stores only its latest built geometry.
const MAX_IDLE_TEXT_SLOTS: usize = 128;

struct TextSlot {
    id: String,
    entity: Entity,
    mesh: Option<Handle<Mesh>>,
    key: Option<(beastie_view::TextCommand, Vec<TextBox>)>,
    active: bool,
    last_used: u64,
}

impl UiCache {
    fn icon(&mut self, meshes: &mut Assets<Mesh>, kind: IconKind, color: [u8; 3]) -> Handle<Mesh> {
        // Object miniatures author their own colors; enabled palettes share them.
        let color = if matches!(kind, IconKind::Toy(_) | IconKind::FoodItem(_)) {
            [0; 3]
        } else {
            color
        };
        let index = self
            .icon_templates
            .iter()
            .position(|(cached_kind, cached_color, _)| {
                *cached_kind == kind && *cached_color == color
            })
            .unwrap_or_else(|| {
                let mesh = meshes.add(icon_mesh(kind, Vec3::ZERO, color));
                self.icon_templates.push((kind, color, mesh));
                self.icon_templates.len() - 1
            });
        self.icon_templates[index].2.clone()
    }
}

#[derive(Clone, Copy, PartialEq)]
struct MeshPose {
    rotation: Quat,
    scale: Vec3,
}

struct SelectionMeasurement {
    pose: MeshPose,
    bounds: Option<(Vec3, Vec2)>,
}

struct GroundMeasurement {
    pose: MeshPose,
    toy: ToyId,
    position: Vec3,
}

/// Keep only the last exact pose per live world mesh, never animation history.
#[derive(Resource, Default)]
struct PoseMeasurements {
    selection: HashMap<AssetId<Mesh>, SelectionMeasurement>,
    grounding: HashMap<AssetId<Mesh>, GroundMeasurement>,
}

impl PoseMeasurements {
    fn selection(
        &mut self,
        id: AssetId<Mesh>,
        mesh: &Mesh,
        transform: &Transform,
    ) -> Option<(Vec3, Vec2)> {
        let pose = MeshPose {
            rotation: transform.rotation,
            scale: transform.scale,
        };
        if self
            .selection
            .get(&id)
            .is_none_or(|entry| entry.pose != pose)
        {
            let local = Transform {
                translation: Vec3::ZERO,
                ..*transform
            };
            self.selection.insert(
                id,
                SelectionMeasurement {
                    pose,
                    bounds: selection_bounds(mesh, &local),
                },
            );
        }
        self.selection[&id]
            .bounds
            .map(|(center, half)| (center + transform.translation, half))
    }

    fn grounding(
        &mut self,
        id: AssetId<Mesh>,
        toy: ToyId,
        mesh: &Mesh,
        rotation: Quat,
        scale: Vec3,
    ) -> Vec3 {
        let pose = MeshPose { rotation, scale };
        if self
            .grounding
            .get(&id)
            .is_none_or(|entry| entry.pose != pose || entry.toy != toy)
        {
            self.grounding.insert(
                id,
                GroundMeasurement {
                    pose,
                    toy,
                    position: grounded_title_toy(toy, mesh, rotation, scale),
                },
            );
        }
        self.grounding[&id].position
    }

    fn asset_event(&mut self, event: &AssetEvent<Mesh>) {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::Removed { id } =
            event
        {
            self.selection.remove(id);
            self.grounding.remove(id);
        }
    }

    fn retain_live(&mut self, live: &HashSet<AssetId<Mesh>>) {
        self.selection.retain(|id, _| live.contains(id));
        self.grounding.retain(|id, _| live.contains(id));
    }
}

fn invalidate_pose_measurements(
    mut events: MessageReader<AssetEvent<Mesh>>,
    objects: Query<&Mesh3d, With<WorldObject>>,
    mut measurements: ResMut<PoseMeasurements>,
) {
    for event in events.read() {
        measurements.asset_event(event);
    }
    let live = objects.iter().map(|mesh| mesh.id()).collect();
    measurements.retain_live(&live);
}

pub struct RendererPlugin;
impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjectMeshes>()
            .init_resource::<EffectMesh>()
            .init_resource::<SceneryPicking>()
            .init_resource::<UiCache>()
            .init_resource::<PoseMeasurements>()
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
                    sync_title,
                )
                    .chain()
                    .after(HostSet::Publish),
            )
            .add_systems(
                PostUpdate,
                invalidate_pose_measurements.after(bevy::asset::AssetEventSystems),
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
        // Vertex colors carry the pale rim and dark underside. A neutral tint
        // avoids multiplying those glints by a second dark cyan color.
        base_color: Color::WHITE,
        unlit: true,
        metallic: 0.0,
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
    // Coated metal retains a broad highlight without tracing sharp scenery reflections.
    let frame_material = materials.add(appearance.material(0.82, 0.25));
    crate::environment::setup(
        &mut commands,
        &mut meshes,
        solid.clone(),
        *appearance,
        ui.clone(),
        frame_material,
    );
    let cube = meshes.add(bubble_mesh());
    for index in 0..24 {
        commands.spawn((
            Bubble(index),
            Mesh3d(cube.clone()),
            MeshMaterial3d(bubble.clone()),
            Transform::default(),
        ));
    }
    commands.spawn((
        TitleLogo,
        Mesh3d(meshes.add(title_logo_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            perceptual_roughness: 0.36,
            diffuse_transmission: 0.18,
            ..default()
        })),
        Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH))
            .with_translation(Quat::from_rotation_x(CAMERA_PITCH) * Vec3::new(0.0, 2.25, 2.7)),
        Visibility::Hidden,
    ));
    use crate::appearance::SurfaceMaterial;
    commands.insert_resource(Palette {
        solid,
        ui,
        panel: materials.add(StandardMaterial {
            unlit: true,
            diffuse_transmission: 0.004,
            ..default()
        }),
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
        return crate::creature::head_position(scene) + carried_toy_offset(scene);
    }
    let fraction = scene.simulation_remainder_ms.min(999) as f32 / 1000.0;
    let mut position = world_position(
        NormalizedPosition::new(
            object
                .position
                .x
                .saturating_add((object.velocity.x as f32 * fraction).round() as i32),
            object
                .position
                .y
                .saturating_add((object.velocity.y as f32 * fraction).round() as i32),
        )
        .clamped(),
    );
    if object.kind == ObjectKind::Toy(ToyId::Sock)
        && object.response == beastie_core::ToyResponse::SockTugged
        && object.velocity.y > 0
    {
        // A released sock begins at the real mouth hold and falls onto the interaction
        // plane. The same halving velocity that drives its fall makes depth continuous
        // across ticks; dropping `carried` must not pop it from the mouth into the head.
        position.z = 0.65
            * (object.velocity.y as f32 / beastie_core::SOCK_RELEASE_SPEED as f32).min(1.0)
            * (1.0 - fraction * 0.5);
    }
    position
}

fn carried_toy_offset(scene: &ScenePlan) -> Vec3 {
    world_position(beastie_core::held_toy_position(scene.creature.position))
        - world_position(scene.creature.position)
        + Vec3::new(0.0, 0.0, 0.65)
}

fn presented_object_position(
    object: &beastie_view::ObjectScene,
    scene: &ScenePlan,
    motion: &crate::creature::CreatureMotion,
) -> Vec3 {
    if object.carried {
        motion.position(scene) + carried_toy_offset(scene)
    } else {
        object_position(object, scene)
    }
}

fn object_transform(
    object: &beastie_view::ObjectScene,
    scene: &ScenePlan,
    motion: &crate::creature::CreatureMotion,
) -> Transform {
    let mut transform =
        Transform::from_translation(presented_object_position(object, scene, motion));
    let time = scene
        .elapsed_ms
        .saturating_add(scene.simulation_remainder_ms) as f32
        / 1000.0;
    if matches!(object.kind, ObjectKind::Plant) && !scene.reduced_motion {
        transform.rotation = Quat::from_rotation_z((time / 1.9 + object.id as f32).sin() * 0.055);
    }
    if object.kind == ObjectKind::Toy(ToyId::Ball) {
        transform.rotation = Quat::from_rotation_z(-transform.translation.x * 1.2);
    }
    if object.kind == ObjectKind::Toy(ToyId::Bell)
        && !scene.reduced_motion
        && !scene.reduced_shake
        && let Some(cue) = scene.effects.iter().find(|cue| {
            cue.target == UiTarget::Toy(ToyId::Bell) && cue.cue == PresentationCueKind::BellStrike
        })
    {
        let elapsed = cue.elapsed_ms.saturating_add(scene.simulation_remainder_ms) as f32 / 1000.0;
        transform.rotation =
            Quat::from_rotation_z((elapsed * 18.0).sin() * (-elapsed * 1.5).exp() * 0.3);
    }
    if object.carried {
        transform.rotation = Quat::from_rotation_z(-0.15);
    }
    transform
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
    mut objects: Query<(
        Entity,
        &WorldObject,
        &mut Transform,
        &mut ObjectPresentation,
    )>,
) {
    for (entity, object, mut transform, mut presentation) in &mut objects {
        let Some(plan) = frame.plan.objects.iter().find(|p| p.id == object.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let target = object_transform(plan, &frame.plan, &motion);
        *transform = presentation.advance(plan, &frame.plan, target, *transform);
    }
    for object in &frame.plan.objects {
        if objects
            .iter()
            .any(|(_, candidate, _, _)| candidate.0 == object.id)
        {
            continue;
        }
        let mesh = cache
            .0
            .entry(object_key(object.kind))
            .or_insert_with(|| meshes.add(appearance.mesh(object_mesh(object.kind, *appearance))))
            .clone();
        let transform = object_transform(object, &frame.plan, &motion);
        let mut entity = commands.spawn((
            WorldObject(object.id),
            ObjectPresentation::new(object, &frame.plan, transform),
            Mesh3d(mesh),
            MeshMaterial3d(match object.kind {
                ObjectKind::Food(_) => palette.food.clone(),
                ObjectKind::Toy(ToyId::Ball) => palette.rubber.clone(),
                ObjectKind::Toy(ToyId::Bell) => palette.brass.clone(),
                ObjectKind::Toy(ToyId::Sock) => palette.cloth.clone(),
                ObjectKind::Plant => palette.plant.clone(),
                ObjectKind::Cave => palette.solid.clone(),
            }),
            transform,
        ));
        // Caves have no sway, carry or title staging animation. Any future actual
        // transform/mesh/visibility change is still covered by static invalidation.
        if object.kind == ObjectKind::Cave {
            entity.insert(crate::ray_scene::RayStatic);
        }
    }
}

fn bubble_mesh() -> Mesh {
    let mut shape = Geometry::default();
    for (x, y, w, h, color) in [
        (-0.4, 0.0, 0.18, 0.55, [124, 180, 177]),
        (0.4, 0.0, 0.18, 0.55, [182, 223, 211]),
        (0.0, 0.4, 0.55, 0.18, [235, 242, 213]),
        (0.0, -0.4, 0.55, 0.18, [81, 142, 147]),
    ] {
        shape.cuboid(Vec3::new(x, y, 0.0), Vec3::new(w, h, 0.17), color);
    }
    shape.mesh()
}

fn title_logo_mesh() -> Mesh {
    // "Beastie" is the game brand; Mop is only the creature's default name.
    // Authored thirteen-row lettering retains filled voxel solids and side walls.
    // Body text continues to use shaped, accessible glyphs.
    let capital_b: &[&str] = &[
        "xxxxxxx  ",
        "xxxxxxxx ",
        "xx    xxx",
        "xx    xxx",
        "xx   xxx ",
        "xxxxxxx  ",
        "xxxxxxxx ",
        "xx    xxx",
        "xx     xx",
        "xx     xx",
        "xx    xxx",
        "xxxxxxxx ",
        "xxxxxxx  ",
    ];
    let e: &[&str] = &[
        "         ",
        "         ",
        "         ",
        "         ",
        "  xxxxx  ",
        " xxxxxxx ",
        "xxx   xxx",
        "xx     xx",
        "xxxxxxxxx",
        "xxxxxxxx ",
        "xx       ",
        " xxxxxxx ",
        "  xxxxxx ",
    ];
    let a: &[&str] = &[
        "         ",
        "         ",
        "         ",
        "         ",
        " xxxxxx  ",
        " xxxxxxx ",
        "      xxx",
        "  xxxxxxx",
        " xxxxxxxx",
        "xxx    xx",
        "xx     xx",
        "xxxxxxxxx",
        " xxxxx xx",
    ];
    let s: &[&str] = &[
        "        ", "        ", "        ", "        ", " xxxxxx ", "xxxxxxxx", "xx      ",
        "xxxxxx  ", " xxxxxx ", "      xx", "      xx", "xxxxxxxx", " xxxxxx ",
    ];
    let t: &[&str] = &[
        "       ", "  xx   ", "  xx   ", "  xx   ", "xxxxxx ", "xxxxxx ", "  xx   ", "  xx   ",
        "  xx   ", "  xx   ", "  xx   ", "  xxxxx", "   xxxx",
    ];
    let i: &[&str] = &[
        "    ", " xx ", " xx ", "    ", "xxx ", "xxx ", " xx ", " xx ", " xx ", " xx ", " xx ",
        "xxxx", "xxxx",
    ];
    let letters = [capital_b, e, a, s, t, i, e];
    let width = letters
        .iter()
        .map(|rows| rows[0].len() as i32 + 2)
        .sum::<i32>()
        - 2;
    let mut model = crate::voxel::VoxelModel::default();
    let mut start = 0;
    for rows in letters {
        for (row, line) in rows.iter().enumerate() {
            for (column, mark) in line.bytes().enumerate() {
                if mark == b'x' {
                    for z in 0..4 {
                        model.set(
                            [start + column as i32 - (width - 1) / 2, 7 - row as i32, z],
                            if z == 3 {
                                // A restrained top-to-bottom ivory shift reads as warm
                                // illumination, while exposed side walls retain depth.
                                let shade = row.min(14) as u8;
                                [247 - shade / 2, 238 - shade, 201 - shade]
                            } else {
                                [172, 153, 105]
                            },
                        );
                    }
                }
            }
        }
        start += rows[0].len() as i32 + 2;
    }
    let mut mesh = model.mesh_with_style(0.085, crate::voxel::SurfaceStyle::Beveled);
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .unwrap()
        .as_float3()
        .unwrap();
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let colors: Vec<[f32; 4]> = normals
        .iter()
        .zip(positions)
        .map(|(normal, position)| {
            let front = normal[2].max(0.0);
            let upper = normal[1].max(0.0);
            let face = Vec3::new(0.94, 0.91, 0.78);
            let wall = Vec3::new(0.49, 0.46, 0.34);
            let base = wall.lerp(face, front) + Vec3::splat(upper * 0.10);
            let gradient = 0.98 + position[1] * 0.025;
            Color::srgb(base.x * gradient, base.y * gradient, base.z * gradient)
                .to_linear()
                .to_f32_array()
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

type TitlePresentation<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static TankCamera>,
        Option<&'static UiGeometry>,
        Option<&'static UiText>,
        Option<&'static TitleLogo>,
        Option<&'static crate::creature::CreaturePart>,
        Option<&'static WorldObject>,
        Option<&'static Mesh3d>,
        &'static mut Transform,
        Option<&'static mut Projection>,
        Option<&'static mut Visibility>,
    ),
>;

fn presentation_extent(drawable: Vec2, title: bool) -> Vec2 {
    let viewport = Viewport::for_drawable(drawable.x, drawable.y);
    drawable / (viewport.scale * UNITS)
        * Vec2::new(1.0, if title { 7.6 / 9.0 } else { PLAY_HEIGHT / 9.0 })
}

fn sync_title(
    frame: Res<SceneFrame>,
    motion: Res<crate::creature::CreatureMotion>,
    meshes: Res<Assets<Mesh>>,
    mut measurements: ResMut<PoseMeasurements>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut entities: TitlePresentation,
) {
    let title = frame.plan.title_screen;
    let rotation = Quat::from_rotation_x(CAMERA_PITCH);
    let height = if title { 7.6 } else { PLAY_HEIGHT };
    let lift = if title { 0.6 } else { PLAY_LIFT };
    let drawable = windows
        .single()
        .map(|window| Vec2::new(window.width(), window.height()))
        .unwrap_or(Vec2::new(LOGICAL_WIDTH, LOGICAL_HEIGHT));
    let extent = presentation_extent(drawable.max(Vec2::ONE), title);
    // The slightly closer play framing retains the upper water edge while
    // bringing the sand toward the interaction rail. Title keeps its own framing.
    // Overlay coordinates remain unchanged, so pointer and keyboard targets agree.
    let overlay = Transform::from_matrix(
        Mat4::from_quat(rotation)
            * Mat4::from_scale_rotation_translation(
                Vec3::new(1.0, height / 9.0, 1.0),
                Quat::IDENTITY,
                Vec3::new(0.0, lift, 0.0),
            ),
    );
    for (camera, ui, text, logo, creature, object, mesh, mut transform, projection, visibility) in
        &mut entities
    {
        if camera.is_some() {
            *transform = Transform::from_rotation(rotation)
                .with_translation(rotation * Vec3::new(0.0, lift, 24.0));
            if let Some(mut projection) = projection
                && let Projection::Orthographic(ref mut orthographic) = *projection
            {
                orthographic.scaling_mode = ScalingMode::Fixed {
                    width: extent.x,
                    height: extent.y,
                };
            }
        } else if ui.is_some() || text.is_some() {
            *transform = overlay;
        } else if logo.is_some() {
            *transform = Transform::from_rotation(rotation * Quat::from_rotation_y(-0.16))
                .with_translation(rotation * Vec3::new(0.0, 2.6, 2.7));
            if let Some(mut visibility) = visibility {
                *visibility = if title {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        } else if title
            && (creature.is_some()
                || object.is_some_and(|object| {
                    frame
                        .plan
                        .objects
                        .iter()
                        .any(|p| p.id == object.0 && p.carried)
                }))
        {
            // Staging changes only rendered transforms after animation. Simulation,
            // motion history, object ownership and the resumed gameplay are untouched.
            stage_title_creature(&mut transform, motion.presented_head);
        } else if title
            && let Some(object) = object
            && let Some(plan) = frame.plan.objects.iter().find(|p| p.id == object.0)
            && let ObjectKind::Toy(toy) = plan.kind
        {
            // Only the noninteractive title stages uncarried toys. sync_objects
            // restores their actual positions before every subsequent play frame.
            if let Some(handle) = mesh
                && let Some(mesh) = meshes.get(&handle.0)
            {
                transform.translation = measurements.grounding(
                    handle.id(),
                    toy,
                    mesh,
                    transform.rotation,
                    transform.scale,
                );
            }
        }
    }
}

fn stage_title_creature(transform: &mut Transform, head: Vec3) {
    let anchor = Vec3::new(-3.35, 1.65, head.z);
    transform.translation = anchor + (transform.translation - head) * 0.82;
    transform.scale *= 0.82;
}

fn grounded_title_toy(toy: ToyId, mesh: &Mesh, rotation: Quat, scale: Vec3) -> Vec3 {
    let anchor = match toy {
        ToyId::Ball => Vec2::new(-3.15, 1.05),
        ToyId::Bell => Vec2::new(3.5, 0.50),
        ToyId::Sock => Vec2::new(5.6, 0.65),
    };
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let y = positions
        .iter()
        .map(|p| {
            let point = rotation * (Vec3::from_array(*p) * scale);
            crate::environment::substrate_surface_height(anchor.x + point.x, anchor.y + point.z)
                - point.y
        })
        .fold(f32::NEG_INFINITY, f32::max);
    Vec3::new(anchor.x, y, anchor.y)
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
            if bubble.0 % 3 == 0 {
                -4.8
            } else if bubble.0 % 3 == 1 {
                4.7
            } else {
                -6.6 + (i * 2.73).rem_euclid(13.2)
            } + (t * 0.3 + i).sin() * 0.18,
            y,
            -1.4,
        );
        transform.scale = Vec3::splat(0.060 + (i % 4.0) * 0.024);
        transform.rotation = Quat::from_rotation_z((t * 0.2 + i).sin() * 0.12);
    }
}

fn icon_mesh(kind: IconKind, center: Vec3, color: [u8; 3]) -> Mesh {
    let mesh = match kind {
        IconKind::Toy(toy) => object_mesh(ObjectKind::Toy(toy), default()),
        IconKind::FoodItem(food) => object_mesh(ObjectKind::Food(food), default()),
        _ => utility_icon_mesh(kind, color),
    };
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for point in positions {
        min = min.min(Vec3::from(*point));
        max = max.max(Vec3::from(*point));
    }
    let extent = max - min;
    let scale = 1.0 / extent.truncate().max_element();
    // Keep the actual voxel silhouette/palette, with its depth inside the UI band.
    // Art bounds scale XY only, so a larger miniature cannot pierce a modal above it.
    let depth_scale = if extent.z > 0.0 {
        ICON_DEPTH / extent.z
    } else {
        1.0
    };
    mesh.translated_by(-(min + max) * 0.5)
        .scaled_by(Vec3::new(scale, scale, depth_scale))
        .translated_by(center)
}

fn icon_transform(icon: &beastie_view::IconCommand) -> Transform {
    let scale = icon.bounds.w.min(icon.bounds.h).max(0) as f32 / UNITS;
    Transform::from_translation(logical_position(
        icon.bounds.x as f32 + icon.bounds.w as f32 * 0.5,
        icon.bounds.y as f32 + icon.bounds.h as f32 * 0.5,
        overlay_depth(icon.layer, OverlayPart::Icon),
    ))
    .with_scale(Vec3::new(scale, scale, 1.0))
}

#[cfg(test)]
fn test_icon_kinds() -> [IconKind; 16] {
    use beastie_core::FoodId;
    [
        IconKind::Food,
        IconKind::Microphone,
        IconKind::Send,
        IconKind::Settings,
        IconKind::Close,
        IconKind::ChevronRight,
        IconKind::Inspect,
        IconKind::Heart,
        IconKind::Rename,
        IconKind::Play,
        IconKind::Toy(ToyId::Ball),
        IconKind::Toy(ToyId::Bell),
        IconKind::Toy(ToyId::Sock),
        IconKind::FoodItem(FoodId::Berry),
        IconKind::FoodItem(FoodId::Mushroom),
        IconKind::FoodItem(FoodId::Pellet),
    ]
}

/// Smooth utility marks share canonical meshes; physical objects keep voxel miniatures.
fn utility_icon_mesh(kind: IconKind, color: [u8; 3]) -> Mesh {
    use std::f32::consts::{PI, TAU};
    let mut shape = Geometry::default();
    match kind {
        IconKind::Microphone => {
            rounded_plate(
                &mut shape,
                Vec3::new(0.0, 0.18, 0.0),
                Vec2::new(0.29, 0.61),
                color,
                0.145,
            );
            icon_arc(
                &mut shape,
                Vec2::new(0.0, -0.02),
                0.3,
                PI,
                TAU,
                0.075,
                color,
            );
            for x in [-0.3, 0.3] {
                icon_stroke(
                    &mut shape,
                    Vec2::new(x, -0.02),
                    Vec2::new(x, 0.13),
                    0.075,
                    color,
                );
            }
            icon_stroke(
                &mut shape,
                Vec2::new(0.0, -0.32),
                Vec2::new(0.0, -0.48),
                0.075,
                color,
            );
            icon_stroke(
                &mut shape,
                Vec2::new(-0.14, -0.48),
                Vec2::new(0.14, -0.48),
                0.075,
                color,
            );
        }
        IconKind::Settings => {
            let outer: Vec<_> = (0..64)
                .map(|index| {
                    let angle = index as f32 * TAU / 64.0;
                    let radius = if matches!(index % 8, 1..=4) { 0.5 } else { 0.4 };
                    Vec2::new(angle.cos(), angle.sin()) * radius
                })
                .collect();
            let inner: Vec<_> = (0..64)
                .map(|index| {
                    let angle = index as f32 * TAU / 64.0;
                    Vec2::new(angle.cos(), angle.sin()) * 0.18
                })
                .collect();
            flat_ring(&mut shape, Vec3::ZERO, &outer, &inner, color, false);
        }
        IconKind::Food => {
            let points: Vec<_> = (0..64)
                .map(|index| {
                    let angle = index as f32 * TAU / 64.0;
                    Vec2::new(angle.cos(), angle.sin()) * (0.4 + 0.075 * (angle * 4.0).cos())
                })
                .collect();
            flat_fan(&mut shape, Vec3::ZERO, &points, color);
        }
        IconKind::Send => {
            // Two wings retain the paper plane's open central fold at small sizes.
            for sign in [-1.0, 1.0] {
                front_triangle(
                    &mut shape,
                    [
                        Vec3::new(-0.48, sign * 0.42, 0.0),
                        Vec3::new(0.52, 0.0, 0.0),
                        Vec3::new(-0.26, sign * 0.07, 0.0),
                    ],
                    color,
                );
            }
        }
        IconKind::Close => {
            for sign in [-1.0, 1.0] {
                icon_stroke(
                    &mut shape,
                    Vec2::new(-0.34, -0.34 * sign),
                    Vec2::new(0.34, 0.34 * sign),
                    0.12,
                    color,
                );
            }
        }
        IconKind::ChevronRight => {
            icon_stroke(
                &mut shape,
                Vec2::new(-0.22, 0.38),
                Vec2::new(0.22, 0.0),
                0.11,
                color,
            );
            icon_stroke(
                &mut shape,
                Vec2::new(0.22, 0.0),
                Vec2::new(-0.22, -0.38),
                0.11,
                color,
            );
        }
        IconKind::Inspect => {
            icon_arc(&mut shape, Vec2::new(-0.1, 0.1), 0.29, 0.0, TAU, 0.1, color);
            icon_stroke(
                &mut shape,
                Vec2::new(0.12, -0.12),
                Vec2::new(0.43, -0.43),
                0.12,
                color,
            );
        }
        IconKind::Heart => {
            // This heart is star-shaped about its center, including the upper cleft.
            let points: Vec<_> = (0..64)
                .map(|index| {
                    let t = index as f32 * TAU / 64.0;
                    Vec2::new(
                        16.0 * t.sin().powi(3),
                        13.0 * t.cos()
                            - 5.0 * (2.0 * t).cos()
                            - 2.0 * (3.0 * t).cos()
                            - (4.0 * t).cos(),
                    ) / 32.0
                })
                .collect();
            flat_fan(&mut shape, Vec3::ZERO, &points, color);
        }
        IconKind::Rename => {
            let direction = Vec2::new(1.0, 1.0).normalize();
            let normal = Vec2::new(-direction.y, direction.x) * 0.12;
            let start = Vec2::new(-0.26, -0.26);
            let end = Vec2::new(0.35, 0.35);
            let points = [start + normal, start - normal, end - normal, end + normal];
            flat_fan(
                &mut shape,
                ((start + end) * 0.5).extend(0.0),
                &points.map(|p| p - (start + end) * 0.5),
                color,
            );
            front_triangle(
                &mut shape,
                [
                    Vec3::new(-0.47, -0.47, 0.0),
                    (start - normal).extend(0.0),
                    (start + normal).extend(0.0),
                ],
                color,
            );
        }
        IconKind::Play => front_triangle(
            &mut shape,
            [
                Vec3::new(-0.34, -0.45, 0.0),
                Vec3::new(0.44, 0.0, 0.0),
                Vec3::new(-0.34, 0.45, 0.0),
            ],
            color,
        ),
        IconKind::Toy(_) | IconKind::FoodItem(_) => {
            unreachable!("physical miniatures use object geometry")
        }
    }
    shape.mesh()
}

fn icon_stroke(shape: &mut Geometry, a: Vec2, b: Vec2, width: f32, color: [u8; 3]) {
    let delta = b - a;
    let rotation = Vec2::from_angle(delta.y.atan2(delta.x));
    let points: Vec<_> = rounded_perimeter(Vec2::new(delta.length() + width, width), width * 0.5)
        .into_iter()
        .map(|point| rotation.rotate(point))
        .collect();
    flat_fan(shape, ((a + b) * 0.5).extend(0.0), &points, color);
}

fn icon_arc(
    shape: &mut Geometry,
    center: Vec2,
    radius: f32,
    start: f32,
    end: f32,
    width: f32,
    color: [u8; 3],
) {
    let steps = ((end - start).abs() * 12.0).ceil() as usize;
    let point = |step: usize, radius: f32| {
        let angle = start + (end - start) * step as f32 / steps as f32;
        center + Vec2::new(angle.cos(), angle.sin()) * radius
    };
    for step in 0..steps {
        let a = point(step, radius + width * 0.5).extend(0.0);
        let b = point(step + 1, radius + width * 0.5).extend(0.0);
        let c = point(step + 1, radius - width * 0.5).extend(0.0);
        let d = point(step, radius - width * 0.5).extend(0.0);
        front_triangle(shape, [a, b, c], color);
        front_triangle(shape, [a, c, d], color);
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

#[derive(bevy::ecs::system::SystemParam)]
struct UiSystem<'w, 's> {
    commands: Commands<'w, 's>,
    frame: Res<'w, SceneFrame>,
    palette: Res<'w, Palette>,
    lettering: ResMut<'w, crate::glyphs::Lettering>,
    cache: ResMut<'w, UiCache>,
    meshes: ResMut<'w, Assets<Mesh>>,
    timing: Option<Res<'w, crate::ray_stats::ComputeGpuTiming>>,
}

/// Eight segments per corner stay smooth at the largest supported native window.
/// Duplicate tangent points on circles are harmless: front_triangle removes them.
fn rounded_perimeter(size: Vec2, radius: f32) -> Vec<Vec2> {
    let half = size * 0.5;
    let radius = radius.clamp(0.0, half.min_element().max(0.0));
    let mut points = Vec::with_capacity(36);
    for corner in 0..4 {
        let angle = corner as f32 * std::f32::consts::FRAC_PI_2;
        let center = match corner {
            0 => Vec2::new(half.x - radius, half.y - radius),
            1 => Vec2::new(-half.x + radius, half.y - radius),
            2 => Vec2::new(-half.x + radius, -half.y + radius),
            _ => Vec2::new(half.x - radius, -half.y + radius),
        };
        for step in 0..=8 {
            let theta = angle + step as f32 * std::f32::consts::FRAC_PI_2 / 8.0;
            points.push(center + Vec2::new(theta.cos(), theta.sin()) * radius);
        }
    }
    points
}

fn front_triangle(shape: &mut Geometry, mut points: [Vec3; 3], color: [u8; 3]) {
    let area = (points[1] - points[0]).cross(points[2] - points[0]).z;
    if area.abs() < 1e-10 {
        return;
    }
    if area < 0.0 {
        points.swap(1, 2);
    }
    shape.triangle(points, color);
}

fn flat_fan(shape: &mut Geometry, center: Vec3, points: &[Vec2], color: [u8; 3]) {
    for index in 0..points.len() {
        front_triangle(
            shape,
            [
                center,
                center + points[index].extend(0.0),
                center + points[(index + 1) % points.len()].extend(0.0),
            ],
            color,
        );
    }
}

fn flat_ring(
    shape: &mut Geometry,
    center: Vec3,
    outer: &[Vec2],
    inner: &[Vec2],
    color: [u8; 3],
    highlight: bool,
) {
    for index in 0..outer.len() {
        let next = (index + 1) % outer.len();
        let [a, b, c, d] = [outer[index], outer[next], inner[next], inner[index]]
            .map(|point| center + point.extend(0.0));
        let color = if highlight && outer[index].y + outer[next].y > 0.0 {
            color.map(|value| value.saturating_add(7))
        } else {
            color
        };
        front_triangle(shape, [a, b, c], color);
        front_triangle(shape, [a, c, d], color);
    }
}

/// Overlay surfaces need only their front face; no world lighting rays or back walls.
fn rounded_plate(shape: &mut Geometry, center: Vec3, size: Vec2, color: [u8; 3], radius: f32) {
    if size.min_element() <= 0.0 {
        return;
    }
    flat_fan(shape, center, &rounded_perimeter(size, radius), color);
}

fn rounded_rim(shape: &mut Geometry, center: Vec3, size: Vec2, color: [u8; 3], radius: f32) {
    if size.min_element() <= 0.0 {
        return;
    }
    let stroke = 0.018_f32.min(size.min_element() * 0.5);
    let radius = radius.min(size.min_element() * 0.5);
    let outer = rounded_perimeter(size, radius);
    let inner = rounded_perimeter(
        (size - Vec2::splat(stroke * 2.0)).max(Vec2::ZERO),
        (radius - stroke).max(0.0),
    );
    flat_ring(shape, center, &outer, &inner, color, true);
}

fn sync_ui(mut ui: UiSystem) {
    let _span = ui.timing.as_ref().map(|timing| timing.cpu_span("sync_ui"));
    let rects_changed = ui.cache.rects != ui.frame.plan.rects;
    let icon_colors: Vec<_> = ui
        .frame
        .plan
        .icons
        .iter()
        .map(|icon| {
            if ui
                .frame
                .plan
                .hit_regions
                .iter()
                .find(|hit| hit.id == icon.id.replace("ui/button-", "compose/"))
                .is_some_and(|hit| !hit.enabled)
            {
                [88, 114, 118]
            } else {
                beastie_view::TextRole::Control.color()
            }
        })
        .collect();
    let icons_changed =
        ui.cache.icons != ui.frame.plan.icons || ui.cache.icon_colors != icon_colors;
    if rects_changed {
        let panel_changed = ui
            .cache
            .rects
            .iter()
            .filter(|rect| rect.id == "settings/panel")
            .ne(ui
                .frame
                .plan
                .rects
                .iter()
                .filter(|rect| rect.id == "settings/panel"));
        let mut shape = Geometry::default();
        let mut panel = Geometry::default();
        let mut has_panel = false;
        for rect in &ui.frame.plan.rects {
            let r = rect.rect;
            let center = logical_position(
                r.x as f32 + r.w as f32 * 0.5,
                r.y as f32 + r.h as f32 * 0.5,
                overlay_depth(
                    rect.layer,
                    if rect.outline {
                        OverlayPart::Rim
                    } else {
                        OverlayPart::Face
                    },
                ),
            );
            let color = [rect.color[0], rect.color[1], rect.color[2]];
            let size = Vec2::new(r.w as f32 / UNITS, r.h as f32 / UNITS);
            let corner = rect.corner_radius as f32 / UNITS;
            if rect.id == "settings/panel" {
                if panel_changed {
                    rounded_plate(&mut panel, center, size, color, corner);
                }
                has_panel = true;
                continue;
            }
            if rect.outline {
                rounded_rim(&mut shape, center, size, color, corner);
            } else {
                rounded_plate(&mut shape, center, size, color, corner);
            }
        }

        if has_panel {
            if let Some(handle) = ui.cache.panel_geometry.clone() {
                if panel_changed {
                    if let Some(mut mesh) = ui.meshes.get_mut(&handle) {
                        *mesh = panel.mesh();
                    }
                    if let Some(entity) = ui.cache.panel_entity {
                        ui.commands.entity(entity).insert(Visibility::Visible);
                    }
                }
            } else {
                let mesh = ui.meshes.add(panel.mesh());
                let material = ui.palette.panel.clone();
                let entity = ui
                    .commands
                    .spawn((
                        UiGeometry,
                        crate::ray_scene::RayOverlay,
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH)),
                    ))
                    .id();
                ui.cache.panel_geometry = Some(mesh);
                ui.cache.panel_entity = Some(entity);
            }
        } else if panel_changed && let Some(entity) = ui.cache.panel_entity {
            ui.commands.entity(entity).insert(Visibility::Hidden);
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
                crate::ray_scene::RayOverlay,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH)),
            ));
            ui.cache.geometry = Some(mesh);
        }
    }
    // One canonical asset per icon kind/palette, instanced by persistent slots.
    // Presentation transforms belong to the parent; each slot only translates
    // its canonical mesh. Position changes never invalidate a mesh or its BLAS.
    if ui.frame.plan.icons.is_empty() {
        if ui.cache.icons_visible {
            if let Some(entity) = ui.cache.icon_entity {
                ui.commands.entity(entity).insert(Visibility::Hidden);
            }
            ui.cache.icons_visible = false;
        }
    } else {
        if icons_changed {
            let parent = if let Some(entity) = ui.cache.icon_entity {
                entity
            } else {
                let entity = ui
                    .commands
                    .spawn((
                        UiGeometry,
                        Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH)),
                        Visibility::Visible,
                    ))
                    .id();
                ui.cache.icon_entity = Some(entity);
                entity
            };
            let icons = ui.frame.plan.icons.clone();
            for (index, (icon, &color)) in icons.iter().zip(&icon_colors).enumerate() {
                let transform = icon_transform(icon);
                let handle = ui.cache.icon(&mut ui.meshes, icon.kind, color);
                if let Some(&entity) = ui.cache.icon_slots.get(index) {
                    ui.commands.entity(entity).insert((
                        Mesh3d(handle),
                        transform,
                        Visibility::Inherited,
                    ));
                } else {
                    let entity = ui
                        .commands
                        .spawn((
                            crate::ray_scene::RayOverlay,
                            Mesh3d(handle),
                            MeshMaterial3d(ui.palette.ui.clone()),
                            transform,
                            Visibility::Inherited,
                        ))
                        .id();
                    ui.commands.entity(parent).add_child(entity);
                    ui.cache.icon_slots.push(entity);
                }
            }
            for &entity in &ui.cache.icon_slots[icons.len()..] {
                ui.commands.entity(entity).insert(Visibility::Hidden);
            }
            ui.cache.icons = icons;
            ui.cache.icon_colors = icon_colors;
        }
        if !ui.cache.icons_visible {
            if let Some(entity) = ui.cache.icon_entity {
                ui.commands.entity(entity).insert(Visibility::Visible);
            }
            ui.cache.icons_visible = true;
        }
    }
    // Text uses logical coordinates. Only label inputs and their actual clipped
    // regions affect tessellation, not rectangle colors, icons or window pixels.
    let labels_changed = ui.cache.text != ui.frame.plan.text;
    let clips = (labels_changed || rects_changed || !ui.cache.text_initialized).then(|| {
        ui.frame
            .plan
            .text
            .iter()
            .map(|text| {
                visible_text_boxes(
                    text,
                    text_content_bounds(text, &ui.frame.plan),
                    &ui.frame.plan,
                )
            })
            .collect::<Vec<_>>()
    });
    let text_changed = labels_changed
        || !ui.cache.text_initialized
        || clips
            .as_ref()
            .is_some_and(|clips| *clips != ui.cache.text_clips);
    if text_changed {
        let clips = clips.unwrap_or_else(|| ui.cache.text_clips.clone());
        sync_label_meshes(&mut ui, &clips);
        ui.cache.text = ui.frame.plan.text.clone();
        ui.cache.text_clips = clips;
        ui.cache.text_initialized = true;
    }
    if rects_changed {
        ui.cache.rects = ui.frame.plan.rects.clone();
    }
}

fn label_mesh(
    lettering: &mut crate::glyphs::Lettering,
    text: &beastie_view::TextCommand,
    visible: &[TextBox],
    plan: &ScenePlan,
) -> Mesh {
    let bounds = text_content_bounds(text, plan);
    let to_glyph_bounds = |b: TextBox| crate::glyphs::Bounds {
        x: b.x,
        y: b.y,
        w: b.w,
        h: b.h,
    };
    let clips: Vec<_> = visible.iter().copied().map(to_glyph_bounds).collect();
    let size = text.role.size(text.scale >= 2);
    let mut content_bounds = to_glyph_bounds(bounds);
    if text.input_state == Some(beastie_view::TextInputState::Caret) {
        let reserve = crate::glyphs::CARET_SPACE.min(content_bounds.w.max(0.0));
        content_bounds.w -= reserve;
        if !text.keep_tail {
            content_bounds.x += reserve;
        }
    }
    let tail = (text.keep_tail && !clips.is_empty())
        .then(|| lettering.tail_line(&text.text, content_bounds, size));
    let mut mesh = crate::glyphs::LetterMesh::default();
    let label = crate::glyphs::Label {
        text: tail.as_deref().unwrap_or(&text.text),
        bounds: content_bounds,
        clips: &clips,
        size,
        centered: text.role.centered(),
        vertical_centered: text.vertical_centered,
        color: if text.muted {
            [101, 128, 130]
        } else {
            text.role.color()
        },
        z: overlay_depth(text.layer, OverlayPart::Text),
    };
    if let Some(state) = text.input_state {
        let (color, part) = match state {
            beastie_view::TextInputState::Selected => ([25, 103, 101], OverlayPart::Selection),
            beastie_view::TextInputState::Caret => ([177, 212, 199], OverlayPart::Text),
        };
        lettering.append_input_decoration(
            &mut mesh,
            crate::glyphs::Label {
                text: if text.keep_tail { label.text } else { "" },
                bounds: to_glyph_bounds(bounds),
                color,
                z: overlay_depth(text.layer, part),
                ..label
            },
            state,
        );
    }
    lettering.append(&mut mesh, label);
    mesh.mesh()
}

fn sync_label_meshes(ui: &mut UiSystem<'_, '_>, clips: &[Vec<TextBox>]) {
    ui.cache.text_epoch = ui.cache.text_epoch.wrapping_add(1);
    let epoch = ui.cache.text_epoch;
    for slot in &mut ui.cache.text_slots {
        slot.active = false;
    }
    let labels = ui.frame.plan.text.clone();
    if !labels.is_empty() && ui.cache.text_parent.is_none() {
        let entity = ui
            .commands
            .spawn((
                UiText,
                Transform::from_rotation(Quat::from_rotation_x(CAMERA_PITCH)),
                Visibility::Visible,
            ))
            .id();
        ui.cache.text_parent = Some(entity);
    }
    for (text, visible) in labels.iter().zip(clips) {
        // Consume each matching slot once, including repeated semantic IDs.
        let index = ui
            .cache
            .text_slots
            .iter()
            .position(|slot| !slot.active && slot.id == text.id)
            .unwrap_or_else(|| {
                let entity = ui
                    .commands
                    .spawn((
                        crate::ray_scene::RayOverlay,
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                ui.commands
                    .entity(ui.cache.text_parent.unwrap())
                    .add_child(entity);
                ui.cache.text_slots.push(TextSlot {
                    id: text.id.clone(),
                    entity,
                    mesh: None,
                    key: None,
                    active: false,
                    last_used: epoch,
                });
                ui.cache.text_slots.len() - 1
            });
        let slot = &mut ui.cache.text_slots[index];
        slot.active = true;
        slot.last_used = epoch;
        if visible.is_empty() {
            ui.commands.entity(slot.entity).insert(Visibility::Hidden);
            continue;
        }
        let unchanged = slot.key.as_ref().is_some_and(|(previous, previous_clips)| {
            previous == text && previous_clips == visible
        });
        if !unchanged {
            let replacement = label_mesh(&mut ui.lettering, text, visible, &ui.frame.plan);
            if let Some(handle) = &slot.mesh {
                if let Some(mut mesh) = ui.meshes.get_mut(handle) {
                    *mesh = replacement;
                }
            } else {
                let mesh = ui.meshes.add(replacement);
                ui.commands
                    .entity(slot.entity)
                    .insert((Mesh3d(mesh.clone()), MeshMaterial3d(ui.palette.ui.clone())));
                slot.mesh = Some(mesh);
            }
            slot.key = Some((text.clone(), visible.clone()));
        }
        ui.commands
            .entity(slot.entity)
            .insert(Visibility::Inherited);
    }
    for slot in ui.cache.text_slots.iter().filter(|slot| !slot.active) {
        ui.commands.entity(slot.entity).insert(Visibility::Hidden);
    }
    // Evict only inactive slots, oldest first. Active labels are never dropped.
    while ui
        .cache
        .text_slots
        .iter()
        .filter(|slot| !slot.active)
        .count()
        > MAX_IDLE_TEXT_SLOTS
    {
        let oldest = ui
            .cache
            .text_slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| !slot.active)
            .min_by_key(|(_, slot)| slot.last_used)
            .map(|(index, _)| index)
            .unwrap();
        let removed = ui.cache.text_slots.swap_remove(oldest);
        ui.commands.entity(removed.entity).despawn();
    }
}

/// Lay out a row-major pixel pattern of cubes centered on `center`. Characters map to colors;
/// spaces are empty.
fn pixel_pattern(
    shape: &mut EffectShape,
    center: Vec3,
    rows: &[&str],
    pixel: f32,
    color: impl Fn(u8) -> Option<[u8; 3]>,
) {
    let height = rows.len() as f32;
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0) as f32;
    for (row, line) in rows.iter().enumerate() {
        for (column, byte) in line.bytes().enumerate() {
            if let Some(rgb) = color(byte) {
                shape.cuboid(
                    center
                        + Vec3::new(
                            (column as f32 - (width - 1.0) / 2.0) * pixel,
                            ((height - 1.0) / 2.0 - row as f32) * pixel,
                            0.0,
                        ),
                    Vec3::new(pixel, pixel, pixel * 0.6),
                    rgb,
                );
            }
        }
    }
}

/// "Not that": the refused thing in a bubble with a red cross over it.
fn refusal_bubble(shape: &mut EffectShape, head: Vec3, refused: beastie_core::Meaning, time: f32) {
    let want = match refused {
        beastie_core::Meaning::Toy(toy) => beastie_core::Want::Toy(toy),
        beastie_core::Meaning::Food(food) => beastie_core::Want::Food(Some(food)),
        _ => return,
    };
    want_bubble(shape, head, want, time, true);
    // A red cross beside the pictogram, so the thing refused stays recognizable.
    let center = head + Vec3::new(1.5, 1.5, 0.62);
    for step in -2..=2 {
        let offset = step as f32 * 0.045;
        for (dx, dy) in [(offset, offset), (offset, -offset)] {
            shape.cuboid(
                center + Vec3::new(dx, dy, 0.0),
                Vec3::splat(0.055),
                [214, 72, 66],
            );
        }
    }
}

/// A soft pulsing ring around whatever the creature is heading for, so its intent reads at a
/// glance before it arrives.
fn intent_ring(shape: &mut EffectShape, center: Vec3, time: f32, reduced_motion: bool) {
    let pulse = if reduced_motion {
        1.0
    } else {
        1.0 + (time * 4.0).sin() * 0.08
    };
    let radius = 0.5 * pulse;
    for index in 0..22 {
        let angle = index as f32 * std::f32::consts::TAU / 22.0 + time * 0.6;
        shape.cuboid(
            center + Vec3::new(angle.cos() * radius, angle.sin() * radius * 0.9, 0.55),
            Vec3::splat(0.05),
            [255, 226, 140],
        );
    }
}

/// Little stars and notes rising around a creature in the middle of a game.
fn play_sparkles(shape: &mut EffectShape, head: Vec3, time: f32) {
    for index in 0..4 {
        let phase = (time * 0.9 + index as f32 * 0.25).fract();
        let side = if index % 2 == 0 { -1.0 } else { 1.0 };
        let center = head + Vec3::new(side * (0.55 + phase * 0.25), 0.35 + phase * 0.9, 0.6);
        let size = 0.05 * (1.0 - phase) + 0.02;
        let color = if index % 2 == 0 {
            [255, 224, 128]
        } else {
            [255, 246, 222]
        };
        shape.cuboid(center, Vec3::splat(size), color);
        shape.cuboid(
            center + Vec3::new(size, size, 0.0),
            Vec3::splat(size * 0.5),
            color,
        );
    }
}

/// A thought bubble above the head holding a pictogram of what the creature wants.
fn want_bubble(
    shape: &mut EffectShape,
    head: Vec3,
    want: beastie_core::Want,
    time: f32,
    reduced_motion: bool,
) {
    use beastie_core::{FoodId, Meaning, ToyId, Want};
    let bob = if reduced_motion {
        0.0
    } else {
        (time * 2.2).sin() * 0.035
    };
    let pixel = 0.09;
    let center = head + Vec3::new(0.95, 1.5 + bob, 0.55);
    // The cloud: a cream fill with a soft teal rim, then two trailing puffs toward the head.
    const CLOUD: &[&str] = &[
        "   ooooooo   ",
        " oocccccccoo ",
        "occcccccccco",
        "occcccccccco",
        "occcccccccco",
        "occcccccccco",
        "occcccccccco",
        " oocccccccoo ",
        "   ooooooo   ",
    ];
    let cream = [246, 238, 214];
    let rim = [123, 168, 166];
    pixel_pattern(shape, center, CLOUD, pixel, |byte| match byte {
        b'o' => Some(rim),
        b'c' => Some(cream),
        _ => None,
    });
    for (offset, size) in [
        (Vec3::new(-0.3, -0.33, 0.0), 2.0),
        (Vec3::new(-0.45, -0.47, 0.0), 1.2),
    ] {
        shape.cuboid(
            center + offset,
            Vec3::new(pixel * size, pixel * size, pixel * 0.6),
            cream,
        );
    }
    let glyph_center = center + Vec3::new(0.0, 0.0, 0.03);
    let (meaning, curious) = match want {
        Want::NameOf(meaning) => (Some(meaning), true),
        Want::Food(Some(food)) => (Some(Meaning::Food(food)), false),
        Want::Toy(toy) => (Some(Meaning::Toy(toy)), false),
        _ => (None, false),
    };
    let icon_center = if curious {
        glyph_center + Vec3::new(-0.12, 0.0, 0.0)
    } else {
        glyph_center
    };
    let draw = |shape: &mut EffectShape, rows: &[&str], palette: &[(u8, [u8; 3])]| {
        pixel_pattern(shape, icon_center, rows, pixel * 0.85, |byte| {
            palette
                .iter()
                .find(|(key, _)| *key == byte)
                .map(|(_, rgb)| *rgb)
        });
    };
    match (meaning, want) {
        (Some(Meaning::Toy(ToyId::Ball)), _) => draw(
            shape,
            &[" rrt ", "rrttt", "ccccc", "ttrrr", " ttr "],
            &[
                (b'r', [222, 112, 96]),
                (b't', [104, 178, 164]),
                (b'c', [238, 214, 160]),
            ],
        ),
        (Some(Meaning::Toy(ToyId::Bell)), _) => draw(
            shape,
            &["  g  ", " ggg ", " ggg ", "ggggg", "  d  "],
            &[(b'g', [214, 168, 70]), (b'd', [150, 110, 40])],
        ),
        (Some(Meaning::Toy(ToyId::Sock)), _) => draw(
            shape,
            &[" ww  ", " ll  ", " ll  ", " lll ", "  ll "],
            &[(b'w', [236, 228, 220]), (b'l', [158, 140, 190])],
        ),
        (Some(Meaning::Food(FoodId::Berry)), _) => draw(
            shape,
            &["  g  ", " rrr ", "rrrrr", "rrrrr", " rrr "],
            &[(b'g', [96, 160, 90]), (b'r', [204, 64, 84])],
        ),
        (Some(Meaning::Food(FoodId::Mushroom)), _) => draw(
            shape,
            &[" ttt ", "ttttt", "  s  ", "  s  ", " sss "],
            &[(b't', [200, 132, 100]), (b's', [236, 222, 196])],
        ),
        (Some(Meaning::Food(FoodId::Pellet)), _) => draw(
            shape,
            &["     ", " bbb ", " bbb ", " bbb ", "     "],
            &[(b'b', [150, 112, 70])],
        ),
        (_, Want::Company) => draw(
            shape,
            &["hh hh", "hhhhh", "hhhhh", " hhh ", "  h  "],
            &[(b'h', [233, 130, 117])],
        ),
        (_, Want::Sleep) => draw(
            shape,
            &["zzzz ", "  z  ", " z   ", "zzzz ", "     "],
            &[(b'z', [110, 132, 170])],
        ),
        // Hungry without a favorite yet: crumbs.
        _ => draw(
            shape,
            &["     ", " b b ", "  b  ", " b b ", "     "],
            &[(b'b', [180, 130, 80])],
        ),
    }
    if curious {
        pixel_pattern(
            shape,
            glyph_center + Vec3::new(0.25, 0.0, 0.0),
            &["xx ", "  x", " x ", "   ", " x "],
            pixel * 0.85,
            |byte| (byte == b'x').then_some([70, 96, 120]),
        );
    }
}

#[allow(clippy::too_many_arguments)] // Independent Bevy resources and read-only object poses.
fn sync_effects(
    mut commands: Commands,
    frame: Res<SceneFrame>,
    palette: Res<Palette>,
    motion: Res<crate::creature::CreatureMotion>,
    mut mesh: ResMut<EffectMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
    objects: Query<(&WorldObject, &Mesh3d, &Transform)>,
    mut measurements: ResMut<PoseMeasurements>,
    timing: Option<Res<crate::ray_stats::ComputeGpuTiming>>,
) {
    let _span = timing
        .as_ref()
        .map(|timing| timing.cpu_span("sync_effects"));
    let mut shape = EffectShape::default();
    let time_s = frame
        .plan
        .elapsed_ms
        .saturating_add(frame.plan.simulation_remainder_ms) as f32
        / 1000.0;
    if let Some(target) = frame.plan.creature.intent_target {
        intent_ring(
            &mut shape,
            world_position(target),
            time_s,
            frame.plan.reduced_motion,
        );
    }
    if let Some(refused) = frame.plan.creature.refusing {
        refusal_bubble(&mut shape, motion.position(&frame.plan), refused, time_s);
    } else if frame.plan.creature.playing && !frame.plan.reduced_flashes {
        play_sparkles(&mut shape, motion.position(&frame.plan), time_s);
    }
    if let Some(want) = frame
        .plan
        .creature
        .want
        .filter(|_| frame.plan.creature.refusing.is_none())
    {
        let time = frame
            .plan
            .elapsed_ms
            .saturating_add(frame.plan.simulation_remainder_ms) as f32
            / 1000.0;
        want_bubble(
            &mut shape,
            motion.position(&frame.plan),
            want,
            time,
            frame.plan.reduced_motion,
        );
    }
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
                    |object| {
                        objects
                            .iter()
                            .find(|(id, _, _)| id.0 == object.id)
                            .map_or_else(
                                || presented_object_position(object, &frame.plan, &motion),
                                |(_, _, transform)| transform.translation,
                            )
                    },
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
            PresentationCueKind::Ripple => {
                // Two pale rings spreading from the tap point on the glass.
                let spread = if frame.plan.reduced_motion {
                    0.5
                } else {
                    elapsed / 0.9
                };
                for ring in 0..2 {
                    let progress = (spread - ring as f32 * 0.25).clamp(0.0, 1.0);
                    if progress <= 0.0 || progress >= 1.0 {
                        continue;
                    }
                    let radius = 0.12 + progress * 0.55;
                    for index in 0..12 {
                        let angle = index as f32 * std::f32::consts::TAU / 12.0;
                        shape.cuboid(
                            position + Vec3::new(angle.cos() * radius, angle.sin() * radius, 1.0),
                            Vec3::splat(0.035 * (1.0 - progress) + 0.012),
                            [196, 226, 222],
                        );
                    }
                }
            }
            PresentationCueKind::Curious => {
                // A pixel question mark that bobs up beside the head.
                let rise = if frame.plan.reduced_motion {
                    0.0
                } else {
                    travel.min(0.4) * 0.25
                };
                let center = position + Vec3::new(0.55, 0.95 + rise, 0.7);
                for (row, line) in [" xxx ", "x   x", "   x ", "  x  ", "     ", "  x  "]
                    .iter()
                    .enumerate()
                {
                    for (column, byte) in line.bytes().enumerate() {
                        if byte == b'x' {
                            shape.cuboid(
                                center
                                    + Vec3::new(
                                        (column as f32 - 2.0) * 0.05,
                                        -(row as f32) * 0.05,
                                        0.0,
                                    ),
                                Vec3::splat(0.048),
                                [240, 226, 168],
                            );
                        }
                    }
                }
            }
            PresentationCueKind::WordLearned => {
                // A celebration that reads from across the room: two rings of sparks burst
                // outward, then a bright star pops above the head.
                let burst = if frame.plan.reduced_motion {
                    0.55
                } else {
                    (elapsed * 1.4).min(1.0)
                };
                let sparks = if frame.plan.reduced_flashes { 6 } else { 14 };
                for ring in 0..2 {
                    let reach = 0.7 + burst * (1.1 + ring as f32 * 0.5);
                    let fade = (1.2 - burst - ring as f32 * 0.15).max(0.0);
                    if fade <= 0.0 {
                        continue;
                    }
                    for index in 0..sparks {
                        let angle = index as f32 * std::f32::consts::TAU / sparks as f32
                            + ring as f32 * 0.22;
                        let size = 0.085 * fade;
                        shape.cuboid(
                            position
                                + Vec3::new(
                                    angle.cos() * reach,
                                    0.2 + angle.sin() * reach * 0.75,
                                    0.75,
                                ),
                            Vec3::splat(size.max(0.03)),
                            if (index + ring) % 2 == 0 {
                                [255, 222, 120]
                            } else {
                                [255, 250, 226]
                            },
                        );
                    }
                }
                let pop = if frame.plan.reduced_motion {
                    1.0
                } else {
                    (elapsed * 5.0).min(1.0) * (1.0 + (elapsed * 8.0).sin().abs() * 0.12)
                };
                pixel_pattern(
                    &mut shape,
                    position + Vec3::new(0.0, 1.2 + travel * 0.1, 0.75),
                    &[
                        "   y   ", "  yyy  ", "yyyyyyy", " yyyyy ", "  yyy  ", " yy yy ", " y   y ",
                    ],
                    0.07 * pop,
                    |byte| (byte == b'y').then_some([255, 214, 92]),
                );
            }
            PresentationCueKind::OpenWaterDrift if effect.target == UiTarget::OpenWater => {
                // A big glossy bubble, wobbling where Mop is headed.
                let wobble = if frame.plan.reduced_motion {
                    0.0
                } else {
                    (elapsed * 3.0).sin() * 0.04
                };
                let center = position + Vec3::new(wobble, 0.0, 0.5);
                for index in 0..14 {
                    let angle = index as f32 * std::f32::consts::TAU / 14.0;
                    shape.cuboid(
                        center + Vec3::new(angle.cos() * 0.17, angle.sin() * 0.17, 0.0),
                        Vec3::splat(0.04),
                        [190, 228, 232],
                    );
                }
                shape.cuboid(
                    center + Vec3::new(-0.06, 0.07, 0.02),
                    Vec3::splat(0.045),
                    [246, 252, 252],
                );
            }
            PresentationCueKind::Sleep
            | PresentationCueKind::CaveShelter
            | PresentationCueKind::OpenWaterDrift
            | PresentationCueKind::Comfort => {}
            PresentationCueKind::Notice | PresentationCueKind::PositiveNotice => {
                // A plain "!" above the head: it noticed something.
                let pop = if frame.plan.reduced_motion {
                    1.0
                } else {
                    (elapsed * 8.0).min(1.0)
                };
                pixel_pattern(
                    &mut shape,
                    position + Vec3::new(0.45, 0.95 + travel.min(0.3) * 0.2, 0.6),
                    &["x", "x", "x", " ", "x"],
                    0.055 * pop,
                    |byte| {
                        (byte == b'x').then_some(
                            if effect.cue == PresentationCueKind::PositiveNotice {
                                [255, 222, 120]
                            } else {
                                [236, 236, 214]
                            },
                        )
                    },
                );
            }
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
    let camera_rotation = Quat::from_rotation_x(CAMERA_PITCH);
    let mut selected = Vec::new();
    if frame.plan.creature.highlight != beastie_view::Highlight::None {
        selected.push((
            motion.position(&frame.plan) + camera_rotation * Vec3::Z,
            Vec2::new(0.78, 0.56),
        ));
    }
    for object in &frame.plan.objects {
        if object.highlight == beastie_view::Highlight::None {
            continue;
        }
        let measured = objects
            .iter()
            .find(|(id, _, _)| id.0 == object.id)
            .and_then(|(_, handle, transform)| {
                meshes
                    .get(&handle.0)
                    .and_then(|mesh| measurements.selection(handle.id(), mesh, transform))
            });
        selected.push(measured.unwrap_or((
            presented_object_position(object, &frame.plan, &motion) + camera_rotation * Vec3::Z,
            Vec2::splat(0.56),
        )));
    }
    for (center, half) in selected {
        for side in [-1.0, 1.0] {
            for vertical in [-1.0, 1.0] {
                let p = center + camera_rotation * Vec3::new(side * half.x, vertical * half.y, 0.0);
                shape.cuboid(p, Vec3::new(0.13, 0.025, 0.04), [233, 235, 197]);
                shape.cuboid(
                    p + camera_rotation * Vec3::new(side * 0.05, -vertical * 0.05, 0.0),
                    Vec3::new(0.025, 0.13, 0.04),
                    [233, 235, 197],
                );
            }
        }
    }
    // Keep the last nonempty shape while hidden. Reappearing unchanged brackets
    // can reuse their mesh; moving effects still compare their exact f32 inputs.
    if shape.0.is_empty() {
        if mesh.visible {
            if let Some(entity) = mesh.entity {
                commands.entity(entity).insert(Visibility::Hidden);
            }
            mesh.visible = false;
        }
        return;
    }
    if let Some(handle) = &mesh.handle {
        if shape != mesh.shape
            && let Some(mut current) = meshes.get_mut(handle)
        {
            *current = shape.mesh();
        }
        if !mesh.visible
            && let Some(entity) = mesh.entity
        {
            commands.entity(entity).insert(Visibility::Visible);
        }
    } else {
        let handle = meshes.add(shape.mesh());
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
    mesh.shape = shape;
    mesh.visible = true;
}

/// Measure the rendered silhouette in camera-facing axes. Keeping depth separate
/// avoids the old +world-Z offset shifting brackets downward under camera pitch.
fn selection_bounds(mesh: &Mesh, transform: &Transform) -> Option<(Vec3, Vec2)> {
    let camera_rotation = Quat::from_rotation_x(CAMERA_PITCH);
    let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
    if positions.is_empty() {
        return None;
    }
    let matrix = transform.to_matrix();
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for point in positions {
        let point = camera_rotation.inverse() * matrix.transform_point3(Vec3::from_array(*point));
        low = low.min(point);
        high = high.max(point);
    }
    let mut center = (low + high) * 0.5;
    center.z = high.z + 0.12;
    Some((
        camera_rotation * center,
        (high - low).truncate() * 0.5 + Vec2::splat(0.10),
    ))
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
        for indices in indices.as_chunks::<3>().0.iter() {
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
        (hit.enabled || hit.id == "compose/microphone" || hit.shape == HitShape::Blocker)
            && matches!(hit.shape, HitShape::Rect | HitShape::Blocker)
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

    fn cache_test_app() -> App {
        use bevy::asset::AssetApp;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_resource::<EffectMesh>()
            .init_resource::<UiCache>()
            .init_resource::<PoseMeasurements>()
            .init_resource::<crate::creature::CreatureMotion>()
            .init_resource::<crate::glyphs::Lettering>()
            .insert_resource(Palette {
                solid: default(),
                ui: default(),
                panel: default(),
                plant: default(),
                rubber: default(),
                cloth: default(),
                brass: default(),
                food: default(),
            });
        let world = beastie_core::WorldState::new(3, "Mop");
        let mut plan = beastie_view::plan(&world, &default()).0;
        plan.effects.clear();
        plan.objects.clear();
        app.insert_resource(SceneFrame { plan });
        app
    }

    fn changed_meshes(app: &mut App) -> Vec<AssetId<Mesh>> {
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .drain()
            .filter_map(|event| match event {
                AssetEvent::Modified { id } => Some(id),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn icon_cache_reuses_geometry_across_rectangles_moves_and_visibility() {
        let mut app = cache_test_app();
        app.add_plugins((TransformPlugin, bevy::camera::visibility::VisibilityPlugin))
            .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
            .add_systems(Update, sync_ui);
        let icon = beastie_view::IconCommand {
            id: "toy/ball".into(),
            kind: IconKind::Toy(ToyId::Ball),
            bounds: beastie_view::Rect {
                x: 80,
                y: 150,
                w: 10,
                h: 10,
            },
            layer: 1,
        };
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.text.clear();
            frame.plan.rects.clear();
            frame.plan.hit_regions.clear();
            frame.plan.icons = vec![icon.clone()];
        }
        app.update();
        let slot = app.world().resource::<UiCache>().icon_slots[0];
        let handle = app.world().get::<Mesh3d>(slot).unwrap().0.clone();
        let entity = app.world().resource::<UiCache>().icon_entity.unwrap();
        assert_eq!(app.world().resource::<UiCache>().icon_templates.len(), 1);
        assert!(app.world().get::<InheritedVisibility>(slot).unwrap().get());
        changed_meshes(&mut app);
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .rects
            .push(beastie_view::RectCommand {
                id: "panel".into(),
                rect: beastie_view::Rect {
                    x: 0,
                    y: 0,
                    w: 20,
                    h: 20,
                },
                color: [20, 30, 40, 255],
                layer: 0,
                outline: false,
                corner_radius: 0,
            });
        app.update();
        assert!(
            !changed_meshes(&mut app).contains(&handle.id()),
            "adding a rectangle leaves icon BLAS intact"
        );
        app.world_mut().resource_mut::<SceneFrame>().plan.rects[0].color[0] += 1;
        app.update();
        assert!(
            !changed_meshes(&mut app).contains(&handle.id()),
            "rectangle recoloring leaves icon BLAS intact"
        );
        app.world_mut().resource_mut::<SceneFrame>().plan.icons[0]
            .bounds
            .x += 1;
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
        assert_eq!(
            app.world().resource::<UiCache>().icon_templates.len(),
            1,
            "moving reuses canonical voxel geometry"
        );
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.icons[0].bounds.w = 18;
            frame.plan.icons[0].bounds.h = 12;
        }
        app.update();
        assert!(
            changed_meshes(&mut app).is_empty(),
            "resizing does not upload geometry"
        );
        assert_eq!(app.world().get::<Mesh3d>(slot).unwrap().0, handle);
        assert_eq!(
            app.world().get::<Transform>(slot).unwrap().scale,
            Vec3::new(0.6, 0.6, 1.0)
        );
        let moved = app.world().resource::<SceneFrame>().plan.icons.clone();
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .icons
            .clear();
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        assert!(!app.world().get::<InheritedVisibility>(slot).unwrap().get());
        app.world_mut().resource_mut::<SceneFrame>().plan.icons = moved;
        app.update();
        assert!(
            changed_meshes(&mut app).is_empty(),
            "same returning icon list reuses its mesh"
        );
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Visible)
        );
        assert!(app.world().get::<InheritedVisibility>(slot).unwrap().get());
        app.world_mut().resource_mut::<SceneFrame>().plan.icons[0].kind = IconKind::Send;
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
        assert_eq!(app.world().resource::<UiCache>().icon_templates.len(), 2);
        app.world_mut().resource_mut::<SceneFrame>().plan.icons[0].kind = icon.kind;
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
        assert_eq!(
            app.world().resource::<UiCache>().icon_templates.len(),
            2,
            "returning to a toy never regenerates its voxel mesh"
        );
    }

    #[test]
    fn icon_instances_follow_title_parent_and_keep_unused_slots_hidden() {
        let mut app = cache_test_app();
        app.add_plugins((TransformPlugin, bevy::camera::visibility::VisibilityPlugin))
            .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
            .add_systems(Update, (sync_ui, sync_title).chain());
        let first = beastie_view::IconCommand {
            id: "same-id".into(),
            kind: IconKind::Toy(ToyId::Ball),
            bounds: beastie_view::Rect {
                x: 80,
                y: 150,
                w: 10,
                h: 10,
            },
            layer: 1,
        };
        let second = beastie_view::IconCommand {
            bounds: beastie_view::Rect {
                x: 120,
                y: 130,
                w: 10,
                h: 10,
            },
            ..first.clone()
        };
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.text.clear();
            frame.plan.rects.clear();
            frame.plan.hit_regions.clear();
            frame.plan.icons = vec![first, second.clone()];
        }
        app.update();
        let parent = app.world().resource::<UiCache>().icon_entity.unwrap();
        let slots = app.world().resource::<UiCache>().icon_slots.clone();
        assert_eq!(slots.len(), 2);
        let first_handle = app.world().get::<Mesh3d>(slots[0]).unwrap().0.clone();
        assert_eq!(first_handle, app.world().get::<Mesh3d>(slots[1]).unwrap().0);
        assert_eq!(app.world().resource::<UiCache>().icon_templates.len(), 1);
        changed_meshes(&mut app);
        for title in [false, true] {
            app.world_mut()
                .resource_mut::<SceneFrame>()
                .plan
                .title_screen = title;
            app.update();
            assert!(
                changed_meshes(&mut app).is_empty(),
                "title changes only presentation transforms"
            );
            let parent_matrix = app
                .world()
                .get::<GlobalTransform>(parent)
                .unwrap()
                .to_matrix();
            for (index, icon) in app
                .world()
                .resource::<SceneFrame>()
                .plan
                .icons
                .iter()
                .enumerate()
            {
                let center = logical_position(
                    icon.bounds.x as f32 + icon.bounds.w as f32 * 0.5,
                    icon.bounds.y as f32 + icon.bounds.h as f32 * 0.5,
                    overlay_depth(icon.layer, OverlayPart::Icon),
                );
                let original = icon_mesh(icon.kind, Vec3::ZERO, [216, 219, 185])
                    .scaled_by(icon_transform(icon).scale)
                    .translated_by(center);
                let local = app
                    .world()
                    .resource::<Assets<Mesh>>()
                    .get(&first_handle)
                    .unwrap();
                let world = app
                    .world()
                    .get::<GlobalTransform>(slots[index])
                    .unwrap()
                    .to_matrix();
                let positions = |mesh: &Mesh| {
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                        .as_float3()
                        .unwrap()
                        .to_vec()
                };
                for (local, original) in positions(local).into_iter().zip(positions(&original)) {
                    let actual = world.transform_point3(Vec3::from(local));
                    let expected = parent_matrix.transform_point3(Vec3::from(original));
                    assert!(
                        actual.distance(expected) < 0.000003,
                        "title={title}: icon world position changed {actual:?} vs {expected:?}"
                    );
                }
            }
        }
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .icons
            .truncate(1);
        app.update();
        assert_eq!(app.world().resource::<UiCache>().icon_slots, slots);
        assert!(
            app.world()
                .get::<InheritedVisibility>(slots[0])
                .unwrap()
                .get()
        );
        assert!(
            !app.world()
                .get::<InheritedVisibility>(slots[1])
                .unwrap()
                .get()
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .icons
            .push(second);
        app.update();
        assert!(
            app.world()
                .get::<InheritedVisibility>(slots[1])
                .unwrap()
                .get()
        );
        assert_eq!(first_handle, app.world().get::<Mesh3d>(slots[1]).unwrap().0);
        assert!(
            changed_meshes(&mut app).is_empty(),
            "slot count changes never mutate canonical meshes"
        );
    }

    #[test]
    fn canonical_icon_cache_preserves_authored_attributes_and_positions() {
        let mut cache = UiCache::default();
        let mut assets = Assets::<Mesh>::default();
        let center = Vec3::new(3.371, -1.673, 8.052);
        for kind in test_icon_kinds() {
            let handle = cache.icon(&mut assets, kind, [216, 219, 185]);
            if matches!(kind, IconKind::Toy(_) | IconKind::FoodItem(_)) {
                assert_eq!(
                    handle,
                    cache.icon(&mut assets, kind, [88, 114, 118]),
                    "object palettes share their authored colored mesh"
                );
            }
            let cached = assets.get(&handle).unwrap().clone().translated_by(center);
            let original = icon_mesh(kind, center, [216, 219, 185]);
            let positions = |mesh: &Mesh| {
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap()
                    .to_vec()
            };
            let cached_positions = positions(&cached);
            let original_positions = positions(&original);
            assert_eq!(cached_positions.len(), original_positions.len());
            for (cached, original) in cached_positions.into_iter().zip(original_positions) {
                assert!(
                    Vec3::from(cached).distance(Vec3::from(original)) < 0.000001,
                    "{kind:?} retained authored placement"
                );
            }
            assert_eq!(
                cached.indices().unwrap().iter().collect::<Vec<_>>(),
                original.indices().unwrap().iter().collect::<Vec<_>>()
            );
            for attribute in [Mesh::ATTRIBUTE_NORMAL, Mesh::ATTRIBUTE_COLOR] {
                assert_eq!(
                    cached.attribute(attribute).unwrap(),
                    original.attribute(attribute).unwrap()
                );
            }
        }
        assert_eq!(cache.icon_templates.len(), test_icon_kinds().len());
    }

    fn label_test_app() -> App {
        let mut app = cache_test_app();
        app.add_plugins((TransformPlugin, bevy::camera::visibility::VisibilityPlugin))
            .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
            .add_systems(Update, (sync_ui, sync_title).chain());
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.rects.clear();
            frame.plan.icons.clear();
            frame.plan.hit_regions.clear();
            frame.plan.text.truncate(1);
            frame.plan.text[0].id = "label/a".into();
            frame.plan.text[0].text = "Alpha".into();
            frame.plan.text[0].bounds = Some(beastie_view::Rect {
                x: 10,
                y: 10,
                w: 60,
                h: 20,
            });
            frame.plan.text[0].layer = 10;
        }
        app
    }

    #[test]
    fn steady_input_decorations_only_rebuild_when_edit_state_changes() {
        let mut app = label_test_app();
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.text[0].role = beastie_view::TextRole::Body;
            frame.plan.text[0].keep_tail = true;
            frame.plan.text[0].input_state = Some(beastie_view::TextInputState::Caret);
        }
        app.update();
        let handle = app.world().resource::<UiCache>().text_slots[0]
            .mesh
            .clone()
            .unwrap();
        changed_meshes(&mut app);
        for _ in 0..3 {
            app.world_mut().resource_mut::<SceneFrame>().plan.elapsed_ms += 1_000;
            app.update();
            assert!(
                changed_meshes(&mut app).is_empty(),
                "steady caret must not churn meshes"
            );
            assert_eq!(
                app.world().resource::<UiCache>().text_slots[0]
                    .mesh
                    .as_ref()
                    .unwrap()
                    .id(),
                handle.id()
            );
        }
        app.world_mut().resource_mut::<SceneFrame>().plan.text[0].input_state =
            Some(beastie_view::TextInputState::Selected);
        app.update();
        assert_eq!(changed_meshes(&mut app), vec![handle.id()]);
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
    }

    #[test]
    fn retained_labels_change_independently_and_reuse_covered_or_absent_geometry() {
        let mut app = label_test_app();
        let mut second = app.world().resource::<SceneFrame>().plan.text[0].clone();
        second.id = "label/b".into();
        second.text = "Beta".into();
        second.bounds = Some(beastie_view::Rect {
            x: 160,
            y: 10,
            w: 60,
            h: 20,
        });
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .text
            .push(second.clone());
        app.update();
        let handles: Vec<_> = app
            .world()
            .resource::<UiCache>()
            .text_slots
            .iter()
            .map(|slot| slot.mesh.clone().unwrap())
            .collect();
        let second_entity = app.world().resource::<UiCache>().text_slots[1].entity;
        changed_meshes(&mut app);
        app.world_mut().resource_mut::<SceneFrame>().plan.text[0]
            .text
            .push('!');
        app.update();
        assert_eq!(
            changed_meshes(&mut app),
            vec![handles[0].id()],
            "only the edited label invalidates its BLAS"
        );
        app.world_mut().resource_mut::<SceneFrame>().plan.text.pop();
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
        assert!(
            !app.world()
                .get::<InheritedVisibility>(second_entity)
                .unwrap()
                .get()
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .text
            .push(second.clone());
        app.update();
        assert!(
            changed_meshes(&mut app).is_empty(),
            "returning labels retain their original mesh"
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(second_entity)
                .unwrap()
                .get()
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .rects
            .push(beastie_view::RectCommand {
                id: "cover".into(),
                rect: second.bounds.unwrap(),
                color: [1, 2, 3, 255],
                layer: 11,
                outline: false,
                corner_radius: 0,
            });
        app.update();
        assert!(
            !changed_meshes(&mut app).contains(&handles[1].id()),
            "fully covered text is hidden without rebuilding"
        );
        assert!(
            !app.world()
                .get::<InheritedVisibility>(second_entity)
                .unwrap()
                .get()
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .rects
            .clear();
        app.update();
        let modified = changed_meshes(&mut app);
        assert!(!modified.contains(&handles[0].id()) && !modified.contains(&handles[1].id()));
        assert!(
            app.world()
                .get::<InheritedVisibility>(second_entity)
                .unwrap()
                .get()
        );
        assert_eq!(
            app.world().get::<Mesh3d>(second_entity).unwrap().id(),
            handles[1].id()
        );
    }

    #[test]
    fn retained_labels_support_duplicate_ids_and_title_parent_transforms() {
        let mut app = label_test_app();
        let mut second = app.world().resource::<SceneFrame>().plan.text[0].clone();
        second.text = "Beta".into();
        second.bounds = Some(beastie_view::Rect {
            x: 160,
            y: 10,
            w: 60,
            h: 20,
        });
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .text
            .push(second);
        app.update();
        assert_eq!(app.world().resource::<UiCache>().text_slots.len(), 2);
        let slots: Vec<_> = app
            .world()
            .resource::<UiCache>()
            .text_slots
            .iter()
            .map(|slot| (slot.entity, slot.mesh.clone().unwrap()))
            .collect();
        assert_ne!(slots[0].0, slots[1].0);
        assert_ne!(slots[0].1, slots[1].1);
        let parent = app.world().resource::<UiCache>().text_parent.unwrap();
        changed_meshes(&mut app);
        for title in [false, true] {
            app.world_mut()
                .resource_mut::<SceneFrame>()
                .plan
                .title_screen = title;
            app.update();
            assert!(changed_meshes(&mut app).is_empty());
            let parent_matrix = app
                .world()
                .get::<GlobalTransform>(parent)
                .unwrap()
                .to_matrix();
            let frame = app.world().resource::<SceneFrame>();
            let mut lettering = crate::glyphs::Lettering::default();
            for ((entity, handle), text) in slots.iter().zip(&frame.plan.text) {
                let clips =
                    visible_text_boxes(text, text_content_bounds(text, &frame.plan), &frame.plan);
                let reference = label_mesh(&mut lettering, text, &clips, &frame.plan);
                let actual = app.world().resource::<Assets<Mesh>>().get(handle).unwrap();
                let world = app
                    .world()
                    .get::<GlobalTransform>(*entity)
                    .unwrap()
                    .to_matrix();
                let positions = |mesh: &Mesh| {
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                        .as_float3()
                        .unwrap()
                        .to_vec()
                };
                assert_eq!(positions(actual), positions(&reference));
                for point in positions(actual) {
                    assert!(
                        world
                            .transform_point3(Vec3::from(point))
                            .distance(parent_matrix.transform_point3(Vec3::from(point)))
                            < 0.000001
                    );
                }
                assert!(
                    app.world()
                        .get::<InheritedVisibility>(*entity)
                        .unwrap()
                        .get()
                );
            }
        }
    }

    #[test]
    fn retained_label_cache_evicts_old_inactive_ids_without_losing_current_text() {
        let mut app = label_test_app();
        app.update();
        let first = app.world().resource::<UiCache>().text_slots[0].entity;
        for index in 0..MAX_IDLE_TEXT_SLOTS + 4 {
            app.world_mut().resource_mut::<SceneFrame>().plan.text[0].id = format!("label/{index}");
            app.update();
            let cache = app.world().resource::<UiCache>();
            assert!(cache.text_slots.len() <= MAX_IDLE_TEXT_SLOTS + 1);
            assert_eq!(
                cache.text_slots.iter().filter(|slot| slot.active).count(),
                1
            );
        }
        assert!(app.world().get::<Visibility>(first).is_none());
        let active = app
            .world()
            .resource::<UiCache>()
            .text_slots
            .iter()
            .find(|slot| slot.active)
            .unwrap();
        assert!(
            app.world()
                .get::<InheritedVisibility>(active.entity)
                .unwrap()
                .get()
        );
    }

    #[test]
    fn physical_object_effect_follows_the_actual_transfer_pose() {
        let mut app = cache_test_app();
        app.add_systems(Update, sync_effects);
        let mut sock = beastie_view::plan(&beastie_core::WorldState::new(9, "Pickup"), &default())
            .0
            .objects
            .into_iter()
            .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
            .unwrap();
        sock.carried = true;
        let id = sock.id;
        let effect = beastie_view::EffectScene {
            owner: beastie_view::SemanticOwner::ToyInteraction(
                std::num::NonZeroU64::new(3).unwrap(),
            ),
            cue: PresentationCueKind::SockTug,
            position: sock.position,
            target: UiTarget::Toy(ToyId::Sock),
            elapsed_ms: 0,
        };
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.objects = vec![sock];
            frame.plan.effects = vec![effect];
        }
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(object_mesh(ObjectKind::Toy(ToyId::Sock), default()));
        let first = Vec3::new(-2.0, 1.0, 0.2);
        let entity = app
            .world_mut()
            .spawn((
                WorldObject(id),
                Mesh3d(mesh),
                Transform::from_translation(first),
            ))
            .id();
        app.update();
        let before = app.world().resource::<EffectMesh>().shape.0.clone();
        assert_eq!(before.len(), 3);
        assert!(before[1].0.distance(first + Vec3::new(0.0, 0.88, 0.6)) < 0.0001);
        let transfer_step = Vec3::new(-0.14, 0.04, 0.05);
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .translation += transfer_step;
        app.update();
        let after = &app.world().resource::<EffectMesh>().shape.0;
        for (before, after) in before.iter().zip(after) {
            assert!((after.0 - before.0).distance(transfer_step) < 0.0001);
            assert_eq!(after.1, before.1);
        }
    }

    #[test]
    fn exact_effect_cache_skips_mesh_mutations_but_preserves_motion_and_visibility() {
        let mut app = cache_test_app();
        app.add_systems(Update, sync_effects);
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .creature
            .highlight = beastie_view::Highlight::Hover;
        app.update();
        let handle = app.world().resource::<EffectMesh>().handle.clone().unwrap();
        changed_meshes(&mut app);
        app.update();
        assert!(
            changed_meshes(&mut app).is_empty(),
            "unchanged selection keeps its BLAS"
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .creature
            .position
            .x += 1;
        app.update();
        assert_eq!(
            changed_meshes(&mut app),
            vec![handle.id()],
            "even one position unit updates geometry"
        );
        let entity = app.world().resource::<EffectMesh>().entity.unwrap();
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .creature
            .highlight = beastie_view::Highlight::None;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        assert!(changed_meshes(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .creature
            .highlight = beastie_view::Highlight::Hover;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Visible)
        );
        assert!(
            changed_meshes(&mut app).is_empty(),
            "reappearing unchanged selection reuses the allocation"
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .effects
            .push(beastie_view::EffectScene {
                owner: beastie_view::SemanticOwner::Ordinary,
                cue: PresentationCueKind::Wake,
                position: NormalizedPosition::new(5000, 5000),
                target: UiTarget::Creature,
                elapsed_ms: 0,
            });
        app.update();
        changed_meshes(&mut app);
        app.world_mut().resource_mut::<SceneFrame>().plan.elapsed_ms += 1;
        app.update();
        assert_eq!(
            changed_meshes(&mut app),
            vec![handle.id()],
            "continuous wake animation is never frozen"
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .reduced_motion = true;
        app.update();
        changed_meshes(&mut app);
        app.world_mut().resource_mut::<SceneFrame>().plan.elapsed_ms += 1;
        app.update();
        assert!(
            changed_meshes(&mut app).is_empty(),
            "suppressed wake changes no geometry"
        );
    }

    #[test]
    fn text_cache_ignores_icon_and_color_changes_but_tracks_clipping_and_labels() {
        let mut app = cache_test_app();
        app.add_systems(Update, sync_ui);
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.text.truncate(1);
            assert_eq!(frame.plan.text.len(), 1);
            frame.plan.text[0].bounds = Some(beastie_view::Rect {
                x: 10,
                y: 10,
                w: 100,
                h: 20,
            });
            frame.plan.text[0].layer = 10;
            frame.plan.icons = vec![beastie_view::IconCommand {
                id: "ui/button-send".into(),
                kind: IconKind::Send,
                bounds: beastie_view::Rect {
                    x: 200,
                    y: 120,
                    w: 10,
                    h: 10,
                },
                layer: 1,
            }];
            frame.plan.rects = vec![beastie_view::RectCommand {
                id: "background".into(),
                rect: beastie_view::Rect {
                    x: 0,
                    y: 0,
                    w: 320,
                    h: 180,
                },
                color: [20, 30, 40, 255],
                layer: 0,
                outline: false,
                corner_radius: 0,
            }];
            frame.plan.rects.push(beastie_view::RectCommand {
                id: "settings/panel".into(),
                rect: beastie_view::Rect {
                    x: 150,
                    y: 20,
                    w: 100,
                    h: 100,
                },
                color: [20, 30, 40, 255],
                layer: 2,
                outline: false,
                corner_radius: 0,
            });
            frame.plan.hit_regions.truncate(1);
            assert_eq!(frame.plan.hit_regions.len(), 1);
            frame.plan.hit_regions[0].id = "compose/send".into();
            frame.plan.hit_regions[0].enabled = true;
        }
        app.update();
        let text = app.world().resource::<UiCache>().text_slots[0]
            .mesh
            .clone()
            .unwrap();
        let geometry = app.world().resource::<UiCache>().geometry.clone().unwrap();
        let icon_slot = app.world().resource::<UiCache>().icon_slots[0];
        let icons = app.world().get::<Mesh3d>(icon_slot).unwrap().0.clone();
        changed_meshes(&mut app);
        app.update();
        assert!(changed_meshes(&mut app).is_empty());
        app.world_mut().resource_mut::<SceneFrame>().plan.rects[0].color[0] += 1;
        app.update();
        assert_eq!(
            changed_meshes(&mut app),
            vec![geometry.id()],
            "background color does not reshape lettering"
        );
        app.world_mut()
            .resource_mut::<SceneFrame>()
            .plan
            .hit_regions[0]
            .enabled = false;
        app.update();
        assert!(
            changed_meshes(&mut app).is_empty(),
            "enabled state selects a canonical asset without mutating meshes"
        );
        assert_ne!(
            app.world().get::<Mesh3d>(icon_slot).unwrap().id(),
            icons.id()
        );
        {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.rects[0].layer = 11;
            frame.plan.rects[0].rect = beastie_view::Rect {
                x: 10,
                y: 10,
                w: 50,
                h: 20,
            };
        }
        app.update();
        let modified = changed_meshes(&mut app);
        assert!(
            modified.contains(&text.id()),
            "a covering rectangle changes clipped glyphs"
        );
        app.world_mut().resource_mut::<SceneFrame>().plan.text[0]
            .text
            .push('!');
        app.update();
        assert_eq!(
            changed_meshes(&mut app),
            vec![text.id()],
            "label edits update only text geometry"
        );
    }

    #[test]
    fn pose_measurements_preserve_translation_rotation_scale_and_asset_changes() {
        let mut meshes = Assets::<Mesh>::default();
        let handle = meshes.add(object_mesh(
            ObjectKind::Toy(ToyId::Ball),
            crate::appearance::RenderAppearance::default(),
        ));
        let mut cache = PoseMeasurements::default();
        for transform in [
            Transform::IDENTITY,
            Transform::from_xyz(2.0, -0.3, 0.7),
            Transform::from_rotation(Quat::from_rotation_z(0.43)),
            Transform::from_scale(Vec3::new(0.7, 1.4, 0.9)),
        ] {
            let mesh = meshes.get(&handle).unwrap();
            let measured = cache.selection(handle.id(), mesh, &transform).unwrap();
            let direct = selection_bounds(mesh, &transform).unwrap();
            assert!((measured.0 - direct.0).length() < 0.00001);
            assert!((measured.1 - direct.1).length() < 0.00001);
            let ground = cache.grounding(
                handle.id(),
                ToyId::Ball,
                mesh,
                transform.rotation,
                transform.scale,
            );
            assert!(
                (ground
                    - grounded_title_toy(ToyId::Ball, mesh, transform.rotation, transform.scale))
                .length()
                    < 0.00001
            );
            assert_eq!(cache.selection.len(), 1);
            assert_eq!(cache.grounding.len(), 1);
        }
        *meshes.get_mut(&handle).unwrap() = object_mesh(
            ObjectKind::Toy(ToyId::Bell),
            crate::appearance::RenderAppearance::default(),
        );
        cache.asset_event(&AssetEvent::Modified { id: handle.id() });
        assert!(cache.selection.is_empty() && cache.grounding.is_empty());
        let mesh = meshes.get(&handle).unwrap();
        assert_eq!(
            cache.selection(handle.id(), mesh, &Transform::IDENTITY),
            selection_bounds(mesh, &Transform::IDENTITY)
        );
        assert_eq!(
            cache.grounding(handle.id(), ToyId::Bell, mesh, Quat::IDENTITY, Vec3::ONE),
            grounded_title_toy(ToyId::Bell, mesh, Quat::IDENTITY, Vec3::ONE)
        );
        cache.asset_event(&AssetEvent::Removed { id: handle.id() });
        assert!(cache.selection.is_empty() && cache.grounding.is_empty());
        cache.selection(handle.id(), mesh, &Transform::IDENTITY);
        cache.grounding(handle.id(), ToyId::Bell, mesh, Quat::IDENTITY, Vec3::ONE);
        cache.retain_live(&HashSet::new());
        assert!(cache.selection.is_empty() && cache.grounding.is_empty());
    }

    #[test]
    fn selection_brackets_enclose_the_projected_rotated_ball_without_depth_drift() {
        let mesh = object_mesh(
            ObjectKind::Toy(ToyId::Ball),
            crate::appearance::RenderAppearance::default(),
        );
        let view = Quat::from_rotation_x(CAMERA_PITCH).inverse();
        for angle in [-1.1, 0.0, 0.8] {
            let transform =
                Transform::from_xyz(-0.4, 0.3, 0.7).with_rotation(Quat::from_rotation_z(angle));
            let (center, half) = selection_bounds(&mesh, &transform).unwrap();
            let center = view * center;
            for point in mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap()
            {
                let projected = view * transform.transform_point(Vec3::from_array(*point));
                let delta = (projected - center).truncate().abs();
                assert!(delta.x <= half.x - 0.099 && delta.y <= half.y - 0.099);
                assert!(projected.z < center.z);
            }
        }
    }

    #[test]
    fn title_toys_contact_the_substrate_in_their_rendered_pose() {
        for toy in [ToyId::Ball, ToyId::Bell, ToyId::Sock] {
            let mesh = object_mesh(
                ObjectKind::Toy(toy),
                crate::appearance::RenderAppearance::default(),
            );
            for angle in [-0.7, 0.0, 1.2] {
                let rotation = Quat::from_rotation_z(angle);
                let translation = grounded_title_toy(toy, &mesh, rotation, Vec3::ONE);
                let clearance = mesh
                    .attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        let p = translation + rotation * Vec3::from_array(*p);
                        p.y - crate::environment::substrate_surface_height(p.x, p.z)
                    })
                    .fold(f32::INFINITY, f32::min);
                assert!(clearance.abs() < 0.00001, "{toy:?}: {clearance}");
                assert!(translation.x.abs() > 2.6, "toy overlaps title menu");
            }
        }
    }

    #[test]
    fn title_portrait_scales_all_part_offsets_about_the_presented_head() {
        let head = Vec3::new(2.0, 0.7, 0.1);
        let original = Transform::from_translation(head + Vec3::new(-0.7, 0.3, 0.0))
            .with_scale(Vec3::splat(1.2));
        let mut staged = original;
        stage_title_creature(&mut staged, head);
        assert!((staged.translation - Vec3::new(-3.924, 1.896, 0.1)).length() < 0.0001);
        assert!((staged.scale - Vec3::splat(0.984)).length() < 0.0001);
        assert_eq!(staged.rotation, original.rotation);
    }

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

    #[test]
    fn presentation_and_pointer_coordinates_agree_across_aspect_ratios() {
        for drawable in [
            Vec2::new(1920.0, 1080.0),
            Vec2::new(1600.0, 720.0),
            Vec2::new(900.0, 1200.0),
        ] {
            for title in [false, true] {
                let extent = presentation_extent(drawable, title);
                let rotation = Quat::from_rotation_x(CAMERA_PITCH);
                let (height, lift) = if title {
                    (7.6, 0.6)
                } else {
                    (PLAY_HEIGHT, PLAY_LIFT)
                };
                let camera = Mat4::from_rotation_translation(
                    rotation,
                    rotation * Vec3::new(0.0, lift, 24.0),
                );
                let overlay = Mat4::from_quat(rotation)
                    * Mat4::from_scale_rotation_translation(
                        Vec3::new(1.0, height / 9.0, 1.0),
                        Quat::IDENTITY,
                        Vec3::new(0.0, lift, 0.0),
                    );
                for logical in [
                    Vec2::new(148.0, 125.0),
                    Vec2::new(8.0, 156.0),
                    Vec2::new(280.0, 165.0),
                    Vec2::new(168.0, 45.0),
                ] {
                    let point = (camera.inverse() * overlay)
                        .transform_point3(logical_position(logical.x, logical.y, 8.0))
                        .truncate();
                    let pixel =
                        (point / extent * Vec2::new(1.0, -1.0) + Vec2::splat(0.5)) * drawable;
                    let actual = Viewport::for_drawable(drawable.x, drawable.y)
                        .logical_point(pixel.x, pixel.y)
                        .unwrap();
                    assert!((actual.0 - logical.x).abs() < 0.001);
                    assert!((actual.1 - logical.y).abs() < 0.001);
                }
            }
        }
    }
}

#[cfg(test)]
mod object_motion_tests {
    use super::*;

    #[test]
    fn observed_pickup_keeps_the_previous_pose_then_reaches_the_moving_mouth() {
        for reduced_motion in [false, true] {
            let mut scene = beastie_view::plan(
                &beastie_core::WorldState::new(9, "Pickup"),
                &beastie_view::ViewState::default(),
            )
            .0;
            scene.reduced_motion = reduced_motion;
            scene.elapsed_ms = 1_000;
            let mut sock = scene
                .objects
                .iter()
                .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
                .unwrap()
                .clone();
            let original =
                Transform::from_xyz(2.0, 1.0, 0.0).with_rotation(Quat::from_rotation_z(0.2));
            let mut presentation = ObjectPresentation::new(&sock, &scene, original);
            // The ECS transform is the visible source, even if another presentation
            // system adjusted it after the component's last sample.
            let rendered = original.with_translation(original.translation + Vec3::X * 0.05);
            sock.carried = true;
            scene.simulation_remainder_ms = 16;
            let held =
                Transform::from_xyz(1.0, 0.0, 0.65).with_rotation(Quat::from_rotation_z(-0.15));
            let first = presentation.advance(&sock, &scene, held, rendered);
            assert_eq!(
                first, rendered,
                "pickup must not teleport or rotate on its first frame"
            );
            let mut previous = first;
            for elapsed in (20..=OBJECT_TRANSFER_MS).step_by(20) {
                scene.simulation_remainder_ms = 16 + elapsed;
                let moving_held =
                    held.with_translation(held.translation + Vec3::X * elapsed as f32 * 0.001);
                let next = presentation.advance(&sock, &scene, moving_held, previous);
                assert!(next.translation.distance(previous.translation) < 0.3);
                assert!(next.rotation.angle_between(previous.rotation) < 0.06);
                assert!((0.0..=0.65).contains(&next.translation.z));
                if elapsed == OBJECT_TRANSFER_MS {
                    assert_eq!(
                        next, moving_held,
                        "finish at the current mouth, not a stale target"
                    );
                    assert!(presentation.transfer.is_none());
                }
                previous = next;
            }
            assert!(
                sock.carried,
                "presentation never changes authoritative carry"
            );
        }
    }

    #[test]
    fn release_during_pickup_starts_at_the_visible_pose_and_converges_to_the_fall() {
        let mut scene =
            beastie_view::plan(&beastie_core::WorldState::new(9, "Release"), &default()).0;
        let mut sock = scene
            .objects
            .iter()
            .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
            .unwrap()
            .clone();
        let original = Transform::from_xyz(2.0, 1.0, 0.0);
        let held = Transform::from_xyz(1.0, 0.0, 0.65).with_rotation(Quat::from_rotation_z(-0.15));
        let mut presentation = ObjectPresentation::new(&sock, &scene, original);
        sock.carried = true;
        let first = presentation.advance(&sock, &scene, held, original);
        scene.simulation_remainder_ms = 80;
        let midway = presentation.advance(&sock, &scene, held, first);
        assert_ne!(midway, original);
        assert_ne!(midway, held);
        sock.carried = false;
        let falling = held.with_rotation(Quat::IDENTITY);
        assert_eq!(presentation.advance(&sock, &scene, falling, midway), midway);
        let mut previous = midway;
        for elapsed in (20..=OBJECT_TRANSFER_MS).step_by(20) {
            scene.simulation_remainder_ms = 80 + elapsed;
            let target =
                falling.with_translation(falling.translation - Vec3::Y * elapsed as f32 * 0.001);
            let next = presentation.advance(&sock, &scene, target, previous);
            assert!(next.translation.distance(previous.translation) < 0.3);
            assert!(next.rotation.angle_between(previous.rotation) < 0.06);
            if elapsed == OBJECT_TRANSFER_MS {
                assert_eq!(next, target);
                assert!(presentation.transfer.is_none());
            }
            previous = next;
        }
    }

    #[test]
    fn carry_presentation_resets_on_load_clock_discontinuities_and_title_changes() {
        let mut scene =
            beastie_view::plan(&beastie_core::WorldState::new(9, "Reset"), &default()).0;
        scene.elapsed_ms = 2_000;
        let mut sock = scene
            .objects
            .iter()
            .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
            .unwrap()
            .clone();
        let original = Transform::from_xyz(2.0, 1.0, 0.0);
        let held = Transform::from_xyz(1.0, 0.0, 0.65).with_rotation(Quat::from_rotation_z(-0.15));
        sock.carried = true;
        let mut loaded = ObjectPresentation::new(&sock, &scene, held);
        assert_eq!(loaded.advance(&sock, &scene, held, held), held);
        assert!(
            loaded.transfer.is_none(),
            "loaded carry never invents a pickup"
        );
        for (now, title, target) in [
            (1_999, false, held),
            (3_001, false, held),
            (2_100, false, held.with_translation(Vec3::splat(20.0))),
            (2_100, true, held),
        ] {
            sock.carried = false;
            let mut presentation = ObjectPresentation::new(&sock, &scene, original);
            sock.carried = true;
            let first = presentation.advance(&sock, &scene, held, original);
            let mut discontinuous = scene.clone();
            discontinuous.elapsed_ms = now;
            discontinuous.title_screen = title;
            assert_eq!(
                presentation.advance(&sock, &discontinuous, target, first),
                target
            );
            assert!(presentation.transfer.is_none());
        }
        let mut title = scene.clone();
        title.title_screen = true;
        let mut staged = ObjectPresentation::new(&sock, &title, original);
        assert_eq!(staged.advance(&sock, &scene, held, original), held);
        assert!(staged.transfer.is_none());
    }

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
    fn toy_surface_targets_keep_authored_meshes_out_of_the_head() {
        let mut world = beastie_core::WorldState::new(9, "Contact");
        let anchor = NormalizedPosition::new(5_000, 5_000);
        let radii = crate::creature::authored_head_half_extents();
        for toy in [ToyId::Ball, ToyId::Bell, ToyId::Sock] {
            world.aquarium.toy_states.get_mut(&toy).unwrap().position = anchor;
            let mesh = object_mesh(ObjectKind::Toy(toy), default());
            let vertices = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            for direction in 0..24 {
                let angle = direction as f32 * std::f32::consts::TAU / 24.0;
                world.creature.aquarium.position = NormalizedPosition::new(
                    5_000 + (angle.cos() * 4_500.0) as i32,
                    5_000 + (angle.sin() * 4_500.0) as i32,
                );
                let target = beastie_core::approach_position(
                    &world,
                    beastie_core::SemanticDestination::Toy(toy),
                )
                .unwrap();
                let head = world_position(target);
                if toy == ToyId::Ball && direction == 18 {
                    let anchor_y = world_position(anchor).y;
                    let gap = vertices
                        .iter()
                        .map(|vertex| (anchor_y + vertex[1] - head.y).abs() - radii.y)
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        (0.0..=0.06).contains(&gap),
                        "vertical ball contact must look like contact, gap {gap}"
                    );
                }
                for vertex in vertices {
                    let relative = (world_position(anchor) + Vec3::from(*vertex) - head) / radii;
                    assert!(
                        relative.length_squared() > 1.0,
                        "{toy:?} intersects the head from direction {direction}"
                    );
                }
            }
        }
    }

    #[test]
    fn released_sock_keeps_its_mouth_anchor_and_continuous_falling_depth() {
        let world = beastie_core::WorldState::new(9, "Drop");
        let mut scene = beastie_view::plan(&world, &beastie_view::ViewState::default()).0;
        let mut sock = scene
            .objects
            .iter()
            .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
            .unwrap()
            .clone();
        sock.carried = true;
        sock.position =
            NormalizedPosition::new(scene.creature.position.x, scene.creature.position.y + 750);
        let held = object_position(&sock, &scene);
        sock.carried = false;
        sock.response = beastie_core::ToyResponse::SockTugged;
        sock.velocity = beastie_core::NormalizedVelocity {
            x: 0,
            y: beastie_core::SOCK_RELEASE_SPEED,
        };
        let released = object_position(&sock, &scene);
        assert!(
            held.distance(released) < 0.002,
            "release must retain the rendered mouth anchor"
        );
        scene.simulation_remainder_ms = 999;
        let before_tick = object_position(&sock, &scene);
        sock.position.y += sock.velocity.y;
        sock.velocity.y /= 2;
        scene.simulation_remainder_ms = 0;
        let after_tick = object_position(&sock, &scene);
        assert!(before_tick.distance(after_tick) < 0.002);
        assert!(after_tick.y < released.y && after_tick.z < released.z);
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

    #[test]
    fn sock_release_at_the_floor_uses_the_same_clamped_hold_anchor() {
        for head_y in [9_400, 10_000] {
            let mut world = beastie_core::WorldState::new(9, "Floor");
            world.creature.aquarium.position = NormalizedPosition::new(5_000, head_y);
            let scene = beastie_view::plan(&world, &beastie_view::ViewState::default()).0;
            let mut sock = scene
                .objects
                .iter()
                .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
                .unwrap()
                .clone();
            sock.carried = true;
            sock.position = beastie_core::held_toy_position(world.creature.aquarium.position);
            let carried = object_position(&sock, &scene);
            sock.carried = false;
            sock.response = beastie_core::ToyResponse::SockTugged;
            sock.velocity.y = beastie_core::SOCK_RELEASE_SPEED;
            let released = object_position(&sock, &scene);
            assert!(carried.distance(released) < 0.0001, "head_y {head_y}");
            assert_eq!(sock.position.y, 10_000);
        }
    }
}

#[cfg(test)]
mod ui_layout_tests {
    use super::*;

    fn flat_triangles(mesh: &Mesh) -> Vec<[Vec3; 3]> {
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|indices| [indices[0], indices[1], indices[2]].map(|i| Vec3::from(positions[i])))
            .collect()
    }

    fn contains_triangle(triangle: [Vec3; 3], point: Vec2) -> bool {
        let [a, b, c] = triangle.map(|p| p.truncate());
        [
            (b - a).perp_dot(point - a),
            (c - b).perp_dot(point - b),
            (a - c).perp_dot(point - c),
        ]
        .iter()
        .all(|side| *side >= -1e-8)
    }

    #[test]
    fn rounded_faces_and_rims_are_bounded_finite_and_keep_the_center_open() {
        for size in [Vec2::new(6.0, 3.0), Vec2::new(1.4, 0.5), Vec2::splat(0.8)] {
            for requested in [0.0_f32, 0.12, 0.4, 10.0] {
                let radius = requested.min(size.min_element() * 0.5);
                for rim in [false, true] {
                    let mut geometry = Geometry::default();
                    if rim {
                        rounded_rim(&mut geometry, Vec3::ZERO, size, [177, 146, 88], requested);
                    } else {
                        rounded_plate(&mut geometry, Vec3::ZERO, size, [24, 43, 47], requested);
                    }
                    let mesh = geometry.mesh();
                    let triangles = flat_triangles(&mesh);
                    assert!(!triangles.is_empty());
                    assert!(triangles.len() <= if rim { 72 } else { 36 });
                    for triangle in &triangles {
                        assert!(
                            (triangle[1] - triangle[0])
                                .cross(triangle[2] - triangle[0])
                                .z
                                > 0.0
                        );
                        for point in triangle {
                            assert!(point.is_finite());
                            let outside_core = (point.truncate().abs()
                                - (size * 0.5 - Vec2::splat(radius)))
                            .max(Vec2::ZERO);
                            assert!(
                                outside_core.length() <= radius + 1e-5,
                                "{size:?}, r={radius}, p={point:?}"
                            );
                        }
                        if rim {
                            assert!(
                                !contains_triangle(*triangle, Vec2::ZERO),
                                "rim filled its center"
                            );
                        }
                    }
                    let normals = mesh
                        .attribute(Mesh::ATTRIBUTE_NORMAL)
                        .unwrap()
                        .as_float3()
                        .unwrap();
                    assert!(
                        normals
                            .iter()
                            .all(|normal| Vec3::from(*normal).distance(Vec3::Z) < 1e-5)
                    );
                    if !rim {
                        assert!(
                            triangles
                                .iter()
                                .any(|triangle| contains_triangle(*triangle, Vec2::ZERO))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn utility_marks_are_smooth_front_faces_and_gear_inspection_counters_stay_open() {
        for kind in test_icon_kinds() {
            if matches!(kind, IconKind::Toy(_) | IconKind::FoodItem(_)) {
                continue;
            }
            let mesh = utility_icon_mesh(kind, [244, 238, 216]);
            let triangles = flat_triangles(&mesh);
            assert!(!triangles.is_empty());
            assert!(triangles.len() < 500, "{kind:?} geometry budget");
            for triangle in &triangles {
                assert!(triangle.iter().all(|p| p.is_finite() && p.z == 0.0));
                assert!(
                    (triangle[1] - triangle[0])
                        .cross(triangle[2] - triangle[0])
                        .z
                        > 0.0
                );
            }
            let hole = match kind {
                IconKind::Settings => Some(Vec2::ZERO),
                IconKind::Inspect => Some(Vec2::new(-0.1, 0.1)),
                _ => None,
            };
            if let Some(hole) = hole {
                assert!(
                    triangles
                        .iter()
                        .all(|triangle| !contains_triangle(*triangle, hole)),
                    "{kind:?} counter was filled"
                );
            }
        }
    }

    #[test]
    fn semantic_layers_order_every_overlay_primitive_without_depth_overlap() {
        for layer in 0..128 {
            let face = overlay_depth(layer, OverlayPart::Face);
            let rim = overlay_depth(layer, OverlayPart::Rim);
            let icon_back = overlay_depth(layer, OverlayPart::Icon) - ICON_DEPTH * 0.5;
            let icon_front = overlay_depth(layer, OverlayPart::Icon) + ICON_DEPTH * 0.5;
            let selection = overlay_depth(layer, OverlayPart::Selection);
            let text = overlay_depth(layer, OverlayPart::Text);
            let next_face = overlay_depth(layer + 1, OverlayPart::Face);
            for (behind, ahead) in [
                (face, rim),
                (rim, icon_back),
                (icon_front, selection),
                (selection, text),
                (text, next_face),
            ] {
                assert!(
                    ahead - behind > 0.001,
                    "layer {layer}: {behind} and {ahead} can fight"
                );
            }
        }
    }

    #[test]
    fn technical_notice_face_is_in_front_of_overlapping_voxel_food_icons() {
        let world = beastie_core::WorldState::new(3, "Mop");
        let scene = beastie_view::plan(
            &world,
            &beastie_view::ViewState {
                mode: beastie_view::UiMode::FoodChoice,
                status_message: Some(
                    "A long recovery notice must cover the action sheet completely. ".repeat(12),
                ),
                ..default()
            },
        )
        .0;
        let mut notice = scene
            .rects
            .iter()
            .find(|rect| rect.id == "status/background")
            .unwrap()
            .clone();
        // Deliberately place the notice over the real food panel to exercise its
        // depth contract independently of the layout's overlap avoidance.
        notice.rect = scene
            .rects
            .iter()
            .find(|rect| rect.id == "mode/food-panel")
            .unwrap()
            .rect;
        let center = logical_position(
            notice.rect.x as f32 + notice.rect.w as f32 * 0.5,
            notice.rect.y as f32 + notice.rect.h as f32 * 0.5,
            overlay_depth(notice.layer, OverlayPart::Face),
        );
        let mut face = Geometry::default();
        rounded_plate(
            &mut face,
            center,
            Vec2::new(notice.rect.w as f32, notice.rect.h as f32) / UNITS,
            [24, 43, 47],
            notice.corner_radius as f32 / UNITS,
        );
        let triangles = flat_triangles(&face.mesh());
        let mut covered = 0;
        for icon in scene
            .icons
            .iter()
            .filter(|icon| matches!(icon.kind, IconKind::FoodItem(_)))
        {
            let transform = icon_transform(icon);
            let point = transform.translation.truncate();
            if !triangles
                .iter()
                .any(|triangle| contains_triangle(*triangle, point))
            {
                continue;
            }
            covered += 1;
            let mesh = icon_mesh(icon.kind, Vec3::ZERO, [255; 3]);
            let vertices = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            for vertex in vertices {
                let actual = transform.transform_point(Vec3::from(*vertex));
                assert!(
                    actual.z < center.z - 0.001,
                    "food icon at layer {} leaks through notice layer {}",
                    icon.layer,
                    notice.layer
                );
            }
            assert!(overlay_depth(icon.layer, OverlayPart::Text) < center.z);
        }
        assert_eq!(
            covered, 3,
            "exercise all three real food miniatures under the notice"
        );
    }

    #[test]
    fn authored_overlay_surfaces_stay_between_the_tank_and_camera() {
        use beastie_view::{BindableAction, UiMode};
        let world = beastie_core::WorldState::new(3, "Mop");
        for mode in [
            UiMode::Title,
            UiMode::Compose,
            UiMode::Context(UiTarget::Creature),
            UiMode::Inspect(UiTarget::Creature),
            UiMode::FoodChoice,
            UiMode::ToyChoice,
            UiMode::OnScreenKeyboard,
            UiMode::Settings,
            UiMode::Bindings,
            UiMode::Rebinding(BindableAction::PushToTalk),
            UiMode::Rename,
            UiMode::DataManagement,
            UiMode::ConfirmReset,
        ] {
            let scene = beastie_view::plan(
                &world,
                &beastie_view::ViewState {
                    mode,
                    status_message: Some("A local technical notice".into()),
                    ..default()
                },
            )
            .0;
            for layer in scene
                .rects
                .iter()
                .map(|rect| rect.layer)
                .chain(scene.icons.iter().map(|icon| icon.layer))
                .chain(scene.text.iter().map(|text| text.layer))
            {
                assert!(overlay_depth(layer, OverlayPart::Face) > 8.0);
                assert!(
                    overlay_depth(layer, OverlayPart::Text) < 20.0,
                    "{mode:?} layer {layer} approaches camera"
                );
            }
        }
    }

    #[test]
    fn icon_geometry_is_centered_on_its_authored_target() {
        let center = logical_position(148.5, 162.0, 8.12);
        for kind in test_icon_kinds() {
            let mesh = icon_mesh(kind, center, [255; 3]);
            let positions = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            let lower = positions
                .iter()
                .copied()
                .map(Vec3::from)
                .reduce(Vec3::min)
                .unwrap();
            let upper = positions
                .iter()
                .copied()
                .map(Vec3::from)
                .reduce(Vec3::max)
                .unwrap();
            assert!(((lower + upper) * 0.5 - center).length() < 1e-5, "{kind:?}");
            assert!(
                ((upper - lower).truncate().max_element() - 1.0).abs() < 1e-5,
                "{kind:?}"
            );
            assert!(
                upper.z - lower.z <= ICON_DEPTH + 0.00001,
                "{kind:?} depth crosses UI layers"
            );
        }
    }

    #[test]
    fn dock_summary_stays_in_its_slot_and_its_label_carries_the_complete_status() {
        // The rail is narrow now that toys and foods are one-click buttons, so a long status
        // may be shortened there; the creature button's hover label always carries all of it.
        let world = beastie_core::WorldState::new(3, "Mop");
        let lettering = crate::glyphs::Lettering::default();
        for scale in [1, 2] {
            let scene = beastie_view::plan(
                &world,
                &beastie_view::ViewState {
                    text_scale: scale,
                    ..default()
                },
            )
            .0;
            let text = scene
                .text
                .iter()
                .find(|text| text.id == "compose/summary-behavior")
                .unwrap();
            assert_eq!(
                text.text,
                format!("{} · {}", scene.summary.mood_label, scene.summary.behavior)
            );
            let hovered = beastie_view::plan(
                &world,
                &beastie_view::ViewState {
                    text_scale: scale,
                    hovered_region: Some("compose/creature".to_owned()),
                    ..default()
                },
            )
            .0;
            assert!(hovered.text.iter().any(|label| label.id == "ui/hover-label"
                && label.text == format!("Mop is {}. Click to look closer.", text.text)));
            let bounds = text_content_bounds(text, &scene);
            let bounds = crate::glyphs::Bounds {
                x: bounds.x,
                y: bounds.y,
                w: bounds.w,
                h: bounds.h,
            };
            for mood in [
                "content",
                "curious",
                "hungry",
                "sleepy",
                "lonely",
                "resentful",
            ] {
                for behavior in [
                    "noticing the ball",
                    "swimming toward the ball",
                    "watching the ball",
                    "watching the berry",
                    "watching the mushroom",
                    "watching the pellet",
                    "settling after the ball",
                    "noticing the bell",
                    "swimming toward the bell",
                    "watching the bell",
                    "settling after the bell",
                    "noticing the sock",
                    "swimming toward the sock",
                    "watching the sock",
                    "settling after the sock",
                    "noticed something",
                    "stopping",
                    "watching",
                    "turning",
                    "swimming over",
                    "inspecting",
                    "eating",
                    "rejecting food",
                    "playing",
                    "doing something",
                    "settling down",
                    "sleeping",
                    "swimming to a toy",
                    "seeking comfort",
                    "watching you",
                    "staring",
                    "staying close",
                    "drifting",
                    "avoiding you",
                    "circling",
                    "investigating",
                    "settling",
                    "hovering",
                    "noticing something to do",
                    "heading somewhere on its own",
                    "finishing up",
                    "settling in the cave",
                    "changing course",
                    "nudging the ball",
                    "striking the bell",
                    "tugging the sock",
                    "resting in the cave",
                    "circling the plant",
                    "foraging in the sand",
                    "drifting through open water",
                    "following a private routine",
                ] {
                    let value = format!("{mood} · {behavior}");
                    let lines = lettering.layout_lines(&value, bounds, text.role.size(scale >= 2));
                    assert!(!lines.is_empty() && lines.len() <= 2, "scale {scale}");
                    let shown: String = lines.join(" ").trim_end_matches('…').to_owned();
                    assert!(
                        value.starts_with(shown.trim_end()),
                        "{shown} is a prefix of {value}"
                    );
                    let hover = format!("Mop is {value}. Click to look closer.");
                    let size = beastie_view::TextRole::Secondary.size(scale >= 2);
                    let typography = beastie_view::typography::typography();
                    let width = (typography.width(&hover, size).ceil() + 12.0).clamp(32.0, 220.0);
                    let height = typography.height(&hover, width - 12.0, size).ceil();
                    let hover_lines = lettering.layout_lines(
                        &hover,
                        crate::glyphs::Bounds {
                            x: 0.0,
                            y: 0.0,
                            w: width - 12.0,
                            h: height,
                        },
                        size,
                    );
                    assert_eq!(hover_lines.join(" "), hover, "scale {scale}");
                }
            }
        }
    }

    #[test]
    fn compact_inspection_preserves_complete_long_names_and_facts() {
        let lettering = crate::glyphs::Lettering::default();
        for scale in [1, 2] {
            for name in ["Mop".to_owned(), "W".repeat(24)] {
                let mut world = beastie_core::WorldState::new(3, &name);
                world
                    .creature
                    .preferences
                    .insert(beastie_core::FoodId::Mushroom, -0.8);
                for target in [
                    beastie_view::UiTarget::Creature,
                    beastie_view::UiTarget::Cave,
                    beastie_view::UiTarget::OpenWater,
                    beastie_view::UiTarget::Toy(beastie_core::ToyId::Bell),
                    beastie_view::UiTarget::Toy(beastie_core::ToyId::Sock),
                    beastie_view::UiTarget::FoodObject(900),
                    beastie_view::UiTarget::Plant(900),
                ] {
                    let scene = beastie_view::plan(
                        &world,
                        &beastie_view::ViewState {
                            mode: beastie_view::UiMode::Inspect(target),
                            text_scale: scale,
                            ..default()
                        },
                    )
                    .0;
                    for text in scene.text.iter().filter(|text| {
                        matches!(text.id.as_str(), "inspect/title" | "inspect/detail")
                    }) {
                        let bounds = text_content_bounds(text, &scene);
                        let lines = lettering.layout_lines(
                            &text.text,
                            crate::glyphs::Bounds {
                                x: bounds.x,
                                y: bounds.y,
                                w: bounds.w,
                                h: bounds.h,
                            },
                            text.role.size(scale >= 2),
                        );
                        // Long unbroken names may wrap between glyphs, so compare
                        // content without layout whitespace rather than word breaks.
                        let expected: String =
                            text.text.chars().filter(|c| !c.is_whitespace()).collect();
                        let actual: String = lines
                            .concat()
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect();
                        assert_eq!(actual, expected, "{} {target:?} scale {scale}", text.id);
                    }
                }
            }
        }
    }

    #[test]
    fn editable_labels_render_the_measured_suffix_at_the_requested_size() {
        let world = beastie_core::WorldState::new(3, "Mop");
        for scale in [1, 2] {
            let scene = beastie_view::plan(
                &world,
                &beastie_view::ViewState {
                    text_scale: scale,
                    text_buffer: format!("{}Z", "WWWWiii ".repeat(60)),
                    ..default()
                },
            )
            .0;
            let text = scene
                .text
                .iter()
                .find(|text| text.id == "compose/input-text")
                .unwrap();
            assert!(text.keep_tail);
            let bounds = text_content_bounds(text, &scene);
            let mut lettering = crate::glyphs::Lettering::default();
            let mut expected_text = text.clone();
            expected_text.text = lettering.tail_line(
                &text.text,
                crate::glyphs::Bounds {
                    x: bounds.x,
                    y: bounds.y,
                    w: bounds.w - crate::glyphs::CARET_SPACE,
                    h: bounds.h,
                },
                text.role.size(scale >= 2),
            );
            expected_text.keep_tail = true;
            assert!(expected_text.text.starts_with('…') && expected_text.text.ends_with('Z'));
            let actual = label_mesh(&mut lettering, text, &[bounds], &scene);
            let expected = label_mesh(&mut lettering, &expected_text, &[bounds], &scene);
            assert!(actual.count_vertices() > 0);
            assert_eq!(
                actual.attribute(Mesh::ATTRIBUTE_POSITION),
                expected.attribute(Mesh::ATTRIBUTE_POSITION)
            );
        }
    }

    #[test]
    fn speech_bubble_never_covers_the_authored_head() {
        // Head bounds use the actual authored voxel radii projected through the tank camera's
        // pitch, rather than the view's planning box. Animated turns remain native review.
        let half = crate::creature::authored_head_half_extents();
        let pitch = Quat::from_rotation_x(CAMERA_PITCH).inverse();
        let half_width = half.x * UNITS;
        let half_height =
            (half.y * CAMERA_PITCH.cos().abs() + half.z * CAMERA_PITCH.sin().abs()) * UNITS;
        for scale in [1, 2] {
            for x in [0, 1_500, 4_950, 5_000, 5_189, 8_500, 10_000] {
                for y in [0, 3_000, 6_000, 10_000] {
                    for speech in [
                        "hm.".to_owned(),
                        "ball? mop wants the red ball now".to_owned(),
                        "W".repeat(beastie_protocol::MAX_DIALOGUE_REPLY_BYTES),
                    ] {
                        let mut world = beastie_core::WorldState::new(3, "Mop");
                        world.creature.aquarium.position =
                            beastie_core::NormalizedPosition::new(x, y);
                        let mut view = beastie_view::ViewState {
                            text_scale: scale,
                            ..default()
                        };
                        view.show_speech(speech.clone(), 0);
                        let scene = beastie_view::plan(&world, &view).0;
                        let head = pitch * crate::creature::head_position(&scene);
                        let head_x = head.x * UNITS + LOGICAL_WIDTH * 0.5;
                        let head_y = LOGICAL_HEIGHT * 0.5 - head.y * UNITS;
                        for part in scene
                            .rects
                            .iter()
                            .filter(|part| !part.outline && part.id.starts_with("speech/"))
                        {
                            let clear_x = (part.rect.x + part.rect.w) as f32 <= head_x - half_width
                                || part.rect.x as f32 >= head_x + half_width;
                            let clear_y = (part.rect.y + part.rect.h) as f32
                                <= head_y - half_height
                                || part.rect.y as f32 >= head_y + half_height;
                            assert!(
                                clear_x || clear_y,
                                "{} covers the authored head at ({head_x}, {head_y}) scale {scale}",
                                part.id
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn speech_bubble_text_fits_the_authored_font() {
        let world = beastie_core::WorldState::new(3, "Mop");
        let lettering = crate::glyphs::Lettering::default();
        for scale in [1, 2] {
            for speech in [
                "mop likes ball".to_owned(),
                "ball? mop wants the red ball now, please".to_owned(),
                "café e\u{301} 🌿 bell".to_owned(),
                "👩‍👩‍👧‍👦 👩‍👩‍👧‍👦 sock".to_owned(),
            ] {
                let mut view = beastie_view::ViewState {
                    text_scale: scale,
                    ..default()
                };
                view.show_speech(speech.clone(), 0);
                let scene = beastie_view::plan(&world, &view).0;
                let caption = scene
                    .text
                    .iter()
                    .find(|text| text.id == "speech/text")
                    .unwrap();
                let bounds = text_content_bounds(caption, &scene);
                let lines = lettering.layout_lines(
                    &caption.text,
                    crate::glyphs::Bounds {
                        x: bounds.x,
                        y: bounds.y,
                        w: bounds.w,
                        h: bounds.h,
                    },
                    caption.role.size(scale >= 2),
                );
                // Short lines show whole: nothing is dropped or ellipsized.
                let expected: String = speech.chars().filter(|c| !c.is_whitespace()).collect();
                let actual: String = lines
                    .concat()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                assert_eq!(actual, expected, "{speech} scale {scale}");
            }
        }
    }

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
            vertical_centered: false,
            keep_tail: false,
            input_state: None,
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
            corner_radius: 0,
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
    fn all_modal_labels_have_room_for_requested_normal_and_large_font_heights() {
        let world = beastie_core::WorldState::new(3, "Mop");
        for scale in [1, 2] {
            for mode in [
                beastie_view::UiMode::Title,
                beastie_view::UiMode::Compose,
                beastie_view::UiMode::Settings,
                beastie_view::UiMode::Bindings,
                beastie_view::UiMode::Rebinding(beastie_view::BindableAction::PushToTalk),
                beastie_view::UiMode::FoodChoice,
                beastie_view::UiMode::ToyChoice,
                beastie_view::UiMode::Context(beastie_view::UiTarget::Creature),
                beastie_view::UiMode::Context(beastie_view::UiTarget::Toy(
                    beastie_core::ToyId::Ball,
                )),
                beastie_view::UiMode::Inspect(beastie_view::UiTarget::Creature),
                beastie_view::UiMode::Rename,
                beastie_view::UiMode::OnScreenKeyboard,
                beastie_view::UiMode::DataManagement,
                beastie_view::UiMode::ConfirmReset,
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
                    let size = text.role.size(text.scale >= 2);
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
    fn large_settings_navigation_labels_fit_inside_their_cards() {
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
        let lettering = crate::glyphs::Lettering::default();
        let mut checked = 0;
        for text in &scene.text {
            let Some(id) = text
                .id
                .strip_suffix("-name")
                .or_else(|| text.id.strip_suffix("-detail"))
            else {
                continue;
            };
            if !id.starts_with("settings/") {
                continue;
            }
            let card = scene
                .hit_regions
                .iter()
                .find(|hit| hit.id == id)
                .expect("settings navigation card");
            let bounds = text_content_bounds(text, &scene);
            assert!(bounds.x >= card.rect.x as f32 && bounds.y >= card.rect.y as f32);
            assert!(bounds.x + bounds.w <= (card.rect.x + card.rect.w) as f32);
            assert!(bounds.y + bounds.h <= (card.rect.y + card.rect.h) as f32);
            let lines = lettering.layout_lines(
                &text.text,
                crate::glyphs::Bounds {
                    x: bounds.x,
                    y: bounds.y,
                    w: bounds.w,
                    h: bounds.h,
                },
                text.role.size(true),
            );
            assert_eq!(
                lines.join(" "),
                text.text,
                "{} lost text at its requested Large size",
                text.id
            );
            checked += 1;
        }
        assert!(
            checked >= 4,
            "exercise navigation names and supporting copy"
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
                .hit_regions
                .iter()
                .find(|hit| hit.id == value.id.strip_suffix("-value").unwrap())
                .expect("value's interactive control");
            assert!(bounds.x >= button.rect.x as f32);
            assert!(bounds.x + bounds.w <= (button.rect.x + button.rect.w) as f32);
            assert!(bounds.y >= button.rect.y as f32);
            assert!(bounds.y + bounds.h <= (button.rect.y + button.rect.h) as f32);
        }
    }
}
