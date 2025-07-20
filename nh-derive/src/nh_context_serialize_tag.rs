
use darling::{FromVariant, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(nh_context_serialize_tag))]
struct DeriveNHContextSerializeTagOpts {
    uuid_type: syn::Path,
}

pub fn derive_nh_context_serialize_tag(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let opts = match DeriveNHContextSerializeTagOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    let uuid_type = opts.uuid_type;
    let syn::Data::Enum(data_enum) = &input_ast.data else {
        return syn::Error::new(
            input_ast.ident.span(),
            "ContextSerializeTag can only be derived for enums",
        )
        .to_compile_error()
        .into();
    };

    let ident = input_ast.ident;

    let (arms_def, (arms_from, arms_deserialize_referenced)) = data_enum.variants.iter().map(|e| {
        let variant = &e.ident;
        let inner_type = e.fields.iter().next().unwrap_or_else(|| panic!("each variant must have exactly one argument (hint: {}::{})", ident, variant));

        (
            quote! { #variant ( #uuid_type ) },
            (
                quote! { super :: #ident :: #variant (..) => Self :: #variant ( *e.uuid() ) },
                quote! { Self :: #variant (e) => Ok( super :: #ident :: #variant (<NHDeserializer as NHDeserializeInstantiator< #uuid_type >>::get::< #inner_type >(deserializer, e)?) ) }
            )
        )
    }).collect::<(Vec<_>, (Vec<_>, Vec<_>))>();

    let mod_name = syn::Ident::new(&format!("{}_complex_serialize_tag", ident), ident.span());
    let output = quote! {
        impl serde::Serialize for #ident {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
                #mod_name :: #ident :: from (self) . serialize ( serializer )
            }
        }

        impl crate::common::project_serde::NHContextDeserialize for #ident {
            fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
                let tag: #mod_name :: #ident = toml::Value::try_into(source.clone())?;
                let value = tag.deserialize_referenced(deserializer)?;
                Ok(value)
            }
        }

        mod #mod_name {
            use super::*;
            use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};

            #[derive(serde::Serialize, serde::Deserialize)]
            pub enum #ident {
                #(#arms_def),*
            }

            impl From<&super::#ident> for #ident {
                fn from(e: &super::#ident) -> Self {
                    match e {
                        #(#arms_from),*
                    }
                }
            }

            impl #ident {
                pub fn deserialize_referenced(&self, deserializer: &mut NHDeserializer) -> Result<super::#ident, NHDeserializeError> {
                    match self {
                        #(#arms_deserialize_referenced),*
                    }
                }
            }
        }
    };
    output.into()
}
