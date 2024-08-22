
use std::{collections::VecDeque, sync::{Arc,RwLock}};
use crate::common::observer::{Observer, Observable, impl_observable};

pub trait RdfElement: Observable {
    fn uuid(&self) -> uuid::Uuid;
}

pub struct RdfDiagram {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub contained_elements: Vec<Arc<RwLock<dyn Observable>>>,

    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfDiagram {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn Observable>>>,
    ) -> Self {
        Self {
            uuid,
            name,
            contained_elements,
            comment: "".to_owned(),
            observers: VecDeque::new(),
        }
    }
    
    pub fn add_element(&mut self, element: Arc<RwLock<dyn Observable>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
}

impl_observable!(RdfDiagram);

/*
pub struct RdfGraph {
    pub name: String,
    pub contained_elements: Vec<Arc<RwLock<dyn Observable>>>,
    
    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl_observable!(RdfGraph);
*/

pub struct RdfLiteral {
    pub uuid: uuid::Uuid,
    pub content: String,
    pub datatype: String,
    pub language: String,
    
    pub comment: String,
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
            uuid,
            content,
            datatype,
            language,
            comment: "".to_owned(),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfLiteral);

impl RdfElement for RdfLiteral {
    fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}

pub struct RdfNode {
    pub uuid: uuid::Uuid,
    pub iri: String,
    
    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfNode {
    pub fn new(
        uuid: uuid::Uuid,
        iri: String,
    ) -> Self {
        Self {
            uuid,
            iri,
            comment: "".to_owned(),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfNode);

impl RdfElement for RdfNode {
    fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}

pub struct RdfPredicate {
    pub uuid: uuid::Uuid,
    pub iri: String,
    pub source: Arc<RwLock<dyn Observable>>,
    pub destination: Arc<RwLock<dyn Observable>>,
    
    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl RdfPredicate {
    pub fn new(
        uuid: uuid::Uuid,
        iri: String,
        source: Arc<RwLock<dyn Observable>>,
        destination: Arc<RwLock<dyn Observable>>,
    ) -> Self {
        Self {
            uuid,
            iri,
            source,
            destination,
            comment: "".to_owned(),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(RdfPredicate);

impl RdfElement for RdfPredicate {
    fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}
