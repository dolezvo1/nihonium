
use proc_macro::TokenStream;

mod model;
mod container_model;

#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model::derive_model(input)
}

#[proc_macro_derive(ContainerModel, attributes(container_model))]
pub fn derive_container_model(input: TokenStream) -> TokenStream {
    container_model::derive_container_model(input)
}
