use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::model::{
    BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT,
    VisitableDiagram, VisitableElement,
};
use crate::common::search::FullTextSearchable;
use crate::common::uuid::ModelUuid;

pub fn deep_copy_diagram(
    d: &UmlStateMachineDiagram,
) -> (
    ERef<UmlStateMachineDiagram>,
    HashMap<ModelUuid, UmlStateMachineElement>,
) {
    fn walk(
        e: &UmlStateMachineElement,
        into: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) -> UmlStateMachineElement {
        let new_uuid = ModelUuid::now_v7().into();
        match e {
            UmlStateMachineElement::StateMachine(inner) => {
                let model = inner.read();

                let new_model = UmlStateMachine {
                    uuid: new_uuid,
                    stereotype: model.stereotype.clone(),
                    name: model.name.clone(),
                    is_protocol: model.is_protocol,
                    contained_elements: model
                        .contained_elements
                        .iter()
                        .map(|e| {
                            let new_model = walk(&e.clone().to_element(), into);
                            into.insert(*e.uuid(), new_model.clone());
                            match new_model.as_standalone() {
                                Some(new_model) => new_model,
                                None => unreachable!(),
                            }
                        })
                        .collect(),
                    comment: model.comment.clone(),
                };
                ERef::new(new_model).into()
            }
            UmlStateMachineElement::CompositeState(inner) => {
                let model = inner.read();

                let new_model = UmlStateMachineCompositeState {
                    uuid: new_uuid,
                    stereotype: model.stereotype.clone(),
                    name: model.name.clone(),
                    internal_transitions: model
                        .internal_transitions
                        .iter()
                        .map(|e| {
                            let new_model = walk(&e.clone().into(), into);
                            if let UmlStateMachineElement::InternalTransition(new_model) = new_model
                            {
                                into.insert(*e.read().uuid(), new_model.clone().into());
                                new_model
                            } else {
                                e.clone()
                            }
                        })
                        .collect(),
                    regions: model
                        .regions
                        .iter()
                        .map(|e| {
                            let new_model = walk(&e.clone().into(), into);
                            if let UmlStateMachineElement::CompositeStateRegion(new_model) =
                                new_model
                            {
                                into.insert(*e.read().uuid(), new_model.clone().into());
                                new_model
                            } else {
                                e.clone()
                            }
                        })
                        .collect(),
                };
                ERef::new(new_model).into()
            }
            UmlStateMachineElement::CompositeStateRegion(inner) => {
                let model = inner.read();

                let new_model = UmlStateMachineCompositeStateRegion {
                    uuid: new_uuid,
                    contained_elements: model
                        .contained_elements
                        .iter()
                        .map(|e| {
                            let new_model = walk(&e.clone().to_element(), into);
                            into.insert(*e.uuid(), new_model.clone());
                            match new_model.as_standalone() {
                                Some(new_model) => new_model,
                                None => unreachable!(),
                            }
                        })
                        .collect(),
                };
                ERef::new(new_model).into()
            }
            UmlStateMachineElement::SimpleState(inner) => {
                let model = inner.read();

                let new_model = UmlStateMachineSimpleState {
                    uuid: new_uuid,
                    stereotype: model.stereotype.clone(),
                    name: model.name.clone(),
                    internal_transitions: model
                        .internal_transitions
                        .iter()
                        .map(|e| {
                            let new_model = walk(&e.clone().into(), into);
                            if let UmlStateMachineElement::InternalTransition(new_model) = new_model
                            {
                                into.insert(*e.read().uuid(), new_model.clone().into());
                                new_model
                            } else {
                                e.clone()
                            }
                        })
                        .collect(),
                };
                ERef::new(new_model).into()
            }
            UmlStateMachineElement::InternalTransition(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
            UmlStateMachineElement::InitialPseudostate(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
            UmlStateMachineElement::TerminatePseudostate(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
            UmlStateMachineElement::FinalState(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlStateMachineElement::Edge(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlStateMachineElement::Note(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlStateMachineElement::NoteLink(inner) => inner.read().clone_with(*new_uuid).into(),
        }
    }

    fn relink(
        e: &mut UmlStateMachineElement,
        all_models: &HashMap<ModelUuid, UmlStateMachineElement>,
    ) {
        match e {
            UmlStateMachineElement::StateMachine(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(&mut e.clone().to_element(), all_models);
                }
            }
            UmlStateMachineElement::CompositeState(inner) => {
                let mut model = inner.write();
                for e in model.internal_transitions.iter_mut() {
                    relink(&mut e.clone().into(), all_models);
                }
                for e in model.regions.iter_mut() {
                    relink(&mut e.clone().into(), all_models);
                }
            }
            UmlStateMachineElement::CompositeStateRegion(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(&mut e.clone().to_element(), all_models);
                }
            }
            UmlStateMachineElement::SimpleState(inner) => {
                let mut model = inner.write();
                for e in model.internal_transitions.iter_mut() {
                    relink(&mut e.clone().into(), all_models);
                }
            }
            UmlStateMachineElement::InternalTransition(..)
            | UmlStateMachineElement::InitialPseudostate(..)
            | UmlStateMachineElement::TerminatePseudostate(..)
            | UmlStateMachineElement::FinalState(..)
            | UmlStateMachineElement::Note(..) => {}
            UmlStateMachineElement::Edge(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.uuid();
                if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_nonfinal()) {
                    model.source = s;
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_noninitial()) {
                    model.target = t;
                }
            }
            UmlStateMachineElement::NoteLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlStateMachineElement::Note(s)) = all_models.get(&source_uuid) {
                    model.source = s.clone();
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid) {
                    model.target = t.clone();
                }
            }
        }
    }

    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        let new_model = walk(&e.clone().to_element(), &mut all_models);
        all_models.insert(*e.uuid(), new_model.clone());
        let new_model = match new_model.as_standalone() {
            Some(new_model) => new_model,
            None => unreachable!(),
        };
        new_contained_elements.push(new_model);
    }
    for e in new_contained_elements.iter_mut() {
        relink(&mut e.clone().to_element(), &all_models);
    }

    let new_diagram = UmlStateMachineDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &UmlStateMachineDiagram) -> HashMap<ModelUuid, UmlStateMachineElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(&e.clone().to_element(), &mut all_models);
    }
    all_models
}
fn enumerate_elements(
    e: &UmlStateMachineElement,
    into: &mut HashMap<ModelUuid, UmlStateMachineElement>,
) {
    into.insert(*e.uuid(), e.clone());
    match e {
        UmlStateMachineElement::StateMachine(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        UmlStateMachineElement::CompositeState(inner) => {
            for e in &inner.read().internal_transitions {
                enumerate_elements(&e.clone().into(), into);
            }
            for e in &inner.read().regions {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        UmlStateMachineElement::CompositeStateRegion(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        UmlStateMachineElement::SimpleState(inner) => {
            for e in &inner.read().internal_transitions {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        _ => {}
    }
}

pub fn transitive_closure(
    d: &UmlStateMachineDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &UmlStateMachineElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                UmlStateMachineElement::StateMachine(inner) => {
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
                UmlStateMachineElement::CompositeState(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.internal_transitions {
                            walk(&e.clone().into(), when_deleting);
                        }
                        for e in &r.regions {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                }
                UmlStateMachineElement::CompositeStateRegion(inner) => {
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
                UmlStateMachineElement::SimpleState(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.internal_transitions {
                            walk(&e.clone().into(), when_deleting);
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
            e: &UmlStateMachineElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                UmlStateMachineElement::StateMachine(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlStateMachineElement::CompositeState(inner) => {
                    let r = inner.read();
                    for e in &r.internal_transitions {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                    for e in &r.regions {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                UmlStateMachineElement::CompositeStateRegion(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlStateMachineElement::SimpleState(inner) => {
                    let r = inner.read();
                    for e in &r.internal_transitions {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                UmlStateMachineElement::InternalTransition(..)
                | UmlStateMachineElement::InitialPseudostate(..)
                | UmlStateMachineElement::TerminatePseudostate(..)
                | UmlStateMachineElement::FinalState(..)
                | UmlStateMachineElement::Note(..) => {}
                UmlStateMachineElement::Edge(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                UmlStateMachineElement::NoteLink(inner) => {
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
#[container_model(element_type = UmlStateMachineElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlStateMachineElement {
    #[container_model(passthrough = "eref")]
    StateMachine(ERef<UmlStateMachine>),
    #[container_model(passthrough = "eref")]
    CompositeState(ERef<UmlStateMachineCompositeState>),
    #[container_model(passthrough = "eref")]
    CompositeStateRegion(ERef<UmlStateMachineCompositeStateRegion>),
    SimpleState(ERef<UmlStateMachineSimpleState>),
    InternalTransition(ERef<UmlStateMachineInternalTransition>),
    InitialPseudostate(ERef<UmlStateMachineInitialPseudostate>),
    TerminatePseudostate(ERef<UmlStateMachineTerminatePseudostate>),
    FinalState(ERef<UmlStateMachineFinalState>),
    Edge(ERef<UmlStateMachineEdge>),
    Note(ERef<UmlStateMachineNote>),
    NoteLink(ERef<UmlStateMachineNoteLink>),
}

impl UmlStateMachineElement {
    pub fn as_standalone(&self) -> Option<UmlStateMachineStandaloneElement> {
        match &self {
            UmlStateMachineElement::StateMachine(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::CompositeState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::CompositeStateRegion(_) => None,
            UmlStateMachineElement::SimpleState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::InternalTransition(_) => None,
            UmlStateMachineElement::InitialPseudostate(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::TerminatePseudostate(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::FinalState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::Edge(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::Note(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::NoteLink(inner) => Some(inner.clone().into()),
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
#[container_model(element_type = UmlStateMachineElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlStateMachineStandaloneElement {
    #[container_model(passthrough = "eref")]
    StateMachine(ERef<UmlStateMachine>),
    #[container_model(passthrough = "eref")]
    CompositeState(ERef<UmlStateMachineCompositeState>),
    SimpleState(ERef<UmlStateMachineSimpleState>),
    InitialPseudostate(ERef<UmlStateMachineInitialPseudostate>),
    TerminatePseudostate(ERef<UmlStateMachineTerminatePseudostate>),
    FinalState(ERef<UmlStateMachineFinalState>),
    Edge(ERef<UmlStateMachineEdge>),
    Note(ERef<UmlStateMachineNote>),
    NoteLink(ERef<UmlStateMachineNoteLink>),
}

impl UmlStateMachineStandaloneElement {
    pub fn to_element(self) -> UmlStateMachineElement {
        match self {
            UmlStateMachineStandaloneElement::StateMachine(inner) => inner.into(),
            UmlStateMachineStandaloneElement::CompositeState(inner) => inner.into(),
            UmlStateMachineStandaloneElement::SimpleState(inner) => inner.into(),
            UmlStateMachineStandaloneElement::InitialPseudostate(inner) => inner.into(),
            UmlStateMachineStandaloneElement::TerminatePseudostate(inner) => inner.into(),
            UmlStateMachineStandaloneElement::FinalState(inner) => inner.into(),
            UmlStateMachineStandaloneElement::Edge(inner) => inner.into(),
            UmlStateMachineStandaloneElement::Note(inner) => inner.into(),
            UmlStateMachineStandaloneElement::NoteLink(inner) => inner.into(),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlStateMachineNonFinalNode {
    CompositeState(ERef<UmlStateMachineCompositeState>),
    SimpleState(ERef<UmlStateMachineSimpleState>),
    InitialPseudostate(ERef<UmlStateMachineInitialPseudostate>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlStateMachineNonInitialNode {
    CompositeState(ERef<UmlStateMachineCompositeState>),
    SimpleState(ERef<UmlStateMachineSimpleState>),
    TerminatePseudostate(ERef<UmlStateMachineTerminatePseudostate>),
    FinalState(ERef<UmlStateMachineFinalState>),
}

impl UmlStateMachineElement {
    pub fn as_nonfinal(&self) -> Option<UmlStateMachineNonFinalNode> {
        match self {
            UmlStateMachineElement::CompositeState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::SimpleState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::InitialPseudostate(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
    pub fn as_noninitial(&self) -> Option<UmlStateMachineNonInitialNode> {
        match self {
            UmlStateMachineElement::CompositeState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::SimpleState(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::TerminatePseudostate(inner) => Some(inner.clone().into()),
            UmlStateMachineElement::FinalState(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
}
impl UmlStateMachineNonFinalNode {
    pub fn to_element(self) -> UmlStateMachineElement {
        match self {
            UmlStateMachineNonFinalNode::CompositeState(inner) => inner.into(),
            UmlStateMachineNonFinalNode::SimpleState(inner) => inner.into(),
            UmlStateMachineNonFinalNode::InitialPseudostate(inner) => inner.into(),
        }
    }
}
impl UmlStateMachineNonInitialNode {
    pub fn to_element(self) -> UmlStateMachineElement {
        match self {
            UmlStateMachineNonInitialNode::CompositeState(inner) => inner.into(),
            UmlStateMachineNonInitialNode::SimpleState(inner) => inner.into(),
            UmlStateMachineNonInitialNode::TerminatePseudostate(inner) => inner.into(),
            UmlStateMachineNonInitialNode::FinalState(inner) => inner.into(),
        }
    }
}

impl VisitableElement for UmlStateMachineElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            UmlStateMachineElement::StateMachine(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            UmlStateMachineElement::CompositeState(inner) => {
                v.open_complex(self);
                for e in &inner.read().internal_transitions {
                    UmlStateMachineElement::from(e.clone()).accept(v);
                }
                for e in &inner.read().regions {
                    UmlStateMachineElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            UmlStateMachineElement::CompositeStateRegion(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            UmlStateMachineElement::SimpleState(inner) => {
                v.open_complex(self);
                for e in &inner.read().internal_transitions {
                    UmlStateMachineElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            e => v.visit_simple(e),
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct UmlStateMachineDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlStateMachineStandaloneElement>,

    pub comment: Arc<String>,
}

impl UmlStateMachineDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<UmlStateMachineStandaloneElement>,
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

    pub fn insert_element_into(
        &mut self,
        parent: ModelUuid,
        element: UmlStateMachineElement,
        b: BucketNoT,
        p: Option<PositionNoT>,
    ) -> Result<(), ()> {
        if *self.uuid == parent {
            self.insert_element(b, p, element)
                .map(|_| ())
                .map_err(|_| ())
        } else {
            self.find_element(&parent).ok_or(()).and_then(|mut e| {
                e.0.insert_element(b, p, element)
                    .map(|_| ())
                    .map_err(|_| ())
            })
        }
    }

    pub fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, UmlStateMachineElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &UmlStateMachineElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, UmlStateMachineElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                UmlStateMachineElement::StateMachine(inner) => {
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
                UmlStateMachineElement::CompositeState(inner) => {
                    let mut w = inner.write();

                    for (idx, e) in w.internal_transitions.iter().enumerate() {
                        if uuids.contains(&e.read().uuid) {
                            undo.push((*w.uuid, e.clone().into(), 1, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.internal_transitions
                        .retain(|e| !uuids.contains(&e.read().uuid));

                    if w.regions.iter().any(|e| !uuids.contains(&e.read().uuid)) {
                        for (idx, e) in w.regions.iter().enumerate() {
                            if uuids.contains(&e.read().uuid) {
                                undo.push((*w.uuid, e.clone().into(), 2, idx.try_into().unwrap()));
                            } else {
                                r(&e.clone().into(), uuids, undo);
                            }
                        }
                        w.regions.retain(|e| !uuids.contains(&e.read().uuid));
                    }
                }
                UmlStateMachineElement::CompositeStateRegion(inner) => {
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
                UmlStateMachineElement::SimpleState(inner) => {
                    let mut w = inner.write();

                    for (idx, e) in w.internal_transitions.iter().enumerate() {
                        if uuids.contains(&e.read().uuid) {
                            undo.push((*w.uuid, e.clone().into(), 1, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.internal_transitions
                        .retain(|e| !uuids.contains(&e.read().uuid));
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

impl Entity for UmlStateMachineDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlStateMachineDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for UmlStateMachineDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.clone().to_element().accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for UmlStateMachineDiagram {
    type ElementT = UmlStateMachineElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlStateMachineElement, ModelUuid)> {
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
    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlStateMachineElement,
    ) -> Result<PositionNoT, UmlStateMachineElement> {
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

impl FullTextSearchable for UmlStateMachineDiagram {
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
pub struct UmlStateMachine {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    pub is_protocol: bool,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlStateMachineStandaloneElement>,

    pub comment: Arc<String>,
}

impl UmlStateMachine {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        is_protocol: bool,
        contained_elements: Vec<UmlStateMachineStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            is_protocol,
            contained_elements,
            comment: "".to_owned().into(),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            is_protocol: self.is_protocol,
            contained_elements: self.contained_elements.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Model for UmlStateMachine {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachine {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlStateMachine {
    type ElementT = UmlStateMachineElement;

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

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: Self::ElementT,
    ) -> Result<PositionNoT, Self::ElementT> {
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

impl FullTextSearchable for UmlStateMachine {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.stereotype,
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
pub struct UmlStateMachineCompositeState {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub internal_transitions: Vec<ERef<UmlStateMachineInternalTransition>>,
    #[nh_context_serde(entity)]
    pub regions: Vec<ERef<UmlStateMachineCompositeStateRegion>>,
}

impl UmlStateMachineCompositeState {
    pub const INTERNAL_TRANSITIONS_BUCKET: BucketNoT = 1;
    pub const REGIONS_BUCKET: BucketNoT = 2;

    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        internal_transitions: Vec<ERef<UmlStateMachineInternalTransition>>,
        regions: Vec<ERef<UmlStateMachineCompositeStateRegion>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            internal_transitions,
            regions,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            internal_transitions: self.internal_transitions.clone(),
            regions: self.regions.clone(),
        })
    }

    pub fn move_element(
        &mut self,
        element: &ModelUuid,
        within: BucketNoT,
        target_pos: PositionNoT,
    ) {
        if within == Self::INTERNAL_TRANSITIONS_BUCKET
            && let Some((idx, _e)) = self
                .internal_transitions
                .iter()
                .enumerate()
                .find(|e| *e.1.read().uuid() == *element)
        {
            let e = self.regions.remove(idx);
            self.regions.insert(target_pos.try_into().unwrap(), e);
        }

        if within == Self::REGIONS_BUCKET
            && let Some((idx, _e)) = self
                .regions
                .iter()
                .enumerate()
                .find(|e| *e.1.read().uuid() == *element)
        {
            let e = self.regions.remove(idx);
            self.regions.insert(target_pos.try_into().unwrap(), e);
        }
    }
}

impl Model for UmlStateMachineCompositeState {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineCompositeState {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlStateMachineCompositeState {
    type ElementT = UmlStateMachineElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.internal_transitions {
            if *e.read().uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        for e in &self.regions {
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
        for (idx, e) in self.internal_transitions.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((Self::INTERNAL_TRANSITIONS_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.regions.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((Self::REGIONS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: Self::ElementT,
    ) -> Result<PositionNoT, Self::ElementT> {
        match bucket {
            0 | Self::INTERNAL_TRANSITIONS_BUCKET
                if let UmlStateMachineElement::InternalTransition(element) = element =>
            {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.internal_transitions.len());
                self.internal_transitions.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            0 | Self::REGIONS_BUCKET
                if let UmlStateMachineElement::CompositeStateRegion(element) = element =>
            {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.regions.len());
                self.regions.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            _ => Err(element),
        }
    }

    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.internal_transitions.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.internal_transitions.remove(idx);
                return Some((Self::INTERNAL_TRANSITIONS_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.regions.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.regions.remove(idx);
                return Some((Self::REGIONS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlStateMachineCompositeState {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.stereotype, &self.name],
        );

        // TODO: I think the user might not expect trivial entities like this to be wholly separate when searching
        for e in &self.internal_transitions {
            e.read().full_text_search(acc);
        }

        for e in &self.regions {
            e.read().full_text_search(acc);
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineCompositeStateRegion {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlStateMachineStandaloneElement>,
}

impl UmlStateMachineCompositeStateRegion {
    pub fn new(uuid: ModelUuid, contained_elements: Vec<UmlStateMachineStandaloneElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            contained_elements,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            contained_elements: self.contained_elements.clone(),
        })
    }
}

impl Model for UmlStateMachineCompositeStateRegion {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineCompositeStateRegion {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlStateMachineCompositeStateRegion {
    type ElementT = UmlStateMachineElement;

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

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: Self::ElementT,
    ) -> Result<PositionNoT, Self::ElementT> {
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

impl FullTextSearchable for UmlStateMachineCompositeStateRegion {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(*self.uuid, &[&self.uuid.to_string()]);

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineSimpleState {
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub internal_transitions: Vec<ERef<UmlStateMachineInternalTransition>>,
}

impl UmlStateMachineSimpleState {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        internal_transitions: Vec<ERef<UmlStateMachineInternalTransition>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            internal_transitions,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            internal_transitions: self.internal_transitions.clone(),
        })
    }
}

impl Model for UmlStateMachineSimpleState {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineSimpleState {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for UmlStateMachineSimpleState {
    type ElementT = UmlStateMachineElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.internal_transitions {
            if *e.read().uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        None
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.internal_transitions.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((
                    UmlStateMachineCompositeState::INTERNAL_TRANSITIONS_BUCKET,
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
        element: Self::ElementT,
    ) -> Result<PositionNoT, Self::ElementT> {
        match bucket {
            0 | UmlStateMachineCompositeState::INTERNAL_TRANSITIONS_BUCKET
                if let UmlStateMachineElement::InternalTransition(element) = element =>
            {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.internal_transitions.len());
                self.internal_transitions.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            _ => Err(element),
        }
    }

    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.internal_transitions.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.internal_transitions.remove(idx);
                return Some((
                    UmlStateMachineCompositeState::INTERNAL_TRANSITIONS_BUCKET,
                    idx.try_into().unwrap(),
                ));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlStateMachineSimpleState {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.stereotype, &self.name],
        );

        // TODO: I think the user might not expect trivial entities like this to be wholly separate when searching
        for e in &self.internal_transitions {
            e.read().full_text_search(acc);
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineInternalTransition {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub trigger: Arc<String>,
    pub guard: Arc<String>,
    pub behavior: Arc<String>,
}

impl UmlStateMachineInternalTransition {
    pub fn new(uuid: ModelUuid, trigger: String, guard: String, behavior: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            trigger: Arc::new(trigger),
            guard: Arc::new(guard),
            behavior: Arc::new(behavior),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            trigger: self.trigger.clone(),
            guard: self.guard.clone(),
            behavior: self.behavior.clone(),
        })
    }
}

impl Model for UmlStateMachineInternalTransition {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineInternalTransition {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineInitialPseudostate {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlStateMachineInitialPseudostate {
    pub fn new(uuid: ModelUuid) -> Self {
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

impl Model for UmlStateMachineInitialPseudostate {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineInitialPseudostate {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineTerminatePseudostate {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlStateMachineTerminatePseudostate {
    pub fn new(uuid: ModelUuid) -> Self {
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

impl Model for UmlStateMachineTerminatePseudostate {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineTerminatePseudostate {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineFinalState {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
}

impl UmlStateMachineFinalState {
    pub fn new(uuid: ModelUuid) -> Self {
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

impl Model for UmlStateMachineFinalState {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineFinalState {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineEdge {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    #[full_text_searchable(skip)]
    pub source: UmlStateMachineNonFinalNode,
    #[nh_context_serde(entity)]
    #[full_text_searchable(skip)]
    pub target: UmlStateMachineNonInitialNode,
}

impl UmlStateMachineEdge {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        source: UmlStateMachineNonFinalNode,
        target: UmlStateMachineNonInitialNode,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            source,
            target,
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            name: self.name.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
        })
    }
}

impl Model for UmlStateMachineEdge {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for UmlStateMachineEdge {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineNote {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub text: Arc<String>,
}

impl UmlStateMachineNote {
    pub fn new(uuid: ModelUuid, stereotype: String, text: String) -> Self {
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

impl Entity for UmlStateMachineNote {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlStateMachineNote {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineNoteLink {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<UmlStateMachineNote>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: UmlStateMachineElement,
}

impl UmlStateMachineNoteLink {
    pub fn new(
        uuid: ModelUuid,
        source: ERef<UmlStateMachineNote>,
        target: UmlStateMachineElement,
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

impl Entity for UmlStateMachineNoteLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlStateMachineNoteLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
