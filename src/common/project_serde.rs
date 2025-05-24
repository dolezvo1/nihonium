
use std::{any::Any, collections::HashMap, sync::{Arc, RwLock}};

use serde::{Deserialize, Serialize};

use super::controller::DiagramController;

#[derive(Serialize, Deserialize)]
pub enum NHProjectHierarchyNodeDTO {
    Folder(uuid::Uuid, Vec<NHProjectHierarchyNodeDTO>),
    Node(uuid::Uuid, Vec<NHProjectHierarchyNodeDTO>),
    Leaf(uuid::Uuid),
}

#[derive(Serialize, Deserialize)]
pub struct NHProjectDTO {
    format_version: String,
    hierarchy: Vec<NHProjectHierarchyNodeDTO>,
    elements: Vec<toml::Value>,
}

impl NHProjectDTO {
    pub fn new(
        format_version: impl Into<String>,
        hierarchy: Vec<NHProjectHierarchyNodeDTO>,
        mut flattened_elements: Vec<(uuid::Uuid, toml::Table)>,
    ) -> Self {
        flattened_elements.sort_by_key(|e| e.0);
        Self {
            format_version: format_version.into(),
            hierarchy,
            elements: flattened_elements.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
        }
    }
}

pub trait NHSerialize {
    fn serialize_into(&self, into: &mut HashMap<uuid::Uuid, toml::Table>);
}

pub trait NHDeserialize: Sized {
    fn deserialize(
        from: &HashMap<uuid::Uuid, toml::Table>,
        using_elements: &mut HashMap<uuid::Uuid, Arc<dyn Any>>
    ) -> Result<Arc<RwLock<Self>>, ()>;
}

