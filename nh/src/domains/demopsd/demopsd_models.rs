use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    common::{
        canvas,
        entity::{Entity, EntityUuid},
        eref::ERef,
        model::{
            BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT,
            VisitableDiagram, VisitableElement,
        },
        search::FullTextSearchable,
        ufoption::UFOption,
        uuid::ModelUuid,
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
#[container_model(element_type = DemoPsdElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoPsdElement {
    #[container_model(passthrough = "eref")]
    Package(ERef<DemoPsdPackage>),
    #[container_model(passthrough = "eref")]
    Transaction(ERef<DemoPsdTransaction>),
    Fact(ERef<DemoPsdFact>),
    Act(ERef<DemoPsdAct>),
    Link(ERef<DemoPsdLink>),
    Note(ERef<DemoPsdNote>),
}

#[derive(
    Clone,
    derive_more::From,
    nh_derive::Model,
    nh_derive::ContainerModel,
    nh_derive::FullTextSearchable,
    nh_derive::NHContextSerDeTag,
)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = DemoPsdElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum DemoPsdState {
    Fact(ERef<DemoPsdFact>),
    Act(ERef<DemoPsdAct>),
}

impl DemoPsdElement {
    pub fn to_state(self) -> Option<DemoPsdState> {
        match self {
            Self::Fact(inner) => Some(DemoPsdState::Fact(inner)),
            Self::Act(inner) => Some(DemoPsdState::Act(inner)),
            Self::Package(..) | Self::Transaction(..) | Self::Link(..) | Self::Note(..) => None,
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> DemoPsdElement {
        match self {
            Self::Package(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Transaction(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Fact(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Act(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Link(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }

    pub fn deep_copy_relink(&self, m: &HashMap<ModelUuid, DemoPsdElement>) {
        match self {
            Self::Package(..) | Self::Transaction(..) | Self::Fact(..) | Self::Act(..) => {}
            Self::Link(inner) => inner.write().deep_copy_relink(m),
            Self::Note(..) => {}
        }
    }
}

impl DemoPsdState {
    pub fn to_element(self) -> DemoPsdElement {
        match self {
            Self::Fact(inner) => DemoPsdElement::Fact(inner),
            Self::Act(inner) => DemoPsdElement::Act(inner),
        }
    }
    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> Self {
        match self {
            Self::Fact(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Act(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
}

impl VisitableElement for DemoPsdElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            DemoPsdElement::Package(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            }
            DemoPsdElement::Transaction(inner) => {
                v.open_complex(self);
                let r = inner.read();
                for e in &r.before {
                    e.state.clone().to_element().accept(v);
                }
                if let UFOption::Some(e) = &r.p_act {
                    DemoPsdElement::from(e.clone()).accept(v);
                }
                for e in &r.after {
                    e.state.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            e => v.visit_simple(e),
        }
    }
}

pub fn deep_copy_diagram(
    d: &DemoPsdDiagram,
) -> (ERef<DemoPsdDiagram>, HashMap<ModelUuid, DemoPsdElement>) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = DemoPsdDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &DemoPsdDiagram) -> HashMap<ModelUuid, DemoPsdElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(e, &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &DemoPsdElement, into: &mut HashMap<ModelUuid, DemoPsdElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        DemoPsdElement::Package(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(e, into);
            }
        }
        DemoPsdElement::Transaction(inner) => {
            let r = inner.read();
            for e in &r.before {
                enumerate_elements(&e.state.clone().to_element(), into);
            }
            if let UFOption::Some(e) = &r.p_act {
                enumerate_elements(&e.clone().into(), into);
            }
            for e in &r.after {
                enumerate_elements(&e.state.clone().to_element(), into);
            }
        }
        DemoPsdElement::Fact(..)
        | DemoPsdElement::Act(..)
        | DemoPsdElement::Link(..)
        | DemoPsdElement::Note(..) => {}
    }
}

pub fn transitive_closure(
    d: &DemoPsdDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &DemoPsdElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                DemoPsdElement::Package(inner) => {
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
                DemoPsdElement::Transaction(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.before {
                            walk(&e.state.clone().to_element(), when_deleting);
                        }
                        if let UFOption::Some(e) = &r.p_act {
                            walk(&e.clone().into(), when_deleting);
                        }
                        for e in &r.after {
                            walk(&e.state.clone().to_element(), when_deleting);
                        }
                    }
                }
                DemoPsdElement::Fact(..)
                | DemoPsdElement::Act(..)
                | DemoPsdElement::Link(..)
                | DemoPsdElement::Note(..) => {}
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &DemoPsdElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                DemoPsdElement::Package(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                }
                DemoPsdElement::Transaction(..)
                | DemoPsdElement::Fact(..)
                | DemoPsdElement::Act(..) => {}
                DemoPsdElement::Link(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid)
                            || when_deleting.contains(&r.target.read().uuid))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                DemoPsdElement::Note(..) => {}
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct DemoPsdDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<DemoPsdElement>,

    pub comment: Arc<String>,
}

impl DemoPsdDiagram {
    pub fn new(uuid: ModelUuid, name: String, contained_elements: Vec<DemoPsdElement>) -> Self {
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
}

impl Entity for DemoPsdDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for DemoPsdDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for DemoPsdDiagram {
    type ElementT = DemoPsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoPsdElement, ModelUuid)> {
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
    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoPsdElement,
    ) -> Result<PositionNoT, DemoPsdElement> {
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

impl FullTextSearchable for DemoPsdDiagram {
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
pub struct DemoPsdPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub kind: DemoPackageKind,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<DemoPsdElement>,

    pub comment: Arc<String>,
}

impl DemoPsdPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        kind: DemoPackageKind,
        contained_elements: Vec<DemoPsdElement>,
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
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(DemoPsdPackage {
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
}

impl Entity for DemoPsdPackage {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for DemoPsdPackage {
    type ElementT = DemoPsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoPsdElement, ModelUuid)> {
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
    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoPsdElement,
    ) -> Result<PositionNoT, DemoPsdElement> {
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

impl FullTextSearchable for DemoPsdPackage {
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
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct DemoPsdStateInfo {
    #[nh_context_serde(entity)]
    pub state: DemoPsdState,
    pub executor: bool,
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdTransaction {
    pub uuid: Arc<ModelUuid>,
    pub kind: DemoTransactionKind,
    pub identifier: Arc<String>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    pub before: Vec<DemoPsdStateInfo>,
    #[nh_context_serde(entity)]
    pub p_act: UFOption<ERef<DemoPsdAct>>,
    #[nh_context_serde(entity)]
    pub after: Vec<DemoPsdStateInfo>,

    pub comment: Arc<String>,
}

impl DemoPsdTransaction {
    pub const CENTER_BUCKET: BucketNoT = 1;
    pub const BEFORE_INITIATOR_BUCKET: BucketNoT = 2;
    pub const BEFORE_EXECUTOR_BUCKET: BucketNoT = 3;
    pub const AFTER_EXECUTOR_BUCKET: BucketNoT = 4;
    pub const AFTER_INITIATOR_BUCKET: BucketNoT = 5;

    pub fn new(
        uuid: ModelUuid,
        kind: DemoTransactionKind,
        identifier: String,
        name: String,
        before: Vec<DemoPsdStateInfo>,
        p_act: UFOption<ERef<DemoPsdAct>>,
        after: Vec<DemoPsdStateInfo>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            identifier: Arc::new(identifier),
            name: Arc::new(name),
            before,
            p_act,
            after,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(DemoPsdTransaction {
            uuid: new_uuid.into(),
            kind: self.kind,
            identifier: self.identifier.clone(),
            name: self.name.clone(),
            before: self
                .before
                .iter()
                .map(|e| DemoPsdStateInfo {
                    state: e.state.deep_copy_clone(ModelUuid::now_v7(), into),
                    executor: e.executor,
                })
                .collect(),
            p_act: self
                .p_act
                .as_ref()
                .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into))
                .into(),
            after: self
                .after
                .iter()
                .map(|e| DemoPsdStateInfo {
                    state: e.state.deep_copy_clone(ModelUuid::now_v7(), into),
                    executor: e.executor,
                })
                .collect(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn move_element(&mut self, element: &ModelUuid, pos: PositionNoT) {
        if let Some((idx, _e)) = self
            .before
            .iter()
            .enumerate()
            .find(|e| *e.1.state.uuid() == *element)
        {
            let e = self.before.remove(idx);
            self.before.insert(pos.try_into().unwrap(), e);
        }
        if let Some((idx, _e)) = self
            .after
            .iter()
            .enumerate()
            .find(|e| *e.1.state.uuid() == *element)
        {
            let e = self.after.remove(idx);
            self.after.insert(pos.try_into().unwrap(), e);
        }
    }
}

impl Entity for DemoPsdTransaction {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdTransaction {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for DemoPsdTransaction {
    type ElementT = DemoPsdElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(DemoPsdElement, ModelUuid)> {
        for e in &self.before {
            if *e.state.uuid() == *uuid {
                return Some((e.state.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.state.find_element(uuid) {
                return Some(e);
            }
        }
        if let UFOption::Some(e) = &self.p_act {
            let r = e.read();
            if *r.uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        for e in &self.after {
            if *e.state.uuid() == *uuid {
                return Some((e.state.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.state.find_element(uuid) {
                return Some(e);
            }
        }
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.before.iter().enumerate() {
            if *e.state.uuid() == *uuid {
                return Some((
                    if !e.executor {
                        Self::BEFORE_INITIATOR_BUCKET
                    } else {
                        Self::BEFORE_EXECUTOR_BUCKET
                    },
                    idx.try_into().unwrap(),
                ));
            }
        }
        if let UFOption::Some(e) = &self.p_act
            && *e.read().uuid == *uuid
        {
            return Some((Self::CENTER_BUCKET, 0));
        }
        for (idx, e) in self.after.iter().enumerate() {
            if *e.state.uuid() == *uuid {
                return Some((
                    if !e.executor {
                        Self::AFTER_INITIATOR_BUCKET
                    } else {
                        Self::AFTER_EXECUTOR_BUCKET
                    },
                    idx.try_into().unwrap(),
                ));
            }
        }
        None
    }
    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: DemoPsdElement,
    ) -> Result<PositionNoT, DemoPsdElement> {
        if bucket == Self::CENTER_BUCKET {
            if !self.p_act.is_some()
                && let DemoPsdElement::Act(act) = element
            {
                self.p_act = UFOption::Some(act.clone());
                Ok(0)
            } else {
                Err(element)
            }
        } else if let Some(state) = element.clone().to_state() {
            let after = match bucket {
                0 | Self::BEFORE_INITIATOR_BUCKET | Self::BEFORE_EXECUTOR_BUCKET => false,
                Self::AFTER_EXECUTOR_BUCKET | Self::AFTER_INITIATOR_BUCKET => true,
                _ => return Err(element),
            };
            let executor = match bucket {
                0 | Self::BEFORE_INITIATOR_BUCKET | Self::AFTER_INITIATOR_BUCKET => false,
                Self::BEFORE_EXECUTOR_BUCKET | Self::AFTER_EXECUTOR_BUCKET => true,
                _ => unreachable!(),
            };
            if !after {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.before.len());
                self.before
                    .insert(pos, DemoPsdStateInfo { state, executor });
                Ok(pos.try_into().unwrap())
            } else {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.after.len());
                self.after.insert(pos, DemoPsdStateInfo { state, executor });
                Ok(pos.try_into().unwrap())
            }
        } else {
            Err(element)
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.before.iter().enumerate() {
            if *e.state.uuid() == *uuid {
                let is_executor = e.executor;
                self.before.remove(idx);
                return Some((
                    if !is_executor {
                        Self::BEFORE_INITIATOR_BUCKET
                    } else {
                        Self::BEFORE_EXECUTOR_BUCKET
                    },
                    idx.try_into().unwrap(),
                ));
            }
        }
        if let UFOption::Some(e) = &self.p_act
            && *e.read().uuid == *uuid
        {
            self.p_act = UFOption::None;
            return Some((Self::CENTER_BUCKET, 0));
        }
        for (idx, e) in self.after.iter().enumerate() {
            if *e.state.uuid() == *uuid {
                let is_executor = e.executor;
                self.after.remove(idx);
                return Some((
                    if !is_executor {
                        Self::AFTER_INITIATOR_BUCKET
                    } else {
                        Self::AFTER_EXECUTOR_BUCKET
                    },
                    idx.try_into().unwrap(),
                ));
            }
        }
        None
    }
}

impl FullTextSearchable for DemoPsdTransaction {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.name, &self.comment],
        );

        for e in &self.before {
            e.state.full_text_search(acc);
        }
        if let UFOption::Some(e) = &self.p_act {
            e.read().full_text_search(acc);
        }
        for e in &self.after {
            e.state.full_text_search(acc);
        }
    }
}

// "Disc"
#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdFact {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub identifier: Arc<String>,
    #[full_text_searchable(skip)]
    pub internal: bool,
    pub comment: Arc<String>,
}

impl DemoPsdFact {
    pub fn new(uuid: ModelUuid, identifier: String, internal: bool) -> Self {
        Self {
            uuid: Arc::new(uuid),
            identifier: Arc::new(identifier),
            internal,
            comment: Arc::new("".to_owned()),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            identifier: self.identifier.clone(),
            internal: self.internal,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for DemoPsdFact {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdFact {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

// "Box"
#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdAct {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub identifier: Arc<String>,
    #[full_text_searchable(skip)]
    pub internal: bool,
    pub comment: Arc<String>,
}

impl DemoPsdAct {
    pub fn new(uuid: ModelUuid, identifier: String, internal: bool) -> Self {
        Self {
            uuid: Arc::new(uuid),
            identifier: Arc::new(identifier),
            internal,
            comment: Arc::new("".to_owned()),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            identifier: self.identifier.clone(),
            internal: self.internal,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for DemoPsdAct {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdAct {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DemoPsdLinkType {
    #[default]
    ResponseLink,
    WaitLink,
}

impl DemoPsdLinkType {
    pub const VARIANTS: [Self; 2] = [Self::ResponseLink, Self::WaitLink];

    pub fn as_str(&self) -> &'static str {
        match self {
            DemoPsdLinkType::ResponseLink => "Response Link",
            DemoPsdLinkType::WaitLink => "Wait Link",
        }
    }

    pub fn line_type(&self) -> canvas::LineType {
        match self {
            DemoPsdLinkType::ResponseLink => canvas::LineType::Solid,
            DemoPsdLinkType::WaitLink => canvas::LineType::Dashed,
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdLink {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,

    #[full_text_searchable(skip)]
    pub link_type: DemoPsdLinkType,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<DemoPsdFact>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: ERef<DemoPsdAct>,
    pub multiplicity: Arc<String>,

    pub comment: Arc<String>,
}

impl DemoPsdLink {
    pub fn new(
        uuid: ModelUuid,
        link_type: DemoPsdLinkType,
        multiplicity: Arc<String>,
        source: ERef<DemoPsdFact>,
        target: ERef<DemoPsdAct>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            link_type,
            source,
            target,
            multiplicity,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            link_type: self.link_type,
            source: self.source.clone(),
            target: self.target.clone(),
            multiplicity: self.multiplicity.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, m: &HashMap<ModelUuid, DemoPsdElement>) {
        let source_uuid = *self.source.read().uuid();
        if let Some(DemoPsdElement::Fact(ta)) = m.get(&source_uuid) {
            self.source = ta.clone();
        }
        let target_uuid = *self.target.read().uuid();
        if let Some(DemoPsdElement::Act(tx)) = m.get(&target_uuid) {
            self.target = tx.clone();
        }
    }
}

impl Entity for DemoPsdLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdNote {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl DemoPsdNote {
    pub fn new(uuid: ModelUuid, text: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            text: self.text.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for DemoPsdNote {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for DemoPsdNote {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
