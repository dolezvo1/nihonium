use crate::common::{
    entity::{Entity, EntityUuid},
    eref::ERef,
    model::{
        BucketNoT, ContainerModel, DiagramModel, DiagramVisitor, ElementVisitor, Model,
        ModelTopSortInfo, PositionNoT, VisitableDiagram, VisitableElement,
    },
    search::FullTextSearchable,
    uuid::ModelUuid,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub fn deep_copy_diagram(d: &BpmnDiagram) -> (ERef<BpmnDiagram>, HashMap<ModelUuid, BpmnElement>) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = BpmnDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &BpmnDiagram) -> HashMap<ModelUuid, BpmnElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(&e.clone().to_element(), &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &BpmnElement, into: &mut HashMap<ModelUuid, BpmnElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        BpmnElement::Task(inner) => {
            for e in &inner.read().attached_events {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        BpmnElement::Subprocess(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
            for e in &inner.read().attached_events {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        BpmnElement::Pool(inner) => {
            for e in &inner.read().swimlanes {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        BpmnElement::PoolSwimlane(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        _ => {}
    }
}

pub fn transitive_closure(
    d: &BpmnDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &BpmnElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                BpmnElement::Task(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.attached_events {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                }
                BpmnElement::Subprocess(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.contained_elements {
                            walk(&e.clone().to_element(), when_deleting);
                        }
                        for e in &r.attached_events {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                }
                BpmnElement::Pool(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.swimlanes {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                }
                BpmnElement::PoolSwimlane(inner) => {
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
            e: &BpmnElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                BpmnElement::Task(inner) => {
                    for e in &inner.read().attached_events {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                BpmnElement::Subprocess(inner) => {
                    let r = inner.read();
                    for e in &r.contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                    for e in &r.attached_events {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                BpmnElement::Pool(inner) => {
                    for e in &inner.read().swimlanes {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                BpmnElement::PoolSwimlane(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                BpmnElement::StartEvent(..)
                | BpmnElement::IntermediateEvent(..)
                | BpmnElement::BoundaryIntermediateEvent(..)
                | BpmnElement::EndEvent(..)
                | BpmnElement::Gateway(..)
                | BpmnElement::Data(..) => {}
                BpmnElement::SequenceEdge(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                BpmnElement::MessageEdge(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                BpmnElement::Association(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                BpmnElement::Annotation(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid) && when_deleting.contains(&r.target.uuid())
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

pub fn top_sort_info(m: &BpmnElement) -> ModelTopSortInfo {
    fn walk(
        e: &BpmnElement,
        required_models: &mut HashSet<ModelUuid>,
        provided_models: &mut HashSet<ModelUuid>,
    ) {
        provided_models.insert(*e.uuid());
        match e {
            BpmnElement::Task(inner) => {
                for e in &inner.read().attached_events {
                    walk(&e.clone().into(), required_models, provided_models);
                }
            }
            BpmnElement::Subprocess(inner) => {
                let r = inner.read();
                for e in &r.contained_elements {
                    walk(&e.clone().to_element(), required_models, provided_models);
                }
                for e in &r.attached_events {
                    walk(&e.clone().into(), required_models, provided_models);
                }
            }
            BpmnElement::StartEvent(_)
            | BpmnElement::IntermediateEvent(_)
            | BpmnElement::BoundaryIntermediateEvent(_)
            | BpmnElement::EndEvent(_)
            | BpmnElement::Gateway(_) => {}
            BpmnElement::Pool(inner) => {
                for e in &inner.read().swimlanes {
                    walk(&e.clone().into(), required_models, provided_models);
                }
            }
            BpmnElement::PoolSwimlane(inner) => {
                for e in &inner.read().contained_elements {
                    walk(&e.clone().to_element(), required_models, provided_models);
                }
            }
            BpmnElement::Data(_) => {}
            BpmnElement::SequenceEdge(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.uuid());
                required_models.insert(*r.target.uuid());
            }
            BpmnElement::MessageEdge(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.uuid());
                required_models.insert(*r.target.uuid());
            }
            BpmnElement::Association(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.uuid());
                required_models.insert(*r.target.uuid());
            }
            BpmnElement::Annotation(inner) => {
                required_models.insert(*inner.read().target.uuid());
            }
        }
    }

    let (mut required_models, mut provided_models) = Default::default();
    walk(m, &mut required_models, &mut provided_models);
    ModelTopSortInfo {
        required_models,
        provided_models,
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
#[container_model(element_type = BpmnElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum BpmnElement {
    #[container_model(passthrough = "eref")]
    Task(ERef<BpmnTask>),
    #[container_model(passthrough = "eref")]
    Subprocess(ERef<BpmnSubprocess>),
    StartEvent(ERef<BpmnStartEvent>),
    IntermediateEvent(ERef<BpmnIntermediateEvent>),
    BoundaryIntermediateEvent(ERef<BpmnBoundaryIntermediateEvent>),
    EndEvent(ERef<BpmnEndEvent>),
    Gateway(ERef<BpmnGateway>),
    #[container_model(passthrough = "eref")]
    Pool(ERef<BpmnPool>),
    #[container_model(passthrough = "eref")]
    PoolSwimlane(ERef<BpmnPoolSwimlane>),
    Data(ERef<BpmnData>),

    SequenceEdge(ERef<BpmnSequenceEdge>),
    MessageEdge(ERef<BpmnMessageEdge>),
    Association(ERef<BpmnAssociation>),

    Annotation(ERef<BpmnAnnotation>),
}

impl BpmnElement {
    pub fn as_standalone(&self) -> Option<BpmnStandaloneElement> {
        match self {
            BpmnElement::Task(inner) => Some(inner.clone().into()),
            BpmnElement::Subprocess(inner) => Some(inner.clone().into()),
            BpmnElement::StartEvent(inner) => Some(inner.clone().into()),
            BpmnElement::IntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::BoundaryIntermediateEvent(_) => None,
            BpmnElement::EndEvent(inner) => Some(inner.clone().into()),
            BpmnElement::Gateway(inner) => Some(inner.clone().into()),
            BpmnElement::Pool(inner) => Some(inner.clone().into()),
            BpmnElement::PoolSwimlane(_) => None,
            BpmnElement::Data(inner) => Some(inner.clone().into()),
            BpmnElement::SequenceEdge(inner) => Some(inner.clone().into()),
            BpmnElement::MessageEdge(inner) => Some(inner.clone().into()),
            BpmnElement::Association(inner) => Some(inner.clone().into()),
            BpmnElement::Annotation(inner) => Some(inner.clone().into()),
        }
    }
    pub fn as_nonfinal(&self) -> Option<BpmnNonFinalElement> {
        match self {
            BpmnElement::Task(inner) => Some(inner.clone().into()),
            BpmnElement::Subprocess(inner) => Some(inner.clone().into()),
            BpmnElement::StartEvent(inner) => Some(inner.clone().into()),
            BpmnElement::IntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::BoundaryIntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::Gateway(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
    pub fn as_noninitial(&self) -> Option<BpmnNonInitialElement> {
        match self {
            BpmnElement::Task(inner) => Some(inner.clone().into()),
            BpmnElement::Subprocess(inner) => Some(inner.clone().into()),
            BpmnElement::IntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::EndEvent(inner) => Some(inner.clone().into()),
            BpmnElement::Gateway(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
    pub fn as_associable(&self) -> Option<BpmnAssociableElement> {
        match self {
            BpmnElement::Task(inner) => Some(inner.clone().into()),
            BpmnElement::Subprocess(inner) => Some(inner.clone().into()),
            BpmnElement::StartEvent(inner) => Some(inner.clone().into()),
            BpmnElement::IntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::BoundaryIntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::EndEvent(inner) => Some(inner.clone().into()),
            BpmnElement::Gateway(inner) => Some(inner.clone().into()),
            BpmnElement::Pool(inner) => Some(inner.clone().into()),
            BpmnElement::Data(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }
    pub fn as_event(&self) -> Option<BpmnEvent> {
        match self {
            BpmnElement::StartEvent(inner) => Some(inner.clone().into()),
            BpmnElement::IntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::BoundaryIntermediateEvent(inner) => Some(inner.clone().into()),
            BpmnElement::EndEvent(inner) => Some(inner.clone().into()),
            _ => None,
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> Self {
        match self {
            BpmnElement::Task(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            BpmnElement::Subprocess(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::StartEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::IntermediateEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::BoundaryIntermediateEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::EndEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::Gateway(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::Pool(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            BpmnElement::PoolSwimlane(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::Data(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            BpmnElement::SequenceEdge(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::MessageEdge(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::Association(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnElement::Annotation(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
        }
    }
    pub fn deep_copy_relink(&self, all_models: &HashMap<ModelUuid, BpmnElement>) {
        match self {
            BpmnElement::Task(..)
            | BpmnElement::Subprocess(..)
            | BpmnElement::StartEvent(..)
            | BpmnElement::IntermediateEvent(..)
            | BpmnElement::BoundaryIntermediateEvent(..)
            | BpmnElement::EndEvent(..)
            | BpmnElement::Gateway(..)
            | BpmnElement::Pool(..)
            | BpmnElement::PoolSwimlane(..)
            | BpmnElement::Data(..) => {}
            BpmnElement::SequenceEdge(inner) => inner.write().deep_copy_relink(all_models),
            BpmnElement::MessageEdge(inner) => inner.write().deep_copy_relink(all_models),
            BpmnElement::Association(inner) => inner.write().deep_copy_relink(all_models),
            BpmnElement::Annotation(inner) => inner.write().deep_copy_relink(all_models),
        }
    }
}

impl VisitableElement for BpmnElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) {
        match self {
            BpmnElement::Task(inner) => {
                v.open_complex(self);
                for e in &inner.read().attached_events {
                    BpmnElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            BpmnElement::Subprocess(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                for e in &inner.read().attached_events {
                    BpmnElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            BpmnElement::Pool(inner) => {
                v.open_complex(self);
                for e in &inner.read().swimlanes {
                    BpmnElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            BpmnElement::PoolSwimlane(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            BpmnElement::StartEvent(_)
            | BpmnElement::IntermediateEvent(_)
            | BpmnElement::BoundaryIntermediateEvent(_)
            | BpmnElement::EndEvent(_)
            | BpmnElement::Gateway(_)
            | BpmnElement::Data(_)
            | BpmnElement::SequenceEdge(_)
            | BpmnElement::MessageEdge(_)
            | BpmnElement::Association(_)
            | BpmnElement::Annotation(_) => v.visit_simple(&self),
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
#[container_model(element_type = BpmnElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum BpmnStandaloneElement {
    #[container_model(passthrough = "eref")]
    Task(ERef<BpmnTask>),
    #[container_model(passthrough = "eref")]
    Subprocess(ERef<BpmnSubprocess>),
    StartEvent(ERef<BpmnStartEvent>),
    IntermediateEvent(ERef<BpmnIntermediateEvent>),
    EndEvent(ERef<BpmnEndEvent>),
    Gateway(ERef<BpmnGateway>),
    #[container_model(passthrough = "eref")]
    Pool(ERef<BpmnPool>),
    Data(ERef<BpmnData>),

    SequenceEdge(ERef<BpmnSequenceEdge>),
    MessageEdge(ERef<BpmnMessageEdge>),
    Association(ERef<BpmnAssociation>),

    Annotation(ERef<BpmnAnnotation>),
}

impl BpmnStandaloneElement {
    pub fn to_element(self) -> BpmnElement {
        match self {
            BpmnStandaloneElement::Task(inner) => inner.into(),
            BpmnStandaloneElement::Subprocess(inner) => inner.into(),
            BpmnStandaloneElement::StartEvent(inner) => inner.into(),
            BpmnStandaloneElement::IntermediateEvent(inner) => inner.into(),
            BpmnStandaloneElement::EndEvent(inner) => inner.into(),
            BpmnStandaloneElement::Gateway(inner) => inner.into(),
            BpmnStandaloneElement::Pool(inner) => inner.into(),
            BpmnStandaloneElement::Data(inner) => inner.into(),
            BpmnStandaloneElement::SequenceEdge(inner) => inner.into(),
            BpmnStandaloneElement::MessageEdge(inner) => inner.into(),
            BpmnStandaloneElement::Association(inner) => inner.into(),
            BpmnStandaloneElement::Annotation(inner) => inner.into(),
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> Self {
        match self {
            BpmnStandaloneElement::Task(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::Subprocess(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::StartEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::IntermediateEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::EndEvent(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::Gateway(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::Pool(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::Data(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::SequenceEdge(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::MessageEdge(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::Association(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            BpmnStandaloneElement::Annotation(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum BpmnNonFinalElement {
    Task(ERef<BpmnTask>),
    Subprocess(ERef<BpmnSubprocess>),
    StartEvent(ERef<BpmnStartEvent>),
    IntermediateEvent(ERef<BpmnIntermediateEvent>),
    BoundaryIntermediateEvent(ERef<BpmnBoundaryIntermediateEvent>),
    Gateway(ERef<BpmnGateway>),
}

impl BpmnNonFinalElement {
    pub fn to_element(self) -> BpmnElement {
        match self {
            BpmnNonFinalElement::Task(inner) => inner.into(),
            BpmnNonFinalElement::Subprocess(inner) => inner.into(),
            BpmnNonFinalElement::StartEvent(inner) => inner.into(),
            BpmnNonFinalElement::IntermediateEvent(inner) => inner.into(),
            BpmnNonFinalElement::BoundaryIntermediateEvent(inner) => inner.into(),
            BpmnNonFinalElement::Gateway(inner) => inner.into(),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum BpmnNonInitialElement {
    Task(ERef<BpmnTask>),
    Subprocess(ERef<BpmnSubprocess>),
    IntermediateEvent(ERef<BpmnIntermediateEvent>),
    EndEvent(ERef<BpmnEndEvent>),
    Gateway(ERef<BpmnGateway>),
}

impl BpmnNonInitialElement {
    pub fn to_element(self) -> BpmnElement {
        match self {
            BpmnNonInitialElement::Task(inner) => inner.into(),
            BpmnNonInitialElement::Subprocess(inner) => inner.into(),
            BpmnNonInitialElement::IntermediateEvent(inner) => inner.into(),
            BpmnNonInitialElement::EndEvent(inner) => inner.into(),
            BpmnNonInitialElement::Gateway(inner) => inner.into(),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum BpmnAssociableElement {
    Task(ERef<BpmnTask>),
    Subprocess(ERef<BpmnSubprocess>),
    StartEvent(ERef<BpmnStartEvent>),
    IntermediateEvent(ERef<BpmnIntermediateEvent>),
    BoundaryIntermediateEvent(ERef<BpmnBoundaryIntermediateEvent>),
    EndEvent(ERef<BpmnEndEvent>),
    Gateway(ERef<BpmnGateway>),
    Pool(ERef<BpmnPool>),
    Data(ERef<BpmnData>),
}

impl BpmnAssociableElement {
    pub fn to_element(self) -> BpmnElement {
        match self {
            BpmnAssociableElement::Task(inner) => inner.into(),
            BpmnAssociableElement::Subprocess(inner) => inner.into(),
            BpmnAssociableElement::StartEvent(inner) => inner.into(),
            BpmnAssociableElement::IntermediateEvent(inner) => inner.into(),
            BpmnAssociableElement::BoundaryIntermediateEvent(inner) => inner.into(),
            BpmnAssociableElement::EndEvent(inner) => inner.into(),
            BpmnAssociableElement::Gateway(inner) => inner.into(),
            BpmnAssociableElement::Pool(inner) => inner.into(),
            BpmnAssociableElement::Data(inner) => inner.into(),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum BpmnEvent {
    StartEvent(ERef<BpmnStartEvent>),
    IntermediateEvent(ERef<BpmnIntermediateEvent>),
    BoundaryIntermediateEvent(ERef<BpmnBoundaryIntermediateEvent>),
    EndEvent(ERef<BpmnEndEvent>),
}

impl BpmnEvent {
    pub fn to_element(self) -> BpmnElement {
        match self {
            BpmnEvent::StartEvent(inner) => inner.into(),
            BpmnEvent::IntermediateEvent(inner) => inner.into(),
            BpmnEvent::BoundaryIntermediateEvent(inner) => inner.into(),
            BpmnEvent::EndEvent(inner) => inner.into(),
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct BpmnDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<BpmnStandaloneElement>,

    pub comment: Arc<String>,
}

impl BpmnDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<BpmnStandaloneElement>,
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
        element: BpmnElement,
    ) -> Result<PositionNoT, BpmnElement> {
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
        undo: &mut Vec<(ModelUuid, BpmnElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &BpmnElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, BpmnElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                BpmnElement::Task(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.attached_events.iter().enumerate() {
                        if uuids.contains(&e.read().uuid()) {
                            undo.push((*w.uuid, e.clone().into(), 0, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.attached_events
                        .retain(|e| !uuids.contains(&e.read().uuid()));
                }
                BpmnElement::Subprocess(inner) => {
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
                    for (idx, e) in w.attached_events.iter().enumerate() {
                        if uuids.contains(&e.read().uuid()) {
                            undo.push((*w.uuid, e.clone().into(), 0, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.attached_events
                        .retain(|e| !uuids.contains(&e.read().uuid()));
                }
                BpmnElement::StartEvent(_)
                | BpmnElement::IntermediateEvent(_)
                | BpmnElement::BoundaryIntermediateEvent(_)
                | BpmnElement::EndEvent(_)
                | BpmnElement::Gateway(_) => {}
                BpmnElement::Pool(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.swimlanes.iter().enumerate() {
                        if uuids.contains(&e.read().uuid) {
                            undo.push((*w.uuid, e.clone().into(), 0, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.swimlanes.retain(|e| !uuids.contains(&e.read().uuid));
                }
                BpmnElement::PoolSwimlane(inner) => {
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
                BpmnElement::Data(_)
                | BpmnElement::SequenceEdge(_)
                | BpmnElement::MessageEdge(_)
                | BpmnElement::Association(_)
                | BpmnElement::Annotation(_) => {}
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

impl Entity for BpmnDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for BpmnDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for BpmnDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.clone().to_element().accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for BpmnDiagram {
    type ElementT = BpmnElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(BpmnElement, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
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
}

impl DiagramModel for BpmnDiagram {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        b: BucketNoT,
        p: Option<PositionNoT>,
        element: BpmnElement,
    ) -> Result<PositionNoT, BpmnElement> {
        // TODO: validate

        if *self.uuid == target {
            self.insert_element_unsafe(b, p, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };

            match e {
                BpmnElement::Task(inner) => inner.write().insert_element(b, p, element),
                BpmnElement::Subprocess(inner) => inner.write().insert_element(b, p, element),
                BpmnElement::Pool(inner) => inner.write().insert_element(b, p, element),
                BpmnElement::PoolSwimlane(inner) => inner.write().insert_element(b, p, element),
                BpmnElement::StartEvent(_)
                | BpmnElement::IntermediateEvent(_)
                | BpmnElement::BoundaryIntermediateEvent(_)
                | BpmnElement::EndEvent(_)
                | BpmnElement::Gateway(_)
                | BpmnElement::Data(_)
                | BpmnElement::SequenceEdge(_)
                | BpmnElement::MessageEdge(_)
                | BpmnElement::Association(_)
                | BpmnElement::Annotation(_) => Err(element),
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
                BpmnElement::Task(inner) => inner.write().remove_element(uuid),
                BpmnElement::Subprocess(inner) => inner.write().remove_element(uuid),
                BpmnElement::Pool(inner) => inner.write().remove_element(uuid),
                BpmnElement::PoolSwimlane(inner) => inner.write().remove_element(uuid),
                BpmnElement::StartEvent(_)
                | BpmnElement::IntermediateEvent(_)
                | BpmnElement::BoundaryIntermediateEvent(_)
                | BpmnElement::EndEvent(_)
                | BpmnElement::Gateway(_)
                | BpmnElement::Data(_)
                | BpmnElement::SequenceEdge(_)
                | BpmnElement::MessageEdge(_)
                | BpmnElement::Association(_)
                | BpmnElement::Annotation(_) => None,
            }
        }
    }
}

impl FullTextSearchable for BpmnDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::ModelSearcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.name, &self.comment],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum BpmnTaskKind {
    #[default]
    Unspecified,
    Send,
    Receive,
    User,
    Manual,
    BusinessRule,
    Service,
    Script,
}

impl BpmnTaskKind {
    pub const VARIANTS: [Self; 8] = [
        Self::Unspecified,
        Self::Send,
        Self::Receive,
        Self::User,
        Self::Manual,
        Self::BusinessRule,
        Self::Service,
        Self::Script,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            BpmnTaskKind::Unspecified => "Unspecified",
            BpmnTaskKind::Send => "Send",
            BpmnTaskKind::Receive => "Receive",
            BpmnTaskKind::User => "User",
            BpmnTaskKind::Manual => "Manual",
            BpmnTaskKind::BusinessRule => "Business Rule",
            BpmnTaskKind::Service => "Service",
            BpmnTaskKind::Script => "Script",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnTask {
    pub uuid: Arc<ModelUuid>,
    pub kind: BpmnTaskKind,
    pub is_call: bool,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub attached_events: Vec<ERef<BpmnBoundaryIntermediateEvent>>,
}

impl BpmnTask {
    pub fn new(uuid: ModelUuid, kind: BpmnTaskKind, is_call: bool, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            is_call,
            name: Arc::new(name),
            attached_events: Vec::new(),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnTask {
            uuid: new_uuid.into(),
            kind: self.kind,
            is_call: self.is_call,
            name: self.name.clone(),
            attached_events: self
                .attached_events
                .iter()
                .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into))
                .collect(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: BpmnElement,
    ) -> Result<PositionNoT, BpmnElement> {
        if bucket == 0
            && let BpmnElement::BoundaryIntermediateEvent(event) = element
        {
            let pos = position
                .map(|e| e.try_into().unwrap())
                .unwrap_or(self.attached_events.len());
            self.attached_events.insert(pos, event);
            Ok(pos.try_into().unwrap())
        } else {
            Err(element)
        }
    }

    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.attached_events.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.attached_events.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl Model for BpmnTask {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnTask {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for BpmnTask {
    type ElementT = BpmnElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.attached_events {
            if *e.read().uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        None
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.attached_events.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for BpmnTask {
    fn full_text_search(&self, acc: &mut crate::common::search::ModelSearcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), self.kind.as_str(), &self.name],
        );

        for e in &self.attached_events {
            e.read().full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum BpmnSubprocessKind {
    #[default]
    Subprocess,
    Transaction,
    EventSubprocess,
}

impl BpmnSubprocessKind {
    pub const VARIANTS: [Self; 3] = [Self::Subprocess, Self::Transaction, Self::EventSubprocess];

    pub fn as_str(&self) -> &'static str {
        match self {
            BpmnSubprocessKind::Subprocess => "Subprocess",
            BpmnSubprocessKind::Transaction => "Transaction",
            BpmnSubprocessKind::EventSubprocess => "Event Subprocess",
        }
    }
}

#[derive(Clone, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnSubprocess {
    pub uuid: Arc<ModelUuid>,
    pub kind: BpmnSubprocessKind,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<BpmnStandaloneElement>,
    #[nh_context_serde(entity)]
    pub attached_events: Vec<ERef<BpmnBoundaryIntermediateEvent>>,
}

impl BpmnSubprocess {
    pub fn new(
        uuid: ModelUuid,
        kind: BpmnSubprocessKind,
        name: String,
        contained_elements: Vec<BpmnStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            name: Arc::new(name),
            contained_elements,
            attached_events: Vec::new(),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnSubprocess {
            uuid: new_uuid.into(),
            kind: self.kind,
            name: self.name.clone(),
            contained_elements: self
                .contained_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
            attached_events: self
                .attached_events
                .iter()
                .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into))
                .collect(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: BpmnElement,
    ) -> Result<PositionNoT, BpmnElement> {
        if bucket == 0
            && let Some(s) = element.as_standalone()
        {
            let pos = position
                .map(|e| e.try_into().unwrap())
                .unwrap_or(self.contained_elements.len());
            self.contained_elements.insert(pos, s);
            Ok(pos.try_into().unwrap())
        } else if bucket == 0
            && let BpmnElement::BoundaryIntermediateEvent(event) = element
        {
            let pos = position
                .map(|e| e.try_into().unwrap())
                .unwrap_or(self.attached_events.len());
            self.attached_events.insert(pos, event);
            Ok(pos.try_into().unwrap())
        } else {
            Err(element)
        }
    }

    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.contained_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.attached_events.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.attached_events.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl Model for BpmnSubprocess {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnSubprocess {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for BpmnSubprocess {
    type ElementT = BpmnElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        for e in &self.attached_events {
            if *e.read().uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
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
        for (idx, e) in self.attached_events.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for BpmnSubprocess {
    fn full_text_search(&self, acc: &mut crate::common::search::ModelSearcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), self.kind.as_str(), &self.name],
        );

        for e in &self.attached_events {
            e.read().full_text_search(acc);
        }

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BpmnEventKind {
    #[default]
    Unspecified,
    Message,
    Timer,
    Escalation,
    Conditional,
    Link,
    Error,
    Cancel,
    Compensation,
    Signal,
    Multiple,
    ParallelMultiple,
    Terminate,
}

impl BpmnEventKind {
    pub const VARIANTS: [Self; 13] = [
        Self::Unspecified,
        Self::Message,
        Self::Timer,
        Self::Escalation,
        Self::Conditional,
        Self::Link,
        Self::Error,
        Self::Cancel,
        Self::Compensation,
        Self::Signal,
        Self::Multiple,
        Self::ParallelMultiple,
        Self::Terminate,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            BpmnEventKind::Unspecified => "Unspecified",
            BpmnEventKind::Message => "Message",
            BpmnEventKind::Timer => "Timer",
            BpmnEventKind::Escalation => "Escalation",
            BpmnEventKind::Conditional => "Conditional",
            BpmnEventKind::Link => "Link",
            BpmnEventKind::Error => "Error",
            BpmnEventKind::Cancel => "Cancel",
            BpmnEventKind::Compensation => "Compensation",
            BpmnEventKind::Signal => "Signal",
            BpmnEventKind::Multiple => "Multiple",
            BpmnEventKind::ParallelMultiple => "Parallel Multiple",
            BpmnEventKind::Terminate => "Terminate",
        }
    }
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BpmnStartEventKind {
    Interrupting,
    Noninterrupting,
}

impl BpmnStartEventKind {
    pub const VARIANTS: [Self; 2] = [Self::Interrupting, Self::Noninterrupting];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interrupting => "Process Start/Interrupting",
            Self::Noninterrupting => "Non-Interrupting",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnStartEvent {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnEventKind,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub subkind: BpmnStartEventKind,
    pub name: Arc<String>,
}

impl BpmnStartEvent {
    pub fn new(
        uuid: ModelUuid,
        kind: BpmnEventKind,
        subkind: BpmnStartEventKind,
        name: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            subkind,
            name: Arc::new(name),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnStartEvent {
            uuid: new_uuid.into(),
            kind: self.kind,
            subkind: self.subkind,
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for BpmnStartEvent {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnStartEvent {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BpmnIntermediateEventKind {
    Catching,
    Throwing,
}

impl BpmnIntermediateEventKind {
    pub const VARIANTS: [Self; 2] = [Self::Catching, Self::Throwing];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Catching => "Catching",
            Self::Throwing => "Throwing",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnIntermediateEvent {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnEventKind,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub subkind: BpmnIntermediateEventKind,
    pub name: Arc<String>,
}

impl BpmnIntermediateEvent {
    pub fn new(
        uuid: ModelUuid,
        kind: BpmnEventKind,
        subkind: BpmnIntermediateEventKind,
        name: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            subkind,
            name: Arc::new(name),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnIntermediateEvent {
            uuid: new_uuid.into(),
            kind: self.kind,
            subkind: self.subkind,
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for BpmnIntermediateEvent {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnIntermediateEvent {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BpmnBoundaryIntermediateEventKind {
    Interrupting,
    Noninterrupting,
}

impl BpmnBoundaryIntermediateEventKind {
    pub const VARIANTS: [Self; 2] = [Self::Interrupting, Self::Noninterrupting];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interrupting => "Interrupting",
            Self::Noninterrupting => "Non-Interrupting",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnBoundaryIntermediateEvent {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnEventKind,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub subkind: BpmnBoundaryIntermediateEventKind,
    pub name: Arc<String>,
}

impl BpmnBoundaryIntermediateEvent {
    pub fn new(
        uuid: ModelUuid,
        kind: BpmnEventKind,
        subkind: BpmnBoundaryIntermediateEventKind,
        name: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            subkind,
            name: Arc::new(name),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnBoundaryIntermediateEvent {
            uuid: new_uuid.into(),
            kind: self.kind,
            subkind: self.subkind,
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for BpmnBoundaryIntermediateEvent {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnBoundaryIntermediateEvent {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnEndEvent {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnEventKind,
    pub name: Arc<String>,
}

impl BpmnEndEvent {
    pub fn new(uuid: ModelUuid, kind: BpmnEventKind, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            name: Arc::new(name),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnEndEvent {
            uuid: new_uuid.into(),
            kind: self.kind,
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for BpmnEndEvent {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnEndEvent {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BpmnGatewayKind {
    #[default]
    Exclusive,
    Parallel,
    Inclusive,
    EventBased,
    ExclusiveEventBased,
    ParallelEventBased,
    Complex,
}

impl BpmnGatewayKind {
    pub const VARIANTS: [Self; 7] = [
        Self::Exclusive,
        Self::Parallel,
        Self::Inclusive,
        Self::EventBased,
        Self::ExclusiveEventBased,
        Self::ParallelEventBased,
        Self::Complex,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Exclusive => "Exclusive",
            Self::Parallel => "Parallel",
            Self::Inclusive => "Inclusive",
            Self::EventBased => "Event-Based",
            Self::ExclusiveEventBased => "Exclusive Event-Based",
            Self::ParallelEventBased => "Parallel Event-Based",
            Self::Complex => "Complex",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnGateway {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnGatewayKind,
    pub condition: Arc<String>,
}

impl BpmnGateway {
    pub fn new(uuid: ModelUuid, kind: BpmnGatewayKind, condition: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            condition: Arc::new(condition),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnGateway {
            uuid: new_uuid.into(),
            kind: self.kind,
            condition: self.condition.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for BpmnGateway {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnGateway {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnPool {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub swimlanes: Vec<ERef<BpmnPoolSwimlane>>,
}

impl BpmnPool {
    pub fn new(uuid: ModelUuid, swimlanes: Vec<ERef<BpmnPoolSwimlane>>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            swimlanes,
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnPool {
            uuid: new_uuid.into(),
            swimlanes: self
                .swimlanes
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
                .swimlanes
                .iter()
                .enumerate()
                .find(|e| *e.1.read().uuid() == *element)
        {
            let e = self.swimlanes.remove(idx);
            self.swimlanes.insert(target_pos.try_into().unwrap(), e);
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: BpmnElement,
    ) -> Result<PositionNoT, BpmnElement> {
        if bucket != 0 {
            return Err(element);
        }
        let BpmnElement::PoolSwimlane(element) = element else {
            return Err(element);
        };

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.swimlanes.len());
        self.swimlanes.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }

    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.swimlanes.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.swimlanes.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl Model for BpmnPool {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnPool {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for BpmnPool {
    type ElementT = BpmnElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.swimlanes {
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
        for (idx, e) in self.swimlanes.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for BpmnPool {
    fn full_text_search(&self, acc: &mut crate::common::search::ModelSearcher) {
        acc.check_element(*self.uuid, &[&self.uuid.to_string()]);

        for e in &self.swimlanes {
            e.read().full_text_search(acc);
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnPoolSwimlane {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<BpmnStandaloneElement>,
}

impl BpmnPoolSwimlane {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<BpmnStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnPoolSwimlane {
            uuid: new_uuid.into(),
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
        element: BpmnElement,
    ) -> Result<PositionNoT, BpmnElement> {
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

impl Model for BpmnPoolSwimlane {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnPoolSwimlane {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl ContainerModel for BpmnPoolSwimlane {
    type ElementT = BpmnElement;

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

impl FullTextSearchable for BpmnPoolSwimlane {
    fn full_text_search(&self, acc: &mut crate::common::search::ModelSearcher) {
        acc.check_element(*self.uuid, &[&self.uuid.to_string(), &self.name]);

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum BpmnDataKind {
    #[default]
    UnspecifiedObject,
    InputObject,
    OutputObject,
    DataStore,
}

impl BpmnDataKind {
    pub const VARIANTS: [Self; 4] = [
        Self::UnspecifiedObject,
        Self::InputObject,
        Self::OutputObject,
        Self::DataStore,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            BpmnDataKind::UnspecifiedObject => "Unspecified Object",
            BpmnDataKind::InputObject => "Input Object",
            BpmnDataKind::OutputObject => "Output Object",
            BpmnDataKind::DataStore => "Data Store",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnData {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnDataKind,
    #[full_text_searchable(skip)]
    pub is_collection: bool,
    pub name: Arc<String>,
}

impl BpmnData {
    pub fn new(uuid: ModelUuid, kind: BpmnDataKind, is_collection: bool, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            is_collection,
            name: Arc::new(name),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnData {
            uuid: new_uuid.into(),
            kind: self.kind,
            is_collection: self.is_collection,
            name: self.name.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Model for BpmnData {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnData {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum BpmnSequenceEdgeKind {
    #[default]
    Normal,
    ConditionalBranch,
    DefaultBranch,
}

impl BpmnSequenceEdgeKind {
    pub const VARIANTS: [Self; 3] = [Self::Normal, Self::ConditionalBranch, Self::DefaultBranch];

    pub fn as_str(&self) -> &'static str {
        match self {
            BpmnSequenceEdgeKind::Normal => "Normal",
            BpmnSequenceEdgeKind::ConditionalBranch => "Conditional Branch",
            BpmnSequenceEdgeKind::DefaultBranch => "Default Branch",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnSequenceEdge {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: BpmnNonFinalElement,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: BpmnNonInitialElement,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnSequenceEdgeKind,
    pub condition: Arc<String>,
}

impl BpmnSequenceEdge {
    pub fn new(
        uuid: ModelUuid,
        source: BpmnNonFinalElement,
        target: BpmnNonInitialElement,
        kind: BpmnSequenceEdgeKind,
        condition: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
            kind,
            condition: Arc::new(condition),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnSequenceEdge {
            uuid: new_uuid.into(),
            source: self.source.clone(),
            target: self.target.clone(),
            kind: self.kind,
            condition: self.condition.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, BpmnElement>) {
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

impl Model for BpmnSequenceEdge {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnSequenceEdge {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum BpmnMessageEdgeKind {
    Unspecified,
    Catching,
    Throwing,
}

impl BpmnMessageEdgeKind {
    pub const VARIANTS: [Self; 3] = [Self::Unspecified, Self::Catching, Self::Throwing];

    pub fn as_str(&self) -> &'static str {
        match self {
            BpmnMessageEdgeKind::Unspecified => "Unspecified",
            BpmnMessageEdgeKind::Catching => "Catching",
            BpmnMessageEdgeKind::Throwing => "Throwing",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnMessageEdge {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: BpmnAssociableElement,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: BpmnAssociableElement,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: BpmnMessageEdgeKind,
}

impl BpmnMessageEdge {
    pub fn new(
        uuid: ModelUuid,
        source: BpmnAssociableElement,
        target: BpmnAssociableElement,
        kind: BpmnMessageEdgeKind,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
            kind,
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnMessageEdge {
            uuid: new_uuid.into(),
            source: self.source.clone(),
            target: self.target.clone(),
            kind: self.kind,
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, BpmnElement>) {
        let source_uuid = *self.source.uuid();
        if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_associable()) {
            self.source = s;
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_associable()) {
            self.target = t;
        }
    }
}

impl Model for BpmnMessageEdge {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnMessageEdge {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnAssociation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: BpmnAssociableElement,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: BpmnAssociableElement,
}

impl BpmnAssociation {
    pub fn new(
        uuid: ModelUuid,
        source: BpmnAssociableElement,
        target: BpmnAssociableElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnAssociation {
            uuid: new_uuid.into(),
            source: self.source.clone(),
            target: self.target.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, BpmnElement>) {
        let source_uuid = *self.source.uuid();
        if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_associable()) {
            self.source = s;
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_associable()) {
            self.target = t;
        }
    }
}

impl Model for BpmnAssociation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnAssociation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct BpmnAnnotation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: BpmnElement,
    pub text: Arc<String>,
}

impl BpmnAnnotation {
    pub fn new(uuid: ModelUuid, target: BpmnElement, text: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            target,
            text: Arc::new(text),
        }
    }

    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, BpmnElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(BpmnAnnotation {
            uuid: new_uuid.into(),
            target: self.target.clone(),
            text: self.text.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, BpmnElement>) {
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid) {
            self.target = t.clone();
        }
    }
}

impl Model for BpmnAnnotation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl Entity for BpmnAnnotation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}
