use super::rdf_models::{RdfDiagram, RdfElement, RdfGraph, RdfLiteral, RdfNode, RdfPredicate};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ClickHandlingStatus, DiagramController, DragHandlingStatus, ElementController,
    InsensitiveCommand, ModifierKeys, SensitiveCommand, TargettingStatus,
};
use crate::common::controller::{
    ContainerGen2, DiagramControllerGen2, ElementControllerGen2, MulticonnectionView, Tool,
};
use crate::common::observer::Observable;
use crate::{CustomTab, NHApp};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use sophia_api::{prelude::SparqlDataset, sparql::Query};
use sophia_sparql::{ResultTerm, SparqlQuery, SparqlWrapper};

pub struct RdfQueryable {}

#[derive(Clone)]
pub enum RdfElementOrVertex {
    Element(
        (
            uuid::Uuid,
            Arc<
                RwLock<dyn ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, Self>>,
            >,
        ),
    ),
    Vertex((uuid::Uuid, uuid::Uuid, egui::Pos2)),
}

impl From<(uuid::Uuid, uuid::Uuid, egui::Pos2)> for RdfElementOrVertex {
    fn from(v: (uuid::Uuid, uuid::Uuid, egui::Pos2)) -> Self {
        RdfElementOrVertex::Vertex(v)
    }
}

impl TryInto<(uuid::Uuid, uuid::Uuid, egui::Pos2)> for RdfElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<(uuid::Uuid, uuid::Uuid, egui::Pos2), ()> {
        match self {
            RdfElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl
    From<(
        uuid::Uuid,
        Arc<RwLock<dyn ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, Self>>>,
    )> for RdfElementOrVertex
{
    fn from(
        v: (
            uuid::Uuid,
            Arc<
                RwLock<dyn ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, Self>>,
            >,
        ),
    ) -> Self {
        RdfElementOrVertex::Element(v)
    }
}

impl
    TryInto<(
        uuid::Uuid,
        Arc<RwLock<dyn ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, Self>>>,
    )> for RdfElementOrVertex
{
    type Error = ();

    fn try_into(
        self,
    ) -> Result<
        (
            uuid::Uuid,
            Arc<
                RwLock<dyn ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, Self>>,
            >,
        ),
        (),
    > {
        match self {
            RdfElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

pub struct RdfDiagramBuffer {
    name: String,
    comment: String,
}

fn show_props_fun(model: &mut RdfDiagram, buffer_object: &mut RdfDiagramBuffer, ui: &mut egui::Ui) {
    ui.label("Name:");
    let r1 = ui.add_sized(
        (ui.available_width(), 20.0),
        egui::TextEdit::singleline(&mut buffer_object.name),
    );

    if r1.changed() {
        model.name = Arc::new(buffer_object.name.clone());
    }

    ui.label("Comment:");
    let r2 = ui.add_sized(
        (ui.available_width(), 20.0),
        egui::TextEdit::multiline(&mut buffer_object.comment),
    );

    if r2.changed() {
        model.comment = Arc::new(buffer_object.comment.clone());
    }

    if r1.union(r2).changed() {
        model.notify_observers();
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
    >,
    context: &mut NHApp,
    ui: &mut egui::Ui,
) {
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
        Arc::new(RwLock::new(DiagramControllerGen2::new(
            diagram.clone(),
            HashMap::new(),
            RdfQueryable {},
            RdfDiagramBuffer {
                name,
                comment: "".to_owned(),
            },
            show_props_fun,
            tool_change_fun,
            menubar_options_fun,
        ))),
    )
}

pub fn demo(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let node_uuid = uuid::Uuid::now_v7();
    let node = Arc::new(RwLock::new(RdfNode::new(
        node_uuid.clone(),
        "http://www.w3.org/People/EM/contact#me".to_owned(),
    )));
    let node_controller = Arc::new(RwLock::new(RdfNodeController {
        model: node.clone(),
        iri_buffer: "http://www.w3.org/People/EM/contact#me".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        position: egui::Pos2::new(300.0, 100.0),
        bounds_radius: egui::Vec2::ZERO,
    }));

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
        language_buffer: "en".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        position: egui::Pos2::new(300.0, 200.0),
        bounds_rect: egui::Rect::ZERO,
    }));

    let (predicate_uuid, predicate, predicate_controller) = rdf_predicate(
        "http://www.w3.org/2000/10/swap/pim/contact#fullName",
        (node.clone(), node_controller.clone()),
        (literal.clone(), literal_controller.clone()),
    );

    let graph_uuid = uuid::Uuid::now_v7();
    let graph = Arc::new(RwLock::new(RdfGraph::new(
        graph_uuid.clone(),
        "http://graph".to_owned(),
        vec![],
    )));
    let graph_controller = Arc::new(RwLock::new(RdfGraphController {
        model: graph.clone(),
        owned_controllers: HashMap::new(),
        selected_elements: HashSet::new(),
        iri_buffer: "http://graph".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::from_min_max(
            egui::Pos2::new(400.0, 50.0),
            egui::Pos2::new(500.0, 150.0),
        ),
    }));

    //<stress test>
    let mut models_st = Vec::<Arc<RwLock<dyn RdfElement>>>::new();
    let mut controllers_st = HashMap::<
        _,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    >::new();

    for xx in 0..=10 {
        for yy in 300..=400 {
            let node_st_uuid = uuid::Uuid::now_v7();
            let node_st = Arc::new(RwLock::new(RdfNode::new(
                node_st_uuid.clone(),
                "http://www.w3.org/People/EM/contact#me".to_owned(),
            )));
            let node_st_controller = Arc::new(RwLock::new(RdfNodeController {
                model: node_st.clone(),
                iri_buffer: "http://www.w3.org/People/EM/contact#me".to_owned(),
                comment_buffer: "".to_owned(),

                highlight: canvas::Highlight::NONE,
                position: egui::Pos2::new(xx as f32, yy as f32),
                bounds_radius: egui::Vec2::ZERO,
            }));
            models_st.push(node_st);
            controllers_st.insert(node_st_uuid, node_st_controller);
        }
    }

    for xx in 3000..=3100 {
        for yy in 3000..=3100 {
            let node_st_uuid = uuid::Uuid::now_v7();
            let node_st = Arc::new(RwLock::new(RdfNode::new(
                node_st_uuid.clone(),
                "http://www.w3.org/People/EM/contact#me".to_owned(),
            )));
            let node_st_controller = Arc::new(RwLock::new(RdfNodeController {
                model: node_st.clone(),
                iri_buffer: "http://www.w3.org/People/EM/contact#me".to_owned(),
                comment_buffer: "".to_owned(),

                highlight: canvas::Highlight::NONE,
                position: egui::Pos2::new(xx as f32, yy as f32),
                bounds_radius: egui::Vec2::ZERO,
            }));
            models_st.push(node_st);
            controllers_st.insert(node_st_uuid, node_st_controller);
        }
    }

    let graph_st_uuid = uuid::Uuid::now_v7();
    let graph_st = Arc::new(RwLock::new(RdfGraph::new(
        graph_st_uuid.clone(),
        "http://stresstestgraph".to_owned(),
        models_st,
    )));
    let graph_st_controller = Arc::new(RwLock::new(RdfGraphController {
        model: graph_st.clone(),
        owned_controllers: controllers_st,
        selected_elements: HashSet::new(),
        iri_buffer: "http://stresstestgraph".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::from_min_max(
            egui::Pos2::new(0.0, 300.0),
            egui::Pos2::new(3000.0, 3300.0),
        ),
    }));
    //</stress test>

    let mut owned_controllers = HashMap::<
        _,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    >::new();
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
        Arc::new(RwLock::new(DiagramControllerGen2::new(
            diagram.clone(),
            owned_controllers,
            RdfQueryable {},
            RdfDiagramBuffer {
                name,
                comment: "".to_owned(),
            },
            show_props_fun,
            tool_change_fun,
            menubar_options_fun,
        ))),
    )
}

#[derive(Clone, Copy)]
pub enum KindedRdfElement<'a> {
    Diagram {},
    Graph {},
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
        >,
    ) -> Self {
        Self::Diagram {}
    }
}

impl<'a> From<&'a RdfGraphController> for KindedRdfElement<'a> {
    fn from(from: &'a RdfGraphController) -> Self {
        Self::Graph {}
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
    Some(
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    ),
    Predicate {
        source: Arc<RwLock<dyn RdfElement>>,
        source_pos: egui::Pos2,
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
    offset: egui::Pos2,
    result: PartialRdfElement,
    event_lock: bool,
}

impl NaiveRdfTool {
    pub fn new(initial_stage: RdfToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            offset: egui::Pos2::ZERO,
            result: PartialRdfElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<dyn RdfElement, RdfQueryable, RdfElementOrVertex> for NaiveRdfTool {
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
        match self.result {
            PartialRdfElement::Predicate { source_pos, .. } => {
                canvas.draw_line(
                    [source_pos, pos],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            PartialRdfElement::Graph { a, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a, pos),
                    egui::Rounding::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            _ => {}
        }
    }

    fn offset_by(&mut self, delta: egui::Vec2) {
        self.offset += delta;
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
                self.result =
                    PartialRdfElement::Some(Arc::new(RwLock::new(RdfLiteralController {
                        model: literal.clone(),
                        content_buffer: "Eric Miller".to_owned(),
                        datatype_buffer: "http://www.w3.org/2001/XMLSchema#string".to_owned(),
                        language_buffer: "en".to_owned(),
                        comment_buffer: "".to_owned(),

                        highlight: canvas::Highlight::NONE,
                        position: pos,
                        bounds_rect: egui::Rect::ZERO,
                    })));
                self.event_lock = true;
            }
            (RdfToolStage::Node, _) => {
                let node = Arc::new(RwLock::new(RdfNode::new(
                    uuid,
                    "http://www.w3.org/People/EM/contact#me".to_owned(),
                )));
                self.result = PartialRdfElement::Some(Arc::new(RwLock::new(RdfNodeController {
                    model: node.clone(),
                    iri_buffer: "http://www.w3.org/People/EM/contact#me".to_owned(),
                    comment_buffer: "".to_owned(),

                    highlight: canvas::Highlight::NONE,
                    position: pos,
                    bounds_radius: egui::Vec2::ZERO,
                })));
                self.event_lock = true;
            }
            (RdfToolStage::GraphStart, _) => {
                self.result = PartialRdfElement::Graph {
                    a: self.offset + pos.to_vec2(),
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
                        source_pos: self.offset + pos.to_vec2(),
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
        into: &dyn ContainerGen2<dyn RdfElement, RdfQueryable, Self, RdfElementOrVertex>,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<dyn RdfElement, RdfQueryable, Self, RdfElementOrVertex>,
            >,
        >,
    > {
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

                let predicate_controller: Option<
                    Arc<
                        RwLock<
                            dyn ElementControllerGen2<
                                dyn RdfElement,
                                RdfQueryable,
                                Self,
                                RdfElementOrVertex,
                            >,
                        >,
                    >,
                > = if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source.read().unwrap().uuid()),
                    into.controller_for(&dest.read().unwrap().uuid()),
                ) {
                    let (_, _, predicate_controller) = rdf_predicate(
                        "http://www.w3.org/2000/10/swap/pim/contact#fullName",
                        (source.clone(), source_controller),
                        (dest.clone(), dest_controller),
                    );

                    Some(predicate_controller)
                } else {
                    None
                };

                self.result = PartialRdfElement::None;
                predicate_controller
            }
            PartialRdfElement::Graph { a, b: Some(b) } => {
                self.current_stage = RdfToolStage::GraphStart;

                let uuid = uuid::Uuid::now_v7();
                let graph = Arc::new(RwLock::new(RdfGraph::new(
                    uuid.clone(),
                    "a graph".to_owned(),
                    vec![],
                )));
                let graph_controller = Arc::new(RwLock::new(RdfGraphController {
                    model: graph.clone(),
                    owned_controllers: HashMap::new(),
                    selected_elements: HashSet::new(),
                    iri_buffer: "a graph".to_owned(),
                    comment_buffer: "".to_owned(),

                    highlight: canvas::Highlight::NONE,
                    bounds_rect: egui::Rect::from_two_pos(*a, *b),
                }));

                self.result = PartialRdfElement::None;
                Some(graph_controller)
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

pub trait RdfElementController:
    ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, RdfElementOrVertex>
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

pub struct RdfGraphController {
    pub model: Arc<RwLock<RdfGraph>>,
    pub owned_controllers: HashMap<
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    >,
    selected_elements: HashSet<uuid::Uuid>,

    iri_buffer: String,
    comment_buffer: String,

    highlight: canvas::Highlight,
    pub bounds_rect: egui::Rect,
}

impl ElementController<dyn RdfElement> for RdfGraphController {
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
        NHShape::Rect {
            inner: self.bounds_rect,
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.bounds_rect.center()
    }
}

impl ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, RdfElementOrVertex>
    for RdfGraphController
{
    fn show_properties(&mut self, parent: &RdfQueryable, ui: &mut egui::Ui) -> bool {
        if self
            .owned_controllers
            .iter()
            .find(|e| e.1.write().unwrap().show_properties(parent, ui))
            .is_some()
        {
            true
        } else if self.highlight.selected {
            ui.label("IRI:");
            let r1 = ui.add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.iri_buffer),
            );

            ui.label("Comment:");
            let r2 = ui.add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            );

            if r1.changed() || r2.changed() {
                let mut model = self.model.write().unwrap();

                if r1.changed() {
                    model.iri = Arc::new(self.iri_buffer.clone());
                }

                if r2.changed() {
                    model.comment = Arc::new(self.comment_buffer.clone());
                }

                model.notify_observers();
            }
            true
        } else {
            false
        }
    }
    fn list_in_project_hierarchy(&self, parent: &RdfQueryable, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        egui::CollapsingHeader::new(format!("{} ({})", model.iri, model.uuid)).show(ui, |ui| {
            for (_uuid, c) in &self.owned_controllers {
                let c = c.read().unwrap();
                c.list_in_project_hierarchy(parent, ui);
            }
        });
    }
    fn draw_in(
        &mut self,
        q: &RdfQueryable,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::Rounding::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_text(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            &self.model.read().unwrap().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        let offset_tool = tool.map(|(p, t)| (p - self.bounds_rect.left_top().to_vec2(), t));
        let mut drawn_child_targetting = TargettingStatus::NotDrawn;

        canvas.offset_by(self.bounds_rect.left_top().to_vec2());
        self.owned_controllers
            .iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| {
                if uc.1.write().unwrap().draw_in(q, canvas, &offset_tool) == TargettingStatus::Drawn
                {
                    drawn_child_targetting = TargettingStatus::Drawn;
                }
            });
        canvas.offset_by(-self.bounds_rect.left_top().to_vec2());

        match (drawn_child_targetting, tool) {
            (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::Rounding::ZERO,
                    t.targetting_for_element(KindedRdfElement::from(&*self)),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );

                canvas.offset_by(self.bounds_rect.left_top().to_vec2());
                self.owned_controllers
                    .iter_mut()
                    .filter(|_| true) // TODO: filter by layers
                    .for_each(|uc| {
                        uc.1.write().unwrap().draw_in(q, canvas, &offset_tool);
                    });
                canvas.offset_by(-self.bounds_rect.left_top().to_vec2());

                TargettingStatus::Drawn
            }
            _ => drawn_child_targetting,
        }
    }

    fn click(
        &mut self,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex>>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        tool.as_mut()
            .map(|e| e.offset_by(self.bounds_rect.left_top().to_vec2()));
        let offset_pos = pos - self.bounds_rect.left_top().to_vec2();

        let uc_status = self
            .owned_controllers
            .iter()
            .map(|uc| {
                (
                    uc,
                    uc.1.write()
                        .unwrap()
                        .click(tool, commands, offset_pos, modifiers),
                )
            })
            .find(|e| e.1 != ClickHandlingStatus::NotHandled);

        tool.as_mut()
            .map(|e| e.offset_by(-self.bounds_rect.left_top().to_vec2()));

        if self.min_shape().contains(pos) {
            if let Some(tool) = tool {
                tool.offset_by(self.bounds_rect.left_top().to_vec2());
                tool.add_position(offset_pos);
                tool.offset_by(-self.bounds_rect.left_top().to_vec2());
                tool.add_element(KindedRdfElement::Graph {}, pos);

                if let Some(new_a) = tool.try_construct(self) {
                    let new_c = new_a.read().unwrap();
                    let uuid = *new_c.uuid();

                    let mut self_m = self.model.write().unwrap();
                    self_m.add_element(new_c.model());
                    drop(new_c);

                    self.owned_controllers.insert(uuid, new_a);
                }

                return ClickHandlingStatus::HandledByContainer;
            } else if let Some((uc, status)) = uc_status {
                if status == ClickHandlingStatus::HandledByElement {
                    if !modifiers.command {
                        commands.push(SensitiveCommand::SelectAll(false));
                        commands.push(SensitiveCommand::Select(
                            std::iter::once(*uc.0).collect(),
                            true,
                        ));
                    } else {
                        commands.push(SensitiveCommand::Select(
                            std::iter::once(*uc.0).collect(),
                            !self.selected_elements.contains(&uc.0),
                        ));
                    }
                }
                return ClickHandlingStatus::HandledByContainer;
            } else {
                return ClickHandlingStatus::HandledByElement;
            }
        }

        ClickHandlingStatus::NotHandled
    }
    fn drag(
        &mut self,
        tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex>>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        tool.as_mut()
            .map(|e| e.offset_by(self.bounds_rect.left_top().to_vec2()));
        let offset_pos = last_pos - self.bounds_rect.left_top().to_vec2();

        let handled = self
            .owned_controllers
            .iter_mut()
            .find(|uc| {
                uc.1.write()
                    .unwrap()
                    .drag(tool, commands, offset_pos, delta)
                    == DragHandlingStatus::Handled
            })
            //.map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            //.ok_or_else(|| {self.last_selected_element = None;})
            .is_some();
        let handled = match handled {
            true => DragHandlingStatus::Handled,
            false => DragHandlingStatus::NotHandled,
        };

        tool.as_mut()
            .map(|e| e.offset_by(-self.bounds_rect.left_top().to_vec2()));

        if handled == DragHandlingStatus::NotHandled && self.min_shape().contains(last_pos) {
            if self.highlight.selected {
                commands.push(SensitiveCommand::MoveSelectedElements(delta));
            } else {
                commands.push(SensitiveCommand::MoveElements(
                    std::iter::once(*self.uuid()).collect(),
                    delta,
                ));
            }
            return DragHandlingStatus::Handled;
        }

        handled
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex>>,
    ) {
        fn recurse(
            this: &mut RdfGraphController,
            command: &InsensitiveCommand<RdfElementOrVertex>,
            undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex>>,
        ) {
            for e in &this.owned_controllers {
                let mut e = e.1.write().unwrap();
                e.apply_command(command, undo_accumulator);
            }
        }

        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
                match select {
                    true => {
                        self.selected_elements =
                            self.owned_controllers.iter().map(|e| *e.0).collect()
                    }
                    false => self.selected_elements.clear(),
                }
                recurse(self, command, undo_accumulator);
            }
            InsensitiveCommand::Select(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }

                for uuid in self.owned_controllers.keys().filter(|k| uuids.contains(k)) {
                    match select {
                        true => self.selected_elements.insert(*uuid),
                        false => self.selected_elements.remove(uuid),
                    };
                }

                recurse(self, command, undo_accumulator);
            }
            InsensitiveCommand::MoveElements(uuids, delta) => {
                if uuids.contains(&*self.uuid()) {
                    self.bounds_rect.set_center(self.position() + *delta);
                    undo_accumulator.push(InsensitiveCommand::MoveElements(
                        std::iter::once(*self.uuid()).collect(),
                        -*delta,
                    ));
                } else {
                    recurse(self, command, undo_accumulator);
                }
            }
            InsensitiveCommand::DeleteElements(uuids) => {
                self.owned_controllers.retain(|k, v| !uuids.contains(&k));
                // TODO: undo commands
            }
            InsensitiveCommand::AddElement(..) => {
                // TODO: stuff
            }
        }
    }

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }

        for e in &self.owned_controllers {
            let mut e = e.1.write().unwrap();
            e.collect_all_selected_elements(into);
        }
    }
}

impl ContainerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, RdfElementOrVertex>
    for RdfGraphController
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    > {
        self.owned_controllers.get(uuid).cloned()
    }
}

pub struct RdfNodeController {
    pub model: Arc<RwLock<RdfNode>>,

    iri_buffer: String,
    comment_buffer: String,

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

impl ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, RdfElementOrVertex>
    for RdfNodeController
{
    fn show_properties(&mut self, _parent: &RdfQueryable, ui: &mut egui::Ui) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("IRI:");
        let r1 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.iri_buffer),
        );

        ui.label("Comment:");
        let r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.comment_buffer),
        );

        if r1.changed() || r2.changed() {
            let mut model = self.model.write().unwrap();

            if r1.changed() {
                model.iri = Arc::new(self.iri_buffer.clone());
            }

            if r2.changed() {
                model.comment = Arc::new(self.comment_buffer.clone());
            }

            model.notify_observers();
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

    fn click(
        &mut self,
        tool: &mut Option<NaiveRdfTool>,
        _commands: &mut Vec<SensitiveCommand<RdfElementOrVertex>>,
        pos: egui::Pos2,
        _modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) {
            return ClickHandlingStatus::NotHandled;
        }

        if let Some(tool) = tool {
            tool.add_element(KindedRdfElement::Node { inner: self }, pos);
        }

        ClickHandlingStatus::HandledByElement
    }
    fn drag(
        &mut self,
        _tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex>>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        if !self.min_shape().contains(last_pos) {
            return DragHandlingStatus::NotHandled;
        }

        if self.highlight.selected {
            commands.push(SensitiveCommand::MoveSelectedElements(delta));
        } else {
            commands.push(SensitiveCommand::MoveElements(
                std::iter::once(*self.uuid()).collect(),
                delta,
            ));
        }

        DragHandlingStatus::Handled
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex>>,
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
            InsensitiveCommand::MoveElements(uuids, delta) => {
                if uuids.contains(&*self.uuid()) {
                    self.position += *delta;
                    undo_accumulator.push(InsensitiveCommand::MoveElements(
                        std::iter::once(*self.uuid()).collect(),
                        -*delta,
                    ));
                }
            }
            InsensitiveCommand::DeleteElements(..) | InsensitiveCommand::AddElement(..) => {}
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
    language_buffer: String,
    comment_buffer: String,

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

impl ElementControllerGen2<dyn RdfElement, RdfQueryable, NaiveRdfTool, RdfElementOrVertex>
    for RdfLiteralController
{
    fn show_properties(&mut self, _parent: &RdfQueryable, ui: &mut egui::Ui) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Content:");
        let r1 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.content_buffer),
        );
        ui.label("Datatype:");
        let r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.datatype_buffer),
        );

        ui.label("Language:");
        let r3 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.language_buffer),
        );

        ui.label("Comment:");
        let r4 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.comment_buffer),
        );

        if r1.changed() || r2.changed() || r3.changed() || r4.changed() {
            let mut model = self.model.write().unwrap();

            if r1.changed() {
                model.content = Arc::new(self.content_buffer.clone());
            }

            if r2.changed() {
                model.datatype = Arc::new(self.datatype_buffer.clone());
            }

            if r3.changed() {
                model.language = Arc::new(self.language_buffer.clone());
            }

            if r4.changed() {
                model.comment = Arc::new(self.comment_buffer.clone());
            }

            model.notify_observers();
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

    fn click(
        &mut self,
        tool: &mut Option<NaiveRdfTool>,
        _commands: &mut Vec<SensitiveCommand<RdfElementOrVertex>>,
        pos: egui::Pos2,
        _modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) {
            return ClickHandlingStatus::NotHandled;
        }

        if let Some(tool) = tool {
            tool.add_element(KindedRdfElement::Literal { inner: self }, pos);
        }

        ClickHandlingStatus::HandledByElement
    }
    fn drag(
        &mut self,
        _tool: &mut Option<NaiveRdfTool>,
        commands: &mut Vec<SensitiveCommand<RdfElementOrVertex>>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        if !self.min_shape().contains(last_pos) {
            return DragHandlingStatus::NotHandled;
        }

        if self.highlight.selected {
            commands.push(SensitiveCommand::MoveSelectedElements(delta));
        } else {
            commands.push(SensitiveCommand::MoveElements(
                std::iter::once(*self.uuid()).collect(),
                delta,
            ));
        }

        DragHandlingStatus::Handled
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<RdfElementOrVertex>,
        undo_accumulator: &mut Vec<InsensitiveCommand<RdfElementOrVertex>>,
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
            InsensitiveCommand::MoveElements(uuids, delta) => {
                if uuids.contains(&*self.uuid()) {
                    self.position += *delta;
                    undo_accumulator.push(InsensitiveCommand::MoveElements(
                        std::iter::once(*self.uuid()).collect(),
                        -*delta,
                    ));
                }
            }
            InsensitiveCommand::DeleteElements(..) | InsensitiveCommand::AddElement(..) => {}
        }
    }

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }
    }
}

fn rdf_predicate(
    iri: &str,
    source: (
        Arc<RwLock<dyn RdfElement>>,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    ),
    destination: (
        Arc<RwLock<dyn RdfElement>>,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn RdfElement,
                    RdfQueryable,
                    NaiveRdfTool,
                    RdfElementOrVertex,
                >,
            >,
        >,
    ),
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
            >,
        >,
    >,
) {
    fn model_to_element_shim(a: Arc<RwLock<RdfPredicate>>) -> Arc<RwLock<dyn RdfElement>> {
        a
    }

    fn show_properties_fun(
        model: &mut RdfPredicate,
        buffer: &mut RdfPredicateBuffer,
        ui: &mut egui::Ui,
    ) {
        ui.label("IRI:");
        let r1 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut buffer.iri),
        );

        ui.label("Comment:");
        let r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut buffer.comment),
        );

        let r3 = if ui.button("Switch source and destination").clicked()
            && /* TODO: must check if target isn't a literal */ true
        {
            (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            // TODO: (self.source, self.destination) = (self.destination.clone(), self.source.clone());
            true
        } else {
            false
        };

        if r1.changed() || r2.changed() || r3 {
            if r1.changed() {
                model.iri = Arc::new(buffer.iri.clone());
            }

            if r2.changed() {
                model.comment = Arc::new(buffer.comment.clone());
            }

            model.notify_observers();
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
    let predicate_controller = Arc::new(RwLock::new(MulticonnectionView {
        model: predicate.clone(),
        buffer: RdfPredicateBuffer {
            iri: iri.to_owned(),
            comment: "".to_owned(),
        },

        source: source.1,
        destination: destination.1,

        highlight: canvas::Highlight::NONE,
        selected_vertices: HashSet::new(),
        center_point: None,
        source_points: vec![vec![(uuid::Uuid::now_v7(), egui::Pos2::ZERO)]],
        dest_points: vec![vec![(uuid::Uuid::now_v7(), egui::Pos2::ZERO)]],

        model_to_element_shim,
        show_properties_fun,

        model_to_uuid,
        model_to_name,
        model_to_line_type,
        model_to_source_arrowhead_type,
        model_to_destination_arrowhead_type,
        model_to_source_arrowhead_label,
        model_to_destination_arrowhead_label,
    }));
    (predicate_uuid, predicate, predicate_controller)
}

struct RdfPredicateBuffer {
    iri: String,
    comment: String,
}

impl RdfElementController
    for MulticonnectionView<
        RdfPredicate,
        dyn RdfElement,
        RdfQueryable,
        RdfPredicateBuffer,
        NaiveRdfTool,
        RdfElementOrVertex,
    >
{
    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        *self.source.read().unwrap().uuid() == *uuid
    }

    fn connection_target_name(&self) -> Option<Arc<String>> {
        Some(self.destination.read().unwrap().model_name())
    }
}
