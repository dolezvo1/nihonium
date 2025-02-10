use crate::common::canvas;
use crate::common::controller::{ContainerModel, Model};
use crate::common::observer::{impl_observable, Observable, Observer};
use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, RwLock},
};

pub trait DemoCsdElement: Observable {
    fn uuid(&self) -> Arc<uuid::Uuid>;
}

// ---

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

// ---

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

// ---

pub struct DemoCsdTransactor {
    pub uuid: Arc<uuid::Uuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,
    pub internal: bool,
    pub transaction: Option<Arc<RwLock<DemoCsdTransaction>>>,
    pub transaction_selfactivating: bool,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdTransactor {
    pub fn new(
        uuid: uuid::Uuid,
        identifier: String,
        name: String,
        internal: bool,
        transaction: Option<Arc<RwLock<DemoCsdTransaction>>>,
        transaction_selfactivating: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),

            identifier: Arc::new(identifier),
            name: Arc::new(name),
            internal,
            transaction,
            transaction_selfactivating,

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

// ---

pub struct DemoCsdTransaction {
    pub uuid: Arc<uuid::Uuid>,

    pub identifier: Arc<String>,
    pub name: Arc<String>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdTransaction {
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

impl_observable!(DemoCsdTransaction);

impl DemoCsdElement for DemoCsdTransaction {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

// ---

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

    pub fn line_type(&self) -> canvas::LineType {
        match self {
            DemoCsdLinkType::Initiation => canvas::LineType::Solid,
            DemoCsdLinkType::Interstriction => canvas::LineType::Dashed,
            DemoCsdLinkType::Interimpediment => canvas::LineType::Dashed,
        }
    }
}

pub struct DemoCsdLink {
    pub uuid: Arc<uuid::Uuid>,

    pub link_type: DemoCsdLinkType,
    pub source: Arc<RwLock<DemoCsdTransactor>>,
    pub target: Arc<RwLock<DemoCsdTransaction>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl DemoCsdLink {
    pub fn new(
        uuid: uuid::Uuid,
        link_type: DemoCsdLinkType,
        source: Arc<RwLock<DemoCsdTransactor>>,
        target: Arc<RwLock<DemoCsdTransaction>>,
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
