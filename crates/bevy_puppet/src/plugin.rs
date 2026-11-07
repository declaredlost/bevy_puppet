//! The plugin keeping puppets, and configuring them on an app.

use bevy::prelude::*;

use crate::clip::Clip;
use crate::label::LayerLabel;
use crate::pose::Layers;
use crate::rig::Rig;
use crate::skeleton;

/// Keeps puppets: builds their skeletons, and poses and places them.
///
/// Builds their skeletons as their rigs load, and poses and places them every
/// frame, before transforms propagate. With `render`, pre-rotates the sprites
/// under bones that turn in steps.
pub struct PuppetPlugin;

/// The system set puppets are posed and placed in, before transforms propagate.
///
/// Set what drives them (what plays, their params, gait, goals and side) before
/// it, and read where their bones are after transforms propagate.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PuppetSystems;

impl Plugin for PuppetPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Rig>()
            .init_asset::<Clip>()
            .init_resource::<Layers>()
            .configure_sets(
                PostUpdate,
                PuppetSystems.before(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                (
                    skeleton::build,
                    skeleton::forget,
                    skeleton::animate,
                    skeleton::place,
                )
                    .chain()
                    .in_set(PuppetSystems),
            );
        #[cfg(feature = "render")]
        crate::render::build(app);
    }
}

/// Configuring puppets on an app: the order its clips' layers play in.
pub trait PuppetApp {
    /// The order clips' layers play in, bottom first: each overrides what those
    /// under it set, or adds to it.
    fn configure_puppet_layers<L: LayerLabel>(
        &mut self,
        layers: impl IntoIterator<Item = L>,
    ) -> &mut Self;
}

impl PuppetApp for App {
    fn configure_puppet_layers<L: LayerLabel>(
        &mut self,
        layers: impl IntoIterator<Item = L>,
    ) -> &mut Self {
        self.insert_resource(Layers::new(layers))
    }
}
