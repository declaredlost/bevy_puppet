#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use quote::format_ident;
use syn::{DeriveInput, parse_macro_input};

/// Derives `bevy_puppet::RoleLabel`: a job a rig's bone does.
#[proc_macro_derive(RoleLabel)]
pub fn derive_role_label(input: TokenStream) -> TokenStream {
    label(input, "RoleLabel")
}

/// Derives `bevy_puppet::LayerLabel`: a layer clips play on.
#[proc_macro_derive(LayerLabel)]
pub fn derive_layer_label(input: TokenStream) -> TokenStream {
    label(input, "LayerLabel")
}

/// Derives `bevy_puppet::ParamLabel`: a value the game sets that drives clips.
#[proc_macro_derive(ParamLabel)]
pub fn derive_param_label(input: TokenStream) -> TokenStream {
    label(input, "ParamLabel")
}

/// Derives `bevy_puppet::GoalLabel`: what a constraint reaches for.
#[proc_macro_derive(GoalLabel)]
pub fn derive_goal_label(input: TokenStream) -> TokenStream {
    label(input, "GoalLabel")
}

/// `input`'s implementation of `bevy_puppet`'s label trait `name`, at
/// `::bevy_puppet::label`, as Bevy derives its own labels.
fn label(input: TokenStream, name: &str) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = format_ident!("{name}");
    let path = syn::parse_quote!(::bevy_puppet::label::#ident);
    bevy_macro_utils::derive_label(input, name, &path)
}
