
use std::{collections::VecDeque, sync::{Arc,RwLock}};
use crate::common::controller::Model;
use crate::common::observer::{Observer, Observable, impl_observable};

pub trait RdfElement: Observable {
    fn uuid(&self) -> Arc<uuid::Uuid>;
}

pub struct RdfDiagram {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfDiagram {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
    
    pub fn add_element(&mut self, element: Arc<RwLock<dyn RdfElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
}

impl Model for RdfDiagram {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl_observable!(RdfDiagram);


pub struct RdfGraph {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,
    
    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfGraph {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn RdfElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
    
    pub fn add_element(&mut self, element: Arc<RwLock<dyn RdfElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
}

impl_observable!(RdfGraph);

impl RdfElement for RdfGraph {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

pub struct RdfLiteral {
    pub uuid: Arc<uuid::Uuid>,
    pub content: Arc<String>,
    pub datatype: Arc<String>,
    pub language: Arc<String>,
    
    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfLiteral {
    pub fn new(
        uuid: uuid::Uuid,
        content: String,
        datatype: String,
        language: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            content: Arc::new(content),
            datatype: Arc::new(datatype),
            language: Arc::new(language),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfLiteral);

impl RdfElement for RdfLiteral {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

pub struct RdfNode {
    pub uuid: Arc<uuid::Uuid>,
    pub iri: Arc<String>,
    
    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfNode {
    pub fn new(
        uuid: uuid::Uuid,
        iri: String,
    ) -> Self {
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
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

pub struct RdfPredicate {
    pub uuid: Arc<uuid::Uuid>,
    pub iri: Arc<String>,
    pub source: Arc<RwLock<dyn RdfElement>>,
    pub destination: Arc<RwLock<dyn RdfElement>>,
    
    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfPredicate {
    pub fn new(
        uuid: uuid::Uuid,
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
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}
