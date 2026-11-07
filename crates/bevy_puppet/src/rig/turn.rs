//! Bones that turn in steps, as their pre-rotated frames are drawn.

use std::f32::consts::TAU;

/// A bone's sprites pre-rotated in even steps, as RotSprite draws them.
///
/// In `steps` even steps from 0, counterclockwise: it shows the step nearest
/// its angle, and is placed at that step's angle, so what hangs from it hangs
/// where it's drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Turn {
    /// How many steps round.
    pub steps: u32,
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a step count and a step, far below where f32 loses integers"
)]
impl Turn {
    /// The step nearest `angle`, in radians, counterclockwise.
    #[must_use]
    pub fn step(self, angle: f32) -> u32 {
        let steps = self.steps.max(1);
        let turns = angle.rem_euclid(TAU) / TAU;
        (turns * steps as f32).round() as u32 % steps
    }

    /// `angle`, in radians, snapped to the nearest step: the angle the step it
    /// shows is drawn at, however many turns round.
    #[must_use]
    pub fn snap(self, angle: f32) -> f32 {
        let step = TAU / self.steps.max(1) as f32;
        (angle / step).round() * step
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;

    #[test]
    fn picks_and_snaps_to_the_nearest_step() {
        let turn = Turn { steps: 32 };
        assert_eq!(turn.step(FRAC_PI_2), 8);
        assert_eq!(turn.step(-FRAC_PI_2), 24);
        assert_eq!(turn.step(PI + 0.01), 16);
        let step = TAU / 32.0;
        assert!(turn.snap(step * 0.4).abs() < 1e-6);
        assert!((turn.snap(step * 0.6) - step).abs() < 1e-6);
        for angle in [0.3, 1.7, -2.2, 5.9] {
            assert_eq!(turn.step(turn.snap(angle)), turn.step(angle));
        }
    }
}
