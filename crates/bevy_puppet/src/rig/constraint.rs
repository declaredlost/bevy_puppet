//! Constraints, bound to a rig: what moves its bones past their clips, each
//! toward its own goal.

use bevy::math::Vec2;

use super::{Rig, RigError};
use crate::label::{GoalLabel, InternedGoal, InternedLayer, LayerLabel};
use crate::pose::{Goal, Pose};
use crate::rig::ConstraintSource;

/// A constraint, bound: its bones by index, its labels interned.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Constraint {
    /// The layer it runs after.
    pub(crate) after: InternedLayer,
    /// The goal it reaches for.
    pub(crate) goal: InternedGoal,
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Aim {
        chain: Vec<usize>,
    },
    Look {
        bone: usize,
        share: f32,
    },
    Reach {
        hand: usize,
        elbow: usize,
        shoulder: usize,
        on: usize,
    },
}

impl Constraint {
    /// `source`, bound to `rig`'s bones.
    pub(crate) fn new<L: LayerLabel, G: GoalLabel>(
        rig: &Rig,
        source: &ConstraintSource<L, G>,
    ) -> Result<Self, RigError> {
        let by = "a constraint";
        let (kind, goal, after) = match source {
            ConstraintSource::Aim { chain, goal, after } => {
                let bones = chain
                    .iter()
                    .map(|bone| rig.find(bone, by))
                    .collect::<Result<Vec<_>, _>>()?;
                if bones
                    .windows(2)
                    .any(|pair| rig.parent(pair[1]) != Some(pair[0]))
                {
                    return Err(RigError::Chain {
                        chain: chain.clone(),
                    });
                }
                (Kind::Aim { chain: bones }, goal.intern(), after.intern())
            }
            ConstraintSource::Look {
                bone,
                share,
                goal,
                after,
            } => (
                Kind::Look {
                    bone: rig.find(bone, by)?,
                    share: *share,
                },
                goal.intern(),
                after.intern(),
            ),
            ConstraintSource::Reach {
                hand,
                on,
                goal,
                after,
            } => {
                let index = rig.find(hand, by)?;
                let elbow = rig.parent(index);
                let shoulder = elbow.and_then(|elbow| rig.parent(elbow));
                let (Some(elbow), Some(shoulder)) = (elbow, shoulder) else {
                    return Err(RigError::Arm { hand: hand.clone() });
                };
                (
                    Kind::Reach {
                        hand: index,
                        elbow,
                        shoulder,
                        on: rig.find(on, by)?,
                    },
                    goal.intern(),
                    after.intern(),
                )
            }
        };
        Ok(Self { after, goal, kind })
    }

    /// Moves `pose` toward `goal`, if it's the kind it reaches for: an angle
    /// for an aim or a look, a point for a reach.
    pub(crate) fn solve(&self, rig: &Rig, pose: &mut Pose, goal: Goal) {
        match (&self.kind, goal) {
            (Kind::Aim { chain }, Goal::Angle(aim)) => point(rig, pose, chain, aim),
            (Kind::Look { bone, share }, Goal::Angle(aim)) => {
                pose.joints[*bone].angle = rig.bones[*bone].limited(share * aim);
            }
            (
                Kind::Reach {
                    hand,
                    elbow,
                    shoulder,
                    on,
                },
                Goal::Point(at),
            ) => bend(rig, pose, *on, at, *hand, *elbow, *shoulder),
            _ => {}
        }
    }
}

/// Points the last of `chain` along `aim`: from the first, each turns as far
/// toward pointing it as its limit lets, with the bones after it as they stand,
/// and the next makes up the rest.
fn point(rig: &Rig, pose: &mut Pose, chain: &[usize], aim: f32) {
    let (Some(&first), Some(&last)) = (chain.first(), chain.last()) else {
        return;
    };
    let end = &rig.bones[last];
    let mut above = rig.turned(pose, first);
    for (index, &bone) in chain.iter().enumerate() {
        // How far the end's art turns from this bone, with the bones
        // between as they stand.
        let after: f32 = chain[index + 1..]
            .iter()
            .map(|&next| rig.bones[next].rest + pose.joints[next].angle)
            .sum();
        let segment = &rig.bones[bone];
        let angle = segment.limited(aim - end.hang - after - segment.rest - above);
        pose.joints[bone].angle = angle;
        above += angle + segment.rest;
    }
}

/// Bends `shoulder` and `elbow` so `hand` reaches `at` in `on`'s frame:
/// two-bone IK, the elbow bending down.
fn bend(
    rig: &Rig,
    pose: &mut Pose,
    on: usize,
    at: Vec2,
    hand: usize,
    elbow: usize,
    shoulder: usize,
) {
    let joints = rig.joints(pose);
    let (on_at, on_angle) = joints[on];
    let target = on_at + Vec2::from_angle(on_angle).rotate(at);
    let (origin, _) = joints[shoulder];
    let above = rig.parent(shoulder).map_or(0.0, |parent| joints[parent].1);
    let (upper, lower) = (rig.bones[elbow].at, rig.bones[hand].at);
    let (first, second) = (upper.length(), lower.length());
    if first <= 0.0 || second <= 0.0 {
        return;
    }
    let to = target - origin;
    let reach = to
        .length()
        .clamp((first - second).abs() + 1e-3, first + second - 1e-3);
    let bend = ((first * first + reach * reach - second * second) / (2.0 * first * reach))
        .clamp(-1.0, 1.0)
        .acos();
    let upper_way = to.to_angle() - bend;
    let elbow_at = origin + Vec2::from_angle(upper_way) * first;
    let lower_way = (target - elbow_at).to_angle();
    let shoulder_turn = upper_way - upper.to_angle();
    let elbow_turn = lower_way - lower.to_angle();
    pose.joints[shoulder].angle = shoulder_turn - above - rig.bones[shoulder].rest;
    pose.joints[elbow].angle = elbow_turn - shoulder_turn - rig.bones[elbow].rest;
}
