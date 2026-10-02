//! An articulated, solid voxel animal. Semantics come exclusively from the scene plan.
use beastie_core::{ActionPhase, ActivityPhase, Facing, GazeTarget, SteeringMode};
use beastie_view::{CreaturePose, CreatureScene, PresentationCueKind, ScenePlan};
use bevy::prelude::*;

use crate::{
    body::{BodyChain, BodyTrail},
    renderer::SceneFrame,
    voxel::VoxelModel,
};

#[path = "creature_art.rs"]
mod art;
use art::{Acting, CREATURE, MOTION};

const SEGMENTS: usize = CREATURE.segments;
const BODY_RADII: [i32; 3] = [6, 5, 5];
const BODY_CELL: f32 = 0.065;

#[derive(Component)]
pub(crate) enum CreaturePart {
    Head,
    Eye(f32),
    Pupil(f32),
    Glint(f32),
    Brow(f32),
    Mouth,
    LowerLip,
    Tongue,
    MouthCorner(f32),
    Cheek(f32),
    Fin(f32),
    Crest,
    Body(usize),
    Tail,
}

struct Motion {
    trail: BodyTrail,
    chain: BodyChain,
    last_ms: u64,
    yaw: f32,
    speed: f32,
    swim_phase: f32,
    idle_phase: f32,
    gesture: Vec3,
    face: Vec3,
    body_pick_volumes: Vec<(Transform, Vec3)>,
}

#[derive(Resource, Default)]
pub(crate) struct CreatureMotion {
    pub presented_head: Vec3,
    state: Option<Motion>,
}

impl CreatureMotion {
    pub fn position(&self, plan: &ScenePlan) -> Vec3 {
        if self.state.is_some() {
            self.presented_head
        } else {
            head_position(plan)
        }
    }

    /// Body picking consumes the same rendered transforms, including curvature,
    /// taper and gait. These ellipsoids approximate the voxel body, not the tail fin.
    pub(crate) fn body_pick_volumes(&self) -> &[(Transform, Vec3)] {
        self.state
            .as_ref()
            .map_or(&[], |state| &state.body_pick_volumes)
    }

    fn advance(&mut self, plan: &ScenePlan) -> (Vec3, f32) {
        let now = plan.elapsed_ms.saturating_add(plan.simulation_remainder_ms);
        let target = head_position(plan);
        let discontinuity = self.state.as_ref().is_none_or(|state| {
            now < state.last_ms
                || now.saturating_sub(state.last_ms) > 1000
                || target.distance(self.presented_head) > 3.0
        });
        if discontinuity {
            let facing = if plan.creature.facing == Facing::Left {
                -1.0
            } else {
                1.0
            };
            let direction = Vec3::X
                * if target.x < -5.0 {
                    -1.0
                } else if target.x > 5.0 {
                    1.0
                } else {
                    facing
                };
            self.presented_head = target;
            self.state = Some(Motion {
                trail: BodyTrail::new(target, direction),
                chain: BodyChain::new(target, direction, SEGMENTS),
                last_ms: now,
                yaw: 0.0,
                speed: 0.0,
                swim_phase: 0.0,
                idle_phase: 0.0,
                gesture: Vec3::ZERO,
                face: Vec3::ZERO,
                body_pick_volumes: Vec::with_capacity(SEGMENTS),
            });
        }
        let state = self.state.as_mut().expect("motion initialized above");
        let dt = now.saturating_sub(state.last_ms).min(250) as f32 / 1000.0;
        state.last_ms = now;
        // Correct tick-boundary prediction errors at creature speed, never a dash across
        // the tank. The ceiling accommodates the fastest authoritative flee velocity.
        let correction = (target - self.presented_head) * (1.0 - (-dt * 10.0).exp());
        let previous_head = self.presented_head;
        let swim_speed = Vec2::new(
            plan.creature.velocity.x as f32 * 0.00132,
            plan.creature.velocity.y as f32 * 0.000455,
        )
        .length();
        let correction_limit = (swim_speed.max(0.9) + 0.15).min(1.9);
        self.presented_head += correction.clamp_length_max(dt * correction_limit);
        // Hover drift can exceed the numerical movement threshold. Only authoritative
        // locomotion restarts body turns; buoyancy preserves the achieved resting pose.
        let forward = if matches!(
            plan.creature.steering,
            SteeringMode::Drift
                | SteeringMode::Approach
                | SteeringMode::Flee
                | SteeringMode::Orbit
                | SteeringMode::Turn
        ) {
            Vec3::new(
                plan.creature.velocity.x as f32 * 0.00132,
                -plan.creature.velocity.y as f32 * 0.000455,
                0.0,
            )
        } else {
            Vec3::ZERO
        };
        // Consume the complete host delta. Articulation has a bounded integration step,
        // but discarding time made low-frame-rate travel lag until it hit the reset distance.
        let steps = (dt / 0.025).ceil().max(1.0) as u32;
        for step in 1..=steps {
            let head = previous_head.lerp(self.presented_head, step as f32 / steps as f32);
            state.trail.advance(head);
            state
                .chain
                .advance(head, forward, &state.trail, dt / steps as f32);
        }
        (self.presented_head, dt)
    }
}

fn golden(point: [i32; 3]) -> [u8; 3] {
    if point[1] < -3 && point[2] > 0 {
        CREATURE.belly
    } else if point[1] > 5 {
        CREATURE.crown
    } else if (point[0] + point[1] * 3 + point[2] * 7).rem_euclid(19) == 0 {
        CREATURE.freckle
    } else {
        CREATURE.body
    }
}

/// Keep stepped geometry while lighting follows the rounded animal volume.
/// Some geometric normal remains so chamfers catch light without a dark cell grid.
fn rounded_mesh(
    model: VoxelModel,
    radii: [i32; 3],
    cell: f32,
    style: crate::voxel::SurfaceStyle,
) -> Mesh {
    let mut mesh = model.mesh_with_style(cell, style);
    if style == crate::voxel::SurfaceStyle::Separated {
        return mesh;
    }
    let extent = Vec3::from_array(radii.map(|r| (r as f32 + 0.5) * cell));
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
    let normals: Vec<_> = positions
        .iter()
        .zip(normals)
        .map(|(p, n)| {
            let volume = (Vec3::from_array(*p) / (extent * extent)).normalize_or_zero();
            (Vec3::from_array(*n) * 0.25 + volume * 0.75)
                .normalize_or_zero()
                .to_array()
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh
}

fn fin_mesh(style: crate::voxel::SurfaceStyle) -> Mesh {
    let mut fin = VoxelModel::default();
    for x in 0_i32..9 {
        for y in -x / 2..=x / 2 {
            let color = if x > 6 {
                CREATURE.fin_tip
            } else {
                CREATURE.fin_base
            };
            fin.set([x, y, 0], color);
        }
    }
    fin.mesh_with_style(CREATURE.fin_cell, style)
}

pub(crate) fn setup_creature(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    appearance: Res<crate::appearance::RenderAppearance>,
) {
    commands.init_resource::<CreatureMotion>();
    let style = appearance.style();
    let solid = |radii, cell, color| {
        rounded_mesh(VoxelModel::ellipsoid(radii, |_| color), radii, cell, style)
    };
    let material = materials.add(appearance.surface(crate::appearance::SurfaceMaterial::Skin));
    let eye_material = materials.add(appearance.surface(crate::appearance::SurfaceMaterial::Eye));
    let face_material = materials.add(StandardMaterial {
        unlit: true,
        ..default()
    });
    // Rigid animation only changes entity transforms. Identical parts share one
    // mesh asset and therefore one cached BLAS in the compute renderer.
    let mut add_mesh = |mesh| meshes.add(appearance.mesh(mesh));
    let head = add_mesh(rounded_mesh(
        VoxelModel::ellipsoid(CREATURE.head_radii, golden),
        CREATURE.head_radii,
        CREATURE.head_cell,
        style,
    ));
    let eyes = add_mesh(solid([4, 5, 2], 0.044, CREATURE.eye));
    let pupils = add_mesh(solid([2, 3, 1], 0.036, CREATURE.pupil));
    let glints = add_mesh(solid([0, 0, 0], 0.039, CREATURE.glint));
    let brows = add_mesh(solid([3, 0, 1], 0.045, CREATURE.brow));
    let mouth_corners = add_mesh(solid([0, 1, 0], 0.038, CREATURE.mouth));
    let cheeks = add_mesh(solid([2, 1, 0], 0.039, CREATURE.cheek));
    let fin = add_mesh(fin_mesh(style));
    let mouth = add_mesh(solid([3, 1, 1], 0.036, CREATURE.mouth));
    let lower_lip = add_mesh(solid([2, 0, 0], 0.036, CREATURE.lip));
    let tongue = add_mesh(solid([1, 0, 0], 0.034, CREATURE.tongue));
    let body = add_mesh(rounded_mesh(
        VoxelModel::ellipsoid(BODY_RADII, golden),
        BODY_RADII,
        BODY_CELL,
        style,
    ));
    let mut spawn = |part: CreaturePart, mesh: Handle<Mesh>, facial: bool| {
        let eye = matches!(part, CreaturePart::Eye(_));
        commands.spawn((
            part,
            Mesh3d(mesh),
            MeshMaterial3d(if eye {
                eye_material.clone()
            } else if facial {
                face_material.clone()
            } else {
                material.clone()
            }),
            Transform::default(),
        ));
    };
    spawn(CreaturePart::Head, head, false);
    for side in [-1.0, 1.0] {
        spawn(CreaturePart::Eye(side), eyes.clone(), true);
        spawn(CreaturePart::Pupil(side), pupils.clone(), true);
        spawn(CreaturePart::Glint(side), glints.clone(), true);
        spawn(CreaturePart::Brow(side), brows.clone(), false);
        spawn(CreaturePart::MouthCorner(side), mouth_corners.clone(), true);
        spawn(CreaturePart::Cheek(side), cheeks.clone(), false);
        spawn(CreaturePart::Fin(side), fin.clone(), false);
    }
    spawn(CreaturePart::Mouth, mouth, true);
    spawn(CreaturePart::LowerLip, lower_lip, true);
    spawn(CreaturePart::Tongue, tongue, true);
    spawn(CreaturePart::Crest, fin.clone(), false);
    for index in 0..SEGMENTS {
        spawn(CreaturePart::Body(index), body.clone(), false);
    }
    spawn(CreaturePart::Tail, fin, false);
}

#[cfg(test)]
pub(crate) fn authored_head_half_extents() -> Vec3 {
    Vec3::from_array(
        CREATURE
            .head_radii
            .map(|radius| (radius as f32 + 0.5) * CREATURE.head_cell),
    )
}

/// Continuous projection used by rendering and head picking. It never changes simulation state.
pub(crate) fn head_position(plan: &ScenePlan) -> Vec3 {
    let c = &plan.creature;
    // A saved action can still contain the velocity of its preceding approach.
    // Stationary phases and sleep never predict another second of that travel.
    let stationary = c.pose == CreaturePose::Sleep
        || c.action_phase
            .is_some_and(|phase| phase != ActionPhase::Approach);
    let fraction = if stationary {
        0.0
    } else {
        plan.simulation_remainder_ms.min(999) as f32 / 1_000.0
    };
    let project = |position: i32, velocity: i32, target: Option<i32>| {
        let step = (velocity as f32 * fraction).round() as i32;
        let step = target.map_or(step, |target| {
            let remaining = target.saturating_sub(position);
            step.clamp(remaining.min(0), remaining.max(0))
        });
        position.saturating_add(step)
    };
    let future = beastie_core::NormalizedPosition::new(
        project(c.position.x, c.velocity.x, c.movement_target.map(|p| p.x)),
        project(c.position.y, c.velocity.y, c.movement_target.map(|p| p.y)),
    )
    .clamped();
    crate::renderer::world_position(future)
}

fn acting(
    creature: &CreatureScene,
    time: f32,
    remainder_ms: u64,
    reduced_motion: bool,
    reduced_shake: bool,
) -> Acting {
    let mut a = art::mood(creature.mood);
    if creature.pose == CreaturePose::Sleep {
        a.eye_open = 0.055;
        a.asymmetry = 0.0;
        a.nod = -0.13;
        a.fin = -0.2;
    }
    if creature.pose == CreaturePose::Eat {
        a.mouth = 0.25
            + (creature.phase_elapsed_ms.saturating_add(remainder_ms) as f32 * 0.008)
                .sin()
                .abs()
                * 0.4;
        a.nod = 0.04;
    }
    if creature.pose == CreaturePose::Play {
        a.fin = 0.18;
    }
    // Scene projection has already selected the owner and priority. Never retain a displaced cue.
    if let Some(expression) = &creature.expression {
        let beat =
            (expression.elapsed_ms.saturating_add(remainder_ms) as f32 / 1_000.0 * 6.0).sin();
        if let Some(recipe) = art::expression(expression.cue) {
            a = recipe;
            // Facial meaning is immediate. Only the supporting gesture eases into its pose.
            let settle = 1.0
                - (-(expression.elapsed_ms.saturating_add(remainder_ms) as f32)
                    / MOTION.gesture_arrival_ms)
                    .exp();
            a.tilt *= settle;
            a.nod *= settle;
            a.fin *= settle;
        }
        match expression.cue {
            PresentationCueKind::Recoil | PresentationCueKind::Spit => {
                a.tilt = if reduced_shake { 0.0 } else { beat * 0.12 };
            }
            PresentationCueKind::Crumbs => {
                a.mouth = 0.3 + beat.abs() * 0.45;
            }
            _ => {}
        }
    }

    if let Some(activity) = &creature.private_life {
        let owns_expression = creature.expression.as_ref().is_none_or(|expression| {
            matches!(expression.owner, beastie_view::SemanticOwner::PrivateLife(id) if id == activity.id)
        });
        if owns_expression && activity.phase == ActivityPhase::Act {
            a = art::activity(
                activity.recipe,
                activity.elapsed_ms.saturating_add(remainder_ms),
            );
        }
    }
    // Preparation belongs to the actual action phase, never a predicted payoff.
    if creature.expression.is_none()
        && matches!(
            creature.action_phase,
            Some(ActionPhase::Notice | ActionPhase::Gaze | ActionPhase::Inspect)
        )
    {
        a.eye_open = a.eye_open.max(1.03);
        a.asymmetry = 0.12;
        a.brow = 0.2;
        a.fin = -0.12;
        a.nod = -0.045;
    }
    // The exact current speech owner drives these phases, so cancellation closes the mouth now.
    if creature.speaking {
        let shape = &art::SPEECH[usize::from(creature.mouth_phase.min(2))];
        a.mouth = shape.open;
        a.mouth_width = shape.width;
        a.smile *= 0.4;
    }
    let blink = time.rem_euclid(MOTION.blink_period);
    if !reduced_motion && blink < MOTION.blink_duration && creature.pose != CreaturePose::Sleep {
        let close = (blink / (MOTION.blink_duration * 0.5) - 1.0)
            .abs()
            .max(0.055);
        a.eye_open *= close;
        a.asymmetry *= close;
    }
    if reduced_motion {
        a.tilt *= MOTION.reduced_gesture;
        a.nod *= MOTION.reduced_gesture;
        a.fin *= MOTION.reduced_gesture;
    }
    a
}

pub(crate) fn animate_creature(
    frame: Res<SceneFrame>,
    mut motion: ResMut<CreatureMotion>,
    mut parts: Query<(&CreaturePart, &mut Transform)>,
) {
    let plan = &frame.plan;
    let c = &plan.creature;
    let now = plan.elapsed_ms.saturating_add(plan.simulation_remainder_ms);
    let blink_time = ((now as f64 / 1_000.0).rem_euclid(f64::from(MOTION.blink_period))) as f32;
    let (head, dt) = motion.advance(plan);
    let facing = if c.facing == Facing::Left { -1.0 } else { 1.0 };
    let motion = motion.state.as_mut().expect("motion initialized above");
    // Integer-frequency fin and tail cycles share a bounded phase, without the old
    // one-hour session-clock reset or large floating-point arguments.
    motion.idle_phase = (motion.idle_phase + dt).rem_euclid(std::f32::consts::TAU);
    let time = motion.idle_phase;
    let target_speed = Vec2::new(c.velocity.x as f32, c.velocity.y as f32).length() / 750.0;
    motion.speed += (target_speed.min(1.0) - motion.speed) * (1.0 - (-dt * 6.0).exp());
    // Integrate frequency. Multiplying the session clock by changing speed shifts
    // the entire wave on every acceleration, increasingly violently as the save ages.
    motion.swim_phase =
        (motion.swim_phase + dt * (3.0 + motion.speed * 3.0)).rem_euclid(std::f32::consts::TAU);
    let to_player = matches!(c.gaze, GazeTarget::Player | GazeTarget::Cursor) || c.speaking;
    let gaze_delta = c
        .gaze_position
        .map(|target| crate::renderer::world_position(target) - head);
    let target_yaw = gaze_delta.map_or(if to_player { 0.0 } else { facing * 0.33 }, |delta| {
        (delta.x * 0.12).clamp(-0.38, 0.38)
    });
    motion.yaw += (target_yaw - motion.yaw) * (1.0 - (-dt * 7.0).exp());
    let a = acting(
        c,
        blink_time,
        plan.simulation_remainder_ms,
        plan.reduced_motion,
        plan.reduced_shake,
    );
    // Blend supporting body gestures across ownership changes. Facial meaning and
    // speech cancellation still use the current recipe immediately.
    let gesture_target = Vec3::new(a.nod, a.tilt, a.fin);
    motion.gesture += (gesture_target - motion.gesture) * (1.0 - (-dt * 12.0).exp());
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        motion.yaw,
        -motion.gesture.x,
        motion.gesture.y,
    );
    // Tiny face features need a shorter settle than body acting. Preserve authored
    // blinks and current speech aperture while removing one-frame brow/smile flips.
    motion.face +=
        (Vec3::new(a.brow, a.smile, a.asymmetry) - motion.face) * (1.0 - (-dt * 18.0).exp());
    let gait = if plan.reduced_motion || c.pose == CreaturePose::Sleep {
        0.0
    } else {
        MOTION.idle_gait + motion.speed * MOTION.swim_gait
    };
    let gaze_x = gaze_delta.map_or(
        if to_player {
            -motion.yaw * 0.07
        } else {
            facing * 0.035
        },
        |delta| (delta.x * 0.025).clamp(-0.045, 0.045),
    );
    let gaze_y = gaze_delta.map_or(0.0, |delta| (delta.y * 0.025).clamp(-0.045, 0.045));
    motion.body_pick_volumes.clear();
    for (part, mut transform) in &mut parts {
        let mut local = Vec3::ZERO;
        let mut scale = Vec3::ONE;
        let mut turn = Quat::IDENTITY;
        match *part {
            CreaturePart::Head => {}
            CreaturePart::Eye(side) => {
                local = Vec3::new(side * CREATURE.eye_spacing, 0.10, 0.455);
                scale.y = (a.eye_open + side * a.asymmetry).clamp(0.045, 1.25);
            }
            CreaturePart::Pupil(side) => {
                local = Vec3::new(
                    side * CREATURE.eye_spacing + gaze_x,
                    0.095 + gaze_y * a.eye_open,
                    0.555,
                );
                scale.y = (a.eye_open + side * a.asymmetry).clamp(0.045, 1.0);
            }
            CreaturePart::Glint(side) => {
                local = Vec3::new(
                    side * CREATURE.eye_spacing + gaze_x - 0.02,
                    0.095 + (0.045 + gaze_y) * (a.eye_open + side * a.asymmetry).clamp(0.045, 1.0),
                    0.611,
                );
                scale = Vec3::splat((a.eye_open + side * a.asymmetry).clamp(0.045, 1.0));
            }
            CreaturePart::Brow(side) => {
                local = Vec3::new(
                    side * CREATURE.eye_spacing,
                    0.36 + side * motion.face.z * 0.12,
                    0.46,
                );
                turn = Quat::from_rotation_z(side * motion.face.x + motion.face.z * 0.8);
            }
            CreaturePart::Mouth => {
                local = Vec3::new(0.0, -0.24, 0.49);
                scale.y = 0.3 + a.mouth * 2.4;
                scale.x = a.mouth_width * (1.0 - a.mouth * 0.18);
            }
            CreaturePart::LowerLip => {
                local = Vec3::new(0.0, -0.256 - a.mouth * 0.13, 0.538);
                scale.x = a.mouth_width;
                scale.y = 0.6;
            }
            CreaturePart::Tongue => {
                local = Vec3::new(0.0, -0.245 - a.mouth * 0.10, 0.535);
                // Disappears into the closed mouth, without stale speech state or entity churn.
                scale = Vec3::new(a.mouth_width, a.mouth.clamp(0.0, 0.8), 1.0);
            }
            CreaturePart::MouthCorner(side) => {
                local = Vec3::new(side * 0.12 * a.mouth_width, -0.24 + motion.face.y, 0.493);
                turn = Quat::from_rotation_z(-side * motion.face.y * 5.0);
            }
            CreaturePart::Cheek(side) => {
                local = Vec3::new(side * 0.43, -0.12, 0.40);
            }
            CreaturePart::Fin(side) => {
                local = Vec3::new(side * 0.53, -0.09, -0.08);
                turn = Quat::from_euler(
                    EulerRot::XYZ,
                    0.1,
                    side * 0.18,
                    if side < 0.0 {
                        std::f32::consts::PI
                    } else {
                        0.0
                    } + side
                        * (motion.gesture.z
                            + side * motion.face.z * 0.7
                            + (time * 5.0 - side * 0.4).sin() * gait * 2.5),
                );
            }
            CreaturePart::Crest => {
                local = Vec3::new(-0.12, 0.50, -0.10);
                turn = Quat::from_rotation_z(1.3);
                scale = Vec3::new(0.8, 0.65 + motion.gesture.z * 0.5, 1.0);
            }
            CreaturePart::Body(index) => {
                let mut center = motion.chain.joint(index);
                let phase = motion.swim_phase - index as f32 * 0.5;
                center.y += phase.sin() * gait * index as f32 / SEGMENTS as f32;
                center.z -= 0.12;
                let taper = 1.0 - index as f32 / SEGMENTS as f32 * 0.78;
                *transform = Transform::from_translation(center)
                    .with_rotation(Quat::from_rotation_z(motion.chain.direction(index)))
                    .with_scale(Vec3::new(0.8, taper, taper));
                motion.body_pick_volumes.push((
                    *transform,
                    Vec3::from_array(BODY_RADII.map(|radius| (radius as f32 + 0.5) * BODY_CELL)),
                ));
                continue;
            }
            CreaturePart::Tail => {
                let center = motion.chain.joint(SEGMENTS);
                *transform =
                    Transform::from_translation(Vec3::new(center.x, center.y, center.z - 0.12))
                        .with_rotation(Quat::from_rotation_z(
                            motion.chain.direction(SEGMENTS)
                                + std::f32::consts::PI
                                + (time * 4.0).sin() * gait,
                        ))
                        .with_scale(Vec3::new(1.0, 1.8, 1.0));
                continue;
            }
        }
        *transform = Transform::from_translation(head + rotation * local)
            .with_rotation(rotation * turn)
            .with_scale(scale);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn identical_articulated_parts_share_mesh_assets() {
        use bevy::prelude::*;
        use std::collections::HashSet;

        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<crate::appearance::RenderAppearance>()
            .add_systems(Startup, super::setup_creature);
        app.update();
        let world = app.world_mut();
        let mut parts = world.query::<(&super::CreaturePart, &Mesh3d)>();
        let mut bodies = HashSet::new();
        let mut fins = HashSet::new();
        let mut body_count = 0;
        let mut fin_count = 0;
        for (part, mesh) in parts.iter(world) {
            match part {
                super::CreaturePart::Body(_) => {
                    bodies.insert(mesh.id());
                    body_count += 1;
                }
                super::CreaturePart::Fin(_)
                | super::CreaturePart::Crest
                | super::CreaturePart::Tail => {
                    fins.insert(mesh.id());
                    fin_count += 1;
                }
                _ => {}
            }
        }
        assert_eq!(body_count, super::SEGMENTS);
        assert_eq!(fin_count, 4);
        assert_eq!(bodies.len(), 1);
        assert_eq!(fins.len(), 1);
        assert_eq!(world.resource::<Assets<Mesh>>().len(), 12);
    }

    #[test]
    fn rounded_lighting_preserves_geometry_and_finite_unit_normals() {
        use crate::voxel::{SurfaceStyle, VoxelModel};
        use bevy::prelude::*;
        let make = || VoxelModel::ellipsoid([4, 5, 2], |_| [200, 160, 80]);
        let original = make().mesh_with_style(0.044, SurfaceStyle::Beveled);
        let rounded = super::rounded_mesh(make(), [4, 5, 2], 0.044, SurfaceStyle::Beveled);
        assert_eq!(
            original
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3(),
            rounded
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3(),
        );
        for normal in rounded
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap()
        {
            let normal = Vec3::from_array(*normal);
            assert!(normal.is_finite());
            assert!((normal.length() - 1.0).abs() < 1e-5);
        }
    }
    use super::*;
    use beastie_core::{ActivityRecipe, Mood};

    fn creature() -> CreatureScene {
        beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0
        .creature
    }

    #[test]
    fn body_picking_tracks_rendered_curvature_taper_and_entity_removal() {
        let plan = beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0;
        let mut app = App::new();
        app.insert_resource(SceneFrame { plan })
            .init_resource::<CreatureMotion>()
            .add_systems(Update, animate_creature);
        let entities: Vec<_> = (0..SEGMENTS)
            .map(|index| {
                app.world_mut()
                    .spawn((CreaturePart::Body(index), Transform::default()))
                    .id()
            })
            .collect();
        assert!(
            app.world()
                .resource::<CreatureMotion>()
                .body_pick_volumes()
                .is_empty()
        );
        app.update();
        let before = app
            .world()
            .resource::<CreatureMotion>()
            .body_pick_volumes()
            .to_vec();
        for tick in 1..=12 {
            let mut frame = app.world_mut().resource_mut::<SceneFrame>();
            frame.plan.simulation_remainder_ms = tick * 16;
            frame.plan.creature.velocity.x = 400;
            frame.plan.creature.steering = SteeringMode::Approach;
            app.update();
        }
        let cached = app.world().resource::<CreatureMotion>().body_pick_volumes();
        assert_eq!(cached.len(), SEGMENTS);
        assert_ne!(cached, before);
        for entity in &entities {
            let rendered = app.world().get::<Transform>(*entity).unwrap();
            assert!(cached.iter().any(|(transform, radii)| transform == rendered
                && radii.abs_diff_eq(Vec3::new(0.4225, 0.3575, 0.3575), 1e-6)));
        }
        // A missing rendered segment must not leave an invisible click target behind.
        app.world_mut().despawn(entities[0]);
        app.update();
        assert_eq!(
            app.world()
                .resource::<CreatureMotion>()
                .body_pick_volumes()
                .len(),
            SEGMENTS - 1
        );
    }

    #[test]
    fn hover_drift_preserves_body_curvature_until_authoritative_swimming() {
        let mut world = beastie_core::WorldState::new(7, "Test");
        world.creature.aquarium.steering = SteeringMode::Hover;
        world.creature.aquarium.velocity.x = -80;
        let mut plan = beastie_view::plan(&world, &beastie_view::ViewState::default()).0;
        assert_eq!(plan.creature.steering, SteeringMode::Hover);
        let mut motion = CreatureMotion::default();
        motion.advance(&plan);
        let original = motion.state.as_ref().unwrap().chain.direction(0);
        for ms in (16..=960).step_by(16) {
            plan.simulation_remainder_ms = ms;
            motion.advance(&plan);
            assert!((motion.state.as_ref().unwrap().chain.direction(0) - original).abs() < 0.0001);
        }
        // The same velocity must still steer the chain during an actual journey.
        world.creature.aquarium.steering = SteeringMode::Approach;
        plan.creature = beastie_view::plan(&world, &beastie_view::ViewState::default())
            .0
            .creature;
        assert_eq!(plan.creature.steering, SteeringMode::Approach);
        for ms in (976..=1936).step_by(16) {
            plan.elapsed_ms = ms;
            plan.simulation_remainder_ms = 0;
            motion.advance(&plan);
        }
        assert!((motion.state.as_ref().unwrap().chain.direction(0) - original).abs() > 0.3);
    }

    #[test]
    fn abrupt_tick_velocity_correction_is_continuous_and_catches_up() {
        let mut plan = beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0;
        let mut motion = CreatureMotion::default();
        let (start, _) = motion.advance(&plan);
        plan.creature.position.x += 750;
        plan.simulation_remainder_ms = 16;
        let (first, _) = motion.advance(&plan);
        assert!(first.distance(start) <= 1.9 * 0.016 + 0.0001);
        assert!(first.distance(head_position(&plan)) > 0.5);
        for ms in (32..=1280).step_by(16) {
            plan.elapsed_ms = ms;
            plan.simulation_remainder_ms = 0;
            motion.advance(&plan);
        }
        assert!(motion.position(&plan).distance(head_position(&plan)) < 0.02);
        // A backwards save clock is an explicit discontinuity, never a long swim across the tank.
        plan.elapsed_ms = 0;
        plan.simulation_remainder_ms = 0;
        plan.creature.position.x = 2000;
        motion.advance(&plan);
        assert_eq!(motion.position(&plan), head_position(&plan));
    }

    #[test]
    fn fins_do_not_jump_when_the_session_clock_crosses_an_hour() {
        let mut plan = beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0;
        plan.elapsed_ms = 3_599_000;
        plan.creature.velocity.x = 650;
        let mut app = App::new();
        app.insert_resource(SceneFrame { plan })
            .init_resource::<CreatureMotion>()
            .add_systems(Update, animate_creature);
        let fin = app
            .world_mut()
            .spawn((CreaturePart::Fin(1.0), Transform::default()))
            .id();
        app.update();
        for frame in 1..=90 {
            let previous = *app.world().get::<Transform>(fin).unwrap();
            app.world_mut().resource_mut::<SceneFrame>().plan.elapsed_ms = 3_599_000 + frame * 16;
            app.update();
            let rotation = app.world().get::<Transform>(fin).unwrap().rotation;
            if frame > 30 {
                assert!(rotation.angle_between(previous.rotation) < 0.035);
            }
        }
    }

    #[test]
    fn slow_frames_consume_the_full_motion_delta_without_resetting() {
        let mut plan = beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0;
        plan.creature.position.x = 1000;
        plan.creature.velocity.x = 1300;
        plan.creature.steering = SteeringMode::Flee;
        let mut motion = CreatureMotion::default();
        let (mut previous, _) = motion.advance(&plan);
        for frame in 1..=24 {
            let now = frame * 250;
            plan.elapsed_ms = now / 1000 * 1000;
            plan.simulation_remainder_ms = now % 1000;
            plan.creature.position.x = 1000 + (now / 1000) as i32 * 1300;
            let (head, dt) = motion.advance(&plan);
            assert_eq!(dt, 0.25);
            assert!(head.distance(previous) <= 1.9 * dt + 0.0001);
            assert!(head.distance(head_position(&plan)) < 0.05);
            previous = head;
        }
    }

    #[test]
    fn saved_stationary_velocity_and_arrival_never_predict_past_the_target() {
        let mut plan = beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0;
        plan.creature.velocity.x = 650;
        plan.simulation_remainder_ms = 900;
        let position = crate::renderer::world_position(plan.creature.position);
        for phase in [
            ActionPhase::Notice,
            ActionPhase::Inspect,
            ActionPhase::Act,
            ActionPhase::Recover,
        ] {
            plan.creature.action_phase = Some(phase);
            assert_eq!(head_position(&plan), position);
        }
        plan.creature.action_phase = None;
        plan.creature.pose = CreaturePose::Sleep;
        assert_eq!(head_position(&plan), position);
        plan.creature.pose = CreaturePose::Swim;
        let mut target = plan.creature.position;
        target.x += 50;
        plan.creature.movement_target = Some(target);
        assert_eq!(
            head_position(&plan),
            crate::renderer::world_position(target)
        );
        target.x -= 100;
        plan.creature.movement_target = Some(target);
        assert_eq!(
            head_position(&plan),
            position,
            "old velocity must not move away from new target"
        );
    }

    #[test]
    fn swim_phase_and_body_gestures_are_continuous_on_old_saves() {
        let mut plan = beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0;
        plan.elapsed_ms = 600_000;
        let mut app = App::new();
        app.insert_resource(SceneFrame { plan })
            .init_resource::<CreatureMotion>()
            .add_systems(Update, animate_creature);
        let body = app
            .world_mut()
            .spawn((CreaturePart::Body(11), Transform::default()))
            .id();
        let head = app
            .world_mut()
            .spawn((CreaturePart::Head, Transform::default()))
            .id();
        let brow = app
            .world_mut()
            .spawn((CreaturePart::Brow(1.0), Transform::default()))
            .id();
        app.update();
        for frame in 1..=180 {
            let previous_body = *app.world().get::<Transform>(body).unwrap();
            let previous_head = *app.world().get::<Transform>(head).unwrap();
            let previous_brow = *app.world().get::<Transform>(brow).unwrap();
            let previous_phase = app
                .world()
                .resource::<CreatureMotion>()
                .state
                .as_ref()
                .unwrap()
                .swim_phase;
            let mut scene = app.world_mut().resource_mut::<SceneFrame>();
            scene.plan.elapsed_ms = 600_000 + frame * 16;
            scene.plan.creature.velocity.x = if (frame / 30) % 2 == 0 { 650 } else { 0 };
            scene.plan.creature.mood = if (frame / 30) % 2 == 0 {
                Mood::Curious
            } else {
                Mood::Resentful
            };
            app.update();
            assert!(
                app.world()
                    .get::<Transform>(brow)
                    .unwrap()
                    .rotation
                    .angle_between(previous_brow.rotation)
                    < 0.19
            );
            let state = app
                .world()
                .resource::<CreatureMotion>()
                .state
                .as_ref()
                .unwrap();
            let step = (state.swim_phase - previous_phase).rem_euclid(std::f32::consts::TAU);
            assert!((0.047..=0.097).contains(&step), "phase step: {step}");
            assert!(
                app.world()
                    .get::<Transform>(body)
                    .unwrap()
                    .translation
                    .distance(previous_body.translation)
                    < 0.015
            );
            assert!(
                app.world()
                    .get::<Transform>(head)
                    .unwrap()
                    .rotation
                    .angle_between(previous_head.rotation)
                    < 0.06
            );
        }
    }

    #[test]
    fn reduced_shake_removes_recoil_oscillation_without_hiding_refusal() {
        let mut c = creature();
        c.expression = Some(beastie_view::ExpressionScene {
            owner: beastie_view::SemanticOwner::DirectOutcome,
            cue: PresentationCueKind::Recoil,
            elapsed_ms: 200,
        });
        let ordinary = acting(&c, 1.0, 0, false, false);
        let accessible = acting(&c, 1.0, 0, false, true);
        assert!(ordinary.tilt.abs() > 0.05);
        assert_eq!(accessible.tilt, 0.0);
        assert_eq!(accessible.eye_open, ordinary.eye_open);
        assert_eq!(accessible.smile, ordinary.smile);
    }

    #[test]
    fn canceled_speech_does_not_retain_open_mouth() {
        let mut c = creature();
        c.speaking = true;
        c.mouth_phase = 1;
        let speaking = acting(&c, 1.0, 0, false, false).mouth;
        c.speaking = false;
        assert!(acting(&c, 1.0, 0, false, false).mouth < speaking);
    }

    #[test]
    fn direct_outcome_replaces_private_life_acting_without_residue() {
        let mut c = creature();
        c.private_life = Some(beastie_view::PrivateLifeScene {
            id: std::num::NonZeroU64::new(1).unwrap(),
            kind: beastie_core::PrivateLifeKind::CaveSettle,
            recipe: ActivityRecipe::CaveShelter,
            phase: ActivityPhase::Act,
            elapsed_ms: 300,
            payoff_reached: false,
        });
        c.expression = Some(beastie_view::ExpressionScene {
            owner: beastie_view::SemanticOwner::PrivateLife(std::num::NonZeroU64::new(1).unwrap()),
            cue: PresentationCueKind::CaveShelter,
            elapsed_ms: 300,
        });
        assert_eq!(acting(&c, 1.0, 0, true, false).eye_open, 0.6);
        c.expression = Some(beastie_view::ExpressionScene {
            owner: beastie_view::SemanticOwner::DirectOutcome,
            cue: PresentationCueKind::Notice,
            elapsed_ms: 0,
        });
        assert!(acting(&c, 1.0, 0, true, false).eye_open > 1.0);
        c.expression = None;
        c.private_life = None;
        c.mood = Mood::Content;
        assert_eq!(acting(&c, 1.0, 0, true, false).eye_open, 0.9);
    }

    #[test]
    fn sleep_and_curiosity_have_distinct_readable_eyes() {
        let mut c = creature();
        c.mood = Mood::Curious;
        c.pose = CreaturePose::Hover;
        assert!(acting(&c, 1.0, 0, false, false).eye_open > 1.0);
        c.pose = CreaturePose::Sleep;
        assert!(acting(&c, 1.0, 0, false, false).eye_open < 0.1);
    }
    #[test]
    fn owned_reactions_have_distinct_face_and_fin_silhouettes() {
        let mut c = creature();
        let mut sample = |cue| {
            c.expression = Some(beastie_view::ExpressionScene {
                owner: beastie_view::SemanticOwner::DirectOutcome,
                cue,
                elapsed_ms: 500,
            });
            acting(&c, 1.0, 0, false, false)
        };
        let affection = sample(PresentationCueKind::Affection);
        let curious = sample(PresentationCueKind::Notice);
        let refusal = sample(PresentationCueKind::Recoil);
        assert!(affection.smile > 0.08 && affection.fin > 0.1);
        assert!(curious.eye_open > affection.eye_open + 0.3);
        assert!(curious.asymmetry > 0.0);
        assert!(refusal.smile < 0.0 && refusal.fin < -0.2);
        assert!(refusal.mouth_width < affection.mouth_width);
    }

    #[test]
    fn anticipation_does_not_imply_consumption_and_clears_at_recovery() {
        let mut c = creature();
        c.mood = Mood::Content;
        c.expression = None;
        c.action_phase = Some(ActionPhase::Inspect);
        let prepared = acting(&c, 1.0, 0, false, false);
        assert!(prepared.eye_open > 1.0 && prepared.fin < 0.0);
        assert_eq!(prepared.mouth, art::mood(Mood::Content).mouth);
        c.action_phase = Some(ActionPhase::Recover);
        assert_eq!(acting(&c, 1.0, 0, false, false), art::mood(Mood::Content));
    }

    #[test]
    fn reduced_motion_keeps_owned_face_but_reduces_supporting_gesture() {
        let mut c = creature();
        c.expression = Some(beastie_view::ExpressionScene {
            owner: beastie_view::SemanticOwner::DirectOutcome,
            cue: PresentationCueKind::Affection,
            elapsed_ms: 500,
        });
        let normal = acting(&c, 1.0, 0, false, false);
        let reduced = acting(&c, 1.0, 0, true, false);
        assert_eq!(normal.eye_open, reduced.eye_open);
        assert_eq!(normal.smile, reduced.smile);
        assert!(reduced.fin.abs() < normal.fin.abs() * 0.5);
        assert!(reduced.tilt.abs() < normal.tilt.abs() * 0.5);
    }

    #[test]
    fn speech_changes_width_and_aperture_then_cancels_without_residue() {
        let mut c = creature();
        c.mood = Mood::Content;
        c.speaking = true;
        c.mouth_phase = 1;
        let open = acting(&c, 1.0, 0, false, false);
        c.mouth_phase = 2;
        let round = acting(&c, 1.0, 0, false, false);
        assert!(round.mouth_width < open.mouth_width && round.mouth < open.mouth);
        c.speaking = false;
        assert_eq!(acting(&c, 1.0, 0, false, false), art::mood(Mood::Content));
    }
}
