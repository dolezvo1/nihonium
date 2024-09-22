
use eframe::egui;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use super::umlclass_models::{
    UmlClassDiagram, UmlClassPackage, UmlClassElement, UmlClass, UmlClassLink, UmlClassLinkType,
};
use crate::common::canvas::{
    self, NHCanvas, NHShape,
};
use crate::common::controller::{
    DiagramController, ElementController, KindedElement, Tool, DiagramControllerGen2, ElementControllerGen2, ContainerGen2,
    ClickHandlingStatus, DragHandlingStatus, ModifierKeys, TargettingStatus,
};
use crate::common::observer::Observable;

pub struct UmlClassQueryable {}

fn show_props_fun(model: &mut UmlClassDiagram, ui: &mut egui::Ui) {
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
fn tool_change_fun(tool: &mut Option<NaiveUmlClassTool>, ui: &mut egui::Ui) {
    let width = ui.available_width();
        
    let stage = tool.as_ref().map(|e| e.initial_stage());
    let c = |s: UmlClassToolStage| -> egui::Color32 {
        if stage.is_some_and(|e| e == s) { egui::Color32::BLUE } else { egui::Color32::BLACK }
    };
    
    for cat in [&[(UmlClassToolStage::Select, "Select"), (UmlClassToolStage::Move, "Move")][..],
                &[(UmlClassToolStage::Class, "Class"), (UmlClassToolStage::PackageStart, "Package")][..],
                &[(UmlClassToolStage::LinkStart{ link_type: UmlClassLinkType::Association }, "Association"),
                    (UmlClassToolStage::LinkStart{ link_type: UmlClassLinkType::InterfaceRealization }, "IntReal"),
                    (UmlClassToolStage::LinkStart{ link_type: UmlClassLinkType::Usage }, "Usage"),][..],
                &[(UmlClassToolStage::Note, "Note")][..],] {
        for (stage, name) in cat {
            if ui.add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage))).clicked() {
                *tool = Some(NaiveUmlClassTool::new(*stage));
            }
        }
        ui.separator();
    }
}

pub fn new(no: u32) -> (uuid::Uuid, Box<dyn DiagramController>) {
    let uuid = uuid::Uuid::now_v7();
                            
    let diagram = Arc::new(RwLock::new(UmlClassDiagram::new(
        uuid.clone(),
        format!("New UML class diagram {}", no),
        vec![],
    )));
    (
        uuid,
        Box::new(DiagramControllerGen2::new(
            diagram.clone(),
            HashMap::new(),
            UmlClassQueryable{},
            show_props_fun,
            tool_change_fun,
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
    
    let mut owned_controllers = HashMap::<_, Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool>>>>::new();
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
        Box::new(DiagramControllerGen2::new(
            diagram2.clone(),
            owned_controllers,
            UmlClassQueryable{},
            show_props_fun,
            tool_change_fun,
        ))
    )
}

#[derive(Clone, Copy)]
pub enum KindedUmlClassElement<'a> {
    Diagram{},
    Package{},
    Class{inner: &'a UmlClassController},
    Link{inner: &'a UmlClassLinkController},
}

impl<'a> KindedElement<'a> for KindedUmlClassElement<'a> {
    type DiagramType = DiagramControllerGen2<UmlClassDiagram, UmlClassQueryable, NaiveUmlClassTool>;
    
    fn diagram(_: &'a Self::DiagramType) -> Self {
        Self::Diagram{}
    }
    fn package() -> Self {
        Self::Package{}
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Select,
    Move,
    Class,
    LinkStart{ link_type: UmlClassLinkType },
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialUmlClassElement {
    None,
    Some(Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool>>>),
    Link{link_type: UmlClassLinkType, source: Arc<RwLock<dyn UmlClassElement>>,
         source_pos: egui::Pos2, dest: Option<Arc<RwLock<dyn UmlClassElement>>>},
    Package{a: egui::Pos2, a_display: egui::Pos2, b: Option<egui::Pos2>},
}

pub struct NaiveUmlClassTool {
    initial_stage: UmlClassToolStage,
    current_stage: UmlClassToolStage,
    offset: egui::Pos2,
    result: PartialUmlClassElement,
    construction_lock: bool,
}

impl NaiveUmlClassTool {
    pub fn new(initial_stage: UmlClassToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            offset: egui::Pos2::ZERO,
            result: PartialUmlClassElement::None,
            construction_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<UmlClassQueryable> for NaiveUmlClassTool {
    type KindedElement<'a> = KindedUmlClassElement<'a>;
    type Stage = UmlClassToolStage;
    
    fn initial_stage(&self) -> Self::Stage { self.initial_stage }

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32 {
        match controller {
            KindedUmlClassElement::Diagram{..} => match self.current_stage {
                UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
                UmlClassToolStage::Class | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::Select | UmlClassToolStage::LinkStart{..} | UmlClassToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
            },
            KindedUmlClassElement::Package{..} => match self.current_stage {
                UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
                UmlClassToolStage::Select | UmlClassToolStage::Class | UmlClassToolStage::Note => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart{..} | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::PackageStart | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            KindedUmlClassElement::Class{..} => match self.current_stage {
                UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
                UmlClassToolStage::Select | UmlClassToolStage::LinkStart{..} | UmlClassToolStage::LinkEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::Class | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            KindedUmlClassElement::Link{..} => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match self.result {
            PartialUmlClassElement::Link{source_pos, link_type, ..} => {
                canvas.draw_line(
                    [source_pos, pos],
                    // TODO: draw correct hint line type
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                );
            },
            PartialUmlClassElement::Package{a_display, ..} => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a_display, pos),
                    egui::Rounding::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                );
            },
            _ => {},
        }
    }
    
    fn offset_by(&mut self, delta: egui::Vec2) {
        self.offset += delta;
    }
    fn add_position(&mut self, pos: egui::Pos2) {
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
                self.result = PartialUmlClassElement::Package{a: pos, a_display: self.offset + pos.to_vec2(), b: None};
                self.current_stage = UmlClassToolStage::PackageEnd;
            },
            (UmlClassToolStage::PackageEnd, PartialUmlClassElement::Package{ref mut b, ..}) => {
                *b = Some(pos)
            },
            (UmlClassToolStage::Note, _) => {},
            _ => {},
        }
    }    
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>, pos: egui::Pos2) {
        match controller {
            KindedUmlClassElement::Diagram{..} => {},
            KindedUmlClassElement::Package{..} => {},
            KindedUmlClassElement::Class{inner} => match (self.current_stage, &mut self.result) {
                (UmlClassToolStage::LinkStart{ link_type }, PartialUmlClassElement::None) => {
                    self.result = PartialUmlClassElement::Link{link_type, source: inner.model.clone(), source_pos: self.offset + pos.to_vec2(), dest: None};
                    self.current_stage = UmlClassToolStage::LinkEnd;
                },
                (UmlClassToolStage::LinkEnd, PartialUmlClassElement::Link{ref mut dest, ..}) => {
                    *dest = Some(inner.model.clone());
                }
                _ => {}
            },
            KindedUmlClassElement::Link{..} => {},
        }
    }
    fn try_construct(&mut self, into: &dyn ContainerGen2<UmlClassQueryable, Self>) -> Option<Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, Self>>>> {
        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                self.result = PartialUmlClassElement::None;
                self.construction_lock = true;
                Some(x)
            }
            PartialUmlClassElement::Link{link_type, source, dest: Some(dest), ..} => {
                self.current_stage = UmlClassToolStage::LinkStart{ link_type: *link_type };
                
                let uuid = uuid::Uuid::now_v7();
                let association = Arc::new(RwLock::new(UmlClassLink::new(
                    uuid.clone(),
                    *link_type,
                    source.clone(),
                    dest.clone(),
                )));
                let association_controller: Option<Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, Self>>>>
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
                self.construction_lock = true;
                association_controller
            },
            PartialUmlClassElement::Package{a, b: Some(b), ..} => {
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
                self.construction_lock = true;
                Some(package_controller)
            }
            _ => { None },
        }
    }
    fn reset_constructed_state(&mut self) {
        self.construction_lock = false;
    }
}

pub trait UmlClassElementController: ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool> {
    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool { false }
    fn connection_target_name(&self) -> Option<String> { None }
}

pub struct UmlClassPackageController {
    pub model: Arc<RwLock<UmlClassPackage>>,
    pub owned_controllers: HashMap<uuid::Uuid, Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool>>>>,
    
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

impl ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool> for UmlClassPackageController {
    fn show_properties(&mut self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) {
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
    fn list_in_project_hierarchy(&self, parent: &UmlClassQueryable, ui: &mut egui::Ui) {
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
    fn draw_in(&mut self, q: &UmlClassQueryable, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>) -> TargettingStatus {
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
            .for_each(|uc| if uc.1.write().unwrap().draw_in(q, canvas, &offset_tool) == TargettingStatus::Drawn { drawn_child_targetting = TargettingStatus::Drawn; });
        canvas.offset_by(-self.bounds_rect.left_top().to_vec2());
        
        match (drawn_child_targetting, tool) {
            (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::Rounding::ZERO,
                    t.targetting_for_element(KindedUmlClassElement::Package{}),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                );
                
                canvas.offset_by(self.bounds_rect.left_top().to_vec2());
                self.owned_controllers.iter_mut()
                    .filter(|_| true) // TODO: filter by layers
                    .for_each(|uc| { uc.1.write().unwrap().draw_in(q, canvas, &offset_tool); });
                canvas.offset_by(-self.bounds_rect.left_top().to_vec2());
                
                TargettingStatus::Drawn
            },
            _ => { drawn_child_targetting },
        }
    }
    
    fn click(&mut self, mut tool: Option<&mut NaiveUmlClassTool>, pos: egui::Pos2, modifiers: ModifierKeys) -> ClickHandlingStatus {
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
                tool.add_position(pos);
                tool.offset_by(-self.bounds_rect.left_top().to_vec2());
                tool.add_element(KindedUmlClassElement::Package{}, pos);
                
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
    fn drag(&mut self, mut tool: Option<&mut NaiveUmlClassTool>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
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

impl ContainerGen2<UmlClassQueryable, NaiveUmlClassTool> for UmlClassPackageController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool>>>> {
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

impl ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool> for UmlClassController {
    fn show_properties(&mut self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) {
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
    
    fn list_in_project_hierarchy(&self, parent: &UmlClassQueryable, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();
    
        egui::CollapsingHeader::new(format!("{} ({})", model.name, model.uuid))
        .show(ui, |ui| {
            /* TODO:
            for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }
            */
        });
    }
    
    fn draw_in(&mut self,  _: &UmlClassQueryable, canvas: &mut dyn NHCanvas, tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>) -> TargettingStatus {
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
                t.targetting_for_element(KindedUmlClassElement::Class{inner: self}),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            );
            TargettingStatus::Drawn
        } else { TargettingStatus::NotDrawn }
    }
    
    fn click(&mut self, tool: Option<&mut NaiveUmlClassTool>, pos: egui::Pos2, modifiers: ModifierKeys) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) { return ClickHandlingStatus::NotHandled; }
        
        if let Some(tool) = tool {
            tool.add_element(KindedUmlClassElement::Class{inner: self}, pos);
        }
        
        ClickHandlingStatus::Handled
    }
    
    fn drag(&mut self, tool: Option<&mut NaiveUmlClassTool>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
        if !self.min_shape().contains(last_pos) { return DragHandlingStatus::NotHandled; }
        
        self.position += delta;
        
        DragHandlingStatus::Handled
    }
}

pub struct UmlClassLinkController {
    pub model: Arc<RwLock<UmlClassLink>>,
    pub source: Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool>>>,
    pub destination: Arc<RwLock<dyn ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool>>>,
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

impl ElementControllerGen2<UmlClassQueryable, NaiveUmlClassTool> for UmlClassLinkController {
    fn show_properties(&mut self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) {
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
    
    fn draw_in(&mut self, _: &UmlClassQueryable, canvas: &mut dyn NHCanvas, _tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>) -> TargettingStatus {
        crate::common::controller::macros::multiconnection_draw_in!(self, canvas);
        TargettingStatus::NotDrawn
    }
    
    fn click(&mut self, tool: Option<&mut NaiveUmlClassTool>, pos: egui::Pos2, modifiers: ModifierKeys) -> ClickHandlingStatus {
        ClickHandlingStatus::NotHandled
    }
    fn drag(&mut self, _tool: Option<&mut NaiveUmlClassTool>, last_pos: egui::Pos2, delta: egui::Vec2) -> DragHandlingStatus {
        crate::common::controller::macros::multiconnection_element_drag!(
            self, last_pos, delta, center_point, sources, destinations, DragHandlingStatus::Handled
        );
        DragHandlingStatus::NotHandled
    }
}

impl UmlClassElementController for UmlClassLinkController {
    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        self.source.read().unwrap().uuid() == *uuid
    }
    
    fn connection_target_name(&self) -> Option<String> { 
        Some(self.destination.read().unwrap().model_name())
    }
}
