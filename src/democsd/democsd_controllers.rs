use crate::common::controller::{
    ClickHandlingStatus, ContainerGen2, ContainerModel, DiagramController, DiagramControllerGen2,
    DragHandlingStatus, ElementController, ElementControllerGen2, FlipMulticonnection,
    InsensitiveCommand, ModifierKeys, MulticonnectionView, SensitiveCommand, TargettingStatus,
    Tool, VertexInformation,
};
use crate::common::canvas;
use crate::democsd::democsd_models::{
    DemoCsdDiagram, DemoCsdElement, DemoCsdPackage, DemoCsdClient, DemoCsdTransactor, DemoCsdBank, DemoCsdLinkType, DemoCsdLink,
};
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};
use crate::NHApp;

type ArcRwLockController = Arc<
    RwLock<
        dyn ElementControllerGen2<
            dyn DemoCsdElement,
            DemoCsdQueryable,
            NaiveDemoCsdTool,
            DemoCsdElementOrVertex,
            DemoCsdPropChange,
        >,
    >,
>;
type DiagramView = DiagramControllerGen2<
    DemoCsdDiagram,
    dyn DemoCsdElement,
    DemoCsdQueryable,
    DemoCsdDiagramBuffer,
    NaiveDemoCsdTool,
    DemoCsdElementOrVertex,
    DemoCsdPropChange,
>;
type PackageView = crate::common::controller::PackageView<
    DemoCsdPackage,
    dyn DemoCsdElement,
    DemoCsdQueryable,
    DemoCsdPackageBuffer,
    NaiveDemoCsdTool,
    DemoCsdElementOrVertex,
    DemoCsdPropChange,
>;

pub struct DemoCsdQueryable {}

#[derive(Clone)]
pub enum DemoCsdPropChange {
    NameChange(Arc<String>),
    IdentifierChange(Arc<String>),
    
    TransactionNameChange(Arc<String>),
    TransactionIdentifierChange(Arc<String>),
    
    LinkTypeChange(DemoCsdLinkType),

    CommentChange(Arc<String>),
    PackageResize(egui::Vec2),
}

impl Debug for DemoCsdPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoCsdPropChange::???")
    }
}

impl TryInto<FlipMulticonnection> for &DemoCsdPropChange {
    type Error = ();

    fn try_into(self) -> Result<FlipMulticonnection, ()> {
        Err(())
    }
}

#[derive(Clone)]
pub enum DemoCsdElementOrVertex {
    Element((uuid::Uuid, ArcRwLockController)),
    Vertex(VertexInformation),
}

impl Debug for DemoCsdElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoCsdElementOrVertex::???")
    }
}

impl From<VertexInformation> for DemoCsdElementOrVertex {
    fn from(v: VertexInformation) -> Self {
        DemoCsdElementOrVertex::Vertex(v)
    }
}

impl TryInto<VertexInformation> for DemoCsdElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<VertexInformation, ()> {
        match self {
            DemoCsdElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl From<(uuid::Uuid, ArcRwLockController)> for DemoCsdElementOrVertex {
    fn from(v: (uuid::Uuid, ArcRwLockController)) -> Self {
        DemoCsdElementOrVertex::Element(v)
    }
}

impl TryInto<(uuid::Uuid, ArcRwLockController)> for DemoCsdElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<(uuid::Uuid, ArcRwLockController), ()> {
        match self {
            DemoCsdElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

pub struct DemoCsdDiagramBuffer {
    uuid: uuid::Uuid,
    name: String,
    comment: String,
}

fn show_props_fun(
    buffer: &mut DemoCsdDiagramBuffer,
    ui: &mut egui::Ui,
    commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
            vec![DemoCsdPropChange::NameChange(Arc::new(buffer.name.clone()))],
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
            vec![DemoCsdPropChange::CommentChange(Arc::new(
                buffer.comment.clone(),
            ))],
        ));
    }
}
fn apply_property_change_fun(
    buffer: &mut DemoCsdDiagramBuffer,
    model: &mut DemoCsdDiagram,
    command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
    undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
) {
    if let InsensitiveCommand::PropertyChange(_, properties) = command {
        for property in properties {
            match property {
                DemoCsdPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(buffer.uuid).collect(),
                        vec![DemoCsdPropChange::NameChange(model.name.clone())],
                    ));
                    buffer.name = (**name).clone();
                    model.name = name.clone();
                }
                DemoCsdPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(buffer.uuid).collect(),
                        vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
                    ));
                    buffer.comment = (**comment).clone();
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
}
fn tool_change_fun(tool: &mut Option<NaiveDemoCsdTool>, ui: &mut egui::Ui) {
    let width = ui.available_width();

    let stage = tool.as_ref().map(|e| e.initial_stage());
    let c = |s: DemoCsdToolStage| -> egui::Color32 {
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
            (DemoCsdToolStage::Client, "Client Role"),
            (DemoCsdToolStage::Transactor, "Actor Role"),
            (DemoCsdToolStage::Bank, "Transaction Bank"),
        ][..],
        &[
            (DemoCsdToolStage::LinkStart, "Link"),
        ][..],
        &[
            (DemoCsdToolStage::PackageStart, "Package"),
        ][..],
        &[(DemoCsdToolStage::Note, "Note")][..],
    ] {
        for (stage, name) in cat {
            if ui
                .add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage)))
                .clicked()
            {
                *tool = Some(NaiveDemoCsdTool::new(*stage));
            }
        }
        ui.separator();
    }
}

fn menubar_options_fun(controller: &mut DiagramView, context: &mut NHApp, ui: &mut egui::Ui) {}

pub fn new(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let uuid = uuid::Uuid::now_v7();
    let name = format!("New RDF diagram {}", no);

    let diagram = Arc::new(RwLock::new(DemoCsdDiagram::new(
        uuid.clone(),
        name.clone(),
        vec![],
    )));
    (
        uuid,
        Arc::new(RwLock::new(DiagramControllerGen2::new(
            diagram.clone(),
            HashMap::new(),
            DemoCsdQueryable {},
            DemoCsdDiagramBuffer {
                uuid,
                name,
                comment: "".to_owned(),
            },
            show_props_fun,
            apply_property_change_fun,
            tool_change_fun,
            menubar_options_fun,
        ))),
    )
}

#[derive(Clone, Copy)]
pub enum KindedDemoCsdElement<'a> {
    Diagram {},
    Package {
        inner: &'a PackageView,
    },
    Client {
        inner: &'a DemoCsdClientView,
    },
    Transactor {
        inner: &'a DemoCsdTransactorView,
    },
    Bank {
        inner: &'a DemoCsdBankView,
    },
    Link {
        inner: &'a MulticonnectionView<
            DemoCsdLink,
            dyn DemoCsdElement,
            DemoCsdQueryable,
            DemoCsdLinkBuffer,
            NaiveDemoCsdTool,
            DemoCsdElementOrVertex,
            DemoCsdPropChange,
        >,
    },
}

impl<'a> From<&'a DiagramView> for KindedDemoCsdElement<'a> {
    fn from(from: &'a DiagramView) -> Self {
        Self::Diagram {}
    }
}

impl<'a> From<&'a PackageView> for KindedDemoCsdElement<'a> {
    fn from(from: &'a PackageView) -> Self {
        Self::Package { inner: from }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DemoCsdToolStage {
    Client,
    Transactor,
    Bank,
    LinkStart,
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialDemoCsdElement {
    None,
    Some((uuid::Uuid, ArcRwLockController)),
    Link {
        source: Arc<RwLock<dyn DemoCsdElement>>,
        source_pos: egui::Pos2,
        dest: Option<Arc<RwLock<dyn DemoCsdElement>>>,
    },
    Package {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveDemoCsdTool {
    initial_stage: DemoCsdToolStage,
    current_stage: DemoCsdToolStage,
    offset: egui::Pos2,
    result: PartialDemoCsdElement,
    event_lock: bool,
}

impl NaiveDemoCsdTool {
    pub fn new(initial_stage: DemoCsdToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            offset: egui::Pos2::ZERO,
            result: PartialDemoCsdElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<dyn DemoCsdElement, DemoCsdQueryable, DemoCsdElementOrVertex, DemoCsdPropChange> for NaiveDemoCsdTool {
    type KindedElement<'a> = KindedDemoCsdElement<'a>;
    type Stage = DemoCsdToolStage;

    fn initial_stage(&self) -> DemoCsdToolStage {
        self.initial_stage
    }

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32 {
        match controller {
            KindedDemoCsdElement::Diagram { .. } => match self.current_stage {
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => TARGETTABLE_COLOR,
                DemoCsdToolStage::LinkStart | DemoCsdToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Package { .. } => match self.current_stage {
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => TARGETTABLE_COLOR,
                DemoCsdToolStage::LinkStart
                | DemoCsdToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Client { .. } => match self.current_stage {
                DemoCsdToolStage::LinkStart => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkEnd
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Transactor { .. } => match self.current_stage {
                DemoCsdToolStage::LinkStart | DemoCsdToolStage::LinkEnd => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Bank { .. } => match self.current_stage {
                DemoCsdToolStage::LinkEnd => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkStart
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Link { .. } => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn canvas::NHCanvas, pos: egui::Pos2) {
        match self.result {
            PartialDemoCsdElement::Link { source_pos, .. } => {
                canvas.draw_line(
                    [source_pos, pos],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            PartialDemoCsdElement::Package { a, .. } => {
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
            (DemoCsdToolStage::Client, _) => {
                let client = Arc::new(RwLock::new(DemoCsdClient::new(
                    uuid,
                    "AR01".to_owned(),
                    "Client".to_owned(),
                )));
                self.result = PartialDemoCsdElement::Some((
                    uuid,
                    Arc::new(RwLock::new(DemoCsdClientView {
                        model: client.clone(),
                        
                        identifier_buffer: "AR01".to_owned(),
                        name_buffer: "Client".to_owned(),
                        internal_buffer: true,
                        comment_buffer: "".to_owned(),

                        highlight: canvas::Highlight::NONE,
                        position: pos,
                        bounds_rect: egui::Rect::ZERO,
                    })),
                ));
                self.event_lock = true;
            }
            (DemoCsdToolStage::Transactor, _) => {
                let transactor = Arc::new(RwLock::new(DemoCsdTransactor::new(
                    uuid,
                    "AR01".to_owned(),
                    "Transactor".to_owned(),
                    "TK01".to_owned(),
                    "Transaction".to_owned(),
                )));
                self.result = PartialDemoCsdElement::Some((
                    uuid,
                    Arc::new(RwLock::new(DemoCsdTransactorView {
                        model: transactor.clone(),
                        
                        identifier_buffer: "AR01".to_owned(),
                        name_buffer: "Transactor".to_owned(),
                        internal_buffer: true,
                        transaction_identifier_buffer: "TK01".to_owned(),
                        transaction_name_buffer: "Transaction".to_owned(),
                        transaction_selfactivating_buffer: false,
                        comment_buffer: "".to_owned(),

                        highlight: canvas::Highlight::NONE,
                        position: pos,
                        bounds_rect: egui::Rect::ZERO,
                    })),
                ));
                self.event_lock = true;
            }
            (DemoCsdToolStage::Bank, _) => {
                let bank = Arc::new(RwLock::new(DemoCsdBank::new(
                    uuid,
                    "TK01".to_owned(),
                    "Bank".to_owned(),
                )));
                self.result = PartialDemoCsdElement::Some((
                    uuid,
                    Arc::new(RwLock::new(DemoCsdBankView {
                        model: bank.clone(),
                        
                        identifier_buffer: "TK01".to_owned(),
                        name_buffer: "Bank".to_owned(),
                        comment_buffer: "".to_owned(),

                        highlight: canvas::Highlight::NONE,
                        position: pos,
                        min_shape: canvas::NHShape::Ellipse {
                            position: egui::Pos2::ZERO,
                            bounds_radius: egui::Vec2::ZERO,
                        },
                    })),
                ));
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageStart, _) => {
                self.result = PartialDemoCsdElement::Package {
                    a: self.offset + pos.to_vec2(),
                    b: None,
                };
                self.current_stage = DemoCsdToolStage::PackageEnd;
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageEnd, PartialDemoCsdElement::Package { ref mut b, .. }) => *b = Some(pos),
            (DemoCsdToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>, pos: egui::Pos2) {
        if self.event_lock {
            return;
        }

        match controller {
            KindedDemoCsdElement::Diagram { .. } => {}
            KindedDemoCsdElement::Package { .. } => {}
            KindedDemoCsdElement::Client { inner } => match (self.current_stage, &mut self.result) {
                (DemoCsdToolStage::LinkStart, PartialDemoCsdElement::None) => {
                    self.result = PartialDemoCsdElement::Link {
                        source: inner.model.clone(),
                        source_pos: self.offset + pos.to_vec2(),
                        dest: None,
                    };
                    self.current_stage = DemoCsdToolStage::LinkEnd;
                    self.event_lock = true;
                }
                _ => {}
            },
            KindedDemoCsdElement::Transactor { inner } => match (self.current_stage, &mut self.result) {
                (DemoCsdToolStage::LinkStart, PartialDemoCsdElement::None) => {
                    self.result = PartialDemoCsdElement::Link {
                        source: inner.model.clone(),
                        source_pos: self.offset + pos.to_vec2(),
                        dest: None,
                    };
                    self.current_stage = DemoCsdToolStage::LinkEnd;
                    self.event_lock = true;
                }
                (DemoCsdToolStage::LinkEnd, PartialDemoCsdElement::Link { ref mut dest, .. }) => {
                    *dest = Some(inner.model.clone());
                    self.event_lock = true;
                }
                _ => {}
            },
            KindedDemoCsdElement::Bank { inner } => match (self.current_stage, &mut self.result) {
                (DemoCsdToolStage::LinkEnd, PartialDemoCsdElement::Link { ref mut dest, .. }) => {
                    *dest = Some(inner.model.clone());
                    self.event_lock = true;
                }
                _ => {}
            },
            KindedDemoCsdElement::Link { .. } => {}
        }
    }

    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<
            dyn DemoCsdElement,
            DemoCsdQueryable,
            Self,
            DemoCsdElementOrVertex,
            DemoCsdPropChange,
        >,
    ) -> Option<(uuid::Uuid, ArcRwLockController)> {
        match &self.result {
            PartialDemoCsdElement::Some(x) => {
                let x = x.clone();
                self.result = PartialDemoCsdElement::None;
                Some(x)
            }
            // TODO: check for source == dest case, set points?
            PartialDemoCsdElement::Link {
                source,
                dest: Some(dest),
                ..
            } => {
                self.current_stage = DemoCsdToolStage::LinkStart;

                let predicate_controller: Option<(uuid::Uuid, ArcRwLockController)> =
                    if let (Some(source_controller), Some(dest_controller)) = (
                        into.controller_for(&source.read().unwrap().uuid()),
                        into.controller_for(&dest.read().unwrap().uuid()),
                    ) {
                        let (uuid, _, predicate_controller) = democsd_link(
                            DemoCsdLinkType::Initiation,
                            (source.clone(), source_controller),
                            (dest.clone(), dest_controller),
                        );

                        Some((uuid, predicate_controller))
                    } else {
                        None
                    };

                self.result = PartialDemoCsdElement::None;
                predicate_controller
            }
            PartialDemoCsdElement::Package { a, b: Some(b) } => {
                self.current_stage = DemoCsdToolStage::PackageStart;

                let (uuid, _, package_controller) =
                    democsd_package("A package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialDemoCsdElement::None;
                Some((uuid, package_controller))
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

pub trait DemoCsdElementController:
    ElementControllerGen2<dyn DemoCsdElement, DemoCsdQueryable, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange>
{
    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool {
        false
    }
    fn connection_target_name(&self) -> Option<Arc<String>> {
        None
    }
}

pub trait RdfContainerController {
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<Arc<RwLock<dyn DemoCsdElementController>>>;
}



pub struct DemoCsdPackageBuffer {
    name: String,
    comment: String,
}

fn democsd_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (
    uuid::Uuid,
    Arc<RwLock<DemoCsdPackage>>,
    Arc<RwLock<PackageView>>,
) {
    fn model_to_element_shim(a: Arc<RwLock<DemoCsdPackage>>) -> Arc<RwLock<dyn DemoCsdElement>> {
        a
    }

    fn show_properties_fun(
        buffer: &mut DemoCsdPackageBuffer,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
                DemoCsdPropChange::NameChange(Arc::new(buffer.name.clone())),
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
                DemoCsdPropChange::CommentChange(Arc::new(buffer.comment.clone())),
            ]));
        }
    }
    fn apply_property_change_fun(
        buffer: &mut DemoCsdPackageBuffer,
        model: &mut DemoCsdPackage,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            for property in properties {
                match property {
                    DemoCsdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![DemoCsdPropChange::NameChange(model.name.clone())],
                        ));
                        buffer.name = (**name).clone();
                        model.name = name.clone();
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
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
    let graph = Arc::new(RwLock::new(DemoCsdPackage::new(
        uuid.clone(),
        name.to_owned(),
        vec![],
    )));
    let graph_controller = Arc::new(RwLock::new(PackageView::new(
        graph.clone(),
        HashMap::new(),
        DemoCsdPackageBuffer {
            name: name.to_owned(),
            comment: "".to_owned(),
        },
        bounds_rect,
        model_to_element_shim,
        show_properties_fun,
        apply_property_change_fun,
    )));

    (uuid, graph, graph_controller)
}

pub struct DemoCsdClientView {
    model: Arc<RwLock<DemoCsdClient>>,
    
    identifier_buffer: String,
    name_buffer: String,
    internal_buffer: bool,
    comment_buffer: String,
    
    highlight: canvas::Highlight,
    position: egui::Pos2,
    bounds_rect: egui::Rect,
}

impl ElementController<dyn DemoCsdElement> for DemoCsdClientView {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn DemoCsdElement>> {
        self.model.clone()
    }
    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Rect { inner: self.bounds_rect }
    }
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<dyn DemoCsdElement, DemoCsdQueryable, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange> for DemoCsdClientView {
    fn show_properties(
        &mut self,
        _parent: &DemoCsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.identifier_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ]));
        }
        
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ]));
        }
        
        ui.label("Coment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }
        
        true
    }

    fn list_in_project_hierarchy(&self, _parent: &DemoCsdQueryable, ui: &mut egui::Ui) {
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
        _: &DemoCsdQueryable,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        let read = self.model.read().unwrap();

        let [identifier_bounds, name_bounds] = [&read.identifier, &read.name].map(|e| canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            e,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        ));
        
        self.bounds_rect = egui::Rect::from_center_size(
            self.position,
            egui::Vec2::new(
                identifier_bounds.width().max(name_bounds.width()),
                identifier_bounds.height() + name_bounds.height(),
            ),
        ).expand(5.0);
        
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::Rounding::ZERO,
            if read.internal { egui::Color32::WHITE } else { egui::Color32::LIGHT_GRAY },
            canvas::Stroke::new_solid(1.0, if read.internal { egui::Color32::BLACK } else { egui::Color32::DARK_GRAY }),
            self.highlight,
        );
        
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_BOTTOM,
            &read.identifier,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
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
                t.targetting_for_element(KindedDemoCsdElement::Client { inner: self }),
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
        tool: &mut Option<NaiveDemoCsdTool>,
        _commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) {
            return ClickHandlingStatus::NotHandled;
        }

        if let Some(tool) = tool {
            tool.add_element(KindedDemoCsdElement::Client { inner: self }, pos);
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
        _tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            DemoCsdPropChange::IdentifierChange(identifier) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                self.identifier_buffer = (**identifier).clone();
                                model.identifier = identifier.clone();
                            }
                            DemoCsdPropChange::NameChange(name) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
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

pub struct DemoCsdTransactorView {
    model: Arc<RwLock<DemoCsdTransactor>>,
    
    identifier_buffer: String,
    name_buffer: String,
    internal_buffer: bool,
    transaction_identifier_buffer: String,
    transaction_name_buffer: String,
    transaction_selfactivating_buffer: bool,
    comment_buffer: String,
    
    highlight: canvas::Highlight,
    position: egui::Pos2,
    bounds_rect: egui::Rect,
}

impl ElementController<dyn DemoCsdElement> for DemoCsdTransactorView {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn DemoCsdElement>> {
        self.model.clone()
    }
    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Rect { inner: self.bounds_rect }
    }
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<dyn DemoCsdElement, DemoCsdQueryable, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange> for DemoCsdTransactorView {
    fn show_properties(
        &mut self,
        _parent: &DemoCsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }
        
        ui.label("Identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.identifier_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ]));
        }
        
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ]));
        }
        
        ui.label("Transaction identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.transaction_identifier_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::TransactionIdentifierChange(Arc::new(self.transaction_identifier_buffer.clone())),
            ]));
        }
        
        ui.label("Transaction name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.transaction_name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::TransactionNameChange(Arc::new(self.transaction_name_buffer.clone())),
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
                DemoCsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }

        true
    }

    fn list_in_project_hierarchy(&self, _parent: &DemoCsdQueryable, ui: &mut egui::Ui) {
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
        _: &DemoCsdQueryable,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        let read = self.model.read().unwrap();

        let radius = 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE;
        
        let [tx_name_bounds, identifier_bounds, name_bounds] = [&read.transaction_name, &read.identifier, &read.name].map(|e| canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            e,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        ));
        
        self.bounds_rect = egui::Rect::from_center_size(
            self.position + egui::Vec2::new(0.0, 1.75 * canvas::CLASS_MIDDLE_FONT_SIZE),
            egui::Vec2::new(
                tx_name_bounds.width().max(identifier_bounds.width()).max(name_bounds.width()),
                2.5 * canvas::CLASS_MIDDLE_FONT_SIZE + tx_name_bounds.height() + identifier_bounds.height() + name_bounds.height(),
            ),
        ).expand(5.0);
        
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::Rounding::ZERO,
            if read.internal { egui::Color32::WHITE } else { egui::Color32::LIGHT_GRAY },
            canvas::Stroke::new_solid(1.0, if read.internal { egui::Color32::BLACK } else { egui::Color32::DARK_GRAY }),
            self.highlight,
        );
        
        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, 0.5 * canvas::CLASS_MIDDLE_FONT_SIZE),
            egui::Align2::CENTER_TOP,
            &read.transaction_name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        
        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, 2.5 * canvas::CLASS_MIDDLE_FONT_SIZE),
            egui::Align2::CENTER_TOP,
            &read.identifier,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        
        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, 3.5 * canvas::CLASS_MIDDLE_FONT_SIZE),
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        
        draw_tx_mark(
            canvas,
            &read.transaction_identifier,
            self.position - egui::Vec2::new(0.0, radius - 6.0),
            radius,
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
                t.targetting_for_element(KindedDemoCsdElement::Transactor { inner: self }),
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
        tool: &mut Option<NaiveDemoCsdTool>,
        _commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) {
            return ClickHandlingStatus::NotHandled;
        }

        if let Some(tool) = tool {
            tool.add_element(KindedDemoCsdElement::Transactor { inner: self }, pos);
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
        _tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            DemoCsdPropChange::IdentifierChange(identifier) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                self.identifier_buffer = (**identifier).clone();
                                model.identifier = identifier.clone();
                            }
                            DemoCsdPropChange::NameChange(name) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::TransactionIdentifierChange(transaction_identifier) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::TransactionIdentifierChange(
                                        model.transaction_identifier.clone(),
                                    )],
                                ));
                                self.transaction_identifier_buffer = (**transaction_identifier).clone();
                                model.transaction_identifier = transaction_identifier.clone();
                            }
                            DemoCsdPropChange::TransactionNameChange(transaction_name) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::TransactionNameChange(model.transaction_name.clone())],
                                ));
                                self.transaction_name_buffer = (**transaction_name).clone();
                                model.transaction_name = transaction_name.clone();
                            }
                            DemoCsdPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
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

pub struct DemoCsdBankView {
    model: Arc<RwLock<DemoCsdBank>>,
    
    identifier_buffer: String,
    name_buffer: String,
    comment_buffer: String,
    
    highlight: canvas::Highlight,
    position: egui::Pos2,
    min_shape: canvas::NHShape,
}

impl ElementController<dyn DemoCsdElement> for DemoCsdBankView {
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
    fn model(&self) -> Arc<RwLock<dyn DemoCsdElement>> {
        self.model.clone()
    }
    fn min_shape(&self) -> canvas::NHShape {
        self.min_shape
    }
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

fn draw_tx_mark(
    canvas: &mut dyn canvas::NHCanvas,
    identifier: &str,
    position: egui::Pos2,
    radius: f32,
    highlight: canvas::Highlight,
) -> canvas::NHShape {
    canvas.draw_ellipse(
        position,
        egui::Vec2::splat(radius),
        egui::Color32::WHITE,
        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        highlight,
    );
    
    let pts = [ position - egui::Vec2::new(0.0, radius),
                position + egui::Vec2::new(radius, 0.0),
                position + egui::Vec2::new(0.0, radius),
                position - egui::Vec2::new(radius, 0.0),
                position - egui::Vec2::new(0.0, radius),];
    let mut iter = pts.iter().peekable();
    while let Some(u) = iter.next() {
        let Some(v) = iter.peek() else { break; };
        canvas.draw_line(
            [*u, **v],
            canvas::Stroke::new_solid(1.0, egui::Color32::RED),
            canvas::Highlight::NONE,
        );
    }
    
    canvas.draw_text(
        position,
        egui::Align2::CENTER_CENTER,
        identifier,
        canvas::CLASS_MIDDLE_FONT_SIZE,
        egui::Color32::BLACK,
    );
    
    canvas::NHShape::Ellipse { position: position, bounds_radius: egui::Vec2::splat(radius) }
}

impl ElementControllerGen2<dyn DemoCsdElement, DemoCsdQueryable, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange> for DemoCsdBankView {
    fn show_properties(
        &mut self,
        _parent: &DemoCsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.identifier_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ]));
        }
        
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                DemoCsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }
        
        true
    }

    fn list_in_project_hierarchy(&self, _parent: &DemoCsdQueryable, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        ui.label(format!("{} ({})", model.name, model.uuid));
    }

    fn draw_in(
        &mut self,
        _: &DemoCsdQueryable,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        let radius = 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE;
        let read = self.model.read().unwrap();

        self.min_shape = draw_tx_mark(canvas, &read.identifier, self.position, radius, self.highlight);
        
        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, radius),
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw targetting rectangle
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_ellipse(
                self.position,
                egui::Vec2::splat(radius),
                t.targetting_for_element(KindedDemoCsdElement::Bank { inner: self }),
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
        tool: &mut Option<NaiveDemoCsdTool>,
        _commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        if !self.min_shape().contains(pos) {
            return ClickHandlingStatus::NotHandled;
        }

        if let Some(tool) = tool {
            tool.add_element(KindedDemoCsdElement::Bank { inner: self }, pos);
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
        _tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
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
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            DemoCsdPropChange::IdentifierChange(identifier) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                self.identifier_buffer = (**identifier).clone();
                                model.identifier = identifier.clone();
                            }
                            DemoCsdPropChange::NameChange(name) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid()).collect(),
                                    vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
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

pub struct DemoCsdLinkBuffer {
    link_type: DemoCsdLinkType,
    comment: String,
}

fn democsd_link(
    link_type: DemoCsdLinkType,
    source: (Arc<std::sync::RwLock<dyn DemoCsdElement>>, ArcRwLockController),
    destination: (Arc<std::sync::RwLock<dyn DemoCsdElement>>, ArcRwLockController),
) -> (
    uuid::Uuid,
    Arc<RwLock<DemoCsdLink>>,
    Arc<RwLock<MulticonnectionView<DemoCsdLink, dyn DemoCsdElement, DemoCsdQueryable, DemoCsdLinkBuffer, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange>>>,
) {
    fn model_to_element_shim(a: Arc<RwLock<DemoCsdLink>>) -> Arc<RwLock<dyn DemoCsdElement>> {
        a
    }

    fn show_properties_fun(
        buffer: &mut DemoCsdLinkBuffer,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {

        ui.label("Type:");
        egui::ComboBox::from_id_source("Type:")
            .selected_text(buffer.link_type.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoCsdLinkType::Initiation,
                    DemoCsdLinkType::Interstriction,
                    DemoCsdLinkType::Interimpediment,
                ] {
                    if ui
                        .selectable_value(&mut buffer.link_type, value, value.char())
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoCsdPropChange::LinkTypeChange(buffer.link_type),
                        ]));
                    }
                }
            });
    
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut buffer.comment),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::CommentChange(Arc::new(buffer.comment.clone())),
            ]));
        }
    }
    fn apply_property_change_fun(
        buffer: &mut DemoCsdLinkBuffer,
        model: &mut DemoCsdLink,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            for property in properties {
                match property {
                    DemoCsdPropChange::LinkTypeChange(link_type) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![DemoCsdPropChange::LinkTypeChange(model.link_type)],
                        ));
                        buffer.link_type = *link_type;
                        model.link_type = *link_type;
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid()).collect(),
                            vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
                        ));
                        buffer.comment = (**comment).clone();
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn model_to_uuid(a: &DemoCsdLink) -> Arc<uuid::Uuid> {
        a.uuid()
    }
    fn model_to_name(a: &DemoCsdLink) -> Arc<String> {
        Arc::new("TODO".to_owned())
    }
    fn model_to_line_type(a: &DemoCsdLink) -> canvas::LineType {
        match a.link_type {
            DemoCsdLinkType::Initiation => canvas::LineType::Solid,
            DemoCsdLinkType::Interstriction
            | DemoCsdLinkType::Interimpediment => canvas::LineType::Dashed,
        }
    }
    fn model_to_source_arrowhead_type(a: &DemoCsdLink) -> canvas::ArrowheadType {
        match a.link_type {
            DemoCsdLinkType::Initiation
            | DemoCsdLinkType::Interstriction => canvas::ArrowheadType::None,
            DemoCsdLinkType::Interimpediment => canvas::ArrowheadType::FullTriangle,
        }
    }
    fn model_to_destination_arrowhead_type(_a: &DemoCsdLink) -> canvas::ArrowheadType {
        canvas::ArrowheadType::None
    }
    fn model_to_source_arrowhead_label(_a: &DemoCsdLink) -> Option<&str> {
        None
    }
    fn model_to_destination_arrowhead_label(_a: &DemoCsdLink) -> Option<&str> {
        None
    }

    let predicate_uuid = uuid::Uuid::now_v7();
    let predicate = Arc::new(RwLock::new(DemoCsdLink::new(
        predicate_uuid.clone(),
        link_type,
        source.0,
        destination.0,
    )));
    let predicate_controller = Arc::new(RwLock::new(MulticonnectionView::new(
        predicate.clone(),
        DemoCsdLinkBuffer {
            link_type,
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
