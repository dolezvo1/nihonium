
use darling::{FromField, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeOpts {
    uuid_type: Option<syn::Path>,
}

#[derive(FromField)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeFieldOpts {
    #[darling(default)]
    entity: bool,
    #[darling(default)]
    skip_and_default: bool,
    #[darling(default)]
    skip_and_set: Option<syn::Expr>,
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
                if o.skip_and_default || o.skip_and_set.is_some() {
                    quote! {}
                } else {
                    quote! {
                        t.insert(stringify!(#field).to_owned(), toml::Value::try_from(& self . #field) . map_err(|e| format!("field {}: {:?}", stringify!(#field), e))?);
                    }
                },
                if !o.skip_and_default && o.skip_and_set.is_none() && o.entity {
                    Some(quote! { self . #field . serialize_into(into) ?; })
                } else {
                    None
                }
            )
        }).ok()
    ).collect::<(Vec<_>, Vec<_>)>();

    let check_and_insert = if let Some(uuid_type) = &opts.uuid_type {
        quote! {
            // check entity is not yet present
            if <NHSerializer as NHSerializeStore<#uuid_type>>::contains(into, &self.uuid) {
                return Ok(());
            }

            // serialize all fields
            let mut t = toml::Table::new();
            #(#all_fields_insert)*
            <NHSerializer as NHSerializeStore<#uuid_type>>::insert(into, *self.uuid, t);
        }
    } else {
        quote! {}
    };

    let ser_mod_name = syn::Ident::new(&format!("{}_context_serialize", ident), ident.span());
    let output = quote! {
        #[allow(non_snake_case)]
        mod #ser_mod_name {
            use super::*;
            use crate::common::project_serde::{NHContextSerialize, NHSerializer, NHSerializeError, NHSerializeStore};

            impl #impl_generics NHContextSerialize for #ident #type_generics #where_clause {
                fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
                    // check for presence and insert, if applicable
                    #check_and_insert

                    // propagate over all marked fields
                    #(#entity_fields_serialize)*

                    Ok(())
                }
            }
        }
    };
    output.into()
}
