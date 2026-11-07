//! What a game writes a clip as: its layer, what advances it, and the channels
//! that move its bones, in its own labels. [`Clip::new`](super::Clip::new)
//! checks it.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// A way a puppet moves, as a game writes it, generic over its roles `R`,
/// layers `L` and params `P`.
///
/// ```ron
/// (
///     layer: Move,
///     driver: Gait(stride: 32),
///     contacts: [0.0, 0.5],
///     channels: [(bone: "leg", angle: Wave(amplitude: 30))],
/// )
/// ```
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "R: Serialize, L: Serialize, P: Serialize",
    deserialize = "R: Deserialize<'de>, L: Deserialize<'de>, P: Deserialize<'de>"
))]
pub struct ClipSource<R, L, P> {
    /// The layer it plays on.
    pub layer: L,
    /// What advances it. Defaults to a looping second.
    #[serde(default = "Driver::second")]
    pub driver: Driver<P>,
    /// Holds its parameter in this many steps per unit, as drawn frames are.
    /// `None` (the default): smooth.
    #[serde(default)]
    pub steps: Option<NonZeroU32>,
    /// Where in a cycle its limbs pass under the body, from 0 to 1: a
    /// [`Gait`](crate::pose::Gait) stops at the next, and starts from the
    /// first.
    #[serde(default)]
    pub contacts: Vec<f32>,
    /// Holds its last pose once played, over the pose it started from, with the
    /// layers under it still (a death), rather than ending.
    #[serde(default)]
    pub hold: bool,
    /// What it moves.
    pub channels: Vec<ChannelSource<R>>,
}

/// What advances a clip's parameter, each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "P: Serialize", deserialize = "P: Deserialize<'de>"))]
pub enum Driver<P> {
    /// Seconds since it started, over `length`: looping, or once.
    Time {
        /// Seconds a cycle takes.
        length: f32,
        /// Whether it loops.
        #[serde(default)]
        loops: bool,
    },
    /// Ground covered, over a `stride` of pixels: the puppet's
    /// [`Gait`](crate::pose::Gait). Backwards, in reverse.
    Gait {
        /// Pixels a cycle covers.
        stride: f32,
    },
    /// A value the game sets ([`Params`](crate::pose::Params)), `from` at 0 and
    /// `to` at 1: an action's progress, how fast it rises. Unset, it's 0.
    Param {
        /// The param.
        param: P,
        /// Its value at the clip's start.
        #[serde(default)]
        from: f32,
        /// Its value at the clip's end.
        #[serde(default = "Driver::<P>::one")]
        to: f32,
    },
}

/// What one channel moves, as a game writes it: a bone, by name, or a role, and
/// how.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(serialize = "R: Serialize", deserialize = "R: Deserialize<'de>"))]
pub struct ChannelSource<R> {
    /// The bone, by name, for a body's own clips.
    #[serde(default)]
    pub bone: Option<String>,
    /// The role, for clips any body can play (an ability's).
    #[serde(default)]
    pub role: Option<R>,
    /// Adds to what lower layers set, rather than replacing it.
    #[serde(default)]
    pub add: bool,
    /// Its turn, in degrees, counterclockwise.
    #[serde(default)]
    pub angle: Option<Curve>,
    /// Its shift right, in pixels.
    #[serde(default)]
    pub x: Option<Curve>,
    /// Its shift up, in pixels.
    #[serde(default)]
    pub y: Option<Curve>,
    /// Its shift along the way it points, in pixels: a thrust.
    #[serde(default)]
    pub extend: Option<Curve>,
}

/// The value a channel takes from its clip's parameter.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum Curve {
    /// `amplitude × sin(2π(frequency × t + phase))`, clamped to one side.
    Wave {
        /// How far it swings.
        amplitude: f32,
        /// Where in its swing it starts, from 0 to 1.
        #[serde(default)]
        phase: f32,
        /// Swings a cycle.
        #[serde(default = "Curve::once")]
        frequency: f32,
        /// A side it's kept to.
        #[serde(default)]
        clamp: Option<Clamp>,
    },
    /// Keyframes, `(t, value)`, in order of `t`: straight lines between them,
    /// held past the first and last.
    Keys(Vec<(f32, f32)>),
}

/// Keeps a curve's value to one side of a bound: above it, or below it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum Clamp {
    /// At least the bound.
    Above(f32),
    /// At most the bound.
    Below(f32),
}

impl<P> Driver<P> {
    /// The default driver: a looping second.
    fn second() -> Self {
        Self::Time {
            length: 1.0,
            loops: true,
        }
    }

    /// The default `to`: 1, the value as it is.
    fn one() -> f32 {
        1.0
    }
}

impl Curve {
    /// The default wave frequency: once a cycle.
    fn once() -> f32 {
        1.0
    }

    /// Whether its keyframes are in order of `t`, none at the same `t`.
    #[must_use]
    pub fn is_ordered(&self) -> bool {
        match self {
            Self::Keys(keys) => keys.windows(2).all(|pair| pair[0].0 < pair[1].0),
            Self::Wave { .. } => true,
        }
    }
}
