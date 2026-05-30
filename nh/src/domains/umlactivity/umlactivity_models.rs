
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::common::controller::{BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT, VisitableDiagram, VisitableElement};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::search::FullTextSearchable;
use crate::common::uuid::ModelUuid;



pub fn deep_copy_diagram(d: &UmlActivityDiagram) -> (ERef<UmlActivityDiagram>, HashMap<ModelUuid, UmlActivityElement>) {
    fn walk(e: &UmlActivityElement, into: &mut HashMap<ModelUuid, UmlActivityElement>) -> UmlActivityElement {
        let new_uuid = ModelUuid::now_v7().into();
        match e {
            UmlActivityElement::Activity(inner) => {
                let model = inner.read();

                let new_model = UmlActivity {
                    uuid: new_uuid,
                    stereotype: model.stereotype.clone(),
                    name: model.name.clone(),
                    parameters: model.stereotype.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                ERef::new(new_model).into()
            },
            UmlActivityElement::InterruptibleRegion(inner) => {
                let model = inner.read();

                let new_model = UmlActivityInterruptibleRegion {
                    uuid: new_uuid,
                    stereotype: model.stereotype.clone(),
                    name: model.name.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                };
                ERef::new(new_model).into()
            },
            UmlActivityElement::ActionNode(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::InitialNode(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::FinalNode(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::DecisionNode(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::ForkNode(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::ObjectNode(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::Edge(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::Comment(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlActivityElement::CommentLink(inner) => inner.read().clone_with(*new_uuid).into(),
        }
    }

    fn relink(e: &mut UmlActivityElement, all_models: &HashMap<ModelUuid, UmlActivityElement>) {
        match e {
            UmlActivityElement::Activity(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            UmlActivityElement::InterruptibleRegion(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            UmlActivityElement::ActionNode(..)
            | UmlActivityElement::InitialNode(..)
            | UmlActivityElement::FinalNode(..)
            | UmlActivityElement::DecisionNode(..)
            | UmlActivityElement::ForkNode(..)
            | UmlActivityElement::ObjectNode(..)
            | UmlActivityElement::Comment(..) => {},
            UmlActivityElement::Edge(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.uuid();
                if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_nonfinal()) {
                    model.source = s;
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_noninitial()) {
                    model.target = t;
                }
            },
            UmlActivityElement::CommentLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlActivityElement::Comment(s)) = all_models.get(&source_uuid) {
                    model.source = s.clone().into();
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid) {
                    model.target = t.clone();
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

    let new_diagram = UmlActivityDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &UmlActivityDiagram) -> HashMap<ModelUuid, UmlActivityElement> {
    fn walk(e: &UmlActivityElement, into: &mut HashMap<ModelUuid, UmlActivityElement>) {
        match e {
            UmlActivityElement::Activity(inner) => {
                let model = inner.read();

                for e in &model.contained_elements {
                    walk(e, into);
                    into.insert(*e.uuid(), e.clone());
                }
            },
            UmlActivityElement::InterruptibleRegion(inner) => {
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

pub fn transitive_closure(d: &UmlActivityDiagram, mut when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &UmlActivityElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                UmlActivityElement::Activity(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        enumerate(e, when_deleting);
                    } else {
                        for e in &r.contained_elements {
                            walk(e, when_deleting);
                        }
                    }
                },
                UmlActivityElement::InterruptibleRegion(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        enumerate(e, when_deleting);
                    } else {
                        for e in &r.contained_elements {
                            walk(e, when_deleting);
                        }
                    }
                },
                _ => {}
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(e: &UmlActivityElement, when_deleting: &HashSet<ModelUuid>, also_delete: &mut HashSet<ModelUuid>) {
            match e {
                UmlActivityElement::Activity(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                },
                UmlActivityElement::InterruptibleRegion(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                },
                UmlActivityElement::ActionNode(..)
                | UmlActivityElement::InitialNode(..)
                | UmlActivityElement::FinalNode(..)
                | UmlActivityElement::DecisionNode(..)
                | UmlActivityElement::ForkNode(..)
                | UmlActivityElement::ObjectNode(..)
                | UmlActivityElement::Comment(..) => {},
                UmlActivityElement::Edge(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
                UmlActivityElement::CommentLink(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid)
                            || when_deleting.contains(&r.target.uuid())) {
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

fn enumerate(e: &UmlActivityElement, into: &mut HashSet<ModelUuid>) {
    into.insert(*e.uuid());
    match e {
        UmlActivityElement::Activity(inner) => {
            for e in &inner.read().contained_elements {
                enumerate(e, into);
            }
        },
        UmlActivityElement::InterruptibleRegion(inner) => {
            for e in &inner.read().contained_elements {
                enumerate(e, into);
            }
        },
        _ => {},
    }
}



#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::FullTextSearchable, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlActivityElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlActivityElement {
    #[container_model(passthrough = "eref")]
    Activity(ERef<UmlActivity>),
    #[container_model(passthrough = "eref")]
    InterruptibleRegion(ERef<UmlActivityInterruptibleRegion>),
    ActionNode(ERef<UmlActivityActionNode>),
    InitialNode(ERef<UmlActivityInitialNode>),
    FinalNode(ERef<UmlActivityFinalNode>),
    DecisionNode(ERef<UmlActivityDecisionNode>),
    ForkNode(ERef<UmlActivityForkNode>),
    ObjectNode(ERef<UmlActivityObjectNode>),
    Edge(ERef<UmlActivityFlowEdge>),
    Comment(ERef<UmlActivityComment>),
    CommentLink(ERef<UmlActivityCommentLink>),
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
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            UmlActivityElement::Activity(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            },
            UmlActivityElement::InterruptibleRegion(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            },
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
    pub contained_elements: Vec<UmlActivityElement>,

    pub comment: Arc<String>,
}

impl UmlActivityDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<UmlActivityElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }

    pub fn get_element_pos_in(&self, parent: &ModelUuid, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if *parent == *self.uuid {
            self.get_element_pos(uuid)
        } else {
            self.find_element(parent).and_then(|e| e.0.get_element_pos(uuid))
        }
    }

    pub fn insert_element_into(&mut self, parent: ModelUuid, element: UmlActivityElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
        if *self.uuid == parent {
            self.insert_element(b, p, element)
                .map(|_| ())
                .map_err(|_| ())
        } else {
            self.find_element(&parent)
                .ok_or(())
                .and_then(|mut e| e.0
                    .insert_element(b, p, element)
                    .map(|_| ())
                    .map_err(|_| ())
                )
        }
    }

    pub fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlActivityElement, BucketNoT, PositionNoT)>) {
        fn r(e: &UmlActivityElement, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlActivityElement, BucketNoT, PositionNoT)>) {
            match e {
                UmlActivityElement::Activity(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((*w.uuid, e.clone(), 0, idx.try_into().unwrap()));
                        } else {
                            r(e, uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                },
                _ => {},
            }
        }

        for (idx, e) in self.contained_elements.iter().enumerate() {
            if uuids.contains(&e.uuid()) {
                undo.push((*self.uuid, e.clone(), 0, idx.try_into().unwrap()));
            } else {
                r(e, uuids, undo);
            }
        }
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
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
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for UmlActivityDiagram {
    type ElementT = UmlActivityElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlActivityElement, ModelUuid)> {
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
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        return None;
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlActivityElement) -> Result<PositionNoT, UmlActivityElement> {
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

impl FullTextSearchable for UmlActivityDiagram {
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
pub struct UmlActivity {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    pub parameters: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlActivityElement>,

    pub comment: Arc<String>,
}

impl UmlActivity {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        parameters: String,
        contained_elements: Vec<UmlActivityElement>,
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
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            contained_elements: self.contained_elements.clone(),
            comment: self.comment.clone(),
        })
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
                return Some((e.clone(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        return None;
    }

    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: Self::ElementT) -> Result<PositionNoT, Self::ElementT> {
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
    pub contained_elements: Vec<UmlActivityElement>,
}

impl UmlActivityInterruptibleRegion {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        contained_elements: Vec<UmlActivityElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            contained_elements,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            contained_elements: self.contained_elements.clone(),
        })
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
                return Some((e.clone(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        return None;
    }

    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: Self::ElementT) -> Result<PositionNoT, Self::ElementT> {
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

impl FullTextSearchable for UmlActivityInterruptibleRegion {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.stereotype,
                &self.name,
            ],
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
        Self::Basic, Self::CallAction,
        Self::SendSignalAction, Self::AcceptSignalAction,
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

#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            kind: self.kind,
        })
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityInitialNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlActivityInitialNode {
    pub fn new(
        uuid: ModelUuid,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
        })
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

#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityFinalNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: UmlActivityFinalNodeKind,
}

impl UmlActivityFinalNode {
    pub fn new(
        uuid: ModelUuid,
        kind: UmlActivityFinalNodeKind,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            kind: self.kind,
        })
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityDecisionNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
}

impl UmlActivityDecisionNode {
    pub fn new(
        uuid: ModelUuid,
        name: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            name: self.name.clone(),
        })
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityForkNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlActivityForkNode {
    pub fn new(
        uuid: ModelUuid,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
        })
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityObjectNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
}

impl UmlActivityObjectNode {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
        })
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

#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            name: self.name.clone(),
            kind: self.kind,
            source: self.source.clone(),
            target: self.target.clone(),
        })
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityComment {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub text: Arc<String>,
}

impl UmlActivityComment {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        text: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            text: Arc::new(text),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            text: self.text.clone(),
        })
    }
}

impl Entity for UmlActivityComment {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlActivityComment {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityCommentLink {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<UmlActivityComment>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: UmlActivityElement,
}

impl UmlActivityCommentLink {
    pub fn new(
        uuid: ModelUuid,
        source: ERef<UmlActivityComment>,
        target: UmlActivityElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            source: self.source.clone(),
            target: self.target.clone(),
        })
    }
}

impl Entity for UmlActivityCommentLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlActivityCommentLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
