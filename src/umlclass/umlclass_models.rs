
use std::{collections::VecDeque, sync::{Arc,RwLock}};
use crate::common::observer::{Observer, Observable, impl_observable};
use crate::common::canvas::{ArrowheadType, LineType};

pub struct UmlClassDiagram {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub contained_elements: Vec<Arc<RwLock<dyn Observable>>>,

    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassDiagram {
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

impl_observable!(UmlClassDiagram);

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
    pub uuid: uuid::Uuid,
    pub name: String,
    pub stereotype: String,
    pub properties: String,
    pub functions: String,
    
    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClass {
    pub fn new(
        uuid: uuid::Uuid,
        name: String,
        stereotype: String,
    ) -> Self {
        Self {
            uuid,
            name,
            stereotype,
            properties: "".to_owned(),
            functions: "".to_owned(),
            comment: "".to_owned(),
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

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassLinkType {
    Association,
    Aggregation,
    Composition,
    Generalization,
    InterfaceRealization,
    Usage,
}

impl UmlClassLinkType {
    pub fn name(&self) -> &'static str {
        match self {
            UmlClassLinkType::Association => "Association",
            UmlClassLinkType::Aggregation => "Aggregation",
            UmlClassLinkType::Composition => "Composition",
            UmlClassLinkType::Generalization => "Generalization",
            UmlClassLinkType::InterfaceRealization => "Interface Realization",
            UmlClassLinkType::Usage => "Usage",
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
    pub uuid: uuid::Uuid,
    pub link_type: UmlClassLinkType,
    pub source: Arc<RwLock<dyn Observable>>,
    pub source_arrowhead_label: String,
    pub destination: Arc<RwLock<dyn Observable>>,
    pub destination_arrowhead_label: String,
    
    pub comment: String,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassLink {
    pub fn new(
        uuid: uuid::Uuid,
        link_type: UmlClassLinkType,
        source: Arc<RwLock<dyn Observable>>,
        destination: Arc<RwLock<dyn Observable>>,
    ) -> Self {
        Self {
            uuid,
            link_type,
            source,
            source_arrowhead_label: "".to_owned(),
            destination,
            destination_arrowhead_label: "".to_owned(),
            comment: "".to_owned(),
            observers: VecDeque::new(),
        }
    }
}

impl_observable!(UmlClassLink);
