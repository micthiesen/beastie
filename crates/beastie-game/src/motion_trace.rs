//! Capture-only measurements of the transforms actually submitted for rendering.
//! Limits flag review candidates, not correctness failures: head 2 world units/s,
//! articulated parts 8 world units/s and
//! 12 radians/s. Clock resets and gaps over 250 ms retain deltas but omit speeds.
use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Serialize;

use crate::creature::CreaturePart;

const HEAD_SPEED_ALERT: f32 = 2.0;
const TRANSLATION_SPEED_ALERT: f32 = 8.0;
const ROTATION_SPEED_ALERT: f32 = 12.0;
const MAX_CONTINUOUS_MS: u64 = 250;

#[derive(Serialize)]
pub(crate) struct MotionTrace {
    presentation_ms: u64,
    frame_delta_ms: Option<u64>,
    clock_discontinuity: bool,
    head_position: Option<[f32; 3]>,
    head_translation: PartDelta,
    part_count: usize,
    max_translation: PartDelta,
    max_rotation: PartDelta,
    alerts: Vec<&'static str>,
}

#[derive(Default, Serialize)]
struct PartDelta {
    part: Option<String>,
    delta: f32,
    speed_per_second: Option<f32>,
}

#[derive(Default)]
pub(crate) struct MotionTracker {
    previous_ms: Option<u64>,
    previous: BTreeMap<String, Transform>,
}

impl MotionTracker {
    pub fn sample<'a>(
        &mut self,
        now: u64,
        parts: impl Iterator<Item = (&'a CreaturePart, &'a Transform)>,
    ) -> MotionTrace {
        let elapsed = self.previous_ms.and_then(|last| now.checked_sub(last));
        let discontinuity =
            self.previous_ms.is_some() && elapsed.is_none_or(|dt| dt > MAX_CONTINUOUS_MS);
        let seconds = elapsed
            .filter(|dt| *dt > 0 && !discontinuity)
            .map(|dt| dt as f32 / 1000.0);
        let mut trace = MotionTrace {
            presentation_ms: now,
            frame_delta_ms: elapsed,
            clock_discontinuity: discontinuity,
            head_position: None,
            head_translation: PartDelta::default(),
            part_count: 0,
            max_translation: PartDelta::default(),
            max_rotation: PartDelta::default(),
            alerts: Vec::new(),
        };
        let mut next = BTreeMap::new();
        for (part, transform) in parts {
            let name = part_name(part);
            trace.part_count += 1;
            if !transform.translation.is_finite() || !transform.rotation.is_finite() {
                if !trace.alerts.contains(&"nonfinite_transform") {
                    trace.alerts.push("nonfinite_transform");
                }
                continue;
            }
            if matches!(part, CreaturePart::Head) {
                trace.head_position = Some(transform.translation.to_array());
            }
            if let Some(previous) = self.previous.get(&name) {
                if matches!(part, CreaturePart::Head) {
                    update_max(
                        &mut trace.head_translation,
                        &name,
                        transform.translation.distance(previous.translation),
                        seconds,
                    );
                }
                update_max(
                    &mut trace.max_translation,
                    &name,
                    transform.translation.distance(previous.translation),
                    seconds,
                );
                update_max(
                    &mut trace.max_rotation,
                    &name,
                    rotation_delta(previous.rotation, transform.rotation),
                    seconds,
                );
            }
            next.insert(name, *transform);
        }
        if trace
            .head_translation
            .speed_per_second
            .is_some_and(|speed| speed > HEAD_SPEED_ALERT)
        {
            trace.alerts.push("head_speed");
        }
        if trace
            .max_translation
            .speed_per_second
            .is_some_and(|speed| speed > TRANSLATION_SPEED_ALERT)
        {
            trace.alerts.push("translation_speed");
        }
        if trace
            .max_rotation
            .speed_per_second
            .is_some_and(|speed| speed > ROTATION_SPEED_ALERT)
        {
            trace.alerts.push("rotation_speed");
        }
        if elapsed == Some(0)
            && (trace.max_translation.delta > 0.00001 || trace.max_rotation.delta > 0.00001)
        {
            trace.alerts.push("movement_without_time");
        }
        self.previous_ms = Some(now);
        self.previous = next;
        trace
    }
}

// acos(dot) loses precision near identity and turns normal quaternion roundoff
// into apparent motion. The relative quaternion retains its small vector component.
fn rotation_delta(previous: Quat, current: Quat) -> f32 {
    if previous == current || previous == -current {
        return 0.0;
    }
    let relative = previous.conjugate() * current;
    2.0 * Vec3::new(relative.x, relative.y, relative.z)
        .length()
        .atan2(relative.w.abs())
}

fn update_max(metric: &mut PartDelta, name: &str, delta: f32, seconds: Option<f32>) {
    if delta.is_finite() && (metric.part.is_none() || delta > metric.delta) {
        metric.part = Some(name.to_owned());
        metric.delta = delta;
        metric.speed_per_second = seconds
            .map(|dt| delta / dt)
            .filter(|speed| speed.is_finite());
    }
}

fn part_name(part: &CreaturePart) -> String {
    match part {
        CreaturePart::Head => "head".into(),
        CreaturePart::Eye(side) => format!("eye_{side}"),
        CreaturePart::Pupil(side) => format!("pupil_{side}"),
        CreaturePart::Glint(side) => format!("glint_{side}"),
        CreaturePart::Brow(side) => format!("brow_{side}"),
        CreaturePart::Mouth => "mouth".into(),
        CreaturePart::LowerLip => "lower_lip".into(),
        CreaturePart::Tongue => "tongue".into(),
        CreaturePart::MouthCorner(side) => format!("mouth_corner_{side}"),
        CreaturePart::Cheek(side) => format!("cheek_{side}"),
        CreaturePart::Fin(side) => format!("fin_{side}"),
        CreaturePart::Crest => "crest".into(),
        CreaturePart::Body(index) => format!("body_{index}"),
        CreaturePart::Tail => "tail".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(tracker: &mut MotionTracker, ms: u64, x: f32, angle: f32) -> MotionTrace {
        let part = CreaturePart::Head;
        let transform =
            Transform::from_xyz(x, 0.0, 0.0).with_rotation(Quat::from_rotation_z(angle));
        tracker.sample(ms, std::iter::once((&part, &transform)))
    }

    #[test]
    fn measures_rendered_displacement_and_shortest_rotation() {
        let mut tracker = MotionTracker::default();
        assert!(
            head(&mut tracker, 0, 0.0, 0.0)
                .max_translation
                .part
                .is_none()
        );
        let trace = head(&mut tracker, 100, 1.0, 0.5);
        assert_eq!(trace.max_translation.delta, 1.0);
        assert_eq!(trace.max_translation.speed_per_second, Some(10.0));
        assert!((trace.max_rotation.speed_per_second.unwrap() - 5.0).abs() < 0.001);
        assert_eq!(trace.max_translation.part.as_deref(), Some("head"));
        assert_eq!(trace.alerts, vec!["head_speed", "translation_speed"]);
        assert_eq!(trace.head_translation.speed_per_second, Some(10.0));
    }

    #[test]
    fn zero_time_detects_movement_without_dividing_by_zero() {
        let mut tracker = MotionTracker::default();
        head(&mut tracker, 100, 0.0, 0.0);
        let trace = head(&mut tracker, 100, 1.0, 0.0);
        assert_eq!(trace.max_translation.speed_per_second, None);
        assert_eq!(trace.alerts, vec!["movement_without_time"]);
        assert!(!trace.clock_discontinuity);
    }

    #[test]
    fn repeated_nonidentity_rotation_and_sign_equivalence_are_motionless() {
        let part = CreaturePart::MouthCorner(-1.0);
        let rotation = Quat::from_euler(EulerRot::YXZ, 0.273, -0.115, 0.034);
        let mut transform = Transform::from_rotation(rotation);
        let mut tracker = MotionTracker::default();
        tracker.sample(100, std::iter::once((&part, &transform)));
        for quaternion in [rotation, -rotation, rotation] {
            transform.rotation = quaternion;
            let trace = tracker.sample(100, std::iter::once((&part, &transform)));
            assert_eq!(trace.max_rotation.delta, 0.0);
            assert!(trace.alerts.is_empty());
        }
    }

    #[test]
    fn small_rotation_is_measured_without_acos_roundoff() {
        let start = Quat::from_euler(EulerRot::YXZ, 0.273, -0.115, 0.034);
        let end = start * Quat::from_rotation_z(0.0001);
        assert!((rotation_delta(start, end) - 0.0001).abs() < 0.000001);
        assert!((rotation_delta(start, -end) - 0.0001).abs() < 0.000001);
    }

    #[test]
    fn invalid_transforms_are_flagged_without_poisoning_json_or_next_frame() {
        let mut tracker = MotionTracker::default();
        let trace = head(&mut tracker, 0, f32::NAN, 0.0);
        assert_eq!(trace.head_position, None);
        assert_eq!(trace.alerts, vec!["nonfinite_transform"]);
        serde_json::to_string(&trace).unwrap();
        let trace = head(&mut tracker, 100, 0.0, 0.0);
        assert_eq!(trace.max_translation.part, None);
        assert!(trace.alerts.is_empty());
    }

    #[test]
    fn reset_and_long_gaps_do_not_report_meaningless_speeds() {
        let mut tracker = MotionTracker::default();
        head(&mut tracker, 100, 0.0, 0.0);
        for now in [0, 10_000] {
            let trace = head(&mut tracker, now, now as f32, 0.0);
            assert!(trace.clock_discontinuity);
            assert_eq!(trace.max_translation.speed_per_second, None);
            assert!(trace.alerts.is_empty());
        }
        let trace = head(&mut tracker, 10_100, 10_000.0, 0.0);
        assert!(!trace.clock_discontinuity);
        assert_eq!(trace.max_translation.speed_per_second, Some(0.0));
    }
}
