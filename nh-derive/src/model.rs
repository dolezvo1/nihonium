
use darling::{FromVariant, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(model))]
struct DeriveModelOpts {
    default_passthrough: String,
}

#[derive(FromVariant)]
#[darling(attributes(model))]
struct DeriveModelVariantOpts {
    passthrough: Option<darling::util::Override<String>>,
}

pub fn derive_model(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input);
    let opts = match DeriveModelOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    let syn::Data::Enum(data_enum) = &input_ast.data else {
        return syn::Error::new(
            input_ast.ident.span(),
            "Model can currently only be derived for enums",
        )
        .to_compile_error()
        .into();
    };

    let arms = data_enum.variants.iter()
        .flat_map(|v| DeriveModelVariantOpts::from_variant(v).map(|e| (v, e))).collect::<Vec<_>>();
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
            "arc_rwlock" => quote! { #arm_matcher => inner.read().unwrap() },
            "bare" | _ => quote! { #arm_matcher => inner },
        }
    }).collect::<Vec<_>>();

    let arms_uuid = arms_immutable.iter().map(|e| quote! { #e.uuid() }).collect::<Vec<_>>();
    let arms_name = arms_immutable.iter().map(|e| quote! { #e.name() }).collect::<Vec<_>>();
    let arms_accept = arms_immutable.iter().map(|e| quote! { #e.accept(v) }).collect::<Vec<_>>();

    let ident = input_ast.ident;

    let output = quote! {
        impl Model for #ident {
            fn uuid(&self) -> Arc<ModelUuid> {
                match self {
                    #(#arms_uuid),*
                }
            }

            fn name(&self) -> Arc<String> {
                match self {
                    #(#arms_name),*
                }
            }

            fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) where Self: Sized {
                match self {
                    #(#arms_accept),*
                }
            }
        }
    };
    output.into()
}
