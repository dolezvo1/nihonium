
use darling::{FromField};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromField)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeFieldOpts {
    #[darling(default)]
    entity: bool,
    #[darling(default)]
    skip_and_default: bool,
}

pub fn derive_nh_context_deserialize(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let syn::Data::Struct(struct_data) = &input_ast.data else {
        return syn::Error::new(
                input_ast.ident.span(),
                "NHContextDeserialize can only be derived for structs (you can use NHContextSerDeTag to derive all necessary implementations for enums)",
            )
            .to_compile_error()
            .into();
    };

    let ident = &input_ast.ident;
    let (impl_generics, type_generics, where_clause) = input_ast.generics.split_for_impl();
    let mut basic_fields_def = Vec::new();
    let mut basic_fields_move = Vec::new();
    let mut default_fields = Vec::new();
    let mut entity_fields_deserialize = Vec::new();

    struct_data.fields.iter().for_each(|e| {
        if let Some(o) = DeriveNHContextSerDeFieldOpts::from_field(e).ok() {
            let field_name = &e.ident;
            let field_type = &e.ty;
            if o.skip_and_default {
                default_fields.push(quote! {
                    #field_name: <#field_type as Default>::default(),
                });
            } else if o.entity {
                entity_fields_deserialize.push(quote! {
                    #field_name: <#field_type as crate::common::project_serde::NHContextDeserialize>::deserialize(
                        source.get(stringify!(#field_name))
                            .ok_or_else(|| NHDeserializeError::StructureError(format!("missing field {} on instance of {}", stringify!(#field_name), stringify!(#ident))))?,
                        deserializer)?,
                });
            } else {
                basic_fields_def.push(quote! { #field_name : #field_type, });
                basic_fields_move.push(quote! { #field_name : helper . #field_name, });
            }
        }
    });

    let des_mod_name = syn::Ident::new(&format!("{}_context_deserialize", ident), ident.span());
    let output = quote! {
        mod #des_mod_name {
            use super::*;
            use crate::common::project_serde::{NHContextDeserialize, NHDeserializer, NHDeserializeError};

            impl #impl_generics NHContextDeserialize for #ident #type_generics #where_clause {
                fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
                    #[derive(serde::Deserialize)]
                    struct Helper {
                        #(#basic_fields_def)*
                    }
                    let helper: Helper = toml::Value::try_into(source.clone())?;

                    Ok(Self {
                        #(#entity_fields_deserialize)*
                        #(#basic_fields_move)*
                        #(#default_fields)*
                    })
                }
            }
        }
    };
    output.into()
}
