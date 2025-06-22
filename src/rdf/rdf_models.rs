use crate::common::controller::{ContainerModel, Model};
use crate::common::observer::{impl_observable, Observable, Observer};
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashMap, HashSet, VecDeque},
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

pub trait RdfElement: Observable {
    fn uuid(&self) -> Arc<ModelUuid>;
    fn term_repr(&self) -> SimpleTerm<'static>;
    fn accept_collector(&self, collector: &mut RdfCollector<'static>);
}

pub struct RdfDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,
    pub stored_queries: HashMap<uuid::Uuid, (String, String)>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,
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
            observers: VecDeque::new(),
        }
    }

    pub fn graph(&self) -> Vec<([SimpleTerm; 3], GraphName<SimpleTerm>)> {
        let mut collector = RdfCollector {
            data: Vec::new(),
            current_graph: None,
        };

        for c in &self.contained_elements {
            let c = c.read().unwrap();
            c.accept_collector(&mut collector);
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

impl ContainerModel<dyn RdfElement> for RdfDiagram {
    fn add_element(&mut self, element: Arc<RwLock<dyn RdfElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl_observable!(RdfDiagram);

pub struct RdfGraph {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfGraph {
    pub fn new(
        uuid: ModelUuid,
        iri: String,
        contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
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

impl ContainerModel<dyn RdfElement> for RdfGraph {
    fn add_element(&mut self, element: Arc<RwLock<dyn RdfElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl_observable!(RdfGraph);

impl RdfElement for RdfGraph {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }

    fn term_repr(&self) -> SimpleTerm<'static> {
        panic!()
    }

    fn accept_collector(&self, collector: &mut RdfCollector<'static>) {
        let old_graph = collector.current_graph.replace(SimpleTerm::Iri(
            IriRef::new(MownStr::from((*self.iri).clone())).unwrap(),
        ));

        for c in &self.contained_elements {
            let c = c.read().unwrap();
            c.accept_collector(collector);
        }

        collector.current_graph = old_graph;
    }
}

pub struct RdfLiteral {
    pub uuid: Arc<ModelUuid>,
    pub content: Arc<String>,
    pub datatype: Arc<String>,
    pub langtag: Arc<String>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfLiteral {
    pub fn new(uuid: ModelUuid, content: String, datatype: String, langtag: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            content: Arc::new(content),
            datatype: Arc::new(datatype),
            langtag: Arc::new(langtag),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfLiteral);

impl RdfElement for RdfLiteral {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
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

    fn accept_collector(&self, _collector: &mut RdfCollector) {}
}

pub struct RdfNode {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfNode {
    pub fn new(uuid: ModelUuid, iri: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfNode);

impl RdfElement for RdfNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }

    fn term_repr(&self) -> SimpleTerm<'static> {
        SimpleTerm::Iri(IriRef::new(MownStr::from((*self.iri).clone())).unwrap())
    }

    fn accept_collector(&self, _collector: &mut RdfCollector) {}
}

pub struct RdfPredicate {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    pub source: Arc<RwLock<dyn RdfElement>>,
    pub destination: Arc<RwLock<dyn RdfElement>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfPredicate {
    pub fn new(
        uuid: ModelUuid,
        iri: String,
        source: Arc<RwLock<dyn RdfElement>>,
        destination: Arc<RwLock<dyn RdfElement>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            source,
            destination,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfPredicate);

impl RdfElement for RdfPredicate {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }

    fn term_repr(&self) -> SimpleTerm<'static> {
        panic!()
    }

    fn accept_collector(&self, collector: &mut RdfCollector<'static>) {
        let subject = {
            let s = self.source.read().unwrap();
            s.term_repr()
        };
        let object = {
            let o = self.destination.read().unwrap();
            o.term_repr()
        };

        collector.add_triple([
            subject,
            SimpleTerm::Iri(IriRef::new(MownStr::from((*self.iri).clone())).unwrap()),
            object,
        ]);
    }
}
