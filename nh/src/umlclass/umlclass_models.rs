use serde::{Deserialize, Serialize};

use crate::common::canvas::{ArrowheadType, LineType};
use crate::common::controller::{ContainerModel, Model, StructuralVisitor};
use crate::common::project_serde::{NHContextDeserialize, NHDeserializeError, NHDeserializer, NHContextSerialize, NHSerializeError, NHSerializer};
use crate::common::uuid::ModelUuid;
use std::{
    collections::{HashMap, HashSet},
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
            let source_name = self.absolute_paths.get(&link.source.uuid()).unwrap();
            let target_name = self.absolute_paths.get(&link.target.uuid()).unwrap();

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

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerializeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[container_model(element_type = UmlClassElement, default_passthrough = "none")]
#[nh_context_serialize_tag(uuid_type = ModelUuid)]
pub enum UmlClassElement {
    #[container_model(passthrough = "arc_rwlock")]
    UmlClassPackage(Arc<RwLock<UmlClassPackage>>),
    UmlClass(Arc<RwLock<UmlClass>>),
    UmlClassLink(Arc<RwLock<UmlClassLink>>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerializeTag)]
#[model(default_passthrough = "arc_rwlock")]
#[nh_context_serialize_tag(uuid_type = ModelUuid)]
pub enum UmlClassWrapper {
    UmlClass(Arc<RwLock<UmlClass>>),
}

impl UmlClassWrapper {
    pub fn unwrap(self) -> Arc<RwLock<UmlClass>> {
        match self {
            Self::UmlClass(c) => c
        }
    }
}

impl UmlClassElement {
    fn accept_uml(&self, visitor: &mut UmlClassCollector) {
        match self {
            UmlClassElement::UmlClassPackage(rw_lock) => visitor.visit_package(&rw_lock.read().unwrap()),
            UmlClassElement::UmlClass(rw_lock) => visitor.visit_class(&rw_lock.read().unwrap()),
            UmlClassElement::UmlClassLink(rw_lock) => visitor.visit_link(&rw_lock.read().unwrap()),
        }
    }
}

impl NHContextSerialize for UmlClassElement {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            UmlClassElement::UmlClassPackage(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            UmlClassElement::UmlClass(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
            UmlClassElement::UmlClassLink(rw_lock) => rw_lock.read().unwrap().serialize_into(into),
        }
    }
}

pub fn deep_copy_diagram(d: &UmlClassDiagram) -> (Arc<RwLock<UmlClassDiagram>>, HashMap<ModelUuid, UmlClassElement>) {
    fn walk(e: &UmlClassElement, into: &mut HashMap<ModelUuid, UmlClassElement>) -> UmlClassElement {
        let new_uuid = Arc::new(uuid::Uuid::now_v7().into());
        match e {
            UmlClassElement::UmlClassPackage(rw_lock) => {
                let model = rw_lock.read().unwrap();

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
                UmlClassElement::UmlClassPackage(Arc::new(RwLock::new(new_model)))
            },
            UmlClassElement::UmlClass(rw_lock) => {
                let model = rw_lock.read().unwrap();

                let new_model = UmlClass {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    stereotype: model.stereotype.clone(),
                    functions: model.functions.clone(),
                    properties: model.properties.clone(),
                    comment: model.comment.clone()
                };
                UmlClassElement::UmlClass(Arc::new(RwLock::new(new_model)))
            },
            UmlClassElement::UmlClassLink(rw_lock) => {
                let model = rw_lock.read().unwrap();

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
                UmlClassElement::UmlClassLink(Arc::new(RwLock::new(new_model)))
            },
        }
    }

    fn relink(e: &mut UmlClassElement, all_models: &HashMap<ModelUuid, UmlClassElement>) {
        match e {
            UmlClassElement::UmlClassPackage(rw_lock) => {
                let mut model = rw_lock.write().unwrap();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            UmlClassElement::UmlClass(rw_lock) => {},
            UmlClassElement::UmlClassLink(rw_lock) => {
                let mut model = rw_lock.write().unwrap();

                let source_uuid = *model.source.uuid();
                if let Some(UmlClassElement::UmlClass(s)) = all_models.get(&source_uuid) {
                    model.source = s.clone().into();
                }
                let target_uuid = *model.target.uuid();
                if let Some(UmlClassElement::UmlClass(t)) = all_models.get(&target_uuid) {
                    model.target = t.clone().into();
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
    (Arc::new(RwLock::new(new_diagram)), all_models)
}

pub fn fake_copy_diagram(d: &UmlClassDiagram) -> HashMap<ModelUuid, UmlClassElement> {
    fn walk(e: &UmlClassElement, into: &mut HashMap<ModelUuid, UmlClassElement>) {
        match e {
            UmlClassElement::UmlClassPackage(rw_lock) => {
                let model = rw_lock.read().unwrap();

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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UmlClassDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[serde(skip_deserializing)]
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

impl Model for UmlClassDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) {
        v.open_complex(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_complex(self);
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

impl NHContextSerialize for UmlClassDiagram {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        for e in &self.contained_elements {
            e.serialize_into(into);
        }

        Ok(())
    }
}

impl NHContextDeserialize for UmlClassDiagram {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let e = source.get("contained_elements").ok_or_else(|| NHDeserializeError::StructureError("contained_elements not found".into()))?;
        let contained_elements = Vec::<UmlClassElement>::deserialize(e, deserializer)?;
        Ok(Self { contained_elements, ..toml::Value::try_into(source.clone()).unwrap() })
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UmlClassPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[serde(skip_deserializing)]
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

impl Model for UmlClassPackage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) {
        v.open_complex(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_complex(self);
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

impl NHContextSerialize for UmlClassPackage {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        for e in &self.contained_elements {
            e.serialize_into(into);
        }

        Ok(())
    }
}

impl NHContextDeserialize for UmlClassPackage {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let e = source.get("contained_elements").ok_or_else(|| NHDeserializeError::StructureError("contained_elements not found".into()))?;
        let contained_elements = Vec::<UmlClassElement>::deserialize(e, deserializer)?;
        Ok(Self { contained_elements, ..toml::Value::try_into(source.clone()).unwrap() })
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

#[derive(serde::Serialize, serde::Deserialize)]
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

impl Model for UmlClass {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.name.clone()
    }
}

impl NHContextSerialize for UmlClass {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        Ok(())
    }
}

impl NHContextDeserialize for UmlClass {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        Ok(toml::Value::try_into(source.clone())?)
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

#[derive(serde::Serialize)]
pub struct UmlClassLink {
    pub uuid: Arc<ModelUuid>,
    pub link_type: UmlClassLinkType,
    pub description: Arc<String>,
    pub source: UmlClassWrapper,
    pub source_arrowhead_label: Arc<String>,
    pub target: UmlClassWrapper,
    pub target_arrowhead_label: Arc<String>,

    pub comment: Arc<String>,
}

#[derive(serde::Deserialize)]
struct UmlClassLinkHelper {
    pub uuid: Arc<ModelUuid>,
    pub link_type: UmlClassLinkType,
    pub description: Arc<String>,
    pub source_arrowhead_label: Arc<String>,
    pub target_arrowhead_label: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlClassLink {
    pub fn new(
        uuid: ModelUuid,
        link_type: UmlClassLinkType,
        description: impl Into<String>,
        source: Arc<RwLock<UmlClass>>,
        target: Arc<RwLock<UmlClass>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            link_type,
            description: Arc::new(description.into()),
            source: source.into(),
            source_arrowhead_label: Arc::new("".to_owned()),
            target: target.into(),
            target_arrowhead_label: Arc::new("".to_owned()),
            comment: Arc::new("".to_owned()),
        }
    }
}

impl Model for UmlClassLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
    fn name(&self) -> Arc<String> {
        self.description.clone()
    }
}

impl NHContextSerialize for UmlClassLink {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        if into.contains_model(&self.uuid) {
            return Ok(());
        }

        let s = toml::Table::try_from(self)?;
        into.insert_model(*self.uuid, s);

        Ok(())
    }
}

impl NHContextDeserialize for UmlClassLink {
    fn deserialize(
        from: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let s = from.get("source").unwrap();
        let source = UmlClassWrapper::deserialize(s, deserializer)?;
        let t = from.get("target").unwrap();
        let target = UmlClassWrapper::deserialize(t, deserializer)?;
        let helper: UmlClassLinkHelper = toml::Value::try_into(from.clone()).unwrap();

        Ok(Self {
            source, target,
            uuid: helper.uuid,
            link_type: helper.link_type,
            description: helper.description,
            source_arrowhead_label: helper.source_arrowhead_label,
            target_arrowhead_label: helper.target_arrowhead_label,
            comment: helper.comment,
        })
    }
}
