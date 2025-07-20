use serde::{Deserialize, Serialize};

use crate::common::canvas;
use crate::common::controller::{ContainerModel, Model, StructuralVisitor};
use crate::common::project_serde::{NHContextDeserialize, NHDeserializeError, NHDeserializer, NHSerializeError, NHContextSerialize, NHSerializer, NHDeserializeInstantiator};
use crate::common::uuid::ModelUuid;
use std::collections::HashMap;
use std::{
    collections::{HashSet},
    sync::{Arc, RwLock},
};

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerializeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[container_model(element_type = DemoCsdElement, default_passthrough = "none")]
#[nh_context_serialize_tag(uuid_type = ModelUuid)]
pub enum DemoCsdElement {
    #[container_model(passthrough = "arc_rwlock")]
    DemoCsdPackage(Arc<RwLock<DemoCsdPackage>>),
    #[container_model(passthrough = "arc_rwlock")]
    DemoCsdTransactor(Arc<RwLock<DemoCsdTransactor>>),
    DemoCsdTransaction(Arc<RwLock<DemoCsdTransaction>>),
    DemoCsdLink(Arc<RwLock<DemoCsdLink>>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerializeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[nh_context_serialize_tag(uuid_type = ModelUuid)]
pub enum DemoCsdTransactorWrapper {
    DemoCsdTransactor(Arc<RwLock<DemoCsdTransactor>>)
}

impl DemoCsdTransactorWrapper {
    pub fn unwrap(self) -> Arc<RwLock<DemoCsdTransactor>> {
        match self {
            Self::DemoCsdTransactor(t) => t,
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerializeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[nh_context_serialize_tag(uuid_type = ModelUuid)]
pub enum DemoCsdTransactionWrapper {
    DemoCsdTransaction(Arc<RwLock<DemoCsdTransaction>>)
}

impl DemoCsdTransactionWrapper {
    pub fn unwrap(self) -> Arc<RwLock<DemoCsdTransaction>> {
        match self {
            Self::DemoCsdTransaction(t) => t,
        }
    }
}

impl NHContextSerialize for DemoCsdElement {
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
                    let new_tx = walk(&DemoCsdElement::DemoCsdTransaction(tx.clone().unwrap()), into);
                    into.insert(*tx.uuid(), new_tx.clone());
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
                    transaction: new_tx.map(|e| e.into()),
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
                    relink(&mut DemoCsdElement::DemoCsdTransaction(ta.clone().unwrap()), all_models);
                }
            },
            DemoCsdElement::DemoCsdTransaction(rw_lock) => {},
            DemoCsdElement::DemoCsdLink(rw_lock) => {
                let mut model = rw_lock.write().unwrap();

                let source_uuid = *model.source.uuid();
                if let Some(DemoCsdElement::DemoCsdTransactor(ta)) = all_models.get(&source_uuid) {
                    model.source = ta.clone().into();
                }
                let target_uuid = *model.target.uuid();
                if let Some(DemoCsdElement::DemoCsdTransaction(tx)) = all_models.get(&target_uuid) {
                    model.target = tx.clone().into();
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
                    walk(&DemoCsdElement::DemoCsdTransaction(tx.clone().unwrap()), into);
                    into.insert(*tx.uuid(), DemoCsdElement::DemoCsdTransaction(tx.clone().unwrap()));
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DemoCsdDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[serde(skip_deserializing)]
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

impl NHContextSerialize for DemoCsdDiagram {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        for e in &self.contained_elements {
            e.serialize_into(into);
        }

        Ok(())
    }
}

impl NHContextDeserialize for DemoCsdDiagram {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let e = source.get("contained_elements").ok_or_else(|| NHDeserializeError::StructureError("contained_elements not found".into()))?;
        let contained_elements = Vec::<DemoCsdElement>::deserialize(e, deserializer)?;
        Ok(Self { contained_elements, ..toml::Value::try_into(source.clone()).unwrap() })
    }
}

// ---

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DemoCsdPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[serde(skip_deserializing)]
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

impl NHContextSerialize for DemoCsdPackage {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        for e in &self.contained_elements {
            e.serialize_into(into);
        }

        Ok(())
    }
}

impl NHContextDeserialize for DemoCsdPackage {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let e = source.get("contained_elements").ok_or_else(|| NHDeserializeError::StructureError("contained_elements not found".into()))?;
        let contained_elements = Vec::<DemoCsdElement>::deserialize(e, deserializer)?;
        Ok(Self { contained_elements, ..toml::Value::try_into(source.clone()).unwrap() })
    }
}

// ---

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DemoCsdTransactor {
    pub uuid: Arc<ModelUuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,
    pub internal: bool,
    #[serde(skip_deserializing)]
    pub transaction: Option<DemoCsdTransactionWrapper>,
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
            transaction: transaction.map(|e| e.into()),
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
            t.accept(v);
            v.close_complex(self);
        } else {
            v.visit_simple(self);
        }
    }
}

impl ContainerModel for DemoCsdTransactor {
    type ElementT = DemoCsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoCsdElement, ModelUuid)> {
        if let Some(e) = &self.transaction && *e.uuid() == *uuid {
            Some((e.clone().unwrap().into(), *self.uuid))
        } else {
            None
        }
    }
}

impl NHContextSerialize for DemoCsdTransactor {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        if let Some(t) = &self.transaction {
            t.clone().unwrap().read().unwrap().serialize_into(into);
        }

        Ok(())
    }
}

impl NHContextDeserialize for DemoCsdTransactor {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        if let Some(t) = source.get("transaction") {
            let transaction = Some(DemoCsdTransactionWrapper::deserialize(t, deserializer)?);
            Ok(Self { transaction, ..toml::Value::try_into(source.clone()).unwrap() })
        } else {
            Ok(toml::Value::try_into(source.clone()).unwrap())
        }
    }
}

// ---

#[derive(serde::Serialize, serde::Deserialize)]
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

impl NHContextSerialize for DemoCsdTransaction {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        Ok(())
    }
}

impl NHContextDeserialize for DemoCsdTransaction {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        Ok(toml::Value::try_into(source.clone())?)
    }
}

// ---

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(serde::Serialize)]
pub struct DemoCsdLink {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    pub link_type: DemoCsdLinkType,
    pub source: DemoCsdTransactorWrapper,
    pub target: DemoCsdTransactionWrapper,

    pub comment: Arc<String>,
}

// TODO: derive
#[derive(serde::Deserialize)]
struct DemoCsdLinkHelper {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    pub link_type: DemoCsdLinkType,

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
            source: source.into(),
            target: target.into(),
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

impl NHContextSerialize for DemoCsdLink {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        Ok(())
    }
}

impl NHContextDeserialize for DemoCsdLink {
    fn deserialize(
        from: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let s = from.get("source").unwrap();
        let source = DemoCsdTransactorWrapper::deserialize(s, deserializer)?;
        let t = from.get("target").unwrap();
        let target = DemoCsdTransactionWrapper::deserialize(t, deserializer)?;
        let helper: DemoCsdLinkHelper = toml::Value::try_into(from.clone()).unwrap();

        Ok(Self {
            source, target,
            uuid: helper.uuid,
            name: helper.name,
            link_type: helper.link_type,
            comment: helper.comment,
        })
    }
}
