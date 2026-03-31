//! `#[scope]` attribute macro — PRD Phase C derive sugar (named scopes on `Entity`).

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Error, ItemFn};

/// Renames `fn foo` → `fn scope_foo` (or keeps `fn scope_foo` as-is) and defaults to `pub` so
/// scopes read as `Entity::scope_active()` in rustdoc (PRD SC-1).
pub fn scope_attr(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(item as ItemFn);

    if func.sig.asyncness.is_some() {
        return Error::new(
            func.sig.asyncness.span(),
            "`#[scope]` does not support async functions",
        )
        .to_compile_error()
        .into();
    }
    if func.sig.receiver().is_some() {
        return Error::new(
            func.sig.fn_token.span(),
            "`#[scope]` must be on an associated function without `self` (e.g. `fn active() -> impl IntoCondition`)",
        )
        .to_compile_error()
        .into();
    }

    let orig = &func.sig.ident;
    let new_ident = if orig.to_string().starts_with("scope_") {
        orig.clone()
    } else {
        syn::Ident::new(&format!("scope_{orig}"), orig.span())
    };
    func.sig.ident = new_ident;

    if matches!(func.vis, syn::Visibility::Inherited) {
        func.vis = syn::parse_quote!(pub);
    }

    quote!(#func).into()
}
