use super::rdf_models::{RdfDiagram, RdfElement, RdfGraph, RdfLiteral, RdfNode, RdfPredicate, RdfTargettableElement};
use crate::common::canvas::{self, Highlight, NHCanvas, NHShape};
use crate::common::controller::{
    ColorBundle, ColorChangeData, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, InputEvent, InsensitiveCommand, MGlobalColor, Model, ModelsLabelAcquirer, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SensitiveCommand, SimpleModelHierarchyView, SnapManager, TargettingStatus, Tool, View
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{ArrowData, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::{CustomTab, CustomModal, CustomModalResult};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

use sophia::api::{prelude::SparqlDataset, sparql::Query};
use sophia_sparql::{ResultTerm, SparqlQuery, SparqlWrapper};

pub struct RdfDomain;
impl Domain for RdfDomain {
    type CommonElementT = RdfElement;
    type DiagramModelT = RdfDiagram;
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

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    FlipMulticonnection(FlipMulticonnection),
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

                Self::ColorChange(color) => format!("ColorChange(..)"),
                Self::CommentChange(comment) => format!("CommentChange({})", comment),
                Self::FlipMulticonnection(_) => format!("FlipMulticonnection"),
            }
        )
    }
}

impl TryFrom<&RdfPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &RdfPropChange) -> Result<Self, Self::Error> {
        match value {
            RdfPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for RdfPropChange {
    fn from(value: ColorChangeData) -> Self {
        RdfPropChange::ColorChange(value)
    }
}
impl TryFrom<RdfPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: RdfPropChange) -> Result<Self, Self::Error> {
        match value {
            RdfPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From, derive_more::TryInto)]
pub enum RdfElementOrVertex {
    Element(RdfElementView),
    Vertex(VertexInformation),
}

impl Debug for RdfElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "RdfElementOrVertex::???")
    }
}

#[derive(Clone, derive_more::From, nh_derive::NHContextSerDeTag)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum RdfElementView {
    Graph(ERef<PackageViewT>),
    Literal(ERef<RdfLiteralView>),
    Node(ERef<RdfNodeView>),
    Predicate(ERef<LinkViewT>),
}

impl Entity for RdfElementView {
    fn tagged_uuid(&self) -> EntityUuid {
        match self {
            Self::Graph(inner) => inner.read().tagged_uuid(),
            Self::Literal(inner) => inner.read().tagged_uuid(),
            Self::Node(inner) => inner.read().tagged_uuid(),
            Self::Predicate(inner) => inner.read().tagged_uuid(),
        }
    }
}

impl View for RdfElementView {
    fn uuid(&self) -> Arc<ViewUuid> {
        match self {
            Self::Graph(inner) => inner.read().uuid(),
            Self::Literal(inner) => inner.read().uuid(),
            Self::Node(inner) => inner.read().uuid(),
            Self::Predicate(inner) => inner.read().uuid(),
        }
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        match self {
            Self::Graph(inner) => inner.read().model_uuid(),
            Self::Literal(inner) => inner.read().model_uuid(),
            Self::Node(inner) => inner.read().model_uuid(),
            Self::Predicate(inner) => inner.read().model_uuid(),
        }
    }
}
impl ElementController<RdfElement> for RdfElementView {
    fn model(&self) -> RdfElement {
        match self {
            RdfElementView::Graph(inner) => inner.read().model(),
            RdfElementView::Literal(inner) => inner.read().model(),
            RdfElementView::Node(inner) => inner.read().model(),
            RdfElementView::Predicate(inner) => inner.read().model(),
        }
    }
    fn min_shape(&self) -> NHShape {
        match self {
            RdfElementView::Graph(inner) => inner.read().min_shape(),
            RdfElementView::Literal(inner) => inner.read().min_shape(),
            RdfElementView::Node(inner) => inner.read().min_shape(),
            RdfElementView::Predicate(inner) => inner.read().min_shape(),
        }
    }
    fn max_shape(&self) -> NHShape {
        match self {
            RdfElementView::Graph(inner) => inner.read().max_shape(),
            RdfElementView::Literal(inner) => inner.read().max_shape(),
            RdfElementView::Node(inner) => inner.read().max_shape(),
            RdfElementView::Predicate(inner) => inner.read().max_shape(),
        }
    }
    fn position(&self) -> egui::Pos2 {
        match self {
            RdfElementView::Graph(inner) => inner.read().position(),
            RdfElementView::Literal(inner) => inner.read().position(),
            RdfElementView::Node(inner) => inner.read().position(),
            RdfElementView::Predicate(inner) => inner.read().position(),
        }
    }
}
impl ContainerGen2<RdfDomain> for RdfElementView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<<RdfDomain as Domain>::CommonElementViewT> {
        match self {
            RdfElementView::Graph(inner) => inner.read().controller_for(uuid),
            RdfElementView::Literal(inner) => inner.read().controller_for(uuid),
            RdfElementView::Node(inner) => inner.read().controller_for(uuid),
            RdfElementView::Predicate(inner) => inner.read().controller_for(uuid),
        }
    }
}
impl ElementControllerGen2<RdfDomain> for RdfElementView {
    fn show_properties(
        &mut self,
        drawing_context: &DrawingContext,
        q: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> PropertiesStatus {
        match self {
            RdfElementView::Graph(inner) => inner.write().show_properties(drawing_context, q, ui, commands),
            RdfElementView::Literal(inner) => inner.write().show_properties(drawing_context, q, ui, commands),
            RdfElementView::Node(inner) => inner.write().show_properties(drawing_context, q, ui, commands),
            RdfElementView::Predicate(inner) => inner.write().show_properties(drawing_context, q, ui, commands),
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
            RdfElementView::Graph(inner) => inner.write().draw_in(q, context, canvas, tool),
            RdfElementView::Literal(inner) => inner.write().draw_in(q, context, canvas, tool),
            RdfElementView::Node(inner) => inner.write().draw_in(q, context, canvas, tool),
            RdfElementView::Predicate(inner) => inner.write().draw_in(q, context, canvas, tool),
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        match self {
            RdfElementView::Graph(inner) => inner.write().collect_allignment(am),
            RdfElementView::Literal(inner) => inner.write().collect_allignment(am),
            RdfElementView::Node(inner) => inner.write().collect_allignment(am),
            RdfElementView::Predicate(inner) => inner.write().collect_allignment(am),
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveRdfTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> EventHandlingStatus {
        match self {
            RdfElementView::Graph(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
            RdfElementView::Literal(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
            RdfElementView::Node(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
            RdfElementView::Predicate(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
        }
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match self {
            RdfElementView::Graph(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            RdfElementView::Literal(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            RdfElementView::Node(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            RdfElementView::Predicate(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
        }
    }
    fn refresh_buffers(&mut self) {
        match self {
            RdfElementView::Graph(inner) => inner.write().refresh_buffers(),
            RdfElementView::Literal(inner) => inner.write().refresh_buffers(),
            RdfElementView::Node(inner) => inner.write().refresh_buffers(),
            RdfElementView::Predicate(inner) => inner.write().refresh_buffers(),
        }
    }
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, RdfElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        match self {
            RdfElementView::Graph(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            RdfElementView::Literal(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            RdfElementView::Node(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            RdfElementView::Predicate(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        match self {
            RdfElementView::Graph(inner) => inner.read().delete_when(deleting),
            RdfElementView::Literal(inner) => inner.read().delete_when(deleting),
            RdfElementView::Node(inner) => inner.read().delete_when(deleting),
            RdfElementView::Predicate(inner) => inner.read().delete_when(deleting),
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
            RdfElementView::Graph(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            RdfElementView::Literal(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            RdfElementView::Node(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            RdfElementView::Predicate(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
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
            RdfElementView::Graph(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            RdfElementView::Literal(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            RdfElementView::Node(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            RdfElementView::Predicate(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, RdfElementView>,
        m: &HashMap<ModelUuid, RdfElement>,
    ) {
        match self {
            RdfElementView::Graph(inner) => inner.write().deep_copy_relink(c, m),
            RdfElementView::Literal(inner) => inner.write().deep_copy_relink(c, m),
            RdfElementView::Node(inner) => inner.write().deep_copy_relink(c, m),
            RdfElementView::Predicate(inner) => inner.write().deep_copy_relink(c, m),
        }
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct RdfDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: RdfDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    placeholders: RdfPlaceholderViews,
}

#[derive(Clone, Default)]
struct RdfDiagramBuffer {
    name: String,
    comment: String,
}

#[derive(Clone)]
struct RdfPlaceholderViews {
    views: [RdfElementView; 4],
}

impl Default for RdfPlaceholderViews {
    fn default() -> Self {
        let (literal, literal_view) = new_rdf_literal("Eric Miller", "http://www.w3.org/2001/XMLSchema#string", "en", egui::Pos2::new(100.0, 75.0));
        let literal = (literal.into(), literal_view.into());
        let (node, node_view) = new_rdf_node("http://iri", egui::Pos2::ZERO);
        let node = (node, node_view.into());
        let (predicate, predicate_view) = new_rdf_predicate("http://iri", node.clone(), literal.clone());

        let (_graph, graph_view) = new_rdf_graph("http://graph", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });

        Self {
            views: [
                literal.1,
                node.1,
                predicate_view.into(),
                graph_view.into(),
            ],
        }
    }
}

struct RdfLabelAcquirer;
impl ModelsLabelAcquirer for RdfLabelAcquirer {
    type ModelT = RdfDiagram;

    fn model_label(&self, m: &Self::ModelT) -> String {
        format!("{} ({} children)", m.name, m.contained_elements.len())
    }

    fn element_label(&self, e: &<Self::ModelT as ContainerModel>::ElementT) -> String {
        match e {
            RdfElement::RdfGraph(inner) => (*inner.read().iri).clone(),
            RdfElement::RdfLiteral(inner) => (*inner.read().content).clone(),
            RdfElement::RdfNode(inner) => (*inner.read().iri).clone(),
            RdfElement::RdfPredicate(inner) => (*inner.read().iri).clone(),
        }
    }
}

impl RdfDiagramAdapter {
    fn new(model: ERef<RdfDiagram>) -> Self {
        let m = model.read();
         Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: RdfDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
            placeholders: Default::default(),
        }
    }
}

impl DiagramAdapter<RdfDomain> for RdfDiagramAdapter {
    fn model(&self) -> ERef<RdfDiagram> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }
    fn view_type(&self) -> &'static str {
        "rdf-diagram-view"
    }
    fn new_hierarchy_view(&self) -> SimpleModelHierarchyView<impl ModelsLabelAcquirer<ModelT = RdfDiagram> + 'static> {
        SimpleModelHierarchyView::new(self.model(), RdfLabelAcquirer {})
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
            RdfElement::RdfLiteral(rw_lock) => {
                RdfElementView::from(
                    new_rdf_literal_view(rw_lock, egui::Pos2::ZERO)
                )
            },
            RdfElement::RdfNode(rw_lock) => {
                RdfElementView::from(
                    new_rdf_node_view(rw_lock, egui::Pos2::ZERO)
                )
            },
            RdfElement::RdfPredicate(rw_lock) => {
                let m = rw_lock.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
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

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32 {
        global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE)
    }
    fn gridlines_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::from_rgb(220, 220, 220)
    }
    fn show_view_props_fun(
        &mut self,
        drawing_context: &DrawingContext,
        ui: &mut egui::Ui,
    ) -> PropertiesStatus {
        ui.label("Background color:");
        if crate::common::controller::mglobalcolor_edit_button(
            &drawing_context.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        PropertiesStatus::Shown
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
                egui::TextEdit::singleline(&mut self.buffer.name),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![RdfPropChange::NameChange(Arc::new(self.buffer.name.clone()))],
                )
                .into(),
            );
        };

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.buffer.comment),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![RdfPropChange::CommentChange(Arc::new(
                        self.buffer.comment.clone(),
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
            let mut model = self.model.write();
            for property in properties {
                match property {
                    RdfPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    RdfPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![RdfPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                        ));
                        self.background_color = *color;
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.buffer.name = (*model.name).clone();
        self.buffer.comment = (*model.comment).clone();
    }

    fn show_tool_palette(
        &mut self,
        tool: &mut Option<NaiveRdfTool>,
        drawing_context: &DrawingContext,
        ui: &mut egui::Ui,
    ) {
        let button_height = 60.0;
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
                [width, button_height],
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

        let (empty_a, empty_b) = (HashMap::new(), HashMap::new());
        let empty_q = RdfQueryable::new(&empty_a, &empty_b);
        let mut icon_counter = 0;
        for cat in [
            &[
                (RdfToolStage::Literal, "Literal"),
                (RdfToolStage::Node, "Node"),
                (RdfToolStage::PredicateStart, "Predicate"),
            ][..],
            &[(RdfToolStage::GraphStart, "Graph")][..],
        ] {
            for (stage, name) in cat {
                let response = ui.add_sized([width, button_height], egui::Button::new(*name).fill(c(*stage)));
                if response.clicked() {
                    if let Some(t) = &tool && t.initial_stage == *stage {
                        *tool = None;
                    } else {
                        *tool = Some(NaiveRdfTool::new(*stage));
                    }
                }

                let icon_rect = egui::Rect::from_min_size(response.rect.min, egui::Vec2::splat(button_height));
                let painter = ui.painter().with_clip_rect(icon_rect);
                let mut mc = canvas::MeasuringCanvas::new(&painter);
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut mc, &None);
                let (scale, offset) = mc.scale_offset_to_fit(egui::Vec2::new(button_height, button_height));
                let mut c = canvas::UiCanvas::new(false, painter, icon_rect, offset, scale, None, Highlight::NONE);
                c.clear(egui::Color32::WHITE.gamma_multiply(0.75));
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut c, &None);
                icon_counter += 1;
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
        let (new_model, models) = super::rdf_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, RdfElement>) {
        let models = super::rdf_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }
}

struct SparqlQueriesTab {
    diagram: ERef<RdfDiagram>,
    selected_query: Option<uuid::Uuid>,
    query_name_buffer: String,
    query_value_buffer: String,
    debug_message: Option<String>,
    query_results: Option<Vec<Vec<Option<ResultTerm>>>>,
}

impl SparqlQueriesTab {
    fn save(&mut self) {
        let mut model = self.diagram.write();

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
        let model = self.diagram.write();

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
        let mut model = self.diagram.write();

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

pub fn new(no: u32) -> ERef<dyn DiagramController> {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New RDF diagram {}", no);

    let diagram = ERef::new(RdfDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        RdfDiagramAdapter::new(diagram.clone()),
        Vec::new(),
    )
}

pub fn demo(no: u32) -> ERef<dyn DiagramController> {
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
            models_st.push(node_st.into());
            controllers_st.push(node_st_view.into());
        }
    }

    for xx in 3000..=3100 {
        for yy in 3000..=3100 {
            let (node_st, node_st_view) = new_rdf_node(
                "http://www.w3.org/People/EM/contact#me",
                egui::Pos2::new(xx as f32, yy as f32),
            );
            models_st.push(node_st.into());
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
    let diagram = ERef::new(RdfDiagram::new(
        model_uuid,
        name.clone(),
        vec![
            node.into(), literal_model.into(),
            predicate.into(), graph.into(), graph_st.into(),
        ],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        RdfDiagramAdapter::new(diagram.clone()),
        owned_controllers,
    )
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<DiagramControllerGen2<RdfDomain, RdfDiagramAdapter>>(&uuid)?)
}

#[derive(Clone, Copy, PartialEq)]
pub enum RdfToolStage {
    Literal,
    Node,
    PredicateStart,
    PredicateEnd,
    GraphStart,
    GraphEnd,
}

enum PartialRdfElement {
    None,
    Some(RdfElementView),
    Predicate {
        source: ERef<RdfNode>,
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
                | RdfToolStage::GraphEnd => TARGETTABLE_COLOR,
                RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfGraph(..)) => match self.current_stage {
                RdfToolStage::Literal | RdfToolStage::Node => {
                    TARGETTABLE_COLOR
                }
                RdfToolStage::PredicateStart
                | RdfToolStage::PredicateEnd
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfLiteral(..)) => match self.current_stage {
                RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::PredicateStart
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfNode(..)) => match self.current_stage {
                RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfPredicate(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &RdfQueryable, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialRdfElement::Predicate { source, .. } => {
                if let Some(source_view) = q.get_view(&source.read().uuid()) {
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
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: RdfElement) {
        if self.event_lock {
            return;
        }

        match controller {
            RdfElement::RdfGraph(..) => {}
            RdfElement::RdfLiteral(inner) => match (self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { dest, .. }) => {
                    *dest = Some(RdfTargettableElement::from(inner));
                    self.event_lock = true;
                }
                _ => {}
            },
            RdfElement::RdfNode(inner) => match (self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateStart, PartialRdfElement::None) => {
                    self.result = PartialRdfElement::Predicate {
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = RdfToolStage::PredicateEnd;
                    self.event_lock = true;
                }
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { dest, .. }) => {
                    *dest = Some(RdfTargettableElement::from(inner));
                }
                _ => {}
            },
            RdfElement::RdfPredicate(..) => {}
        }
    }

    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<RdfDomain>,
    ) -> Option<(RdfElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialRdfElement::Some(x) => {
                let x = x.clone();
                self.result = PartialRdfElement::None;
                let esm: Option<Box<dyn CustomModal>> = match &x {
                    RdfElementView::Literal(eref) => {
                        Some(Box::new(RdfLiteralSetupModal::from(&eref.read().model)))
                    },
                    RdfElementView::Node(eref) => {
                        Some(Box::new(RdfIriBasedSetupModal::from(RdfElement::from(eref.read().model.clone()))))
                    },
                    RdfElementView::Predicate(..)
                    | RdfElementView::Graph(..) => unreachable!(),
                };
                Some((x, esm))
            }
            // TODO: check for source == dest case, set points?
            PartialRdfElement::Predicate {
                source,
                dest: Some(dest),
                ..
            } => {
                self.current_stage = RdfToolStage::PredicateStart;

                let predicate_view: Option<(_, Option<Box<dyn CustomModal>>)> =
                    if let (Some(source_controller), Some(dest_controller)) = (
                        into.controller_for(&source.read().uuid()),
                        into.controller_for(&dest.uuid()),
                    ) {
                        let (predicate_model, predicate_view) = new_rdf_predicate(
                            "http://www.w3.org/2000/10/swap/pim/contact#fullName",
                            (source.clone(), source_controller),
                            (dest.clone(), dest_controller),
                        );

                        Some((predicate_view.into(), Some(Box::new(RdfIriBasedSetupModal::from(RdfElement::from(predicate_model))))))
                    } else {
                        None
                    };

                self.result = PartialRdfElement::None;
                predicate_view
            }
            PartialRdfElement::Graph { a, b: Some(b) } => {
                self.current_stage = RdfToolStage::GraphStart;

                let (graph_model, graph_view) =
                    new_rdf_graph("http://a-graph", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialRdfElement::None;
                Some((graph_view.into(), Some(Box::new(RdfIriBasedSetupModal::from(RdfElement::from(graph_model))))))
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}


struct RdfIriBasedSetupModal {
    model: RdfElement,
    iri_buffer: String,
}

impl From<RdfElement> for RdfIriBasedSetupModal {
    fn from(model: RdfElement) -> Self {
        let iri_buffer = match &model {
            RdfElement::RdfGraph(eref) => (*eref.read().iri).clone(),
            RdfElement::RdfNode(eref) => (*eref.read().iri).clone(),
            RdfElement::RdfPredicate(eref) => (*eref.read().iri).clone(),
            RdfElement::RdfLiteral(..) => unreachable!(),
        };
        Self {
            model,
            iri_buffer,
        }
    }
}

impl CustomModal for RdfIriBasedSetupModal {
    fn show(
        &mut self,
        d: &mut DrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("IRI:");
        ui.text_edit_singleline(&mut self.iri_buffer);
        ui.separator();

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let iri = Arc::new(self.iri_buffer.clone());
                match &self.model {
                    RdfElement::RdfGraph(eref) => eref.write().iri = iri,
                    RdfElement::RdfNode(eref) => eref.write().iri = iri,
                    RdfElement::RdfPredicate(eref) => eref.write().iri = iri,
                    RdfElement::RdfLiteral(eref) => unreachable!(),
                }
                result = CustomModalResult::CloseModified(*self.model.uuid());
            }
            if ui.button("Cancel").clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct RdfGraphAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfGraph>,
    #[nh_context_serde(skip_and_default)]
    iri_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<RdfDomain> for RdfGraphAdapter {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().iri.clone()
    }

    fn add_element(&mut self, element: RdfElement) {
        self.model.write().add_element(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.model.write().delete_elements(uuids);
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>
    ) {
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
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.iri_buffer = (*model.iri).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(RdfElement::RdfGraph(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(RdfGraph::new(new_uuid, (*old_model.iri).clone(), old_model.contained_elements.clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };
        Self { model, iri_buffer: self.iri_buffer.clone(), comment_buffer: self.comment_buffer.clone() }
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
) -> (ERef<RdfGraph>, ERef<PackageViewT>) {
    let graph_model = ERef::new(RdfGraph::new(
        uuid::Uuid::now_v7().into(),
        iri.to_owned(),
        Vec::new(),
    ));
    let graph_view = new_rdf_graph_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_rdf_graph_view(
    model: ERef<RdfGraph>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT> {
    let m = model.read();
    PackageView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        RdfGraphAdapter {
            model: model.clone(),
            iri_buffer: (*m.iri).clone(),
            comment_buffer: (*m.comment).clone()
        },
        Vec::new(),
        bounds_rect,
    )
}

fn new_rdf_node(
    iri: &str,
    position: egui::Pos2,
) -> (ERef<RdfNode>, ERef<RdfNodeView>) {
    let node_model = ERef::new(RdfNode::new(uuid::Uuid::now_v7().into(), iri.to_owned()));
    let node_view = new_rdf_node_view(node_model.clone(), position);
    (node_model, node_view)
}
fn new_rdf_node_view(
    model: ERef<RdfNode>,
    position: egui::Pos2,
) -> ERef<RdfNodeView> {
    let m = model.read();
    let node_view = ERef::new(RdfNodeView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),

        iri_buffer: (*m.iri).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_radius: egui::Vec2::ZERO,
    });
    node_view
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<RdfNode>,

    #[nh_context_serde(skip_and_default)]
    iri_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    #[nh_context_serde(skip_and_default)]
    pub bounds_radius: egui::Vec2,
}

impl Entity for RdfNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for RdfNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<RdfElement> for RdfNodeView {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
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

impl ContainerGen2<RdfDomain> for RdfNodeView {}

impl ElementControllerGen2<RdfDomain> for RdfNodeView {
    fn show_properties(
        &mut self,
        drawing_context: &DrawingContext,
        _: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> PropertiesStatus {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
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

        PropertiesStatus::Shown
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
            &self.model.read().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        self.bounds_radius = text_bounds.size() / 1.5;

        canvas.draw_ellipse(
            self.position,
            self.bounds_radius,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
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
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
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
        affected_models: &mut HashSet<ModelUuid>,
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
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            RdfPropChange::IriChange(iri) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::IriChange(model.iri.clone())],
                                ));
                                model.iri = iri.clone();
                            }
                            RdfPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::CommentChange(model.comment.clone())],
                                ));
                                model.comment = comment.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.iri_buffer = (*model.iri).clone();
        self.comment_buffer = (*model.comment).clone();
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
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(RdfElement::RdfNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(RdfNode::new(model_uuid, (*old_model.iri).clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            iri_buffer: self.iri_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_radius: self.bounds_radius,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_rdf_literal(
    content: &str,
    datatype: &str,
    langtag: &str,
    position: egui::Pos2,
) -> (ERef<RdfLiteral>, ERef<RdfLiteralView>) {
    let literal_model_uuid = uuid::Uuid::now_v7().into();
    let literal_model = ERef::new(RdfLiteral::new(
        literal_model_uuid,
        content.to_owned(),
        datatype.to_owned(),
        langtag.to_owned(),
    ));
    let literal_view = new_rdf_literal_view(literal_model.clone(), position);
    (literal_model, literal_view)
}
fn new_rdf_literal_view(
    model: ERef<RdfLiteral>,
    position: egui::Pos2,
) -> ERef<RdfLiteralView> {
    let m = model.read();
    let literal_view_uuid = uuid::Uuid::now_v7().into();
    let literal_view = ERef::new(RdfLiteralView {
        uuid: Arc::new(literal_view_uuid),
        model: model.clone(),

        content_buffer: (*m.content).to_owned(),
        datatype_buffer: (*m.datatype).to_owned(),
        langtag_buffer: (*m.langtag).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_rect: egui::Rect::ZERO,
    });
    literal_view
}

struct RdfLiteralSetupModal {
    model: ERef<RdfLiteral>,

    content_buffer: String,
    datatype_buffer: String,
    langtag_buffer: String,
}

impl From<&ERef<RdfLiteral>> for RdfLiteralSetupModal {
    fn from(model: &ERef<RdfLiteral>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            content_buffer: (*m.content).clone(),
            datatype_buffer: (*m.datatype).clone(),
            langtag_buffer: (*m.langtag).clone(),
        }
    }
}

impl CustomModal for RdfLiteralSetupModal {
    fn show(
        &mut self,
        d: &mut DrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Content:");
        ui.text_edit_singleline(&mut self.content_buffer);
        ui.label("Datatype:");
        ui.text_edit_singleline(&mut self.datatype_buffer);
        ui.label("Langtag:");
        ui.text_edit_singleline(&mut self.langtag_buffer);
        ui.separator();

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.content = Arc::new(self.content_buffer.clone());
                m.datatype = Arc::new(self.datatype_buffer.clone());
                m.langtag = Arc::new(self.langtag_buffer.clone());
                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button("Cancel").clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfLiteralView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<RdfLiteral>,

    #[nh_context_serde(skip_and_default)]
    content_buffer: String,
    #[nh_context_serde(skip_and_default)]
    datatype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    langtag_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl Entity for RdfLiteralView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for RdfLiteralView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<RdfElement> for RdfLiteralView {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
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

impl ContainerGen2<RdfDomain> for RdfLiteralView {}

impl ElementControllerGen2<RdfDomain> for RdfLiteralView {
    fn show_properties(
        &mut self,
        drawing_context: &DrawingContext,
        _: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> PropertiesStatus {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
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

        PropertiesStatus::Shown
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
            &self.model.read().content,
            None,
            &[],
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
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
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
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
        affected_models: &mut HashSet<ModelUuid>,
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
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            RdfPropChange::ContentChange(content) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::ContentChange(model.content.clone())],
                                ));
                                model.content = content.clone();
                            }
                            RdfPropChange::DataTypeChange(datatype) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::DataTypeChange(model.datatype.clone())],
                                ));
                                model.datatype = datatype.clone();
                            }
                            RdfPropChange::LangTagChange(langtag) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::LangTagChange(model.langtag.clone())],
                                ));
                                model.langtag = langtag.clone();
                            }
                            RdfPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![RdfPropChange::CommentChange(model.comment.clone())],
                                ));
                                model.comment = comment.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.content_buffer = (*model.content).clone();
        self.datatype_buffer = (*model.datatype).clone();
        self.langtag_buffer = (*model.langtag).clone();
        self.comment_buffer = (*model.comment).clone();
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
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(RdfElement::RdfLiteral(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(RdfLiteral::new(model_uuid, (*old_model.content).clone(), (*old_model.datatype).clone(), (*old_model.langtag).clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            content_buffer: self.content_buffer.clone(),
            datatype_buffer: self.datatype_buffer.clone(),
            langtag_buffer: self.langtag_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct RdfPredicateAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfPredicate>,
    #[nh_context_serde(skip_and_default)]
    iri_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl MulticonnectionAdapter<RdfDomain> for RdfPredicateAdapter {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        self.model.read().iri.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.model_name())
    }

    fn source_arrow(&self) -> ArrowData {
        ArrowData::new_labelless(
            canvas::LineType::Solid,
            canvas::ArrowheadType::None,
        )
    }

    fn destination_arrow(&self) -> ArrowData {
        ArrowData::new_labelless(
            canvas::LineType::Solid,
            canvas::ArrowheadType::OpenTriangle,
        )
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>
    ) {
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

        if ui.button("Switch source and destination").clicked()
            && let RdfTargettableElement::RdfNode(_) = &self.model.read().target {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::FlipMulticonnection(FlipMulticonnection {}),
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
            let mut model = self.model.write();
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
                    RdfPropChange::FlipMulticonnection(_) => {
                        if let RdfTargettableElement::RdfNode(t) = &model.target {
                            let tmp = model.source.clone();
                            model.source = t.clone();
                            model.target = tmp.into();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.iri_buffer = (*model.iri).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, RdfElement>
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(RdfElement::RdfPredicate(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(RdfPredicate::new(new_uuid, (*old_model.iri).clone(), old_model.source.clone(), old_model.target.clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self { model, iri_buffer: self.iri_buffer.clone(), comment_buffer: self.comment_buffer.clone() }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, RdfElement>,
    ) {
        let mut model = self.model.write();
        
        let source_uuid = *model.source.read().uuid();
        if let Some(RdfElement::RdfNode(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone().into();
        }

        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_targettable_element()) {
            model.target = new_target;
        }
    }
}

fn new_rdf_predicate(
    iri: &str,
    source: (ERef<RdfNode>, RdfElementView),
    target: (RdfTargettableElement, RdfElementView),
) -> (ERef<RdfPredicate>, ERef<LinkViewT>) {
    let predicate_model = ERef::new(RdfPredicate::new(
        uuid::Uuid::now_v7().into(),
        iri.to_owned(),
        source.0,
        target.0,
    ));
    let predicate_view = new_rdf_predicate_view(
        predicate_model.clone(),
        source.1,
        target.1
    );

    (predicate_model, predicate_view)
}
fn new_rdf_predicate_view(
    model: ERef<RdfPredicate>,
    source: RdfElementView,
    target: RdfElementView,
) -> ERef<LinkViewT> {
    let m = model.read();

    let (sp, mp, tp) = if source.model_uuid() == target.model_uuid() {
        let s = source.min_shape();
        let (min, quarter_size) = match s {
            NHShape::Rect { inner } => (inner.min, inner.size() / 4.0),
            NHShape::Ellipse { position, bounds_radius } => (position - bounds_radius, bounds_radius / 2.0),
        };

        (
            vec![vec![
                (uuid::Uuid::now_v7().into(), egui::Pos2::ZERO),
                (uuid::Uuid::now_v7().into(), min + egui::Vec2::new(quarter_size.x, -quarter_size.y)),
            ]],
            Some((uuid::Uuid::now_v7().into(), min - quarter_size)),
            vec![vec![
                (uuid::Uuid::now_v7().into(), egui::Pos2::ZERO),
                (uuid::Uuid::now_v7().into(), min + egui::Vec2::new(-quarter_size.x, quarter_size.y)),
            ]],
        )
    } else {
        (
            vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
            None,
            vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        )
    };

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        RdfPredicateAdapter {
            model: model.clone(),
            iri_buffer: (*m.iri).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        source,
        target,
        mp,
        sp,
        tp,
    )
}
