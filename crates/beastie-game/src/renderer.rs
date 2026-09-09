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
    text_geometry: Option<Handle<Mesh>>,
    panel_geometry: Option<Handle<Mesh>>,
    panel_entity: Option<Entity>,
}

impl UiCache {
    fn icon(&mut self, meshes: &mut Assets<Mesh>, kind: IconKind, color: [u8; 3]) -> Handle<Mesh> {
        // Toy models author their own colors; enabled palettes must share them.
        let color = if matches!(kind, IconKind::Toy(_)) {
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
        transform.scale = Vec3::ONE;
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
    if let IconKind::Toy(toy) = kind {
        let mesh = object_mesh(ObjectKind::Toy(toy), default());
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
        let scale = 0.54 / (max - min).max_element();
        return mesh
            .translated_by(-(min + max) * 0.5)
            .scaled_by(Vec3::splat(scale))
            .translated_by(center);
    }
    if kind == IconKind::Settings {
        let mut gear = crate::voxel::VoxelModel::default();
        for x in -4_i32..=4 {
            for y in -4_i32..=4 {
                let radius = x * x + y * y;
                if radius >= 5 && (radius <= 13 || x.abs() <= 1 || y.abs() <= 1) {
                    gear.set([x, y, 0], color);
                }
            }
        }
        return gear
            .mesh_with_style(0.047, crate::voxel::SurfaceStyle::Sharp)
            .translated_by(center);
    }
    let cell = 0.062;
    let pattern: &[&str] = match kind {
        IconKind::Microphone => &[
            "  xx  ", "  xx  ", "x xx x", "x xx x", " xxxx ", "  xx  ", " xxxx ",
        ],
        IconKind::Food => &["  x   ", " xxx  ", "xxxxx ", "xxxxx ", " xxx  "],
        IconKind::Settings => &[" x  x ", "xxxxxx", "xx  xx", "xx  xx", "xxxxxx", " x  x "],
        IconKind::Send => &[
            "x     ", "xxx   ", "xxxxx ", "xxxxxx", "xxxxx ", "xxx   ", "x     ",
        ],
        IconKind::Toy(_) => unreachable!("toy miniatures return above"),
    };
    let mut model = crate::voxel::VoxelModel::default();
    let mut lower = IVec2::splat(i32::MAX);
    let mut upper = IVec2::splat(i32::MIN);
    for (row, line) in pattern.iter().enumerate() {
        for (column, byte) in line.bytes().enumerate() {
            if byte == b'x' {
                let point = IVec2::new(column as i32, -(row as i32));
                lower = lower.min(point);
                upper = upper.max(point);
                model.set([point.x, point.y, 0], color);
            }
        }
    }
    // Center occupied cells, not the padded stencil: Food has an empty last column.
    let offset = (lower + upper).as_vec2() * (cell * 0.5);
    model
        .mesh_with_style(cell, crate::voxel::SurfaceStyle::Sharp)
        .translated_by(center - offset.extend(0.0))
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
    palette: Res<'w, Palette>,
    lettering: ResMut<'w, crate::glyphs::Lettering>,
    cache: ResMut<'w, UiCache>,
    meshes: ResMut<'w, Assets<Mesh>>,
    timing: Option<Res<'w, crate::ray_stats::ComputeGpuTiming>>,
}

/// Five joined voxel-width strips clip the corners without smooth vector geometry.
/// Semantic hit rectangles stay generous, including the tiny omitted corners.
fn stepped_plate(shape: &mut Geometry, center: Vec3, size: Vec3, color: [u8; 3], corner: f32) {
    let c = corner.min(size.x * 0.2).min(size.y * 0.2);
    shape.cuboid(center, Vec3::new(size.x - c * 4.0, size.y, size.z), color);
    for side in [-1.0, 1.0] {
        shape.cuboid(
            center + Vec3::X * side * (size.x * 0.5 - c * 1.5),
            Vec3::new(c, size.y - c * 2.0, size.z),
            color,
        );
        shape.cuboid(
            center + Vec3::X * side * (size.x * 0.5 - c * 0.5),
            Vec3::new(c, size.y - c * 4.0, size.z),
            color,
        );
    }
}

fn stepped_rim(shape: &mut Geometry, center: Vec3, size: Vec2, color: [u8; 3], corner: f32) {
    let c = corner.min(size.x * 0.2).min(size.y * 0.2);
    let stroke = 0.012;
    let upper = color.map(|v| v.saturating_add(9));
    for side in [-1.0, 1.0] {
        let edge = if side > 0.0 { upper } else { color };
        shape.cuboid(
            center + Vec3::Y * side * size.y * 0.5,
            Vec3::new(size.x - c * 4.0, stroke, 0.026),
            edge,
        );
        shape.cuboid(
            center + Vec3::X * side * size.x * 0.5,
            Vec3::new(stroke, size.y - c * 4.0, 0.026),
            color,
        );
        for vertical in [-1.0, 1.0] {
            for step in 0..2 {
                let k = step as f32;
                let x = side * (size.x * 0.5 - c * (1.5 - k));
                let y = vertical * (size.y * 0.5 - c * (1.0 + k));
                shape.cuboid(
                    center + Vec3::new(x, y, 0.0),
                    Vec3::new(c + stroke, stroke, 0.026),
                    color,
                );
                shape.cuboid(
                    center + Vec3::new(x - side * c * 0.5, y + vertical * c * 0.5, 0.0),
                    Vec3::new(stroke, c + stroke, 0.026),
                    color,
                );
            }
        }
    }
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
                [216, 219, 185]
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
                8.0 + rect.layer as f32 * 0.002,
            );
            let color = [rect.color[0], rect.color[1], rect.color[2]];
            let size = Vec3::new(r.w as f32 / UNITS, r.h as f32 / UNITS, 0.02);
            let panel_id = rect.id.strip_suffix("-edge").unwrap_or(&rect.id);
            let crafted = rect.id.starts_with("title/")
                || rect.id.starts_with("settings/")
                || panel_id.ends_with("-panel")
                || matches!(
                    panel_id,
                    "rename/prompt-background"
                        | "mode/drop-food-background"
                        | "data/panel"
                        | "reset/panel"
                        | "bindings/panel"
                        | "bindings/capture/panel"
                        | "keyboard/panel"
                );
            let corner = if r.h > 20 { 0.050 } else { 0.025 };
            if rect.id == "settings/panel" {
                if panel_changed {
                    stepped_plate(&mut panel, center, size, color, corner);
                }
                has_panel = true;
                continue;
            }
            if crafted && rect.outline {
                stepped_rim(&mut shape, center, size.truncate(), color, corner);
            } else if rect.outline {
                for y in [-size.y * 0.5, size.y * 0.5] {
                    shape.cuboid(center + Vec3::Y * y, Vec3::new(size.x, 0.014, 0.02), color);
                }
                for x in [-size.x * 0.5, size.x * 0.5] {
                    shape.cuboid(center + Vec3::X * x, Vec3::new(0.014, size.y, 0.02), color);
                }
            } else if crafted && r.h >= 6 && r.w >= 6 {
                stepped_plate(&mut shape, center, size, color, corner);
            } else {
                shape.cuboid(center, size, color);
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
                let center = logical_position(
                    icon.bounds.x as f32 + icon.bounds.w as f32 * 0.5,
                    icon.bounds.y as f32 + icon.bounds.h as f32 * 0.5,
                    8.05 + icon.layer as f32 * 0.002,
                );
                let handle = ui.cache.icon(&mut ui.meshes, icon.kind, color);
                if let Some(&entity) = ui.cache.icon_slots.get(index) {
                    ui.commands.entity(entity).insert((
                        Mesh3d(handle),
                        Transform::from_translation(center),
                        Visibility::Inherited,
                    ));
                } else {
                    let entity = ui
                        .commands
                        .spawn((
                            crate::ray_scene::RayOverlay,
                            Mesh3d(handle),
                            MeshMaterial3d(ui.palette.ui.clone()),
                            Transform::from_translation(center),
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
    let clips = (labels_changed || rects_changed || ui.cache.text_geometry.is_none()).then(|| {
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
        || ui.cache.text_geometry.is_none()
        || clips
            .as_ref()
            .is_some_and(|clips| *clips != ui.cache.text_clips);
    if text_changed {
        let clips = clips.unwrap_or_else(|| ui.cache.text_clips.clone());
        let mut lettering_mesh = crate::glyphs::LetterMesh::default();
        let labels = ui.frame.plan.text.clone();
        for (text, visible) in labels.into_iter().zip(&clips) {
            let bounds = text_content_bounds(&text, &ui.frame.plan);
            let to_glyph_bounds = |b: TextBox| crate::glyphs::Bounds {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
            };
            let clips: Vec<_> = visible.iter().copied().map(to_glyph_bounds).collect();
            ui.lettering.append(
                &mut lettering_mesh,
                crate::glyphs::Label {
                    text: &text.text,
                    bounds: to_glyph_bounds(bounds),
                    clips: &clips,
                    size: fitting_font_size(&text, bounds),
                    centered: text.role.centered(),
                    vertical_centered: text.vertical_centered,
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
        ui.cache.text_clips = clips;
    }
    if rects_changed {
        ui.cache.rects = ui.frame.plan.rects.clone();
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
                    8.05 + icon.layer as f32 * 0.002,
                );
                let original = icon_mesh(icon.kind, center, [216, 219, 185]);
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
        for kind in [
            IconKind::Food,
            IconKind::Microphone,
            IconKind::Send,
            IconKind::Settings,
            IconKind::Toy(ToyId::Ball),
            IconKind::Toy(ToyId::Bell),
            IconKind::Toy(ToyId::Sock),
        ] {
            let handle = cache.icon(&mut assets, kind, [216, 219, 185]);
            if matches!(kind, IconKind::Toy(_)) {
                assert_eq!(
                    handle,
                    cache.icon(&mut assets, kind, [88, 114, 118]),
                    "toy palettes share their authored colored mesh"
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
        assert_eq!(cache.icon_templates.len(), 7);
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
            });
            frame.plan.hit_regions.truncate(1);
            assert_eq!(frame.plan.hit_regions.len(), 1);
            frame.plan.hit_regions[0].id = "compose/send".into();
            frame.plan.hit_regions[0].enabled = true;
        }
        app.update();
        let text = app
            .world()
            .resource::<UiCache>()
            .text_geometry
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
    fn icon_geometry_is_centered_on_its_authored_target() {
        let center = logical_position(148.5, 162.0, 8.12);
        for kind in [
            IconKind::Food,
            IconKind::Microphone,
            IconKind::Send,
            IconKind::Settings,
            IconKind::Toy(beastie_core::ToyId::Ball),
            IconKind::Toy(beastie_core::ToyId::Bell),
            IconKind::Toy(beastie_core::ToyId::Sock),
        ] {
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
            value_size >= value.role.size(true) - 0.01,
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
