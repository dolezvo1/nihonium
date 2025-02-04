use crate::common::canvas;
use crate::common::controller::{
    ColorLabels, ColorProfile, ContainerGen2, DiagramController, DiagramControllerGen2, ElementController, ElementControllerGen2, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, ModifierKeys, MulticonnectionView, SensitiveCommand, TargettingStatus, Tool, VertexInformation
};
use crate::democsd::democsd_models::{
    DemoCsdTransaction, DemoCsdDiagram, DemoCsdElement, DemoCsdLink, DemoCsdLinkType,
    DemoCsdPackage, DemoCsdTransactor,
};
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
type LinkView = MulticonnectionView<
                DemoCsdLink,
                dyn DemoCsdElement,
                DemoCsdQueryable,
                DemoCsdLinkBuffer,
                NaiveDemoCsdTool,
                DemoCsdElementOrVertex,
                DemoCsdPropChange,
            >;

pub struct DemoCsdQueryable {}

#[derive(Clone)]
pub enum DemoCsdPropChange {
    NameChange(Arc<String>),
    IdentifierChange(Arc<String>),
    TransactorSelfactivatingChange(bool),
    TransactorInternalChange(bool),

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


pub fn colors() -> (String, ColorLabels, HashMap<String, ColorProfile>) {
    let c = crate::common::controller::build_colors!(
                                      ["Light",              "Darker"             ],
        [("Diagram background",       [egui::Color32::WHITE, egui::Color32::GRAY, ]),
         ("Package background",       [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159), ]),
         ("Connection background",    [egui::Color32::WHITE, egui::Color32::WHITE,]),
         ("External role background", [egui::Color32::LIGHT_GRAY, egui::Color32::from_rgb(127, 127, 127), ]),
         ("Internal role background", [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159), ]),
         ("Transaction background",   [egui::Color32::WHITE, egui::Color32::from_rgb(191, 191, 191), ]),],
        [("Diagram gridlines",        [egui::Color32::from_rgb(220, 220, 220),  egui::Color32::from_rgb(127, 127, 127), ]),
         ("Package foreground",       [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Connection foreground",    [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("External role foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Internal role foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Transaction foreground",   [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Performa Transaction",     [egui::Color32::RED,   egui::Color32::RED,  ]),],
        [("Selection",                [egui::Color32::BLUE,  egui::Color32::LIGHT_BLUE, ]),],
    );
    ("DEMO CSD diagram".to_owned(), c.0, c.1)
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
        commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::PropertyChange(
            std::iter::once(buffer.uuid).collect(),
            vec![DemoCsdPropChange::NameChange(Arc::new(buffer.name.clone()))],
        )));
    };

    ui.label("Comment:");
    if ui
        .add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut buffer.comment),
        )
        .changed()
    {
        commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::PropertyChange(
            std::iter::once(buffer.uuid).collect(),
            vec![DemoCsdPropChange::CommentChange(Arc::new(
                buffer.comment.clone(),
            ))],
        )));
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
            (DemoCsdToolStage::LinkStart { link_type: DemoCsdLinkType::Initiation }, "Initiation"),
            (DemoCsdToolStage::LinkStart { link_type: DemoCsdLinkType::Interstriction }, "Interstriction"),
            (DemoCsdToolStage::LinkStart { link_type: DemoCsdLinkType::Interimpediment }, "Interimpediment"),
        ][..],
        &[(DemoCsdToolStage::PackageStart, "Package")][..],
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

fn menubar_options_fun(_controller: &mut DiagramView, _context: &mut NHApp, _ui: &mut egui::Ui) {}

pub fn new(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let uuid = uuid::Uuid::now_v7();
    let name = format!("New DEMO CSD diagram {}", no);

    let diagram = Arc::new(RwLock::new(DemoCsdDiagram::new(
        uuid.clone(),
        name.clone(),
        vec![],
    )));
    (
        uuid,
        DiagramControllerGen2::new(
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
        ),
    )
}

pub fn demo(no: u32) -> (uuid::Uuid, Arc<RwLock<dyn DiagramController>>) {
    let mut models: Vec<Arc<RwLock<dyn DemoCsdElement>>> = vec![];
    let mut controllers =
        HashMap::<_, Arc<RwLock<dyn ElementControllerGen2<_, _, _, _, _>>>>::new();

    {
        let (client_uuid, client, client_controller) = democsd_transactor(
            "CTAR01", "Client",
            false, None, false, egui::Pos2::new(200.0, 200.0),
        );
        
        models.push(client);
        controllers.insert(client_uuid, client_controller);
    }

    {
        let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
            "TK01", "Sale completion",
            egui::Pos2::new(200.0, 400.0), true,
        );
        
        let (transactor_uuid, transactor, transactor_controller) = democsd_transactor(
            "AR01", "Sale completer",
            true, Some((transaction, transaction_controller)), false, egui::Pos2::new(200.0, 400.0),
        );
        models.push(transactor);
        controllers.insert(transactor_uuid, transactor_controller);
    }
    
    {
        let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
            "TK10", "Sale transportation",
            egui::Pos2::new(200.0, 600.0), true,
        );

        let (transactor_uuid, transactor, transactor_controller) = democsd_transactor(
            "AR02", "Sale transporter",
            true, Some((transaction, transaction_controller)), false, egui::Pos2::new(200.0, 600.0),
        );
        models.push(transactor);
        controllers.insert(transactor_uuid, transactor_controller);
    }
    
    {
        let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
            "TK11", "Sale controlling",
            egui::Pos2::new(400.0, 200.0), true,
        );
        
        let (transactor_uuid, transactor, transactor_controller) = democsd_transactor(
            "AR03", "Sale controller",
            true, Some((transaction, transaction_controller)), true, egui::Pos2::new(400.0, 200.0),
        );
        models.push(transactor);
        controllers.insert(transactor_uuid, transactor_controller);
    }
    
    // TK02 - Purchase completer

    {
        let uuid = uuid::Uuid::now_v7();
        let name = format!("New DEMO CSD diagram {}", no);
        let diagram = Arc::new(RwLock::new(DemoCsdDiagram::new(
            uuid.clone(),
            name.clone(),
            models,
        )));
        (
            uuid,
            DiagramControllerGen2::new(
                diagram.clone(),
                controllers,
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
            ),
        )
    }
}

#[derive(Clone, Copy)]
pub enum KindedDemoCsdElement<'a> {
    Diagram {},
    Package {
        inner: &'a PackageView,
    },
    Transactor {
        inner: &'a DemoCsdTransactorView,
    },
    Bank {
        inner: &'a DemoCsdTransactionView,
    },
    Link {
        inner: &'a LinkView,
    },
}

impl<'a> From<&'a DiagramView> for KindedDemoCsdElement<'a> {
    fn from(_from: &'a DiagramView) -> Self {
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
    LinkStart { link_type: DemoCsdLinkType },
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialDemoCsdElement {
    None,
    Some((uuid::Uuid, ArcRwLockController)),
    Link {
        link_type: DemoCsdLinkType,
        source: Arc<RwLock<DemoCsdTransactor>>,
        source_view: ArcRwLockController,
        dest: Option<Arc<RwLock<DemoCsdTransaction>>>,
    },
    Package {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveDemoCsdTool {
    initial_stage: DemoCsdToolStage,
    current_stage: DemoCsdToolStage,
    result: PartialDemoCsdElement,
    event_lock: bool,
}

impl NaiveDemoCsdTool {
    pub fn new(initial_stage: DemoCsdToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialDemoCsdElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<dyn DemoCsdElement, DemoCsdQueryable, DemoCsdElementOrVertex, DemoCsdPropChange>
    for NaiveDemoCsdTool
{
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
                DemoCsdToolStage::LinkStart {..} | DemoCsdToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Package { .. } => match self.current_stage {
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => TARGETTABLE_COLOR,
                DemoCsdToolStage::LinkStart {..} | DemoCsdToolStage::LinkEnd => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Transactor { .. } => match self.current_stage {
                DemoCsdToolStage::LinkStart {..} => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkEnd
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Bank { .. } => match self.current_stage {
                DemoCsdToolStage::LinkEnd => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkStart {..}
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Link { .. } => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn canvas::NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialDemoCsdElement::Link { source_view, link_type, .. } => {
                canvas.draw_line(
                    [source_view.read().unwrap().position(), pos],
                    match link_type.line_type() {
                        canvas::LineType::Solid => canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::LineType::Dashed => canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    },
                    canvas::Highlight::NONE,
                );
            }
            PartialDemoCsdElement::Package { a, .. } => {
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

        match (self.current_stage, &mut self.result) {
            (DemoCsdToolStage::Client, _) => {
                let (client_uuid, _client, client_controller) = democsd_transactor(
                    "CTAR01", "Client",
                    false, None,
                    false, pos,
                );
                self.result = PartialDemoCsdElement::Some((client_uuid, client_controller));
                self.event_lock = true;
            }
            (DemoCsdToolStage::Transactor, _) => {
                let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
                    "TK01", "Transaction",
                    pos, true,
                );
                let (transactor_uuid, _transactor, transactor_controller) = democsd_transactor(
                    "AR01", "Transactor",
                    true, Some((transaction, transaction_controller)),
                    false, pos,
                );
                self.result = PartialDemoCsdElement::Some((transactor_uuid, transactor_controller));
                self.event_lock = true;
            }
            (DemoCsdToolStage::Bank, _) => {
                let (bank_uuid, _bank, bank_controller) = democsd_transaction(
                    "TK01", "Bank",
                    pos, false,
                );
                self.result = PartialDemoCsdElement::Some((bank_uuid, bank_controller));
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageStart, _) => {
                self.result = PartialDemoCsdElement::Package {
                    a: pos,
                    b: None,
                };
                self.current_stage = DemoCsdToolStage::PackageEnd;
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageEnd, PartialDemoCsdElement::Package { ref mut b, .. }) => {
                *b = Some(pos)
            }
            (DemoCsdToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>) {
        if self.event_lock {
            return;
        }

        match controller {
            KindedDemoCsdElement::Diagram { .. } => {}
            KindedDemoCsdElement::Package { .. } => {}
            KindedDemoCsdElement::Transactor { inner } => match (self.current_stage, &mut self.result) {
                (DemoCsdToolStage::LinkStart { link_type }, PartialDemoCsdElement::None) => {
                    self.result = PartialDemoCsdElement::Link {
                        link_type,
                        source: inner.model.clone(),
                        source_view: inner.self_reference.upgrade().unwrap(),
                        dest: None,
                    };
                    self.current_stage = DemoCsdToolStage::LinkEnd;
                    self.event_lock = true;
                }
                _ => {}
            }
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
                link_type,
                ..
            } => {
                self.current_stage = self.initial_stage;

                let predicate_controller: Option<(uuid::Uuid, ArcRwLockController)> =
                    if let (Some(source_controller), Some(dest_controller)) = (
                        into.controller_for(&source.read().unwrap().uuid()),
                        into.controller_for(&dest.read().unwrap().uuid()),
                    ) {
                        let (uuid, _, predicate_controller) = democsd_link(
                            *link_type,
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
    ElementControllerGen2<
    dyn DemoCsdElement,
    DemoCsdQueryable,
    NaiveDemoCsdTool,
    DemoCsdElementOrVertex,
    DemoCsdPropChange,
>
{
    fn is_connection_from(&self, _uuid: &uuid::Uuid) -> bool {
        false
    }
    fn connection_target_name(&self) -> Option<Arc<String>> {
        None
    }
}

pub trait RdfContainerController {
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<Arc<RwLock<dyn DemoCsdElementController>>>;
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

// ---

fn democsd_transactor(
    identifier: &str,
    name: &str,
    internal: bool,
    transaction: Option<(Arc<std::sync::RwLock<DemoCsdTransaction>>, Arc<std::sync::RwLock<DemoCsdTransactionView>>)>,
    transaction_selfactivating: bool,
    position: egui::Pos2,
) -> (uuid::Uuid, Arc<RwLock<DemoCsdTransactor>>, Arc<RwLock<DemoCsdTransactorView>>) {
    let ta_uuid = uuid::Uuid::now_v7();
    let ta = Arc::new(RwLock::new(DemoCsdTransactor::new(
        ta_uuid,
        identifier.to_owned(),
        name.to_owned(),
        internal,
        transaction.as_ref().map(|t| t.0.clone()),
        transaction_selfactivating,
    )));
    let ta_controller = Arc::new(RwLock::new(DemoCsdTransactorView {
        model: ta.clone(),
        self_reference: Weak::new(),
        transaction_view: transaction.map(|t| t.1),

        identifier_buffer: identifier.to_owned(),
        name_buffer: name.to_owned(),
        internal_buffer: internal,
        transaction_selfactivating_buffer: transaction_selfactivating,
        comment_buffer: "".to_owned(),

        dragged: false,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::ZERO,
    }));
    
    ta_controller.write().unwrap().self_reference = Arc::downgrade(&ta_controller);
    (ta_uuid, ta, ta_controller)
}

pub struct DemoCsdTransactorView {
    model: Arc<RwLock<DemoCsdTransactor>>,
    self_reference: Weak<RwLock<Self>>,
    transaction_view: Option<Arc<RwLock<DemoCsdTransactionView>>>,
    
    identifier_buffer: String,
    name_buffer: String,
    internal_buffer: bool,
    transaction_selfactivating_buffer: bool,
    comment_buffer: String,

    dragged: bool,
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
        canvas::NHShape::Rect {
            inner: self.bounds_rect,
        }
    }
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ContainerGen2<dyn DemoCsdElement, DemoCsdQueryable, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange>
    for DemoCsdTransactorView
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<ArcRwLockController> {    
        match &self.transaction_view {
            Some(t) if *uuid == *t.read().unwrap().uuid() => {
                Some(t.clone() as ArcRwLockController)
            }
            _ => None
        }
    }
}

impl
    ElementControllerGen2<
        dyn DemoCsdElement,
        DemoCsdQueryable,
        NaiveDemoCsdTool,
        DemoCsdElementOrVertex,
        DemoCsdPropChange,
    > for DemoCsdTransactorView
{
    fn show_properties(
        &mut self,
        queryable: &DemoCsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> bool {
        if let Some(t) = &self.transaction_view {
            let mut t = t.write().unwrap();
            if t.show_properties(queryable, ui, commands) {
                return true;
            }
        }
        
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

        if ui.checkbox(&mut self.transaction_selfactivating_buffer, "Transaction Self-activating").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::TransactorSelfactivatingChange(self.transaction_selfactivating_buffer)
            ]));
        }

        if ui.checkbox(&mut self.internal_buffer, "Internal").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::TransactorInternalChange(self.internal_buffer)
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
        queryable: &DemoCsdQueryable,
        canvas: &mut dyn canvas::NHCanvas,
        profile: &ColorProfile,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        let read = self.model.read().unwrap();

        let radius = 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE;

        let tx_name_bounds = if let Some(t) = &self.transaction_view {
            let t = t.write().unwrap();
            let m = t.model.write().unwrap();
            canvas.measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &m.name,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
        } else { egui::Rect::ZERO };
        let [identifier_bounds, name_bounds] = [&read.identifier, &read.name].map(|e| {
            canvas.measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                e,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
        });
        let [identifier_offset, name_offset] = [0.0, identifier_bounds.height()].map(|e|
            egui::Vec2::new(0.0, e + if read.transaction.is_some() { tx_name_bounds.height() - 1.0 * canvas::CLASS_MIDDLE_FONT_SIZE } else { 0.0 })
        );

        let max_row = tx_name_bounds
                    .width()
                    .max(identifier_bounds.width())
                    .max(name_bounds.width())
                    .max(2.0 * radius);
        let box_y_offset = if read.transaction.is_some() { if read.transaction_selfactivating { 6.0 } else { 3.5 } } else { 0.0 } * canvas::CLASS_MIDDLE_FONT_SIZE;
        self.bounds_rect = egui::Rect::from_min_size(
            self.position - egui::Vec2::new(max_row/2.0, box_y_offset),
            egui::Vec2::new(
                max_row,
                if read.transaction.is_some() { if read.transaction_selfactivating { 5.0 } else { 2.5 } } else { 0.0 }* canvas::CLASS_MIDDLE_FONT_SIZE
                    + tx_name_bounds.height()
                    + identifier_bounds.height()
                    + name_bounds.height(),
            ),
        )
        .expand(5.0);

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::Rounding::ZERO,
            if read.internal {
                profile.backgrounds[4]
            } else {
                profile.backgrounds[3]
            },
            canvas::Stroke::new_solid(
                1.0,
                if read.internal {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::DARK_GRAY
                },
            ),
            self.highlight,
        );
        
        let text_color = if read.internal {
            profile.foregrounds[4]
        } else {
            profile.foregrounds[3]
        };
        
        // Draw identifier below the position (plus tx name)
        canvas.draw_text(
            self.position + identifier_offset,
            egui::Align2::CENTER_TOP,
            &read.identifier,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            text_color,
        );

        // Draw identifier one row below the position (plus tx name)
        canvas.draw_text(
            self.position + name_offset,
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            text_color,
        );
        
        // If tx is present, draw it 4 rows above the position
        if let Some(t) = &self.transaction_view {
            let mut t = t.write().unwrap();
            let res = t.draw_in(queryable, canvas, profile, &tool);
            if res == TargettingStatus::Drawn {
                return TargettingStatus::Drawn;
            }
        }
        
        // canvas.draw_ellipse(self.position, egui::Vec2::splat(1.0), egui::Color32::RED, canvas::Stroke::new_solid(1.0, egui::Color32::RED), canvas::Highlight::NONE);

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

    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
        tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        let child = self.transaction_view.as_ref()
            .map(|t| t.write().unwrap().handle_event(event, modifiers, tool, commands))
            .filter(|e| *e != EventHandlingStatus::NotHandled);
    
        match event {
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) | InputEvent::Drag { .. } if child.is_some() => EventHandlingStatus::HandledByContainer,
            InputEvent::MouseDown(pos) | InputEvent::MouseUp(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }
                if matches!(event, InputEvent::MouseDown(_)) {
                    self.dragged = true;
                    EventHandlingStatus::HandledByElement
                } else if self.dragged {
                    self.dragged = false;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            },
            InputEvent::Click(pos) => {
                if let Some(t) = &self.transaction_view {
                    let mut t = t.write().unwrap();
                    match t.handle_event(event, modifiers, tool, commands) {
                        EventHandlingStatus::NotHandled => {},
                        EventHandlingStatus::HandledByElement => {
                            if !modifiers.command {
                                commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::SelectAll(false)));
                                commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(*t.uuid()).collect(),
                                    true,
                                )));
                            } else {
                                commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(*t.uuid()).collect(),
                                    !t.highlight.selected,
                                )));
                            }
                            return EventHandlingStatus::HandledByContainer;
                        }
                        EventHandlingStatus::HandledByContainer => {
                            return EventHandlingStatus::HandledByContainer;
                        }
                    }
                }
                
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }

                if let Some(tool) = tool {
                    tool.add_element(KindedDemoCsdElement::Transactor { inner: self });
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
                    commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::MoveSpecificElements(
                        std::iter::once(*self.uuid()).collect(),
                        delta,
                    )));
                }

                EventHandlingStatus::HandledByElement
            },
            _ => EventHandlingStatus::NotHandled
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        macro_rules! recurse {
            ($self:ident) => {
                if let Some(t) = &$self.transaction_view {
                    let mut t = t.write().unwrap();
                    t.apply_command(command, undo_accumulator);
                }
            }
        }
        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
                recurse!(self);
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }
                recurse!(self);
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
                recurse!(self);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, delta)
                if !uuids.contains(&*self.uuid()) && !self.transaction_view.as_ref().is_some_and(|e| uuids.contains(&e.read().unwrap().uuid())) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
                if let Some(t) = &self.transaction_view {
                    let mut t = t.write().unwrap();
                    t.apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut vec![]);
                }
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddElement(..) => {}
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
                            DemoCsdPropChange::TransactorSelfactivatingChange(value) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::TransactorSelfactivatingChange(model.transaction_selfactivating)],
                                ));
                                self.transaction_selfactivating_buffer = value.clone();
                                model.transaction_selfactivating = value.clone();
                            }
                            DemoCsdPropChange::TransactorInternalChange(value) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::TransactorInternalChange(model.internal)],
                                ));
                                self.internal_buffer = value.clone();
                                model.internal = value.clone();
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
                
                recurse!(self);
            }
        }
    }

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }
        if let Some(t) = &self.transaction_view {
            let mut t = t.write().unwrap();
            t.collect_all_selected_elements(into);
        }
    }
}

fn democsd_transaction(
    identifier: &str,
    name: &str,
    position: egui::Pos2,
    actor: bool,
) -> (uuid::Uuid, Arc<RwLock<DemoCsdTransaction>>, Arc<RwLock<DemoCsdTransactionView>>) {
    let transaction_uuid = uuid::Uuid::now_v7();
    let transaction = Arc::new(RwLock::new(DemoCsdTransaction::new(
        transaction_uuid,
        identifier.to_owned(),
        name.to_owned(),
    )));
    let transaction_controller = Arc::new(RwLock::new(DemoCsdTransactionView {
        model: transaction.clone(),
        
        identifier_buffer: identifier.to_owned(),
        name_buffer: name.to_owned(),
        comment_buffer: "".to_owned(),
        
        dragged: false,
        highlight: canvas::Highlight::NONE,
        position: position - if actor {egui::Vec2::new(0.0, 3.84 * canvas::CLASS_MIDDLE_FONT_SIZE)} else {egui::Vec2::ZERO},
        min_shape: canvas::NHShape::ELLIPSE_ZERO,
    }));
    (transaction_uuid, transaction, transaction_controller)
}

pub struct DemoCsdTransactionView {
    model: Arc<RwLock<DemoCsdTransaction>>,

    identifier_buffer: String,
    name_buffer: String,
    comment_buffer: String,

    dragged: bool,
    highlight: canvas::Highlight,
    position: egui::Pos2,
    min_shape: canvas::NHShape,
}

impl ElementController<dyn DemoCsdElement> for DemoCsdTransactionView {
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

impl ContainerGen2<dyn DemoCsdElement, DemoCsdQueryable, NaiveDemoCsdTool, DemoCsdElementOrVertex, DemoCsdPropChange>
    for DemoCsdTransactionView
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<ArcRwLockController> {
        None
    }
}

fn draw_tx_mark(
    canvas: &mut dyn canvas::NHCanvas,
    identifier: &str,
    position: egui::Pos2,
    radius: f32,
    highlight: canvas::Highlight,
    background: egui::Color32,
    foreground: egui::Color32,
    transaction: egui::Color32,
) -> canvas::NHShape {
    canvas.draw_ellipse(
        position,
        egui::Vec2::splat(radius),
        background,
        canvas::Stroke::new_solid(1.0, foreground),
        highlight,
    );

    let pts = [
        position - egui::Vec2::new(0.0, radius),
        position + egui::Vec2::new(radius, 0.0),
        position + egui::Vec2::new(0.0, radius),
        position - egui::Vec2::new(radius, 0.0),
        position - egui::Vec2::new(0.0, radius),
    ];
    let mut iter = pts.iter().peekable();
    while let Some(u) = iter.next() {
        let Some(v) = iter.peek() else {
            break;
        };
        canvas.draw_line(
            [*u, **v],
            canvas::Stroke::new_solid(1.0, transaction),
            canvas::Highlight::NONE,
        );
    }

    canvas.draw_text(
        position,
        egui::Align2::CENTER_CENTER,
        identifier,
        canvas::CLASS_MIDDLE_FONT_SIZE,
        foreground,
    );

    canvas::NHShape::Ellipse {
        position: position,
        bounds_radius: egui::Vec2::splat(radius),
    }
}

impl
    ElementControllerGen2<
        dyn DemoCsdElement,
        DemoCsdQueryable,
        NaiveDemoCsdTool,
        DemoCsdElementOrVertex,
        DemoCsdPropChange,
    > for DemoCsdTransactionView
{
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
        profile: &ColorProfile,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        let radius = 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE;
        let read = self.model.read().unwrap();

        self.min_shape = draw_tx_mark(
            canvas,
            &read.identifier,
            self.position,
            radius,
            self.highlight,
            profile.backgrounds[5],
            profile.foregrounds[5],
            profile.foregrounds[6],
        );

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

    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
        tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            e if !self.min_shape().contains(*e.mouse_position()) => return EventHandlingStatus::NotHandled,
            InputEvent::MouseDown(_) => {
                self.dragged = true;
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => if self.dragged {
                self.dragged = false;
                EventHandlingStatus::HandledByElement
            } else {
                EventHandlingStatus::NotHandled
            },
            InputEvent::Click(_) => {
                if let Some(tool) = tool {
                    tool.add_element(KindedDemoCsdElement::Bank { inner: self });
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
                    commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::MoveSpecificElements(
                        std::iter::once(*self.uuid()).collect(),
                        delta,
                    )));
                }

                EventHandlingStatus::HandledByElement
            },
            _ => EventHandlingStatus::NotHandled
        }
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
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, delta) if !uuids.contains(&*self.uuid()) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddElement(..) => {}
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
    source: (
        Arc<std::sync::RwLock<DemoCsdTransactor>>,
        ArcRwLockController,
    ),
    destination: (
        Arc<std::sync::RwLock<DemoCsdTransaction>>,
        ArcRwLockController,
    ),
) -> (
    uuid::Uuid,
    Arc<RwLock<DemoCsdLink>>,
    Arc<
        RwLock<
            LinkView,
        >,
    >,
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
        egui::ComboBox::from_id_salt("Type:")
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
            DemoCsdLinkType::Interstriction | DemoCsdLinkType::Interimpediment => {
                canvas::LineType::Dashed
            }
        }
    }
    fn model_to_source_arrowhead_type(a: &DemoCsdLink) -> canvas::ArrowheadType {
        match a.link_type {
            DemoCsdLinkType::Initiation | DemoCsdLinkType::Interstriction => {
                canvas::ArrowheadType::None
            }
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
