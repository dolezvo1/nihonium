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
#[container_model(element_type = UmlSequenceElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
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
    DurationConstraint(ERef<UmlSequenceDurationConstraint>),
    Note(ERef<UmlSequenceNote>),
    NoteLink(ERef<UmlSequenceNoteLink>),
}

impl UmlSequenceElement {
    pub fn as_standalone(&self) -> Option<UmlSequenceStandaloneElement> {
        match &self {
            UmlSequenceElement::Diagram(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Note(inner) => Some(inner.clone().into()),
            UmlSequenceElement::DurationConstraint(inner) => Some(inner.clone().into()),
            UmlSequenceElement::CombinedFragment(..)
            | UmlSequenceElement::CombinedFragmentSection(..)
            | UmlSequenceElement::Lifeline(..)
            | UmlSequenceElement::Message(..)
            | UmlSequenceElement::Ref(..)
            | UmlSequenceElement::NoteLink(..) => None,
        }
    }

    pub fn as_nondiagram_standalone(&self) -> Option<UmlSequenceNonDiagramStandaloneElement> {
        match &self {
            UmlSequenceElement::Note(inner) => Some(inner.clone().into()),
            UmlSequenceElement::DurationConstraint(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Diagram(..)
            | UmlSequenceElement::CombinedFragment(..)
            | UmlSequenceElement::CombinedFragmentSection(..)
            | UmlSequenceElement::Lifeline(..)
            | UmlSequenceElement::Message(..)
            | UmlSequenceElement::Ref(..)
            | UmlSequenceElement::NoteLink(..) => None,
        }
    }

    pub fn as_horizontal(&self) -> Option<UmlSequenceHorizontalElement> {
        match &self {
            UmlSequenceElement::CombinedFragment(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Message(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Ref(inner) => Some(inner.clone().into()),
            UmlSequenceElement::Diagram(..)
            | UmlSequenceElement::CombinedFragmentSection(..)
            | UmlSequenceElement::Lifeline(..)
            | UmlSequenceElement::DurationConstraint(..)
            | UmlSequenceElement::Note(..)
            | UmlSequenceElement::NoteLink(..) => None,
        }
    }

    pub fn deep_copy_relink(&self, all_models: &HashMap<ModelUuid, UmlSequenceElement>) {
        match self {
            Self::Diagram(..)
            | Self::CombinedFragment(..)
            | Self::CombinedFragmentSection(..)
            | Self::Lifeline(..) => {}
            Self::Message(inner) => inner.write().deep_copy_relink(all_models),
            Self::Ref(..) => {}
            Self::DurationConstraint(inner) => inner.write().deep_copy_relink(all_models),
            Self::Note(..) => {}
            Self::NoteLink(inner) => inner.write().deep_copy_relink(all_models),
        }
    }
}

impl VisitableElement for UmlSequenceElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            UmlSequenceElement::Diagram(inner) => {
                v.open_complex(self);
                for e in &inner.read().vertical_elements {
                    UmlSequenceElement::from(e.clone()).accept(v);
                }
                for e in &inner.read().horizontal_elements {
                    e.clone().to_element().accept(v);
                }
                for e in &inner.read().standalone_elements {
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
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                v.open_complex(self);
                for e in &inner.read().horizontal_elements {
                    e.clone().to_element().accept(v);
                }
                v.close_complex(self);
            }
            e => v.visit_simple(e),
        }
    }
}

pub fn deep_copy_diagram(
    d: &UmlSequenceDiagramBoard,
) -> (
    ERef<UmlSequenceDiagramBoard>,
    HashMap<ModelUuid, UmlSequenceElement>,
) {
    let mut all_models = HashMap::new();
    let mut new_elements = Vec::new();
    for e in &d.elements {
        new_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = UmlSequenceDiagramBoard {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        elements: new_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &UmlSequenceDiagramBoard) -> HashMap<ModelUuid, UmlSequenceElement> {
    let mut all_models = HashMap::new();
    for e in &d.elements {
        enumerate_elements(&e.clone().to_element(), &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &UmlSequenceElement, into: &mut HashMap<ModelUuid, UmlSequenceElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        UmlSequenceElement::Diagram(inner) => {
            let r = inner.read();
            for e in &r.vertical_elements {
                enumerate_elements(&e.clone().into(), into);
            }
            for e in &r.horizontal_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
            for e in &r.standalone_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        UmlSequenceElement::CombinedFragment(inner) => {
            for s in &inner.read().sections {
                enumerate_elements(&s.clone().into(), into);
            }
        }
        UmlSequenceElement::CombinedFragmentSection(inner) => {
            for e in &inner.read().horizontal_elements {
                enumerate_elements(&e.clone().to_element(), into);
            }
        }
        UmlSequenceElement::Lifeline(..)
        | UmlSequenceElement::Message(..)
        | UmlSequenceElement::Ref(..)
        | UmlSequenceElement::DurationConstraint(..)
        | UmlSequenceElement::Note(..)
        | UmlSequenceElement::NoteLink(..) => {}
    }
}

pub fn transitive_closure(
    d: &UmlSequenceDiagramBoard,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    fn walk(e: &UmlSequenceElement, when_deleting: &mut HashSet<ModelUuid>) {
        match e {
            UmlSequenceElement::Diagram(inner) => {
                let r = inner.read();
                if when_deleting.contains(&r.uuid) {
                    let mut c = Default::default();
                    enumerate_elements(e, &mut c);
                    when_deleting.extend(c.into_keys());
                } else {
                    for e in &r.vertical_elements {
                        walk(&e.clone().into(), when_deleting);
                    }
                    for e in &r.horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting);
                    }
                    for e in &r.standalone_elements {
                        walk(&e.clone().to_element(), when_deleting);
                    }
                }
            }
            UmlSequenceElement::CombinedFragment(inner) => {
                let r = inner.read();
                if when_deleting.contains(&r.uuid) {
                    let mut c = Default::default();
                    enumerate_elements(e, &mut c);
                    when_deleting.extend(c.into_keys());
                } else {
                    for e in &r.sections {
                        walk(&e.clone().into(), when_deleting);
                    }
                }
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let r = inner.read();
                if when_deleting.contains(&r.uuid) {
                    let mut c = Default::default();
                    enumerate_elements(e, &mut c);
                    when_deleting.extend(c.into_keys());
                } else {
                    for e in &r.horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting);
                    }
                }
            }
            UmlSequenceElement::Lifeline(..)
            | UmlSequenceElement::Message(..)
            | UmlSequenceElement::Ref(..)
            | UmlSequenceElement::DurationConstraint(..)
            | UmlSequenceElement::Note(..)
            | UmlSequenceElement::NoteLink(..) => {}
        }
    }

    for e in &d.elements {
        walk(&e.clone().to_element(), &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &UmlSequenceElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                UmlSequenceElement::Diagram(inner) => {
                    let r = inner.read();
                    for e in &r.vertical_elements {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                    for e in &r.horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                    for e in &r.standalone_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlSequenceElement::CombinedFragment(inner) => {
                    for e in &inner.read().sections {
                        walk(&e.clone().into(), when_deleting, also_delete);
                    }
                }
                UmlSequenceElement::CombinedFragmentSection(inner) => {
                    for e in &inner.read().horizontal_elements {
                        walk(&e.clone().to_element(), when_deleting, also_delete);
                    }
                }
                UmlSequenceElement::Lifeline(..) => {}
                UmlSequenceElement::Message(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid())
                            || when_deleting.contains(&r.target.read().uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                UmlSequenceElement::Ref(..) => {}
                UmlSequenceElement::DurationConstraint(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.element.uuid())
                            || when_deleting.contains(&r.target.element.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                UmlSequenceElement::Note(..) => {}
                UmlSequenceElement::NoteLink(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.read().uuid)
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
            }
        }
        for e in &d.elements {
            walk(&e.clone().to_element(), &when_deleting, &mut also_delete);
        }
        if also_delete.is_empty() {
            break;
        }
        when_deleting.extend(also_delete.drain());
    }

    when_deleting
}

pub fn top_sort_info(m: &UmlSequenceElement) -> ModelTopSortInfo {
    fn walk(
        e: &UmlSequenceElement,
        required_models: &mut HashSet<ModelUuid>,
        provided_models: &mut HashSet<ModelUuid>,
    ) {
        provided_models.insert(*e.uuid());
        match e {
            UmlSequenceElement::Diagram(inner) => {
                let r = inner.read();
                for e in &r.vertical_elements {
                    walk(&e.clone().into(), required_models, provided_models);
                }
                for e in &r.horizontal_elements {
                    walk(&e.clone().to_element(), required_models, provided_models);
                }
                for e in &r.standalone_elements {
                    walk(&e.clone().to_element(), required_models, provided_models);
                }
            }
            UmlSequenceElement::CombinedFragment(inner) => {
                for e in &inner.read().sections {
                    walk(&e.clone().into(), required_models, provided_models);
                }
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                for e in &inner.read().horizontal_elements {
                    walk(&e.clone().to_element(), required_models, provided_models);
                }
            }
            UmlSequenceElement::Lifeline(_) => {}
            UmlSequenceElement::Message(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.read().uuid);
                required_models.insert(*r.target.read().uuid);
            }
            UmlSequenceElement::Ref(_) => {}
            UmlSequenceElement::DurationConstraint(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.element.uuid());
                required_models.insert(*r.target.element.uuid());
            }
            UmlSequenceElement::Note(_) => {}
            UmlSequenceElement::NoteLink(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.read().uuid);
                required_models.insert(*r.target.uuid());
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

pub const VERTICALS_BUCKET: BucketNoT = 1;
pub const HORIZONTALS_BUCKET: BucketNoT = 2;
pub const NONDIAGRAM_STANDALONE_BUCKET: BucketNoT = 3;

#[derive(
    Clone,
    derive_more::From,
    nh_derive::Model,
    nh_derive::ContainerModel,
    nh_derive::FullTextSearchable,
    nh_derive::NHContextSerDeTag,
)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlSequenceElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlSequenceStandaloneElement {
    #[container_model(passthrough = "eref")]
    Diagram(ERef<UmlSequenceDiagram>),
    DurationConstraint(ERef<UmlSequenceDurationConstraint>),
    Note(ERef<UmlSequenceNote>),
}

impl UmlSequenceStandaloneElement {
    pub fn to_element(self) -> UmlSequenceElement {
        match self {
            UmlSequenceStandaloneElement::Diagram(inner) => inner.into(),
            UmlSequenceStandaloneElement::DurationConstraint(inner) => inner.into(),
            UmlSequenceStandaloneElement::Note(inner) => inner.into(),
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> Self {
        match self {
            Self::Diagram(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::DurationConstraint(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
}

#[derive(
    Clone,
    derive_more::From,
    nh_derive::Model,
    nh_derive::ContainerModel,
    nh_derive::FullTextSearchable,
    nh_derive::NHContextSerDeTag,
)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = UmlSequenceElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum UmlSequenceNonDiagramStandaloneElement {
    Note(ERef<UmlSequenceNote>),
    DurationConstraint(ERef<UmlSequenceDurationConstraint>),
}

impl UmlSequenceNonDiagramStandaloneElement {
    pub fn to_element(self) -> UmlSequenceElement {
        match self {
            UmlSequenceNonDiagramStandaloneElement::Note(inner) => inner.into(),
            UmlSequenceNonDiagramStandaloneElement::DurationConstraint(inner) => inner.into(),
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> Self {
        match self {
            Self::DurationConstraint(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
}

#[derive(
    Clone,
    derive_more::From,
    nh_derive::Model,
    nh_derive::ContainerModel,
    nh_derive::NHContextSerDeTag,
)]
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
    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> Self {
        match self {
            Self::CombinedFragment(inner) => {
                inner.read().deep_copy_clone_inner(new_uuid, into).into()
            }
            Self::Message(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Ref(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct UmlSequenceDiagramBoard {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    pub elements: Vec<UmlSequenceStandaloneElement>,

    pub comment: Arc<String>,
}

impl UmlSequenceDiagramBoard {
    pub fn new(uuid: ModelUuid, name: String, elements: Vec<UmlSequenceStandaloneElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            elements,
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
        element: UmlSequenceElement,
    ) -> Result<PositionNoT, UmlSequenceElement> {
        if bucket != 0 {
            return Err(element);
        }
        let Some(element) = element.as_standalone() else {
            return Err(element);
        };

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.elements.len());
        self.elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element_safe(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.elements.remove(idx);
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }

    pub fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, UmlSequenceElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &UmlSequenceElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, UmlSequenceElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                UmlSequenceElement::Diagram(inner) => {
                    let mut w = inner.write();

                    for (idx, e) in w.vertical_elements.iter().enumerate() {
                        if uuids.contains(&e.read().uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().into(),
                                VERTICALS_BUCKET,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().into(), uuids, undo);
                        }
                    }
                    w.vertical_elements
                        .retain(|e| !uuids.contains(&e.read().uuid()));

                    for (idx, e) in w.horizontal_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().to_element(),
                                HORIZONTALS_BUCKET,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.horizontal_elements.retain(|e| !uuids.contains(&e.uuid()));

                    for (idx, e) in w.standalone_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().to_element(),
                                NONDIAGRAM_STANDALONE_BUCKET,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.standalone_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                UmlSequenceElement::CombinedFragment(inner) => {
                    let mut w = inner.write();
                    if w.sections.iter().any(|e| !uuids.contains(&e.read().uuid)) {
                        for (idx, e) in w.sections.iter().enumerate() {
                            if uuids.contains(&e.read().uuid()) {
                                undo.push((
                                    *w.uuid,
                                    e.clone().into(),
                                    HORIZONTALS_BUCKET,
                                    idx.try_into().unwrap(),
                                ));
                            } else {
                                r(&e.clone().into(), uuids, undo);
                            }
                        }
                        w.sections.retain(|e| !uuids.contains(&e.read().uuid()));
                    }
                }
                UmlSequenceElement::CombinedFragmentSection(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.horizontal_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.clone().to_element(),
                                HORIZONTALS_BUCKET,
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.clone().to_element(), uuids, undo);
                        }
                    }
                    w.horizontal_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                UmlSequenceElement::Lifeline(..)
                | UmlSequenceElement::Message(..)
                | UmlSequenceElement::Ref(..)
                | UmlSequenceElement::DurationConstraint(..)
                | UmlSequenceElement::Note(..)
                | UmlSequenceElement::NoteLink(..) => {}
            }
        }

        for (idx, e) in self.elements.iter().enumerate() {
            if uuids.contains(&e.uuid()) {
                undo.push((
                    *self.uuid,
                    e.clone().to_element(),
                    0,
                    idx.try_into().unwrap(),
                ));
            } else {
                r(&e.clone().to_element(), uuids, undo);
            }
        }
        self.elements.retain(|e| !uuids.contains(&e.uuid()));
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
        for e in &self.elements {
            e.clone().to_element().accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for UmlSequenceDiagramBoard {
    type ElementT = UmlSequenceElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        for e in &self.elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        None
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((0, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl DiagramModel for UmlSequenceDiagramBoard {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        b: BucketNoT,
        p: Option<PositionNoT>,
        element: UmlSequenceElement,
    ) -> Result<PositionNoT, UmlSequenceElement> {
        if *self.uuid == target {
            self.insert_element_unsafe(b, p, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };
            match e {
                UmlSequenceElement::Diagram(inner) => inner.write().insert_element(b, p, element),
                UmlSequenceElement::CombinedFragment(inner) => {
                    inner.write().insert_element(b, p, element)
                }
                UmlSequenceElement::CombinedFragmentSection(inner) => {
                    inner.write().insert_element(b, p, element)
                }
                UmlSequenceElement::Lifeline(_)
                | UmlSequenceElement::Message(_)
                | UmlSequenceElement::Ref(_)
                | UmlSequenceElement::DurationConstraint(_)
                | UmlSequenceElement::Note(_)
                | UmlSequenceElement::NoteLink(_) => Err(element),
            }
        }
    }
    fn remove_element_from(
        &mut self,
        target: ModelUuid,
        uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        if *self.uuid == target {
            self.remove_element_safe(uuid)
        } else {
            match self.find_element(&target)?.0 {
                UmlSequenceElement::Diagram(inner) => inner.write().remove_element(uuid),
                UmlSequenceElement::CombinedFragment(inner) => inner.write().remove_element(uuid),
                UmlSequenceElement::CombinedFragmentSection(inner) => {
                    inner.write().remove_element(uuid)
                }
                UmlSequenceElement::Lifeline(_)
                | UmlSequenceElement::Message(_)
                | UmlSequenceElement::Ref(_)
                | UmlSequenceElement::DurationConstraint(_)
                | UmlSequenceElement::Note(_)
                | UmlSequenceElement::NoteLink(_) => None,
            }
        }
    }
}

impl FullTextSearchable for UmlSequenceDiagramBoard {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.name, &self.comment],
        );

        for e in &self.elements {
            e.full_text_search(acc);
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
    #[nh_context_serde(entity)]
    pub standalone_elements: Vec<UmlSequenceNonDiagramStandaloneElement>,

    pub comment: Arc<String>,
}

impl UmlSequenceDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        vertical_elements: Vec<ERef<UmlSequenceLifeline>>,
        horizontal_elements: Vec<UmlSequenceHorizontalElement>,
        standalone_elements: Vec<UmlSequenceNonDiagramStandaloneElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            vertical_elements,
            horizontal_elements,
            standalone_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlSequenceDiagram {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            vertical_elements: self
                .vertical_elements
                .iter()
                .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into))
                .collect(),
            horizontal_elements: self
                .horizontal_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
            standalone_elements: self
                .standalone_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn move_element(
        &mut self,
        element: &ModelUuid,
        within: BucketNoT,
        target_pos: PositionNoT,
    ) {
        if within == VERTICALS_BUCKET {
            if let Some((idx, _e)) = self
                .vertical_elements
                .iter()
                .enumerate()
                .find(|e| *e.1.read().uuid() == *element)
            {
                let e = self.vertical_elements.remove(idx);
                self.vertical_elements
                    .insert(target_pos.try_into().unwrap(), e);
            }
        } else if within == HORIZONTALS_BUCKET
            && let Some((idx, _e)) = self
                .horizontal_elements
                .iter()
                .enumerate()
                .find(|e| *e.1.uuid() == *element)
        {
            let e = self.horizontal_elements.remove(idx);
            self.horizontal_elements
                .insert(target_pos.try_into().unwrap(), e);
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlSequenceElement,
    ) -> Result<PositionNoT, UmlSequenceElement> {
        match bucket {
            0 | VERTICALS_BUCKET if let UmlSequenceElement::Lifeline(element) = element => {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.vertical_elements.len());
                self.vertical_elements.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            0 | HORIZONTALS_BUCKET if let Some(element) = element.clone().as_horizontal() => {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.horizontal_elements.len());
                self.horizontal_elements.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            0 | NONDIAGRAM_STANDALONE_BUCKET
                if let Some(element) = element.clone().as_nondiagram_standalone() =>
            {
                let pos = position
                    .map(|e| e.try_into().unwrap())
                    .unwrap_or(self.standalone_elements.len());
                self.standalone_elements.insert(pos, element);
                Ok(pos.try_into().unwrap())
            }
            _ => Err(element),
        }
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.vertical_elements.iter().enumerate() {
            if *e.read().uuid == *uuid {
                self.vertical_elements.remove(idx);
                return Some((VERTICALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.horizontal_elements.remove(idx);
                return Some((HORIZONTALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.standalone_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.standalone_elements.remove(idx);
                return Some((NONDIAGRAM_STANDALONE_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
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
        for e in &self.standalone_elements {
            if *e.uuid() == *uuid {
                return Some((e.clone().to_element(), *self.uuid));
            }
            if let Some(e) = e.find_element(uuid) {
                return Some(e);
            }
        }
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.vertical_elements.iter().enumerate() {
            if *e.read().uuid == *uuid {
                return Some((VERTICALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((HORIZONTALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        for (idx, e) in self.standalone_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((NONDIAGRAM_STANDALONE_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlSequenceDiagram {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.name, &self.comment],
        );

        for e in &self.vertical_elements {
            e.read().full_text_search(acc);
        }
        for e in &self.horizontal_elements {
            e.clone().to_element().full_text_search(acc);
        }
        for e in &self.standalone_elements {
            e.clone().to_element().full_text_search(acc);
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceLifeline {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,

    pub name: Arc<String>,
    pub stereotype: Arc<String>,

    pub comment: Arc<String>,
}

impl UmlSequenceLifeline {
    pub fn new(uuid: ModelUuid, name: String, stereotype: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            stereotype: Arc::new(stereotype),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            stereotype: self.stereotype.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
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

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum UmlSequenceMessageSynchronicityKind {
    #[default]
    Synchronous,
    AsynchronousCall,
    AsynchronousSignal,
}

impl UmlSequenceMessageSynchronicityKind {
    pub const VARIANTS: [Self; 3] = [
        Self::Synchronous,
        Self::AsynchronousCall,
        Self::AsynchronousSignal,
    ];

    pub fn as_str(&self) -> &'static str {
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
    pub const VARIANTS: [Self; 3] = [Self::None, Self::Create, Self::Delete];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Create => "Create",
            Self::Delete => "Delete",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceMessage {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub state_invariant: Arc<String>,

    #[full_text_searchable(skip)]
    pub synchronicity: UmlSequenceMessageSynchronicityKind,
    #[full_text_searchable(skip)]
    pub lifecycle: UmlSequenceMessageLifecycleKind,
    #[full_text_searchable(skip)]
    pub is_return: bool,
    #[full_text_searchable(skip)]
    pub duration: f32,

    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<UmlSequenceLifeline>,
    #[full_text_searchable(skip)]
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
        duration: f32,
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
            duration,
            source,
            target,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            state_invariant: self.state_invariant.clone(),
            synchronicity: self.synchronicity,
            lifecycle: self.lifecycle,
            is_return: self.is_return,
            duration: self.duration,
            source: self.source.clone(),
            target: self.target.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, UmlSequenceElement>) {
        let source_uuid = *self.source.read().uuid();
        if let Some(UmlSequenceElement::Lifeline(s)) = all_models.get(&source_uuid) {
            self.source = s.clone();
        }
        let target_uuid = *self.target.read().uuid();
        if let Some(UmlSequenceElement::Lifeline(t)) = all_models.get(&target_uuid) {
            self.target = t.clone();
        }
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
    pub const VARIANTS: [Self; 12] = [
        Self::Opt,
        Self::Alt,
        Self::Loop,
        Self::Break,
        Self::Par,
        Self::Strict,
        Self::Seq,
        Self::Critical,
        Self::Ignore,
        Self::Consider,
        Self::Assert,
        Self::Neg,
    ];

    pub fn as_str(&self) -> &'static str {
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

    pub fn takes_argument(&self) -> bool {
        match self {
            UmlSequenceCombinedFragmentKind::Loop
            | UmlSequenceCombinedFragmentKind::Ignore
            | UmlSequenceCombinedFragmentKind::Consider => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum UmlSequenceActivationBehaviour {
    #[default]
    ContinueFirstVariant,
    ResetToInitialState,
    TerminateActivations,
    // ConvergingOtherwiseResetToInitialState,
    // ConvergingOtherwiseTerminate,
}

impl UmlSequenceActivationBehaviour {
    pub const VARIANTS: [Self; 3] = [
        Self::ContinueFirstVariant,
        Self::ResetToInitialState,
        Self::TerminateActivations,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            UmlSequenceActivationBehaviour::ContinueFirstVariant => "Continue First Variant",
            UmlSequenceActivationBehaviour::ResetToInitialState => "Reset to Initial State",
            UmlSequenceActivationBehaviour::TerminateActivations => "Terminate Activations",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceCombinedFragment {
    pub uuid: Arc<ModelUuid>,
    pub kind: UmlSequenceCombinedFragmentKind,
    pub kind_argument: Arc<String>,
    pub end_behaviour: UmlSequenceActivationBehaviour,

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
        end_behaviour: UmlSequenceActivationBehaviour,
        horizontal_span: HashSet<ModelUuid>,
        sections: Vec<ERef<UmlSequenceCombinedFragmentSection>>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            kind,
            kind_argument: Arc::new(kind_argument),
            end_behaviour,
            horizontal_span,
            sections,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlSequenceCombinedFragment {
            uuid: new_uuid.into(),
            kind: self.kind,
            kind_argument: self.kind_argument.clone(),
            end_behaviour: self.end_behaviour,
            horizontal_span: self.horizontal_span.clone(),
            sections: self
                .sections
                .iter()
                .map(|e| e.read().deep_copy_clone_inner(ModelUuid::now_v7(), into))
                .collect(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn move_element(
        &mut self,
        element: &ModelUuid,
        within: BucketNoT,
        target_pos: PositionNoT,
    ) {
        if within == 1
            && let Some((idx, _e)) = self
                .sections
                .iter()
                .enumerate()
                .find(|e| *e.1.read().uuid() == *element)
        {
            let e = self.sections.remove(idx);
            self.sections.insert(target_pos.try_into().unwrap(), e);
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlSequenceElement,
    ) -> Result<PositionNoT, UmlSequenceElement> {
        if bucket != 0 && bucket != HORIZONTALS_BUCKET {
            return Err(element);
        }
        let UmlSequenceElement::CombinedFragmentSection(section) = element else {
            return Err(element);
        };

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.sections.len());
        self.sections.insert(pos, section);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.sections.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                self.sections.remove(idx);
                return Some((HORIZONTALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
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
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.sections.iter().enumerate() {
            if *e.read().uuid() == *uuid {
                return Some((HORIZONTALS_BUCKET, idx.try_into().unwrap()));
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
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(UmlSequenceCombinedFragmentSection {
            uuid: new_uuid.into(),
            guard: self.guard.clone(),
            horizontal_elements: self
                .horizontal_elements
                .iter()
                .map(|e| e.deep_copy_clone(ModelUuid::now_v7(), into))
                .collect(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn move_element(
        &mut self,
        element: &ModelUuid,
        within: BucketNoT,
        target_pos: PositionNoT,
    ) {
        if within == 1
            && let Some((idx, _e)) = self
                .horizontal_elements
                .iter()
                .enumerate()
                .find(|e| *e.1.uuid() == *element)
        {
            let e = self.horizontal_elements.remove(idx);
            self.horizontal_elements
                .insert(target_pos.try_into().unwrap(), e);
        }
    }

    fn insert_element(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: UmlSequenceElement,
    ) -> Result<PositionNoT, UmlSequenceElement> {
        if bucket != 0 && bucket != HORIZONTALS_BUCKET {
            return Err(element);
        }
        let Some(element) = element.as_horizontal() else {
            return Err(element);
        };

        let pos = position
            .map(|e| e.try_into().unwrap())
            .unwrap_or(self.horizontal_elements.len());
        self.horizontal_elements.insert(pos, element);
        Ok(pos.try_into().unwrap())
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                self.horizontal_elements.remove(idx);
                return Some((HORIZONTALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
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
        None
    }
    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        for (idx, e) in self.horizontal_elements.iter().enumerate() {
            if *e.uuid() == *uuid {
                return Some((HORIZONTALS_BUCKET, idx.try_into().unwrap()));
            }
        }
        None
    }
}

impl FullTextSearchable for UmlSequenceCombinedFragmentSection {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(*self.uuid, &[&self.uuid.to_string(), &self.guard]);

        for e in &self.horizontal_elements {
            e.clone().to_element().full_text_search(acc);
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceRef {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
    #[full_text_searchable(skip)]
    pub horizontal_span: HashSet<ModelUuid>,
}

impl UmlSequenceRef {
    pub fn new(uuid: ModelUuid, text: String, horizontal_span: HashSet<ModelUuid>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
            horizontal_span,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            text: self.text.clone(),
            horizontal_span: self.horizontal_span.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
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

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct DurationEnding {
    #[nh_context_serde(entity)]
    pub element: UmlSequenceHorizontalElement,
    pub end: bool,
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceDurationConstraint {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
    #[nh_context_serde(entity)]
    #[full_text_searchable(skip)]
    pub source: DurationEnding,
    #[nh_context_serde(entity)]
    #[full_text_searchable(skip)]
    pub target: DurationEnding,

    pub comment: Arc<String>,
}

impl UmlSequenceDurationConstraint {
    pub fn new(
        uuid: ModelUuid,
        text: String,
        source: (bool, UmlSequenceHorizontalElement),
        target: (bool, UmlSequenceHorizontalElement),
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
            source: DurationEnding {
                element: source.1,
                end: source.0,
            },
            target: DurationEnding {
                element: target.1,
                end: target.0,
            },
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            text: self.text.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, UmlSequenceElement>) {
        let source_uuid = *self.source.element.uuid();
        if let Some(s) = all_models.get(&source_uuid).and_then(|e| e.as_horizontal()) {
            self.source.element = s;
        }
        let target_uuid = *self.target.element.uuid();
        if let Some(t) = all_models.get(&target_uuid).and_then(|e| e.as_horizontal()) {
            self.target.element = t;
        }
    }
}

impl Entity for UmlSequenceDurationConstraint {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceDurationConstraint {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceNote {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl UmlSequenceNote {
    pub fn new(uuid: ModelUuid, text: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            text: self.text.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for UmlSequenceNote {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceNote {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceNoteLink {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<UmlSequenceNote>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: UmlSequenceElement,
}

impl UmlSequenceNoteLink {
    pub fn new(uuid: ModelUuid, source: ERef<UmlSequenceNote>, target: UmlSequenceElement) -> Self {
        Self {
            uuid: Arc::new(uuid),
            source,
            target,
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            source: self.source.clone(),
            target: self.target.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, UmlSequenceElement>) {
        let source_uuid = *self.source.read().uuid();
        if let Some(UmlSequenceElement::Note(s)) = all_models.get(&source_uuid) {
            self.source = s.clone();
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid) {
            self.target = t.clone();
        }
    }
}

impl Entity for UmlSequenceNoteLink {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for UmlSequenceNoteLink {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
