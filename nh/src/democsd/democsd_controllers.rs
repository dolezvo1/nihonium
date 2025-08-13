use crate::common::canvas::{self, Highlight, NHShape};
use crate::common::controller::{
    ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, InputEvent, InsensitiveCommand, Model, ModelsLabelAcquirer, ProjectCommand, Queryable, SelectionStatus, SensitiveCommand, SimpleModelHierarchyView, SnapManager, TargettingStatus, Tool, View
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::ufoption::UFOption;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::democsd::democsd_models::{
    DemoCsdDiagram, DemoCsdElement, DemoCsdLink, DemoCsdLinkType, DemoCsdPackage, DemoCsdTransaction, DemoCsdTransactionKind, DemoCsdTransactor
};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::{ElementSetupModal, SetupModalResult};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc},
};

pub struct DemoCsdDomain;
impl Domain for DemoCsdDomain {
    type CommonElementT = DemoCsdElement;
    type DiagramModelT = DemoCsdDiagram;
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
    TransactionKindChange(DemoCsdTransactionKind),

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


pub const EXTERNAL_ROLE_BACKGROUND: egui::Color32 = egui::Color32::LIGHT_GRAY;
pub const INTERNAL_ROLE_BACKGROUND: egui::Color32 = egui::Color32::WHITE;

pub const PERFORMA_DETAIL: egui::Color32 = egui::Color32::RED;
pub const INFORMA_DETAIL: egui::Color32 = egui::Color32::from_rgb(0, 175, 0);
pub const FORMA_DETAIL: egui::Color32 = egui::Color32::BLUE;


#[derive(Clone, derive_more::From, nh_derive::NHContextSerDeTag)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum DemoCsdElementView {
    Package(ERef<PackageViewT>),
    Transactor(ERef<DemoCsdTransactorView>),
    Transaction(ERef<DemoCsdTransactionView>),
    Link(ERef<LinkViewT>),
}

impl Debug for DemoCsdElementView {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoCsdElementView::???")
    }
}

impl Entity for DemoCsdElementView {
    fn tagged_uuid(&self) -> EntityUuid {
        match self {
            Self::Package(inner) => inner.read().tagged_uuid(),
            Self::Transactor(inner) => inner.read().tagged_uuid(),
            Self::Transaction(inner) => inner.read().tagged_uuid(),
            Self::Link(inner) => inner.read().tagged_uuid(),
        }
    }
}

impl View for DemoCsdElementView {
    fn uuid(&self) -> Arc<ViewUuid> {
        match self {
            Self::Package(inner) => inner.read().uuid(),
            Self::Transactor(inner) => inner.read().uuid(),
            Self::Transaction(inner) => inner.read().uuid(),
            Self::Link(inner) => inner.read().uuid(),
        }
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        match self {
            Self::Package(inner) => inner.read().model_uuid(),
            Self::Transactor(inner) => inner.read().model_uuid(),
            Self::Transaction(inner) => inner.read().model_uuid(),
            Self::Link(inner) => inner.read().model_uuid(),
        }
    }
}
impl ElementController<DemoCsdElement> for DemoCsdElementView {
    fn model(&self) -> DemoCsdElement {
        match self {
            Self::Package(inner) => inner.read().model(),
            Self::Transactor(inner) => inner.read().model(),
            Self::Transaction(inner) => inner.read().model(),
            Self::Link(inner) => inner.read().model(),
        }
    }
    fn min_shape(&self) -> NHShape {
        match self {
            Self::Package(inner) => inner.read().min_shape(),
            Self::Transactor(inner) => inner.read().min_shape(),
            Self::Transaction(inner) => inner.read().min_shape(),
            Self::Link(inner) => inner.read().min_shape(),
        }
    }
    fn max_shape(&self) -> NHShape {
        match self {
            Self::Package(inner) => inner.read().max_shape(),
            Self::Transactor(inner) => inner.read().max_shape(),
            Self::Transaction(inner) => inner.read().max_shape(),
            Self::Link(inner) => inner.read().max_shape(),
        }
    }
    fn position(&self) -> egui::Pos2 {
        match self {
            Self::Package(inner) => inner.read().position(),
            Self::Transactor(inner) => inner.read().position(),
            Self::Transaction(inner) => inner.read().position(),
            Self::Link(inner) => inner.read().position(),
        }
    }
}
impl ContainerGen2<DemoCsdDomain> for DemoCsdElementView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<<DemoCsdDomain as Domain>::CommonElementViewT> {
        match self {
            DemoCsdElementView::Package(inner) => inner.read().controller_for(uuid),
            DemoCsdElementView::Transactor(inner) => inner.read().controller_for(uuid),
            DemoCsdElementView::Transaction(inner) => inner.read().controller_for(uuid),
            DemoCsdElementView::Link(inner) => inner.read().controller_for(uuid),
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
            DemoCsdElementView::Package(inner) => inner.write().show_properties(q, ui, commands),
            DemoCsdElementView::Transactor(inner) => inner.write().show_properties(q, ui, commands),
            DemoCsdElementView::Transaction(inner) => inner.write().show_properties(q, ui, commands),
            DemoCsdElementView::Link(inner) => inner.write().show_properties(q, ui, commands),
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
            DemoCsdElementView::Package(inner) => inner.write().draw_in(q, context, canvas, tool),
            DemoCsdElementView::Transactor(inner) => inner.write().draw_in(q, context, canvas, tool),
            DemoCsdElementView::Transaction(inner) => inner.write().draw_in(q, context, canvas, tool),
            DemoCsdElementView::Link(inner) => inner.write().draw_in(q, context, canvas, tool),
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        match self {
            DemoCsdElementView::Package(inner) => inner.write().collect_allignment(am),
            DemoCsdElementView::Transactor(inner) => inner.write().collect_allignment(am),
            DemoCsdElementView::Transaction(inner) => inner.write().collect_allignment(am),
            DemoCsdElementView::Link(inner) => inner.write().collect_allignment(am),
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoCsdTool>,
        element_setup_modal: &mut Option<Box<dyn ElementSetupModal>>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        match self {
            DemoCsdElementView::Package(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
            DemoCsdElementView::Transactor(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
            DemoCsdElementView::Transaction(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
            DemoCsdElementView::Link(inner) => inner.write().handle_event(event, ehc, tool, element_setup_modal, commands),
        }
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match self {
            DemoCsdElementView::Package(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            DemoCsdElementView::Transactor(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            DemoCsdElementView::Transaction(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            DemoCsdElementView::Link(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
        }
    }
    fn refresh_buffers(&mut self) {
        match self {
            DemoCsdElementView::Package(inner) => inner.write().refresh_buffers(),
            DemoCsdElementView::Transactor(inner) => inner.write().refresh_buffers(),
            DemoCsdElementView::Transaction(inner) => inner.write().refresh_buffers(),
            DemoCsdElementView::Link(inner) => inner.write().refresh_buffers(),
        }
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoCsdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        match self {
            DemoCsdElementView::Package(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            DemoCsdElementView::Transactor(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            DemoCsdElementView::Transaction(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            DemoCsdElementView::Link(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        match self {
            DemoCsdElementView::Package(inner) => inner.read().delete_when(deleting),
            DemoCsdElementView::Transactor(inner) => inner.read().delete_when(deleting),
            DemoCsdElementView::Transaction(inner) => inner.read().delete_when(deleting),
            DemoCsdElementView::Link(inner) => inner.read().delete_when(deleting),
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
            DemoCsdElementView::Package(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            DemoCsdElementView::Transactor(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            DemoCsdElementView::Transaction(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            DemoCsdElementView::Link(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
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
            DemoCsdElementView::Package(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            DemoCsdElementView::Transactor(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            DemoCsdElementView::Transaction(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            DemoCsdElementView::Link(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, DemoCsdElementView>,
        m: &HashMap<ModelUuid, DemoCsdElement>,
    ) {
        match self {
            DemoCsdElementView::Package(inner) => inner.write().deep_copy_relink(c, m),
            DemoCsdElementView::Transactor(inner) => inner.write().deep_copy_relink(c, m),
            DemoCsdElementView::Transaction(inner) => inner.write().deep_copy_relink(c, m),
            DemoCsdElementView::Link(inner) => inner.write().deep_copy_relink(c, m),
        }
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoCsdDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoCsdDiagram>,
    background_color: egui::Color32,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: DemoCsdDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    placeholders: DemoCsdPlaceholderViews,
}

#[derive(Clone, Default)]
struct DemoCsdDiagramBuffer {
    name: String,
    comment: String,
}

#[derive(Clone)]
struct DemoCsdPlaceholderViews {
    views: [DemoCsdElementView; 7],
}

impl Default for DemoCsdPlaceholderViews {
    fn default() -> Self {
        let (client, client_view) = new_democsd_transactor("CTAR01", "Client", false, None, false, egui::Pos2::ZERO);
        let (_actor, actor_view) = {
            let tx = new_democsd_transaction("TK01", "Transaction", egui::Pos2::ZERO, true);
            new_democsd_transactor("AR01", "Transactor", true, Some(tx), false, egui::Pos2::ZERO)
        };
        let (bank, bank_view) = new_democsd_transaction("TK01", "Bank", egui::Pos2::new(100.0, 75.0), false);
        let bank = (bank, bank_view.into());

        let (_init, init_view) = new_democsd_link(DemoCsdLinkType::Initiation, (client.clone(), client_view.clone().into()), bank.clone());
        let (_ints, ints_view) = new_democsd_link(DemoCsdLinkType::Interstriction, (client.clone(), client_view.clone().into()), bank.clone());
        let (_inim, inim_view) = new_democsd_link(DemoCsdLinkType::Interimpediment, (client.clone(), client_view.clone().into()), bank.clone());

        let (_package, package_view) = new_democsd_package("A package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });

        Self {
            views: [
                client_view.into(),
                actor_view.into(),
                bank.1,
                init_view.into(),
                ints_view.into(),
                inim_view.into(),
                package_view.into(),
            ],
        }
    }
}

struct DemoCsdLabelAcquirer;
impl ModelsLabelAcquirer for DemoCsdLabelAcquirer {
    type ModelT = DemoCsdDiagram;

    fn model_label(&self, m: &Self::ModelT) -> String {
        format!("{} ({} children)", m.name, m.contained_elements.len())
    }

    fn element_label(&self, e: &<Self::ModelT as ContainerModel>::ElementT) -> String {
        match e {
            DemoCsdElement::DemoCsdPackage(inner) => (*inner.read().name).clone(),
            DemoCsdElement::DemoCsdTransactor(inner) => (*inner.read().name).clone(),
            DemoCsdElement::DemoCsdTransaction(inner) => (*inner.read().name).clone(),
            DemoCsdElement::DemoCsdLink(inner) => inner.read().link_type.char().to_owned(),
        }
    }
}

impl DemoCsdDiagramAdapter {
    fn new(model: ERef<DemoCsdDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: egui::Color32::WHITE,
            buffer: DemoCsdDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
            placeholders: Default::default(),
        }
    }
}

impl DiagramAdapter<DemoCsdDomain> for DemoCsdDiagramAdapter {
    fn model(&self) -> ERef<DemoCsdDiagram> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }
    fn view_type(&self) -> &'static str {
        "democsd-diagram-view"
    }
    fn new_hierarchy_view(&self) -> SimpleModelHierarchyView<impl ModelsLabelAcquirer<ModelT = DemoCsdDiagram> + 'static> {
        SimpleModelHierarchyView::new(self.model(), DemoCsdLabelAcquirer {})
    }

    fn create_new_view_for(
        &self,
        q: &DemoCsdQueryable<'_>,
        element: DemoCsdElement,
    ) -> Result<DemoCsdElementView, HashSet<ModelUuid>> {
        let v = match element {
            DemoCsdElement::DemoCsdPackage(inner) => {
                DemoCsdElementView::from(
                    new_democsd_package_view(
                        inner,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            },
            DemoCsdElement::DemoCsdTransactor(inner) => {
                let m = inner.read();
                let tx_view = m.transaction.as_ref().map(|e| new_democsd_transaction_view(e.clone(), egui::Pos2::ZERO, true));
                DemoCsdElementView::from(
                    new_democsd_transactor_view(inner.clone(), tx_view, egui::Pos2::ZERO)
                )
            },
            DemoCsdElement::DemoCsdTransaction(inner) => {
                DemoCsdElementView::from(
                    new_democsd_transaction_view(inner, egui::Pos2::ZERO, false)
                )
            },
            DemoCsdElement::DemoCsdLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.read().uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                DemoCsdElementView::from(
                    new_democsd_link_view(
                        inner.clone(),
                        source_view,
                        target_view,
                    )
                )
            },
        };

        Ok(v)
    }

    fn background_color(&self) -> egui::Color32 {
        self.background_color
    }
    fn gridlines_color(&self) -> egui::Color32 {
        egui::Color32::from_rgb(220, 220, 220)
    }
    fn show_view_props_fun(&mut self, ui: &mut egui::Ui) {
        ui.label("Background color:");
        egui::widgets::color_picker::color_edit_button_srgba(
            ui,
            &mut self.background_color,
            egui::widgets::color_picker::Alpha::OnlyBlend
        );
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
                egui::TextEdit::singleline(&mut self.buffer.name),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![DemoCsdPropChange::NameChange(Arc::new(self.buffer.name.clone()))],
                )
                .into(),
            );
        };

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.buffer.comment),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![DemoCsdPropChange::CommentChange(Arc::new(
                        self.buffer.comment.clone(),
                    ))],
                )
                .into(),
            );
        }
    }

    fn apply_property_change_fun(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.buffer.name = (*model.name).clone();
        self.buffer.comment = (*model.comment).clone();
    }

    fn show_tool_palette(
        &mut self,
        tool: &mut Option<NaiveDemoCsdTool>,
        drawing_context: &DrawingContext,
        ui: &mut egui::Ui,
    ) {
        let button_height = 60.0;
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
                [width, button_height],
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

        let (empty_a, empty_b) = (HashMap::new(), HashMap::new());
        let empty_q = DemoCsdQueryable::new(&empty_a, &empty_b);
        let mut icon_counter = 0;
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
        ] {
            for (stage, name) in cat {
                let response = ui.add_sized([width, button_height], egui::Button::new(*name).fill(c(*stage)));
                if response.clicked() {
                    if let Some(t) = &tool && t.initial_stage == *stage {
                        *tool = None;
                    } else {
                        *tool = Some(NaiveDemoCsdTool::new(*stage));
                    }
                }

                let icon_rect = egui::Rect::from_min_size(response.rect.min, egui::Vec2::splat(button_height));
                let painter = ui.painter().with_clip_rect(icon_rect);
                let mut mc = canvas::MeasuringCanvas::new(&painter);
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut mc, &None);
                let (scale, offset) = mc.scale_offset_to_fit(egui::Vec2::new(button_height, button_height));
                let mut c = canvas::UiCanvas::new(false, painter, icon_rect, offset, scale, None, Highlight::NONE);
                c.clear(egui::Color32::WHITE.gamma_multiply(0.75));
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut c, &None);
                icon_counter += 1;
            }
            ui.separator();
        }
    }

    fn menubar_options_fun(&self, _ui: &mut egui::Ui, _commands: &mut Vec<ProjectCommand>) {}

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DemoCsdElement>) {
        let (new_model, models) = super::democsd_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, DemoCsdElement>) {
        let models = super::democsd_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }
}

pub fn new(no: u32) -> ERef<dyn DiagramController> {
    let name = format!("New DEMO CSD diagram {}", no);

    let diagram = ERef::new(DemoCsdDiagram::new(
        uuid::Uuid::now_v7().into(),
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        name.clone().into(),
        DemoCsdDiagramAdapter::new(diagram.clone()),
        Vec::new(),
    )
}

pub fn demo(no: u32) -> ERef<dyn DiagramController> {
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
        let name = format!("New DEMO CSD diagram {}", no);
        let diagram = ERef::new(DemoCsdDiagram::new(
            uuid::Uuid::now_v7().into(),
            name.clone(),
            models,
        ));
        DiagramControllerGen2::new(
            Arc::new(uuid::Uuid::now_v7().into()),
            name.clone().into(),
            DemoCsdDiagramAdapter::new(diagram.clone()),
            controllers,
        )
    }
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<DiagramControllerGen2<DemoCsdDomain, DemoCsdDiagramAdapter>>(&uuid)?)
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
}

enum PartialDemoCsdElement {
    None,
    Some(DemoCsdElementView),
    Link {
        link_type: DemoCsdLinkType,
        source: ERef<DemoCsdTransactor>,
        dest: Option<ERef<DemoCsdTransaction>>,
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
                | DemoCsdToolStage::PackageEnd => TARGETTABLE_COLOR,
                DemoCsdToolStage::LinkStart { .. } | DemoCsdToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(DemoCsdElement::DemoCsdPackage(..)) => match self.current_stage {
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd => TARGETTABLE_COLOR,
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
                | DemoCsdToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            Some(DemoCsdElement::DemoCsdTransaction(tx)) => match self.current_stage {
                DemoCsdToolStage::LinkEnd => match &self.result {
                    PartialDemoCsdElement::Link { source, .. } => {
                        if source.read().transaction.as_ref().is_some_and(|e| *e.read().uuid == *tx.read().uuid) {
                            NON_TARGETTABLE_COLOR
                        } else {
                            TARGETTABLE_COLOR
                        }
                    },
                    _ => NON_TARGETTABLE_COLOR
                }
                DemoCsdToolStage::Client
                | DemoCsdToolStage::Transactor
                | DemoCsdToolStage::Bank
                | DemoCsdToolStage::LinkStart { .. }
                | DemoCsdToolStage::PackageStart
                | DemoCsdToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
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
                if let Some(source_view) = q.get_view(&source.read().uuid()) {
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
                (DemoCsdToolStage::LinkEnd, PartialDemoCsdElement::Link { source, dest, .. }) => {
                    if source.read().transaction.as_ref().is_some_and(|e| *e.read().uuid == *inner.read().uuid) {
                        return;
                    }

                    *dest = Some(inner);
                    self.event_lock = true;
                }
                _ => {}
            },
            DemoCsdElement::DemoCsdLink(..) => {}
        }
    }

    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<DemoCsdDomain>,
    ) -> Option<(DemoCsdElementView, Option<Box<dyn ElementSetupModal>>)> {
        match &self.result {
            PartialDemoCsdElement::Some(x) => {
                let x = x.clone();
                self.result = PartialDemoCsdElement::None;
                let esm: Option<Box<dyn ElementSetupModal>> = match &x {
                    DemoCsdElementView::Transactor(eref) => {
                        Some(Box::new(DemoCsdTransactorSetupModal::from(&eref.read().model)))
                    },
                    DemoCsdElementView::Transaction(eref) => {
                        Some(Box::new(DemoCsdTransactionSetupModal::from(&eref.read().model)))
                    },
                    DemoCsdElementView::Package(..)
                    | DemoCsdElementView::Link(..) => unreachable!(),
                };
                Some((x, esm))
            }
            PartialDemoCsdElement::Link {
                source,
                dest: Some(target),
                link_type,
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *target.read().uuid());
                if let (Some(source_view), Some(target_view)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = self.initial_stage;

                    let (_link_model, link_view) = new_democsd_link(
                        *link_type,
                        (source.clone(), source_view),
                        (target.clone(), target_view),
                    );

                    self.result = PartialDemoCsdElement::None;

                    Some((link_view.into(), None))
                } else {
                    None
                }
            }
            PartialDemoCsdElement::Package { a, b: Some(b) } => {
                self.current_stage = DemoCsdToolStage::PackageStart;

                let (_package_model, package_view) =
                    new_democsd_package("A package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialDemoCsdElement::None;
                Some((package_view.into(), None))
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoCsdPackageAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoCsdPackage>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<DemoCsdDomain> for DemoCsdPackageAdapter {
    fn model(&self) -> DemoCsdElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }

    fn add_element(&mut self, e: DemoCsdElement) {
        self.model.write().add_element(e);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.model.write().delete_elements(uuids);
    }
    
    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>
    ) {
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
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.name_buffer = (*model.name).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoCsdElement>,
    ) -> Self where Self: Sized {
        let model_uuid = *self.model.read().uuid;
        let model = if let Some(DemoCsdElement::DemoCsdPackage(m)) = m.get(&model_uuid) {
            m.clone()
        } else {
            let model = self.model.read();
            let model = ERef::new(DemoCsdPackage::new(new_uuid, (*model.name).clone(), model.contained_elements.clone()));
            m.insert(model_uuid, model.clone().into());
            model
        };
        Self { model, name_buffer: self.name_buffer.clone(), comment_buffer: self.comment_buffer.clone() }
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
) -> (ERef<DemoCsdPackage>, ERef<PackageViewT>) {
    let graph_model = ERef::new(DemoCsdPackage::new(
        uuid::Uuid::now_v7().into(),
        name.to_owned(),
        vec![],
    ));
    let graph_view = new_democsd_package_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_democsd_package_view(
    model: ERef<DemoCsdPackage>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT> {
    let m = model.read();
    PackageViewT::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoCsdPackageAdapter {
            model: model.clone(),
            name_buffer: (*m.name).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

// ---

fn new_democsd_transactor(
    identifier: &str,
    name: &str,
    internal: bool,
    transaction: Option<(
        ERef<DemoCsdTransaction>,
        ERef<DemoCsdTransactionView>,
    )>,
    transaction_selfactivating: bool,
    position: egui::Pos2,
) -> (ERef<DemoCsdTransactor>, ERef<DemoCsdTransactorView>) {
    let ta_model = ERef::new(DemoCsdTransactor::new(
        uuid::Uuid::now_v7().into(),
        identifier.to_owned(),
        name.to_owned(),
        internal,
        transaction.as_ref().map(|t| t.0.clone()),
        transaction_selfactivating,
    ));
    let ta_view = new_democsd_transactor_view(
        ta_model.clone(),
        transaction.as_ref().map(|t| t.1.clone()),
        position,
    );

    (ta_model, ta_view)
}
fn new_democsd_transactor_view(
    model: ERef<DemoCsdTransactor>,
    transaction: Option<ERef<DemoCsdTransactionView>>,
    position: egui::Pos2,
) -> ERef<DemoCsdTransactorView> {
    let m = model.read();
    ERef::new(DemoCsdTransactorView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),
        transaction_view: transaction.into(),

        identifier_buffer: (*m.identifier).clone(),
        name_buffer: (*m.name).clone(),
        internal_buffer: m.internal,
        transaction_selfactivating_buffer: m.transaction_selfactivating,
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::ZERO,
    })
}

struct DemoCsdTransactorSetupModal {
    model: ERef<DemoCsdTransactor>,
    identifier_buffer: String,
    name_buffer: String,
}

impl From<&ERef<DemoCsdTransactor>> for DemoCsdTransactorSetupModal {
    fn from(model: &ERef<DemoCsdTransactor>) -> Self {
        let m = model.read();

        Self {
            model: model.clone(),
            identifier_buffer: (*m.identifier).clone(),
            name_buffer: (*m.name).clone(),
        }
    }
}

impl ElementSetupModal for DemoCsdTransactorSetupModal {
    fn show(&mut self, ui: &mut egui::Ui) -> crate::SetupModalResult {
        ui.label("Identifier:");
        ui.text_edit_singleline(&mut self.identifier_buffer);
        ui.label("Name:");
        ui.text_edit_multiline(&mut self.name_buffer);

        let mut result = SetupModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.identifier = Arc::new(self.identifier_buffer.clone());
                m.name = Arc::new(self.name_buffer.clone());
                result = SetupModalResult::CloseModified(*m.uuid);
            }
            if ui.button("Cancel").clicked() {
                result = SetupModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoCsdTransactorView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<DemoCsdTransactor>,
    #[nh_context_serde(entity)]
    transaction_view: UFOption<ERef<DemoCsdTransactionView>>,

    #[nh_context_serde(skip_and_default)]
    identifier_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    internal_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    transaction_selfactivating_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    position: egui::Pos2,
    bounds_rect: egui::Rect,
}

impl Entity for DemoCsdTransactorView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for DemoCsdTransactorView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
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
            UFOption::Some(t) if *uuid == *t.read().model_uuid() => {
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
        if let UFOption::Some(t) = &self.transaction_view {
            if t.write().show_properties(queryable, ui, commands) {
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
                egui::TextEdit::singleline(&mut self.identifier_buffer),
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.identifier_buffer = (*model.identifier).clone();
        self.name_buffer = (*model.name).clone();
        self.internal_buffer = model.internal;
        self.transaction_selfactivating_buffer = model.transaction_selfactivating;
        self.comment_buffer = (*model.comment).clone();
    }

    fn draw_in(
        &mut self,
        queryable: &DemoCsdQueryable,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoCsdTool)>,
    ) -> TargettingStatus {
        let read = self.model.read();

        let radius = 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE;

        let tx_name_bounds = if let UFOption::Some(t) = &self.transaction_view {
            canvas.measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &t.read().model.read().name,
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
                INTERNAL_ROLE_BACKGROUND
            } else {
                EXTERNAL_ROLE_BACKGROUND
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

        // Draw identifier below the position (plus tx name)
        canvas.draw_text(
            self.position + identifier_offset,
            egui::Align2::CENTER_TOP,
            &read.identifier,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw identifier one row below the position (plus tx name)
        canvas.draw_text(
            self.position + name_offset,
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // If tx is present, draw it 4 rows above the position
        if let UFOption::Some(t) = &self.transaction_view {
            let res = t.write().draw_in(queryable, context, canvas, &tool);
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

        if let UFOption::Some(t) = &self.transaction_view {
            t.write().collect_allignment(am);
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoCsdTool>,
        element_setup_modal: &mut Option<Box<dyn ElementSetupModal>>,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) -> EventHandlingStatus {
        let child = self
            .transaction_view
            .as_ref()
            .map(|t| t.write().handle_event(event, ehc, tool, element_setup_modal, commands))
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
                if let UFOption::Some(t) = &self.transaction_view {
                    let t = t.read();
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
                let transaction_id = self.transaction_view.as_ref().map(|t| *t.read().uuid());
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
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            ($self:ident) => {
                if let UFOption::Some(t) = &$self.transaction_view {
                    t.write().apply_command(command, undo_accumulator, affected_models);
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
                        .is_some_and(|e| uuids.contains(&e.read().uuid())) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
                if let UFOption::Some(t) = &self.transaction_view {
                    t.write().apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut vec![], affected_models);
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
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            DemoCsdPropChange::IdentifierChange(identifier) => {
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
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::TransactorSelfactivatingChange(value) => {
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

        if let UFOption::Some(tx) = &self.transaction_view {
            let mut views_tx = HashMap::new();
            let mut tx_l = tx.write();
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
        if requested.is_none_or(|e| e.contains(&self.uuid()) || self.transaction_view.as_ref().is_some_and(|t| e.contains(&t.read().uuid()))) {
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
            t.read().deep_copy_clone(uuid_present, &mut inner, c, m);
            if let Some(DemoCsdElementView::Transaction(t)) = c.get(&t.read().uuid()) {
                Some(t.clone())
            } else { None }
        } else { None }.into();

        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoCsdElement::DemoCsdTransactor(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(DemoCsdTransactor::new(model_uuid, (*old_model.identifier).clone(), (*old_model.name).clone(), old_model.internal,
            old_model.transaction.clone().into(), old_model.transaction_selfactivating));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };
        
        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
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
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, DemoCsdElementView>,
        m: &HashMap<ModelUuid, DemoCsdElement>,
    ) {
        if let Some(DemoCsdElementView::Transaction(new_ta)) = self.transaction_view.as_ref().and_then(|e| c.get(&e.read().uuid()))  {
            self.transaction_view = UFOption::Some(new_ta.clone());
        }
    }
}

fn new_democsd_transaction(
    identifier: &str,
    name: &str,
    position: egui::Pos2,
    actor: bool,
) -> (ERef<DemoCsdTransaction>, ERef<DemoCsdTransactionView>) {
    let tx_model = ERef::new(DemoCsdTransaction::new(
        uuid::Uuid::now_v7().into(),
        super::democsd_models::DemoCsdTransactionKind::Performa,
        identifier.to_owned(),
        name.to_owned(),
    ));
    let tx_view = new_democsd_transaction_view(tx_model.clone(), position, actor);
    (tx_model, tx_view)
}
fn new_democsd_transaction_view(
    model: ERef<DemoCsdTransaction>,
    position: egui::Pos2,
    actor: bool,
) -> ERef<DemoCsdTransactionView> {
    let m = model.read();
    ERef::new(DemoCsdTransactionView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),

        kind_buffer: m.kind,
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
    })
}


struct DemoCsdTransactionSetupModal {
    model: ERef<DemoCsdTransaction>,
    identifier_buffer: String,
    name_buffer: String,
}

impl From<&ERef<DemoCsdTransaction>> for DemoCsdTransactionSetupModal {
    fn from(model: &ERef<DemoCsdTransaction>) -> Self {
        let m = model.read();

        Self {
            model: model.clone(),
            identifier_buffer: (*m.identifier).clone(),
            name_buffer: (*m.name).clone(),
        }
    }
}

impl ElementSetupModal for DemoCsdTransactionSetupModal {
    fn show(&mut self, ui: &mut egui::Ui) -> crate::SetupModalResult {
        ui.label("Identifier:");
        ui.text_edit_singleline(&mut self.identifier_buffer);
        ui.label("Name:");
        ui.text_edit_multiline(&mut self.name_buffer);

        let mut result = SetupModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.identifier = Arc::new(self.identifier_buffer.clone());
                m.name = Arc::new(self.name_buffer.clone());
                result = SetupModalResult::CloseModified(*m.uuid);
            }
            if ui.button("Cancel").clicked() {
                result = SetupModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoCsdTransactionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<DemoCsdTransaction>,

    #[nh_context_serde(skip_and_default)]
    kind_buffer: DemoCsdTransactionKind,
    #[nh_context_serde(skip_and_default)]
    identifier_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged: bool,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    position: egui::Pos2,
    min_shape: canvas::NHShape,
}

impl Entity for DemoCsdTransactionView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for DemoCsdTransactionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
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

        ui.label("Transaction Kind:");
        egui::ComboBox::from_id_salt("Transaction Kind:")
            .selected_text(self.kind_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoCsdTransactionKind::Performa,
                    DemoCsdTransactionKind::Informa,
                    DemoCsdTransactionKind::Forma,
                ] {
                    if ui
                        .selectable_value(&mut self.kind_buffer, value, value.char())
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoCsdPropChange::TransactionKindChange(self.kind_buffer),
                        ]));
                    }
                }
            });

        ui.label("Identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.identifier_buffer),
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
        let read = self.model.read();

        self.min_shape = draw_tx_mark(
            canvas,
            &read.identifier,
            self.position,
            radius,
            self.highlight,
            egui::Color32::WHITE,
            egui::Color32::BLACK,
            match read.kind {
                super::democsd_models::DemoCsdTransactionKind::Performa => PERFORMA_DETAIL,
                super::democsd_models::DemoCsdTransactionKind::Informa => INFORMA_DETAIL,
                super::democsd_models::DemoCsdTransactionKind::Forma => FORMA_DETAIL,
            },
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
        element_setup_modal: &mut Option<Box<dyn ElementSetupModal>>,
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
        affected_models: &mut HashSet<ModelUuid>,
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
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            DemoCsdPropChange::TransactionKindChange(kind) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::TransactionKindChange(
                                        model.kind,
                                    )],
                                ));
                                model.kind = *kind;
                            }
                            DemoCsdPropChange::IdentifierChange(identifier) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                model.identifier = identifier.clone();
                            }
                            DemoCsdPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::NameChange(model.name.clone())],
                                ));
                                model.name = name.clone();
                            }
                            DemoCsdPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoCsdPropChange::CommentChange(model.comment.clone())],
                                ));
                                model.comment = comment.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.kind_buffer = model.kind;
        self.identifier_buffer = (*model.identifier).clone();
        self.name_buffer = (*model.name).clone();
        self.comment_buffer = (*model.comment).clone();
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
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoCsdElement::DemoCsdTransaction(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(DemoCsdTransaction::new(model_uuid, old_model.kind, (*old_model.identifier).clone(), (*old_model.name).clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            kind_buffer: self.kind_buffer,
            identifier_buffer: self.identifier_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged: false,
            highlight: self.highlight,
            position: self.position,
            min_shape: self.min_shape,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoCsdLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoCsdLink>,
    #[nh_context_serde(skip_and_default)]
    link_type_buffer: DemoCsdLinkType,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl DemoCsdLinkAdapter {
    fn line_type(&self) -> canvas::LineType {
        match self.model.read().link_type {
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
        self.model.read().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        Arc::new("TODO".to_owned())
    }

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        (self.line_type(), match self.model.read().link_type {
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
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>
    ) {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("Type:")
            .selected_text(self.link_type_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoCsdLinkType::Initiation,
                    DemoCsdLinkType::Interstriction,
                    DemoCsdLinkType::Interimpediment,
                ] {
                    if ui
                        .selectable_value(&mut self.link_type_buffer, value, value.char())
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoCsdPropChange::LinkTypeChange(self.link_type_buffer),
                        ]));
                    }
                }
            });

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
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoCsdElementOrVertex, DemoCsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.link_type_buffer = model.link_type;
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoCsdElement>
    ) -> Self where Self: Sized {
        let model = self.model.read();
        let model = if let Some(DemoCsdElement::DemoCsdLink(m)) = m.get(&model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(DemoCsdLink::new(new_uuid, model.link_type, model.source.clone(), model.target.clone()));
            m.insert(*model.uuid, modelish.clone().into());
            modelish
        };
        Self { model, link_type_buffer: self.link_type_buffer, comment_buffer: self.comment_buffer.clone() }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DemoCsdElement>
    ) {
        let mut model = self.model.write();
        
        let source_uuid = *model.source.read().uuid();
        if let Some(DemoCsdElement::DemoCsdTransactor(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }

        let target_uuid = *model.target.read().uuid();
        if let Some(DemoCsdElement::DemoCsdTransaction(new_target)) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}

fn new_democsd_link(
    link_type: DemoCsdLinkType,
    source: (
        ERef<DemoCsdTransactor>,
        DemoCsdElementView,
    ),
    target: (
        ERef<DemoCsdTransaction>,
        DemoCsdElementView,
    ),
) -> (ERef<DemoCsdLink>, ERef<LinkViewT>) {
    let link_model = ERef::new(DemoCsdLink::new(
        uuid::Uuid::now_v7().into(),
        link_type,
        source.0,
        target.0,
    ));
    let link_view = new_democsd_link_view(link_model.clone(), source.1, target.1);
    (link_model, link_view)
}
fn new_democsd_link_view(
    model: ERef<DemoCsdLink>,
    source: DemoCsdElementView,
    target: DemoCsdElementView,
) -> ERef<LinkViewT> {
    let m = model.read();
    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoCsdLinkAdapter {
            model: model.clone(),
            link_type_buffer: m.link_type,
            comment_buffer: (*m.comment).clone(),
        },
        source,
        target,
        None,
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
    )
}
