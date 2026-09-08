//! Presentation-only articulated motion. Spacing depends on distance traveled, not frame count.

use std::collections::VecDeque;

use bevy::prelude::*;

const SAMPLE_SPACING: f32 = 0.035;
const MAX_TRAIL_LENGTH: f32 = 5.0;

pub struct BodyTrail {
    points: VecDeque<Vec3>,
    head: Vec3,
}

impl BodyTrail {
    pub fn new(head: Vec3, forward: Vec3) -> Self {
        let mut points = VecDeque::new();
        let forward = forward.try_normalize().unwrap_or(Vec3::X);
        for i in 0..=150 {
            points.push_back(head - forward * (i as f32 * SAMPLE_SPACING));
        }
        Self { points, head }
    }

    pub fn advance(&mut self, head: Vec3) {
        // Save restoration and scenario jumps are discontinuities, not five-meter swim turns.
        if head.distance(self.head) > 2.5 {
            *self = Self::new(head, head - self.head);
            return;
        }
        self.head = head;
        let newest = *self.points.front().unwrap_or(&head);
        let distance = head.distance(newest);
        if distance >= SAMPLE_SPACING {
            let count = (distance / SAMPLE_SPACING).floor() as usize;
            for i in 1..=count {
                self.points
                    .push_front(newest.lerp(head, i as f32 / count as f32));
            }
        }
        let mut length = self.head.distance(*self.points.front().unwrap_or(&head));
        let mut retain = self.points.len();
        for i in 1..self.points.len() {
            length += self.points[i - 1].distance(self.points[i]);
            if length > MAX_TRAIL_LENGTH {
                retain = i + 1;
                break;
            }
        }
        self.points.truncate(retain);
    }

    pub fn sample(&self, distance: f32) -> Vec3 {
        let mut remaining = distance.max(0.0);
        let mut previous = self.head;
        for &point in &self.points {
            let segment = previous.distance(point);
            if segment > f32::EPSILON && remaining <= segment {
                return previous.lerp(point, remaining / segment);
            }
            remaining -= segment;
            previous = point;
        }
        previous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_spacing_is_independent_of_head_speed_and_stopping() {
        let mut trail = BodyTrail::new(Vec3::ZERO, Vec3::X);
        for step in [0.01, 0.04, 0.12, 0.3, 0.6, 0.6] {
            trail.advance(Vec3::X * step);
            for distance in [0.2, 0.4, 0.8, 1.2] {
                assert!((trail.sample(distance).x - (step - distance)).abs() < 0.0001);
            }
        }
    }

    #[test]
    fn chain_follows_a_corner_instead_of_cutting_across_it() {
        let mut trail = BodyTrail::new(Vec3::ZERO, Vec3::X);
        trail.advance(Vec3::X);
        trail.advance(Vec3::new(1.0, 1.0, 0.0));
        assert!(trail.sample(0.5).distance(Vec3::new(1.0, 0.5, 0.0)) < 0.001);
        assert!(trail.sample(1.5).distance(Vec3::new(0.5, 0.0, 0.0)) < 0.001);
    }
}

/// A distance-constrained chain guided by the traveled path. Turning cannot collapse its length.
pub struct BodyChain {
    joints: Vec<Vec3>,
    headings: Vec<f32>,
    heading: f32,
}

impl BodyChain {
    pub fn new(head: Vec3, forward: Vec3, count: usize) -> Self {
        let behind = -forward.try_normalize().unwrap_or(Vec3::X);
        let heading = behind.y.atan2(behind.x);
        Self {
            joints: (0..=count)
                .map(|index| head + behind * joint_distance(index))
                .collect(),
            headings: vec![heading; count + 1],
            heading,
        }
    }

    pub fn advance(&mut self, head: Vec3, forward: Vec3, trail: &BodyTrail, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let travelling = forward.length_squared() > 0.0025;
        if travelling {
            let behind = -forward.normalize();
            self.heading = turn_towards(self.heading, behind.y.atan2(behind.x), dt * 3.5);
        }
        let mut parent = head;
        let mut parent_heading = self.heading;
        for index in 0..self.joints.len() {
            if travelling {
                // Tangent guidance is stable as earlier joints turn. Point-to-parent guidance
                // fed those turns back into itself and made a stopped animal's tail orbit its head.
                let distance = joint_distance(index);
                let guide =
                    trail.sample(distance + 0.08) - trail.sample((distance - 0.08).max(0.0));
                let desired = if guide.length_squared() > 0.0025 {
                    guide.y.atan2(guide.x)
                } else {
                    self.headings[index]
                };
                let constrained =
                    parent_heading + angle_difference(desired, parent_heading).clamp(-0.26, 0.26);
                let heading = turn_towards(self.headings[index], constrained, dt * 5.0);
                self.headings[index] =
                    parent_heading + angle_difference(heading, parent_heading).clamp(-0.3, 0.3);
            }
            // Settled hover preserves the achieved curvature. Small buoyancy changes translate
            // that pose; they do not restart a turn or erase the body's length.
            let length = if index == 0 { 0.35 } else { 0.145 };
            self.headings[index] = bounded_heading(parent, self.headings[index], length);
            self.joints[index] = parent
                + Vec3::new(self.headings[index].cos(), self.headings[index].sin(), 0.0) * length;
            parent = self.joints[index];
            parent_heading = self.headings[index];
        }
    }

    pub fn joint(&self, index: usize) -> Vec3 {
        self.joints[index.min(self.joints.len() - 1)]
    }

    pub fn direction(&self, index: usize) -> f32 {
        self.headings[index.min(self.headings.len() - 1)] + std::f32::consts::PI
    }
}

fn bounded_heading(parent: Vec3, desired: f32, length: f32) -> f32 {
    let inside = |heading: f32| {
        let point = parent + Vec3::new(heading.cos(), heading.sin(), 0.0) * length;
        (-7.2..=7.2).contains(&point.x) && (-1.45..=3.65).contains(&point.y)
    };
    if inside(desired) {
        return desired;
    }
    // Find the nearest in-tank direction instead of clamping positions and shortening joints.
    for step in 1..=128 {
        let offset = step as f32 * std::f32::consts::PI / 128.0;
        for sign in [-1.0, 1.0] {
            let candidate = desired + offset * sign;
            if inside(candidate) {
                return candidate;
            }
        }
    }
    desired
}

fn joint_distance(index: usize) -> f32 {
    0.35 + index as f32 * 0.145
}

fn angle_difference(target: f32, current: f32) -> f32 {
    (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

fn turn_towards(current: f32, target: f32, maximum: f32) -> f32 {
    current + angle_difference(target, current).clamp(-maximum, maximum)
}

#[cfg(test)]
mod chain_tests {
    use super::*;

    #[test]
    fn stopped_and_near_stationary_body_holds_its_curvature() {
        let mut trail = BodyTrail::new(Vec3::ZERO, Vec3::X);
        let mut chain = BodyChain::new(Vec3::ZERO, Vec3::X, 12);
        for frame in 0..90 {
            let head = Vec3::new(frame as f32 / 90.0, (frame as f32 / 30.0).sin() * 0.3, 0.0);
            trail.advance(head);
            chain.advance(head, Vec3::new(1.0, 0.3, 0.0), &trail, 1.0 / 60.0);
        }
        let head = Vec3::new(1.0, 0.0, 0.0);
        trail.advance(head);
        chain.advance(head, Vec3::ZERO, &trail, 1.0 / 60.0);
        let held = chain.joints.clone();
        for frame in 0..600 {
            // The inspect trace retains a 35-unit normalized drift, well below travel speed.
            let forward = if frame % 2 == 0 {
                Vec3::ZERO
            } else {
                Vec3::Y * 0.016
            };
            chain.advance(head, forward, &trail, 1.0 / 60.0);
            for (joint, expected) in chain.joints.iter().zip(&held) {
                assert!(joint.distance(*expected) < 0.0001);
            }
        }
    }

    #[test]
    fn boundary_turns_keep_joints_in_tank_without_shortening_them() {
        for head in [Vec3::new(-6.6, 3.4, 0.0), Vec3::new(6.6, -1.15, 0.0)] {
            let mut trail = BodyTrail::new(head, Vec3::NEG_Y);
            let mut chain = BodyChain::new(head, Vec3::NEG_Y, 12);
            trail.advance(head);
            chain.advance(head, Vec3::NEG_Y, &trail, 1.0 / 60.0);
            let mut parent = head;
            for index in 0..=12 {
                let joint = chain.joint(index);
                assert!((-7.2..=7.2).contains(&joint.x) && (-1.45..=3.65).contains(&joint.y));
                assert!(
                    (parent.distance(joint) - if index == 0 { 0.35 } else { 0.145 }).abs() < 0.0001
                );
                parent = joint;
            }
        }
    }

    #[test]
    fn reversal_preserves_chain_spacing_instead_of_folding_into_head() {
        let mut trail = BodyTrail::new(Vec3::ZERO, Vec3::X);
        let mut chain = BodyChain::new(Vec3::ZERO, Vec3::X, 12);
        for frame in 0..240 {
            let direction = if frame < 120 { Vec3::X } else { Vec3::NEG_X };
            let x = if frame < 120 {
                frame as f32 / 120.0
            } else {
                (240 - frame) as f32 / 120.0
            };
            let head = Vec3::X * x;
            trail.advance(head);
            chain.advance(head, direction, &trail, 1.0 / 60.0);
            let mut parent = head;
            for index in 0..=12 {
                let expected = if index == 0 { 0.35 } else { 0.145 };
                assert!((parent.distance(chain.joint(index)) - expected).abs() < 0.0001);
                parent = chain.joint(index);
            }
            assert!(head.distance(chain.joint(12)) > 0.9);
        }
    }
}
