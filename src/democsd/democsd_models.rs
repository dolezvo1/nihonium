use serde::{Deserialize, Serialize};

use crate::common::canvas;
use crate::common::controller::{ContainerModel, Model};
use crate::common::project_serde::{NHSerialize, NHSerializeError, NHSerializer};
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashSet},
    sync::{Arc, RwLock},
};

pub trait DemoCsdElement: Model + NHSerialize + Send + Sync {}

// ---

pub struct DemoCsdDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,

    pub comment: Arc<String>,
}

impl DemoCsdDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for DemoCsdDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl DemoCsdElement for DemoCsdDiagram {}

impl ContainerModel<dyn DemoCsdElement> for DemoCsdDiagram {
    fn add_element(&mut self, element: Arc<RwLock<dyn DemoCsdElement>>) {
        self.contained_elements.push(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl NHSerialize for DemoCsdDiagram {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("democsd-diagram-model".to_owned()));
        element.insert("name".to_owned(), toml::Value::String((*self.name).clone()));

        for e in &self.contained_elements {
            e.read().unwrap().serialize_into(into)?;
        }
        element.insert("contained_elements".to_owned(),
            toml::Value::Array(self.contained_elements.iter().map(|e| toml::Value::String(e.read().unwrap().uuid().to_string())).collect())
        );

        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

// ---

pub struct DemoCsdPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,

    pub comment: Arc<String>,
}

impl DemoCsdPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for DemoCsdPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl DemoCsdElement for DemoCsdPackage {}

impl ContainerModel<dyn DemoCsdElement> for DemoCsdPackage {
    fn add_element(&mut self, element: Arc<RwLock<dyn DemoCsdElement>>) {
        self.contained_elements.push(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl NHSerialize for DemoCsdPackage {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("democsd-package-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("name".to_owned(), toml::Value::String((*self.name).clone()));

        for e in &self.contained_elements {
            e.read().unwrap().serialize_into(into)?;
        }
        element.insert("contained_elements".to_owned(),
            toml::Value::Array(self.contained_elements.iter().map(|e| toml::Value::String(e.read().unwrap().uuid().to_string())).collect())
        );

        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

// ---

pub struct DemoCsdTransactor {
    pub uuid: Arc<ModelUuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,
    pub internal: bool,
    pub transaction: Option<Arc<RwLock<DemoCsdTransaction>>>,
    pub transaction_selfactivating: bool,

    pub comment: Arc<String>,
}

impl DemoCsdTransactor {
    pub fn new(
        uuid: ModelUuid,
        identifier: String,
        name: String,
        internal: bool,
        transaction: Option<Arc<RwLock<DemoCsdTransaction>>>,
        transaction_selfactivating: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),

            identifier: Arc::new(identifier),
            name: Arc::new(name),
            internal,
            transaction,
            transaction_selfactivating,

            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for DemoCsdTransactor {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl DemoCsdElement for DemoCsdTransactor {}

impl NHSerialize for DemoCsdTransactor {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("democsd-transactor-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("identifier".to_owned(), toml::Value::String((*self.identifier).clone()));
        element.insert("name".to_owned(), toml::Value::String((*self.name).clone()));
        element.insert("internal".to_owned(), toml::Value::Boolean(self.internal));

        if let Some(e) = &self.transaction {
            e.read().unwrap().serialize_into(into)?;
        }
        element.insert("transaction".to_owned(),
            toml::Value::Array(self.transaction.iter().map(|e| toml::Value::String(e.read().unwrap().uuid().to_string())).collect())
        );

        element.insert("transaction_selfactivating".to_owned(), toml::Value::Boolean(self.transaction_selfactivating));
        element.insert("comment".to_owned(), toml::Value::String((*self.comment).clone()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

// ---

pub struct DemoCsdTransaction {
    pub uuid: Arc<ModelUuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,

    pub comment: Arc<String>,
}

impl DemoCsdTransaction {
    pub fn new(uuid: ModelUuid, identifier: String, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),

            identifier: Arc::new(identifier),
            name: Arc::new(name),

            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for DemoCsdTransaction {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl DemoCsdElement for DemoCsdTransaction {}

impl NHSerialize for DemoCsdTransaction {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("democsd-transaction-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("identifier".to_owned(), toml::Value::String((*self.identifier).clone()));
        element.insert("name".to_owned(), toml::Value::String((*self.name).clone()));
        element.insert("comment".to_owned(), toml::Value::String((*self.comment).clone()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

// ---

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DemoCsdLinkType {
    Initiation,
    Interstriction,
    Interimpediment,
}

impl DemoCsdLinkType {
    pub fn char(&self) -> &str {
        match self {
            DemoCsdLinkType::Initiation => "Initiation",
            DemoCsdLinkType::Interstriction => "Interstriction",
            DemoCsdLinkType::Interimpediment => "Interimpediment",
        }
    }

    pub fn line_type(&self) -> canvas::LineType {
        match self {
            DemoCsdLinkType::Initiation => canvas::LineType::Solid,
            DemoCsdLinkType::Interstriction => canvas::LineType::Dashed,
            DemoCsdLinkType::Interimpediment => canvas::LineType::Dashed,
        }
    }
}

pub struct DemoCsdLink {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    pub link_type: DemoCsdLinkType,
    pub source: Arc<RwLock<DemoCsdTransactor>>,
    pub target: Arc<RwLock<DemoCsdTransaction>>,

    pub comment: Arc<String>,
}

impl DemoCsdLink {
    pub fn new(
        uuid: ModelUuid,
        link_type: DemoCsdLinkType,
        source: Arc<RwLock<DemoCsdTransactor>>,
        target: Arc<RwLock<DemoCsdTransaction>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(format!("Link ({})", link_type.char())),
            link_type,
            source,
            target,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for DemoCsdLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl DemoCsdElement for DemoCsdLink {}

impl NHSerialize for DemoCsdLink {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("democsd-transaction-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("name".to_owned(), toml::Value::String((*self.name).clone()));
        element.insert("link_type".to_owned(), toml::Value::try_from(self.link_type)?);
        element.insert("source".to_owned(), toml::Value::String(self.source.read().unwrap().uuid().to_string()));
        element.insert("target".to_owned(), toml::Value::String(self.target.read().unwrap().uuid().to_string()));
        element.insert("comment".to_owned(), toml::Value::String((*self.comment).clone()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}
