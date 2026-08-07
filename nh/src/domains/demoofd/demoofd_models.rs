use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    common::{
        entity::{Entity, EntityUuid},
        eref::ERef,
        model::{
            BucketNoT, ContainerModel, DiagramModel, DiagramVisitor, ElementVisitor, Model,
            ModelTopSortInfo, PositionNoT, VisitableDiagram, VisitableElement,
        },
        search::FullTextSearchable,
        ufoption::UFOption,
        uuid::ModelUuid,
        views::multiconnection_view::MULTICONNECTION_SOURCE_BUCKET,
    },
    domains::demo::{DemoPackageKind, DemoTransactionKind},
};

#[derive(
    Clone,
    derive_more::From,
    nh_derive::Model,
    nh_derive::ContainerModel,
    nh_derive::FullTextSearchable,
    nh_derive::NHContextSerDeTag,
)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = DemoOfdElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoOfdElement {
    #[container_model(passthrough = "eref")]
    Package(ERef<DemoOfdPackage>),
    EntityType(ERef<DemoOfdEntityType>),
    #[container_model(passthrough = "eref")]
    EventType(ERef<DemoOfdEventType>),
    PropertyType(ERef<DemoOfdPropertyType>),
    Specialization(ERef<DemoOfdSpecialization>),
    Aggregation(ERef<DemoOfdAggregation>),
    Precedence(ERef<DemoOfdPrecedence>),
    Exclusion(ERef<DemoOfdExclusion>),
    Note(ERef<DemoOfdNote>),
}

impl DemoOfdElement {
    pub fn as_type(self) -> Option<DemoOfdType> {
        match self {
            DemoOfdElement::EntityType(inner) => Some(inner.into()),
            DemoOfdElement::EventType(inner) => Some(inner.into()),
            DemoOfdElement::PropertyType(inner) => Some(inner.into()),
            DemoOfdElement::Package(..)
            | DemoOfdElement::Precedence(..)
            | DemoOfdElement::Specialization(..)
            | DemoOfdElement::Aggregation(..)
            | DemoOfdElement::Exclusion(..)
            | DemoOfdElement::Note(..) => None,
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self {
        match self {
            Self::Package(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::EntityType(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::EventType(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::PropertyType(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Specialization(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::Aggregation(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Precedence(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Exclusion(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }

    pub fn deep_copy_relink(&self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        match self {
            Self::Package(..) | Self::EntityType(..) => {}
            Self::EventType(inner) => inner.write().deep_copy_relink(m),
            Self::PropertyType(inner) => inner.write().deep_copy_relink(m),
            Self::Aggregation(inner) => inner.write().deep_copy_relink(m),
            Self::Precedence(inner) => inner.write().deep_copy_relink(m),
            Self::Specialization(inner) => inner.write().deep_copy_relink(m),
            Self::Exclusion(inner) => inner.write().deep_copy_relink(m),
            Self::Note(_) => {}
        }
    }
}

impl VisitableElement for DemoOfdElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            DemoOfdElement::Package(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            }
            DemoOfdElement::EventType(inner) => {
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

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoOfdType {
    EntityType(ERef<DemoOfdEntityType>),
    EventType(ERef<DemoOfdEventType>),
    PropertyType(ERef<DemoOfdPropertyType>),
}

pub fn deep_copy_diagram(
    d: &DemoOfdDiagram,
) -> (ERef<DemoOfdDiagram>, HashMap<ModelUuid, DemoOfdElement>) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = DemoOfdDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &DemoOfdDiagram) -> HashMap<ModelUuid, DemoOfdElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(e, &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &DemoOfdElement, into: &mut HashMap<ModelUuid, DemoOfdElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        DemoOfdElement::Package(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(e, into);
            }
        }
        DemoOfdElement::EntityType(..) => {}
        DemoOfdElement::EventType(inner) => {
            if let UFOption::Some(e) = &inner.read().specialization_entity_type {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        DemoOfdElement::PropertyType(..)
        | DemoOfdElement::Specialization(..)
        | DemoOfdElement::Aggregation(..)
        | DemoOfdElement::Precedence(..)
        | DemoOfdElement::Exclusion(..)
        | DemoOfdElement::Note(..) => {}
    }
}

pub fn transitive_closure(
    d: &DemoOfdDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &DemoOfdElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                DemoOfdElement::Package(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.contained_elements {
                            walk(e, when_deleting);
                        }
                    }
                }
                DemoOfdElement::EntityType(..) => {}
                DemoOfdElement::EventType(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        if let UFOption::Some(e) = &r.specialization_entity_type {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                }
                DemoOfdElement::PropertyType(..)
                | DemoOfdElement::Specialization(..)
                | DemoOfdElement::Aggregation(..)
                | DemoOfdElement::Precedence(..)
                | DemoOfdElement::Exclusion(..)
                | DemoOfdElement::Note(..) => {}
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &DemoOfdElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                DemoOfdElement::Package(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                }
                DemoOfdElement::EntityType(..) => {}
                DemoOfdElement::EventType(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && when_deleting.contains(&r.base_entity_type.read().uuid)
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoOfdElement::PropertyType(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.read().uuid)
                            || when_deleting.contains(&r.range_element.read().uuid))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoOfdElement::Specialization(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.read().uuid)
                            || when_deleting.contains(&r.range_element.read().uuid))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoOfdElement::Aggregation(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (r
                            .domain_elements
                            .iter()
                            .all(|e| when_deleting.contains(&e.read().uuid))
                            || when_deleting.contains(&r.range_element.read().uuid))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoOfdElement::Precedence(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.read().uuid)
                            || when_deleting.contains(&r.range_element.read().uuid))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoOfdElement::Exclusion(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.domain_element.uuid())
                            || when_deleting.contains(&r.range_element.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoOfdElement::Note(..) => {}
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

pub fn top_sort_info(m: &DemoOfdElement) -> ModelTopSortInfo {
    fn walk(
        e: &DemoOfdElement,
        required_models: &mut HashSet<ModelUuid>,
        provided_models: &mut HashSet<ModelUuid>,
    ) {
        provided_models.insert(*e.uuid());
        match e {
            DemoOfdElement::Package(inner) => {
                for e in &inner.read().contained_elements {
                    walk(e, required_models, provided_models);
                }
            }
            DemoOfdElement::EntityType(_) => {}
            DemoOfdElement::EventType(inner) => {
                let r = inner.read();
                required_models.insert(*r.base_entity_type.read().uuid);
                if let Some(e) = r.specialization_entity_type.as_ref() {
                    walk(&e.clone().into(), required_models, provided_models);
                }
            }
            DemoOfdElement::PropertyType(inner) => {
                let r = inner.read();
                required_models.insert(*r.domain_element.read().uuid);
                required_models.insert(*r.range_element.read().uuid);
            }
            DemoOfdElement::Specialization(inner) => {
                let r = inner.read();
                required_models.insert(*r.domain_element.read().uuid);
                required_models.insert(*r.range_element.read().uuid);
            }
            DemoOfdElement::Aggregation(inner) => {
                let r = inner.read();
                for e in &r.domain_elements {
                    required_models.insert(*e.read().uuid);
                }
                required_models.insert(*r.range_element.read().uuid);
            }
            DemoOfdElement::Precedence(inner) => {
                let r = inner.read();
                required_models.insert(*r.domain_element.read().uuid);
                required_models.insert(*r.range_element.read().uuid);
            }
            DemoOfdElement::Exclusion(inner) => {
                let r = inner.read();
                required_models.insert(*r.domain_element.uuid());
                required_models.insert(*r.range_element.uuid());
            }
            DemoOfdElement::Note(_) => {}
        }
    }

    let (mut required_models, mut provided_models) = Default::default();
    walk(m, &mut required_models, &mut provided_models);
    ModelTopSortInfo {
        required_models,
        provided_models,
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
    pub fn new(uuid: ModelUuid, name: String, contained_elements: Vec<DemoOfdElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }

    pub fn get_element_pos_in(
        &self,
        parent: &ModelUuid,
        uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        if *parent == *self.uuid {
            self.get_element_pos(uuid)
        } else {
            self.find_element(parent)
                .and_then(|e| e.0.get_element_pos(uuid))
        }
    }
    fn insert_element_unsafe(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoOfdElement,
    ) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.contained_elements.len());
        self.contained_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element_unsafe(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.contained_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
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
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl DiagramModel for DemoOfdDiagram {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoOfdElement,
    ) -> Result<PositionNoT, DemoOfdElement> {
        if *self.uuid == target {
            self.insert_element_unsafe(bucket, position, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };
            match e {
                DemoOfdElement::Package(inner) => {
                    inner.write().insert_element(bucket, position, element)
                }
                DemoOfdElement::EventType(inner) => {
                    inner.write().insert_element(bucket, position, element)
                }
                DemoOfdElement::Aggregation(inner) => {
                    inner.write().insert_element(bucket, position, element)
                }
                DemoOfdElement::EntityType(_)
                | DemoOfdElement::PropertyType(_)
                | DemoOfdElement::Specialization(_)
                | DemoOfdElement::Precedence(_)
                | DemoOfdElement::Exclusion(_)
                | DemoOfdElement::Note(_) => Err(element),
            }
        }
    }
    fn remove_element_from(
        &mut self,
        target: ModelUuid,
        uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        if *self.uuid == target {
            self.remove_element_unsafe(uuid)
        } else {
            match self.find_element(&target)?.0 {
                DemoOfdElement::Package(inner) => inner.write().remove_element(uuid),
                DemoOfdElement::EventType(inner) => inner.write().remove_element(uuid),
                DemoOfdElement::Aggregation(inner) => inner.write().remove_element(uuid),
                DemoOfdElement::EntityType(_)
                | DemoOfdElement::PropertyType(_)
                | DemoOfdElement::Specialization(_)
                | DemoOfdElement::Precedence(_)
                | DemoOfdElement::Exclusion(_)
                | DemoOfdElement::Note(_) => None,
            }
        }
    }
}

impl FullTextSearchable for DemoOfdDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.name, &self.comment],
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
    pub kind: DemoPackageKind,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<DemoOfdElement>,

    pub comment: Arc<String>,
}

impl DemoOfdPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        kind: DemoPackageKind,
        contained_elements: Vec<DemoOfdElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(DemoOfdPackage {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            kind: self.kind,
            contained_elements: self
                .contained_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoOfdElement,
    ) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.contained_elements.len());
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
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
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
            &[&self.uuid.to_string(), &self.name, &self.comment],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdEntityType {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub properties: Arc<String>,
    #[full_text_searchable(skip)]
    pub internal: bool,

    pub comment: Arc<String>,
}

impl DemoOfdEntityType {
    pub fn new(uuid: ModelUuid, name: String, properties: String, internal: bool) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            properties: Arc::new(properties),
            internal,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            properties: self.properties.clone(),
            internal: self.internal,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
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
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_specialization = self
            .specialization_entity_type
            .as_ref()
            .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into));
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            kind: self.kind,
            identifier: self.identifier.clone(),
            name: self.name.clone(),
            base_entity_type: self.base_entity_type.clone(),
            specialization_entity_type: new_specialization.into(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        let base_id = *self.base_entity_type.read().uuid;
        if let Some(DemoOfdElement::EntityType(b)) = m.get(&base_id) {
            self.base_entity_type = b.clone();
        }
        if let UFOption::Some(spec) = &mut self.specialization_entity_type {
            let spec_id = *spec.read().uuid;
            if let Some(DemoOfdElement::EntityType(s)) = m.get(&spec_id) {
                *spec = s.clone();
            }
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        _position: Option<PositionNoT>,
        element: DemoOfdElement,
    ) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != 0 {
            return Err(element);
        }

        if !self.specialization_entity_type.is_some()
            && let DemoOfdElement::EntityType(e) = element
        {
            self.specialization_entity_type = UFOption::Some(e);
            Ok(0)
        } else {
            Err(element)
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if let UFOption::Some(e) = &self.specialization_entity_type
            && *e.read().uuid == *uuid
        {
            self.specialization_entity_type = UFOption::None;
            Some((0, 0))
        } else {
            None
        }
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
            && *uuid == *e.read().uuid
        {
            Some((e.clone().into(), *self.uuid))
        } else {
            None
        }
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if let UFOption::Some(e) = &self.specialization_entity_type
            && *uuid == *e.read().uuid
        {
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdPropertyType {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub domain_element: ERef<DemoOfdEntityType>,
    pub domain_multiplicity: Arc<String>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub range_element: ERef<DemoOfdEntityType>,
    pub range_multiplicity: Arc<String>,

    pub comment: Arc<String>,
}

impl DemoOfdPropertyType {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        domain_multiplicity: String,
        domain_element: ERef<DemoOfdEntityType>,
        range_multiplicity: String,
        range_element: ERef<DemoOfdEntityType>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            domain_element,
            domain_multiplicity: Arc::new(domain_multiplicity),
            range_element,
            range_multiplicity: Arc::new(range_multiplicity),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            domain_element: self.domain_element.clone(),
            domain_multiplicity: self.domain_multiplicity.clone(),
            range_element: self.range_element.clone(),
            range_multiplicity: self.range_multiplicity.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        let source_uuid = *self.domain_element.read().uuid;
        if let Some(DemoOfdElement::EntityType(de)) = m.get(&source_uuid) {
            self.domain_element = de.clone();
        }
        let target_uuid = *self.range_element.read().uuid;
        if let Some(DemoOfdElement::EntityType(re)) = m.get(&target_uuid) {
            self.range_element = re.clone();
        }
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdSpecialization {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub domain_element: ERef<DemoOfdEntityType>,
    #[full_text_searchable(skip)]
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
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_element: self.domain_element.clone(),
            range_element: self.range_element.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        let source_uuid = *self.domain_element.read().uuid;
        if let Some(DemoOfdElement::EntityType(de)) = m.get(&source_uuid) {
            self.domain_element = de.clone();
        }
        let target_uuid = *self.range_element.read().uuid;
        if let Some(DemoOfdElement::EntityType(re)) = m.get(&target_uuid) {
            self.range_element = re.clone();
        }
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdAggregation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub domain_elements: Vec<ERef<DemoOfdEntityType>>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub range_element: ERef<DemoOfdEntityType>,
    #[full_text_searchable(skip)]
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
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_elements: self.domain_elements.clone(),
            range_element: self.range_element.clone(),
            is_generalization: self.is_generalization,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        for e in self.domain_elements.iter_mut() {
            let source_uuid = *e.read().uuid;
            if let Some(DemoOfdElement::EntityType(de)) = m.get(&source_uuid) {
                *e = de.clone();
            }
        }
        let target_uuid = *self.range_element.read().uuid;
        if let Some(DemoOfdElement::EntityType(re)) = m.get(&target_uuid) {
            self.range_element = re.clone();
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoOfdElement,
    ) -> Result<PositionNoT, DemoOfdElement> {
        if bucket != MULTICONNECTION_SOURCE_BUCKET {
            return Err(element);
        }

        let DemoOfdElement::EntityType(entity) = element else {
            return Err(element);
        };

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.domain_elements.len());
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
                return Some((MULTICONNECTION_SOURCE_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
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
                return Some((e.clone().into(), *self.uuid));
            }
        }
        if *self.range_element.read().uuid == *uuid {
            return Some((self.range_element.clone().into(), *self.uuid));
        }
        None
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdPrecedence {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub domain_element: ERef<DemoOfdEventType>,
    #[full_text_searchable(skip)]
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
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_element: self.domain_element.clone(),
            range_element: self.range_element.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        let source_uuid = *self.domain_element.read().uuid;
        if let Some(DemoOfdElement::EventType(de)) = m.get(&source_uuid) {
            self.domain_element = de.clone();
        }
        let target_uuid = *self.range_element.read().uuid;
        if let Some(DemoOfdElement::EventType(re)) = m.get(&target_uuid) {
            self.range_element = re.clone();
        }
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdExclusion {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub domain_element: DemoOfdType,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub range_element: DemoOfdType,

    pub comment: Arc<String>,
}

impl DemoOfdExclusion {
    pub fn new(uuid: ModelUuid, domain_element: DemoOfdType, range_element: DemoOfdType) -> Self {
        Self {
            uuid: Arc::new(uuid),
            domain_element,
            range_element,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            domain_element: self.domain_element.clone(),
            range_element: self.range_element.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoOfdElement>) {
        let source_uuid = *self.domain_element.uuid();
        if let Some(de) = m.get(&source_uuid).and_then(|e| e.clone().as_type()) {
            self.domain_element = de.clone();
        }
        let target_uuid = *self.range_element.uuid();
        if let Some(re) = m.get(&target_uuid).and_then(|e| e.clone().as_type()) {
            self.range_element = re.clone();
        }
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoOfdNote {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl DemoOfdNote {
    pub fn new(uuid: ModelUuid, text: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            text: self.text.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for DemoOfdNote {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoOfdNote {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
