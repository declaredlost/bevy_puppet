//! Clips driven by ground covered: a walk that finishes its step.

use bevy::ecs::component::Component;
use bevy::prelude::{ReflectComponent, ReflectDefault};
use bevy::reflect::Reflect;

use crate::clip::Clip;

/// How far a puppet has walked, in cycles of its gait's clip.
///
/// What a [`Driver::Gait`](crate::clip::Driver::Gait) clip plays by. The game
/// advances it each frame by how fast the puppet moves.
#[derive(Component, Reflect, Debug, Default, Clone, PartialEq)]
#[reflect(Component, Debug, Default, PartialEq)]
pub struct Gait {
    pub(crate) cycles: f32,
    pace: f32,
    walking: bool,
}

impl Gait {
    /// The least pace a step finishes at, in strides a second.
    const FINISH: f32 = 4.0;

    /// Walks `cycle` on by `speed` (pixels a second, backwards below 0) for
    /// `delta` seconds while it `walks`; once it stops, finishes the step to
    /// the clip's next contact first. Whether it's still walking: the game
    /// plays the clip while it is.
    pub fn advance(&mut self, cycle: &Clip, walks: bool, speed: f32, delta: f32) -> bool {
        let Some(stride) = cycle.stride() else {
            self.walking = walks;
            return walks;
        };
        match (walks, self.walking) {
            (true, true) => {
                self.pace = speed;
                self.cycles += speed * delta / stride;
            }
            (true, false) => {
                // Starting: from a step's start.
                self.pace = speed;
                self.cycles = cycle
                    .contacts()
                    .first()
                    .copied()
                    .unwrap_or(self.cycles.rem_euclid(1.0));
                self.walking = true;
            }
            (false, true) => {
                // Finishing the step: on at the last pace, or briskly if that
                // was slow, to the next contact.
                let pace = self.pace.signum() * self.pace.abs().max(Self::FINISH * stride);
                let by = pace * delta / stride;
                match cycle.passes(self.cycles, by) {
                    Some(ahead) => {
                        self.cycles += ahead;
                        self.walking = false;
                    }
                    None if !cycle.contacts().is_empty() => self.cycles += by,
                    None => self.walking = false,
                }
            }
            (false, false) => {}
        }
        self.walking
    }

    /// Stops at once, mid-step: a fall, a knockback.
    pub fn halt(&mut self) {
        self.walking = false;
    }

    /// How far it's walked, in cycles.
    #[must_use]
    pub fn cycles(&self) -> f32 {
        self.cycles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::clip;

    fn walk() -> Clip {
        clip(
            r#"(
                layer: Move,
                driver: Gait(stride: 32),
                contacts: [0.0, 0.5],
                channels: [(bone: "leg", angle: Wave(amplitude: 30))],
            )"#,
        )
    }

    #[test]
    fn steps_with_the_ground_covered_and_backwards_in_reverse() {
        let walk = walk();
        let mut gait = Gait::default();
        assert!(gait.advance(&walk, true, 80.0, 0.1));
        // From the contact it started at, a quarter of a stride on.
        gait.advance(&walk, true, 80.0, 0.1);
        assert_eq!(gait.cycles(), 0.25);
        let mut back = Gait::default();
        back.advance(&walk, true, -80.0, 0.1);
        back.advance(&walk, true, -80.0, 0.1);
        assert_eq!(back.cycles(), -0.25);
    }

    #[test]
    fn finishes_its_step_before_standing() {
        let walk = walk();
        let mut gait = Gait::default();
        gait.advance(&walk, true, 80.0, 0.01);
        // A third of a stride in: mid-step.
        for _ in 0..4 {
            gait.advance(&walk, true, 80.0, 0.033);
        }
        assert!(gait.advance(&walk, false, 0.0, 0.01), "still finishing");
        // Walking on, briskly, it reaches the contact at half, and stands.
        let mut walking = true;
        for _ in 0..20 {
            walking = gait.advance(&walk, false, 0.0, 0.033);
        }
        assert!(!walking);
        assert_eq!(gait.cycles().rem_euclid(1.0), 0.5);
    }
}
