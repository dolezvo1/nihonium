use serde::{Deserialize, Serialize};

use crate::common::canvas;
use crate::common::controller::{ContainerModel, Model, StructuralVisitor};
use crate::common::project_serde::{NHDeserializeEntity, NHDeserializeError, NHDeserializer, NHSerialize, NHSerializeError, NHSerializer};
use crate::common::uuid::ModelUuid;
use std::collections::HashMap;
use std::{
    collections::{HashSet},
    sync::{Arc, RwLock},
};

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel)]
#[model(default_passthrough = "arc_rwlock")]
#[container_model(element_type = DemoCsdElement, default_passthrough = "none")]
pub enum DemoCsdElement {
    #[container_model(passthrough = "arc_rwlock")]
    DemoCsdPackage(Arc<RwLock<DemoCsdPackage>>),
    #[container_model(passthrough = "arc_rwlock")]
    DemoCsdTransactor(Arc<RwLock<DemoCsdTransactor>>),
    DemoCsdTransaction(Arc<RwLock<DemoCsdTransaction>>),
    DemoCsdLink(Arc<RwLock<DemoCsdLink>>),
}

impl NHSerialize for DemoCsdElement {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            DemoCsdElement::DemoCsdPackage(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            DemoCsdElement::DemoCsdTransactor(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            DemoCsdElement::DemoCsdTransaction(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            DemoCsdElement::DemoCsdLink(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
        }
    }
}

pub fn deep_copy_diagram(d: &DemoCsdDiagram) -> (Arc<RwLock<DemoCsdDiagram>>, HashMap<ModelUuid, DemoCsdElement>) {
    fn walk(e: &DemoCsdElement, into: &mut HashMap<ModelUuid, DemoCsdElement>) -> DemoCsdElement {
        let new_uuid = Arc::new(uuid::Uuid::now_v7().into());
        match e {
            DemoCsdElement::DemoCsdPackage(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = DemoCsdPackage {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                DemoCsdElement::DemoCsdPackage(Arc::new(RwLock::new(new_model)))
            },
            DemoCsdElement::DemoCsdTransactor(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_tx = if let Some(tx) = &model.transaction {
                    let new_tx = walk(&DemoCsdElement::DemoCsdTransaction(tx.clone()), into);
                    into.insert(*tx.read().unwrap().uuid(), new_tx.clone());
                    if let DemoCsdElement::DemoCsdTransaction(new_tx) = new_tx {
                        Some(new_tx)
                    } else {
                        None
                    }
                } else { None };
                let new_model = DemoCsdTransactor {
                    uuid: new_uuid,
                    identifier: model.identifier.clone(),
                    name: model.name.clone(),
                    internal: model.internal,
                    transaction: new_tx,
                    transaction_selfactivating: model.transaction_selfactivating,
                    comment: model.comment.clone()
                };
                DemoCsdElement::DemoCsdTransactor(Arc::new(RwLock::new(new_model)))
            },
            DemoCsdElement::DemoCsdTransaction(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = DemoCsdTransaction {
                    uuid: new_uuid,
                    identifier: model.identifier.clone(),
                    name: model.name.clone(),
                    comment: model.comment.clone(),
                };
                DemoCsdElement::DemoCsdTransaction(Arc::new(RwLock::new(new_model)))
            },
            DemoCsdElement::DemoCsdLink(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = DemoCsdLink {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    link_type: model.link_type,
                    source: model.source.clone(),
                    target: model.target.clone(),
                    comment: model.comment.clone(),
                };
                DemoCsdElement::DemoCsdLink(Arc::new(RwLock::new(new_model)))
            },
        }
    }

    fn relink(e: &mut DemoCsdElement, all_models: &HashMap<ModelUuid, DemoCsdElement>) {
        match e {
            DemoCsdElement::DemoCsdPackage(rw_lock) => {
                let mut model = rw_lock.write().unwrap();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            DemoCsdElement::DemoCsdTransactor(rw_lock) => {
                let mut model = rw_lock.write().unwrap();
                if let Some(ta) = &mut model.transaction {
                    relink(&mut DemoCsdElement::DemoCsdTransaction(ta.clone()), all_models);
                }
            },
            DemoCsdElement::DemoCsdTransaction(rw_lock) => {},
            DemoCsdElement::DemoCsdLink(rw_lock) => {
                let mut model = rw_lock.write().unwrap();

                let source_uuid = *model.source.read().unwrap().uuid;
                if let Some(DemoCsdElement::DemoCsdTransactor(ta)) = all_models.get(&source_uuid) {
                    model.source = ta.clone();
                }
                let target_uuid = *model.target.read().unwrap().uuid;
                if let Some(DemoCsdElement::DemoCsdTransaction(tx)) = all_models.get(&target_uuid) {
                    model.target = tx.clone();
                }
            },
        }
    }

    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        let new_model = walk(&e, &mut all_models);
        all_models.insert(*e.uuid(), new_model.clone());
        new_contained_elements.push(new_model);
    }
    for e in new_contained_elements.iter_mut() {
        relink(e, &all_models);
    }

    let new_diagram = DemoCsdDiagram {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (Arc::new(RwLock::new(new_diagram)), all_models)
}

pub fn fake_copy_diagram(d: &DemoCsdDiagram) -> HashMap<ModelUuid, DemoCsdElement> {
    fn walk(e: &DemoCsdElement, into: &mut HashMap<ModelUuid, DemoCsdElement>) {
        match e {
            DemoCsdElement::DemoCsdPackage(rw_lock) => {
                let model = rw_lock.read().unwrap();

                for e in &model.contained_elements {
                    walk(e, into);
                    into.insert(*e.uuid(), e.clone());
                }
            },
            DemoCsdElement::DemoCsdTransactor(rw_lock) => {
                let model = rw_lock.read().unwrap();

                if let Some(tx) = &model.transaction {
                    walk(&DemoCsdElement::DemoCsdTransaction(tx.clone()), into);
                    into.insert(*tx.read().unwrap().uuid(), DemoCsdElement::DemoCsdTransaction(tx.clone()));
                }
            },
            DemoCsdElement::DemoCsdTransaction(rw_lock) => {},
            DemoCsdElement::DemoCsdLink(rw_lock) => {},
        }
    }

    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        walk(e, &mut all_models);
        all_models.insert(*e.uuid(), e.clone());
    }

    all_models
}

// ---

pub struct DemoCsdDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<DemoCsdElement>,

    pub comment: Arc<String>,
}

impl DemoCsdDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<DemoCsdElement>,
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
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) {
        v.open_complex(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_complex(self);
    }
}

impl ContainerModel for DemoCsdDiagram {
    type ElementT = DemoCsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoCsdElement, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }
    fn add_element(&mut self, element: DemoCsdElement) -> Result<(), DemoCsdElement> {
        self.contained_elements.push(element);
        Ok(())
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
        Ok(())
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
        element.insert("comment".to_owned(), toml::Value::String((*self.name).clone()));

        for e in &self.contained_elements {
            e.serialize_into(into)?;
        }
        element.insert("contained_elements".to_owned(),
            toml::Value::Array(self.contained_elements.iter().map(|e| toml::Value::String(e.uuid().to_string())).collect())
        );

        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

impl NHDeserializeEntity for DemoCsdDiagram {
    fn deserialize(
        source: &toml::Table,
        deserializer: &NHDeserializer,
    ) -> Result<Arc<RwLock<Self>>, NHDeserializeError> {
        let uuid = {
            let v = source.get("uuid").ok_or_else(|| NHDeserializeError::StructureError(format!("missing uuid")))?;
            let toml::Value::String(s) = v else {
                return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
            };
            Arc::new(uuid::Uuid::parse_str(s)?.into())
        };
        let name = {
            let v = source.get("name").ok_or_else(|| NHDeserializeError::StructureError(format!("missing name")))?;
            let toml::Value::String(s) = v else {
                return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
            };
            Arc::new(s.clone())
        };
        let comment = {
            let v = source.get("comment").ok_or_else(|| NHDeserializeError::StructureError(format!("missing comment")))?;
            let toml::Value::String(s) = v else {
                return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
            };
            Arc::new(s.clone())
        };

        Ok(Arc::new(RwLock::new(Self { uuid, name, contained_elements: Vec::new(), comment })))
    }
}

// ---

pub struct DemoCsdPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<DemoCsdElement>,

    pub comment: Arc<String>,
}

impl DemoCsdPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<DemoCsdElement>,
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
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) {
        v.open_complex(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_complex(self);
    }
}

impl ContainerModel for DemoCsdPackage {
    type ElementT = DemoCsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoCsdElement, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }
    fn add_element(&mut self, element: DemoCsdElement) -> Result<(), DemoCsdElement> {
        self.contained_elements.push(element);
        Ok(())
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
        Ok(())
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
            e.serialize_into(into)?;
        }
        element.insert("contained_elements".to_owned(),
            toml::Value::Array(self.contained_elements.iter().map(|e| toml::Value::String(e.uuid().to_string())).collect())
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
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) {
        if let Some(t) = &self.transaction {
            v.open_complex(self);
            t.read().unwrap().accept(v);
            v.close_complex(self);
        } else {
            v.visit_simple(self);
        }
    }
}

impl ContainerModel for DemoCsdTransactor {
    type ElementT = DemoCsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoCsdElement, ModelUuid)> {
        if let Some(e) = &self.transaction
            && *e.read().unwrap().uuid == *uuid {
            Some((e.clone().into(), *self.uuid))
        } else {
            None
        }
    }
}

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
