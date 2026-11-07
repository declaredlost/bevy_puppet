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

/// A number no rig or clip has had: what tells one made apart from the one it
/// replaces.
pub(crate) fn revision() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
pub use self::skeleton::{Bone, Bones, Puppet, Skeleton};
