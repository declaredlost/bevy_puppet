#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error, parse_macro_input, parse_quote};

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

/// `input`'s implementation of `bevy_puppet`'s label trait `name`.
///
/// The trait is at `::bevy_puppet::label`; the implementation is the one
/// Bevy derives for its own labels.
fn label(input: TokenStream, name: &str) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if let Data::Union(_) = input.data {
        let message = format!("{name} can't be derived for a union");
        return Error::new_spanned(&input.ident, message)
            .to_compile_error()
            .into();
    }
    let ident = &input.ident;
    let label = format_ident!("{name}");
    let path: syn::Path = parse_quote!(::bevy_puppet::label::#label);
    let (impls, types, wheres) = input.generics.split_for_impl();
    let mut wheres = wheres.cloned().unwrap_or_else(|| parse_quote!(where));
    wheres.predicates.push(parse_quote! {
        Self: 'static
            + ::core::marker::Send
            + ::core::marker::Sync
            + ::core::clone::Clone
            + ::core::cmp::Eq
            + ::core::fmt::Debug
            + ::core::hash::Hash
    });
    quote! {
        const _: () = {
            extern crate alloc;

            impl #impls #path for #ident #types #wheres {
                fn dyn_clone(&self) -> alloc::boxed::Box<dyn #path> {
                    alloc::boxed::Box::new(::core::clone::Clone::clone(self))
                }
            }
        };
    }
    .into()
}
