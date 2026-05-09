
use darling::{FromField, FromVariant, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(full_text_searchable))]
struct DeriveFullTextSearchableOpts {
    #[darling(default)]
    default_passthrough: String,
    #[darling(default)]
    default_search_kind: String,
}

pub fn derive_full_text_searchable(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input);
    let opts = match DeriveFullTextSearchableOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    match &input_ast.data {
        syn::Data::Enum(e) => derive_enum(&input_ast, &opts, e),
        syn::Data::Struct(s) => derive_struct(&input_ast, &opts, s),
        syn::Data::Union(_u) => return syn::Error::new(
            input_ast.ident.span(),
            "FullTextSearchable cannot be derived for unions",
        )
        .to_compile_error()
        .into(),
    }
}

#[derive(FromVariant)]
#[darling(attributes(full_text_searchable))]
struct DeriveFullTextSearchableVariantOpts {
    passthrough: Option<darling::util::Override<String>>,
}

fn derive_enum(input_ast: &syn::DeriveInput, opts: &DeriveFullTextSearchableOpts, data_enum: &syn::DataEnum) -> TokenStream {
    let (impl_generics, type_generics, where_clause) = input_ast.generics.split_for_impl();

    let arms = data_enum.variants.iter()
        .flat_map(|v| DeriveFullTextSearchableVariantOpts::from_variant(v).map(|e| (v, e))).collect::<Vec<_>>();
    let arms2 = arms.iter()
            .map(|e| (e.0, match &e.1.passthrough {
                Some(darling::util::Override::Explicit(p)) => &p,
                _ => &opts.default_passthrough,
            }))
            .map(|e| {
                let variant_ident = &e.0.ident;
                (quote! { Self::#variant_ident(inner) }, e.1)
            }).collect::<Vec<_>>();
    let arms_immutable = arms2.iter().map(|e| {
        let arm_matcher = &e.0;
        match e.1.as_str() {
            "eref" => quote! { #arm_matcher => inner.read() },
            "bare" | _ => quote! { #arm_matcher => inner },
        }
    }).collect::<Vec<_>>();
    let arms_search = arms_immutable.iter().map(|e| quote! { #e.full_text_search(acc) }).collect::<Vec<_>>();

    let ident = input_ast.ident.clone();

    let output = quote! {
        impl #impl_generics crate::common::search::FullTextSearchable for #ident #type_generics #where_clause {
            fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
                match self {
                    #(#arms_search),*
                }
            }
        }
    };
    output.into()
}

#[derive(FromField)]
#[darling(attributes(full_text_searchable))]
struct DeriveFullTextSearchableFieldOpts {
    #[darling(default)]
    skip: bool,
    #[darling(default)]
    search_kind: Option<String>,
}

fn derive_struct(input_ast: &syn::DeriveInput, opts: &DeriveFullTextSearchableOpts, data_struct: &syn::DataStruct) -> TokenStream {
    let (impl_generics, type_generics, where_clause) = input_ast.generics.split_for_impl();

    let searched_values = data_struct.fields.iter().filter_map(|e|
        DeriveFullTextSearchableFieldOpts::from_field(e).map(|o| {
            let field = &e.ident;
            if o.skip {
                quote! {}
            } else {
                let kind = o.search_kind.as_ref().unwrap_or(&opts.default_search_kind);
                match kind.as_str() {
                    "to_string_ref" => quote! { & self. #field . to_string(), },
                    "as_str_ref" => quote! { & self. #field . as_str(), },
                    "as_ref" | _ => quote! { & self . #field, },
                }
            }
        }).ok()
    ).collect::<Vec<_>>();

    let ident = input_ast.ident.clone();

    let output = quote! {
        impl #impl_generics crate::common::search::FullTextSearchable for #ident #type_generics #where_clause {
            fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
                acc.check_element(
                    *self.uuid,
                    &[
                        #(#searched_values)*
                    ],
                );
            }
        }
    };
    output.into()
}
