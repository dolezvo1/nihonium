
use std::{any::Any, collections::HashMap, sync::{Arc, RwLock}};

use serde::{Deserialize, Serialize};

use super::{controller::DiagramController, uuid::{ModelUuid, ViewUuid}};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NHProjectHierarchyNodeDTO {
    Folder { uuid: ViewUuid, hierarchy: Vec<NHProjectHierarchyNodeDTO> },
    Diagram { uuid: ViewUuid },
}

#[derive(Serialize, Deserialize)]
pub struct NHProjectDTO {
    format_version: String,
    hierarchy: Vec<NHProjectHierarchyNodeDTO>,
    flattened_models: Vec<toml::Value>,
    flattened_views: Vec<toml::Value>,
}

impl NHProjectDTO {
    pub fn new(
        hierarchy: Vec<NHProjectHierarchyNodeDTO>,
        serializer: NHSerializer,
    ) -> Self {
        let (flattened_models, flattened_views) = {
            let NHSerializer { models, views } = serializer;
            let (mut m, mut v): (Vec<_>, Vec<_>) = (models.into_iter().collect(), views.into_iter().collect());
            m.sort_by_key(|e| e.0);
            v.sort_by_key(|e| e.0);
            (m, v)
        };
        Self {
            format_version: "0.1.0".into(),
            hierarchy,
            flattened_models: flattened_models.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
            flattened_views: flattened_views.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
        }
    }
}

pub struct NHSerializer {
    models: HashMap<ModelUuid, toml::Table>,
    views: HashMap<ViewUuid, toml::Table>,
}

impl NHSerializer {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            views: HashMap::new(),
        }
    }

    pub fn contains_model(&self, uuid: &ModelUuid) -> bool {
        self.models.contains_key(uuid)
    }
    pub fn insert_model(&mut self, uuid: ModelUuid, data: toml::Table) {
        self.models.insert(uuid, data);
    }
    pub fn contains_view(&self, uuid: &ViewUuid) -> bool {
        self.views.contains_key(uuid)
    }
    pub fn insert_view(&mut self, uuid: ViewUuid, data: toml::Table) {
        self.views.insert(uuid, data);
    }
}

#[derive(Debug)]
pub enum NHSerializeError {
    StructureError(String),
    TomlSer(toml::ser::Error),
}

impl From<toml::ser::Error> for NHSerializeError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}

pub trait NHSerialize {
    #[clippy::must_use]
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError>;
}

pub trait NHDeserialize: Sized {
    fn deserialize(
        from: &HashMap<uuid::Uuid, toml::Table>,
        using_elements: &mut HashMap<uuid::Uuid, Arc<dyn Any>>
    ) -> Result<Arc<RwLock<Self>>, ()>;
}

