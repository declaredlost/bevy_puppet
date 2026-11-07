//! Clips: ways a puppet moves, as assets, moving bones by name or by role.
//!
//! What advances each, the layer it plays on, and the channels moving its
//! bones.

mod shape;
mod source;

use std::hash::Hash;

use bevy::asset::Asset;
use bevy::math::Vec2;
use bevy::reflect::TypePath;
use thiserror::Error;

pub use self::shape::Shape;
pub use self::source::{ChannelSource, Clamp, ClipSource, Curve, Driver};
use crate::label::{InternedLayer, InternedParam, InternedRole, LayerLabel, ParamLabel, RoleLabel};

/// A way a puppet moves: channels moving bones, what advances them, its layer.
///
/// Made for any rig with the bones (or roles) it names; bound to each as it
/// plays. An asset: an [`Animator`](crate::pose::Animator) plays one by handle.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct Clip {
    /// Which clip this is, as made: a clip replaced has a new one, so it's
    /// bound again.
    pub(crate) revision: u64,
    pub(crate) layer: InternedLayer,
    pub(crate) driver: Drive,
    pub(crate) steps: Option<u32>,
    pub(crate) contacts: Vec<f32>,
    pub(crate) hold: bool,
    pub(crate) channels: Vec<Channel>,
}

/// What advances a clip, interned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Drive {
    Time {
        length: f32,
        loops: bool,
    },
    Gait {
        stride: f32,
    },
    Param {
        param: InternedParam,
        from: f32,
        to: f32,
    },
}

/// A channel, checked: what it moves, and its curves.
#[derive(Debug, Clone)]
pub(crate) struct Channel {
    pub(crate) target: Target,
    pub(crate) add: bool,
    pub(crate) angle: Option<Shape>,
    pub(crate) x: Option<Shape>,
    pub(crate) y: Option<Shape>,
    pub(crate) extend: Option<Shape>,
}

/// What a channel moves: a bone, by name, or the bone doing a role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Bone(String),
    Role(InternedRole),
}

/// A clip's channels bound to a rig's bones, by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Binding(pub(crate) Vec<usize>);

/// What's wrong with a clip: a channel without a target, or keys out of order.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ClipError {
    /// A channel names neither a bone nor a role, or both.
    #[error("channel {channel} names neither a bone nor a role, or both")]
    Target {
        /// Which channel, from 0.
        channel: usize,
    },
    /// A channel's keyframes aren't in order, or two are at the same `t`.
    #[error("channel {channel} has keyframes out of order, or two at the same t")]
    Keys {
        /// Which channel, from 0.
        channel: usize,
    },
    /// A time driver's length isn't a number above 0.
    #[error("a time driver's length of {length} isn't above 0")]
    Length {
        /// The length.
        length: f32,
    },
    /// A gait driver's stride isn't a number above 0.
    #[error("a gait driver's stride of {stride} isn't above 0")]
    Stride {
        /// The stride.
        stride: f32,
    },
}

impl Clip {
    /// Checks `source`: every channel names a bone or a role, and its keyframes
    /// are in order.
    ///
    /// # Errors
    ///
    /// The first thing wrong.
    pub fn new<R, L, P>(source: &ClipSource<R, L, P>) -> Result<Self, ClipError>
    where
        R: RoleLabel + Hash + Eq,
        L: LayerLabel,
        P: ParamLabel,
    {
        match source.driver {
            Driver::Time { length, .. } if !(length.is_finite() && length > 0.0) => {
                return Err(ClipError::Length { length });
            }
            Driver::Gait { stride } if !(stride.is_finite() && stride > 0.0) => {
                return Err(ClipError::Stride { stride });
            }
            _ => {}
        }
        let channels = source
            .channels
            .iter()
            .enumerate()
            .map(|(index, channel)| {
                let target = match (&channel.bone, &channel.role) {
                    (Some(bone), None) => Target::Bone(bone.clone()),
                    (None, Some(role)) => Target::Role(role.intern()),
                    _ => return Err(ClipError::Target { channel: index }),
                };
                let shape = |curve: &Option<Curve>| {
                    curve
                        .as_ref()
                        .map(|curve| {
                            Shape::from_curve(curve).ok_or(ClipError::Keys { channel: index })
                        })
                        .transpose()
                };
                Ok(Channel {
                    target,
                    add: channel.add,
                    angle: shape(&channel.angle)?,
                    x: shape(&channel.x)?,
                    y: shape(&channel.y)?,
                    extend: shape(&channel.extend)?,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            revision: crate::revision(),
            layer: source.layer.intern(),
            driver: match &source.driver {
                Driver::Time { length, loops } => Drive::Time {
                    length: *length,
                    loops: *loops,
                },
                Driver::Gait { stride } => Drive::Gait { stride: *stride },
                Driver::Param { param, from, to } => Drive::Param {
                    param: param.intern(),
                    from: *from,
                    to: *to,
                },
            },
            steps: source.steps.map(std::num::NonZeroU32::get),
            contacts: source.contacts.clone(),
            hold: source.hold,
            channels,
        })
    }

    /// The layer it plays on.
    #[must_use]
    pub fn layer(&self) -> InternedLayer {
        self.layer
    }

    /// Whether it holds its last pose once played (a death).
    #[must_use]
    pub fn holds(&self) -> bool {
        self.hold
    }

    /// The pixels of ground one cycle covers, if a gait drives it.
    #[must_use]
    pub fn stride(&self) -> Option<f32> {
        match self.driver {
            Drive::Gait { stride } if stride > 0.0 => Some(stride),
            _ => None,
        }
    }

    /// Where in a cycle its limbs pass under the body.
    #[must_use]
    pub fn contacts(&self) -> &[f32] {
        &self.contacts
    }

    /// Whether it loops: its parameter runs on past 1 (a gait's ground covered,
    /// a looping time), rather than playing once.
    pub(crate) fn loops(&self) -> bool {
        matches!(
            self.driver,
            Drive::Gait { .. } | Drive::Time { loops: true, .. }
        )
    }

    /// Whether, `played` seconds in, it's played out: only a time that doesn't
    /// loop ends; the game stops the rest.
    pub(crate) fn finished(&self, played: f32) -> bool {
        match self.driver {
            Drive::Time { length, loops } => !loops && played >= length,
            _ => false,
        }
    }

    /// `t` held to its steps, if it has them.
    #[expect(
        clippy::cast_precision_loss,
        reason = "steps a unit, far below where f32 loses integers"
    )]
    pub(crate) fn step(&self, t: f32) -> f32 {
        match self.steps {
            Some(steps) => (t * steps as f32).floor() / steps as f32,
            None => t,
        }
    }

    /// How far on from `from` (backwards, below 0) the first of its contacts it
    /// passes going `by` is, as parts of a cycle: where a gait stops.
    pub(crate) fn passes(&self, from: f32, by: f32) -> Option<f32> {
        let from = from.rem_euclid(1.0);
        self.contacts
            .iter()
            .filter_map(|&contact| {
                let ahead = if by >= 0.0 {
                    (contact - from).rem_euclid(1.0)
                } else {
                    (from - contact).rem_euclid(1.0)
                };
                (ahead > 0.0 && ahead <= by.abs()).then_some(ahead)
            })
            .min_by(f32::total_cmp)
            .map(|ahead| ahead.copysign(by))
    }
}

impl Channel {
    /// Its shift along `direction`, at `t`, looping as its clip does.
    pub(crate) fn extended(&self, t: f32, loops: bool, direction: Vec2) -> Vec2 {
        self.extend
            .as_ref()
            .map_or(Vec2::ZERO, |shape| direction * shape.sample(t, loops))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Job, Knob, Tier, de};

    fn new(source: &str) -> Result<Clip, ClipError> {
        let source: ClipSource<Job, Tier, Knob> = de(source);
        Clip::new(&source)
    }

    #[test]
    fn refuses_drivers_and_keys_that_cant_play() {
        assert!(matches!(
            new("(layer: Act, driver: Time(length: 0), channels: [])"),
            Err(ClipError::Length { .. })
        ));
        assert!(matches!(
            new("(layer: Move, driver: Gait(stride: -1), channels: [])"),
            Err(ClipError::Stride { .. })
        ));
        assert!(matches!(
            new(r#"(layer: Act, channels: [(bone: "arm", angle: Keys([(0, 0), (0, 5)]))])"#),
            Err(ClipError::Keys { channel: 0 })
        ));
        assert!(matches!(
            new(r#"(layer: Act, channels: [(bone: "arm", role: Reach)])"#),
            Err(ClipError::Target { channel: 0 })
        ));
    }
}
