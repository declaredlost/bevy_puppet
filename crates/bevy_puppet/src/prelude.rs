//! What using puppets takes: `use bevy_puppet::prelude::*`.

#[doc(hidden)]
pub use crate::clip::{ChannelSource, Clamp, Clip, ClipSource, Curve, Driver};
#[doc(hidden)]
pub use crate::label::{GoalLabel, LayerLabel, ParamLabel, RoleLabel};
#[cfg(feature = "ron")]
#[doc(hidden)]
pub use crate::loader::{PuppetLabels, PuppetRonPlugin};
#[doc(hidden)]
pub use crate::pose::{Animator, Gait, Goal, Goals, Params, Placement, Pose, Side};
#[doc(hidden)]
pub use crate::rig::{BoneSource, ConstraintSource, Limit, Rig, RigSource};
#[doc(hidden)]
pub use crate::{Bone, Bones, Puppet, PuppetApp, PuppetPlugin, PuppetSystems, Skeleton};
