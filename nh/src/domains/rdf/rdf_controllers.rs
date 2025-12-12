use super::rdf_models::{RdfDiagram, RdfElement, RdfGraph, RdfLiteral, RdfNode, RdfPredicate, RdfTargettableElement};
use crate::common::canvas::{self, Highlight, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GlobalDrawingContext, InputEvent, InsensitiveCommand, MGlobalColor, Model, PositionNoT, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, View
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{self, ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::{CustomModal, CustomModalResult};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct RdfDomain;
impl Domain for RdfDomain {
    type CommonElementT = RdfElement;
    type DiagramModelT = RdfDiagram;
    type CommonElementViewT = RdfElementView;
    type ViewTargettingSectionT = RdfElement;
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

                Self::ColorChange(_color) => format!("ColorChange(..)"),
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

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "RdfDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum RdfElementView {
    Graph(ERef<PackageViewT>),
    Literal(ERef<RdfLiteralView>),
    Node(ERef<RdfNodeView>),
    Predicate(ERef<LinkViewT>),
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
        let (_predicate, predicate_view) = new_rdf_predicate("http://iri", node.clone(), literal.clone());

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
    fn label_for(&self, e: &RdfElement) -> Arc<String> {
        match e {
            RdfElement::RdfGraph(inner) => {
                inner.read().iri.clone()
            },
            RdfElement::RdfLiteral(inner) => {
                inner.read().content.clone()
            },
            RdfElement::RdfNode(inner) => {
                inner.read().iri.clone()
            },
            RdfElement::RdfPredicate(inner) => {
                inner.read().iri.clone()
            },
        }
    }

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32 {
        global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE)
    }
    fn gridlines_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::from_rgb(220, 220, 220)
    }
    fn show_view_props_fun(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> PropertiesStatus<RdfDomain> {
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
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) {
        let button_height = drawing_context.tool_palette_item_height as f32;
        let width = ui.available_width();
        let selected_background_color = if ui.style().visuals.dark_mode {
            egui::Color32::BLUE
        } else {
            egui::Color32::LIGHT_BLUE
        };
        let button_background_color = ui.style().visuals.extreme_bg_color;

        let stage = tool.as_ref().map(|e| e.initial_stage());
        let c = |s: RdfToolStage| -> egui::Color32 {
            if stage.is_some_and(|e| e == s) {
                selected_background_color
            } else {
                button_background_color
            }
        };

        if ui
            .add_sized(
                [width, button_height],
                egui::Button::new("Select/Move").fill(if stage == None {
                    selected_background_color
                } else {
                    button_background_color
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
                c.clear(egui::Color32::GRAY);
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut c, &None);
                icon_counter += 1;
            }
            ui.separator();
        }
    }

    fn menubar_options_fun(
        &self,
        _view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        // TODO: re-enable when sophia's getrandom dependency gets updated
        #[cfg(not(target_arch = "wasm32"))]
        {
            if ui.button("Import RDF data").clicked() {
                // TODO: import stuff
            }
            if ui.button("SPARQL Queries").clicked() {
                let uuid = uuid::Uuid::now_v7();
                commands.push(ProjectCommand::AddCustomTab(
                    uuid,
                    Arc::new(RwLock::new(super::rdf_queries::SparqlQueriesTab::new(self.model.clone()))),
                ));
            }
            ui.separator();
        }
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

    fn transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::rdf_models::transitive_closure(&self.model.read(), when_deleting)
    }
}

pub fn new(no: u32) -> ERef<dyn DiagramController> {
    let name = format!("New RDF diagram {}", no);

    let diagram = ERef::new(RdfDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        ViewUuid::now_v7().into(),
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
    let diagram = ERef::new(RdfDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            node.into(), literal_model.into(),
            predicate.into(), graph.into(), graph_st.into(),
        ],
    ));
    DiagramControllerGen2::new(
        ViewUuid::now_v7().into(),
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

    fn targetting_for_section(&self, element: Option<RdfElement>) -> egui::Color32 {
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
                let (_literal_model, literal_view) = new_rdf_literal(
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
    fn add_section(&mut self, controller: RdfElement) {
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

    fn try_additional_dependency(&mut self) -> Option<(BucketNoT, ModelUuid, ModelUuid)> {
        None
    }

    fn try_construct_view(
        &mut self,
        into: &dyn ContainerGen2<RdfDomain>,
    ) -> Option<(RdfElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialRdfElement::Some(x) => {
                let x = x.clone();
                self.result = PartialRdfElement::None;
                let esm: Option<Box<dyn CustomModal>> = match &x {
                    RdfElementView::Literal(inner) => {
                        Some(Box::new(RdfLiteralSetupModal::from(&inner.read().model)))
                    },
                    RdfElementView::Node(inner) => {
                        Some(Box::new(RdfIriBasedSetupModal::from(RdfElement::from(inner.read().model.clone()))))
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
    first_frame: bool,
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
            first_frame: true,
            iri_buffer,
        }
    }
}

impl CustomModal for RdfIriBasedSetupModal {
    fn show(
        &mut self,
        _gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("IRI:");
        let r = ui.text_edit_singleline(&mut self.iri_buffer);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let iri = Arc::new(self.iri_buffer.clone());
                match &self.model {
                    RdfElement::RdfGraph(inner) => inner.write().iri = iri,
                    RdfElement::RdfNode(inner) => inner.write().iri = iri,
                    RdfElement::RdfPredicate(inner) => inner.write().iri = iri,
                    RdfElement::RdfLiteral(_inner) => unreachable!(),
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
    fn model_section(&self) -> RdfElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().iri.clone()
    }

    fn insert_element(&mut self, position: Option<PositionNoT>, element: RdfElement) -> Result<PositionNoT, ()> {
        self.model.write().insert_element(0, position, element).map_err(|_| ())
    }
    fn delete_element(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        self.model.write().remove_element(uuid).map(|e| e.1)
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
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()) {
                *e = new_model.clone();
            }
        }
    }
}

fn new_rdf_graph(
    iri: &str,
    bounds_rect: egui::Rect,
) -> (ERef<RdfGraph>, ERef<PackageViewT>) {
    let graph_model = ERef::new(RdfGraph::new(
        ModelUuid::now_v7(),
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
        ViewUuid::now_v7().into(),
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
    let node_model = ERef::new(RdfNode::new(ModelUuid::now_v7(), iri.to_owned()));
    let node_view = new_rdf_node_view(node_model.clone(), position);
    (node_model, node_view)
}
fn new_rdf_node_view(
    model: ERef<RdfNode>,
    position: egui::Pos2,
) -> ERef<RdfNodeView> {
    let m = model.read();
    let node_view = ERef::new(RdfNodeView {
        uuid: ViewUuid::now_v7().into(),
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

impl RdfNodeView {
    fn predicate_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_radius = 8.0;
        let b_center = self.position + egui::Vec2::new(self.bounds_radius.x + b_radius / ui_scale, -self.bounds_radius.y + b_radius / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * b_radius / ui_scale),
        )
    }
}

impl Entity for RdfNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
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
        _gdc: &GlobalDrawingContext,
        _q: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> PropertiesStatus<RdfDomain> {
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
        _q: &RdfQueryable,
        _gdc: &GlobalDrawingContext,
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

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            let b_rect = self.predicate_button_rect(ui_scale);
            canvas.draw_rectangle(
                b_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b_rect.center(), egui::Align2::CENTER_CENTER, "↘", 14.0 / ui_scale, egui::Color32::BLACK);
        }

        // Draw targetting ellipse
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_ellipse(
                self.position,
                self.bounds_radius,
                t.targetting_for_section(Some(self.model())),
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
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
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
            InputEvent::Click(pos) if self.highlight.selected && self.predicate_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveRdfTool {
                    initial_stage: RdfToolStage::PredicateStart,
                    current_stage: RdfToolStage::PredicateEnd,
                    result: PartialRdfElement::Predicate { source: self.model.clone(), dest: None },
                    event_lock: true,
                });

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model());
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
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
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
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
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
        _flattened_views: &mut HashMap<ViewUuid, RdfElementView>,
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
            (ViewUuid::now_v7(), ModelUuid::now_v7())
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
    let literal_model = ERef::new(RdfLiteral::new(
        ModelUuid::now_v7(),
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
    let literal_view = ERef::new(RdfLiteralView {
        uuid: ViewUuid::now_v7().into(),
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
    first_frame: bool,
    content_buffer: String,
    datatype_buffer: String,
    langtag_buffer: String,
}

impl From<&ERef<RdfLiteral>> for RdfLiteralSetupModal {
    fn from(model: &ERef<RdfLiteral>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            first_frame: true,
            content_buffer: (*m.content).clone(),
            datatype_buffer: (*m.datatype).clone(),
            langtag_buffer: (*m.langtag).clone(),
        }
    }
}

impl CustomModal for RdfLiteralSetupModal {
    fn show(
        &mut self,
        _gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Content:");
        let r = ui.text_edit_singleline(&mut self.content_buffer);
        ui.label("Datatype:");
        ui.text_edit_singleline(&mut self.datatype_buffer);
        ui.label("Langtag:");
        ui.text_edit_singleline(&mut self.langtag_buffer);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

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
        (*self.uuid).into()
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
        _gdc: &GlobalDrawingContext,
        _q: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> PropertiesStatus<RdfDomain> {
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
        _q: &RdfQueryable,
        _gdc: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        self.bounds_rect = crate::domains::umlclass::umlclass_controllers::draw_uml_class(
            canvas,
            self.position,
            None,
            &self.model.read().content,
            None,
            false,
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
                t.targetting_for_section(Some(self.model())),
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
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
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
                    tool.add_section(self.model());
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
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
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
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
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
        _flattened_views: &mut HashMap<ViewUuid, RdfElementView>,
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
            (ViewUuid::now_v7().into(), ModelUuid::now_v7().into())
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


fn new_rdf_predicate(
    iri: &str,
    source: (ERef<RdfNode>, RdfElementView),
    target: (RdfTargettableElement, RdfElementView),
) -> (ERef<RdfPredicate>, ERef<LinkViewT>) {
    let predicate_model = ERef::new(RdfPredicate::new(
        ModelUuid::now_v7(),
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

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.source.read().uuid), *m.target.uuid(), target.min_shape(), None);

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        RdfPredicateAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct RdfPredicateAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfPredicate>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: RdfPredicateTemporaries,
}

#[derive(Clone, Default)]
struct RdfPredicateTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    iri_buffer: String,
    comment_buffer: String,
}

impl MulticonnectionAdapter<RdfDomain> for RdfPredicateAdapter {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.model.read().iri.clone())
    }

    fn arrow_data(&self) -> &HashMap<(bool, ModelUuid), ArrowData> {
        &self.temporaries.arrow_data
    }

    fn source_uuids(&self) -> &[ModelUuid] {
        &self.temporaries.source_uuids
    }

    fn target_uuids(&self) -> &[ModelUuid] {
        &self.temporaries.target_uuids
    }

    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        let mut w = self.model.write();
        if let RdfTargettableElement::RdfNode(t) = &w.target {
            let tmp = w.source.clone();
            w.source = t.clone();
            w.target = tmp.into();
            Ok(())
        } else {
            Err(())
        }
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>
    ) ->PropertiesStatus<RdfDomain> {
        ui.label("IRI:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.iri_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::IriChange(Arc::new(self.temporaries.iri_buffer.clone())),
            ]));
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        if ui.button("Switch source and destination").clicked()
            && let RdfTargettableElement::RdfNode(_) = &self.model.read().target {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ]));
        }

        PropertiesStatus::Shown
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

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert((false, *model.source.read().uuid), ArrowData::new_labelless(
            canvas::LineType::Solid,
            canvas::ArrowheadType::None,
        ));
        self.temporaries.arrow_data.insert((true, *model.target.uuid()), ArrowData::new_labelless(
            canvas::LineType::Solid,
            canvas::ArrowheadType::OpenTriangle,
        ));

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.iri_buffer = (*model.iri).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
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

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
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
