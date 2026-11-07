//! What this crate's tests share: a game's labels, and its data read as a game
//! would.

use bevy::asset::{AssetId, Assets, Handle};
use serde::Deserialize;

use crate::clip::{Clip, ClipSource};
use crate::label::{GoalLabel, LayerLabel, ParamLabel, RoleLabel};
use crate::pose::{Animator, Gait, Goals, Layers, Params, Pose, animator_inputs};
use crate::rig::{Rig, RigError, RigSource};

/// The tests' roles.
#[derive(RoleLabel, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Job {
    Reach,
    Grip,
    Off,
    Body,
}

/// The tests' layers, bottom first.
#[derive(LayerLabel, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Tier {
    Move,
    Carry,
    Act,
    React,
    Hold,
}

/// The tests' params.
#[derive(ParamLabel, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Knob {
    Progress,
    Rise,
}

/// The tests' goals.
#[derive(GoalLabel, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Mark {
    Aim,
    Gaze,
    Hold,
}

/// `source` read as a game's data would be: `Some` left out.
pub(crate) fn de<T: for<'de> Deserialize<'de>>(source: &str) -> T {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .unwrap()
}

/// The rig `source` writes.
pub(crate) fn rig(source: &str) -> Result<Rig, RigError> {
    let source: RigSource<Job, Tier, Mark> = de(source);
    Rig::new(&source)
}

/// The clip `source` writes.
pub(crate) fn clip(source: &str) -> Clip {
    let source: ClipSource<Job, Tier, Knob> = de(source);
    Clip::new(&source).unwrap()
}

/// The order the tests' layers play in.
pub(crate) fn layers() -> Layers {
    Layers::new([Tier::Move, Tier::Carry, Tier::Act, Tier::React, Tier::Hold])
}

/// A rig and the clips played on it, posed as the plugin would.
pub(crate) struct Stage {
    pub(crate) rig: Rig,
    clips: Assets<Clip>,
}

impl Stage {
    pub(crate) fn new(rig: Rig) -> Self {
        Self {
            rig,
            clips: Assets::default(),
        }
    }

    /// The clip `source` writes, to play.
    pub(crate) fn clip(&mut self, source: &str) -> Handle<Clip> {
        self.clips.add(clip(source))
    }

    /// `animator`'s pose, `delta` seconds on.
    pub(crate) fn pose(
        &self,
        animator: &mut Animator,
        params: Option<&Params>,
        gait: Option<&Gait>,
        goals: Option<&Goals>,
        delta: f32,
    ) -> Pose {
        animator.pose(
            &self.rig,
            AssetId::default(),
            &self.clips,
            &layers(),
            animator_inputs(params, gait, goals),
            delta,
        )
    }

    /// The pose with nothing playing and only `goals` set.
    pub(crate) fn aimed(&self, goals: &Goals) -> Pose {
        self.pose(&mut Animator::default(), None, None, Some(goals), 0.01)
    }

    pub(crate) fn index(&self, name: &str) -> usize {
        self.rig.index(name).unwrap()
    }
}

/// Whether `angle`, in radians, is `degrees`.
pub(crate) fn near(angle: f32, degrees: f32) -> bool {
    (angle - degrees.to_radians()).abs() < 1e-4
}
