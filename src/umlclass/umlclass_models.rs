use crate::common::canvas::{ArrowheadType, LineType};
use crate::common::controller::{ContainerModel, Model};
use crate::common::observer::{impl_observable, Observable, Observer};
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, LazyLock, RwLock},
};

pub struct UmlClassCollector {
    collecting_absolute_paths: bool,
    package_stack: Vec<String>,
    absolute_paths: HashMap<ModelUuid, String>,
    plantuml_data: String,
}

impl UmlClassCollector {
    fn absolute_with_current_stack(&self, name: &str) -> String {
        if self.package_stack.is_empty() {
            format!("{:?}", name)
        } else {
            format!("{:?}", self.package_stack.join(".") + "." + name)
        }
    }

    fn visit_package(&mut self, package: &UmlClassPackage) {
        self.package_stack.push((*package.name).clone());
        if !self.collecting_absolute_paths {
            self.plantuml_data
                .push_str(&format!("package {:?} {{\n", package.name));
        }

        for e in &package.contained_elements {
            let e = e.read().unwrap();
            e.accept(self);
        }

        self.package_stack.pop();
        if !self.collecting_absolute_paths {
            self.plantuml_data.push_str("}\n");
        }
    }
    fn visit_class(&mut self, class: &UmlClass) {
        if self.collecting_absolute_paths {
            self.absolute_paths.insert(
                (*class.uuid).clone(),
                self.absolute_with_current_stack(&*class.name),
            );
        } else {
            self.plantuml_data.push_str(&format!(
                "{} {:?} {{\n",
                class.stereotype.name(),
                class.name
            ));
            self.plantuml_data.push_str(&class.properties);
            self.plantuml_data.push_str("\n");
            self.plantuml_data.push_str(&class.functions);
            self.plantuml_data.push_str("}\n");
        }
    }
    fn visit_link(&mut self, link: &UmlClassLink) {
        if !self.collecting_absolute_paths {
            let source_name = {
                let s = link.source.read().unwrap();
                self.absolute_paths.get(&s.uuid()).unwrap()
            };
            let dest_name = {
                let d = link.destination.read().unwrap();
                self.absolute_paths.get(&d.uuid()).unwrap()
            };

            self.plantuml_data.push_str(source_name);
            if !link.source_arrowhead_label.is_empty() {
                self.plantuml_data
                    .push_str(&format!(" {:?}", link.source_arrowhead_label));
            }
            self.plantuml_data.push_str(match link.link_type {
                UmlClassLinkType::Association => " -- ",
                UmlClassLinkType::Aggregation => " --o ",
                UmlClassLinkType::Composition => " --* ",
                UmlClassLinkType::Generalization => " -- ",
                UmlClassLinkType::InterfaceRealization => " ..|> ",
                UmlClassLinkType::Usage => " ..> ",
            });
            if !link.destination_arrowhead_label.is_empty() {
                self.plantuml_data
                    .push_str(&format!("{:?} ", link.destination_arrowhead_label));
            }
            self.plantuml_data.push_str(dest_name);
            self.plantuml_data.push_str("\n");
        }
    }
}

pub trait UmlClassElement: Observable {
    fn uuid(&self) -> Arc<ModelUuid>;
    fn accept(&self, visitor: &mut UmlClassCollector);
}

pub struct UmlClassDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn UmlClassElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassDiagram {
    pub fn new(
        uuid: ModelUuid,
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

    pub fn plantuml(&self) -> String {
        let mut collector = UmlClassCollector {
            collecting_absolute_paths: true,
            package_stack: Vec::new(),
            absolute_paths: HashMap::new(),
            plantuml_data: "".to_owned(),
        };

        for e in &self.contained_elements {
            let e = e.read().unwrap();
            e.accept(&mut collector);
        }

        collector.collecting_absolute_paths = false;

        for e in &self.contained_elements {
            let e = e.read().unwrap();
            e.accept(&mut collector);
        }

        collector.plantuml_data
    }
}

impl Model for UmlClassDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl ContainerModel<dyn UmlClassElement> for UmlClassDiagram {
    fn add_element(&mut self, element: Arc<RwLock<dyn UmlClassElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl_observable!(UmlClassDiagram);

pub struct UmlClassPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub contained_elements: Vec<Arc<RwLock<dyn UmlClassElement>>>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClassPackage {
    pub fn new(
        uuid: ModelUuid,
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
}

impl Model for UmlClassPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl ContainerModel<dyn UmlClassElement> for UmlClassPackage {
    fn add_element(&mut self, element: Arc<RwLock<dyn UmlClassElement>>) {
        self.contained_elements.push(element);
        self.notify_observers();
    }
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>) {
        // TODO
    }
}

impl_observable!(UmlClassPackage);

impl UmlClassElement for UmlClassPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }

    fn accept(&self, visitor: &mut UmlClassCollector) {
        visitor.visit_package(&self);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UmlClassStereotype {
    Abstract,
    AbstractClass,
    Class,
    Entity,
    Enum,
    Interface,
}

impl UmlClassStereotype {
    pub fn char(&self) -> &'static str {
        match self {
            UmlClassStereotype::Abstract => "<<abstract>>",
            UmlClassStereotype::AbstractClass => "<<abstract class>>",
            UmlClassStereotype::Class => "<<class>>",
            UmlClassStereotype::Entity => "<<entity>>",
            UmlClassStereotype::Enum => "<<enum>>",
            UmlClassStereotype::Interface => "<<interface>>",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            UmlClassStereotype::Abstract => "abstract",
            UmlClassStereotype::AbstractClass => "abstract class",
            UmlClassStereotype::Class => "class",
            UmlClassStereotype::Entity => "entity",
            UmlClassStereotype::Enum => "enum",
            UmlClassStereotype::Interface => "interface",
        }
    }
}

#[derive(Clone, Copy, Debug)]
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
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub stereotype: UmlClassStereotype,
    pub properties: Arc<String>,
    pub functions: Arc<String>,

    pub comment: Arc<String>,
    observers: VecDeque<Arc<RwLock<dyn Observer>>>,
}

impl UmlClass {
    pub fn new(
        uuid: ModelUuid,
        stereotype: UmlClassStereotype,
        name: String,
        properties: String,
        functions: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: stereotype,
            name: Arc::new(name),
            properties: Arc::new(properties),
            functions: Arc::new(functions),
            comment: Arc::new("".to_owned()),
            observers: VecDeque::new(),
        }
    }

    pub fn parse_properties(&self) -> Vec<(&str, &str)> {
        Self::parse_string(&self.properties)
    }

    pub fn parse_functions(&self) -> Vec<(&str, &str)> {
        Self::parse_string(&self.functions)
    }

    fn parse_string(input: &str) -> Vec<(&str, &str)> {
        input
            .split("\n")
            .filter(|e| e.len() > 0)
            .map(Self::strip_access_modifiers)
            .collect()
    }

    fn strip_access_modifiers(input: &str) -> (&str, &str) {
        for m in [
            UMLClassAccessModifier::Public,
            UMLClassAccessModifier::Package,
            UMLClassAccessModifier::Protected,
            UMLClassAccessModifier::Private,
        ] {
            if let Some(r) = input.strip_prefix(m.char()) {
                return (m.char(), r.trim());
            }
        }
        return ("", input.trim());
    }
}

impl_observable!(UmlClass);

impl UmlClassElement for UmlClass {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }

    fn accept(&self, visitor: &mut UmlClassCollector) {
        visitor.visit_class(&self);
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
static ASSOCIATION_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Assocation".to_owned()));
static AGGREGATION_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Aggregation".to_owned()));
static COMPOSITION_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Composition".to_owned()));
static GENERALIZATION_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Generalization".to_owned()));
static INTERFACE_REALIZATION_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Interface Realization".to_owned()));
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
            UmlClassLinkType::Generalization | UmlClassLinkType::InterfaceRealization => {
                ArrowheadType::EmptyTriangle
            }
            UmlClassLinkType::Aggregation => ArrowheadType::EmptyRhombus,
            UmlClassLinkType::Composition => ArrowheadType::FullRhombus,
        }
    }
}

pub struct UmlClassLink {
    pub uuid: Arc<ModelUuid>,
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
        uuid: ModelUuid,
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
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }

    fn accept(&self, visitor: &mut UmlClassCollector) {
        visitor.visit_link(&self)
    }
}
