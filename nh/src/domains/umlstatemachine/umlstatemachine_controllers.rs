use crate::common::canvas::{self, NHCanvas, NHIcon, NHShape};
use crate::common::controller::{
    ColorBundle, ColorChangeData, ControllerAdapter, DeleteKind, DiagramAdapter, DiagramController,
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
use crate::common::ui_ext::UiExt;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::views::multiconnection_view::{
    self, ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView,
    VertexInformation,
};
use crate::common::views::ordered_views::OrderedViews;
use crate::common::views::package_view::{PackageAdapter, PackageDragType, PackageView};
use crate::domains::umlstatemachine::umlstatemachine_models::{
    UmlStateMachine, UmlStateMachineCompositeState, UmlStateMachineCompositeStateRegion,
    UmlStateMachineDiagram, UmlStateMachineEdge, UmlStateMachineElement, UmlStateMachineFinalState,
    UmlStateMachineInitialPseudostate, UmlStateMachineInternalTransition,
    UmlStateMachineNonFinalNode, UmlStateMachineNonInitialNode, UmlStateMachineNote,
    UmlStateMachineNoteLink, UmlStateMachineStandaloneElement, UmlStateMachineTerminatePseudostate,
};
use crate::{
    CustomModal, DefaultNameF, DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF,
    DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::{egui, epaint};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct UmlStateMachineDomain;
impl Domain for UmlStateMachineDomain {
    type SettingsT = UmlStateMachineSettings;
    type CommonElementT = UmlStateMachineElement;
    type DiagramModelT = UmlStateMachineDiagram;
    type CommonElementViewT = UmlStateMachineElementView;
    type ViewTargettingSectionT = UmlStateMachineElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveUmlStateMachineTool;
    type OrdinalMovementT = UmlStateMachineOrdinalMovement;
    type AddCommandElementT = UmlStateMachineElementOrVertex;
    type PropChangeT = UmlStateMachinePropChange;
}

type StateMachineViewT = PackageView<UmlStateMachineDomain, UmlStateMachineAdapter>;
type EdgeViewT = MulticonnectionView<UmlStateMachineDomain, UmlStateMachineEdgeAdapter>;
type NoteLinkViewT = MulticonnectionView<UmlStateMachineDomain, UmlStateMachineNoteLinkAdapter>;

#[derive(Clone, Copy, Debug)]
pub enum UmlStateMachineOrdinalMovement {
    Up,
    Down,
}

impl UmlStateMachineOrdinalMovement {
    pub fn inverse(&self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

#[derive(Clone)]
pub enum UmlStateMachinePropChange {
    NameChange(Arc<String>),
    StereotypeChange(Arc<String>),

    TransitionGuardChange(Arc<String>),
    TransitionBehaviorChange(Arc<String>),
    StateMachineIsProtocolChange(bool),

    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    NoteAlignChange(Option<egui::Align>, Option<egui::Align>),
}

impl Debug for UmlStateMachinePropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlStateMachinePropChange::???")
    }
}

impl TryFrom<&UmlStateMachinePropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &UmlStateMachinePropChange) -> Result<Self, Self::Error> {
        match value {
            UmlStateMachinePropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for UmlStateMachinePropChange {
    fn from(value: ColorChangeData) -> Self {
        UmlStateMachinePropChange::ColorChange(value)
    }
}
impl TryFrom<UmlStateMachinePropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: UmlStateMachinePropChange) -> Result<Self, Self::Error> {
        match value {
            UmlStateMachinePropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for UmlStateMachinePropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::StereotypeChange(_), newer @ Self::StereotypeChange(_))
            | (Self::TransitionGuardChange(_), newer @ Self::TransitionGuardChange(_))
            | (Self::TransitionBehaviorChange(_), newer @ Self::TransitionBehaviorChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum UmlStateMachineElementOrVertex {
    Element(UmlStateMachineElementView),
    Vertex(VertexInformation),
}

impl Debug for UmlStateMachineElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlStateMachineElementOrVertex::???")
    }
}

impl TryFrom<UmlStateMachineElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: UmlStateMachineElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlStateMachineElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<UmlStateMachineElementOrVertex> for UmlStateMachineElementView {
    type Error = ();

    fn try_from(value: UmlStateMachineElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlStateMachineElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "UmlStateMachineDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlStateMachineElementView {
    StateMachine(ERef<StateMachineViewT>),
    CompositeState(ERef<UmlStateMachineCompositeStateView>),
    CompositeStateRegion(ERef<UmlStateMachineCompositeStateRegionView>),
    InternalTransition(ERef<UmlStateMachineInternalTransitionView>),
    InitialPseudostate(ERef<UmlStateMachineInitialPseudostateView>),
    TerminatePseudostate(ERef<UmlStateMachineTerminatePseudostateView>),
    FinalState(ERef<UmlStateMachineFinalStateView>),
    Edge(ERef<EdgeViewT>),
    Note(ERef<UmlStateMachineNoteView>),
    NoteLink(ERef<NoteLinkViewT>),
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlStateMachineControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachineDiagram>,
}

impl ControllerAdapter<UmlStateMachineDomain> for UmlStateMachineControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<UmlStateMachineDomain, UmlStateMachineDiagramAdapter>;

    fn model(&self) -> ERef<UmlStateMachineDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<UmlStateMachineDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "umlstatemachine"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::umlstatemachine_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, UmlStateMachineElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("UML Activity Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared UML Activity Diagram".to_owned().into(),
                UmlStateMachineDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlStateMachineDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachineDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: UmlStateMachineDiagramBuffer,
}

#[derive(Clone, Default)]
struct UmlStateMachineDiagramBuffer {
    name: String,
    comment: String,
}

impl UmlStateMachineDiagramAdapter {
    pub fn new(model: ERef<UmlStateMachineDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: UmlStateMachineDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<UmlStateMachineDomain> for UmlStateMachineDiagramAdapter {
    fn model(&self) -> ERef<UmlStateMachineDiagram> {
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
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        element: UmlStateMachineElement,
    ) -> Result<UmlStateMachineElementView, HashSet<ModelUuid>> {
        let v = match element {
            UmlStateMachineElement::StateMachine(inner) => new_umlstatemachine_statemachine_view(
                inner,
                egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0),
            )
            .into(),
            UmlStateMachineElement::CompositeState(inner) => {
                let r = inner.read();
                let internal_transition_views = r
                    .internal_transitions
                    .iter()
                    .map(|e| new_umlstatemachine_internaltransition_view(e.clone()))
                    .collect();
                let section_views: Result<Vec<_>, _> = r
                    .regions
                    .iter()
                    .map(|e| {
                        self.create_new_view_for(q, e.clone().into())
                            .map(|e| match e {
                                UmlStateMachineElementView::CompositeStateRegion(inner) => inner,
                                _ => unreachable!(),
                            })
                    })
                    .collect();
                new_umlstatemachine_compositestate_view(
                    inner.clone(),
                    internal_transition_views,
                    section_views?,
                    egui::Pos2::ZERO,
                    MGlobalColor::None,
                )
                .into()
            }
            UmlStateMachineElement::CompositeStateRegion(inner) => {
                new_umlstatemachine_compositestateregion_view(
                    inner,
                    egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0),
                )
                .into()
            }
            UmlStateMachineElement::InternalTransition(inner) => {
                new_umlstatemachine_internaltransition_view(inner).into()
            }
            UmlStateMachineElement::InitialPseudostate(inner) => {
                new_umlstatemachine_initialpseudostate_view(inner, egui::Pos2::ZERO).into()
            }
            UmlStateMachineElement::TerminatePseudostate(inner) => {
                new_umlstatemachine_terminatepseudostate_view(inner, egui::Pos2::ZERO).into()
            }
            UmlStateMachineElement::FinalState(inner) => {
                new_umlstatemachine_finalstate_view(inner, egui::Pos2::ZERO).into()
            }
            UmlStateMachineElement::Edge(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                new_umlstatemachine_edge_view(inner.clone(), None, source_view, target_view).into()
            }
            UmlStateMachineElement::Note(inner) => new_umlstatemachine_note_view(
                inner,
                egui::Pos2::ZERO,
                egui::Align2::CENTER_CENTER,
                MGlobalColor::None,
            )
            .into(),
            UmlStateMachineElement::NoteLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                new_umlstatemachine_notelink_view(inner.clone(), None, source_view, target_view)
                    .into()
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &UmlStateMachineElement) -> Arc<String> {
        match e {
            UmlStateMachineElement::StateMachine(inner) => {
                let r = inner.read();
                let mut s = "State Machine (".to_owned();
                s.push_str(&r.name);
                if !r.stereotype.is_empty() {
                    s.push('«');
                    s.push_str(&r.stereotype);
                    s.push('»');
                }
                s.push(')');
                s.into()
            }
            UmlStateMachineElement::CompositeState(inner) => {
                let r = inner.read();
                let mut s = match inner.read().regions.is_empty() {
                    false => "Composite State (".to_owned(),
                    true => "Simple State (".to_owned(),
                };
                s.push_str(&r.name);
                if !r.stereotype.is_empty() {
                    s.push('«');
                    s.push_str(&r.stereotype);
                    s.push('»');
                }
                s.push(')');
                s.into()
            }
            UmlStateMachineElement::CompositeStateRegion(_inner) => {
                "Composite State Region".to_owned().into()
            }
            UmlStateMachineElement::InternalTransition(inner) => {
                let r = inner.read();
                format!("Internal Transition ({})", r.trigger).into()
            }
            UmlStateMachineElement::InitialPseudostate(..) => {
                "Initial Pseudostate".to_owned().into()
            }
            UmlStateMachineElement::TerminatePseudostate(..) => {
                "Terminate Pseudostate".to_owned().into()
            }
            UmlStateMachineElement::FinalState(..) => "Final State".to_owned().into(),
            UmlStateMachineElement::Edge(inner) => {
                let r = inner.read();
                let mut s = String::new();
                s.push_str("Edge");
                if !r.name.is_empty() {
                    s.push_str(" (");
                    s.push_str(&r.name);
                    s.push(')');
                }
                Arc::new(s)
            }
            UmlStateMachineElement::Note(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Note".to_owned()
                } else {
                    format!("Note ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
            UmlStateMachineElement::NoteLink(_inner) => Arc::new("Note Link".to_string()),
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
    fn requested_headers(
        &self,
        settings: &UmlStateMachineSettings,
        (last_h, _): (u8, u8),
    ) -> (canvas::HeaderMode, canvas::HeaderMode) {
        let v = match settings.vertical_header {
            e @ (canvas::HeaderMode::Expanding(0) | canvas::HeaderMode::Compact) => e,
            canvas::HeaderMode::Expanding(max) => {
                canvas::HeaderMode::Expanding(last_h.clamp(1, max))
            }
        };
        (canvas::HeaderMode::Expanding(0), v)
    }
    fn show_view_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
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
                UmlStateMachinePropChange::ColorChange((0, new_color).into()),
            ));
        }
    }
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        _drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlStateMachinePropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlStateMachinePropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlStateMachinePropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlStateMachinePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                UmlStateMachinePropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::CommentChange(model.comment.clone()),
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
        settings: &UmlStateMachineSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveUmlStateMachineTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialUmlStateMachineElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, UmlStateMachineElement>) {
        let (new_model, models) =
            super::umlstatemachine_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }
    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, UmlStateMachineElement>) {
        let models = super::umlstatemachine_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
    fn top_sort_info(
        &self,
        m: &<UmlStateMachineDomain as Domain>::CommonElementT,
    ) -> crate::common::model::ModelTopSortInfo {
        super::umlstatemachine_models::top_sort_info(m)
    }
}

fn new_controlller(
    model: ERef<UmlStateMachineDiagram>,
    name: String,
    elements: Vec<UmlStateMachineElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            UmlStateMachineControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                UmlStateMachineDiagramAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let diagram = ERef::new(UmlStateMachineDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![],
    ));
    new_controlller(diagram, name.to_owned(), vec![])
}

pub fn demo(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    // From https://en.wikipedia.org/wiki/File:UML_state_machine_Fig2b.png

    let (initial1, initial1_view) =
        new_umlstatemachine_initialpseudostate(egui::Pos2::new(300.0, 275.0));

    let (operand1, operand1_view) = new_umlstatemachine_compositestate(
        "operand1",
        "",
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(450.0, 275.0),
        MGlobalColor::None,
    );
    let (opentered, opentered_view) = new_umlstatemachine_compositestate(
        "opEntered",
        "",
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(300.0, 400.0),
        MGlobalColor::None,
    );
    let (operand2, operand2_view) = new_umlstatemachine_compositestate(
        "operand2",
        "",
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(450.0, 525.0),
        MGlobalColor::None,
    );
    let (result, result_view) = new_umlstatemachine_compositestate(
        "result",
        "",
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(575.0, 400.0),
        MGlobalColor::None,
    );

    let (_e1, e1_view) = new_umlstatemachine_edge(
        "",
        None,
        (initial1.clone().into(), initial1_view.clone().into()),
        (operand1.clone().into(), operand1_view.clone().into()),
    );
    let (_e2, e2_view) = new_umlstatemachine_edge(
        "'+', '-', '*', '/'",
        None,
        (operand1.clone().into(), operand1_view.clone().into()),
        (opentered.clone().into(), opentered_view.clone().into()),
    );
    let (_e3, e3_view) = new_umlstatemachine_edge(
        "'0'..'9', '.'",
        None,
        (opentered.clone().into(), opentered_view.clone().into()),
        (operand2.clone().into(), operand2_view.clone().into()),
    );
    let (_e4, e4_view) = new_umlstatemachine_edge(
        "'='",
        None,
        (operand2.clone().into(), operand2_view.clone().into()),
        (result.clone().into(), result_view.clone().into()),
    );
    let (_e5, e5_view) = new_umlstatemachine_edge(
        "'+', '-', '*', '/'",
        None,
        (result.clone().into(), result_view.clone().into()),
        (opentered.clone().into(), opentered_view.clone().into()),
    );
    let (_e6, e6_view) = new_umlstatemachine_edge(
        "'0'..'9', '.'",
        None,
        (result.clone().into(), result_view.clone().into()),
        (operand1.clone().into(), operand1_view.clone().into()),
    );

    let (composite_region, composite_region_view) = new_umlstatemachine_compositestateregion(
        egui::Rect::from_x_y_ranges(200.0..=700.0, 200.0..=600.0),
    );
    let (composite, composite_view) = new_umlstatemachine_compositestate(
        "on",
        "",
        Vec::new(),
        vec![(composite_region, composite_region_view.clone())],
        egui::Pos2::ZERO,
        MGlobalColor::None,
    );

    let (initial2, initial2_view) =
        new_umlstatemachine_initialpseudostate(egui::Pos2::new(800.0, 150.0));
    let (r#final, final_view) = new_umlstatemachine_finalstate(egui::Pos2::new(800.0, 700.0));

    let (_e21, e21_view) = new_umlstatemachine_edge(
        "",
        None,
        (initial2.clone().into(), initial2_view.clone().into()),
        (composite.clone().into(), composite_view.clone().into()),
    );
    let (_e22, e22_view) = new_umlstatemachine_edge(
        "",
        None,
        (composite.clone().into(), composite_view.clone().into()),
        (r#final.clone().into(), final_view.clone().into()),
    );

    let (activity, activity_view) = new_umlstatemachine_statemachine(
        "Calculator",
        "",
        false,
        egui::Rect::from_x_y_ranges(100.0..=900.0, 100.0..=800.0),
    );

    let diagram = ERef::new(UmlStateMachineDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![activity.into()],
    ));

    {
        let mut w = activity_view.write();
        let activity_uuid = *w.uuid();
        let (mut u, mut a) = Default::default();
        for e in [
            composite_view.into(),
            initial2_view.clone().into(),
            final_view.into(),
            e21_view.into(),
            e22_view.into(),
        ] {
            w.apply_command(
                &diagram,
                &InsensitiveCommand::AddDependency {
                    target: activity_uuid,
                    bucket: 0,
                    position: None,
                    element: UmlStateMachineElementOrVertex::Element(e),
                    into_model: true,
                },
                &mut u,
                &mut a,
            );
        }
    }
    {
        let mut w = composite_region_view.write();
        let region = *w.uuid();
        let (mut u, mut a) = Default::default();
        for e in [
            initial1_view.into(),
            operand1_view.into(),
            opentered_view.into(),
            operand2_view.into(),
            result_view.into(),
            e1_view.into(),
            e2_view.into(),
            e3_view.into(),
            e4_view.into(),
            e5_view.into(),
            e6_view.into(),
        ] {
            w.apply_command(
                &diagram,
                &InsensitiveCommand::AddDependency {
                    target: region,
                    bucket: 0,
                    position: None,
                    element: UmlStateMachineElementOrVertex::Element(e),
                    into_model: true,
                },
                &mut u,
                &mut a,
            );
        }
    }

    new_controlller(diagram, name.to_owned(), vec![activity_view.into()])
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        UmlStateMachineDomain,
        UmlStateMachineControllerAdapter,
        DiagramControllerGen2<UmlStateMachineDomain, UmlStateMachineDiagramAdapter>,
    >>(&uuid)?)
}

pub struct UmlStateMachineSettings {
    palette: RwLock<ToolPalette<UmlStateMachineToolStage, UmlStateMachineDomain>>,
    palette_edit_buffer:
        RwLock<PaletteEditBuffer<UmlStateMachineToolStage, UmlStateMachineElementView>>,
    nonfinal_buttons: Vec<(usize, usize, &'static str, &'static NonFinalStateButtonF)>,
    compositestate_buttons: Vec<(usize, usize, &'static str, &'static NonFinalStateButtonF)>,
    vertical_header: canvas::HeaderMode,
}

impl DiagramSettings for UmlStateMachineSettings {
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
                            UmlStateMachineToolStage::StateMachineStart {
                                stereotype,
                                name,
                                is_protocol,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Stereotype", stereotype)
                                    .changed();
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();
                                modified |=
                                    columns[1].checkbox(is_protocol, "isProtocol").changed();
                            }
                            UmlStateMachineToolStage::CompositeStateStart {
                                stereotype,
                                name,
                                background_color,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Stereotype", stereotype)
                                    .changed();
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

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
                            UmlStateMachineToolStage::SimpleState {
                                stereotype,
                                name,
                                background_color,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Stereotype", stereotype)
                                    .changed();
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

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
                            UmlStateMachineToolStage::LinkStart {
                                link_type: LinkType::Edge { name },
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();
                            }
                            UmlStateMachineToolStage::Note {
                                stereotype,
                                text,
                                align,
                                background_color,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Stereotype", stereotype)
                                    .changed();
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
                            _ => {}
                        }

                        if modified {
                            *view = view_for_stage(tool);
                            w.set_from_buffer(buffer.clone());
                        }
                    }
                }
            });
        }

        ui.label("Vertical header style:");
        egui::ComboBox::from_id_salt("vertical header style")
            .selected_text(self.vertical_header.as_str())
            .show_ui(ui, |ui| {
                for e in [
                    canvas::HeaderMode::Expanding(0),
                    canvas::HeaderMode::Compact,
                    canvas::HeaderMode::Expanding(u8::MAX),
                ] {
                    ui.selectable_value(&mut self.vertical_header, e, e.as_str());
                }
            });

        self.show_reduced(gdc, ui);

        ret
    }
    fn show_reduced(&mut self, _gdc: &GlobalDrawingContext, _ui: &mut egui::Ui) {}
    fn clone_reduced(&self) -> Box<dyn DiagramSettings> {
        Box::new(Self {
            palette: ToolPalette::new(Vec::new()).into(),
            palette_edit_buffer: PaletteEditBuffer::None.into(),
            nonfinal_buttons: Vec::new(),
            compositestate_buttons: Vec::new(),
            vertical_header: canvas::HeaderMode::Compact,
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
        table.insert(
            "vertical_header".to_owned(),
            toml::Value::try_from(self.vertical_header).map_err(|_| ())?,
        );
        Ok(table.into())
    }
}
impl DiagramSettings2<UmlStateMachineDomain> for UmlStateMachineSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                GroupDisplayStyle,
                Vec<(
                    uuid::Uuid,
                    UmlStateMachineToolStage,
                    String,
                    UmlStateMachineElementView,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        ),
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

type NonFinalStateButtonF = dyn Fn(
    UmlStateMachineNonFinalNode,
) -> (
    UmlStateMachineToolStage,
    UmlStateMachineToolStage,
    PartialUmlStateMachineElement,
    bool,
);
mod buttons {
    use crate::domains::umlstatemachine::umlstatemachine_models::UmlStateMachineNonFinalNode;

    use super::*;
    use std::sync::LazyLock;

    fn nonfinal_edge(
        m: UmlStateMachineNonFinalNode,
    ) -> (
        UmlStateMachineToolStage,
        UmlStateMachineToolStage,
        PartialUmlStateMachineElement,
        bool,
    ) {
        let link_type = LinkType::Edge {
            name: "".to_owned(),
        };
        (
            UmlStateMachineToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlStateMachineToolStage::LinkEnd,
            PartialUmlStateMachineElement::Link {
                link_type,
                source: m,
                dest: None,
            },
            true,
        )
    }
    fn nonfinal_simple(
        m: UmlStateMachineNonFinalNode,
    ) -> (
        UmlStateMachineToolStage,
        UmlStateMachineToolStage,
        PartialUmlStateMachineElement,
        bool,
    ) {
        let stage = UmlStateMachineToolStage::SimpleState {
            stereotype: "".to_owned(),
            name: "Simple State".to_owned(),
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.uuid()),
        };
        (
            stage.clone(),
            stage,
            PartialUmlStateMachineElement::None,
            true,
        )
    }
    fn nonfinal_terminate(
        m: UmlStateMachineNonFinalNode,
    ) -> (
        UmlStateMachineToolStage,
        UmlStateMachineToolStage,
        PartialUmlStateMachineElement,
        bool,
    ) {
        let stage = UmlStateMachineToolStage::TerminatePseudostate {
            with_edge_from: Some(*m.uuid()),
        };
        (
            stage.clone(),
            stage,
            PartialUmlStateMachineElement::None,
            true,
        )
    }
    fn nonfinal_final(
        m: UmlStateMachineNonFinalNode,
    ) -> (
        UmlStateMachineToolStage,
        UmlStateMachineToolStage,
        PartialUmlStateMachineElement,
        bool,
    ) {
        let stage = UmlStateMachineToolStage::FinalState {
            with_edge_from: Some(*m.uuid()),
        };
        (
            stage.clone(),
            stage,
            PartialUmlStateMachineElement::None,
            true,
        )
    }
    pub const NONFINAL_BUTTONS: LazyLock<
        Vec<(usize, usize, &'static str, &'static NonFinalStateButtonF)>,
    > = LazyLock::new(|| {
        vec![
            (0, 0, "↘", &nonfinal_edge as &NonFinalStateButtonF),
            (1, 0, "S", &nonfinal_simple as &NonFinalStateButtonF),
            (2, 0, "⊗", &nonfinal_terminate as &NonFinalStateButtonF),
            (
                2,
                1,
                "◎", // Does not work: ⊙⊚⨀⨁⨂◉⯄
                &nonfinal_final as &NonFinalStateButtonF,
            ),
        ]
    });

    fn state_internaltransition(
        _m: UmlStateMachineNonFinalNode,
    ) -> (
        UmlStateMachineToolStage,
        UmlStateMachineToolStage,
        PartialUmlStateMachineElement,
        bool,
    ) {
        let stage = UmlStateMachineToolStage::InternalTransition {
            trigger: "entry".to_owned(),
            guard: "".to_owned(),
            behavior: "doThing()".to_owned(),
        };
        (
            stage.clone(),
            stage,
            PartialUmlStateMachineElement::None,
            false,
        )
    }
    fn compositestate_region(
        _m: UmlStateMachineNonFinalNode,
    ) -> (
        UmlStateMachineToolStage,
        UmlStateMachineToolStage,
        PartialUmlStateMachineElement,
        bool,
    ) {
        let stage = UmlStateMachineToolStage::CompositeStateRegion {};
        (
            stage.clone(),
            stage,
            PartialUmlStateMachineElement::None,
            false,
        )
    }
    pub const COMPOSITE_STATE_BUTTONS: LazyLock<
        Vec<(usize, usize, &'static str, &'static NonFinalStateButtonF)>,
    > = LazyLock::new(|| {
        vec![
            (
                3,
                0,
                "T",
                &state_internaltransition as &NonFinalStateButtonF,
            ),
            (3, 1, "R", &compositestate_region as &NonFinalStateButtonF),
        ]
    });
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let palette_items = vec![
        (
            "States",
            vec![
                (
                    UmlStateMachineToolStage::SimpleState {
                        stereotype: "".to_owned(),
                        name: "Simple State".to_owned(),
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Simple State",
                    None,
                ),
                (
                    UmlStateMachineToolStage::CompositeStateStart {
                        stereotype: "".to_owned(),
                        name: "Composite State".to_owned(),
                        background_color: MGlobalColor::None,
                    },
                    "Composite State",
                    None,
                ),
                (
                    UmlStateMachineToolStage::InitialPseudostate {},
                    "Initial Pseudostate",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num1,
                    )),
                ),
                (
                    UmlStateMachineToolStage::TerminatePseudostate {
                        with_edge_from: None,
                    },
                    "Terminate Pseudostate",
                    None,
                ),
                (
                    UmlStateMachineToolStage::FinalState {
                        with_edge_from: None,
                    },
                    "Final State",
                    None,
                ),
            ],
        ),
        (
            "Relationships",
            vec![(
                UmlStateMachineToolStage::LinkStart {
                    link_type: LinkType::Edge {
                        name: "".to_owned(),
                    },
                },
                "Edge",
                Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Num2,
                )),
            )],
        ),
        (
            "State Machines",
            vec![(
                UmlStateMachineToolStage::StateMachineStart {
                    name: "state machine".to_owned(),
                    stereotype: "".to_owned(),
                    is_protocol: false,
                },
                "State Machine",
                Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Num3,
                )),
            )],
        ),
        (
            "Other",
            vec![
                (
                    UmlStateMachineToolStage::Note {
                        stereotype: "".to_owned(),
                        text: "a note".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                        background_color: MGlobalColor::None,
                    },
                    "Note",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num9,
                    )),
                ),
                (
                    UmlStateMachineToolStage::NoteLinkStart {},
                    "Note Link",
                    None,
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
                    (e.0, e.1, v, e.2)
                })
                .collect(),
        )
    })
    .collect();

    Box::new(UmlStateMachineSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
        nonfinal_buttons: buttons::NONFINAL_BUTTONS.clone(),
        compositestate_buttons: buttons::NONFINAL_BUTTONS
            .iter()
            .cloned()
            .chain(buttons::COMPOSITE_STATE_BUTTONS.iter().cloned())
            .collect(),
        vertical_header: canvas::HeaderMode::Compact,
    })
}

fn view_for_stage(s: &UmlStateMachineToolStage) -> UmlStateMachineElementView {
    match s {
        UmlStateMachineToolStage::SimpleState {
            stereotype,
            name,
            background_color,
            with_edge_from: _,
        } => {
            let view = new_umlstatemachine_compositestate(
                name,
                stereotype,
                Vec::new(),
                Vec::new(),
                egui::Pos2::ZERO,
                *background_color,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::InternalTransition {
            trigger,
            guard,
            behavior,
        } => {
            let view = new_umlstatemachine_internaltransition(trigger, guard, behavior).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::InitialPseudostate {} => {
            let view = new_umlstatemachine_initialpseudostate(egui::Pos2::ZERO).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::TerminatePseudostate { with_edge_from: _ } => {
            let view = new_umlstatemachine_terminatepseudostate(egui::Pos2::ZERO).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::FinalState { with_edge_from: _ } => {
            let view = new_umlstatemachine_finalstate(egui::Pos2::ZERO).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::LinkStart { link_type } => {
            let (d, dv) = new_umlstatemachine_initialpseudostate(egui::Pos2::ZERO);
            let dummy_1_nonfinal = (d.into(), dv.into());
            let (d, dv) = new_umlstatemachine_terminatepseudostate(egui::Pos2::new(200.0, 150.0));
            let dummy_2_noninitial = (d.clone().into(), dv.clone().into());

            match link_type {
                LinkType::Edge { name } => {
                    let view = new_umlstatemachine_edge(
                        name,
                        None,
                        dummy_1_nonfinal.clone(),
                        dummy_2_noninitial.clone(),
                    )
                    .1;
                    view.into()
                }
            }
        }
        UmlStateMachineToolStage::StateMachineStart {
            stereotype,
            name,
            is_protocol,
        } => {
            let view = new_umlstatemachine_statemachine(
                name,
                stereotype,
                *is_protocol,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(200.0, 100.0),
                },
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::CompositeStateStart {
            stereotype,
            name,
            background_color,
        } => {
            let ps = new_umlstatemachine_compositestateregion(egui::Rect {
                min: egui::Pos2::ZERO,
                max: egui::Pos2::new(175.0, 75.0),
            });
            ps.1.write().refresh_buffers();
            let view = new_umlstatemachine_compositestate(
                name,
                stereotype,
                Vec::new(),
                vec![ps],
                egui::Pos2::ZERO,
                *background_color,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::Note {
            stereotype,
            text,
            align,
            background_color,
        } => {
            let view = new_umlstatemachine_note(
                text,
                stereotype,
                egui::Pos2::ZERO,
                *align,
                *background_color,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlStateMachineToolStage::NoteLinkStart => {
            let (comment, comment_view) = new_umlstatemachine_note(
                "dummy",
                "",
                egui::Pos2::ZERO,
                egui::Align2::CENTER_CENTER,
                MGlobalColor::None,
            );
            let (d, dv) = new_umlstatemachine_terminatepseudostate(egui::Pos2::new(200.0, 150.0));
            let dummy_2_element = (d.into(), dv.into());

            let view = new_umlstatemachine_notelink(
                None,
                (comment.clone(), comment_view.clone().into()),
                dummy_2_element.clone(),
            )
            .1;
            view.into()
        }
        UmlStateMachineToolStage::CompositeStateRegion {}
        | UmlStateMachineToolStage::LinkEnd
        | UmlStateMachineToolStage::StateMachineEnd
        | UmlStateMachineToolStage::CompositeStateEnd
        | UmlStateMachineToolStage::NoteLinkEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(UmlStateMachineSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
        nonfinal_buttons: buttons::NONFINAL_BUTTONS.clone(),
        compositestate_buttons: buttons::NONFINAL_BUTTONS
            .iter()
            .cloned()
            .chain(buttons::COMPOSITE_STATE_BUTTONS.iter().cloned())
            .collect(),
        vertical_header: value
            .get("vertical_header")
            .cloned()
            .ok_or(())
            .and_then(|e| e.try_into().map_err(|_| ()))?,
    }))
}

inventory::submit! {DiagramInfo {
    type_indentifier: "umlstatemachine",
    pretty_name: "UML State Machine diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "/Unified Modeling Language",
        description: "UML State Machine diagram (state machines, states, etc.)",
        constructors: &[
            ("empty", &((|no| format!("New UML State Machine diagram {}", no)) as DefaultNameF), &(new as DiagramConstructorF)),
            ("demo", &((|no| format!("Demo UML State Machine diagram {}", no)) as DefaultNameF), &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkType {
    Edge { name: String },
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum UmlStateMachineToolStage {
    SimpleState {
        stereotype: String,
        name: String,
        background_color: MGlobalColor,
        with_edge_from: Option<ModelUuid>,
    },
    InternalTransition {
        trigger: String,
        guard: String,
        behavior: String,
    },
    InitialPseudostate {},
    TerminatePseudostate {
        with_edge_from: Option<ModelUuid>,
    },
    FinalState {
        with_edge_from: Option<ModelUuid>,
    },
    LinkStart {
        link_type: LinkType,
    },
    LinkEnd,
    StateMachineStart {
        stereotype: String,
        name: String,
        is_protocol: bool,
    },
    StateMachineEnd,
    CompositeStateStart {
        stereotype: String,
        name: String,
        background_color: MGlobalColor,
    },
    CompositeStateEnd,
    CompositeStateRegion {},
    Note {
        stereotype: String,
        text: String,
        align: egui::Align2,
        background_color: MGlobalColor,
    },
    NoteLinkStart,
    NoteLinkEnd,
}

pub enum PartialUmlStateMachineElement {
    None,
    Some(UmlStateMachineElementView),
    Link {
        link_type: LinkType,
        source: UmlStateMachineNonFinalNode,
        dest: Option<UmlStateMachineNonInitialNode>,
    },
    StateMachine {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    CompositeState {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    NoteLink {
        source: ERef<UmlStateMachineNote>,
        dest: Option<UmlStateMachineElement>,
    },
}

pub struct NaiveUmlStateMachineTool {
    uuid: uuid::Uuid,
    initial_stage: UmlStateMachineToolStage,
    current_stage: UmlStateMachineToolStage,
    result: PartialUmlStateMachineElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveUmlStateMachineTool {
    fn try_spend(&mut self) {
        self.result = PartialUmlStateMachineElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<UmlStateMachineDomain> for NaiveUmlStateMachineTool {
    type Stage = UmlStateMachineToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: UmlStateMachineToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialUmlStateMachineElement::None,
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
        element: Result<UmlStateMachineElement, ERef<UmlStateMachineDiagram>>,
    ) -> egui::Color32 {
        macro_rules! already_contains_initial {
            ($c:expr) => {
                $c.contained_elements
                    .iter()
                    .any(|e| matches!(e, UmlStateMachineStandaloneElement::InitialPseudostate(_)))
            };
        }
        match element {
            Err(d)
                if matches!(
                    self.current_stage,
                    UmlStateMachineToolStage::InitialPseudostate { .. }
                ) && already_contains_initial!(d.read()) =>
            {
                NON_TARGETTABLE_COLOR
            }
            Ok(UmlStateMachineElement::StateMachine(c))
                if matches!(
                    self.current_stage,
                    UmlStateMachineToolStage::InitialPseudostate { .. }
                ) && already_contains_initial!(c.read()) =>
            {
                NON_TARGETTABLE_COLOR
            }
            Ok(UmlStateMachineElement::CompositeStateRegion(c))
                if matches!(
                    self.current_stage,
                    UmlStateMachineToolStage::InitialPseudostate { .. }
                ) && already_contains_initial!(c.read()) =>
            {
                NON_TARGETTABLE_COLOR
            }
            Err(_)
            | Ok(
                UmlStateMachineElement::StateMachine(_)
                | UmlStateMachineElement::CompositeStateRegion(_),
            ) => match self.current_stage {
                UmlStateMachineToolStage::LinkStart { .. }
                | UmlStateMachineToolStage::LinkEnd
                | UmlStateMachineToolStage::NoteLinkStart
                | UmlStateMachineToolStage::NoteLinkEnd => NON_TARGETTABLE_COLOR,
                _ => TARGETTABLE_COLOR,
            },
            Ok(UmlStateMachineElement::CompositeState(_)) => match self.current_stage {
                UmlStateMachineToolStage::LinkStart { .. }
                | UmlStateMachineToolStage::LinkEnd
                | UmlStateMachineToolStage::NoteLinkEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(UmlStateMachineElement::InitialPseudostate(_)) => match self.current_stage {
                UmlStateMachineToolStage::LinkStart { .. }
                | UmlStateMachineToolStage::NoteLinkEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(
                UmlStateMachineElement::TerminatePseudostate(_)
                | UmlStateMachineElement::FinalState(_),
            ) => match self.current_stage {
                UmlStateMachineToolStage::LinkEnd | UmlStateMachineToolStage::NoteLinkEnd => {
                    TARGETTABLE_COLOR
                }
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(UmlStateMachineElement::InternalTransition(_)) => NON_TARGETTABLE_COLOR,
            Ok(UmlStateMachineElement::Note(_)) => match self.current_stage {
                UmlStateMachineToolStage::NoteLinkStart => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(UmlStateMachineElement::Edge(_) | UmlStateMachineElement::NoteLink(_)) => {
                unreachable!()
            }
        }
    }
    fn draw_status_hint(
        &self,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match (&self.current_stage, &self.result) {
            (_, PartialUmlStateMachineElement::Link { source, .. }) => {
                if let Some(source_view) = q.get_view_for(&source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                UmlStateMachineToolStage::SimpleState {
                    with_edge_from: Some(source_uuid),
                    ..
                }
                | UmlStateMachineToolStage::FinalState {
                    with_edge_from: Some(source_uuid),
                    ..
                }
                | UmlStateMachineToolStage::TerminatePseudostate {
                    with_edge_from: Some(source_uuid),
                    ..
                },
                _,
            ) => {
                // TODO: don't show when such edge would not be valid
                if let Some(source_view) = q.get_view_for(source_uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                _,
                PartialUmlStateMachineElement::StateMachine { a, .. }
                | PartialUmlStateMachineElement::CompositeState { a, .. },
            ) => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a, pos),
                    egui::CornerRadius::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            (_, PartialUmlStateMachineElement::NoteLink { source, .. }) => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            _ => {}
        }
    }

    fn add_position(&mut self, pos: egui::Pos2) {
        if self.event_lock {
            return;
        }

        match (&self.current_stage, &mut self.result) {
            (
                UmlStateMachineToolStage::SimpleState {
                    stereotype,
                    name,
                    background_color,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) = new_umlstatemachine_compositestate(
                    name,
                    stereotype,
                    Vec::new(),
                    Vec::new(),
                    pos,
                    *background_color,
                );
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
            }
            (UmlStateMachineToolStage::InitialPseudostate {}, _) => {
                let (_model, view) = new_umlstatemachine_initialpseudostate(pos);
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
            }
            (UmlStateMachineToolStage::TerminatePseudostate { with_edge_from: _ }, _) => {
                let (_model, view) = new_umlstatemachine_terminatepseudostate(pos);
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
            }
            (UmlStateMachineToolStage::FinalState { with_edge_from: _ }, _) => {
                let (_model, view) = new_umlstatemachine_finalstate(pos);
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
            }
            (
                UmlStateMachineToolStage::Note {
                    stereotype,
                    text,
                    align,
                    background_color,
                },
                _,
            ) => {
                let (_model, view) =
                    new_umlstatemachine_note(text, stereotype, pos, *align, *background_color);
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
            }
            (UmlStateMachineToolStage::StateMachineStart { .. }, _) => {
                self.result = PartialUmlStateMachineElement::StateMachine { a: pos, b: None };
                self.current_stage = UmlStateMachineToolStage::StateMachineEnd;
                self.event_lock = true;
            }
            (
                UmlStateMachineToolStage::StateMachineEnd,
                PartialUmlStateMachineElement::StateMachine { b, .. },
            ) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (UmlStateMachineToolStage::CompositeStateStart { .. }, _) => {
                self.result = PartialUmlStateMachineElement::CompositeState { a: pos, b: None };
                self.current_stage = UmlStateMachineToolStage::CompositeStateEnd;
                self.event_lock = true;
            }
            (
                UmlStateMachineToolStage::CompositeStateEnd,
                PartialUmlStateMachineElement::CompositeState { b, .. },
            ) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, element: UmlStateMachineElement) {
        if self.event_lock {
            return;
        }

        match &self.current_stage {
            UmlStateMachineToolStage::CompositeStateRegion {}
                if let UmlStateMachineElement::CompositeState(_) = element =>
            {
                let (_model, view) = new_umlstatemachine_compositestateregion(
                    egui::Rect::from_x_y_ranges(0.0..=50.0, 0.0..=100.0),
                );
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
                return;
            }
            UmlStateMachineToolStage::InternalTransition {
                trigger,
                guard,
                behavior,
            } if let UmlStateMachineElement::CompositeState(_) = element => {
                let (_model, view) =
                    new_umlstatemachine_internaltransition(trigger, guard, behavior);
                self.result = PartialUmlStateMachineElement::Some(view.into());
                self.event_lock = true;
                return;
            }
            _ => {}
        }

        match element {
            e @ (UmlStateMachineElement::CompositeState(..)
            | UmlStateMachineElement::InitialPseudostate(..)
            | UmlStateMachineElement::TerminatePseudostate(..)
            | UmlStateMachineElement::FinalState(..)) => {
                match (&self.current_stage, &mut self.result) {
                    (
                        UmlStateMachineToolStage::LinkStart { link_type },
                        PartialUmlStateMachineElement::None,
                    ) if let Some(e) = e.as_nonfinal() => {
                        self.result = PartialUmlStateMachineElement::Link {
                            link_type: link_type.clone(),
                            source: e,
                            dest: None,
                        };
                        self.current_stage = UmlStateMachineToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        UmlStateMachineToolStage::LinkEnd,
                        PartialUmlStateMachineElement::Link { source, dest, .. },
                    ) if let Some(e) = e.as_noninitial() => {
                        if source
                            .clone()
                            .to_element()
                            .find_element(&e.uuid())
                            .is_some()
                            || e.clone()
                                .to_element()
                                .find_element(&source.uuid())
                                .is_some()
                        {
                            self.event_lock = true;
                            return;
                        }

                        *dest = Some(e);
                        self.event_lock = true;
                    }
                    (
                        UmlStateMachineToolStage::NoteLinkEnd,
                        PartialUmlStateMachineElement::NoteLink { dest, .. },
                    ) => {
                        *dest = Some(e);
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            UmlStateMachineElement::Note(inner) => {
                if let UmlStateMachineToolStage::NoteLinkStart = &self.current_stage {
                    self.result = PartialUmlStateMachineElement::NoteLink {
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = UmlStateMachineToolStage::NoteLinkEnd;
                    self.event_lock = true;
                }
            }
            _ => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlStateMachineDomain as Domain>::OrdinalMovementT,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &self.result {
            PartialUmlStateMachineElement::Some(element) => {
                let element = element.clone();
                let additional_edge = match &self.initial_stage {
                    UmlStateMachineToolStage::SimpleState {
                        with_edge_from: Some(source_uuid),
                        ..
                    }
                    | UmlStateMachineToolStage::TerminatePseudostate {
                        with_edge_from: Some(source_uuid),
                        ..
                    }
                    | UmlStateMachineToolStage::FinalState {
                        with_edge_from: Some(source_uuid),
                        ..
                    } if let Some(source) = q.get_view_for(source_uuid)
                        && let nearest_common_container = q
                            .find_container(&source.uuid(), |uuid, e| {
                                (uuid == preferred_container
                                    || q.is_contained(preferred_container, uuid))
                                    && !matches!(e, UmlStateMachineElementView::CompositeState(_))
                            })
                            .map(|e| e.0)
                            .unwrap_or_else(|| q.get_root()) =>
                    {
                        let edge_view = new_umlstatemachine_edge(
                            "",
                            None,
                            (source.model().as_nonfinal().unwrap(), source),
                            (element.model().as_noninitial().unwrap(), element.clone()),
                        )
                        .1;
                        Some((nearest_common_container, edge_view))
                    }
                    _ => None,
                };

                self.try_spend();

                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: element.into(),
                    into_model: true,
                });
                if let Some((parent, e)) = additional_edge {
                    commands.push(InsensitiveCommand::AddDependency {
                        target: parent,
                        bucket: 0,
                        position: None,
                        element: UmlStateMachineElementView::from(e).into(),
                        into_model: true,
                    });
                }
                Ok(None)
            }
            PartialUmlStateMachineElement::Link {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.uuid(), *dest.uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_view.uuid(), preferred_container)
                    && q.is_contained(&target_view.uuid(), preferred_container)
                    && q.find_container(&source_view.uuid(), |_, e| {
                        matches!(e, UmlStateMachineElementView::StateMachine(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlStateMachineElementView::StateMachine(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view: UmlStateMachineElementView = match link_type {
                        LinkType::Edge { name } => new_umlstatemachine_edge(
                            name,
                            None,
                            (source.clone(), source_view),
                            (dest.clone(), target_view),
                        )
                        .1
                        .into(),
                    };

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: link_view.into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialUmlStateMachineElement::StateMachine { a, b: Some(b) }
                if let UmlStateMachineToolStage::StateMachineStart {
                    stereotype,
                    name,
                    is_protocol,
                } = &self.initial_stage =>
            {
                self.current_stage = self.initial_stage.clone();

                let view = new_umlstatemachine_statemachine(
                    name,
                    stereotype,
                    *is_protocol,
                    egui::Rect::from_two_pos(*a, *b),
                )
                .1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlStateMachineElementView::from(view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlStateMachineElement::CompositeState { a, b: Some(b) }
                if let UmlStateMachineToolStage::CompositeStateStart {
                    stereotype,
                    name,
                    background_color,
                } = &self.initial_stage =>
            {
                self.current_stage = self.initial_stage.clone();

                let r = egui::Rect::from_two_pos(*a, *b);
                let s = new_umlstatemachine_compositestateregion(r);
                let view = new_umlstatemachine_compositestate(
                    name,
                    stereotype,
                    Vec::new(),
                    vec![s],
                    egui::Pos2::ZERO,
                    *background_color,
                )
                .1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlStateMachineElementView::from(view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlStateMachineElement::NoteLink {
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid, *dest.uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_view.uuid(), preferred_container)
                    && q.is_contained(&target_view.uuid(), preferred_container)
                    && q.find_container(&source_view.uuid(), |_, e| {
                        matches!(e, UmlStateMachineElementView::StateMachine(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlStateMachineElementView::StateMachine(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view = new_umlstatemachine_notelink(
                        None,
                        (source.clone(), source_view),
                        (dest.clone(), target_view),
                    )
                    .1;

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: UmlStateMachineElementView::from(link_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

pub fn new_umlstatemachine_statemachine(
    name: &str,
    stereotype: &str,
    is_protocol: bool,
    bounds_rect: egui::Rect,
) -> (ERef<UmlStateMachine>, ERef<StateMachineViewT>) {
    let package_model = ERef::new(UmlStateMachine::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        is_protocol,
        Vec::new(),
    ));
    let package_view = new_umlstatemachine_statemachine_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_umlstatemachine_statemachine_view(
    model: ERef<UmlStateMachine>,
    bounds_rect: egui::Rect,
) -> ERef<StateMachineViewT> {
    let m = model.read();
    PackageView::new(
        ViewUuid::now_v7().into(),
        UmlStateMachineAdapter {
            model: model.clone(),
            background_color: MGlobalColor::None,
            display_text: Arc::new("".to_owned()),
            stereotype_buffer: (*m.stereotype).clone(),
            name_buffer: (*m.name).clone(),
            is_protocol_buffer: m.is_protocol,
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlStateMachineAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachine>,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    display_text: Arc<String>,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    is_protocol_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<UmlStateMachineDomain> for UmlStateMachineAdapter {
    fn model_section(&self) -> UmlStateMachineElement {
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

    fn draw_area_or_get_props(
        &self,
        _bounds_rect: egui::Rect,
        _highlight: canvas::Highlight,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        gdc: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlStateMachineDomain as Domain>::ToolT)>,
    ) -> Result<(), (egui::Color32, canvas::Stroke)> {
        Err((
            gdc.global_colors
                .get(&self.background_color)
                .unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        ))
    }
    fn draw_label_or_get_text(
        &self,
        bounds_rect: egui::Rect,
        highlight: canvas::Highlight,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlStateMachineDomain as Domain>::ToolT)>,
    ) -> Result<egui::Rect, (egui::Color32, Arc<String>)> {
        // Draw top left pentagon
        const PENTAGON_PADDING: f32 = 4.0;
        let pentagon_bg = egui::Color32::WHITE;
        let left_top_pentagon_rect = canvas
            .measure_text(
                bounds_rect.left_top() + egui::Vec2::splat(PENTAGON_PADDING),
                egui::Align2::LEFT_TOP,
                &self.display_text,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .expand(PENTAGON_PADDING);
        canvas.draw_polygon(
            [
                left_top_pentagon_rect.left_top(),
                left_top_pentagon_rect.right_top(),
                left_top_pentagon_rect.right_bottom() - egui::Vec2::new(0.0, PENTAGON_PADDING),
                left_top_pentagon_rect.right_bottom() - egui::Vec2::new(PENTAGON_PADDING, 0.0),
                left_top_pentagon_rect.left_bottom(),
            ]
            .to_vec(),
            pentagon_bg,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            highlight,
        );
        canvas.draw_text(
            bounds_rect.left_top() + egui::Vec2::splat(PENTAGON_PADDING),
            egui::Align2::LEFT_TOP,
            &self.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        Ok(left_top_pentagon_rect)
    }

    fn show_model_properties(
        &mut self,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::StereotypeChange(Arc::new(
                    self.stereotype_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .checkbox(&mut self.is_protocol_buffer, "isProtocol")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::StateMachineIsProtocolChange(self.is_protocol_buffer),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }
    }
    fn show_color_property(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ColorChangeData> {
        ui.label("Background color:");
        crate::common::controller::mglobalcolor_edit_button(context, ui, &self.background_color)
            .map(|e| (0, e).into())
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlStateMachinePropChange::StereotypeChange(stereotype) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::StereotypeChange(model.stereotype.clone()),
                    ));
                    model.stereotype = stereotype.clone();
                }
                UmlStateMachinePropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlStateMachinePropChange::StateMachineIsProtocolChange(is_protocol) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::StateMachineIsProtocolChange(model.is_protocol),
                    ));
                    model.is_protocol = *is_protocol;
                }
                UmlStateMachinePropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                UmlStateMachinePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.display_text = {
            let mut acc = "stm: ".to_owned();

            if !model.stereotype.is_empty() {
                acc.push('«');
                acc.push_str(&model.stereotype);
                acc.push_str("» ");
            }
            acc.push_str(&model.name);
            if model.is_protocol {
                acc.push_str(" {protocol}");
            }

            acc.into()
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
        self.is_protocol_buffer = model.is_protocol;
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlStateMachineElement::StateMachine(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(new_uuid, m)
        };

        Self {
            model,
            background_color: self.background_color,
            display_text: self.display_text.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            is_protocol_buffer: self.is_protocol_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
        }
    }

    fn deep_copy_finish(&mut self, _m: &HashMap<ModelUuid, UmlStateMachineElement>) {}
}

pub fn new_umlstatemachine_compositestate(
    name: &str,
    stereotype: &str,
    internal_transitions: Vec<(
        ERef<UmlStateMachineInternalTransition>,
        ERef<UmlStateMachineInternalTransitionView>,
    )>,
    regions: Vec<(
        ERef<UmlStateMachineCompositeStateRegion>,
        ERef<UmlStateMachineCompositeStateRegionView>,
    )>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> (
    ERef<UmlStateMachineCompositeState>,
    ERef<UmlStateMachineCompositeStateView>,
) {
    let (it_models, it_views) = internal_transitions.into_iter().collect();
    let (region_models, region_views) = regions.into_iter().collect();
    let model = ERef::new(UmlStateMachineCompositeState::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        it_models,
        region_models,
    ));
    let view = new_umlstatemachine_compositestate_view(
        model.clone(),
        it_views,
        region_views,
        position,
        background_color,
    );

    (model, view)
}
pub fn new_umlstatemachine_compositestate_view(
    model: ERef<UmlStateMachineCompositeState>,
    internal_transition_views: Vec<ERef<UmlStateMachineInternalTransitionView>>,
    region_views: Vec<ERef<UmlStateMachineCompositeStateRegionView>>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> ERef<UmlStateMachineCompositeStateView> {
    ERef::new(UmlStateMachineCompositeStateView {
        uuid: ViewUuid::now_v7().into(),
        model,
        internal_transition_views,
        region_views,
        bounds_rect: egui::Rect::from_pos(position),
        background_color,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineCompositeStateView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachineCompositeState>,
    #[nh_context_serde(entity)]
    internal_transition_views: Vec<ERef<UmlStateMachineInternalTransitionView>>,
    #[nh_context_serde(entity)]
    region_views: Vec<ERef<UmlStateMachineCompositeStateRegionView>>,

    bounds_rect: egui::Rect,
    background_color: MGlobalColor,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlStateMachineCompositeStateViewTemporaries,
}

#[derive(Clone, Default)]
struct UmlStateMachineCompositeStateViewTemporaries {
    stereotype_in_guillemets: String,
    stereotype_buffer: String,
    name_buffer: String,

    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
}

impl UmlStateMachineCompositeStateView {
    fn handle_size(&self, ui_scale: f32) -> f32 {
        10.0_f32
            .min(self.bounds_rect.width() * ui_scale / 6.0)
            .min(self.bounds_rect.height() * ui_scale / 3.0)
    }
    fn drag_handle_position(&self, ui_scale: f32) -> egui::Pos2 {
        egui::Pos2::new(
            (self.bounds_rect.right() - 2.0 * self.handle_size(ui_scale) / ui_scale)
                .max((self.bounds_rect.center().x + self.bounds_rect.right()) / 2.0),
            self.bounds_rect.top(),
        )
    }
}

impl Entity for UmlStateMachineCompositeStateView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineCompositeStateView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineCompositeStateView {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
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

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineCompositeStateView {
    fn draw_in(
        &mut self,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        gdc: &GlobalDrawingContext,
        settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlStateMachineDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        let background_color = gdc
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);

        let regions_rect = self
            .region_views
            .iter()
            .fold(egui::Rect::NOTHING, |b, e| b.union(e.read().bounds_rect));

        let mut transitions_height = 0.0;
        let mut max_transition_width = 0.0;
        for e in &self.internal_transition_views {
            let r = e.read();
            transitions_height += r.bounds_rect.height();
            max_transition_width += r.bounds_rect.width();
        }

        let text_size = canvas
            .measure_text(
                self.bounds_rect.center_top(),
                egui::Align2::CENTER_BOTTOM,
                &self.temporaries.name_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .size();
        let stereotype_size = if !self.temporaries.stereotype_in_guillemets.is_empty() {
            canvas
                .measure_text(
                    self.bounds_rect.center_top(),
                    egui::Align2::CENTER_BOTTOM,
                    &self.temporaries.stereotype_in_guillemets,
                    canvas::CLASS_TOP_FONT_SIZE,
                )
                .size()
        } else {
            egui::Vec2::ZERO
        };

        const PADDING: egui::Vec2 = egui::Vec2::new(10.0, 10.0);
        let content_rect = if self.region_views.is_empty() {
            let size_x = stereotype_size.x.max(text_size.x).max(max_transition_width);
            let size_y = stereotype_size.y + text_size.y + transitions_height;
            egui::Rect::from_min_size(
                self.bounds_rect.center_top() + (-size_x / 2.0, 0.0).into(),
                (size_x, size_y).into(),
            )
            .expand2(PADDING)
            .translate((0.0, PADDING.y).into())
        } else {
            let mut r = regions_rect;
            r.min.y -= transitions_height;
            r.min.y -= text_size.y;
            r.min.y -= stereotype_size.y;
            r.min.y -= PADDING.y;
            r.max.y += PADDING.y;
            r
        };
        self.bounds_rect = content_rect;
        let activities_center_top =
            content_rect.center_top() + (0.0, PADDING.y + stereotype_size.y + text_size.y).into();

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::same(10),
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );

        if !self.temporaries.stereotype_in_guillemets.is_empty() {
            canvas.draw_text(
                self.bounds_rect.center_top(),
                egui::Align2::CENTER_TOP,
                &self.temporaries.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }
        canvas.draw_text(
            activities_center_top,
            egui::Align2::CENTER_BOTTOM,
            &self.temporaries.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        if !self.internal_transition_views.is_empty() {
            canvas.draw_line(
                [
                    (self.bounds_rect.left(), activities_center_top.y).into(),
                    (self.bounds_rect.right(), activities_center_top.y).into(),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
        }
        let mut top_mut = activities_center_top;
        for e in &self.internal_transition_views {
            let (r, _t) = e.write().draw_inner(
                top_mut,
                egui::Align2::CENTER_TOP,
                q,
                gdc,
                settings,
                canvas,
                tool,
            );
            top_mut = r.center_bottom();
        }

        let (mut first, mut child_targetting_drawn) = (true, false);
        let mut iter = self.region_views.iter().peekable();
        while let Some(e) = iter.next() {
            let mut w = e.write();
            child_targetting_drawn |=
                w.draw_in(q, gdc, settings, canvas, tool) != TargettingStatus::NotDrawn;
            if first {
                canvas.draw_line(
                    [w.bounds_rect.left_top(), w.bounds_rect.right_top()],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                first = false;
            }
            canvas.draw_line(
                [w.bounds_rect.left_bottom(), w.bounds_rect.right_bottom()],
                canvas::Stroke {
                    width: 1.0,
                    color: egui::Color32::BLACK,
                    line_type: if iter.peek().is_some() {
                        canvas::LineType::Dashed
                    } else {
                        canvas::LineType::Solid
                    },
                },
                canvas::Highlight::NONE,
            );
        }

        // Draw buttons
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            draw_button_rects(
                &settings.compositestate_buttons,
                canvas,
                self.bounds_rect.right_top(),
                ui_scale,
            );
        }

        // Draw resize/drag handles
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            let handle_size = self.handle_size(ui_scale);
            let handles_rect = self.bounds_rect.shrink(handle_size / 2.0 / ui_scale);
            for (h, c) in [
                (handles_rect.left_center(), NHIcon::ArrowLeft),
                (handles_rect.right_center(), NHIcon::ArrowRight),
            ] {
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size / ui_scale)),
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                c.draw(canvas, h, 8.0 / ui_scale, egui::Color32::BLACK);
            }

            let dc = self.drag_handle_position(ui_scale);
            canvas.draw_rectangle(
                egui::Rect::from_center_size(dc, egui::Vec2::splat(handle_size / ui_scale)),
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            NHIcon::Move.draw(canvas, dc, 8.0 / ui_scale, egui::Color32::BLACK);
        }

        if child_targetting_drawn {
            return TargettingStatus::Drawn;
        }

        if let Some((pos, tool)) = tool
            && self.bounds_rect.contains(*pos)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                tool.targetting_for_section(Ok(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );

            TargettingStatus::Drawn
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        for (idx, e) in self.internal_transition_views.iter().enumerate() {
            let mut w = e.write();
            let child = w.show_properties_inner(gdc, q, ui, commands);

            if let Some(add_sibling) = child.1 {
                let idx = match add_sibling {
                    UmlStateMachineOrdinalMovement::Up => idx,
                    UmlStateMachineOrdinalMovement::Down => idx + 1,
                };
                let sibling = new_umlstatemachine_internaltransition("trigger", "", "doThing()");
                commands.push(InsensitiveCommand::AddDependency {
                    target: *self.uuid,
                    bucket: 0,
                    position: Some(idx.try_into().unwrap()),
                    element: UmlStateMachineElementOrVertex::Element(sibling.1.into()),
                    into_model: true,
                });
            }

            if let Some(child) = child.0.non_default() {
                return child;
            }
        }

        for (idx, e) in self.region_views.iter().enumerate() {
            let mut w = e.write();
            let child = w.show_properties_inner(gdc, q, ui, commands);

            if let Some(add_sibling) = child.1 {
                let idx = match add_sibling {
                    UmlStateMachineOrdinalMovement::Up => idx,
                    UmlStateMachineOrdinalMovement::Down => idx + 1,
                };
                let y_range = match add_sibling {
                    UmlStateMachineOrdinalMovement::Up => {
                        (w.bounds_rect.top() - 200.0)..=w.bounds_rect.top()
                    }
                    UmlStateMachineOrdinalMovement::Down => {
                        w.bounds_rect.bottom()..=(w.bounds_rect.bottom() + 200.0)
                    }
                };
                let sibling = new_umlstatemachine_compositestateregion(
                    egui::Rect::from_x_y_ranges(w.bounds_rect.x_range(), y_range),
                );
                commands.push(InsensitiveCommand::AddDependency {
                    target: *self.uuid,
                    bucket: 0,
                    position: Some(idx.try_into().unwrap()),
                    element: UmlStateMachineElementOrVertex::Element(sibling.1.into()),
                    into_model: true,
                });
            }

            if let Some(child) = child.0.non_default() {
                return child;
            }
        }

        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.temporaries.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::StereotypeChange(Arc::new(
                    self.temporaries.stereotype_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::NameChange(Arc::new(
                    self.temporaries.name_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        self.region_views
            .iter()
            .for_each(|v| v.write().collect_allignment(am));
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<UmlStateMachineDomain as Domain>::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlStateMachineDomain as Domain>::OrdinalMovementT,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        let k_status = self
            .internal_transition_views
            .iter()
            .flat_map(|v| {
                let mut w = v.write();
                let s =
                    w.handle_event(event, ehc, settings, q, tool, element_setup_modal, commands);
                if s != EventHandlingStatus::NotHandled {
                    Some((*w.uuid(), s))
                } else {
                    None
                }
            })
            .next()
            .or_else(|| {
                self.region_views
                    .iter()
                    .flat_map(|v| {
                        let mut w = v.write();
                        let s = w.handle_event(
                            event,
                            ehc,
                            settings,
                            q,
                            tool,
                            element_setup_modal,
                            commands,
                        );
                        if s != EventHandlingStatus::NotHandled {
                            Some((*w.uuid(), s))
                        } else {
                            None
                        }
                    })
                    .next()
            });

        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if k_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) => {
                let handle_size = self.handle_size(1.0);
                if self.temporaries.highlight.selected {
                    let handle_size = self.handle_size(1.0);
                    let handles_rect = self.bounds_rect.shrink(handle_size / 2.0 / ehc.ui_scale);
                    for (a, h) in [
                        (egui::Align2::RIGHT_CENTER, handles_rect.left_center()),
                        (egui::Align2::LEFT_CENTER, handles_rect.right_center()),
                    ] {
                        if egui::Rect::from_center_size(
                            h,
                            egui::Vec2::splat(handle_size) / ehc.ui_scale,
                        )
                        .contains(pos)
                        {
                            self.temporaries.dragged_type_and_shape =
                                Some((PackageDragType::Resize(a), self.bounds_rect));
                            return EventHandlingStatus::HandledByElement;
                        }
                    }
                }

                if self.min_shape().border_distance(pos) <= 2.0 / ehc.ui_scale
                    || egui::Rect::from_center_size(
                        self.drag_handle_position(ehc.ui_scale),
                        egui::Vec2::splat(handle_size) / ehc.ui_scale,
                    )
                    .contains(pos)
                {
                    self.temporaries.dragged_type_and_shape =
                        Some((PackageDragType::Move, self.bounds_rect));
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::MouseUp(_) => {
                if self.temporaries.dragged_type_and_shape.is_some() {
                    self.temporaries.dragged_type_and_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos)
                if self.temporaries.highlight.selected
                    && let Some(f) = handle_button_click(
                        &settings.compositestate_buttons,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlStateMachineTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage,
                    current_stage,
                    result,
                    event_lock,
                    is_spent: Some(false),
                });

                if let Some(tool) = tool {
                    tool.add_section(self.model());
                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands)
                        && ehc
                            .modifier_settings
                            .alternative_tool_mode
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
                        *element_setup_modal = esm;
                    }
                }

                EventHandlingStatus::HandledByContainer
            }
            InputEvent::Click(pos) => {
                if !self.bounds_rect.contains(pos) {
                    return k_status
                        .map(|e| e.1)
                        .unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands)
                        && ehc
                            .modifier_settings
                            .alternative_tool_mode
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
                        *element_setup_modal = esm;
                    }

                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());

                    EventHandlingStatus::HandledByContainer
                } else if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc
                            .modifier_settings
                            .hold_selection
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            commands.push(InsensitiveCommand::HighlightAll(
                                false,
                                canvas::Highlight::SELECTED,
                            ));
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                canvas::Highlight::SELECTED,
                            ));
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.temporaries.selected_direct_elements.contains(&k),
                                canvas::Highlight::SELECTED,
                            ));
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            }
            InputEvent::Drag { delta, .. } => match self.temporaries.dragged_type_and_shape {
                Some((PackageDragType::Move, real_bounds)) => {
                    let translated_bounds = real_bounds.translate(delta);
                    self.temporaries.dragged_type_and_shape =
                        Some((PackageDragType::Move, translated_bounds));
                    let translated_real_shape = canvas::NHShape::Rect {
                        inner: translated_bounds,
                    };
                    let coerced_pos = ehc.snap_manager.coerce(translated_real_shape, |e| {
                        if self.temporaries.highlight.selected {
                            !ehc.all_elements
                                .get(e)
                                .is_some_and(|e| *e != SelectionStatus::NotSelected)
                        } else {
                            *e != *self.uuid
                        }
                    });
                    let coerced_delta = coerced_pos - self.position();

                    if self.temporaries.highlight.selected {
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
                Some((PackageDragType::Resize(align), real_bounds)) => {
                    let (left, right) = match align.x() {
                        egui::Align::Min => (0.0, delta.x),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => (-delta.x, 0.0),
                    };
                    let (top, bottom) = match align.y() {
                        egui::Align::Min => (0.0, delta.y),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => (-delta.y, 0.0),
                    };
                    let new_real_bounds = real_bounds
                        + epaint::MarginF32 {
                            left,
                            right,
                            top,
                            bottom,
                        };
                    self.temporaries.dragged_type_and_shape =
                        Some((PackageDragType::Resize(align), new_real_bounds));
                    let handle_x = match align.x() {
                        egui::Align::Min => (new_real_bounds.right(), self.bounds_rect.right()),
                        egui::Align::Center => {
                            (new_real_bounds.center().x, self.bounds_rect.center().x)
                        }
                        egui::Align::Max => (new_real_bounds.left(), self.bounds_rect.left()),
                    };
                    let handle_y = match align.y() {
                        egui::Align::Min => (new_real_bounds.bottom(), self.bounds_rect.bottom()),
                        egui::Align::Center => {
                            (new_real_bounds.center().y, self.bounds_rect.center().y)
                        }
                        egui::Align::Max => (new_real_bounds.top(), self.bounds_rect.top()),
                    };
                    let coerced_point = ehc.snap_manager.coerce(
                        canvas::NHShape::Rect {
                            inner: egui::Rect::from_min_size(
                                egui::Pos2::new(handle_x.0, handle_y.0),
                                egui::Vec2::ZERO,
                            ),
                        },
                        |e| {
                            !ehc.all_elements
                                .get(e)
                                .is_some_and(|e| *e != SelectionStatus::NotSelected)
                        },
                    );
                    let coerced_delta = coerced_point - egui::Pos2::new(handle_x.1, handle_y.1);

                    commands.push(InsensitiveCommand::ResizeElementsBy(
                        q.selected_views(),
                        align,
                        coerced_delta,
                    ));
                    EventHandlingStatus::HandledByElement
                }
                None => EventHandlingStatus::NotHandled,
            },
        }
    }

    fn apply_command(
        &mut self,
        diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            <UmlStateMachineDomain as Domain>::OrdinalMovementT,
            <UmlStateMachineDomain as Domain>::AddCommandElementT,
            <UmlStateMachineDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <UmlStateMachineDomain as Domain>::OrdinalMovementT,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.internal_transition_views.iter().for_each(|t| {
                    t.write().apply_command(
                        diagram_model,
                        command,
                        undo_accumulator,
                        affected_models,
                    );
                });
                self.region_views.iter().for_each(|s| {
                    s.write().apply_command(
                        diagram_model,
                        command,
                        undo_accumulator,
                        affected_models,
                    )
                });
            };
        }

        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                if h.selected {
                    match set {
                        true => {
                            self.temporaries.selected_direct_elements = self
                                .internal_transition_views
                                .iter()
                                .map(|v| *v.read().uuid)
                                .chain(self.region_views.iter().map(|v| *v.read().uuid))
                                .collect();
                        }
                        false => self.temporaries.selected_direct_elements.clear(),
                    }
                }
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                }

                if h.selected {
                    for k in self
                        .internal_transition_views
                        .iter()
                        .map(|v| *v.read().uuid)
                        .chain(self.region_views.iter().map(|v| *v.read().uuid))
                        .filter(|k| uuids.contains(k))
                    {
                        match set {
                            true => self.temporaries.selected_direct_elements.insert(k),
                            false => self.temporaries.selected_direct_elements.remove(&k),
                        };
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected = (self.temporaries.highlight.selected
                    && *retain)
                    || self.min_shape().contained_within(*rect);

                recurse!();
            }
            InsensitiveCommand::MovePositional(uuids, _)
                if !uuids.contains(&self.uuid)
                    && !self
                        .region_views
                        .iter()
                        .any(|e| uuids.contains(&e.read().uuid)) =>
            {
                recurse!();
            }
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.bounds_rect = self.bounds_rect.translate(*delta);
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.region_views.iter_mut().for_each(|v| {
                    v.write().apply_command(
                        diagram_model,
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    undo_accumulator.push(InsensitiveCommand::ResizeElementTo(
                        *self.uuid,
                        self.bounds_rect,
                    ));

                    for e in self.region_views.iter() {
                        let mut w = e.write();
                        match align.x() {
                            egui::Align::Min => w.bounds_rect.max.x += delta.x,
                            egui::Align::Max => w.bounds_rect.min.x += delta.x,
                            _ => {}
                        }
                    }
                }

                if self
                    .region_views
                    .iter()
                    .any(|e| uuids.contains(&e.read().uuid))
                {
                    let mut delta_x = egui::Vec2::ZERO;
                    let (mut u, mut v) = Default::default();

                    let sections_iter: Box<
                        dyn Iterator<Item = &ERef<UmlStateMachineCompositeStateRegionView>>,
                    > = match align.y() {
                        egui::Align::Min | egui::Align::Center => {
                            Box::new(self.region_views.iter())
                        }
                        egui::Align::Max => Box::new(self.region_views.iter().rev()),
                    };

                    for e in sections_iter {
                        let mut w = e.write();
                        w.apply_command(
                            diagram_model,
                            &InsensitiveCommand::MovePositionalAll(delta_x),
                            &mut u,
                            &mut v,
                        );
                        let mut new_rect = w.bounds_rect;
                        match align.x() {
                            egui::Align::Min => new_rect.max.x += delta.x,
                            egui::Align::Max => new_rect.min.x += delta.x,
                            _ => {}
                        }
                        if uuids.contains(&w.uuid) {
                            match align.y() {
                                egui::Align::Min => new_rect.max.y += delta.y,
                                egui::Align::Max => new_rect.min.y += delta.y,
                                _ => {}
                            }
                            undo_accumulator
                                .push(InsensitiveCommand::ResizeElementTo(*w.uuid, w.bounds_rect));
                            if new_rect.height() >= 40.0 {
                                w.bounds_rect.min.y = new_rect.min.y;
                                w.bounds_rect.max.y = new_rect.max.y;
                                delta_x.y += delta.y;
                            }
                        }
                        if new_rect.width() >= 40.0 {
                            w.bounds_rect.min.x = new_rect.min.x;
                            w.bounds_rect.max.x = new_rect.max.x;
                        }
                    }
                }

                recurse!();
            }
            InsensitiveCommand::ResizeElementTo(uuid, rect) => {
                if *uuid == *self.uuid {
                    undo_accumulator.push(InsensitiveCommand::ResizeElementTo(
                        *self.uuid,
                        self.bounds_rect,
                    ));

                    for e in self.region_views.iter() {
                        let mut w = e.write();
                        w.bounds_rect.min.x = rect.min.x;
                        w.bounds_rect.max.x = rect.max.x;
                    }
                }

                if let Some((idx, br)) = self
                    .region_views
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, e)| {
                        if let r = e.read()
                            && *r.uuid == *uuid
                        {
                            Some((idx, r.bounds_rect))
                        } else {
                            None
                        }
                    })
                    .next()
                {
                    {
                        let mut w = self.region_views[idx].write();
                        undo_accumulator
                            .push(InsensitiveCommand::ResizeElementTo(*uuid, w.bounds_rect));
                        w.bounds_rect = *rect;
                    }

                    let (mut u, mut v) = Default::default();
                    macro_rules! adjust {
                        ($w:expr, $dx:expr) => {
                            $w.apply_command(
                                diagram_model,
                                &InsensitiveCommand::MovePositionalAll(egui::Vec2::new($dx, 0.0)),
                                &mut u,
                                &mut v,
                            );
                            $w.bounds_rect.set_height(rect.height());
                        };
                    }

                    let delta_left = rect.min.x - br.min.x;
                    for e in self.region_views.iter().take(idx).rev() {
                        adjust!(e.write(), delta_left);
                    }

                    let delta_right = rect.max.x - br.max.x;
                    for e in self.region_views.iter().skip(idx + 1) {
                        adjust!(e.write(), delta_right);
                    }
                }

                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self
                    .internal_transition_views
                    .iter()
                    .filter(|v| uuids.contains(&v.read().uuid))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
                    } else if let Some((b, pos)) = self
                        .model
                        .read()
                        .get_element_pos(&element.read().model_uuid())
                    {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid,
                        bucket: b,
                        position: pos,
                        element: UmlStateMachineElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }
                self.internal_transition_views
                    .retain(|e| !uuids.contains(&e.read().uuid));

                for element in self
                    .region_views
                    .iter()
                    .filter(|v| uuids.contains(&v.read().uuid))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
                    } else if let Some((b, pos)) = self
                        .model
                        .read()
                        .get_element_pos(&element.read().model_uuid())
                    {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid,
                        bucket: b,
                        position: pos,
                        element: UmlStateMachineElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }
                let mut delta = egui::Vec2::ZERO;
                let (mut u, mut m) = Default::default();
                let old_sections = std::mem::take(&mut self.region_views);
                for e in old_sections {
                    let mut w = e.write();
                    if uuids.contains(&w.uuid) {
                        delta.y += w.bounds_rect.height();
                    } else {
                        w.apply_command(
                            diagram_model,
                            &InsensitiveCommand::MovePositionalAll(-delta),
                            &mut u,
                            &mut m,
                        );
                        drop(w);
                        self.region_views.push(e);
                    }
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
                    if (*bucket == 0
                        || *bucket == UmlStateMachineCompositeState::INTERNAL_TRANSITIONS_BUCKET)
                        && let Ok(UmlStateMachineElementView::InternalTransition(view)) =
                            element.clone().try_into()
                    {
                        let mut vw = view.write();
                        let pos = self.model.read().get_element_pos(&vw.model_uuid());
                        if let Some(model_pos) = pos.map(|e| e.1).or_else(|| {
                            if *into_model {
                                diagram_model
                                    .write()
                                    .insert_element_into(model_uuid, *bucket, *position, vw.model())
                                    .ok()
                            } else {
                                None
                            }
                        }) {
                            let uuid = *vw.uuid;

                            let mut model_transitives = HashMap::new();
                            vw.head_count(
                                &mut HashMap::new(),
                                &mut HashMap::new(),
                                &mut model_transitives,
                            );
                            affected_models.extend(model_transitives.into_keys());

                            undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                                target: *self.uuid,
                                bucket: *bucket,
                                element: uuid,
                                including_model: *into_model,
                            });
                            affected_models.insert(model_uuid);

                            let view_pos = {
                                let mut view_pos: PositionNoT = 0;
                                for e in &self.internal_transition_views {
                                    let Some((_b, pos)) =
                                        self.model.read().get_element_pos(&e.read().model_uuid())
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
                            self.internal_transition_views
                                .insert(view_pos, view.clone());
                        }
                    }

                    if (*bucket == 0 || *bucket == UmlStateMachineCompositeState::REGIONS_BUCKET)
                        && let Ok(UmlStateMachineElementView::CompositeStateRegion(view)) =
                            element.clone().try_into()
                    {
                        let mut vw = view.write();
                        let pos = self.model.read().get_element_pos(&vw.model_uuid());
                        if let Some(model_pos) = pos.map(|e| e.1).or_else(|| {
                            if *into_model {
                                diagram_model
                                    .write()
                                    .insert_element_into(model_uuid, *bucket, *position, vw.model())
                                    .ok()
                            } else {
                                None
                            }
                        }) {
                            let uuid = *vw.uuid;

                            let (old_uuid, old_rect) = self
                                .region_views
                                .first()
                                .map(|e| {
                                    let r = e.read();
                                    (*r.uuid, r.bounds_rect)
                                })
                                .unwrap_or((*self.uuid, self.bounds_rect));
                            if old_rect.width() >= vw.bounds_rect.width() {
                                vw.bounds_rect.set_width(old_rect.width());
                            } else {
                                for e in &self.region_views {
                                    e.write().bounds_rect.set_width(vw.bounds_rect.width());
                                }
                            }
                            let horizontal_delta = old_rect.width() - vw.bounds_rect.width();

                            undo_accumulator.extend([
                                InsensitiveCommand::ResizeElementsBy(
                                    std::iter::once(old_uuid).collect(),
                                    egui::Align2::CENTER_TOP,
                                    egui::Vec2::new(-horizontal_delta, 0.0),
                                ),
                                InsensitiveCommand::RemoveDependency {
                                    target: *self.uuid,
                                    bucket: *bucket,
                                    element: uuid,
                                    including_model: *into_model,
                                },
                                InsensitiveCommand::ResizeElementsBy(
                                    std::iter::once(old_uuid).collect(),
                                    egui::Align2::CENTER_TOP,
                                    egui::Vec2::new(horizontal_delta, 0.0),
                                ),
                            ]);

                            if *into_model {
                                affected_models.insert(model_uuid);
                            }
                            let mut model_transitives = HashMap::new();
                            vw.head_count(
                                &mut HashMap::new(),
                                &mut HashMap::new(),
                                &mut model_transitives,
                            );
                            affected_models.extend(model_transitives.into_keys());

                            let view_pos = {
                                let mut view_pos: PositionNoT = 0;
                                for e in &self.region_views {
                                    let Some((_b, pos)) =
                                        self.model.read().get_element_pos(&e.read().model_uuid())
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

                            let old_position = if self.region_views.len() == view_pos {
                                self.region_views
                                    .last()
                                    .map(|e| e.read().bounds_rect.left_bottom())
                            } else {
                                self.region_views
                                    .iter()
                                    .skip(view_pos)
                                    .map(|e| e.read().bounds_rect.min)
                                    .next()
                            }
                            .unwrap_or_default();
                            let delta = (0.0, vw.bounds_rect.height()).into();
                            let (mut u, mut m) = Default::default();
                            for e in self.region_views.iter().skip(view_pos) {
                                e.write().apply_command(
                                    diagram_model,
                                    &InsensitiveCommand::MovePositionalAll(delta),
                                    &mut u,
                                    &mut m,
                                );
                            }
                            let delta = old_position - vw.bounds_rect.min;
                            vw.apply_command(
                                diagram_model,
                                &InsensitiveCommand::MovePositionalAll(delta),
                                &mut u,
                                &mut m,
                            );
                            self.region_views
                                .insert(view_pos.try_into().unwrap(), view.clone());
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

                    if *bucket == 0
                        || *bucket == UmlStateMachineCompositeState::INTERNAL_TRANSITIONS_BUCKET
                    {
                        self.internal_transition_views.retain(|e| {
                            let r = e.read();
                            if *r.uuid == *element
                                && let Some((b, pos)) = diagram_model
                                    .write()
                                    .remove_element_from(model_uuid, &r.model_uuid())
                            {
                                undo_accumulator.push(InsensitiveCommand::AddDependency {
                                    target: *self.uuid,
                                    bucket: b,
                                    position: Some(pos),
                                    element: UmlStateMachineElementOrVertex::Element(
                                        e.clone().into(),
                                    ),
                                    into_model: true,
                                });
                                if *including_model {
                                    affected_models.insert(model_uuid);
                                }
                                false
                            } else {
                                true
                            }
                        });
                    }

                    if (*bucket == 0 || *bucket == UmlStateMachineCompositeState::REGIONS_BUCKET)
                        && let Some(view) = self
                            .region_views
                            .iter()
                            .find(|v| *v.read().uuid == *element)
                            .cloned()
                    {
                        let child_model_uuid = *view.read().model_uuid();

                        if let Some((_b, pos)) = diagram_model
                            .write()
                            .remove_element_from(model_uuid, &child_model_uuid)
                        {
                            undo_accumulator.push(InsensitiveCommand::AddDependency {
                                target: *self.uuid,
                                bucket: *bucket,
                                position: Some(pos),
                                element: UmlStateMachineElementView::from(view.clone()).into(),
                                into_model: *including_model,
                            });

                            if *including_model {
                                affected_models.insert(model_uuid);
                            }

                            let mut delta = egui::Vec2::ZERO;
                            let (mut u, mut m) = Default::default();
                            let old_sections = std::mem::take(&mut self.region_views);
                            for e in old_sections {
                                let mut w = e.write();
                                if *w.uuid == *element {
                                    delta.y += w.bounds_rect.height();
                                } else {
                                    w.apply_command(
                                        diagram_model,
                                        &InsensitiveCommand::MovePositionalAll(-delta),
                                        &mut u,
                                        &mut m,
                                    );
                                    drop(w);
                                    self.region_views.push(e);
                                }
                            }
                        }
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {}
            InsensitiveCommand::MoveOrdinal(uuids, direction) => {
                let mut undo_uuids = HashSet::new();
                match direction {
                    UmlStateMachineOrdinalMovement::Up | UmlStateMachineOrdinalMovement::Down => {
                        let lifelines_iter: Box<
                            dyn Iterator<Item = &mut ERef<UmlStateMachineCompositeStateRegionView>>,
                        > = match direction {
                            UmlStateMachineOrdinalMovement::Up => {
                                Box::new(self.region_views.iter_mut())
                            }
                            UmlStateMachineOrdinalMovement::Down => {
                                Box::new(self.region_views.iter_mut().rev())
                            }
                        };
                        let mut lifelines_iter = lifelines_iter.peekable();
                        while let Some(dest) = lifelines_iter.next()
                            && let Some(src) = lifelines_iter.peek_mut()
                        {
                            if uuids.contains(&src.read().uuid)
                                && !uuids.contains(&dest.read().uuid)
                            {
                                {
                                    let (mut srcw, mut destw) = (src.write(), dest.write());
                                    let mut w = self.model.write();
                                    let Some(new_pos) = w.get_element_pos(&destw.model_uuid())
                                    else {
                                        continue;
                                    };
                                    w.move_element(&srcw.model_uuid(), 0, new_pos.1);
                                    undo_uuids.insert(*srcw.uuid);
                                    let (src_d, dest_d) = match direction {
                                        UmlStateMachineOrdinalMovement::Up => (
                                            (0.0, -destw.bounds_rect.height()).into(),
                                            (0.0, srcw.bounds_rect.height()).into(),
                                        ),
                                        UmlStateMachineOrdinalMovement::Down => (
                                            (0.0, destw.bounds_rect.height()).into(),
                                            (0.0, -srcw.bounds_rect.height()).into(),
                                        ),
                                    };
                                    let (mut u, mut m) = Default::default();
                                    srcw.apply_command(
                                        diagram_model,
                                        &InsensitiveCommand::MovePositionalAll(src_d),
                                        &mut u,
                                        &mut m,
                                    );
                                    destw.apply_command(
                                        diagram_model,
                                        &InsensitiveCommand::MovePositionalAll(dest_d),
                                        &mut u,
                                        &mut m,
                                    );
                                }
                                std::mem::swap(dest, *src);
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
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    match property {
                        UmlStateMachinePropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::StereotypeChange(
                                    model.stereotype.clone(),
                                ),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlStateMachinePropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
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
        let r = self.model.read();

        self.temporaries.stereotype_in_guillemets.clear();
        if !r.stereotype.is_empty() {
            self.temporaries.stereotype_in_guillemets = format!("«{}»", r.stereotype);
        }
        self.temporaries.stereotype_buffer = (*r.stereotype).clone();
        self.temporaries.name_buffer = (*r.name).clone();

        // Structural refresh
        let views_map = self
            .internal_transition_views
            .iter()
            .map(|e| (*e.read().model_uuid(), e.clone()))
            .collect::<HashMap<_, _>>();
        self.internal_transition_views = r
            .internal_transitions
            .iter()
            .flat_map(|e| views_map.get(&e.read().uuid).cloned())
            .collect();
        let views_map = self
            .region_views
            .iter()
            .map(|e| (*e.read().model_uuid(), e.clone()))
            .collect::<HashMap<_, _>>();
        self.region_views = r
            .regions
            .iter()
            .flat_map(|e| views_map.get(&e.read().uuid).cloned())
            .collect();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlStateMachineElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.internal_transition_views.iter().for_each(|s| {
            let mut w = s.write();
            w.head_count(
                flattened_views,
                flattened_views_status,
                flattened_represented_models,
            );
            flattened_views.insert(*w.uuid(), (s.clone().into(), *self.uuid));
        });

        self.region_views.iter().for_each(|s| {
            let mut w = s.write();
            w.head_count(
                flattened_views,
                flattened_views_status,
                flattened_represented_models,
            );
            flattened_views.insert(*w.uuid(), (s.clone().into(), *self.uuid));
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlStateMachineDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.internal_transition_views
                .iter()
                .for_each(|v| v.read().deep_copy_walk(requested, uuid_present, tlc, c, m));
            self.region_views
                .iter()
                .for_each(|v| v.read().deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlStateMachineDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(UmlStateMachineElement::CompositeState(m)) = m.get(&old_model.uuid)
        {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let mut inner = HashMap::new();
        let new_internal_transitions = self
            .internal_transition_views
            .iter()
            .map(|v| {
                let v = v.read();
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                let Some(UmlStateMachineElementView::InternalTransition(s)) = c.get(&v.uuid) else {
                    unreachable!()
                };
                s.clone()
            })
            .collect();
        let new_sections = self
            .region_views
            .iter()
            .map(|v| {
                let v = v.read();
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                let Some(UmlStateMachineElementView::CompositeStateRegion(s)) = c.get(&v.uuid)
                else {
                    unreachable!()
                };
                s.clone()
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            internal_transition_views: new_internal_transitions,
            region_views: new_sections,

            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlStateMachineDomain as Domain>::CommonElementT>,
    ) {
        self.internal_transition_views
            .iter_mut()
            .for_each(|v| v.write().deep_copy_relink(c, m));
        self.region_views
            .iter_mut()
            .for_each(|v| v.write().deep_copy_relink(c, m));
    }
}

pub fn new_umlstatemachine_compositestateregion(
    bounds_rect: egui::Rect,
) -> (
    ERef<UmlStateMachineCompositeStateRegion>,
    ERef<UmlStateMachineCompositeStateRegionView>,
) {
    let package_model = ERef::new(UmlStateMachineCompositeStateRegion::new(
        ModelUuid::now_v7(),
        Vec::new(),
    ));
    let package_view =
        new_umlstatemachine_compositestateregion_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_umlstatemachine_compositestateregion_view(
    model: ERef<UmlStateMachineCompositeStateRegion>,
    bounds_rect: egui::Rect,
) -> ERef<UmlStateMachineCompositeStateRegionView> {
    ERef::new(UmlStateMachineCompositeStateRegionView {
        uuid: ViewUuid::now_v7().into(),
        model,
        contained_elements: OrderedViews::new(Vec::new()),
        bounds_rect,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineCompositeStateRegionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachineCompositeStateRegion>,
    #[nh_context_serde(entity)]
    contained_elements: OrderedViews<UmlStateMachineElementView>,

    pub bounds_rect: egui::Rect,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlStateMachineCompositeStateRegionViewTemporaries,
}

#[derive(Clone, Default)]
struct UmlStateMachineCompositeStateRegionViewTemporaries {
    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
    all_elements: HashMap<ViewUuid, SelectionStatus>,
}

impl UmlStateMachineCompositeStateRegionView {
    fn handle_size(&self, ui_scale: f32) -> f32 {
        10.0_f32
            .min(self.bounds_rect.width() * ui_scale / 6.0)
            .min(self.bounds_rect.height() * ui_scale / 3.0)
    }

    fn show_properties_inner(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlStateMachineDomain as Domain>::OrdinalMovementT,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> (
        PropertiesStatus<UmlStateMachineDomain>,
        Option<UmlStateMachineOrdinalMovement>,
    ) {
        let mut add_sibling = None::<UmlStateMachineOrdinalMovement>;

        if let Some(child) = self.contained_elements.event_order_find_mut(|v| {
            v.show_properties(drawing_context, q, ui, commands)
                .non_default()
        }) {
            return (child, add_sibling);
        }

        if !self.temporaries.highlight.selected {
            return (PropertiesStatus::NotShown, add_sibling);
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.bounds_rect.left_top();

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(x - self.bounds_rect.left(), 0.0),
                ));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MovePositional(
                    q.selected_views(),
                    egui::Vec2::new(0.0, y - self.bounds_rect.top()),
                ));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Add sibling up").clicked() {
                add_sibling = Some(UmlStateMachineOrdinalMovement::Up);
            }
            if ui.button("Add sibling down").clicked() {
                add_sibling = Some(UmlStateMachineOrdinalMovement::Down);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    UmlStateMachineOrdinalMovement::Up,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    UmlStateMachineOrdinalMovement::Down,
                ));
            }
        });

        (PropertiesStatus::Shown, add_sibling)
    }
}

impl Entity for UmlStateMachineCompositeStateRegionView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineCompositeStateRegionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineCompositeStateRegionView {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
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

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineCompositeStateRegionView {
    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                <UmlStateMachineDomain as Domain>::OrdinalMovementT,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        unreachable!()
    }

    fn draw_in(
        &mut self,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlStateMachineSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlStateMachineTool)>,
    ) -> TargettingStatus {
        canvas.draw_rectangle(
            self.bounds_rect.shrink(2.0),
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            self.temporaries.highlight,
        );

        // Draw resize handles
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            let handle_size = self.handle_size(ui_scale);
            let handles_rect = self.bounds_rect.shrink(handle_size / 2.0 / ui_scale);
            for (h, c) in [
                (handles_rect.center_top(), NHIcon::ArrowUp),
                (handles_rect.center_bottom(), NHIcon::ArrowDown),
            ] {
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size / ui_scale)),
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                c.draw(canvas, h, 8.0 / ui_scale, egui::Color32::BLACK);
            }
        }

        macro_rules! draw_header_and_children {
            () => {{
                let mut targetting_drawn = false;
                self.contained_elements.draw_order_foreach_mut(|e| {
                    targetting_drawn |=
                        e.draw_in(q, context, settings, canvas, tool) != TargettingStatus::NotDrawn;
                });
                targetting_drawn
            }};
        }

        if draw_header_and_children!() {
            return TargettingStatus::Drawn;
        }

        let Some((_, tool)) = tool.filter(|(pos, _)| self.bounds_rect.contains(*pos)) else {
            return TargettingStatus::NotDrawn;
        };

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            tool.targetting_for_section(Ok(self.model.clone().into())),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );
        draw_header_and_children!();
        TargettingStatus::Drawn
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid, self.min_shape());

        self.contained_elements
            .event_order_foreach_mut(|v| v.collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlStateMachineTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        let k_status = self.contained_elements.event_order_find_mut(|v| {
            let s = v.handle_event(event, ehc, settings, q, tool, element_setup_modal, commands);
            if s != EventHandlingStatus::NotHandled {
                Some((*v.uuid(), s))
            } else {
                None
            }
        });

        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if k_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) => {
                let handle_size = self.handle_size(1.0);
                let handles_rect = self.bounds_rect.shrink(handle_size / 2.0 / ehc.ui_scale);
                if self.temporaries.highlight.selected {
                    for (a, h) in [
                        (egui::Align2::CENTER_BOTTOM, handles_rect.center_top()),
                        (egui::Align2::CENTER_TOP, handles_rect.center_bottom()),
                    ] {
                        if egui::Rect::from_center_size(
                            h,
                            egui::Vec2::splat(handle_size) / ehc.ui_scale,
                        )
                        .contains(pos)
                        {
                            self.temporaries.dragged_type_and_shape =
                                Some((PackageDragType::Resize(a), self.bounds_rect));
                            return EventHandlingStatus::HandledByElement;
                        }
                    }
                }

                EventHandlingStatus::NotHandled
            }
            InputEvent::MouseUp(_pos) => {
                if self.temporaries.dragged_type_and_shape.is_some() {
                    self.temporaries.dragged_type_and_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) if self.bounds_rect.contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());

                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands)
                        && ehc
                            .modifier_settings
                            .alternative_tool_mode
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                    {
                        *element_setup_modal = esm;
                    }

                    EventHandlingStatus::HandledByContainer
                } else if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc
                            .modifier_settings
                            .hold_selection
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            commands.push(InsensitiveCommand::HighlightAll(
                                false,
                                canvas::Highlight::SELECTED,
                            ));
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                canvas::Highlight::SELECTED,
                            ));
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.temporaries.selected_direct_elements.contains(&k),
                                canvas::Highlight::SELECTED,
                            ));
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            }
            InputEvent::Drag { delta, .. } => match self.temporaries.dragged_type_and_shape {
                Some((PackageDragType::Move, real_bounds)) => {
                    let translated_bounds = real_bounds.translate(delta);
                    self.temporaries.dragged_type_and_shape =
                        Some((PackageDragType::Move, translated_bounds));
                    let translated_real_shape = canvas::NHShape::Rect {
                        inner: translated_bounds,
                    };
                    let coerced_pos = ehc
                        .snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid);
                    let coerced_delta = coerced_pos - self.position();

                    if self.temporaries.highlight.selected {
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
                Some((PackageDragType::Resize(align), real_bounds)) => {
                    let (left, right) = match align.x() {
                        egui::Align::Min => (0.0, delta.x),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => (-delta.x, 0.0),
                    };
                    let (top, bottom) = match align.y() {
                        egui::Align::Min => (0.0, delta.y),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => (-delta.y, 0.0),
                    };
                    let new_real_bounds = real_bounds
                        + egui::epaint::MarginF32 {
                            left,
                            right,
                            top,
                            bottom,
                        };
                    self.temporaries.dragged_type_and_shape =
                        Some((PackageDragType::Resize(align), new_real_bounds));
                    let handle_x = match align.x() {
                        egui::Align::Min => (new_real_bounds.right(), self.bounds_rect.right()),
                        egui::Align::Center => {
                            (new_real_bounds.center().x, self.bounds_rect.center().x)
                        }
                        egui::Align::Max => (new_real_bounds.left(), self.bounds_rect.left()),
                    };
                    let handle_y = match align.y() {
                        egui::Align::Min => (new_real_bounds.bottom(), self.bounds_rect.bottom()),
                        egui::Align::Center => {
                            (new_real_bounds.center().y, self.bounds_rect.center().y)
                        }
                        egui::Align::Max => (new_real_bounds.top(), self.bounds_rect.top()),
                    };
                    let coerced_point = ehc.snap_manager.coerce(
                        canvas::NHShape::Rect {
                            inner: egui::Rect::from_min_size(
                                egui::Pos2::new(handle_x.0, handle_y.0),
                                egui::Vec2::ZERO,
                            ),
                        },
                        |e| {
                            !ehc.all_elements
                                .get(e)
                                .is_some_and(|e| *e != SelectionStatus::NotSelected)
                        },
                    );
                    let coerced_delta = coerced_point - egui::Pos2::new(handle_x.1, handle_y.1);

                    commands.push(InsensitiveCommand::ResizeElementsBy(
                        q.selected_views(),
                        align,
                        coerced_delta,
                    ));
                    EventHandlingStatus::HandledByElement
                }
                None => EventHandlingStatus::NotHandled,
            },
            _ => k_status
                .map(|e| e.1)
                .unwrap_or(EventHandlingStatus::NotHandled),
        }
    }

    fn apply_command(
        &mut self,
        diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.contained_elements.event_order_foreach_mut(|v| {
                    v.apply_command(diagram_model, command, undo_accumulator, affected_models)
                });
            };
        }
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                if h.selected {
                    match set {
                        true => {
                            self.temporaries.selected_direct_elements =
                                self.contained_elements.iter_event_order_keys().collect()
                        }
                        false => self.temporaries.selected_direct_elements.clear(),
                    }
                }
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                }

                if h.selected {
                    for k in self
                        .contained_elements
                        .iter_event_order_keys()
                        .filter(|k| uuids.contains(k))
                    {
                        match set {
                            true => self.temporaries.selected_direct_elements.insert(k),
                            false => self.temporaries.selected_direct_elements.remove(&k),
                        };
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected = (self.temporaries.highlight.selected
                    && *retain)
                    || self.min_shape().contained_within(*rect);

                recurse!();
            }
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.contained_elements.event_order_foreach_mut(|v| {
                    v.apply_command(
                        diagram_model,
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {
                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for (_uuid, element) in self
                    .contained_elements
                    .iter_event_order_pairs()
                    .filter(|e| uuids.contains(&e.0))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
                    } else if let Some((b, pos)) =
                        self.model.read().get_element_pos(&element.model_uuid())
                    {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid,
                        bucket: b,
                        position: pos,
                        element: element.clone().into(),
                        into_model: false,
                    });
                }
                self.contained_elements.retain(|k, _v| !uuids.contains(k));

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
                    if *bucket == 0
                        && let Ok(mut view) = UmlStateMachineElementView::try_from(element.clone())
                        && (!*into_model
                            || diagram_model
                                .write()
                                .insert_element_into(model_uuid, *bucket, *position, view.model())
                                .is_ok())
                    {
                        let uuid = *view.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            element: uuid,
                            including_model: *into_model,
                        });

                        if *into_model {
                            affected_models.insert(model_uuid);
                        }
                        let mut model_transitives = HashMap::new();
                        view.head_count(
                            &mut HashMap::new(),
                            &mut HashMap::new(),
                            &mut model_transitives,
                        );
                        affected_models.extend(model_transitives.into_keys());

                        self.contained_elements.push(uuid, view);
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
                    if *bucket == 0
                        && let Some(view) = self.contained_elements.get(element)
                        && let Some((_b, pos)) = diagram_model
                            .write()
                            .remove_element_from(model_uuid, &view.model_uuid())
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            position: Some(pos),
                            element: view.clone().into(),
                            into_model: *including_model,
                        });

                        if *including_model {
                            affected_models.insert(model_uuid);
                        }

                        self.contained_elements.retain(|k, _v| *k != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {}
            InsensitiveCommand::MoveOrdinal(..) => {
                recurse!();
            }
            InsensitiveCommand::PropertyChange(uuids, _property) => {
                if uuids.contains(&self.uuid) {
                    let model = self.model.read();
                    affected_models.insert(*model.uuid);
                }
                recurse!();
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {}

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlStateMachineElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.temporaries.all_elements.clear();
        self.contained_elements.event_order_foreach_mut(|v| {
            v.head_count(
                flattened_views,
                &mut self.temporaries.all_elements,
                flattened_represented_models,
            )
        });
        for e in &self.temporaries.all_elements {
            flattened_views_status.insert(
                *e.0,
                match *e.1 {
                    SelectionStatus::NotSelected if self.temporaries.highlight.selected => {
                        SelectionStatus::TransitivelySelected
                    }
                    e => e,
                },
            );
        }

        self.contained_elements.event_order_foreach_mut(|v| {
            flattened_views.insert(*v.uuid(), (v.clone(), *self.uuid));
        });
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        c: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model =
            if let Some(UmlStateMachineElement::CompositeStateRegion(m)) = m.get(&old_model.uuid) {
                m.clone()
            } else {
                old_model.deep_copy_clone_inner(model_uuid, m)
            };

        let mut inner = HashMap::new();
        self.contained_elements
            .event_order_foreach(|v| v.deep_copy_clone(uuid_present, &mut inner, c, m));

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            contained_elements: OrderedViews::new(inner.into_values().collect()),

            bounds_rect: self.bounds_rect,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlStateMachineDomain as Domain>::CommonElementT>,
    ) {
        self.contained_elements
            .event_order_foreach_mut(|v| v.deep_copy_relink(c, m));
    }
}

fn nonfinal_node_button_rect(
    origin: egui::Pos2,
    ui_scale: f32,
    row_index: usize,
    column_index: usize,
) -> egui::Rect {
    const BUTTON_RADIUS: f32 = 8.0;
    let b_center = origin
        + egui::Vec2::new(
            (1.0 + 2.0 * column_index as f32) * BUTTON_RADIUS / ui_scale,
            (1.0 + 2.0 * row_index as f32) * BUTTON_RADIUS / ui_scale,
        );
    egui::Rect::from_center_size(b_center, egui::Vec2::splat(2.0 * BUTTON_RADIUS / ui_scale))
}
fn draw_button_rects(
    buttons: &[(usize, usize, &'static str, &NonFinalStateButtonF)],
    canvas: &mut dyn NHCanvas,
    origin: egui::Pos2,
    ui_scale: f32,
) {
    for (row_idx, col_idx, l, _f) in buttons {
        let r = nonfinal_node_button_rect(origin, ui_scale, *row_idx, *col_idx);
        canvas.draw_rectangle(
            r,
            egui::CornerRadius::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );
        canvas.draw_text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            l,
            14.0 / ui_scale,
            egui::Color32::BLACK,
        );
    }
}
fn handle_button_click<'a>(
    buttons: &'a [(usize, usize, &'static str, &'static NonFinalStateButtonF)],
    origin: egui::Pos2,
    ui_scale: f32,
    click_pos: egui::Pos2,
) -> Option<&'a NonFinalStateButtonF> {
    for (row_idx, col_idx, _l, f) in buttons {
        let r = nonfinal_node_button_rect(origin, ui_scale, *row_idx, *col_idx);
        if r.contains(click_pos) {
            return Some(f);
        }
    }
    None
}

fn new_umlstatemachine_internaltransition(
    trigger: &str,
    guard: &str,
    behavior: &str,
) -> (
    ERef<UmlStateMachineInternalTransition>,
    ERef<UmlStateMachineInternalTransitionView>,
) {
    let model = ERef::new(UmlStateMachineInternalTransition::new(
        ModelUuid::now_v7(),
        trigger.to_owned(),
        guard.to_owned(),
        behavior.to_owned(),
    ));
    let view = new_umlstatemachine_internaltransition_view(model.clone());

    (model, view)
}
fn new_umlstatemachine_internaltransition_view(
    model: ERef<UmlStateMachineInternalTransition>,
) -> ERef<UmlStateMachineInternalTransitionView> {
    ERef::new(UmlStateMachineInternalTransitionView {
        uuid: ViewUuid::now_v7().into(),
        model,

        display_text: String::new(),
        trigger_buffer: String::new(),
        guard_buffer: String::new(),
        behavior_buffer: String::new(),

        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::ZERO,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineInternalTransitionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlStateMachineInternalTransition>,

    #[nh_context_serde(skip_and_default)]
    display_text: String,
    #[nh_context_serde(skip_and_default)]
    trigger_buffer: String,
    #[nh_context_serde(skip_and_default)]
    guard_buffer: String,
    #[nh_context_serde(skip_and_default)]
    behavior_buffer: String,

    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,
}

impl UmlStateMachineInternalTransitionView {
    fn show_properties_inner(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlStateMachineDomain as Domain>::OrdinalMovementT,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> (
        PropertiesStatus<UmlStateMachineDomain>,
        Option<UmlStateMachineOrdinalMovement>,
    ) {
        let mut add_sibling = None::<UmlStateMachineOrdinalMovement>;

        if !self.highlight.selected {
            return (PropertiesStatus::NotShown, add_sibling);
        }

        if ui
            .labeled_text_edit_singleline("Trigger:", &mut self.trigger_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::NameChange(Arc::new(self.trigger_buffer.clone())),
            ));
        }

        let mut guard_label = egui::RichText::new("Guard:");
        if matches!(self.trigger_buffer.as_str(), "entry" | "do" | "exit") {
            guard_label = guard_label.strikethrough();
        }
        if ui
            .labeled_text_edit_singleline(guard_label, &mut self.guard_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::TransitionGuardChange(Arc::new(
                    self.guard_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Behavior:", &mut self.behavior_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::TransitionBehaviorChange(Arc::new(
                    self.behavior_buffer.clone(),
                )),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlStateMachineOrdinalMovement::Up,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlStateMachineOrdinalMovement::Down,
                ));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Add sibling up").clicked() {
                add_sibling = Some(UmlStateMachineOrdinalMovement::Up);
            }
            if ui.button("Add sibling down").clicked() {
                add_sibling = Some(UmlStateMachineOrdinalMovement::Down);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    UmlStateMachineOrdinalMovement::Up,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    UmlStateMachineOrdinalMovement::Down,
                ));
            }
        });

        (PropertiesStatus::Shown, add_sibling)
    }

    fn draw_inner(
        &mut self,
        at: egui::Pos2,
        align: egui::Align2,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlStateMachineTool)>,
    ) -> (egui::Rect, TargettingStatus) {
        self.bounds_rect =
            canvas.measure_text(at, align, &self.display_text, canvas::CLASS_ITEM_FONT_SIZE);
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            self.highlight,
        );
        canvas.draw_text(
            at,
            align,
            &self.display_text,
            canvas::CLASS_ITEM_FONT_SIZE,
            egui::Color32::BLACK,
        );
        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && self.bounds_rect.contains(*pos)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                tool.targetting_for_section(Ok(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                canvas::Highlight::NONE,
            );

            (self.bounds_rect, TargettingStatus::Drawn)
        } else {
            (self.bounds_rect, TargettingStatus::NotDrawn)
        }
    }
}

impl Entity for UmlStateMachineInternalTransitionView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineInternalTransitionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineInternalTransitionView {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
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

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineInternalTransitionView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        unreachable!()
    }

    fn draw_in(
        &mut self,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlStateMachineDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            q,
            context,
            settings,
            canvas,
            tool,
        )
        .1
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlStateMachineDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            <UmlStateMachineDomain as Domain>::AddCommandElementT,
            <UmlStateMachineDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
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
            InsensitiveCommand::MovePositional(..)
            | InsensitiveCommand::MovePositionalAll(..)
            | InsensitiveCommand::ResizeElementsBy(..)
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
                        UmlStateMachinePropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::NameChange(model.trigger.clone()),
                            ));
                            model.trigger = name.clone();
                        }
                        UmlStateMachinePropChange::TransitionGuardChange(guard) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::TransitionGuardChange(
                                    model.guard.clone(),
                                ),
                            ));
                            model.guard = guard.clone();
                        }
                        UmlStateMachinePropChange::TransitionBehaviorChange(behavior) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::TransitionBehaviorChange(
                                    model.behavior.clone(),
                                ),
                            ));
                            model.behavior = behavior.clone();
                        }
                        _ => {}
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {
        let m = self.model.read();

        self.display_text = {
            let mut t = (*m.trigger).clone();

            if !m.guard.is_empty() && !matches!(m.trigger.as_str(), "entry" | "do" | "exit") {
                t.push_str(" [");
                t.push_str(&m.guard);
                t.push_str("]");
            }

            t.push_str(" / ");
            t.push_str(&m.behavior);

            t
        };

        self.trigger_buffer = (*m.trigger).clone();
        self.guard_buffer = (*m.guard).clone();
        self.behavior_buffer = (*m.behavior).clone();
    }
    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<
            ViewUuid,
            (
                <UmlStateMachineDomain as Domain>::CommonElementViewT,
                ViewUuid,
            ),
        >,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlStateMachineDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish =
            if let Some(UmlStateMachineElement::InternalTransition(m)) = m.get(&old_model.uuid) {
                m.clone()
            } else {
                old_model.deep_copy_clone_inner(model_uuid, m)
            };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,

            display_text: self.display_text.clone(),
            trigger_buffer: self.trigger_buffer.clone(),
            guard_buffer: self.guard_buffer.clone(),
            behavior_buffer: self.behavior_buffer.clone(),

            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_umlstatemachine_initialpseudostate(
    position: egui::Pos2,
) -> (
    ERef<UmlStateMachineInitialPseudostate>,
    ERef<UmlStateMachineInitialPseudostateView>,
) {
    let model = ERef::new(UmlStateMachineInitialPseudostate::new(ModelUuid::now_v7()));
    let view = new_umlstatemachine_initialpseudostate_view(model.clone(), position);

    (model, view)
}

fn new_umlstatemachine_initialpseudostate_view(
    model: ERef<UmlStateMachineInitialPseudostate>,
    position: egui::Pos2,
) -> ERef<UmlStateMachineInitialPseudostateView> {
    ERef::new(UmlStateMachineInitialPseudostateView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineInitialPseudostateView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlStateMachineInitialPseudostate>,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    position: egui::Pos2,
}

impl UmlStateMachineInitialPseudostateView {
    const CIRCLE_RADIUS: f32 = 15.0;
    fn buttons_origin(&self) -> egui::Pos2 {
        self.position + egui::Vec2::new(Self::CIRCLE_RADIUS, -Self::CIRCLE_RADIUS)
    }
}

impl Entity for UmlStateMachineInitialPseudostateView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineInitialPseudostateView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineInitialPseudostateView {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse {
            position: self.position,
            bounds_radius: egui::Vec2::splat(Self::CIRCLE_RADIUS),
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineInitialPseudostateView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
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
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlStateMachineDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        canvas.draw_ellipse(
            self.position,
            egui::Vec2::splat(Self::CIRCLE_RADIUS),
            egui::Color32::BLACK,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            self.highlight,
        );

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            draw_button_rects(
                &settings.nonfinal_buttons,
                canvas,
                self.buttons_origin(),
                ui_scale,
            );
        }

        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && self.min_shape().contains(*pos)
        {
            canvas.draw_ellipse(
                self.position,
                egui::Vec2::splat(Self::CIRCLE_RADIUS),
                tool.targetting_for_section(Ok(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
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
        settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<UmlStateMachineDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
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
            InputEvent::Click(pos)
                if self.highlight.selected
                    && let Some(f) = handle_button_click(
                        &settings.nonfinal_buttons,
                        self.buttons_origin(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlStateMachineTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage,
                    current_stage,
                    result,
                    event_lock,
                    is_spent: Some(false),
                });
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model());
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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            <UmlStateMachineDomain as Domain>::AddCommandElementT,
            <UmlStateMachineDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                <UmlStateMachineDomain as Domain>::AddCommandElementT,
                <UmlStateMachineDomain as Domain>::PropChangeT,
            >,
        >,
        _affected_models: &mut HashSet<ModelUuid>,
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
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(..) => {}
            InsensitiveCommand::AddDependency { .. } => {}
            InsensitiveCommand::RemoveDependency { .. } => {}
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(..) => {}
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {}
    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<
            ViewUuid,
            (
                <UmlStateMachineDomain as Domain>::CommonElementViewT,
                ViewUuid,
            ),
        >,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlStateMachineDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlStateMachineDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish =
            if let Some(UmlStateMachineElement::InitialPseudostate(m)) = m.get(&old_model.uuid) {
                m.clone()
            } else {
                old_model.deep_copy_clone_inner(model_uuid, m)
            };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,

            dragged_shape: self.dragged_shape,
            highlight: self.highlight,
            position: self.position,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlstatemachine_terminatepseudostate(
    position: egui::Pos2,
) -> (
    ERef<UmlStateMachineTerminatePseudostate>,
    ERef<UmlStateMachineTerminatePseudostateView>,
) {
    let node_model = ERef::new(UmlStateMachineTerminatePseudostate::new(ModelUuid::now_v7()));
    let node_view = new_umlstatemachine_terminatepseudostate_view(node_model.clone(), position);

    (node_model, node_view)
}
pub fn new_umlstatemachine_terminatepseudostate_view(
    model: ERef<UmlStateMachineTerminatePseudostate>,
    position: egui::Pos2,
) -> ERef<UmlStateMachineTerminatePseudostateView> {
    ERef::new(UmlStateMachineTerminatePseudostateView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineTerminatePseudostateView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlStateMachineTerminatePseudostate>,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
}

impl UmlStateMachineTerminatePseudostateView {
    const RADIUS_INCREMENT: f32 = 10.0;
}

impl Entity for UmlStateMachineTerminatePseudostateView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineTerminatePseudostateView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineTerminatePseudostateView {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse {
            position: self.position,
            bounds_radius: egui::Vec2::splat(
                UmlStateMachineInitialPseudostateView::CIRCLE_RADIUS + Self::RADIUS_INCREMENT,
            ),
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineTerminatePseudostateView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
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
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlStateMachineTool)>,
    ) -> TargettingStatus {
        let r = UmlStateMachineInitialPseudostateView::CIRCLE_RADIUS + Self::RADIUS_INCREMENT;
        let sin45 = 0.70;

        for e in [1.0, -1.0, -1.0, 1.0, 1.0].array_windows::<4>() {
            canvas.draw_line(
                [
                    self.position + egui::Vec2::new(e[0] * r * sin45, e[1] * r * sin45),
                    self.position + egui::Vec2::new(e[2] * r * sin45, e[3] * r * sin45),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );
        }

        if canvas.ui_scale().is_some() {
            // Draw targetting ellipse
            if let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
            {
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(r),
                    t.targetting_for_section(Ok(self.model())),
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
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlStateMachineTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
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
                    tool.add_section(self.model());
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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
        _affected_models: &mut HashSet<ModelUuid>,
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
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(..) => {}
            InsensitiveCommand::AddDependency { .. } => {}
            InsensitiveCommand::RemoveDependency { .. } => {}
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(..) => {}
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {}

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlStateMachineElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        c: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish =
            if let Some(UmlStateMachineElement::TerminatePseudostate(m)) = m.get(&old_model.uuid) {
                m.clone()
            } else {
                old_model.deep_copy_clone_inner(model_uuid, m)
            };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlstatemachine_finalstate(
    position: egui::Pos2,
) -> (
    ERef<UmlStateMachineFinalState>,
    ERef<UmlStateMachineFinalStateView>,
) {
    let node_model = ERef::new(UmlStateMachineFinalState::new(ModelUuid::now_v7()));
    let node_view = new_umlstatemachine_finalstate_view(node_model.clone(), position);

    (node_model, node_view)
}
pub fn new_umlstatemachine_finalstate_view(
    model: ERef<UmlStateMachineFinalState>,
    position: egui::Pos2,
) -> ERef<UmlStateMachineFinalStateView> {
    ERef::new(UmlStateMachineFinalStateView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlStateMachineFinalStateView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlStateMachineFinalState>,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
}

impl UmlStateMachineFinalStateView {
    const RADIUS_INCREMENT: f32 = 10.0;
}

impl Entity for UmlStateMachineFinalStateView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineFinalStateView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineFinalStateView {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse {
            position: self.position,
            bounds_radius: egui::Vec2::splat(
                UmlStateMachineInitialPseudostateView::CIRCLE_RADIUS + Self::RADIUS_INCREMENT,
            ),
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineFinalStateView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
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
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlStateMachineTool)>,
    ) -> TargettingStatus {
        let r = UmlStateMachineInitialPseudostateView::CIRCLE_RADIUS + Self::RADIUS_INCREMENT;

        canvas.draw_ellipse(
            self.position,
            egui::Vec2::splat(r),
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_ellipse(
            self.position,
            egui::Vec2::splat(UmlStateMachineInitialPseudostateView::CIRCLE_RADIUS),
            egui::Color32::BLACK,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            canvas::Highlight::NONE,
        );

        if canvas.ui_scale().is_some() {
            // Draw targetting ellipse
            if let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
            {
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(r),
                    t.targetting_for_section(Ok(self.model())),
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
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlStateMachineTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
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
                    tool.add_section(self.model());
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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
        _affected_models: &mut HashSet<ModelUuid>,
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
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(..) => {}
            InsensitiveCommand::AddDependency { .. } => {}
            InsensitiveCommand::RemoveDependency { .. } => {}
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(..) => {}
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {}

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlStateMachineElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        c: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlStateMachineElement::FinalState(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlstatemachine_edge(
    name: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (UmlStateMachineNonFinalNode, UmlStateMachineElementView),
    target: (UmlStateMachineNonInitialNode, UmlStateMachineElementView),
) -> (ERef<UmlStateMachineEdge>, ERef<EdgeViewT>) {
    let link_model = ERef::new(UmlStateMachineEdge::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        source.0,
        target.0,
    ));
    let link_view =
        new_umlstatemachine_edge_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlstatemachine_edge_view(
    model: ERef<UmlStateMachineEdge>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlStateMachineElementView,
    target: UmlStateMachineElementView,
) -> ERef<EdgeViewT> {
    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once((*source.uuid(), source.min_shape())),
        std::iter::once(*target.uuid()),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlStateMachineEdgeAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlStateMachineEdgeAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachineEdge>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlStateMachineEdgeTemporaries,
}

#[derive(Clone, Default)]
struct UmlStateMachineEdgeTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    midpoint_label: Option<Arc<String>>,
    name_buffer: String,
}

impl MulticonnectionAdapter<UmlStateMachineDomain> for UmlStateMachineEdgeAdapter {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _sources: &Vec<Ending<UmlStateMachineElementView>>,
        _targets: &Vec<Ending<UmlStateMachineElementView>>,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlStateMachineDomain as Domain>::ToolT)>,
    ) -> Result<(), Arc<String>> {
        match self.temporaries.midpoint_label.clone() {
            None => Ok(()),
            Some(label) => Err(label),
        }
    }

    fn arrow_data(&self) -> &HashMap<(bool, ModelUuid), ArrowData> {
        &self.temporaries.arrow_data
    }

    fn source_uuids(&self) -> &[ModelUuid] {
        &self.temporaries.source_uuids
    }

    fn target_uuids(&self) -> &[ModelUuid] {
        &self.temporaries.target_uuids
    }

    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        let mut w = self.model.write();
        if let Some(new_source) = w.target.clone().to_element().as_nonfinal()
            && let Some(new_target) = w.source.clone().to_element().as_noninitial()
        {
            w.source = new_source;
            w.target = new_target;
            Ok(())
        } else {
            Err(())
        }
    }

    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::NameChange(Arc::new(
                    self.temporaries.name_buffer.clone(),
                )),
            ));
        }

        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.separator();

        PropertiesStatus::Shown
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlStateMachinePropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlStateMachinePropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(
        &mut self,
        _sources: &Vec<Ending<UmlStateMachineElementView>>,
        _targets: &Vec<Ending<UmlStateMachineElementView>>,
    ) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.uuid()),
            ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::OpenTriangle),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.midpoint_label = match model.name.is_empty() {
            true => None,
            false => Some(model.name.clone()),
        };
        self.temporaries.name_buffer = (*model.name).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlStateMachineElement::Edge(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(new_uuid, m)
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlStateMachineElement>) {
        self.model.write().deep_copy_relink(m);
    }
}

pub fn new_umlstatemachine_note(
    text: &str,
    stereotype: &str,
    position: egui::Pos2,
    align: egui::Align2,
    background_color: MGlobalColor,
) -> (ERef<UmlStateMachineNote>, ERef<UmlStateMachineNoteView>) {
    let comment_model = ERef::new(UmlStateMachineNote::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        text.to_owned(),
    ));
    let comment_view =
        new_umlstatemachine_note_view(comment_model.clone(), position, align, background_color);

    (comment_model, comment_view)
}
pub fn new_umlstatemachine_note_view(
    model: ERef<UmlStateMachineNote>,
    position: egui::Pos2,
    align: egui::Align2,
    background_color: MGlobalColor,
) -> ERef<UmlStateMachineNoteView> {
    let m = model.read();
    ERef::new(UmlStateMachineNoteView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        display_text: String::new(),
        stereotype_buffer: (*m.stereotype).clone(),
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
pub struct UmlStateMachineNoteView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlStateMachineNote>,

    #[nh_context_serde(skip_and_default)]
    display_text: String,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
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

impl UmlStateMachineNoteView {
    const CORNER_SIZE: f32 = 10.0;

    fn comment_link_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_radius = 8.0;
        let b_center = self.bounds_rect.right_top() + egui::Vec2::splat(b_radius / ui_scale);
        egui::Rect::from_center_size(b_center, egui::Vec2::splat(2.0 * b_radius / ui_scale))
    }
}

impl Entity for UmlStateMachineNoteView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlStateMachineNoteView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlStateMachineElement> for UmlStateMachineNoteView {
    fn model(&self) -> UmlStateMachineElement {
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

impl ElementControllerGen2<UmlStateMachineDomain> for UmlStateMachineNoteView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::StereotypeChange(Arc::new(
                    self.stereotype_buffer.clone(),
                )),
            ));
        }
        if ui
            .labeled_text_edit_multiline("Text:", &mut self.text_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlStateMachinePropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
                            UmlStateMachinePropChange::NoteAlignChange(Some(tmp_x), None),
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
                            UmlStateMachinePropChange::NoteAlignChange(None, Some(tmp_y)),
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
                UmlStateMachinePropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlStateMachineTool)>,
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
                &self.display_text,
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
            &self.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            let b_rect = self.comment_link_button_rect(ui_scale);
            canvas.draw_rectangle(
                b_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(
                b_rect.center(),
                egui::Align2::CENTER_CENTER,
                "\\",
                14.0 / ui_scale,
                egui::Color32::BLACK,
            );
        }

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
                    t.targetting_for_section(Ok(self.model())),
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
        _settings: &<UmlStateMachineDomain as Domain>::SettingsT,
        q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlStateMachineTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
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
            InputEvent::Click(pos) if self.comment_link_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveUmlStateMachineTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage: UmlStateMachineToolStage::NoteLinkStart,
                    current_stage: UmlStateMachineToolStage::NoteLinkEnd,
                    result: PartialUmlStateMachineElement::NoteLink {
                        source: self.model.clone(),
                        dest: None,
                    },
                    event_lock: true,
                    is_spent: Some(false),
                });

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model());
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
        _diagram_model: &ERef<UmlStateMachineDiagram>,
        command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
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
                        UmlStateMachinePropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::StereotypeChange(
                                    model.stereotype.clone(),
                                ),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlStateMachinePropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        UmlStateMachinePropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color,
                        }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlStateMachinePropChange::NoteAlignChange(x, y) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlStateMachinePropChange::NoteAlignChange(
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

        self.display_text = {
            let mut s = "".to_owned();
            if !model.stereotype.is_empty() {
                s.push('«');
                s.push_str(&model.stereotype);
                s.push_str("»\n");
            }
            s.push_str(&model.text);
            s
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.text_buffer = (*model.text).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlStateMachineElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        c: &mut HashMap<ViewUuid, UmlStateMachineElementView>,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlStateMachineElement::Note(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            display_text: self.display_text.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
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

pub fn new_umlstatemachine_notelink(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlStateMachineNote>, UmlStateMachineElementView),
    target: (UmlStateMachineElement, UmlStateMachineElementView),
) -> (ERef<UmlStateMachineNoteLink>, ERef<NoteLinkViewT>) {
    let link_model = ERef::new(UmlStateMachineNoteLink::new(
        ModelUuid::now_v7(),
        source.0,
        target.0,
    ));
    let link_view =
        new_umlstatemachine_notelink_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlstatemachine_notelink_view(
    model: ERef<UmlStateMachineNoteLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlStateMachineElementView,
    target: UmlStateMachineElementView,
) -> ERef<NoteLinkViewT> {
    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlStateMachineNoteLinkAdapter {
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
pub struct UmlStateMachineNoteLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlStateMachineNoteLink>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlStateMachineNoteLinkTemporaries,
}

#[derive(Clone, Default)]
struct UmlStateMachineNoteLinkTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
}

impl MulticonnectionAdapter<UmlStateMachineDomain> for UmlStateMachineNoteLinkAdapter {
    fn model(&self) -> UmlStateMachineElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn arrow_data(&self) -> &HashMap<(bool, ModelUuid), ArrowData> {
        &self.temporaries.arrow_data
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
        _q: &<UmlStateMachineDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlStateMachineDomain> {
        PropertiesStatus::NotShown
    }
    fn apply_change(
        &mut self,
        _view_uuid: &ViewUuid,
        _command: &InsensitiveCommand<
            UmlStateMachineOrdinalMovement,
            UmlStateMachineElementOrVertex,
            UmlStateMachinePropChange,
        >,
        _undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlStateMachineOrdinalMovement,
                UmlStateMachineElementOrVertex,
                UmlStateMachinePropChange,
            >,
        >,
    ) {
    }
    fn refresh_buffers(
        &mut self,
        _sources: &Vec<Ending<UmlStateMachineElementView>>,
        _targets: &Vec<Ending<UmlStateMachineElementView>>,
    ) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.read().uuid),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries
            .source_uuids
            .push(*model.source.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlStateMachineElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlStateMachineElement::NoteLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(new_uuid, m)
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlStateMachineElement>) {
        self.model.write().deep_copy_relink(m);
    }
}
