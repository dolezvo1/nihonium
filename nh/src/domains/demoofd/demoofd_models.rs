
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{common::{
    controller::{BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT, VisitableDiagram, VisitableElement}, entity::{Entity, EntityUuid}, eref::ERef, search::FullTextSearchable, ufoption::UFOption, uuid::ModelUuid
}, domains::demo::DemoTransactionKind};


#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = DemoOfdElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoOfdElement {
    #[container_model(passthrough = "eref")]
    DemoOfdPackage(ERef<DemoOfdPackage>),
    DemoOfdEntityType(ERef<DemoOfdEntityType>),
    #[container_model(passthrough = "eref")]
    DemoOfdEventType(ERef<DemoOfdEventType>),
    DemoOfdPropertyType(ERef<DemoOfdPropertyType>),
    DemoOfdSpecialization(ERef<DemoOfdSpecialization>),
    DemoOfdAggregation(ERef<DemoOfdAggregation>),
    DemoOfdPrecedence(ERef<DemoOfdPrecedence>),
    DemoOfdExclusion(ERef<DemoOfdExclusion>),
}

impl DemoOfdElement {
    pub fn as_type(self) -> Option<DemoOfdType> {
        match self {
            DemoOfdElement::DemoOfdEntityType(inner) => Some(inner.into()),
            DemoOfdElement::DemoOfdEventType(inner) => Some(inner.into()),
            DemoOfdElement::DemoOfdPropertyType(inner) => Some(inner.into()),
            DemoOfdElement::DemoOfdPackage(..)
            | DemoOfdElement::DemoOfdPrecedence(..)
            | DemoOfdElement::DemoOfdSpecialization(..)
            | DemoOfdElement::DemoOfdAggregation(..)
            | DemoOfdElement::DemoOfdExclusion(..) => None,
        }
    }
}

impl VisitableElement for DemoOfdElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            DemoOfdElement::DemoOfdPackage(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            },
            DemoOfdElement::DemoOfdEventType(inner) => {
                if let UFOption::Some(t) = &inner.read().specialization_entity_type {
                    v.open_complex(self);
                    DemoOfdElement::from(t.clone()).accept(v);
                    v.close_complex(self);
                } else {
                    v.visit_simple(self);
                }
            }
            _ => v.visit_simple(self),
        }
    }
}

impl FullTextSearchable for DemoOfdElement {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        match self {
            DemoOfdElement::DemoOfdPackage(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdEntityType(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdEventType(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdPropertyType(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdSpecialization(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdAggregation(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdPrecedence(inner) => inner.read().full_text_search(acc),
            DemoOfdElement::DemoOfdExclusion(inner) => inner.read().full_text_search(acc),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoOfdType {
    DemoOfdEntityType(ERef<DemoOfdEntityType>),
    DemoOfdEventType(ERef<DemoOfdEventType>),
    DemoOfdPropertyType(ERef<DemoOfdPropertyType>),
}


pub fn deep_copy_diagram(d: &DemoOfdDiagram) -> (ERef<DemoOfdDiagram>, HashMap<ModelUuid, DemoOfdElement>) {
    fn walk(e: &DemoOfdElement, into: &mut HashMap<ModelUuid, DemoOfdElement>) -> DemoOfdElement {
        let new_uuid = ModelUuid::now_v7().into();
        match e {
            DemoOfdElement::DemoOfdPackage(inner) => {
                let model = inner.read();

                let new_model = DemoOfdPackage {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                ERef::new(new_model).into()
            },
            DemoOfdElement::DemoOfdEntityType(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
            DemoOfdElement::DemoOfdEventType(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
            DemoOfdElement::DemoOfdPropertyType(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
            DemoOfdElement::DemoOfdSpecialization(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
            DemoOfdElement::DemoOfdAggregation(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
            DemoOfdElement::DemoOfdPrecedence(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
            DemoOfdElement::DemoOfdExclusion(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
        }
    }

    fn relink(e: &mut DemoOfdElement, all_models: &HashMap<ModelUuid, DemoOfdElement>) {
        match e {
            DemoOfdElement::DemoOfdPackage(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            DemoOfdElement::DemoOfdEntityType(..) => {},
            DemoOfdElement::DemoOfdEventType(inner) => {
                let mut model = inner.write();

                let base_id = *model.base_entity_type.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEntityType(b)) = all_models.get(&base_id) {
                    model.base_entity_type = b.clone();
                }
                if let UFOption::Some(spec) = &mut model.specialization_entity_type {
                    let spec_id = *spec.read().uuid;
                    if let Some(DemoOfdElement::DemoOfdEntityType(s)) = all_models.get(&spec_id) {
                        *spec = s.clone();
                    }
                }
            },
            DemoOfdElement::DemoOfdPropertyType(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.domain_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEntityType(de)) = all_models.get(&source_uuid) {
                    model.domain_element = de.clone();
                }
                let target_uuid = *model.range_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEntityType(re)) = all_models.get(&target_uuid) {
                    model.range_element = re.clone();
                }
            },
            DemoOfdElement::DemoOfdAggregation(inner) => {
                let mut model = inner.write();

                for e in model.domain_elements.iter_mut() {
                    let source_uuid = *e.read().uuid;
                    if let Some(DemoOfdElement::DemoOfdEntityType(de)) = all_models.get(&source_uuid) {
                        *e = de.clone();
                    }
                }
                let target_uuid = *model.range_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEntityType(re)) = all_models.get(&target_uuid) {
                    model.range_element = re.clone();
                }
            },
            DemoOfdElement::DemoOfdPrecedence(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.domain_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEventType(de)) = all_models.get(&source_uuid) {
                    model.domain_element = de.clone();
                }
                let target_uuid = *model.range_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEventType(re)) = all_models.get(&target_uuid) {
                    model.range_element = re.clone();
                }
            },
            DemoOfdElement::DemoOfdSpecialization(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.domain_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEntityType(de)) = all_models.get(&source_uuid) {
                    model.domain_element = de.clone();
                }
                let target_uuid = *model.range_element.read().uuid;
                if let Some(DemoOfdElement::DemoOfdEntityType(re)) = all_models.get(&target_uuid) {
                    model.range_element = re.clone();
                }
            },
            DemoOfdElement::DemoOfdExclusion(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.domain_element.uuid();
                if let Some(de) = all_models.get(&source_uuid).and_then(|e| e.clone().as_type()) {
                    model.domain_element = de.clone();
                }
                let target_uuid = *model.range_element.uuid();
                if let Some(re) = all_models.get(&target_uuid).and_then(|e| e.clone().as_type()) {
                    model.range_element = re.clone();
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

    let new_diagram = DemoOfdDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &DemoOfdDiagram) -> HashMap<ModelUuid, DemoOfdElement> {
    fn walk(e: &DemoOfdElement, into: &mut HashMap<ModelUuid, DemoOfdElement>) {
        match e {
            DemoOfdElement::DemoOfdPackage(inner) => {
                let model = inner.read();

                for e in &model.contained_elements {
                    walk(e, into);
                    into.insert(*e.uuid(), e.clone());
                }
            },
            _ => {},
        }
    }

    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        walk(e, &mut all_models);
        all_models.insert(*e.uuid(), e.clone());
    }

    all_models
}

pub fn transitive_closure(d: &DemoOfdDiagram, mut when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &DemoOfdElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                DemoOfdElement::DemoOfdPackage(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        enumerate(e, when_deleting);
                    } else {
                        for e in &r.contained_elements {
                            walk(e, when_deleting);
                        }
                    }
                },
                DemoOfdElement::DemoOfdEntityType(..) => {},
                DemoOfdElement::DemoOfdEventType(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        enumerate(e, when_deleting);
                    } else {
                        if let UFOption::Some(e) = &r.specialization_entity_type {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                },
                DemoOfdElement::DemoOfdPropertyType(..)
                | DemoOfdElement::DemoOfdSpecialization(..)
                | DemoOfdElement::DemoOfdAggregation(..)
                | DemoOfdElement::DemoOfdPrecedence(..)
                | DemoOfdElement::DemoOfdExclusion(..) => {},
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(e: &DemoOfdElement, when_deleting: &HashSet<ModelUuid>, also_delete: &mut HashSet<ModelUuid>) {
            match e {
                DemoOfdElement::DemoOfdPackage(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                },
                DemoOfdElement::DemoOfdEntityType(..) => {},
                DemoOfdElement::DemoOfdEventType(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && when_deleting.contains(&r.base_entity_type.read().uuid) {
                        also_delete.insert(*r.uuid);
                    }
                },
                DemoOfdElement::DemoOfdPropertyType(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.read().uuid)
                            || when_deleting.contains(&r.range_element.read().uuid)) {
                        also_delete.insert(*r.uuid);
                    }
                },
                DemoOfdElement::DemoOfdSpecialization(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.read().uuid)
                            || when_deleting.contains(&r.range_element.read().uuid)) {
                        also_delete.insert(*r.uuid);
                    }
                },
                DemoOfdElement::DemoOfdAggregation(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (r.domain_elements.iter().all(|e| when_deleting.contains(&e.read().uuid))
                            || when_deleting.contains(&r.range_element.read().uuid)) {
                        also_delete.insert(*r.uuid);
                    }
                },
                DemoOfdElement::DemoOfdPrecedence(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.read().uuid)
                            || when_deleting.contains(&r.range_element.read().uuid)) {
                        also_delete.insert(*r.uuid);
                    }
                },
                DemoOfdElement::DemoOfdExclusion(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.uuid())
                            || when_deleting.contains(&r.range_element.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
            }
        }
        for e in &d.contained_elements {
            walk(e, &when_deleting, &mut also_delete);
        }
        if also_delete.is_empty() {
            break;
        }
        when_deleting.extend(also_delete.drain());
    }

    when_deleting
}

fn enumerate(e: &DemoOfdElement, into: &mut HashSet<ModelUuid>) {
    into.insert(*e.uuid());
    match e {
        DemoOfdElement::DemoOfdPackage(inner) => {
            for e in &inner.read().contained_elements {
                enumerate(e, into);
            }
        },
        DemoOfdElement::DemoOfdEntityType(..) => {},
        DemoOfdElement::DemoOfdEventType(inner) => {
            if let UFOption::Some(e) = &inner.read().specialization_entity_type {
                enumerate(&e.clone().into(), into);
            }
        },
        DemoOfdElement::DemoOfdPropertyType(..)
        | DemoOfdElement::DemoOfdSpecialization(..)
        | DemoOfdElement::DemoOfdAggregation(..)
        | DemoOfdElement::DemoOfdPrecedence(..)
        | DemoOfdElement::DemoOfdExclusion(..) => {},
    }
}

// ---

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct DemoOfdDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<DemoOfdElement>,

    pub comment: Arc<String>,
}

impl DemoOfdDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<DemoOfdElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Entity for DemoOfdDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for DemoOfdDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for DemoOfdDiagram {
    type ElementT = DemoOfdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoOfdElement, ModelUuid)> {
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
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: DemoOfdElement) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.contained_elements.len());
        self.contained_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.contained_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for DemoOfdDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.comment,
            ],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<DemoOfdElement>,

    pub comment: Arc<String>,
}

impl DemoOfdPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<DemoOfdElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            contained_elements: self.contained_elements.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for DemoOfdPackage {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for DemoOfdPackage {
    type ElementT = DemoOfdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoOfdElement, ModelUuid)> {
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
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: DemoOfdElement) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.contained_elements.len());
        self.contained_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.contained_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for DemoOfdPackage {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.comment,
            ],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}



#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdEntityType {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub properties: Arc<String>,
    pub internal: bool,

    pub comment: Arc<String>,
}

impl DemoOfdEntityType {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        properties: String,
        internal: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            properties: Arc::new(properties),
            internal,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            properties: self.properties.clone(),
            internal: self.internal,
            comment: self.comment.clone(),
        })
    }
}

impl Entity for DemoOfdEntityType {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdEntityType {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for DemoOfdEntityType {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.properties,
                &self.comment,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdEventType {
    pub uuid: Arc<ModelUuid>,

    pub kind: DemoTransactionKind,
    pub identifier: Arc<String>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    pub base_entity_type: ERef<DemoOfdEntityType>,
    #[nh_context_serde(entity)]
    pub specialization_entity_type: UFOption<ERef<DemoOfdEntityType>>,

    pub comment: Arc<String>,
}

impl DemoOfdEventType {
    pub fn new(
        uuid: ModelUuid,
        kind: DemoTransactionKind,
        identifier: String,
        name: String,
        base_entity_type: ERef<DemoOfdEntityType>,
        specialization_entity_type: Option<ERef<DemoOfdEntityType>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            identifier: Arc::new(identifier),
            name: Arc::new(name),
            base_entity_type,
            specialization_entity_type: specialization_entity_type.into(),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            kind: self.kind,
            identifier: self.identifier.clone(),
            name: self.name.clone(),
            base_entity_type: self.base_entity_type.clone(),
            specialization_entity_type: self.specialization_entity_type.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for DemoOfdEventType {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdEventType {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for DemoOfdEventType {
    type ElementT = DemoOfdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoOfdElement, ModelUuid)> {
        if let UFOption::Some(e) = &self.specialization_entity_type
            && *uuid == *e.read().uuid {
            Some((e.clone().into(), *self.uuid))
        } else {
            None
        }
    }
    fn insert_element(&mut self, bucket: BucketNoT, _position: Option<PositionNoT>, element: DemoOfdElement) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        if !self.specialization_entity_type.is_some()
            && let DemoOfdElement::DemoOfdEntityType(e) = element {
            self.specialization_entity_type = UFOption::Some(e);
            Ok(0)
        } else {
            Err(element)
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if let UFOption::Some(e) = &self.specialization_entity_type
            && *e.read().uuid == *uuid {
            self.specialization_entity_type = UFOption::None;
            Some((0, 0))
        } else {
            None
        }
    }
}

impl FullTextSearchable for DemoOfdEventType {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.identifier,
                &self.name,
                &self.comment,
            ],
        );

        if let UFOption::Some(e) = &self.specialization_entity_type {
            e.read().full_text_search(acc);
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdPropertyType {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub domain_element: ERef<DemoOfdEntityType>,
    pub domain_multiplicity: Arc<String>,
    #[nh_context_serde(entity)]
    pub range_element: ERef<DemoOfdEntityType>,
    pub range_multiplicity: Arc<String>,

    pub comment: Arc<String>,
}

impl DemoOfdPropertyType {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        domain_element: ERef<DemoOfdEntityType>,
        range_element: ERef<DemoOfdEntityType>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            domain_element,
            domain_multiplicity: Arc::new("0..*".to_owned()),
            range_element,
            range_multiplicity: Arc::new("1..1".to_owned()),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            domain_element: self.domain_element.clone(),
            domain_multiplicity: self.domain_multiplicity.clone(),
            range_element: self.range_element.clone(),
            range_multiplicity: self.range_multiplicity.clone(),
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.domain_element, &mut self.range_element);
    }
}

impl Entity for DemoOfdPropertyType {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdPropertyType {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for DemoOfdPropertyType {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.domain_multiplicity,
                &self.range_multiplicity,
                &self.comment,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdSpecialization {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub domain_element: ERef<DemoOfdEntityType>,
    #[nh_context_serde(entity)]
    pub range_element: ERef<DemoOfdEntityType>,

    pub comment: Arc<String>,
}

impl DemoOfdSpecialization {
    pub fn new(
        uuid: ModelUuid,
        domain_element: ERef<DemoOfdEntityType>,
        range_element: ERef<DemoOfdEntityType>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            domain_element,
            range_element,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_element: self.domain_element.clone(),
            range_element: self.range_element.clone(),
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.domain_element, &mut self.range_element);
    }
}

impl Entity for DemoOfdSpecialization {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdSpecialization {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for DemoOfdSpecialization {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.comment,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdAggregation {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub domain_elements: Vec<ERef<DemoOfdEntityType>>,
    #[nh_context_serde(entity)]
    pub range_element: ERef<DemoOfdEntityType>,
    pub is_generalization: bool,

    pub comment: Arc<String>,
}

impl DemoOfdAggregation {
    pub fn new(
        uuid: ModelUuid,
        domain_elements: Vec<ERef<DemoOfdEntityType>>,
        range_element: ERef<DemoOfdEntityType>,
        is_generalization: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            domain_elements,
            range_element,
            is_generalization,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_elements: self.domain_elements.clone(),
            range_element: self.range_element.clone(),
            is_generalization: self.is_generalization,
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) -> Result<(), ()> {
        if self.domain_elements.len() == 1 {
            let tmp = self.range_element.clone();
            self.range_element = self.domain_elements[0].clone();
            self.domain_elements = vec![tmp];
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Entity for DemoOfdAggregation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdAggregation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for DemoOfdAggregation {
    type ElementT = DemoOfdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoOfdElement, ModelUuid)> {
        for e in &self.domain_elements {
            if *e.read().uuid == *uuid {
                return Some((e.clone().into(), *self.uuid))
            }
        }
        if *self.range_element.read().uuid == *uuid {
            return Some((self.range_element.clone().into(), *self.uuid))
        }
        None
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: DemoOfdElement) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        let DemoOfdElement::DemoOfdEntityType(entity) = element else {
            return Err(element);
        };

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.domain_elements.len());
        self.domain_elements.insert(pos, entity);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if self.domain_elements.len() == 1 {
            return None;
        }
        for (idx, e) in self.domain_elements.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.domain_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for DemoOfdAggregation {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.comment,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdPrecedence {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub domain_element: ERef<DemoOfdEventType>,
    #[nh_context_serde(entity)]
    pub range_element: ERef<DemoOfdEventType>,

    pub comment: Arc<String>,
}

impl DemoOfdPrecedence {
    pub fn new(
        uuid: ModelUuid,
        domain_element: ERef<DemoOfdEventType>,
        range_element: ERef<DemoOfdEventType>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            domain_element,
            range_element,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_element: self.domain_element.clone(),
            range_element: self.range_element.clone(),
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.domain_element, &mut self.range_element);
    }
}

impl Entity for DemoOfdPrecedence {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdPrecedence {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for DemoOfdPrecedence {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.comment,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdExclusion {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub domain_element: DemoOfdType,
    #[nh_context_serde(entity)]
    pub range_element: DemoOfdType,

    pub comment: Arc<String>,
}

impl DemoOfdExclusion {
    pub fn new(
        uuid: ModelUuid,
        domain_element: DemoOfdType,
        range_element: DemoOfdType,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            domain_element,
            range_element,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_element: self.domain_element.clone(),
            range_element: self.range_element.clone(),
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.domain_element, &mut self.range_element);
    }
}

impl Entity for DemoOfdExclusion {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdExclusion {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for DemoOfdExclusion {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.comment,
            ],
        );
    }
}
