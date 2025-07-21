
use proc_macro::{self, TokenStream};
use quote::quote;

pub fn derive_unwrap(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as syn::DeriveInput);
    let syn::Data::Enum(data_enum) = &input_ast.data else {
        return syn::Error::new(
            input_ast.ident.span(),
            "Unwrap can currently only be derived for enums",
        )
        .to_compile_error()
        .into();
    };
    if data_enum.variants.len() != 1 {
        return syn::Error::new(
            input_ast.ident.span(),
            "Unwrap requires the enum to have precisely one variant",
        )
        .to_compile_error()
        .into();
    }

    let ident = &input_ast.ident;
    let variant = data_enum.variants.iter().next().unwrap();
    let variant_name = &variant.ident;
    let inner_type = &variant.fields.iter().next().unwrap().ty;

    let output = quote! {
        impl #ident {
            pub fn unwrap(self) -> #inner_type {
                match self {
                    Self :: #variant_name (inner) => inner,
                }
            }
        }
    };
    output.into()
}
