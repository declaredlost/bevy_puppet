//! Rigs: body plans as assets, with their bones, roles and constraints.
//!
//! Bones checked and laid out, the bone doing each role, and the constraints
//! that move them past their clips.

mod constraint;
mod source;
mod turn;

use std::collections::HashMap;
use std::hash::Hash;

use bevy::asset::Asset;
use bevy::math::Vec2;
use bevy::reflect::TypePath;
use thiserror::Error;

pub(crate) use self::constraint::Constraint;
pub use self::source::{BoneSource, ConstraintSource, Limit, RigSource};
pub use self::turn::Turn;
use crate::clip::{Binding, Clip, Target};
use crate::label::{GoalLabel, InternedRole, LayerLabel, RoleLabel};
use crate::pose::{Placement, Pose};

/// A body plan, checked and laid out for posing: bones, roles, constraints.
///
/// Its bones in an order where each comes after its parent, the bone doing each
/// role, and its constraints. An asset: a [`Puppet`](crate::Puppet) holds one
/// by handle.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct Rig {
    pub(crate) bones: Vec<Segment>,
    names: HashMap<String, usize>,
    roles: HashMap<InternedRole, usize>,
    pub(crate) constraints: Vec<Constraint>,
}

/// A bone, checked: its parent by index, its angles in radians.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Segment {
    pub(crate) name: String,
    pub(crate) parent: Option<usize>,
    pub(crate) at: Vec2,
    pub(crate) depth: i32,
    pub(crate) rest: f32,
    pub(crate) hang: f32,
    pub(crate) limit: Option<Limit>,
    pub(crate) turn: Option<Turn>,
}

/// What's wrong with a body plan, or with a clip bound to one.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RigError {
    /// Something names a bone the rig doesn't have.
    #[error("`{by}` names bone `{bone}`, which the rig doesn't have")]
    Unknown {
        /// The bone.
        bone: String,
        /// What names it.
        by: String,
    },
    /// A clip moves a role the rig gives no bone.
    #[error("a clip moves the {role}, which the rig has no bone for")]
    Role {
        /// The role.
        role: String,
    },
    /// Bones hang from each other in a loop.
    #[error("bones hang from each other in a loop: {}", bones.join(", "))]
    Loop {
        /// The bones.
        bones: Vec<String>,
    },
    /// An aim's chain isn't bones each hanging from the one before.
    #[error("the aim's chain `{}` isn't each bone hanging from the last", chain.join(", "))]
    Chain {
        /// The chain.
        chain: Vec<String>,
    },
    /// A bone's limit has its least above its most.
    #[error("bone `{bone}`'s limit has its `min` above its `max`")]
    Limit {
        /// The bone.
        bone: String,
    },
    /// A reach's hand doesn't hang from two bones to bend.
    #[error("the reach's hand `{hand}` doesn't hang from a forearm and an upper arm")]
    Arm {
        /// The hand.
        hand: String,
    },
}

impl Segment {
    /// `angle`, a turn from rest in radians, kept within its limit.
    pub(crate) fn limited(&self, angle: f32) -> f32 {
        self.limit
            .map_or(angle, |limit| angle.max(limit.min).min(limit.max))
    }
}

impl Rig {
    /// Checks `source` and lays it out: every bone, role and constraint names
    /// bones it has, no bones hang from each other in a loop, and no limits are
    /// the wrong way round.
    ///
    /// # Errors
    ///
    /// The first thing wrong.
    pub fn new<R, L, G>(source: &RigSource<R, L, G>) -> Result<Self, RigError>
    where
        R: RoleLabel + Hash + Eq,
        L: LayerLabel,
        G: GoalLabel,
    {
        let order = Self::order(&source.bones)?;
        let names: HashMap<String, usize> = order
            .iter()
            .enumerate()
            .map(|(index, name)| ((*name).to_owned(), index))
            .collect();
        let find = |bone: &str, by: &str| {
            names.get(bone).copied().ok_or_else(|| RigError::Unknown {
                bone: bone.to_owned(),
                by: by.to_owned(),
            })
        };
        let bones = order
            .iter()
            .map(|name| {
                let bone = &source.bones[*name];
                if bone.limit.is_some_and(|limit| !limit.is_ordered()) {
                    return Err(RigError::Limit {
                        bone: (*name).to_owned(),
                    });
                }
                Ok(Segment {
                    name: (*name).to_owned(),
                    parent: bone
                        .parent
                        .as_deref()
                        .map(|parent| find(parent, name))
                        .transpose()?,
                    at: bone.at,
                    depth: bone.depth,
                    rest: bone.rest.to_radians(),
                    hang: bone.hang.to_radians(),
                    limit: bone.limit.map(|limit| Limit {
                        min: limit.min.to_radians(),
                        max: limit.max.to_radians(),
                    }),
                    turn: bone.turns.map(|steps| Turn { steps: steps.get() }),
                })
            })
            .collect::<Result<_, RigError>>()?;
        let roles = source
            .roles
            .iter()
            .map(|(role, bone)| Ok((role.intern(), find(bone, &format!("{role:?}"))?)))
            .collect::<Result<_, RigError>>()?;
        let mut rig = Self {
            bones,
            names,
            roles,
            constraints: Vec::new(),
        };
        rig.constraints = source
            .constraints
            .iter()
            .map(|constraint| Constraint::new(&rig, constraint))
            .collect::<Result<_, RigError>>()?;
        Ok(rig)
    }

    /// How many bones it has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bones.len()
    }

    /// Whether it has no bones.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// A bone's index, by name: its joint in a [`Pose`], its entity in
    /// [`Bones`](crate::Bones).
    #[must_use]
    pub fn index(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }

    /// The bone doing `role`, by index, if it has one.
    #[must_use]
    pub fn role(&self, role: impl RoleLabel) -> Option<usize> {
        self.roles.get(&role.intern()).copied()
    }

    /// A bone's name, by index.
    #[must_use]
    pub fn name(&self, index: usize) -> &str {
        &self.bones[index].name
    }

    /// A bone's parent, by index: `None` for one hanging from the puppet.
    #[must_use]
    pub fn parent(&self, index: usize) -> Option<usize> {
        self.bones[index].parent
    }

    /// How bone `index`'s sprites are pre-rotated, if they are.
    #[must_use]
    pub fn turn(&self, index: usize) -> Option<Turn> {
        self.bones.get(index).and_then(|bone| bone.turn)
    }

    /// `clip`'s channels, bound to this rig's bones: by name, or by the role a
    /// clip made for any rig moves.
    pub(crate) fn bind(&self, clip: &Clip) -> Result<Binding, RigError> {
        clip.channels
            .iter()
            .map(|channel| match &channel.target {
                Target::Bone(name) => self.find(name, "a clip"),
                Target::Role(role) => self.roles.get(role).copied().ok_or_else(|| RigError::Role {
                    role: format!("{role:?}"),
                }),
            })
            .collect::<Result<_, _>>()
            .map(Binding)
    }

    /// Where every bone goes in `pose`, facing right: its joint from the
    /// puppet's origin, on a whole pixel; its angle; and its depth. A joint is
    /// rounded off its parent's rounded joint, not on its own: its parent's art
    /// is drawn from there, so it stays on the pixel that art puts it on (a
    /// fist's grip on the forearm's fist). Facing left mirrors it all exactly,
    /// so the pixels stay put.
    #[must_use]
    pub fn place(&self, pose: &Pose) -> Vec<Placement> {
        let joints = self.joints(pose);
        let mut placed: Vec<Placement> = Vec::with_capacity(joints.len());
        for (&(at, angle), bone) in joints.iter().zip(&self.bones) {
            let at = match bone.parent {
                Some(parent) => placed[parent].at + (at - joints[parent].0).round(),
                None => at.round(),
            };
            placed.push(Placement {
                at,
                angle,
                depth: bone.depth,
            });
        }
        placed
    }

    /// Every bone's joint and angle in `pose`, facing right, not rounded: a
    /// stepped bone's angle at the step its frame is drawn at, so what hangs
    /// from it hangs where it's drawn. Each bone's step is its own true
    /// angle's, not its parent's step turned on: a wrist turning back as its
    /// arm swings keeps its blade steady.
    pub(crate) fn joints(&self, pose: &Pose) -> Vec<(Vec2, f32)> {
        let mut joints: Vec<(Vec2, f32)> = Vec::with_capacity(self.bones.len());
        let mut exact: Vec<f32> = Vec::with_capacity(self.bones.len());
        for (index, bone) in self.bones.iter().enumerate() {
            let joint = pose.joints.get(index).copied().unwrap_or_default();
            let (from, drawn, turned) = bone.parent.map_or((Vec2::ZERO, 0.0, 0.0), |parent| {
                (joints[parent].0, joints[parent].1, exact[parent])
            });
            let at = from + Vec2::from_angle(drawn).rotate(bone.at) + joint.shift;
            let angle = turned + bone.rest + joint.angle;
            exact.push(angle);
            joints.push((at, bone.turn.map_or(angle, |turn| turn.snap(angle))));
        }
        joints
    }

    /// How far `bone`'s parents have turned it, in all, in `pose`: each one's
    /// angle and rest, up to the puppet.
    pub(crate) fn turned(&self, pose: &Pose, bone: usize) -> f32 {
        let mut turned = 0.0;
        let mut at = self.parent(bone);
        while let Some(parent) = at {
            turned += pose.joints[parent].angle + self.bones[parent].rest;
            at = self.parent(parent);
        }
        turned
    }

    /// A bone, by name, for `by`'s errors.
    pub(crate) fn find(&self, bone: &str, by: &str) -> Result<usize, RigError> {
        self.index(bone).ok_or_else(|| RigError::Unknown {
            bone: bone.to_owned(),
            by: by.to_owned(),
        })
    }

    /// The bones' names, each after its parent.
    fn order(
        bones: &std::collections::BTreeMap<String, BoneSource>,
    ) -> Result<Vec<&str>, RigError> {
        let mut order: Vec<&str> = Vec::with_capacity(bones.len());
        let mut left: Vec<&str> = bones.keys().map(String::as_str).collect();
        while !left.is_empty() {
            let before = left.len();
            left.retain(|name| {
                let parent = bones[*name].parent.as_deref();
                let ready = parent
                    .is_none_or(|parent| order.contains(&parent) || !bones.contains_key(parent));
                if ready {
                    order.push(name);
                }
                !ready
            });
            if left.len() == before {
                return Err(RigError::Loop {
                    bones: left.iter().map(|name| (*name).to_owned()).collect(),
                });
            }
        }
        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;
    use crate::pose::{Joint, Pose};
    use crate::testing::{Job, clip, rig};

    #[test]
    fn orders_bones_after_their_parents_and_binds_roles() {
        let rig = rig(r#"(
            bones: {
                "arm": (parent: "torso", at: (2, 7)),
                "hand": (parent: "arm", at: (0, -6)),
                "torso": (),
            },
            roles: {Reach: "arm"},
        )"#)
        .unwrap();
        assert_eq!(rig.name(0), "torso");
        let [arm, hand] = ["arm", "hand"].map(|name| rig.index(name).unwrap());
        assert!(arm < hand);
        assert_eq!(rig.role(Job::Reach), Some(arm));
        assert_eq!(rig.role(Job::Off), None);
    }

    #[test]
    fn refuses_what_it_doesnt_have() {
        assert_eq!(
            rig(r#"(bones: {"arm": (parent: "torso")})"#).unwrap_err(),
            RigError::Unknown {
                bone: "torso".into(),
                by: "arm".into(),
            }
        );
        let looped = rig(r#"(bones: {"a": (parent: "b"), "b": (parent: "a")})"#);
        assert!(matches!(looped, Err(RigError::Loop { .. })));
        let role = rig(r#"(bones: {"torso": ()}, roles: {Reach: "arm"})"#);
        assert!(matches!(role, Err(RigError::Unknown { .. })));
        let limit = rig(r#"(bones: {"arm": (limit: (min: 30, max: -30))})"#);
        assert!(matches!(limit, Err(RigError::Limit { .. })));
        let chain = rig(r#"(
            bones: {"torso": (), "arm": (parent: "torso"), "leg": (parent: "torso")},
            constraints: [Aim(chain: ["arm", "leg"], goal: Aim, after: Carry)],
        )"#);
        assert!(matches!(chain, Err(RigError::Chain { .. })));
        let arm = rig(r#"(
            bones: {"torso": (), "hand": ()},
            constraints: [Reach(hand: "hand", on: "torso", goal: Hold, after: React)],
        )"#);
        assert!(matches!(arm, Err(RigError::Arm { .. })));
        let torso = rig(r#"(bones: {"torso": ()})"#).unwrap();
        let reach = clip("(layer: Act, channels: [(role: Reach)])");
        assert!(matches!(torso.bind(&reach), Err(RigError::Role { .. })));
    }

    #[test]
    fn places_children_on_their_parents_turned_joints() {
        let rig = rig(r#"(bones: {
            "torso": (at: (0, 10)),
            "arm": (parent: "torso", at: (4, 0)),
            "hand": (parent: "arm", at: (0, -6)),
        })"#)
        .unwrap();
        let mut pose = Pose::rest(&rig);
        // The arm turned a quarter, counterclockwise: the hand, below it,
        // swings out to its right.
        pose.joints[rig.index("arm").unwrap()] = Joint {
            angle: FRAC_PI_2,
            shift: Vec2::ZERO,
        };
        let placed = rig.place(&pose);
        assert_eq!(placed[rig.index("arm").unwrap()].at, Vec2::new(4.0, 10.0));
        assert_eq!(placed[rig.index("hand").unwrap()].at, Vec2::new(10.0, 10.0));
    }

    #[test]
    fn rounds_a_joint_off_its_parents_rounded_joint() {
        // Each 0.6 up: on its own the hand's 1.2 rounds to 1, the arm's too;
        // off the arm's rounded joint it's a whole pixel above it.
        let rig = rig(r#"(bones: {
            "arm": (at: (0, 0.6)),
            "hand": (parent: "arm", at: (0, 0.6)),
        })"#)
        .unwrap();
        let placed = rig.place(&Pose::rest(&rig));
        assert_eq!(placed[rig.index("arm").unwrap()].at, Vec2::new(0.0, 1.0));
        assert_eq!(placed[rig.index("hand").unwrap()].at, Vec2::new(0.0, 2.0));
    }

    #[test]
    fn steps_a_bone_by_its_own_angle_not_its_parents_step() {
        let rig = rig(r#"(bones: {
            "arm": (turns: 4),
            "hand": (parent: "arm", at: (0, -10), turns: 4),
        })"#)
        .unwrap();
        let mut pose = Pose::rest(&rig);
        // The arm 50 round (drawn at the quarter), the hand turned back 50:
        // level in truth, so drawn level, not a quarter round with the arm.
        pose.joints[rig.index("arm").unwrap()].angle = 50f32.to_radians();
        pose.joints[rig.index("hand").unwrap()].angle = -50f32.to_radians();
        let placed = rig.place(&pose);
        assert!((placed[rig.index("arm").unwrap()].angle - FRAC_PI_2).abs() < 1e-5);
        assert!(placed[rig.index("hand").unwrap()].angle.abs() < 1e-5);
    }

    #[test]
    fn hangs_from_a_stepped_bone_where_its_frame_is_drawn() {
        let rig = rig(r#"(bones: {
            "arm": (turns: 4),
            "hand": (parent: "arm", at: (0, -10)),
        })"#)
        .unwrap();
        let mut pose = Pose::rest(&rig);
        // Turned a little short of a quarter: drawn at the quarter, so the
        // hand hangs straight out to its right, not a little low.
        pose.joints[rig.index("arm").unwrap()].angle = 80f32.to_radians();
        let placed = rig.place(&pose);
        assert!((placed[rig.index("arm").unwrap()].angle - FRAC_PI_2).abs() < 1e-5);
        assert_eq!(placed[rig.index("hand").unwrap()].at, Vec2::new(10.0, 0.0));
    }
}
