use crate::common::canvas::{self, NHShape};
use crate::common::controller::{
    ColorLabels, ColorProfile, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, Model, ModelHierarchyView, MulticonnectionAdapter, MulticonnectionView, PackageAdapter, PackageView, ProjectCommand, Queryable, SelectionStatus, SensitiveCommand, SimpleModelHierarchyView, SnapManager, TargettingStatus, Tool, VertexInformation, View
};
use crate::common::project_serde::{get_model_uuid, NHDeserializeError, NHDeserializeScalar, NHDeserializer, NHSerialize, NHSerializeError, NHSerializeToScalar, NHSerializer};
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::democsd::democsd_models::{
    DemoCsdDiagram, DemoCsdElement, DemoCsdLink, DemoCsdLinkType, DemoCsdPackage,
    DemoCsdTransaction, DemoCsdTransactor,
};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock, Weak},
};

struct DemoCsdDomain;
impl Domain for DemoCsdDomain {
    type CommonElementT = DemoCsdElement;
    type CommonElementViewT = DemoCsdElementView;
    type QueryableT<'a> = DemoCsdQueryable<'a>;
    type ToolT = NaiveDemoCsdTool;
    type AddCommandElementT = DemoCsdElementOrVertex;
    type PropChangeT = DemoCsdPropChange;
}

type PackageViewT = PackageView<DemoCsdDomain, DemoCsdPackageAdapter>;
type LinkViewT = MulticonnectionView<DemoCsdDomain, DemoCsdLinkAdapter>;

pub struct DemoCsdQueryable<'a> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, DemoCsdElementView>,
}

impl<'a> Queryable<'a, DemoCsdDomain> for DemoCsdQueryable<'a> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, DemoCsdElementView>,
    ) -> Self {
        Self { models_to_views, flattened_views }
    }

    fn get_view(&self, m: &ModelUuid) -> Option<DemoCsdElementView> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).cloned()
    }
}

#[derive(Clone)]
pub enum DemoCsdPropChange {
    NameChange(Arc<String>),
    IdentifierChange(Arc<String>),
    TransactorSelfactivatingChange(bool),
    TransactorInternalChange(bool),

    LinkTypeChange(DemoCsdLinkType),

    CommentChange(Arc<String>),
}

impl Debug for DemoCsdPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoCsdPropChange::???")
    }
}

impl TryFrom<&DemoCsdPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &DemoCsdPropChange) -> Result<Self, Self::Error> {
        Err(())
    }
}

#[derive(Clone, derive_more::From)]
pub enum DemoCsdElementOrVertex {
    Element(DemoCsdElementView),
    Vertex(VertexInformation),
}

impl Debug for DemoCsdElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoCsdElementOrVertex::???")
    }
}

impl TryFrom<DemoCsdElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: DemoCsdElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            DemoCsdElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<DemoCsdElementOrVertex> for DemoCsdElementView {
    type Error = ();

    fn try_from(value: DemoCsdElementOrVertex) -> Result<Self, Self::Error> {
        match value {
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

#[derive(Clone, derive_more::From)]
pub enum DemoCsdElementView {
    Package(Arc<RwLock<PackageViewT>>),
    Transactor(Arc<RwLock<DemoCsdTransactorView>>),
    Transaction(Arc<RwLock<DemoCsdTransactionView>>),
    Link(Arc<RwLock<LinkViewT>>),
}

impl Debug for DemoCsdElementView {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoCsdElementView::???")
    }
}

impl NHSerialize for DemoCsdElementView {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            Self::Package(inner) => inner.read().unwrap().serialize_into(into),
            Self::Transactor(inner) => inner.read().unwrap().serialize_into(into),
            Self::Transaction(inner) => inner.read().unwrap().serialize_into(into),
            Self::Link(inner) => inner.read().unwrap().serialize_into(into),
        }
    }
}
// impl NHDeserialize for DemoCsdElementView {}
impl View for DemoCsdElementView {
    fn uuid(&self) -> Arc<ViewUuid> {
        match self {
            Self::Package(inner) => inner.read().unwrap().uuid(),
            Self::Transactor(inner) => inner.read().unwrap().uuid(),
            Self::Transaction(inner) => inner.read().unwrap().uuid(),
            Self::Link(inner) => inner.read().unwrap().uuid(),
        }
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        match self {
            Self::Package(inner) => inner.read().unwrap().model_uuid(),
            Self::Transactor(inner) => inner.read().unwrap().model_uuid(),
            Self::Transaction(inner) => inner.read().unwrap().model_uuid(),
            Self::Link(inner) => inner.read().unwrap().model_uuid(),
        }
    }
    fn model_name(&self) -> Arc<String> {
        match self {
            Self::Package(inner) => inner.read().unwrap().model_name(),
            Self::Transactor(inner) => inner.read().unwrap().model_name(),
            Self::Transaction(inner) => inner.read().unwrap().model_name(),
            Self::Link(inner) => inner.read().unwrap().model_name(),
        }
    }
}
impl ElementController<DemoCsdElement> for DemoCsdElementView {
    fn model(&self) -> DemoCsdElement {
        match self {
            Self::Package(inner) => inner.read().unwrap().model(),
            Self::Transactor(inner) => inner.read().unwrap().model(),
            Self::Transaction(inner) => inner.read().unwrap().model(),
            Self::Link(inner) => inner.read().unwrap().model(),
        }
    }
    fn min_shape(&self) -> NHShape {
        match self {
            Self::Package(inner) => inner.read().unwrap().min_shape(),
            Self::Transactor(inner) => inner.read().unwrap().min_shape(),
            Self::Transaction(inner) => inner.read().unwrap().min_shape(),
            Self::Link(inner) => inner.read().unwrap().min_shape(),
        }
    }
    fn max_shape(&self) -> NHShape {
        match self {
            Self::Package(inner) => inner.read().unwrap().max_shape(),
            Self::Transactor(inner) => inner.read().unwrap().max_shape(),
            Self::Transaction(inner) => inner.read().unwrap().max_shape(),
            Self::Link(inner) => inner.read().unwrap().max_shape(),
        }
    }
    fn position(&self) -> egui::Pos2 {
        match self {
            Self::Package(inner) => inner.read().unwrap().position(),
            Self::Transactor(inner) => inner.read().unwrap().position(),
            Self::Transaction(inner) => inner.read().unwrap().position(),
            Self::Link(inner) => inner.read().unwrap().position(),
        }
    }
}
impl ContainerGen2<DemoCsdDomain> for DemoCsdElementView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<<DemoCsdDomain as Domain>::CommonElementViewT> {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            DemoCsdElementView::Link(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
        }
    }
}
impl ElementControllerGen2<DemoCsdDomain> for DemoCsdElementView {
    fn show_properties(
        &mut self,
        q: &DemoCsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> bool {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
        }
    }
    fn draw_in(
        &mut self,
        q: &DemoCsdQueryable,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoCsdTool>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
        }
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
        }
    }
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoCsdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
            DemoCsdElementView::Link(rw_lock) => rw_lock.read().unwrap().delete_when(deleting),
        }
    }
    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoCsdElementView>,
        c: &mut HashMap<ViewUuid, DemoCsdElementView>,
        m: &mut HashMap<ModelUuid, DemoCsdElement>,
    ) {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            DemoCsdElementView::Link(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoCsdElementView>,
        c: &mut HashMap<ViewUuid, DemoCsdElementView>,
        m: &mut HashMap<ModelUuid, DemoCsdElement>,
    ) {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            DemoCsdElementView::Link(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, DemoCsdElementView>,
        m: &HashMap<ModelUuid, DemoCsdElement>,
    ) {
        match self {
            DemoCsdElementView::Package(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            DemoCsdElementView::Transactor(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            DemoCsdElementView::Transaction(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            DemoCsdElementView::Link(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
        }
    }
}

#[derive(Clone)]
pub struct DemoCsdDiagramAdapter {
    model: Arc<RwLock<DemoCsdDiagram>>,
    name_buffer: String,
    comment_buffer: String,
}

impl DiagramAdapter<DemoCsdDomain, DemoCsdDiagram> for DemoCsdDiagramAdapter {
    fn model(&self) -> Arc<RwLock<DemoCsdDiagram>> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name()
    }
    fn view_type(&self) -> &'static str {
        "democsd-diagram-view"
    }

    fn create_new_view_for(
        &self,
        q: &DemoCsdQueryable<'_>,
        element: DemoCsdElement,
    ) -> DemoCsdElementOrVertex {
        let v = match element {
            DemoCsdElement::DemoCsdPackage(rw_lock) => {
                DemoCsdElementView::from(
                    new_democsd_package_view(
                        rw_lock,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            },
            DemoCsdElement::DemoCsdTransactor(rw_lock) => {
                let m = rw_lock.read().unwrap();
                let tx_view = m.transaction.as_ref().map(|e| new_democsd_transaction_view(e.clone(), egui::Pos2::ZERO, true));
                DemoCsdElementView::from(
                    new_democsd_transactor_view(rw_lock.clone(), tx_view, egui::Pos2::ZERO)
                )
            },
            DemoCsdElement::DemoCsdTransaction(rw_lock) => {
                DemoCsdElementView::from(
                    new_democsd_transaction_view(rw_lock, egui::Pos2::ZERO, false)
                )
            },
            DemoCsdElement::DemoCsdLink(rw_lock) => {
                let m = rw_lock.read().unwrap();
                let source_view = q.get_view(&m.source.read().unwrap().uuid).unwrap();
                let target_view = q.get_view(&m.target.read().unwrap().uuid).unwrap();
                DemoCsdElementView::from(
                    new_democsd_link_view(
                        rw_lock.clone(),
                        source_view,
                        target_view,
                    )
                )
            },
        };
        DemoCsdElementOrVertex::from(v)
    }

    fn show_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![DemoCsdPropChange::NameChange(Arc::new(self.name_buffer.clone()))],
                )
                .into(),
            );
        };

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![DemoCsdPropChange::CommentChange(Arc::new(
                        self.comment_buffer.clone(),
                    ))],
                )
                .into(),
            );
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    DemoCsdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoCsdPropChange::NameChange(model.name.clone())],
                        ));
                        self.name_buffer = (**name).clone();
                        model.name = name.clone();
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
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

    fn tool_change_fun(&self, tool: &mut Option<NaiveDemoCsdTool>, ui: &mut egui::Ui) {
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

    fn menubar_options_fun(&self, _ui: &mut egui::Ui, _commands: &mut Vec<ProjectCommand>) {}

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DemoCsdElement>) {
        let (new_model, models) = super::democsd_models::deep_copy_diagram(&self.model.read().unwrap());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, DemoCsdElement>) {
        let models = super::democsd_models::fake_copy_diagram(&self.model.read().unwrap());
        (self.clone(), models)
    }
}

impl NHSerializeToScalar for DemoCsdDiagramAdapter {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<toml::Value, NHSerializeError> {
        self.model.read().unwrap().serialize_into(into)?;

        Ok(toml::Value::String(self.model.read().unwrap().uuid().to_string()))
    }
}

impl NHDeserializeScalar for DemoCsdDiagramAdapter {
    fn deserialize(
        source: &toml::Value,
        deserializer: &NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let toml::Value::String(s) = source else {
            return Err(NHDeserializeError::StructureError(format!("expected string, got {:?}", source)));
        };
        let uuid = uuid::Uuid::parse_str(s)?.into();
        let model = deserializer.get_or_instantiate_model::<DemoCsdDiagram>(&uuid)?;
        let name_buffer = (*model.read().unwrap().name).clone();
        let comment_buffer = (*model.read().unwrap().comment).clone();
        Ok(Self { model, name_buffer, comment_buffer })
    }
}

pub fn new(no: u32) -> (Arc<RwLock<dyn DiagramController>>, Arc<dyn ModelHierarchyView>) {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New DEMO CSD diagram {}", no);

    let diagram = Arc::new(RwLock::new(DemoCsdDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    )));
    (
        DiagramControllerGen2::new(
            Arc::new(view_uuid),
            DemoCsdDiagramAdapter {
                model: diagram.clone(),
                name_buffer: name,
                comment_buffer: "".to_owned(),
            },
            Vec::new(),
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram)),
    )
}

pub fn demo(no: u32) -> (Arc<RwLock<dyn DiagramController>>, Arc<dyn ModelHierarchyView>) {
    let mut models: Vec<DemoCsdElement> = vec![];
    let mut controllers = Vec::<DemoCsdElementView>::new();

    {
        let (client, client_view) = new_democsd_transactor(
            "CTAR01",
            "Client",
            false,
            None,
            false,
            egui::Pos2::new(200.0, 200.0),
        );

        models.push(client.into());
        controllers.push(client_view.into());
    }

    {
        let (tx_model, tx_view) = new_democsd_transaction(
            "TK01",
            "Sale completion",
            egui::Pos2::new(200.0, 400.0),
            true,
        );

        let (ta, ta_view) = new_democsd_transactor(
            "AR01",
            "Sale completer",
            true,
            Some((tx_model, tx_view)),
            false,
            egui::Pos2::new(200.0, 400.0),
        );
        models.push(ta.into());
        controllers.push(ta_view.into());
    }

    {
        let (tx, tx_view) = new_democsd_transaction(
            "TK10",
            "Sale transportation",
            egui::Pos2::new(200.0, 600.0),
            true,
        );

        let (ta_model, ta_view) = new_democsd_transactor(
            "AR02",
            "Sale transporter",
            true,
            Some((tx, tx_view)),
            false,
            egui::Pos2::new(200.0, 600.0),
        );
        models.push(ta_model.into());
        controllers.push(ta_view.into());
    }

    {
        let (tx_model, tx_view) = new_democsd_transaction(
            "TK11",
            "Sale controlling",
            egui::Pos2::new(400.0, 200.0),
            true,
        );

        let (ta_model, ta_view) = new_democsd_transactor(
            "AR03",
            "Sale controller",
            true,
            Some((tx_model, tx_view)),
            true,
            egui::Pos2::new(400.0, 200.0),
        );
        models.push(ta_model.into());
        controllers.push(ta_view.into());
    }

    // TK02 - Purchase completer

    {
        let view_uuid = uuid::Uuid::now_v7().into();
        let model_uuid = uuid::Uuid::now_v7().into();
        let name = format!("New DEMO CSD diagram {}", no);
        let diagram = Arc::new(RwLock::new(DemoCsdDiagram::new(
            model_uuid,
            name.clone(),
            models,
        )));
        (
            DiagramControllerGen2::new(
                Arc::new(view_uuid),
                DemoCsdDiagramAdapter {
                    model: diagram.clone(),
                    name_buffer: name,
                    comment_buffer: "".to_owned(),
                },
                controllers,
            ),
            Arc::new(SimpleModelHierarchyView::new(diagram)),
        )
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
    Some(DemoCsdElementView),
    Link {
        link_type: DemoCsdLinkType,
        source: Arc<RwLock<DemoCsdTransactor>>,
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

impl Tool<DemoCsdDomain> for NaiveDemoCsdTool {
    type Stage = DemoCsdToolStage;

    fn initial_stage(&self) -> DemoCsdToolStage {
        self.initial_stage
    }

    fn targetting_for_element(&self, element: Option<DemoCsdElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
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
            Some(DemoCsdElement::DemoCsdPackage(..)) => match self.current_stage {
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
            Some(DemoCsdElement::DemoCsdTransactor(..)) => match self.current_stage {
                DemoCsdToolStage::LinkStart { .. } => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkEnd
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            Some(DemoCsdElement::DemoCsdTransaction(..)) => match self.current_stage {
                DemoCsdToolStage::LinkEnd => TARGETTABLE_COLOR,
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkStart { .. }
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd
                | DemoCsdToolStage::Note => NON_TARGETTABLE_COLOR,
            },
            Some(DemoCsdElement::DemoCsdLink(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &DemoCsdQueryable, canvas: &mut dyn canvas::NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialDemoCsdElement::Link {
                source,
                link_type,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.read().unwrap().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
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
            }
            PartialDemoCsdElement::Package { a, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a, pos),
                    egui::CornerRadius::ZERO,
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
                let (_client_model, client_view) =
                    new_democsd_transactor("CTAR01", "Client", false, None, false, pos);
                self.result = PartialDemoCsdElement::Some(client_view.into());
                self.event_lock = true;
            }
            (DemoCsdToolStage::Transactor, _) => {
                let (tx_model, tx_view) =
                    new_democsd_transaction("TK01", "Transaction", pos, true);
                let (_ta_model, ta_view) = new_democsd_transactor(
                    "AR01",
                    "Transactor",
                    true,
                    Some((tx_model, tx_view)),
                    false,
                    pos,
                );
                self.result = PartialDemoCsdElement::Some(ta_view.into());
                self.event_lock = true;
            }
            (DemoCsdToolStage::Bank, _) => {
                let (_bank_model, transaction_view) =
                    new_democsd_transaction("TK01", "Bank", pos, false);
                self.result = PartialDemoCsdElement::Some(transaction_view.into());
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageStart, _) => {
                self.result = PartialDemoCsdElement::Package { a: pos, b: None };
                self.current_stage = DemoCsdToolStage::PackageEnd;
                self.event_lock = true;
            }
            (DemoCsdToolStage::PackageEnd, PartialDemoCsdElement::Package { b, .. }) => {
                *b = Some(pos)
            }
            (DemoCsdToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: DemoCsdElement) {
        if self.event_lock {
            return;
        }

        match controller {
            DemoCsdElement::DemoCsdPackage(..) => {}
            DemoCsdElement::DemoCsdTransactor(inner) => {
                match (self.current_stage, &mut self.result) {
                    (DemoCsdToolStage::LinkStart { link_type }, PartialDemoCsdElement::None) => {
                        self.result = PartialDemoCsdElement::Link {
                            link_type,
                            source: inner,
                            dest: None,
                        };
                        self.current_stage = DemoCsdToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            DemoCsdElement::DemoCsdTransaction(inner) => match (self.current_stage, &mut self.result) {
                (DemoCsdToolStage::LinkEnd, PartialDemoCsdElement::Link { dest, .. }) => {
                    *dest = Some(inner);
                    self.event_lock = true;
                }
                _ => {}
            },
            DemoCsdElement::DemoCsdLink(..) => {}
        }
    }

    fn try_construct(&mut self, into: &dyn ContainerGen2<DemoCsdDomain>) -> Option<DemoCsdElementView> {
        match &self.result {
            PartialDemoCsdElement::Some(x) => {
                let x = x.clone();
                self.result = PartialDemoCsdElement::None;
                Some(x)
            }
            // TODO: check for source == dest case, set points?
            PartialDemoCsdElement::Link {
                source,
                dest: Some(target),
                link_type,
                ..
            } => {
                self.current_stage = self.initial_stage;

                let link_view: Option<DemoCsdElementView> =
                    if let (Some(source_view), Some(target_view)) = (
                        into.controller_for(&source.read().unwrap().uuid()),
                        into.controller_for(&target.read().unwrap().uuid()),
                    ) {
                        let (_link_model, link_view) = new_democsd_link(
                            *link_type,
                            (source.clone(), source_view),
                            (target.clone(), target_view),
                        );

                        Some(link_view.into())
                    } else {
                        None
                    };

                self.result = PartialDemoCsdElement::None;
                link_view
            }
            PartialDemoCsdElement::Package { a, b: Some(b) } => {
                self.current_stage = DemoCsdToolStage::PackageStart;

                let (_package_model, package_view) =
                    new_democsd_package("A package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialDemoCsdElement::None;
                Some(package_view.into())
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

#[derive(Clone)]
pub struct DemoCsdPackageAdapter {
    model: Arc<RwLock<DemoCsdPackage>>,
}

impl PackageAdapter<DemoCsdDomain> for DemoCsdPackageAdapter {
    fn model(&self) -> DemoCsdElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }

    fn view_type(&self) -> &'static str {
        "democsd-package-view"
    }

    fn add_element(&mut self, e: DemoCsdElement) {
        self.model.write().unwrap().add_element(e);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
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
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    DemoCsdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoCsdPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
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
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoCsdElement>,
    ) -> Self where Self: Sized {
        let model_uuid = *self.model.read().unwrap().uuid;
        let model = if let Some(DemoCsdElement::DemoCsdPackage(m)) = m.get(&model_uuid) {
            m.clone()
        } else {
            let model = self.model.read().unwrap();
            let model = Arc::new(RwLock::new(DemoCsdPackage::new(new_uuid, (*model.name).clone(), model.contained_elements.clone())));
            m.insert(model_uuid, model.clone().into());
            model
        };
        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DemoCsdElement>,
    ) {
        todo!()
    }
}

fn new_democsd_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (
    Arc<RwLock<DemoCsdPackage>>,
    Arc<RwLock<PackageViewT>>,
) {
    let model_uuid = uuid::Uuid::now_v7().into();
    let graph_model = Arc::new(RwLock::new(DemoCsdPackage::new(
        model_uuid,
        name.to_owned(),
        vec![],
    )));
    let graph_view = new_democsd_package_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_democsd_package_view(
    model: Arc<RwLock<DemoCsdPackage>>,
    bounds_rect: egui::Rect,
) -> Arc<RwLock<PackageViewT>> {
    let view_uuid = uuid::Uuid::now_v7().into();
    PackageViewT::new(
        Arc::new(view_uuid),
        DemoCsdPackageAdapter {
            model,
        },
        HashMap::new(),
        bounds_rect,
    )
}

// ---

fn new_democsd_transactor(
    identifier: &str,
    name: &str,
    internal: bool,
    transaction: Option<(
        Arc<RwLock<DemoCsdTransaction>>,
        Arc<RwLock<DemoCsdTransactionView>>,
    )>,
    transaction_selfactivating: bool,
    position: egui::Pos2,
) -> (
    Arc<RwLock<DemoCsdTransactor>>,
    Arc<RwLock<DemoCsdTransactorView>>,
) {
    let ta_model_uuid = uuid::Uuid::now_v7().into();
    let ta_model = Arc::new(RwLock::new(DemoCsdTransactor::new(
        ta_model_uuid,
        identifier.to_owned(),
        name.to_owned(),
        internal,
        transaction.as_ref().map(|t| t.0.clone()),
        transaction_selfactivating,
    )));
    let ta_view = new_democsd_transactor_view(
        ta_model.clone(),
        transaction.as_ref().map(|t| t.1.clone()),
        position,
    );

    (ta_model, ta_view)
}
fn new_democsd_transactor_view(
    model: Arc<RwLock<DemoCsdTransactor>>,
    transaction: Option<Arc<RwLock<DemoCsdTransactionView>>>,
    position: egui::Pos2,
) -> Arc<RwLock<DemoCsdTransactorView>> {
    let m = model.read().unwrap();
    let ta_view_uuid = uuid::Uuid::now_v7().into();
    let ta_view = Arc::new(RwLock::new(DemoCsdTransactorView {
        uuid: Arc::new(ta_view_uuid),
        model: model.clone(),
        self_reference: Weak::new(),
        transaction_view: transaction,

        identifier_buffer: (*m.identifier).clone(),
        name_buffer: (*m.name).clone(),
        internal_buffer: m.internal,
        transaction_selfactivating_buffer: m.transaction_selfactivating,
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::ZERO,
    }));
    ta_view.write().unwrap().self_reference = Arc::downgrade(&ta_view);
    ta_view
}

pub struct DemoCsdTransactorView {
    uuid: Arc<ViewUuid>,
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

impl View for DemoCsdTransactorView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
}

impl NHSerialize for DemoCsdTransactorView {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        // Serialize itself
        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("democsd-transactor-view".to_owned()));
        element.insert("position".to_owned(), toml::Value::Array(vec![toml::Value::Float(self.position.x as f64), toml::Value::Float(self.position.y as f64)]));
        element.insert("children".to_owned(), toml::Value::Array(self.transaction_view.iter().map(|e| toml::Value::String(e.read().unwrap().model_uuid().to_string())).collect()));
        into.insert_view(*self.uuid, element);

        // Serialize child
        for e in self.transaction_view.iter() {
            e.read().unwrap().serialize_into(into)?;
        }

        Ok(())
    }
}

impl ElementController<DemoCsdElement> for DemoCsdTransactorView {
    fn model(&self) -> DemoCsdElement {
        self.model.clone().into()
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

impl ContainerGen2<DemoCsdDomain> for DemoCsdTransactorView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<DemoCsdElementView> {
        match &self.transaction_view {
            Some(t) if *uuid == *t.read().unwrap().model_uuid() => {
                Some(t.clone().into())
            }
            _ => None,
        }
    }
}

impl ElementControllerGen2<DemoCsdDomain> for DemoCsdTransactorView {
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

        ui.label("Model properties");

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

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position;

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.position.x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.position.y)));
            }
        });

        true
    }
    fn draw_in(
        &mut self,
        queryable: &DemoCsdQueryable,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
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
            egui::CornerRadius::ZERO,
            if read.internal {
                context.profile.backgrounds[4]
            } else {
                context.profile.backgrounds[3]
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
            context.profile.foregrounds[4]
        } else {
            context.profile.foregrounds[3]
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
            let res = t.draw_in(queryable, context, canvas, &tool);
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
                egui::CornerRadius::ZERO,
                t.targetting_for_element(Some(self.model())),
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
                    tool.add_element(self.model());
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
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        match property {
                            DemoCsdPropChange::IdentifierChange(identifier) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
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
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::TransactorSelfactivatingChange(value) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
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
                                    std::iter::once(*self.uuid).collect(),
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
                                    std::iter::once(*self.uuid).collect(),
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

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoCsdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        if let Some(tx) = &self.transaction_view {
            let mut views_tx = HashMap::new();
            let mut tx_l = tx.write().unwrap();
            tx_l.head_count(flattened_views, &mut views_tx, flattened_represented_models);

            for e in views_tx {
                flattened_views_status.insert(e.0, match e.1 {
                    SelectionStatus::NotSelected if self.highlight.selected => SelectionStatus::TransitivelySelected,
                    e => e,
                });
            }

            flattened_views.insert(*tx_l.uuid(), tx.clone().into());
        }
    }
    
    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoCsdElementView>,
        c: &mut HashMap<ViewUuid, DemoCsdElementView>,
        m: &mut HashMap<ModelUuid, DemoCsdElement>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid()) || self.transaction_view.as_ref().is_some_and(|t| e.contains(&t.read().unwrap().uuid()))) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoCsdElementView>,
        c: &mut HashMap<ViewUuid, DemoCsdElementView>,
        m: &mut HashMap<ModelUuid, DemoCsdElement>,
    ) {
        let tx_clone = if let Some(t) = self.transaction_view.as_ref() {
            let mut inner = HashMap::new();
            t.read().unwrap().deep_copy_clone(uuid_present, &mut inner, c, m);
            if let Some(DemoCsdElementView::Transaction(t)) = c.get(&t.read().unwrap().uuid()) {
                Some(t.clone())
            } else { None }
        } else { None };

        let old_model = self.model.read().unwrap();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoCsdElement::DemoCsdTransactor(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(DemoCsdTransactor::new(model_uuid, (*old_model.identifier).clone(), (*old_model.name).clone(), old_model.internal,
            old_model.transaction.clone(), old_model.transaction_selfactivating)));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };
        
        let cloneish = Arc::new(RwLock::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            self_reference: Weak::new(),
            transaction_view: tx_clone,
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
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, DemoCsdElementView>,
        m: &HashMap<ModelUuid, DemoCsdElement>,
    ) {
        if let Some(DemoCsdElementView::Transaction(new_ta)) = self.transaction_view.as_ref().and_then(|e| c.get(&e.read().unwrap().uuid()))  {
            self.transaction_view = Some(new_ta.clone());
        }
    }
}

fn new_democsd_transaction(
    identifier: &str,
    name: &str,
    position: egui::Pos2,
    actor: bool,
) -> (
    Arc<RwLock<DemoCsdTransaction>>,
    Arc<RwLock<DemoCsdTransactionView>>,
) {
    let tx_model_uuid = uuid::Uuid::now_v7().into();
    let tx_model = Arc::new(RwLock::new(DemoCsdTransaction::new(
        tx_model_uuid,
        identifier.to_owned(),
        name.to_owned(),
    )));
    let tx_view = new_democsd_transaction_view(tx_model.clone(), position, actor);
    (tx_model, tx_view)
}
fn new_democsd_transaction_view(
    model: Arc<RwLock<DemoCsdTransaction>>,
    position: egui::Pos2,
    actor: bool,
) -> Arc<RwLock<DemoCsdTransactionView>> {
    let m = model.read().unwrap();
    let tx_view_uuid = uuid::Uuid::now_v7().into();
    let tx_view = Arc::new(RwLock::new(DemoCsdTransactionView {
        uuid: Arc::new(tx_view_uuid),
        model: model.clone(),
        self_reference: Weak::new(),

        identifier_buffer: (*m.identifier).clone(),
        name_buffer: (*m.name).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

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
    tx_view.write().unwrap().self_reference = Arc::downgrade(&tx_view);
    tx_view
}

pub struct DemoCsdTransactionView {
    uuid: Arc<ViewUuid>,
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

impl View for DemoCsdTransactionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
}

impl NHSerialize for DemoCsdTransactionView {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("democsd-transaction-view".to_owned()));
        element.insert("position".to_owned(), toml::Value::Array(vec![toml::Value::Float(self.position.x as f64), toml::Value::Float(self.position.y as f64)]));
        into.insert_view(*self.uuid, element);

        Ok(())
    }
}

impl ElementController<DemoCsdElement> for DemoCsdTransactionView {
    fn model(&self) -> DemoCsdElement {
        self.model.clone().into()
    }
    fn min_shape(&self) -> canvas::NHShape {
        self.min_shape
    }
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ContainerGen2<DemoCsdDomain> for DemoCsdTransactionView {}

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

impl ElementControllerGen2<DemoCsdDomain> for DemoCsdTransactionView {
    fn show_properties(
        &mut self,
        _parent: &DemoCsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Model properties");

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

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position;

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.position.x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.position.y)));
            }
        });

        true
    }
    fn draw_in(
        &mut self,
        _: &DemoCsdQueryable,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
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
            context.profile.backgrounds[5],
            context.profile.foregrounds[5],
            context.profile.foregrounds[6],
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
                t.targetting_for_element(Some(self.model())),
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
                    tool.add_element(self.model());
                } else {
                    if !ehc.modifiers.command {
                        commands.push(InsensitiveCommand::SelectAll(false).into());
                        commands.push(
                            InsensitiveCommand::SelectSpecific(
                                std::iter::once(*self.uuid).collect(),
                                true,
                            )
                            .into(),
                        );
                    } else {
                        commands.push(
                            InsensitiveCommand::SelectSpecific(
                                std::iter::once(*self.uuid).collect(),
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
                            std::iter::once(*self.uuid).collect(),
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
                if uuids.contains(&*self.uuid) {
                    self.highlight.selected = *select;
                }
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _)
                if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddElement(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    for property in properties {
                        match property {
                            DemoCsdPropChange::IdentifierChange(identifier) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
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
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
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

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoCsdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }
    
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoCsdElementView>,
        c: &mut HashMap<ViewUuid, DemoCsdElementView>,
        m: &mut HashMap<ModelUuid, DemoCsdElement>
    ) {
        let old_model = self.model.read().unwrap();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoCsdElement::DemoCsdTransaction(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(DemoCsdTransaction::new(model_uuid, (*old_model.identifier).clone(), (*old_model.name).clone())));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = Arc::new(RwLock::new(Self {
            uuid: view_uuid.into(),
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
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
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

impl MulticonnectionAdapter<DemoCsdDomain> for DemoCsdLinkAdapter {
    fn model(&self) -> DemoCsdElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        Arc::new("TODO".to_owned())
    }

    fn view_type(&self) -> &'static str {
        "democsd-link-view"
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
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    DemoCsdPropChange::LinkTypeChange(link_type) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoCsdPropChange::LinkTypeChange(model.link_type)],
                        ));
                        model.link_type = *link_type;
                    }
                    DemoCsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
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
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoCsdElement>
    ) -> Self where Self: Sized {
        let model_uuid = *self.model.read().unwrap().uuid;
        let model = if let Some(DemoCsdElement::DemoCsdLink(m)) = m.get(&model_uuid) {
            m.clone()
        } else {
            let model = self.model.read().unwrap();
            let model = Arc::new(RwLock::new(DemoCsdLink::new(new_uuid, model.link_type, model.source.clone(), model.target.clone())));
            m.insert(model_uuid, model.clone().into());
            model
        };
        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DemoCsdElement>
    ) {
        let mut model = self.model.write().unwrap();
        
        let source_uuid = *model.source.read().unwrap().uuid;
        if let Some(DemoCsdElement::DemoCsdTransactor(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }

        let target_uuid = *model.target.read().unwrap().uuid;
        if let Some(DemoCsdElement::DemoCsdTransaction(new_target)) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}

fn new_democsd_link(
    link_type: DemoCsdLinkType,
    source: (
        Arc<RwLock<DemoCsdTransactor>>,
        DemoCsdElementView,
    ),
    target: (
        Arc<RwLock<DemoCsdTransaction>>,
        DemoCsdElementView,
    ),
) -> (Arc<RwLock<DemoCsdLink>>, Arc<RwLock<LinkViewT>>) {
    let link_model_uuid = uuid::Uuid::now_v7().into();
    let link_model = Arc::new(RwLock::new(DemoCsdLink::new(
        link_model_uuid,
        link_type,
        source.0,
        target.0,
    )));
    let link_view = new_democsd_link_view(link_model.clone(), source.1, target.1);
    (link_model, link_view)
}
fn new_democsd_link_view(
    model: Arc<RwLock<DemoCsdLink>>,
    source: DemoCsdElementView,
    target: DemoCsdElementView,
) -> Arc<RwLock<LinkViewT>> {
    let link_view_uuid = uuid::Uuid::now_v7().into();
    MulticonnectionView::new(
        Arc::new(link_view_uuid),
        DemoCsdLinkAdapter {
            model,
        },
        source,
        target,
        None,
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
    )
}
