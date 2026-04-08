//! `#[scope_bundle]` attribute macro — AND-combine multiple `#[scope]` helpers (PRD Phase C).

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Error, Ident, ItemFn, Token};

struct ScopeBundleArgs {
    idents: syn::punctuated::Punctuated<Ident, Token![,]>,
}

impl Parse for ScopeBundleArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            idents: syn::punctuated::Punctuated::parse_terminated(input)?,
        })
    }
}

fn resolve_scope_expr(ident: &Ident) -> proc_macro2::TokenStream {
    let name = ident.to_string();
    if name.starts_with("scope_") {
        let i = ident.clone();
        quote::quote!(Self::#i().into_condition())
    } else {
        let scope_name = format!("scope_{name}");
        let scope_ident = Ident::new(&scope_name, ident.span());
        quote::quote!(Self::#scope_ident().into_condition())
    }
}

/// Generates `pub fn scope_<name>() -> sea_query::Condition` by `ANDing` the listed scopes.
pub fn scope_bundle_attr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ScopeBundleArgs);
    if args.idents.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[scope_bundle]` requires at least one scope identifier, e.g. `#[scope_bundle(active, published)]`",
        )
        .to_compile_error()
        .into();
    }

    let mut func = parse_macro_input!(item as ItemFn);

    if func.sig.asyncness.is_some() {
        return Error::new(
            func.sig.asyncness.span(),
            "`#[scope_bundle]` does not support async functions",
        )
        .to_compile_error()
        .into();
    }
    if func.sig.receiver().is_some() {
        return Error::new(
            func.sig.fn_token.span(),
            "`#[scope_bundle]` must be on an associated function without `self`",
        )
        .to_compile_error()
        .into();
    }

    let orig = &func.sig.ident;
    let new_ident = if orig.to_string().starts_with("scope_") {
        orig.clone()
    } else {
        Ident::new(&format!("scope_{orig}"), orig.span())
    };

    let pieces: Vec<proc_macro2::TokenStream> =
        args.idents.iter().map(resolve_scope_expr).collect();

    let first = &pieces[0];
    let rest = &pieces[1..];

    let body = if rest.is_empty() {
        quote!(sea_query::Condition::all().add(#first))
    } else {
        let mut stmts = quote! {
            let mut __lg_c = sea_query::Condition::all().add(#first);
        };
        for r in rest {
            stmts.extend(quote! { __lg_c = __lg_c.add(#r); });
        }
        stmts.extend(quote! { __lg_c });
        quote!({ #stmts })
    };

    func.sig.ident = new_ident;
    func.sig.generics = syn::Generics::default();
    func.sig.output = syn::parse_quote!(-> sea_query::Condition);
    func.block = syn::parse_quote!({ #body });

    if matches!(func.vis, syn::Visibility::Inherited) {
        func.vis = syn::parse_quote!(pub);
    }

    quote!(#func).into()
}
