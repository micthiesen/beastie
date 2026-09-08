//! An articulated, solid voxel animal. Semantics come exclusively from the scene plan.
use beastie_core::{ActivityPhase, ActivityRecipe, Facing, GazeTarget, Mood, SteeringMode};
use beastie_view::{CreaturePose, CreatureScene, PresentationCueKind, ScenePlan};
use bevy::prelude::*;

use crate::{
    body::{BodyChain, BodyTrail},
    renderer::SceneFrame,
    voxel::VoxelModel,
};

const SEGMENTS: usize = 12;

#[derive(Component)]
pub(crate) enum CreaturePart {
    Head,
    Eye(f32),
    Pupil(f32),
    Glint(f32),
    Brow(f32),
    Mouth,
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
            });
        }
        let state = self.state.as_mut().expect("motion initialized above");
        let dt = now.saturating_sub(state.last_ms).min(100) as f32 / 1000.0;
        state.last_ms = now;
        // Bound correction speed at tick-boundary velocity changes, with ~100ms catch-up.
        let correction = (target - self.presented_head) * (1.0 - (-dt * 50.0).exp());
        self.presented_head += correction.clamp_length_max(dt * 12.0);
        state.trail.advance(self.presented_head);
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
        state
            .chain
            .advance(self.presented_head, forward, &state.trail, dt);
        (self.presented_head, dt)
    }
}

fn golden(point: [i32; 3]) -> [u8; 3] {
    if point[1] < -3 && point[2] > 0 {
        [250, 206, 112]
    } else if point[1] > 5 {
        [255, 194, 70]
    } else if (point[0] + point[1] * 3 + point[2] * 7).rem_euclid(19) == 0 {
        [235, 151, 46]
    } else {
        [245, 174, 58]
    }
}

fn solid(radii: [i32; 3], cell: f32, color: [u8; 3]) -> Mesh {
    VoxelModel::ellipsoid(radii, |_| color).mesh(cell)
}

fn fin_mesh() -> Mesh {
    let mut fin = VoxelModel::default();
    for x in 0_i32..9 {
        for y in -x / 2..=x / 2 {
            let color = if x > 6 {
                [246, 190, 87]
            } else {
                [210, 127, 49]
            };
            fin.set([x, y, 0], color);
        }
    }
    fin.mesh(0.045)
}

pub(crate) fn setup_creature(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.init_resource::<CreatureMotion>();
    let material = materials.add(StandardMaterial {
        perceptual_roughness: 0.82,
        ..default()
    });
    let face_material = materials.add(StandardMaterial {
        unlit: true,
        ..default()
    });
    let mut spawn = |part: CreaturePart, mesh: Mesh, facial: bool| {
        commands.spawn((
            part,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(if facial {
                face_material.clone()
            } else {
                material.clone()
            }),
            Transform::default(),
        ));
    };
    spawn(
        CreaturePart::Head,
        VoxelModel::ellipsoid([12, 10, 9], golden).mesh(0.055),
        false,
    );
    for side in [-1.0, 1.0] {
        spawn(
            CreaturePart::Eye(side),
            solid([4, 5, 2], 0.044, [255, 242, 201]),
            true,
        );
        spawn(
            CreaturePart::Pupil(side),
            solid([2, 3, 1], 0.036, [30, 40, 44]),
            true,
        );
        spawn(
            CreaturePart::Glint(side),
            solid([0, 0, 0], 0.039, [255, 253, 230]),
            true,
        );
        spawn(
            CreaturePart::Brow(side),
            solid([3, 0, 1], 0.045, [121, 73, 41]),
            false,
        );
        spawn(
            CreaturePart::MouthCorner(side),
            solid([0, 1, 0], 0.038, [100, 56, 40]),
            true,
        );
        spawn(
            CreaturePart::Cheek(side),
            solid([2, 1, 0], 0.039, [234, 128, 65]),
            false,
        );
        spawn(CreaturePart::Fin(side), fin_mesh(), false);
    }
    spawn(
        CreaturePart::Mouth,
        solid([3, 1, 1], 0.036, [85, 49, 40]),
        true,
    );
    spawn(CreaturePart::Crest, fin_mesh(), false);
    for index in 0..SEGMENTS {
        spawn(
            CreaturePart::Body(index),
            VoxelModel::ellipsoid([6, 5, 5], golden).mesh(0.065),
            false,
        );
    }
    spawn(CreaturePart::Tail, fin_mesh(), false);
}

/// Continuous projection used by rendering and head picking. It never changes simulation state.
pub(crate) fn head_position(plan: &ScenePlan) -> Vec3 {
    let c = &plan.creature;
    let fraction = plan.simulation_remainder_ms.min(999) as f32 / 1_000.0;
    let future = beastie_core::NormalizedPosition::new(
        c.position
            .x
            .saturating_add((c.velocity.x as f32 * fraction).round() as i32),
        c.position
            .y
            .saturating_add((c.velocity.y as f32 * fraction).round() as i32),
    );
    crate::renderer::world_position(future)
}

#[derive(Clone, Copy, Debug)]
struct Acting {
    eye_open: f32,
    brow: f32,
    smile: f32,
    mouth: f32,
    tilt: f32,
    nod: f32,
    fin: f32,
}

fn acting(
    creature: &CreatureScene,
    time: f32,
    remainder_ms: u64,
    reduced_motion: bool,
    reduced_shake: bool,
) -> Acting {
    let mut a = match creature.mood {
        Mood::Content => Acting {
            eye_open: 0.9,
            brow: 0.0,
            smile: 0.055,
            mouth: 0.1,
            tilt: 0.0,
            nod: 0.0,
            fin: 0.0,
        },
        Mood::Curious => Acting {
            eye_open: 1.12,
            brow: 0.18,
            smile: 0.02,
            mouth: 0.25,
            tilt: 0.10,
            nod: 0.04,
            fin: 0.12,
        },
        Mood::Hungry => Acting {
            eye_open: 0.93,
            brow: 0.13,
            smile: -0.02,
            mouth: 0.38,
            tilt: -0.03,
            nod: 0.0,
            fin: 0.05,
        },
        Mood::Sleepy => Acting {
            eye_open: 0.34,
            brow: -0.05,
            smile: 0.02,
            mouth: 0.1,
            tilt: -0.07,
            nod: -0.07,
            fin: -0.1,
        },
        Mood::Lonely => Acting {
            eye_open: 0.68,
            brow: 0.3,
            smile: -0.055,
            mouth: 0.08,
            tilt: 0.06,
            nod: -0.05,
            fin: -0.12,
        },
        Mood::Resentful => Acting {
            eye_open: 0.52,
            brow: -0.28,
            smile: -0.025,
            mouth: 0.05,
            tilt: -0.07,
            nod: -0.01,
            fin: -0.1,
        },
    };
    if creature.pose == CreaturePose::Sleep {
        a.eye_open = 0.055;
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
        match expression.cue {
            PresentationCueKind::Recoil | PresentationCueKind::Spit => {
                a.eye_open = 0.48;
                a.brow = -0.3;
                a.mouth = 0.6;
                a.smile = -0.07;
                a.tilt = if reduced_shake { 0.0 } else { beat * 0.12 };
                a.nod = -0.15;
            }
            PresentationCueKind::Suspicion | PresentationCueKind::FoodSuspicion => {
                a.eye_open = 0.5;
                a.brow = -0.24;
                a.tilt = 0.15;
                a.smile = -0.03;
            }
            PresentationCueKind::Delight => {
                a.eye_open = 0.82;
                a.smile = 0.1;
                a.mouth = 0.5;
                a.fin = 0.3;
                a.nod = 0.12;
            }
            PresentationCueKind::Affection | PresentationCueKind::Comfort => {
                a.eye_open = 0.65;
                a.smile = 0.09;
                a.tilt = 0.12;
                a.nod = 0.06;
                a.fin = 0.08;
            }
            PresentationCueKind::Notice
            | PresentationCueKind::PositiveNotice
            | PresentationCueKind::PlaceNotice => {
                a.eye_open = 1.12;
                a.brow = 0.2;
                a.nod = 0.09;
            }
            PresentationCueKind::Sleep => {
                a.eye_open = 0.055;
                a.nod = -0.1;
            }
            PresentationCueKind::Crumbs => {
                a.mouth = 0.3 + beat.abs() * 0.45;
                a.smile = 0.055;
            }
            _ => {}
        }
    }
    if let Some(activity) = &creature.private_life {
        let owns_expression = creature.expression.as_ref().is_none_or(|expression| {
            matches!(expression.owner, beastie_view::SemanticOwner::PrivateLife(id) if id == activity.id)
        });
        if owns_expression && activity.phase == ActivityPhase::Act {
            let beat = (activity.elapsed_ms.saturating_add(remainder_ms) as f32 * 0.003).sin();
            match activity.recipe {
                ActivityRecipe::BallNudge => {
                    a.nod = beat.abs() * 0.15;
                    a.fin = 0.2;
                }
                ActivityRecipe::BellStrike => {
                    a.tilt = beat * 0.15;
                    a.eye_open = 1.1;
                }
                ActivityRecipe::SockTug => {
                    a.nod = -beat.abs() * 0.16;
                    a.mouth = 0.25;
                }
                ActivityRecipe::CaveShelter => {
                    a.eye_open = 0.6;
                    a.nod = -0.07;
                    a.fin = -0.2;
                }
                ActivityRecipe::PlantOrbit => {
                    a.tilt = beat * 0.14;
                    a.brow = 0.2;
                    a.eye_open = 1.05;
                }
                ActivityRecipe::BottomForage => {
                    a.nod = -0.18;
                    a.mouth = 0.1 + beat.abs() * 0.2;
                }
                ActivityRecipe::OpenWaterDrift => {
                    a.tilt = beat * 0.04;
                    a.eye_open = 0.8;
                }
            }
        }
    }
    // The exact current speech owner drives these phases, so cancellation closes the mouth now.
    if creature.speaking {
        a.mouth = [0.18, 0.75, 0.45][usize::from(creature.mouth_phase.min(2))];
    }
    let blink = time.rem_euclid(5.7);
    if !reduced_motion && blink < 0.15 && creature.pose != CreaturePose::Sleep {
        a.eye_open *= (blink / 0.075 - 1.0).abs().max(0.055);
    }
    if reduced_motion {
        a.tilt *= 0.3;
        a.nod *= 0.3;
        a.fin *= 0.3;
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
    let time = (now % 3_600_000) as f32 / 1_000.0;
    let (head, dt) = motion.advance(plan);
    let facing = if c.facing == Facing::Left { -1.0 } else { 1.0 };
    let motion = motion.state.as_mut().expect("motion initialized above");
    let target_speed = Vec2::new(c.velocity.x as f32, c.velocity.y as f32).length() / 750.0;
    motion.speed += (target_speed.min(1.0) - motion.speed) * (1.0 - (-dt * 6.0).exp());
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
        time,
        plan.simulation_remainder_ms,
        plan.reduced_motion,
        plan.reduced_shake,
    );
    let rotation = Quat::from_euler(EulerRot::YXZ, motion.yaw, -a.nod, a.tilt);
    let gait = if plan.reduced_motion || c.pose == CreaturePose::Sleep {
        0.0
    } else {
        0.015 + motion.speed * 0.075
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
    for (part, mut transform) in &mut parts {
        let mut local = Vec3::ZERO;
        let mut scale = Vec3::ONE;
        let mut turn = Quat::IDENTITY;
        match *part {
            CreaturePart::Head => {}
            CreaturePart::Eye(side) => {
                local = Vec3::new(side * 0.27, 0.10, 0.455);
                scale.y = a.eye_open;
            }
            CreaturePart::Pupil(side) => {
                local = Vec3::new(side * 0.27 + gaze_x, 0.095 + gaze_y * a.eye_open, 0.555);
                scale.y = a.eye_open.min(1.0);
            }
            CreaturePart::Glint(side) => {
                local = Vec3::new(
                    side * 0.27 + gaze_x - 0.02,
                    0.095 + (0.045 + gaze_y) * a.eye_open.min(1.0),
                    0.611,
                );
                scale = Vec3::splat(a.eye_open.min(1.0));
            }
            CreaturePart::Brow(side) => {
                local = Vec3::new(side * 0.27, 0.36, 0.46);
                turn = Quat::from_rotation_z(side * a.brow);
            }
            CreaturePart::Mouth => {
                local = Vec3::new(0.0, -0.24, 0.49);
                scale.y = 0.3 + a.mouth * 2.4;
                scale.x = 1.0 - a.mouth * 0.18;
            }
            CreaturePart::MouthCorner(side) => {
                local = Vec3::new(side * 0.12, -0.24 + a.smile, 0.493);
                turn = Quat::from_rotation_z(-side * a.smile * 5.0);
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
                    } + side * (a.fin + (time * 5.0).sin() * gait * 2.5),
                );
            }
            CreaturePart::Crest => {
                local = Vec3::new(-0.12, 0.50, -0.10);
                turn = Quat::from_rotation_z(1.3);
                scale = Vec3::new(0.8, 0.65, 1.0);
            }
            CreaturePart::Body(index) => {
                let mut center = motion.chain.joint(index);
                let phase = time * (3.0 + motion.speed * 3.0) - index as f32 * 0.5;
                center.y += phase.sin() * gait * index as f32 / SEGMENTS as f32;
                center.z -= 0.12;
                let taper = 1.0 - index as f32 / SEGMENTS as f32 * 0.78;
                *transform = Transform::from_translation(center)
                    .with_rotation(Quat::from_rotation_z(motion.chain.direction(index)))
                    .with_scale(Vec3::new(0.8, taper, taper));
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
    use super::*;

    fn creature() -> CreatureScene {
        beastie_view::plan(
            &beastie_core::WorldState::new(7, "Test"),
            &beastie_view::ViewState::default(),
        )
        .0
        .creature
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
        assert!(first.distance(start) <= 12.0 * 0.016 + 0.0001);
        assert!(first.distance(head_position(&plan)) > 0.5);
        for ms in (32..=128).step_by(16) {
            plan.simulation_remainder_ms = ms;
            motion.advance(&plan);
        }
        assert!(motion.position(&plan).distance(head_position(&plan)) < 0.02);
        // A backwards save clock is an explicit discontinuity, never a long swim across the tank.
        plan.simulation_remainder_ms = 0;
        plan.creature.position.x = 2000;
        motion.advance(&plan);
        assert_eq!(motion.position(&plan), head_position(&plan));
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
}
