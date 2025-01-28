use super::rdf_models::{RdfDiagram, RdfElement, RdfGraph, RdfLiteral, RdfNode, RdfPredicate};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ContainerGen2, DiagramController, DiagramControllerGen2, ElementController, ElementControllerGen2, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, ModifierKeys, MulticonnectionView, PackageView, SensitiveCommand, TargettingStatus, Tool, VertexInformation
};
use crate::{CustomTab, NHApp};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock, Weak},
};

use sophia::api::{prelude::SparqlDataset, sparql::Query};
use sophia_sparql::{ResultTerm, SparqlQuery, SparqlWrapper};

type ArcRwLockController = Arc<
    RwLock<
        dyn ElementControllerGen2<
            dyn RdfElement,
            RdfQueryable,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    >,
>;

pub struct RdfQueryable {}

#[derive(Clone)]
pub enum RdfPropChange {
    NameChange(Arc<String>),
    IriChange(Arc<String>),

    ContentChange(Arc<String>),
    DataTypeChange(Arc<String>),
    LangTagChange(Arc<String>),

    CommentChange(Arc<String>),
    GraphResize(egui::Vec2),
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
                Self::GraphResize(size) => format!("GraphResize({})", size),
                Self::FlipMulticonnection => format!("FlipMulticonnection"),
            }
        )
    }
}

impl TryInto<FlipMulticonnection> for &RdfPropChange {
    type Error = ();

    fn try_into(self) -> Result<FlipMulticonnection, ()> {
        match self {
            RdfPropChange::FlipMulticonnection => Ok(FlipMulticonnection {}),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
pub enum RdfElementOrVertex {
    Element((uuid::Uuid, ArcRwLockController)),
    Vertex(VertexInformation),
}

impl Debug for RdfElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "RdfElementOrVertex::???")
    }
}

impl From<VertexInformation> for RdfElementOrVertex {
    fn from(v: VertexInformation) -> Self {
        Self::Vertex(v)
    }
}

impl TryInto<VertexInformation> for RdfElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<VertexInformation, ()> {
        match self {
            Self::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl From<(uuid::Uuid, ArcRwLockController)> for RdfElementOrVertex {
    fn from(v: (uuid::Uuid, ArcRwLockController)) -> Self {
        Self::Element(v)
    }
}

impl TryInto<(uuid::Uuid, ArcRwLockController)> for RdfElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<(uuid::Uuid, ArcRwLockController), ()> {
        match self {
            Self::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

pub struct RdfDiagramBuffer {
    uuid: uuid::Uuid,
    name: String,
    comment: String,
}

fn show_props_fun(
    buffer: &mut RdfDiagramBuffer,
    ui: &mut egui::Ui,
    commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
) {
    ui.label("Name:");
    if ui
        .add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut buffer.name),
        )
        .changed()
    {
        commands.push(SensitiveCommand::PropertyChange(
            std::iter::once(buffer.uuid).collect(),
            vec![RdfPropChange::NameChange(Arc::new(buffer.name.clone()))],
        ));
    };

    ui.label("Comment:");
    if ui
        .add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut buffer.comment),
        )
        .changed()
    {
        commands.push(SensitiveCommand::PropertyChange(
            std::iter::once(buffer.uuid).collect(),
            vec![RdfPropChange::CommentChange(Arc::new(
                buffer.comment.clone(),
            ))],
        ));
    }
}
fn apply_property_change_fun(
    buffer: &mut RdfDiagramBuffer,
    model: &mut RdfDiagram,
    command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
    undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
) {
    if let InsensitiveCommand::PropertyChange(_, properties) = command {
        for property in properties {
            match property {
                RdfPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(buffer.uuid).collect(),
                        vec![RdfPropChange::NameChange(model.name.clone())],
                    ));
                    buffer.name = (**name).clone();
                    model.name = name.clone();
                }
                RdfPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(buffer.uuid).collect(),
                        vec![RdfPropChange::CommentChange(model.comment.clone())],
                    ));
                    buffer.comment = (**comment).clone();
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
}
fn tool_change_fun(tool: &mut Option<NaiveRdfTool>, ui: &mut egui::Ui) {
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

fn menubar_options_fun(
    controller: &mut DiagramControllerGen2<
        RdfDiagram,
        dyn RdfElement,
        RdfQueryable,
        RdfDiagramBuffer,
        NaiveRdfTool,
        RdfElementOrVertex,
        RdfPropChange,
    >,
    context: &mut NHApp,
    ui: &mut egui::Ui,
) {
    if ui.button("Import RDF data").clicked() {
        // TODO: import stuff
    }
    if ui.button("SPARQL Queries").clicked() {
        let uuid = uuid::Uuid::now_v7();
        context.add_custom_tab(
            uuid,
            Arc::new(RwLock::new(SparqlQueriesTab {
                diagram: controller.model(),
                selected_query: None,
                query_name_buffer: "".to_owned(),
                query_value_buffer: "".to_owned(),
                debug_message: None,
                query_results: None,
            })),
        );
    }
    if ui.button("Ontology alignment").clicked() {
        // TODO: similar to the above?
    }
    ui.separator();
}

pub fn new(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let uuid = uuid::Uuid::now_v7();
    let name = format!("New RDF diagram {}", no);

    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        uuid.clone(),
        name.clone(),
        vec![],
    )));
    (
        uuid,
        DiagramControllerGen2::new(
            diagram.clone(),
            HashMap::new(),
            RdfQueryable {},
            RdfDiagramBuffer {
                uuid,
                name,
                comment: "".to_owned(),
            },
            show_props_fun,
            apply_property_change_fun,
            tool_change_fun,
            menubar_options_fun,
        ),
    )
}

pub fn demo(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let (node_uuid, node, node_controller) = rdf_node(
        "http://www.w3.org/People/EM/contact#me",
        egui::Pos2::new(300.0, 100.0),
    );

    let literal_uuid = uuid::Uuid::now_v7();
    let literal = Arc::new(RwLock::new(RdfLiteral::new(
        literal_uuid.clone(),
        "Eric Miller".to_owned(),
        "http://www.w3.org/2001/XMLSchema#string".to_owned(),
        "en".to_owned(),
    )));
    let literal_controller = Arc::new(RwLock::new(RdfLiteralController {
        model: literal.clone(),
        content_buffer: "Eric Miller".to_owned(),
        datatype_buffer: "http://www.w3.org/2001/XMLSchema#string".to_owned(),
        langtag_buffer: "en".to_owned(),
        comment_buffer: "".to_owned(),

        dragged: false,
        highlight: canvas::Highlight::NONE,
        position: egui::Pos2::new(300.0, 200.0),
        bounds_rect: egui::Rect::ZERO,
    }));

    let (predicate_uuid, predicate, predicate_controller) = rdf_predicate(
        "http://www.w3.org/2000/10/swap/pim/contact#fullName",
        (node.clone(), node_controller.clone()),
        (literal.clone(), literal_controller.clone()),
    );

    let (graph_uuid, graph, graph_controller) = rdf_graph(
        "http://graph",
        egui::Rect::from_min_max(egui::Pos2::new(400.0, 50.0), egui::Pos2::new(500.0, 150.0)),
    );

    //<stress test>
    let mut models_st = Vec::<Arc<RwLock<dyn RdfElement>>>::new();
    let mut controllers_st = HashMap::<_, ArcRwLockController>::new();

    for xx in 0..=10 {
        for yy in 300..=400 {
            let (node_st_uuid, node_st, node_st_controller) = rdf_node(
                "http://www.w3.org/People/EM/contact#me",
                egui::Pos2::new(xx as f32, yy as f32),
            );
            models_st.push(node_st);
            controllers_st.insert(node_st_uuid, node_st_controller);
        }
    }

    for xx in 3000..=3100 {
        for yy in 3000..=3100 {
            let (node_st_uuid, node_st, node_st_controller) = rdf_node(
                "http://www.w3.org/People/EM/contact#me",
                egui::Pos2::new(xx as f32, yy as f32),
            );
            models_st.push(node_st);
            controllers_st.insert(node_st_uuid, node_st_controller);
        }
    }

    let (graph_st_uuid, graph_st, graph_st_controller) = rdf_graph(
        "http://stresstestgraph",
        egui::Rect::from_min_max(egui::Pos2::new(0.0, 300.0), egui::Pos2::new(3000.0, 3300.0)),
    );
    //</stress test>

    let mut owned_controllers = HashMap::<_, ArcRwLockController>::new();
    owned_controllers.insert(node_uuid, node_controller);
    owned_controllers.insert(literal_uuid, literal_controller);
    owned_controllers.insert(predicate_uuid, predicate_controller);
    owned_controllers.insert(graph_uuid, graph_controller);
    owned_controllers.insert(graph_st_uuid, graph_st_controller);

    let name = format!("Demo RDF diagram {}", no);
    let diagram_uuid = uuid::Uuid::now_v7();
    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        diagram_uuid.clone(),
        name.clone(),
        vec![node, literal, predicate, graph, graph_st],
    )));
    (
        diagram_uuid,
        DiagramControllerGen2::new(
            diagram.clone(),
            owned_controllers,
            RdfQueryable {},
            RdfDiagramBuffer {
                uuid: diagram_uuid,
                name,
                comment: "".to_owned(),
            },
            show_props_fun,
            apply_property_change_fun,
            tool_change_fun,
            menubar_options_fun,
        ),
    )
}

#[derive(Clone, Copy)]
pub enum KindedRdfElement<'a> {
    Diagram {},
    Graph {
        inner: &'a PackageView<
            RdfGraph,
            dyn RdfElement,
            RdfQueryable,
            RdfGraphBuffer,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    },
    Literal {
        inner: &'a RdfLiteralController,
    },
    Node {
        inner: &'a RdfNodeController,
    },
    Predicate {
        inner: &'a MulticonnectionView<
            RdfPredicate,
            dyn RdfElement,
            RdfQueryable,
            RdfPredicateBuffer,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    },
}

impl<'a>
    From<
        &'a DiagramControllerGen2<
            RdfDiagram,
            dyn RdfElement,
            RdfQueryable,
            RdfDiagramBuffer,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    > for KindedRdfElement<'a>
{
    fn from(
        from: &'a DiagramControllerGen2<
            RdfDiagram,
            dyn RdfElement,
            RdfQueryable,
            RdfDiagramBuffer,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    ) -> Self {
        Self::Diagram {}
    }
}

impl<'a>
    From<
        &'a PackageView<
            RdfGraph,
            dyn RdfElement,
            RdfQueryable,
            RdfGraphBuffer,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    > for KindedRdfElement<'a>
{
    fn from(
        from: &'a PackageView<
            RdfGraph,
            dyn RdfElement,
            RdfQueryable,
            RdfGraphBuffer,
            NaiveRdfTool,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    ) -> Self {
        Self::Graph { inner: from }
    }
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
    Some((uuid::Uuid, ArcRwLockController)),
    Predicate {
        source: Arc<RwLock<dyn RdfElement>>,
        source_view: ArcRwLockController,
        dest: Option<Arc<RwLock<dyn RdfElement>>>,
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

impl Tool<dyn RdfElement, RdfQueryable, RdfElementOrVertex, RdfPropChange> for NaiveRdfTool {
    type KindedElement<'a> = KindedRdfElement<'a>;
    type Stage = RdfToolStage;

    fn initial_stage(&self) -> RdfToolStage {
        self.initial_stage
    }

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32 {
        match controller {
            KindedRdfElement::Diagram { .. } => match self.current_stage {
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd
                | RdfToolStage::Note => TARGETTABLE_COLOR,
                RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => NON_TARGETTABLE_COLOR,
            },
            KindedRdfElement::Graph { .. } => match self.current_stage {
                RdfToolStage::Literal | RdfToolStage::Node | RdfToolStage::Note => {
                    TARGETTABLE_COLOR
                }
                RdfToolStage::PredicateStart
                | RdfToolStage::PredicateEnd
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            KindedRdfElement::Literal { .. } => match self.current_stage {
                RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::PredicateStart
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd
                | RdfToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedRdfElement::Node { .. } => match self.current_stage {
                RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal
                | RdfToolStage::Node
                | RdfToolStage::GraphStart
                | RdfToolStage::GraphEnd
                | RdfToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedRdfElement::Predicate { .. } => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialRdfElement::Predicate { source_view, .. } => {
                canvas.draw_line(
                    [source_view.read().unwrap().position(), pos],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            PartialRdfElement::Graph { a, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a, pos),
                    egui::Rounding::ZERO,
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

        let uuid = uuid::Uuid::now_v7();
        match (self.current_stage, &mut self.result) {
            (RdfToolStage::Literal, _) => {
                let literal = Arc::new(RwLock::new(RdfLiteral::new(
                    uuid,
                    "Eric Miller".to_owned(),
                    "http://www.w3.org/2001/XMLSchema#string".to_owned(),
                    "en".to_owned(),
                )));
                self.result = PartialRdfElement::Some((
                    uuid,
                    Arc::new(RwLock::new(RdfLiteralController {
                        model: literal.clone(),
                        content_buffer: "Eric Miller".to_owned(),
                        datatype_buffer: "http://www.w3.org/2001/XMLSchema#string".to_owned(),
                        langtag_buffer: "en".to_owned(),
                        comment_buffer: "".to_owned(),

                        dragged: false,
                        highlight: canvas::Highlight::NONE,
                        position: pos,
                        bounds_rect: egui::Rect::ZERO,
                    })),
                ));
                self.event_lock = true;
            }
            (RdfToolStage::Node, _) => {
                let (node_uuid, node, node_controller) = rdf_node("http://www.w3.org/People/EM/contact#me", pos);
                self.result = PartialRdfElement::Some((node_uuid, node_controller));
                self.event_lock = true;
            }
            (RdfToolStage::GraphStart, _) => {
                self.result = PartialRdfElement::Graph {
                    a: pos,
                    b: None,
                };
                self.current_stage = RdfToolStage::GraphEnd;
                self.event_lock = true;
            }
            (RdfToolStage::GraphEnd, PartialRdfElement::Graph { ref mut b, .. }) => *b = Some(pos),
            (RdfToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>, pos: egui::Pos2) {
        if self.event_lock {
            return;
        }

        match controller {
            KindedRdfElement::Diagram { .. } => {}
            KindedRdfElement::Graph { .. } => {}
            KindedRdfElement::Literal { inner } => match (self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { ref mut dest, .. }) => {
                    *dest = Some(inner.model.clone());
                    self.event_lock = true;
                }
                _ => {}
            },
            KindedRdfElement::Node { inner } => match (self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateStart, PartialRdfElement::None) => {
                    self.result = PartialRdfElement::Predicate {
                        source: inner.model.clone(),
                        source_view: inner.self_reference.upgrade().unwrap(),
                        dest: None,
                    };
                    self.current_stage = RdfToolStage::PredicateEnd;
                    self.event_lock = true;
                }
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { ref mut dest, .. }) => {
                    *dest = Some(inner.model.clone());
                }
                _ => {}
            },
            KindedRdfElement::Predicate { .. } => {}
        }
    }

    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<
            dyn RdfElement,
            RdfQueryable,
            Self,
            RdfElementOrVertex,
            RdfPropChange,
        >,
    ) -> Option<(uuid::Uuid, ArcRwLockController)> {
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

                let predicate_controller: Option<(uuid::Uuid, ArcRwLockController)> =
                    if let (Some(source_controller), Some(dest_controller)) = (
                        into.controller_for(&source.read().unwrap().uuid()),
                        into.controller_for(&dest.read().unwrap().uuid()),
                    ) {
                        let (uuid, _, predicate_controller) = rdf_predicate(
                            "http://www.w3.org/2000/10/swap/pim/contact#fullName",
                            (source.clone(), source_controller),
                            (dest.clone(), dest_controller),
                        );

                        Some((uuid, predicate_controller))
                    } else {
                        None
                    };

                self.result = PartialRdfElement::None;
                predicate_controller
            }
            PartialRdfElement::Graph { a, b: Some(b) } => {
                self.current_stage = RdfToolStage::GraphStart;

                let (uuid, _, graph_controller) =
                    rdf_graph("http://a-graph", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialRdfElement::None;
                Some((uuid, graph_controller))
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

pub trait RdfElementController:
    ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, RdfElementOrVertex, RdfPropChange>
{
    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool {
        false
    }
    fn connection_target_name(&self) -> Option<Arc<String>> {
        None
    }
}

pub trait RdfContainerController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn RdfElementController>>>;
}

/*
impl RdfDiagramController {
    fn outgoing_for<'a>(&'a self, uuid: &'a uuid::Uuid) -> Box<dyn Iterator<Item=Arc<RwLock<dyn RdfElementController>>> + 'a> {
        Box::new(self.owned_controllers.iter()
                    .filter(|e| e.1.read().unwrap().is_connection_from(uuid))
                    .map(|e| e.1.clone()))
    }
}

impl RdfContainerController for RdfDiagramController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn RdfElementController>>> {
        self.owned_controllers.get(uuid).cloned()
    }
}
*/

pub struct RdfGraphBuffer {
    iri: String,
    comment: String,
}

fn rdf_graph(
    iri: &str,
    bounds_rect: egui::Rect,
) -> (
    uuid::Uuid,
    Arc<RwLock<RdfGraph>>,
    Arc<
        RwLock<
            PackageView<
                RdfGraph,
                dyn RdfElement,
                RdfQueryable,
                RdfGraphBuffer,
                NaiveRdfTool,
                RdfElementOrVertex,
                RdfPropChange,
            >,
        >,
    >,
) {
    fn model_to_element_shim(a: Arc<RwLock<RdfGraph>>) -> Arc<RwLock<dyn RdfElement>> {
        a
    }

    fn show_properties_fun(
        buffer: &mut RdfGraphBuffer,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        ui.label("IRI:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.iri),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::IriChange(Arc::new(buffer.iri.clone())),
            ]));
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.comment),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(buffer.comment.clone())),
            ]));
        }
    }
    fn apply_property_change_fun(
        buffer: &mut RdfGraphBuffer,
        model: &mut RdfGraph,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            for property in properties {
                match property {
                    RdfPropChange::IriChange(iri) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![RdfPropChange::IriChange(model.iri.clone())],
                        ));
                        buffer.iri = (**iri).clone();
                        model.iri = iri.clone();
                    }
                    RdfPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![RdfPropChange::CommentChange(model.comment.clone())],
                        ));
                        buffer.comment = (**comment).clone();
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    let uuid = uuid::Uuid::now_v7();
    let graph = Arc::new(RwLock::new(RdfGraph::new(
        uuid.clone(),
        iri.to_owned(),
        vec![],
    )));
    let graph_controller = Arc::new(RwLock::new(PackageView::new(
        graph.clone(),
        HashMap::new(),
        RdfGraphBuffer {
            iri: iri.to_owned(),
            comment: "".to_owned(),
        },
        bounds_rect,
        model_to_element_shim,
        show_properties_fun,
        apply_property_change_fun,
    )));

    (uuid, graph, graph_controller)
}

fn rdf_node(
    iri: &str,
    position: egui::Pos2,
) -> (uuid::Uuid, Arc<RwLock<RdfNode>>, Arc<RwLock<RdfNodeController>>) {
    let node_uuid = uuid::Uuid::now_v7();
    let node = Arc::new(RwLock::new(RdfNode::new(
        node_uuid.clone(),
        iri.to_owned(),
    )));
    let node_controller = Arc::new(RwLock::new(RdfNodeController {
        model: node.clone(),
        self_reference: Weak::new(),
        iri_buffer: iri.to_owned(),
        comment_buffer: "".to_owned(),

        dragged: false,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_radius: egui::Vec2::ZERO,
    }));
    node_controller.write().unwrap().self_reference = Arc::downgrade(&node_controller);
    (node_uuid, node, node_controller)
}

pub struct RdfNodeController {
    pub model: Arc<RwLock<RdfNode>>,
    self_reference: Weak<RwLock<Self>>,

    iri_buffer: String,
    comment_buffer: String,

    dragged: bool,
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_radius: egui::Vec2,
}

impl ElementController<dyn RdfElement> for RdfNodeController {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().iri.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn RdfElement>> {
        self.model.clone()
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

impl
    ElementControllerGen2<
        dyn RdfElement,
        RdfQueryable,
        NaiveRdfTool,
        RdfElementOrVertex,
        RdfPropChange,
    > for RdfNodeController
{
    fn show_properties(
        &mut self,
        _: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

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

        true
    }
    fn list_in_project_hierarchy(&self, _parent: &RdfQueryable, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        egui::CollapsingHeader::new(format!("{} ({})", model.iri, model.uuid)).show(ui, |_ui| {
            /* TODO: parent -> queryable
            for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }
            */
        });
    }
    fn draw_in(
        &mut self,
        _: &RdfQueryable,
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
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().unwrap().iri,
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
                t.targetting_for_element(KindedRdfElement::Node { inner: self }),
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
        _modifiers: ModifierKeys,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            e if !self.min_shape().contains(*e.mouse_position()) => return EventHandlingStatus::NotHandled,
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) => {
                self.dragged = matches!(event, InputEvent::MouseDown(_));
                EventHandlingStatus::HandledByElement
            },
            InputEvent::Click(pos) => {
                if let Some(tool) = tool {
                    tool.add_element(KindedRdfElement::Node { inner: self }, pos);
                }
                
                EventHandlingStatus::HandledByElement
            },
            InputEvent::Drag { delta, .. } if self.dragged => {
                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(delta));
                } else {
                    commands.push(SensitiveCommand::MoveElements(
                        std::iter::once(*self.uuid()).collect(),
                        delta,
                    ));
                }
                EventHandlingStatus::HandledByElement
            },
            _ => EventHandlingStatus::NotHandled
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
            InsensitiveCommand::Select(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::MoveElements(uuids, delta) if !uuids.contains(&*self.uuid()) => {}
            InsensitiveCommand::MoveElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::DeleteElements(..) | InsensitiveCommand::AddElement(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            RdfPropChange::IriChange(iri) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![RdfPropChange::IriChange(model.iri.clone())],
                                ));
                                self.iri_buffer = (**iri).clone();
                                model.iri = iri.clone();
                            }
                            RdfPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
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

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }
    }
}

pub struct RdfLiteralController {
    pub model: Arc<RwLock<RdfLiteral>>,

    content_buffer: String,
    datatype_buffer: String,
    langtag_buffer: String,
    comment_buffer: String,

    dragged: bool,
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl ElementController<dyn RdfElement> for RdfLiteralController {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().content.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn RdfElement>> {
        self.model.clone()
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

impl
    ElementControllerGen2<
        dyn RdfElement,
        RdfQueryable,
        NaiveRdfTool,
        RdfElementOrVertex,
        RdfPropChange,
    > for RdfLiteralController
{
    fn show_properties(
        &mut self,
        _: &RdfQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

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

        true
    }

    fn list_in_project_hierarchy(&self, _parent: &RdfQueryable, ui: &mut egui::Ui) {
        ui.label(&*self.model.read().unwrap().content);
    }

    fn draw_in(
        &mut self,
        _: &RdfQueryable,
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
                egui::Rounding::ZERO,
                t.targetting_for_element(KindedRdfElement::Literal { inner: self }),
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
        _modifiers: ModifierKeys,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            e if !self.min_shape().contains(*e.mouse_position()) => return EventHandlingStatus::NotHandled,
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) => {
                self.dragged = matches!(event, InputEvent::MouseDown(_));
                EventHandlingStatus::HandledByElement
            },
            InputEvent::Click(pos) => {
                if let Some(tool) = tool {
                    tool.add_element(KindedRdfElement::Literal { inner: self }, pos);
                }
                
                EventHandlingStatus::HandledByElement
            },
            InputEvent::Drag { delta, .. } if self.dragged => {
                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(delta));
                } else {
                    commands.push(SensitiveCommand::MoveElements(
                        std::iter::once(*self.uuid()).collect(),
                        delta,
                    ));
                }
                
                EventHandlingStatus::HandledByElement
            },
            _ => EventHandlingStatus::NotHandled
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
            InsensitiveCommand::Select(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::MoveElements(uuids, delta) if !uuids.contains(&*self.uuid()) => {}
            InsensitiveCommand::MoveElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::DeleteElements(..) | InsensitiveCommand::AddElement(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            RdfPropChange::ContentChange(content) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![RdfPropChange::ContentChange(model.content.clone())],
                                ));
                                self.content_buffer = (**content).clone();
                                model.content = content.clone();
                            }
                            RdfPropChange::DataTypeChange(datatype) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![RdfPropChange::DataTypeChange(model.datatype.clone())],
                                ));
                                self.datatype_buffer = (**datatype).clone();
                                model.datatype = datatype.clone();
                            }
                            RdfPropChange::LangTagChange(langtag) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![RdfPropChange::LangTagChange(model.langtag.clone())],
                                ));
                                self.langtag_buffer = (**langtag).clone();
                                model.langtag = langtag.clone();
                            }
                            RdfPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
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

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }
    }
}

pub struct RdfPredicateBuffer {
    iri: String,
    comment: String,
}

fn rdf_predicate(
    iri: &str,
    source: (Arc<RwLock<dyn RdfElement>>, ArcRwLockController),
    destination: (Arc<RwLock<dyn RdfElement>>, ArcRwLockController),
) -> (
    uuid::Uuid,
    Arc<RwLock<RdfPredicate>>,
    Arc<
        RwLock<
            MulticonnectionView<
                RdfPredicate,
                dyn RdfElement,
                RdfQueryable,
                RdfPredicateBuffer,
                NaiveRdfTool,
                RdfElementOrVertex,
                RdfPropChange,
            >,
        >,
    >,
) {
    fn model_to_element_shim(a: Arc<RwLock<RdfPredicate>>) -> Arc<RwLock<dyn RdfElement>> {
        a
    }

    fn show_properties_fun(
        buffer: &mut RdfPredicateBuffer,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        ui.label("IRI:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.iri),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::IriChange(Arc::new(buffer.iri.clone())),
            ]));
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.comment),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                RdfPropChange::CommentChange(Arc::new(buffer.comment.clone())),
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
    fn apply_property_change_fun(
        buffer: &mut RdfPredicateBuffer,
        model: &mut RdfPredicate,
        command: &InsensitiveCommand<RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex, RdfPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            for property in properties {
                match property {
                    RdfPropChange::IriChange(iri) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![RdfPropChange::IriChange(model.iri.clone())],
                        ));
                        buffer.iri = (**iri).clone();
                        model.iri = iri.clone();
                    }
                    RdfPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![RdfPropChange::CommentChange(model.comment.clone())],
                        ));
                        buffer.comment = (**comment).clone();
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn model_to_uuid(a: &RdfPredicate) -> Arc<uuid::Uuid> {
        a.uuid()
    }
    fn model_to_name(a: &RdfPredicate) -> Arc<String> {
        a.iri.clone()
    }
    fn model_to_line_type(_a: &RdfPredicate) -> canvas::LineType {
        canvas::LineType::Solid
    }
    fn model_to_source_arrowhead_type(_a: &RdfPredicate) -> canvas::ArrowheadType {
        canvas::ArrowheadType::None
    }
    fn model_to_destination_arrowhead_type(_a: &RdfPredicate) -> canvas::ArrowheadType {
        canvas::ArrowheadType::OpenTriangle
    }
    fn model_to_source_arrowhead_label(_a: &RdfPredicate) -> Option<&str> {
        None
    }
    fn model_to_destination_arrowhead_label(_a: &RdfPredicate) -> Option<&str> {
        None
    }

    let predicate_uuid = uuid::Uuid::now_v7();
    let predicate = Arc::new(RwLock::new(RdfPredicate::new(
        predicate_uuid.clone(),
        iri.to_owned(),
        source.0,
        destination.0,
    )));
    let predicate_controller = Arc::new(RwLock::new(MulticonnectionView::new(
        predicate.clone(),
        RdfPredicateBuffer {
            iri: iri.to_owned(),
            comment: "".to_owned(),
        },
        source.1,
        destination.1,
        None,
        vec![vec![(uuid::Uuid::now_v7(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7(), egui::Pos2::ZERO)]],
        model_to_element_shim,
        show_properties_fun,
        apply_property_change_fun,
        model_to_uuid,
        model_to_name,
        model_to_line_type,
        model_to_source_arrowhead_type,
        model_to_destination_arrowhead_type,
        model_to_source_arrowhead_label,
        model_to_destination_arrowhead_label,
    )));
    (predicate_uuid, predicate, predicate_controller)
}

impl RdfElementController
    for MulticonnectionView<
        RdfPredicate,
        dyn RdfElement,
        RdfQueryable,
        RdfPredicateBuffer,
        NaiveRdfTool,
        RdfElementOrVertex,
        RdfPropChange,
    >
{
    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        *self.source.read().unwrap().uuid() == *uuid
    }

    fn connection_target_name(&self) -> Option<Arc<String>> {
        Some(self.destination.read().unwrap().model_name())
    }
}
