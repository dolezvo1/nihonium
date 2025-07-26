
use std::{any::Any, collections::HashMap, sync::{RwLock}};

use serde::{Deserialize, Serialize};

use super::entity::EntityUuid;
use super::eref::ERef;
use super::{controller::DiagramController, ufoption::UFOption, uuid::{ModelUuid, ViewUuid}};

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
}

pub trait NHSerializeStore<K> {
    fn contains(&self, uuid: &K) -> bool;
    fn insert(&mut self, uuid: K, data: toml::Table);
}

macro_rules! serialize_store {
    ($uuid_type:ty, $store:ident) => {
        impl NHSerializeStore<$uuid_type> for NHSerializer {
            fn contains(&self, uuid: &$uuid_type) -> bool {
                self.$store.contains_key(uuid)
            }
            fn insert(&mut self, uuid: $uuid_type, data: toml::Table) {
                self.$store.insert(uuid, data);
            }
        }
    };
}

serialize_store!(ModelUuid, models);
serialize_store!(ViewUuid, views);

#[derive(Debug, derive_more::From)]
pub enum NHSerializeError {
    StructureError(String),
    TomlSer(toml::ser::Error),
}

pub trait NHContextSerialize {
    #[clippy::must_use]
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError>;
}

impl<T> NHContextSerialize for UFOption<T> where T: NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            UFOption::None => Ok(()),
            UFOption::Some(e) => e.serialize_into(into),
        }
    }
}

impl<T> NHContextSerialize for Vec<T> where T: NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        for e in self.iter() {
            e.serialize_into(into)?;
        }
        Ok(())
    }
}

impl<T> NHContextSerialize for ERef<T> where T: NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        self.read().serialize_into(into)
    }
}

// TODO: why is the RwLock necessary??
pub struct NHDeserializer {
    source_models: HashMap<ModelUuid, toml::Table>,
    source_views: HashMap<ViewUuid, toml::Table>,
    instantiated_models: RwLock<HashMap<ModelUuid, Box<dyn Any>>>,
    instantiated_views: RwLock<HashMap<ViewUuid, Box<dyn Any>>>,
}

pub trait NHDeserializeInstantiator<K> {
    fn get_entity<T>(&mut self, uuid: &K) -> Result<ERef<T>, NHDeserializeError>
    where T: NHContextDeserialize + 'static;
}

macro_rules! deserialize_instantiator {
    ($uuid_type:ty, $instantiated:ident, $source:ident) => {
        impl NHDeserializeInstantiator<$uuid_type> for NHDeserializer {
            fn get_entity<T>(&mut self, uuid: &$uuid_type) -> Result<ERef<T>, NHDeserializeError>
            where T: NHContextDeserialize + 'static,
            {
                if let Some(e) = self.$instantiated.read().unwrap().get(uuid) {
                    return Ok(e.downcast_ref::<ERef<T>>()
                        .ok_or(NHDeserializeError::StructureError(format!("element has unexpected type: {:?}", uuid)))?
                        .clone());
                }

                let Some(e) = self.$source.get(uuid).cloned().map(|e| toml::Value::Table(e)) else {
                    return Err(NHDeserializeError::StructureError(format!("element not found in source: {:?}", uuid)));
                };

                let e = ERef::new(T::deserialize(&e, self)?);
                self.$instantiated.write().unwrap().insert(*uuid, Box::new(e.clone()));
                Ok(e)
            }
        }
    };
}

deserialize_instantiator!(ModelUuid, instantiated_models, source_models);
deserialize_instantiator!(ViewUuid, instantiated_views, source_views);

#[derive(Debug, derive_more::From)]
pub enum NHDeserializeError {
    StructureError(String),
    UuidError(uuid::Error),
    TomlError(toml::de::Error),
}

pub trait NHContextDeserialize: Sized {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError>;
}

impl<T> NHContextDeserialize for UFOption<T> where T: NHContextDeserialize {
    fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
        #[derive(serde::Deserialize)]
        enum Helper {
            None,
            Some(toml::Value),
        }
        match toml::Value::try_into(source.clone())? {
            Helper::None => Ok(UFOption::None),
            Helper::Some(v) => Ok(UFOption::Some(T::deserialize(&v, deserializer)?)),
        }
    }
}
impl<T> NHContextDeserialize for Vec<T> where T: NHContextDeserialize {
    fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
        source.as_array().ok_or_else(|| NHDeserializeError::StructureError("expected array".into()))?
            .iter().map(|e| T::deserialize(e, deserializer)).collect()
    }
}
impl<T> NHContextDeserialize for ERef<T> where T: NHContextDeserialize + 'static {
    fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
        let uuid = toml::Value::try_into(source.clone())?;
        match uuid {
            EntityUuid::Model(uuid) => deserializer.get_entity(&uuid),
            EntityUuid::View(uuid) => deserializer.get_entity(&uuid),
        }
    }
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

