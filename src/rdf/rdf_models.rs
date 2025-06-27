use crate::common::controller::{ContainerModel, Model};
use crate::common::project_serde::{NHSerialize, NHSerializeError, NHSerializer};
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

#[derive(Clone, derive_more::From)]
pub enum RdfElement {
    RdfGraph(Arc<RwLock<RdfGraph>>),
    RdfTargettable(RdfTargettableElement),
    RdfPredicate(Arc<RwLock<RdfPredicate>>),
}

#[derive(Clone, derive_more::From)]
pub enum RdfTargettableElement {
    RdfLiteral(Arc<RwLock<RdfLiteral>>),
    RdfNode(Arc<RwLock<RdfNode>>),
}

impl RdfElement {
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
            RdfElement::RdfTargettable(_) => {}
            RdfElement::RdfPredicate(rw_lock) => {
                let model = rw_lock.read().unwrap();
                let subject = {
                    let s = model.source.read().unwrap();
                    s.term_repr()
                };
                let object = model.destination.term_repr();

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

impl Model for RdfElement {
    fn uuid(&self) -> Arc<ModelUuid> {
        match self {
            RdfElement::RdfGraph(rw_lock) => rw_lock.read().unwrap().uuid(),
            RdfElement::RdfTargettable(t) => t.uuid(),
            RdfElement::RdfPredicate(rw_lock) => rw_lock.read().unwrap().uuid(),
        }
    }

    fn name(&self) -> Arc<String> {
        match self {
            RdfElement::RdfGraph(rw_lock) => rw_lock.read().unwrap().name(),
            RdfElement::RdfTargettable(t) => t.name(),
            RdfElement::RdfPredicate(rw_lock) => rw_lock.read().unwrap().name(),
        }
    }
}

impl Model for RdfTargettableElement {
    fn uuid(&self) -> Arc<ModelUuid> {
        match self {
            RdfTargettableElement::RdfLiteral(rw_lock) => rw_lock.read().unwrap().uuid(),
            RdfTargettableElement::RdfNode(rw_lock) => rw_lock.read().unwrap().uuid(),
        }
    }

    fn name(&self) -> Arc<String> {
        match self {
            RdfTargettableElement::RdfLiteral(rw_lock) => rw_lock.read().unwrap().name(),
            RdfTargettableElement::RdfNode(rw_lock) => rw_lock.read().unwrap().name(),
        }
    }
}

impl NHSerialize for RdfElement {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            RdfElement::RdfGraph(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            RdfElement::RdfTargettable(t) => t.serialize_into(into),
            RdfElement::RdfPredicate(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
        }
    }
}

impl NHSerialize for RdfTargettableElement {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            RdfTargettableElement::RdfLiteral(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            RdfTargettableElement::RdfNode(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
        }
    }
}

pub struct RdfDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
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
}

impl ContainerModel<RdfElement> for RdfDiagram {
    fn add_element(&mut self, element: RdfElement) {
        self.contained_elements.push(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
    }
}

impl NHSerialize for RdfDiagram {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("rdf-diagram-model".to_owned()));
        element.insert("name".to_owned(), toml::Value::String((*self.name).clone()));

        for e in &self.contained_elements {
            e.serialize_into(into)?;
        }
        element.insert("contained_elements".to_owned(),
            toml::Value::Array(self.contained_elements.iter().map(|e| toml::Value::String(e.uuid().to_string())).collect())
        );

        element.insert("stored_queries".to_owned(), toml::Value::Array(self.stored_queries.iter().map(|e| {
            let mut hm = toml::Table::new();
            hm.insert("uuid".to_owned(), toml::Value::String(e.0.to_string()));
            hm.insert("name".to_owned(), toml::Value::String(e.1.0.clone()));
            hm.insert("value".to_owned(), toml::Value::String(e.1.1.clone()));
            toml::Value::Table(hm)
        }).collect()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

pub struct RdfGraph {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
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
}

impl ContainerModel<RdfElement> for RdfGraph {
    fn add_element(&mut self, element: RdfElement) {
        self.contained_elements.push(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
    }
}

impl NHSerialize for RdfGraph {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("rdf-graph-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("iri".to_owned(), toml::Value::String((*self.iri).clone()));

        for e in &self.contained_elements {
            e.serialize_into(into)?;
        }
        element.insert("contained_elements".to_owned(),
            toml::Value::Array(self.contained_elements.iter().map(|e| toml::Value::String(e.uuid().to_string())).collect())
        );

        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

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

impl NHSerialize for RdfLiteral {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("rdf-literal-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("content".to_owned(), toml::Value::String((*self.content).clone()));
        element.insert("datatype".to_owned(), toml::Value::String((*self.datatype).clone()));
        element.insert("langtag".to_owned(), toml::Value::String((*self.langtag).clone()));
        element.insert("comment".to_owned(), toml::Value::String((*self.comment).clone()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

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

impl NHSerialize for RdfNode {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("rdf-node-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("iri".to_owned(), toml::Value::String((*self.iri).clone()));
        element.insert("comment".to_owned(), toml::Value::String((*self.comment).clone()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}

pub struct RdfPredicate {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    pub source: Arc<RwLock<RdfNode>>,
    pub destination: RdfTargettableElement,

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
            source,
            destination,
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

impl NHSerialize for RdfPredicate {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let mut element = toml::Table::new();
        element.insert("_type".to_owned(), toml::Value::String("rdf-predicate-model".to_owned()));
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("iri".to_owned(), toml::Value::String((*self.iri).clone()));
        element.insert("source".to_owned(), toml::Value::String(self.source.read().unwrap().uuid().to_string()));
        element.insert("destination".to_owned(), toml::Value::String(self.destination.uuid().to_string()));
        element.insert("comment".to_owned(), toml::Value::String((*self.comment).clone()));
        into.insert_model(*self.uuid, element);

        Ok(())
    }
}
