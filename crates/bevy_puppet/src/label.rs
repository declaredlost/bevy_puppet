//! The game's labels for its puppets' roles, layers, params and goals.
//!
//! Its own enums, derived (`#[derive(RoleLabel)]`) and interned, as Bevy's
//! labels are, so rigs and clips aren't generic over them.

use bevy::ecs::define_label;
use bevy::ecs::intern::Interned;
pub use bevy_puppet_macros::{GoalLabel, LayerLabel, ParamLabel, RoleLabel};

define_label!(
    /// A job a bone does, any rig's: the reach, the off hand, the head.
    ///
    /// Clips made for any rig move roles; each rig maps them to its bones.
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(RoleLabel)]`"
    )]
    RoleLabel
);

define_label!(
    /// A layer clips play on, overriding or adding to the layers under it.
    ///
    /// Higher layers, in the order the app configures
    /// ([`PuppetApp`](crate::PuppetApp)), override what lower ones set on the
    /// bones they move, or add to it.
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(LayerLabel)]`"
    )]
    LayerLabel
);

define_label!(
    /// A value the game sets on a puppet that drives its clips: a progress.
    ///
    /// An action's progress, how fast it rises
    /// ([`Params`](crate::pose::Params)).
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(ParamLabel)]`"
    )]
    ParamLabel
);

define_label!(
    /// What a rig's constraints reach for: where it aims, where a hand holds.
    ///
    /// Each constraint names its goal; the game sets each goal on a puppet
    /// ([`Goals`](crate::pose::Goals)).
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(GoalLabel)]`"
    )]
    GoalLabel
);

/// A role, interned: what rigs and clips keep, as Bevy keeps labels.
pub type InternedRole = Interned<dyn RoleLabel>;
/// A layer, interned: what clips and puppets keep, as Bevy keeps labels.
pub type InternedLayer = Interned<dyn LayerLabel>;
/// A param, interned: what clips and puppets keep, as Bevy keeps labels.
pub type InternedParam = Interned<dyn ParamLabel>;
/// A goal, interned: what constraints and puppets keep, as Bevy keeps labels.
pub type InternedGoal = Interned<dyn GoalLabel>;
