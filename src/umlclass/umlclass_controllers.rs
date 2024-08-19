
use eframe::egui;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use super::umlclass_models::{
    UmlClassDiagram, UmlClass, UmlClassLink, UmlClassLinkType,
};
use crate::common::canvas::{
    self, NHCanvas, UiCanvas, Drawable, NHShape
};
use crate::common::controller::{
    DiagramController, ElementController,
};
use crate::common::observer::Observable;

pub fn new(no: u32) -> (uuid::Uuid, Box<dyn DiagramController>) {
    let uuid = uuid::Uuid::now_v7();
                            
    let diagram = Arc::new(RwLock::new(UmlClassDiagram::new(
        uuid.clone(),
        format!("New UML class diagram {}", no),
        vec![],
    )));
    (
        uuid,
        Box::new(UmlClassDiagramController::new(
            diagram.clone(),
            HashMap::new(),
        )),
    )
}

pub fn demo(no: u32) -> (uuid::Uuid, Box<dyn DiagramController>) {
    // https://www.uml-diagrams.org/class-diagrams-overview.html
    // https://www.uml-diagrams.org/design-pattern-abstract-factory-uml-class-diagram-example.html
    
    let class_af_uuid = uuid::Uuid::now_v7();
    let class_af = Arc::new(RwLock::new(UmlClass::new(
        class_af_uuid.clone(),
        "AbstractFactory".to_owned(),
        "interface".to_string(),
    )));
    class_af.write().unwrap().functions = "+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned();
    let class_af_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_af.clone(),
        position: egui::Pos2::new(200.0, 150.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let class_cfx_uuid = uuid::Uuid::now_v7();
    let class_cfx = Arc::new(RwLock::new(UmlClass::new(
        class_cfx_uuid.clone(),
        "ConcreteFactoryX".to_owned(),
        "".to_string(),
    )));
    class_cfx.write().unwrap().functions = "+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned();
    let class_cfx_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_cfx.clone(),
        position: egui::Pos2::new(100.0, 250.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let class_cfy_uuid = uuid::Uuid::now_v7();
    let class_cfy = Arc::new(RwLock::new(UmlClass::new(
        class_cfy_uuid.clone(),
        "ConcreteFactoryY".to_owned(),
        "".to_string(),
    )));
    class_cfy.write().unwrap().functions = "+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned();
    let class_cfy_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_cfy.clone(),
        position: egui::Pos2::new(300.0, 250.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let realization_cfx_uuid = uuid::Uuid::now_v7();
    let realization_cfx = Arc::new(RwLock::new(UmlClassLink::new(
        realization_cfx_uuid.clone(),
        UmlClassLinkType::InterfaceRealization,
        class_cfx.clone(),
        class_af.clone(),
    )));
    let realization_cfx_controller = Arc::new(RwLock::new(UmlClassLinkController {
        model: realization_cfx.clone(),
        source: class_cfx_controller.clone(),
        destination: class_af_controller.clone(),
        center_point: None,
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));
    
    let association_cfy_uuid = uuid::Uuid::now_v7();
    let association_cfy = Arc::new(RwLock::new(UmlClassLink::new(
        association_cfy_uuid.clone(),
        UmlClassLinkType::InterfaceRealization,
        class_cfy.clone(),
        class_af.clone(),
    )));
    let association_cfy_controller = Arc::new(RwLock::new(UmlClassLinkController {
        model: association_cfy.clone(),
        source: class_cfy_controller.clone(),
        destination: class_af_controller.clone(),
        center_point: None,
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));
    
    let class_client_uuid = uuid::Uuid::now_v7();
    let class_client = Arc::new(RwLock::new(UmlClass::new(
        class_client_uuid.clone(),
        "Client".to_owned(),
        "".to_string(),
    )));
    let class_client_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_client.clone(),
        position: egui::Pos2::new(300.0, 50.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let usage_client_af_uuid = uuid::Uuid::now_v7();
    let usage_client_af = Arc::new(RwLock::new(UmlClassLink::new(
        usage_client_af_uuid.clone(),
        UmlClassLinkType::Usage,
        class_client.clone(),
        class_af.clone(),
    )));
    let usage_client_af_controller = Arc::new(RwLock::new(UmlClassLinkController {
        model: usage_client_af.clone(),
        source: class_client_controller.clone(),
        destination: class_af_controller.clone(),
        center_point: Some(egui::Pos2::new(200.0, 50.0)),
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));
    
    let class_producta_uuid = uuid::Uuid::now_v7();
    let class_producta = Arc::new(RwLock::new(UmlClass::new(
        class_producta_uuid.clone(),
        "ProductA".to_owned(),
        "interface".to_string(),
    )));
    let class_producta_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_producta.clone(),
        position: egui::Pos2::new(450.0, 150.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let usage_client_producta_uuid = uuid::Uuid::now_v7();
    let usage_client_producta = Arc::new(RwLock::new(UmlClassLink::new(
        usage_client_producta_uuid.clone(),
        UmlClassLinkType::Usage,
        class_client.clone(),
        class_producta.clone(),
    )));
    let usage_client_producta_controller = Arc::new(RwLock::new(UmlClassLinkController {
        model: usage_client_producta.clone(),
        source: class_client_controller.clone(),
        destination: class_producta_controller.clone(),
        center_point: Some(egui::Pos2::new(450.0, 52.0)),
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));
    
    let class_productb_uuid = uuid::Uuid::now_v7();
    let class_productb = Arc::new(RwLock::new(UmlClass::new(
        class_productb_uuid.clone(),
        "ProductB".to_owned(),
        "interface".to_string(),
    )));
    let class_productb_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_productb.clone(),
        position: egui::Pos2::new(650.0, 150.0),
        bounds_rect: egui::Rect::ZERO,
    }));
    
    let usage_client_productb_uuid = uuid::Uuid::now_v7();
    let usage_client_productb = Arc::new(RwLock::new(UmlClassLink::new(
        usage_client_productb_uuid.clone(),
        UmlClassLinkType::Usage,
        class_client.clone(),
        class_productb.clone(),
    )));
    let usage_client_productb_controller = Arc::new(RwLock::new(UmlClassLinkController {
        model: usage_client_productb.clone(),
        source: class_client_controller.clone(),
        destination: class_productb_controller.clone(),
        center_point: Some(egui::Pos2::new(650.0, 48.0)),
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));
    
    let mut owned_controllers = HashMap::<_, Arc<RwLock<dyn UmlClassElementController>>>::new();
    owned_controllers.insert(class_af_uuid, class_af_controller);
    owned_controllers.insert(class_cfx_uuid, class_cfx_controller);
    owned_controllers.insert(class_cfy_uuid, class_cfy_controller);
    owned_controllers.insert(realization_cfx_uuid, realization_cfx_controller);
    owned_controllers.insert(association_cfy_uuid, association_cfy_controller);
    owned_controllers.insert(class_client_uuid, class_client_controller);
    owned_controllers.insert(usage_client_af_uuid, usage_client_af_controller);
    owned_controllers.insert(class_producta_uuid, class_producta_controller);
    owned_controllers.insert(usage_client_producta_uuid, usage_client_producta_controller);
    owned_controllers.insert(class_productb_uuid, class_productb_controller);
    owned_controllers.insert(usage_client_productb_uuid, usage_client_productb_controller);
    
    let diagram_uuid = uuid::Uuid::now_v7();
    let diagram2 = Arc::new(RwLock::new(UmlClassDiagram::new(
        diagram_uuid.clone(),
        format!("Demo UML class diagram {}", no),
        vec![class_af, class_cfx, class_cfy, realization_cfx, association_cfy,
            class_client, usage_client_af,
            class_producta, usage_client_producta,
            class_productb, usage_client_productb],
    )));
    (
        diagram_uuid,
        Box::new(UmlClassDiagramController::new(
            diagram2.clone(),
            owned_controllers,
        ))
    )
}

pub trait UmlClassElementController: ElementController {
    fn show_properties(&mut self, _parent: &UmlClassDiagramController, _ui: &mut egui::Ui) {}
    fn list_in_project_hierarchy(&self, _parent: &UmlClassDiagramController, _ui: &mut egui::Ui) {}

    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool { false }
    fn target_name(&self) -> Option<String> { None }
}

pub struct UmlClassDiagramController {
    model: Arc<RwLock<UmlClassDiagram>>,
    owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn UmlClassElementController>>>,
    
    layers: Vec<bool>,
    
    camera_offset: egui::Pos2,
    camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_selected_element: Option<uuid::Uuid>,
}

impl UmlClassDiagramController {
    pub fn new(
        model: Arc<RwLock<UmlClassDiagram>>,
        owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn UmlClassElementController>>>,
    ) -> Self {
        Self {
            model,
            owned_controllers,
            
            layers: vec![true],
            
            camera_offset: egui::Pos2::ZERO,
            camera_scale: 1.0,
            last_unhandled_mouse_pos: None,
            last_selected_element: None,
        }
    }
    
    fn last_selected_element(&mut self) -> Option<Arc<RwLock<dyn UmlClassElementController>>> {
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
    
    fn outgoing_for<'a>(&'a self, uuid: &'a uuid::Uuid) -> Box<dyn Iterator<Item=Arc<RwLock<dyn UmlClassElementController>>> + 'a> {
        Box::new(self.owned_controllers.iter()
                    .filter(|e| e.1.read().unwrap().is_connection_from(uuid))
                    .map(|e| e.1.clone()))
    }
}

impl Drawable for UmlClassDiagramController {
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas) {
        self.owned_controllers.iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| uc.1.write().unwrap().draw_in(canvas));
    }
}

impl DiagramController for UmlClassDiagramController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().name.clone()
    }
    
    fn new_ui_canvas(&self, ui: &mut egui::Ui) -> (Box<dyn NHCanvas>, egui::Response) {
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
                                 Some((50.0, egui::Color32::from_rgb(220,220,220)))
        );
        (Box::new(ui_canvas), painter_response)
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
    // TODO: this repeats a lot, move to trait?
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        self.owned_controllers.iter_mut()
            .find(|uc| uc.1.write().unwrap().drag(last_pos, delta))
            .map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            .ok_or_else(|| {self.last_selected_element = None;})
            .is_ok()
    }
    // TODO: the math is all wrong, also context_menu() doesn't actually
    //         correspond to the creation event (storing elements necessary)
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        if let Some(cursor_pos) = ui.ctx().pointer_interact_pos() {
            let cursor_pos = ((cursor_pos - self.camera_offset) / self.camera_scale).to_pos2();
            for uc in &self.owned_controllers {
                let c = uc.1.write().unwrap();
                if c.min_shape().contains(cursor_pos) {
                    if ui.button("Delete View").clicked() {
                        // TODO: remove controller
                    } else if ui.button("Delete Model").clicked() {
                        // TODO: what now?
                    }
                }
            }
        }
    }
    
    fn show_toolbar(&self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        
        if ui.add_sized([width, 20.0], egui::Button::new("Select")).clicked() {
            println!("asdf");
        }
        
        if ui.add_sized([width, 20.0], egui::Button::new("Move")).clicked() {
            println!("asdf");
        }
        
        ui.separator();
        
        if ui.add_sized([width, 20.0], egui::Button::new("UML Class")).clicked() {
            println!("asdf");
        }
        if ui.add_sized([width, 20.0], egui::Button::new("UML Association")).clicked() {
            println!("asdf");
        }
        
        ui.separator();
        
        if ui.add_sized([width, 20.0], egui::Button::new("Note")).clicked() {
            println!("asdf");
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
}

pub struct UmlClassController {
    pub model: Arc<RwLock<UmlClass>>,
    
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl Drawable for UmlClassController {
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas) {
        let read = self.model.read().unwrap();
        let stereotype = if read.stereotype != "" {
            Some(format!("<<{}>>", read.stereotype))
        } else { None };
        
        self.bounds_rect = canvas.draw_class(
            self.position,
            if let Some(stereotype) = &stereotype {
                Some(&stereotype)
            } else { None },
            &read.name,
            None,
            &[&read.parse_properties(), &read.parse_functions()],
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        );
    }
}

impl ElementController for UmlClassController {
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
        self.position
    }
    crate::common::controller::macros::simple_element_drag!();
}

impl UmlClassElementController for UmlClassController {
    fn show_properties(&mut self, _parent: &UmlClassDiagramController, ui: &mut egui::Ui) {
        let mut model = self.model.write().unwrap();
        
        ui.label("Name:");
        let r1 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.name));
        
        ui.label("Stereotype:");
        let r2 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.stereotype));
        
        ui.label("Properties:");
        let r3 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.properties));
        
        ui.label("Functions:");
        let r4 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.functions));
        
        ui.label("Comment:");
        let r5 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.comment));
        
        if r1.union(r2).union(r3).union(r4).union(r5).changed() {
            model.notify_observers();
        }
    }
    
    fn list_in_project_hierarchy(&self, parent: &UmlClassDiagramController, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();
    
        egui::CollapsingHeader::new(format!("{} ({})", model.name, model.uuid))
        .show(ui, |ui| {
            for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.target_name().unwrap()));
            }
        });
    }
}

pub struct UmlClassLinkController {
    pub model: Arc<RwLock<UmlClassLink>>,
    pub source: Arc<RwLock<dyn ElementController>>,
    pub destination: Arc<RwLock<dyn ElementController>>,
    pub center_point: Option<egui::Pos2>,
    pub source_points: Vec<Vec<egui::Pos2>>,
    pub dest_points: Vec<Vec<egui::Pos2>>,
}

impl UmlClassLinkController {
    fn sources(&mut self) -> &mut [Vec<egui::Pos2>] {
        &mut self.source_points
    }
    fn destinations(&mut self) -> &mut [Vec<egui::Pos2>] {
        &mut self.dest_points
    }
}

impl Drawable for UmlClassLinkController {
    crate::common::controller::macros::multiconnection_draw_in!();
}

impl ElementController for UmlClassLinkController {
    fn uuid(&self) -> uuid::Uuid {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> String {
        self.model.read().unwrap().link_type.name().to_string()
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

impl UmlClassElementController for UmlClassLinkController {
    fn show_properties(&mut self, _parent: &UmlClassDiagramController, ui: &mut egui::Ui) {
        let mut model = self.model.write().unwrap();
        
        ui.label("Link type:");
        let r1 = egui::ComboBox::from_id_source("link type")
            .selected_text(model.link_type.name())
            .show_ui(ui, |ui| {
                for sv in [UmlClassLinkType::Association,
                           UmlClassLinkType::Aggregation,
                           UmlClassLinkType::Composition,
                           UmlClassLinkType::Generalization,
                           UmlClassLinkType::InterfaceRealization,
                           UmlClassLinkType::Usage,] {
                    ui.selectable_value(&mut model.link_type, sv, sv.name());
                }
            }).response;
        
        ui.label("Source:");
        let r2 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::singleline(&mut model.source_arrowhead_label));
        ui.separator();
        
        ui.label("Destination:");
        let r3 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::singleline(&mut model.destination_arrowhead_label));
        ui.separator();
        
        /*
        TODO: Say I have an object A, point p, object B, set of objects C and set of objects D, all 2D rectangles. Say I want to find optimal position of object A such that it is as close as possible to point p while also not intersecting with any of objects of C, and while being closer to B than to any object in D. What would be your approach?
        */
        
        ui.label("Swap source and destination:");
        let r4 = if ui.button("Swap").clicked() {
            (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            (self.source, self.destination) = (self.destination.clone(), self.source.clone());
            (self.source_points, self.dest_points) = (self.dest_points.clone(), self.source_points.clone());
            true
        } else { false };
        ui.separator();
        
        ui.label("Comment:");
        let r5 = ui.add_sized((ui.available_width(), 20.0),
                              egui::TextEdit::multiline(&mut model.comment));
        
        if r1.union(r2).union(r3).union(r5).changed() || r4 {
            model.notify_observers();
        }
    }

    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        self.source.read().unwrap().uuid() == *uuid
    }
    
    fn target_name(&self) -> Option<String> { 
        Some(self.destination.read().unwrap().model_name())
    }
}
