
use darling::{FromVariant, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(container_model))]
struct DeriveContainerModelOpts {
    element_type: syn::Path,
    default_passthrough: String,
}

#[derive(FromVariant)]
#[darling(attributes(container_model))]
struct DeriveContainerModelVariantOpts {
    passthrough: Option<darling::util::Override<String>>,
}

pub fn derive_container_model(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input);
    let opts = match DeriveContainerModelOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    let syn::Data::Enum(data_enum) = &input_ast.data else {
        return syn::Error::new(
            input_ast.ident.span(),
            "ContainerModel can only be derived on enums",
        )
        .to_compile_error()
        .into();
    };

    let arms = data_enum.variants.iter()
        .flat_map(|v| DeriveContainerModelVariantOpts::from_variant(v).map(|e| (v, e))).collect::<Vec<_>>();
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
            "none" => Err(quote! { #arm_matcher }),
            "eref" => Ok(quote! { #arm_matcher => inner.read() }),
            "bare" | _ => Ok(quote! { #arm_matcher => inner }),
        }
    }).collect::<Vec<_>>();
    let arms_mutable = arms2.iter().map(|e| {
        let arm_matcher = &e.0;
        match e.1.as_str() {
            "none" => Err(quote! { #arm_matcher }),
            "eref" => Ok(quote! { #arm_matcher => inner.write() }),
            "bare" | _ => Ok(quote! { #arm_matcher => inner }),
        }
    }).collect::<Vec<_>>();

    let arms_find_element = arms_immutable.iter().map(|e| match e {
        Ok(e) => quote! { #e.find_element(uuid) },
        Err(e) => quote! { #e => None },
    }).collect::<Vec<_>>();
    let arms_add_element = arms_mutable.iter().map(|e| match e {
        Ok(e) => quote! { #e.add_element(element) },
        Err(e) => quote! { #e => Err(element) },
    }).collect::<Vec<_>>();
    let arms_delete_elements = arms_mutable.iter().map(|e| match e {
        Ok(e) => quote! { #e.delete_elements(uuids) },
        Err(e) => quote! { #e => Err(()) },
    }).collect::<Vec<_>>();

    let ident = input_ast.ident;
    let element_type = opts.element_type;

    let output = quote! {
        impl ContainerModel for #ident {
            type ElementT = #element_type;

            fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
                match self {
                    #(#arms_find_element),*
                }
            }

            fn add_element(&mut self, element: Self::ElementT) -> Result<(), Self::ElementT> {
                match self {
                    #(#arms_add_element),*
                }
            }

            fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
                match self {
                    #(#arms_delete_elements),*
                }
            }
        }
    };
    output.into()
}
