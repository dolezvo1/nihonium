use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::model::{
    BucketNoT, ContainerModel, DiagramModel, DiagramVisitor, ElementVisitor, Model, PositionNoT,
    VisitableDiagram, VisitableElement,
};
use crate::common::search::FullTextSearchable;
use crate::common::uuid::ModelUuid;
use std::collections::HashSet;
use std::{collections::HashMap, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use sophia::api::{
    MownStr,
    term::{GraphName, IriRef, LanguageTag, SimpleTerm},
};

#[cfg(not(target_arch = "wasm32"))]
pub struct RdfCollector<'a> {
    data: Vec<([SimpleTerm<'a>; 3], GraphName<SimpleTerm<'a>>)>,
    current_graph: GraphName<SimpleTerm<'a>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> RdfCollector<'a> {
    fn add_triple(&mut self, triple: [SimpleTerm<'a>; 3]) {
        self.data.push((triple, self.current_graph.clone()));
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
#[container_model(element_type = RdfElement, default_passthrough = "none")]
#[full_text_searchable(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfElement {
    #[container_model(passthrough = "eref")]
    Graph(ERef<RdfGraph>),
    Literal(ERef<RdfLiteral>),
    Node(ERef<RdfNode>),
    Predicate(ERef<RdfPredicate>),
}

#[derive(Clone, derive_more::From, nh_derive::Model, nh_derive::NHContextSerDeTag)]
#[model(default_passthrough = "eref")]
#[nh_context_serde(uuid_type = ModelUuid)]
pub enum RdfTargettableElement {
    Literal(ERef<RdfLiteral>),
    Node(ERef<RdfNode>),
}

impl RdfElement {
    pub fn as_targettable_element(&self) -> Option<RdfTargettableElement> {
        match self {
            RdfElement::Literal(inner) => Some(inner.clone().into()),
            RdfElement::Node(inner) => Some(inner.clone().into()),
            RdfElement::Graph(_) | RdfElement::Predicate(_) => None,
        }
    }

    pub fn deep_copy_clone(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, RdfElement>,
    ) -> Self {
        match self {
            Self::Graph(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Literal(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Node(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
            Self::Predicate(inner) => inner.read().deep_copy_clone_inner(new_uuid, into).into(),
        }
    }
    pub fn deep_copy_relink(&self, all_models: &HashMap<ModelUuid, RdfElement>) {
        match self {
            Self::Graph(_) | Self::Literal(_) | Self::Node(_) => {}
            Self::Predicate(inner) => inner.write().deep_copy_relink(all_models),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn accept_collector(&self, collector: &mut RdfCollector<'static>) {
        match self {
            RdfElement::Graph(inner) => {
                let model = inner.read();
                let old_graph = collector.current_graph.replace(SimpleTerm::Iri(
                    IriRef::new(MownStr::from((*model.iri).clone())).unwrap(),
                ));

                for c in &model.contained_elements {
                    c.accept_collector(collector);
                }

                collector.current_graph = old_graph;
            }
            RdfElement::Literal(_) | RdfElement::Node(_) => {}
            RdfElement::Predicate(inner) => {
                let model = inner.read();
                let subject = model.source.read().term_repr();
                let object = model.target.term_repr();

                collector.add_triple([
                    subject,
                    SimpleTerm::Iri(IriRef::new(MownStr::from((*model.iri).clone())).unwrap()),
                    object,
                ]);
            }
        }
    }
}

impl RdfTargettableElement {
    #[cfg(not(target_arch = "wasm32"))]
    fn term_repr(&self) -> SimpleTerm<'static> {
        match self {
            RdfTargettableElement::Literal(inner) => inner.read().term_repr(),
            RdfTargettableElement::Node(inner) => inner.read().term_repr(),
        }
    }
}

impl VisitableElement for RdfElement {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        match self {
            RdfElement::Graph(inner) => {
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

pub fn deep_copy_diagram(d: &RdfDiagram) -> (ERef<RdfDiagram>, HashMap<ModelUuid, RdfElement>) {
    let mut all_models = HashMap::new();
    let mut new_contained_elements = Vec::new();
    for e in &d.contained_elements {
        new_contained_elements.push(e.deep_copy_clone(ModelUuid::now_v7(), &mut all_models));
    }
    for e in all_models.values() {
        e.deep_copy_relink(&all_models);
    }

    let new_diagram = RdfDiagram {
        uuid: ModelUuid::now_v7().into(),
        name: d.name.clone(),
        contained_elements: new_contained_elements,
        comment: d.comment.clone(),
        stored_queries: d
            .stored_queries
            .iter()
            .map(|e| (uuid::Uuid::now_v7(), e.1.clone()))
            .collect(),
    };
    (ERef::new(new_diagram), all_models)
}

pub fn enumerate_diagram(d: &RdfDiagram) -> HashMap<ModelUuid, RdfElement> {
    let mut all_models = HashMap::new();
    for e in &d.contained_elements {
        enumerate_elements(e, &mut all_models);
    }
    all_models
}
fn enumerate_elements(e: &RdfElement, into: &mut HashMap<ModelUuid, RdfElement>) {
    into.insert(*e.uuid(), e.clone());
    match e {
        RdfElement::Graph(inner) => {
            for e in &inner.read().contained_elements {
                enumerate_elements(e, into);
            }
        }
        RdfElement::Literal(..) | RdfElement::Node(..) | RdfElement::Predicate(..) => {}
    }
}

pub fn transitive_closure(
    d: &RdfDiagram,
    mut when_deleting: HashSet<ModelUuid>,
) -> HashSet<ModelUuid> {
    for e in &d.contained_elements {
        fn walk(e: &RdfElement, when_deleting: &mut HashSet<ModelUuid>) {
            match e {
                RdfElement::Graph(inner) => {
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
                RdfElement::Literal(..) | RdfElement::Node(..) | RdfElement::Predicate(..) => {}
            }
        }
        walk(e, &mut when_deleting);
    }

    let mut also_delete = HashSet::new();
    loop {
        fn walk(
            e: &RdfElement,
            when_deleting: &HashSet<ModelUuid>,
            also_delete: &mut HashSet<ModelUuid>,
        ) {
            match e {
                RdfElement::Graph(inner) => {
                    for e in &inner.read().contained_elements {
                        walk(e, when_deleting, also_delete);
                    }
                }
                RdfElement::Literal(..) | RdfElement::Node(..) => {}
                RdfElement::Predicate(inner) => {
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
pub struct RdfDiagram {
    pub uuid: Arc<ModelUuid>,
    pub name: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<RdfElement>,
    pub stored_queries: HashMap<uuid::Uuid, (String, String)>,

    pub comment: Arc<String>,
}

impl RdfDiagram {
    pub fn new(uuid: ModelUuid, name: String, contained_elements: Vec<RdfElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            name: Arc::new(name),
            contained_elements,
            stored_queries: {
                let mut hm = HashMap::new();
                hm.insert(
                    uuid::Uuid::now_v7(),
                    (
                        "all".to_owned(),
                        "SELECT ?s ?p ?o WHERE { ?s ?p ?o }".to_owned(),
                    ),
                );
                hm
            },
            comment: Arc::new("".to_owned()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn graph(&self) -> Vec<([SimpleTerm<'_>; 3], GraphName<SimpleTerm<'_>>)> {
        let mut collector = RdfCollector {
            data: Vec::new(),
            current_graph: None,
        };

        for c in &self.contained_elements {
            c.accept_collector(&mut collector);
        }

        collector.data
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

    pub fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, RdfElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &RdfElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, RdfElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                RdfElement::Graph(inner) => {
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
                RdfElement::Literal(_) | RdfElement::Node(_) | RdfElement::Predicate(_) => {}
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

    fn insert_element_unsafe(
        &mut self,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: RdfElement,
    ) -> Result<PositionNoT, RdfElement> {
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
}

impl Entity for RdfDiagram {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfDiagram {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl VisitableDiagram for RdfDiagram {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>) {
        v.open_diagram(self);
        for e in &self.contained_elements {
            e.accept(v);
        }
        v.close_diagram(self);
    }
}

impl ContainerModel for RdfDiagram {
    type ElementT = RdfElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(RdfElement, ModelUuid)> {
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

impl DiagramModel for RdfDiagram {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: RdfElement,
    ) -> Result<PositionNoT, RdfElement> {
        if let RdfElement::Predicate(p) = &element {
            // TODO: Check that predicate source and target are both directly inside the desired parent
        }

        if *self.uuid == target {
            self.insert_element_unsafe(bucket, position, element)
        } else {
            let Some((e, _)) = self.find_element(&target) else {
                return Err(element);
            };
            match e {
                RdfElement::Graph(inner) => inner.write().insert_element(bucket, position, element),
                RdfElement::Literal(_) | RdfElement::Node(_) => Err(element),
                RdfElement::Predicate(_) => Err(element),
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
                RdfElement::Graph(inner) => inner.write().remove_element(uuid),
                RdfElement::Literal(_) | RdfElement::Node(_) => None,
                RdfElement::Predicate(_) => None,
            }
        }
    }
}

impl FullTextSearchable for RdfDiagram {
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
pub struct RdfGraph {
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    #[nh_context_serde(entity)]
    pub contained_elements: Vec<RdfElement>,

    pub comment: Arc<String>,
}

impl RdfGraph {
    pub fn new(uuid: ModelUuid, iri: String, contained_elements: Vec<RdfElement>) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            contained_elements,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, RdfElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(RdfGraph {
            uuid: new_uuid.into(),
            iri: self.iri.clone(),
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
        element: RdfElement,
    ) -> Result<PositionNoT, RdfElement> {
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

impl Entity for RdfGraph {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfGraph {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

impl ContainerModel for RdfGraph {
    type ElementT = RdfElement;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(RdfElement, ModelUuid)> {
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

impl FullTextSearchable for RdfGraph {
    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.check_element(
            *self.uuid,
            &[&self.uuid.to_string(), &self.iri, &self.comment],
        );

        for e in &self.contained_elements {
            e.full_text_search(acc);
        }
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct RdfLiteral {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub content: Arc<String>,
    pub datatype: Arc<String>,
    pub langtag: Arc<String>,

    pub comment: Arc<String>,
}

impl RdfLiteral {
    pub fn new(uuid: ModelUuid, content: String, datatype: String, langtag: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            content: Arc::new(content),
            datatype: Arc::new(datatype),
            langtag: Arc::new(langtag),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, RdfElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            content: self.content.clone(),
            datatype: self.datatype.clone(),
            langtag: self.langtag.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn term_repr(&self) -> SimpleTerm<'static> {
        if !self.langtag.is_empty() {
            SimpleTerm::LiteralLanguage(
                MownStr::from((*self.content).clone()),
                LanguageTag::new(MownStr::from((*self.langtag).clone())).unwrap(),
                None,
            )
        } else {
            let datatype = if !self.datatype.is_empty() {
                &self.datatype
            } else {
                "asdf"
            }
            .to_owned();
            SimpleTerm::LiteralDatatype(
                MownStr::from((*self.content).clone()),
                IriRef::new(MownStr::from(datatype)).unwrap(),
            )
        }
    }
}

impl Entity for RdfLiteral {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfLiteral {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct RdfNode {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,

    pub comment: Arc<String>,
}

impl RdfNode {
    pub fn new(uuid: ModelUuid, iri: String) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, RdfElement>,
    ) -> ERef<Self> {
        let new_uuid = ERef::new(Self {
            uuid: new_uuid.into(),
            iri: self.iri.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_uuid.clone().into());
        new_uuid
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn term_repr(&self) -> SimpleTerm<'static> {
        SimpleTerm::Iri(IriRef::new(MownStr::from((*self.iri).clone())).unwrap())
    }
}

impl Entity for RdfNode {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfNode {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}

#[derive(
    nh_derive::FullTextSearchable, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
#[nh_context_serde(is_entity)]
pub struct RdfPredicate {
    #[full_text_searchable(search_kind = "to_string_ref")]
    pub uuid: Arc<ModelUuid>,
    pub iri: Arc<String>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub source: ERef<RdfNode>,
    #[full_text_searchable(skip)]
    #[nh_context_serde(entity)]
    pub target: RdfTargettableElement,

    pub comment: Arc<String>,
}

impl RdfPredicate {
    pub fn new(
        uuid: ModelUuid,
        iri: String,
        source: ERef<RdfNode>,
        target: RdfTargettableElement,
    ) -> Self {
        Self {
            uuid: Arc::new(uuid),
            iri: Arc::new(iri),
            source,
            target,
            comment: Arc::new("".to_owned()),
        }
    }
    pub fn deep_copy_clone_inner(
        &self,
        new_uuid: ModelUuid,
        into: &mut HashMap<ModelUuid, RdfElement>,
    ) -> ERef<Self> {
        let new_model = ERef::new(Self {
            uuid: new_uuid.into(),
            iri: self.iri.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            comment: self.comment.clone(),
        });

        into.insert(*self.uuid, new_model.clone().into());
        new_model
    }
    pub fn deep_copy_relink(&mut self, all_models: &HashMap<ModelUuid, RdfElement>) {
        let source_uuid = *self.source.read().uuid();
        if let Some(RdfElement::Node(n)) = all_models.get(&source_uuid) {
            self.source = n.clone();
        }
        let target_uuid = *self.target.uuid();
        if let Some(t) = all_models
            .get(&target_uuid)
            .and_then(|e| e.as_targettable_element())
        {
            self.target = t;
        }
    }
}

impl Entity for RdfPredicate {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl Model for RdfPredicate {
    fn uuid(&self) -> Arc<ModelUuid> {
        self.uuid.clone()
    }
}
