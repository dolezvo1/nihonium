
use std::{collections::VecDeque, sync::{Arc,LazyLock,RwLock}};
use crate::common::observer::{Observer, Observable, impl_observable};
use crate::common::canvas::{ArrowheadType, LineType};
use crate::common::controller::Model;

pub trait UmlClassElement: Observable {
    fn uuid(&self) -> Arc<uuid::Uuid>;
}

pub struct UmlClassDiagram {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn UmlClassElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassDiagram {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn UmlClassElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
    
    pub fn add_element(&mut self, element: Arc<RwLock<dyn UmlClassElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
}

impl Model for UmlClassDiagram {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl_observable!(UmlClassDiagram);

pub struct UmlClassPackage {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn UmlClassElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassPackage {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        contained_elements: Vec<Arc<RwLock<dyn UmlClassElement>>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
    
    pub fn add_element(&mut self, element: Arc<RwLock<dyn UmlClassElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
}

impl_observable!(UmlClassPackage);

pub enum UMLClassAccessModifier {
    Public,
    Package,
    Protected,
    Private,
}

impl UMLClassAccessModifier {
    pub fn char(&self) -> &'static str {
        match self {
            UMLClassAccessModifier::Public => "+",
            UMLClassAccessModifier::Package => "~",
            UMLClassAccessModifier::Protected => "#",
            UMLClassAccessModifier::Private => "-",
        }
    }
}


pub struct UmlClass {
    pub uuid: Arc<uuid::Uuid>,
    pub name: Arc<String>,
    pub stereotype: Arc<String>,
    pub properties: Arc<String>,
    pub functions: Arc<String>,
    
    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClass {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        stereotype: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            stereotype: Arc::new(stereotype),
            properties: Arc::new("".to_owned()),
            functions: Arc::new("".to_owned()),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
    
    pub fn parse_properties(&self) -> Vec<(&str, &str)> {
        Self::parse_string(&self.properties)
    }
    
    pub fn parse_functions(&self) -> Vec<(&str,  &str)> {
        Self::parse_string(&self.functions)
    }
    
    fn parse_string(input: &str) -> Vec<(&str, &str)> {
        input.split("\n")
            .filter(|e| e.len() > 0)
            .map(Self::strip_access_modifiers).collect()
    }
    
    fn strip_access_modifiers(input: &str) -> (&str, &str) {
        for m in [UMLClassAccessModifier::Public,
                  UMLClassAccessModifier::Package,
                  UMLClassAccessModifier::Protected,
                  UMLClassAccessModifier::Private] {
            if let Some(r) = input.strip_prefix(m.char()) {
                return (m.char(), r.trim())
            }
        }
        return ("", input.trim())
    }
}

impl_observable!(UmlClass);

impl UmlClassElement for UmlClass {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassLinkType {
    Association,
    Aggregation,
    Composition,
    Generalization,
    InterfaceRealization,
    Usage,
}

// I hate this so much
static ASSOCIATION_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Assocation".to_owned()));
static AGGREGATION_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Aggregation".to_owned()));
static COMPOSITION_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Composition".to_owned()));
static GENERALIZATION_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Generalization".to_owned()));
static INTERFACE_REALIZATION_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Interface Realization".to_owned()));
static USAGE_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Usage".to_owned()));

impl UmlClassLinkType {
    pub fn name(&self) -> Arc<String> {
        match self {
            UmlClassLinkType::Association => ASSOCIATION_TEXT.clone(),
            UmlClassLinkType::Aggregation => AGGREGATION_TEXT.clone(),
            UmlClassLinkType::Composition => COMPOSITION_TEXT.clone(),
            UmlClassLinkType::Generalization => GENERALIZATION_TEXT.clone(),
            UmlClassLinkType::InterfaceRealization => INTERFACE_REALIZATION_TEXT.clone(),
            UmlClassLinkType::Usage => USAGE_TEXT.clone(),
        }
    }
    
    pub fn line_type(&self) -> LineType {
        match self {
            UmlClassLinkType::InterfaceRealization | UmlClassLinkType::Usage => LineType::Dashed,
            _ => LineType::Solid,
        }
    }
    
    pub fn source_arrowhead_type(&self) -> ArrowheadType {
        ArrowheadType::None
    }
    
    pub fn destination_arrowhead_type(&self) -> ArrowheadType {
        match self {
            UmlClassLinkType::Association => ArrowheadType::None,
            UmlClassLinkType::Usage => ArrowheadType::OpenTriangle,
            UmlClassLinkType::Generalization | UmlClassLinkType::InterfaceRealization => ArrowheadType::EmptyTriangle,
            UmlClassLinkType::Aggregation => ArrowheadType::EmptyRhombus,
            UmlClassLinkType::Composition => ArrowheadType::FullRhombus,
        }
    }
}

pub struct UmlClassLink {
    pub uuid: Arc<uuid::Uuid>,
    pub link_type: UmlClassLinkType,
    pub source: Arc<RwLock<dyn UmlClassElement>>,
    pub source_arrowhead_label: Arc<String>,
    pub destination: Arc<RwLock<dyn UmlClassElement>>,
    pub destination_arrowhead_label: Arc<String>,
    
    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassLink {
    pub fn new(
        uuid: uuid::Uuid,
        link_type: UmlClassLinkType,
        source: Arc<RwLock<dyn UmlClassElement>>,
        destination: Arc<RwLock<dyn UmlClassElement>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            link_type,
            source,
            source_arrowhead_label: Arc::new("".to_owned()),
            destination,
            destination_arrowhead_label: Arc::new("".to_owned()),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(UmlClassLink);

impl UmlClassElement for UmlClassLink {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.uuid.clone()
    }
}
