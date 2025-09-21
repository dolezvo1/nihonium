pub mod rdf_controllers;
pub mod rdf_models;

#[cfg(not(target_arch = "wasm32"))]
pub mod rdf_queries;
