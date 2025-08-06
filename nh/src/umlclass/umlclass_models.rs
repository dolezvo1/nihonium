use crate::common::canvas::{ArrowheadType, LineType};
use crate::common::controller::{ContainerModel, DiagramVisitor, Model, ElementVisitor, VisitableDiagram, VisitableElement};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
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
            e.accept_uml(self);
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
            let source_name = self.absolute_paths.get(&link.source.read().uuid()).unwrap();
            let target_name = self.absolute_paths.get(&link.target.read().uuid()).unwrap();

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
            if !link.target_arrowhead_label.is_empty() {
                self.plantuml_data
                    .push_str(&format!("{:?} ", link.target_arrowhead_label));
            }
            self.plantuml_data.push_str(target_name);
            self.plantuml_data.push_str("\n");
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlClassElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlClassElement {
    #[container_model(passthrough = "eref")]
    UmlClassPackage(ERef<UmlClassPackage>),
    UmlClass(ERef<UmlClass>),
    UmlClassLink(ERef<UmlClassLink>),
    UmlClassComment(ERef<UmlClassComment>),
    UmlClassCommentLink(ERef<UmlClassCommentLink>),
}

impl UmlClassElement {
    fn accept_uml(&self, visitor: &mut UmlClassCollector) {
        match self {
            UmlClassElement::UmlClassPackage(inner) => visitor.visit_package(&inner.read()),
            UmlClassElement::UmlClass(inner) => visitor.visit_class(&inner.read()),
            UmlClassElement::UmlClassLink(inner) => visitor.visit_link(&inner.read()),
            UmlClassElement::UmlClassComment(..) | UmlClassElement::UmlClassCommentLink(..) => {},
        }
    }
}

impl VisitableElement for UmlClassElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            UmlClassElement::UmlClassPackage(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            },
            e => v.visit_simple(e),
        }
    }
}

pub fn deep_copy_diagram(d: &UmlClassDiagram) -> (ERef<UmlClassDiagram>, HashMap<ModelUuid, UmlClassElement>) {
    fn walk(e: &UmlClassElement, into: &mut HashMap<ModelUuid, UmlClassElement>) -> UmlClassElement {
        let new_uuid = Arc::new(uuid::Uuid::now_v7().into());
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let model = inner.read();

                let new_model = UmlClassPackage {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                UmlClassElement::UmlClassPackage(ERef::new(new_model))
            },
            UmlClassElement::UmlClass(inner) => {
                let model = inner.read();

                let new_model = UmlClass {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    stereotype: model.stereotype.clone(),
                    functions: model.functions.clone(),
                    properties: model.properties.clone(),
                    comment: model.comment.clone()
                };
                UmlClassElement::UmlClass(ERef::new(new_model))
            },
            UmlClassElement::UmlClassLink(inner) => {
                let model = inner.read();

                let new_model = UmlClassLink {
                    uuid: new_uuid,
                    description: model.description.clone(),
                    link_type: model.link_type,
                    source: model.source.clone(),
                    source_arrowhead_label: model.source_arrowhead_label.clone(),
                    target: model.target.clone(),
                    target_arrowhead_label: model.target_arrowhead_label.clone(),
                    comment: model.comment.clone(),
                };
                UmlClassElement::UmlClassLink(ERef::new(new_model))
            },
            UmlClassElement::UmlClassComment(inner) => {
                let model = inner.read();

                let new_model = UmlClassComment {
                    uuid: new_uuid,
                    text: model.text.clone(),
                };
                UmlClassElement::UmlClassComment(ERef::new(new_model))
            }
            UmlClassElement::UmlClassCommentLink(inner) => {
                let model = inner.read();

                let new_model = UmlClassCommentLink {
                    uuid: new_uuid,
                    source: model.source.clone(),
                    target: model.target.clone(),
                };
                UmlClassElement::UmlClassCommentLink(ERef::new(new_model))
            }
        }
    }

    fn relink(e: &mut UmlClassElement, all_models: &HashMap<ModelUuid, UmlClassElement>) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            UmlClassElement::UmlClass(_inner) => {},
            UmlClassElement::UmlClassLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlClassElement::UmlClass(s)) = all_models.get(&source_uuid) {
                    model.source = s.clone().into();
                }
                let target_uuid = *model.target.read().uuid();
                if let Some(UmlClassElement::UmlClass(t)) = all_models.get(&target_uuid) {
                    model.target = t.clone().into();
                }
            },
            UmlClassElement::UmlClassComment(_inner) => {},
            UmlClassElement::UmlClassCommentLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlClassElement::UmlClassComment(s)) = all_models.get(&source_uuid) {
                    model.source = s.clone().into();
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid) {
                    model.target = t.clone();
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

    let new_diagram = UmlClassDiagram {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &UmlClassDiagram) -> HashMap<ModelUuid, UmlClassElement> {
    fn walk(e: &UmlClassElement, into: &mut HashMap<ModelUuid, UmlClassElement>) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let model = inner.read();

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
pub struct UmlClassDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlClassElement>,

    pub comment: Arc<String>,
}

impl UmlClassDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<UmlClassElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
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
            e.accept_uml(&mut collector);
        }

        collector.collecting_absolute_paths = false;

        for e in &self.contained_elements {
            e.accept_uml(&mut collector);
        }

        collector.plantuml_data
    }
}

impl Entity for UmlClassDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for UmlClassDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for UmlClassDiagram {
    type ElementT = UmlClassElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlClassElement, ModelUuid)> {
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
    fn add_element(&mut self, element: UmlClassElement) -> Result<(), UmlClassElement> {
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
pub struct UmlClassPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlClassElement>,

    pub comment: Arc<String>,
}

impl UmlClassPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<UmlClassElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Entity for UmlClassPackage {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlClassPackage {
    type ElementT = UmlClassElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlClassElement, ModelUuid)> {
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
    fn add_element(&mut self, element: UmlClassElement) -> Result<(), UmlClassElement> {
        self.contained_elements.push(element);
        Ok(())
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlClassStereotype {
    Abstract,
    AbstractClass,
    Class,
    Entity,
    Enum,
    Interface,
}

// TODO: remove, this is nonsense
impl Default for UmlClassStereotype {
    fn default() -> Self {
        Self::Class
    }
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

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct UmlClass {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub stereotype: UmlClassStereotype,
    pub properties: Arc<String>,
    pub functions: Arc<String>,

    pub comment: Arc<String>,
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

impl Entity for UmlClass {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClass {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlClassLinkType {
    Association,
    Aggregation,
    Composition,
    Generalization,
    InterfaceRealization,
    Usage,
}

impl Default for UmlClassLinkType {
    fn default() -> Self {
        Self::Association
    }
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct UmlClassLink {
    pub uuid: Arc<ModelUuid>,
    pub link_type: UmlClassLinkType,
    pub description: Arc<String>,
    #[nh_context_serde(entity)]
    pub source: ERef<UmlClass>,
    pub source_arrowhead_label: Arc<String>,
    #[nh_context_serde(entity)]
    pub target: ERef<UmlClass>,
    pub target_arrowhead_label: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlClassLink {
    pub fn new(
        uuid: ModelUuid,
        link_type: UmlClassLinkType,
        description: impl Into<String>,
        source: ERef<UmlClass>,
        target: ERef<UmlClass>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            link_type,
            description: Arc::new(description.into()),
            source,
            source_arrowhead_label: Arc::new("".to_owned()),
            target,
            target_arrowhead_label: Arc::new("".to_owned()),
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Entity for UmlClassLink {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct UmlClassComment {
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl UmlClassComment {
    pub fn new(
        uuid: ModelUuid,
        text: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
        }
    }
}

impl Entity for UmlClassComment {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassComment {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ModelUuid)]
pub struct UmlClassCommentLink {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub source: ERef<UmlClassComment>,
    #[nh_context_serde(entity)]
    pub target: UmlClassElement,
}

impl UmlClassCommentLink {
    pub fn new(
        uuid: ModelUuid,
        source: ERef<UmlClassComment>,
        target: UmlClassElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
        }
    }
}

impl Entity for UmlClassCommentLink {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

static COMMENT_LINK_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Comment link".to_owned()));

impl Model for UmlClassCommentLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
