//! Derive macro for `FromRow` trait
//!
//! This macro generates the `FromRow` implementation for converting
//! `may_postgres::Row` into a Model struct. It's separate from `LifeModel`
//! to avoid trait bound resolution issues during macro expansion.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::utils;

/// Generate `FromRow` implementation for a Model struct
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(syn::DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "FromRow can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate field extraction code
    let mut from_row_fields: Vec<TokenStream2> = Vec::new();
    for (field_pos, field) in fields.iter().enumerate() {
        let col_idx_lit = proc_macro2::Literal::usize_unsuffixed(field_pos);
        let field_name = match utils::field_ident(field) {
            Ok(i) => i,
            Err(e) => return e.to_compile_error().into(),
        };
        let field_type = &field.ty;

        // Get column name from attribute or use snake_case of field name
        let column_name = field
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("column_name"))
            .and_then(|attr| attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value()))
            .unwrap_or_else(|| {
                // Convert field name to snake_case
                let name = field_name.to_string();
                utils::snake_case(&name)
            });

        let column_name_str = column_name.as_str();

        // Handle unsigned integer types by converting to signed first
        let get_expr = {
            // Check if this is an unsigned integer type
            crate::type_conversion::checked_int_from_row_expr(
                field_type,
                &col_idx_lit,
                column_name_str,
            )
            .unwrap_or_else(|| {
                quote! {
                    lifeguard::__lg_try_get::<#field_type>(row, #col_idx_lit, #column_name_str)?
                }
            })
        };

        from_row_fields.push(quote! {
            #field_name: #get_expr,
        });
    }

    let expanded: TokenStream2 = quote! {
        // Implement FromRow trait for Model
        impl lifeguard::FromRow for #struct_name {
            fn from_row(
                row: &lifeguard::may_postgres::Row,
            ) -> Result<Self, lifeguard::may_postgres::Error> {
                Ok(Self {
                    #(#from_row_fields)*
                })
            }
        }
    };

    TokenStream::from(expanded)
}
