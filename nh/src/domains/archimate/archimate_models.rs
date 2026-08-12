use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::common::{
    entity::{Entity, EntityUuid},
    eref::ERef,
    model::{
        BucketNoT, ContainerModel, DiagramModel, DiagramVisitor, ElementVisitor, Model,
        ModelTopSortInfo, PositionNoT, VisitableDiagram, VisitableElement,
    },
    search::FullTextSearchable,
    uuid::ModelUuid,
    views::multiconnection_view::{MULTICONNECTION_SOURCE_BUCKET, MULTICONNECTION_TARGET_BUCKET},
};

#[derive(
    Clone,
    derive_more::From,
    nh_derive::Model,
    nh_derive::ContainerModel,
    nh_derive::FullTextSearchable,
    nh_derive::NHContextSerDeTag,
)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = ArchiMateElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum ArchiMateElement {
    #[container_model(passthrough = "eref")]
    Concept(ERef<ArchiMateConcept>),
    Relationship(ERef<ArchiMateRelationship>),
}

impl VisitableElement for ArchiMateElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            ArchiMateElement::Concept(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            }
            e => v.visit_simple(e),
        }
    }
}

impl ArchiMateElement {
    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, ArchiMateElement>,
    ) -> Self {
        match self {
            ArchiMateElement::Concept(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            ArchiMateElement::Relationship(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
        }
    }
    pub fn deep_copy_relink(&self, all_models: &HashMap<ModelUuid, ArchiMateElement>) {
        match self {
            ArchiMateElement::Concept(_) => {}
            ArchiMateElement::Relationship(inner) => inner.write().deep_copy_relink(all_models),
        }
    }
}

pub fn deep_copy_diagram(
    d: &ArchiMateDiagram,
) -> (ERef<ArchiMateDiagram>, HashMap<ModelUuid, ArchiMateElement>) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = ArchiMateDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &ArchiMateDiagram) -> HashMap<ModelUuid, ArchiMateElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(e, &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &ArchiMateElement, into: &mut HashMap<ModelUuid, ArchiMateElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        ArchiMateElement::Concept(inner) => {
            let model = inner.read();

            for e in &model.contained_elements {
                enumerate_elements(e, into);
            }
        }
        _ => {}
    }
}

pub fn transitive_closure(
    d: &ArchiMateDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &ArchiMateElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                ArchiMateElement::Concept(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        let mut c = Default::default();
                        enumerate_elements(e, &mut c);
                        when_deleting.extend(c.into_keys());
                    } else {
                        for e in &r.contained_elements {
                            walk(e, when_deleting);
                        }
                    }
                }
                ArchiMateElement::Relationship(..) => {}
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &ArchiMateElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                ArchiMateElement::Concept(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                }
                ArchiMateElement::Relationship(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (r
                            .sources
                            .iter()
                            .all(|e| when_deleting.contains(&e.concept.read().uuid))
                            || r.targets
                                .iter()
                                .all(|e| when_deleting.contains(&e.concept.read().uuid)))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
            }
        }
        for e in &d.contained_elements {
            walk(e, &when_deleting, &mut also_delete);
        }
        if also_delete.is_empty() {
            break;
        }
        when_deleting.extend(also_delete.drain());
    }

    when_deleting
}

pub fn top_sort_info(m: &ArchiMateElement) -> ModelTopSortInfo {
    fn walk(
        e: &ArchiMateElement,
        required_models: &mut HashSet<ModelUuid>,
        provided_models: &mut HashSet<ModelUuid>,
    ) {
        provided_models.insert(*e.uuid());
        match e {
            ArchiMateElement::Concept(inner) => {
                for e in &inner.read().contained_elements {
                    walk(e, required_models, provided_models);
                }
            }
            ArchiMateElement::Relationship(inner) => {
                let r = inner.read();
                for e in &r.sources {
                    required_models.insert(*e.concept.read().uuid);
                }
                for e in &r.targets {
                    required_models.insert(*e.concept.read().uuid);
                }
            }
        }
    }

    let (mut required_models, mut provided_models) = Default::default();
    walk(m, &mut required_models, &mut provided_models);
    ModelTopSortInfo {
        required_models,
        provided_models,
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct ArchiMateDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<ArchiMateElement>,
    pub comment: Arc<String>,
}

impl ArchiMateDiagram {
    pub fn new(uuid: ModelUuid, name: String, contained_elements: Vec<ArchiMateElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }

    pub fn get_element_pos_in(
        &self,
        parent: &ModelUuid,
        uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        if *parent == *self.uuid {
            self.get_element_pos(uuid)
        } else {
            self.find_element(parent)
                .and_then(|e| e.0.get_element_pos(uuid))
        }
    }

    fn insert_element_unsafe(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: ArchiMateElement,
    ) -> Result<PositionNoT, ArchiMateElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.contained_elements.len());
        self.contained_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element_unsafe(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.contained_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }

    pub fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, ArchiMateElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &ArchiMateElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, ArchiMateElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                ArchiMateElement::Concept(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((*w.uuid, e.clone(), 0, idx.try_into().unwrap()));
                        } else {
                            r(e, uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                ArchiMateElement::Relationship(_) => {}
            }
        }

        for (idx, e) in self.contained_elements.iter().enumerate() {
            if uuids.contains(&e.uuid()) {
                undo.push((*self.uuid, e.clone(), 0, idx.try_into().unwrap()));
            } else {
                r(e, uuids, undo);
            }
        }
        self.contained_elements
            .retain(|e| !uuids.contains(&e.uuid()));
    }
}

impl Entity for ArchiMateDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for ArchiMateDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for ArchiMateDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for ArchiMateDiagram {
    type ElementT = ArchiMateElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(ArchiMateElement, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl DiagramModel for ArchiMateDiagram {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        b: BucketNoT,
        p: Option<PositionNoT>,
        element: ArchiMateElement,
    ) -> Result<PositionNoT, ArchiMateElement> {
        match &element {
            ArchiMateElement::Relationship(inner) => {
                if inner
                    .read()
                    .sources
                    .iter()
                    .any(|e| self.find_element(&e.concept.read().uuid).is_none())
                    || inner
                        .read()
                        .targets
                        .iter()
                        .any(|e| self.find_element(&e.concept.read().uuid).is_none())
                {
                    return Err(element);
                }
            }
            _ => {}
        }

        if *self.uuid == target {
            self.insert_element_unsafe(b, p, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };
            match e {
                ArchiMateElement::Concept(inner) => inner.write().insert_element(b, p, element),
                ArchiMateElement::Relationship(inner) => {
                    inner.write().insert_element(b, p, element)
                }
            }
        }
    }

    fn remove_element_from(
        &mut self,
        target: ModelUuid,
        uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        if *self.uuid == target {
            self.remove_element_unsafe(uuid)
        } else {
            match self.find_element(&target)?.0 {
                ArchiMateElement::Concept(inner) => inner.write().remove_element(uuid),
                ArchiMateElement::Relationship(inner) => inner.write().remove_element(uuid),
            }
        }
    }
}

impl FullTextSearchable for ArchiMateDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.name, &self.comment],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum ArchiMateConceptKind {
    // Common domain
    #[default]
    Role,
    Collaboration,
    Path,
    Process,
    Function,
    Service,
    Event,
    Grouping,
    Location,
    // Motivation domain
    Stakeholder,
    Driver,
    Assessment,
    Goal,
    Outcome,
    Principle,
    Requirement,
    Meaning,
    Value,
    // Strategy domain
    Resource,
    Capability,
    ValueStream,
    CourseOfAction,
    // Business domain
    BusinessActor,
    BusinessInterface,
    BusinessObject,
    Product,
    // Application domain
    ApplicationComponent,
    ApplicationInterface,
    DataObject,
    // Technology domain
    Node,
    TechnologyInterface,
    Device,
    SystemSoftware,
    Equipment,
    Facility,
    CommunicationNetwork,
    DistributionNetwork,
    Artifact,
    Material,
    // Implementation and Migration domain
    WorkPackage,
    Deliverable,
    Plateau,
}

pub enum ArchiMateConceptKindColorGroup {
    Common,
    Motivation,
    Strategy,
    Business,
    Application,
    Technology,
    ImplementationAndMigration,
}

pub enum ArchiMateConceptKindShapeGroup {
    Motivational,
    Structural,
    Behavioral,
}

impl ArchiMateConceptKind {
    pub const VARIANTS: [Self; 42] = [
        // Common domain
        Self::Role,
        Self::Collaboration,
        Self::Path,
        Self::Process,
        Self::Function,
        Self::Service,
        Self::Event,
        Self::Grouping,
        Self::Location,
        // Motivation domain
        Self::Stakeholder,
        Self::Driver,
        Self::Assessment,
        Self::Goal,
        Self::Outcome,
        Self::Principle,
        Self::Requirement,
        Self::Meaning,
        Self::Value,
        // Strategy domain
        Self::Resource,
        Self::Capability,
        Self::ValueStream,
        Self::CourseOfAction,
        // Business domain
        Self::BusinessActor,
        Self::BusinessInterface,
        Self::BusinessObject,
        Self::Product,
        // Application domain
        Self::ApplicationComponent,
        Self::ApplicationInterface,
        Self::DataObject,
        // Technology domain
        Self::Node,
        Self::TechnologyInterface,
        Self::Device,
        Self::SystemSoftware,
        Self::Equipment,
        Self::Facility,
        Self::CommunicationNetwork,
        Self::DistributionNetwork,
        Self::Artifact,
        Self::Material,
        // Implementation and Migration domain
        Self::WorkPackage,
        Self::Deliverable,
        Self::Plateau,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            // Common domain
            ArchiMateConceptKind::Role => "Role",
            ArchiMateConceptKind::Collaboration => "Collaboration",
            ArchiMateConceptKind::Path => "Path",
            ArchiMateConceptKind::Process => "Process",
            ArchiMateConceptKind::Function => "Function",
            ArchiMateConceptKind::Service => "Service",
            ArchiMateConceptKind::Event => "Event",
            ArchiMateConceptKind::Grouping => "Grouping",
            ArchiMateConceptKind::Location => "Location",
            // Motivation domain
            ArchiMateConceptKind::Stakeholder => "Stakeholder",
            ArchiMateConceptKind::Driver => "Driver",
            ArchiMateConceptKind::Assessment => "Assessment",
            ArchiMateConceptKind::Goal => "Goal",
            ArchiMateConceptKind::Outcome => "Outcome",
            ArchiMateConceptKind::Principle => "Principle",
            ArchiMateConceptKind::Requirement => "Requirement",
            ArchiMateConceptKind::Meaning => "Meaning",
            ArchiMateConceptKind::Value => "Value",
            // Strategy domain
            ArchiMateConceptKind::Resource => "Resource",
            ArchiMateConceptKind::Capability => "Capability",
            ArchiMateConceptKind::ValueStream => "Value Stream",
            ArchiMateConceptKind::CourseOfAction => "Course of Action",
            // Business domain
            ArchiMateConceptKind::BusinessActor => "Business Actor",
            ArchiMateConceptKind::BusinessInterface => "Business Interface",
            ArchiMateConceptKind::BusinessObject => "Business Object",
            ArchiMateConceptKind::Product => "Product",
            // Application domain
            ArchiMateConceptKind::ApplicationComponent => "Application Component",
            ArchiMateConceptKind::ApplicationInterface => "Application Interface",
            ArchiMateConceptKind::DataObject => "Data Object",
            // Technology domain
            ArchiMateConceptKind::Node => "Node",
            ArchiMateConceptKind::Device => "Device",
            ArchiMateConceptKind::SystemSoftware => "System Software",
            ArchiMateConceptKind::TechnologyInterface => "Technology Interface",
            ArchiMateConceptKind::CommunicationNetwork => "Communication Network",
            ArchiMateConceptKind::Artifact => "Artifact",
            ArchiMateConceptKind::Equipment => "Equipment",
            ArchiMateConceptKind::Facility => "Facility",
            ArchiMateConceptKind::DistributionNetwork => "Distribution Network",
            ArchiMateConceptKind::Material => "Material",
            // Implementation and Migration domain
            ArchiMateConceptKind::WorkPackage => "Work Package",
            ArchiMateConceptKind::Deliverable => "Deliverable",
            ArchiMateConceptKind::Plateau => "Plateau",
        }
    }

    pub fn color_group(&self) -> ArchiMateConceptKindColorGroup {
        match self {
            ArchiMateConceptKind::Role
            | ArchiMateConceptKind::Collaboration
            | ArchiMateConceptKind::Path
            | ArchiMateConceptKind::Process
            | ArchiMateConceptKind::Function
            | ArchiMateConceptKind::Service
            | ArchiMateConceptKind::Event
            | ArchiMateConceptKind::Grouping
            | ArchiMateConceptKind::Location => ArchiMateConceptKindColorGroup::Common,
            ArchiMateConceptKind::Stakeholder
            | ArchiMateConceptKind::Driver
            | ArchiMateConceptKind::Assessment
            | ArchiMateConceptKind::Goal
            | ArchiMateConceptKind::Outcome
            | ArchiMateConceptKind::Principle
            | ArchiMateConceptKind::Requirement
            | ArchiMateConceptKind::Meaning
            | ArchiMateConceptKind::Value => ArchiMateConceptKindColorGroup::Motivation,
            ArchiMateConceptKind::Resource
            | ArchiMateConceptKind::Capability
            | ArchiMateConceptKind::ValueStream
            | ArchiMateConceptKind::CourseOfAction => ArchiMateConceptKindColorGroup::Strategy,
            ArchiMateConceptKind::BusinessActor
            | ArchiMateConceptKind::BusinessInterface
            | ArchiMateConceptKind::BusinessObject
            | ArchiMateConceptKind::Product => ArchiMateConceptKindColorGroup::Business,
            ArchiMateConceptKind::ApplicationComponent
            | ArchiMateConceptKind::ApplicationInterface
            | ArchiMateConceptKind::DataObject => ArchiMateConceptKindColorGroup::Application,
            ArchiMateConceptKind::Node
            | ArchiMateConceptKind::TechnologyInterface
            | ArchiMateConceptKind::Device
            | ArchiMateConceptKind::SystemSoftware
            | ArchiMateConceptKind::Equipment
            | ArchiMateConceptKind::Facility
            | ArchiMateConceptKind::CommunicationNetwork
            | ArchiMateConceptKind::DistributionNetwork
            | ArchiMateConceptKind::Artifact
            | ArchiMateConceptKind::Material => ArchiMateConceptKindColorGroup::Technology,
            ArchiMateConceptKind::WorkPackage
            | ArchiMateConceptKind::Deliverable
            | ArchiMateConceptKind::Plateau => {
                ArchiMateConceptKindColorGroup::ImplementationAndMigration
            }
        }
    }
    pub fn rectangle_shape_group(&self) -> ArchiMateConceptKindShapeGroup {
        match self {
            // Common domain
            ArchiMateConceptKind::Role
            | ArchiMateConceptKind::Collaboration
            | ArchiMateConceptKind::Path => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::Process
            | ArchiMateConceptKind::Function
            | ArchiMateConceptKind::Service
            | ArchiMateConceptKind::Event => ArchiMateConceptKindShapeGroup::Behavioral,
            ArchiMateConceptKind::Grouping
            | ArchiMateConceptKind::Location => ArchiMateConceptKindShapeGroup::Structural,
            // Motivation domain
            ArchiMateConceptKind::Stakeholder
            | ArchiMateConceptKind::Driver
            | ArchiMateConceptKind::Assessment
            | ArchiMateConceptKind::Goal
            | ArchiMateConceptKind::Outcome
            | ArchiMateConceptKind::Principle
            | ArchiMateConceptKind::Requirement
            | ArchiMateConceptKind::Meaning
            | ArchiMateConceptKind::Value => ArchiMateConceptKindShapeGroup::Motivational,
            // Strategy domain
            ArchiMateConceptKind::Resource => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::Capability
            | ArchiMateConceptKind::ValueStream
            | ArchiMateConceptKind::CourseOfAction => ArchiMateConceptKindShapeGroup::Behavioral,
            // Business domain
            ArchiMateConceptKind::BusinessActor
            | ArchiMateConceptKind::BusinessInterface
            | ArchiMateConceptKind::BusinessObject
            | ArchiMateConceptKind::Product
            // Application domain
            | ArchiMateConceptKind::ApplicationComponent
            | ArchiMateConceptKind::ApplicationInterface => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::DataObject
            // Technology domain
            | ArchiMateConceptKind::Node
            | ArchiMateConceptKind::Device
            | ArchiMateConceptKind::SystemSoftware
            | ArchiMateConceptKind::TechnologyInterface
            | ArchiMateConceptKind::CommunicationNetwork
            | ArchiMateConceptKind::Artifact
            | ArchiMateConceptKind::Equipment
            | ArchiMateConceptKind::Facility
            | ArchiMateConceptKind::DistributionNetwork
            | ArchiMateConceptKind::Material => ArchiMateConceptKindShapeGroup::Structural,
            // Implementation and Migration domain
            ArchiMateConceptKind::WorkPackage => ArchiMateConceptKindShapeGroup::Behavioral,
            ArchiMateConceptKind::Deliverable
            | ArchiMateConceptKind::Plateau => ArchiMateConceptKindShapeGroup::Structural,
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct ArchiMateConcept {
    pub uuid: Arc<ModelUuid>,
    pub kind: ArchiMateConceptKind,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<ArchiMateElement>,
    pub comment: Arc<String>,
}

impl ArchiMateConcept {
    pub fn new(
        uuid: ModelUuid,
        kind: ArchiMateConceptKind,
        stereotype: String,
        name: String,
        contained_elements: Vec<ArchiMateElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, ArchiMateElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            kind: self.kind,
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            contained_elements: self
                .contained_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: ArchiMateElement,
    ) -> Result<PositionNoT, ArchiMateElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.contained_elements.len());
        self.contained_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.contained_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl Entity for ArchiMateConcept {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for ArchiMateConcept {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for ArchiMateConcept {
    type ElementT = ArchiMateElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(ArchiMateElement, ModelUuid)> {
        for e in &self.contained_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for ArchiMateConcept {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.kind.as_str(),
                &self.stereotype,
                &self.name,
                &self.comment,
            ],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum ArchiMateJunctionKind {
    #[default]
    AndJunction,
    OrJunction,
}

impl ArchiMateJunctionKind {
    pub const VARIANTS: [Self; 2] = [Self::AndJunction, Self::OrJunction];

    pub fn as_str(&self) -> &str {
        match self {
            ArchiMateJunctionKind::AndJunction => "And Junction",
            ArchiMateJunctionKind::OrJunction => "Or Junction",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum ArchiMateRelationshipKind {
    // Structural Relationships
    Composition,
    Aggregation,
    Assignment,
    Realization,
    // Dependency Relationships
    Serving,
    AccessUnspecified,
    AccessUnidirectional,
    AccessBidirectional,
    Influence,
    #[default]
    AssociationUndirected,
    AssociationDirected,
    // Dynamic Relationships
    Triggering,
    Flow,
    // Other Relationships
    Specialization,
}

impl ArchiMateRelationshipKind {
    pub const VARIANTS: [Self; 14] = [
        // Structural Relationships
        Self::Composition,
        Self::Aggregation,
        Self::Assignment,
        Self::Realization,
        // Dependency Relationships
        Self::Serving,
        Self::AccessUnspecified,
        Self::AccessUnidirectional,
        Self::AccessBidirectional,
        Self::Influence,
        Self::AssociationUndirected,
        Self::AssociationDirected,
        // Dynamic Relationships
        Self::Triggering,
        Self::Flow,
        // Other Relationships
        Self::Specialization,
    ];

    pub fn as_str(&self) -> &str {
        match self {
            ArchiMateRelationshipKind::Composition => "Composition",
            ArchiMateRelationshipKind::Aggregation => "Aggregation",
            ArchiMateRelationshipKind::Assignment => "Assignment",
            ArchiMateRelationshipKind::Realization => "Realization",
            ArchiMateRelationshipKind::Serving => "Serving",
            ArchiMateRelationshipKind::AccessUnspecified => "Access (unspecified)",
            ArchiMateRelationshipKind::AccessUnidirectional => "Access (unidirectional)",
            ArchiMateRelationshipKind::AccessBidirectional => "Access (bidirectional)",
            ArchiMateRelationshipKind::Influence => "Influence",
            ArchiMateRelationshipKind::AssociationUndirected => "Association (undirected)",
            ArchiMateRelationshipKind::AssociationDirected => "Association (directed)",
            ArchiMateRelationshipKind::Triggering => "Triggering",
            ArchiMateRelationshipKind::Flow => "Flow",
            ArchiMateRelationshipKind::Specialization => "Specialization",
        }
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct ArchiMateRelationshipEnding {
    #[nh_context_serde(entity)]
    pub concept: ERef<ArchiMateConcept>,
    pub multiplicity: Arc<String>,
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct ArchiMateRelationship {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: ArchiMateRelationshipKind,
    pub stereotype: Arc<String>,
    #[full_text_searchable(skip)]
    pub junction_kind: ArchiMateJunctionKind,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub sources: Vec<ArchiMateRelationshipEnding>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub targets: Vec<ArchiMateRelationshipEnding>,
}

impl ArchiMateRelationship {
    pub fn new(
        uuid: ModelUuid,
        kind: ArchiMateRelationshipKind,
        stereotype: String,
        sources: Vec<ArchiMateRelationshipEnding>,
        targets: Vec<ArchiMateRelationshipEnding>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            stereotype: Arc::new(stereotype),
            junction_kind: ArchiMateJunctionKind::AndJunction,
            sources,
            targets,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, ArchiMateElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            kind: self.kind,
            stereotype: self.stereotype.clone(),
            junction_kind: self.junction_kind,
            sources: self.sources.clone(),
            targets: self.targets.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, ArchiMateElement>) {
        for e in self.sources.iter_mut() {
            let sid = *e.concept.read().uuid;
            if let Some(ArchiMateElement::Concept(s)) = all_models.get(&sid) {
                e.concept = s.clone();
            }
        }
        for e in self.targets.iter_mut() {
            let tid = *e.concept.read().uuid;
            if let Some(ArchiMateElement::Concept(t)) = all_models.get(&tid) {
                e.concept = t.clone();
            }
        }
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.sources, &mut self.targets);
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: ArchiMateElement,
    ) -> Result<PositionNoT, ArchiMateElement> {
        match bucket {
            MULTICONNECTION_SOURCE_BUCKET if let ArchiMateElement::Concept(c) = element => {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.sources.len());
                self.sources.insert(
                    pos,
                    ArchiMateRelationshipEnding {
                        concept: c,
                        multiplicity: Arc::new("".to_owned()),
                    },
                );
                Ok(pos.try_into().unwrap())
            }
            MULTICONNECTION_TARGET_BUCKET if let ArchiMateElement::Concept(c) = element => {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.targets.len());
                self.targets.insert(
                    pos,
                    ArchiMateRelationshipEnding {
                        concept: c,
                        multiplicity: Arc::new("".to_owned()),
                    },
                );
                Ok(pos.try_into().unwrap())
            }
            _ => Err(element),
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if self.sources.len() > 1 {
            for (idx, e) in self.sources.iter().enumerate() {
                if *e.concept.read().uuid == *uuid {
                    self.sources.remove(idx);
                    return Some((MULTICONNECTION_SOURCE_BUCKET, idx.try_into().unwrap()));
                }
            }
        }
        if self.targets.len() > 1 {
            for (idx, e) in self.targets.iter().enumerate() {
                if *e.concept.read().uuid == *uuid {
                    self.targets.remove(idx);
                    return Some((MULTICONNECTION_TARGET_BUCKET, idx.try_into().unwrap()));
                }
            }
        }
        None
    }
}

impl Entity for ArchiMateRelationship {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for ArchiMateRelationship {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for ArchiMateRelationship {
    type ElementT = ArchiMateElement;

    fn find_element(&self, _uuid: &ModelUuid) -> Option<(ArchiMateElement, ModelUuid)> {
        None
    }
}
