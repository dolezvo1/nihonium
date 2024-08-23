
use eframe::egui;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use super::umlclass_models::{
    UmlClassDiagram, UmlClassPackage, UmlClassElement, UmlClass, UmlClassLink, UmlClassLinkType,
};
use crate::common::canvas::{
    self, NHCanvas, UiCanvas, NHShape,
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

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Select,
    Move,
    Class,
    LinkStart,
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialUmlClassElement {
    None,
    Some(Arc<RwLock<dyn UmlClassElementController>>),
    Link{source: Arc<RwLock<dyn UmlClassElement>>, dest: Option<Arc<RwLock<dyn UmlClassElement>>>},
    Package{a: egui::Pos2, b: Option<egui::Pos2>},
}

pub trait UmlClassTool {
    fn initial_stage(&self) -> UmlClassToolStage;

    fn targetting_for_class(&self) -> egui::Color32;
    fn targetting_for_package(&self) -> egui::Color32;
    fn targetting_for_diagram(&self) -> egui::Color32;
    
    fn add_by_position(&mut self, pos: egui::Pos2);
    fn add_class(&mut self, model: Arc<RwLock<UmlClass>>);
    fn add_package(&mut self, model: Arc<RwLock<UmlClassPackage>>);
    
    fn try_construct(&mut self, into: &dyn UmlClassContainerController) -> Option<Arc<RwLock<dyn UmlClassElementController>>>;
}

pub struct NaiveUmlClassTool {
    initial_stage: UmlClassToolStage,
    current_stage: UmlClassToolStage,
    result: PartialUmlClassElement,
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl UmlClassTool for NaiveUmlClassTool {
    fn initial_stage(&self) -> UmlClassToolStage { self.initial_stage }

    fn targetting_for_class(&self) -> egui::Color32 {
        match self.current_stage {
            UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
            UmlClassToolStage::Select | UmlClassToolStage::LinkStart | UmlClassToolStage::LinkEnd => TARGETTABLE_COLOR,
            UmlClassToolStage::Class | UmlClassToolStage::Note
            | UmlClassToolStage::PackageStart | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
        }
    }
    fn targetting_for_package(&self) -> egui::Color32 {
        match self.current_stage {
            UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
            UmlClassToolStage::Select | UmlClassToolStage::Class | UmlClassToolStage::Note => TARGETTABLE_COLOR,
            UmlClassToolStage::LinkStart | UmlClassToolStage::LinkEnd
            | UmlClassToolStage::PackageStart | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
        }
    }
    fn targetting_for_diagram(&self) -> egui::Color32 {
        match self.current_stage {
            UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
            UmlClassToolStage::Class | UmlClassToolStage::Note
            | UmlClassToolStage::PackageStart | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
            UmlClassToolStage::Select | UmlClassToolStage::LinkStart | UmlClassToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
        }
    }
    
    fn add_by_position(&mut self, pos: egui::Pos2) {
        let uuid = uuid::Uuid::now_v7();
        match (self.current_stage, &mut self.result) {
            (UmlClassToolStage::Class, _) => {
                let node = Arc::new(RwLock::new(UmlClass::new(
                    uuid,
                    "a class".to_owned(),
                    "".to_owned(),
                )));
                self.result = PartialUmlClassElement::Some(Arc::new(RwLock::new(UmlClassController {
                    model: node.clone(),
                    position: pos,
                    bounds_rect: egui::Rect::ZERO,
                })));
            },
            (UmlClassToolStage::PackageStart, _) => {
                self.result = PartialUmlClassElement::Package{a: pos, b: None};
                self.current_stage = UmlClassToolStage::PackageEnd;
            },
            (UmlClassToolStage::PackageEnd, PartialUmlClassElement::Package{ref mut b, ..}) => {
                *b = Some(pos)
            },
            (UmlClassToolStage::Note, _) => {},
            _ => {},
        }
    }
    fn add_class(&mut self, model: Arc<RwLock<UmlClass>>) {
        match (self.current_stage, &mut self.result) {
            (UmlClassToolStage::LinkStart, PartialUmlClassElement::None) => {
                self.result = PartialUmlClassElement::Link{source: model, dest: None};
                self.current_stage = UmlClassToolStage::LinkEnd;
            },
            (UmlClassToolStage::LinkEnd, PartialUmlClassElement::Link{ref mut dest, ..}) => {
                *dest = Some(model);
            }
            _ => {}
        }
    }
    fn add_package(&mut self, _model: Arc<RwLock<UmlClassPackage>>) {}
    
    fn try_construct(&mut self, into: &dyn UmlClassContainerController) -> Option<Arc<RwLock<dyn UmlClassElementController>>> {
        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                self.result = PartialUmlClassElement::None;
                Some(x)
            }
            PartialUmlClassElement::Link{source, dest: Some(dest)} => {
                self.current_stage = UmlClassToolStage::LinkStart;
                
                let uuid = uuid::Uuid::now_v7();
                let association = Arc::new(RwLock::new(UmlClassLink::new(
                    uuid.clone(),
                    UmlClassLinkType::Association,
                    source.clone(),
                    dest.clone(),
                )));
                let association_controller: Option<Arc<RwLock<dyn UmlClassElementController>>>
                    = if let (Some(source_controller), Some(dest_controller))
                        = (into.controller_for(&source.read().unwrap().uuid()), into.controller_for(&dest.read().unwrap().uuid())) {
                    Some(Arc::new(RwLock::new(UmlClassLinkController {
                        model: association.clone(),
                        source: source_controller,
                        destination: dest_controller,
                        center_point: None,
                        source_points: vec![vec![egui::Pos2::ZERO]],
                        dest_points: vec![vec![egui::Pos2::ZERO]],
                    })))
                } else { None };
                
                self.result = PartialUmlClassElement::None;
                association_controller
            },
            PartialUmlClassElement::Package{a, b: Some(b)} => {
                self.current_stage = UmlClassToolStage::PackageStart;
                
                let uuid = uuid::Uuid::now_v7();
                let package = Arc::new(RwLock::new(UmlClassPackage::new(
                    uuid.clone(),
                    "a package".to_owned(),
                    vec![],
                )));
                let package_controller = Arc::new(RwLock::new(UmlClassPackageController {
                    model: package.clone(),
                    owned_controllers: HashMap::new(),
                    bounds_rect: egui::Rect::from_two_pos(*a, *b),
                }));
                
                self.result = PartialUmlClassElement::None;
                Some(package_controller)
            }
            _ => { None },
        }
    }
}


pub trait UmlClassElementController: ElementController {
    fn show_properties(&mut self, _parent: &UmlClassDiagramController, _ui: &mut egui::Ui) {}
    fn list_in_project_hierarchy(&self, _parent: &UmlClassDiagramController, _ui: &mut egui::Ui) {}

    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool { false }
    fn connection_target_name(&self) -> Option<String> { None }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn UmlClassTool)>) -> bool;
    fn drag(&mut self, tool: Option<&mut Box<dyn UmlClassTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> bool;
}

pub trait UmlClassContainerController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn UmlClassElementController>>>;
}

pub struct UmlClassDiagramController {
    model: Arc<RwLock<UmlClassDiagram>>,
    owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn UmlClassElementController>>>,
    
    layers: Vec<bool>,
    
    camera_offset: egui::Pos2,
    camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_selected_element: Option<uuid::Uuid>,
    current_tool: Option<Box<dyn UmlClassTool>>,
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
            current_tool: None,
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

impl DiagramController for UmlClassDiagramController {
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
    // TODO: this repeats a lot, move to trait?
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        let handled = self.owned_controllers.iter_mut()
            .find(|uc| uc.1.write().unwrap().drag(self.current_tool.as_mut(), last_pos, delta))
            .map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            .ok_or_else(|| {self.last_selected_element = None;})
            .is_ok();
        
        if !handled && delta == egui::Vec2::ZERO {
            if let Some(t) = self.current_tool.as_mut() {
                t.add_by_position(last_pos);
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
    
    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        
        let stage = self.current_tool.as_ref().map(|e| e.initial_stage());
        let c = |s: UmlClassToolStage| -> egui::Color32 {
            if stage.is_some_and(|e| e == s) { egui::Color32::BLUE } else { egui::Color32::BLACK }
        };
        
        for cat in [&[(UmlClassToolStage::Select, "Select"), (UmlClassToolStage::Move, "Move")][..],
                    &[(UmlClassToolStage::Class, "Class"), (UmlClassToolStage::PackageStart, "Package")][..],
                    &[(UmlClassToolStage::LinkStart, "Association"),][..],
                    &[(UmlClassToolStage::Note, "Note")][..],] {
            for (stage, name) in cat {
                if ui.add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage))).clicked() {
                    self.current_tool = Some(Box::new(NaiveUmlClassTool { initial_stage: *stage, current_stage: *stage, result: PartialUmlClassElement::None }));
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
        let mut drawn_targetting = false;
        
        self.owned_controllers.iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &tool) { drawn_targetting = true; });
        
        if !drawn_targetting && tool.is_some() {
            canvas.draw_rectangle(
                egui::Rect::EVERYTHING,
                egui::Rounding::ZERO,
                tool.unwrap().1.targetting_for_diagram(),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            self.owned_controllers.iter_mut()
                .filter(|_| true) // TODO: filter by layers
                .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &tool) { drawn_targetting = true; });
        }
    }
}

impl UmlClassContainerController for UmlClassDiagramController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn UmlClassElementController>>> {
        self.owned_controllers.get(uuid).cloned()
    }
}

pub struct UmlClassPackageController {
    pub model: Arc<RwLock<UmlClassPackage>>,
    pub owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn UmlClassElementController>>>,
    
    pub bounds_rect: egui::Rect,
}

impl ElementController for UmlClassPackageController {
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

impl UmlClassElementController for UmlClassPackageController {
    fn show_properties(&mut self, _parent: &UmlClassDiagramController, ui: &mut egui::Ui) {
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
    fn list_in_project_hierarchy(&self, parent: &UmlClassDiagramController, ui: &mut egui::Ui) {
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
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn UmlClassTool)>) -> bool {
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
        let mut drawn_child_targetting = false;
        
        canvas.offset_by(self.bounds_rect.left_top().to_vec2());
        self.owned_controllers.iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &offset_tool) { drawn_child_targetting = true; });
        canvas.offset_by(-self.bounds_rect.left_top().to_vec2());
        
        match (drawn_child_targetting, tool) {
            (false, Some((pos, t))) if self.min_shape().contains(*pos) => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::Rounding::ZERO,
                    t.targetting_for_diagram(),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                );
                
                canvas.offset_by(self.bounds_rect.left_top().to_vec2());
                self.owned_controllers.iter_mut()
                    .filter(|_| true) // TODO: filter by layers
                    .for_each(|uc| if uc.1.write().unwrap().draw_in(canvas, &offset_tool) { drawn_child_targetting = true; });
                canvas.offset_by(-self.bounds_rect.left_top().to_vec2());
                true
            },
            _ => { drawn_child_targetting },
        }
    }
    
    fn drag(&mut self, mut tool: Option<&mut Box<dyn UmlClassTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        //if !self.min_shape().contains(last_pos) { return false; }
        
        let offset_pos = last_pos - self.bounds_rect.left_top().to_vec2();
        let handled = self.owned_controllers.iter_mut()
            .find(|uc| match tool.take() {
                Some(inner) => {
                    let r = uc.1.write().unwrap().drag(Some(inner), offset_pos, delta);
                    tool = Some(inner);
                    r
                },
                None => uc.1.write().unwrap().drag(None, offset_pos, delta),
            })
            //.map(|uc| {self.last_selected_element = Some(uc.0.clone());})
            //.ok_or_else(|| {self.last_selected_element = None;})
            .is_some();
        // TODO: drag children
        // otherwise drag self:
        
        match (handled, self.min_shape().contains(last_pos), tool, delta) {
            (_, true, Some(tool), egui::Vec2::ZERO) => {
                tool.add_by_position(last_pos - self.bounds_rect.left_top().to_vec2());
                tool.add_package(self.model.clone());
                
                if let Some(new) = tool.try_construct(self) {
                    let uuid = new.read().unwrap().uuid();
                    self.owned_controllers.insert(uuid, new);
                }
                return true;
            },
            (false, true, _, _) => {
                self.bounds_rect.set_center(self.position() + delta);
                return true;
            },
            _ => {},
        }
        
        handled
    }
}

impl UmlClassContainerController for UmlClassPackageController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn UmlClassElementController>>> {
        self.owned_controllers.get(uuid).cloned()
    }
}

pub struct UmlClassController {
    pub model: Arc<RwLock<UmlClass>>,
    
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
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
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }
        });
    }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &dyn UmlClassTool)>) -> bool {
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
        
        // Draw targetting rectangle
        if let Some(t) = tool.as_ref().filter(|e| self.min_shape().contains(e.0)).map(|e| e.1) {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::Rounding::ZERO,
                t.targetting_for_class(),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            true
        } else { false }
    }
    
    fn drag(&mut self, tool: Option<&mut Box<dyn UmlClassTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        if !self.min_shape().contains(last_pos) { return false; }
        
        match (tool, delta) {
            (Some(tool), egui::Vec2::ZERO) => {
                tool.add_class(self.model.clone());
            }
            _ => {
                self.position += delta;
            }
        }
        
        true
    }
}

pub struct UmlClassLinkController {
    pub model: Arc<RwLock<UmlClassLink>>,
    pub source: Arc<RwLock<dyn UmlClassElementController>>,
    pub destination: Arc<RwLock<dyn UmlClassElementController>>,
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
    
    fn connection_target_name(&self) -> Option<String> { 
        Some(self.destination.read().unwrap().model_name())
    }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, _tool: &Option<(egui::Pos2, &dyn UmlClassTool)>) -> bool {
        crate::common::controller::macros::multiconnection_draw_in!(self, canvas);
        false
    }
    
    fn drag(&mut self, _tool: Option<&mut Box<dyn UmlClassTool>>, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        crate::common::controller::macros::multiconnection_element_drag!(self, last_pos, delta, center_point, sources, destinations);
        false
    }
}
