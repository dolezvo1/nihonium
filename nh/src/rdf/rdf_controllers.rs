use super::rdf_models::{RdfDiagram, RdfElement, RdfGraph, RdfLiteral, RdfNode, RdfPredicate, RdfTargettableElement};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ColorLabels, ColorProfile, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, Model, ModelHierarchyView, MulticonnectionAdapter, MulticonnectionView, PackageAdapter, PackageView, ProjectCommand, Queryable, SelectionStatus, SensitiveCommand, SimpleModelHierarchyView, SnapManager, TargettingStatus, Tool, VertexInformation, View
};
use crate::common::project_serde::{NHDeserializeError, NHDeserializeScalar, NHDeserializer, NHSerialize, NHSerializeError, NHSerializeToScalar, NHSerializer};
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::{CustomTab};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock, Weak},
};

use sophia::api::{prelude::SparqlDataset, sparql::Query};
use sophia_sparql::{ResultTerm, SparqlQuery, SparqlWrapper};

struct RdfDomain;
impl Domain for RdfDomain {
    type CommonElementT = RdfElement;
    type CommonElementViewT = RdfElementView;
    type QueryableT<'a> = RdfQueryable<'a>;
    type ToolT = NaiveRdfTool;
    type AddCommandElementT = RdfElementOrVertex;
    type PropChangeT = RdfPropChange;
}

type PackageViewT = PackageView<RdfDomain, RdfGraphAdapter>;
type LinkViewT = MulticonnectionView<RdfDomain, RdfPredicateAdapter>;

pub struct RdfQueryable<'a> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, RdfElementView>,
}

impl<'a> Queryable<'a, RdfDomain> for RdfQueryable<'a> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, RdfElementView>,
    ) -> Self {
        Self { models_to_views, flattened_views }
    }

    fn get_view(&self, m: &ModelUuid) -> Option<RdfElementView> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).cloned()
    }
}

#[derive(Clone)]
pub enum RdfPropChange {
    NameChange(Arc<String>),
    IriChange(Arc<String>),

    ContentChange(Arc<String>),
    DataTypeChange(Arc<String>),
    LangTagChange(Arc<String>),

    CommentChange(Arc<String>),
    FlipMulticonnection,
}

impl Debug for RdfPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "RdfPropChange::{}",
            match self {
                Self::NameChange(name) => format!("NameChange({})", name),
                Self::IriChange(iri) => format!("IriChange({})", iri),

                Self::ContentChange(content) => format!("ContentChange({})", content),
                Self::DataTypeChange(datatype) => format!("DataTypeChange({})", datatype),
                Self::LangTagChange(langtag) => format!("LangTagChange({})", langtag),

                Self::CommentChange(comment) => format!("CommentChange({})", comment),
                Self::FlipMulticonnection => format!("FlipMulticonnection"),
            }
        )
    }
}

impl TryFrom<&RdfPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &RdfPropChange) -> Result<Self, Self::Error> {
        match value {
            RdfPropChange::FlipMulticonnection => Ok(FlipMulticonnection {}),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum RdfElementOrVertex {
    Element(RdfElementView),
    Vertex(VertexInformation),
}

impl Debug for RdfElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "RdfElementOrVertex::???")
    }
}

impl TryFrom<RdfElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: RdfElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            RdfElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<RdfElementOrVertex> for RdfElementView {
    type Error = ();

    fn try_from(value: RdfElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            RdfElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

pub fn colors() -> (String, ColorLabels, Vec<ColorProfile>) {
    #[rustfmt::skip]
    let c = crate::common::controller::build_colors!(
                                   ["Light",              "Darker"],
        [("Diagram background",    [egui::Color32::WHITE, egui::Color32::GRAY,]),
         ("Graph background",      [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),
         ("Connection background", [egui::Color32::WHITE, egui::Color32::WHITE,]),
         ("Node background",       [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),
         ("Literal background",    [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),],
        [("Diagram gridlines",     [egui::Color32::from_rgb(220, 220, 220), egui::Color32::from_rgb(127, 127, 127),]),
         ("Graph foreground",      [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Connection foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Node foreground",       [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Literal foreground",    [egui::Color32::BLACK, egui::Color32::BLACK,]),],
        [("Selection",             [egui::Color32::BLUE,  egui::Color32::LIGHT_BLUE,]),],
    );
    ("RDF diagram".to_owned(), c.0, c.1)
}

#[derive(Clone, derive_more::From)]
pub enum RdfElementView {
    Graph(Arc<RwLock<PackageViewT>>),
    Literal(Arc<RwLock<RdfLiteralController>>),
    Node(Arc<RwLock<RdfNodeController>>),
    Predicate(Arc<RwLock<LinkViewT>>),
}

impl NHSerialize for RdfElementView {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            Self::Graph(inner) => inner.read().unwrap().serialize_into(into),
            Self::Literal(inner) => inner.read().unwrap().serialize_into(into),
            Self::Node(inner) => inner.read().unwrap().serialize_into(into),
            Self::Predicate(inner) => inner.read().unwrap().serialize_into(into),
        }
    }
}
// impl NHDeserialize for RdfElementView {}
impl View for RdfElementView {
    fn uuid(&self) -> Arc<ViewUuid> {
        match self {
            Self::Graph(inner) => inner.read().unwrap().uuid(),
            Self::Literal(inner) => inner.read().unwrap().uuid(),
            Self::Node(inner) => inner.read().unwrap().uuid(),
            Self::Predicate(inner) => inner.read().unwrap().uuid(),
        }
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        match self {
            Self::Graph(inner) => inner.read().unwrap().model_uuid(),
            Self::Literal(inner) => inner.read().unwrap().model_uuid(),
            Self::Node(inner) => inner.read().unwrap().model_uuid(),
            Self::Predicate(inner) => inner.read().unwrap().model_uuid(),
        }
    }
    fn model_name(&self) -> Arc<String> {
        match self {
            Self::Graph(inner) => inner.read().unwrap().model_name(),
            Self::Literal(inner) => inner.read().unwrap().model_name(),
            Self::Node(inner) => inner.read().unwrap().model_name(),
            Self::Predicate(inner) => inner.read().unwrap().model_name(),
        }
    }
}
impl ElementController<RdfElement> for RdfElementView {
    fn model(&self) -> RdfElement {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().model(),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().model(),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().model(),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().model(),
        }
    }
    fn min_shape(&self) -> NHShape {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().min_shape(),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().min_shape(),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().min_shape(),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().min_shape(),
        }
    }
    fn max_shape(&self) -> NHShape {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().max_shape(),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().max_shape(),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().max_shape(),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().max_shape(),
        }
    }
    fn position(&self) -> egui::Pos2 {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().position(),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().position(),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().position(),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().position(),
        }
    }
}
impl ContainerGen2<RdfDomain> for RdfElementView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<<RdfDomain as Domain>::CommonElementViewT> {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
        }
    }
}
impl ElementControllerGen2<RdfDomain> for RdfElementView {
    fn show_properties(
        &mut self,
        q: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> bool {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
        }
    }
    fn draw_in(
        &mut self,
        q: &RdfQueryable,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> EventHandlingStatus {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
        }
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
        }
    }
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, RdfElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
        }
    }
    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, RdfElementView>,
        c: &mut HashMap<ViewUuid, RdfElementView>,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, RdfElementView>,
        c: &mut HashMap<ViewUuid, RdfElementView>,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            RdfElementView::Literal(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            RdfElementView::Node(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            RdfElementView::Predicate(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, RdfElementView>,
        m: &HashMap<ModelUuid, RdfElement>,
    ) {
        match self {
            RdfElementView::Graph(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            RdfElementView::Literal(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            RdfElementView::Node(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            RdfElementView::Predicate(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
        }
    }

}

#[derive(Clone)]
pub struct RdfDiagramAdapter {
    model: Arc<RwLock<RdfDiagram>>,
    name_buffer: String,
    comment_buffer: String,
}

impl DiagramAdapter<RdfDomain, RdfDiagram> for RdfDiagramAdapter {
    fn model(&self) -> Arc<RwLock<RdfDiagram>> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name()
    }
    fn view_type(&self) -> &'static str {
        "rdf-diagram-view"
    }

    fn create_new_view_for(
        &self,
        q: &RdfQueryable<'_>,
        element: RdfElement,
    ) -> Result<RdfElementView, HashSet<ModelUuid>> {
        let v = match element {
            RdfElement::RdfGraph(rw_lock) => {
                RdfElementView::from(
                    new_rdf_graph_view(
                        rw_lock,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            },
            RdfElement::RdfTargettable(rdf_targettable_element) => {
                match rdf_targettable_element {
                    RdfTargettableElement::RdfLiteral(rw_lock) => {
                        RdfElementView::from(
                            new_rdf_literal_view(rw_lock, egui::Pos2::ZERO)
                        )
                    },
                    RdfTargettableElement::RdfNode(rw_lock) => {
                        RdfElementView::from(
                            new_rdf_node_view(rw_lock, egui::Pos2::ZERO)
                        )
                    },
                }
            },
            RdfElement::RdfPredicate(rw_lock) => {
                let m = rw_lock.read().unwrap();
                let (source_view, target_view) = match (q.get_view(&m.source.read().unwrap().uuid), q.get_view(&m.target.uuid())) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*m.source.read().unwrap().uuid, *m.target.uuid()])),
                };
                RdfElementView::from(
                    new_rdf_predicate_view(
                        rw_lock.clone(),
                        source_view,
                        target_view,
                    )
                )
            },
        };

        Ok(v)
    }

    fn show_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![RdfPropChange::NameChange(Arc::new(self.name_buffer.clone()))],
                )
                .into(),
            );
        };

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![RdfPropChange::CommentChange(Arc::new(
                        self.comment_buffer.clone(),
                    ))],
                )
                .into(),
            );
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    RdfPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::NameChange(model.name.clone())],
                        ));
                        self.name_buffer = (**name).clone();
                        model.name = name.clone();
                    }
                    RdfPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::CommentChange(model.comment.clone())],
                        ));
                        self.comment_buffer = (**comment).clone();
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn tool_change_fun(&self, tool: &mut Option<NaiveRdfTool>, ui: &mut egui::Ui) {
        let width = ui.available_width();

        let stage = tool.as_ref().map(|e| e.initial_stage());
        let c = |s: RdfToolStage| -> egui::Color32 {
            if stage.is_some_and(|e| e == s) {
                egui::Color32::BLUE
            } else {
                egui::Color32::BLACK
            }
        };

        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::new("Select/Move").fill(if stage == None {
                    egui::Color32::BLUE
                } else {
                    egui::Color32::BLACK
                }),
            )
            .clicked()
        {
            *tool = None;
        }
        ui.separator();

        for cat in [
            &[
                (RdfToolStage::Literal, "Literal"),
                (RdfToolStage::Node, "Node"),
                (RdfToolStage::PredicateStart, "Predicate"),
                (RdfToolStage::GraphStart, "Graph"),
            ][..],
            &[(RdfToolStage::Note, "Note")][..],
        ] {
            for (stage, name) in cat {
                if ui
                    .add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage)))
                    .clicked()
                {
                    *tool = Some(NaiveRdfTool::new(*stage));
                }
            }
            ui.separator();
        }
    }

    fn menubar_options_fun(&self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
        if ui.button("Import RDF data").clicked() {
            // TODO: import stuff
        }
        if ui.button("SPARQL Queries").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(SparqlQueriesTab {
                    diagram: self.model.clone(),
                    selected_query: None,
                    query_name_buffer: "".to_owned(),
                    query_value_buffer: "".to_owned(),
                    debug_message: None,
                    query_results: None,
                })),
            ));
        }
        if ui.button("Ontology alignment").clicked() {
            // TODO: similar to the above?
        }
        ui.separator();
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, RdfElement>) {
        let (new_model, models) = super::rdf_models::deep_copy_diagram(&self.model.read().unwrap());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, RdfElement>) {
        let models = super::rdf_models::fake_copy_diagram(&self.model.read().unwrap());
        (self.clone(), models)
    }
}

impl NHSerializeToScalar for RdfDiagramAdapter {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<toml::Value, NHSerializeError> {
        self.model.read().unwrap().serialize_into(into)?;

        Ok(toml::Value::String(self.model.read().unwrap().uuid().to_string()))
    }
}

impl NHDeserializeScalar for RdfDiagramAdapter {
    fn deserialize(
        source: &toml::Value,
        deserializer: &NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let toml::Value::String(s) = source else {
            return Err(NHDeserializeError::StructureError(format!("expected string, got {:?}", source)));
        };
        let uuid = uuid::Uuid::parse_str(s)?.into();
        let model = deserializer.get_or_instantiate_model::<RdfDiagram>(&uuid)?;
        let name_buffer = (*model.read().unwrap().name).clone();
        let comment_buffer = (*model.read().unwrap().comment).clone();
        Ok(Self { model, name_buffer, comment_buffer })
    }
}

struct SparqlQueriesTab {
    diagram: Arc<RwLock<RdfDiagram>>,
    selected_query: Option<uuid::Uuid>,
    query_name_buffer: String,
    query_value_buffer: String,
    debug_message: Option<String>,
    query_results: Option<Vec<Vec<Option<ResultTerm>>>>,
}

impl SparqlQueriesTab {
    fn save(&mut self) {
        let mut model = self.diagram.write().unwrap();

        if let Some(q) = self
            .selected_query
            .as_ref()
            .and_then(|uuid| model.stored_queries.get_mut(uuid))
        {
            q.0 = self.query_name_buffer.clone();
            q.1 = self.query_value_buffer.clone();
        } else {
            let uuid = uuid::Uuid::now_v7();
            model.stored_queries.insert(
                uuid.clone(),
                (
                    self.query_name_buffer.to_owned(),
                    self.query_value_buffer.to_owned(),
                ),
            );
            self.selected_query = Some(uuid);
        }
    }
    fn execute(&mut self) {
        let model = self.diagram.write().unwrap();

        match SparqlQuery::parse(&self.query_value_buffer) {
            Err(e) => {
                self.debug_message = Some(format!("{:?}", e));
            }
            Ok(query) => match SparqlWrapper(&model.graph())
                .query(&query)
                .map(|e| e.into_bindings())
            {
                Err(e) => {
                    self.debug_message = Some(format!("{:?}", e));
                }
                Ok(results) => {
                    self.debug_message = None;
                    self.query_results =
                        Some(results.into_iter().flat_map(|e| e.into_iter()).collect());
                }
            },
        }
    }
}

impl CustomTab for SparqlQueriesTab {
    fn title(&self) -> String {
        "SPARQL Queries".to_owned()
    }

    fn show(&mut self, /*context: &mut NHApp,*/ ui: &mut egui::Ui) {
        let mut model = self.diagram.write().unwrap();

        egui::ComboBox::from_label("Select diagram")
            .selected_text(format!("{}", model.name))
            .show_ui(ui, |_ui| {
                // TODO: if ui.selectable_value(&mut self.diagram, e.clone(), format!("{:?}", e.name)).clicked() {}
                // TODO: zero out selected query?
            });

        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Select query")
                .selected_text(if let Some(uuid) = &self.selected_query {
                    model.stored_queries.get(uuid).unwrap().0.clone()
                } else {
                    "".to_owned()
                })
                .show_ui(ui, |ui| {
                    for (k, q) in &model.stored_queries {
                        if ui
                            .selectable_value(
                                &mut self.selected_query,
                                Some(k.clone()),
                                q.0.clone(),
                            )
                            .clicked()
                        {
                            self.query_name_buffer = q.0.clone();
                            self.query_value_buffer = q.1.clone();
                        }
                    }
                });

            if ui.button("Add new").clicked() {
                let uuid = uuid::Uuid::now_v7();
                model
                    .stored_queries
                    .insert(uuid.clone(), ("".to_owned(), "".to_owned()));
                self.selected_query = Some(uuid);
            }

            if self.selected_query.is_none() {
                ui.disable();
            }

            if ui.button("Delete").clicked() {
                model.stored_queries.remove(&self.selected_query.unwrap());
                self.selected_query = None;
            }
        });

        if self.selected_query.is_none() {
            ui.disable();
        }

        ui.label("Query name:");
        let _r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.query_name_buffer),
        );

        ui.label("Query:");
        let _r3 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.query_value_buffer),
        );

        drop(model);

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                self.save();
            }

            if ui.button("Save & Execute").clicked() {
                self.save();
                self.execute();
            }

            if ui.button("Execute").clicked() {
                self.execute();
            }
        });

        if let Some(m) = &self.debug_message {
            ui.colored_label(egui::Color32::RED, m);
        }

        if let Some(results) = &self.query_results {
            ui.label("Results:");

            let mut tb = TableBuilder::new(ui);

            if let Some(max_cols) = results.iter().map(|e| e.len()).max() {
                for _ in 0..max_cols {
                    tb = tb.column(Column::auto().resizable(true));
                }

                tb.body(|mut body| {
                    for rr in results {
                        body.row(30.0, |mut row| {
                            for ee in rr {
                                row.col(|ui| {
                                    ui.label(match ee {
                                        Some(term) => format!("{}", term),
                                        _ => "".to_owned(),
                                    });
                                });
                            }
                        });
                    }
                });
            }
        }
    }
}

pub fn new(no: u32) -> (Arc<RwLock<dyn DiagramController>>, Arc<dyn ModelHierarchyView>) {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New RDF diagram {}", no);

    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    )));
    (
        DiagramControllerGen2::new(
            Arc::new(view_uuid),
            RdfDiagramAdapter {
                model: diagram.clone(),
                name_buffer: name,
                comment_buffer: "".to_owned(),
            },
            Vec::new(),
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram)),
    )
}

pub fn demo(no: u32) -> (Arc<RwLock<dyn DiagramController>>, Arc<dyn ModelHierarchyView>) {
    let (node, node_view) = new_rdf_node(
        "http://www.w3.org/People/EM/contact#me",
        egui::Pos2::new(300.0, 100.0),
    );

    let (literal_model, literal_view) = new_rdf_literal(
        "Eric Miller",
        "http://www.w3.org/2001/XMLSchema#string",
        "en",
        egui::Pos2::new(300.0, 200.0),
    );

    let (predicate, predicate_view) = new_rdf_predicate(
        "http://www.w3.org/2000/10/swap/pim/contact#fullName",
        (node.clone(), node_view.clone().into()),
        (literal_model.clone().into(), literal_view.clone().into()),
    );

    let (graph, graph_view) = new_rdf_graph(
        "http://graph",
        egui::Rect::from_min_max(egui::Pos2::new(400.0, 50.0), egui::Pos2::new(500.0, 150.0)),
    );

    //<stress test>
    let mut models_st = Vec::<RdfElement>::new();
    let mut controllers_st = Vec::<RdfElementView>::new();

    for xx in 0..=10 {
        for yy in 300..=400 {
            let (node_st, node_st_view) = new_rdf_node(
                "http://www.w3.org/People/EM/contact#me",
                egui::Pos2::new(xx as f32, yy as f32),
            );
            models_st.push(RdfTargettableElement::from(node_st).into());
            controllers_st.push(node_st_view.into());
        }
    }

    for xx in 3000..=3100 {
        for yy in 3000..=3100 {
            let (node_st, node_st_view) = new_rdf_node(
                "http://www.w3.org/People/EM/contact#me",
                egui::Pos2::new(xx as f32, yy as f32),
            );
            models_st.push(RdfTargettableElement::from(node_st).into());
            controllers_st.push(node_st_view.into());
        }
    }

    let (graph_st, graph_st_view) = new_rdf_graph(
        "http://stresstestgraph",
        egui::Rect::from_min_max(egui::Pos2::new(0.0, 300.0), egui::Pos2::new(3000.0, 3300.0)),
    );
    //</stress test>

    let mut owned_controllers = Vec::<RdfElementView>::new();
    owned_controllers.push(node_view.into());
    owned_controllers.push(literal_view.into());
    owned_controllers.push(predicate_view.into());
    owned_controllers.push(graph_view.into());
    owned_controllers.push(graph_st_view.into());

    let name = format!("Demo RDF diagram {}", no);
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        model_uuid,
        name.clone(),
        vec![
            RdfTargettableElement::from(node).into(),
            RdfTargettableElement::from(literal_model).into(),
            predicate.into(), graph.into(), graph_st.into(),
        ],
    )));
    (
        DiagramControllerGen2::new(
            Arc::new(view_uuid),
            RdfDiagramAdapter {
                model: diagram.clone(),
                name_buffer: name,
                comment_buffer: "".to_owned(),
            },
            owned_controllers,
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram)),
    )
}

#[derive(Clone, Copy, PartialEq)]
pub enum RdfToolStage {
    Literal,
    Node,
    PredicateStart,
    PredicateEnd,
    GraphStart,
    GraphEnd,
    Note,
}

enum PartialRdfElement {
    None,
    Some(RdfElementView),
    Predicate {
        source: Arc<RwLock<RdfNode>>,
        dest: Option<RdfTargettableElement>,
    },
    Graph {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveRdfTool {
    initial_stage: RdfToolStage,
    current_stage: RdfToolStage,
    result: PartialRdfElement,
    event_lock: bool,
}

impl NaiveRdfTool {
    pub fn new(initial_stage: RdfToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialRdfElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<RdfDomain> for NaiveRdfTool {
    type Stage = RdfToolStage;

    fn initial_stage(&self) -> RdfToolStage {
        self.initial_stage
    }

    fn targetting_for_element(&self, element: Option<RdfElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd
                | RdfToolStage::Note => TARGETTABLE_COLOR,
                RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfGraph(..)) => match self.current_stage {
                RdfToolStage::Literal | RdfToolStage::Node | RdfToolStage::Note => {
                    TARGETTABLE_COLOR
                }
                RdfToolStage::PredicateStart
                | RdfToolStage::PredicateEnd
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfTargettable(RdfTargettableElement::RdfLiteral(..))) => match self.current_stage {
                RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::PredicateStart
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd
                | RdfToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfTargettable(RdfTargettableElement::RdfNode(..))) => match self.current_stage {
                RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd
                | RdfToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfPredicate(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &RdfQueryable, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialRdfElement::Predicate { source, .. } => {
                if let Some(source_view) = q.get_view(&source.read().unwrap().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialRdfElement::Graph { a, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a, pos),
                    egui::CornerRadius::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            _ => {}
        }
    }

    fn add_position(&mut self, pos: egui::Pos2) {
        if self.event_lock {
            return;
        }

        match (self.current_stage, &mut self.result) {
            (RdfToolStage::Literal, _) => {
                let (literal_model, literal_view) = new_rdf_literal(
                    "Eric Miller",
                    "http://www.w3.org/2001/XMLSchema#string",
                    "en",
                    pos,
                );
                
                self.result = PartialRdfElement::Some(literal_view.into());
                self.event_lock = true;
            }
            (RdfToolStage::Node, _) => {
                let (_node, node_view) =
                    new_rdf_node("http://www.w3.org/People/EM/contact#me", pos);
                self.result = PartialRdfElement::Some(node_view.into());
                self.event_lock = true;
            }
            (RdfToolStage::GraphStart, _) => {
                self.result = PartialRdfElement::Graph { a: pos, b: None };
                self.current_stage = RdfToolStage::GraphEnd;
                self.event_lock = true;
            }
            (RdfToolStage::GraphEnd, PartialRdfElement::Graph { b, .. }) => *b = Some(pos),
            (RdfToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: RdfElement) {
        if self.event_lock {
            return;
        }

        match controller {
            RdfElement::RdfGraph(..) => {}
            RdfElement::RdfTargettable(RdfTargettableElement::RdfLiteral(inner)) => match (self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { dest, .. }) => {
                    *dest = Some(RdfTargettableElement::from(inner).into());
                    self.event_lock = true;
                }
                _ => {}
            },
            RdfElement::RdfTargettable(RdfTargettableElement::RdfNode(inner)) => match (self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateStart, PartialRdfElement::None) => {
                    self.result = PartialRdfElement::Predicate {
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = RdfToolStage::PredicateEnd;
                    self.event_lock = true;
                }
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { dest, .. }) => {
                    *dest = Some(RdfTargettableElement::from(inner).into());
                }
                _ => {}
            },
            RdfElement::RdfPredicate(..) => {}
        }
    }

    fn try_construct(&mut self, into: &dyn ContainerGen2<RdfDomain>) -> Option<RdfElementView> {
        match &self.result {
            PartialRdfElement::Some(x) => {
                let x = x.clone();
                self.result = PartialRdfElement::None;
                Some(x)
            }
            // TODO: check for source == dest case, set points?
            PartialRdfElement::Predicate {
                source,
                dest: Some(dest),
                ..
            } => {
                self.current_stage = RdfToolStage::PredicateStart;

                let predicate_view: Option<RdfElementView> =
                    if let (Some(source_controller), Some(dest_controller)) = (
                        into.controller_for(&source.read().unwrap().uuid()),
                        into.controller_for(&dest.uuid()),
                    ) {
                        let (_predicate_model, predicate_view) = new_rdf_predicate(
                            "http://www.w3.org/2000/10/swap/pim/contact#fullName",
                            (source.clone(), source_controller),
                            (dest.clone(), dest_controller),
                        );

                        Some(predicate_view.into())
                    } else {
                        None
                    };

                self.result = PartialRdfElement::None;
                predicate_view
            }
            PartialRdfElement::Graph { a, b: Some(b) } => {
                self.current_stage = RdfToolStage::GraphStart;

                let (_graph_model, graph_view) =
                    new_rdf_graph("http://a-graph", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialRdfElement::None;
                Some(graph_view.into())
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

#[derive(Clone)]
pub struct RdfGraphAdapter {
    model: Arc<RwLock<RdfGraph>>,
}

impl PackageAdapter<RdfDomain> for RdfGraphAdapter {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().iri.clone()
    }
    
    fn view_type(&self) -> &'static str {
        "rdf-graph-view"
    }

    fn add_element(&mut self, element: RdfElement) {
        self.model.write().unwrap().add_element(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.model.write().unwrap().delete_elements(uuids);
    }

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>
    ) {
        let model = self.model.read().unwrap();
        let mut iri_buffer = (*model.iri).clone();
        ui.label("IRI:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut iri_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::IriChange(Arc::new(iri_buffer)),
            ]));
        }

        let mut comment_buffer = (*model.comment).clone();
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(comment_buffer)),
            ]));
        }
    }

    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    RdfPropChange::IriChange(iri) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::IriChange(model.iri.clone())],
                        ));
                        model.iri = iri.clone();
                    }
                    RdfPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::CommentChange(model.comment.clone())],
                        ));
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read().unwrap();

        let model = if let Some(RdfElement::RdfGraph(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(RdfGraph::new(new_uuid, (*old_model.iri).clone(), old_model.contained_elements.clone())));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };
        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, RdfElement>,
    ) {
        todo!()
    }
}

fn new_rdf_graph(
    iri: &str,
    bounds_rect: egui::Rect,
) -> (Arc<RwLock<RdfGraph>>, Arc<RwLock<PackageViewT>>) {
    let model_uuid = uuid::Uuid::now_v7().into();
    let graph_model = Arc::new(RwLock::new(RdfGraph::new(
        model_uuid,
        iri.to_owned(),
        vec![],
    )));
    let graph_view = new_rdf_graph_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_rdf_graph_view(
    model: Arc<RwLock<RdfGraph>>,
    bounds_rect: egui::Rect,
) -> Arc<RwLock<PackageViewT>> {
    PackageView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        RdfGraphAdapter {
            model,
        },
        HashMap::new(),
        bounds_rect,
    )
}

fn new_rdf_node(
    iri: &str,
    position: egui::Pos2,
) -> (
    Arc<RwLock<RdfNode>>,
    Arc<RwLock<RdfNodeController>>,
) {
    let node_model_uuid = uuid::Uuid::now_v7().into();
    let node_model = Arc::new(RwLock::new(RdfNode::new(node_model_uuid, iri.to_owned())));
    let node_view = new_rdf_node_view(node_model.clone(), position);
    (node_model, node_view)
}
fn new_rdf_node_view(
    model: Arc<RwLock<RdfNode>>,
    position: egui::Pos2,
) -> Arc<RwLock<RdfNodeController>> {
    let m = model.read().unwrap();
    let node_view_uuid = uuid::Uuid::now_v7().into();
    let node_view = Arc::new(RwLock::new(RdfNodeController {
        uuid: Arc::new(node_view_uuid),
        model: model.clone(),
        self_reference: Weak::new(),
        iri_buffer: (*m.iri).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_radius: egui::Vec2::ZERO,
    }));
    node_view.write().unwrap().self_reference = Arc::downgrade(&node_view);
    node_view
}

pub struct RdfNodeController {
    uuid: Arc<ViewUuid>,
    pub model: Arc<RwLock<RdfNode>>,
    self_reference: Weak<RwLock<Self>>,

    iri_buffer: String,
    comment_buffer: String,

    dragged_shape: Option<NHShape>,
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_radius: egui::Vec2,
}

impl View for RdfNodeController {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().iri.clone()
    }
}

impl NHSerialize for RdfNodeController {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("rdf-node-view".to_owned()));
        element.insert("position".to_owned(), toml::Value::Array(vec![toml::Value::Float(self.position.x as f64), toml::Value::Float(self.position.y as f64)]));
        into.insert_view(*self.uuid, element);

        Ok(())
    }
}

impl ElementController<RdfElement> for RdfNodeController {
    fn model(&self) -> RdfElement {
        RdfTargettableElement::from(self.model.clone()).into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse {
            position: self.position,
            bounds_radius: self.bounds_radius,
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ContainerGen2<RdfDomain> for RdfNodeController {}

impl ElementControllerGen2<RdfDomain> for RdfNodeController {
    fn show_properties(
        &mut self,
        _: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Model properties");

        ui.label("IRI:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.iri_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::IriChange(Arc::new(self.iri_buffer.clone())),
            ]));
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position;

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.position.x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.position.y)));
            }
        });

        true
    }
    fn draw_in(
        &mut self,
        _: &RdfQueryable,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        let text_bounds = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().unwrap().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        self.bounds_radius = text_bounds.size() / 1.5;

        canvas.draw_ellipse(
            self.position,
            self.bounds_radius,
            context.profile.backgrounds[3],
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().unwrap().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            context.profile.foregrounds[3],
        );

        // Draw targetting ellipse
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_ellipse(
                self.position,
                self.bounds_radius,
                t.targetting_for_element(Some(self.model())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            TargettingStatus::Drawn
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::MouseDown(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled
                }
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_element(self.model());
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Drag { delta, .. } if self.dragged_shape.is_some() => {
                let translated_real_shape = self.dragged_shape.unwrap().translate(delta);
                self.dragged_shape = Some(translated_real_shape);
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| {
                        !ehc.all_elements
                            .get(e)
                            .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.position;

                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
                } else {
                    commands.push(
                        InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid).collect(),
                            coerced_delta,
                        )
                        .into(),
                    );
                }
                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _)
                if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddElement(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    for property in properties {
                        match property {
                            RdfPropChange::IriChange(iri) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::IriChange(model.iri.clone())],
                                ));
                                self.iri_buffer = (**iri).clone();
                                model.iri = iri.clone();
                            }
                            RdfPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::CommentChange(model.comment.clone())],
                                ));
                                self.comment_buffer = (**comment).clone();
                                model.comment = comment.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, RdfElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }
    
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, RdfElementView>,
        c: &mut HashMap<ViewUuid, RdfElementView>,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) {
        let old_model = self.model.read().unwrap();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(RdfElement::RdfTargettable(RdfTargettableElement::RdfNode(m))) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(RdfNode::new(model_uuid, (*old_model.iri).clone())));
            m.insert(*old_model.uuid, RdfTargettableElement::from(modelish.clone()).into());
            modelish
        };

        let cloneish = Arc::new(RwLock::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            self_reference: Weak::new(),
            iri_buffer: self.iri_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_radius: self.bounds_radius,
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_rdf_literal(
    content: &str,
    datatype: &str,
    langtag: &str,
    position: egui::Pos2,
) -> (
    Arc<RwLock<RdfLiteral>>,
    Arc<RwLock<RdfLiteralController>>,
) {
    let literal_model_uuid = uuid::Uuid::now_v7().into();
    let literal_model = Arc::new(RwLock::new(RdfLiteral::new(
        literal_model_uuid,
        content.to_owned(),
        datatype.to_owned(),
        langtag.to_owned(),
    )));
    let literal_view = new_rdf_literal_view(literal_model.clone(), position);
    (literal_model, literal_view)
}
fn new_rdf_literal_view(
    model: Arc<RwLock<RdfLiteral>>,
    position: egui::Pos2,
) -> Arc<RwLock<RdfLiteralController>> {
    let m = model.read().unwrap();
    let literal_view_uuid = uuid::Uuid::now_v7().into();
    let literal_view = Arc::new(RwLock::new(RdfLiteralController {
        uuid: Arc::new(literal_view_uuid),
        model: model.clone(),
        self_reference: Weak::new(),

        content_buffer: (*m.content).to_owned(),
        datatype_buffer: (*m.datatype).to_owned(),
        langtag_buffer: (*m.langtag).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_rect: egui::Rect::ZERO,
    }));
    literal_view.write().unwrap().self_reference = Arc::downgrade(&literal_view);
    literal_view
}

pub struct RdfLiteralController {
    uuid: Arc<ViewUuid>,
    pub model: Arc<RwLock<RdfLiteral>>,
    self_reference: Weak<RwLock<Self>>,

    content_buffer: String,
    datatype_buffer: String,
    langtag_buffer: String,
    comment_buffer: String,

    dragged_shape: Option<NHShape>,
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl View for RdfLiteralController {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().content.clone()
    }
}

impl NHSerialize for RdfLiteralController {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("rdf-literal-view".to_owned()));
        element.insert("position".to_owned(), toml::Value::Array(vec![toml::Value::Float(self.position.x as f64), toml::Value::Float(self.position.y as f64)]));
        into.insert_view(*self.uuid, element);

        Ok(())
    }
}

impl ElementController<RdfElement> for RdfLiteralController {
    fn model(&self) -> RdfElement {
        RdfTargettableElement::from(self.model.clone()).into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rect {
            inner: self.bounds_rect,
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ContainerGen2<RdfDomain> for RdfLiteralController {}

impl ElementControllerGen2<RdfDomain> for RdfLiteralController {
    fn show_properties(
        &mut self,
        _: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Model properties");

        ui.label("Content:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.content_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::ContentChange(Arc::new(self.content_buffer.clone())),
            ]));
        }
        ui.label("Datatype:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.datatype_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::DataTypeChange(Arc::new(self.datatype_buffer.clone())),
            ]));
        };

        ui.label("Language:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.langtag_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::LangTagChange(Arc::new(self.langtag_buffer.clone())),
            ]));
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position;

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.position.x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.position.y)));
            }
        });

        true
    }

    fn draw_in(
        &mut self,
        _: &RdfQueryable,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        self.bounds_rect = canvas.draw_class(
            self.position,
            None,
            &self.model.read().unwrap().content,
            None,
            &[],
            context.profile.backgrounds[4],
            canvas::Stroke::new_solid(1.0, context.profile.foregrounds[4]),
            self.highlight,
        );

        // Draw targetting rectangle
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                t.targetting_for_element(Some(self.model())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            TargettingStatus::Drawn
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::MouseDown(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_element(self.model());
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Drag { delta, .. } if self.dragged_shape.is_some() => {
                let translated_real_shape = self.dragged_shape.unwrap().translate(delta);
                self.dragged_shape = Some(translated_real_shape);
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| {
                        !ehc.all_elements
                            .get(e)
                            .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.position;

                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
                } else {
                    commands.push(
                        InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid).collect(),
                            coerced_delta,
                        )
                        .into(),
                    );
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _)
                if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddElement(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    for property in properties {
                        match property {
                            RdfPropChange::ContentChange(content) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::ContentChange(model.content.clone())],
                                ));
                                self.content_buffer = (**content).clone();
                                model.content = content.clone();
                            }
                            RdfPropChange::DataTypeChange(datatype) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::DataTypeChange(model.datatype.clone())],
                                ));
                                self.datatype_buffer = (**datatype).clone();
                                model.datatype = datatype.clone();
                            }
                            RdfPropChange::LangTagChange(langtag) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::LangTagChange(model.langtag.clone())],
                                ));
                                self.langtag_buffer = (**langtag).clone();
                                model.langtag = langtag.clone();
                            }
                            RdfPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::CommentChange(model.comment.clone())],
                                ));
                                self.comment_buffer = (**comment).clone();
                                model.comment = comment.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, RdfElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }
    
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, RdfElementView>,
        c: &mut HashMap<ViewUuid, RdfElementView>,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) {
        let old_model = self.model.read().unwrap();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(RdfElement::RdfTargettable(RdfTargettableElement::RdfLiteral(m))) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(RdfLiteral::new(model_uuid, (*old_model.content).clone(), (*old_model.datatype).clone(), (*old_model.langtag).clone())));
            m.insert(*old_model.uuid, RdfTargettableElement::from(modelish.clone()).into());
            modelish
        };

        let cloneish = Arc::new(RwLock::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            self_reference: Weak::new(),
            content_buffer: self.content_buffer.clone(),
            datatype_buffer: self.datatype_buffer.clone(),
            langtag_buffer: self.langtag_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

#[derive(Clone)]
pub struct RdfPredicateAdapter {
    model: Arc<RwLock<RdfPredicate>>,
}

impl MulticonnectionAdapter<RdfDomain> for RdfPredicateAdapter {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().iri.clone()
    }

    fn view_type(&self) -> &'static str {
        "rdf-predicate-view"
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.model_name())
    }

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        (canvas::LineType::Solid, canvas::ArrowheadType::None, None)
    }

    fn destination_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        (canvas::LineType::Solid, canvas::ArrowheadType::OpenTriangle, None)
    }

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>
    ) {
        let model = self.model.read().unwrap();
        let mut iri_buffer = (*model.iri).clone();
        ui.label("IRI:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut iri_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::IriChange(Arc::new(iri_buffer)),
            ]));
        }

        let mut comment_buffer = (*model.comment).clone();
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(comment_buffer)),
            ]));
        }

        if ui.button("Switch source and destination").clicked()
            && /* TODO: must check if target isn't a literal */ true
        {
            // TODO: (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            // TODO: (self.source, self.destination) = (self.destination.clone(), self.source.clone());

            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::FlipMulticonnection,
            ]));
        }
    }

    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    RdfPropChange::IriChange(iri) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::IriChange(model.iri.clone())],
                        ));
                        model.iri = iri.clone();
                    }
                    RdfPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::CommentChange(model.comment.clone())],
                        ));
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, RdfElement>
    ) -> Self where Self: Sized {
        let old_model = self.model.read().unwrap();

        let model = if let Some(RdfElement::RdfPredicate(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(RdfPredicate::new(new_uuid, (*old_model.iri).clone(), old_model.source.clone(), old_model.target.clone())));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, RdfElement>,
    ) {
        let mut model = self.model.write().unwrap();
        
        let source_uuid = *model.source.read().unwrap().uuid;
        if let Some(RdfElement::RdfTargettable(RdfTargettableElement::RdfNode(new_source))) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }

        let target_uuid = *model.target.uuid();
        if let Some(RdfElement::RdfTargettable(new_target)) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}

fn new_rdf_predicate(
    iri: &str,
    source: (Arc<RwLock<RdfNode>>, RdfElementView),
    target: (RdfTargettableElement, RdfElementView),
) -> (
    Arc<RwLock<RdfPredicate>>,
    Arc<RwLock<LinkViewT>>,
) {
    let predicate_model_uuid = uuid::Uuid::now_v7().into();
    let predicate_model = Arc::new(RwLock::new(RdfPredicate::new(
        predicate_model_uuid,
        iri.to_owned(),
        source.0,
        target.0,
    )));
    let predicate_view = new_rdf_predicate_view(
        predicate_model.clone(),
        source.1,
        target.1
    );

    (predicate_model, predicate_view)
}
fn new_rdf_predicate_view(
    model: Arc<RwLock<RdfPredicate>>,
    source: RdfElementView,
    target: RdfElementView,
) -> Arc<RwLock<LinkViewT>> {
    let predicate_view_uuid = uuid::Uuid::now_v7().into();
    let predicate_view = MulticonnectionView::new(
        Arc::new(predicate_view_uuid),
        RdfPredicateAdapter {
            model,
        },
        source,
        target,
        None,
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
    );
    predicate_view
}
