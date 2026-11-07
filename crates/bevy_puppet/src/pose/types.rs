//! A puppet's pose, where its bones go, and which way it faces.

use bevy::ecs::component::Component;
use bevy::math::Vec2;

use crate::rig::Rig;

/// How a puppet stands this frame: every bone's joint, by index in its rig.
///
/// Made by its [`Animator`](super::Animator), then laid out on its bones
/// ([`Placement`]).
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Pose {
    /// Each bone's joint, by index.
    pub joints: Vec<Joint>,
}

/// How a bone is moved from rest: turned about its joint, and shifted.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Joint {
    /// In radians, counterclockwise.
    pub angle: f32,
    /// In pixels, right and up.
    pub shift: Vec2,
}

/// Where a bone is this frame from the puppet's origin, facing right.
///
/// On each bone entity, for anything that wants a joint without its transform.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Its joint, on a whole pixel.
    pub at: Vec2,
    /// How far it's turned, in radians, counterclockwise: at its step, if it
    /// turns in steps.
    pub angle: f32,
    /// Which bones it's drawn over: those of lower depth.
    pub depth: i32,
}

/// Which way a puppet faces: its skeleton mirrored, exactly, facing left. Set
/// by the game.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Facing right, as rigs are drawn.
    #[default]
    Right,
    /// Facing left: mirrored.
    Left,
}

impl Pose {
    /// Every bone at rest.
    #[must_use]
    pub fn rest(rig: &Rig) -> Self {
        Self {
            joints: vec![Joint::default(); rig.len()],
        }
    }

    /// The pose `share` of the way from this to `to`.
    #[must_use]
    pub fn between(&self, to: &Self, share: f32) -> Self {
        Self {
            joints: self
                .joints
                .iter()
                .zip(&to.joints)
                .map(|(from, to)| Joint {
                    angle: from.angle + (to.angle - from.angle) * share,
                    shift: from.shift.lerp(to.shift, share),
                })
                .collect(),
        }
    }
}

impl Side {
    /// The side `direction` points to; `None` straight up or down.
    #[must_use]
    pub fn of(direction: Vec2) -> Option<Self> {
        match direction.x {
            x if x > 0.0 => Some(Self::Right),
            x if x < 0.0 => Some(Self::Left),
            _ => None,
        }
    }

    /// `1` facing right, `-1` facing left: mirrors an x across the puppet.
    #[must_use]
    pub fn sign(self) -> f32 {
        match self {
            Self::Right => 1.0,
            Self::Left => -1.0,
        }
    }

    /// A horizontal speed in the world, positive the way this faces.
    #[must_use]
    pub fn forward(self, speed: f32) -> f32 {
        speed * self.sign()
    }

    /// A direction in the world as an aim: its angle from straight ahead,
    /// facing this way.
    #[must_use]
    pub fn aim(self, direction: Vec2) -> f32 {
        direction.y.atan2(direction.x * self.sign())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_world_directions_into_aims_as_it_faces() {
        let up_ahead = Vec2::new(-1.0, 1.0).normalize();
        assert_eq!(Side::of(up_ahead), Some(Side::Left));
        assert!((Side::Left.aim(up_ahead) - 45f32.to_radians()).abs() < 1e-5);
        assert_eq!(Side::Left.forward(-50.0), 50.0);
    }
}
