use crate::common::controller::{BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT, VisitableDiagram, VisitableElement};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::search::FullTextSearchable;
use crate::common::ufoption::UFOption;
use crate::common::uuid::ModelUuid;
use crate::common::views::multiconnection_view::{MULTICONNECTION_SOURCE_BUCKET, MULTICONNECTION_TARGET_BUCKET};
use crate::domains::umlclass::umlclass_plantuml::UmlClassPlantUmlCollector;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    sync::Arc,
};

pub trait UmlClassVisitor {
    fn visit_package(&mut self, package: &UmlClassPackage);
    fn visit_instance(&mut self, instance: &UmlClassInstance);
    fn visit_class(&mut self, class: &UmlClass);
    fn visit_usecase(&mut self, usecase: &UmlUseCase);
    fn visit_generalization(&mut self, generalization: &UmlClassGeneralization);
    fn visit_dependency(&mut self, dependency: &UmlClassDependency);
    fn visit_association(&mut self, association: &UmlClassAssociation);
    fn visit_usecasegeneralization(&mut self, usecasegen: &UmlUseCaseGeneralization);
    fn visit_comment(&mut self, comment: &UmlClassComment);
    fn visit_commentlink(&mut self, commentlink: &UmlClassCommentLink);
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::FullTextSearchable, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlClassElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlClassElement {
    #[container_model(passthrough = "eref")]
    Package(ERef<UmlClassPackage>),
    Instance(ERef<UmlClassInstance>),
    #[container_model(passthrough = "eref")]
    Class(ERef<UmlClass>),
    Property(ERef<UmlClassProperty>),
    Operation(ERef<UmlClassOperation>),
    UseCase(ERef<UmlUseCase>),
    Generalization(ERef<UmlClassGeneralization>),
    Dependency(ERef<UmlClassDependency>),
    Association(ERef<UmlClassAssociation>),
    UseCaseGeneralization(ERef<UmlUseCaseGeneralization>),
    Comment(ERef<UmlClassComment>),
    CommentLink(ERef<UmlClassCommentLink>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlClassAssociable {
    Instance(ERef<UmlClassInstance>),
    Class(ERef<UmlClass>),
    UseCase(ERef<UmlUseCase>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlGeneralization {
    Generalization(ERef<UmlClassGeneralization>),
    UseCaseGeneralization(ERef<UmlUseCaseGeneralization>),
}

impl UmlClassElement {
    pub fn as_associable(&self) -> Option<UmlClassAssociable> {
        match self {
            UmlClassElement::Instance(inner) => Some(inner.clone().into()),
            UmlClassElement::Class(inner) => Some(inner.clone().into()),
            UmlClassElement::UseCase(inner) => Some(inner.clone().into()),
            UmlClassElement::Package(..)
            | UmlClassElement::Property(..)
            | UmlClassElement::Operation(..)
            | UmlClassElement::Generalization(..)
            | UmlClassElement::Dependency(..)
            | UmlClassElement::Association(..)
            | UmlClassElement::UseCaseGeneralization(..)
            | UmlClassElement::Comment(..)
            | UmlClassElement::CommentLink(..) => None,
        }
    }

    pub fn accept_uml(&self, visitor: &mut dyn UmlClassVisitor) {
        match self {
            UmlClassElement::Package(inner) => visitor.visit_package(&inner.read()),
            UmlClassElement::Instance(inner) => visitor.visit_instance(&inner.read()),
            UmlClassElement::Class(inner) => visitor.visit_class(&inner.read()),
            UmlClassElement::Property(..)
            | UmlClassElement::Operation(..) => unreachable!(),
            UmlClassElement::UseCase(inner) => visitor.visit_usecase(&inner.read()),
            UmlClassElement::Generalization(inner) => visitor.visit_generalization(&inner.read()),
            UmlClassElement::Dependency(inner) => visitor.visit_dependency(&inner.read()),
            UmlClassElement::Association(inner) => visitor.visit_association(&inner.read()),
            UmlClassElement::UseCaseGeneralization(inner) => visitor.visit_usecasegeneralization(&inner.read()),
            UmlClassElement::Comment(inner) => visitor.visit_comment(&inner.read()),
            UmlClassElement::CommentLink(inner) => visitor.visit_commentlink(&inner.read()),
        }
    }
}

impl VisitableElement for UmlClassElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            UmlClassElement::Package(inner) => {
                v.open_complex(self);
                for e in &inner.read().contained_elements {
                    e.accept(v);
                }
                v.close_complex(self);
            },
            UmlClassElement::Class(inner) => {
                v.open_complex(self);
                let r = inner.read();
                for e in &r.properties {
                    UmlClassElement::from(e.clone()).accept(v);
                }
                for e in &r.operations {
                    UmlClassElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            }
            e => v.visit_simple(e),
        }
    }
}


pub fn deep_copy_diagram(d: &UmlClassDiagram) -> (ERef<UmlClassDiagram>, HashMap<ModelUuid, UmlClassElement>) {
    fn walk(e: &UmlClassElement, into: &mut HashMap<ModelUuid, UmlClassElement>) -> UmlClassElement {
        let new_uuid = ModelUuid::now_v7().into();
        match e {
            UmlClassElement::Package(inner) => {
                let model = inner.read();

                let new_model = UmlClassPackage {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    stereotype: model.stereotype.clone(),
                    kind: model.kind.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                UmlClassElement::Package(ERef::new(new_model))
            },
            UmlClassElement::Instance(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Class(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Property(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Operation(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::UseCase(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Generalization(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Dependency(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Association(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::UseCaseGeneralization(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::Comment(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlClassElement::CommentLink(inner) => inner.read().clone_with(*new_uuid).into(),
        }
    }

    fn relink(e: &mut UmlClassElement, all_models: &HashMap<ModelUuid, UmlClassElement>) {
        match e {
            UmlClassElement::Package(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            },
            UmlClassElement::Instance(..)
            | UmlClassElement::Class(..)
            | UmlClassElement::Property(..)
            | UmlClassElement::Operation(..)
            | UmlClassElement::UseCase(..) => {},
            UmlClassElement::Generalization(inner) => {
                let mut model = inner.write();

                for e in model.sources.iter_mut() {
                    let sid = *e.read().uuid;
                    if let Some(UmlClassElement::Class(s)) = all_models.get(&sid) {
                        *e = s.clone();
                    }
                }
                for e in model.targets.iter_mut() {
                    let tid = *e.read().uuid;
                    if let Some(UmlClassElement::Class(t)) = all_models.get(&tid) {
                        *e = t.clone();
                    }
                }
            },
            UmlClassElement::Dependency(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.uuid();
                if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_associable()) {
                    model.source = s;
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_associable()) {
                    model.target = t;
                }
            }
            UmlClassElement::Association(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.uuid();
                if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_associable()) {
                    model.source = s;
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_associable()) {
                    model.target = t;
                }
            },
            UmlClassElement::UseCaseGeneralization(inner) => {
                let mut model = inner.write();

                for e in model.sources.iter_mut() {
                    let sid = *e.read().uuid;
                    if let Some(UmlClassElement::UseCase(s)) = all_models.get(&sid) {
                        *e = s.clone();
                    }
                }
                for e in model.targets.iter_mut() {
                    let tid = *e.read().uuid;
                    if let Some(UmlClassElement::UseCase(t)) = all_models.get(&tid) {
                        *e = t.clone();
                    }
                }
            },
            UmlClassElement::Comment(..) => {},
            UmlClassElement::CommentLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlClassElement::Comment(s)) = all_models.get(&source_uuid) {
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
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &UmlClassDiagram) -> HashMap<ModelUuid, UmlClassElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(e, &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &UmlClassElement, into: &mut HashMap<ModelUuid, UmlClassElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        UmlClassElement::Package(inner) => {
            let model = inner.read();

            for e in &model.contained_elements {
                enumerate_elements(e, into);
            }
        },
        UmlClassElement::Class(inner) => {
            let model = inner.read();

            for e in &model.properties {
                enumerate_elements(&e.clone().into(), into);
            }
            for e in &model.operations {
                enumerate_elements(&e.clone().into(), into);
            }
        }
        _ => {},
    }
}

pub fn transitive_closure(d: &UmlClassDiagram, mut when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &UmlClassElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                UmlClassElement::Package(inner) => {
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
                },
                UmlClassElement::Instance(..)
                | UmlClassElement::Property(..)
                | UmlClassElement::Operation(..)
                | UmlClassElement::UseCase(..) => {},
                UmlClassElement::Class(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        for e in &r.properties {
                            let mut c = Default::default();
                            enumerate_elements(&e.clone().into(), &mut c);
                            when_deleting.extend(c.into_keys());
                        }
                        for e in &r.operations {
                            let mut c = Default::default();
                            enumerate_elements(&e.clone().into(), &mut c);
                            when_deleting.extend(c.into_keys());
                        }
                    } else {
                        for e in &r.properties {
                            walk(&e.clone().into(), when_deleting);
                        }
                        for e in &r.operations {
                            walk(&e.clone().into(), when_deleting);
                        }
                    }
                },
                UmlClassElement::Generalization(..)
                | UmlClassElement::Dependency(..)
                | UmlClassElement::Association(..)
                | UmlClassElement::UseCaseGeneralization(..)
                | UmlClassElement::Comment(..)
                | UmlClassElement::CommentLink(..) => {},
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(e: &UmlClassElement, when_deleting: &HashSet<ModelUuid>, also_delete: &mut HashSet<ModelUuid>) {
            match e {
                UmlClassElement::Package(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                },
                UmlClassElement::Instance(..)
                | UmlClassElement::Class(..)
                | UmlClassElement::Property(..)
                | UmlClassElement::Operation(..)
                | UmlClassElement::UseCase(..) => {},
                UmlClassElement::Generalization(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (r.sources.iter().all(|e| when_deleting.contains(&e.read().uuid))
                            || r.targets.iter().all(|e| when_deleting.contains(&e.read().uuid))) {
                        also_delete.insert(*r.uuid);
                    }
                },
                UmlClassElement::Dependency(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
                UmlClassElement::Association(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
                UmlClassElement::UseCaseGeneralization(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (r.sources.iter().all(|e| when_deleting.contains(&e.read().uuid))
                            || r.targets.iter().all(|e| when_deleting.contains(&e.read().uuid))) {
                        also_delete.insert(*r.uuid);
                    }
                },
                UmlClassElement::Comment(..) => {},
                UmlClassElement::CommentLink(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid)
                            || when_deleting.contains(&r.target.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
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
        let mut collector = UmlClassPlantUmlCollector::new();

        for e in &self.contained_elements {
            e.accept_uml(&mut collector);
        }

        collector.finish()
    }

    pub fn get_element_pos_in(&self, parent: &ModelUuid, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if *parent == *self.uuid {
            self.get_element_pos(uuid)
        } else {
            self.find_element(parent).and_then(|e| e.0.get_element_pos(uuid))
        }
    }

    pub fn insert_element_into(&mut self, parent: ModelUuid, element: UmlClassElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
        if *self.uuid == parent {
            self.insert_element(b, p, element)
                .map(|_| ())
                .map_err(|_| ())
        } else {
            self.find_element(&parent)
                .ok_or(())
                .and_then(|mut e| e.0
                    .insert_element(b, p, element)
                    .map(|_| ())
                    .map_err(|_| ())
                )
        }
    }

    pub fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlClassElement, BucketNoT, PositionNoT)>) {
        fn r(e: &UmlClassElement, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlClassElement, BucketNoT, PositionNoT)>) {
            match e {
                UmlClassElement::Package(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((*w.uuid, e.clone(), 0, idx.try_into().unwrap()));
                        } else {
                            r(e, uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                },
                UmlClassElement::Instance(_)
                | UmlClassElement::Property(_)
                | UmlClassElement::Operation(_) => {},
                UmlClassElement::Class(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.properties.iter().enumerate() {
                        if uuids.contains(&e.read().uuid) {
                            undo.push((*w.uuid, e.clone().into(), UmlClass::PROPERTIES_BUCKET, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.properties.retain(|e| !uuids.contains(&e.read().uuid));
                    for (idx, e) in w.operations.iter().enumerate() {
                        if uuids.contains(&e.read().uuid) {
                            undo.push((*w.uuid, e.clone().into(), UmlClass::OPERATIONS_BUCKET, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.operations.retain(|e| !uuids.contains(&e.read().uuid));
                }
                UmlClassElement::UseCase(_)
                | UmlClassElement::Generalization(_)
                | UmlClassElement::Dependency(_)
                | UmlClassElement::Association(_)
                | UmlClassElement::UseCaseGeneralization(_)
                | UmlClassElement::Comment(_)
                | UmlClassElement::CommentLink(_) => {},
            }
        }

        for (idx, e) in self.contained_elements.iter().enumerate() {
            if uuids.contains(&e.uuid()) {
                undo.push((*self.uuid, e.clone(), 0, idx.try_into().unwrap()));
            } else {
                r(e, uuids, undo);
            }
        }
        self.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
    }
}

impl Entity for UmlClassDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
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
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        return None;
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlClassElement) -> Result<PositionNoT, UmlClassElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.contained_elements.len());
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

impl FullTextSearchable for UmlClassDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
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
pub enum UmlClassPackageKind {
    #[default]
    Package,
    Boundary,
}

impl UmlClassPackageKind {
    pub const VARIANTS: [Self; 2] = [Self::Package, Self::Boundary];

    pub fn as_str(&self) -> &'static str {
        match self {
            UmlClassPackageKind::Package => "Package",
            UmlClassPackageKind::Boundary => "Boundary",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassPackage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub stereotype: Arc<String>,
    pub kind: UmlClassPackageKind,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<UmlClassElement>,

    pub comment: Arc<String>,
}

impl UmlClassPackage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        stereotype: String,
        kind: UmlClassPackageKind,
        contained_elements: Vec<UmlClassElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            stereotype: Arc::new(stereotype),
            kind,
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            stereotype: self.stereotype.clone(),
            kind: self.kind.clone(),
            contained_elements: self.contained_elements.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlClassPackage {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
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
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.contained_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        return None;
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlClassElement) -> Result<PositionNoT, UmlClassElement> {
        if bucket != 0 {
            return Err(element);
        }

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.contained_elements.len());
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

impl FullTextSearchable for UmlClassPackage {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.stereotype,
                self.kind.as_str(),
                &self.comment,
            ],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassInstance {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub instance_name: Arc<String>,
    pub instance_type: Arc<String>,
    pub stereotype: Arc<String>,
    pub instance_slots: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlClassInstance {
    pub fn new(
        uuid: ModelUuid,
        instance_name: String,
        instance_type: String,
        stereotype: String,
        instance_slots: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            instance_name: Arc::new(instance_name),
            instance_type: Arc::new(instance_type),
            stereotype: Arc::new(stereotype),
            instance_slots: Arc::new(instance_slots),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            instance_name: self.instance_name.clone(),
            instance_type: self.instance_type.clone(),
            stereotype: self.stereotype.clone(),
            instance_slots: self.instance_slots.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlClassInstance {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassInstance {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlClassVisibilityKind {
    Public,
    Package,
    Protected,
    Private,
}

impl UmlClassVisibilityKind {
    pub fn as_char(&self) -> &'static str {
        match self {
            UmlClassVisibilityKind::Public => "+",
            UmlClassVisibilityKind::Package => "~",
            UmlClassVisibilityKind::Protected => "#",
            UmlClassVisibilityKind::Private => "-",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            UmlClassVisibilityKind::Public => "Public",
            UmlClassVisibilityKind::Package => "Package",
            UmlClassVisibilityKind::Protected => "Protected",
            UmlClassVisibilityKind::Private => "Private",
        }
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlClassMemberInheritanceKind {
    #[default]
    None,
    Inherited,
    Redefines(String),
}

#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassProperty {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub value_type: Arc<String>,
    pub multiplicity: Arc<String>,
    pub default_value: Arc<String>,
    pub stereotype: Arc<String>,

    #[full_text_searchable(skip)]
    pub visibility: UFOption<UmlClassVisibilityKind>,
    #[full_text_searchable(skip)]
    pub inherited: UmlClassMemberInheritanceKind,
    #[full_text_searchable(skip)]
    pub is_static: bool,
    #[full_text_searchable(skip)]
    pub is_derived: bool,
    #[full_text_searchable(skip)]
    pub is_read_only: bool,
    #[full_text_searchable(skip)]
    pub is_ordered: bool,
    #[full_text_searchable(skip)]
    pub is_unique: bool,
    #[full_text_searchable(skip)]
    pub is_id: bool,
}

impl UmlClassProperty {
    pub fn new(
        uuid: ModelUuid,
        visibility: UFOption<UmlClassVisibilityKind>,
        name: String,
        value_type: String,
        multiplicity: String,
        default_value: String,
        stereotype: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            visibility,
            name: Arc::new(name),
            value_type: Arc::new(value_type),
            multiplicity: Arc::new(multiplicity),
            default_value: Arc::new(default_value),
            stereotype: Arc::new(stereotype),

            inherited: UmlClassMemberInheritanceKind::None,
            is_static: false,
            is_derived: false,
            is_read_only: false,
            is_ordered: false,
            is_unique: false,
            is_id: false,
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            value_type: self.value_type.clone(),
            multiplicity: self.multiplicity.clone(),
            default_value: self.default_value.clone(),
            stereotype: self.stereotype.clone(),

            visibility: self.visibility.clone(),
            inherited: self.inherited.clone(),
            is_static: self.is_static,
            is_derived: self.is_derived,
            is_read_only: self.is_read_only,
            is_ordered: self.is_ordered,
            is_unique: self.is_unique,
            is_id: self.is_id,
        })
    }
}

impl Entity for UmlClassProperty {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassProperty {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassOperation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub parameters: Arc<String>,
    pub return_type: Arc<String>,
    pub stereotype: Arc<String>,

    #[full_text_searchable(skip)]
    pub visibility: UFOption<UmlClassVisibilityKind>,
    #[full_text_searchable(skip)]
    pub inherited: UmlClassMemberInheritanceKind,
    #[full_text_searchable(skip)]
    pub is_static: bool,
    #[full_text_searchable(skip)]
    pub is_abstract: bool,
    #[full_text_searchable(skip)]
    pub is_query: bool,
    #[full_text_searchable(skip)]
    pub is_ordered: bool,
    #[full_text_searchable(skip)]
    pub is_unique: bool,
}

impl UmlClassOperation {
    pub fn new(
        uuid: ModelUuid,
        visibility: UFOption<UmlClassVisibilityKind>,
        name: String,
        parameters: String,
        return_type: String,
        stereotype: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            visibility,
            name: Arc::new(name),
            parameters: Arc::new(parameters),
            return_type: Arc::new(return_type),
            stereotype: Arc::new(stereotype),

            inherited: UmlClassMemberInheritanceKind::None,
            is_static: false,
            is_abstract: false,
            is_query: false,
            is_ordered: false,
            is_unique: false,
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            stereotype: self.stereotype.clone(),

            visibility: self.visibility.clone(),
            inherited: self.inherited.clone(),
            is_static: self.is_static,
            is_abstract: self.is_abstract,
            is_query: self.is_query,
            is_ordered: self.is_ordered,
            is_unique: self.is_unique,
        })
    }
}

impl Entity for UmlClassOperation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassOperation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClass {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub stereotype: Arc<String>,
    pub template_parameters: Arc<String>,
    pub is_abstract: bool,
    #[nh_context_serde(entity)]
    pub properties: Vec<ERef<UmlClassProperty>>,
    #[nh_context_serde(entity)]
    pub operations: Vec<ERef<UmlClassOperation>>,

    pub comment: Arc<String>,
}

impl UmlClass {
    pub const PROPERTIES_BUCKET: BucketNoT = 1;
    pub const OPERATIONS_BUCKET: BucketNoT = 2;

    pub fn new(
        uuid: ModelUuid,
        name: String,
        stereotype: String,
        template_parameters: String,
        is_abstract: bool,
        properties: Vec<ERef<UmlClassProperty>>,
        operations: Vec<ERef<UmlClassOperation>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            stereotype: Arc::new(stereotype),
            template_parameters: Arc::new(template_parameters),
            is_abstract,
            properties,
            operations,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            stereotype: self.stereotype.clone(),
            template_parameters: self.template_parameters.clone(),
            is_abstract: self.is_abstract,
            properties: self.properties.clone(),
            operations: self.operations.clone(),
            comment: self.comment.clone(),
        })
    }
    pub fn move_element(&mut self, element: &ModelUuid, within: BucketNoT, target_pos: PositionNoT) {
        if within == Self::PROPERTIES_BUCKET {
            if let Some((idx, _e)) = self.properties.iter().enumerate().find(|e| *e.1.read().uuid() == *element) {
                let e = self.properties.remove(idx);
                self.properties.insert(target_pos.try_into().unwrap(), e);
            }
        } else if within == Self::OPERATIONS_BUCKET {
            if let Some((idx, _e)) = self.operations.iter().enumerate().find(|e| *e.1.read().uuid() == *element) {
                let e = self.operations.remove(idx);
                self.operations.insert(target_pos.try_into().unwrap(), e);
            }
        }
    }
}

impl Entity for UmlClass {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClass {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlClass {
    type ElementT = UmlClassElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlClassElement, ModelUuid)> {
        for e in &self.properties {
            if *e.read().uuid == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        for e in &self.operations {
            if *e.read().uuid == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.properties.iter().enumerate() {
            if *e.read().uuid == *uuid {
                return Some((Self::PROPERTIES_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.operations.iter().enumerate() {
            if *e.read().uuid == *uuid {
                return Some((Self::OPERATIONS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlClassElement) -> Result<PositionNoT, UmlClassElement> {
        if (bucket == 0 || bucket == Self::PROPERTIES_BUCKET)
            && let UmlClassElement::Property(p) = element {
            let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.properties.len());
            self.properties.insert(pos, p);
            Ok(pos.try_into().unwrap())
        } else if (bucket == 0 || bucket == Self::OPERATIONS_BUCKET)
            && let UmlClassElement::Operation(o) = element {
            let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.operations.len());
            self.operations.insert(pos, o);
            Ok(pos.try_into().unwrap())
        } else {
            Err(element)
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.properties.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.properties.remove(idx);
                return Some((Self::PROPERTIES_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.operations.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.operations.remove(idx);
                return Some((Self::OPERATIONS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlClass {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.stereotype,
                &self.template_parameters,
                &self.comment,
            ],
        );

        for e in &self.properties {
            e.read().full_text_search(acc);
        }
        for e in &self.operations {
            e.read().full_text_search(acc);
        }
    }
}


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlUseCase {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub stereotype: Arc<String>,
    #[full_text_searchable(skip)]
    pub is_abstract: bool,

    pub comment: Arc<String>,
}

impl UmlUseCase {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        stereotype: String,
        is_abstract: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            stereotype: Arc::new(stereotype),
            is_abstract,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            stereotype: self.stereotype.clone(),
            is_abstract: self.is_abstract,
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlUseCase {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlUseCase {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassGeneralization {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub sources: Vec<ERef<UmlClass>>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub targets: Vec<ERef<UmlClass>>,

    pub set_name: Arc<String>,
    #[full_text_searchable(skip)]
    pub set_is_covering: bool,
    #[full_text_searchable(skip)]
    pub set_is_disjoint: bool,

    pub comment: Arc<String>,
}

impl UmlClassGeneralization {
    pub fn new(
        uuid: ModelUuid,
        set_name: String,
        sources: Vec<ERef<UmlClass>>,
        targets: Vec<ERef<UmlClass>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            sources,
            targets,

            set_name: Arc::new(set_name),
            set_is_covering: false,
            set_is_disjoint: false,

            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            sources: self.sources.clone(),
            targets: self.targets.clone(),

            set_name: self.set_name.clone(),
            set_is_covering: self.set_is_covering,
            set_is_disjoint: self.set_is_disjoint,

            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.sources, &mut self.targets);
    }
}

impl Entity for UmlClassGeneralization {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassGeneralization {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlClassGeneralization {
    type ElementT = UmlClassElement;

    fn find_element(&self, _uuid: &ModelUuid) -> Option<(UmlClassElement, ModelUuid)> {
        None
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlClassElement) -> Result<PositionNoT, UmlClassElement> {
        match bucket {
            MULTICONNECTION_SOURCE_BUCKET if let UmlClassElement::Class(c) = element => {
                let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.sources.len());
                self.sources.insert(pos, c);
                Ok(pos.try_into().unwrap())
            }
            MULTICONNECTION_TARGET_BUCKET if let UmlClassElement::Class(c) = element => {
                let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.targets.len());
                self.targets.insert(pos, c);
                Ok(pos.try_into().unwrap())
            }
            _ => {
                Err(element)
            }
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassDependency {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: UmlClassAssociable,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: UmlClassAssociable,
    #[full_text_searchable(skip)]
    pub target_arrow_open: bool,

    pub comment: Arc<String>,
}

impl UmlClassDependency {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        source: UmlClassAssociable,
        target: UmlClassAssociable,
        target_arrow_open: bool,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            source,
            target,
            target_arrow_open,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            target_arrow_open: self.target_arrow_open,
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.source, &mut self.target);
    }
}

impl Entity for UmlClassDependency {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassDependency {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlClassAssociationNavigability {
    #[default]
    Unspecified,
    Navigable,
    NonNavigable,
}

impl UmlClassAssociationNavigability {
    pub fn name(&self) -> &'static str {
        match self {
            UmlClassAssociationNavigability::Unspecified => "Unspecified",
            UmlClassAssociationNavigability::Navigable => "Navigable",
            UmlClassAssociationNavigability::NonNavigable => "Non-navigable",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlClassAssociationAggregation {
    #[default]
    None,
    Shared,
    Composite,
}

impl UmlClassAssociationAggregation {
    pub fn name(&self) -> &'static str {
        match self {
            UmlClassAssociationAggregation::None => "None",
            UmlClassAssociationAggregation::Shared => "Shared",
            UmlClassAssociationAggregation::Composite => "Composite",
        }
    }
}

#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassAssociation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub name: Arc<String>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: UmlClassAssociable,
    pub source_label_multiplicity: Arc<String>,
    pub source_label_role: Arc<String>,
    pub source_label_reading: Arc<String>,
    #[full_text_searchable(skip)]
    pub source_navigability: UmlClassAssociationNavigability,
    #[full_text_searchable(skip)]
    pub source_aggregation: UmlClassAssociationAggregation,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: UmlClassAssociable,
    pub target_label_multiplicity: Arc<String>,
    pub target_label_role: Arc<String>,
    pub target_label_reading: Arc<String>,
    #[full_text_searchable(skip)]
    pub target_navigability: UmlClassAssociationNavigability,
    #[full_text_searchable(skip)]
    pub target_aggregation: UmlClassAssociationAggregation,

    pub comment: Arc<String>,
}

impl UmlClassAssociation {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        name: String,
        source: UmlClassAssociable,
        source_label_multiplicity: String,
        target: UmlClassAssociable,
        target_label_multiplicity: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            name: Arc::new(name),
            source,
            source_label_multiplicity: Arc::new(source_label_multiplicity),
            source_label_role: Arc::new("".to_owned()),
            source_label_reading: Arc::new("".to_owned()),
            source_navigability: UmlClassAssociationNavigability::Unspecified,
            source_aggregation: UmlClassAssociationAggregation::None,
            target,
            target_label_multiplicity: Arc::new(target_label_multiplicity),
            target_label_role: Arc::new("".to_owned()),
            target_label_reading: Arc::new("".to_owned()),
            target_navigability: UmlClassAssociationNavigability::Unspecified,
            target_aggregation: UmlClassAssociationAggregation::None,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            name: self.name.clone(),
            source: self.source.clone(),
            source_label_multiplicity: self.source_label_multiplicity.clone(),
            source_label_role: self.source_label_role.clone(),
            source_label_reading: self.source_label_reading.clone(),
            source_navigability: self.source_navigability,
            source_aggregation: self.source_aggregation,
            target: self.target.clone(),
            target_label_multiplicity: self.target_label_multiplicity.clone(),
            target_label_role: self.target_label_role.clone(),
            target_label_reading: self.target_label_reading.clone(),
            target_navigability: self.target_navigability,
            target_aggregation: self.target_aggregation,
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.source, &mut self.target);
    }
}

impl Entity for UmlClassAssociation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassAssociation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlUseCaseGeneralization {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub sources: Vec<ERef<UmlUseCase>>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub targets: Vec<ERef<UmlUseCase>>,

    pub set_name: Arc<String>,
    #[full_text_searchable(skip)]
    pub set_is_covering: bool,
    #[full_text_searchable(skip)]
    pub set_is_disjoint: bool,

    pub comment: Arc<String>,
}

impl UmlUseCaseGeneralization {
    pub fn new(
        uuid: ModelUuid,
        sources: Vec<ERef<UmlUseCase>>,
        targets: Vec<ERef<UmlUseCase>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            sources,
            targets,

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
            targets: self.targets.clone(),

            set_name: self.set_name.clone(),
            set_is_covering: self.set_is_covering,
            set_is_disjoint: self.set_is_disjoint,

            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.sources, &mut self.targets);
    }
}

impl Entity for UmlUseCaseGeneralization {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlUseCaseGeneralization {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlUseCaseGeneralization {
    type ElementT = UmlClassElement;

    fn find_element(&self, _uuid: &ModelUuid) -> Option<(UmlClassElement, ModelUuid)> {
        None
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlClassElement) -> Result<PositionNoT, UmlClassElement> {
        match bucket {
            MULTICONNECTION_SOURCE_BUCKET if let UmlClassElement::UseCase(c) = element => {
                let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.sources.len());
                self.sources.insert(pos, c);
                Ok(pos.try_into().unwrap())
            }
            MULTICONNECTION_TARGET_BUCKET if let UmlClassElement::UseCase(c) = element => {
                let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.targets.len());
                self.targets.insert(pos, c);
                Ok(pos.try_into().unwrap())
            }
            _ => {
                Err(element)
            }
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


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassComment {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub stereotype: Arc<String>,
    pub text: Arc<String>,
}

impl UmlClassComment {
    pub fn new(
        uuid: ModelUuid,
        stereotype: String,
        text: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            stereotype: Arc::new(stereotype),
            text: Arc::new(text),
        }
    }
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            stereotype: self.stereotype.clone(),
            text: self.text.clone(),
        })
    }
}

impl Entity for UmlClassComment {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassComment {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}


#[derive(nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassCommentLink {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<UmlClassComment>,
    #[full_text_searchable(skip)]
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
    pub fn clone_with(&self, uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(uuid),
            source: self.source.clone(),
            target: self.target.clone(),
        })
    }
}

impl Entity for UmlClassCommentLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlClassCommentLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
