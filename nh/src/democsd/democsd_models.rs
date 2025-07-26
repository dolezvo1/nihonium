use crate::common::canvas;
use crate::common::controller::{ContainerModel, Model, StructuralVisitor};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::ufoption::UFOption;
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = DemoCsdElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoCsdElement {
    #[container_model(passthrough = "eref")]
    DemoCsdPackage(ERef<DemoCsdPackage>),
    #[container_model(passthrough = "eref")]
    DemoCsdTransactor(ERef<DemoCsdTransactor>),
    DemoCsdTransaction(ERef<DemoCsdTransaction>),
    DemoCsdLink(ERef<DemoCsdLink>),
}

pub fn deep_copy_diagram(d: &DemoCsdDiagram) -> (ERef<DemoCsdDiagram>, HashMap<ModelUuid, DemoCsdElement>) {
    fn walk(e: &DemoCsdElement, into: &mut HashMap<ModelUuid, DemoCsdElement>) -> DemoCsdElement {
        let new_uuid = Arc::new(uuid::Uuid::now_v7().into());
        match e {
            DemoCsdElement::DemoCsdPackage(inner) => {
                let model = inner.read();

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
                DemoCsdElement::DemoCsdPackage(ERef::new(new_model))
            },
            DemoCsdElement::DemoCsdTransactor(inner) => {
                let model = inner.read();

                let new_tx = if let UFOption::Some(tx) = &model.transaction {
                    let new_tx = walk(&DemoCsdElement::DemoCsdTransaction(tx.clone()), into);
                    into.insert(*tx.read().uuid(), new_tx.clone());
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
                    transaction: new_tx.map(|e| e.into()).into(),
                    transaction_selfactivating: model.transaction_selfactivating,
                    comment: model.comment.clone()
                };
                DemoCsdElement::DemoCsdTransactor(ERef::new(new_model))
            },
            DemoCsdElement::DemoCsdTransaction(inner) => {
                let model = inner.read();

                let new_model = DemoCsdTransaction {
                    uuid: new_uuid,
                    identifier: model.identifier.clone(),
                    name: model.name.clone(),
                    comment: model.comment.clone(),
                };
                DemoCsdElement::DemoCsdTransaction(ERef::new(new_model))
            },
            DemoCsdElement::DemoCsdLink(inner) => {
                let model = inner.read();

                let new_model = DemoCsdLink {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    link_type: model.link_type,
                    source: model.source.clone(),
                    target: model.target.clone(),
                    comment: model.comment.clone(),
                };
                DemoCsdElement::DemoCsdLink(ERef::new(new_model))
            },
        }
    }

    fn relink(e: &mut DemoCsdElement, all_models: &HashMap<ModelUuid, DemoCsdElement>) {
        match e {
            DemoCsdElement::DemoCsdPackage(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            DemoCsdElement::DemoCsdTransactor(inner) => {
                let mut model = inner.write();
                if let UFOption::Some(ta) = &mut model.transaction {
                    relink(&mut DemoCsdElement::DemoCsdTransaction(ta.clone()), all_models);
                }
            },
            DemoCsdElement::DemoCsdTransaction(inner) => {},
            DemoCsdElement::DemoCsdLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(DemoCsdElement::DemoCsdTransactor(ta)) = all_models.get(&source_uuid) {
                    model.source = ta.clone();
                }
                let target_uuid = *model.target.read().uuid();
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
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &DemoCsdDiagram) -> HashMap<ModelUuid, DemoCsdElement> {
    fn walk(e: &DemoCsdElement, into: &mut HashMap<ModelUuid, DemoCsdElement>) {
        match e {
            DemoCsdElement::DemoCsdPackage(rw_lock) => {
                let model = rw_lock.read();

                for e in &model.contained_elements {
                    walk(e, into);
                    into.insert(*e.uuid(), e.clone());
                }
            },
            DemoCsdElement::DemoCsdTransactor(rw_lock) => {
                let model = rw_lock.read();

                if let UFOption::Some(tx) = &model.transaction {
                    walk(&DemoCsdElement::DemoCsdTransaction(tx.clone()), into);
                    into.insert(*tx.read().uuid(), DemoCsdElement::DemoCsdTransaction(tx.clone()));
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct DemoCsdDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
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

impl Entity for DemoCsdDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
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

// ---

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct DemoCsdPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
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

impl Entity for DemoCsdPackage {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
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

// ---

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct DemoCsdTransactor {
    pub uuid: Arc<ModelUuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,
    pub internal: bool,
    #[nh_context_serde(entity)]
    pub transaction: UFOption<ERef<DemoCsdTransaction>>,
    pub transaction_selfactivating: bool,

    pub comment: Arc<String>,
}

impl DemoCsdTransactor {
    pub fn new(
        uuid: ModelUuid,
        identifier: String,
        name: String,
        internal: bool,
        transaction: Option<ERef<DemoCsdTransaction>>,
        transaction_selfactivating: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),

            identifier: Arc::new(identifier),
            name: Arc::new(name),
            internal,
            transaction: transaction.into(),
            transaction_selfactivating,

            comment: Arc::new("".to_owned()),
        }
    }
}

impl Entity for DemoCsdTransactor {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
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
        if let UFOption::Some(t) = &self.transaction {
            v.open_complex(self);
            t.read().accept(v);
            v.close_complex(self);
        } else {
            v.visit_simple(self);
        }
    }
}

impl ContainerModel for DemoCsdTransactor {
    type ElementT = DemoCsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoCsdElement, ModelUuid)> {
        if let UFOption::Some(e) = &self.transaction && *e.read().uuid() == *uuid {
            Some((e.clone().into(), *self.uuid))
        } else {
            None
        }
    }
}

// ---

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
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

impl Entity for DemoCsdTransaction {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct DemoCsdLink {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    pub link_type: DemoCsdLinkType,
    #[nh_context_serde(entity)]
    pub source: ERef<DemoCsdTransactor>,
    #[nh_context_serde(entity)]
    pub target: ERef<DemoCsdTransaction>,

    pub comment: Arc<String>,
}

impl DemoCsdLink {
    pub fn new(
        uuid: ModelUuid,
        link_type: DemoCsdLinkType,
        source: ERef<DemoCsdTransactor>,
        target: ERef<DemoCsdTransaction>,
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

impl Entity for DemoCsdLink {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
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
