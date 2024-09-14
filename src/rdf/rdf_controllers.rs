
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};
use super::rdf_models::{RdfDiagram, RdfElement, RdfGraph, RdfNode, RdfLiteral, RdfPredicate};
use crate::common::canvas::{self, ArrowheadType, Stroke, NHCanvas, UiCanvas, NHShape};
use crate::common::controller::{
    DiagramController, ElementController,
    TargettingStatus, ClickHandlingStatus, DragHandlingStatus, ModifierKeys,
};
use crate::common::observer::Observable;

pub fn new(no: u32) -> (uuid::Uuid, Box<dyn DiagramController>) {
    let uuid = uuid::Uuid::now_v7();
    
    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        uuid.clone(),
        format!("New RDF diagram {}", no),
        vec![],
    )));
    (
        uuid,
        Box::new(RdfDiagramController::new(
            diagram.clone(),
            HashMap::new(),
        )),
    )
}

pub fn demo(no: u32) -> (uuid::Uuid, Box<dyn DiagramController>) {
    let node_uuid = uuid::Uuid::now_v7();
    let node = Arc::new(RwLock::new(RdfNode::new(
        node_uuid.clone(),
        "http://www.w3.org/People/EM/contact#me".to_owned(),
    )));
    let node_controller = Arc::new(RwLock::new(RdfNodeController {
        model: node.clone(),
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
        position: egui::Pos2::new(300.0, 200.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let predicate_uuid = uuid::Uuid::now_v7();
    let predicate = Arc::new(RwLock::new(RdfPredicate::new(
        predicate_uuid.clone(),
        "http://www.w3.org/2000/10/swap/pim/contact#fullName".to_owned(),
        node.clone(),
        literal.clone(),
    )));
    let predicate_controller = Arc::new(RwLock::new(RdfPredicateController {
        model: predicate.clone(),
        source: node_controller.clone(),
        destination: literal_controller.clone(),
        center_point: None,
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));
    
    let graph_uuid = uuid::Uuid::now_v7();
    let graph = Arc::new(RwLock::new(RdfGraph::new(
        graph_uuid.clone(),
        "a graph".to_owned(),
        vec![],
    )));
    let graph_controller = Arc::new(RwLock::new(RdfGraphController {
        model: graph.clone(),
        owned_controllers: HashMap::new(),
        bounds_rect: egui::Rect::from_min_max(egui::Pos2::new(400.0, 50.0),
                                              egui::Pos2::new(500.0, 150.0),)
    }));
    
    //<stress test>
    let mut models_st = Vec::<Arc<RwLock<dyn RdfElement>>>::new();
    let mut controllers_st = HashMap::<_, Arc<RwLock<dyn RdfElementController>>>::new();
    
    for xx in 0..=10 {
        for yy in 300..=400 {
            let node_st_uuid = uuid::Uuid::now_v7();
            let node_st = Arc::new(RwLock::new(RdfNode::new(
                node_st_uuid.clone(),
                "http://www.w3.org/People/EM/contact#me".to_owned(),
            )));
            let node_st_controller = Arc::new(RwLock::new(RdfNodeController {
                model: node_st.clone(),
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
        "a graph".to_owned(),
        models_st,
    )));
    let graph_st_controller = Arc::new(RwLock::new(RdfGraphController {
        model: graph.clone(),
        owned_controllers: controllers_st,
        bounds_rect: egui::Rect::from_min_max(egui::Pos2::new(0.0, 300.0),
                                              egui::Pos2::new(3000.0, 3300.0),)
    }));
    //</stress test>
    
    
    let mut owned_controllers = HashMap::<_, Arc<RwLock<dyn RdfElementController>>>::new();
    owned_controllers.insert(node_uuid, node_controller);
    owned_controllers.insert(literal_uuid, literal_controller);
    owned_controllers.insert(predicate_uuid, predicate_controller);
    owned_controllers.insert(graph_uuid, graph_controller);
    owned_controllers.insert(graph_st_uuid, graph_st_controller);
    
    let diagram_uuid = uuid::Uuid::now_v7();
    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        diagram_uuid.clone(),
        format!("Demo RDF diagram {}", no),
        vec![node, literal, predicate, graph, graph_st],
    )));
    (
        diagram_uuid,
        Box::new(RdfDiagramController::new(
            diagram.clone(),
            owned_controllers,
        ))
    )
}

#[derive(Clone, Copy, PartialEq)]
pub enum RdfToolStage {
    Select,
    Move,
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
    Some(Arc<RwLock<dyn RdfElementController>>),
    Predicate{source: Arc<RwLock<dyn RdfElement>>, source_pos: egui::Pos2, dest: Option<Arc<RwLock<dyn RdfElement>>>},
    Graph{a: egui::Pos2, b: Option<egui::Pos2>},
}

pub trait RdfTool {
    fn initial_stage(&self) -> RdfToolStage;

    fn targetting_for_node(&self) -> egui::Color32;
    fn targetting_for_literal(&self) -> egui::Color32;
    fn targetting_for_graph(&self) -> egui::Color32;
    fn targetting_for_diagram(&self) -> egui::Color32;
    
    fn offset_by(&mut self, delta: egui::Vec2);
    fn add_by_position(&mut self, pos: egui::Pos2);
    fn add_node(&mut self, model: Arc<RwLock<RdfNode>>, pos: egui::Pos2);
    fn add_literal(&mut self, model: Arc<RwLock<RdfLiteral>>, pos: egui::Pos2);
    fn add_graph(&mut self, model: Arc<RwLock<RdfGraph>>, pos: egui::Pos2);
    
    fn try_construct(&mut self, into: &dyn RdfContainerController) -> Option<Arc<RwLock<dyn RdfElementController>>>;
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2);
}

pub struct NaiveRdfTool {
    initial_stage: RdfToolStage,
    current_stage: RdfToolStage,
    offset: egui::Pos2,
    result: PartialRdfElement,
}

impl NaiveRdfTool {
    pub fn new(initial_stage: RdfToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            offset: egui::Pos2::ZERO,
            result: PartialRdfElement::None,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl RdfTool for NaiveRdfTool {
    fn initial_stage(&self) -> RdfToolStage { self.initial_stage }

    fn targetting_for_node(&self) -> egui::Color32 {
        match self.current_stage {
            RdfToolStage::Select | RdfToolStage::Move => egui::Color32::TRANSPARENT,
            RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
            RdfToolStage::Literal | RdfToolStage::Node
            | RdfToolStage::GraphStart | RdfToolStage::GraphEnd | RdfToolStage::Note => NON_TARGETTABLE_COLOR,
        }
    }
    fn targetting_for_literal(&self) -> egui::Color32 {
        match self.current_stage {
            RdfToolStage::Select | RdfToolStage::Move => egui::Color32::TRANSPARENT,
            RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
            RdfToolStage::Literal | RdfToolStage::Node | RdfToolStage::PredicateStart
            | RdfToolStage::GraphStart | RdfToolStage::GraphEnd | RdfToolStage::Note => NON_TARGETTABLE_COLOR,
        }
    }
    fn targetting_for_graph(&self) -> egui::Color32 {
        match self.current_stage {
            RdfToolStage::Select | RdfToolStage::Move => egui::Color32::TRANSPARENT,
            RdfToolStage::Literal | RdfToolStage::Node | RdfToolStage::Note => TARGETTABLE_COLOR,
            RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd
            | RdfToolStage::GraphStart | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
        }
    }
    fn targetting_for_diagram(&self) -> egui::Color32 {
        match self.current_stage {
            RdfToolStage::Select | RdfToolStage::Move => egui::Color32::TRANSPARENT,
            RdfToolStage::Literal | RdfToolStage::Node
            | RdfToolStage::GraphStart | RdfToolStage::GraphEnd | RdfToolStage::Note => TARGETTABLE_COLOR,
            RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => NON_TARGETTABLE_COLOR,
        }
    }
    
    fn offset_by(&mut self, delta: egui::Vec2) {
        self.offset += delta;
    }
    fn add_by_position(&mut self, pos: egui::Pos2) {
        let uuid = uuid::Uuid::now_v7();
        match (self.current_stage, &mut self.result) {
            (RdfToolStage::Literal, _) => {
                let literal = Arc::new(RwLock::new(RdfLiteral::new(
                    uuid,
                    "Eric Miller".to_owned(),
                    "http://www.w3.org/2001/XMLSchema#string".to_owned(),
                    "en".to_owned(),
                )));
                self.result = PartialRdfElement::Some(Arc::new(RwLock::new(RdfLiteralController {
                    model: literal.clone(),
                    position: pos,
                    bounds_rect: egui::Rect::ZERO,
                })));
            },
            (RdfToolStage::Node, _) => {
                let node = Arc::new(RwLock::new(RdfNode::new(
                    uuid,
                    "http://www.w3.org/People/EM/contact#me".to_owned(),
                )));
                self.result = PartialRdfElement::Some(Arc::new(RwLock::new(RdfNodeController {
                    model: node.clone(),
                    position: pos,
                    bounds_radius: egui::Vec2::ZERO,
                })));
            },
            (RdfToolStage::GraphStart, _) => {
                self.result = PartialRdfElement::Graph{a: self.offset + pos.to_vec2(), b: None};
                self.current_stage = RdfToolStage::GraphEnd;
            },
            (RdfToolStage::GraphEnd, PartialRdfElement::Graph{ref mut b, ..}) => {
                *b = Some(pos)
            },
            (RdfToolStage::Note, _) => {},
            _ => {},
        }
    }
    fn add_node(&mut self, model: Arc<RwLock<RdfNode>>, pos: egui::Pos2) {
        match (self.current_stage, &mut self.result) {
            (RdfToolStage::PredicateStart, PartialRdfElement::None) => {
                self.result = PartialRdfElement::Predicate{source: model, source_pos: self.offset + pos.to_vec2(), dest: None};
                self.current_stage = RdfToolStage::PredicateEnd;
            },
            (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate{ref mut dest, ..}) => {
                *dest = Some(model);
            }
            _ => {}
        }
    }
    fn add_literal(&mut self, model: Arc<RwLock<RdfLiteral>>, _pos: egui::Pos2) {
        match (self.current_stage, &mut self.result) {
            (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate{ref mut dest, ..}) => {
                *dest = Some(model);
            }
            _ => {}
        }
    }
    fn add_graph(&mut self, _model: Arc<RwLock<RdfGraph>>, _pos: egui::Pos2) {}
    
    fn try_construct(&mut self, into: &dyn RdfContainerController) -> Option<Arc<RwLock<dyn RdfElementController>>> {
        match &self.result {
            PartialRdfElement::Some(x) => {
                let x = x.clone();
                self.result = PartialRdfElement::None;
                Some(x)
            }
            // TODO: check for source == dest case
            PartialRdfElement::Predicate{source, dest: Some(dest), ..} => {
                self.current_stage = RdfToolStage::PredicateStart;
                
                let uuid = uuid::Uuid::now_v7();
                let predicate = Arc::new(RwLock::new(RdfPredicate::new(
                    uuid.clone(),
                    "http://www.w3.org/2000/10/swap/pim/contact#fullName".to_owned(),
                    source.clone(),
                    dest.clone(),
                )));
                let predicate_controller: Option<Arc<RwLock<dyn RdfElementController>>>
                    = if let (Some(source_controller), Some(dest_controller))
                        = (into.controller_for(&source.read().unwrap().uuid()), into.controller_for(&dest.read().unwrap().uuid())) {
                    Some(Arc::new(RwLock::new(RdfPredicateController {
                        model: predicate.clone(),
                        source: source_controller,
                        destination: dest_controller,
                        center_point: None,
                        source_points: vec![vec![egui::Pos2::ZERO]],
                        dest_points: vec![vec![egui::Pos2::ZERO]],
                    })))
                } else { None };
                
                self.result = PartialRdfElement::None;
                predicate_controller
            },
            PartialRdfElement::Graph{a, b: Some(b)} => {
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
                    bounds_rect: egui::Rect::from_two_pos(*a, *b),
                }));
                
                self.result = PartialRdfElement::None;
                Some(graph_controller)
            }
            _ => { None },
        }
    }
    
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match self.result {
            PartialRdfElement::Predicate{source_pos, ..} => {
                canvas.draw_line(
                    [source_pos, pos],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                );
            },
            PartialRdfElement::Graph{a, ..} => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a, pos),
                    egui::Rounding::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                );
            },
            _ => {},
        }
    }
}

pub trait RdfElementController: ElementController {
    fn show_properties(&mut self, _parent: &RdfDiagramController, _ui: &mut egui::Ui) {}
    fn list_in_project_hierarchy(&self, _parent: &RdfDiagramController, _ui: &mut egui::Ui) {}

    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool { false }
    fn connection_target_name(&self) -> Option<String> { None }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn RdfTool)>) -> TargettingStatus;
    fn click(&mut self, tool: Option<&mut Box<dyn RdfTool>>, pos: egui::Pos2, modifiers: ModifierKeys) -> ClickHandlingStatus;
    fn drag(&mut self, tool: Option<&mut Box<dyn RdfTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus;
}

pub trait RdfContainerController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn RdfElementController>>>;
}

pub struct RdfDiagramController {
    pub model: Arc<RwLock<RdfDiagram>>,
    // NOTE: using Arc<RwLock<_>> seems inefficient, but using Boxes leads to BC issues
    //       and doesn't improve the performance in significant way
    pub owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn RdfElementController>>>,
    
    pub _layers: Vec<bool>,
    
    pub camera_offset: egui::Pos2,
    pub camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    selected_elements: HashSet<uuid::Uuid>,
    current_tool: Option<Box<dyn RdfTool>>,
}

impl RdfDiagramController {
    pub fn new(
        model: Arc<RwLock<RdfDiagram>>,
        owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn RdfElementController>>>,
    ) -> Self {
        Self {
            model,
            owned_controllers,
            
            _layers: vec![true],
            
            camera_offset: egui::Pos2::ZERO,
            camera_scale: 1.0,
            last_unhandled_mouse_pos: None,
            selected_elements: HashSet::new(),
            current_tool: None,
        }
    }
    
    fn last_selected_element(&self) -> Option<Arc<RwLock<dyn RdfElementController>>> {
        if self.selected_elements.len() != 1 {
            return None;
        }
        let id = self.selected_elements.iter().next()?;
        self.owned_controllers.get(&id).cloned()
    }
    
    fn outgoing_for<'a>(&'a self, uuid: &'a uuid::Uuid) -> Box<dyn Iterator<Item=Arc<RwLock<dyn RdfElementController>>> + 'a> {
        Box::new(self.owned_controllers.iter()
                    .filter(|e| e.1.read().unwrap().is_connection_from(uuid))
                    .map(|e| e.1.clone()))
    }
}

impl DiagramController for RdfDiagramController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().name.clone()
    }
    
    fn new_ui_canvas(&self, ui: &mut egui::Ui) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>) {
        let canvas_pos = ui.next_widget_position();
        let canvas_size = ui.available_size();
        let canvas_rect = egui::Rect{ min: canvas_pos, max: canvas_pos + canvas_size };
        
        let (painter_response, painter) = ui.allocate_painter(ui.available_size(),
                                                              egui::Sense::click_and_drag());
        let ui_canvas = UiCanvas::new(
            painter,
            canvas_rect,
            self.camera_offset,
            self.camera_scale,
            ui.ctx().pointer_interact_pos()
                .map(|e| ((e - self.camera_offset - painter_response.rect.min.to_vec2()) / self.camera_scale).to_pos2()),
        );
        ui_canvas.clear(egui::Color32::WHITE);
        ui_canvas.draw_gridlines(
            Some((50.0, egui::Color32::from_rgb(220,220,220))),
            Some((50.0, egui::Color32::from_rgb(220,220,220))),
        );
        
        let inner_mouse = ui.ctx().pointer_interact_pos()
                .filter(|e| canvas_rect.contains(*e))
                .map(|e| ((e - self.camera_offset - canvas_pos.to_vec2()) / self.camera_scale).to_pos2());
        
        (
            Box::new(ui_canvas),
            painter_response,
            inner_mouse,
        )
    }
    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // Handle camera and element clicks/drags
        if response.clicked() {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.click(
                     ((pos - self.camera_offset - response.rect.min.to_vec2()) / self.camera_scale).to_pos2(),
                );
            }
        } else if response.dragged_by(egui::PointerButton::Middle) {
            self.camera_offset += response.drag_delta();
        } else if response.drag_started_by(egui::PointerButton::Primary) {
            self.last_unhandled_mouse_pos = ui.ctx().pointer_interact_pos();
        } else if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(cursor_pos) = &self.last_unhandled_mouse_pos {
                let last_down_pos =
                (*cursor_pos - self.camera_offset - response.rect.min.to_vec2())
                / self.camera_scale;
                self.drag(
                    last_down_pos.to_pos2(),
                    response.drag_delta() / self.camera_scale,
                );
                self.last_unhandled_mouse_pos = ui.ctx().pointer_interact_pos();
            }
        } else if response.drag_stopped() {
            self.last_unhandled_mouse_pos = None;
        }
        
        // Handle zoom
        if response.hovered() {
            let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta);
            
            let factor = if scroll_delta.y > 0.0 && self.camera_scale < 10.0 {
                1.5
            } else if scroll_delta.y < 0.0 && self.camera_scale > 0.01 {
                0.66
            } else { 0.0 };
            
            if factor != 0.0 {
                if let Some(cursor_pos) = ui.ctx().pointer_interact_pos() {
                    let old_factor = self.camera_scale;
                    self.camera_scale *= factor;
                    self.camera_offset -= 
                    ((cursor_pos - self.camera_offset - response.rect.min.to_vec2()) / old_factor)
                    * (self.camera_scale - old_factor);
                }
            }
        }
    }
    fn click(&mut self, pos: egui::Pos2) -> bool {
        let handled = self.owned_controllers.iter_mut()
            .find(|uc| uc.1.write().unwrap().click(self.current_tool.as_mut(), pos, ModifierKeys::NONE) == ClickHandlingStatus::Handled)
            .map(|uc| {self.selected_elements.insert(uc.0.clone());})
            .ok_or_else(|| {self.selected_elements.clear();})
            .is_ok();
        
        if !handled {
            if let Some(t) = self.current_tool.as_mut() {
                t.add_by_position(pos);
            }
        }
        let mut tool = self.current_tool.take();
        if let Some(new) = tool.as_mut().and_then(|e| e.try_construct(self)) {
            let uuid = new.read().unwrap().uuid();
            self.owned_controllers.insert(uuid, new);
            return true;
        }
        self.current_tool = tool;
        handled
    }
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        self.owned_controllers.iter_mut()
            .find(|uc| uc.1.write().unwrap().drag(self.current_tool.as_mut(), last_pos, delta) == DragHandlingStatus::Handled)
            .is_some()
    }
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        ui.label("asdf");
    }
    
    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        
        let stage = self.current_tool.as_ref().map(|e| e.initial_stage());
        let c = |s: RdfToolStage| -> egui::Color32 {
            if stage.is_some_and(|e| e == s) { egui::Color32::BLUE } else { egui::Color32::BLACK }
        };
        
        for cat in [&[(RdfToolStage::Select, "Select"), (RdfToolStage::Move, "Move"),][..],
                    &[(RdfToolStage::Literal, "Literal"), (RdfToolStage::Node, "Node"),
                      (RdfToolStage::PredicateStart, "Predicate"), (RdfToolStage::GraphStart, "Graph"),][..],
                    &[(RdfToolStage::Note, "Note")][..]] {
            for (stage, name) in cat {
                if ui.add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage))).clicked() {
                    self.current_tool = Some(Box::new(NaiveRdfTool::new(*stage)));
                }
            }
            ui.separator();
        }
        
    }
    fn show_properties(&mut self, ui: &mut egui::Ui) {
        if let Some(element) = self.last_selected_element() {
            element.write().unwrap().show_properties(self, ui);
        } else {
            let mut model = self.model.write().unwrap();
        
            ui.label("Name:");
            let r1 = ui.add_sized((ui.available_width(), 20.0),
                                egui::TextEdit::singleline(&mut model.name));
            
            ui.label("Comment:");
            let r2 = ui.add_sized((ui.available_width(), 20.0),
                                egui::TextEdit::multiline(&mut model.comment));
            
            if r1.union(r2).changed() {
                model.notify_observers();
            }
        }
    }
    fn show_layers(&self, _ui: &mut egui::Ui) {
        // TODO: Layers???
    }
    
    fn list_in_project_hierarchy(&self, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();
        
        egui::CollapsingHeader::new(format!("{} ({})", model.name, model.uuid))
        .show(ui, |ui| {
            for uc in &self.owned_controllers {
                uc.1.read().unwrap().list_in_project_hierarchy(self, ui);
            }
        });
    }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, mouse_pos: Option<egui::Pos2>) {
        let tool = if let (Some(pos), Some(stage)) = (mouse_pos, self.current_tool.as_ref().map(|e| e.as_ref())) {
            Some((pos, stage))
        } else { None };
        let mut drawn_targetting = TargettingStatus::NotDrawn;
        
        self.owned_controllers.iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &tool) == TargettingStatus::Drawn { drawn_targetting = TargettingStatus::Drawn; });
        
        if let Some((pos, tool)) = tool {
            if drawn_targetting == TargettingStatus::NotDrawn {
                canvas.draw_rectangle(
                    egui::Rect::EVERYTHING,
                    egui::Rounding::ZERO,
                    tool.targetting_for_diagram(),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                );
                self.owned_controllers.iter_mut()
                    .filter(|_| true) // TODO: filter by layers
                    .for_each(|uc| { uc.1.write().unwrap().draw_in(canvas, &Some((pos, tool))); });
            }
            tool.draw_status_hint(canvas, pos);
        }
    }
}

impl RdfContainerController for RdfDiagramController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn RdfElementController>>> {
        self.owned_controllers.get(uuid).cloned()
    }
}

pub struct RdfGraphController {
    pub model: Arc<RwLock<RdfGraph>>,
    pub owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn RdfElementController>>>,
    
    pub bounds_rect: egui::Rect,
}

impl ElementController for RdfGraphController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().name.clone()
    }
    
    fn min_shape(&self) -> NHShape {
        NHShape::Rect{ inner: self.bounds_rect }
    }
    
    fn position(&self) -> egui::Pos2 {
        self.bounds_rect.center()
    }
}

impl RdfElementController for RdfGraphController {
    fn show_properties(&mut self, _parent: &RdfDiagramController, ui: &mut egui::Ui) {
        let mut model = self.model.write().unwrap();
        
        ui.label("IRI:");
        let r1 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.name));
        
        ui.label("Comment:");
        let r2 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.comment));
        
        if r1.union(r2).changed() {
            model.notify_observers();
        }
    }
    fn list_in_project_hierarchy(&self, parent: &RdfDiagramController, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();
    
        egui::CollapsingHeader::new(format!("{} ({})", model.name, model.uuid))
        .show(ui, |_ui| {
            // TODO: child elements in project view
            /*for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }*/
        });
    }
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn RdfTool)>) -> TargettingStatus {
        // Draw shape and text
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::Rounding::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        );
        
        canvas.draw_text(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            &self.model.read().unwrap().name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        
        let offset_tool = tool.map(|(p, t)| (p - self.bounds_rect.left_top().to_vec2(), t));
        let mut drawn_child_targetting = TargettingStatus::NotDrawn;
        
        canvas.offset_by(self.bounds_rect.left_top().to_vec2());
        self.owned_controllers.iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &offset_tool) == TargettingStatus::Drawn { drawn_child_targetting = TargettingStatus::Drawn; });
        canvas.offset_by(-self.bounds_rect.left_top().to_vec2());
        
        match (drawn_child_targetting, tool) {
            (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::Rounding::ZERO,
                    t.targetting_for_diagram(),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                );
                
                canvas.offset_by(self.bounds_rect.left_top().to_vec2());
                self.owned_controllers.iter_mut()
                    .filter(|_| true) // TODO: filter by layers
                    .for_each(|uc| { uc.1.write().unwrap().draw_in(canvas, &offset_tool); });
                canvas.offset_by(-self.bounds_rect.left_top().to_vec2());
                
                TargettingStatus::Drawn
            },
            _ => { drawn_child_targetting },
        }
    }
    
    fn click(&mut self, mut tool: Option<&mut Box<dyn RdfTool>>, pos: egui::Pos2, modifiers: ModifierKeys) -> ClickHandlingStatus {
        tool.as_mut().map(|e| e.offset_by(self.bounds_rect.left_top().to_vec2()));
        let offset_pos = pos - self.bounds_rect.left_top().to_vec2();
        
        let handled = self.owned_controllers.iter_mut()
            .find(|uc| match tool.take() {
                Some(inner) => {
                    let r = uc.1.write().unwrap().click(Some(inner), offset_pos, modifiers);
                    tool = Some(inner);
                    r
                },
                None => uc.1.write().unwrap().click(None, offset_pos, modifiers),
            } == ClickHandlingStatus::Handled)
            //.map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            //.ok_or_else(|| {self.last_selected_element = None;})
            .is_some();
        let handled = match handled {
            true => ClickHandlingStatus::Handled,
            false => ClickHandlingStatus::NotHandled,
        };
        
        tool.as_mut().map(|e| e.offset_by(-self.bounds_rect.left_top().to_vec2()));
        
        match (self.min_shape().contains(pos), tool) {
            (true, Some(tool)) => {
                tool.offset_by(self.bounds_rect.left_top().to_vec2());
                tool.add_by_position(pos);
                tool.offset_by(-self.bounds_rect.left_top().to_vec2());
                tool.add_graph(self.model.clone(), pos);
                
                if let Some(new) = tool.try_construct(self) {
                    let uuid = new.read().unwrap().uuid();
                    self.owned_controllers.insert(uuid, new);
                }
                return ClickHandlingStatus::Handled;
            },
            _ => {},
        }
        
        handled
    }
    fn drag(&mut self, mut tool: Option<&mut Box<dyn RdfTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
        tool.as_mut().map(|e| e.offset_by(self.bounds_rect.left_top().to_vec2()));
        let offset_pos = last_pos - self.bounds_rect.left_top().to_vec2();
        
        let handled = self.owned_controllers.iter_mut()
            .find(|uc| match tool.take() {
                Some(inner) => {
                    let r = uc.1.write().unwrap().drag(Some(inner), offset_pos, delta);
                    tool = Some(inner);
                    r
                },
                None => uc.1.write().unwrap().drag(None, offset_pos, delta),
            } == DragHandlingStatus::Handled)
            //.map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            //.ok_or_else(|| {self.last_selected_element = None;})
            .is_some();
        let handled = match handled {
            true => DragHandlingStatus::Handled,
            false => DragHandlingStatus::NotHandled,
        };
        
        tool.as_mut().map(|e| e.offset_by(-self.bounds_rect.left_top().to_vec2()));
        
        match (handled, self.min_shape().contains(last_pos)) {
            (DragHandlingStatus::NotHandled, true) => {
                self.bounds_rect.set_center(self.position() + delta);
                return DragHandlingStatus::Handled;
            },
            _ => {},
        }
        
        handled
    }
}

impl RdfContainerController for RdfGraphController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn RdfElementController>>> {
        self.owned_controllers.get(uuid).cloned()
    }
}


pub struct RdfNodeController {
    pub model: Arc<RwLock<RdfNode>>,
    
    pub position: egui::Pos2,
    pub bounds_radius: egui::Vec2,
}

impl ElementController for RdfNodeController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().iri.clone()
    }
    
    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse{ position: self.position, bounds_radius: self.bounds_radius }
    }
    
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl RdfElementController for RdfNodeController {
    fn show_properties(&mut self, _parent: &RdfDiagramController, ui: &mut egui::Ui) {
        let mut model = self.model.write().unwrap();
        
        ui.label("IRI:");
        let r1 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.iri));
        
        ui.label("Comment:");
        let r2 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.comment));
        
        if r1.union(r2).changed() {
            model.notify_observers();
        }
    }
    fn list_in_project_hierarchy(&self, parent: &RdfDiagramController, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();
    
        egui::CollapsingHeader::new(format!("{} ({})", model.iri, model.uuid))
        .show(ui, |ui| {
            for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }
        });
    }
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn RdfTool)>) -> TargettingStatus {
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
        );
        
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().unwrap().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        
        // Draw targetting ellipse
        if let Some(t) = tool.as_ref().filter(|e| self.min_shape().contains(e.0)).map(|e| e.1) {
            canvas.draw_ellipse(
                self.position,
                self.bounds_radius,
                t.targetting_for_node(),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            TargettingStatus::Drawn
        } else { TargettingStatus::NotDrawn }
    }
    
    fn click(&mut self, tool: Option<&mut Box<dyn RdfTool>>, pos: egui::Pos2, _modifiers: ModifierKeys) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) { return ClickHandlingStatus::NotHandled; }
        
        if let Some(tool) = tool {
            tool.add_node(self.model.clone(), pos);
        }
        
        ClickHandlingStatus::Handled
    }
    fn drag(&mut self, _tool: Option<&mut Box<dyn RdfTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
        if !self.min_shape().contains(last_pos) { return DragHandlingStatus::NotHandled; }
        
        self.position += delta;
        
        DragHandlingStatus::Handled
    }
}

pub struct RdfLiteralController {
    pub model: Arc<RwLock<RdfLiteral>>,
    
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl ElementController for RdfLiteralController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().content.clone()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rect{ inner: self.bounds_rect }
    }
    
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl RdfElementController for RdfLiteralController {
    fn show_properties(&mut self, _parent: &RdfDiagramController, ui: &mut egui::Ui) {
        let mut model = self.model.write().unwrap();
        
        ui.label("Content:");
        let r1 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.content));
        ui.label("Datatype:");
        let r2 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::singleline(&mut model.datatype));
        
        ui.label("Language:");
        let r3 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::singleline(&mut model.language));
        
        ui.label("Comment:");
        let r4 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.comment));
        
        if r1.union(r2).union(r3).union(r4).changed() {
            model.notify_observers();
        }
    }
    
    fn list_in_project_hierarchy(&self, _parent: &RdfDiagramController, ui: &mut egui::Ui) {
        ui.label(&self.model.read().unwrap().content);
    }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn RdfTool)>) -> TargettingStatus {
        // Draw shape and text
        self.bounds_rect = canvas.draw_class(
            self.position,
            None,
            &self.model.read().unwrap().content,
            None,
            &[],
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        );
        
        // Draw targetting rectangle
        if let Some(t) = tool.as_ref().filter(|e| self.min_shape().contains(e.0)).map(|e| e.1) {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::Rounding::ZERO,
                t.targetting_for_literal(),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            TargettingStatus::Drawn
        } else { TargettingStatus::NotDrawn }
    }
    
    fn click(&mut self, tool: Option<&mut Box<dyn RdfTool>>, pos: egui::Pos2, _modifiers: ModifierKeys) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) { return ClickHandlingStatus::NotHandled; }
        
        if let Some(tool) = tool {
            tool.add_literal(self.model.clone(), pos);
        }
        
        ClickHandlingStatus::Handled
    }
    fn drag(&mut self, _tool: Option<&mut Box<dyn RdfTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
        if !self.min_shape().contains(last_pos) { return DragHandlingStatus::NotHandled; }
        
        self.position += delta;
        
        DragHandlingStatus::Handled
    }
}

pub struct RdfPredicateController {
    pub model: Arc<RwLock<RdfPredicate>>,
    pub source: Arc<RwLock<dyn RdfElementController>>,
    pub destination: Arc<RwLock<dyn RdfElementController>>,
    pub center_point: Option<egui::Pos2>,
    pub source_points: Vec<Vec<egui::Pos2>>,
    pub dest_points: Vec<Vec<egui::Pos2>>,
}

impl RdfPredicateController {
    fn sources(&mut self) -> &mut [Vec<egui::Pos2>] {
        &mut self.source_points
    }
    fn destinations(&mut self) -> &mut [Vec<egui::Pos2>] {
        &mut self.dest_points
    }
}

impl ElementController for RdfPredicateController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().iri.clone()
    }
    
    fn min_shape(&self) -> NHShape {
        NHShape::Rect{ inner: egui::Rect::NOTHING }
    }
    fn max_shape(&self) -> NHShape {
        todo!()
    }
    
    fn position(&self) -> egui::Pos2 {
        match &self.center_point {
            Some(point) => *point,
            None => (self.source_points[0][0] + self.dest_points[0][0].to_vec2()) / 2.0,
        }
    }
}

impl RdfElementController for RdfPredicateController {
    fn show_properties(&mut self, _parent: &RdfDiagramController, ui: &mut egui::Ui) {
        let mut model = self.model.write().unwrap();
        
        ui.label("IRI:");
        let r1 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.iri));
        
        ui.label("Comment:");
        let r2 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.comment));
        
        let r3 = if ui.button("Switch source and destination").clicked()
            && /* TODO: must check if target isn't a literal */ true
        {
            (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            (self.source, self.destination) = (self.destination.clone(), self.source.clone());
            true
        } else { false };
        
        if r1.union(r2).changed() || r3 {
            model.notify_observers();
        }
    }

    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        self.source.read().unwrap().uuid() == *uuid
    }
    
    fn connection_target_name(&self) -> Option<String> { 
        Some(self.destination.read().unwrap().model_name())
    }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, _tool: &Option<(egui::Pos2, &dyn RdfTool)>) -> TargettingStatus {
        let (source_pos, source_bounds) = {
            let lock = self.source.read().unwrap();
            (lock.position(), lock.min_shape())
        };
        let (dest_pos, dest_bounds) = {
            let lock = self.destination.read().unwrap();
            (lock.position(), lock.min_shape())
        };
        match (source_bounds.center_intersect(
                self.source_points[0].get(1).map(|e| *e)
                    .or(self.center_point).unwrap_or(dest_pos)),
               dest_bounds.center_intersect(
                self.dest_points[0].get(1).map(|e| *e)
                    .or(self.center_point).unwrap_or(source_pos))) {
            (Some(source_intersect), Some(dest_intersect)) => {
                self.source_points[0][0] = source_intersect;
                self.dest_points[0][0] = dest_intersect;
                canvas.draw_multiconnection(
                    &[(ArrowheadType::None, Stroke::new_solid(1.0, egui::Color32::BLACK), &self.source_points[0], None)],
                    &[(ArrowheadType::OpenTriangle, Stroke::new_solid(1.0, egui::Color32::BLACK), &self.dest_points[0], None)],
                    self.position(),
                    Some(&self.model.read().unwrap().iri),
                );
            },
            _ => {},
        }
        TargettingStatus::NotDrawn
    }
    
    fn click(&mut self, _tool: Option<&mut Box<dyn RdfTool>>, _pos: egui::Pos2, _modifiers: ModifierKeys) -> ClickHandlingStatus {
        ClickHandlingStatus::NotHandled
    }
    fn drag(&mut self, _tool: Option<&mut Box<dyn RdfTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
        crate::common::controller::macros::multiconnection_element_drag!(
            self, last_pos, delta, center_point, sources, destinations, DragHandlingStatus::Handled
        );
        DragHandlingStatus::NotHandled
    }
}
