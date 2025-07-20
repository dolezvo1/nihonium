
use darling::{FromVariant, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

pub fn derive_nh_context_serialize(raw_input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(raw_input as syn::DeriveInput);

    let syn::Data::Struct(s) = &input_ast.data else {
        return syn::Error::new(
                input_ast.ident.span(),
                "NHSerialize can only be derived for structs",
            )
            .to_compile_error()
            .into();
    };

    syn::Error::new(
        input_ast.ident.span(),
        "jk, NHSerialize cannot yet be derived for structs",
    )
    .to_compile_error()
    .into()
}
