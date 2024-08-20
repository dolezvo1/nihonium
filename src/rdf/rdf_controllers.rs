
use eframe::egui;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use super::rdf_models::{RdfDiagram, RdfLiteral, RdfNode, RdfPredicate};
use crate::common::canvas::{self, ArrowheadType, Stroke, NHCanvas, UiCanvas, NHShape};
use crate::common::controller::{
    DiagramController, ElementController,
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
    
    let mut owned_controllers = HashMap::<_, Arc<RwLock<dyn RdfElementController>>>::new();
    owned_controllers.insert(node_uuid, node_controller);
    owned_controllers.insert(literal_uuid, literal_controller);
    owned_controllers.insert(predicate_uuid, predicate_controller);
    
    let diagram_uuid = uuid::Uuid::now_v7();
    let diagram = Arc::new(RwLock::new(RdfDiagram::new(
        diagram_uuid.clone(),
        format!("Demo RDF diagram {}", no),
        vec![node, literal, predicate],
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
    Graph,
    Note,
}

impl RdfToolStage {
    pub fn targettable_color(&self) -> egui::Color32 {
        match self {
            RdfToolStage::Move => egui::Color32::TRANSPARENT,
            _ => egui::Color32::from_rgba_unmultiplied(0,255,0,63)
        }
    }
    pub fn non_targettable_color(&self) -> egui::Color32 {
        match self {
            RdfToolStage::Move => egui::Color32::TRANSPARENT,
            _ => egui::Color32::from_rgba_unmultiplied(255,0,0,63)
        }
    }
    // pub fn targettable_cursor(&self) -> 
    // pub fn non_targettable_cursor(&self) ->
}

pub trait RdfTool {
    fn stage(&self) -> RdfToolStage;
}

pub struct NaiveRdfTool {
    stage: RdfToolStage,
}

impl RdfTool for NaiveRdfTool {
    fn stage(&self) -> RdfToolStage { self.stage }
}

pub trait RdfElementController: ElementController {
    fn show_properties(&mut self, _parent: &RdfDiagramController, _ui: &mut egui::Ui) {}
    fn list_in_project_hierarchy(&self, _parent: &RdfDiagramController, _ui: &mut egui::Ui) {}

    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool { false }
    fn connection_target_name(&self) -> Option<String> { None }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, RdfToolStage)>) -> bool;
}

pub struct RdfDiagramController {
    pub model: Arc<RwLock<RdfDiagram>>,
    pub owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn RdfElementController>>>,
    
    pub _layers: Vec<bool>,
    
    pub camera_offset: egui::Pos2,
    pub camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_selected_element: Option<uuid::Uuid>,
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
            last_selected_element: None,
            current_tool: None,
        }
    }
    
    fn last_selected_element(&mut self) -> Option<Arc<RwLock<dyn RdfElementController>>> {
        if let Some(last_selected_element) = self.last_selected_element {
            match self.owned_controllers.get(&last_selected_element) {
                Some(e) => Some(e.clone()),
                None => {
                    self.last_selected_element = None;
                    None
                }
            }
        } else {
            None
        }
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
                self.drag(
                     ((pos - self.camera_offset - response.rect.min.to_vec2()) / self.camera_scale).to_pos2(),
                    egui::Vec2::ZERO
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
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        self.owned_controllers.iter_mut()
            .find(|uc| uc.1.write().unwrap().drag(last_pos, delta))
            .map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            .ok_or_else(|| {self.last_selected_element = None;})
            .is_ok()
    }
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        ui.label("asdf");
    }
    
    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        
        if ui.add_sized([width, 20.0], egui::Button::new("Select")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::Select }));
        }
        
        if ui.add_sized([width, 20.0], egui::Button::new("Move")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::Move }));
        }
        
        ui.separator();
        
        if ui.add_sized([width, 20.0], egui::Button::new("Literal")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::Literal }));
        }
        if ui.add_sized([width, 20.0], egui::Button::new("Node")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::Node }));
        }
        if ui.add_sized([width, 20.0], egui::Button::new("Predicate")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::PredicateStart }));
        }
        
        ui.separator();
        
        if ui.add_sized([width, 20.0], egui::Button::new("Graph")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::Graph }));
        }
        
        ui.separator();
        
        if ui.add_sized([width, 20.0], egui::Button::new("Note")).clicked() {
            self.current_tool = Some(Box::new(NaiveRdfTool { stage: RdfToolStage::Note }));
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
        let tool = if let (Some(pos), Some(stage)) = (mouse_pos, self.current_tool.as_ref().map(|t| t.stage())) {
            Some((pos, stage))
        } else { None };
        let mut drawn_targetting = false;
        
        self.owned_controllers.iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &tool) { drawn_targetting = true; });
        
        if !drawn_targetting && tool.is_some() {
            let c = match tool.unwrap().1 {
                t @ (RdfToolStage::Literal | RdfToolStage::Node
                     | RdfToolStage::Graph | RdfToolStage::Note) => t.targettable_color(),
                t @ (RdfToolStage::Select | RdfToolStage::Move
                     | RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd) => t.non_targettable_color(),
            };
            canvas.draw_rectangle(
                egui::Rect::EVERYTHING,
                egui::Rounding::ZERO,
                c,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
        }
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
    crate::common::controller::macros::simple_element_drag!();
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
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, RdfToolStage)>) -> bool {
        self.bounds_rect = canvas.draw_class(
            self.position,
            None,
            &self.model.read().unwrap().content,
            None,
            &[],
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        );
        
        if let Some(t) = tool.as_ref().filter(|e| self.min_shape().contains(e.0)).map(|e| e.1) {
            let c = match t {
                RdfToolStage::Select | RdfToolStage::Move | RdfToolStage::PredicateEnd => t.targettable_color(),
                RdfToolStage::Literal | RdfToolStage::Node | RdfToolStage::PredicateStart
                | RdfToolStage::Graph | RdfToolStage::Note => t.non_targettable_color(),
            };
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::Rounding::ZERO,
                c,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            true
        } else { false }
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
    crate::common::controller::macros::simple_element_drag!();
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
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, RdfToolStage)>) -> bool {
        let text_bounds = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().unwrap().iri,
            20.0,
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
            20.0,
            egui::Color32::BLACK,
        );
        
        if let Some(t) = tool.as_ref().filter(|e| self.min_shape().contains(e.0)).map(|e| e.1) {
            let c = match t {
                RdfToolStage::Select | RdfToolStage::Move
                | RdfToolStage::PredicateStart | RdfToolStage::PredicateEnd => t.targettable_color(),
                RdfToolStage::Literal | RdfToolStage::Node
                | RdfToolStage::Graph | RdfToolStage::Note => t.non_targettable_color(),
            };
            canvas.draw_ellipse(
                self.position,
                self.bounds_radius,
                c,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            true
        } else { false }
    }
}

pub struct RdfPredicateController {
    pub model: Arc<RwLock<RdfPredicate>>,
    pub source: Arc<RwLock<dyn ElementController>>,
    pub destination: Arc<RwLock<dyn ElementController>>,
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
    crate::common::controller::macros::multiconnection_element_drag!(center_point, sources, destinations);
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
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, _tool: &Option<(egui::Pos2, RdfToolStage)>) -> bool {
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
        false
    }
}
