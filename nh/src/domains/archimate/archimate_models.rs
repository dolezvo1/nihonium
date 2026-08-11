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
                            .all(|e| when_deleting.contains(&e.read().uuid))
                            || r.targets
                                .iter()
                                .all(|e| when_deleting.contains(&e.read().uuid)))
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
                    required_models.insert(*e.read().uuid);
                }
                for e in &r.targets {
                    required_models.insert(*e.read().uuid);
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
                    .any(|e| self.find_element(&e.read().uuid).is_none())
                    || inner
                        .read()
                        .targets
                        .iter()
                        .any(|e| self.find_element(&e.read().uuid).is_none())
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
    // Motivational concepts
    #[default]
    Stakeholder,
    Driver,
    Assessment,
    Goal,
    Outcome,
    Principle,
    Requirement,
    Constraint,
    Meaning,
    Value,
    // Strategy Layer concepts
    Resource,
    Capability,
    ValueStream,
    CourseOfAction,
    // Business Layer concepts
    BusinessActor,
    BusinessRole,
    BusinessCollaboration,
    BusinessInterface,
    BusinessProcess,
    BusinessFunction,
    BusinessInteraction,
    BusinessEvent,
    BusinessService,
    BusinessObject,
    Contract,
    Representation,
    Product,
    // Application Layer concepts
    ApplicationComponent,
    ApplicationCollaboration,
    ApplicationInterface,
    ApplicationFunction,
    ApplicationInteraction,
    ApplicationProcess,
    ApplicationEvent,
    ApplicationService,
    DataObject,
    // Technology Layer concepts
    Node,
    Device,
    SystemSoftware,
    TechnologyCollaboration,
    TechnologyInterface,
    Path,
    CommunicationNetwork,
    TechnologyFunction,
    TechnologyProcess,
    TechnologyInteraction,
    TechnologyEvent,
    TechnologyService,
    Artifact,
    Equipment,
    Facility,
    DistributionNetwork,
    Material,
    // Implementation and Migration concepts
    WorkPackage,
    Deliverable,
    ImplementationEvent,
    Plateau,
    Gap,
    // "Composite" concepts
    Grouping,
    Location,
}

pub enum ArchiMateConceptKindColorGroup {
    Motivational,
    StrategyLayer,
    BusinessLayer,
    ApplicationLayer,
    TechnologyLayer,
    ImplementationAndMigration,
    Grouping,
    Location,
}

pub enum ArchiMateConceptKindShapeGroup {
    Motivational,
    Structural,
    Behavioral,
}

impl ArchiMateConceptKind {
    pub const VARIANTS: [Self; 60] = [
        // Motivational concepts
        Self::Stakeholder,
        Self::Driver,
        Self::Assessment,
        Self::Goal,
        Self::Outcome,
        Self::Principle,
        Self::Requirement,
        Self::Constraint,
        Self::Meaning,
        Self::Value,
        // Strategy Layer concepts
        Self::Resource,
        Self::Capability,
        Self::ValueStream,
        Self::CourseOfAction,
        // Business Layer concepts
        Self::BusinessActor,
        Self::BusinessRole,
        Self::BusinessCollaboration,
        Self::BusinessInterface,
        Self::BusinessProcess,
        Self::BusinessFunction,
        Self::BusinessInteraction,
        Self::BusinessEvent,
        Self::BusinessService,
        Self::BusinessObject,
        Self::Contract,
        Self::Representation,
        Self::Product,
        // Application Layer concepts
        Self::ApplicationComponent,
        Self::ApplicationCollaboration,
        Self::ApplicationInterface,
        Self::ApplicationFunction,
        Self::ApplicationInteraction,
        Self::ApplicationProcess,
        Self::ApplicationEvent,
        Self::ApplicationService,
        Self::DataObject,
        // Technology Layer concepts
        Self::Node,
        Self::Device,
        Self::SystemSoftware,
        Self::TechnologyCollaboration,
        Self::TechnologyInterface,
        Self::Path,
        Self::CommunicationNetwork,
        Self::TechnologyFunction,
        Self::TechnologyProcess,
        Self::TechnologyInteraction,
        Self::TechnologyEvent,
        Self::TechnologyService,
        Self::Artifact,
        Self::Equipment,
        Self::Facility,
        Self::DistributionNetwork,
        Self::Material,
        // Implementation and Migration concepts
        Self::WorkPackage,
        Self::Deliverable,
        Self::ImplementationEvent,
        Self::Plateau,
        Self::Gap,
        // "Composite" concepts
        Self::Grouping,
        Self::Location,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ArchiMateConceptKind::Stakeholder => "Stakeholder",
            ArchiMateConceptKind::Driver => "Driver",
            ArchiMateConceptKind::Assessment => "Assessment",
            ArchiMateConceptKind::Goal => "Goal",
            ArchiMateConceptKind::Outcome => "Outcome",
            ArchiMateConceptKind::Principle => "Principle",
            ArchiMateConceptKind::Requirement => "Requirement",
            ArchiMateConceptKind::Constraint => "Constraint",
            ArchiMateConceptKind::Meaning => "Meaning",
            ArchiMateConceptKind::Value => "Value",
            ArchiMateConceptKind::Resource => "Resource",
            ArchiMateConceptKind::Capability => "Capability",
            ArchiMateConceptKind::ValueStream => "Value Stream",
            ArchiMateConceptKind::CourseOfAction => "Course of Action",
            ArchiMateConceptKind::BusinessActor => "Business Actor",
            ArchiMateConceptKind::BusinessRole => "Business Role",
            ArchiMateConceptKind::BusinessCollaboration => "Business Collaboration",
            ArchiMateConceptKind::BusinessInterface => "Business Interface",
            ArchiMateConceptKind::BusinessProcess => "Business Process",
            ArchiMateConceptKind::BusinessFunction => "Business Function",
            ArchiMateConceptKind::BusinessInteraction => "Business Interaction",
            ArchiMateConceptKind::BusinessEvent => "Business Event",
            ArchiMateConceptKind::BusinessService => "Business Service",
            ArchiMateConceptKind::BusinessObject => "Business Object",
            ArchiMateConceptKind::Contract => "Contract",
            ArchiMateConceptKind::Representation => "Representation",
            ArchiMateConceptKind::Product => "Product",
            ArchiMateConceptKind::ApplicationComponent => "Application Component",
            ArchiMateConceptKind::ApplicationCollaboration => "Application Collaboration",
            ArchiMateConceptKind::ApplicationInterface => "Application Interface",
            ArchiMateConceptKind::ApplicationFunction => "Application Function",
            ArchiMateConceptKind::ApplicationInteraction => "Application Interaction",
            ArchiMateConceptKind::ApplicationProcess => "Application Process",
            ArchiMateConceptKind::ApplicationEvent => "Application Event",
            ArchiMateConceptKind::ApplicationService => "Application Service",
            ArchiMateConceptKind::DataObject => "Data Object",
            ArchiMateConceptKind::Node => "Node",
            ArchiMateConceptKind::Device => "Device",
            ArchiMateConceptKind::SystemSoftware => "System Software",
            ArchiMateConceptKind::TechnologyCollaboration => "Technology Collaboration",
            ArchiMateConceptKind::TechnologyInterface => "Technology Interface",
            ArchiMateConceptKind::Path => "Path",
            ArchiMateConceptKind::CommunicationNetwork => "Communication Network",
            ArchiMateConceptKind::TechnologyFunction => "Technology Function",
            ArchiMateConceptKind::TechnologyProcess => "Technology Process",
            ArchiMateConceptKind::TechnologyInteraction => "Technology Interaction",
            ArchiMateConceptKind::TechnologyEvent => "Technology Event",
            ArchiMateConceptKind::TechnologyService => "Technology Service",
            ArchiMateConceptKind::Artifact => "Artifact",
            ArchiMateConceptKind::Equipment => "Equipment",
            ArchiMateConceptKind::Facility => "Facility",
            ArchiMateConceptKind::DistributionNetwork => "Distribution Network",
            ArchiMateConceptKind::Material => "Material",
            ArchiMateConceptKind::WorkPackage => "Work Package",
            ArchiMateConceptKind::Deliverable => "Deliverable",
            ArchiMateConceptKind::ImplementationEvent => "Implementation Event",
            ArchiMateConceptKind::Plateau => "Plateau",
            ArchiMateConceptKind::Gap => "Gap",
            ArchiMateConceptKind::Grouping => "Grouping",
            ArchiMateConceptKind::Location => "Location",
        }
    }

    pub fn color_group(&self) -> ArchiMateConceptKindColorGroup {
        match self {
            ArchiMateConceptKind::Stakeholder
            | ArchiMateConceptKind::Driver
            | ArchiMateConceptKind::Assessment
            | ArchiMateConceptKind::Goal
            | ArchiMateConceptKind::Outcome
            | ArchiMateConceptKind::Principle
            | ArchiMateConceptKind::Requirement
            | ArchiMateConceptKind::Constraint
            | ArchiMateConceptKind::Meaning
            | ArchiMateConceptKind::Value => ArchiMateConceptKindColorGroup::Motivational,
            ArchiMateConceptKind::Resource
            | ArchiMateConceptKind::Capability
            | ArchiMateConceptKind::ValueStream
            | ArchiMateConceptKind::CourseOfAction => ArchiMateConceptKindColorGroup::StrategyLayer,
            ArchiMateConceptKind::BusinessActor
            | ArchiMateConceptKind::BusinessRole
            | ArchiMateConceptKind::BusinessCollaboration
            | ArchiMateConceptKind::BusinessInterface
            | ArchiMateConceptKind::BusinessProcess
            | ArchiMateConceptKind::BusinessFunction
            | ArchiMateConceptKind::BusinessInteraction
            | ArchiMateConceptKind::BusinessEvent
            | ArchiMateConceptKind::BusinessService
            | ArchiMateConceptKind::BusinessObject
            | ArchiMateConceptKind::Contract
            | ArchiMateConceptKind::Representation
            | ArchiMateConceptKind::Product => ArchiMateConceptKindColorGroup::BusinessLayer,
            ArchiMateConceptKind::ApplicationComponent
            | ArchiMateConceptKind::ApplicationCollaboration
            | ArchiMateConceptKind::ApplicationInterface
            | ArchiMateConceptKind::ApplicationFunction
            | ArchiMateConceptKind::ApplicationInteraction
            | ArchiMateConceptKind::ApplicationProcess
            | ArchiMateConceptKind::ApplicationEvent
            | ArchiMateConceptKind::ApplicationService
            | ArchiMateConceptKind::DataObject => ArchiMateConceptKindColorGroup::ApplicationLayer,
            ArchiMateConceptKind::Node
            | ArchiMateConceptKind::Device
            | ArchiMateConceptKind::SystemSoftware
            | ArchiMateConceptKind::TechnologyCollaboration
            | ArchiMateConceptKind::TechnologyInterface
            | ArchiMateConceptKind::Path
            | ArchiMateConceptKind::CommunicationNetwork
            | ArchiMateConceptKind::TechnologyFunction
            | ArchiMateConceptKind::TechnologyProcess
            | ArchiMateConceptKind::TechnologyInteraction
            | ArchiMateConceptKind::TechnologyEvent
            | ArchiMateConceptKind::TechnologyService
            | ArchiMateConceptKind::Artifact
            | ArchiMateConceptKind::Equipment
            | ArchiMateConceptKind::Facility
            | ArchiMateConceptKind::DistributionNetwork
            | ArchiMateConceptKind::Material => ArchiMateConceptKindColorGroup::TechnologyLayer,
            ArchiMateConceptKind::WorkPackage
            | ArchiMateConceptKind::ImplementationEvent
            | ArchiMateConceptKind::Deliverable
            | ArchiMateConceptKind::Plateau
            | ArchiMateConceptKind::Gap => {
                ArchiMateConceptKindColorGroup::ImplementationAndMigration
            }
            ArchiMateConceptKind::Grouping => ArchiMateConceptKindColorGroup::Grouping,
            ArchiMateConceptKind::Location => ArchiMateConceptKindColorGroup::Location,
        }
    }
    pub fn rectangle_shape_group(&self) -> ArchiMateConceptKindShapeGroup {
        match self {
            // Motivational concepts
            ArchiMateConceptKind::Stakeholder
            | ArchiMateConceptKind::Driver
            | ArchiMateConceptKind::Assessment
            | ArchiMateConceptKind::Goal
            | ArchiMateConceptKind::Outcome
            | ArchiMateConceptKind::Principle
            | ArchiMateConceptKind::Requirement
            | ArchiMateConceptKind::Constraint
            | ArchiMateConceptKind::Meaning
            | ArchiMateConceptKind::Value => ArchiMateConceptKindShapeGroup::Motivational,
            // Strategy concepts
            ArchiMateConceptKind::Resource => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::Capability
            | ArchiMateConceptKind::ValueStream
            | ArchiMateConceptKind::CourseOfAction => ArchiMateConceptKindShapeGroup::Behavioral,
            // Business Layer concepts
            ArchiMateConceptKind::BusinessActor
            | ArchiMateConceptKind::BusinessRole
            | ArchiMateConceptKind::BusinessCollaboration
            | ArchiMateConceptKind::BusinessInterface => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::BusinessProcess
            | ArchiMateConceptKind::BusinessFunction
            | ArchiMateConceptKind::BusinessInteraction
            | ArchiMateConceptKind::BusinessEvent
            | ArchiMateConceptKind::BusinessService => ArchiMateConceptKindShapeGroup::Behavioral,
            ArchiMateConceptKind::BusinessObject
            | ArchiMateConceptKind::Contract
            | ArchiMateConceptKind::Representation
            | ArchiMateConceptKind::Product
            // Application Layer concepts
            | ArchiMateConceptKind::ApplicationComponent
            | ArchiMateConceptKind::ApplicationCollaboration
            | ArchiMateConceptKind::ApplicationInterface => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::ApplicationFunction
            | ArchiMateConceptKind::ApplicationInteraction
            | ArchiMateConceptKind::ApplicationProcess
            | ArchiMateConceptKind::ApplicationEvent
            | ArchiMateConceptKind::ApplicationService => ArchiMateConceptKindShapeGroup::Behavioral,
            ArchiMateConceptKind::DataObject
            // Technology Layer concepts
            | ArchiMateConceptKind::Node
            | ArchiMateConceptKind::Device
            | ArchiMateConceptKind::SystemSoftware
            | ArchiMateConceptKind::TechnologyCollaboration
            | ArchiMateConceptKind::TechnologyInterface
            | ArchiMateConceptKind::Path
            | ArchiMateConceptKind::CommunicationNetwork => ArchiMateConceptKindShapeGroup::Structural,
            ArchiMateConceptKind::TechnologyFunction
            | ArchiMateConceptKind::TechnologyProcess
            | ArchiMateConceptKind::TechnologyInteraction
            | ArchiMateConceptKind::TechnologyEvent
            | ArchiMateConceptKind::TechnologyService => ArchiMateConceptKindShapeGroup::Behavioral,
            ArchiMateConceptKind::Artifact
            | ArchiMateConceptKind::Equipment
            | ArchiMateConceptKind::Facility
            | ArchiMateConceptKind::DistributionNetwork
            | ArchiMateConceptKind::Material => ArchiMateConceptKindShapeGroup::Structural,
            // Implementation and Migration concepts
            ArchiMateConceptKind::WorkPackage
            | ArchiMateConceptKind::ImplementationEvent => ArchiMateConceptKindShapeGroup::Behavioral,
            ArchiMateConceptKind::Deliverable
            | ArchiMateConceptKind::Plateau
            | ArchiMateConceptKind::Gap
            // "Composite" concepts
            | ArchiMateConceptKind::Grouping
            | ArchiMateConceptKind::Location => ArchiMateConceptKindShapeGroup::Structural,
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct ArchiMateConcept {
    pub uuid: Arc<ModelUuid>,
    pub kind: ArchiMateConceptKind,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<ArchiMateElement>,
    pub comment: Arc<String>,
}

impl ArchiMateConcept {
    pub fn new(
        uuid: ModelUuid,
        kind: ArchiMateConceptKind,
        name: String,
        contained_elements: Vec<ArchiMateElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
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
    #[default]
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
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct ArchiMateRelationship {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: ArchiMateRelationshipKind,
    #[full_text_searchable(skip)]
    pub junction_kind: ArchiMateJunctionKind,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub sources: Vec<ERef<ArchiMateConcept>>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub targets: Vec<ERef<ArchiMateConcept>>,
}

impl ArchiMateRelationship {
    pub fn new(
        uuid: ModelUuid,
        kind: ArchiMateRelationshipKind,
        sources: Vec<ERef<ArchiMateConcept>>,
        targets: Vec<ERef<ArchiMateConcept>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
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
            junction_kind: self.junction_kind,
            sources: self.sources.clone(),
            targets: self.targets.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, ArchiMateElement>) {
        for e in self.sources.iter_mut() {
            let sid = *e.read().uuid;
            if let Some(ArchiMateElement::Concept(s)) = all_models.get(&sid) {
                *e = s.clone();
            }
        }
        for e in self.targets.iter_mut() {
            let tid = *e.read().uuid;
            if let Some(ArchiMateElement::Concept(t)) = all_models.get(&tid) {
                *e = t.clone();
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
                self.sources.insert(pos, c);
                Ok(pos.try_into().unwrap())
            }
            MULTICONNECTION_TARGET_BUCKET if let ArchiMateElement::Concept(c) = element => {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.targets.len());
                self.targets.insert(pos, c);
                Ok(pos.try_into().unwrap())
            }
            _ => Err(element),
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if self.sources.len() > 1 {
            for (idx, e) in self.sources.iter().enumerate() {
                if *e.read().uuid == *uuid {
                    self.sources.remove(idx);
                    return Some((MULTICONNECTION_SOURCE_BUCKET, idx.try_into().unwrap()));
                }
            }
        }
        if self.targets.len() > 1 {
            for (idx, e) in self.targets.iter().enumerate() {
                if *e.read().uuid == *uuid {
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
