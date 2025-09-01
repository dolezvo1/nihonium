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
    fn visit_object(&mut self, object: &UmlClassInstance) {
        if self.collecting_absolute_paths {
            self.absolute_paths.insert(
                (*object.uuid).clone(),
                self.absolute_with_current_stack(&*object.instance_type),
            );
        } else {
            self.plantuml_data.push_str(&format!(
                "object {:?}\n",
                if object.instance_name.is_empty() {
                    format!(":{}", object.instance_type)
                } else {
                    format!("{}: {}", object.instance_name, object.instance_type)
                }
            ));
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
                "class {:?} ",
                class.name,
            ));

            if !class.stereotype.is_empty() {
                self.plantuml_data.push_str(&format!("<<{}>> ", class.stereotype));
            }
            self.plantuml_data.push_str("{\n");
            self.plantuml_data.push_str(&class.properties);
            self.plantuml_data.push_str("\n");
            self.plantuml_data.push_str(&class.functions);
            self.plantuml_data.push_str("}\n");
        }
    }
    fn visit_generalization(&mut self, link: &UmlClassGeneralization) {
        if !self.collecting_absolute_paths {
            let target_name = self.absolute_paths.get(&link.target.read().uuid).unwrap();
            for source_name in link.sources.iter().flat_map(|e| self.absolute_paths.get(&e.read().uuid)) {
                self.plantuml_data.push_str(source_name);
                self.plantuml_data.push_str(" --|> ");
                self.plantuml_data.push_str(target_name);
                self.plantuml_data.push_str("\n");
            }
        }
    }
    fn visit_association(&mut self, link: &UmlClassAssociation) {
        if !self.collecting_absolute_paths {
            let source_name = self.absolute_paths.get(&link.source.uuid()).unwrap();
            let target_name = self.absolute_paths.get(&link.target.uuid()).unwrap();

            self.plantuml_data.push_str(source_name);
            if !link.source_label_multiplicity.is_empty() {
                self.plantuml_data
                    .push_str(&format!(" {:?}", link.source_label_multiplicity));
            }
            self.plantuml_data.push_str(match link.link_type {
                UmlClassAssociationType::Association => " -- ",
                UmlClassAssociationType::Aggregation => " --o ",
                UmlClassAssociationType::Composition => " --* ",
                UmlClassAssociationType::InterfaceRealization => " ..|> ",
                UmlClassAssociationType::Usage => " ..> ",
            });
            if !link.target_label_multiplicity.is_empty() {
                self.plantuml_data
                    .push_str(&format!("{:?} ", link.target_label_multiplicity));
            }
            self.plantuml_data.push_str(target_name);
            if !link.stereotype.is_empty() {
                self.plantuml_data.push_str(&format!(": <<{}>>", link.stereotype));
            }
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
    UmlClassInstance(ERef<UmlClassInstance>),
    UmlClass(ERef<UmlClass>),
    UmlClassGeneralization(ERef<UmlClassGeneralization>),
    UmlClassAssociation(ERef<UmlClassAssociation>),
    UmlClassComment(ERef<UmlClassComment>),
    UmlClassCommentLink(ERef<UmlClassCommentLink>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlClassClassifier {
    UmlClassObject(ERef<UmlClassInstance>),
    UmlClass(ERef<UmlClass>),
}

impl UmlClassElement {
    pub fn as_classifier(&self) -> Option<UmlClassClassifier> {
        match self {
            UmlClassElement::UmlClassInstance(inner) => Some(inner.clone().into()),
            UmlClassElement::UmlClass(inner) => Some(inner.clone().into()),
            UmlClassElement::UmlClassPackage(..)
            | UmlClassElement::UmlClassGeneralization(..)
            | UmlClassElement::UmlClassAssociation(..)
            | UmlClassElement::UmlClassComment(..)
            | UmlClassElement::UmlClassCommentLink(..) => None,
        }
    }

    fn accept_uml(&self, visitor: &mut UmlClassCollector) {
        match self {
            UmlClassElement::UmlClassPackage(inner) => visitor.visit_package(&inner.read()),
            UmlClassElement::UmlClassInstance(inner) => visitor.visit_object(&inner.read()),
            UmlClassElement::UmlClass(inner) => visitor.visit_class(&inner.read()),
            UmlClassElement::UmlClassGeneralization(inner) => visitor.visit_generalization(&inner.read()),
            UmlClassElement::UmlClassAssociation(inner) => visitor.visit_association(&inner.read()),
            UmlClassElement::UmlClassComment(..) | UmlClassElement::UmlClassCommentLink(..) => {
                // TODO: comments
            },
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
            UmlClassElement::UmlClassInstance(inner) => {
                UmlClassElement::UmlClassInstance(inner.read().clone_with(*new_uuid))
            }
            UmlClassElement::UmlClass(inner) => {
                UmlClassElement::UmlClass(inner.read().clone_with(*new_uuid))
            },
            UmlClassElement::UmlClassGeneralization(inner) => {
                UmlClassElement::UmlClassGeneralization(inner.read().clone_with(*new_uuid))
            },
            UmlClassElement::UmlClassAssociation(inner) => {
                UmlClassElement::UmlClassAssociation(inner.read().clone_with(*new_uuid))
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
            UmlClassElement::UmlClassInstance(..)
            | UmlClassElement::UmlClass(..) => {},
            UmlClassElement::UmlClassGeneralization(inner) => {
                let mut model = inner.write();

                for e in model.sources.iter_mut() {
                    let sid = *e.read().uuid;
                    if let Some(UmlClassElement::UmlClass(s)) = all_models.get(&sid) {
                        *e = s.clone();
                    }
                }

                let target_uuid = *model.target.read().uuid;
                if let Some(UmlClassElement::UmlClass(t)) = all_models.get(&target_uuid) {
                    model.target = t.clone();
                }
            },
            UmlClassElement::UmlClassAssociation(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.uuid();
                if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_classifier()) {
                    model.source = s;
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_classifier()) {
                    model.target = t;
                }
            },
            UmlClassElement::UmlClassComment(..) => {},
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
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
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
#[nh_context_serde(is_entity)]
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


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassInstance {
    pub uuid: Arc<ModelUuid>,
    pub instance_name: Arc<String>,
    pub instance_type: Arc<String>,
    pub instance_slots: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlClassInstance {
    pub fn new(
        uuid: ModelUuid,
        instance_name: String,
        instance_type: String,
        instance_slots: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            instance_name: Arc::new(instance_name),
            instance_type: Arc::new(instance_type),
            instance_slots: Arc::new(instance_slots),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            instance_name: self.instance_name.clone(),
            instance_type: self.instance_type.clone(),
            instance_slots: self.instance_slots.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlClassInstance {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassInstance {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
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
#[nh_context_serde(is_entity)]
pub struct UmlClass {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub stereotype: Arc<String>,
    pub properties: Arc<String>,
    pub functions: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlClass {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        properties: String,
        functions: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            properties: Arc::new(properties),
            functions: Arc::new(functions),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            properties: self.properties.clone(),
            functions: self.functions.clone(),
            comment: self.comment.clone(),
        })
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


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassGeneralization {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub sources: Vec<ERef<UmlClass>>,
    #[nh_context_serde(entity)]
    pub target: ERef<UmlClass>,

    pub set_name: Arc<String>,
    pub set_is_covering: bool,
    pub set_is_disjoint: bool,

    pub comment: Arc<String>,
}

impl UmlClassGeneralization {
    pub fn new(
        uuid: ModelUuid,
        sources: Vec<ERef<UmlClass>>,
        target: ERef<UmlClass>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            sources,
            target,

            set_name: Arc::new("".to_owned()),
            set_is_covering: false,
            set_is_disjoint: false,

            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            sources: self.sources.clone(),
            target: self.target.clone(),

            set_name: self.set_name.clone(),
            set_is_covering: self.set_is_covering,
            set_is_disjoint: self.set_is_disjoint,

            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlClassGeneralization {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassGeneralization {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlClassAssociationType {
    Association,
    Aggregation,
    Composition,
    InterfaceRealization,
    Usage,
}

impl Default for UmlClassAssociationType {
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
static INTERFACE_REALIZATION_TEXT: LazyLock<Arc<String>> =
    LazyLock::new(|| Arc::new("Interface Realization".to_owned()));
static USAGE_TEXT: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Usage".to_owned()));

impl UmlClassAssociationType {
    pub fn name(&self) -> Arc<String> {
        match self {
            UmlClassAssociationType::Association => ASSOCIATION_TEXT.clone(),
            UmlClassAssociationType::Aggregation => AGGREGATION_TEXT.clone(),
            UmlClassAssociationType::Composition => COMPOSITION_TEXT.clone(),
            UmlClassAssociationType::InterfaceRealization => INTERFACE_REALIZATION_TEXT.clone(),
            UmlClassAssociationType::Usage => USAGE_TEXT.clone(),
        }
    }

    pub fn line_type(&self) -> LineType {
        match self {
            UmlClassAssociationType::InterfaceRealization | UmlClassAssociationType::Usage => LineType::Dashed,
            _ => LineType::Solid,
        }
    }

    pub fn source_arrowhead_type(&self) -> ArrowheadType {
        ArrowheadType::None
    }

    pub fn destination_arrowhead_type(&self) -> ArrowheadType {
        match self {
            UmlClassAssociationType::Association => ArrowheadType::None,
            UmlClassAssociationType::Usage => ArrowheadType::OpenTriangle,
            UmlClassAssociationType::InterfaceRealization => ArrowheadType::EmptyTriangle,
            UmlClassAssociationType::Aggregation => ArrowheadType::EmptyRhombus,
            UmlClassAssociationType::Composition => ArrowheadType::FullRhombus,
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassAssociation {
    pub uuid: Arc<ModelUuid>,
    pub link_type: UmlClassAssociationType,
    pub stereotype: Arc<String>,
    #[nh_context_serde(entity)]
    pub source: UmlClassClassifier,
    pub source_label_multiplicity: Arc<String>,
    pub source_label_role: Arc<String>,
    pub source_label_reading: Arc<String>,
    #[nh_context_serde(entity)]
    pub target: UmlClassClassifier,
    pub target_label_multiplicity: Arc<String>,
    pub target_label_role: Arc<String>,
    pub target_label_reading: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlClassAssociation {
    pub fn new(
        uuid: ModelUuid,
        link_type: UmlClassAssociationType,
        stereotype: String,
        source: UmlClassClassifier,
        target: UmlClassClassifier,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            link_type,
            stereotype: Arc::new(stereotype),
            source,
            source_label_multiplicity: Arc::new("".to_owned()),
            source_label_role: Arc::new("".to_owned()),
            source_label_reading: Arc::new("".to_owned()),
            target,
            target_label_multiplicity: Arc::new("".to_owned()),
            target_label_role: Arc::new("".to_owned()),
            target_label_reading: Arc::new("".to_owned()),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            link_type: self.link_type,
            stereotype: self.stereotype.clone(),
            source: self.source.clone(),
            source_label_multiplicity: self.source_label_multiplicity.clone(),
            source_label_role: self.source_label_role.clone(),
            source_label_reading: self.source_label_reading.clone(),
            target: self.target.clone(),
            target_label_multiplicity: self.target_label_multiplicity.clone(),
            target_label_role: self.target_label_role.clone(),
            target_label_reading: self.target_label_reading.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlClassAssociation {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::Model(*self.uuid)
    }
}

impl Model for UmlClassAssociation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
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
#[nh_context_serde(is_entity)]
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

impl Model for UmlClassCommentLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
