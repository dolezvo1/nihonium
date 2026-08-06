use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::model::{
    BucketNoT, ContainerModel, DiagramModel, DiagramVisitor, ElementVisitor, Model, PositionNoT,
    VisitableDiagram, VisitableElement,
};
use crate::common::search::FullTextSearchable;
use crate::common::uuid::ModelUuid;

pub fn deep_copy_diagram(
    d: &UmlActivityDiagram,
) -> (
    ERef<UmlActivityDiagram>,
    HashMap<ModelUuid, UmlActivityElement>,
) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = UmlActivityDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &UmlActivityDiagram) -> HashMap<ModelUuid, UmlActivityElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(&e.clone().to_element(), &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &UmlActivityElement, into: &mut HashMap<ModelUuid, UmlActivityElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        UmlActivityElement::Activity(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        UmlActivityElement::InterruptibleRegion(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        UmlActivityElement::Partition(inner) => {
            for e in &inner.read().sections {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        UmlActivityElement::PartitionSection(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        _ => {}
    }
}

pub fn transitive_closure(
    d: &UmlActivityDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &UmlActivityElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                UmlActivityElement::Activity(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.contained_elements {
                            walk(&e.clone().to_element(), when_deleting);
                        }
                    }
                }
                UmlActivityElement::InterruptibleRegion(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.contained_elements {
                            walk(&e.clone().to_element(), when_deleting);
                        }
                    }
                }
                UmlActivityElement::Partition(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.sections {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                }
                UmlActivityElement::PartitionSection(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.contained_elements {
                            walk(&e.clone().to_element(), when_deleting);
                        }
                    }
                }
                _ => {}
            }
        }
        walk(&e.clone().to_element(), &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &UmlActivityElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                UmlActivityElement::Activity(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlActivityElement::InterruptibleRegion(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlActivityElement::Partition(inner) => {
                    for e in &inner.read().sections {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                UmlActivityElement::PartitionSection(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlActivityElement::ActionNode(..)
                | UmlActivityElement::InitialNode(..)
                | UmlActivityElement::FinalNode(..)
                | UmlActivityElement::DecisionNode(..)
                | UmlActivityElement::ForkNode(..)
                | UmlActivityElement::ObjectNode(..)
                | UmlActivityElement::Note(..) => {}
                UmlActivityElement::Edge(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                UmlActivityElement::NoteLink(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid)
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
            }
        }
        for e in &d.contained_elements {
            walk(&e.clone().to_element(), &when_deleting, &mut also_delete);
        }
        if also_delete.is_empty() {
            break;
        }
        when_deleting.extend(also_delete.drain());
    }

    when_deleting
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
#[container_model(element_type = UmlActivityElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlActivityElement {
    #[container_model(passthrough = "eref")]
    Activity(ERef<UmlActivity>),
    #[container_model(passthrough = "eref")]
    InterruptibleRegion(ERef<UmlActivityInterruptibleRegion>),
    #[container_model(passthrough = "eref")]
    Partition(ERef<UmlActivityPartition>),
    #[container_model(passthrough = "eref")]
    PartitionSection(ERef<UmlActivityPartitionSection>),
    ActionNode(ERef<UmlActivityActionNode>),
    InitialNode(ERef<UmlActivityInitialNode>),
    FinalNode(ERef<UmlActivityFinalNode>),
    DecisionNode(ERef<UmlActivityDecisionNode>),
    ForkNode(ERef<UmlActivityForkNode>),
    ObjectNode(ERef<UmlActivityObjectNode>),
    Edge(ERef<UmlActivityFlowEdge>),
    Note(ERef<UmlActivityNote>),
    NoteLink(ERef<UmlActivityNoteLink>),
}

impl UmlActivityElement {
    pub fn as_standalone(&self) -> Option<UmlActivityStandaloneElement> {
        match &self {
            Self::Activity(inner) => Some(inner.clone().into()),
            Self::InterruptibleRegion(inner) => Some(inner.clone().into()),
            Self::Partition(inner) => Some(inner.clone().into()),
            Self::PartitionSection(_) => None,
            Self::ActionNode(inner) => Some(inner.clone().into()),
            Self::InitialNode(inner) => Some(inner.clone().into()),
            Self::FinalNode(inner) => Some(inner.clone().into()),
            Self::DecisionNode(inner) => Some(inner.clone().into()),
            Self::ForkNode(inner) => Some(inner.clone().into()),
            Self::ObjectNode(inner) => Some(inner.clone().into()),
            Self::Edge(inner) => Some(inner.clone().into()),
            Self::Note(inner) => Some(inner.clone().into()),
            Self::NoteLink(inner) => Some(inner.clone().into()),
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> Self {
        match self {
            Self::Activity(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::InterruptibleRegion(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::Partition(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::PartitionSection(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::ActionNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::InitialNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::FinalNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::DecisionNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::ForkNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::ObjectNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Edge(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::NoteLink(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }

    pub fn deep_copy_relink(&self, all_models: &HashMap<ModelUuid, UmlActivityElement>) {
        match self {
            Self::Activity(..)
            | Self::InterruptibleRegion(..)
            | Self::Partition(..)
            | Self::PartitionSection(..) => {}
            Self::ActionNode(..)
            | Self::InitialNode(..)
            | Self::FinalNode(..)
            | Self::DecisionNode(..)
            | Self::ForkNode(..)
            | Self::ObjectNode(..)
            | Self::Note(..) => {}
            Self::Edge(inner) => inner.write().deep_copy_relink(all_models),
            Self::NoteLink(inner) => inner.write().deep_copy_relink(all_models),
        }
    }
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
#[container_model(element_type = UmlActivityElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlActivityStandaloneElement {
    #[container_model(passthrough = "eref")]
    Activity(ERef<UmlActivity>),
    #[container_model(passthrough = "eref")]
    InterruptibleRegion(ERef<UmlActivityInterruptibleRegion>),
    #[container_model(passthrough = "eref")]
    Partition(ERef<UmlActivityPartition>),
    ActionNode(ERef<UmlActivityActionNode>),
    InitialNode(ERef<UmlActivityInitialNode>),
    FinalNode(ERef<UmlActivityFinalNode>),
    DecisionNode(ERef<UmlActivityDecisionNode>),
    ForkNode(ERef<UmlActivityForkNode>),
    ObjectNode(ERef<UmlActivityObjectNode>),
    Edge(ERef<UmlActivityFlowEdge>),
    Note(ERef<UmlActivityNote>),
    NoteLink(ERef<UmlActivityNoteLink>),
}

impl UmlActivityStandaloneElement {
    pub fn to_element(self) -> UmlActivityElement {
        match self {
            Self::Activity(inner) => inner.into(),
            Self::InterruptibleRegion(inner) => inner.into(),
            Self::Partition(inner) => inner.into(),
            Self::ActionNode(inner) => inner.into(),
            Self::InitialNode(inner) => inner.into(),
            Self::FinalNode(inner) => inner.into(),
            Self::DecisionNode(inner) => inner.into(),
            Self::ForkNode(inner) => inner.into(),
            Self::ObjectNode(inner) => inner.into(),
            Self::Edge(inner) => inner.into(),
            Self::Note(inner) => inner.into(),
            Self::NoteLink(inner) => inner.into(),
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> Self {
        match self {
            Self::Activity(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::InterruptibleRegion(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::Partition(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::ActionNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::InitialNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::FinalNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::DecisionNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::ForkNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::ObjectNode(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Edge(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::NoteLink(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlActivityNonFinalNode {
    ActionNode(ERef<UmlActivityActionNode>),
    InitialNode(ERef<UmlActivityInitialNode>),
    DecisionNode(ERef<UmlActivityDecisionNode>),
    ForkNode(ERef<UmlActivityForkNode>),
    ObjectNode(ERef<UmlActivityObjectNode>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlActivityNonInitialNode {
    ActionNode(ERef<UmlActivityActionNode>),
    FinalNode(ERef<UmlActivityFinalNode>),
    DecisionNode(ERef<UmlActivityDecisionNode>),
    ForkNode(ERef<UmlActivityForkNode>),
    ObjectNode(ERef<UmlActivityObjectNode>),
}

impl UmlActivityElement {
    pub fn as_nonfinal(&self) -> Option<UmlActivityNonFinalNode> {
        match self {
            UmlActivityElement::ActionNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::InitialNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::DecisionNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::ForkNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::ObjectNode(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
    pub fn as_noninitial(&self) -> Option<UmlActivityNonInitialNode> {
        match self {
            UmlActivityElement::ActionNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::FinalNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::DecisionNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::ForkNode(inner) => Some(inner.clone().into()),
            UmlActivityElement::ObjectNode(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
}
impl UmlActivityNonFinalNode {
    pub fn to_element(self) -> UmlActivityElement {
        match self {
            UmlActivityNonFinalNode::ActionNode(inner) => inner.into(),
            UmlActivityNonFinalNode::InitialNode(inner) => inner.into(),
            UmlActivityNonFinalNode::DecisionNode(inner) => inner.into(),
            UmlActivityNonFinalNode::ForkNode(inner) => inner.into(),
            UmlActivityNonFinalNode::ObjectNode(inner) => inner.into(),
        }
    }
}
impl UmlActivityNonInitialNode {
    pub fn to_element(self) -> UmlActivityElement {
        match self {
            UmlActivityNonInitialNode::ActionNode(inner) => inner.into(),
            UmlActivityNonInitialNode::FinalNode(inner) => inner.into(),
            UmlActivityNonInitialNode::DecisionNode(inner) => inner.into(),
            UmlActivityNonInitialNode::ForkNode(inner) => inner.into(),
            UmlActivityNonInitialNode::ObjectNode(inner) => inner.into(),
        }
    }
}

impl VisitableElement for UmlActivityElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            UmlActivityElement::Activity(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            UmlActivityElement::InterruptibleRegion(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            UmlActivityElement::Partition(inner) => {
                v.open_complex(self);
                for e in &inner.read().sections {
                    UmlActivityElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            UmlActivityElement::PartitionSection(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            e => v.visit_simple(e),
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct UmlActivityDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlActivityStandaloneElement>,

    pub comment: Arc<String>,
}

impl UmlActivityDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<UmlActivityStandaloneElement>,
    ) -> Self {
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
        element: UmlActivityElement,
    ) -> Result<PositionNoT, UmlActivityElement> {
        if bucket != 0 {
            return Err(element);
        }
        let Some(element) = element.as_standalone() else {
            return Err(element);
        };

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

    pub fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, UmlActivityElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &UmlActivityElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, UmlActivityElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                UmlActivityElement::Activity(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().to_element(),
                                0,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                UmlActivityElement::InterruptibleRegion(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().to_element(),
                                0,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                UmlActivityElement::Partition(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.sections.iter().enumerate() {
                        if uuids.contains(&e.read().uuid) {
                            undo.push((*w.uuid, e.clone().into(), 0, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.sections.retain(|e| !uuids.contains(&e.read().uuid));
                }
                UmlActivityElement::PartitionSection(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().to_element(),
                                0,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                _ => {}
            }
        }

        for (idx, e) in self.contained_elements.iter().enumerate() {
            if uuids.contains(&e.uuid()) {
                undo.push((
                    *self.uuid,
                    e.clone().to_element(),
                    0,
                    idx.try_into().unwrap(),
                ));
            } else {
                r(&e.clone().to_element(), uuids, undo);
            }
        }
        self.contained_elements
            .retain(|e| !uuids.contains(&e.uuid()));
    }
}

impl Entity for UmlActivityDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlActivityDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for UmlActivityDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.clone().to_element().accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for UmlActivityDiagram {
    type ElementT = UmlActivityElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlActivityElement, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
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

impl DiagramModel for UmlActivityDiagram {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        b: BucketNoT,
        p: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, UmlActivityElement> {
        if *self.uuid == target {
            self.insert_element_unsafe(b, p, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };
            match e {
                UmlActivityElement::Activity(inner) => inner.write().insert_element(b, p, element),
                UmlActivityElement::InterruptibleRegion(inner) => {
                    inner.write().insert_element(b, p, element)
                }
                UmlActivityElement::Partition(inner) => inner.write().insert_element(b, p, element),
                UmlActivityElement::PartitionSection(inner) => {
                    inner.write().insert_element(b, p, element)
                }
                UmlActivityElement::ActionNode(_)
                | UmlActivityElement::InitialNode(_)
                | UmlActivityElement::FinalNode(_)
                | UmlActivityElement::DecisionNode(_)
                | UmlActivityElement::ForkNode(_)
                | UmlActivityElement::ObjectNode(_)
                | UmlActivityElement::Edge(_)
                | UmlActivityElement::Note(_)
                | UmlActivityElement::NoteLink(_) => Err(element),
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
                UmlActivityElement::Activity(inner) => inner.write().remove_element(uuid),
                UmlActivityElement::InterruptibleRegion(inner) => {
                    inner.write().remove_element(uuid)
                }
                UmlActivityElement::Partition(inner) => inner.write().remove_element(uuid),
                UmlActivityElement::PartitionSection(inner) => inner.write().remove_element(uuid),
                UmlActivityElement::ActionNode(_)
                | UmlActivityElement::InitialNode(_)
                | UmlActivityElement::FinalNode(_)
                | UmlActivityElement::DecisionNode(_)
                | UmlActivityElement::ForkNode(_)
                | UmlActivityElement::ObjectNode(_)
                | UmlActivityElement::Edge(_)
                | UmlActivityElement::Note(_)
                | UmlActivityElement::NoteLink(_) => None,
            }
        }
    }
}

impl FullTextSearchable for UmlActivityDiagram {
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
pub struct UmlActivity {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    pub parameters: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlActivityStandaloneElement>,

    pub comment: Arc<String>,
}

impl UmlActivity {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        parameters: String,
        contained_elements: Vec<UmlActivityStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            parameters: Arc::new(parameters),
            contained_elements,
            comment: "".to_owned().into(),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlActivity {
            uuid: new_uuid.into(),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            parameters: self.stereotype.clone(),
            contained_elements: self
                .contained_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model.into()
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, UmlActivityElement> {
        if bucket != 0 {
            return Err(element);
        }
        let Some(element) = element.as_standalone() else {
            return Err(element);
        };

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

impl Model for UmlActivity {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivity {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlActivity {
    type ElementT = UmlActivityElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
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

impl FullTextSearchable for UmlActivity {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.stereotype,
                &self.name,
                &self.parameters,
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
pub struct UmlActivityInterruptibleRegion {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlActivityStandaloneElement>,
}

impl UmlActivityInterruptibleRegion {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        contained_elements: Vec<UmlActivityStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            contained_elements,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlActivityInterruptibleRegion {
            uuid: new_uuid.into(),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            contained_elements: self
                .contained_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, UmlActivityElement> {
        if bucket != 0 {
            return Err(element);
        }
        let Some(element) = element.as_standalone() else {
            return Err(element);
        };

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

impl Model for UmlActivityInterruptibleRegion {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityInterruptibleRegion {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlActivityInterruptibleRegion {
    type ElementT = UmlActivityElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
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

impl FullTextSearchable for UmlActivityInterruptibleRegion {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.stereotype, &self.name],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityPartition {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub sections: Vec<ERef<UmlActivityPartitionSection>>,
}

impl UmlActivityPartition {
    pub fn new(
        uuid: ModelUuid,
        contained_elements: Vec<ERef<UmlActivityPartitionSection>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            sections: contained_elements,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlActivityPartition {
            uuid: new_uuid.into(),
            sections: self
                .sections
                .iter()
                .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into))
                .collect(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    pub fn move_element(
        &mut self,
        element: &ModelUuid,
        within: BucketNoT,
        target_pos: PositionNoT,
    ) {
        if within == 0
            && let Some((idx, _e)) = self
                .sections
                .iter()
                .enumerate()
                .find(|e| *e.1.read().uuid() == *element)
        {
            let e = self.sections.remove(idx);
            self.sections.insert(target_pos.try_into().unwrap(), e);
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, UmlActivityElement> {
        if bucket != 0 {
            return Err(element);
        }
        let UmlActivityElement::PartitionSection(element) = element else {
            return Err(element);
        };

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.sections.len());
        self.sections.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.sections.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.sections.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl Model for UmlActivityPartition {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityPartition {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlActivityPartition {
    type ElementT = UmlActivityElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.sections {
            if *e.read().uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
            if let Some(e) = e.read().find_element(uuid) {
                return Some(e);
            }
        }
        None
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.sections.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlActivityPartition {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(*self.uuid, &[&self.uuid.to_string()]);

        for e in &self.sections {
            e.read().full_text_search(acc);
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityPartitionSection {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlActivityStandaloneElement>,
}

impl UmlActivityPartitionSection {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        contained_elements: Vec<UmlActivityStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            contained_elements,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlActivityPartitionSection {
            uuid: new_uuid.into(),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            contained_elements: self
                .contained_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model.into()
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, UmlActivityElement> {
        if bucket != 0 {
            return Err(element);
        }
        let Some(element) = element.as_standalone() else {
            return Err(element);
        };

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

impl Model for UmlActivityPartitionSection {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityPartitionSection {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlActivityPartitionSection {
    type ElementT = UmlActivityElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
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

impl FullTextSearchable for UmlActivityPartitionSection {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.stereotype, &self.name],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
pub enum UmlActivityActionKind {
    #[default]
    Basic,
    CallAction,
    SendSignalAction,
    AcceptSignalAction,
    WaitTimeAction,
}

impl UmlActivityActionKind {
    pub const VARIANTS: [Self; 5] = [
        Self::Basic,
        Self::CallAction,
        Self::SendSignalAction,
        Self::AcceptSignalAction,
        Self::WaitTimeAction,
    ];

    pub fn as_str(&self) -> &str {
        match self {
            UmlActivityActionKind::Basic => "Basic",
            UmlActivityActionKind::CallAction => "Call Action",
            UmlActivityActionKind::SendSignalAction => "Send Signal Action",
            UmlActivityActionKind::AcceptSignalAction => "Accept Signal Action",
            UmlActivityActionKind::WaitTimeAction => "Wait Time Action",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityActionNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: UmlActivityActionKind,
}

impl UmlActivityActionNode {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        kind: UmlActivityActionKind,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            kind,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            kind: self.kind,
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for UmlActivityActionNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityActionNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityInitialNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlActivityInitialNode {
    pub fn new(uuid: ModelUuid) -> Self {
        Self {
            uuid: Arc::new(uuid),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for UmlActivityInitialNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityInitialNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlActivityFinalNodeKind {
    #[default]
    FlowFinal,
    ActivityFinal,
}

impl UmlActivityFinalNodeKind {
    pub const VARIANTS: [Self; 2] = [Self::FlowFinal, Self::ActivityFinal];

    pub fn as_str(&self) -> &str {
        match self {
            UmlActivityFinalNodeKind::FlowFinal => "Flow Final",
            UmlActivityFinalNodeKind::ActivityFinal => "Activity Final",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityFinalNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: UmlActivityFinalNodeKind,
}

impl UmlActivityFinalNode {
    pub fn new(uuid: ModelUuid, kind: UmlActivityFinalNodeKind) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            kind: self.kind,
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for UmlActivityFinalNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityFinalNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityDecisionNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
}

impl UmlActivityDecisionNode {
    pub fn new(uuid: ModelUuid, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for UmlActivityDecisionNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityDecisionNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityForkNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlActivityForkNode {
    pub fn new(uuid: ModelUuid) -> Self {
        Self {
            uuid: Arc::new(uuid),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for UmlActivityForkNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityForkNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityObjectNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
}

impl UmlActivityObjectNode {
    pub fn new(uuid: ModelUuid, stereotype: String, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: Arc::new(new_uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for UmlActivityObjectNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityObjectNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlActivityEdgeKind {
    #[default]
    Regular,
    Interrupting,
}

impl UmlActivityEdgeKind {
    pub const VARIANTS: [Self; 2] = [Self::Regular, Self::Interrupting];

    pub fn as_str(&self) -> &str {
        match self {
            UmlActivityEdgeKind::Regular => "Regular",
            UmlActivityEdgeKind::Interrupting => "Interrupting",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityFlowEdge {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: UmlActivityEdgeKind,

    #[nh_context_serde(entity)]
    #[full_text_searchable(skip)]
    pub source: UmlActivityNonFinalNode,
    #[nh_context_serde(entity)]
    #[full_text_searchable(skip)]
    pub target: UmlActivityNonInitialNode,
}

impl UmlActivityFlowEdge {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        kind: UmlActivityEdgeKind,
        source: UmlActivityNonFinalNode,
        target: UmlActivityNonInitialNode,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            source,
            target,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            kind: self.kind,
            source: self.source.clone(),
            target: self.target.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, UmlActivityElement>) {
        let source_uuid = *self.source.uuid();
        if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_nonfinal()) {
            self.source = s;
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_noninitial()) {
            self.target = t;
        }
    }
}

impl Model for UmlActivityFlowEdge {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlActivityFlowEdge {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityNote {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub text: Arc<String>,
}

impl UmlActivityNote {
    pub fn new(uuid: ModelUuid, stereotype: String, text: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            text: Arc::new(text),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            stereotype: self.stereotype.clone(),
            text: self.text.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for UmlActivityNote {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlActivityNote {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityNoteLink {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<UmlActivityNote>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: UmlActivityElement,
}

impl UmlActivityNoteLink {
    pub fn new(uuid: ModelUuid, source: ERef<UmlActivityNote>, target: UmlActivityElement) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            source: self.source.clone(),
            target: self.target.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, UmlActivityElement>) {
        let source_uuid = *self.source.read().uuid();
        if let Some(UmlActivityElement::Note(s)) = all_models.get(&source_uuid) {
            self.source = s.clone();
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid) {
            self.target = t.clone();
        }
    }
}

impl Entity for UmlActivityNoteLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlActivityNoteLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
