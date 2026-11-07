//! Loaders for rigs (`.rig.ron`) and clips (`.clip.ron`), in the game's labels.
//!
//! `Some` is left out, as the game's data writes it.

use std::hash::Hash;
use std::marker::PhantomData;

use bevy::asset::io::Reader;
use bevy::asset::{AssetApp, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::clip::{Clip, ClipError, ClipSource};
use crate::label::{GoalLabel, LayerLabel, ParamLabel, RoleLabel};
use crate::rig::{Rig, RigError, RigSource};

/// The game's labels, named once for its loaders: roles, layers, params, goals.
///
/// Each deserializable, as the game's files write them.
pub trait PuppetLabels: TypePath + Send + Sync + 'static {
    /// The game's roles.
    type Role: RoleLabel + Hash + Eq + DeserializeOwned;
    /// The game's layers.
    type Layer: LayerLabel + DeserializeOwned;
    /// The game's params.
    type Param: ParamLabel + DeserializeOwned;
    /// The game's goals.
    type Goal: GoalLabel + DeserializeOwned;
}

/// Loads rigs from `.rig.ron` and clips from `.clip.ron`, in `L`'s labels.
pub struct PuppetRonPlugin<L>(PhantomData<fn() -> L>);

/// Loads a [`Rig`] from a `.rig.ron` file: a [`RigSource`] in `L`'s labels.
#[derive(TypePath)]
pub struct RigLoader<L>(PhantomData<fn() -> L>);

/// Loads a [`Clip`] from a `.clip.ron` file: a [`ClipSource`] in `L`'s labels.
#[derive(TypePath)]
pub struct ClipLoader<L>(PhantomData<fn() -> L>);

/// Why a rig or a clip didn't load: unread, not RON, or not a valid one.
#[derive(Debug, Error)]
pub enum LoadError {
    /// It couldn't be read.
    #[error("couldn't read it: {0}")]
    Io(#[from] std::io::Error),
    /// It isn't what it should be, as RON.
    #[error("it isn't valid: {0}")]
    Ron(#[from] ron::error::SpannedError),
    /// It's a rig with something wrong.
    #[error(transparent)]
    Rig(#[from] RigError),
    /// It's a clip with something wrong.
    #[error(transparent)]
    Clip(#[from] ClipError),
}

impl<L> Default for PuppetRonPlugin<L> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<L: PuppetLabels> Plugin for PuppetRonPlugin<L> {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(RigLoader::<L>(PhantomData))
            .register_asset_loader(ClipLoader::<L>(PhantomData));
    }
}

/// `bytes` as RON, `Some` left out.
fn read<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ron::error::SpannedError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_bytes(bytes)
}

impl<L: PuppetLabels> AssetLoader for RigLoader<L> {
    type Asset = Rig;
    type Error = LoadError;
    type Settings = ();

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> Result<Rig, LoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let source: RigSource<L::Role, L::Layer, L::Goal> = read(&bytes)?;
        Ok(Rig::new(&source)?)
    }

    fn extensions(&self) -> &[&str] {
        &["rig.ron"]
    }
}

impl<L: PuppetLabels> AssetLoader for ClipLoader<L> {
    type Asset = Clip;
    type Error = LoadError;
    type Settings = ();

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> Result<Clip, LoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let source: ClipSource<L::Role, L::Layer, L::Param> = read(&bytes)?;
        Ok(Clip::new(&source)?)
    }

    fn extensions(&self) -> &[&str] {
        &["clip.ron"]
    }
}
