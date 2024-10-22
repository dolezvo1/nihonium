use crate::common::controller::{ContainerModel, Model};
use crate::common::observer::{impl_observable, Observable, Observer};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, RwLock},
};

pub trait DemoCsdElement: Observable {
    fn uuid(&self) -> Arc<uuid::Uuid>;
}

pub struct DemoCsdDiagram {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdDiagram {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl Model for DemoCsdDiagram {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl ContainerModel<dyn DemoCsdElement> for DemoCsdDiagram {
    fn add_element(&mut self, element: Arc<RwLock<dyn DemoCsdElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl_observable!(DemoCsdDiagram);

pub struct DemoCsdPackage {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdPackage {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn DemoCsdElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl Model for DemoCsdPackage {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl ContainerModel<dyn DemoCsdElement> for DemoCsdPackage {
    fn add_element(&mut self, element: Arc<RwLock<dyn DemoCsdElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl_observable!(DemoCsdPackage);

impl DemoCsdElement for DemoCsdPackage {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

pub struct DemoCsdClient {
    pub uuid: Arc<uuid::Uuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,
    pub internal: bool,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdClient {
    pub fn new(uuid: uuid::Uuid, identifier: String, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            identifier: Arc::new(identifier),
            name: Arc::new(name),
            internal: false,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(DemoCsdClient);

impl DemoCsdElement for DemoCsdClient {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

pub struct DemoCsdTransactor {
    pub uuid: Arc<uuid::Uuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,
    pub internal: bool,
    pub transaction_identifier: Arc<String>,
    pub transaction_name: Arc<String>,
    pub transaction_selfactivating: bool,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdTransactor {
    pub fn new(uuid: uuid::Uuid, identifier: String, name: String, transaction_identifier: String, transaction_name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            identifier: Arc::new(identifier),
            name: Arc::new(name),
            internal: true,
            transaction_identifier: Arc::new(transaction_identifier),
            transaction_name: Arc::new(transaction_name),
            transaction_selfactivating: false,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(DemoCsdTransactor);

impl DemoCsdElement for DemoCsdTransactor {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

pub struct DemoCsdBank {
    pub uuid: Arc<uuid::Uuid>,
    
    pub identifier: Arc<String>,
    pub name: Arc<String>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdBank {
    pub fn new(uuid: uuid::Uuid, identifier: String, name: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            identifier: Arc::new(identifier),
            name: Arc::new(name),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(DemoCsdBank);

impl DemoCsdElement for DemoCsdBank {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemoCsdLinkType {
    Initiation,
    Interstriction,
    Interimpediment,
}

impl DemoCsdLinkType {
    pub fn char(&self) -> &str {
        match self {
            DemoCsdLinkType::Initiation => "Initiation",
            DemoCsdLinkType::Interstriction => "Interstriction",
            DemoCsdLinkType::Interimpediment => "Interimpediment",
        }
    }
}

pub struct DemoCsdLink {
    pub uuid: Arc<uuid::Uuid>,

    pub link_type: DemoCsdLinkType,
    // Client or transactor
    pub source: Arc<RwLock<dyn DemoCsdElement>>,
    // Transaction or bank
    pub target: Arc<RwLock<dyn DemoCsdElement>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdLink {
    pub fn new(
        uuid: uuid::Uuid,
        link_type: DemoCsdLinkType,
        source: Arc<RwLock<dyn DemoCsdElement>>,
        target: Arc<RwLock<dyn DemoCsdElement>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            link_type,
            source,
            target,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(DemoCsdLink);

impl DemoCsdElement for DemoCsdLink {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}
