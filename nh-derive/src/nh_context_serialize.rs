
use darling::{FromField, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeOpts {
    uuid_type: syn::Path,
}

#[derive(FromField)]
#[darling(attributes(nh_context_serde))]
struct DeriveNHContextSerDeFieldOpts {
    #[darling(default)]
    entity: bool,
    #[darling(default)]
    option: bool,
}

pub fn derive_nh_context_serialize(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let opts = match DeriveNHContextSerDeOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    let uuid_type = opts.uuid_type;
    let syn::Data::Struct(struct_data) = &input_ast.data else {
        return syn::Error::new(
                input_ast.ident.span(),
                "NHContextSerialize can only be derived for structs (you can use NHContextSerDeTag to derive all necessary implementations for enums)",
            )
            .to_compile_error()
            .into();
    };

    let ident = &input_ast.ident;
    let (all_fields_insert, entity_fields_serialize) = struct_data.fields.iter().filter_map(|e|
        DeriveNHContextSerDeFieldOpts::from_field(e).map(|o| {
            let field = &e.ident;
            (
                if o.option {
                    quote! {
                        if let Some(e) = &self.#field {
                            t.insert(stringify!(#field).to_owned(), toml::Value::try_from(e)?);
                        }
                    }
                } else {
                    quote! {
                        t.insert(stringify!(#field).to_owned(), toml::Value::try_from(& self . #field)?);
                    }
                },
                if o.entity {
                    Some(quote! { self . #field . serialize_into(into) ?; })
                } else {
                    None
                }
            )
        }).ok()
    ).collect::<(Vec<_>, Vec<_>)>();

    let output = quote! {
        impl crate::common::project_serde::NHContextSerialize for #ident {
            fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
                // check entity is not yet present
                if into.contains_model(&self.uuid()) {
                    return Ok(());
                }

                // serialize all fields
                let mut t = toml::Table::new();
                #(#all_fields_insert)*
                into.insert_model(*self.uuid(), t);

                // propagate over all marked fields
                #(#entity_fields_serialize)*

                Ok(())
            }
        }
    };
    output.into()
}
