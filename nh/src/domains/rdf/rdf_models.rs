use crate::common::controller::{BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT, VisitableDiagram, VisitableElement};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::ModelUuid;
use std::{
    collections::HashMap,
    sync::{Arc},
};

#[cfg(not(target_arch = "wasm32"))]
use sophia::api::{
    term::{GraphName, IriRef, LanguageTag, SimpleTerm},
    MownStr,
};

#[cfg(not(target_arch = "wasm32"))]
pub struct RdfCollector<'a> {
    data: Vec<([SimpleTerm<'a>; 3], GraphName<SimpleTerm<'a>>)>,
    current_graph: GraphName<SimpleTerm<'a>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> RdfCollector<'a> {
    fn add_triple(&mut self, triple: [SimpleTerm<'a>; 3]) {
        self.data.push((triple, self.current_graph.clone()));
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = RdfElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfElement {
    #[container_model(passthrough = "eref")]
    RdfGraph(ERef<RdfGraph>),
    RdfLiteral(ERef<RdfLiteral>),
    RdfNode(ERef<RdfNode>),
    RdfPredicate(ERef<RdfPredicate>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfTargettableElement {
    RdfLiteral(ERef<RdfLiteral>),
    RdfNode(ERef<RdfNode>),
}

impl RdfElement {
    pub fn as_targettable_element(&self) -> Option<RdfTargettableElement> {
        match self {
            RdfElement::RdfLiteral(inner) => Some(inner.clone().into()),
            RdfElement::RdfNode(inner) => Some(inner.clone().into()),
            RdfElement::RdfGraph(_) | RdfElement::RdfPredicate(_) => None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn accept_collector(&self, collector: &mut RdfCollector<'static>) {
        match self {
            RdfElement::RdfGraph(inner) => {
                let model = inner.read();
                let old_graph = collector.current_graph.replace(SimpleTerm::Iri(
                    IriRef::new(MownStr::from((*model.iri).clone())).unwrap(),
                ));

                for c in &model.contained_elements {
                    c.accept_collector(collector);
                }

                collector.current_graph = old_graph;
            },
            RdfElement::RdfLiteral(_) | RdfElement::RdfNode(_) => {}
            RdfElement::RdfPredicate(inner) => {
                let model = inner.read();
                let subject = model.source.read().term_repr();
                let object = model.target.term_repr();

                collector.add_triple([
                    subject,
                    SimpleTerm::Iri(IriRef::new(MownStr::from((*model.iri).clone())).unwrap()),
                    object,
                ]);
            },
        }
    }
}

impl RdfTargettableElement {
    #[cfg(not(target_arch = "wasm32"))]
    fn term_repr(&self) -> SimpleTerm<'static> {
        match self {
            RdfTargettableElement::RdfLiteral(inner) => inner.read().term_repr(),
            RdfTargettableElement::RdfNode(inner) => inner.read().term_repr(),
        }
    }
}

impl VisitableElement for RdfElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            RdfElement::RdfGraph(inner) => {
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

pub fn deep_copy_diagram(d: &RdfDiagram) -> (ERef<RdfDiagram>, HashMap<ModelUuid, RdfElement>) {
    fn walk(e: &RdfElement, into: &mut HashMap<ModelUuid, RdfElement>) -> RdfElement {
        let new_uuid = Arc::new(uuid::Uuid::now_v7().into());
        match e {
            RdfElement::RdfGraph(inner) => {
                let model = inner.read();

                let new_model = RdfGraph {
                    uuid: new_uuid,
                    iri: model.iri.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                RdfElement::RdfGraph(ERef::new(new_model))
            },
            RdfElement::RdfLiteral(inner) => {
                let model = inner.read();

                let new_model = RdfLiteral {
                    uuid: new_uuid,
                    content: model.content.clone(),
                    datatype: model.datatype.clone(),
                    langtag: model.langtag.clone(),
                    comment: model.comment.clone(),
                };
                RdfElement::RdfLiteral(ERef::new(new_model))
            },
            RdfElement::RdfNode(inner) => {
                let model = inner.read();

                let new_model = RdfNode {
                    uuid: new_uuid,
                    iri: model.iri.clone(),
                    comment: model.comment.clone(),
                };
                RdfElement::RdfNode(ERef::new(new_model))
            },
            RdfElement::RdfPredicate(inner) => {
                let model = inner.read();

                let new_model = RdfPredicate {
                    uuid: new_uuid,
                    iri: model.iri.clone(),
                    source: model.source.clone(),
                    target: model.target.clone(),
                    comment: model.comment.clone(),
                };
                RdfElement::RdfPredicate(ERef::new(new_model))
            },
        }
    }

    fn relink(e: &mut RdfElement, all_models: &HashMap<ModelUuid, RdfElement>) {
        match e {
            RdfElement::RdfGraph(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            }
            RdfElement::RdfLiteral(_) | RdfElement::RdfNode(_) => {},
            RdfElement::RdfPredicate(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(RdfElement::RdfNode(n)) = all_models.get(&source_uuid) {
                    model.source = n.clone().into();
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_targettable_element()) {
                    model.target = t;
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

    let new_diagram = RdfDiagram {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
        stored_queries: d.stored_queries.iter().map(|e| (uuid::Uuid::now_v7(), e.1.clone())).collect(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &RdfDiagram) -> HashMap<ModelUuid, RdfElement> {
    fn walk(e: &RdfElement, into: &mut HashMap<ModelUuid, RdfElement>) {
        match e {
            RdfElement::RdfGraph(inner) => {
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct RdfDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<RdfElement>,
    pub stored_queries: HashMap<uuid::Uuid, (String, String)>,

    pub comment: Arc<String>,
}

impl RdfDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<RdfElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            stored_queries: {
                let mut hm = HashMap::new();
                hm.insert(
                    uuid::Uuid::now_v7(),
                    ("all".to_owned(), "SELECT * WHERE { ?s ?p ?o }".to_owned()),
                );
                hm
            },
            comment: Arc::new("".to_owned()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn graph(&self) -> Vec<([SimpleTerm<'_>; 3], GraphName<SimpleTerm<'_>>)> {
        let mut collector = RdfCollector {
            data: Vec::new(),
            current_graph: None,
        };

        for c in &self.contained_elements {
            let c = c.accept_collector(&mut collector);
        }

        collector.data
    }
}

impl Entity for RdfDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for RdfDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for RdfDiagram {
    type ElementT = RdfElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(RdfElement, ModelUuid)> {
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
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: RdfElement) -> Result<PositionNoT, RdfElement> {
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfGraph {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<RdfElement>,

    pub comment: Arc<String>,
}

impl RdfGraph {
    pub fn new(
        uuid: ModelUuid,
        iri: String,
        contained_elements: Vec<RdfElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Entity for RdfGraph {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfGraph {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for RdfGraph {
    type ElementT = RdfElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(RdfElement, ModelUuid)> {
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
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: RdfElement) -> Result<PositionNoT, RdfElement> {
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfLiteral {
    pub uuid: Arc<ModelUuid>,
    pub content: Arc<String>,
    pub datatype: Arc<String>,
    pub langtag: Arc<String>,

    pub comment: Arc<String>,
}

impl RdfLiteral {
    pub fn new(uuid: ModelUuid, content: String, datatype: String, langtag: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            content: Arc::new(content),
            datatype: Arc::new(datatype),
            langtag: Arc::new(langtag),
            comment: Arc::new("".to_owned()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn term_repr(&self) -> SimpleTerm<'static> {
        if !self.langtag.is_empty() {
            SimpleTerm::LiteralLanguage(
                MownStr::from((*self.content).clone()),
                LanguageTag::new(MownStr::from((*self.langtag).clone())).unwrap(),
            )
        } else {
            let datatype = if !self.datatype.is_empty() {
                &self.datatype
            } else {
                "asdf"
            }
            .to_owned();
            SimpleTerm::LiteralDatatype(
                MownStr::from((*self.content).clone()),
                IriRef::new(MownStr::from(datatype)).unwrap(),
            )
        }
    }
}

impl Entity for RdfLiteral {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfLiteral {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfNode {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,

    pub comment: Arc<String>,
}

impl RdfNode {
    pub fn new(uuid: ModelUuid, iri: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            comment: Arc::new("".to_owned()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn term_repr(&self) -> SimpleTerm<'static> {
        SimpleTerm::Iri(IriRef::new(MownStr::from((*self.iri).clone())).unwrap())
    }
}

impl Entity for RdfNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfPredicate {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    #[nh_context_serde(entity)]
    pub source: ERef<RdfNode>,
    #[nh_context_serde(entity)]
    pub target: RdfTargettableElement,

    pub comment: Arc<String>,
}

impl RdfPredicate {
    pub fn new(
        uuid: ModelUuid,
        iri: String,
        source: ERef<RdfNode>,
        target: RdfTargettableElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            source,
            target,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Entity for RdfPredicate {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfPredicate {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
