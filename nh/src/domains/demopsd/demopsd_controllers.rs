use super::super::demo::{
    DemoTransactionKind, EXTERNAL_ROLE_BACKGROUND, FORMA_DETAIL, INFORMA_DETAIL,
    INTERNAL_ROLE_BACKGROUND, PERFORMA_DETAIL,
};
use super::demopsd_models::{
    DemoPsdAct, DemoPsdDiagram, DemoPsdElement, DemoPsdFact, DemoPsdLink, DemoPsdLinkType,
    DemoPsdPackage, DemoPsdTransaction,
};
use crate::common::canvas::{self, Highlight, NHShape};
use crate::common::controller::{
    ColorBundle, ColorChangeData, ControllerAdapter, DiagramAdapter, DiagramController,
    DiagramControllerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext,
    EventHandlingStatus, GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand,
    LabelProvider, MGlobalColor, MultiDiagramController, ProjectCommand, PropertiesStatus,
    Queryable, SelectionStatus, SnapManager, TargettingStatus, Tool, TryMerge, View,
};
use crate::common::diagram_settings::{
    DiagramSettings, DiagramSettings2, GroupDisplayStyle, PaletteEditBuffer, ShortCutStatus,
    ShowSettingsResult, ToolPalette,
};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::model::{BucketNoT, ContainerModel, DiagramModel, Model, PositionNoT};
use crate::common::project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer};
use crate::common::ufoption::UFOption;
use crate::common::ui_ext::UiExt;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::views::multiconnection_view::{
    ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView,
    VertexInformation,
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::domains::demo::DemoPackageKind;
use crate::domains::demopsd::demopsd_models::{DemoPsdNote, DemoPsdState, DemoPsdStateInfo};
use crate::{
    CustomModal, CustomModalResult, DefaultNameF, DefaultSettingsF, DeserializeControllerF,
    DeserializeSettingsF, DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::{egui, epaint};
use std::collections::HashSet;
use std::sync::RwLock;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::Arc,
};

pub struct DemoPsdDomain;
impl Domain for DemoPsdDomain {
    type SettingsT = DemoPsdSettings;
    type CommonElementT = DemoPsdElement;
    type DiagramModelT = DemoPsdDiagram;
    type CommonElementViewT = DemoPsdElementView;
    type ViewTargettingSectionT = DemoPsdElementTargettingSection;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveDemoPsdTool;
    type OrdinalMovementT = DemoPsdOrdinalMovement;
    type AddCommandElementT = DemoPsdElementOrVertex;
    type PropChangeT = DemoPsdPropChange;
}

type PackageViewT = PackageView<DemoPsdDomain, DemoPsdPackageAdapter>;
type LinkViewT = MulticonnectionView<DemoPsdDomain, DemoPsdLinkAdapter>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemoPsdOrdinalMovement {
    StateUp,
    StateDown,
    StateLeft,
    StateRight,
}

impl DemoPsdOrdinalMovement {
    fn inverse(&self) -> Self {
        match self {
            Self::StateUp => Self::StateDown,
            Self::StateDown => Self::StateUp,
            Self::StateLeft => Self::StateRight,
            Self::StateRight => Self::StateLeft,
        }
    }
}

#[derive(Clone)]
pub enum DemoPsdPropChange {
    NameChange(Arc<String>),
    IdentifierChange(Arc<String>),

    TransactionKindChange(DemoTransactionKind),
    TransactionPercentageChange(f32),

    StateInternalChange(bool),

    LinkTypeChange(DemoPsdLinkType),
    LinkMultiplicityChange(Arc<String>),

    PackageKindChange(DemoPackageKind),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    NoteAlignChange(Option<egui::Align>, Option<egui::Align>),
}

impl Debug for DemoPsdPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoPsdPropChange::???")
    }
}

impl TryFrom<&DemoPsdPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(_value: &DemoPsdPropChange) -> Result<Self, Self::Error> {
        Err(())
    }
}

impl From<ColorChangeData> for DemoPsdPropChange {
    fn from(value: ColorChangeData) -> Self {
        DemoPsdPropChange::ColorChange(value)
    }
}
impl TryFrom<DemoPsdPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: DemoPsdPropChange) -> Result<Self, Self::Error> {
        match value {
            DemoPsdPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for DemoPsdPropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::IdentifierChange(_), newer @ Self::IdentifierChange(_))
            | (
                Self::TransactionPercentageChange(_),
                newer @ Self::TransactionPercentageChange(_),
            )
            | (Self::LinkMultiplicityChange(_), newer @ Self::LinkMultiplicityChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From, derive_more::TryInto)]
pub enum DemoPsdElementOrVertex {
    Element(DemoPsdElementView),
    Vertex(VertexInformation),
}

impl Debug for DemoPsdElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoPsdElementOrVertex::???")
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "DemoPsdDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum DemoPsdElementView {
    Package(ERef<PackageViewT>),
    Transaction(ERef<DemoPsdTransactionView>),
    Fact(ERef<DemoPsdFactView>),
    Act(ERef<DemoPsdActView>),
    Link(ERef<LinkViewT>),
    Note(ERef<DemoPsdNoteView>),
}

impl DemoPsdElementView {
    fn as_state_view(self) -> Option<DemoPsdStateView> {
        match self {
            DemoPsdElementView::Fact(inner) => Some(inner.into()),
            DemoPsdElementView::Act(inner) => Some(inner.into()),
            DemoPsdElementView::Package(..)
            | DemoPsdElementView::Transaction(..)
            | DemoPsdElementView::Link(..)
            | DemoPsdElementView::Note(..) => None,
        }
    }
}

impl Debug for DemoPsdElementView {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoPsdElementView::???")
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "DemoPsdDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum DemoPsdStateView {
    Fact(ERef<DemoPsdFactView>),
    Act(ERef<DemoPsdActView>),
}

impl DemoPsdStateView {
    fn as_element_view(self) -> DemoPsdElementView {
        match self {
            Self::Fact(inner) => DemoPsdElementView::Fact(inner),
            Self::Act(inner) => DemoPsdElementView::Act(inner),
        }
    }

    fn draw_inner(
        &mut self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &DemoPsdSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
        pos: egui::Pos2,
        text_align: egui::Align2,
    ) -> TargettingStatus {
        match self {
            DemoPsdStateView::Fact(inner) => inner
                .write()
                .draw_inner(q, context, settings, canvas, tool, pos, text_align),
            DemoPsdStateView::Act(inner) => inner
                .write()
                .draw_inner(q, context, settings, canvas, tool, pos, text_align),
        }
    }
}

impl Debug for DemoPsdStateView {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoPsdStateView::???")
    }
}

#[derive(derive_more::From)]
pub enum DemoPsdElementTargettingSection {
    Package(ERef<DemoPsdPackage>),
    Transaction(ERef<DemoPsdTransaction>, egui::Align2),
    Fact(ERef<DemoPsdFact>),
    Act(ERef<DemoPsdAct>),
    Link(ERef<DemoPsdLink>),
    Note(ERef<DemoPsdNote>),
}

impl From<DemoPsdElementTargettingSection> for DemoPsdElement {
    fn from(val: DemoPsdElementTargettingSection) -> Self {
        match val {
            DemoPsdElementTargettingSection::Package(inner) => inner.into(),
            DemoPsdElementTargettingSection::Transaction(inner, ..) => inner.into(),
            DemoPsdElementTargettingSection::Fact(inner) => inner.into(),
            DemoPsdElementTargettingSection::Act(inner) => inner.into(),
            DemoPsdElementTargettingSection::Link(inner) => inner.into(),
            DemoPsdElementTargettingSection::Note(inner) => inner.into(),
        }
    }
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoPsdControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdDiagram>,
}

impl ControllerAdapter<DemoPsdDomain> for DemoPsdControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<DemoPsdDomain, DemoPsdDiagramAdapter>;

    fn model(&self) -> ERef<DemoPsdDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<DemoPsdDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "demopsd"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::demopsd_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, DemoPsdElement, BucketNoT, PositionNoT)>,
    ) {
        fn r(
            e: &DemoPsdElement,
            uuids: &HashSet<ModelUuid>,
            undo: &mut Vec<(ModelUuid, DemoPsdElement, BucketNoT, PositionNoT)>,
        ) {
            match e {
                DemoPsdElement::Package(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.contained_elements.iter().enumerate() {
                        if uuids.contains(&e.uuid()) {
                            undo.push((*w.uuid, e.clone(), 0, idx.try_into().unwrap()));
                        } else {
                            r(e, uuids, undo);
                        }
                    }
                    w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
                }
                DemoPsdElement::Transaction(inner) => {
                    let mut w = inner.write();
                    for (idx, e) in w.before.iter().enumerate() {
                        if uuids.contains(&e.state.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.state.clone().to_element(),
                                if !e.executor {
                                    DemoPsdTransaction::BEFORE_INITIATOR_BUCKET
                                } else {
                                    DemoPsdTransaction::BEFORE_EXECUTOR_BUCKET
                                },
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.state.clone().to_element(), uuids, undo);
                        }
                    }
                    w.before.retain(|e| !uuids.contains(&e.state.uuid()));
                    if let UFOption::Some(e) = &w.p_act
                        && uuids.contains(&e.read().uuid)
                    {
                        undo.push((
                            *w.uuid,
                            e.clone().into(),
                            DemoPsdTransaction::CENTER_BUCKET,
                            0,
                        ));
                        w.p_act = UFOption::None;
                    }
                    for (idx, e) in w.after.iter().enumerate() {
                        if uuids.contains(&e.state.uuid()) {
                            undo.push((
                                *w.uuid,
                                e.state.clone().to_element(),
                                if !e.executor {
                                    DemoPsdTransaction::AFTER_INITIATOR_BUCKET
                                } else {
                                    DemoPsdTransaction::AFTER_EXECUTOR_BUCKET
                                },
                                idx.try_into().unwrap(),
                            ));
                        } else {
                            r(&e.state.clone().to_element(), uuids, undo);
                        }
                    }
                    w.after.retain(|e| !uuids.contains(&e.state.uuid()));
                }
                DemoPsdElement::Fact(_)
                | DemoPsdElement::Act(_)
                | DemoPsdElement::Link(_)
                | DemoPsdElement::Note(_) => {}
            }
        }

        let mut w = self.model.write();
        for (idx, e) in w.contained_elements.iter().enumerate() {
            if uuids.contains(&e.uuid()) {
                undo.push((*w.uuid, e.clone(), 0, idx.try_into().unwrap()));
            } else {
                r(e, uuids, undo);
            }
        }
        w.contained_elements.retain(|e| !uuids.contains(&e.uuid()));
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("DEMO PSD Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared DEMO PSD Diagram".to_owned().into(),
                DemoPsdDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct DemoPsdDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: DemoPsdDiagramBuffer,
}

#[derive(Clone, Default)]
struct DemoPsdDiagramBuffer {
    name: String,
    comment: String,
}

impl DemoPsdDiagramAdapter {
    fn new(model: ERef<DemoPsdDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: DemoPsdDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<DemoPsdDomain> for DemoPsdDiagramAdapter {
    fn model(&self) -> ERef<DemoPsdDiagram> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }

    fn get_element_pos_in(
        &self,
        parent: &ModelUuid,
        model_uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        self.model.read().get_element_pos_in(parent, model_uuid)
    }

    fn create_new_view_for(
        &self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        element: DemoPsdElement,
    ) -> Result<DemoPsdElementView, HashSet<ModelUuid>> {
        let v = match element {
            DemoPsdElement::Package(inner) => DemoPsdElementView::from(new_demopsd_package_view(
                inner,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 100.0),
                },
            )),
            DemoPsdElement::Transaction(inner) => {
                let r = inner.read();

                let f = |e: &DemoPsdStateInfo| DemoPsdStateViewInfo {
                    view: match &e.state {
                        DemoPsdState::Fact(inner) => {
                            new_demopsd_fact_view(inner.clone(), egui::Pos2::ZERO).into()
                        }
                        DemoPsdState::Act(inner) => {
                            new_demopsd_act_view(inner.clone(), egui::Pos2::ZERO).into()
                        }
                    },
                    executor: e.executor,
                };
                let before = r.before.iter().map(&f).collect();
                let p_act = if let UFOption::Some(p_act) = &r.p_act {
                    UFOption::Some(new_demopsd_act_view(p_act.clone(), egui::Pos2::ZERO))
                } else {
                    UFOption::None
                };
                let after = r.after.iter().map(&f).collect();

                DemoPsdElementView::from(new_demopsd_transaction_view(
                    inner.clone(),
                    before,
                    p_act,
                    after,
                    egui::Pos2::ZERO,
                    200.0,
                ))
            }
            DemoPsdElement::Fact(inner) => new_demopsd_fact_view(inner, egui::Pos2::ZERO).into(),
            DemoPsdElement::Act(inner) => new_demopsd_act_view(inner, egui::Pos2::ZERO).into(),
            DemoPsdElement::Link(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.read().uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                DemoPsdElementView::from(new_demopsd_link_view(
                    inner.clone(),
                    source_view,
                    target_view,
                    None,
                ))
            }
            DemoPsdElement::Note(inner) => new_demopsd_note_view(
                inner,
                egui::Pos2::ZERO,
                egui::Align2::CENTER_CENTER,
                MGlobalColor::None,
            )
            .into(),
        };

        Ok(v)
    }
    fn label_for(&self, e: &DemoPsdElement) -> Arc<String> {
        match e {
            DemoPsdElement::Package(inner) => inner.read().name.clone(),
            DemoPsdElement::Transaction(inner) => {
                let r = inner.read();
                let mut l = format!("Transaction {}", r.identifier);
                if !r.name.is_empty() {
                    l.push_str(" (");
                    l.push_str(&r.name);
                    l.push_str(")");
                }

                Arc::new(l)
            }
            DemoPsdElement::Fact(inner) => {
                let r = inner.read();
                let mut l = "Fact".to_string();
                if !r.identifier.is_empty() {
                    l.push_str(" (");
                    l.push_str(&r.identifier);
                    l.push_str(")");
                }

                Arc::new(l)
            }
            DemoPsdElement::Act(inner) => {
                let r = inner.read();
                let mut l = "Act".to_string();
                if !r.identifier.is_empty() {
                    l.push_str(" (");
                    l.push_str(&r.identifier);
                    l.push_str(")");
                }

                Arc::new(l)
            }
            DemoPsdElement::Link(inner) => Arc::new(inner.read().link_type.as_str().to_owned()),
            DemoPsdElement::Note(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Note".to_owned()
                } else {
                    format!("Note ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
        }
    }

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32 {
        global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE)
    }
    fn gridlines_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::from_rgb(220, 220, 220)
    }
    fn show_view_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) {
        ui.label("Background color:");
        if let Some(new_color) = crate::common::controller::mglobalcolor_edit_button(
            drawing_context,
            ui,
            &self.background_color,
        ) {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                DemoPsdPropChange::ColorChange((0, new_color).into()),
            ));
        }
    }
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        _drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                DemoPsdPropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        };

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                DemoPsdPropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                DemoPsdPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                DemoPsdPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                DemoPsdPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.buffer.name = (*model.name).clone();
        self.buffer.comment = (*model.comment).clone();
    }

    fn menubar_options_fun(
        &self,
        _view_uuid: &ViewUuid,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) {
    }
    fn try_handle_custom_shortcut(
        &mut self,
        settings: &DemoPsdSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<DemoPsdDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveDemoPsdTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialDemoPsdElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DemoPsdElement>) {
        let (new_model, models) = super::demopsd_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }
    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, DemoPsdElement>) {
        let models = super::demopsd_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
    fn top_sort_info(
        &self,
        m: &<DemoPsdDomain as Domain>::CommonElementT,
    ) -> crate::common::model::ModelTopSortInfo {
        super::demopsd_models::top_sort_info(m)
    }
}

fn new_controlller(
    model: ERef<DemoPsdDiagram>,
    name: String,
    elements: Vec<DemoPsdElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            DemoPsdControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                DemoPsdDiagramAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let diagram = ERef::new(DemoPsdDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![],
    ));
    new_controlller(diagram, name.to_owned(), vec![])
}

pub fn demo(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let fact1 = new_demopsd_fact("", false, egui::Pos2::new(100.0, 100.0));
    let act1 = new_demopsd_act("rq", true, egui::Pos2::ZERO);
    let fact2 = new_demopsd_fact("TK04/ac", false, egui::Pos2::new(375.0, 400.0));
    let act2 = new_demopsd_act("", false, egui::Pos2::new(200.0, 500.0));

    let response_link = new_demopsd_link(
        DemoPsdLinkType::ResponseLink,
        "",
        (fact1.0.clone(), fact1.1.clone().into()),
        (act1.0.clone(), act1.1.clone().into()),
        None,
    );
    let wait_link = new_demopsd_link(
        DemoPsdLinkType::WaitLink,
        "",
        (fact2.0.clone(), fact2.1.clone().into()),
        (act2.0.clone(), act2.1.clone().into()),
        Some((ViewUuid::now_v7(), egui::Pos2::new(300.0, 400.0))),
    );

    let (tx01, tx01_view) = new_demopsd_transaction(
        "01",
        "usufruct case concluding",
        DemoTransactionKind::Performa,
        vec![(false, act1.0.clone().into(), act1.1.clone().into())],
        None,
        vec![],
        egui::Pos2::new(200.0, 200.0),
        350.0,
    );
    let (tx02, tx02_view) = new_demopsd_transaction(
        "02",
        "resource seizing",
        DemoTransactionKind::Performa,
        vec![],
        None,
        vec![],
        egui::Pos2::new(100.0, 300.0),
        150.0,
    );
    let (tx03, tx03_view) = new_demopsd_transaction(
        "03",
        "resource releasing",
        DemoTransactionKind::Performa,
        vec![],
        Some(act2.clone()),
        vec![],
        egui::Pos2::new(300.0, 300.0),
        150.0,
    );

    let models = vec![
        tx01.into(),
        tx02.into(),
        tx03.into(),
        fact1.0.into(),
        fact2.0.into(),
        response_link.0.into(),
        wait_link.0.into(),
    ];
    let views = vec![
        tx01_view.into(),
        tx02_view.into(),
        tx03_view.into(),
        fact1.1.into(),
        fact2.1.into(),
        response_link.1.into(),
        wait_link.1.into(),
    ];

    {
        let diagram = ERef::new(DemoPsdDiagram::new(
            ModelUuid::now_v7(),
            name.to_owned(),
            models,
        ));
        new_controlller(diagram, name.to_owned(), views)
    }
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        DemoPsdDomain,
        DemoPsdControllerAdapter,
        DiagramControllerGen2<DemoPsdDomain, DemoPsdDiagramAdapter>,
    >>(&uuid)?)
}

pub struct DemoPsdSettings {
    palette: RwLock<ToolPalette<DemoPsdToolStage, DemoPsdDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<DemoPsdToolStage, DemoPsdElementView>>,
}
impl DiagramSettings for DemoPsdSettings {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        shortcut_being_set: &Option<SetShortcut>,
    ) -> ShowSettingsResult {
        let mut ret = ShowSettingsResult::None;
        {
            let mut w = self.palette.write().unwrap();
            let mut buffer = self.palette_edit_buffer.write().unwrap();
            ui.columns(2, |columns| {
                w.show_treeview(gdc, &mut columns[0]);

                let selected = w.get_selected();
                if selected.uuid() != buffer.uuid() {
                    *buffer = w.get_buffer(selected.uuid().cloned());
                }
                match &mut *buffer {
                    PaletteEditBuffer::None => {}
                    PaletteEditBuffer::Group(_uuid, name, display_style) => {
                        let mut modified = false;

                        modified |= columns[1]
                            .labeled_text_edit_singleline("Label", name)
                            .changed();

                        columns[1].label("Display style");
                        egui::ComboBox::from_id_salt("group display style")
                            .selected_text(display_style.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in GroupDisplayStyle::VARIANTS {
                                    modified |=
                                        ui.selectable_value(display_style, e, e.as_str()).clicked();
                                }
                            });

                        if modified {
                            w.set_from_buffer(buffer.clone());
                        }
                    }
                    PaletteEditBuffer::Tool(uuid, name, tool, view, ksc) => {
                        let mut modified = false;
                        modified |= columns[1]
                            .labeled_text_edit_singleline("Label", name)
                            .changed();

                        match crate::common::diagram_settings::show_shortcut(
                            &mut columns[1],
                            ksc,
                            shortcut_being_set
                                .as_ref()
                                .is_some_and(|e| e.is_diagram(uuid)),
                        ) {
                            ShortCutStatus::NoChange => {}
                            ShortCutStatus::Cleared => modified = true,
                            ShortCutStatus::Set => {
                                ret = ShowSettingsResult::SetShortcut(*uuid);
                            }
                            ShortCutStatus::CancelSet => {
                                ret = ShowSettingsResult::CancelShortcutSetting;
                            }
                        }

                        match tool {
                            DemoPsdToolStage::TransactionStart {
                                identifier,
                                name,
                                transaction_kind,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Identifier", identifier)
                                    .changed();
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Transaction kind");
                                egui::ComboBox::from_id_salt("transaction kind")
                                    .selected_text(transaction_kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in DemoTransactionKind::VARIANTS {
                                            modified |= ui
                                                .selectable_value(transaction_kind, e, e.as_str())
                                                .clicked();
                                        }
                                    });
                            }
                            DemoPsdToolStage::Fact {
                                identifier,
                                internal,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Identifier", identifier)
                                    .changed();
                                modified |= columns[1].checkbox(internal, "internal").changed();
                            }
                            DemoPsdToolStage::Act {
                                identifier,
                                internal,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Identifier", identifier)
                                    .changed();
                                modified |= columns[1].checkbox(internal, "internal").changed();
                            }
                            DemoPsdToolStage::LinkStart {
                                link_type,
                                multiplicity,
                            } => {
                                columns[1].label("Link type");
                                egui::ComboBox::from_id_salt("link type")
                                    .selected_text(link_type.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in DemoPsdLinkType::VARIANTS {
                                            modified |= ui
                                                .selectable_value(link_type, e, e.as_str())
                                                .clicked();
                                        }
                                    });

                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Multiplicity", multiplicity)
                                    .changed();
                            }
                            DemoPsdToolStage::PackageStart { name, kind } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();
                                egui::ComboBox::new("package kind", "Package kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in DemoPackageKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).changed();
                                        }
                                    });
                            }
                            DemoPsdToolStage::Note {
                                text,
                                align,
                                background_color,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Text", text)
                                    .changed();

                                egui::ComboBox::new("horizontal align", "Horizontal align")
                                    .selected_text(format!("{:?}", align.x()))
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in [
                                            egui::Align::Min,
                                            egui::Align::Center,
                                            egui::Align::Max,
                                        ] {
                                            modified |= ui
                                                .selectable_value(
                                                    &mut align.0[0],
                                                    e,
                                                    format!("{:?}", e),
                                                )
                                                .changed();
                                        }
                                    });
                                egui::ComboBox::new("vertical align", "Vertical align")
                                    .selected_text(format!("{:?}", align.y()))
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in [
                                            egui::Align::Min,
                                            egui::Align::Center,
                                            egui::Align::Max,
                                        ] {
                                            modified |= ui
                                                .selectable_value(
                                                    &mut align.0[1],
                                                    e,
                                                    format!("{:?}", e),
                                                )
                                                .changed();
                                        }
                                    });

                                columns[1].label("Background color:");
                                if let Some(new_color) =
                                    crate::common::controller::mglobalcolor_edit_button(
                                        gdc,
                                        &mut columns[1],
                                        background_color,
                                    )
                                {
                                    *background_color = new_color;
                                    modified = true;
                                }
                            }
                            DemoPsdToolStage::TransactionEnd
                            | DemoPsdToolStage::LinkEnd
                            | DemoPsdToolStage::PackageEnd => unreachable!(),
                        }

                        if modified {
                            *view = view_for_stage(tool);
                            w.set_from_buffer(buffer.clone());
                        }
                    }
                }
            });
        }

        self.show_reduced(gdc, ui);

        ret
    }
    fn show_reduced(&mut self, _gdc: &GlobalDrawingContext, _ui: &mut egui::Ui) {}
    fn clone_reduced(&self) -> Box<dyn DiagramSettings> {
        Box::new(Self {
            palette: ToolPalette::new(Vec::new()).into(),
            palette_edit_buffer: PaletteEditBuffer::None.into(),
        })
    }

    fn try_set_shortcut(&mut self, tool: uuid::Uuid, shortcut: egui::KeyboardShortcut) {
        let mut wp = self.palette.write().unwrap();
        wp.set_shortcut(tool, Some(shortcut));
        let mut wb = self.palette_edit_buffer.write().unwrap();
        *wb = wp.get_buffer(wb.uuid().cloned());
    }

    fn serialize(&self) -> Result<toml::Value, ()> {
        let mut table = toml::Table::new();
        table.insert(
            "palette".to_owned(),
            self.palette.read().unwrap().serialize()?,
        );
        Ok(table.into())
    }
}
impl DiagramSettings2<DemoPsdDomain> for DemoPsdSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                GroupDisplayStyle,
                Vec<(
                    uuid::Uuid,
                    DemoPsdToolStage,
                    String,
                    DemoPsdElementView,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        ),
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let palette_items = vec![
        (
            "Elements",
            vec![
                (
                    DemoPsdToolStage::TransactionStart {
                        identifier: "01".to_owned(),
                        name: "resource seizing".to_owned(),
                        transaction_kind: DemoTransactionKind::Performa,
                    },
                    "Transaction",
                ),
                (
                    DemoPsdToolStage::Fact {
                        identifier: "rq".to_owned(),
                        internal: true,
                    },
                    "Fact",
                ),
                (
                    DemoPsdToolStage::Act {
                        identifier: "rq".to_owned(),
                        internal: true,
                    },
                    "Act",
                ),
            ],
        ),
        (
            "Relationships",
            vec![
                (
                    DemoPsdToolStage::LinkStart {
                        link_type: DemoPsdLinkType::ResponseLink,
                        multiplicity: "".to_owned(),
                    },
                    "Response Link",
                ),
                (
                    DemoPsdToolStage::LinkStart {
                        link_type: DemoPsdLinkType::WaitLink,
                        multiplicity: "".to_owned(),
                    },
                    "Wait Link",
                ),
            ],
        ),
        (
            "Other",
            vec![
                (
                    DemoPsdToolStage::PackageStart {
                        name: "a package".to_owned(),
                        kind: DemoPackageKind::Package,
                    },
                    "Package",
                ),
                (
                    DemoPsdToolStage::PackageStart {
                        name: "a scope of interest".to_owned(),
                        kind: DemoPackageKind::ScopeOfInterest,
                    },
                    "Scope of Interest",
                ),
                (
                    DemoPsdToolStage::Note {
                        text: "a note".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                        background_color: MGlobalColor::None,
                    },
                    "Note",
                ),
            ],
        ),
    ]
    .into_iter()
    .map(|e| {
        (
            e.0,
            GroupDisplayStyle::List,
            e.1.into_iter()
                .map(|e| {
                    let v = view_for_stage(&e.0);
                    (e.0, e.1, v, None)
                })
                .collect(),
        )
    })
    .collect();

    Box::new(DemoPsdSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
    })
}

fn view_for_stage(s: &DemoPsdToolStage) -> DemoPsdElementView {
    match s {
        DemoPsdToolStage::TransactionStart {
            identifier,
            name,
            transaction_kind,
        } => {
            let ta_view = new_demopsd_transaction(
                identifier,
                name,
                *transaction_kind,
                vec![],
                None,
                vec![],
                egui::Pos2::new(100.0, 75.0),
                200.0,
            )
            .1;
            ta_view.write().refresh_buffers();
            ta_view.into()
        }
        DemoPsdToolStage::Fact {
            identifier,
            internal,
        } => {
            let fact_view = new_demopsd_fact(identifier, *internal, egui::Pos2::ZERO).1;
            fact_view.write().refresh_buffers();
            fact_view.into()
        }
        DemoPsdToolStage::Act {
            identifier,
            internal,
        } => {
            let act_view = new_demopsd_act(identifier, *internal, egui::Pos2::new(100.0, 75.0)).1;
            act_view.write().refresh_buffers();
            act_view.into()
        }
        DemoPsdToolStage::LinkStart {
            link_type,
            multiplicity,
        } => {
            let d1 = new_demopsd_fact("dummy", true, egui::Pos2::ZERO);
            let d2 = new_demopsd_act("dummy", true, egui::Pos2::new(100.0, 75.0));

            let link_view = new_demopsd_link(
                *link_type,
                multiplicity,
                (d1.0, d1.1.into()),
                (d2.0, d2.1.into()),
                None,
            )
            .1;
            link_view.into()
        }
        DemoPsdToolStage::PackageStart { name, kind } => {
            let package_view = new_demopsd_package(
                name,
                *kind,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(150.0, 75.0),
                },
            )
            .1;
            package_view.into()
        }
        DemoPsdToolStage::Note {
            text,
            align,
            background_color,
        } => new_demopsd_note(text, egui::Pos2::ZERO, *align, *background_color)
            .1
            .into(),
        DemoPsdToolStage::TransactionEnd
        | DemoPsdToolStage::LinkEnd
        | DemoPsdToolStage::PackageEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(DemoPsdSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
    }))
}

inventory::submit! {DiagramInfo {
    type_indentifier: "demopsd",
    pretty_name: "Process Structure Diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "/Design & Engineering Methodology for Organizations",
        description: "Process Structure Diagram (transactions, acts, facts, etc.)",
        constructors: &[
            ("empty", &((|no| format!("New DEMO PSD diagram {}", no)) as DefaultNameF), &(new as DiagramConstructorF)),
            ("demo", &((|no| format!("Demo DEMO PSD diagram {}", no)) as DefaultNameF), &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DemoPsdToolStage {
    TransactionStart {
        identifier: String,
        name: String,
        transaction_kind: DemoTransactionKind,
    },
    TransactionEnd,
    Fact {
        identifier: String,
        internal: bool,
    },
    Act {
        identifier: String,
        internal: bool,
    },
    LinkStart {
        link_type: DemoPsdLinkType,
        multiplicity: String,
    },
    LinkEnd,
    PackageStart {
        name: String,
        kind: DemoPackageKind,
    },
    PackageEnd,
    Note {
        text: String,
        align: egui::Align2,
        background_color: MGlobalColor,
    },
}

enum PartialDemoPsdElement {
    None,
    Some(DemoPsdElementView),
    TransactionStart {
        start_pos: egui::Pos2,
    },
    Link {
        source: ERef<DemoPsdFact>,
        dest: Option<ERef<DemoPsdAct>>,
    },
    Package {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveDemoPsdTool {
    uuid: uuid::Uuid,
    initial_stage: DemoPsdToolStage,
    current_stage: DemoPsdToolStage,
    result: PartialDemoPsdElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveDemoPsdTool {
    fn try_spend(&mut self) {
        self.result = PartialDemoPsdElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<DemoPsdDomain> for NaiveDemoPsdTool {
    type Stage = DemoPsdToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: DemoPsdToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialDemoPsdElement::None,
            event_lock: false,
            is_spent: if repeat { None } else { Some(false) },
        }
    }
    fn initial_stage_uuid(&self) -> &uuid::Uuid {
        &self.uuid
    }
    fn repeats(&self) -> bool {
        self.is_spent.is_none()
    }
    fn is_spent(&self) -> bool {
        self.is_spent.is_some_and(|e| e)
    }

    fn targetting_for_section(
        &self,
        element: Result<DemoPsdElementTargettingSection, ERef<DemoPsdDiagram>>,
    ) -> egui::Color32 {
        type TS = DemoPsdElementTargettingSection;
        match element {
            Err(_) | Ok(TS::Package(..)) => match self.current_stage {
                DemoPsdToolStage::TransactionStart { .. }
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact { .. }
                | DemoPsdToolStage::Act { .. }
                | DemoPsdToolStage::PackageStart { .. }
                | DemoPsdToolStage::PackageEnd
                | DemoPsdToolStage::Note { .. } => TARGETTABLE_COLOR,
                DemoPsdToolStage::LinkStart { .. } | DemoPsdToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Ok(TS::Transaction(tx, align)) => {
                if align == egui::Align2::CENTER_CENTER {
                    return if matches!(self.current_stage, DemoPsdToolStage::Act { .. })
                        && !tx.read().p_act.is_some()
                    {
                        TARGETTABLE_COLOR
                    } else {
                        NON_TARGETTABLE_COLOR
                    };
                }

                if matches!(
                    self.current_stage,
                    DemoPsdToolStage::Fact { .. } | DemoPsdToolStage::Act { .. }
                ) {
                    TARGETTABLE_COLOR
                } else {
                    NON_TARGETTABLE_COLOR
                }
            }
            Ok(TS::Fact(..)) => match self.current_stage {
                DemoPsdToolStage::LinkStart { .. } => TARGETTABLE_COLOR,
                DemoPsdToolStage::TransactionStart { .. }
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact { .. }
                | DemoPsdToolStage::Act { .. }
                | DemoPsdToolStage::LinkEnd
                | DemoPsdToolStage::PackageStart { .. }
                | DemoPsdToolStage::PackageEnd
                | DemoPsdToolStage::Note { .. } => NON_TARGETTABLE_COLOR,
            },
            Ok(TS::Act(..)) => match self.current_stage {
                DemoPsdToolStage::LinkEnd => TARGETTABLE_COLOR,
                DemoPsdToolStage::TransactionStart { .. }
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact { .. }
                | DemoPsdToolStage::Act { .. }
                | DemoPsdToolStage::LinkStart { .. }
                | DemoPsdToolStage::PackageStart { .. }
                | DemoPsdToolStage::PackageEnd
                | DemoPsdToolStage::Note { .. } => NON_TARGETTABLE_COLOR,
            },
            Ok(TS::Note(..)) => NON_TARGETTABLE_COLOR,
            Ok(TS::Link(..)) => unreachable!(),
        }
    }
    fn draw_status_hint(
        &self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn canvas::NHCanvas,
        pos: egui::Pos2,
    ) {
        match &self.result {
            PartialDemoPsdElement::TransactionStart { start_pos, .. } => {
                canvas.draw_line(
                    [*start_pos, egui::Pos2::new(pos.x, start_pos.y)],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            PartialDemoPsdElement::Link { source, .. }
                if let DemoPsdToolStage::LinkStart { link_type, .. } = &self.initial_stage =>
            {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke {
                            line_type: link_type.line_type(),
                            width: 1.0,
                            color: egui::Color32::BLACK,
                        },
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialDemoPsdElement::Package { a, .. } => {
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

        match (&self.current_stage, &mut self.result) {
            (DemoPsdToolStage::TransactionStart { .. }, _) => {
                self.result = PartialDemoPsdElement::TransactionStart { start_pos: pos };
                self.current_stage = DemoPsdToolStage::TransactionEnd;
                self.event_lock = true;
            }
            (
                DemoPsdToolStage::TransactionEnd,
                PartialDemoPsdElement::TransactionStart { start_pos },
            ) if let DemoPsdToolStage::TransactionStart {
                identifier,
                name,
                transaction_kind,
            } = &self.initial_stage =>
            {
                let rect = egui::Rect::from_two_pos(
                    egui::Pos2::new(start_pos.x, start_pos.y),
                    egui::Pos2::new(pos.x, start_pos.y),
                );
                let (_transaction_model, transaction_view) = new_demopsd_transaction(
                    identifier,
                    name,
                    *transaction_kind,
                    vec![],
                    None,
                    vec![],
                    rect.center(),
                    rect.width(),
                );
                self.result = PartialDemoPsdElement::Some(transaction_view.into());
                self.current_stage = self.initial_stage.clone();
                self.event_lock = true;
            }
            (
                DemoPsdToolStage::Fact {
                    identifier,
                    internal,
                },
                _,
            ) => {
                let (_fact_model, fact_view) = new_demopsd_fact(identifier, *internal, pos);
                self.result = PartialDemoPsdElement::Some(fact_view.into());
                self.event_lock = true;
            }
            (
                DemoPsdToolStage::Act {
                    identifier,
                    internal,
                },
                _,
            ) => {
                let (_act_model, act_view) = new_demopsd_act(identifier, *internal, pos);
                self.result = PartialDemoPsdElement::Some(act_view.into());
                self.event_lock = true;
            }
            (DemoPsdToolStage::PackageStart { .. }, _) => {
                self.result = PartialDemoPsdElement::Package { a: pos, b: None };
                self.current_stage = DemoPsdToolStage::PackageEnd;
                self.event_lock = true;
            }
            (DemoPsdToolStage::PackageEnd, PartialDemoPsdElement::Package { b, .. }) => {
                *b = Some(pos)
            }
            (
                DemoPsdToolStage::Note {
                    text,
                    align,
                    background_color,
                },
                _,
            ) => {
                let view = new_demopsd_note(text, pos, *align, *background_color).1;
                self.result = PartialDemoPsdElement::Some(view.into());
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, section: DemoPsdElementTargettingSection) {
        if self.event_lock {
            return;
        }

        type TS = DemoPsdElementTargettingSection;

        match section {
            TS::Package(..) | TS::Transaction(..) => {}
            TS::Fact(inner) => {
                if let DemoPsdToolStage::LinkStart { .. } = &self.current_stage {
                    self.result = PartialDemoPsdElement::Link {
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = DemoPsdToolStage::LinkEnd;
                    self.event_lock = true;
                }
            }
            TS::Act(inner) => {
                if let DemoPsdToolStage::LinkEnd = self.current_stage
                    && let PartialDemoPsdElement::Link { dest, .. } = &mut self.result
                {
                    *dest = Some(inner);
                    self.event_lock = true;
                }
            }
            TS::Link(..) | TS::Note(..) => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <DemoPsdDomain as Domain>::OrdinalMovementT,
                <DemoPsdDomain as Domain>::AddCommandElementT,
                <DemoPsdDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &self.result {
            PartialDemoPsdElement::Some(element) => {
                let element = element.clone();
                self.try_spend();
                let esm: Option<Box<dyn CustomModal>> = match &element {
                    DemoPsdElementView::Transaction(inner) => Some(Box::new(
                        DemoPsdTransactionSetupModal::from(&inner.read().model),
                    )),
                    DemoPsdElementView::Fact(..)
                    | DemoPsdElementView::Act(..)
                    | DemoPsdElementView::Note(..) => None,
                    DemoPsdElementView::Package(..) | DemoPsdElementView::Link(..) => {
                        unreachable!()
                    }
                };
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: element.into(),
                    into_model: true,
                });
                Ok(esm)
            }
            PartialDemoPsdElement::Link {
                source,
                dest: Some(target),
                ..
            } if let DemoPsdToolStage::LinkStart {
                link_type,
                multiplicity,
            } = &self.initial_stage =>
            {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *target.read().uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_view.uuid(), preferred_container)
                    && q.is_contained(&target_view.uuid(), preferred_container)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view = new_demopsd_link(
                        *link_type,
                        multiplicity,
                        (source.clone(), source_view),
                        (target.clone(), target_view),
                        None,
                    )
                    .1;

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: DemoPsdElementView::from(link_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialDemoPsdElement::Package { a, b: Some(b) }
                if let DemoPsdToolStage::PackageStart { name, kind } = &self.initial_stage =>
            {
                self.current_stage = self.initial_stage.clone();

                let package_view =
                    new_demopsd_package(name, *kind, egui::Rect::from_two_pos(*a, *b)).1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: DemoPsdElementView::from(package_view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            _ => Err(()),
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct DemoPsdPackageAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdPackage>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: DemoPackageKind,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<DemoPsdDomain> for DemoPsdPackageAdapter {
    fn model_section(&self) -> DemoPsdElementTargettingSection {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        self.model.read().get_element_pos(uuid)
    }

    fn background_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        match self.kind_buffer {
            DemoPackageKind::Package => egui::Color32::WHITE,
            DemoPackageKind::ScopeOfInterest => egui::Color32::TRANSPARENT,
        }
    }
    fn border_stroke(&self, _global_colors: &ColorBundle) -> canvas::Stroke {
        match self.kind_buffer {
            DemoPackageKind::Package => canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            DemoPackageKind::ScopeOfInterest => canvas::Stroke::new_solid(2.0, egui::Color32::GRAY),
        }
    }
    fn show_model_properties(
        &mut self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        egui::ComboBox::new("package kind", "Package kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in DemoPackageKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            DemoPsdPropChange::PackageKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                DemoPsdPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                DemoPsdPropChange::PackageKindChange(kind) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::PackageKindChange(model.kind),
                    ));
                    model.kind = *kind;
                }
                DemoPsdPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                _ => {}
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
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let model_uuid = *self.model.read().uuid;
        let model = if let Some(DemoPsdElement::Package(m)) = m.get(&model_uuid) {
            m.clone()
        } else {
            self.model.read().deep_copy_clone_inner(new_uuid, m)
        };
        Self {
            model,
            name_buffer: self.name_buffer.clone(),
            kind_buffer: self.kind_buffer,
            comment_buffer: self.comment_buffer.clone(),
        }
    }

    fn deep_copy_finish(&mut self, _m: &HashMap<ModelUuid, DemoPsdElement>) {}
}

fn new_demopsd_package(
    name: &str,
    kind: DemoPackageKind,
    bounds_rect: egui::Rect,
) -> (ERef<DemoPsdPackage>, ERef<PackageViewT>) {
    let graph_model = ERef::new(DemoPsdPackage::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        kind,
        vec![],
    ));
    let graph_view = new_demopsd_package_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_demopsd_package_view(
    model: ERef<DemoPsdPackage>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT> {
    let m = model.read();
    PackageViewT::new(
        ViewUuid::now_v7().into(),
        DemoPsdPackageAdapter {
            model: model.clone(),
            name_buffer: (*m.name).clone(),
            kind_buffer: m.kind,
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

// ---

fn new_demopsd_transaction(
    identifier: &str,
    name: &str,
    transaction_kind: DemoTransactionKind,
    before: Vec<(bool, DemoPsdState, DemoPsdStateView)>,
    p_act: Option<(ERef<DemoPsdAct>, ERef<DemoPsdActView>)>,
    after: Vec<(bool, DemoPsdState, DemoPsdStateView)>,
    position: egui::Pos2,
    width: f32,
) -> (ERef<DemoPsdTransaction>, ERef<DemoPsdTransactionView>) {
    let f = |(executor, state, view)| {
        (
            DemoPsdStateInfo { executor, state },
            DemoPsdStateViewInfo { executor, view },
        )
    };
    let (before_models, before_views) = before.into_iter().map(&f).collect();
    let (p_act_model, p_act_view) = if let Some((m, v)) = p_act {
        (UFOption::Some(m), UFOption::Some(v))
    } else {
        (UFOption::None, UFOption::None)
    };
    let (after_models, after_views) = after.into_iter().map(&f).collect();

    let tx_model = ERef::new(DemoPsdTransaction::new(
        ModelUuid::now_v7(),
        transaction_kind,
        identifier.to_owned(),
        name.to_owned(),
        before_models,
        p_act_model,
        after_models,
    ));
    let tx_view = new_demopsd_transaction_view(
        tx_model.clone(),
        before_views,
        p_act_view,
        after_views,
        position,
        width,
    );
    (tx_model, tx_view)
}
fn new_demopsd_transaction_view(
    model: ERef<DemoPsdTransaction>,
    before_views: Vec<DemoPsdStateViewInfo>,
    p_act_view: UFOption<ERef<DemoPsdActView>>,
    after_views: Vec<DemoPsdStateViewInfo>,
    position: egui::Pos2,
    width: f32,
) -> ERef<DemoPsdTransactionView> {
    let m = model.read();
    ERef::new(DemoPsdTransactionView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        before_views,
        p_act_view,
        after_views,
        selected_direct_elements: HashSet::new(),

        kind_buffer: m.kind,
        identifier_buffer: (*m.identifier).clone(),
        name_buffer: (*m.name).clone(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_rect: None,
        highlight: canvas::Highlight::NONE,
        tx_outer_rectangle: egui::Rect::from_center_size(position, egui::Vec2::new(width, 50.0)),
        tx_mark_percentage: 0.5,
    })
}

struct DemoPsdTransactionSetupModal {
    model: ERef<DemoPsdTransaction>,
    first_frame: bool,
    kind_buffer: DemoTransactionKind,
    identifier_buffer: String,
    name_buffer: String,
}

impl From<&ERef<DemoPsdTransaction>> for DemoPsdTransactionSetupModal {
    fn from(model: &ERef<DemoPsdTransaction>) -> Self {
        let m = model.read();

        Self {
            model: model.clone(),
            first_frame: true,
            kind_buffer: m.kind,
            identifier_buffer: (*m.identifier).clone(),
            name_buffer: (*m.name).clone(),
        }
    }
}

impl CustomModal for DemoPsdTransactionSetupModal {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Transaction Kind:");
        egui::ComboBox::from_id_salt("transaction kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for value in DemoTransactionKind::VARIANTS {
                    ui.selectable_value(&mut self.kind_buffer, value, value.as_str());
                }
            });
        ui.label("Identifier:");
        let r = ui.text_edit_singleline(&mut self.identifier_buffer);
        ui.label("Name:");
        ui.text_edit_singleline(&mut self.name_buffer);

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let mut m = self.model.write();
                m.kind = self.kind_buffer;
                m.identifier = Arc::new(self.identifier_buffer.clone());
                m.name = Arc::new(self.name_buffer.clone());
                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button(gdc.translate_0("nh-generic-cancel")).clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct DemoPsdStateViewInfo {
    #[nh_context_serde(entity)]
    view: DemoPsdStateView,
    executor: bool,
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdTransactionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdTransaction>,

    #[nh_context_serde(entity)]
    before_views: Vec<DemoPsdStateViewInfo>,
    #[nh_context_serde(entity)]
    p_act_view: UFOption<ERef<DemoPsdActView>>,
    #[nh_context_serde(entity)]
    after_views: Vec<DemoPsdStateViewInfo>,
    #[nh_context_serde(skip_and_default)]
    selected_direct_elements: HashSet<ViewUuid>,

    #[nh_context_serde(skip_and_default)]
    kind_buffer: DemoTransactionKind,
    #[nh_context_serde(skip_and_default)]
    identifier_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_rect: Option<egui::Rect>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    tx_outer_rectangle: egui::Rect,
    tx_mark_percentage: f32,
}

impl DemoPsdTransactionView {
    const MIN_SIZE: egui::Vec2 = egui::Vec2::splat(50.0);

    fn section_for(&self, pos: egui::Pos2) -> (ERef<DemoPsdTransaction>, egui::Align2) {
        let radius = self.tx_outer_rectangle.height() / 2.0;
        let tx_mark_center = egui::Pos2::new(
            self.tx_outer_rectangle.min.x
                + self.tx_outer_rectangle.width() * self.tx_mark_percentage,
            self.tx_outer_rectangle.center().y,
        );
        let delta = tx_mark_center - pos;

        if delta.x.abs() + delta.y.abs() <= radius {
            (self.model.clone(), egui::Align2::CENTER_CENTER)
        } else {
            let quadrant = match (pos.x > tx_mark_center.x, pos.y > tx_mark_center.y) {
                (false, false) => egui::Align2::LEFT_TOP,
                (false, true) => egui::Align2::LEFT_BOTTOM,
                (true, true) => egui::Align2::RIGHT_BOTTOM,
                (true, false) => egui::Align2::RIGHT_TOP,
            };
            (self.model.clone(), quadrant)
        }
    }

    fn state_insertion_place(
        &self,
        quadrant: egui::Align2,
        pos: egui::Pos2,
    ) -> (PositionNoT, egui::Rect) {
        if quadrant == egui::Align2::CENTER_CENTER {
            return (0, egui::Rect::NOTHING);
        }
        let tx_mark_center = egui::Pos2::new(
            self.tx_outer_rectangle.min.x
                + self.tx_outer_rectangle.width() * self.tx_mark_percentage,
            self.tx_outer_rectangle.center().y,
        );
        let states_total = match quadrant.x() {
            egui::Align::Min => self.before_views.len(),
            egui::Align::Center => unreachable!(),
            egui::Align::Max => self.after_views.len(),
        };
        let (quadrant_start_x, quadrant_width) = match quadrant.x() {
            egui::Align::Min => (
                self.tx_outer_rectangle.min.x,
                self.tx_mark_percentage * self.tx_outer_rectangle.width(),
            ),
            egui::Align::Center => unreachable!(),
            egui::Align::Max => (
                tx_mark_center.x,
                (1.0 - self.tx_mark_percentage) * self.tx_outer_rectangle.width(),
            ),
        };
        let area_start_x = match quadrant.x() {
            egui::Align::Min => self.tx_outer_rectangle.min.x,
            egui::Align::Center => unreachable!(),
            egui::Align::Max => tx_mark_center.x + Self::MIN_SIZE.x / 2.0,
        };
        let state_width = (quadrant_width - Self::MIN_SIZE.x / 2.0) / (states_total as f32 + 1.0);
        let current_state_idx = ((pos.x - area_start_x) / state_width).floor();

        let selected_state_start_x = match quadrant.x() {
            egui::Align::Max if current_state_idx <= 0.0 => quadrant_start_x,
            _ => area_start_x + current_state_idx.clamp(0.0, states_total as f32) * state_width,
        };

        let selected_state_width = match quadrant.x() {
            egui::Align::Min if current_state_idx >= states_total as f32 => {
                state_width + Self::MIN_SIZE.x / 2.0
            }
            egui::Align::Max if current_state_idx <= 0.0 => state_width + Self::MIN_SIZE.x / 2.0,
            _ => state_width,
        };

        let start_y = match quadrant.y() {
            egui::Align::Min => self.tx_outer_rectangle.min.y,
            egui::Align::Center => unreachable!(),
            egui::Align::Max => tx_mark_center.y,
        };

        (
            (current_state_idx as usize).try_into().unwrap(),
            egui::Rect::from_min_size(
                egui::Pos2::new(selected_state_start_x, start_y),
                egui::Vec2::new(selected_state_width, Self::MIN_SIZE.y / 2.0),
            ),
        )
    }
}

impl Entity for DemoPsdTransactionView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for DemoPsdTransactionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
}

impl ElementController<DemoPsdElement> for DemoPsdTransactionView {
    fn model(&self) -> DemoPsdElement {
        self.model.clone().into()
    }
    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Rect {
            inner: self.tx_outer_rectangle,
        }
    }
    fn position(&self) -> egui::Pos2 {
        self.tx_outer_rectangle.center()
    }
}

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdTransactionView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> PropertiesStatus<DemoPsdDomain> {
        // try before
        if let Some(child) = self
            .before_views
            .iter_mut()
            .flat_map(|e| e.view.show_properties(gdc, q, ui, commands).non_default())
            .next()
        {
            return child;
        }
        // try P-act
        if let Some(child) = self.p_act_view.as_mut().and_then(|c| {
            c.write()
                .show_properties(gdc, q, ui, commands)
                .non_default()
        }) {
            return child;
        }
        // try after
        if let Some(child) = self
            .after_views
            .iter_mut()
            .flat_map(|e| e.view.show_properties(gdc, q, ui, commands).non_default())
            .next()
        {
            return child;
        }

        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Transaction Kind:");
        egui::ComboBox::from_id_salt("transaction kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for value in DemoTransactionKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, value, value.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            DemoPsdPropChange::TransactionKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline("Identifier:", &mut self.identifier_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position();

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(x - self.position().x, 0.0),
                ));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(0.0, y - self.position().y),
                ));
            }
        });

        ui.horizontal(|ui| {
            let mut width = self.tx_outer_rectangle.width();
            let mark_deadzone = 2500.0 / width;
            let mut mark_percentage = self.tx_mark_percentage * 100.0;

            ui.label("width");
            if ui
                .add(
                    egui::DragValue::new(&mut width)
                        .range(Self::MIN_SIZE.x..=5000.0)
                        .speed(1.0),
                )
                .changed()
            {
                commands.push(InsensitiveCommand::ResizeElementsBy(
                    q.selected_views(),
                    egui::Align2::LEFT_CENTER,
                    egui::Vec2::new(width - self.tx_outer_rectangle.width(), 0.0),
                ));
            }
            ui.label("mark percentage");
            if ui
                .add(
                    egui::DragValue::new(&mut mark_percentage)
                        .range(mark_deadzone..=(100.0 - mark_deadzone))
                        .speed(1.0),
                )
                .changed()
            {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    DemoPsdPropChange::TransactionPercentageChange(mark_percentage / 100.0),
                ));
            }
        });

        PropertiesStatus::Shown
    }
    fn draw_in(
        &mut self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &DemoPsdSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
    ) -> TargettingStatus {
        let read = self.model.read();

        let detail_color = match read.kind {
            DemoTransactionKind::Performa => PERFORMA_DETAIL,
            DemoTransactionKind::Informa => INFORMA_DETAIL,
            DemoTransactionKind::Forma => FORMA_DETAIL,
        };

        canvas.draw_rectangle(
            self.tx_outer_rectangle,
            egui::CornerRadius::same(127),
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        let radius = self.tx_outer_rectangle.height() / 2.0;
        let tx_mark_center = egui::Pos2::new(
            self.tx_outer_rectangle.min.x
                + self.tx_outer_rectangle.width() * self.tx_mark_percentage,
            self.tx_outer_rectangle.center().y,
        );

        let draw_tx_mark = |canvas: &mut dyn canvas::NHCanvas| {
            canvas.draw_polygon(
                vec![
                    tx_mark_center - egui::Vec2::new(0.0, radius),
                    tx_mark_center + egui::Vec2::new(radius, 0.0),
                    tx_mark_center + egui::Vec2::new(0.0, radius),
                    tx_mark_center - egui::Vec2::new(radius, 0.0),
                    tx_mark_center - egui::Vec2::new(0.0, radius),
                ],
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, detail_color),
                canvas::Highlight::NONE,
            );

            canvas.draw_text(
                tx_mark_center,
                egui::Align2::CENTER_CENTER,
                &read.identifier,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
            canvas.draw_text(
                self.tx_outer_rectangle.center_top(),
                egui::Align2::CENTER_BOTTOM,
                &read.name,
                canvas::CLASS_BOTTOM_FONT_SIZE,
                egui::Color32::BLACK,
            );
        };
        draw_tx_mark(canvas);

        let mut child_targetting_drawn = false;
        // draw before
        let spaces_count = (self.before_views.len() + 1) as f32;
        let width_before =
            self.tx_outer_rectangle.width() * self.tx_mark_percentage - Self::MIN_SIZE.x / 2.0;
        for (idx, e) in self
            .before_views
            .iter_mut()
            .enumerate()
            .map(|(idx, e)| ((idx + 1) as f32, e))
        {
            let (pos_y, align) = if !e.executor {
                (self.tx_outer_rectangle.min.y, egui::Align2::CENTER_TOP)
            } else {
                (self.tx_outer_rectangle.max.y, egui::Align2::CENTER_BOTTOM)
            };

            child_targetting_drawn |= e.view.draw_inner(
                q,
                context,
                settings,
                canvas,
                tool,
                egui::Pos2::new(
                    self.tx_outer_rectangle.min.x + width_before * idx / spaces_count,
                    pos_y,
                ),
                align,
            ) == TargettingStatus::Drawn;
        }
        // draw P-act
        if let UFOption::Some(e) = &self.p_act_view {
            child_targetting_drawn |= e.write().draw_inner(
                q,
                context,
                settings,
                canvas,
                tool,
                egui::Pos2::new(tx_mark_center.x, self.tx_outer_rectangle.max.y),
                egui::Align2::LEFT_TOP,
            ) == TargettingStatus::Drawn;
        }
        // draw after
        let spaces_count = (self.after_views.len() + 1) as f32;
        let width_after = self.tx_outer_rectangle.width() * (1.0 - self.tx_mark_percentage)
            - Self::MIN_SIZE.x / 2.0;
        for (idx, e) in self
            .after_views
            .iter_mut()
            .enumerate()
            .map(|(idx, e)| ((idx + 1) as f32, e))
        {
            let (pos_y, align) = if !e.executor {
                (self.tx_outer_rectangle.min.y, egui::Align2::CENTER_TOP)
            } else {
                (self.tx_outer_rectangle.max.y, egui::Align2::CENTER_BOTTOM)
            };

            child_targetting_drawn |= e.view.draw_inner(
                q,
                context,
                settings,
                canvas,
                tool,
                egui::Pos2::new(
                    tx_mark_center.x + Self::MIN_SIZE.x / 2.0 + width_after * idx / spaces_count,
                    pos_y,
                ),
                align,
            ) == TargettingStatus::Drawn;
        }

        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && !child_targetting_drawn
        {
            let section = self.section_for(*pos);
            if !matches!(
                &tool.initial_stage,
                DemoPsdToolStage::Fact { .. } | DemoPsdToolStage::Act { .. }
            ) && self.tx_outer_rectangle.contains(*pos)
            {
                canvas.draw_rectangle(
                    self.tx_outer_rectangle,
                    egui::CornerRadius::ZERO,
                    tool.targetting_for_section(Ok(section.into())),
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                return TargettingStatus::Drawn;
            }
            if section.1 == egui::Align2::CENTER_CENTER {
                canvas.draw_polygon(
                    vec![
                        tx_mark_center - egui::Vec2::new(0.0, radius),
                        tx_mark_center + egui::Vec2::new(radius, 0.0),
                        tx_mark_center + egui::Vec2::new(0.0, radius),
                        tx_mark_center - egui::Vec2::new(radius, 0.0),
                        tx_mark_center - egui::Vec2::new(0.0, radius),
                    ],
                    tool.targetting_for_section(Ok(section.into())),
                    canvas::Stroke::new_solid(1.0, detail_color),
                    canvas::Highlight::NONE,
                );
                return TargettingStatus::Drawn;
            } else if self.tx_outer_rectangle.contains(*pos) {
                canvas.draw_rectangle(
                    self.state_insertion_place(section.1, *pos).1,
                    egui::CornerRadius::ZERO,
                    tool.targetting_for_section(Ok(section.into())),
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                draw_tx_mark(canvas);
                return TargettingStatus::Drawn;
            }
        }

        TargettingStatus::NotDrawn
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());

        for e in self.before_views.iter_mut() {
            e.view.collect_allignment(am);
        }
        if let UFOption::Some(e) = &mut self.p_act_view {
            e.write().collect_allignment(am);
        }
        for e in self.after_views.iter_mut() {
            e.view.collect_allignment(am);
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<DemoPsdDomain as Domain>::SettingsT,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveDemoPsdTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> EventHandlingStatus {
        let child_status = self
            .before_views
            .iter_mut()
            .flat_map(|e| {
                let s = e.view.handle_event(
                    event,
                    ehc,
                    settings,
                    q,
                    tool,
                    element_setup_modal,
                    commands,
                );
                if s != EventHandlingStatus::NotHandled {
                    Some((*e.view.uuid(), s))
                } else {
                    None
                }
            })
            .next();
        let child_status = child_status.or_else(|| {
            self.p_act_view.as_ref().and_then(|e| {
                let mut w = e.write();
                let s =
                    w.handle_event(event, ehc, settings, q, tool, element_setup_modal, commands);
                if s != EventHandlingStatus::NotHandled {
                    Some((*w.uuid(), s))
                } else {
                    None
                }
            })
        });
        let child_status = child_status.or_else(|| {
            self.after_views
                .iter_mut()
                .flat_map(|e| {
                    let s = e.view.handle_event(
                        event,
                        ehc,
                        settings,
                        q,
                        tool,
                        element_setup_modal,
                        commands,
                    );
                    if s != EventHandlingStatus::NotHandled {
                        Some((*e.view.uuid(), s))
                    } else {
                        None
                    }
                })
                .next()
        });

        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if child_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) => {
                if self.min_shape().contains(pos) {
                    self.dragged_rect = Some(self.tx_outer_rectangle);
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::MouseUp(_) => {
                if self.dragged_rect.is_some() {
                    self.dragged_rect = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) => {
                match child_status {
                    Some((k, EventHandlingStatus::HandledByElement)) => {
                        if ehc
                            .modifier_settings
                            .hold_selection
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            commands
                                .push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED));
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                Highlight::SELECTED,
                            ));
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ));
                        }
                        return EventHandlingStatus::HandledByContainer;
                    }
                    Some((_, EventHandlingStatus::HandledByContainer)) => {
                        return EventHandlingStatus::HandledByContainer;
                    }
                    _ => {}
                }

                if !self.min_shape().contains(pos) {
                    return child_status
                        .map(|e| e.1)
                        .unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    let section = self.section_for(pos);
                    let quadrant = section.1;
                    tool.add_section(section.into());

                    if (self.p_act_view.as_ref().is_none()
                        && !matches!(tool.initial_stage, DemoPsdToolStage::Fact { .. }))
                        || quadrant != egui::Align2::CENTER_CENTER
                    {
                        tool.add_position(pos);
                        let quadrant_no = match quadrant {
                            egui::Align2::CENTER_CENTER => DemoPsdTransaction::CENTER_BUCKET,
                            egui::Align2::LEFT_TOP => DemoPsdTransaction::BEFORE_INITIATOR_BUCKET,
                            egui::Align2::LEFT_BOTTOM => DemoPsdTransaction::BEFORE_EXECUTOR_BUCKET,
                            egui::Align2::RIGHT_BOTTOM => DemoPsdTransaction::AFTER_EXECUTOR_BUCKET,
                            egui::Align2::RIGHT_TOP => DemoPsdTransaction::AFTER_INITIATOR_BUCKET,
                            _ => unreachable!(),
                        };
                        let pos = self.state_insertion_place(quadrant, pos).0;

                        if let Ok(esm) =
                            tool.try_flush(q, &self.uuid, quadrant_no, Some(pos), commands)
                            && ehc
                                .modifier_settings
                                .alternative_tool_mode
                                .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            *element_setup_modal = esm;
                        }
                    }

                    EventHandlingStatus::HandledByContainer
                } else {
                    if ehc
                        .modifier_settings
                        .hold_selection
                        .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
                        self.highlight.selected = true;
                    } else {
                        self.highlight.selected = !self.highlight.selected;
                    }

                    EventHandlingStatus::HandledByElement
                }
            }
            InputEvent::Drag { delta, .. } if self.dragged_rect.is_some() => {
                let translated_real_rect = self.dragged_rect.unwrap().translate(delta);
                self.dragged_rect = Some(translated_real_rect);
                let translated_shape = canvas::NHShape::Rect {
                    inner: translated_real_rect,
                };
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_shape, |e| {
                        !ehc.all_elements
                            .get(e)
                            .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.tx_outer_rectangle.center();

                if self.highlight.selected {
                    commands.push(InsensitiveCommand::MovePositional(
                        q.selected_views(),
                        coerced_delta,
                    ));
                } else {
                    commands.push(InsensitiveCommand::MovePositional(
                        std::iter::once(*self.uuid).collect(),
                        coerced_delta,
                    ));
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        diagram_model: &ERef<DemoPsdDiagram>,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                for e in &mut self.before_views {
                    e.view
                        .apply_command(diagram_model, command, undo_accumulator, affected_models);
                }
                if let UFOption::Some(e) = &self.p_act_view {
                    e.write().apply_command(
                        diagram_model,
                        command,
                        undo_accumulator,
                        affected_models,
                    );
                }
                for e in &mut self.after_views {
                    e.view
                        .apply_command(diagram_model, command, undo_accumulator, affected_models);
                }
            };
        }
        macro_rules! resize_to {
            ($rect:expr) => {
                undo_accumulator.push(InsensitiveCommand::ResizeElementTo(
                    *self.uuid,
                    self.tx_outer_rectangle,
                ));
                self.tx_outer_rectangle = $rect;
            };
        }

        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);

                if h.selected {
                    match set {
                        true => {
                            for e in &self.before_views {
                                self.selected_direct_elements.insert(*e.view.uuid());
                            }
                            if let UFOption::Some(e) = &self.p_act_view {
                                self.selected_direct_elements.insert(*e.read().uuid);
                            }
                            for e in &self.after_views {
                                self.selected_direct_elements.insert(*e.view.uuid());
                            }
                        }
                        false => self.selected_direct_elements.clear(),
                    }
                }

                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }

                if h.selected {
                    for e in self
                        .before_views
                        .iter()
                        .filter(|e| uuids.contains(&e.view.uuid()))
                    {
                        match set {
                            true => self.selected_direct_elements.insert(*e.view.uuid()),
                            false => self.selected_direct_elements.remove(&e.view.uuid()),
                        };
                    }
                    if let UFOption::Some(e) = &self.p_act_view
                        && let r = e.read()
                        && uuids.contains(&r.uuid)
                    {
                        match set {
                            true => self.selected_direct_elements.insert(*r.uuid),
                            false => self.selected_direct_elements.remove(&r.uuid),
                        };
                    }
                    for e in self
                        .after_views
                        .iter()
                        .filter(|e| uuids.contains(&e.view.uuid()))
                    {
                        match set {
                            true => self.selected_direct_elements.insert(*e.view.uuid()),
                            false => self.selected_direct_elements.remove(&e.view.uuid()),
                        };
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.highlight.selected = (self.highlight.selected && *retain)
                    || self.min_shape().contained_within(*rect);
                recurse!();
            }
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.tx_outer_rectangle = self.tx_outer_rectangle.translate(*delta);
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }

            InsensitiveCommand::ResizeElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    let min_delta_x = Self::MIN_SIZE.x - self.tx_outer_rectangle.width();
                    let (left, right) = match align.x() {
                        egui::Align::Min => (0.0, delta.x.max(min_delta_x)),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => ((-delta.x).max(min_delta_x), 0.0),
                    };

                    let r = self.tx_outer_rectangle
                        + epaint::MarginF32 {
                            left,
                            right,
                            top: 0.0,
                            bottom: 0.0,
                        };
                    resize_to!(r);
                }
                recurse!();
            }
            InsensitiveCommand::ResizeElementTo(uuid, rect) => {
                if *uuid == *self.uuid {
                    resize_to!(*rect);
                }
                recurse!();
            }

            InsensitiveCommand::DeleteSpecificElements(uuids, _) => {
                {
                    let r = self.model.read();

                    if let Some(e) = self.p_act_view.as_ref()
                        && uuids.contains(&e.read().uuid)
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: 0,
                            position: None,
                            element: DemoPsdElementOrVertex::Element(e.clone().into()),
                            into_model: false,
                        });
                        self.p_act_view = UFOption::None;
                    }

                    let mut closure = |e: &DemoPsdStateViewInfo| {
                        if uuids.contains(&e.view.uuid())
                            && let Some((b, pos)) = r.get_element_pos(&e.view.model_uuid())
                        {
                            undo_accumulator.push(InsensitiveCommand::AddDependency {
                                target: *self.uuid,
                                bucket: b,
                                position: Some(pos),
                                element: DemoPsdElementOrVertex::Element(
                                    e.view.clone().as_element_view(),
                                ),
                                into_model: false,
                            });
                            false
                        } else {
                            true
                        }
                    };
                    self.before_views.retain(&mut closure);
                    self.after_views.retain(&mut closure);
                }
                recurse!();
            }
            InsensitiveCommand::AddDependency {
                target,
                bucket,
                position,
                element,
                into_model,
            } => {
                if *target == *self.uuid {
                    let model_uuid = *self.model_uuid();
                    if *bucket == DemoPsdTransaction::CENTER_BUCKET {
                        if let DemoPsdElementOrVertex::Element(DemoPsdElementView::Act(e)) = element
                            && diagram_model
                                .write()
                                .insert_element_into(model_uuid, *bucket, None, e.read().model())
                                .is_ok()
                        {
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                                target: *target,
                                bucket: *bucket,
                                element: *e.read().uuid,
                                including_model: *into_model,
                            });
                            if *into_model {
                                affected_models.insert(*e.read().model_uuid());
                            }
                            self.p_act_view = UFOption::Some(e.clone());
                        }
                    } else if let DemoPsdElementOrVertex::Element(e) = element
                        && let Some(e) = e.clone().as_state_view()
                    {
                        let pos = self.model.read().get_element_pos(&e.model_uuid());
                        if let Some(model_pos) = pos.map(|e| e.1).or_else(|| {
                            if *into_model {
                                diagram_model
                                    .write()
                                    .insert_element_into(model_uuid, *bucket, *position, e.model())
                                    .ok()
                            } else {
                                None
                            }
                        }) {
                            let after = match *bucket {
                                0
                                | DemoPsdTransaction::BEFORE_INITIATOR_BUCKET
                                | DemoPsdTransaction::BEFORE_EXECUTOR_BUCKET => false,
                                DemoPsdTransaction::AFTER_EXECUTOR_BUCKET
                                | DemoPsdTransaction::AFTER_INITIATOR_BUCKET => true,
                                _ => return,
                            };
                            let executor = match *bucket {
                                0
                                | DemoPsdTransaction::BEFORE_INITIATOR_BUCKET
                                | DemoPsdTransaction::AFTER_INITIATOR_BUCKET => false,
                                DemoPsdTransaction::BEFORE_EXECUTOR_BUCKET
                                | DemoPsdTransaction::AFTER_EXECUTOR_BUCKET => true,
                                _ => unreachable!(),
                            };

                            undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                                target: *target,
                                bucket: *bucket,
                                element: *e.uuid(),
                                including_model: *into_model,
                            });
                            if *into_model {
                                affected_models.insert(*e.model_uuid());
                            }

                            let view_pos = |arr: &Vec<DemoPsdStateViewInfo>| {
                                let mut view_pos: PositionNoT = 0;
                                for e in arr {
                                    let Some((_b, pos)) =
                                        self.model.read().get_element_pos(&e.view.model_uuid())
                                    else {
                                        unreachable!()
                                    };
                                    if pos < model_pos {
                                        view_pos += 1;
                                    } else {
                                        break;
                                    }
                                }
                                view_pos
                            };

                            if !after {
                                let view_pos = view_pos(&self.before_views);
                                self.before_views.insert(
                                    view_pos.try_into().unwrap(),
                                    DemoPsdStateViewInfo { view: e, executor },
                                );
                            } else {
                                let view_pos = view_pos(&self.after_views);
                                self.after_views.insert(
                                    view_pos.try_into().unwrap(),
                                    DemoPsdStateViewInfo { view: e, executor },
                                );
                            }
                        }
                    }
                }
                recurse!();
            }
            InsensitiveCommand::RemoveDependency {
                target,
                bucket,
                element,
                including_model,
            } => {
                if *target == *self.uuid {
                    let model_uuid = *self.model_uuid();
                    if matches!(*bucket, 0 | DemoPsdTransaction::CENTER_BUCKET)
                        && let Some(e) = self.p_act_view.as_ref()
                        && *element == *e.read().uuid
                        && diagram_model
                            .write()
                            .remove_element_from(model_uuid, &e.read().model_uuid())
                            .is_some()
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *target,
                            bucket: *bucket,
                            position: None,
                            element: DemoPsdElementOrVertex::Element(e.clone().into()),
                            into_model: *including_model,
                        });

                        self.p_act_view = UFOption::None;
                    }
                    let mut closure = |e: &DemoPsdStateViewInfo| {
                        if *e.view.uuid() == *element
                            && let Some((b, pos)) = diagram_model
                                .write()
                                .remove_element_from(model_uuid, &e.view.model_uuid())
                        {
                            undo_accumulator.push(InsensitiveCommand::AddDependency {
                                target: *target,
                                bucket: b,
                                position: Some(pos),
                                element: DemoPsdElementOrVertex::Element(
                                    e.view.clone().as_element_view(),
                                ),
                                into_model: *including_model,
                            });
                            false
                        } else {
                            true
                        }
                    };
                    if matches!(
                        *bucket,
                        0 | DemoPsdTransaction::BEFORE_INITIATOR_BUCKET
                            | DemoPsdTransaction::BEFORE_EXECUTOR_BUCKET
                    ) {
                        self.before_views.retain(&mut closure);
                    }
                    if matches!(
                        *bucket,
                        0 | DemoPsdTransaction::AFTER_EXECUTOR_BUCKET
                            | DemoPsdTransaction::AFTER_INITIATOR_BUCKET
                    ) {
                        self.after_views.retain(&mut closure);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(uuids, direction) => {
                let mut undo_uuids = HashSet::new();
                match direction {
                    DemoPsdOrdinalMovement::StateUp | DemoPsdOrdinalMovement::StateDown => {
                        let mut mw = self.model.write();
                        let mut try_flip =
                            |b: &mut Vec<DemoPsdStateInfo>, e: &mut DemoPsdStateViewInfo| {
                                if uuids.contains(&e.view.uuid())
                                    && e.executor == (*direction == DemoPsdOrdinalMovement::StateUp)
                                {
                                    let muiid = *e.view.model_uuid();
                                    b.iter_mut().for_each(|e| {
                                        if *e.state.uuid() == muiid {
                                            e.executor = !e.executor;
                                        }
                                    });
                                    e.executor = !e.executor;
                                    undo_uuids.insert(*e.view.uuid());
                                }
                            };
                        self.before_views
                            .iter_mut()
                            .for_each(|e| try_flip(&mut mw.before, e));
                        self.after_views
                            .iter_mut()
                            .for_each(|e| try_flip(&mut mw.after, e));
                    }
                    DemoPsdOrdinalMovement::StateLeft | DemoPsdOrdinalMovement::StateRight => {
                        let mut remainder = None;
                        if *direction == DemoPsdOrdinalMovement::StateRight
                            && self
                                .before_views
                                .last()
                                .filter(|e| uuids.contains(&e.view.uuid()))
                                .is_some()
                        {
                            remainder = Some((
                                self.model.write().before.pop().unwrap(),
                                self.before_views.pop().unwrap(),
                            ));
                        }
                        {
                            let before_iter: Box<dyn Iterator<Item = &mut DemoPsdStateViewInfo>> =
                                match direction {
                                    DemoPsdOrdinalMovement::StateLeft => {
                                        Box::new(self.before_views.iter_mut())
                                    }
                                    DemoPsdOrdinalMovement::StateRight => {
                                        Box::new(self.before_views.iter_mut().rev())
                                    }
                                    _ => unreachable!(),
                                };
                            let mut before_iter = before_iter.peekable();
                            while let Some(dest) = before_iter.next()
                                && let Some(src) = before_iter.peek_mut()
                            {
                                if uuids.contains(&src.view.uuid())
                                    && !uuids.contains(&dest.view.uuid())
                                {
                                    let mut w = self.model.write();
                                    let Some(new_pos) = w.get_element_pos(&dest.view.model_uuid())
                                    else {
                                        continue;
                                    };
                                    w.move_element(&src.view.model_uuid(), new_pos.1);
                                    undo_uuids.insert(*src.view.uuid());
                                    std::mem::swap(dest, *src);
                                }
                            }
                        }
                        if *direction == DemoPsdOrdinalMovement::StateLeft
                            && self
                                .after_views
                                .first()
                                .filter(|e| uuids.contains(&e.view.uuid()))
                                .is_some()
                        {
                            remainder = Some((
                                self.model.write().after.remove(0),
                                self.after_views.remove(0),
                            ));
                        }
                        {
                            let after_iter: Box<dyn Iterator<Item = &mut DemoPsdStateViewInfo>> =
                                match direction {
                                    DemoPsdOrdinalMovement::StateLeft => {
                                        Box::new(self.after_views.iter_mut())
                                    }
                                    DemoPsdOrdinalMovement::StateRight => {
                                        Box::new(self.after_views.iter_mut().rev())
                                    }
                                    _ => unreachable!(),
                                };
                            let mut after_iter = after_iter.peekable();
                            while let Some(dest) = after_iter.next()
                                && let Some(src) = after_iter.peek_mut()
                            {
                                if uuids.contains(&src.view.uuid())
                                    && !uuids.contains(&dest.view.uuid())
                                {
                                    let mut w = self.model.write();
                                    let Some(new_pos) = w.get_element_pos(&dest.view.model_uuid())
                                    else {
                                        continue;
                                    };
                                    w.move_element(&src.view.model_uuid(), new_pos.1);
                                    undo_uuids.insert(*src.view.uuid());
                                    std::mem::swap(dest, *src);
                                }
                            }
                        }
                        if let Some((mi, vi)) = remainder {
                            undo_uuids.insert(*vi.view.uuid());
                            match direction {
                                DemoPsdOrdinalMovement::StateLeft => {
                                    self.model.write().before.push(mi);
                                    self.before_views.push(vi);
                                }
                                DemoPsdOrdinalMovement::StateRight => {
                                    self.model.write().after.insert(0, mi);
                                    self.after_views.insert(0, vi);
                                }
                                DemoPsdOrdinalMovement::StateUp
                                | DemoPsdOrdinalMovement::StateDown => unreachable!(),
                            }
                        }
                    }
                }

                if !undo_uuids.is_empty() {
                    undo_accumulator.push(InsensitiveCommand::MoveOrdinal(
                        undo_uuids,
                        direction.inverse(),
                    ));
                    affected_models.insert(*self.model_uuid());
                }

                recurse!();
            }
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        DemoPsdPropChange::TransactionKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::TransactionKindChange(model.kind),
                            ));
                            model.kind = *kind;
                        }
                        DemoPsdPropChange::IdentifierChange(identifier) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::IdentifierChange(model.identifier.clone()),
                            ));
                            model.identifier = identifier.clone();
                        }
                        DemoPsdPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        DemoPsdPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
                        }
                        DemoPsdPropChange::TransactionPercentageChange(percentage) => {
                            let w = 25.0 / self.tx_outer_rectangle.width();
                            let new_percentage = percentage.clamp(w, 1.0 - w);

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::TransactionPercentageChange(
                                    self.tx_mark_percentage,
                                ),
                            ));
                            self.tx_mark_percentage = new_percentage;
                        }
                        _ => {}
                    }
                }
                recurse!();
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.kind_buffer = model.kind;
        self.identifier_buffer = (*model.identifier).clone();
        self.comment_buffer = (*model.comment).clone();

        // Structural refresh
        let views_map = self
            .before_views
            .iter()
            .map(|e| (*e.view.model_uuid(), e.view.clone()))
            .chain(
                self.p_act_view
                    .as_ref()
                    .map(|e| (*e.read().model_uuid(), e.clone().into())),
            )
            .chain(
                self.after_views
                    .iter()
                    .map(|e| (*e.view.model_uuid(), e.view.clone())),
            )
            .collect::<HashMap<_, _>>();
        self.before_views = model
            .before
            .iter()
            .flat_map(|e1| {
                views_map
                    .get(&e1.state.uuid())
                    .map(|e2| DemoPsdStateViewInfo {
                        view: e2.clone(),
                        executor: e1.executor,
                    })
            })
            .collect();
        self.p_act_view = model
            .p_act
            .as_ref()
            .and_then(|e| {
                views_map.get(&e.read().uuid).map(|e| match e {
                    DemoPsdStateView::Fact(_) => panic!(),
                    DemoPsdStateView::Act(inner) => inner.clone(),
                })
            })
            .into();
        self.after_views = model
            .after
            .iter()
            .flat_map(|e1| {
                views_map
                    .get(&e1.state.uuid())
                    .map(|e2| DemoPsdStateViewInfo {
                        view: e2.clone(),
                        executor: e1.executor,
                    })
            })
            .collect();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (DemoPsdElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        let mut flattened_status_temp = HashMap::new();
        for e in &mut self.before_views {
            e.view.head_count(
                flattened_views,
                &mut flattened_status_temp,
                flattened_represented_models,
            );
            flattened_views.insert(
                *e.view.uuid(),
                (e.view.clone().as_element_view(), *self.uuid),
            );
        }
        if let UFOption::Some(e) = &self.p_act_view {
            let mut w = e.write();
            w.head_count(
                flattened_views,
                &mut flattened_status_temp,
                flattened_represented_models,
            );
            flattened_views.insert(*w.uuid(), (e.clone().into(), *self.uuid));
        }
        for e in &mut self.after_views {
            e.view.head_count(
                flattened_views,
                &mut flattened_status_temp,
                flattened_represented_models,
            );
            flattened_views.insert(
                *e.view.uuid(),
                (e.view.clone().as_element_view(), *self.uuid),
            );
        }

        flattened_status_temp.iter().for_each(|e| {
            let s = match e.1 {
                SelectionStatus::NotSelected if self.highlight.selected => {
                    SelectionStatus::TransitivelySelected
                }
                a => *a,
            };
            flattened_views_status.insert(*e.0, s);
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <DemoPsdDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <DemoPsdDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <DemoPsdDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            for e in &self.before_views {
                e.view.deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
            if let UFOption::Some(act) = &self.p_act_view {
                act.read()
                    .deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
            for e in &self.after_views {
                e.view.deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoPsdElementView>,
        c: &mut HashMap<ViewUuid, DemoPsdElementView>,
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) {
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoPsdElement::Transaction(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let new_before_views = self
            .before_views
            .iter()
            .map(|e| {
                e.view
                    .deep_copy_clone(uuid_present, &mut HashMap::new(), c, m);
                DemoPsdStateViewInfo {
                    view: c
                        .get(&e.view.uuid())
                        .and_then(|e| e.clone().as_state_view())
                        .unwrap(),
                    executor: e.executor,
                }
            })
            .collect();
        let new_p_act_view = if let UFOption::Some(e) = &self.p_act_view {
            e.write()
                .deep_copy_clone(uuid_present, &mut HashMap::new(), c, m);
            if let Some(DemoPsdElementView::Act(e)) = c.get(&e.read().uuid()) {
                Some(e.clone())
            } else {
                None
            }
        } else {
            None
        }
        .into();
        let new_after_views = self
            .after_views
            .iter()
            .map(|e| {
                e.view
                    .deep_copy_clone(uuid_present, &mut HashMap::new(), c, m);
                DemoPsdStateViewInfo {
                    view: c
                        .get(&e.view.uuid())
                        .and_then(|e| e.clone().as_state_view())
                        .unwrap(),
                    executor: e.executor,
                }
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,

            before_views: new_before_views,
            p_act_view: new_p_act_view,
            after_views: new_after_views,
            selected_direct_elements: self.selected_direct_elements.clone(),

            kind_buffer: self.kind_buffer,
            identifier_buffer: self.identifier_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_rect: None,
            highlight: self.highlight,
            tx_outer_rectangle: self.tx_outer_rectangle,
            tx_mark_percentage: self.tx_mark_percentage,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_demopsd_fact(
    identifier: &str,
    internal: bool,
    position: egui::Pos2,
) -> (ERef<DemoPsdFact>, ERef<DemoPsdFactView>) {
    let model = ERef::new(DemoPsdFact::new(
        ModelUuid::now_v7(),
        identifier.to_owned(),
        internal,
    ));
    let view = new_demopsd_fact_view(model.clone(), position);
    (model, view)
}
fn new_demopsd_fact_view(model: ERef<DemoPsdFact>, position: egui::Pos2) -> ERef<DemoPsdFactView> {
    let r = model.read();
    ERef::new(DemoPsdFactView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        identifier_buffer: (*r.identifier).clone(),
        internal_buffer: r.internal,
        comment_buffer: (*r.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdFactView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdFact>,

    #[nh_context_serde(skip_and_default)]
    identifier_buffer: String,
    #[nh_context_serde(skip_and_default)]
    internal_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<canvas::NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    position: egui::Pos2,
}

impl DemoPsdFactView {
    const RADIUS: egui::Vec2 = egui::Vec2::splat(7.0);

    fn draw_inner(
        &mut self,
        _q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &DemoPsdSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
        pos: egui::Pos2,
        text_align: egui::Align2,
    ) -> TargettingStatus {
        let read = self.model.read();

        self.position = pos;

        canvas.draw_ellipse(
            self.position,
            Self::RADIUS,
            if read.internal {
                INTERNAL_ROLE_BACKGROUND
            } else {
                EXTERNAL_ROLE_BACKGROUND
            },
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_text(
            self.position
                + egui::Vec2::new(
                    match text_align.x() {
                        egui::Align::Min => 1.5 * Self::RADIUS.x,
                        egui::Align::Center => 0.0,
                        egui::Align::Max => -1.5 * Self::RADIUS.x,
                    },
                    match text_align.y() {
                        egui::Align::Min => Self::RADIUS.y,
                        egui::Align::Center => 0.0,
                        egui::Align::Max => -Self::RADIUS.y,
                    },
                ),
            text_align,
            &read.identifier,
            canvas::CLASS_BOTTOM_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw targetting rectangle
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
        {
            canvas.draw_ellipse(
                self.position,
                Self::RADIUS,
                t.targetting_for_section(Ok(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            TargettingStatus::Drawn
        } else {
            TargettingStatus::NotDrawn
        }
    }
}

impl Entity for DemoPsdFactView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for DemoPsdFactView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
}

impl ElementController<DemoPsdElement> for DemoPsdFactView {
    fn model(&self) -> DemoPsdElement {
        self.model.clone().into()
    }
    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Ellipse {
            position: self.position,
            bounds_radius: Self::RADIUS,
        }
    }
    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdFactView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> PropertiesStatus<DemoPsdDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Identifier:", &mut self.identifier_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ));
        }

        if ui.checkbox(&mut self.internal_buffer, "internal").changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::StateInternalChange(self.internal_buffer),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateDown,
                ));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Move left").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateLeft,
                ));
            }
            if ui.button("Move right").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateRight,
                ));
            }
        });

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position;

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(x - self.position.x, 0.0),
                ));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(0.0, y - self.position.y),
                ));
            }
        });

        PropertiesStatus::Shown
    }
    fn draw_in(
        &mut self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &DemoPsdSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
    ) -> TargettingStatus {
        self.draw_inner(
            q,
            context,
            settings,
            canvas,
            tool,
            self.position,
            egui::Align2::LEFT_CENTER,
        )
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _settings: &<DemoPsdDomain as Domain>::SettingsT,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveDemoPsdTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Drag { delta, .. } if self.dragged_shape.is_some() => {
                let translated_real_shape = self.dragged_shape.unwrap().translate(delta);
                self.dragged_shape = Some(translated_real_shape);
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| {
                        !ehc.all_elements
                            .get(e)
                            .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.position;

                if self.highlight.selected {
                    commands.push(InsensitiveCommand::MovePositional(
                        q.selected_views(),
                        coerced_delta,
                    ));
                } else {
                    commands.push(InsensitiveCommand::MovePositional(
                        std::iter::once(*self.uuid).collect(),
                        coerced_delta,
                    ));
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                }
                EventHandlingStatus::NotHandled
            }
            e if !self.min_shape().contains(*e.mouse_position()) => EventHandlingStatus::NotHandled,
            InputEvent::MouseDown(_) => {
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(_) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model.clone().into());
                } else {
                    if ehc
                        .modifier_settings
                        .hold_selection
                        .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
                        commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED));
                        commands.push(InsensitiveCommand::HighlightSpecific(
                            std::iter::once(*self.uuid).collect(),
                            true,
                            Highlight::SELECTED,
                        ));
                    } else {
                        commands.push(InsensitiveCommand::HighlightSpecific(
                            std::iter::once(*self.uuid).collect(),
                            !self.highlight.selected,
                            Highlight::SELECTED,
                        ));
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<DemoPsdDiagram>,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.highlight.selected = (self.highlight.selected && *retain)
                    || self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeElementsBy(..)
            | InsensitiveCommand::ResizeElementTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddDependency { .. }
            | InsensitiveCommand::RemoveDependency { .. }
            | InsensitiveCommand::ArrangeSpecificElements(..)
            | InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        DemoPsdPropChange::IdentifierChange(identifier) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::IdentifierChange(model.identifier.clone()),
                            ));
                            model.identifier = identifier.clone();
                        }
                        DemoPsdPropChange::StateInternalChange(internal) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::StateInternalChange(model.internal),
                            ));
                            model.internal = *internal;
                        }
                        DemoPsdPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
                        }
                        _ => {}
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.identifier_buffer = (*model.identifier).clone();
        self.internal_buffer = model.internal;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (DemoPsdElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoPsdElementView>,
        c: &mut HashMap<ViewUuid, DemoPsdElementView>,
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) {
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoPsdElement::Fact(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            identifier_buffer: self.identifier_buffer.clone(),
            internal_buffer: self.internal_buffer,
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_demopsd_act(
    identifier: &str,
    internal: bool,
    position: egui::Pos2,
) -> (ERef<DemoPsdAct>, ERef<DemoPsdActView>) {
    let model = ERef::new(DemoPsdAct::new(
        ModelUuid::now_v7(),
        identifier.to_owned(),
        internal,
    ));
    let view = new_demopsd_act_view(model.clone(), position);
    (model, view)
}
fn new_demopsd_act_view(model: ERef<DemoPsdAct>, position: egui::Pos2) -> ERef<DemoPsdActView> {
    let r = model.read();
    ERef::new(DemoPsdActView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        identifier_buffer: (*r.identifier).clone(),
        internal_buffer: r.internal,
        comment_buffer: (*r.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::from_center_size(position, DemoPsdActView::SIZE),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdActView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdAct>,

    #[nh_context_serde(skip_and_default)]
    identifier_buffer: String,
    #[nh_context_serde(skip_and_default)]
    internal_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<canvas::NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,
}

impl DemoPsdActView {
    const SIZE: egui::Vec2 = egui::Vec2::splat(2.0 * DemoPsdFactView::RADIUS.x);

    fn draw_inner(
        &mut self,
        _q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &DemoPsdSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
        pos: egui::Pos2,
        text_align: egui::Align2,
    ) -> TargettingStatus {
        let read = self.model.read();

        self.bounds_rect = egui::Rect::from_center_size(pos, Self::SIZE);

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            if read.internal {
                INTERNAL_ROLE_BACKGROUND
            } else {
                EXTERNAL_ROLE_BACKGROUND
            },
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_text(
            pos + egui::Vec2::new(
                match text_align.x() {
                    egui::Align::Min => 2.0 * Self::SIZE.x / 3.0,
                    egui::Align::Center => 0.0,
                    egui::Align::Max => -2.0 * Self::SIZE.x / 3.0,
                },
                match text_align.y() {
                    egui::Align::Min => Self::SIZE.y / 2.0,
                    egui::Align::Center => 0.0,
                    egui::Align::Max => -Self::SIZE.y / 2.0,
                },
            ),
            text_align,
            &read.identifier,
            canvas::CLASS_BOTTOM_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw targetting rectangle
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                t.targetting_for_section(Ok(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            TargettingStatus::Drawn
        } else {
            TargettingStatus::NotDrawn
        }
    }
}

impl Entity for DemoPsdActView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for DemoPsdActView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
}

impl ElementController<DemoPsdElement> for DemoPsdActView {
    fn model(&self) -> DemoPsdElement {
        self.model.clone().into()
    }
    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Rect {
            inner: self.bounds_rect,
        }
    }
    fn position(&self) -> egui::Pos2 {
        self.bounds_rect.center()
    }
}

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdActView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> PropertiesStatus<DemoPsdDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Identifier:", &mut self.identifier_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ));
        }

        if ui.checkbox(&mut self.internal_buffer, "internal").changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::StateInternalChange(self.internal_buffer),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateDown,
                ));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Move left").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateLeft,
                ));
            }
            if ui.button("Move right").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    DemoPsdOrdinalMovement::StateRight,
                ));
            }
        });

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position();

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(x - self.position().x, 0.0),
                ));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(0.0, y - self.position().y),
                ));
            }
        });

        PropertiesStatus::Shown
    }
    fn draw_in(
        &mut self,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &DemoPsdSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
    ) -> TargettingStatus {
        self.draw_inner(
            q,
            context,
            settings,
            canvas,
            tool,
            self.bounds_rect.center(),
            egui::Align2::LEFT_CENTER,
        )
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _settings: &<DemoPsdDomain as Domain>::SettingsT,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveDemoPsdTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Drag { delta, .. } if self.dragged_shape.is_some() => {
                let translated_real_shape = self.dragged_shape.unwrap().translate(delta);
                self.dragged_shape = Some(translated_real_shape);
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| {
                        !ehc.all_elements
                            .get(e)
                            .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.bounds_rect.center();

                if self.highlight.selected {
                    commands.push(InsensitiveCommand::MovePositional(
                        q.selected_views(),
                        coerced_delta,
                    ));
                } else {
                    commands.push(InsensitiveCommand::MovePositional(
                        std::iter::once(*self.uuid).collect(),
                        coerced_delta,
                    ));
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                }
                EventHandlingStatus::NotHandled
            }
            e if !self.min_shape().contains(*e.mouse_position()) => EventHandlingStatus::NotHandled,
            InputEvent::MouseDown(_) => {
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(_) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model.clone().into());
                } else {
                    if ehc
                        .modifier_settings
                        .hold_selection
                        .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
                        commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED));
                        commands.push(InsensitiveCommand::HighlightSpecific(
                            std::iter::once(*self.uuid).collect(),
                            true,
                            Highlight::SELECTED,
                        ));
                    } else {
                        commands.push(InsensitiveCommand::HighlightSpecific(
                            std::iter::once(*self.uuid).collect(),
                            !self.highlight.selected,
                            Highlight::SELECTED,
                        ));
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<DemoPsdDiagram>,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.highlight.selected = (self.highlight.selected && *retain)
                    || self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.bounds_rect = self.bounds_rect.translate(*delta);
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeElementsBy(..)
            | InsensitiveCommand::ResizeElementTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddDependency { .. }
            | InsensitiveCommand::RemoveDependency { .. }
            | InsensitiveCommand::ArrangeSpecificElements(..)
            | InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        DemoPsdPropChange::IdentifierChange(identifier) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::IdentifierChange(model.identifier.clone()),
                            ));
                            model.identifier = identifier.clone();
                        }
                        DemoPsdPropChange::StateInternalChange(internal) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::StateInternalChange(model.internal),
                            ));
                            model.internal = *internal;
                        }
                        DemoPsdPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
                        }
                        _ => {}
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.identifier_buffer = (*model.identifier).clone();
        self.internal_buffer = model.internal;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (DemoPsdElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoPsdElementView>,
        c: &mut HashMap<ViewUuid, DemoPsdElementView>,
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) {
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoPsdElement::Act(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            identifier_buffer: self.identifier_buffer.clone(),
            internal_buffer: self.internal_buffer,
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_demopsd_link(
    link_type: DemoPsdLinkType,
    multiplicity: &str,
    source: (ERef<DemoPsdFact>, DemoPsdElementView),
    target: (ERef<DemoPsdAct>, DemoPsdElementView),
    center_point: Option<(ViewUuid, egui::Pos2)>,
) -> (ERef<DemoPsdLink>, ERef<LinkViewT>) {
    let link_model = ERef::new(DemoPsdLink::new(
        ModelUuid::now_v7(),
        link_type,
        multiplicity.to_owned().into(),
        source.0,
        target.0,
    ));
    let link_view = new_demopsd_link_view(link_model.clone(), source.1, target.1, center_point);
    (link_model, link_view)
}
fn new_demopsd_link_view(
    model: ERef<DemoPsdLink>,
    source: DemoPsdElementView,
    target: DemoPsdElementView,
    center_point: Option<(ViewUuid, egui::Pos2)>,
) -> ERef<LinkViewT> {
    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        DemoPsdLinkAdapter {
            model,
            temporaries: Default::default(),
        },
        vec![Ending::new(source)],
        vec![Ending::new(target)],
        center_point,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct DemoPsdLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdLink>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: DemoPsdLinkTemporaries,
}

#[derive(Clone, Default)]
struct DemoPsdLinkTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    link_type_buffer: DemoPsdLinkType,
    multiplicity_buffer: String,
    comment_buffer: String,
}

impl DemoPsdLinkAdapter {
    fn line_type(&self) -> canvas::LineType {
        match self.model.read().link_type {
            DemoPsdLinkType::ResponseLink => canvas::LineType::Solid,
            DemoPsdLinkType::WaitLink => canvas::LineType::Dashed,
        }
    }
}

impl MulticonnectionAdapter<DemoPsdDomain> for DemoPsdLinkAdapter {
    fn model(&self) -> DemoPsdElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn arrow_data(&self) -> &HashMap<(bool, ModelUuid), ArrowData> {
        &self.temporaries.arrow_data
    }

    fn draw_center_or_get_label(
        &self,
        _sources: &Vec<Ending<DemoPsdElementView>>,
        _targets: &Vec<Ending<DemoPsdElementView>>,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<DemoPsdDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<DemoPsdDomain as Domain>::ToolT)>,
    ) -> Result<(), Arc<String>> {
        Err(self.model.read().multiplicity.clone())
    }

    fn source_uuids(&self) -> &[ModelUuid] {
        &self.temporaries.source_uuids
    }

    fn target_uuids(&self) -> &[ModelUuid] {
        &self.temporaries.target_uuids
    }

    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> PropertiesStatus<DemoPsdDomain> {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("link type")
            .selected_text(self.temporaries.link_type_buffer.as_str())
            .show_ui(ui, |ui| {
                for value in DemoPsdLinkType::VARIANTS {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.link_type_buffer,
                            value,
                            value.as_str(),
                        )
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            DemoPsdPropChange::LinkTypeChange(self.temporaries.link_type_buffer),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline(
                "Multiplicity:",
                &mut self.temporaries.multiplicity_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::LinkMultiplicityChange(Arc::new(
                    self.temporaries.multiplicity_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                DemoPsdPropChange::LinkTypeChange(link_type) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::LinkTypeChange(model.link_type),
                    ));
                    model.link_type = *link_type;
                }
                DemoPsdPropChange::LinkMultiplicityChange(multiplicity) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::LinkMultiplicityChange(model.multiplicity.clone()),
                    ));
                    model.multiplicity = multiplicity.clone();
                }
                DemoPsdPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        DemoPsdPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(
        &mut self,
        _sources: &Vec<Ending<DemoPsdElementView>>,
        _targets: &Vec<Ending<DemoPsdElementView>>,
    ) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        let line_type = self.line_type();
        self.temporaries.arrow_data.insert(
            (false, *model.source.read().uuid),
            ArrowData::new_labelless(line_type, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.read().uuid),
            ArrowData::new_labelless(line_type, canvas::ArrowheadType::FullTriangle),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries
            .source_uuids
            .push(*model.source.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries
            .target_uuids
            .push(*model.target.read().uuid);

        self.temporaries.link_type_buffer = model.link_type;
        self.temporaries.multiplicity_buffer = (*model.multiplicity).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let model = self.model.read();
        let model = if let Some(DemoPsdElement::Link(m)) = m.get(&model.uuid) {
            m.clone()
        } else {
            model.deep_copy_clone_inner(new_uuid, m)
        };
        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, DemoPsdElement>) {
        self.model.write().deep_copy_relink(m);
    }
}

pub fn new_demopsd_note(
    text: &str,
    position: egui::Pos2,
    align: egui::Align2,
    background_color: MGlobalColor,
) -> (ERef<DemoPsdNote>, ERef<DemoPsdNoteView>) {
    let model = ERef::new(DemoPsdNote::new(ModelUuid::now_v7(), text.to_owned()));
    let view = new_demopsd_note_view(model.clone(), position, align, background_color);

    (model, view)
}
pub fn new_demopsd_note_view(
    model: ERef<DemoPsdNote>,
    position: egui::Pos2,
    align: egui::Align2,
    background_color: MGlobalColor,
) -> ERef<DemoPsdNoteView> {
    let m = model.read();
    ERef::new(DemoPsdNoteView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        text_buffer: (*m.text).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        align,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct DemoPsdNoteView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<DemoPsdNote>,

    #[nh_context_serde(skip_and_default)]
    text_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub align: egui::Align2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl DemoPsdNoteView {
    const CORNER_SIZE: f32 = 10.0;
}

impl Entity for DemoPsdNoteView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for DemoPsdNoteView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<DemoPsdElement> for DemoPsdNoteView {
    fn model(&self) -> DemoPsdElement {
        self.model.clone().into()
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

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdNoteView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> PropertiesStatus<DemoPsdDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_multiline("Text:", &mut self.text_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::NameChange(Arc::new(self.text_buffer.clone())),
            ));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position;

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(x - self.position.x, 0.0),
                ));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(0.0, y - self.position.y),
                ));
            }
        });

        egui::ComboBox::new("horizontal align", "Horizontal align")
            .selected_text(format!("{:?}", self.align.x()))
            .show_ui(ui, |ui| {
                let mut tmp_x = self.align.x();
                for e in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                    if ui
                        .selectable_value(&mut tmp_x, e, format!("{:?}", e))
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            DemoPsdPropChange::NoteAlignChange(Some(tmp_x), None),
                        ));
                    }
                }
            });
        egui::ComboBox::new("vertical align", "Vertical align")
            .selected_text(format!("{:?}", self.align.y()))
            .show_ui(ui, |ui| {
                let mut tmp_y = self.align.y();
                for e in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                    if ui
                        .selectable_value(&mut tmp_y, e, format!("{:?}", e))
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            DemoPsdPropChange::NoteAlignChange(None, Some(tmp_y)),
                        ));
                    }
                }
            });

        ui.label("Background color:");
        if let Some(new_color) =
            crate::common::controller::mglobalcolor_edit_button(gdc, ui, &self.background_color)
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                DemoPsdPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<DemoPsdDomain as Domain>::SettingsT,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
    ) -> TargettingStatus {
        let align_offset = egui::Vec2 {
            x: match self.align.x() {
                egui::Align::Min => -Self::CORNER_SIZE,
                egui::Align::Center => 0.0,
                egui::Align::Max => Self::CORNER_SIZE,
            },
            y: match self.align.y() {
                egui::Align::Min => Self::CORNER_SIZE,
                egui::Align::Center => 0.0,
                egui::Align::Max => -Self::CORNER_SIZE,
            },
        };
        self.bounds_rect = canvas
            .measure_text(
                self.position,
                self.align,
                &self.text_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .expand2(egui::Vec2 {
                x: Self::CORNER_SIZE,
                y: Self::CORNER_SIZE,
            })
            .translate(align_offset);

        canvas.draw_polygon(
            [
                self.bounds_rect.min,
                egui::Pos2::new(self.bounds_rect.min.x, self.bounds_rect.max.y),
                self.bounds_rect.max,
                egui::Pos2::new(
                    self.bounds_rect.max.x,
                    self.bounds_rect.min.y + Self::CORNER_SIZE,
                ),
                egui::Pos2::new(
                    self.bounds_rect.max.x - Self::CORNER_SIZE,
                    self.bounds_rect.min.y,
                ),
            ]
            .into_iter()
            .collect(),
            context
                .global_colors
                .get(&self.background_color)
                .unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_polygon(
            [
                egui::Pos2::new(
                    self.bounds_rect.max.x,
                    self.bounds_rect.min.y + Self::CORNER_SIZE,
                ),
                egui::Pos2::new(
                    self.bounds_rect.max.x - Self::CORNER_SIZE,
                    self.bounds_rect.min.y + Self::CORNER_SIZE,
                ),
                egui::Pos2::new(
                    self.bounds_rect.max.x - Self::CORNER_SIZE,
                    self.bounds_rect.min.y,
                ),
            ]
            .into_iter()
            .collect(),
            context
                .global_colors
                .get(&self.background_color)
                .unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_text(
            self.position + align_offset,
            self.align,
            &self.text_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        if canvas.ui_scale().is_some() {
            if self.dragged_shape.is_some() {
                canvas.draw_line(
                    [
                        egui::Pos2::new(self.bounds_rect.min.x, self.bounds_rect.center().y),
                        egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.center().y),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLUE),
                    canvas::Highlight::NONE,
                );
                canvas.draw_line(
                    [
                        egui::Pos2::new(self.bounds_rect.center().x, self.bounds_rect.min.y),
                        egui::Pos2::new(self.bounds_rect.center().x, self.bounds_rect.max.y),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLUE),
                    canvas::Highlight::NONE,
                );
            }

            // Draw targetting rectangle
            if let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
            {
                canvas.draw_polygon(
                    [
                        self.bounds_rect.min,
                        egui::Pos2::new(self.bounds_rect.min.x, self.bounds_rect.max.y),
                        self.bounds_rect.max,
                        egui::Pos2::new(
                            self.bounds_rect.max.x,
                            self.bounds_rect.min.y + Self::CORNER_SIZE,
                        ),
                        egui::Pos2::new(
                            self.bounds_rect.max.x - Self::CORNER_SIZE,
                            self.bounds_rect.min.y,
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    t.targetting_for_section(Ok(self.model.clone().into())),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                TargettingStatus::Drawn
            } else {
                TargettingStatus::NotDrawn
            }
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _settings: &<DemoPsdDomain as Domain>::SettingsT,
        q: &<DemoPsdDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveDemoPsdTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::MouseDown(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model.clone().into());
                } else {
                    if ehc
                        .modifier_settings
                        .hold_selection
                        .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
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
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| {
                        !ehc.all_elements
                            .get(e)
                            .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.bounds_rect.center();

                if self.highlight.selected {
                    commands.push(InsensitiveCommand::MovePositional(
                        q.selected_views(),
                        coerced_delta,
                    ));
                } else {
                    commands.push(InsensitiveCommand::MovePositional(
                        std::iter::once(*self.uuid).collect(),
                        coerced_delta,
                    ));
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<DemoPsdDiagram>,
        command: &InsensitiveCommand<
            DemoPsdOrdinalMovement,
            DemoPsdElementOrVertex,
            DemoPsdPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<DemoPsdOrdinalMovement, DemoPsdElementOrVertex, DemoPsdPropChange>,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.highlight.selected = (self.highlight.selected && *retain)
                    || self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeElementsBy(..)
            | InsensitiveCommand::ResizeElementTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddDependency { .. }
            | InsensitiveCommand::RemoveDependency { .. }
            | InsensitiveCommand::ArrangeSpecificElements(..)
            | InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        DemoPsdPropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        DemoPsdPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        DemoPsdPropChange::NoteAlignChange(x, y) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                DemoPsdPropChange::NoteAlignChange(
                                    Some(self.align.x()),
                                    Some(self.align.y()),
                                ),
                            ));
                            if let Some(x) = x {
                                self.align.0[0] = *x;
                            }
                            if let Some(y) = y {
                                self.align.0[1] = *y;
                            }
                        }
                        _ => {}
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.text_buffer = (*model.text).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (DemoPsdElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoPsdElementView>,
        c: &mut HashMap<ViewUuid, DemoPsdElementView>,
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoPsdElement::Note(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            text_buffer: self.text_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            align: self.align,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}
