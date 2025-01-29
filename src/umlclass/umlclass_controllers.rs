use super::umlclass_models::{
    UmlClass, UmlClassDiagram, UmlClassElement, UmlClassLink, UmlClassLinkType, UmlClassPackage,
    UmlClassStereotype,
};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ContainerGen2, DiagramController, DiagramControllerGen2, ElementController, ElementControllerGen2, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, ModifierKeys, MulticonnectionView, PackageView, SensitiveCommand, TargettingStatus, Tool, VertexInformation
};
use crate::CustomTab;
use crate::NHApp;
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock, Weak},
};

type ArcRwLockController = Arc<
    RwLock<
        dyn ElementControllerGen2<
            dyn UmlClassElement,
            UmlClassQueryable,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    >,
>;

pub struct UmlClassQueryable {}

#[derive(Clone)]
pub enum UmlClassPropChange {
    NameChange(Arc<String>),

    StereotypeChange(UmlClassStereotype),
    PropertiesChange(Arc<String>),
    FunctionsChange(Arc<String>),

    LinkTypeChange(UmlClassLinkType),
    SourceArrowheadLabelChange(Arc<String>),
    DestinationArrowheadLabelChange(Arc<String>),

    CommentChange(Arc<String>),
    PackageResize(egui::Vec2),
    FlipMulticonnection,
}

impl Debug for UmlClassPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassPropChange::???")
    }
}

impl TryInto<FlipMulticonnection> for &UmlClassPropChange {
    type Error = ();

    fn try_into(self) -> Result<FlipMulticonnection, ()> {
        match self {
            UmlClassPropChange::FlipMulticonnection => Ok(FlipMulticonnection {}),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
pub enum UmlClassElementOrVertex {
    Element(
        (
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        dyn UmlClassElement,
                        UmlClassQueryable,
                        NaiveUmlClassTool,
                        Self,
                        UmlClassPropChange,
                    >,
                >,
            >,
        ),
    ),
    Vertex(VertexInformation),
}

impl Debug for UmlClassElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassElementOrVertex::???")
    }
}

impl From<VertexInformation> for UmlClassElementOrVertex {
    fn from(v: VertexInformation) -> Self {
        UmlClassElementOrVertex::Vertex(v)
    }
}

impl TryInto<VertexInformation> for UmlClassElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<VertexInformation, ()> {
        match self {
            UmlClassElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl
    From<(
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                    UmlClassElementOrVertex,
                    UmlClassPropChange,
                >,
            >,
        >,
    )> for UmlClassElementOrVertex
{
    fn from(
        v: (
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        dyn UmlClassElement,
                        UmlClassQueryable,
                        NaiveUmlClassTool,
                        UmlClassElementOrVertex,
                        UmlClassPropChange,
                    >,
                >,
            >,
        ),
    ) -> Self {
        UmlClassElementOrVertex::Element(v)
    }
}

impl
    TryInto<(
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                    UmlClassElementOrVertex,
                    UmlClassPropChange,
                >,
            >,
        >,
    )> for UmlClassElementOrVertex
{
    type Error = ();

    fn try_into(
        self,
    ) -> Result<
        (
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        dyn UmlClassElement,
                        UmlClassQueryable,
                        NaiveUmlClassTool,
                        UmlClassElementOrVertex,
                        UmlClassPropChange,
                    >,
                >,
            >,
        ),
        (),
    > {
        match self {
            UmlClassElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

pub struct UmlClassDiagramBuffer {
    uuid: uuid::Uuid,
    name: String,
    comment: String,
}

fn show_props_fun(
    buffer: &mut UmlClassDiagramBuffer,
    ui: &mut egui::Ui,
    commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
            vec![UmlClassPropChange::NameChange(Arc::new(
                buffer.name.clone(),
            ))],
        ));
    }

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
            vec![UmlClassPropChange::CommentChange(Arc::new(
                buffer.comment.clone(),
            ))],
        ));
    }
}
fn apply_property_change_fun(
    buffer: &mut UmlClassDiagramBuffer,
    model: &mut UmlClassDiagram,
    command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
    undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
) {
    if let InsensitiveCommand::PropertyChange(_, properties) = command {
        for property in properties {
            match property {
                UmlClassPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(buffer.uuid).collect(),
                        vec![UmlClassPropChange::NameChange(model.name.clone())],
                    ));
                    buffer.name = (**name).clone();
                    model.name = name.clone();
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(buffer.uuid).collect(),
                        vec![UmlClassPropChange::CommentChange(model.comment.clone())],
                    ));
                    buffer.comment = (**comment).clone();
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
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
        UmlClassElementOrVertex,
        UmlClassPropChange,
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
    ui.separator();
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
        DiagramControllerGen2::new(
            diagram.clone(),
            HashMap::new(),
            UmlClassQueryable {},
            UmlClassDiagramBuffer {
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
    // https://www.uml-diagrams.org/class-diagrams-overview.html
    // https://www.uml-diagrams.org/design-pattern-abstract-factory-uml-class-diagram-example.html
    
    let (class_af_uuid, class_af, class_af_controller) = uml_class(
        UmlClassStereotype::Interface, "AbstractFactory",
        "", "+createProductA(): ProductA\n+createProductB(): ProductB\n",
        egui::Pos2::new(200.0, 150.0),
    );
    
    let (class_cfx_uuid, class_cfx, class_cfx_controller) = uml_class(
        UmlClassStereotype::Class, "ConcreteFactoryX",
        "", "+createProductA(): ProductA\n+createProductB(): ProductB\n",
        egui::Pos2::new(100.0, 250.0),
    );
    
    let (class_cfy_uuid, class_cfy, class_cfy_controller) = uml_class(
        UmlClassStereotype::Class, "ConcreteFactoryY",
        "", "+createProductA(): ProductA\n+createProductB(): ProductB\n",
        egui::Pos2::new(300.0, 250.0),
    );

    let (realization_cfx_uuid, realization_cfx, realization_cfx_controller) = umlclass_link(
        UmlClassLinkType::InterfaceRealization,
        None,
        (class_cfx.clone(), class_cfx_controller.clone()),
        (class_af.clone(), class_af_controller.clone()),
    );

    let (realization_cfy_uuid, realization_cfy, realization_cfy_controller) = umlclass_link(
        UmlClassLinkType::InterfaceRealization,
        None,
        (class_cfy.clone(), class_cfy_controller.clone()),
        (class_af.clone(), class_af_controller.clone()),
    );

    let (class_client_uuid, class_client, class_client_controller) = uml_class(
        UmlClassStereotype::Class, "Client",
        "", "",
        egui::Pos2::new(300.0, 50.0),
    );

    let (usage_client_af_uuid, usage_client_af, usage_client_af_controller) = umlclass_link(
        UmlClassLinkType::Usage,
        Some((uuid::Uuid::now_v7(), egui::Pos2::new(200.0, 50.0))),
        (class_client.clone(), class_client_controller.clone()),
        (class_af.clone(), class_af_controller.clone()),
    );
    
    let (class_producta_uuid, class_producta, class_producta_controller) = uml_class(
        UmlClassStereotype::Interface, "ProductA",
        "", "",
        egui::Pos2::new(450.0, 150.0),
    );

    let (usage_client_producta_uuid, usage_client_producta, usage_client_producta_controller) =
        umlclass_link(
            UmlClassLinkType::Usage,
            Some((uuid::Uuid::now_v7(), egui::Pos2::new(450.0, 52.0))),
            (class_client.clone(), class_client_controller.clone()),
            (class_producta.clone(), class_producta_controller.clone()),
        );
    
    let (class_productb_uuid, class_productb, class_productb_controller) = uml_class(
        UmlClassStereotype::Interface, "ProductB",
        "", "",
        egui::Pos2::new(650.0, 150.0),
    );

    let (usage_client_productb_uuid, usage_client_productb, usage_client_productb_controller) =
        umlclass_link(
            UmlClassLinkType::Usage,
            Some((uuid::Uuid::now_v7(), egui::Pos2::new(650.0, 48.0))),
            (class_client.clone(), class_client_controller.clone()),
            (class_productb.clone(), class_productb_controller.clone()),
        );

    let mut owned_controllers =
        HashMap::<_, Arc<RwLock<dyn ElementControllerGen2<_, _, _, _, _>>>>::new();
    owned_controllers.insert(class_af_uuid, class_af_controller);
    owned_controllers.insert(class_cfx_uuid, class_cfx_controller);
    owned_controllers.insert(class_cfy_uuid, class_cfy_controller);
    owned_controllers.insert(realization_cfx_uuid, realization_cfx_controller);
    owned_controllers.insert(realization_cfy_uuid, realization_cfy_controller);
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
            realization_cfy,
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
        DiagramControllerGen2::new(
            diagram2.clone(),
            owned_controllers,
            UmlClassQueryable {},
            UmlClassDiagramBuffer {
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
pub enum KindedUmlClassElement<'a> {
    Diagram {},
    Package {
        inner: &'a PackageView<
            UmlClassPackage,
            dyn UmlClassElement,
            UmlClassQueryable,
            UmlClassPackageBuffer,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    },
    Class {
        inner: &'a UmlClassController,
    },
    Link {
        inner: &'a MulticonnectionView<
            UmlClassLink,
            dyn UmlClassElement,
            UmlClassQueryable,
            UmlClassLinkBuffer,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    },
}

impl<'a>
    From<
        &'a DiagramControllerGen2<
            UmlClassDiagram,
            dyn UmlClassElement,
            UmlClassQueryable,
            UmlClassDiagramBuffer,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    > for KindedUmlClassElement<'a>
{
    fn from(
        from: &'a DiagramControllerGen2<
            UmlClassDiagram,
            dyn UmlClassElement,
            UmlClassQueryable,
            UmlClassDiagramBuffer,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    ) -> Self {
        Self::Diagram {}
    }
}

impl<'a>
    From<
        &'a PackageView<
            UmlClassPackage,
            dyn UmlClassElement,
            UmlClassQueryable,
            UmlClassPackageBuffer,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    > for KindedUmlClassElement<'a>
{
    fn from(
        from: &'a PackageView<
            UmlClassPackage,
            dyn UmlClassElement,
            UmlClassQueryable,
            UmlClassPackageBuffer,
            NaiveUmlClassTool,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    ) -> Self {
        Self::Package { inner: from }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Class,
    LinkStart { link_type: UmlClassLinkType },
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialUmlClassElement {
    None,
    Some((uuid::Uuid, ArcRwLockController)),
    Link {
        link_type: UmlClassLinkType,
        source: Arc<RwLock<dyn UmlClassElement>>,
        source_view: ArcRwLockController,
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
    result: PartialUmlClassElement,
    event_lock: bool,
}

impl NaiveUmlClassTool {
    pub fn new(initial_stage: UmlClassToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialUmlClassElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<dyn UmlClassElement, UmlClassQueryable, UmlClassElementOrVertex, UmlClassPropChange>
    for NaiveUmlClassTool
{
    type KindedElement<'a> = KindedUmlClassElement<'a>;
    type Stage = UmlClassToolStage;

    fn initial_stage(&self) -> Self::Stage {
        self.initial_stage
    }

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32 {
        match controller {
            KindedUmlClassElement::Diagram { .. } => match self.current_stage {
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            KindedUmlClassElement::Package { .. } => match self.current_stage {
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            KindedUmlClassElement::Class { .. } => match self.current_stage {
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            KindedUmlClassElement::Link { .. } => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialUmlClassElement::Link {
                source_view,
                link_type,
                ..
            } => {
                canvas.draw_line(
                    [source_view.read().unwrap().position(), pos],
                    match link_type.line_type() {
                        canvas::LineType::Solid => canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::LineType::Dashed => canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    },
                    canvas::Highlight::NONE,
                );
            }
            PartialUmlClassElement::Package { a_display, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a_display, pos),
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

        match (self.current_stage, &mut self.result) {
            (UmlClassToolStage::Class, _) => {
                let (class_uuid, _class, class_controller) = uml_class(
                    UmlClassStereotype::Class, "a class",
                    "", "", pos,
                );
                self.result = PartialUmlClassElement::Some((class_uuid, class_controller));
                self.event_lock = true;
            }
            (UmlClassToolStage::PackageStart, _) => {
                self.result = PartialUmlClassElement::Package {
                    a: pos,
                    a_display: pos,
                    b: None,
                };
                self.current_stage = UmlClassToolStage::PackageEnd;
                self.event_lock = true;
            }
            (UmlClassToolStage::PackageEnd, PartialUmlClassElement::Package { ref mut b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (UmlClassToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>) {
        if self.event_lock {
            return;
        }

        match controller {
            KindedUmlClassElement::Diagram { .. } => {}
            KindedUmlClassElement::Package { .. } => {}
            KindedUmlClassElement::Class { inner } => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::Link {
                            link_type,
                            source: inner.model.clone(),
                            source_view: inner.self_reference.upgrade().unwrap(),
                            dest: None,
                        };
                        self.current_stage = UmlClassToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        UmlClassToolStage::LinkEnd,
                        PartialUmlClassElement::Link { ref mut dest, .. },
                    ) => {
                        *dest = Some(inner.model.clone());
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            KindedUmlClassElement::Link { .. } => {}
        }
    }
    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<
            dyn UmlClassElement,
            UmlClassQueryable,
            Self,
            UmlClassElementOrVertex,
            UmlClassPropChange,
        >,
    ) -> Option<(
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    Self,
                    UmlClassElementOrVertex,
                    UmlClassPropChange,
                >,
            >,
        >,
    )> {
        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                self.result = PartialUmlClassElement::None;
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

                let association_controller: Option<(
                    uuid::Uuid,
                    Arc<
                        RwLock<
                            dyn ElementControllerGen2<
                                dyn UmlClassElement,
                                UmlClassQueryable,
                                Self,
                                UmlClassElementOrVertex,
                                UmlClassPropChange,
                            >,
                        >,
                    >,
                )> = if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source.read().unwrap().uuid()),
                    into.controller_for(&dest.read().unwrap().uuid()),
                ) {
                    let (uuid, _, controller) = umlclass_link(
                        *link_type,
                        None,
                        (source.clone(), source_controller),
                        (dest.clone(), dest_controller),
                    );

                    Some((uuid, controller))
                } else {
                    None
                };

                self.result = PartialUmlClassElement::None;
                association_controller
            }
            PartialUmlClassElement::Package { a, b: Some(b), .. } => {
                self.current_stage = UmlClassToolStage::PackageStart;

                let (uuid, _package, package_controller) =
                    umlclass_package("a package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialUmlClassElement::None;
                Some((uuid, package_controller))
            }
            _ => None,
        }
    }
    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

pub trait UmlClassElementController:
    ElementControllerGen2<
    dyn UmlClassElement,
    UmlClassQueryable,
    NaiveUmlClassTool,
    UmlClassElementOrVertex,
    UmlClassPropChange,
>
{
    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool {
        false
    }
    fn connection_target_name(&self) -> Option<Arc<String>> {
        None
    }
}

pub struct UmlClassPackageBuffer {
    name: String,
    comment: String,
}

fn umlclass_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (
    uuid::Uuid,
    Arc<RwLock<UmlClassPackage>>,
    Arc<
        RwLock<
            PackageView<
                UmlClassPackage,
                dyn UmlClassElement,
                UmlClassQueryable,
                UmlClassPackageBuffer,
                NaiveUmlClassTool,
                UmlClassElementOrVertex,
                UmlClassPropChange,
            >,
        >,
    >,
) {
    fn model_to_element_shim(a: Arc<RwLock<UmlClassPackage>>) -> Arc<RwLock<dyn UmlClassElement>> {
        a
    }

    fn show_properties_fun(
        buffer: &mut UmlClassPackageBuffer,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.name),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(buffer.name.clone())),
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
                UmlClassPropChange::CommentChange(Arc::new(buffer.comment.clone())),
            ]));
        }
    }
    fn apply_property_change_fun(
        buffer: &mut UmlClassPackageBuffer,
        model: &mut UmlClassPackage,
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            for property in properties {
                match property {
                    UmlClassPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![UmlClassPropChange::NameChange(model.name.clone())],
                        ));
                        buffer.name = (**name).clone();
                        model.name = name.clone();
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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
    let package = Arc::new(RwLock::new(UmlClassPackage::new(
        uuid.clone(),
        name.to_owned(),
        vec![],
    )));
    let package_controller = Arc::new(RwLock::new(PackageView::new(
        package.clone(),
        HashMap::new(),
        UmlClassPackageBuffer {
            name: name.to_owned(),
            comment: "".to_owned(),
        },
        bounds_rect,
        model_to_element_shim,
        show_properties_fun,
        apply_property_change_fun,
    )));

    (uuid, package, package_controller)
}

fn uml_class(
    stereotype: UmlClassStereotype,
    name: &str,
    properties: &str,
    functions: &str,
    position: egui::Pos2,
) -> (uuid::Uuid, Arc<RwLock<UmlClass>>, Arc<RwLock<UmlClassController>>) {
    let class_uuid = uuid::Uuid::now_v7();
    let class = Arc::new(RwLock::new(UmlClass::new(
        class_uuid.clone(),
        stereotype,
        name.to_owned(),
        properties.to_owned(),
        functions.to_owned(),
    )));
    let class_controller = Arc::new(RwLock::new(UmlClassController {
        model: class.clone(),
        self_reference: Weak::new(),
        stereotype_buffer: stereotype,
        name_buffer: name.to_owned(),
        properties_buffer: properties.to_owned(),
        functions_buffer: functions.to_owned(),
        comment_buffer: "".to_owned(),

        dragged: false,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::ZERO,
    }));
    class_controller.write().unwrap().self_reference = Arc::downgrade(&class_controller);
    (class_uuid, class, class_controller)
}

pub struct UmlClassController {
    pub model: Arc<RwLock<UmlClass>>,
    self_reference: Weak<RwLock<Self>>,

    stereotype_buffer: UmlClassStereotype,
    name_buffer: String,
    properties_buffer: String,
    functions_buffer: String,
    comment_buffer: String,

    dragged: bool,
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

impl ContainerGen2<dyn UmlClassElement, UmlClassQueryable, NaiveUmlClassTool, UmlClassElementOrVertex, UmlClassPropChange>
    for UmlClassController
{
    fn controller_for(
        &self,
        _uuid: &uuid::Uuid,
    ) -> Option<ArcRwLockController> {
        None
    }
}

impl
    ElementControllerGen2<
        dyn UmlClassElement,
        UmlClassQueryable,
        NaiveUmlClassTool,
        UmlClassElementOrVertex,
        UmlClassPropChange,
    > for UmlClassController
{
    fn show_properties(
        &mut self,
        _parent: &UmlClassQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Stereotype:");
        egui::ComboBox::from_id_salt("Stereotype:")
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
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::StereotypeChange(self.stereotype_buffer),
                        ]));
                    }
                }
            });

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ]));
        }

        ui.label("Properties:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.properties_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::PropertiesChange(Arc::new(self.properties_buffer.clone())),
            ]));
        }

        ui.label("Functions:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.functions_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::FunctionsChange(Arc::new(self.functions_buffer.clone())),
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
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
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

    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
        tool: &mut Option<NaiveUmlClassTool>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            e if !self.min_shape().contains(*e.mouse_position()) => return EventHandlingStatus::NotHandled,
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) => {
                self.dragged = matches!(event, InputEvent::MouseDown(_));
                EventHandlingStatus::HandledByElement
            },
            InputEvent::Click(_pos) => {
                if let Some(tool) = tool {
                    tool.add_element(KindedUmlClassElement::Class { inner: self });
                } else {
                    if !modifiers.command {
                        self.highlight.selected = true;
                    } else {
                        self.highlight.selected = !self.highlight.selected;
                    }
                }
                
                EventHandlingStatus::HandledByElement
            },
            InputEvent::Drag { delta, .. } if self.dragged => {
                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(delta));
                } else {
                    commands.push(SensitiveCommand::MoveSpecificElements(
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _) if !uuids.contains(&*self.uuid()) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::DeleteSpecificElements(..) | InsensitiveCommand::AddElement(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            UmlClassPropChange::StereotypeChange(stereotype) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![UmlClassPropChange::StereotypeChange(
                                        model.stereotype.clone(),
                                    )],
                                ));
                                self.stereotype_buffer = stereotype.clone();
                                model.stereotype = stereotype.clone();
                            }
                            UmlClassPropChange::NameChange(name) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![UmlClassPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            UmlClassPropChange::PropertiesChange(properties) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![UmlClassPropChange::PropertiesChange(
                                        model.properties.clone(),
                                    )],
                                ));
                                self.properties_buffer = (**properties).clone();
                                model.properties = properties.clone();
                            }
                            UmlClassPropChange::FunctionsChange(functions) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![UmlClassPropChange::FunctionsChange(
                                        model.functions.clone(),
                                    )],
                                ));
                                self.functions_buffer = (**functions).clone();
                                model.functions = functions.clone();
                            }
                            UmlClassPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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

pub struct UmlClassLinkBuffer {
    link_type: UmlClassLinkType,
    source_arrowhead_label: String,
    destination_arrowhead_label: String,
    comment: String,
}

fn umlclass_link(
    link_type: UmlClassLinkType,
    center_point: Option<(uuid::Uuid, egui::Pos2)>,
    source: (
        Arc<RwLock<dyn UmlClassElement>>,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                    UmlClassElementOrVertex,
                    UmlClassPropChange,
                >,
            >,
        >,
    ),
    destination: (
        Arc<RwLock<dyn UmlClassElement>>,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    dyn UmlClassElement,
                    UmlClassQueryable,
                    NaiveUmlClassTool,
                    UmlClassElementOrVertex,
                    UmlClassPropChange,
                >,
            >,
        >,
    ),
) -> (
    uuid::Uuid,
    Arc<RwLock<UmlClassLink>>,
    Arc<
        RwLock<
            MulticonnectionView<
                UmlClassLink,
                dyn UmlClassElement,
                UmlClassQueryable,
                UmlClassLinkBuffer,
                NaiveUmlClassTool,
                UmlClassElementOrVertex,
                UmlClassPropChange,
            >,
        >,
    >,
) {
    fn model_to_element_shim(a: Arc<RwLock<UmlClassLink>>) -> Arc<RwLock<dyn UmlClassElement>> {
        a
    }

    fn show_properties_fun(
        buffer: &mut UmlClassLinkBuffer,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        ui.label("Link type:");
        egui::ComboBox::from_id_salt("link type")
            .selected_text(&*buffer.link_type.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassLinkType::Association,
                    UmlClassLinkType::Aggregation,
                    UmlClassLinkType::Composition,
                    UmlClassLinkType::Generalization,
                    UmlClassLinkType::InterfaceRealization,
                    UmlClassLinkType::Usage,
                ] {
                    if ui
                        .selectable_value(&mut buffer.link_type, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkTypeChange(buffer.link_type),
                        ]));
                    }
                }
            });

        ui.label("Source:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut buffer.source_arrowhead_label),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::SourceArrowheadLabelChange(Arc::new(
                    buffer.source_arrowhead_label.clone(),
                )),
            ]));
        }
        ui.separator();

        ui.label("Destination:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut buffer.destination_arrowhead_label),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::DestinationArrowheadLabelChange(Arc::new(
                    buffer.destination_arrowhead_label.clone(),
                )),
            ]));
        }
        ui.separator();

        ui.label("Swap source and destination:");
        if ui.button("Swap").clicked() {
            // (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            /* TODO:
            (self.source, self.destination) = (self.destination.clone(), self.source.clone());
            (self.source_points, self.dest_points) =
                (self.dest_points.clone(), self.source_points.clone());
                */
        }
        ui.separator();

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.comment),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::CommentChange(Arc::new(buffer.comment.clone())),
            ]));
        }
    }
    fn apply_property_change_fun(
        buffer: &mut UmlClassLinkBuffer,
        model: &mut UmlClassLink,
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            for property in properties {
                match property {
                    UmlClassPropChange::LinkTypeChange(link_type) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![UmlClassPropChange::LinkTypeChange(model.link_type.clone())],
                        ));
                        buffer.link_type = link_type.clone();
                        model.link_type = link_type.clone();
                    }
                    UmlClassPropChange::SourceArrowheadLabelChange(source_arrowhead_label) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![UmlClassPropChange::CommentChange(
                                model.source_arrowhead_label.clone(),
                            )],
                        ));
                        buffer.source_arrowhead_label = (**source_arrowhead_label).clone();
                        model.source_arrowhead_label = source_arrowhead_label.clone();
                    }
                    UmlClassPropChange::DestinationArrowheadLabelChange(
                        destination_arrowhead_label,
                    ) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![UmlClassPropChange::CommentChange(
                                model.destination_arrowhead_label.clone(),
                            )],
                        ));
                        buffer.destination_arrowhead_label =
                            (**destination_arrowhead_label).clone();
                        model.destination_arrowhead_label = destination_arrowhead_label.clone();
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
                        ));
                        buffer.comment = (**comment).clone();
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn model_to_uuid(a: &UmlClassLink) -> Arc<uuid::Uuid> {
        a.uuid()
    }
    fn model_to_name(a: &UmlClassLink) -> Arc<String> {
        a.link_type.name()
    }
    fn model_to_line_type(a: &UmlClassLink) -> canvas::LineType {
        a.link_type.line_type()
    }
    fn model_to_source_arrowhead_type(a: &UmlClassLink) -> canvas::ArrowheadType {
        a.link_type.source_arrowhead_type()
    }
    fn model_to_destination_arrowhead_type(a: &UmlClassLink) -> canvas::ArrowheadType {
        a.link_type.destination_arrowhead_type()
    }
    fn model_to_source_arrowhead_label(a: &UmlClassLink) -> Option<&str> {
        if !a.source_arrowhead_label.is_empty() {
            Some(&a.source_arrowhead_label)
        } else {
            None
        }
    }
    fn model_to_destination_arrowhead_label(a: &UmlClassLink) -> Option<&str> {
        if !a.destination_arrowhead_label.is_empty() {
            Some(&a.destination_arrowhead_label)
        } else {
            None
        }
    }

    let link_uuid = uuid::Uuid::now_v7();
    let link = Arc::new(RwLock::new(UmlClassLink::new(
        link_uuid.clone(),
        link_type,
        source.0,
        destination.0,
    )));
    let link_controller = Arc::new(RwLock::new(MulticonnectionView::new(
        link.clone(),
        UmlClassLinkBuffer {
            link_type,
            source_arrowhead_label: "".to_owned(),
            destination_arrowhead_label: "".to_owned(),
            comment: "".to_owned(),
        },
        source.1,
        destination.1,
        center_point,
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
    (link_uuid, link, link_controller)
}

impl UmlClassElementController
    for MulticonnectionView<
        UmlClassLink,
        dyn UmlClassElement,
        UmlClassQueryable,
        UmlClassLinkBuffer,
        NaiveUmlClassTool,
        UmlClassElementOrVertex,
        UmlClassPropChange,
    >
{
    fn is_connection_from(&self, uuid: &uuid::Uuid) -> bool {
        *self.source.read().unwrap().uuid() == *uuid
    }

    fn connection_target_name(&self) -> Option<Arc<String>> {
        Some(self.destination.read().unwrap().model_name())
    }
}
