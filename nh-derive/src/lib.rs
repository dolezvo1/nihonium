
use proc_macro::TokenStream;

mod unwrap;
mod model;
mod container_model;
mod nh_context_serialize;
mod nh_context_deserialize;
mod nh_context_serde_tag;

#[proc_macro_derive(Unwrap)]
pub fn derive_unwrap(input: TokenStream) -> TokenStream {
    unwrap::derive_unwrap(input)
}

#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model::derive_model(input)
}

#[proc_macro_derive(ContainerModel, attributes(container_model))]
pub fn derive_container_model(input: TokenStream) -> TokenStream {
    container_model::derive_container_model(input)
}

#[proc_macro_derive(NHContextSerialize, attributes(nh_context_serde))]
pub fn derive_nh_context_serialize(input: TokenStream) -> TokenStream {
    nh_context_serialize::derive_nh_context_serialize(input)
}

#[proc_macro_derive(NHContextDeserialize, attributes(nh_context_serde))]
pub fn derive_nh_context_deserialize(input: TokenStream) -> TokenStream {
    nh_context_deserialize::derive_nh_context_deserialize(input)
}

#[proc_macro_derive(NHContextSerDeTag, attributes(nh_context_serde))]
pub fn derive_nh_context_serde_tag(input: TokenStream) -> TokenStream {
    nh_context_serde_tag::derive_nh_context_serde_tag(input)
}
