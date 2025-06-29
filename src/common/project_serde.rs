
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
    models: Vec<toml::Value>,
    views: Vec<toml::Value>,
}

impl NHProjectDTO {
    pub fn new(
        hierarchy: Vec<NHProjectHierarchyNodeDTO>,
        serializer: NHSerializer,
    ) -> Self {
        let (models, views) = {
            let NHSerializer { models, views } = serializer;
            let (mut m, mut v): (Vec<_>, Vec<_>) = (models.into_iter().collect(), views.into_iter().collect());
            m.sort_by_key(|e| e.0);
            v.sort_by_key(|e| e.0);
            (m, v)
        };
        Self {
            format_version: "0.1.0".into(),
            hierarchy,
            models: models.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
            views: views.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
        }
    }

    pub fn deserializer(&self) -> Result<NHDeserializer, NHDeserializeError> {

        let source_models = self.models.iter().map(|v| {
            let toml::Value::Table(t) = v else {
                return Err(NHDeserializeError::StructureError(format!("expected table, found {:?}", v)));
            };
            Ok((get_model_uuid(t)?, t.clone()))
        }).collect::<Result<HashMap<_, _>, _>>()?;
        let source_views = self.views.iter().map(|v| {
            let toml::Value::Table(t) = v else {
                return Err(NHDeserializeError::StructureError(format!("expected table, found {:?}", v)));
            };
            Ok((get_view_uuid(t)?, t.clone()))
        }).collect::<Result<HashMap<_, _>, _>>()?;

        Ok(NHDeserializer {
            source_models,
            source_views,
            instantiated_models: HashMap::new().into(),
            instantiated_views: HashMap::new().into(),
        })
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

#[derive(Debug, derive_more::From)]
pub enum NHSerializeError {
    StructureError(String),
    TomlSer(toml::ser::Error),
}

pub trait NHSerialize {
    #[clippy::must_use]
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError>;
}

pub trait NHSerializeToScalar {
    #[clippy::must_use]
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<toml::Value, NHSerializeError>;
}

pub struct NHDeserializer {
    source_models: HashMap<ModelUuid, toml::Table>,
    source_views: HashMap<ViewUuid, toml::Table>,
    instantiated_models: RwLock<HashMap<ModelUuid, Arc<dyn Any>>>,
    instantiated_views: RwLock<HashMap<ViewUuid, Arc<dyn Any>>>,
}

impl NHDeserializer {
    pub fn get_or_instantiate_model<T>(&self, uuid: &ModelUuid) -> Result<Arc<RwLock<T>>, NHDeserializeError>
    where T: NHDeserializeEntity + 'static,
    {
        if let Some(m) = self.instantiated_models.read().unwrap().get(uuid) {
            return Ok(m.downcast_ref::<Arc<RwLock<T>>>()
                .ok_or(NHDeserializeError::StructureError(format!("model has unexpected type: {:?}", uuid)))?
                .clone());
        }

        let Some(t) = self.source_models.get(uuid) else {
            return Err(NHDeserializeError::StructureError(format!("Model not found in source: {:?}", uuid)));
        };

        let m = T::deserialize(t, self)?;
        self.instantiated_models.write().unwrap().insert(*uuid, m.clone());
        Ok(m)
    }
}

#[derive(Debug, derive_more::From)]
pub enum NHDeserializeError {
    StructureError(String),
    UuidError(uuid::Error),
}

pub trait NHDeserializeScalar: Sized {
    fn deserialize(
        source: &toml::Value,
        deserializer: &NHDeserializer,
    ) -> Result<Self, NHDeserializeError>;
}

pub trait NHDeserializeEntity: Sized {
    fn deserialize(
        source: &toml::Table,
        deserializer: &NHDeserializer,
    ) -> Result<Arc<RwLock<Self>>, NHDeserializeError>;
}

pub fn get_model_uuid(table: &toml::Table) -> Result<ModelUuid, NHDeserializeError> {
    let v = table.get("uuid").ok_or_else(|| NHDeserializeError::StructureError(format!("missing model uuid: {:?}", table)))?;
    let toml::Value::String(s) = v else {
        return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
    };
    Ok(uuid::Uuid::parse_str(s)?.into())
}

pub fn get_view_uuid(table: &toml::Table) -> Result<ViewUuid, NHDeserializeError> {
    let v = table.get("uuid").ok_or_else(|| NHDeserializeError::StructureError(format!("missing view uuid {:?}", table)))?;
    let toml::Value::String(s) = v else {
        return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
    };
    Ok(uuid::Uuid::parse_str(s)?.into())
}

