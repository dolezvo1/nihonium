
use proc_macro::TokenStream;

mod model;
mod container_model;
mod nh_context_serialize;
mod nh_context_serialize_tag;

#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model::derive_model(input)
}

#[proc_macro_derive(ContainerModel, attributes(container_model))]
pub fn derive_container_model(input: TokenStream) -> TokenStream {
    container_model::derive_container_model(input)
}

#[proc_macro_derive(NHContextSerialize, attributes(nh_context_serialize))]
pub fn derive_nh_context_serialize(input: TokenStream) -> TokenStream {
    nh_context_serialize::derive_nh_context_serialize(input)
}

#[proc_macro_derive(NHContextSerializeTag, attributes(nh_context_serialize_tag))]
pub fn derive_nh_context_serialize_tag(input: TokenStream) -> TokenStream {
    nh_context_serialize_tag::derive_nh_context_serialize_tag(input)
}
