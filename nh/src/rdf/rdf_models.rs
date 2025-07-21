use crate::common::controller::{ContainerModel, Model, StructuralVisitor};
use crate::common::project_serde::{NHContextDeserialize, NHDeserializeError, NHDeserializer, NHContextSerialize, NHSerializeError, NHSerializer};
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use sophia::api::{
    term::{GraphName, IriRef, LanguageTag, SimpleTerm},
    MownStr,
};

pub struct RdfCollector<'a> {
    data: Vec<([SimpleTerm<'a>; 3], GraphName<SimpleTerm<'a>>)>,
    current_graph: GraphName<SimpleTerm<'a>>,
}

impl<'a> RdfCollector<'a> {
    fn add_triple(&mut self, triple: [SimpleTerm<'a>; 3]) {
        self.data.push((triple, self.current_graph.clone()));
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[container_model(element_type = RdfElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfElement {
    #[container_model(passthrough = "arc_rwlock")]
    RdfGraph(Arc<RwLock<RdfGraph>>),
    RdfLiteral(Arc<RwLock<RdfLiteral>>),
    RdfNode(Arc<RwLock<RdfNode>>),
    RdfPredicate(Arc<RwLock<RdfPredicate>>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag, nh_derive::Unwrap)]
#[model(default_passthrough = "arc_rwlock")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfNodeWrapper {
    RdfNode(Arc<RwLock<RdfNode>>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfTargettableElement {
    RdfLiteral(Arc<RwLock<RdfLiteral>>),
    RdfNode(Arc<RwLock<RdfNode>>),
}

impl RdfElement {
    pub fn as_targettable_element(&self) -> Option<RdfTargettableElement> {
        match self {
            RdfElement::RdfLiteral(rw_lock) => Some(RdfTargettableElement::RdfLiteral(rw_lock.clone())),
            RdfElement::RdfNode(rw_lock) => Some(RdfTargettableElement::RdfNode(rw_lock.clone())),
            RdfElement::RdfGraph(_) | RdfElement::RdfPredicate(_) => None,
        }
    }

    fn accept_collector(&self, collector: &mut RdfCollector<'static>) {
        match self {
            RdfElement::RdfGraph(rw_lock) => {
                let model = rw_lock.read().unwrap();
                let old_graph = collector.current_graph.replace(SimpleTerm::Iri(
                    IriRef::new(MownStr::from((*model.iri).clone())).unwrap(),
                ));

                for c in &model.contained_elements {
                    c.accept_collector(collector);
                }

                collector.current_graph = old_graph;
            },
            RdfElement::RdfLiteral(_) | RdfElement::RdfNode(_) => {}
            RdfElement::RdfPredicate(rw_lock) => {
                let model = rw_lock.read().unwrap();
                let subject = model.source.clone().unwrap().read().unwrap().term_repr();
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
    fn term_repr(&self) -> SimpleTerm<'static> {
        match self {
            RdfTargettableElement::RdfLiteral(rw_lock) => rw_lock.read().unwrap().term_repr(),
            RdfTargettableElement::RdfNode(rw_lock) => rw_lock.read().unwrap().term_repr(),
        }
    }
}

pub fn deep_copy_diagram(d: &RdfDiagram) -> (Arc<RwLock<RdfDiagram>>, HashMap<ModelUuid, RdfElement>) {
    fn walk(e: &RdfElement, into: &mut HashMap<ModelUuid, RdfElement>) -> RdfElement {
        let new_uuid = Arc::new(uuid::Uuid::now_v7().into());
        match e {
            RdfElement::RdfGraph(rw_lock) => {
                let model = rw_lock.read().unwrap();

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
                RdfElement::RdfGraph(Arc::new(RwLock::new(new_model)))
            },
            RdfElement::RdfLiteral(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = RdfLiteral {
                    uuid: new_uuid,
                    content: model.content.clone(),
                    datatype: model.datatype.clone(),
                    langtag: model.langtag.clone(),
                    comment: model.comment.clone(),
                };
                RdfElement::RdfLiteral(Arc::new(RwLock::new(new_model)))
            },
            RdfElement::RdfNode(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = RdfNode {
                    uuid: new_uuid,
                    iri: model.iri.clone(),
                    comment: model.comment.clone(),
                };
                RdfElement::RdfNode(Arc::new(RwLock::new(new_model)))
            },
            RdfElement::RdfPredicate(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = RdfPredicate {
                    uuid: new_uuid,
                    iri: model.iri.clone(),
                    source: model.source.clone(),
                    target: model.target.clone(),
                    comment: model.comment.clone(),
                };
                RdfElement::RdfPredicate(Arc::new(RwLock::new(new_model)))
            },
        }
    }

    fn relink(e: &mut RdfElement, all_models: &HashMap<ModelUuid, RdfElement>) {
        match e {
            RdfElement::RdfGraph(rw_lock) => {
                let mut model = rw_lock.write().unwrap();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            }
            RdfElement::RdfLiteral(rw_lock) => {},
            RdfElement::RdfNode(rw_lock) => {},
            RdfElement::RdfPredicate(rw_lock) => {
                let mut model = rw_lock.write().unwrap();

                let source_uuid = *model.source.uuid();
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
    (Arc::new(RwLock::new(new_diagram)), all_models)
}

pub fn fake_copy_diagram(d: &RdfDiagram) -> HashMap<ModelUuid, RdfElement> {
    fn walk(e: &RdfElement, into: &mut HashMap<ModelUuid, RdfElement>) {
        match e {
            RdfElement::RdfGraph(rw_lock) => {
                let model = rw_lock.read().unwrap();

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
#[nh_context_serde(uuid_type = ModelUuid)]
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

    pub fn graph(&self) -> Vec<([SimpleTerm; 3], GraphName<SimpleTerm>)> {
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

impl Model for RdfDiagram {
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
    fn add_element(&mut self, element: RdfElement) -> Result<(), RdfElement> {
        self.contained_elements.push(element);
        Ok(())
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
        Ok(())
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
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

impl Model for RdfGraph {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.iri.clone()
    }
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) {
        v.open_complex(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_complex(self);
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
    fn add_element(&mut self, element: RdfElement) -> Result<(), RdfElement> {
        self.contained_elements.push(element);
        Ok(())
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
        Ok(())
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
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

impl Model for RdfLiteral {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.content.clone()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
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

    fn term_repr(&self) -> SimpleTerm<'static> {
        SimpleTerm::Iri(IriRef::new(MownStr::from((*self.iri).clone())).unwrap())
    }
}

impl Model for RdfNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.iri.clone()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct RdfPredicate {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    #[nh_context_serde(entity)]
    pub source: RdfNodeWrapper,
    #[nh_context_serde(entity)]
    pub target: RdfTargettableElement,

    pub comment: Arc<String>,
}

impl RdfPredicate {
    pub fn new(
        uuid: ModelUuid,
        iri: String,
        source: Arc<RwLock<RdfNode>>,
        destination: RdfTargettableElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            source: source.into(),
            target: destination,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for RdfPredicate {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.iri.clone()
    }
}
