
use proc_macro::{self, TokenStream};
use quote::quote;

pub fn derive_nh_context_serde_tag(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let syn::Data::Enum(data_enum) = &input_ast.data else {
        return syn::Error::new(
            input_ast.ident.span(),
            "NHContextSerDeTag can only be derived for enums",
        )
        .to_compile_error()
        .into();
    };

    let ident = input_ast.ident;

    let (arms_serialize_passthrough, (arms_tag_def, (arms_tag_from, arms_tag_deserialize_referenced))) = data_enum.variants.iter().map(|e| {
        let variant = &e.ident;
        let inner_type = e.fields.iter().next().unwrap_or_else(|| panic!("each variant must have exactly one argument (hint: {}::{})", ident, variant));

        (
            quote! { Self :: #variant ( inner ) => inner.read().serialize_into(into) },
        (
            quote! { #variant ( toml::Value ) },
        (
            quote! { super :: #ident :: #variant (e) => Self :: #variant ( toml::Value::try_from(&e)? ) },
            quote! { Self :: #variant (e) => Ok( super :: #ident :: #variant ( < #inner_type as NHContextDeserialize > :: deserialize (e, deserializer)? ) ) }
        )))
    }).collect::<(Vec<_>, (Vec<_>, (Vec<_>, Vec<_>)))>();

    let tag_mod_name = syn::Ident::new(&format!("{}_context_serde_tag", ident), ident.span());
    let output = quote! {
        mod #tag_mod_name {
            use super::*;
            use serde::ser::Error;
            use crate::common::project_serde::{NHContextSerialize, NHSerializer, NHSerializeError};
            use crate::common::project_serde::{NHContextDeserialize, NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};

            impl serde::Serialize for super :: #ident {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
                    #ident :: try_from (self) . map_err(|e| S::Error::custom(format!("{:?}", e))) ? . serialize ( serializer )
                }
            }

            impl NHContextSerialize for super :: #ident {
                fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
                    match self {
                        #(#arms_serialize_passthrough),*
                    }
                }
            }

            impl NHContextDeserialize for super :: #ident {
                fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
                    let tag: #ident = toml::Value::try_into(source.clone())?;
                    Ok(tag.deserialize_referenced(deserializer)?)
                }
            }

            #[derive(serde::Serialize, serde::Deserialize)]
            pub(super) enum #ident {
                #(#arms_tag_def),*
            }

            impl TryFrom<&super::#ident> for #ident {
                type Error = NHSerializeError;
                fn try_from(e: &super::#ident) -> Result<Self, NHSerializeError> {
                    Ok(
                        match e {
                            #(#arms_tag_from),*
                        }
                    )
                }
            }

            impl #ident {
                pub(super) fn deserialize_referenced(&self, deserializer: &mut NHDeserializer) -> Result<super::#ident, NHDeserializeError> {
                    match self {
                        #(#arms_tag_deserialize_referenced),*
                    }
                }
            }
        }
    };
    output.into()
}
