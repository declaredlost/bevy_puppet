//! What the game sets on a puppet: the params its clips read, and the goals its
//! constraints reach for.

use std::collections::HashMap;

use bevy::ecs::component::Component;
use bevy::math::Vec2;

use crate::label::{GoalLabel, InternedGoal, InternedParam, ParamLabel};

/// The values the game sets on a puppet that drive its clips.
///
/// An action's progress, how fast it rises
/// ([`Driver::Param`](crate::clip::Driver::Param)). One unset reads as 0.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Params(HashMap<InternedParam, f32>);

/// What a puppet's constraints reach for, by goal: an angle, or a point.
///
/// Where it aims, where a second hand holds on. A constraint whose goal is
/// unset, or of the other kind, does nothing.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Goals(HashMap<InternedGoal, Goal>);

/// A goal's value: an angle to aim along, or a point to reach for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Goal {
    /// An angle from straight ahead, in radians, counterclockwise, as the
    /// puppet faces ([`Side::aim`](super::Side::aim)): for aims and looks.
    Angle(f32),
    /// A point in a bone's frame, in pixels: for reaches.
    Point(Vec2),
}

impl Params {
    /// Sets `param` to `value`, or unsets it.
    pub fn set(&mut self, param: impl ParamLabel, value: Option<f32>) {
        match value {
            Some(value) => {
                self.0.insert(param.intern(), value);
            }
            None => {
                self.0.remove(&param.intern());
            }
        }
    }

    /// `param`'s value: 0 unset.
    #[must_use]
    pub fn get(&self, param: impl ParamLabel) -> f32 {
        self.value(param.intern())
    }

    pub(crate) fn value(&self, param: InternedParam) -> f32 {
        self.0.get(&param).copied().unwrap_or_default()
    }
}

impl Goals {
    /// Sets `goal` to `value`, or unsets it.
    pub fn set(&mut self, goal: impl GoalLabel, value: Option<Goal>) {
        match value {
            Some(value) => {
                self.0.insert(goal.intern(), value);
            }
            None => {
                self.0.remove(&goal.intern());
            }
        }
    }

    /// `goal`'s value, if it's set.
    #[must_use]
    pub fn get(&self, goal: impl GoalLabel) -> Option<Goal> {
        self.value(goal.intern())
    }

    pub(crate) fn value(&self, goal: InternedGoal) -> Option<Goal> {
        self.0.get(&goal).copied()
    }
}
