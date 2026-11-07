//! Posing: what plays on a puppet, what the game sets, and the pose it makes.

mod animator;
mod gait;
mod inputs;
mod types;

pub use self::animator::{Animator, Layers};
pub use self::gait::Gait;
pub use self::inputs::{Goal, Goals, Params};
pub use self::types::{Joint, Placement, Pose, Side};

/// What posing a puppet reads besides its clips, gathered.
pub(crate) fn animator_inputs<'a>(
    params: Option<&'a Params>,
    gait: Option<&'a Gait>,
    goals: Option<&'a Goals>,
) -> animator::Inputs<'a> {
    animator::Inputs {
        params,
        gait,
        goals,
    }
}
