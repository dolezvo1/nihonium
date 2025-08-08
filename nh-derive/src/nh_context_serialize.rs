
use darling::{FromField, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeOpts {
    #[darling(default)]
    is_entity: bool,
    #[darling(default)]
    is_subset_with: Option<Option<syn::Path>>,
    #[expect(dead_code)]
    #[darling(default)]
    initialize_with: Option<syn::Path>,
}

#[derive(FromField)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeFieldOpts {
    #[darling(default)]
    entity: bool,
    #[darling(default)]
    skip_and_default: bool,
}

pub fn derive_nh_context_serialize(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let opts = match DeriveNHContextSerDeOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    let syn::Data::Struct(struct_data) = &input_ast.data else {
        return syn::Error::new(
                input_ast.ident.span(),
                "NHContextSerialize can only be derived for structs (you can use NHContextSerDeTag to derive all necessary implementations for enums)",
            )
            .to_compile_error()
            .into();
    };

    let ident = &input_ast.ident;
    let (impl_generics, type_generics, where_clause) = input_ast.generics.split_for_impl();
    let (all_fields_insert, entity_fields_serialize) = struct_data.fields.iter().filter_map(|e|
        DeriveNHContextSerDeFieldOpts::from_field(e).map(|o| {
            let field = &e.ident;
            (
                if o.skip_and_default {
                    quote! {}
                } else {
                    quote! {
                        t.insert(stringify!(#field).to_owned(), toml::Value::try_from(& self . #field) . map_err(|e| format!("field {}: {:?}", stringify!(#field), e))?);
                    }
                },
                if !o.skip_and_default && o.entity {
                    Some(quote! { self . #field . serialize_into(into) ?; })
                } else {
                    None
                }
            )
        }).ok()
    ).collect::<(Vec<_>, Vec<_>)>();

    let (subset_open, subset_close) = if let Some(depends_on_fn) = &opts.is_subset_with {
        (
            quote! {
                into.open_new_subset(self.tagged_uuid(), #depends_on_fn(self));
            },
            quote! {
                into.close_last_subset();
            },
        )
    } else {
        (quote! {}, quote! {})
    };

    let check_and_insert = if opts.is_entity {
        quote! {
            // check entity is not yet present
            if into.contains(&self.tagged_uuid()) {
                return Ok(());
            }

            #subset_open

            // serialize all fields
            let mut t = toml::Table::new();
            #(#all_fields_insert)*
            into.insert(self.tagged_uuid(), t);
        }
    } else {
        quote! {
            #subset_open
        }
    };

    let ser_mod_name = syn::Ident::new(&format!("{}_context_serialize", ident), ident.span());
    let output = quote! {
        #[allow(non_snake_case)]
        mod #ser_mod_name {
            use super::*;
            use crate::common::project_serde::{NHContextSerialize, NHSerializer, NHSerializeError};

            impl #impl_generics NHContextSerialize for #ident #type_generics #where_clause {
                fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
                    // check for presence and insert, if applicable
                    #check_and_insert

                    // propagate over all marked fields
                    #(#entity_fields_serialize)*

                    #subset_close

                    Ok(())
                }
            }
        }
    };
    output.into()
}
