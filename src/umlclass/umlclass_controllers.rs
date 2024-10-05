use super::umlclass_models::{
    UmlClass, UmlClassDiagram, UmlClassElement, UmlClassLink, UmlClassLinkType, UmlClassPackage,
    UmlClassStereotype,
};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ClickHandlingStatus, ContainerGen2, DiagramCommand, DiagramController, DiagramControllerGen2,
    DragHandlingStatus, ElementController, ElementControllerGen2, KindedElement, ModifierKeys,
    TargettingStatus, Tool,
};
use crate::common::observer::Observable;
use crate::CustomTab;
use crate::NHApp;
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

pub struct UmlClassQueryable {}

pub struct UmlClassDiagramBuffer {
    name: String,
    comment: String,
}

fn show_props_fun(
    model: &mut UmlClassDiagram,
    buffer_object: &mut UmlClassDiagramBuffer,
    ui: &mut egui::Ui,
) {
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
fn tool_change_fun(tool: &mut Option<NaiveUmlClassTool>, ui: &mut egui::Ui) {
    let width = ui.available_width();

    let stage = tool.as_ref().map(|e| e.initial_stage());
    let c = |s: UmlClassToolStage| -> egui::Color32 {
        if stage.is_some_and(|e| e == s) {
            egui::Color32::BLUE
        } else {
            egui::Color32::BLACK
        }
    };

    if ui
        .add_sized(
            [width, 20.0],
            egui::Button::new("Select").fill(if stage == None {
                egui::Color32::BLUE
            } else {
                egui::Color32::BLACK
            }),
        )
        .clicked()
    {
        *tool = None;
    }

    for cat in [
        &[(UmlClassToolStage::Move, "Move")][..],
        &[
            (UmlClassToolStage::Class, "Class"),
            (UmlClassToolStage::PackageStart, "Package"),
        ][..],
        &[
            (
                UmlClassToolStage::LinkStart {
                    link_type: UmlClassLinkType::Association,
                },
                "Association",
            ),
            (
                UmlClassToolStage::LinkStart {
                    link_type: UmlClassLinkType::InterfaceRealization,
                },
                "IntReal",
            ),
            (
                UmlClassToolStage::LinkStart {
                    link_type: UmlClassLinkType::Usage,
                },
                "Usage",
            ),
        ][..],
        &[(UmlClassToolStage::Note, "Note")][..],
    ] {
        for (stage, name) in cat {
            if ui
                .add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage)))
                .clicked()
            {
                *tool = Some(NaiveUmlClassTool::new(*stage));
            }
        }
        ui.separator();
    }
}
fn menubar_options_fun(
    controller: &mut DiagramControllerGen2<
        UmlClassDiagram,
        dyn UmlClassElement,
        UmlClassQueryable,
        UmlClassDiagramBuffer,
        NaiveUmlClassTool,
    >,
    context: &mut NHApp,
    ui: &mut egui::Ui,
) {
    if ui.button("PlantUML description").clicked() {
        let uuid = uuid::Uuid::now_v7();
        context.add_custom_tab(
            uuid,
            Arc::new(RwLock::new(PlantUmlTab {
                diagram: controller.model(),
                plantuml_description: "".to_owned(),
            })),
        );
    }
}

struct PlantUmlTab {
    diagram: Arc<RwLock<UmlClassDiagram>>,
    plantuml_description: String,
}

impl CustomTab for PlantUmlTab {
    fn title(&self) -> String {
        "PlantUML description".to_owned()
    }

    fn show(&mut self, /*context: &mut NHApp,*/ ui: &mut egui::Ui) {
        if ui.button("Refresh").clicked() {
            let diagram = self.diagram.read().unwrap();
            self.plantuml_description = diagram.plantuml();
        }

        ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.plantuml_description.as_str()),
        );
    }
}

pub fn new(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let uuid = uuid::Uuid::now_v7();
    let name = format!("New UML class diagram {}", no);
    let diagram = Arc::new(RwLock::new(UmlClassDiagram::new(
        uuid.clone(),
        name.clone(),
        vec![],
    )));
    (
        uuid,
        Arc::new(RwLock::new(DiagramControllerGen2::new(
            diagram.clone(),
            HashMap::new(),
            UmlClassQueryable {},
            UmlClassDiagramBuffer {
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
    // https://www.uml-diagrams.org/class-diagrams-overview.html
    // https://www.uml-diagrams.org/design-pattern-abstract-factory-uml-class-diagram-example.html

    let class_af_uuid = uuid::Uuid::now_v7();
    let class_af = Arc::new(RwLock::new(UmlClass::new(
        class_af_uuid.clone(),
        UmlClassStereotype::Interface,
        "AbstractFactory".to_owned(),
    )));
    class_af.write().unwrap().functions =
        Arc::new("+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned());
    let class_af_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_af.clone(),
        stereotype_buffer: UmlClassStereotype::Interface,
        name_buffer: "AbstractFactory".to_owned(),
        properties_buffer: "".to_owned(),
        functions_buffer: "+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        position: egui::Pos2::new(200.0, 150.0),
        bounds_rect: egui::Rect::ZERO,
    }));

    let class_cfx_uuid = uuid::Uuid::now_v7();
    let class_cfx = Arc::new(RwLock::new(UmlClass::new(
        class_cfx_uuid.clone(),
        UmlClassStereotype::Class,
        "ConcreteFactoryX".to_owned(),
    )));
    class_cfx.write().unwrap().functions =
        Arc::new("+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned());
    let class_cfx_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_cfx.clone(),
        stereotype_buffer: UmlClassStereotype::Class,
        name_buffer: "ConcreteFactoryX".to_owned(),
        properties_buffer: "".to_owned(),
        functions_buffer: "+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        position: egui::Pos2::new(100.0, 250.0),
        bounds_rect: egui::Rect::ZERO,
    }));

    let class_cfy_uuid = uuid::Uuid::now_v7();
    let class_cfy = Arc::new(RwLock::new(UmlClass::new(
        class_cfy_uuid.clone(),
        UmlClassStereotype::Class,
        "ConcreteFactoryY".to_owned(),
    )));
    class_cfy.write().unwrap().functions =
        Arc::new("+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned());
    let class_cfy_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_cfy.clone(),
        stereotype_buffer: UmlClassStereotype::Class,
        name_buffer: "ConcreteFactoryY".to_owned(),
        properties_buffer: "".to_owned(),
        functions_buffer: "+createProductA(): ProductA\n+createProductB(): ProductB\n".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
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
        source_arrowhead_label_buffer: "".to_owned(),
        destination_arrowhead_label_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
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
        source_arrowhead_label_buffer: "".to_owned(),
        destination_arrowhead_label_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        center_point: None,
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));

    let class_client_uuid = uuid::Uuid::now_v7();
    let class_client = Arc::new(RwLock::new(UmlClass::new(
        class_client_uuid.clone(),
        UmlClassStereotype::Class,
        "Client".to_owned(),
    )));
    let class_client_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_client.clone(),
        stereotype_buffer: UmlClassStereotype::Class,
        name_buffer: "Client".to_owned(),
        properties_buffer: "".to_owned(),
        functions_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
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
        source_arrowhead_label_buffer: "".to_owned(),
        destination_arrowhead_label_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        center_point: Some(egui::Pos2::new(200.0, 50.0)),
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));

    let class_producta_uuid = uuid::Uuid::now_v7();
    let class_producta = Arc::new(RwLock::new(UmlClass::new(
        class_producta_uuid.clone(),
        UmlClassStereotype::Interface,
        "ProductA".to_owned(),
    )));
    let class_producta_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_producta.clone(),
        stereotype_buffer: UmlClassStereotype::Interface,
        name_buffer: "ProductA".to_owned(),
        properties_buffer: "".to_owned(),
        functions_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
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
        source_arrowhead_label_buffer: "".to_owned(),
        destination_arrowhead_label_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        center_point: Some(egui::Pos2::new(450.0, 52.0)),
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));

    let class_productb_uuid = uuid::Uuid::now_v7();
    let class_productb = Arc::new(RwLock::new(UmlClass::new(
        class_productb_uuid.clone(),
        UmlClassStereotype::Interface,
        "ProductB".to_owned(),
    )));
    let class_productb_controller = Arc::new(RwLock::new(UmlClassController {
        model: class_productb.clone(),
        stereotype_buffer: UmlClassStereotype::Interface,
        name_buffer: "ProductB".to_owned(),
        properties_buffer: "".to_owned(),
        functions_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
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
        source_arrowhead_label_buffer: "".to_owned(),
        destination_arrowhead_label_buffer: "".to_owned(),
        comment_buffer: "".to_owned(),

        highlight: canvas::Highlight::NONE,
        center_point: Some(egui::Pos2::new(650.0, 48.0)),
        source_points: vec![vec![egui::Pos2::ZERO]],
        dest_points: vec![vec![egui::Pos2::ZERO]],
    }));

    let mut owned_controllers = HashMap::<
        _,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                >,
            >,
        >,
    >::new();
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
    let name = format!("Demo UML class diagram {}", no);
    let diagram2 = Arc::new(RwLock::new(UmlClassDiagram::new(
        diagram_uuid.clone(),
        name.clone(),
        vec![
            class_af,
            class_cfx,
            class_cfy,
            realization_cfx,
            association_cfy,
            class_client,
            usage_client_af,
            class_producta,
            usage_client_producta,
            class_productb,
            usage_client_productb,
        ],
    )));
    (
        diagram_uuid,
        Arc::new(RwLock::new(DiagramControllerGen2::new(
            diagram2.clone(),
            owned_controllers,
            UmlClassQueryable {},
            UmlClassDiagramBuffer {
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
pub enum KindedUmlClassElement<'a> {
    Diagram {},
    Package {},
    Class { inner: &'a UmlClassController },
    Link { inner: &'a UmlClassLinkController },
}

impl<'a> KindedElement<'a> for KindedUmlClassElement<'a> {
    type DiagramType = DiagramControllerGen2<
        UmlClassDiagram,
        dyn UmlClassElement,
        UmlClassQueryable,
        UmlClassDiagramBuffer,
        NaiveUmlClassTool,
    >;

    fn diagram(_: &'a Self::DiagramType) -> Self {
        Self::Diagram {}
    }
    fn package() -> Self {
        Self::Package {}
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Select,
    Move,
    Class,
    LinkStart { link_type: UmlClassLinkType },
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialUmlClassElement {
    None,
    Some(
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                >,
            >,
        >,
    ),
    Link {
        link_type: UmlClassLinkType,
        source: Arc<RwLock<dyn UmlClassElement>>,
        source_pos: egui::Pos2,
        dest: Option<Arc<RwLock<dyn UmlClassElement>>>,
    },
    Package {
        a: egui::Pos2,
        a_display: egui::Pos2,
        b: Option<egui::Pos2>,
    },
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

impl Tool<dyn UmlClassElement, UmlClassQueryable> for NaiveUmlClassTool {
    type KindedElement<'a> = KindedUmlClassElement<'a>;
    type Stage = UmlClassToolStage;

    fn initial_stage(&self) -> Self::Stage {
        self.initial_stage
    }

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32 {
        match controller {
            KindedUmlClassElement::Diagram { .. } => match self.current_stage {
                UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::Select
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
            },
            KindedUmlClassElement::Package { .. } => match self.current_stage {
                UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
                UmlClassToolStage::Select
                | UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            KindedUmlClassElement::Class { .. } => match self.current_stage {
                UmlClassToolStage::Move => egui::Color32::TRANSPARENT,
                UmlClassToolStage::Select
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            KindedUmlClassElement::Link { .. } => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match self.result {
            PartialUmlClassElement::Link {
                source_pos,
                link_type,
                ..
            } => {
                canvas.draw_line(
                    [source_pos, pos],
                    // TODO: draw correct hint line type
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            PartialUmlClassElement::Package { a_display, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a_display, pos),
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
        let uuid = uuid::Uuid::now_v7();
        match (self.current_stage, &mut self.result) {
            (UmlClassToolStage::Class, _) => {
                let node = Arc::new(RwLock::new(UmlClass::new(
                    uuid,
                    UmlClassStereotype::Class,
                    "a class".to_owned(),
                )));
                self.result =
                    PartialUmlClassElement::Some(Arc::new(RwLock::new(UmlClassController {
                        model: node.clone(),
                        stereotype_buffer: UmlClassStereotype::Class,
                        name_buffer: "a class".to_owned(),
                        properties_buffer: "".to_owned(),
                        functions_buffer: "".to_owned(),
                        comment_buffer: "".to_owned(),

                        highlight: canvas::Highlight::NONE,
                        position: pos,
                        bounds_rect: egui::Rect::ZERO,
                    })));
            }
            (UmlClassToolStage::PackageStart, _) => {
                self.result = PartialUmlClassElement::Package {
                    a: pos,
                    a_display: self.offset + pos.to_vec2(),
                    b: None,
                };
                self.current_stage = UmlClassToolStage::PackageEnd;
            }
            (UmlClassToolStage::PackageEnd, PartialUmlClassElement::Package { ref mut b, .. }) => {
                *b = Some(pos)
            }
            (UmlClassToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>, pos: egui::Pos2) {
        match controller {
            KindedUmlClassElement::Diagram { .. } => {}
            KindedUmlClassElement::Package { .. } => {}
            KindedUmlClassElement::Class { inner } => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::Link {
                            link_type,
                            source: inner.model.clone(),
                            source_pos: self.offset + pos.to_vec2(),
                            dest: None,
                        };
                        self.current_stage = UmlClassToolStage::LinkEnd;
                    }
                    (
                        UmlClassToolStage::LinkEnd,
                        PartialUmlClassElement::Link { ref mut dest, .. },
                    ) => {
                        *dest = Some(inner.model.clone());
                    }
                    _ => {}
                }
            }
            KindedUmlClassElement::Link { .. } => {}
        }
    }
    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<dyn UmlClassElement, UmlClassQueryable, Self>,
    ) -> Option<Arc<RwLock<dyn ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, Self>>>>
    {
        if self.construction_lock {
            return None;
        }

        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                self.result = PartialUmlClassElement::None;
                self.construction_lock = true;
                Some(x)
            }
            PartialUmlClassElement::Link {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                self.current_stage = UmlClassToolStage::LinkStart {
                    link_type: *link_type,
                };

                let uuid = uuid::Uuid::now_v7();
                let association = Arc::new(RwLock::new(UmlClassLink::new(
                    uuid.clone(),
                    *link_type,
                    source.clone(),
                    dest.clone(),
                )));
                let association_controller: Option<
                    Arc<
                        RwLock<
                            dyn ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, Self>,
                        >,
                    >,
                > = if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source.read().unwrap().uuid()),
                    into.controller_for(&dest.read().unwrap().uuid()),
                ) {
                    Some(Arc::new(RwLock::new(UmlClassLinkController {
                        model: association.clone(),
                        source: source_controller,
                        destination: dest_controller,
                        source_arrowhead_label_buffer: "".to_owned(),
                        destination_arrowhead_label_buffer: "".to_owned(),
                        comment_buffer: "".to_owned(),

                        highlight: canvas::Highlight::NONE,
                        center_point: None,
                        source_points: vec![vec![egui::Pos2::ZERO]],
                        dest_points: vec![vec![egui::Pos2::ZERO]],
                    })))
                } else {
                    None
                };

                self.result = PartialUmlClassElement::None;
                self.construction_lock = true;
                association_controller
            }
            PartialUmlClassElement::Package { a, b: Some(b), .. } => {
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
                    selected_elements: HashSet::new(),
                    name_buffer: "a package".to_owned(),
                    comment_buffer: "".to_owned(),

                    highlight: canvas::Highlight::NONE,
                    bounds_rect: egui::Rect::from_two_pos(*a, *b),
                }));

                self.result = PartialUmlClassElement::None;
                self.construction_lock = true;
                Some(package_controller)
            }
            _ => None,
        }
    }
    fn reset_constructed_state(&mut self) {
        self.construction_lock = false;
    }
}

pub trait UmlClassElementController:
    ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>
{
    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool {
        false
    }
    fn connection_target_name(&self) -> Option<Arc<String>> {
        None
    }
}

pub struct UmlClassPackageController {
    pub model: Arc<RwLock<UmlClassPackage>>,
    pub owned_controllers: HashMap<
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                >,
            >,
        >,
    >,
    selected_elements: HashSet<uuid::Uuid>,

    name_buffer: String,
    comment_buffer: String,

    highlight: canvas::Highlight,
    pub bounds_rect: egui::Rect,
}

impl ElementController<dyn UmlClassElement> for UmlClassPackageController {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn UmlClassElement>> {
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

impl ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>
    for UmlClassPackageController
{
    fn show_properties(&mut self, parent: &UmlClassQueryable, ui: &mut egui::Ui) -> bool {
        if self
            .owned_controllers
            .iter()
            .find(|e| e.1.write().unwrap().show_properties(parent, ui))
            .is_some()
        {
            true
        } else if self.highlight.selected {
            ui.label("Name:");
            let r1 = ui.add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.name_buffer),
            );

            ui.label("Comment:");
            let r2 = ui.add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            );

            if r1.changed() || r2.changed() {
                let mut model = self.model.write().unwrap();

                if r1.changed() {
                    model.name = Arc::new(self.name_buffer.clone());
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
    fn list_in_project_hierarchy(&self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        egui::CollapsingHeader::new(format!("{} ({})", model.name, model.uuid)).show(ui, |_ui| {
            // TODO: child elements in project view
            /*for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }*/
        });
    }
    fn draw_in(
        &mut self,
        q: &UmlClassQueryable,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
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
            &self.model.read().unwrap().name,
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
                    t.targetting_for_element(KindedUmlClassElement::Package {}),
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
        mut tool: &mut Option<NaiveUmlClassTool>,
        commands: &mut Vec<DiagramCommand>,
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
                    uc.1.write().unwrap().click(
                        tool,
                        commands,
                        offset_pos,
                        modifiers,
                    ),
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
                tool.add_element(KindedUmlClassElement::Package {}, pos);

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
                        commands.push(DiagramCommand::UnselectAll);
                        commands.push(DiagramCommand::Select(*uc.0));
                    } else if self.selected_elements.contains(&uc.0) {
                        commands.push(DiagramCommand::Unselect(*uc.0));
                    } else {
                        commands.push(DiagramCommand::Select(*uc.0));
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
        tool: &mut Option<NaiveUmlClassTool>,
        commands: &mut Vec<DiagramCommand>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        tool.as_mut()
            .map(|e| e.offset_by(self.bounds_rect.left_top().to_vec2()));
        let offset_pos = last_pos - self.bounds_rect.left_top().to_vec2();

        let handled = self.owned_controllers.iter_mut()
            .find(|uc| uc.1.write().unwrap().drag(tool, commands, offset_pos, delta) == DragHandlingStatus::Handled)
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
                commands.push(DiagramCommand::MoveSelectedElements(delta));
            } else {
                self.bounds_rect.set_center(self.position() + delta);
            }
            return DragHandlingStatus::Handled;
        }

        handled
    }

    fn apply_command(&mut self, command: &DiagramCommand) {
        fn recurse(this: &mut UmlClassPackageController, command: &DiagramCommand) {
            for e in &this.owned_controllers {
                let mut e = e.1.write().unwrap();
                e.apply_command(command);
            }
        }

        match command {
            DiagramCommand::SelectAll => {
                self.highlight.selected = true;
                self.selected_elements = self.owned_controllers.iter().map(|e| *e.0).collect();
                recurse(self, command);
            }
            DiagramCommand::UnselectAll => {
                self.highlight.selected = false;
                self.selected_elements.clear();
                recurse(self, command);
            }
            DiagramCommand::Select(uuid) => {
                if *self.uuid() == *uuid {
                    self.highlight.selected = true;
                } else if let Some(e) = self.owned_controllers.get(&uuid) {
                    self.selected_elements.insert(*uuid);
                    e.write().unwrap().apply_command(command);
                } else {
                    recurse(self, command);
                }
            }
            DiagramCommand::Unselect(uuid) => {
                if *self.uuid() == *uuid {
                    self.highlight.selected = false;
                } else if let Some(e) = self.owned_controllers.get(&uuid) {
                    self.selected_elements.remove(uuid);
                    e.write().unwrap().apply_command(command);
                } else {
                    recurse(self, command);
                }
            }
            DiagramCommand::MoveSelectedElements(delta) => {
                if self.highlight.selected {
                    self.bounds_rect.set_center(self.position() + *delta);
                } else {
                    recurse(self, command);
                }
            }
        }
    }
}

impl ContainerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>
    for UmlClassPackageController
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                >,
            >,
        >,
    > {
        self.owned_controllers.get(uuid).cloned()
    }
}

pub struct UmlClassController {
    pub model: Arc<RwLock<UmlClass>>,

    stereotype_buffer: UmlClassStereotype,
    name_buffer: String,
    properties_buffer: String,
    functions_buffer: String,
    comment_buffer: String,

    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl ElementController<dyn UmlClassElement> for UmlClassController {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn UmlClassElement>> {
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

impl ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>
    for UmlClassController
{
    fn show_properties(&mut self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Stereotype:");
        let mut r1 = false;
        egui::ComboBox::from_id_source("Stereotype:")
            .selected_text(self.stereotype_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    UmlClassStereotype::Abstract,
                    UmlClassStereotype::AbstractClass,
                    UmlClassStereotype::Class,
                    UmlClassStereotype::Entity,
                    UmlClassStereotype::Enum,
                    UmlClassStereotype::Interface,
                ] {
                    if ui
                        .selectable_value(&mut self.stereotype_buffer, value, value.char())
                        .clicked()
                    {
                        r1 = true;
                    }
                }
            });

        ui.label("Name:");
        let r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.name_buffer),
        );

        ui.label("Properties:");
        let r3 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.properties_buffer),
        );

        ui.label("Functions:");
        let r4 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.functions_buffer),
        );

        ui.label("Comment:");
        let r5 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.comment_buffer),
        );

        if r1 || r2.changed() || r3.changed() || r4.changed() || r5.changed() {
            let mut model = self.model.write().unwrap();

            if r1 {
                model.stereotype = self.stereotype_buffer.clone();
            }

            if r2.changed() {
                model.name = Arc::new(self.name_buffer.clone());
            }

            if r3.changed() {
                model.properties = Arc::new(self.properties_buffer.clone());
            }

            if r4.changed() {
                model.functions = Arc::new(self.functions_buffer.clone());
            }

            if r5.changed() {
                model.comment = Arc::new(self.comment_buffer.clone());
            }

            model.notify_observers();
        }

        true
    }

    fn list_in_project_hierarchy(&self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        egui::CollapsingHeader::new(format!("{} ({})", model.name, model.uuid)).show(ui, |_ui| {
            /* TODO:
            for connection in parent.outgoing_for(&model.uuid) {
                let connection = connection.read().unwrap();
                ui.label(format!("{} (-> {})", connection.model_name(), connection.connection_target_name().unwrap()));
            }
            */
        });
    }

    fn draw_in(
        &mut self,
        _: &UmlClassQueryable,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
    ) -> TargettingStatus {
        let read = self.model.read().unwrap();

        self.bounds_rect = canvas.draw_class(
            self.position,
            Some(read.stereotype.char()),
            &read.name,
            None,
            &[&read.parse_properties(), &read.parse_functions()],
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
                t.targetting_for_element(KindedUmlClassElement::Class { inner: self }),
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
        tool: &mut Option<NaiveUmlClassTool>,
        _commands: &mut Vec<DiagramCommand>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) {
            return ClickHandlingStatus::NotHandled;
        }

        if let Some(tool) = tool {
            tool.add_element(KindedUmlClassElement::Class { inner: self }, pos);
        } else {
            if !modifiers.command {
                self.highlight.selected = true;
            } else {
                self.highlight.selected = !self.highlight.selected;
            }
        }

        ClickHandlingStatus::HandledByElement
    }

    fn drag(
        &mut self,
        _tool: &mut Option<NaiveUmlClassTool>,
        commands: &mut Vec<DiagramCommand>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        if !self.min_shape().contains(last_pos) {
            return DragHandlingStatus::NotHandled;
        }

        if self.highlight.selected {
            commands.push(DiagramCommand::MoveSelectedElements(delta));
        } else {
            self.position += delta;
        }

        DragHandlingStatus::Handled
    }

    fn apply_command(&mut self, command: &DiagramCommand) {
        match command {
            DiagramCommand::SelectAll => {
                self.highlight.selected = true;
            }
            DiagramCommand::UnselectAll => self.highlight.selected = false,
            DiagramCommand::Select(uuid) => {
                if *self.uuid() == *uuid {
                    self.highlight.selected = true;
                }
            }
            DiagramCommand::Unselect(uuid) => {
                if *self.uuid() == *uuid {
                    self.highlight.selected = false;
                }
            }
            DiagramCommand::MoveSelectedElements(delta) => {
                if self.highlight.selected {
                    self.position += *delta;
                }
            }
        }
    }
}

pub struct UmlClassLinkController {
    pub model: Arc<RwLock<UmlClassLink>>,
    pub source: Arc<
        RwLock<
            dyn ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>,
        >,
    >,
    pub destination: Arc<
        RwLock<
            dyn ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>,
        >,
    >,
    source_arrowhead_label_buffer: String,
    destination_arrowhead_label_buffer: String,
    comment_buffer: String,

    highlight: canvas::Highlight,
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

impl ElementController<dyn UmlClassElement> for UmlClassLinkController {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().link_type.name()
    }
    fn model(&self) -> Arc<RwLock<dyn UmlClassElement>> {
        self.model.clone()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rect {
            inner: egui::Rect::NOTHING,
        }
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

impl ElementControllerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool>
    for UmlClassLinkController
{
    fn show_properties(&mut self, _parent: &UmlClassQueryable, ui: &mut egui::Ui) -> bool {
        if !self.highlight.selected {
            return false;
        }

        let mut model = self.model.write().unwrap();

        ui.label("Link type:");
        let r1 = egui::ComboBox::from_id_source("link type")
            .selected_text(&*model.link_type.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassLinkType::Association,
                    UmlClassLinkType::Aggregation,
                    UmlClassLinkType::Composition,
                    UmlClassLinkType::Generalization,
                    UmlClassLinkType::InterfaceRealization,
                    UmlClassLinkType::Usage,
                ] {
                    ui.selectable_value(&mut model.link_type, sv, &*sv.name());
                }
            })
            .response;

        ui.label("Source:");
        let r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.source_arrowhead_label_buffer),
        );
        ui.separator();

        ui.label("Destination:");
        let r3 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.destination_arrowhead_label_buffer),
        );
        ui.separator();

        ui.label("Swap source and destination:");
        let r4 = if ui.button("Swap").clicked() {
            (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            (self.source, self.destination) = (self.destination.clone(), self.source.clone());
            (self.source_points, self.dest_points) =
                (self.dest_points.clone(), self.source_points.clone());
            true
        } else {
            false
        };
        ui.separator();

        ui.label("Comment:");
        let r5 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.comment_buffer),
        );

        if r1.changed() || r2.changed() || r3.changed() || r4 || r5.changed() {
            if r2.changed() {
                model.source_arrowhead_label = Arc::new(self.source_arrowhead_label_buffer.clone());
            }

            if r3.changed() {
                model.destination_arrowhead_label =
                    Arc::new(self.destination_arrowhead_label_buffer.clone());
            }

            if r5.changed() {
                model.comment = Arc::new(self.comment_buffer.clone());
            }

            model.notify_observers();
        }

        true
    }

    fn draw_in(
        &mut self,
        _: &UmlClassQueryable,
        canvas: &mut dyn NHCanvas,
        _tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
    ) -> TargettingStatus {
        crate::common::controller::macros::multiconnection_draw_in!(self, canvas);
        TargettingStatus::NotDrawn
    }

    fn click(
        &mut self,
        _tool: &mut Option<NaiveUmlClassTool>,
        _commands: &mut Vec<DiagramCommand>,
        pos: egui::Pos2,
        _modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        crate::common::controller::macros::multiconnection_element_click!(
            self,
            pos,
            delta,
            center_point,
            sources,
            destinations,
            ClickHandlingStatus::HandledByElement
        );
        ClickHandlingStatus::NotHandled
    }
    fn drag(
        &mut self,
        _tool: &mut Option<NaiveUmlClassTool>,
        _commands: &mut Vec<DiagramCommand>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        crate::common::controller::macros::multiconnection_element_drag!(
            self,
            last_pos,
            delta,
            center_point,
            sources,
            destinations,
            DragHandlingStatus::Handled
        );
        DragHandlingStatus::NotHandled
    }

    fn apply_command(&mut self, command: &DiagramCommand) {
        match command {
            DiagramCommand::SelectAll => {
                self.highlight.selected = true;
            }
            DiagramCommand::UnselectAll => self.highlight.selected = false,
            DiagramCommand::Select(uuid) => {
                if *self.uuid() == *uuid {
                    self.highlight.selected = true;
                }
            }
            DiagramCommand::Unselect(uuid) => {
                if *self.uuid() == *uuid {
                    self.highlight.selected = false;
                }
            }
            DiagramCommand::MoveSelectedElements(delta) => {
                if self.highlight.selected {
                    if let Some(center_point) = self.center_point.as_mut() {
                        *center_point += *delta;
                    }

                    for path in self.source_points.iter_mut() {
                        for p in path.iter_mut() {
                            *p += *delta;
                        }
                    }

                    for path in self.dest_points.iter_mut() {
                        for p in path.iter_mut() {
                            *p += *delta;
                        }
                    }
                }
            }
        }
    }
}

impl UmlClassElementController for UmlClassLinkController {
    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        *self.source.read().unwrap().uuid() == *uuid
    }

    fn connection_target_name(&self) -> Option<Arc<String>> {
        Some(self.destination.read().unwrap().model_name())
    }
}
