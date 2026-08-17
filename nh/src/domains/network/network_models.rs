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

pub fn deep_copy_diagram(
    d: &NetworkDiagram,
) -> (ERef<NetworkDiagram>, HashMap<ModelUuid, NetworkElement>) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = NetworkDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &NetworkDiagram) -> HashMap<ModelUuid, NetworkElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(e, &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &NetworkElement, into: &mut HashMap<ModelUuid, NetworkElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        NetworkElement::Container(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(e, into);
            }
        }
        NetworkElement::Node(_)
        | NetworkElement::User(_)
        | NetworkElement::File(_)
        | NetworkElement::Location(_)
        | NetworkElement::Association(_)
        | NetworkElement::Note(_) => {}
    }
}

pub fn transitive_closure(
    d: &NetworkDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &NetworkElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                NetworkElement::Container(inner) => {
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
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::File(_)
                | NetworkElement::Location(_)
                | NetworkElement::Association(_)
                | NetworkElement::Note(_) => {}
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &NetworkElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                NetworkElement::Container(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                }
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::File(_)
                | NetworkElement::Location(_) => {}
                NetworkElement::Association(inner) => {
                    let r = inner.read();
                    if !when_deleting.contains(&r.uuid)
                        && (when_deleting.contains(&r.source.uuid())
                            || when_deleting.contains(&r.target.uuid()))
                    {
                        also_delete.insert(*r.uuid);
                    }
                }
                NetworkElement::Note(_) => {}
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

pub fn top_sort_info(m: &NetworkElement) -> ModelTopSortInfo {
    fn walk(
        e: &NetworkElement,
        required_models: &mut HashSet<ModelUuid>,
        provided_models: &mut HashSet<ModelUuid>,
    ) {
        provided_models.insert(*e.uuid());
        match e {
            NetworkElement::Container(inner) => {
                for e in &inner.read().contained_elements {
                    walk(e, required_models, provided_models);
                }
            }
            NetworkElement::Node(_)
            | NetworkElement::User(_)
            | NetworkElement::File(_)
            | NetworkElement::Location(_) => {}
            NetworkElement::Association(inner) => {
                let r = inner.read();
                required_models.insert(*r.source.uuid());
                required_models.insert(*r.target.uuid());
            }
            NetworkElement::Note(_) => {}
        }
    }

    let (mut required_models, mut provided_models) = Default::default();
    walk(m, &mut required_models, &mut provided_models);
    ModelTopSortInfo {
        required_models,
        provided_models,
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
#[container_model(element_type = NetworkElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum NetworkElement {
    #[container_model(passthrough = "eref")]
    Container(ERef<NetworkContainer>),
    Node(ERef<NetworkNode>),
    User(ERef<NetworkUser>),
    File(ERef<NetworkFile>),
    Location(ERef<NetworkLocation>),

    Association(ERef<NetworkAssociation>),

    Note(ERef<NetworkNote>),
}

impl NetworkElement {
    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> Self {
        match self {
            Self::Container(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Node(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::User(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::File(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Location(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Association(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Note(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
    pub fn deep_copy_relink(&self, all_models: &HashMap<ModelUuid, NetworkElement>) {
        match self {
            Self::Container(_)
            | Self::Node(_)
            | Self::User(_)
            | Self::File(_)
            | Self::Location(_) => {}
            Self::Association(inner) => inner.write().deep_copy_relink(all_models),
            Self::Note(_) => {}
        }
    }
}

impl VisitableElement for NetworkElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            NetworkElement::Container(inner) => {
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
    pub fn new(uuid: ModelUuid, name: String, contained_elements: Vec<NetworkElement>) -> Self {
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
        element: NetworkElement,
    ) -> Result<PositionNoT, NetworkElement> {
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
        undo: &mut Vec<(ModelUuid, NetworkElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &NetworkElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, NetworkElement, BucketNoT, PositionNoT)>,
        ) {
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
                }
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::File(_)
                | NetworkElement::Location(_)
                | NetworkElement::Association(_)
                | NetworkElement::Note(_) => {}
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

impl DiagramModel for NetworkDiagram {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: NetworkElement,
    ) -> Result<PositionNoT, NetworkElement> {
        if let NetworkElement::Association(e) = &element {
            let (source_uuid, target_uuid) = {
                let r = e.read();
                (*r.source.uuid(), *r.target.uuid())
            };
            if self.find_element(&source_uuid).is_none()
                || self.find_element(&target_uuid).is_none()
            {
                return Err(element);
            }
        }

        if *self.uuid == target {
            self.insert_element_unsafe(bucket, position, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };
            match e {
                NetworkElement::Container(inner) => {
                    inner.write().insert_element(bucket, position, element)
                }
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::File(_)
                | NetworkElement::Location(_)
                | NetworkElement::Association(_)
                | NetworkElement::Note(_) => Err(element),
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
                NetworkElement::Container(inner) => inner.write().remove_element(uuid),
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::File(_)
                | NetworkElement::Location(_)
                | NetworkElement::Association(_)
                | NetworkElement::Note(_) => None,
            }
        }
    }
}

impl FullTextSearchable for NetworkDiagram {
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

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkContainer {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,

    #[nh_context_serde(entity)]
    pub contained_elements: Vec<NetworkElement>,

    pub comment: Arc<String>,
}

impl NetworkContainer {
    pub fn new(uuid: ModelUuid, name: String, contained_elements: Vec<NetworkElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(NetworkContainer {
            uuid: new_uuid.into(),
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
        element: NetworkElement,
    ) -> Result<PositionNoT, NetworkElement> {
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

impl FullTextSearchable for NetworkContainer {
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
pub enum NetworkNodeKind {
    Cloud,

    Firewall,
    IntrusionPreventionSystem,
    LoadBalancer,

    Hub,
    Router,
    WirelessRouter,
    Switch,
    #[default]
    Server,

    VirtualMachine,
    Workstation,
    IpPhone,
    Printer,

    Laptop,
    Tablet,
    CellularPhone,
    UsbDrive,
    OpticalMedia,
}

impl NetworkNodeKind {
    pub const VARIANTS: [Self; 18] = [
        Self::Cloud,
        Self::Firewall,
        Self::IntrusionPreventionSystem,
        Self::LoadBalancer,
        Self::Hub,
        Self::Router,
        Self::WirelessRouter,
        Self::Switch,
        Self::Server,
        Self::VirtualMachine,
        Self::Workstation,
        Self::IpPhone,
        Self::Printer,
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
            NetworkNodeKind::IntrusionPreventionSystem => "Intrusion Prevention System",
            NetworkNodeKind::LoadBalancer => "Load Balancer",
            NetworkNodeKind::Hub => "Hub",
            NetworkNodeKind::Router => "Router",
            NetworkNodeKind::WirelessRouter => "Wireless Router",
            NetworkNodeKind::Switch => "Switch",
            NetworkNodeKind::Server => "Server",
            NetworkNodeKind::VirtualMachine => "Virtual Machine",
            NetworkNodeKind::Workstation => "Workstation",
            NetworkNodeKind::IpPhone => "IP Phone",
            NetworkNodeKind::Printer => "Printer",
            NetworkNodeKind::Laptop => "Laptop",
            NetworkNodeKind::Tablet => "Tablet",
            NetworkNodeKind::CellularPhone => "Cellular Phone",
            NetworkNodeKind::UsbDrive => "USB Drive",
            NetworkNodeKind::OpticalMedia => "Optical Media",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct NetworkNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: NetworkNodeKind,

    pub comment: Arc<String>,
}

impl NetworkNode {
    pub fn new(uuid: ModelUuid, name: String, kind: NetworkNodeKind) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            kind: self.kind,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct NetworkUser {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: NetworkUserKind,

    pub comment: Arc<String>,
}

impl NetworkUser {
    pub fn new(uuid: ModelUuid, name: String, kind: NetworkUserKind) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            kind: self.kind,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
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

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkFileKind {
    #[default]
    Unspecified,

    Document,
    SourceCode,
    Certificate,

    Audio,
    Image,
    Video,

    Binary,
    Archive,
}

impl NetworkFileKind {
    pub const VARIANTS: [Self; 9] = [
        Self::Unspecified,
        Self::Document,
        Self::SourceCode,
        Self::Certificate,
        Self::Audio,
        Self::Image,
        Self::Video,
        Self::Binary,
        Self::Archive,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkFileKind::Unspecified => "Unspecified",
            NetworkFileKind::Document => "Document",
            NetworkFileKind::SourceCode => "Source Code",
            NetworkFileKind::Certificate => "Certificate",
            NetworkFileKind::Audio => "Audio",
            NetworkFileKind::Image => "Image",
            NetworkFileKind::Video => "Video",
            NetworkFileKind::Binary => "Binary",
            NetworkFileKind::Archive => "Archive",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct NetworkFile {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: NetworkFileKind,

    pub comment: Arc<String>,
}

impl NetworkFile {
    pub fn new(uuid: ModelUuid, name: String, kind: NetworkFileKind) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            kind: self.kind,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for NetworkFile {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkFile {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkLocationKind {
    #[default]
    Home,
    Neighbourhood,
    OfficeBranch,
    HeadOffice,
    Factory,
}

impl NetworkLocationKind {
    pub const VARIANTS: [Self; 5] = [
        Self::Home,
        Self::Neighbourhood,
        Self::OfficeBranch,
        Self::HeadOffice,
        Self::Factory,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkLocationKind::Home => "Home",
            NetworkLocationKind::Neighbourhood => "Neighbourhood",
            NetworkLocationKind::OfficeBranch => "Office Branch",
            NetworkLocationKind::HeadOffice => "Head Office",
            NetworkLocationKind::Factory => "Factory",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct NetworkLocation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[full_text_searchable(search_kind = "as_str_ref")]
    pub kind: NetworkLocationKind,

    pub comment: Arc<String>,
}

impl NetworkLocation {
    pub fn new(uuid: ModelUuid, name: String, kind: NetworkLocationKind) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            kind,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            name: self.name.clone(),
            kind: self.kind,
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for NetworkLocation {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkLocation {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkAssociationLineType {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl NetworkAssociationLineType {
    pub const VARIANTS: [Self; 3] = [Self::Solid, Self::Dashed, Self::Dotted];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkAssociationLineType::Solid => "Solid",
            NetworkAssociationLineType::Dashed => "Dashed",
            NetworkAssociationLineType::Dotted => "Dotted",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetworkAssociationArrowheadType {
    #[default]
    None,
    OpenTriangle,
    EmptyTriangle,
    FullTriangle,
    FullCircle,
}

impl NetworkAssociationArrowheadType {
    pub const VARIANTS: [Self; 5] = [
        Self::None,
        Self::OpenTriangle,
        Self::EmptyTriangle,
        Self::FullTriangle,
        Self::FullCircle,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkAssociationArrowheadType::None => "None",
            NetworkAssociationArrowheadType::OpenTriangle => "Open Triangle",
            NetworkAssociationArrowheadType::EmptyTriangle => "Empty Triangle",
            NetworkAssociationArrowheadType::FullTriangle => "Full Triangle",
            NetworkAssociationArrowheadType::FullCircle => "Full Circle",
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct NetworkAssociation {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,

    #[full_text_searchable(skip)]
    pub line_type: NetworkAssociationLineType,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: NetworkElement,
    #[full_text_searchable(skip)]
    pub source_arrowhead: NetworkAssociationArrowheadType,
    pub source_label_multiplicity: Arc<String>,
    pub source_label_role: Arc<String>,
    pub source_label_reading: Arc<String>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: NetworkElement,
    #[full_text_searchable(skip)]
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
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),

            line_type: self.line_type,
            source: self.source.clone(),
            source_arrowhead: self.source_arrowhead,
            source_label_multiplicity: self.source_label_multiplicity.clone(),
            source_label_role: self.source_label_role.clone(),
            source_label_reading: self.source_label_reading.clone(),
            target: self.target.clone(),
            target_arrowhead: self.target_arrowhead,
            target_label_multiplicity: self.target_label_multiplicity.clone(),
            target_label_role: self.target_label_role.clone(),
            target_label_reading: self.target_label_reading.clone(),

            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, NetworkElement>) {
        let source_uuid = *self.source.uuid();
        if let Some(s) = all_models.get(&source_uuid) {
            self.source = s.clone();
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models.get(&target_uuid) {
            self.target = t.clone();
        }
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

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct NetworkNote {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub text: Arc<String>,
}

impl NetworkNote {
    pub fn new(uuid: ModelUuid, text: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            text: Arc::new(text),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            text: self.text.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
}

impl Entity for NetworkNote {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for NetworkNote {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
