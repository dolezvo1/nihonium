
use std::{collections::{HashMap, HashSet}, sync::Arc};

use crate::common::{controller::{BucketNoT, ContainerModel, DiagramVisitor, ElementVisitor, Model, PositionNoT, VisitableDiagram, VisitableElement}, entity::{Entity, EntityUuid}, eref::ERef, search::FullTextSearchable, uuid::ModelUuid};


pub fn deep_copy_diagram(d: &NetworkDiagram) -> (ERef<NetworkDiagram>, HashMap<ModelUuid, NetworkElement>) {
    fn walk(e: &NetworkElement, into: &mut HashMap<ModelUuid, NetworkElement>) -> NetworkElement {
        let new_uuid = ModelUuid::now_v7().into();
        match e {
            NetworkElement::Container(inner) => {
                let model = inner.read();

                let new_model = NetworkContainer {
                    uuid: new_uuid,
                    name: model.name.clone(),
                    kind: model.kind.clone(),
                    contained_elements: model.contained_elements.iter().map(|e| {
                        let new_model = walk(e, into);
                        into.insert(*e.uuid(), new_model.clone());
                        new_model
                    }).collect(),
                    comment: model.comment.clone()
                };
                ERef::new(new_model).into()
            },
            NetworkElement::Node(inner) => inner.read().clone_with(*new_uuid).into(),
            NetworkElement::User(inner) => inner.read().clone_with(*new_uuid).into(),
            NetworkElement::Association(inner) => inner.read().clone_with(*new_uuid).into(),
            NetworkElement::Comment(inner) => inner.read().clone_with(*new_uuid).into(),
        }
    }

    fn relink(e: &mut NetworkElement, all_models: &HashMap<ModelUuid, NetworkElement>) {
        match e {
            NetworkElement::Container(inner) => {
                let mut model = inner.write();
                for e in model.contained_elements.iter_mut() {
                    relink(e, all_models);
                }
            }
            NetworkElement::Node(_) | NetworkElement::User(_) => {},
            NetworkElement::Association(inner) => {
                let mut model = inner.write();

                let source_uuid = *model.source.uuid();
                if let Some(s) = all_models.get(&source_uuid) {
                    model.source = s.clone();
                }
                let target_uuid = *model.target.uuid();
                if let Some(t) = all_models.get(&target_uuid) {
                    model.target = t.clone();
                }
            },
            NetworkElement::Comment(_) => {},
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

    let new_diagram = NetworkDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn fake_copy_diagram(d: &NetworkDiagram) -> HashMap<ModelUuid, NetworkElement> {
    fn walk(e: &NetworkElement, into: &mut HashMap<ModelUuid, NetworkElement>) {
        match e {
            NetworkElement::Container(inner) => {
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

pub fn transitive_closure(d: &NetworkDiagram, mut when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &NetworkElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                NetworkElement::Container(inner) => {
                    let r = inner.read();
                    if when_deleting.contains(&r.uuid) {
                        enumerate(e, when_deleting);
                    } else {
                        for e in &r.contained_elements {
                            walk(e, when_deleting);
                        }
                    }
                },
                NetworkElement::Node(..)
                | NetworkElement::User(..)
                | NetworkElement::Association(..)
                | NetworkElement::Comment(..) => {},
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(e: &NetworkElement, when_deleting: &HashSet<ModelUuid>, also_delete: &mut HashSet<ModelUuid>) {
            match e {
                NetworkElement::Container(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                },
                NetworkElement::Node(..)
                | NetworkElement::User(..) => {},
                NetworkElement::Association(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid())) {
                        also_delete.insert(*r.uuid);
                    }
                },
                NetworkElement::Comment(..) => {}
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

fn enumerate(e: &NetworkElement, into: &mut HashSet<ModelUuid>) {
    into.insert(*e.uuid());
    match e {
        NetworkElement::Container(inner) => {
            for e in &inner.read().contained_elements {
                enumerate(e, into);
            }
        },
        NetworkElement::Node(..)
        | NetworkElement::User(..)
        | NetworkElement::Association(..)
        | NetworkElement::Comment(..) => {},
    }
}


#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::ContainerModel, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[container_model(element_type = NetworkElement, default_passthrough = "none")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum NetworkElement {
    #[container_model(passthrough = "eref")]
    Container(ERef<NetworkContainer>),
    Node(ERef<NetworkNode>),
    User(ERef<NetworkUser>),

    Association(ERef<NetworkAssociation>),

    Comment(ERef<NetworkComment>),
}

impl VisitableElement for NetworkElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        match self {
            NetworkElement::Container(inner) => {
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

impl FullTextSearchable for NetworkElement {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        match self {
            NetworkElement::Container(inner) => inner.read().full_text_search(acc),
            NetworkElement::Node(inner) => inner.read().full_text_search(acc),
            NetworkElement::User(inner) => inner.read().full_text_search(acc),
            NetworkElement::Association(inner) => inner.read().full_text_search(acc),
            NetworkElement::Comment(inner) => inner.read().full_text_search(acc),
        }
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = crate::common::project_serde::no_dependencies)]
pub struct NetworkDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<NetworkElement>,

    pub comment: Arc<String>,
}

impl NetworkDiagram {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        contained_elements: Vec<NetworkElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
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

    pub fn insert_element_into(&mut self, parent: ModelUuid, element: NetworkElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
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

    pub fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, NetworkElement, BucketNoT, PositionNoT)>) {
        fn r(e: &NetworkElement, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, NetworkElement, BucketNoT, PositionNoT)>) {
            match e {
                NetworkElement::Container(inner) => {
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
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::Association(_)
                | NetworkElement::Comment(_) => {},
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

impl Entity for NetworkDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for NetworkDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for NetworkDiagram {
    type ElementT = NetworkElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(NetworkElement, ModelUuid)> {
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
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: NetworkElement) -> Result<PositionNoT, NetworkElement> {
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

impl FullTextSearchable for NetworkDiagram {
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


#[derive(Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkContainerShapeKind {
    #[default]
    Rectangle,
    Rhombus,
    Ellipse,
}

impl NetworkContainerShapeKind {
    pub const VARIANTS: [Self; 3] = [Self::Rectangle, Self::Rhombus, Self::Ellipse];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkContainerShapeKind::Rectangle => "Rectangle",
            NetworkContainerShapeKind::Rhombus => "Rhombus",
            NetworkContainerShapeKind::Ellipse => "Ellipse",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkContainer {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    pub kind: NetworkContainerShapeKind,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<NetworkElement>,

    pub comment: Arc<String>,
}

impl NetworkContainer {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        kind: NetworkContainerShapeKind,
        contained_elements: Vec<NetworkElement>,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            kind: self.kind.clone(),
            contained_elements: self.contained_elements.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for NetworkContainer {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkContainer {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for NetworkContainer {
    type ElementT = NetworkElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(NetworkElement, ModelUuid)> {
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
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: NetworkElement) -> Result<PositionNoT, NetworkElement> {
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

impl FullTextSearchable for NetworkContainer {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.kind.as_str(),
                &self.comment,
            ],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}


#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkNodeKind {
    Cloud,

    Firewall,
    Router,
    Switch,
    #[default]
    Server,

    Workstation,
    Laptop,
    Tablet,
    CellularPhone,
    UsbDrive,
    OpticalMedia,
}

impl NetworkNodeKind {
    pub const VARIANTS: [Self; 11] = [
        Self::Cloud,
        Self::Firewall,
        Self::Router,
        Self::Switch,
        Self::Server,
        Self::Workstation,
        Self::Laptop,
        Self::Tablet,
        Self::CellularPhone,
        Self::UsbDrive,
        Self::OpticalMedia,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkNodeKind::Cloud => "Cloud",
            NetworkNodeKind::Firewall => "Firewall",
            NetworkNodeKind::Router => "Router",
            NetworkNodeKind::Switch => "Switch",
            NetworkNodeKind::Server => "Server",
            NetworkNodeKind::Workstation => "Workstation",
            NetworkNodeKind::Laptop => "Laptop",
            NetworkNodeKind::Tablet => "Tablet",
            NetworkNodeKind::CellularPhone => "Cellular Phone",
            NetworkNodeKind::UsbDrive => "USB Drive",
            NetworkNodeKind::OpticalMedia => "Optical Media",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkNode {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub kind: NetworkNodeKind,

    pub comment: Arc<String>,
}

impl NetworkNode {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        kind: NetworkNodeKind,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            kind: self.kind.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for NetworkNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for NetworkNode {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.kind.as_str(),
                &self.comment,
            ],
        );
    }
}


#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkUserKind {
    #[default]
    Normal,
    Sysadmin,
    Tie,
    Audit,
    Developer,

    BlackHat,
    GrayHat,
    WhiteHat,
}

impl NetworkUserKind {
    pub const VARIANTS: [Self; 8] = [
        Self::Normal,
        Self::Sysadmin,
        Self::Tie,
        Self::Audit,
        Self::Developer,
        Self::BlackHat,
        Self::GrayHat,
        Self::WhiteHat,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkUserKind::Normal => "Normal",
            NetworkUserKind::Sysadmin => "Sysadmin",
            NetworkUserKind::Tie => "Tie",
            NetworkUserKind::Audit => "Audit",
            NetworkUserKind::Developer => "Developer",
            NetworkUserKind::BlackHat => "Black Hat",
            NetworkUserKind::GrayHat => "Gray Hat",
            NetworkUserKind::WhiteHat => "White Hat",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkUser {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    pub kind: NetworkUserKind,

    pub comment: Arc<String>,
}

impl NetworkUser {
    pub fn new(
        uuid: ModelUuid,
        name: String,
        kind: NetworkUserKind,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),
            name: self.name.clone(),
            kind: self.kind.clone(),
            comment: self.comment.clone(),
        })
    }
}

impl Entity for NetworkUser {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkUser {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for NetworkUser {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.name,
                &self.kind.as_str(),
                &self.comment,
            ],
        );
    }
}


#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkAssociationLineType {
    #[default]
    Solid,
    Dashed,
}

impl NetworkAssociationLineType {
    pub const VARIANTS: [Self; 2] = [Self::Solid, Self::Dashed];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkAssociationLineType::Solid => "Solid",
            NetworkAssociationLineType::Dashed => "Dashed",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkAssociationArrowheadType {
    #[default]
    None,
    OpenTriangle,
    EmptyTriangle,
}

impl NetworkAssociationArrowheadType {
    pub const VARIANTS: [Self; 3] = [Self::None, Self::OpenTriangle, Self::EmptyTriangle];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkAssociationArrowheadType::None => "None",
            NetworkAssociationArrowheadType::OpenTriangle => "Open Triangle",
            NetworkAssociationArrowheadType::EmptyTriangle => "Empty Triangle",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkAssociation {
    pub uuid: Arc<ModelUuid>,

    pub line_type: NetworkAssociationLineType,
    #[nh_context_serde(entity)]
    pub source: NetworkElement,
    pub source_arrowhead: NetworkAssociationArrowheadType,
    pub source_label_multiplicity: Arc<String>,
    pub source_label_role: Arc<String>,
    pub source_label_reading: Arc<String>,
    #[nh_context_serde(entity)]
    pub target: NetworkElement,
    pub target_arrowhead: NetworkAssociationArrowheadType,
    pub target_label_multiplicity: Arc<String>,
    pub target_label_role: Arc<String>,
    pub target_label_reading: Arc<String>,

    pub comment: Arc<String>,
}

impl NetworkAssociation {
    pub fn new(
        uuid: ModelUuid,
        line_type: NetworkAssociationLineType,
        source: NetworkElement,
        source_arrowhead: NetworkAssociationArrowheadType,
        target: NetworkElement,
        target_arrowhead: NetworkAssociationArrowheadType,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),

            line_type,
            source,
            source_arrowhead,
            source_label_multiplicity: "".to_owned().into(),
            source_label_role: "".to_owned().into(),
            source_label_reading: "".to_owned().into(),
            target,
            target_arrowhead,
            target_label_multiplicity: "".to_owned().into(),
            target_label_role: "".to_owned().into(),
            target_label_reading: "".to_owned().into(),

            comment: Arc::new("".to_owned()),
        }
    }
    pub fn clone_with(&self, new_uuid: ModelUuid) -> ERef<Self> {
        ERef::new(Self {
            uuid: Arc::new(new_uuid),

            line_type: self.line_type.clone(),
            source: self.source.clone(),
            source_arrowhead: self.source_arrowhead.clone(),
            source_label_multiplicity: self.source_label_multiplicity.clone(),
            source_label_role: self.source_label_role.clone(),
            source_label_reading: self.source_label_reading.clone(),
            target: self.target.clone(),
            target_arrowhead: self.target_arrowhead.clone(),
            target_label_multiplicity: self.target_label_multiplicity.clone(),
            target_label_role: self.target_label_role.clone(),
            target_label_reading: self.target_label_reading.clone(),

            comment: self.comment.clone(),
        })
    }
    pub fn flip_multiconnection(&mut self) {
        std::mem::swap(&mut self.source, &mut self.target);
    }
}

impl Entity for NetworkAssociation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkAssociation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for NetworkAssociation {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[
                &self.uuid.to_string(),
                &self.comment,
            ],
        );
    }
}


#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkComment {
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl NetworkComment {
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

impl Entity for NetworkComment {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkComment {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl FullTextSearchable for NetworkComment {
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
