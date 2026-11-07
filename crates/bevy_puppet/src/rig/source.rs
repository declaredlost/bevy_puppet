//! What a game writes a rig as: its bones, the jobs they do, and its
//! constraints, in its own labels. [`Rig::new`](super::Rig::new) checks it.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::num::NonZeroU32;

use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

/// A body plan as a game writes it, generic over its roles `R`, layers `L` and
/// goals `G`.
///
/// ```ron
/// (
///     bones: {
///         "torso": (at: (0, 20)),
///         "arm": (parent: "torso", at: (2, 6), limit: (min: -60, max: 120)),
///         "hand": (parent: "arm", at: (0, -6)),
///     },
///     roles: {Reach: "arm", Grip: "hand"},
///     constraints: [Aim(chain: ["arm"], goal: Aim, after: Carry)],
/// )
/// ```
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "R: Serialize, L: Serialize, G: Serialize",
    deserialize = "R: Deserialize<'de> + Hash + Eq, L: Deserialize<'de>, G: Deserialize<'de>"
))]
pub struct RigSource<R, L, G> {
    /// Its bones, by name.
    pub bones: BTreeMap<String, BoneSource>,
    /// The bone doing each role, by name.
    #[serde(default = "HashMap::new")]
    pub roles: HashMap<R, String>,
    /// What moves its bones past their clips, from goals the game sets.
    #[serde(default = "Vec::new")]
    pub constraints: Vec<ConstraintSource<L, G>>,
}

/// One bone of a body: a part turning about a joint, its art centred on it.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoneSource {
    /// The bone it hangs from. `None` (the default): the puppet itself.
    #[serde(default)]
    pub parent: Option<String>,
    /// Where its joint is, from its parent's joint (or the puppet's origin), in
    /// pixels, right and up, facing right.
    #[serde(default)]
    pub at: Vec2,
    /// Which bones it's drawn over: those of lower depth.
    #[serde(default)]
    pub depth: i32,
    /// How far it's turned at rest from how its art is drawn, in degrees,
    /// counterclockwise.
    #[serde(default)]
    pub rest: f32,
    /// Which way its art points, in degrees from straight ahead: `-90` (the
    /// default) for a limb drawn hanging down. Aiming it points this way along
    /// the aim; extending it moves it this way.
    #[serde(default = "BoneSource::down")]
    pub hang: f32,
    /// How far it may turn from rest, in degrees, counterclockwise. Whatever
    /// turns it (a clip, a constraint) keeps within. `None` (the default): any
    /// way.
    #[serde(default)]
    pub limit: Option<Limit>,
    /// Its sprites pre-rotated into this many steps (RotSprite), so pixel art
    /// stays crisp as it turns: it's placed at the step nearest its angle, as
    /// its frames are drawn, and so is what hangs from it. `None` (the
    /// default): turned smoothly.
    #[serde(default)]
    pub turns: Option<NonZeroU32>,
}

/// How far a bone may turn from rest, both ends included.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limit {
    /// The least, in degrees (in radians, once in a [`Rig`](super::Rig)).
    pub min: f32,
    /// The most.
    pub max: f32,
}

/// A constraint as a game writes it: what it moves, and its goal and layer.
///
/// The goal it reaches for, and the layer it runs after (and before the next's
/// clips).
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum ConstraintSource<L, G> {
    /// While its goal is an angle: points the last of `chain` (bones, each the
    /// next's parent) along it. From the first, each turns as far toward
    /// pointing it as its limit lets, the bones after it as they stand, and the
    /// next makes up the rest: an arm turning at the shoulder with its elbow
    /// and grip as they are, its forearm bending once the arm stops short.
    Aim {
        /// The bones, from the one nearest the body.
        chain: Vec<String>,
        /// The goal: an angle from straight ahead.
        goal: G,
        /// The layer it runs after.
        after: L,
    },
    /// While its goal is an angle: turns `bone` from rest by a `share` of it: a
    /// head looking a little the way it aims.
    Look {
        /// The bone.
        bone: String,
        /// How much of the angle it turns: `0.25`, a quarter.
        share: f32,
        /// The goal: an angle from straight ahead.
        goal: G,
        /// The layer it runs after.
        after: L,
    },
    /// While its goal is a point: bends the two bones above `hand` (its forearm
    /// and its upper arm) so it reaches that point in `on`'s frame: a second
    /// hand on what the grip holds.
    Reach {
        /// The bone that reaches: the hand.
        hand: String,
        /// The bone whose frame the point is in.
        on: String,
        /// The goal: a point in `on`'s frame.
        goal: G,
        /// The layer it runs after.
        after: L,
    },
}

impl BoneSource {
    /// The default hang: straight down.
    fn down() -> f32 {
        -90.0
    }
}

impl Default for BoneSource {
    fn default() -> Self {
        Self {
            parent: None,
            at: Vec2::ZERO,
            depth: 0,
            rest: 0.0,
            hang: Self::down(),
            limit: None,
            turns: None,
        }
    }
}

impl Limit {
    /// Whether its least is at most its most: false for a bound that isn't a
    /// number.
    #[must_use]
    pub fn is_ordered(self) -> bool {
        self.min <= self.max
    }
}

impl<L, G> ConstraintSource<L, G> {
    /// The bones it names.
    #[must_use]
    pub fn bones(&self) -> Vec<&str> {
        match self {
            Self::Aim { chain, .. } => chain.iter().map(String::as_str).collect(),
            Self::Look { bone, .. } => vec![bone.as_str()],
            Self::Reach { hand, on, .. } => vec![hand.as_str(), on.as_str()],
        }
    }
}
