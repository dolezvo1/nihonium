use proc_macro::TokenStream;

mod container_model;
mod full_text_searchable;
mod model;
mod nh_context_deserialize;
mod nh_context_serde_tag;
mod nh_context_serialize;
mod unwrap;
mod view;

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

#[proc_macro_derive(FullTextSearchable, attributes(full_text_searchable))]
pub fn derive_full_text_searchable(input: TokenStream) -> TokenStream {
    full_text_searchable::derive_full_text_searchable(input)
}

#[proc_macro_derive(View, attributes(view))]
pub fn derive_view(input: TokenStream) -> TokenStream {
    view::derive_view(input)
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
