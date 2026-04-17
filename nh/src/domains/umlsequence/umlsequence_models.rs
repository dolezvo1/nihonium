use std::{collections::{HashMap, HashSet}, sync::Arc};

use crate::common::{controller::{BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT, VisitableDiagram, VisitableElement}, entity::{Entity, EntityUuid}, eref::ERef, search::FullTextSearchable, uuid::ModelUuid};

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlSequenceElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlSequenceElement {
    #[container_model(passthrough = "eref")]
    Diagram(ERef<UmlSequenceDiagram>),
    #[container_model(passthrough = "eref")]
    CombinedFragment(ERef<UmlSequenceCombinedFragment>),
    #[container_model(passthrough = "eref")]
    CombinedFragmentSection(ERef<UmlSequenceCombinedFragmentSection>),
    Lifeline(ERef<UmlSequenceLifeline>),
    Message(ERef<UmlSequenceMessage>),
    Ref(ERef<UmlSequenceRef>),
    Comment(ERef<UmlSequenceComment>),
    CommentLink(ERef<UmlSequenceCommentLink>),
}

impl UmlSequenceElement {
    pub fn as_horizontal(&self) -> Option<UmlSequenceHorizontalElement> {
        match &self {
            UmlSequenceElement::CombinedFragment(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Message(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Ref(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Diagram(..)
            | UmlSequenceElement::CombinedFragmentSection(..)
            | UmlSequenceElement::Lifeline(..)
            | UmlSequenceElement::Comment(..)
            | UmlSequenceElement::CommentLink(..) => None,
        }
    }
}

impl VisitableElement for UmlSequenceElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            UmlSequenceElement::Diagram(inner) => {
                v.open_complex(self);
                for e in &inner.read().vertical_elements {
                    UmlSequenceElement::from(e.clone()).accept(v);
                }
                for e in &inner.read().horizontal_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            UmlSequenceElement::CombinedFragment(inner) => {
                v.open_complex(self);
                for e in &inner.read().sections {
                    UmlSequenceElement::from(e.clone()).accept(v);
                }
                v.close_complex(self);
            },
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                v.open_complex(self);
                for e in &inner.read().horizontal_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            },
            e => v.visit_simple(e),
        }
    }
}

impl FullTextSearchable for UmlSequenceElement {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        match self {
            UmlSequenceElement::Diagram(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::CombinedFragment(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::CombinedFragmentSection(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::Lifeline(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::Message(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::Ref(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::Comment(inner) => inner.read().full_text_search(acc),
            UmlSequenceElement::CommentLink(inner) => inner.read().full_text_search(acc),
        }
    }
}


pub fn deep_copy_diagram(d: &UmlSequenceDiagramBoard) -> (ERef<UmlSequenceDiagramBoard>, HashMap<ModelUuid, UmlSequenceElement>) {
    fn walk(e: &UmlSequenceElement, into: &mut HashMap<ModelUuid, UmlSequenceElement>) -> UmlSequenceElement {
        let new_uuid = ModelUuid::now_v7().into();
        match e {
            UmlSequenceElement::Diagram(inner) => {
                let model = inner.read();
                let new_model = UmlSequenceDiagram {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    vertical_elements: model.vertical_elements.iter().map(|e| {
                        let new_model = walk(&e.clone().into(), into);
                        if let UmlSequenceElement::Lifeline(new_model) = new_model {
                            into.insert(*e.read().uuid(), new_model.clone().into());
                            new_model
                        } else {
                            e.clone()
                        }
                    }).collect(),
                    horizontal_elements: model.horizontal_elements.iter().map(|e| {
                        let new_model = walk(&e.clone().to_element(), into);
                        if let Some(new_model) = new_model.as_horizontal() {
                            into.insert(*e.uuid(), new_model.clone().to_element());
                            new_model
                        } else {
                            e.clone()
                        }
                    }).collect(),
                    comment: model.comment.clone()
                };
                UmlSequenceElement::Diagram(ERef::new(new_model))
            },
            UmlSequenceElement::CombinedFragment(inner) => {
                let model = inner.read();
                let new_model = UmlSequenceCombinedFragment {
                    uuid: new_uuid,
                    kind: model.kind.clone(),
                    kind_argument: model.kind_argument.clone(),
                    horizontal_span: model.horizontal_span.clone(),
                    sections: model.sections.iter().map(|e| {
                        let new_model = walk(&e.clone().into(), into);
                        if let UmlSequenceElement::CombinedFragmentSection(new_model) = new_model {
                            into.insert(*e.read().uuid(), new_model.clone().into());
                            new_model
                        } else {
                            e.clone()
                        }
                    }).collect(),
                    comment: model.comment.clone()
                };
                UmlSequenceElement::CombinedFragment(ERef::new(new_model))
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let model = inner.read();
                let new_model = UmlSequenceCombinedFragmentSection {
                    uuid: new_uuid,
                    guard: model.guard.clone(),
                    horizontal_elements: model.horizontal_elements.iter().map(|e| {
                        let new_model = walk(&e.clone().to_element(), into);
                        if let Some(new_model) = new_model.as_horizontal() {
                            into.insert(*e.uuid(), new_model.clone().to_element());
                            new_model
                        } else {
                            e.clone()
                        }
                    }).collect(),
                };
                UmlSequenceElement::CombinedFragmentSection(ERef::new(new_model))
            },
            UmlSequenceElement::Lifeline(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
            UmlSequenceElement::Message(inner) => {
                inner.read().clone_with(*new_uuid).into()
            },
            UmlSequenceElement::Ref(inner) => inner.read().clone_with(*new_uuid).into(),
            UmlSequenceElement::Comment(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
            UmlSequenceElement::CommentLink(inner) => {
                inner.read().clone_with(*new_uuid).into()
            }
        }
    }

    fn relink(e: &mut UmlSequenceElement, all_models: &HashMap<ModelUuid, UmlSequenceElement>) {
        match e {
            UmlSequenceElement::Diagram(inner) => {
                let mut model = inner.write();
                for e in model.vertical_elements.iter_mut() {
                    relink(&mut e.clone().into(), all_models);
                }
                for e in model.horizontal_elements.iter_mut() {
                    relink(&mut e.clone().to_element(), all_models);
                }
            },
            UmlSequenceElement::CombinedFragment(inner) => {
                let mut model = inner.write();
                for e in model.sections.iter_mut() {
                    relink(&mut e.clone().into(), all_models);
                }
            },
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let mut model = inner.write();
                for e in model.horizontal_elements.iter_mut() {
                    relink(&mut e.clone().to_element(), all_models);
                }
            },
            UmlSequenceElement::Lifeline(..) => {},
            UmlSequenceElement::Message(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlSequenceElement::Lifeline(s)) = all_models.get(&source_uuid) {
                    model.source = s.clone();
                }
                let target_uuid = *model.target.read().uuid();
                if let Some(UmlSequenceElement::Lifeline(t)) = all_models.get(&target_uuid) {
                    model.target = t.clone();
                }
            },
            UmlSequenceElement::Ref(..) => {},
            UmlSequenceElement::Comment(..) => {},
            UmlSequenceElement::CommentLink(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.read().uuid();
                if let Some(UmlSequenceElement::Comment(s)) = all_models.get(&source_uuid) {
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
    let mut new_diagrams = Vec::new();
    for e in &d.diagrams {
        let new_model = walk(&e.clone().into(), &mut all_models);
        if let UmlSequenceElement::Diagram(new_model) = new_model {
            all_models.insert(*e.read().uuid(), new_model.clone().into());
            new_diagrams.push(new_model);
        } else {
            new_diagrams.push(e.clone());
        }
    }
    for e in new_diagrams.iter_mut() {
        relink(&mut e.clone().into(), &all_models);
    }

    let new_diagram = UmlSequenceDiagramBoard {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        diagrams: new_diagrams,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &UmlSequenceDiagramBoard) -> HashMap<ModelUuid, UmlSequenceElement> {
    fn walk(e: &UmlSequenceElement, into: &mut HashMap<ModelUuid, UmlSequenceElement>) {
        match e {
            UmlSequenceElement::Diagram(inner) => {
                let model = inner.read();

                for e in &model.vertical_elements {
                    walk(&e.clone().into(), into);
                    into.insert(*e.read().uuid(), e.clone().into());
                }
                for e in &model.horizontal_elements {
                    walk(&e.clone().to_element(), into);
                    into.insert(*e.uuid(), e.clone().to_element());
                }
            },
            UmlSequenceElement::CombinedFragment(inner) => {
                let model = inner.read();

                for e in &model.sections {
                    walk(&e.clone().into(), into);
                    into.insert(*e.read().uuid, e.clone().into());
                }
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let model = inner.read();

                for e in &model.horizontal_elements {
                    walk(&e.clone().to_element(), into);
                    into.insert(*e.uuid(), e.clone().to_element());
                }
            }
            _ => {},
        }
    }

    let mut all_models = HashMap::new();
    for e in &d.diagrams {
        walk(&e.clone().into(), &mut all_models);
        all_models.insert(*e.read().uuid(), e.clone().into());
    }

    all_models
}

pub fn transitive_closure(d: &UmlSequenceDiagramBoard, mut when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
    fn walk(e: &UmlSequenceElement, when_deleting: &mut HashSet<ModelUuid>) {
        match e {
            UmlSequenceElement::Diagram(inner) => {
                let r = inner.read();
                if when_deleting.contains(&r.uuid) {
                    enumerate(e, when_deleting);
                } else {
                    for e in &r.vertical_elements {
                        walk(&e.clone().into(), when_deleting);
                    }
                    for e in &r.horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting);
                    }
                }
            },
            UmlSequenceElement::CombinedFragment(inner) => {
                let r = inner.read();
                if when_deleting.contains(&r.uuid) {
                    enumerate(e, when_deleting);
                } else {
                    for e in &r.sections {
                        walk(&e.clone().into(), when_deleting);
                    }
                }
            },
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let r = inner.read();
                if when_deleting.contains(&r.uuid) {
                    enumerate(e, when_deleting);
                } else {
                    for e in &r.horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting);
                    }
                }
            },
            UmlSequenceElement::Lifeline(..)
            | UmlSequenceElement::Message(..)
            | UmlSequenceElement::Ref(..)
            | UmlSequenceElement::Comment(..)
            | UmlSequenceElement::CommentLink(..) => {},
        }
    }

    for e in &d.diagrams {
        walk(&e.clone().into(), &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(e: &UmlSequenceElement, when_deleting: &HashSet<ModelUuid>, also_delete: &mut HashSet<ModelUuid>) {
            match e {
                UmlSequenceElement::Diagram(inner) => {
                    for e in &inner.read().vertical_elements {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                    for e in &inner.read().horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                },
                UmlSequenceElement::CombinedFragment(inner) => {
                    for e in &inner.read().sections {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                },
                UmlSequenceElement::CombinedFragmentSection(inner) => {
                    for e in &inner.read().horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                },
                UmlSequenceElement::Lifeline(..) => {},
                UmlSequenceElement::Message(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid())
                            || when_deleting.contains(&r.target.read().uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
                UmlSequenceElement::Ref(..)
                | UmlSequenceElement::Comment(..) => {},
                UmlSequenceElement::CommentLink(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid)
                            || when_deleting.contains(&r.target.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
            }
        }
        for e in &d.diagrams {
            walk(&e.clone().into(), &when_deleting, &mut also_delete);
        }
        if also_delete.is_empty() {
            break;
        }
        when_deleting.extend(also_delete.drain());
    }

    when_deleting
}

fn enumerate(e: &UmlSequenceElement, into: &mut HashSet<ModelUuid>) {
    into.insert(*e.uuid());
    match e {
        UmlSequenceElement::Diagram(inner) => {
            let r = inner.read();
            for e in &r.vertical_elements {
                enumerate(&e.clone().into(), into);
            }
            for e in &r.horizontal_elements {
                enumerate(&e.clone().to_element(), into);
            }
        },
        UmlSequenceElement::CombinedFragment(inner) => {
            for s in &inner.read().sections {
                enumerate(&s.clone().into(), into);
            }
        }
        UmlSequenceElement::CombinedFragmentSection(inner) => {
            for e in &inner.read().horizontal_elements {
                enumerate(&e.clone().to_element(), into);
            }
        }
        UmlSequenceElement::Lifeline(..)
        | UmlSequenceElement::Message(..)
        | UmlSequenceElement::Ref(..)
        | UmlSequenceElement::Comment(..)
        | UmlSequenceElement::CommentLink(..) => {},
    }
}


#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlSequenceElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlSequenceHorizontalElement {
    #[container_model(passthrough = "eref")]
    CombinedFragment(ERef<UmlSequenceCombinedFragment>),
    Message(ERef<UmlSequenceMessage>),
    Ref(ERef<UmlSequenceRef>),
}

impl UmlSequenceHorizontalElement {
    pub fn to_element(self) -> UmlSequenceElement {
        match self {
            UmlSequenceHorizontalElement::CombinedFragment(inner) => inner.into(),
            UmlSequenceHorizontalElement::Message(inner) => inner.into(),
            UmlSequenceHorizontalElement::Ref(inner) => inner.into(),
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct UmlSequenceDiagramBoard {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    pub diagrams: Vec<ERef<UmlSequenceDiagram>>,

    pub comment: Arc<String>,
}

impl UmlSequenceDiagramBoard {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        diagrams: Vec<ERef<UmlSequenceDiagram>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            diagrams,
            comment: Arc::new("".to_owned()),
        }
    }

    pub fn get_element_pos_in(&self, parent: &ModelUuid, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        if *parent == *self.uuid {
            self.get_element_pos(uuid)
        } else {
            self.find_element(parent).and_then(|e| e.0.get_element_pos(uuid))
        }
    }

    pub fn insert_element_into(&mut self, parent: ModelUuid, element: UmlSequenceElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
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

    pub fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlSequenceElement, BucketNoT, PositionNoT)>) {
        fn r(e: &UmlSequenceElement, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlSequenceElement, BucketNoT, PositionNoT)>) {
            match e {
                UmlSequenceElement::Diagram(inner) => {
                    let mut w = inner.write();

                    for (idx, e) in w.vertical_elements.iter().enumerate() {
                        if uuids.contains(&e.read().uuid()) {
                            undo.push((*w.uuid, e.clone().into(), 0, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.vertical_elements.retain(|e| !uuids.contains(&e.read().uuid()));

                    for (idx, e) in w.horizontal_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((*w.uuid, e.clone().to_element(), 1, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.horizontal_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                UmlSequenceElement::CombinedFragment(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.sections.iter().enumerate() {
                        if uuids.contains(&e.read().uuid()) {
                            undo.push((*w.uuid, e.clone().into(), 1, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.sections.retain(|e| !uuids.contains(&e.read().uuid()));
                },
                UmlSequenceElement::CombinedFragmentSection(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.horizontal_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((*w.uuid, e.clone().to_element(), 1, idx.try_into().unwrap()));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.horizontal_elements.retain(|e| !uuids.contains(&e.uuid()));
                },
                UmlSequenceElement::Lifeline(..)
                | UmlSequenceElement::Message(..)
                | UmlSequenceElement::Ref(..)
                | UmlSequenceElement::Comment(..)
                | UmlSequenceElement::CommentLink(..) => {},
            }
        }


        for (idx, e) in self.diagrams.iter().enumerate() {
            if uuids.contains(&e.read().uuid()) {
                undo.push((*self.uuid, e.clone().into(), 0, idx.try_into().unwrap()));
            } else {
                r(&e.clone().into(), uuids, undo);
            }
        }
        self.diagrams.retain(|e| !uuids.contains(&e.read().uuid()));
    }
}

impl Entity for UmlSequenceDiagramBoard {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceDiagramBoard {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for UmlSequenceDiagramBoard {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.diagrams {
            UmlSequenceElement::from(e.clone()).accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for UmlSequenceDiagramBoard {
    type ElementT = UmlSequenceElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.diagrams {
            if *e.read().uuid == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
            if let Some(e) = e.read().find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.diagrams.iter().enumerate() {
            if *e.read().uuid == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        return None;
    }

    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: Self::ElementT) -> Result<PositionNoT, Self::ElementT> {
        if bucket != 0 {
            return Err(element);
        }
        let UmlSequenceElement::Diagram(element) = element else {
            return Err(element);
        };

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.diagrams.len());
        self.diagrams.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }

    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.diagrams.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.diagrams.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlSequenceDiagramBoard {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.comment,
            ],
        );

        for e in &self.diagrams {
            e.read().full_text_search(acc);
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    pub vertical_elements: Vec<ERef<UmlSequenceLifeline>>,
    #[nh_context_serde(entity)]
    pub horizontal_elements: Vec<UmlSequenceHorizontalElement>,

    pub comment: Arc<String>,
}

impl UmlSequenceDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        vertical_elements: Vec<ERef<UmlSequenceLifeline>>,
        horizontal_elements: Vec<UmlSequenceHorizontalElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            vertical_elements,
            horizontal_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            vertical_elements: self.vertical_elements.clone(),
            horizontal_elements: self.horizontal_elements.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlSequenceDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlSequenceDiagram {
    type ElementT = UmlSequenceElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlSequenceElement, ModelUuid)> {
        for e in &self.vertical_elements {
            if *e.read().uuid == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
        }
        for e in &self.horizontal_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.vertical_elements.iter().enumerate() {
            if *e.read().uuid == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((1, idx.try_into().unwrap()));
            }
        }
        return None;
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlSequenceElement) -> Result<PositionNoT, UmlSequenceElement> {
        match bucket {
            0 => {
                let UmlSequenceElement::Lifeline(element) = element else {
                    return Err(element);
                };
                let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.vertical_elements.len());
                self.vertical_elements.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            1 => {
                let Some(element) = element.clone().as_horizontal() else {
                    return Err(element);
                };
                let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.horizontal_elements.len());
                self.horizontal_elements.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            _ => return Err(element),
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.vertical_elements.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.vertical_elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.horizontal_elements.remove(idx);
                return Some((1, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlSequenceDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.comment,
            ],
        );

        for e in &self.vertical_elements {
            e.read().full_text_search(acc);
        }
        for e in &self.horizontal_elements {
            e.clone().to_element().full_text_search(acc);
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceLifeline {
    pub uuid: Arc<ModelUuid>,

    pub name: Arc<String>,
    pub stereotype: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlSequenceLifeline {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        stereotype: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            stereotype: Arc::new(stereotype),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            stereotype: self.stereotype.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlSequenceLifeline {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceLifeline {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for UmlSequenceLifeline {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.stereotype,
            ],
        );
    }
}


#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlSequenceMessageSynchronicityKind {
    #[default]
    Synchronous,
    AsynchronousCall,
    AsynchronousSignal,
}

impl UmlSequenceMessageSynchronicityKind {
    pub fn char(&self) -> &'static str {
        match self {
            Self::Synchronous => "Synchronous",
            Self::AsynchronousCall => "Asynchronous Call",
            Self::AsynchronousSignal => "Asynchronous Signal",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlSequenceMessageLifecycleKind {
    #[default]
    None,
    Create,
    Delete,
}

impl UmlSequenceMessageLifecycleKind {
    pub fn char(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Create => "Create",
            Self::Delete => "Delete",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceMessage {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub state_invariant: Arc<String>,

    pub synchronicity: UmlSequenceMessageSynchronicityKind,
    pub lifecycle: UmlSequenceMessageLifecycleKind,
    pub is_return: bool,

    #[nh_context_serde(entity)]
    pub source: ERef<UmlSequenceLifeline>,
    #[nh_context_serde(entity)]
    pub target: ERef<UmlSequenceLifeline>,

    pub comment: Arc<String>,
}

impl UmlSequenceMessage {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        state_invariant: String,
        synchronicity: UmlSequenceMessageSynchronicityKind,
        lifecycle: UmlSequenceMessageLifecycleKind,
        is_return: bool,
        source: ERef<UmlSequenceLifeline>,
        target: ERef<UmlSequenceLifeline>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            state_invariant: Arc::new(state_invariant),
            synchronicity,
            lifecycle,
            is_return,
            source,
            target,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            state_invariant: self.state_invariant.clone(),
            synchronicity: self.synchronicity.clone(),
            lifecycle: self.lifecycle.clone(),
            is_return: self.is_return.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.source, &mut self.target);
    }
}

impl Entity for UmlSequenceMessage {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceMessage {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for UmlSequenceMessage {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
            ],
        );
    }
}


#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlSequenceCombinedFragmentKind {
    #[default]
    Opt,
    Alt,
    Loop,
    Break,
    Par,
    Strict,
    Seq,
    Critical,
    Ignore,
    Consider,
    Assert,
    Neg,
}

impl UmlSequenceCombinedFragmentKind {
    pub fn char(&self) -> &'static str {
        match self {
            UmlSequenceCombinedFragmentKind::Opt => "opt",
            UmlSequenceCombinedFragmentKind::Alt => "alt",
            UmlSequenceCombinedFragmentKind::Loop => "loop",
            UmlSequenceCombinedFragmentKind::Break => "break",
            UmlSequenceCombinedFragmentKind::Par => "par",
            UmlSequenceCombinedFragmentKind::Strict => "strict",
            UmlSequenceCombinedFragmentKind::Seq => "seq",
            UmlSequenceCombinedFragmentKind::Critical => "critical",
            UmlSequenceCombinedFragmentKind::Ignore => "ignore",
            UmlSequenceCombinedFragmentKind::Consider => "consider",
            UmlSequenceCombinedFragmentKind::Assert => "assert",
            UmlSequenceCombinedFragmentKind::Neg => "neg",
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            UmlSequenceCombinedFragmentKind::Opt => "Opt",
            UmlSequenceCombinedFragmentKind::Alt => "Alt",
            UmlSequenceCombinedFragmentKind::Loop => "Loop",
            UmlSequenceCombinedFragmentKind::Break => "Break",
            UmlSequenceCombinedFragmentKind::Par => "Par",
            UmlSequenceCombinedFragmentKind::Strict => "Strict",
            UmlSequenceCombinedFragmentKind::Seq => "Seq",
            UmlSequenceCombinedFragmentKind::Critical => "Critical",
            UmlSequenceCombinedFragmentKind::Ignore => "Ignore",
            UmlSequenceCombinedFragmentKind::Consider => "Consider",
            UmlSequenceCombinedFragmentKind::Assert => "Assert",
            UmlSequenceCombinedFragmentKind::Neg => "Neg",
        }
    }
    pub fn max_allowed_sections_count(&self) -> Option<PositionNoT> {
        match self {
            UmlSequenceCombinedFragmentKind::Opt => Some(1),
            UmlSequenceCombinedFragmentKind::Alt => None,
            UmlSequenceCombinedFragmentKind::Loop => Some(1),
            UmlSequenceCombinedFragmentKind::Break => Some(1),
            UmlSequenceCombinedFragmentKind::Par => None,
            UmlSequenceCombinedFragmentKind::Strict => None,
            UmlSequenceCombinedFragmentKind::Seq => None,
            UmlSequenceCombinedFragmentKind::Critical => Some(1),
            UmlSequenceCombinedFragmentKind::Ignore => Some(1),
            UmlSequenceCombinedFragmentKind::Consider => Some(1),
            UmlSequenceCombinedFragmentKind::Assert => Some(1),
            UmlSequenceCombinedFragmentKind::Neg => Some(1),
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceCombinedFragment {
    pub uuid: Arc<ModelUuid>,
    pub kind: UmlSequenceCombinedFragmentKind,
    pub kind_argument: Arc<String>,

    pub horizontal_span: HashSet<ModelUuid>,
    #[nh_context_serde(entity)]
    pub sections: Vec<ERef<UmlSequenceCombinedFragmentSection>>,

    pub comment: Arc<String>,
}

impl UmlSequenceCombinedFragment {
    pub fn new(
        uuid: ModelUuid,
        kind: UmlSequenceCombinedFragmentKind,
        kind_argument: String,
        horizontal_span: HashSet<ModelUuid>,
        sections: Vec<ERef<UmlSequenceCombinedFragmentSection>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            kind_argument: Arc::new(kind_argument),
            horizontal_span,
            sections,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            kind: self.kind.clone(),
            kind_argument: self.kind_argument.clone(),
            horizontal_span: self.horizontal_span.clone(),
            sections: self.sections.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for UmlSequenceCombinedFragment {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceCombinedFragment {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlSequenceCombinedFragment {
    type ElementT = UmlSequenceElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlSequenceElement, ModelUuid)> {
        for e in &self.sections {
            if *e.read().uuid() == *uuid {
                return Some((e.clone().into(), *self.uuid));
            }
            if let Some(e) = e.read().find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.sections.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((1, idx.try_into().unwrap()));
            }
        }
        return None;
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlSequenceElement) -> Result<PositionNoT, UmlSequenceElement> {
        if bucket != 1 {
            return Err(element);
        }
        let UmlSequenceElement::CombinedFragmentSection(section) = element else {
            return Err(element)
        };

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.sections.len());
        self.sections.insert(pos, section);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.sections.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.sections.remove(idx);
                return Some((1, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlSequenceCombinedFragment {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                // &self.kind.char(),
                &self.comment,
            ],
        );

        for e in &self.sections {
            e.read().full_text_search(acc);
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceCombinedFragmentSection {
    pub uuid: Arc<ModelUuid>,
    pub guard: Arc<String>,

    #[nh_context_serde(entity)]
    pub horizontal_elements: Vec<UmlSequenceHorizontalElement>,
}

impl UmlSequenceCombinedFragmentSection {
    pub fn new(
        uuid: ModelUuid,
        guard: String,
        horizontal_elements: Vec<UmlSequenceHorizontalElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            guard: Arc::new(guard),
            horizontal_elements,
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            guard: self.guard.clone(),
            horizontal_elements: self.horizontal_elements.clone(),
        })
    }
}

impl Entity for UmlSequenceCombinedFragmentSection {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceCombinedFragmentSection {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for UmlSequenceCombinedFragmentSection {
    type ElementT = UmlSequenceElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(UmlSequenceElement, ModelUuid)> {
        for e in &self.horizontal_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        return None;
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((1, idx.try_into().unwrap()));
            }
        }
        return None;
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: UmlSequenceElement) -> Result<PositionNoT, UmlSequenceElement> {
        if bucket != 1 {
            return Err(element);
        }
        let Some(element) = element.as_horizontal() else {
            return Err(element)
        };

        let pos = position.map(|e| e.try_into().unwrap()).unwrap_or(self.horizontal_elements.len());
        self.horizontal_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.horizontal_elements.remove(idx);
                return Some((1, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlSequenceCombinedFragmentSection {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.guard,
            ],
        );

        for e in &self.horizontal_elements {
            e.clone().to_element().full_text_search(acc);
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceRef {
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
    pub horizontal_span: HashSet<ModelUuid>,
}

impl UmlSequenceRef {
    pub fn new(
        uuid: ModelUuid,
        text: String,
        horizontal_span: HashSet<ModelUuid>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
            horizontal_span,
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            text: self.text.clone(),
            horizontal_span: self.horizontal_span.clone(),
        })
    }
}

impl Entity for UmlSequenceRef {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceRef {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for UmlSequenceRef {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.text,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceComment {
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl UmlSequenceComment {
    pub fn new(
        uuid: ModelUuid,
        text: String,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            text: self.text.clone(),
        })
    }
}

impl Entity for UmlSequenceComment {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceComment {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for UmlSequenceComment {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.text,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceCommentLink {
    pub uuid: Arc<ModelUuid>,
    #[nh_context_serde(entity)]
    pub source: ERef<UmlSequenceComment>,
    #[nh_context_serde(entity)]
    pub target: UmlSequenceElement,
}

impl UmlSequenceCommentLink {
    pub fn new(
        uuid: ModelUuid,
        source: ERef<UmlSequenceComment>,
        target: UmlSequenceElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            source: self.source.clone(),
            target: self.target.clone(),
        })
    }
}

impl Entity for UmlSequenceCommentLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceCommentLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for UmlSequenceCommentLink {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
            ],
        );
    }
}
