#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

// The label derives name `::bevy_puppet`, here too.
extern crate self as bevy_puppet;

pub mod clip;
pub mod label;
#[cfg(feature = "ron")]
pub mod loader;
mod plugin;
pub mod pose;
pub mod prelude;
#[cfg(feature = "render")]
mod render;
pub mod rig;
mod skeleton;
#[cfg(test)]
mod testing;

pub use self::plugin::{PuppetApp, PuppetPlugin, PuppetSystems};
pub use self::skeleton::{Bone, Bones, Puppet, Skeleton};
