//! `#[derive(DeriveMigrationName)]` for unit structs that implement [`lifeguard::migration::Migration`].

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Generics};

pub fn derive_migration_name(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if let Err(e) = validate_no_generics(&input.generics) {
        return e.to_compile_error().into();
    }
    let ident = &input.ident;
    let struct_err = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Unit => None,
            Fields::Named(f) => Some(syn::Error::new_spanned(
                f,
                "DeriveMigrationName requires a unit struct (e.g. `struct MyMigration;`)",
            )),
            Fields::Unnamed(f) => Some(syn::Error::new_spanned(
                f,
                "DeriveMigrationName requires a unit struct (e.g. `struct MyMigration;`)",
            )),
        },
        Data::Enum(e) => Some(syn::Error::new_spanned(
            e.enum_token,
            "DeriveMigrationName only supports unit structs",
        )),
        Data::Union(u) => Some(syn::Error::new_spanned(
            u.union_token,
            "DeriveMigrationName only supports unit structs",
        )),
    };
    if let Some(e) = struct_err {
        return e.to_compile_error().into();
    }

    let snake = crate::utils::snake_case(&ident.to_string());
    let lit = syn::LitStr::new(&snake, ident.span());

    let expanded = quote! {
        impl #ident {
            /// Stable migration identifier (snake_case of the struct name).
            pub const MIGRATION_NAME: &'static str = #lit;
        }

        impl ::lifeguard::migration::MigrationName for #ident {
            fn migration_name(&self) -> &'static str {
                Self::MIGRATION_NAME
            }
        }
    };
    TokenStream::from(expanded)
}

fn validate_no_generics(generics: &Generics) -> Result<(), syn::Error> {
    if generics.params.is_empty() {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            generics,
            "DeriveMigrationName does not support generic parameters",
        ))
    }
}
