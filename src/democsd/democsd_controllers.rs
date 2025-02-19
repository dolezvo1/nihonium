use crate::common::canvas::{self, NHShape};
use crate::common::controller::{
    arc_to_usize, ColorLabels, ColorProfile, ContainerGen2, ContainerModel, DiagramController, DiagramControllerGen2, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, MulticonnectionAdapter, MulticonnectionView, PackageAdapter, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, VertexInformation
};
use crate::democsd::democsd_models::{
    DemoCsdDiagram, DemoCsdElement, DemoCsdLink, DemoCsdLinkType, DemoCsdPackage,
    DemoCsdTransaction, DemoCsdTransactor,
};
use crate::NHApp;
use eframe::egui;
use std::any::Any;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock, Weak},
};

type ControllerT = dyn ElementControllerGen2<
    dyn DemoCsdElement,
    DemoCsdQueryable,
    NaiveDemoCsdTool,
    DemoCsdElementOrVertex,
    DemoCsdPropChange,
>;
type ArcRwLockControllerT = Arc<RwLock<ControllerT>>;
type DiagramViewT = DiagramControllerGen2<
    DemoCsdDiagram,
    dyn DemoCsdElement,
    DemoCsdQueryable,
    DemoCsdDiagramBuffer,
    NaiveDemoCsdTool,
    DemoCsdElementOrVertex,
    DemoCsdPropChange,
>;
type PackageViewT = crate::common::controller::PackageView<
    DemoCsdPackageAdapter,
    dyn DemoCsdElement,
    DemoCsdQueryable,
    NaiveDemoCsdTool,
    DemoCsdElementOrVertex,
    DemoCsdPropChange,
>;
type LinkViewT = MulticonnectionView<
    DemoCsdLinkAdapter,
    dyn DemoCsdElement,
    DemoCsdQueryable,
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
    Element((uuid::Uuid, ArcRwLockControllerT)),
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

impl From<(uuid::Uuid, ArcRwLockControllerT)> for DemoCsdElementOrVertex {
    fn from(v: (uuid::Uuid, ArcRwLockControllerT)) -> Self {
        DemoCsdElementOrVertex::Element(v)
    }
}

impl TryInto<(uuid::Uuid, ArcRwLockControllerT)> for DemoCsdElementOrVertex {
    type Error = ();

    fn try_into(self) -> Result<(uuid::Uuid, ArcRwLockControllerT), ()> {
        match self {
            DemoCsdElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

pub fn colors() -> (String, ColorLabels, Vec<ColorProfile>) {
    #[rustfmt::skip]
    let c = crate::common::controller::build_colors!(
                                      ["Light",              "Darker"],
        [("Diagram background",       [egui::Color32::WHITE, egui::Color32::GRAY,]),
         ("Package background",       [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),
         ("Connection background",    [egui::Color32::WHITE, egui::Color32::WHITE,]),
         ("External role background", [egui::Color32::LIGHT_GRAY, egui::Color32::from_rgb(127, 127, 127),]),
         ("Internal role background", [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),
         ("Transaction background",   [egui::Color32::WHITE, egui::Color32::from_rgb(191, 191, 191),]),],
        [("Diagram gridlines",        [egui::Color32::from_rgb(220, 220, 220), egui::Color32::from_rgb(127, 127, 127),]),
         ("Package foreground",       [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Connection foreground",    [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("External role foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Internal role foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Transaction foreground",   [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Performa Transaction",     [egui::Color32::RED,   egui::Color32::RED,]),],
        [("Selection",                [egui::Color32::BLUE,  egui::Color32::LIGHT_BLUE,]),],
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
        commands.push(
            InsensitiveCommand::PropertyChange(
                std::iter::once(buffer.uuid).collect(),
                vec![DemoCsdPropChange::NameChange(Arc::new(buffer.name.clone()))],
            )
            .into(),
        );
    };

    ui.label("Comment:");
    if ui
        .add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut buffer.comment),
        )
        .changed()
    {
        commands.push(
            InsensitiveCommand::PropertyChange(
                std::iter::once(buffer.uuid).collect(),
                vec![DemoCsdPropChange::CommentChange(Arc::new(
                    buffer.comment.clone(),
                ))],
            )
            .into(),
        );
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
            (
                DemoCsdToolStage::LinkStart {
                    link_type: DemoCsdLinkType::Initiation,
                },
                "Initiation",
            ),
            (
                DemoCsdToolStage::LinkStart {
                    link_type: DemoCsdLinkType::Interstriction,
                },
                "Interstriction",
            ),
            (
                DemoCsdToolStage::LinkStart {
                    link_type: DemoCsdLinkType::Interimpediment,
                },
                "Interimpediment",
            ),
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

fn menubar_options_fun(_controller: &mut DiagramViewT, _context: &mut NHApp, _ui: &mut egui::Ui) {}

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
            "CTAR01",
            "Client",
            false,
            None,
            false,
            egui::Pos2::new(200.0, 200.0),
        );

        models.push(client);
        controllers.insert(client_uuid, client_controller);
    }

    {
        let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
            "TK01",
            "Sale completion",
            egui::Pos2::new(200.0, 400.0),
            true,
        );

        let (transactor_uuid, transactor, transactor_controller) = democsd_transactor(
            "AR01",
            "Sale completer",
            true,
            Some((transaction, transaction_controller)),
            false,
            egui::Pos2::new(200.0, 400.0),
        );
        models.push(transactor);
        controllers.insert(transactor_uuid, transactor_controller);
    }

    {
        let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
            "TK10",
            "Sale transportation",
            egui::Pos2::new(200.0, 600.0),
            true,
        );

        let (transactor_uuid, transactor, transactor_controller) = democsd_transactor(
            "AR02",
            "Sale transporter",
            true,
            Some((transaction, transaction_controller)),
            false,
            egui::Pos2::new(200.0, 600.0),
        );
        models.push(transactor);
        controllers.insert(transactor_uuid, transactor_controller);
    }

    {
        let (_transaction_uuid, transaction, transaction_controller) = democsd_transaction(
            "TK11",
            "Sale controlling",
            egui::Pos2::new(400.0, 200.0),
            true,
        );

        let (transactor_uuid, transactor, transactor_controller) = democsd_transactor(
            "AR03",
            "Sale controller",
            true,
            Some((transaction, transaction_controller)),
            true,
            egui::Pos2::new(400.0, 200.0),
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
    Package { inner: &'a PackageViewT },
    Transactor { inner: &'a DemoCsdTransactorView },
    Bank { inner: &'a DemoCsdTransactionView },
    Link { inner: &'a LinkViewT },
}

impl<'a> From<&'a DiagramViewT> for KindedDemoCsdElement<'a> {
    fn from(_from: &'a DiagramViewT) -> Self {
        Self::Diagram {}
    }
}

impl<'a> From<&'a PackageViewT> for KindedDemoCsdElement<'a> {
    fn from(from: &'a PackageViewT) -> Self {
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
    Some((uuid::Uuid, ArcRwLockControllerT)),
    Link {
        link_type: DemoCsdLinkType,
        source: Arc<RwLock<DemoCsdTransactor>>,
        source_view: ArcRwLockControllerT,
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
                DemoCsdToolStage::LinkStart { .. } | DemoCsdToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            KindedDemoCsdElement::Package { .. } => match self.current_stage {
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => TARGETTABLE_COLOR,
                DemoCsdToolStage::LinkStart { .. } | DemoCsdToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            KindedDemoCsdElement::Transactor { .. } => match self.current_stage {
                DemoCsdToolStage::LinkStart { .. } => TARGETTABLE_COLOR,
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
                | DemoCsdToolStage::LinkStart { .. }
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            KindedDemoCsdElement::Link { .. } => todo!(),
        }
    }
    fn draw_status_hint(&self, canvas: &mut dyn canvas::NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialDemoCsdElement::Link {
                source_view,
                link_type,
                ..
            } => {
                canvas.draw_line(
                    [source_view.read().unwrap().position(), pos],
                    match link_type.line_type() {
                        canvas::LineType::Solid => {
                            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK)
                        }
                        canvas::LineType::Dashed => {
                            canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK)
                        }
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
                let (client_uuid, _client, client_controller) =
                    democsd_transactor("CTAR01", "Client", false, None, false, pos);
                self.result = PartialDemoCsdElement::Some((client_uuid, client_controller));
                self.event_lock = true;
            }
            (DemoCsdToolStage::Transactor, _) => {
                let (_transaction_uuid, transaction, transaction_controller) =
                    democsd_transaction("TK01", "Transaction", pos, true);
                let (transactor_uuid, _transactor, transactor_controller) = democsd_transactor(
                    "AR01",
                    "Transactor",
                    true,
                    Some((transaction, transaction_controller)),
                    false,
                    pos,
                );
                self.result = PartialDemoCsdElement::Some((transactor_uuid, transactor_controller));
                self.event_lock = true;
            }
            (DemoCsdToolStage::Bank, _) => {
                let (bank_uuid, _bank, bank_controller) =
                    democsd_transaction("TK01", "Bank", pos, false);
                self.result = PartialDemoCsdElement::Some((bank_uuid, bank_controller));
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageStart, _) => {
                self.result = PartialDemoCsdElement::Package { a: pos, b: None };
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
            KindedDemoCsdElement::Transactor { inner } => {
                match (self.current_stage, &mut self.result) {
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
    ) -> Option<(uuid::Uuid, ArcRwLockControllerT)> {
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

                let predicate_controller: Option<(uuid::Uuid, ArcRwLockControllerT)> =
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

#[derive(Clone)]
pub struct DemoCsdPackageAdapter {
    model: Arc<RwLock<DemoCsdPackage>>,
}

impl PackageAdapter<dyn DemoCsdElement, DemoCsdElementOrVertex, DemoCsdPropChange> for DemoCsdPackageAdapter {
    fn model(&self) -> Arc<RwLock<dyn DemoCsdElement>> {
        self.model.clone()
    }

    fn model_uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }

    fn add_element(&mut self, e: Arc<RwLock<dyn DemoCsdElement>>) {
        self.model.write().unwrap().add_element(e);
    }

    fn delete_elements(&mut self, uuids: &std::collections::HashSet<uuid::Uuid>) {
        self.model.write().unwrap().delete_elements(uuids);
    }
    
    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>
    ) {
        let model = self.model.read().unwrap();
        let mut name_buffer = (*model.name).clone();
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::NameChange(Arc::new(name_buffer)),
            ]));
        }

        let mut comment_buffer = (*model.comment).clone();
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::CommentChange(Arc::new(comment_buffer)),
            ]));
        }
    }

    fn apply_change(
        &self,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    DemoCsdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid).collect(),
                            vec![DemoCsdPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid).collect(),
                            vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
                        ));
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn deep_copy_init(
        &self,
        uuid: uuid::Uuid,
        m: &mut HashMap<usize, (Arc<RwLock<dyn DemoCsdElement>>, Arc<dyn Any + Send + Sync>)>,
    ) -> Self where Self: Sized {
        let model = self.model.read().unwrap();
        let model = Arc::new(RwLock::new(DemoCsdPackage::new(uuid, (*model.name).clone(), model.contained_elements.clone())));
        m.insert(arc_to_usize(&self.model), (model.clone(), model.clone()));
        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<usize, (Arc<RwLock<dyn DemoCsdElement>>, Arc<dyn Any + Send + Sync>)>
    ) {
        todo!()
    }
}

fn democsd_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (
    uuid::Uuid,
    Arc<RwLock<DemoCsdPackage>>,
    Arc<RwLock<PackageViewT>>,
) {
    let uuid = uuid::Uuid::now_v7();
    let graph = Arc::new(RwLock::new(DemoCsdPackage::new(
        uuid.clone(),
        name.to_owned(),
        vec![],
    )));
    let graph_controller = PackageViewT::new(
        DemoCsdPackageAdapter {
            model: graph.clone(),
        },
        HashMap::new(),
        bounds_rect,
    );

    (uuid, graph, graph_controller)
}

// ---

fn democsd_transactor(
    identifier: &str,
    name: &str,
    internal: bool,
    transaction: Option<(
        Arc<std::sync::RwLock<DemoCsdTransaction>>,
        Arc<std::sync::RwLock<DemoCsdTransactionView>>,
    )>,
    transaction_selfactivating: bool,
    position: egui::Pos2,
) -> (
    uuid::Uuid,
    Arc<RwLock<DemoCsdTransactor>>,
    Arc<RwLock<DemoCsdTransactorView>>,
) {
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

        dragged_shape: None,
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

    dragged_shape: Option<NHShape>,
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

impl
    ContainerGen2<
        dyn DemoCsdElement,
        DemoCsdQueryable,
        NaiveDemoCsdTool,
        DemoCsdElementOrVertex,
        DemoCsdPropChange,
    > for DemoCsdTransactorView
{
    fn controller_for(&self, uuid: &uuid::Uuid) -> Option<ArcRwLockControllerT> {
        match &self.transaction_view {
            Some(t) if *uuid == *t.read().unwrap().uuid() => {
                Some(t.clone() as ArcRwLockControllerT)
            }
            _ => None,
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

        if ui
            .checkbox(
                &mut self.transaction_selfactivating_buffer,
                "Transaction Self-activating",
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::TransactorSelfactivatingChange(
                    self.transaction_selfactivating_buffer,
                ),
            ]));
        }

        if ui.checkbox(&mut self.internal_buffer, "Internal").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::TransactorInternalChange(self.internal_buffer),
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
        } else {
            egui::Rect::ZERO
        };
        let [identifier_bounds, name_bounds] = [&read.identifier, &read.name].map(|e| {
            canvas.measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                e,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
        });
        let [identifier_offset, name_offset] = [0.0, identifier_bounds.height()].map(|e| {
            egui::Vec2::new(
                0.0,
                e + if read.transaction.is_some() {
                    tx_name_bounds.height() - 1.0 * canvas::CLASS_MIDDLE_FONT_SIZE
                } else {
                    0.0
                },
            )
        });

        let max_row = tx_name_bounds
            .width()
            .max(identifier_bounds.width())
            .max(name_bounds.width())
            .max(2.0 * radius);
        let box_y_offset = if read.transaction.is_some() {
            if read.transaction_selfactivating {
                6.0
            } else {
                3.5
            }
        } else {
            0.0
        } * canvas::CLASS_MIDDLE_FONT_SIZE;
        self.bounds_rect = egui::Rect::from_min_size(
            self.position - egui::Vec2::new(max_row / 2.0, box_y_offset),
            egui::Vec2::new(
                max_row,
                if read.transaction.is_some() {
                    if read.transaction_selfactivating {
                        5.0
                    } else {
                        2.5
                    }
                } else {
                    0.0
                } * canvas::CLASS_MIDDLE_FONT_SIZE
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

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());

        self.transaction_view
            .iter()
            .for_each(|c| c.write().unwrap().collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        let child = self
            .transaction_view
            .as_ref()
            .map(|t| t.write().unwrap().handle_event(event, ehc, tool, commands))
            .filter(|e| *e != EventHandlingStatus::NotHandled);

        match event {
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) | InputEvent::Drag { .. }
                if child.is_some() =>
            {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) | InputEvent::MouseUp(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }
                if matches!(event, InputEvent::MouseDown(_)) {
                    self.dragged_shape = Some(self.min_shape());
                    EventHandlingStatus::HandledByElement
                } else if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) => {
                if let Some(t) = &self.transaction_view {
                    let t = t.read().unwrap();
                    match child {
                        Some(EventHandlingStatus::HandledByElement) => {
                            if !ehc.modifiers.command {
                                commands.push(InsensitiveCommand::SelectAll(false).into());
                                commands.push(
                                    InsensitiveCommand::SelectSpecific(
                                        std::iter::once(*t.uuid()).collect(),
                                        true,
                                    )
                                    .into(),
                                );
                            } else {
                                commands.push(
                                    InsensitiveCommand::SelectSpecific(
                                        std::iter::once(*t.uuid()).collect(),
                                        !t.highlight.selected,
                                    )
                                    .into(),
                                );
                            }
                            return EventHandlingStatus::HandledByContainer;
                        }
                        Some(EventHandlingStatus::HandledByContainer) => {
                            return EventHandlingStatus::HandledByContainer;
                        }
                        _ => {}
                    }
                }

                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }

                if let Some(tool) = tool {
                    tool.add_element(KindedDemoCsdElement::Transactor { inner: self });
                } else {
                    if !ehc.modifiers.command {
                        self.highlight.selected = true;
                    } else {
                        self.highlight.selected = !self.highlight.selected;
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Drag { delta, .. } if self.dragged_shape.is_some() => {
                let translated_real_shape = self.dragged_shape.unwrap().translate(delta);
                self.dragged_shape = Some(translated_real_shape);
                let transaction_id = self.transaction_view.as_ref().map(|t| *t.read().unwrap().uuid());
                let coerced_pos = ehc.snap_manager.coerce(translated_real_shape,
                        |e| !transaction_id.is_some_and(|t| t == *e) && !if self.highlight.selected { ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected) } else {*e == *self.uuid()}
                    );
                let coerced_delta = coerced_pos - self.min_shape().center();
                
                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
                } else {
                    commands.push(
                        InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid()).collect(),
                            coerced_delta,
                        )
                        .into(),
                    );
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
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
            };
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
                if !uuids.contains(&*self.uuid())
                    && !self
                        .transaction_view
                        .as_ref()
                        .is_some_and(|e| uuids.contains(&e.read().unwrap().uuid())) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
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
            | InsensitiveCommand::AddElement(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..) => {}
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
                                    vec![DemoCsdPropChange::TransactorSelfactivatingChange(
                                        model.transaction_selfactivating,
                                    )],
                                ));
                                self.transaction_selfactivating_buffer = value.clone();
                                model.transaction_selfactivating = value.clone();
                            }
                            DemoCsdPropChange::TransactorInternalChange(value) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*model.uuid).collect(),
                                    vec![DemoCsdPropChange::TransactorInternalChange(
                                        model.internal,
                                    )],
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

    fn head_count(&mut self, into: &mut HashMap<uuid::Uuid, SelectionStatus>) {
        into.insert(*self.uuid(), self.highlight.selected.into());

        if let Some(t) = &self.transaction_view {
            let mut t = t.write().unwrap();
            t.head_count(into);
        }
    }
    
    fn deep_copy_init(
        &self,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        c: &mut HashMap<usize, (uuid::Uuid, 
            ArcRwLockControllerT,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<dyn DemoCsdElement>>,
            Arc<dyn Any + Send + Sync>,
        )>
    ) {
        let model = self.model.read().unwrap();
        let uuid = if uuid_present(&*model.uuid) { uuid::Uuid::now_v7() } else { *model.uuid };
        let modelish = Arc::new(RwLock::new(DemoCsdTransactor::new(uuid, (*model.identifier).clone(), (*model.name).clone(), model.internal,
            model.transaction.clone(), model.transaction_selfactivating)));
        m.insert(arc_to_usize(&self.model), (modelish.clone(), modelish.clone()));
        
        let cloneish = Arc::new(RwLock::new(Self {
            model: modelish,
            self_reference: Weak::new(),
            transaction_view: self.transaction_view.clone(),
            identifier_buffer: self.identifier_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            internal_buffer: self.internal_buffer.clone(),
            transaction_selfactivating_buffer: self.transaction_selfactivating_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        c.insert(arc_to_usize(&Weak::upgrade(&self.self_reference).unwrap()), (uuid, cloneish.clone(), cloneish));
    }
    
    fn deep_copy_finish(
        &mut self,
        c: &HashMap<usize, (uuid::Uuid, 
            ArcRwLockControllerT,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &HashMap<usize, (
            Arc<RwLock<dyn DemoCsdElement>>,
            Arc<dyn Any + Send + Sync>,
        )>,
    ) {
        if let Some((_, _, new_ta)) = self.transaction_view.as_ref().and_then(|e| c.get(&arc_to_usize(e)))  {
            let new_ta: Result<Arc<RwLock<DemoCsdTransactionView>>, _> = Arc::downcast(new_ta.clone());
            if let Ok(new_ta) = new_ta {
                self.transaction_view = Some(new_ta.clone());
            }
        }
    }
}

fn democsd_transaction(
    identifier: &str,
    name: &str,
    position: egui::Pos2,
    actor: bool,
) -> (
    uuid::Uuid,
    Arc<RwLock<DemoCsdTransaction>>,
    Arc<RwLock<DemoCsdTransactionView>>,
) {
    let transaction_uuid = uuid::Uuid::now_v7();
    let transaction = Arc::new(RwLock::new(DemoCsdTransaction::new(
        transaction_uuid,
        identifier.to_owned(),
        name.to_owned(),
    )));
    let transaction_controller = Arc::new(RwLock::new(DemoCsdTransactionView {
        model: transaction.clone(),
        self_reference: Weak::new(),

        identifier_buffer: identifier.to_owned(),
        name_buffer: name.to_owned(),
        comment_buffer: "".to_owned(),

        dragged: false,
        highlight: canvas::Highlight::NONE,
        position: position
            - if actor {
                egui::Vec2::new(0.0, 3.84 * canvas::CLASS_MIDDLE_FONT_SIZE)
            } else {
                egui::Vec2::ZERO
            },
        min_shape: canvas::NHShape::ELLIPSE_ZERO,
    }));
    transaction_controller.write().unwrap().self_reference = Arc::downgrade(&transaction_controller);
    (transaction_uuid, transaction, transaction_controller)
}

pub struct DemoCsdTransactionView {
    model: Arc<RwLock<DemoCsdTransaction>>,
    self_reference: Weak<RwLock<Self>>,

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

impl
    ContainerGen2<
        dyn DemoCsdElement,
        DemoCsdQueryable,
        NaiveDemoCsdTool,
        DemoCsdElementOrVertex,
        DemoCsdPropChange,
    > for DemoCsdTransactionView
{
    fn controller_for(&self, _uuid: &uuid::Uuid) -> Option<ArcRwLockControllerT> {
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
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            e if !self.min_shape().contains(*e.mouse_position()) => {
                return EventHandlingStatus::NotHandled
            }
            InputEvent::MouseDown(_) => {
                self.dragged = true;
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.dragged {
                    self.dragged = false;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(_) => {
                if let Some(tool) = tool {
                    tool.add_element(KindedDemoCsdElement::Bank { inner: self });
                } else {
                    if !ehc.modifiers.command {
                        commands.push(InsensitiveCommand::SelectAll(false).into());
                        commands.push(
                            InsensitiveCommand::SelectSpecific(
                                std::iter::once(*self.uuid()).collect(),
                                true,
                            )
                            .into(),
                        );
                    } else {
                        commands.push(
                            InsensitiveCommand::SelectSpecific(
                                std::iter::once(*self.uuid()).collect(),
                                !self.highlight.selected,
                            )
                            .into(),
                        );
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Drag { delta, .. } if self.dragged => {
                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(delta));
                } else {
                    commands.push(
                        InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid()).collect(),
                            delta,
                        )
                        .into(),
                    );
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
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
            InsensitiveCommand::MoveSpecificElements(uuids, _)
                if !uuids.contains(&*self.uuid()) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddElement(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..) => {}
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

    fn head_count(&mut self, into: &mut HashMap<uuid::Uuid, SelectionStatus>) {
        into.insert(*self.uuid(), self.highlight.selected.into());
    }
    
    fn deep_copy_init(
        &self,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        c: &mut HashMap<usize, (uuid::Uuid, 
            ArcRwLockControllerT,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<dyn DemoCsdElement>>,
            Arc<dyn Any + Send + Sync>,
        )>
    ) {
        let model = self.model.read().unwrap();
        let uuid = if uuid_present(&*model.uuid) { uuid::Uuid::now_v7() } else { *model.uuid };
        let modelish = Arc::new(RwLock::new(DemoCsdTransaction::new(uuid, (*model.identifier).clone(), (*model.name).clone())));
        m.insert(arc_to_usize(&self.model), (modelish.clone(), modelish.clone()));
        
        let cloneish = Arc::new(RwLock::new(Self {
            model: modelish,
            self_reference: Weak::new(),
            identifier_buffer: self.identifier_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged: false,
            highlight: self.highlight,
            position: self.position,
            min_shape: self.min_shape,
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        c.insert(arc_to_usize(&Weak::upgrade(&self.self_reference).unwrap()), (uuid, cloneish.clone(), cloneish));
    }
}

#[derive(Clone)]
pub struct DemoCsdLinkAdapter {
    model: Arc<RwLock<DemoCsdLink>>,
}

impl DemoCsdLinkAdapter {
    fn line_type(&self) -> canvas::LineType {
        match self.model.read().unwrap().link_type {
            DemoCsdLinkType::Initiation => canvas::LineType::Solid,
            DemoCsdLinkType::Interstriction | DemoCsdLinkType::Interimpediment => {
                canvas::LineType::Dashed
            }
        }
    }
}

impl MulticonnectionAdapter<dyn DemoCsdElement, DemoCsdElementOrVertex, DemoCsdPropChange> for DemoCsdLinkAdapter {
    fn model(&self) -> Arc<RwLock<dyn DemoCsdElement>> {
        self.model.clone()
    }

    fn model_uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        Arc::new("TODO".to_owned())
    }

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        (self.line_type(), match self.model.read().unwrap().link_type {
            DemoCsdLinkType::Initiation | DemoCsdLinkType::Interstriction => {
                canvas::ArrowheadType::None
            }
            DemoCsdLinkType::Interimpediment => canvas::ArrowheadType::FullTriangle,
        }, None)
    }

    fn destination_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        (self.line_type(), canvas::ArrowheadType::None, None)
    }

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>
    ) {
        let model = self.model.read().unwrap();
        
        let mut link_type_buffer = model.link_type.clone();
        ui.label("Type:");
        egui::ComboBox::from_id_salt("Type:")
            .selected_text(link_type_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoCsdLinkType::Initiation,
                    DemoCsdLinkType::Interstriction,
                    DemoCsdLinkType::Interimpediment,
                ] {
                    if ui
                        .selectable_value(&mut link_type_buffer, value, value.char())
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoCsdPropChange::LinkTypeChange(link_type_buffer),
                        ]));
                    }
                }
            });

        let mut comment_buffer = (*model.comment).clone();
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoCsdPropChange::CommentChange(Arc::new(comment_buffer)),
            ]));
        }
    }

    fn apply_change(
        &self,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    DemoCsdPropChange::LinkTypeChange(link_type) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid).collect(),
                            vec![DemoCsdPropChange::LinkTypeChange(model.link_type)],
                        ));
                        model.link_type = *link_type;
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*model.uuid).collect(),
                            vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
                        ));
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn deep_copy_init(
        &self,
        uuid: uuid::Uuid,
        m: &mut HashMap<usize, (Arc<RwLock<dyn DemoCsdElement>>, Arc<dyn Any + Send + Sync>)>
    ) -> Self where Self: Sized {
        let model = self.model.read().unwrap();
        let model = Arc::new(RwLock::new(DemoCsdLink::new(uuid, model.link_type, model.source.clone(), model.target.clone())));
        m.insert(arc_to_usize(&self.model), (model.clone(), model.clone()));
        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<usize, (Arc<RwLock<dyn DemoCsdElement>>, Arc<dyn Any + Send + Sync>)>
    ) {
        let mut model = self.model.write().unwrap();
        
        if let Some((_, new_source)) = m.get(&arc_to_usize(&model.source)) {
            let new_source: Result<Arc<RwLock<DemoCsdTransactor>>, _> = Arc::downcast(new_source.clone());
            if let Ok(new_source) = new_source {
                model.source = new_source;
            }
        }
        if let Some((_, new_dest)) = m.get(&arc_to_usize(&model.target)) {
            let new_dest: Result<Arc<RwLock<DemoCsdTransaction>>, _> = Arc::downcast(new_dest.clone());
            if let Ok(new_dest) = new_dest {
                model.target = new_dest;
            }
        }
    }
}

fn democsd_link(
    link_type: DemoCsdLinkType,
    source: (
        Arc<std::sync::RwLock<DemoCsdTransactor>>,
        ArcRwLockControllerT,
    ),
    destination: (
        Arc<std::sync::RwLock<DemoCsdTransaction>>,
        ArcRwLockControllerT,
    ),
) -> (uuid::Uuid, Arc<RwLock<DemoCsdLink>>, Arc<RwLock<LinkViewT>>) {
    let predicate_uuid = uuid::Uuid::now_v7();
    let predicate = Arc::new(RwLock::new(DemoCsdLink::new(
        predicate_uuid.clone(),
        link_type,
        source.0,
        destination.0,
    )));
    let predicate_controller = MulticonnectionView::new(
        DemoCsdLinkAdapter {
            model: predicate.clone(),
        },
        source.1,
        destination.1,
        None,
        vec![vec![(uuid::Uuid::now_v7(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7(), egui::Pos2::ZERO)]],
    );
    (predicate_uuid, predicate, predicate_controller)
}
