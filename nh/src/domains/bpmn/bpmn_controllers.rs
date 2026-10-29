use crate::common::canvas::{self, NHCanvas, NHShape};
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
use crate::common::views::package_view::PackageDragType;
use crate::domains::bpmn::bpmn_models::{
    BpmnAnnotation, BpmnBoundaryIntermediateEvent, BpmnBoundaryIntermediateEventKind, BpmnData,
    BpmnDataKind, BpmnDiagram, BpmnElement, BpmnEndEvent, BpmnEvent, BpmnEventKind, BpmnGateway,
    BpmnGatewayKind, BpmnIntermediateEvent, BpmnIntermediateEventKind, BpmnNonFinalElement,
    BpmnNonInitialElement, BpmnPool, BpmnPoolSwimlane, BpmnSequenceEdge, BpmnSequenceEdgeKind,
    BpmnStartEvent, BpmnStartEventKind, BpmnSubprocess, BpmnSubprocessKind, BpmnTask, BpmnTaskKind,
};
use crate::{
    DefaultNameF, DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF,
    DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::{egui, epaint};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct BpmnDomain;
impl Domain for BpmnDomain {
    type SettingsT = BpmnSettings;
    type CommonElementT = BpmnElement;
    type DiagramModelT = BpmnDiagram;
    type CommonElementViewT = BpmnElementView;
    type ViewTargettingSectionT = BpmnElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveBpmnTool;
    type OrdinalMovementT = BpmnOrdinalMovement;
    type AddCommandElementT = BpmnElementOrVertex;
    type PropChangeT = BpmnPropChange;
}

type SequenceEdgeViewT = MulticonnectionView<BpmnDomain, BpmnSequenceEdgeAdapter>;
// type MessageEdgeViewT = MulticonnectionView<BpmnDomain, BpmnMessageEdgeAdapter>;
// type AssociationViewT = MulticonnectionView<BpmnDomain, BpmnAssociationAdapter>;

#[derive(Clone, Copy, Debug)]
pub enum BpmnOrdinalMovement {
    Up,
    Down,
}

impl BpmnOrdinalMovement {
    pub fn inverse(&self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

#[derive(Clone)]
pub enum BpmnPropChange {
    NameChange(Arc<String>),

    TaskKindChange(BpmnTaskKind),
    IsCallChange(bool),
    SubprocessKindChange(BpmnSubprocessKind),
    EventKindChange(BpmnEventKind),
    StartEventSubkindChange(BpmnStartEventKind),
    IntermediateEventSubkindChange(BpmnIntermediateEventKind),
    BoundaryIntermediateEventSubkindChange(BpmnBoundaryIntermediateEventKind),
    GatewayKindChange(BpmnGatewayKind),
    DataKindChange(BpmnDataKind),
    DataIsCollectionChange(bool),

    SequenceEdgeKindChange(BpmnSequenceEdgeKind),
    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
}

impl Debug for BpmnPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "BpmnPropChange::???")
    }
}

impl TryFrom<&BpmnPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &BpmnPropChange) -> Result<Self, Self::Error> {
        match value {
            BpmnPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for BpmnPropChange {
    fn from(value: ColorChangeData) -> Self {
        BpmnPropChange::ColorChange(value)
    }
}
impl TryFrom<BpmnPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: BpmnPropChange) -> Result<Self, Self::Error> {
        match value {
            BpmnPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for BpmnPropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum BpmnElementOrVertex {
    Element(BpmnElementView),
    Vertex(VertexInformation),
}

impl Debug for BpmnElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "BpmnElementOrVertex::???")
    }
}

impl TryFrom<BpmnElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: BpmnElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            BpmnElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<BpmnElementOrVertex> for BpmnElementView {
    type Error = ();

    fn try_from(value: BpmnElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            BpmnElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "BpmnDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum BpmnElementView {
    Task(ERef<BpmnTaskView>),
    Subprocess(ERef<BpmnSubprocessView>),
    Pool(ERef<BpmnPoolView>),
    PoolSwimlane(ERef<BpmnPoolSwimlaneView>),
    Event(ERef<BpmnEventView>),
    Gateway(ERef<BpmnGatewayView>),
    Data(ERef<BpmnDataView>),
    SequenceEdge(ERef<SequenceEdgeViewT>),
    Annotation(ERef<BpmnAnnotationView>),
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct BpmnControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<BpmnDiagram>,
}

impl ControllerAdapter<BpmnDomain> for BpmnControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<BpmnDomain, BpmnDiagramAdapter>;

    fn model(&self) -> ERef<BpmnDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<BpmnDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "bpmn"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::bpmn_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, BpmnElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("BPMN Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared BPMN Diagram".to_owned().into(),
                BpmnDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct BpmnDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<BpmnDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: BpmnDiagramBuffer,
}

#[derive(Clone, Default)]
struct BpmnDiagramBuffer {
    name: String,
    comment: String,
}

impl BpmnDiagramAdapter {
    pub fn new(model: ERef<BpmnDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: BpmnDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<BpmnDomain> for BpmnDiagramAdapter {
    fn model(&self) -> ERef<BpmnDiagram> {
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
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        element: BpmnElement,
    ) -> Result<BpmnElementView, HashSet<ModelUuid>> {
        let v = match element {
            BpmnElement::Task(inner) => {
                // TODO: attached events?
                new_bpmn_task_view(inner, egui::Pos2::ZERO, MGlobalColor::None, false).into()
            }
            BpmnElement::Subprocess(inner) => {
                // TODO: attached events?
                new_bpmn_subprocess_view(
                    inner,
                    egui::Rect::from_two_pos(egui::Pos2::ZERO, (100.0, 50.0).into()),
                    false,
                )
                .into()
            }
            BpmnElement::StartEvent(inner) => {
                new_bpmn_event_view(inner.into(), egui::Pos2::ZERO, false).into()
            }
            BpmnElement::IntermediateEvent(inner) => {
                new_bpmn_event_view(inner.into(), egui::Pos2::ZERO, false).into()
            }
            BpmnElement::BoundaryIntermediateEvent(inner) => {
                new_bpmn_event_view(inner.into(), egui::Pos2::ZERO, false).into()
            }
            BpmnElement::EndEvent(inner) => {
                new_bpmn_event_view(inner.into(), egui::Pos2::ZERO, false).into()
            }
            BpmnElement::Gateway(inner) => {
                new_bpmn_gateway_view(inner, egui::Pos2::ZERO, false).into()
            }
            BpmnElement::Pool(inner) => {
                let r = inner.read();
                let swimlane_views = r
                    .swimlanes
                    .iter()
                    .map(|e| {
                        new_bpmn_poolswimlane_view(
                            e.clone(),
                            egui::Rect::from_two_pos(egui::Pos2::ZERO, (100.0, 50.0).into()),
                        )
                    })
                    .collect();
                new_bpmn_pool_view(inner.clone(), swimlane_views).into()
            }
            BpmnElement::PoolSwimlane(inner) => new_bpmn_poolswimlane_view(
                inner,
                egui::Rect::from_two_pos(egui::Pos2::ZERO, (100.0, 50.0).into()),
            )
            .into(),
            BpmnElement::Data(inner) => {
                new_bpmn_data_view(inner, egui::Pos2::ZERO, MGlobalColor::None, false).into()
            }
            BpmnElement::SequenceEdge(inner) => {
                let r = inner.read();
                let source_uuid = *r.source.uuid();
                let target_uuid = *r.target.uuid();
                let (source, target) =
                    match (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid)) {
                        (Some(source), Some(target)) => (source, target),
                        _ => return Err([source_uuid, target_uuid].into_iter().collect()),
                    };
                new_bpmn_sequenceedge_view(inner.clone(), None, source, target).into()
            }
            BpmnElement::MessageEdge(_inner) => todo!(),
            BpmnElement::Association(_inner) => todo!(),
            BpmnElement::Annotation(inner) => {
                let r = inner.read();
                let target_uuid = *r.target.uuid();
                let Some(target) = q.get_view_for(&target_uuid) else {
                    return Err(std::iter::once(target_uuid).collect());
                };
                new_bpmn_annotation_view(inner.clone(), target, egui::Pos2::ZERO).into()
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &BpmnElement) -> Arc<String> {
        match e {
            BpmnElement::Task(inner) => {
                let r = inner.read();
                format!("{} Task ({})", r.kind.as_str(), r.name).into()
            }
            BpmnElement::Subprocess(inner) => {
                let r = inner.read();
                format!("{} ({})", r.kind.as_str(), r.name).into()
            }
            BpmnElement::StartEvent(inner) => {
                let r = inner.read();
                format!("{} Start Event", r.kind.as_str()).into()
            }
            BpmnElement::IntermediateEvent(inner) => {
                let r = inner.read();
                format!("{} Intermediate Event", r.kind.as_str()).into()
            }
            BpmnElement::EndEvent(inner) => {
                let r = inner.read();
                format!("{} End Event", r.kind.as_str()).into()
            }
            BpmnElement::BoundaryIntermediateEvent(inner) => {
                let r = inner.read();
                format!("{} Boundary Intermediate Event", r.kind.as_str()).into()
            }
            BpmnElement::Gateway(inner) => {
                let r = inner.read();
                format!("{} Gateway", r.kind.as_str()).into()
            }
            BpmnElement::Data(inner) => {
                let r = inner.read();
                format!("{} ({})", r.kind.as_str(), r.name).into()
            }
            BpmnElement::Pool(_inner) => format!("Pool").into(),
            BpmnElement::PoolSwimlane(inner) => format!("Swimlane ({})", inner.read().name).into(),
            BpmnElement::SequenceEdge(_inner) => format!("Sequence Edge").into(),
            BpmnElement::MessageEdge(_inner) => format!("Message Edge").into(),
            BpmnElement::Association(_inner) => format!("Association").into(),
            BpmnElement::Annotation(inner) => format!(
                "Annotation ({})",
                LabelProvider::filter_and_elipsis(&inner.read().text)
            )
            .into(),
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
        _settings: &BpmnSettings,
        (_, last_v): (u8, u8),
    ) -> (canvas::HeaderMode, canvas::HeaderMode) {
        (
            canvas::HeaderMode::Expanding(0),
            canvas::HeaderMode::Expanding(last_v.clamp(1, 5)),
        )
    }
    fn show_view_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
                BpmnPropChange::ColorChange((0, new_color).into()),
            ));
        }
    }
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        _drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                BpmnPropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                BpmnPropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                BpmnPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        BpmnPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                BpmnPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        BpmnPropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                BpmnPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        BpmnPropChange::CommentChange(model.comment.clone()),
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
        settings: &BpmnSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<BpmnDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveBpmnTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialBpmnElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, BpmnElement>) {
        let (new_model, models) = super::bpmn_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, BpmnElement>) {
        let models = super::bpmn_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
    fn top_sort_info(&self, m: &BpmnElement) -> crate::common::model::ModelTopSortInfo {
        super::bpmn_models::top_sort_info(m)
    }
}

fn new_controlller(
    model: ERef<BpmnDiagram>,
    name: String,
    elements: Vec<BpmnElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            BpmnControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                BpmnDiagramAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let diagram = ERef::new(BpmnDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![],
    ));
    new_controlller(diagram, name.to_owned(), vec![])
}

pub fn demo(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (start_event, start_event_view) = new_bpmn_startevent(
        BpmnEventKind::Message,
        BpmnStartEventKind::Interrupting,
        "",
        egui::Pos2::new(200.0, 200.0),
        false,
    );

    let (task, task_view) = new_bpmn_task(
        "Ship items",
        BpmnTaskKind::Unspecified,
        false,
        egui::Pos2::new(750.0, 200.0),
        MGlobalColor::None,
        false,
    );
    let (end_event, end_event_view) = new_bpmn_endevent(
        BpmnEventKind::Message,
        "",
        egui::Pos2::new(750.0, 350.0),
        false,
    );

    let diagram = ERef::new(BpmnDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![start_event.into(), task.into(), end_event.into()],
    ));
    new_controlller(
        diagram,
        name.to_owned(),
        vec![
            start_event_view.into(),
            task_view.into(),
            end_event_view.into(),
        ],
    )
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        BpmnDomain,
        BpmnControllerAdapter,
        DiagramControllerGen2<BpmnDomain, BpmnDiagramAdapter>,
    >>(&uuid)?)
}

pub struct BpmnSettings {
    palette: RwLock<ToolPalette<BpmnToolStage, BpmnDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<BpmnToolStage, BpmnElementView>>,
    nonfinal_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            BpmnNonFinalElement,
        ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool),
    )>,
}

impl DiagramSettings for BpmnSettings {
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
                            BpmnToolStage::Task {
                                name,
                                kind,
                                is_call,
                                background_color,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Kind:");
                                egui::ComboBox::from_id_salt("task kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnTaskKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).changed();
                                        }
                                    });

                                modified |= columns[1].checkbox(is_call, "is call").changed();

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
                            BpmnToolStage::SubprocessStart {
                                kind,
                                name,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Kind:");
                                egui::ComboBox::from_id_salt("subprocess kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnSubprocessKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).changed();
                                        }
                                    });
                            }
                            BpmnToolStage::PoolStart { swimlane_name } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Swimlane name", swimlane_name)
                                    .changed();
                            }
                            BpmnToolStage::StartEvent {
                                kind,
                                subkind,
                                name,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Event kind:");
                                egui::ComboBox::from_id_salt("event kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnEventKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).clicked();
                                        }
                                    });

                                columns[1].label("Start event kind:");
                                egui::ComboBox::from_id_salt("start event kind")
                                    .selected_text(subkind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnStartEventKind::VARIANTS {
                                            modified |= ui
                                                .selectable_value(subkind, e, e.as_str())
                                                .clicked();
                                        }
                                    });
                            }
                            BpmnToolStage::IntermediateEvent {
                                kind,
                                subkind,
                                name,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Event kind:");
                                egui::ComboBox::from_id_salt("event kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnEventKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).clicked();
                                        }
                                    });

                                columns[1].label("Intermediate event kind:");
                                egui::ComboBox::from_id_salt("intermediate event kind")
                                    .selected_text(subkind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnIntermediateEventKind::VARIANTS {
                                            modified |= ui
                                                .selectable_value(subkind, e, e.as_str())
                                                .clicked();
                                        }
                                    });
                            }
                            BpmnToolStage::EndEvent {
                                kind,
                                name,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Event kind:");
                                egui::ComboBox::from_id_salt("event kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnEventKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).clicked();
                                        }
                                    });
                            }
                            BpmnToolStage::BoundaryIntermediateEvent {
                                kind,
                                subkind,
                                name,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Event kind:");
                                egui::ComboBox::from_id_salt("event kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnEventKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).clicked();
                                        }
                                    });

                                columns[1].label("Boundary intermediate event kind:");
                                egui::ComboBox::from_id_salt("boundary intermediate event kind")
                                    .selected_text(subkind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnBoundaryIntermediateEventKind::VARIANTS {
                                            modified |= ui
                                                .selectable_value(subkind, e, e.as_str())
                                                .clicked();
                                        }
                                    });
                            }
                            BpmnToolStage::Gateway {
                                condition: name,
                                kind,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Condition", name)
                                    .changed();

                                columns[1].label("Kind:");
                                egui::ComboBox::from_id_salt("gateway kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnGatewayKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).clicked();
                                        }
                                    });
                            }
                            BpmnToolStage::Data {
                                kind,
                                name,
                                is_collection,
                                background_color,
                                with_edge_from: _,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Kind:");
                                egui::ComboBox::from_id_salt("data kind")
                                    .selected_text(kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in BpmnDataKind::VARIANTS {
                                            modified |=
                                                ui.selectable_value(kind, e, e.as_str()).changed();
                                        }
                                    });

                                modified |= columns[1]
                                    .checkbox(is_collection, "is collection")
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
                            BpmnToolStage::LinkStart {
                                link_type: LinkType::Edge { condition },
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Condition", condition)
                                    .changed();
                            }
                            BpmnToolStage::AnnotationStart { text } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Text", text)
                                    .changed();
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

        self.show_reduced(gdc, ui);

        ret
    }
    fn show_reduced(&mut self, _gdc: &GlobalDrawingContext, _ui: &mut egui::Ui) {}
    fn clone_reduced(&self) -> Box<dyn DiagramSettings> {
        Box::new(Self {
            palette: ToolPalette::new(Vec::new()).into(),
            palette_edit_buffer: PaletteEditBuffer::None.into(),
            nonfinal_buttons: Vec::new(),
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
impl DiagramSettings2<BpmnDomain> for BpmnSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                GroupDisplayStyle,
                Vec<(
                    uuid::Uuid,
                    BpmnToolStage,
                    String,
                    BpmnElementView,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        ),
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

type NonFinalElementButtonF =
    dyn Fn(BpmnNonFinalElement) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool);
mod buttons {
    use std::cell::LazyCell;

    use super::*;

    fn nonfinal_edge(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        (
            BpmnToolStage::LinkStart {
                link_type: LinkType::Edge {
                    condition: "".to_owned(),
                },
            },
            BpmnToolStage::LinkEnd,
            PartialBpmnElement::Link {
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    fn nonfinal_association(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        (
            BpmnToolStage::LinkStart {
                link_type: LinkType::Association {
                    name: "".to_owned(),
                },
            },
            BpmnToolStage::LinkEnd,
            PartialBpmnElement::Link {
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    fn nonfinal_intermediateevent(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        let ts = BpmnToolStage::IntermediateEvent {
            kind: BpmnEventKind::Message,
            subkind: BpmnIntermediateEventKind::Throwing,
            name: "".to_owned(),
            with_edge_from: Some(*m.uuid()),
        };
        (ts.clone(), ts, PartialBpmnElement::None, true)
    }
    fn nonfinal_endevent(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        let ts = BpmnToolStage::EndEvent {
            kind: BpmnEventKind::Message,
            name: "".to_owned(),
            with_edge_from: Some(*m.uuid()),
        };
        (ts.clone(), ts, PartialBpmnElement::None, true)
    }

    fn nonfinal_task(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        let ts = BpmnToolStage::Task {
            name: "task".to_owned(),
            kind: BpmnTaskKind::Unspecified,
            is_call: false,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.uuid()),
        };
        (ts.clone(), ts, PartialBpmnElement::None, true)
    }
    fn nonfinal_subprocess(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        let ts = BpmnToolStage::SubprocessStart {
            kind: BpmnSubprocessKind::Subprocess,
            name: "subprocess".to_owned(),
            with_edge_from: Some(*m.uuid()),
        };
        (ts.clone(), ts, PartialBpmnElement::None, true)
    }
    fn nonfinal_gateway(
        m: BpmnNonFinalElement,
    ) -> (BpmnToolStage, BpmnToolStage, PartialBpmnElement, bool) {
        let ts = BpmnToolStage::Gateway {
            kind: BpmnGatewayKind::Exclusive,
            condition: "".to_owned(),
            with_edge_from: Some(*m.uuid()),
        };
        (ts.clone(), ts, PartialBpmnElement::None, true)
    }

    pub const NONFINAL_BUTTONS: LazyCell<
        Vec<(usize, usize, &'static str, &'static NonFinalElementButtonF)>,
    > = LazyCell::new(|| {
        vec![
            (0, 0, "↘", &nonfinal_edge as &NonFinalElementButtonF),
            // (0, 1, "\\", &nonfinal_association as &NonFinalElementButtonF),
            (
                1,
                0,
                "I",
                &nonfinal_intermediateevent as &NonFinalElementButtonF,
            ),
            (1, 1, "E", &nonfinal_endevent as &NonFinalElementButtonF),
            (2, 0, "T", &nonfinal_task as &NonFinalElementButtonF),
            (2, 1, "S", &nonfinal_subprocess as &NonFinalElementButtonF),
            (2, 2, "G", &nonfinal_gateway as &NonFinalElementButtonF),
        ]
    });
}
pub fn default_settings() -> Box<dyn DiagramSettings> {
    let palette_items = vec![
        (
            "Tasks",
            vec![
                (
                    BpmnToolStage::Task {
                        name: "basic action".to_owned(),
                        kind: BpmnTaskKind::Unspecified,
                        is_call: false,
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Basic Action Node",
                    None,
                ),
                (
                    BpmnToolStage::SubprocessStart {
                        name: "Subprocess".to_owned(),
                        kind: BpmnSubprocessKind::Subprocess,
                        with_edge_from: None,
                    },
                    "Subprocess",
                    None,
                ),
                (
                    BpmnToolStage::PoolStart {
                        swimlane_name: "Swimlane".to_owned(),
                    },
                    "Pool",
                    None,
                ),
            ],
        ),
        (
            "Other Nodes",
            vec![
                (
                    BpmnToolStage::StartEvent {
                        kind: BpmnEventKind::Message,
                        subkind: BpmnStartEventKind::Interrupting,
                        name: "".to_owned(),
                    },
                    "Start Event",
                    None,
                ),
                (
                    BpmnToolStage::IntermediateEvent {
                        kind: BpmnEventKind::Message,
                        subkind: BpmnIntermediateEventKind::Catching,
                        name: "".to_owned(),
                        with_edge_from: None,
                    },
                    "Intermediate Event",
                    None,
                ),
                (
                    BpmnToolStage::EndEvent {
                        kind: BpmnEventKind::Message,
                        name: "".to_owned(),
                        with_edge_from: None,
                    },
                    "End Event",
                    None,
                ),
                (
                    BpmnToolStage::BoundaryIntermediateEvent {
                        kind: BpmnEventKind::Compensation,
                        subkind: BpmnBoundaryIntermediateEventKind::Interrupting,
                        name: "".to_owned(),
                    },
                    "Boundary Intermediate Event",
                    None,
                ),
                (
                    BpmnToolStage::Gateway {
                        kind: BpmnGatewayKind::Exclusive,
                        condition: "".to_owned(),
                        with_edge_from: None,
                    },
                    "Gateway",
                    None,
                ),
                (
                    BpmnToolStage::Data {
                        kind: BpmnDataKind::UnspecifiedObject,
                        is_collection: false,
                        name: "data object".to_owned(),
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Data Object",
                    None,
                ),
            ],
        ),
        (
            "Relationships",
            vec![
                (
                    BpmnToolStage::LinkStart {
                        link_type: LinkType::Edge {
                            condition: "".to_owned(),
                        },
                    },
                    "Regular Edge",
                    None,
                ),
                (
                    BpmnToolStage::LinkStart {
                        link_type: LinkType::Edge {
                            condition: "".to_owned(),
                        },
                    },
                    "Interrupting Edge",
                    None,
                ),
            ],
        ),
        (
            "Other",
            vec![(
                BpmnToolStage::AnnotationStart {
                    text: "an annotation".to_owned(),
                },
                "Annotation",
                None,
            )],
        ),
    ]
    .into_iter()
    .map(|e| {
        (
            e.0,
            e.1.into_iter()
                .map(|e| {
                    let v = view_for_stage(&e.0);
                    (e.0, e.1, v, e.2)
                })
                .collect(),
        )
    })
    .map(|e| (e.0, GroupDisplayStyle::List, e.1))
    .collect();

    Box::new(BpmnSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
        nonfinal_buttons: buttons::NONFINAL_BUTTONS.clone(),
    })
}

fn view_for_stage(s: &BpmnToolStage) -> BpmnElementView {
    match s {
        BpmnToolStage::Task {
            name,
            kind,
            is_call,
            background_color,
            with_edge_from: _,
        } => {
            let view = new_bpmn_task(
                name,
                *kind,
                *is_call,
                egui::Pos2::ZERO,
                *background_color,
                false,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::StartEvent {
            kind,
            subkind,
            name,
        } => {
            let view = new_bpmn_startevent(*kind, *subkind, name, egui::Pos2::ZERO, false).1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::IntermediateEvent {
            kind,
            subkind,
            name,
            with_edge_from: _,
        } => {
            let view = new_bpmn_intermediateevent(*kind, *subkind, name, egui::Pos2::ZERO, false).1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::EndEvent {
            kind,
            name,
            with_edge_from: _,
        } => {
            let view = new_bpmn_endevent(*kind, name, egui::Pos2::ZERO, false).1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::BoundaryIntermediateEvent {
            kind,
            subkind,
            name,
        } => {
            let view =
                new_bpmn_boundaryinetermediateevent(*kind, *subkind, name, egui::Pos2::ZERO, false)
                    .1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::Gateway {
            kind,
            condition,
            with_edge_from: _,
        } => {
            let view = new_bpmn_gateway(*kind, condition, egui::Pos2::ZERO, false).1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::Data {
            kind,
            is_collection,
            name,
            background_color,
            with_edge_from: _,
        } => {
            let view = new_bpmn_data(
                *kind,
                *is_collection,
                name,
                egui::Pos2::ZERO,
                *background_color,
                false,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::LinkStart { link_type } => {
            let (d, dv) = new_bpmn_startevent(
                BpmnEventKind::Unspecified,
                BpmnStartEventKind::Interrupting,
                "",
                egui::Pos2::ZERO,
                false,
            );
            let dummy_1_nonfinal = (d.into(), dv.into());
            let (d, dv) = new_bpmn_endevent(
                BpmnEventKind::Unspecified,
                "",
                egui::Pos2::new(200.0, 150.0),
                false,
            );
            let dummy_2_noninitial = (d.clone().into(), dv.clone().into());

            match link_type {
                LinkType::Edge { condition } => {
                    let view = new_bpmn_sequenceedge(
                        BpmnSequenceEdgeKind::Normal,
                        condition,
                        None,
                        dummy_1_nonfinal.clone(),
                        dummy_2_noninitial.clone(),
                    )
                    .1;
                    view.into()
                }
                LinkType::Association { name } => todo!(),
            }
        }
        BpmnToolStage::SubprocessStart {
            kind,
            name,
            with_edge_from: _,
        } => {
            let view = new_bpmn_subprocess(
                *kind,
                name,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 50.0),
                },
                false,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::PoolStart { swimlane_name } => {
            let ps = new_bpmn_poolswimlane(
                swimlane_name,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(175.0, 75.0),
                },
            );
            ps.1.write().refresh_buffers();
            let view = new_bpmn_pool(vec![ps]).1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::AnnotationStart { text } => {
            let dummy = new_bpmn_task(
                "dummy",
                BpmnTaskKind::Manual,
                false,
                egui::Pos2::ZERO,
                MGlobalColor::None,
                false,
            );
            let view = new_bpmn_annotation(
                (dummy.0.into(), dummy.1.into()),
                text,
                egui::Pos2::new(50.0, -25.0),
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        BpmnToolStage::LinkEnd
        | BpmnToolStage::SubprocessEnd
        | BpmnToolStage::PoolEnd
        | BpmnToolStage::AnnotationEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(BpmnSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
        nonfinal_buttons: buttons::NONFINAL_BUTTONS.clone(),
    }))
}

inventory::submit! {DiagramInfo {
    type_indentifier: "bpmn",
    pretty_name: "Business Process Model and Notation",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "",
        description: "BPMN diagram (tasks, gateways, events, pools, etc.)",
        constructors: &[
            ("empty", &((|no| format!("New BPMN diagram {}", no)) as DefaultNameF), &(new as DiagramConstructorF)),
            ("demo", &((|no| format!("Demo BPMN diagram {}", no)) as DefaultNameF), &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkType {
    Edge { condition: String },
    Association { name: String },
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum BpmnToolStage {
    Task {
        name: String,
        kind: BpmnTaskKind,
        is_call: bool,
        background_color: MGlobalColor,
        with_edge_from: Option<ModelUuid>,
    },
    StartEvent {
        kind: BpmnEventKind,
        subkind: BpmnStartEventKind,
        name: String,
    },
    IntermediateEvent {
        kind: BpmnEventKind,
        subkind: BpmnIntermediateEventKind,
        name: String,
        with_edge_from: Option<ModelUuid>,
    },
    EndEvent {
        kind: BpmnEventKind,
        name: String,
        with_edge_from: Option<ModelUuid>,
    },
    BoundaryIntermediateEvent {
        kind: BpmnEventKind,
        subkind: BpmnBoundaryIntermediateEventKind,
        name: String,
    },
    Gateway {
        kind: BpmnGatewayKind,
        condition: String,
        with_edge_from: Option<ModelUuid>,
    },
    Data {
        kind: BpmnDataKind,
        is_collection: bool,
        name: String,
        background_color: MGlobalColor,
        with_edge_from: Option<ModelUuid>,
    },
    LinkStart {
        link_type: LinkType,
    },
    LinkEnd,
    SubprocessStart {
        kind: BpmnSubprocessKind,
        name: String,
        with_edge_from: Option<ModelUuid>,
    },
    SubprocessEnd,
    PoolStart {
        swimlane_name: String,
    },
    PoolEnd,
    AnnotationStart {
        text: String,
    },
    AnnotationEnd,
}

pub enum PartialBpmnElement {
    None,
    Some(BpmnElementView),
    Link {
        source: BpmnNonFinalElement,
        dest: Option<BpmnNonInitialElement>,
    },
    Subprocess {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    Pool {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    Annotation {
        target: BpmnElement,
        dest: Option<egui::Pos2>,
    },
}

pub struct NaiveBpmnTool {
    uuid: uuid::Uuid,
    initial_stage: BpmnToolStage,
    current_stage: BpmnToolStage,
    result: PartialBpmnElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveBpmnTool {
    fn try_spend(&mut self) {
        self.result = PartialBpmnElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<BpmnDomain> for NaiveBpmnTool {
    type Stage = BpmnToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: BpmnToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialBpmnElement::None,
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
        element: Result<BpmnElement, ERef<BpmnDiagram>>,
    ) -> egui::Color32 {
        match element {
            Err(_) | Ok(BpmnElement::Pool(_) | BpmnElement::PoolSwimlane(_)) => {
                match self.current_stage {
                    BpmnToolStage::BoundaryIntermediateEvent { .. }
                    | BpmnToolStage::LinkStart { .. }
                    | BpmnToolStage::LinkEnd
                    | BpmnToolStage::AnnotationStart { .. } => NON_TARGETTABLE_COLOR,
                    _ => TARGETTABLE_COLOR,
                }
            }
            Ok(BpmnElement::Task(_)) => match self.current_stage {
                BpmnToolStage::BoundaryIntermediateEvent { .. }
                | BpmnToolStage::LinkStart { .. }
                | BpmnToolStage::LinkEnd
                | BpmnToolStage::AnnotationStart { .. } => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(BpmnElement::Subprocess(_)) => TARGETTABLE_COLOR,
            Ok(BpmnElement::StartEvent(_) | BpmnElement::BoundaryIntermediateEvent(_)) => {
                match self.current_stage {
                    BpmnToolStage::LinkStart { .. } | BpmnToolStage::AnnotationStart { .. } => {
                        TARGETTABLE_COLOR
                    }
                    _ => NON_TARGETTABLE_COLOR,
                }
            }
            Ok(BpmnElement::EndEvent(_)) => match self.current_stage {
                BpmnToolStage::LinkEnd | BpmnToolStage::AnnotationStart { .. } => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(
                BpmnElement::IntermediateEvent(_) | BpmnElement::Gateway(_) | BpmnElement::Data(_),
            ) => match self.current_stage {
                BpmnToolStage::LinkStart { .. }
                | BpmnToolStage::LinkEnd
                | BpmnToolStage::AnnotationStart { .. } => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(BpmnElement::Annotation(_)) => NON_TARGETTABLE_COLOR,
            Ok(
                BpmnElement::SequenceEdge(_)
                | BpmnElement::MessageEdge(_)
                | BpmnElement::Association(_),
            ) => unreachable!(),
        }
    }
    fn draw_status_hint(
        &self,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match (&self.current_stage, &self.result) {
            (_, PartialBpmnElement::Link { source, .. }) => {
                if let Some(source_view) = q.get_view_for(&source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                BpmnToolStage::Task {
                    with_edge_from: Some(source),
                    ..
                }
                | BpmnToolStage::SubprocessStart {
                    with_edge_from: Some(source),
                    ..
                }
                | BpmnToolStage::IntermediateEvent {
                    with_edge_from: Some(source),
                    ..
                }
                | BpmnToolStage::EndEvent {
                    with_edge_from: Some(source),
                    ..
                }
                | BpmnToolStage::Gateway {
                    with_edge_from: Some(source),
                    ..
                }
                | BpmnToolStage::Data {
                    with_edge_from: Some(source),
                    ..
                },
                _,
            ) => {
                if let Some(source_view) = q.get_view_for(source) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (_, PartialBpmnElement::Subprocess { a, .. } | PartialBpmnElement::Pool { a, .. }) => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a, pos),
                    egui::CornerRadius::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            (_, PartialBpmnElement::Annotation { target, .. }) => {
                if let Some(source_view) = q.get_view_for(&target.uuid()) {
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
                BpmnToolStage::Task {
                    name,
                    kind,
                    is_call,
                    background_color,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) =
                    new_bpmn_task(name, *kind, *is_call, pos, *background_color, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (
                BpmnToolStage::StartEvent {
                    kind,
                    subkind,
                    name,
                },
                _,
            ) => {
                let (_model, view) = new_bpmn_startevent(*kind, *subkind, name, pos, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (
                BpmnToolStage::IntermediateEvent {
                    kind,
                    subkind,
                    name,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) = new_bpmn_intermediateevent(*kind, *subkind, name, pos, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (
                BpmnToolStage::EndEvent {
                    kind,
                    name,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) = new_bpmn_endevent(*kind, name, pos, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (
                BpmnToolStage::BoundaryIntermediateEvent {
                    kind,
                    subkind,
                    name,
                },
                _,
            ) => {
                let (_model, view) =
                    new_bpmn_boundaryinetermediateevent(*kind, *subkind, name, pos, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (
                BpmnToolStage::Gateway {
                    kind,
                    condition,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) = new_bpmn_gateway(*kind, condition, pos, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (
                BpmnToolStage::Data {
                    kind,
                    is_collection,
                    name,
                    background_color,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) =
                    new_bpmn_data(*kind, *is_collection, name, pos, *background_color, true);
                self.result = PartialBpmnElement::Some(view.into());
                self.event_lock = true;
            }
            (BpmnToolStage::AnnotationEnd { .. }, PartialBpmnElement::Annotation { dest, .. }) => {
                *dest = Some(pos);
                self.event_lock = true;
            }
            (BpmnToolStage::SubprocessStart { .. }, _) => {
                self.result = PartialBpmnElement::Subprocess { a: pos, b: None };
                self.current_stage = BpmnToolStage::SubprocessEnd;
                self.event_lock = true;
            }
            (BpmnToolStage::SubprocessEnd, PartialBpmnElement::Subprocess { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (BpmnToolStage::PoolStart { .. }, _) => {
                self.result = PartialBpmnElement::Pool { a: pos, b: None };
                self.current_stage = BpmnToolStage::PoolEnd;
                self.event_lock = true;
            }
            (BpmnToolStage::PoolEnd, PartialBpmnElement::Pool { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, element: BpmnElement) {
        if self.event_lock {
            return;
        }

        match element {
            e @ (BpmnElement::Task(..)
            | BpmnElement::Subprocess(..)
            | BpmnElement::StartEvent(..)
            | BpmnElement::IntermediateEvent(..)
            | BpmnElement::EndEvent(..)
            | BpmnElement::BoundaryIntermediateEvent(..)
            | BpmnElement::Gateway(..)
            | BpmnElement::Data(..)) => match (&self.current_stage, &mut self.result) {
                (BpmnToolStage::LinkStart { link_type }, PartialBpmnElement::None)
                    if let Some(e) = e.as_nonfinal() =>
                {
                    self.result = PartialBpmnElement::Link {
                        source: e,
                        dest: None,
                    };
                    self.current_stage = BpmnToolStage::LinkEnd;
                    self.event_lock = true;
                }
                (BpmnToolStage::LinkEnd, PartialBpmnElement::Link { dest, .. })
                    if let Some(e) = e.as_noninitial() =>
                {
                    *dest = Some(e);
                    self.event_lock = true;
                }
                (BpmnToolStage::AnnotationStart { .. }, _) => {
                    self.result = PartialBpmnElement::Annotation {
                        target: e,
                        dest: None,
                    };
                    self.current_stage = BpmnToolStage::AnnotationEnd;
                    self.event_lock = true;
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<(), ()> {
        match &self.result {
            PartialBpmnElement::Some(element) => {
                let element = element.clone();
                let additional_edge = match &self.initial_stage {
                    BpmnToolStage::Task {
                        with_edge_from: Some(source),
                        ..
                    }
                    | BpmnToolStage::IntermediateEvent {
                        with_edge_from: Some(source),
                        ..
                    }
                    | BpmnToolStage::EndEvent {
                        with_edge_from: Some(source),
                        ..
                    }
                    | BpmnToolStage::Gateway {
                        with_edge_from: Some(source),
                        ..
                    }
                    | BpmnToolStage::Data {
                        with_edge_from: Some(source),
                        ..
                    } if let Some(source) = q.get_view_for(source)
                        && let nearest_common_container = q
                            .find_container(&source.uuid(), |uuid, e| {
                                (uuid == preferred_container
                                    || q.is_contained(preferred_container, uuid))
                                    && !matches!(e, BpmnElementView::Pool(_))
                            })
                            .map(|e| e.0)
                            .unwrap_or_else(|| q.get_root()) =>
                    {
                        let edge_view = new_bpmn_sequenceedge(
                            BpmnSequenceEdgeKind::Normal,
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

                commands.push(InsensitiveCommand::HighlightAll(
                    false,
                    canvas::Highlight::SELECTED,
                ));
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: BpmnElementView::from(element).into(),
                    into_model: true,
                });
                if let Some((parent, e)) = additional_edge {
                    commands.push(InsensitiveCommand::AddDependency {
                        target: parent,
                        bucket: 0,
                        position: None,
                        element: BpmnElementView::from(e).into(),
                        into_model: true,
                    });
                }
                Ok(())
            }
            PartialBpmnElement::Link {
                source,
                dest: Some(dest),
                ..
            } if let BpmnToolStage::LinkStart { link_type } = &self.initial_stage => {
                let (source_uuid, target_uuid) = (*source.uuid(), *dest.uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_view.uuid(), preferred_container)
                    && q.is_contained(&target_view.uuid(), preferred_container)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view: BpmnElementView = match link_type {
                        LinkType::Edge { condition } => new_bpmn_sequenceedge(
                            BpmnSequenceEdgeKind::Normal,
                            condition,
                            None,
                            (source.clone(), source_view),
                            (dest.clone(), target_view),
                        )
                        .1
                        .into(),
                        LinkType::Association { name } => {
                            todo!()
                        }
                    };

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: link_view.into(),
                        into_model: true,
                    });
                    Ok(())
                } else {
                    Err(())
                }
            }
            PartialBpmnElement::Subprocess { a, b: Some(b) }
                if let BpmnToolStage::SubprocessStart {
                    kind,
                    name,
                    with_edge_from: _,
                } = &self.initial_stage =>
            {
                self.current_stage = self.initial_stage.clone();

                let activity_view =
                    new_bpmn_subprocess(*kind, name, egui::Rect::from_two_pos(*a, *b), true).1;

                let additional_edge = match &self.initial_stage {
                    BpmnToolStage::SubprocessStart {
                        with_edge_from: Some(source),
                        ..
                    } if let Some(source) = q.get_view_for(source)
                        && let nearest_common_container = q
                            .find_container(&source.uuid(), |uuid, e| {
                                (uuid == preferred_container
                                    || q.is_contained(preferred_container, uuid))
                                    && !matches!(e, BpmnElementView::Pool(_))
                            })
                            .map(|e| e.0)
                            .unwrap_or_else(|| q.get_root()) =>
                    {
                        let edge_view = new_bpmn_sequenceedge(
                            BpmnSequenceEdgeKind::Normal,
                            "",
                            None,
                            (source.model().as_nonfinal().unwrap(), source),
                            (
                                activity_view.read().model().as_noninitial().unwrap(),
                                activity_view.clone().into(),
                            ),
                        )
                        .1;
                        Some((nearest_common_container, edge_view))
                    }
                    _ => None,
                };

                self.try_spend();

                commands.push(InsensitiveCommand::HighlightAll(
                    false,
                    canvas::Highlight::SELECTED,
                ));
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: BpmnElementView::from(activity_view).into(),
                    into_model: true,
                });
                if let Some((parent, e)) = additional_edge {
                    commands.push(InsensitiveCommand::AddDependency {
                        target: parent,
                        bucket: 0,
                        position: None,
                        element: BpmnElementView::from(e).into(),
                        into_model: true,
                    });
                }
                Ok(())
            }
            PartialBpmnElement::Pool { a, b: Some(b) }
                if let BpmnToolStage::PoolStart { swimlane_name } = &self.initial_stage =>
            {
                self.current_stage = self.initial_stage.clone();

                let r = egui::Rect::from_two_pos(*a, *b);
                let s = new_bpmn_poolswimlane(swimlane_name, r);
                let partition_view = new_bpmn_pool(vec![s]).1;

                self.try_spend();

                commands.push(InsensitiveCommand::HighlightAll(
                    false,
                    canvas::Highlight::SELECTED,
                ));
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: BpmnElementView::from(partition_view).into(),
                    into_model: true,
                });
                Ok(())
            }
            PartialBpmnElement::Annotation {
                target,
                dest: Some(dest),
            } if let BpmnToolStage::AnnotationStart { text } = &self.initial_stage => {
                let target_uuid = *target.uuid();
                if let Some(target_view) = q.get_view_for(&target_uuid)
                    && q.is_contained(&target_view.uuid(), preferred_container)
                {
                    self.current_stage = self.initial_stage.clone();

                    let view = new_bpmn_annotation((target.clone(), target_view), text, *dest).1;

                    self.try_spend();

                    commands.push(InsensitiveCommand::HighlightAll(
                        false,
                        canvas::Highlight::SELECTED,
                    ));
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: BpmnElementView::from(view).into(),
                        into_model: true,
                    });
                    Ok(())
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

pub fn new_bpmn_subprocess(
    kind: BpmnSubprocessKind,
    name: &str,
    bounds_rect: egui::Rect,
    request_focus: bool,
) -> (ERef<BpmnSubprocess>, ERef<BpmnSubprocessView>) {
    let model = ERef::new(BpmnSubprocess::new(
        ModelUuid::now_v7(),
        kind,
        name.to_owned(),
        Vec::new(),
    ));
    let package_view = new_bpmn_subprocess_view(model.clone(), bounds_rect, request_focus);

    (model, package_view)
}
pub fn new_bpmn_subprocess_view(
    model: ERef<BpmnSubprocess>,
    bounds_rect: egui::Rect,
    request_focus: bool,
) -> ERef<BpmnSubprocessView> {
    let m = model.read();
    ERef::new(BpmnSubprocessView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        contained_elements: OrderedViews::new(Vec::new()),
        attached_events: Vec::new(),

        temporaries: BpmnSubprocessViewTemporaries {
            all_children_status: HashMap::new(),
            request_focus,
            kind_buffer: m.kind,
            name_buffer: (*m.name).clone(),
            dragged_type_and_shape: None,
            highlight: canvas::Highlight::from_selected(request_focus),
        },
        bounds_rect,
        background_color: MGlobalColor::None,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnSubprocessView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<BpmnSubprocess>,
    #[nh_context_serde(entity)]
    contained_elements: OrderedViews<BpmnElementView>,
    #[nh_context_serde(entity)]
    attached_events: Vec<ERef<BpmnEventView>>,

    #[nh_context_serde(skip_and_default)]
    temporaries: BpmnSubprocessViewTemporaries,
    bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

#[derive(Clone, Default)]
struct BpmnSubprocessViewTemporaries {
    all_children_status: HashMap<ViewUuid, SelectionStatus>,

    request_focus: bool,
    kind_buffer: BpmnSubprocessKind,
    name_buffer: String,

    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
}

impl BpmnSubprocessView {
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

    fn refresh_all_children_status(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        self.temporaries.all_children_status.clear();
        self.contained_elements.event_order_foreach_mut(|v| {
            v.head_count(
                flattened_views,
                &mut self.temporaries.all_children_status,
                flattened_represented_models,
            )
        });
        self.attached_events.iter_mut().for_each(|v| {
            v.write().head_count(
                flattened_views,
                &mut self.temporaries.all_children_status,
                flattened_represented_models,
            )
        });
        for e in &self.temporaries.all_children_status {
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
    }
}

impl Entity for BpmnSubprocessView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnSubprocessView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnSubprocessView {
    fn model(&self) -> BpmnElement {
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

impl ElementControllerGen2<BpmnDomain> for BpmnSubprocessView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        if let Some(child) = self
            .contained_elements
            .event_order_find_mut(|e| e.show_properties(gdc, q, ui, commands).non_default())
        {
            return child;
        }
        if let Some(child) = self
            .attached_events
            .iter_mut()
            .flat_map(|e| {
                e.write()
                    .show_properties(gdc, q, ui, commands)
                    .non_default()
            })
            .next()
        {
            return child;
        }

        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Transaction Kind:");
        egui::ComboBox::from_id_salt("transaction kind")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for value in BpmnSubprocessKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, value, value.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            BpmnPropChange::SubprocessKindChange(self.temporaries.kind_buffer),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline2(
                "Name:",
                &mut self.temporaries.name_buffer,
                self.temporaries.request_focus,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }
        self.temporaries.request_focus = false;

        ui.label("View properties");

        egui::Grid::new("size_grid").show(ui, |ui| {
            {
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
                ui.end_row();
            }

            {
                let egui::Vec2 { mut x, mut y } = self.bounds_rect.size();

                ui.label("width");
                if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                    commands.push(InsensitiveCommand::ResizeElementsBy(
                        q.selected_views(),
                        egui::Align2::LEFT_CENTER,
                        egui::Vec2::new(x - self.bounds_rect.width(), 0.0),
                    ));
                }
                ui.label("height");
                if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                    commands.push(InsensitiveCommand::ResizeElementsBy(
                        q.selected_views(),
                        egui::Align2::CENTER_TOP,
                        egui::Vec2::new(0.0, y - self.bounds_rect.height()),
                    ));
                }
                ui.end_row();
            }
        });

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<BpmnDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::same(10),
            egui::Color32::WHITE,
            canvas::Stroke {
                width: 1.0,
                color: egui::Color32::BLACK,
                line_type: match self.temporaries.kind_buffer {
                    BpmnSubprocessKind::Subprocess | BpmnSubprocessKind::EventSubprocess => {
                        canvas::LineType::Solid
                    }
                    BpmnSubprocessKind::Transaction => canvas::LineType::Dashed,
                },
            },
            self.temporaries.highlight,
        );
        const EVENT_BORDER_WIDTH: f32 = 5.0;
        if self.temporaries.kind_buffer == BpmnSubprocessKind::EventSubprocess {
            canvas.draw_rectangle(
                self.bounds_rect.shrink(EVENT_BORDER_WIDTH),
                egui::CornerRadius::same(10),
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.temporaries.highlight,
            );
        }

        canvas.draw_text(
            self.bounds_rect.center_top() + (0.0, EVENT_BORDER_WIDTH).into(),
            egui::Align2::CENTER_TOP,
            &self.temporaries.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw resize/drag handles
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            let handle_size = self.handle_size(ui_scale);

            for (h, c) in [
                (self.bounds_rect.left_top(), canvas::NHIcon::ArrowUpLeft),
                (self.bounds_rect.center_top(), canvas::NHIcon::ArrowUp),
                (self.bounds_rect.right_top(), canvas::NHIcon::ArrowUpRight),
                (self.bounds_rect.left_center(), canvas::NHIcon::ArrowLeft),
                (self.bounds_rect.right_center(), canvas::NHIcon::ArrowRight),
                (
                    self.bounds_rect.left_bottom(),
                    canvas::NHIcon::ArrowDownLeft,
                ),
                (self.bounds_rect.center_bottom(), canvas::NHIcon::ArrowDown),
                (
                    self.bounds_rect.right_bottom(),
                    canvas::NHIcon::ArrowDownRight,
                ),
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
            canvas::NHIcon::Move.draw(canvas, dc, 8.0 / ui_scale, egui::Color32::BLACK);
        }
        // Draw buttons
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            draw_nonfinal_node_button_rects(
                settings,
                canvas,
                self.bounds_rect.right_top(),
                ui_scale,
            );
        }

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;
        self.contained_elements.event_order_foreach_mut(|e| {
            drawn_child_targetting |= e.draw_in(q, context, settings, canvas, tool)
        });
        for e in self.attached_events.iter_mut() {
            drawn_child_targetting |= e.write().draw_in(q, context, settings, canvas, tool);
        }

        if canvas.ui_scale().is_some() {
            if self.temporaries.dragged_type_and_shape.is_some() {
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

            match (drawn_child_targetting, tool) {
                (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                    canvas.draw_rectangle(
                        self.bounds_rect,
                        egui::CornerRadius::ZERO,
                        t.targetting_for_section(Ok(self.model())),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );

                    self.contained_elements.draw_order_foreach_mut(|v| {
                        v.draw_in(q, context, settings, canvas, tool);
                    });

                    TargettingStatus::Drawn
                }
                _ => drawn_child_targetting,
            }
        } else {
            TargettingStatus::NotDrawn
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());

        self.contained_elements
            .event_order_foreach_mut(|e| e.collect_allignment(am));
        for e in self.attached_events.iter_mut() {
            e.write().collect_allignment(am);
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<BpmnDomain as Domain>::ToolT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        let k_status = self
            .contained_elements
            .event_order_find_mut(|v| {
                let s = v.handle_event(event, ehc, settings, q, tool, commands);
                if s != EventHandlingStatus::NotHandled {
                    Some((*v.uuid(), s))
                } else {
                    None
                }
            })
            .or_else(|| {
                self.attached_events
                    .iter_mut()
                    .filter_map(|e| {
                        let s = e
                            .write()
                            .handle_event(event, ehc, settings, q, tool, commands);
                        if s != EventHandlingStatus::NotHandled {
                            Some((*e.read().uuid, s))
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
                    for (a, h) in [
                        (egui::Align2::RIGHT_BOTTOM, self.bounds_rect.left_top()),
                        (egui::Align2::CENTER_BOTTOM, self.bounds_rect.center_top()),
                        (egui::Align2::LEFT_BOTTOM, self.bounds_rect.right_top()),
                        (egui::Align2::RIGHT_CENTER, self.bounds_rect.left_center()),
                        (egui::Align2::LEFT_CENTER, self.bounds_rect.right_center()),
                        (egui::Align2::RIGHT_TOP, self.bounds_rect.left_bottom()),
                        (egui::Align2::CENTER_TOP, self.bounds_rect.center_bottom()),
                        (egui::Align2::LEFT_TOP, self.bounds_rect.right_bottom()),
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
            InputEvent::MouseUp(_pos) => {
                if self.temporaries.dragged_type_and_shape.is_some() {
                    self.temporaries.dragged_type_and_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos)
                if k_status.is_none()
                    && self.temporaries.highlight.selected
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveBpmnTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage,
                    current_stage,
                    result,
                    event_lock,
                    is_spent: Some(false),
                });
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::Click(pos) => {
                if !self.bounds_rect.contains(pos) {
                    return k_status
                        .map(|e| e.1)
                        .unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());

                    if !tool.result_references(&self.model.read().uuid) {
                        let _ = tool.try_flush(q, &self.uuid, 0, None, commands);
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
                                !self
                                    .temporaries
                                    .all_children_status
                                    .get(&k)
                                    .is_some_and(|e| e.selected()),
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
                        !self.temporaries.all_children_status.get(e).is_some()
                            && !if self.temporaries.highlight.selected {
                                ehc.all_elements
                                    .get(e)
                                    .is_some_and(|e| *e != SelectionStatus::NotSelected)
                            } else {
                                *e == *self.uuid
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
                            !self.temporaries.all_children_status.get(e).is_some()
                                && !ehc
                                    .all_elements
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
        diagram_model: &ERef<<BpmnDomain as Domain>::DiagramModelT>,
        command: &InsensitiveCommand<
            <BpmnDomain as Domain>::OrdinalMovementT,
            <BpmnDomain as Domain>::AddCommandElementT,
            <BpmnDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
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
        macro_rules! resize_to {
            ($rect:expr) => {
                undo_accumulator.push(InsensitiveCommand::ResizeElementTo(
                    *self.uuid,
                    self.bounds_rect,
                ));
                self.bounds_rect = $rect;
            };
        }

        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                if h.selected {
                    let new_state = match set {
                        true => SelectionStatus::Selected,
                        false => SelectionStatus::NotSelected,
                    };
                    self.temporaries
                        .all_children_status
                        .iter_mut()
                        .for_each(|e| *e.1 = new_state);
                }
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                }

                if h.selected {
                    let new_state = match set {
                        true => SelectionStatus::Selected,
                        false => SelectionStatus::NotSelected,
                    };
                    for k in self
                        .contained_elements
                        .iter_event_order_keys()
                        .filter(|k| uuids.contains(k))
                    {
                        if let Some(e) = self.temporaries.all_children_status.get_mut(&k) {
                            *e = new_state;
                        }
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected = (self.temporaries.highlight.selected
                    && *retain)
                    || rect.contains_rect(self.bounds_rect);

                recurse!();

                self.refresh_all_children_status(
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                );
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
            InsensitiveCommand::MoveOrdinal(..) => {
                recurse!();
            }
            InsensitiveCommand::ResizeElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    let min_delta_x = 40.0 - self.bounds_rect.width();
                    let (left, right) = match align.x() {
                        egui::Align::Min => (0.0, delta.x.max(min_delta_x)),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => ((-delta.x).max(min_delta_x), 0.0),
                    };
                    let min_delta_y = 20.0 - self.bounds_rect.height();
                    let (top, bottom) = match align.y() {
                        egui::Align::Min => (0.0, delta.y.max(min_delta_y)),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => ((-delta.y).max(min_delta_y), 0.0),
                    };

                    let r = self.bounds_rect
                        + epaint::MarginF32 {
                            left,
                            right,
                            top,
                            bottom,
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
                let model_uuid = *self.model_uuid();
                if *target == *self.uuid
                    && *bucket == 0
                    && let BpmnElementOrVertex::Element(mut view) = element.clone()
                    && (!*into_model
                        || diagram_model
                            .write()
                            .insert_element_into(model_uuid, 0, *position, view.model())
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

                recurse!();
            }
            InsensitiveCommand::RemoveDependency {
                target,
                bucket,
                element,
                including_model,
            } => {
                let model_uuid = *self.model_uuid();
                if *target == *self.uuid
                    && *bucket == 0
                    && let Some(view) = self.contained_elements.get(element)
                    && let Some((_, pos)) = diagram_model
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
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(uuids, arr) => {
                self.contained_elements.apply_arrangement(uuids, *arr);
            }
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    let mut w = self.model.write();
                    affected_models.insert(*w.uuid);
                    match property {
                        BpmnPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::NameChange(w.name.clone()),
                            ));
                            w.name = name.clone();
                        }
                        BpmnPropChange::SubprocessKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::SubprocessKindChange(w.kind),
                            ));
                            w.kind = *kind;
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
        self.temporaries.kind_buffer = r.kind;
        self.temporaries.name_buffer = (*r.name).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<
            ViewUuid,
            (<BpmnDomain as Domain>::CommonElementViewT, ViewUuid),
        >,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model.read().uuid, *self.uuid);

        self.refresh_all_children_status(
            flattened_views,
            flattened_views_status,
            flattened_represented_models,
        );

        self.contained_elements.event_order_foreach_mut(|v| {
            flattened_views.insert(*v.uuid(), (v.clone(), *self.uuid));
        });
        self.attached_events.iter_mut().for_each(|v| {
            flattened_views.insert(*v.read().uuid(), (v.clone().into(), *self.uuid));
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.contained_elements
                .event_order_foreach(|e| e.deep_copy_walk(requested, uuid_present, tlc, c, m));
            for e in &self.attached_events {
                e.read().deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(BpmnElement::Subprocess(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let mut inner = HashMap::new();
        self.contained_elements
            .event_order_foreach(|v| v.deep_copy_clone(uuid_present, &mut inner, c, m));
        self.attached_events
            .iter()
            .for_each(|v| v.read().deep_copy_clone(uuid_present, &mut inner, c, m));
        let attached_events = self
            .attached_events
            .iter()
            .flat_map(|e| {
                if let Some(BpmnElementView::Event(e)) = inner.get(&e.read().uuid) {
                    Some(e.clone())
                } else {
                    None
                }
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            contained_elements: OrderedViews::new(inner.into_values().collect()),
            attached_events,
            temporaries: self.temporaries.clone(),
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        self.contained_elements
            .event_order_foreach_mut(|v| v.deep_copy_relink(c, m));
    }
}

pub fn new_bpmn_pool(
    sections: Vec<(ERef<BpmnPoolSwimlane>, ERef<BpmnPoolSwimlaneView>)>,
) -> (ERef<BpmnPool>, ERef<BpmnPoolView>) {
    let (section_models, section_views) = sections.into_iter().collect();
    let model = ERef::new(BpmnPool::new(ModelUuid::now_v7(), section_models));
    let view = new_bpmn_pool_view(model.clone(), section_views);

    (model, view)
}
pub fn new_bpmn_pool_view(
    model: ERef<BpmnPool>,
    section_views: Vec<ERef<BpmnPoolSwimlaneView>>,
) -> ERef<BpmnPoolView> {
    ERef::new(BpmnPoolView {
        uuid: ViewUuid::now_v7().into(),
        model,
        swimlane_views: section_views,
        bounds_rect: egui::Rect::ZERO,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnPoolView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<BpmnPool>,
    #[nh_context_serde(entity)]
    swimlane_views: Vec<ERef<BpmnPoolSwimlaneView>>,

    bounds_rect: egui::Rect,
    #[nh_context_serde(skip_and_default)]
    temporaries: BpmnPoolViewTemporaries,
}

#[derive(Clone, Default)]
struct BpmnPoolViewTemporaries {
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
}

impl Entity for BpmnPoolView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnPoolView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnPoolView {
    fn model(&self) -> BpmnElement {
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

impl ElementControllerGen2<BpmnDomain> for BpmnPoolView {
    fn draw_in(
        &mut self,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<BpmnDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        let mut child_targetting_drawn = false;
        let mut r = egui::Rect::ZERO;
        for e in self.swimlane_views.iter() {
            let mut w = e.write();
            child_targetting_drawn |=
                w.draw_in(q, context, settings, canvas, tool) != TargettingStatus::NotDrawn;
            r = r.union(w.bounds_rect);
        }
        self.bounds_rect = r;

        match child_targetting_drawn {
            false => TargettingStatus::NotDrawn,
            true => TargettingStatus::Drawn,
        }
    }

    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        for (idx, e) in self.swimlane_views.iter().enumerate() {
            let mut w = e.write();
            let child = w.show_properties_inner(gdc, q, ui, commands);

            if let Some(add_sibling) = child.1 {
                let idx = match add_sibling {
                    BpmnOrdinalMovement::Up => idx,
                    BpmnOrdinalMovement::Down => idx + 1,
                };
                let y_range = match add_sibling {
                    BpmnOrdinalMovement::Up => (w.bounds_rect.top() - 200.0)..=w.bounds_rect.top(),
                    BpmnOrdinalMovement::Down => {
                        w.bounds_rect.bottom()..=(w.bounds_rect.bottom() + 200.0)
                    }
                };
                let sibling = new_bpmn_poolswimlane(
                    "New Swimlane",
                    egui::Rect::from_x_y_ranges(w.bounds_rect.x_range(), y_range),
                );
                commands.push(InsensitiveCommand::AddDependency {
                    target: *self.uuid,
                    bucket: 0,
                    position: Some(idx.try_into().unwrap()),
                    element: BpmnElementOrVertex::Element(sibling.1.into()),
                    into_model: true,
                });
            }

            if let Some(child) = child.0.non_default() {
                return child;
            }
        }

        return PropertiesStatus::NotShown;
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        self.swimlane_views
            .iter()
            .for_each(|v| v.write().collect_allignment(am));
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<BpmnDomain as Domain>::ToolT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(_) => {
                let k_status = self
                    .swimlane_views
                    .iter()
                    .map(|e| {
                        let mut w = e.write();
                        (
                            *w.uuid,
                            w.handle_event(event, ehc, settings, q, tool, commands),
                        )
                    })
                    .find(|e| e.1 != EventHandlingStatus::NotHandled);

                if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc
                            .modifier_settings
                            .hold_selection
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            commands.push(
                                InsensitiveCommand::HighlightAll(
                                    false,
                                    canvas::Highlight::SELECTED,
                                )
                                .into(),
                            );
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(k).collect(),
                                    true,
                                    canvas::Highlight::SELECTED,
                                )
                                .into(),
                            );
                        } else {
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(k).collect(),
                                    !self.temporaries.selected_direct_elements.contains(&k),
                                    canvas::Highlight::SELECTED,
                                )
                                .into(),
                            );
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            _ => {
                let k_status = self
                    .swimlane_views
                    .iter()
                    .map(|e| {
                        let mut w = e.write();
                        (
                            *w.uuid,
                            w.handle_event(event, ehc, settings, q, tool, commands),
                        )
                    })
                    .find(|e| e.1 != EventHandlingStatus::NotHandled);
                k_status
                    .map(|e| e.1)
                    .unwrap_or(EventHandlingStatus::NotHandled)
            }
        }
    }

    fn apply_command(
        &mut self,
        diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<
            <BpmnDomain as Domain>::OrdinalMovementT,
            <BpmnDomain as Domain>::AddCommandElementT,
            <BpmnDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.swimlane_views.iter().for_each(|s| {
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
                            self.temporaries.selected_direct_elements =
                                self.swimlane_views.iter().map(|v| *v.read().uuid).collect();
                        }
                        false => self.temporaries.selected_direct_elements.clear(),
                    }
                }
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if h.selected {
                    for k in self
                        .swimlane_views
                        .iter()
                        .map(|v| *v.read().uuid)
                        .filter(|k| uuids.contains(k))
                    {
                        match set {
                            true => self.temporaries.selected_direct_elements.insert(k),
                            false => self.temporaries.selected_direct_elements.remove(&k),
                        };
                    }
                }

                recurse!();

                if uuids.contains(&*self.uuid) {
                    self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                } else {
                    self.temporaries.highlight.selected = self
                        .swimlane_views
                        .iter()
                        .all(|e| e.read().temporaries.highlight.selected);
                }
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected = (self.temporaries.highlight.selected
                    && *retain)
                    || self.min_shape().contained_within(*rect);

                recurse!();

                self.temporaries.highlight.selected = self
                    .swimlane_views
                    .iter()
                    .all(|e| e.read().temporaries.highlight.selected);
            }
            InsensitiveCommand::MovePositional(uuids, _)
                if !uuids.contains(&self.uuid)
                    && !self
                        .swimlane_views
                        .iter()
                        .any(|e| uuids.contains(&e.read().uuid)) =>
            {
                recurse!();
            }
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.swimlane_views.iter_mut().for_each(|v| {
                    v.write().apply_command(
                        diagram_model,
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(uuids, align, delta) => {
                if self
                    .swimlane_views
                    .iter()
                    .any(|e| uuids.contains(&e.read().uuid))
                {
                    let mut delta_y = egui::Vec2::ZERO;
                    let (mut u, mut v) = Default::default();

                    let sections_iter: Box<dyn Iterator<Item = &ERef<BpmnPoolSwimlaneView>>> =
                        match align.y() {
                            egui::Align::Min | egui::Align::Center => {
                                Box::new(self.swimlane_views.iter())
                            }
                            egui::Align::Max => Box::new(self.swimlane_views.iter().rev()),
                        };

                    for e in sections_iter {
                        let mut w = e.write();
                        w.apply_command(
                            diagram_model,
                            &InsensitiveCommand::MovePositionalAll(delta_y),
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
                                delta_y.y += delta.y;
                            }
                        }
                        if new_rect.height() >= 40.0 {
                            w.bounds_rect.min.x = new_rect.min.x;
                            w.bounds_rect.max.x = new_rect.max.x;
                        }
                    }
                }

                recurse!();
            }
            InsensitiveCommand::ResizeElementTo(uuid, rect) => {
                if let Some((idx, br)) = self
                    .swimlane_views
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
                        let mut w = self.swimlane_views[idx].write();
                        undo_accumulator
                            .push(InsensitiveCommand::ResizeElementTo(*uuid, w.bounds_rect));
                        w.bounds_rect = *rect;
                    }

                    let (mut u, mut v) = Default::default();
                    macro_rules! adjust {
                        ($w:expr, $dy:expr) => {
                            $w.apply_command(
                                diagram_model,
                                &InsensitiveCommand::MovePositionalAll(egui::Vec2::new(0.0, $dy)),
                                &mut u,
                                &mut v,
                            );
                            $w.bounds_rect.set_width(rect.width());
                        };
                    }

                    let delta_up = rect.min.y - br.min.y;
                    for e in self.swimlane_views.iter().take(idx).rev() {
                        adjust!(e.write(), delta_up);
                    }

                    let delta_down = rect.max.y - br.max.y;
                    for e in self.swimlane_views.iter().skip(idx + 1) {
                        adjust!(e.write(), delta_down);
                    }
                }

                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self
                    .swimlane_views
                    .iter()
                    .filter(|v| uuids.contains(&v.read().uuid))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (1, None)
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
                        element: BpmnElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }
                let mut delta = egui::Vec2::ZERO;
                let (mut u, mut m) = Default::default();
                let old_sections = std::mem::take(&mut self.swimlane_views);
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
                        self.swimlane_views.push(e);
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
                    let model_uuid = *self.model.read().uuid;
                    if *bucket == 0
                        && let Ok(BpmnElementView::PoolSwimlane(view)) = element.clone().try_into()
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
                                .swimlane_views
                                .first()
                                .map(|e| {
                                    let r = e.read();
                                    (*r.uuid, r.bounds_rect)
                                })
                                .unwrap();
                            if old_rect.width() >= vw.bounds_rect.width() {
                                vw.bounds_rect.set_width(old_rect.width());
                            } else {
                                for e in &self.swimlane_views {
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
                                for e in &self.swimlane_views {
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

                            let old_position = if self.swimlane_views.len() == view_pos.into() {
                                self.swimlane_views
                                    .last()
                                    .map(|e| e.read().bounds_rect.left_bottom())
                            } else {
                                self.swimlane_views
                                    .iter()
                                    .skip(view_pos.into())
                                    .map(|e| e.read().bounds_rect.min)
                                    .next()
                            }
                            .unwrap_or_default();
                            let delta = (0.0, vw.bounds_rect.height()).into();
                            let (mut u, mut m) = Default::default();
                            for e in self.swimlane_views.iter().skip(view_pos.into()) {
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
                            self.swimlane_views
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
                    let model_uuid = *self.model.read().uuid;
                    if *bucket == 0
                        && let Some(view) = self
                            .swimlane_views
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
                                element: BpmnElementView::from(view.clone()).into(),
                                into_model: *including_model,
                            });

                            if *including_model {
                                affected_models.insert(model_uuid);
                            }

                            let mut delta = egui::Vec2::ZERO;
                            let (mut u, mut m) = Default::default();
                            let old_sections = std::mem::take(&mut self.swimlane_views);
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
                                    self.swimlane_views.push(e);
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
                    BpmnOrdinalMovement::Up | BpmnOrdinalMovement::Down => {
                        let lifelines_iter: Box<
                            dyn Iterator<Item = &mut ERef<BpmnPoolSwimlaneView>>,
                        > = match direction {
                            BpmnOrdinalMovement::Up => Box::new(self.swimlane_views.iter_mut()),
                            BpmnOrdinalMovement::Down => {
                                Box::new(self.swimlane_views.iter_mut().rev())
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
                                        BpmnOrdinalMovement::Up => (
                                            (0.0, -destw.bounds_rect.height()).into(),
                                            (0.0, srcw.bounds_rect.height()).into(),
                                        ),
                                        BpmnOrdinalMovement::Down => (
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
                }
                recurse!();
            }
            InsensitiveCommand::PropertyChange(..) => {
                recurse!();
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {}

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.swimlane_views.iter().for_each(|s| {
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
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.swimlane_views
                .iter()
                .for_each(|v| v.read().deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(BpmnElement::Pool(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let mut inner = HashMap::new();
        let new_sections = self
            .swimlane_views
            .iter()
            .map(|v| {
                let v = v.read();
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                let Some(BpmnElementView::PoolSwimlane(s)) = c.get(&v.uuid) else {
                    unreachable!()
                };
                s.clone()
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            swimlane_views: new_sections,

            bounds_rect: self.bounds_rect,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        self.swimlane_views
            .iter_mut()
            .for_each(|v| v.write().deep_copy_relink(c, m));

        let mut w = self.model.write();
        for e in w.swimlanes.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(BpmnElement::PoolSwimlane(new_model)) = m.get(&uuid) {
                *e = new_model.clone();
            }
        }
    }
}

pub fn new_bpmn_poolswimlane(
    name: &str,
    bounds_rect: egui::Rect,
) -> (ERef<BpmnPoolSwimlane>, ERef<BpmnPoolSwimlaneView>) {
    let package_model = ERef::new(BpmnPoolSwimlane::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        Vec::new(),
    ));
    let package_view = new_bpmn_poolswimlane_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_bpmn_poolswimlane_view(
    model: ERef<BpmnPoolSwimlane>,
    bounds_rect: egui::Rect,
) -> ERef<BpmnPoolSwimlaneView> {
    ERef::new(BpmnPoolSwimlaneView {
        uuid: ViewUuid::now_v7().into(),
        model,
        contained_elements: OrderedViews::new(Vec::new()),
        bounds_rect,
        background_color: MGlobalColor::None,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnPoolSwimlaneView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<BpmnPoolSwimlane>,
    #[nh_context_serde(entity)]
    contained_elements: OrderedViews<BpmnElementView>,

    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
    #[nh_context_serde(skip_and_default)]
    temporaries: BpmnPoolSwimlaneViewTemporaries,
}

#[derive(Clone, Default)]
struct BpmnPoolSwimlaneViewTemporaries {
    name_buffer: String,

    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
    all_elements: HashMap<ViewUuid, SelectionStatus>,
}

impl BpmnPoolSwimlaneView {
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

    fn show_properties_inner(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> (PropertiesStatus<BpmnDomain>, Option<BpmnOrdinalMovement>) {
        let mut add_sibling = None::<BpmnOrdinalMovement>;

        if let Some(child) = self.contained_elements.event_order_find_mut(|v| {
            v.show_properties(drawing_context, q, ui, commands)
                .non_default()
        }) {
            return (child, add_sibling);
        }

        if !self.temporaries.highlight.selected {
            return (PropertiesStatus::NotShown, add_sibling);
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
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

        ui.label("Background color:");
        if let Some(new_color) = crate::common::controller::mglobalcolor_edit_button(
            drawing_context,
            ui,
            &self.background_color,
        ) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::ColorChange((0, new_color).into()),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Add sibling above").clicked() {
                add_sibling = Some(BpmnOrdinalMovement::Up);
            }
            if ui.button("Add sibling below").clicked() {
                add_sibling = Some(BpmnOrdinalMovement::Down);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    BpmnOrdinalMovement::Up,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    BpmnOrdinalMovement::Down,
                ));
            }
        });

        (PropertiesStatus::Shown, add_sibling)
    }
}

impl Entity for BpmnPoolSwimlaneView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnPoolSwimlaneView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnPoolSwimlaneView {
    fn model(&self) -> BpmnElement {
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

impl ElementControllerGen2<BpmnDomain> for BpmnPoolSwimlaneView {
    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        _q: &<BpmnDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                <BpmnDomain as Domain>::OrdinalMovementT,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        unreachable!()
    }

    fn draw_in(
        &mut self,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &BpmnSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveBpmnTool)>,
    ) -> TargettingStatus {
        let background_color = context
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );
        canvas.draw_rectangle(
            egui::Rect::from_min_size(
                self.bounds_rect.left_top(),
                (
                    2.5 * canvas::CLASS_MIDDLE_FONT_SIZE,
                    self.bounds_rect.height(),
                )
                    .into(),
            ),
            egui::CornerRadius::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );

        canvas.draw_text(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            &self.temporaries.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw resize/drag handles
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            let handle_size = self.handle_size(ui_scale);
            let handles_rect = self.bounds_rect.shrink(handle_size / 2.0);
            for (h, c) in [
                (handles_rect.left_top(), "↖"),
                (handles_rect.center_top(), "^"),
                (handles_rect.right_top(), "↗"),
                (handles_rect.left_center(), "<"),
                (handles_rect.right_center(), ">"),
                (handles_rect.left_bottom(), "↙"),
                (handles_rect.center_bottom(), "v"),
                (handles_rect.right_bottom(), "↘"),
            ] {
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size / ui_scale)),
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_text(
                    h,
                    egui::Align2::CENTER_CENTER,
                    c,
                    10.0 / ui_scale,
                    egui::Color32::BLACK,
                );
            }

            let dc = self.drag_handle_position(ui_scale);
            canvas.draw_rectangle(
                egui::Rect::from_center_size(dc, egui::Vec2::splat(handle_size / ui_scale)),
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );

            let da_radius = (handle_size / 2.0 - 1.0) / ui_scale;
            canvas.draw_line(
                [
                    dc - egui::Vec2::new(0.0, da_radius),
                    dc + egui::Vec2::new(0.0, da_radius),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_line(
                [
                    dc - egui::Vec2::new(da_radius, 0.0),
                    dc + egui::Vec2::new(da_radius, 0.0),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
        }

        macro_rules! draw_header_and_children {
            () => {{
                canvas.open_header(
                    &self.temporaries.name_buffer,
                    canvas::HeaderLocation::Vertical,
                    self.bounds_rect,
                    Some((
                        canvas::UiCanvas::HEADER_CELL_BACKGROUND,
                        egui::Color32::BLACK,
                    )),
                );

                let mut targetting_drawn = false;
                self.contained_elements.draw_order_foreach_mut(|e| {
                    targetting_drawn |=
                        e.draw_in(q, context, settings, canvas, tool) != TargettingStatus::NotDrawn;
                });

                canvas.close_header(canvas::HeaderLocation::Vertical, self.bounds_rect);

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
        settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveBpmnTool>,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> EventHandlingStatus {
        let k_status = self.contained_elements.event_order_find_mut(|v| {
            let s = v.handle_event(event, ehc, settings, q, tool, commands);
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
                let handles_rect = self.bounds_rect.shrink(handle_size / 2.0);
                if self.temporaries.highlight.selected {
                    for (a, h) in [
                        (egui::Align2::RIGHT_BOTTOM, handles_rect.left_top()),
                        (egui::Align2::CENTER_BOTTOM, handles_rect.center_top()),
                        (egui::Align2::LEFT_BOTTOM, handles_rect.right_top()),
                        (egui::Align2::RIGHT_CENTER, handles_rect.left_center()),
                        (egui::Align2::LEFT_CENTER, handles_rect.right_center()),
                        (egui::Align2::RIGHT_TOP, handles_rect.left_bottom()),
                        (egui::Align2::CENTER_TOP, handles_rect.center_bottom()),
                        (egui::Align2::LEFT_TOP, handles_rect.right_bottom()),
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

                    let _ = tool.try_flush(q, &self.uuid, 0, None, commands);

                    EventHandlingStatus::HandledByContainer
                } else if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc
                            .modifier_settings
                            .hold_selection
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            commands.push(
                                InsensitiveCommand::HighlightAll(
                                    false,
                                    canvas::Highlight::SELECTED,
                                )
                                .into(),
                            );
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(k).collect(),
                                    true,
                                    canvas::Highlight::SELECTED,
                                )
                                .into(),
                            );
                        } else {
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(k).collect(),
                                    !self.temporaries.selected_direct_elements.contains(&k),
                                    canvas::Highlight::SELECTED,
                                )
                                .into(),
                            );
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
        diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
                        (1, None)
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
                        && let Ok(mut view) = BpmnElementView::try_from(element.clone())
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
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    match property {
                        BpmnPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        BpmnPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
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

        self.temporaries.name_buffer = (*r.name).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
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

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.contained_elements
                .event_order_foreach(|v| v.deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, BpmnElementView>,
        c: &mut HashMap<ViewUuid, BpmnElementView>,
        m: &mut HashMap<ModelUuid, BpmnElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(BpmnElement::PoolSwimlane(m)) = m.get(&old_model.uuid) {
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

            bounds_rect: self.bounds_rect.clone(),
            background_color: self.background_color.clone(),
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        self.contained_elements
            .event_order_foreach_mut(|v| v.deep_copy_relink(c, m));

        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            let uuid = *e.uuid();
            if let Some(new_model) = m.get(&uuid).and_then(|e| e.as_standalone()) {
                *e = new_model;
            }
        }
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
fn draw_nonfinal_node_button_rects(
    settings: &<BpmnDomain as Domain>::SettingsT,
    canvas: &mut dyn NHCanvas,
    origin: egui::Pos2,
    ui_scale: f32,
) {
    for (row_idx, col_idx, l, _f) in settings.nonfinal_buttons.iter() {
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
fn handle_nonfinal_node_button_click(
    settings: &<BpmnDomain as Domain>::SettingsT,
    origin: egui::Pos2,
    ui_scale: f32,
    click_pos: egui::Pos2,
) -> Option<&NonFinalElementButtonF> {
    for (row_idx, col_idx, _l, f) in settings.nonfinal_buttons.iter() {
        let r = nonfinal_node_button_rect(origin, ui_scale, *row_idx, *col_idx);
        if r.contains(click_pos) {
            return Some(f);
        }
    }
    None
}

fn new_bpmn_task(
    name: &str,
    kind: BpmnTaskKind,
    is_call: bool,
    position: egui::Pos2,
    background_color: MGlobalColor,
    request_focus: bool,
) -> (ERef<BpmnTask>, ERef<BpmnTaskView>) {
    let model = ERef::new(BpmnTask::new(
        ModelUuid::now_v7(),
        kind,
        is_call,
        name.to_owned(),
    ));
    let view = new_bpmn_task_view(model.clone(), position, background_color, request_focus);

    (model, view)
}
fn new_bpmn_task_view(
    model: ERef<BpmnTask>,
    position: egui::Pos2,
    background_color: MGlobalColor,
    request_focus: bool,
) -> ERef<BpmnTaskView> {
    let m = model.read();
    ERef::new(BpmnTaskView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        attached_events: Vec::new(),
        temporaries: BpmnTaskViewTemporaries {
            all_children_status: Default::default(),
            request_focus,
            name_buffer: (*m.name).clone(),
            kind_buffer: m.kind,
            is_call_buffer: m.is_call,
            dragged_shape: None,
            highlight: canvas::Highlight::from_selected(request_focus),
        },
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnTaskView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<BpmnTask>,
    #[nh_context_serde(entity)]
    attached_events: Vec<ERef<BpmnEventView>>,

    #[nh_context_serde(skip_and_default)]
    temporaries: BpmnTaskViewTemporaries,

    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

#[derive(Clone, Default)]
struct BpmnTaskViewTemporaries {
    all_children_status: HashMap<ViewUuid, SelectionStatus>,

    request_focus: bool,
    name_buffer: String,
    kind_buffer: BpmnTaskKind,
    is_call_buffer: bool,

    dragged_shape: Option<NHShape>,
    highlight: canvas::Highlight,
}

impl BpmnTaskView {
    fn refresh_all_children_status(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        self.temporaries.all_children_status.clear();
        self.attached_events.iter_mut().for_each(|v| {
            v.write().head_count(
                flattened_views,
                &mut self.temporaries.all_children_status,
                flattened_represented_models,
            )
        });
        for e in &self.temporaries.all_children_status {
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
    }
}

impl Entity for BpmnTaskView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnTaskView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnTaskView {
    fn model(&self) -> BpmnElement {
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

impl ElementControllerGen2<BpmnDomain> for BpmnTaskView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_multiline2(
                "Name:",
                &mut self.temporaries.name_buffer,
                self.temporaries.request_focus,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }
        self.temporaries.request_focus = false;

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in BpmnTaskKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            BpmnPropChange::TaskKindChange(e),
                        ));
                    }
                }
            });

        if ui
            .checkbox(&mut self.temporaries.is_call_buffer, "is call")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::IsCallChange(self.temporaries.is_call_buffer),
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

        ui.label("Background color:");
        if let Some(new_color) =
            crate::common::controller::mglobalcolor_edit_button(gdc, ui, &self.background_color)
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveBpmnTool)>,
    ) -> TargettingStatus {
        let background_color = context
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);

        self.bounds_rect = canvas
            .measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &self.temporaries.name_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .expand(20.0);
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::same(10),
            background_color,
            canvas::Stroke::new_solid(
                match self.temporaries.is_call_buffer {
                    false => 1.0,
                    true => 2.0,
                },
                egui::Color32::BLACK,
            ),
            self.temporaries.highlight,
        );
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.temporaries.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        let icon_rect = egui::Rect::from_min_size(
            self.bounds_rect.left_top() + (5.0, 5.0).into(),
            (15.0, 15.0).into(),
        );
        match self.temporaries.kind_buffer {
            BpmnTaskKind::Unspecified => {}
            BpmnTaskKind::Send | BpmnTaskKind::Receive => {
                // draw an envelope icon
                let icon_rect = icon_rect.shrink2((0.0, 2.5).into());
                let (bg, fg) = match self.temporaries.kind_buffer {
                    BpmnTaskKind::Send => (egui::Color32::BLACK, egui::Color32::WHITE),
                    BpmnTaskKind::Receive => (egui::Color32::WHITE, egui::Color32::BLACK),
                    _ => unreachable!(),
                };
                canvas.draw_rectangle(
                    icon_rect,
                    egui::CornerRadius::ZERO,
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );
                for e in [icon_rect.left_top(), icon_rect.right_top()] {
                    canvas.draw_line(
                        [e, icon_rect.center()],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnTaskKind::User => {
                let lb = icon_rect.left_bottom();
                let rb = icon_rect.right_bottom();
                canvas.draw_line(
                    [lb, rb],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let height6 = icon_rect.height() / 6.0;
                for (e1, d) in [(lb, 1.0), (rb, -1.0)] {
                    let d = d * height6;
                    for e2 in [e1, icon_rect.center()] {
                        canvas.draw_line(
                            [e1 + (0.0, -icon_rect.height() / 3.0).into(), e2],
                            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                            canvas::Highlight::NONE,
                        );
                    }
                    canvas.draw_line(
                        [e1 + (d, 0.0).into(), e1 + (d, -height6).into()],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                canvas.draw_ellipse(
                    icon_rect.center() + (0.0, -height6 / 2.0).into(),
                    egui::Vec2::splat(icon_rect.height() / 4.0),
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            BpmnTaskKind::Manual => {
                canvas.draw_text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✋",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
            BpmnTaskKind::BusinessRule => {
                let table_rect = icon_rect.shrink2((0.0, 2.5).into());
                canvas.draw_rectangle(
                    table_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let small_step = table_rect.height() / 5.0;
                let larger_step = 2.0 * table_rect.height() / 5.0;
                for e in [small_step, small_step + larger_step] {
                    canvas.draw_line(
                        [
                            table_rect.left_top() + (0.0, e).into(),
                            table_rect.right_top() + (0.0, e).into(),
                        ],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                canvas.draw_line(
                    [
                        table_rect.center_top() + (0.0, small_step).into(),
                        table_rect.center_bottom(),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            BpmnTaskKind::Service => {
                canvas.draw_text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "⚙",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
            BpmnTaskKind::Script => {
                let height3 = icon_rect.height() / 3.0;
                let left1 = icon_rect.left_top() + (0.0, height3).into();
                let right2 = icon_rect.right_bottom() + (0.0, -height3).into();
                for e in [
                    icon_rect.left_top() + (height3, 0.0).into(),
                    icon_rect.right_top(),
                    icon_rect.right_top() + (-height3, height3).into(),
                    right2,
                    icon_rect.right_bottom() + (-height3, 0.0).into(),
                    icon_rect.left_bottom(),
                    icon_rect.left_bottom() + (height3, -height3).into(),
                    left1,
                    icon_rect.left_top() + (height3, 0.0).into(),
                ]
                .array_windows::<2>()
                {
                    canvas.draw_line(
                        *e,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                let row_width = icon_rect.width() / 2.5;
                let row_x_adjust = height3 / 1.6;
                let row_y_adjust = height3 / 3.7;
                for p1 in [
                    left1 + (row_x_adjust, -row_y_adjust).into(),
                    left1 + (row_x_adjust, row_y_adjust).into(),
                    right2 + (-row_width - row_x_adjust, -row_y_adjust).into(),
                    right2 + (-row_width - row_x_adjust, row_y_adjust).into(),
                ] {
                    canvas.draw_line(
                        [p1, p1 + (row_width, 0.0).into()],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
        }

        // Draw buttons
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
        {
            draw_nonfinal_node_button_rects(
                settings,
                canvas,
                self.bounds_rect.right_top(),
                ui_scale,
            );
        }

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;
        for e in self.attached_events.iter_mut() {
            drawn_child_targetting |= e.write().draw_in(q, context, settings, canvas, tool);
        }

        if canvas.ui_scale().is_some() {
            // Draw targetting rectangle
            match (drawn_child_targetting, tool) {
                (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                    canvas.draw_rectangle(
                        self.bounds_rect,
                        egui::CornerRadius::same(10),
                        t.targetting_for_section(Ok(self.model())),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );

                    for e in self.attached_events.iter_mut() {
                        e.write().draw_in(q, context, settings, canvas, tool);
                    }

                    TargettingStatus::Drawn
                }
                _ => drawn_child_targetting,
            }
        } else {
            TargettingStatus::NotDrawn
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());

        for e in self.attached_events.iter_mut() {
            e.write().collect_allignment(am);
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveBpmnTool>,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> EventHandlingStatus {
        let k_status = self
            .attached_events
            .iter()
            .flat_map(|v| {
                let mut w = v.write();
                let s = w.handle_event(event, ehc, settings, q, tool, commands);
                if s != EventHandlingStatus::NotHandled {
                    Some((*w.uuid(), s))
                } else {
                    None
                }
            })
            .next();

        match event {
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) if k_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }
                self.temporaries.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.temporaries.dragged_shape.is_some() {
                    self.temporaries.dragged_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos)
                if self.temporaries.highlight.selected
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveBpmnTool {
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
                if !self.bounds_rect.contains(pos) {
                    return k_status
                        .map(|e| e.1)
                        .unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());

                    if !tool.result_references(&self.model.read().uuid) {
                        let _ = tool.try_flush(q, &self.uuid, 0, None, commands);
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
                                !self
                                    .temporaries
                                    .all_children_status
                                    .get(&k)
                                    .is_some_and(|e| e.selected()),
                                canvas::Highlight::SELECTED,
                            ));
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            }
            InputEvent::Drag { delta, .. } if self.temporaries.dragged_shape.is_some() => {
                let translated_real_shape =
                    self.temporaries.dragged_shape.unwrap().translate(delta);
                self.temporaries.dragged_shape = Some(translated_real_shape);
                let coerced_pos = if self.temporaries.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| {
                        !self.temporaries.all_children_status.contains_key(e)
                            && !ehc
                                .all_elements
                                .get(e)
                                .is_some_and(|e| *e != SelectionStatus::NotSelected)
                    })
                } else {
                    ehc.snap_manager
                        .coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.bounds_rect.center();

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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.attached_events.iter().for_each(|v| {
                    v.write().apply_command(
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
                    let new_state = match set {
                        true => SelectionStatus::Selected,
                        false => SelectionStatus::NotSelected,
                    };
                    self.temporaries
                        .all_children_status
                        .iter_mut()
                        .for_each(|e| *e.1 = new_state);
                }
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                }

                if h.selected {
                    let new_state = match set {
                        true => SelectionStatus::Selected,
                        false => SelectionStatus::NotSelected,
                    };
                    for k in self
                        .attached_events
                        .iter()
                        .map(|e| *e.read().uuid)
                        .filter(|k| uuids.contains(k))
                    {
                        if let Some(e) = self.temporaries.all_children_status.get_mut(&k) {
                            *e = new_state;
                        }
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected = (self.temporaries.highlight.selected
                    && *retain)
                    || rect.contains_rect(self.bounds_rect);

                recurse!();

                self.refresh_all_children_status(
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                );
            }
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.attached_events.iter().for_each(|v| {
                    v.write().apply_command(
                        diagram_model,
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self.attached_events.iter() {
                    let r = element.read();
                    if !uuids.contains(&r.uuid) {
                        continue;
                    }

                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
                    } else if let Some((b, pos)) =
                        self.model.read().get_element_pos(&r.model_uuid())
                    {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid,
                        bucket: b,
                        position: pos,
                        element: BpmnElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }

                self.attached_events
                    .retain(|v| !uuids.contains(&v.read().uuid));

                recurse!();
            }
            InsensitiveCommand::AddDependency {
                target,
                bucket,
                position,
                element,
                into_model,
            } => {
                let model_uuid = *self.model_uuid();
                if *target == *self.uuid
                    && *bucket == 0
                    && let BpmnElementOrVertex::Element(BpmnElementView::Event(view)) =
                        element.clone()
                    && (!*into_model
                        || diagram_model
                            .write()
                            .insert_element_into(model_uuid, 0, *position, view.read().model())
                            .is_ok())
                {
                    let uuid = *view.read().uuid();
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
                    view.write().head_count(
                        &mut HashMap::new(),
                        &mut HashMap::new(),
                        &mut model_transitives,
                    );
                    affected_models.extend(model_transitives.into_keys());

                    self.attached_events.push(view);
                }

                recurse!();
            }
            InsensitiveCommand::RemoveDependency {
                target,
                bucket,
                element,
                including_model,
            } => {
                let model_uuid = *self.model_uuid();
                if *target == *self.uuid
                    && *bucket == 0
                    && let Some(view) = self
                        .attached_events
                        .iter()
                        .find(|e| *e.read().uuid == *element)
                        .cloned()
                    && let Some((_, pos)) = diagram_model
                        .write()
                        .remove_element_from(model_uuid, &view.read().model_uuid())
                {
                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid,
                        bucket: *bucket,
                        position: Some(pos),
                        element: BpmnElementView::from(view.clone()).into(),
                        into_model: *including_model,
                    });

                    if *including_model {
                        affected_models.insert(model_uuid);
                    }

                    self.attached_events.retain(|v| *v.read().uuid != *element);
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(..)
            | InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        BpmnPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        BpmnPropChange::TaskKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::TaskKindChange(model.kind),
                            ));
                            model.kind = *kind;
                        }
                        BpmnPropChange::IsCallChange(is_call) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::IsCallChange(model.is_call),
                            ));
                            model.is_call = *is_call;
                        }
                        BpmnPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
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

        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.kind_buffer = model.kind;
        self.temporaries.is_call_buffer = model.is_call;
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.refresh_all_children_status(
            flattened_views,
            flattened_views_status,
            flattened_represented_models,
        );

        self.attached_events.iter_mut().for_each(|v| {
            flattened_views.insert(*v.read().uuid(), (v.clone().into(), *self.uuid));
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            for e in &self.attached_events {
                e.read().deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, BpmnElementView>,
        c: &mut HashMap<ViewUuid, BpmnElementView>,
        m: &mut HashMap<ModelUuid, BpmnElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(BpmnElement::Task(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let mut inner = HashMap::new();
        self.attached_events
            .iter()
            .for_each(|v| v.read().deep_copy_clone(uuid_present, &mut inner, c, m));
        let attached_events = self
            .attached_events
            .iter()
            .flat_map(|e| {
                if let Some(BpmnElementView::Event(e)) = inner.get(&e.read().uuid) {
                    Some(e.clone())
                } else {
                    None
                }
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            attached_events,
            temporaries: self.temporaries.clone(),
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_bpmn_startevent(
    kind: BpmnEventKind,
    subkind: BpmnStartEventKind,
    name: &str,
    position: egui::Pos2,
    request_focus: bool,
) -> (ERef<BpmnStartEvent>, ERef<BpmnEventView>) {
    let model = ERef::new(BpmnStartEvent::new(
        ModelUuid::now_v7(),
        kind,
        subkind,
        name.to_owned(),
    ));
    let view = new_bpmn_event_view(model.clone().into(), position, request_focus);

    (model, view)
}
fn new_bpmn_intermediateevent(
    kind: BpmnEventKind,
    subkind: BpmnIntermediateEventKind,
    name: &str,
    position: egui::Pos2,
    request_focus: bool,
) -> (ERef<BpmnIntermediateEvent>, ERef<BpmnEventView>) {
    let model = ERef::new(BpmnIntermediateEvent::new(
        ModelUuid::now_v7(),
        kind,
        subkind,
        name.to_owned(),
    ));
    let view = new_bpmn_event_view(model.clone().into(), position, request_focus);

    (model, view)
}
fn new_bpmn_boundaryinetermediateevent(
    kind: BpmnEventKind,
    subkind: BpmnBoundaryIntermediateEventKind,
    name: &str,
    position: egui::Pos2,
    request_focus: bool,
) -> (ERef<BpmnBoundaryIntermediateEvent>, ERef<BpmnEventView>) {
    let model = ERef::new(BpmnBoundaryIntermediateEvent::new(
        ModelUuid::now_v7(),
        kind,
        subkind,
        name.to_owned(),
    ));
    let view = new_bpmn_event_view(model.clone().into(), position, request_focus);

    (model, view)
}
fn new_bpmn_endevent(
    kind: BpmnEventKind,
    name: &str,
    position: egui::Pos2,
    request_focus: bool,
) -> (ERef<BpmnEndEvent>, ERef<BpmnEventView>) {
    let model = ERef::new(BpmnEndEvent::new(
        ModelUuid::now_v7(),
        kind,
        name.to_owned(),
    ));
    let view = new_bpmn_event_view(model.clone().into(), position, request_focus);

    (model, view)
}

fn new_bpmn_event_view(
    model: BpmnEvent,
    position: egui::Pos2,
    request_focus: bool,
) -> ERef<BpmnEventView> {
    ERef::new(BpmnEventView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        temporaries: BpmnEventViewTemporaries {
            request_focus,
            kind_buffer: Default::default(),
            subkind_buffer: Default::default(),
            name_buffer: match model {
                BpmnEvent::StartEvent(inner) => (*inner.read().name).clone(),
                BpmnEvent::IntermediateEvent(inner) => (*inner.read().name).clone(),
                BpmnEvent::BoundaryIntermediateEvent(inner) => (*inner.read().name).clone(),
                BpmnEvent::EndEvent(inner) => (*inner.read().name).clone(),
            },
            dragged_shape: None,
            highlight: canvas::Highlight::from_selected(request_focus),
        },
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnEventView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: BpmnEvent,

    #[nh_context_serde(skip_and_default)]
    temporaries: BpmnEventViewTemporaries,
    position: egui::Pos2,
}

#[derive(Clone, Default)]
struct BpmnEventViewTemporaries {
    request_focus: bool,
    kind_buffer: BpmnEventKind,
    subkind_buffer: BpmnEventViewSubkindBuffer,
    name_buffer: String,

    dragged_shape: Option<NHShape>,
    highlight: canvas::Highlight,
}

#[derive(Clone, Default)]
enum BpmnEventViewSubkindBuffer {
    Start(BpmnStartEventKind),
    Intermediate(BpmnIntermediateEventKind),
    BoundaryIntermediate(BpmnBoundaryIntermediateEventKind),
    #[default]
    End,
}

impl BpmnEventView {
    const CIRCLE_RADIUS: f32 = 25.0;
    fn buttons_origin(&self) -> egui::Pos2 {
        self.position + egui::Vec2::new(Self::CIRCLE_RADIUS, -Self::CIRCLE_RADIUS)
    }
}

impl Entity for BpmnEventView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnEventView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.uuid()
    }
}

impl ElementController<BpmnElement> for BpmnEventView {
    fn model(&self) -> BpmnElement {
        self.model.clone().to_element()
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

impl ElementControllerGen2<BpmnDomain> for BpmnEventView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                BpmnOrdinalMovement,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline2(
                "Name:",
                &mut self.temporaries.name_buffer,
                self.temporaries.request_focus,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }
        self.temporaries.request_focus = false;

        ui.label("Event kind:");
        egui::ComboBox::from_id_salt("event kind")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in BpmnEventKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            BpmnPropChange::EventKindChange(self.temporaries.kind_buffer),
                        ));
                    }
                }
            });

        match &mut self.temporaries.subkind_buffer {
            BpmnEventViewSubkindBuffer::Start(subkind) => {
                ui.label("Start event kind:");
                egui::ComboBox::from_id_salt("start event kind")
                    .selected_text(subkind.as_str())
                    .show_ui(ui, |ui| {
                        for e in BpmnStartEventKind::VARIANTS {
                            if ui.selectable_value(subkind, e, e.as_str()).clicked() {
                                commands.push(InsensitiveCommand::PropertyChange(
                                    q.selected_views(),
                                    BpmnPropChange::StartEventSubkindChange(*subkind),
                                ));
                            }
                        }
                    });
            }
            BpmnEventViewSubkindBuffer::Intermediate(subkind) => {
                ui.label("Intermediate event kind:");
                egui::ComboBox::from_id_salt("intermediate event kind")
                    .selected_text(subkind.as_str())
                    .show_ui(ui, |ui| {
                        for e in BpmnIntermediateEventKind::VARIANTS {
                            if ui.selectable_value(subkind, e, e.as_str()).clicked() {
                                commands.push(InsensitiveCommand::PropertyChange(
                                    q.selected_views(),
                                    BpmnPropChange::IntermediateEventSubkindChange(*subkind),
                                ));
                            }
                        }
                    });
            }
            BpmnEventViewSubkindBuffer::BoundaryIntermediate(subkind) => {
                ui.label("Boundary intermediate event kind:");
                egui::ComboBox::from_id_salt("boundary intermediate event kind")
                    .selected_text(subkind.as_str())
                    .show_ui(ui, |ui| {
                        for e in BpmnBoundaryIntermediateEventKind::VARIANTS {
                            if ui.selectable_value(subkind, e, e.as_str()).clicked() {
                                commands.push(InsensitiveCommand::PropertyChange(
                                    q.selected_views(),
                                    BpmnPropChange::BoundaryIntermediateEventSubkindChange(
                                        *subkind,
                                    ),
                                ));
                            }
                        }
                    });
            }
            BpmnEventViewSubkindBuffer::End => {}
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
        _q: &<BpmnDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<BpmnDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        // draw circle type based on event type
        const INNER_CIRCLE: f32 = BpmnEventView::CIRCLE_RADIUS - 3.0;
        let icon_rect =
            egui::Rect::from_center_size(self.position, egui::Vec2::splat(Self::CIRCLE_RADIUS));
        match &self.temporaries.subkind_buffer {
            BpmnEventViewSubkindBuffer::Start(subkind) => {
                let s = match subkind {
                    BpmnStartEventKind::Interrupting => {
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK)
                    }
                    BpmnStartEventKind::Noninterrupting => {
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK)
                    }
                };
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(BpmnEventView::CIRCLE_RADIUS),
                    egui::Color32::WHITE,
                    s,
                    self.temporaries.highlight,
                );
            }
            BpmnEventViewSubkindBuffer::Intermediate(_) => {
                for r in [BpmnEventView::CIRCLE_RADIUS, INNER_CIRCLE] {
                    canvas.draw_ellipse(
                        self.position,
                        egui::Vec2::splat(r),
                        egui::Color32::WHITE,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        self.temporaries.highlight,
                    );
                }
            }
            BpmnEventViewSubkindBuffer::BoundaryIntermediate(subkind) => {
                let s = match subkind {
                    BpmnBoundaryIntermediateEventKind::Interrupting => {
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK)
                    }
                    BpmnBoundaryIntermediateEventKind::Noninterrupting => {
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK)
                    }
                };
                for r in [BpmnEventView::CIRCLE_RADIUS, INNER_CIRCLE] {
                    canvas.draw_ellipse(
                        self.position,
                        egui::Vec2::splat(r),
                        egui::Color32::WHITE,
                        s,
                        self.temporaries.highlight,
                    );
                }
            }
            BpmnEventViewSubkindBuffer::End => {
                for (r, c) in [
                    (BpmnEventView::CIRCLE_RADIUS, egui::Color32::BLACK),
                    (INNER_CIRCLE, egui::Color32::WHITE),
                ] {
                    canvas.draw_ellipse(
                        self.position,
                        egui::Vec2::splat(r),
                        c,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        self.temporaries.highlight,
                    );
                }
            }
        }
        // draw event kind icon
        let (fg, bg) = match &self.temporaries.subkind_buffer {
            BpmnEventViewSubkindBuffer::Intermediate(BpmnIntermediateEventKind::Throwing)
            | BpmnEventViewSubkindBuffer::End => (egui::Color32::WHITE, egui::Color32::BLACK),
            _ => (egui::Color32::BLACK, egui::Color32::WHITE),
        };
        match self.temporaries.kind_buffer {
            BpmnEventKind::Unspecified => {}
            BpmnEventKind::Message => {
                let envelope_rect = icon_rect.shrink2((0.0, 2.5).into());
                canvas.draw_rectangle(
                    envelope_rect,
                    egui::CornerRadius::ZERO,
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );
                for e in [envelope_rect.left_top(), envelope_rect.right_top()] {
                    canvas.draw_line(
                        [e, envelope_rect.center()],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Timer => {
                let r = icon_rect.width() / 2.0;
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(r),
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );

                for (h, l) in [(1.0_f32, r - 3.0), (3.0_f32, r / 2.0)] {
                    let a = (450.0 - 360.0 * h / 12.0).to_radians();
                    let d = (a.cos() * l, -a.sin() * l);
                    canvas.draw_line(
                        [self.position, self.position + d.into()],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Escalation => {
                for e1 in [icon_rect.left_bottom(), icon_rect.right_bottom()] {
                    canvas.draw_polygon(
                        [icon_rect.center_top(), e1, icon_rect.center()].to_vec(),
                        bg,
                        canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                        canvas::Highlight::NONE,
                    );
                    for e2 in [icon_rect.center_top(), icon_rect.center()] {
                        canvas.draw_line(
                            [e1, e2],
                            canvas::Stroke::new_solid(1.0, fg),
                            canvas::Highlight::NONE,
                        );
                    }
                }
            }
            BpmnEventKind::Conditional => {
                canvas.draw_rectangle(
                    icon_rect,
                    egui::CornerRadius::ZERO,
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );
                let padding_x = icon_rect.width() / 10.0;
                let step_y = icon_rect.height() / 5.0;
                for y in (1..=4).map(|e| icon_rect.min.y + e as f32 * step_y) {
                    canvas.draw_line(
                        [
                            (icon_rect.min.x + padding_x, y).into(),
                            (icon_rect.max.x - padding_x, y).into(),
                        ],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Link => {
                canvas.draw_polygon(
                    [
                        icon_rect.center_top(),
                        icon_rect.right_center(),
                        icon_rect.center_bottom(),
                    ]
                    .to_vec(),
                    bg,
                    canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                let tail_height2: f32 = icon_rect.height() / 5.0;
                let tail_a = icon_rect.left_center() + (0.0, -tail_height2).into();
                let tail_b = icon_rect.center() + (0.0, -tail_height2).into();
                let tail_c = icon_rect.center() + (0.0, tail_height2).into();
                let tail_d = icon_rect.left_center() + (0.0, tail_height2).into();
                canvas.draw_polygon(
                    [tail_a, tail_b, tail_c, tail_d].to_vec(),
                    bg,
                    canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                for [p1, p2] in [
                    icon_rect.center_top(),
                    icon_rect.right_center(),
                    icon_rect.center_bottom(),
                    tail_c,
                    tail_d,
                    tail_a,
                    tail_b,
                    icon_rect.center_top(),
                ]
                .array_windows()
                {
                    canvas.draw_line(
                        [*p1, *p2],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Error => {
                let step_x = icon_rect.width() / 3.0;
                let left_turn_top = icon_rect.left_top() + (step_x, 0.0).into();
                let left_turn_bottom = icon_rect.left_center() + (step_x, 0.0).into();
                let right_turn_top = icon_rect.right_center() + (-step_x, 0.0).into();
                let right_turn_bottom = icon_rect.right_bottom() + (-step_x, 0.0).into();
                canvas.draw_polygon(
                    [icon_rect.left_bottom(), left_turn_top, left_turn_bottom].to_vec(),
                    bg,
                    canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        left_turn_top,
                        right_turn_top,
                        right_turn_bottom,
                        left_turn_bottom,
                    ]
                    .to_vec(),
                    bg,
                    canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [icon_rect.right_top(), right_turn_bottom, right_turn_top].to_vec(),
                    bg,
                    canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                for (p1, p2) in [
                    (icon_rect.left_bottom(), left_turn_top),
                    (left_turn_top, right_turn_top),
                    (right_turn_top, icon_rect.right_top()),
                    (icon_rect.right_top(), right_turn_bottom),
                    (right_turn_bottom, left_turn_bottom),
                    (left_turn_bottom, icon_rect.left_bottom()),
                ] {
                    canvas.draw_line(
                        [p1, p2],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Cancel => {
                let [top_left, left_top, right_bottom, bottom_right] =
                    [(-0.33, -0.47), (-0.47, -0.33), (0.47, 0.33), (0.33, 0.47)].map(|(x, y)| {
                        self.position + (x * icon_rect.width(), y * icon_rect.height()).into()
                    });
                let [top_right, right_top, left_bottom, bottom_left] =
                    [(0.33, -0.47), (0.47, -0.33), (-0.47, 0.33), (-0.33, 0.47)].map(|(x, y)| {
                        self.position + (x * icon_rect.width(), y * icon_rect.height()).into()
                    });

                let [center_top, center_bottom] =
                    [-0.14, 0.14].map(|e| self.position + (0.0, e * icon_rect.height()).into());
                let [center_left, center_right] =
                    [-0.14, 0.14].map(|e| self.position + (e * icon_rect.width(), 0.0).into());

                for e in [
                    [top_left, right_bottom, bottom_right, left_top],
                    [top_right, right_top, bottom_left, left_bottom],
                ] {
                    canvas.draw_polygon(
                        e.to_vec(),
                        bg,
                        canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                for [p1, p2] in [
                    top_left,
                    center_top,
                    top_right,
                    right_top,
                    center_right,
                    right_bottom,
                    bottom_right,
                    center_bottom,
                    bottom_left,
                    left_bottom,
                    center_left,
                    left_top,
                    top_left,
                ]
                .array_windows()
                {
                    canvas.draw_line(
                        [*p1, *p2],
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Compensation => {
                for (a, b, c) in [
                    (
                        icon_rect.left_center(),
                        icon_rect.center_top(),
                        icon_rect.center_bottom(),
                    ),
                    (
                        icon_rect.center(),
                        icon_rect.right_top(),
                        icon_rect.right_bottom(),
                    ),
                ] {
                    canvas.draw_polygon(
                        [a, b, c].to_vec(),
                        bg,
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Signal => {
                let r = icon_rect.height() / 2.0;
                canvas.draw_polygon(
                    (0..=2)
                        .map(|e| (450.0 - e as f32 * 120.0).to_radians())
                        .map(|e| self.position + (e.cos() * r, -e.sin() * r).into())
                        .collect(),
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );
            }
            BpmnEventKind::Multiple => {
                let r = icon_rect.height() / 2.0;
                canvas.draw_polygon(
                    (0..=4)
                        .map(|e| (450.0 - e as f32 * 72.0).to_radians())
                        .map(|e| self.position + (e.cos() * r, -e.sin() * r).into())
                        .collect(),
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );
            }
            BpmnEventKind::ParallelMultiple => {
                const PLUS_SHORTER: f32 = 0.15;
                const PLUS_LONGER: f32 = 0.6 * 0.75;

                for [p1, p2] in [
                    (PLUS_SHORTER, -PLUS_LONGER),
                    (PLUS_SHORTER, -PLUS_SHORTER),
                    (PLUS_LONGER, -PLUS_SHORTER),
                    (PLUS_LONGER, PLUS_SHORTER),
                    (PLUS_SHORTER, PLUS_SHORTER),
                    (PLUS_SHORTER, PLUS_LONGER),
                    (-PLUS_SHORTER, PLUS_LONGER),
                    (-PLUS_SHORTER, PLUS_SHORTER),
                    (-PLUS_LONGER, PLUS_SHORTER),
                    (-PLUS_LONGER, -PLUS_SHORTER),
                    (-PLUS_SHORTER, -PLUS_SHORTER),
                    (-PLUS_SHORTER, -PLUS_LONGER),
                    (PLUS_SHORTER, -PLUS_LONGER),
                ]
                .array_windows()
                {
                    let ps = [p1, p2]
                        .map(|e| (e.0 * icon_rect.width(), e.1 * icon_rect.height()))
                        .map(|e| self.position + e.into());
                    canvas.draw_line(
                        ps,
                        canvas::Stroke::new_solid(1.0, fg),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnEventKind::Terminate => {
                canvas.draw_ellipse(
                    self.position,
                    icon_rect.size() / 2.0,
                    bg,
                    canvas::Stroke::new_solid(1.0, fg),
                    canvas::Highlight::NONE,
                );
            }
        }

        if !self.temporaries.name_buffer.is_empty() {
            canvas.draw_text(
                self.position
                    + (
                        0.0,
                        Self::CIRCLE_RADIUS + canvas::CLASS_MIDDLE_FONT_SIZE / 2.0,
                    )
                        .into(),
                egui::Align2::CENTER_TOP,
                &self.temporaries.name_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        // Draw buttons
        if let Some(ui_scale) = canvas
            .ui_scale()
            .filter(|_| self.temporaries.highlight.selected)
            .filter(|_| !matches!(self.model, BpmnEvent::EndEvent(_)))
        {
            draw_nonfinal_node_button_rects(settings, canvas, self.buttons_origin(), ui_scale);
        }

        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && self.min_shape().contains(*pos)
        {
            canvas.draw_ellipse(
                self.position,
                egui::Vec2::splat(Self::CIRCLE_RADIUS),
                tool.targetting_for_section(Ok(self.model.clone().to_element())),
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
        settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<BpmnDomain as Domain>::ToolT>,
        commands: &mut Vec<
            InsensitiveCommand<
                BpmnOrdinalMovement,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::MouseDown(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled;
                }

                self.temporaries.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) => {
                if self.temporaries.dragged_shape.is_some() {
                    self.temporaries.dragged_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos)
                if self.temporaries.highlight.selected
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.buttons_origin(),
                        ehc.ui_scale,
                        pos,
                    )
                    && let Some(nf) = self.model.clone().to_element().as_nonfinal() =>
            {
                let (initial_stage, current_stage, result, event_lock) = f(nf);
                *tool = Some(NaiveBpmnTool {
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
                        self.temporaries.highlight.selected = true;
                    } else {
                        self.temporaries.highlight.selected = !self.temporaries.highlight.selected;
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Drag { delta, .. } if self.temporaries.dragged_shape.is_some() => {
                let translated_real_shape =
                    self.temporaries.dragged_shape.unwrap().translate(delta);
                self.temporaries.dragged_shape = Some(translated_real_shape);
                let coerced_pos = if self.temporaries.highlight.selected {
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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        _diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<
            BpmnOrdinalMovement,
            <BpmnDomain as Domain>::AddCommandElementT,
            <BpmnDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                BpmnOrdinalMovement,
                <BpmnDomain as Domain>::AddCommandElementT,
                <BpmnDomain as Domain>::PropChangeT,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                }
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected = (self.temporaries.highlight.selected
                    && *retain)
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
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.uuid());
                    match property {
                        BpmnPropChange::NameChange(name) => match &self.model {
                            BpmnEvent::StartEvent(inner) => {
                                let mut model = inner.write();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    BpmnPropChange::NameChange(model.name.clone()),
                                ));
                                model.name = name.clone();
                            }
                            BpmnEvent::IntermediateEvent(inner) => {
                                let mut model = inner.write();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    BpmnPropChange::NameChange(model.name.clone()),
                                ));
                                model.name = name.clone();
                            }
                            BpmnEvent::BoundaryIntermediateEvent(inner) => {
                                let mut model = inner.write();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    BpmnPropChange::NameChange(model.name.clone()),
                                ));
                                model.name = name.clone();
                            }
                            BpmnEvent::EndEvent(inner) => {
                                let mut model = inner.write();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    BpmnPropChange::NameChange(model.name.clone()),
                                ));
                                model.name = name.clone();
                            }
                        },
                        BpmnPropChange::EventKindChange(kind) => {
                            // TODO: check validity
                            match &self.model {
                                BpmnEvent::StartEvent(inner) => {
                                    let mut model = inner.write();
                                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                        std::iter::once(*self.uuid).collect(),
                                        BpmnPropChange::EventKindChange(model.kind),
                                    ));
                                    model.kind = *kind;
                                }
                                BpmnEvent::IntermediateEvent(inner) => {
                                    let mut model = inner.write();
                                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                        std::iter::once(*self.uuid).collect(),
                                        BpmnPropChange::EventKindChange(model.kind),
                                    ));
                                    model.kind = *kind;
                                }
                                BpmnEvent::BoundaryIntermediateEvent(inner) => {
                                    let mut model = inner.write();
                                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                        std::iter::once(*self.uuid).collect(),
                                        BpmnPropChange::EventKindChange(model.kind),
                                    ));
                                    model.kind = *kind;
                                }
                                BpmnEvent::EndEvent(inner) => {
                                    let mut model = inner.write();
                                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                        std::iter::once(*self.uuid).collect(),
                                        BpmnPropChange::EventKindChange(model.kind),
                                    ));
                                    model.kind = *kind;
                                }
                            }
                        }
                        BpmnPropChange::StartEventSubkindChange(subkind)
                            if let BpmnEvent::StartEvent(inner) = &self.model =>
                        {
                            let mut model = inner.write();
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::StartEventSubkindChange(model.subkind),
                            ));
                            model.subkind = *subkind;
                        }
                        BpmnPropChange::IntermediateEventSubkindChange(subkind)
                            if let BpmnEvent::IntermediateEvent(inner) = &self.model =>
                        {
                            let mut model = inner.write();
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::IntermediateEventSubkindChange(model.subkind),
                            ));
                            model.subkind = *subkind;
                        }
                        BpmnPropChange::BoundaryIntermediateEventSubkindChange(subkind)
                            if let BpmnEvent::BoundaryIntermediateEvent(inner) = &self.model =>
                        {
                            let mut model = inner.write();
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::BoundaryIntermediateEventSubkindChange(
                                    model.subkind,
                                ),
                            ));
                            model.subkind = *subkind;
                        }
                        _ => {}
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {
        match &self.model {
            BpmnEvent::StartEvent(inner) => {
                let r = inner.read();
                self.temporaries.name_buffer = (*r.name).clone();
                self.temporaries.kind_buffer = r.kind;
                self.temporaries.subkind_buffer = BpmnEventViewSubkindBuffer::Start(r.subkind);
            }
            BpmnEvent::IntermediateEvent(inner) => {
                let r = inner.read();
                self.temporaries.name_buffer = (*r.name).clone();
                self.temporaries.kind_buffer = r.kind;
                self.temporaries.subkind_buffer =
                    BpmnEventViewSubkindBuffer::Intermediate(r.subkind);
            }
            BpmnEvent::BoundaryIntermediateEvent(inner) => {
                let r = inner.read();
                self.temporaries.name_buffer = (*r.name).clone();
                self.temporaries.kind_buffer = r.kind;
                self.temporaries.subkind_buffer =
                    BpmnEventViewSubkindBuffer::BoundaryIntermediate(r.subkind);
            }
            BpmnEvent::EndEvent(inner) => {
                let r = inner.read();
                self.temporaries.name_buffer = (*r.name).clone();
                self.temporaries.kind_buffer = r.kind;
                self.temporaries.subkind_buffer = BpmnEventViewSubkindBuffer::End;
            }
        }
    }
    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<
            ViewUuid,
            (<BpmnDomain as Domain>::CommonElementViewT, ViewUuid),
        >,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <BpmnDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <BpmnDomain as Domain>::CommonElementT>,
    ) {
        let old_model_uuid = *self.model.uuid();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, old_model_uuid)
        };

        let modelish = if let Some(m) = m.get(&old_model_uuid).and_then(|e| e.as_event()) {
            m.clone()
        } else {
            match &self.model {
                BpmnEvent::StartEvent(inner) => {
                    inner.read().deep_copy_clone_inner(model_uuid, m).into()
                }
                BpmnEvent::IntermediateEvent(inner) => {
                    inner.read().deep_copy_clone_inner(model_uuid, m).into()
                }
                BpmnEvent::BoundaryIntermediateEvent(inner) => {
                    inner.read().deep_copy_clone_inner(model_uuid, m).into()
                }
                BpmnEvent::EndEvent(inner) => {
                    inner.read().deep_copy_clone_inner(model_uuid, m).into()
                }
            }
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,

            temporaries: self.temporaries.clone(),
            position: self.position,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_bpmn_gateway(
    kind: BpmnGatewayKind,
    condition: &str,
    position: egui::Pos2,
    request_focus: bool,
) -> (ERef<BpmnGateway>, ERef<BpmnGatewayView>) {
    let model = ERef::new(BpmnGateway::new(
        ModelUuid::now_v7(),
        kind,
        condition.to_owned(),
    ));
    let node_view = new_bpmn_gateway_view(model.clone(), position, request_focus);

    (model, node_view)
}
pub fn new_bpmn_gateway_view(
    model: ERef<BpmnGateway>,
    position: egui::Pos2,
    request_focus: bool,
) -> ERef<BpmnGatewayView> {
    let m = model.read();
    ERef::new(BpmnGatewayView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        request_focus,
        kind_buffer: m.kind,
        condition_buffer: (*m.condition).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::from_selected(request_focus),
        position,
        bounds_radius: egui::Vec2::ZERO,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnGatewayView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<BpmnGateway>,

    #[nh_context_serde(skip_and_default)]
    request_focus: bool,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: BpmnGatewayKind,
    #[nh_context_serde(skip_and_default)]
    condition_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_radius: egui::Vec2,
}

impl BpmnGatewayView {
    const EMPTY_WIDTH: f32 = 60.0;
    const EMPTY_HEIGHT: f32 = 60.0;

    fn buttons_origin(&self) -> egui::Pos2 {
        self.position + egui::Vec2::new(self.bounds_radius.x, -self.bounds_radius.y)
    }
}

impl Entity for BpmnGatewayView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnGatewayView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnGatewayView {
    fn model(&self) -> BpmnElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rhombus {
            position: self.position,
            bounds_radius: self.bounds_radius,
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<BpmnDomain> for BpmnGatewayView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("gateway kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in BpmnGatewayKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            BpmnPropChange::GatewayKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline2(
                "Condition:",
                &mut self.condition_buffer,
                self.request_focus,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.condition_buffer.clone())),
            ));
        }
        self.request_focus = false;

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
        _q: &<BpmnDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveBpmnTool)>,
    ) -> TargettingStatus {
        // Draw shape
        self.bounds_radius = egui::Vec2::new(Self::EMPTY_WIDTH / 2.0, Self::EMPTY_HEIGHT / 2.0);
        canvas.draw_polygon(
            [
                self.position - egui::Vec2::new(0.0, self.bounds_radius.y),
                self.position + egui::Vec2::new(self.bounds_radius.x, 0.0),
                self.position + egui::Vec2::new(0.0, self.bounds_radius.y),
                self.position - egui::Vec2::new(self.bounds_radius.x, 0.0),
            ]
            .to_vec(),
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        const PLUS_SHORTER: f32 = 0.1;
        const PLUS_LONGER: f32 = 0.6;
        const OUTER_CIRCLE: f32 = 0.55;
        const INNER_CIRCLE: f32 = 0.45;
        match self.kind_buffer {
            BpmnGatewayKind::Exclusive | BpmnGatewayKind::Parallel | BpmnGatewayKind::Complex => {
                if matches!(
                    self.kind_buffer,
                    BpmnGatewayKind::Exclusive | BpmnGatewayKind::Complex
                ) {
                    // draw a solid X
                    for e in [1.0, -1.0] {
                        canvas.draw_polygon(
                            [
                                (0.33, e * -0.47),
                                (0.47, e * -0.33),
                                (-0.33, e * 0.47),
                                (-0.47, e * 0.33),
                            ]
                            .map(|(x, y)| {
                                self.position
                                    + (x * self.bounds_radius.x, y * self.bounds_radius.y).into()
                            })
                            .to_vec(),
                            egui::Color32::BLACK,
                            canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                            canvas::Highlight::NONE,
                        );
                    }
                }
                if matches!(
                    self.kind_buffer,
                    BpmnGatewayKind::Parallel | BpmnGatewayKind::Complex
                ) {
                    // draw a solid +
                    for (x, y) in [(PLUS_SHORTER, PLUS_LONGER), (PLUS_LONGER, PLUS_SHORTER)] {
                        canvas.draw_rectangle(
                            egui::Rect::from_center_size(
                                self.position,
                                (x * Self::EMPTY_WIDTH, y * Self::EMPTY_HEIGHT).into(),
                            ),
                            egui::CornerRadius::ZERO,
                            egui::Color32::BLACK,
                            canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                            canvas::Highlight::NONE,
                        );
                    }
                }
            }
            BpmnGatewayKind::Inclusive => {
                // draw a strong circle
                for (r, c) in [
                    (OUTER_CIRCLE, egui::Color32::BLACK),
                    (INNER_CIRCLE, egui::Color32::WHITE),
                ] {
                    canvas.draw_ellipse(
                        self.position,
                        r * self.bounds_radius,
                        c,
                        canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnGatewayKind::EventBased | BpmnGatewayKind::ExclusiveEventBased => {
                let inner_circle = match self.kind_buffer {
                    BpmnGatewayKind::EventBased => Some(INNER_CIRCLE),
                    _ => None,
                };
                for r in std::iter::once(OUTER_CIRCLE).chain(inner_circle) {
                    canvas.draw_ellipse(
                        self.position,
                        r * self.bounds_radius,
                        egui::Color32::TRANSPARENT,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                let pentagon_radius = 0.35 * self.bounds_radius;
                let mut iter = (0..=360)
                    .step_by(360 / 5)
                    .map(|e| ((e + 90) as f32).to_radians())
                    .map(|e| {
                        self.position
                            + (e.cos() * pentagon_radius.x, -e.sin() * pentagon_radius.y).into()
                    })
                    .peekable();
                while let Some(p1) = iter.next()
                    && let Some(p2) = iter.peek()
                {
                    canvas.draw_line(
                        [p1, *p2],
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            BpmnGatewayKind::ParallelEventBased => {
                // draw a hollow + in a simple circle
                let s = canvas::Stroke::new_solid(1.0, egui::Color32::BLACK);
                let h = canvas::Highlight::NONE;
                canvas.draw_ellipse(
                    self.position,
                    OUTER_CIRCLE * self.bounds_radius,
                    egui::Color32::TRANSPARENT,
                    s,
                    h,
                );
                const PLUS_LONGER2: f32 = PLUS_LONGER * 0.75;
                for [p1, p2] in [
                    (PLUS_SHORTER, -PLUS_LONGER2),
                    (PLUS_SHORTER, -PLUS_SHORTER),
                    (PLUS_LONGER2, -PLUS_SHORTER),
                    (PLUS_LONGER2, PLUS_SHORTER),
                    (PLUS_SHORTER, PLUS_SHORTER),
                    (PLUS_SHORTER, PLUS_LONGER2),
                    (-PLUS_SHORTER, PLUS_LONGER2),
                    (-PLUS_SHORTER, PLUS_SHORTER),
                    (-PLUS_LONGER2, PLUS_SHORTER),
                    (-PLUS_LONGER2, -PLUS_SHORTER),
                    (-PLUS_SHORTER, -PLUS_SHORTER),
                    (-PLUS_SHORTER, -PLUS_LONGER2),
                    (PLUS_SHORTER, -PLUS_LONGER2),
                ]
                .array_windows()
                {
                    let ps = [p1, p2]
                        .map(|e| (e.0 * self.bounds_radius.x, e.1 * self.bounds_radius.y))
                        .map(|e| self.position + e.into());
                    canvas.draw_line(ps, s, h);
                }
            }
        }

        if !self.condition_buffer.is_empty() {
            canvas.draw_text(
                self.position
                    + (
                        0.0,
                        (Self::EMPTY_HEIGHT + canvas::CLASS_MIDDLE_FONT_SIZE) / 2.0,
                    )
                        .into(),
                egui::Align2::CENTER_TOP,
                &self.condition_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            draw_nonfinal_node_button_rects(settings, canvas, self.buttons_origin(), ui_scale);
        }

        // Draw targetting ellipse
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
        {
            canvas.draw_polygon(
                [
                    self.position - egui::Vec2::new(0.0, self.bounds_radius.y),
                    self.position + egui::Vec2::new(self.bounds_radius.x, 0.0),
                    self.position + egui::Vec2::new(0.0, self.bounds_radius.y),
                    self.position - egui::Vec2::new(self.bounds_radius.x, 0.0),
                ]
                .to_vec(),
                t.targetting_for_section(Ok(self.model())),
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
        settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveBpmnTool>,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.buttons_origin(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveBpmnTool {
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
        _diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
                        BpmnPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::NameChange(model.condition.clone()),
                            ));
                            model.condition = name.clone();
                        }
                        BpmnPropChange::GatewayKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::GatewayKindChange(model.kind.clone()),
                            ));
                            model.kind = kind.clone();
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
        self.kind_buffer = model.kind;
        self.condition_buffer = (*model.condition).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, BpmnElementView>,
        c: &mut HashMap<ViewUuid, BpmnElementView>,
        m: &mut HashMap<ModelUuid, BpmnElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(BpmnElement::Gateway(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            request_focus: false,
            kind_buffer: self.kind_buffer,
            condition_buffer: self.condition_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_radius: self.bounds_radius,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_bpmn_data(
    kind: BpmnDataKind,
    is_collection: bool,
    name: &str,
    position: egui::Pos2,
    background_color: MGlobalColor,
    request_focus: bool,
) -> (ERef<BpmnData>, ERef<BpmnDataView>) {
    let node_model = ERef::new(BpmnData::new(
        ModelUuid::now_v7(),
        kind,
        is_collection,
        name.to_owned(),
    ));
    let node_view = new_bpmn_data_view(
        node_model.clone(),
        position,
        background_color,
        request_focus,
    );

    (node_model, node_view)
}
pub fn new_bpmn_data_view(
    model: ERef<BpmnData>,
    position: egui::Pos2,
    background_color: MGlobalColor,
    request_focus: bool,
) -> ERef<BpmnDataView> {
    let m = model.read();
    ERef::new(BpmnDataView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        request_focus,
        kind_buffer: m.kind,
        is_collection_buffer: m.is_collection,
        name_buffer: (*m.name).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::from_selected(request_focus),
        position,
        bounds_rect: egui::Rect::ZERO,
        background_color,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnDataView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<BpmnData>,

    #[nh_context_serde(skip_and_default)]
    request_focus: bool,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: BpmnDataKind,
    #[nh_context_serde(skip_and_default)]
    is_collection_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl BpmnDataView {
    const WIDTH: f32 = 60.0;
    const HEIGHT: f32 = 60.0;
}

impl Entity for BpmnDataView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnDataView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnDataView {
    fn model(&self) -> BpmnElement {
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

impl ElementControllerGen2<BpmnDomain> for BpmnDataView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline2("Name:", &mut self.name_buffer, self.request_focus)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }
        self.request_focus = false;

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("data kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in BpmnDataKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            BpmnPropChange::DataKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

        if ui
            .checkbox(&mut self.is_collection_buffer, "is collection")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::DataIsCollectionChange(self.is_collection_buffer),
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

        ui.label("Background color:");
        if let Some(new_color) =
            crate::common::controller::mglobalcolor_edit_button(gdc, ui, &self.background_color)
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<BpmnDomain as Domain>::QueryableT<'_>,
        gdc: &GlobalDrawingContext,
        _settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveBpmnTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        self.bounds_rect =
            egui::Rect::from_center_size(self.position, (Self::WIDTH, Self::HEIGHT).into());

        let background_color = gdc
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
        match self.kind_buffer {
            BpmnDataKind::UnspecifiedObject
            | BpmnDataKind::InputObject
            | BpmnDataKind::OutputObject => {
                const CORNER: f32 = BpmnDataView::WIDTH / 6.0;
                const ICON_SIZE: egui::Vec2 = egui::Vec2::splat(BpmnDataView::WIDTH / 3.0);
                canvas.draw_polygon(
                    [
                        self.bounds_rect.left_top(),
                        self.bounds_rect.right_top() + (-CORNER, 0.0).into(),
                        self.bounds_rect.right_top() + (0.0, CORNER).into(),
                        self.bounds_rect.right_bottom(),
                        self.bounds_rect.left_bottom(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                canvas.draw_polygon(
                    [
                        self.bounds_rect.right_top() + (-CORNER, 0.0).into(),
                        self.bounds_rect.right_top() + (0.0, CORNER).into(),
                        self.bounds_rect.right_top() + (-CORNER, CORNER).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );

                if self.is_collection_buffer {
                    // draw 3 vertical lines
                    let b1 = egui::Pos2::new(
                        self.bounds_rect.center().x - ICON_SIZE.x / 2.0,
                        self.bounds_rect.bottom() - CORNER / 2.0 - ICON_SIZE.y,
                    );
                    let sliver_size = (ICON_SIZE.x / 5.0, ICON_SIZE.y).into();
                    for e in (0..=4)
                        .step_by(2)
                        .map(|e| (e as f32 * ICON_SIZE.x / 5.0, 0.0).into())
                    {
                        canvas.draw_rectangle(
                            egui::Rect::from_min_size(b1 + e, sliver_size),
                            egui::CornerRadius::ZERO,
                            egui::Color32::BLACK,
                            canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                            canvas::Highlight::NONE,
                        );
                    }
                }

                'a: {
                    // draw an arrow
                    let (bg, fg) = match self.kind_buffer {
                        BpmnDataKind::UnspecifiedObject => break 'a,
                        BpmnDataKind::InputObject => (egui::Color32::WHITE, egui::Color32::BLACK),
                        BpmnDataKind::OutputObject => (egui::Color32::BLACK, egui::Color32::WHITE),
                        _ => unreachable!(),
                    };
                    let icon_rect = egui::Rect::from_min_size(
                        self.bounds_rect.min + (CORNER / 2.0, CORNER / 2.0).into(),
                        ICON_SIZE,
                    );
                    canvas.draw_polygon(
                        [
                            icon_rect.center_top(),
                            icon_rect.right_center(),
                            icon_rect.center_bottom(),
                        ]
                        .to_vec(),
                        bg,
                        canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                        canvas::Highlight::NONE,
                    );
                    let tail_height2: f32 = icon_rect.height() / 5.0;
                    let tail_a = icon_rect.left_center() + (0.0, -tail_height2).into();
                    let tail_b = icon_rect.center() + (0.0, -tail_height2).into();
                    let tail_c = icon_rect.center() + (0.0, tail_height2).into();
                    let tail_d = icon_rect.left_center() + (0.0, tail_height2).into();
                    canvas.draw_polygon(
                        [tail_a, tail_b, tail_c, tail_d].to_vec(),
                        bg,
                        canvas::Stroke::new_solid(0.0, egui::Color32::TRANSPARENT),
                        canvas::Highlight::NONE,
                    );
                    for [p1, p2] in [
                        icon_rect.center_top(),
                        icon_rect.right_center(),
                        icon_rect.center_bottom(),
                        tail_c,
                        tail_d,
                        tail_a,
                        tail_b,
                        icon_rect.center_top(),
                    ]
                    .array_windows()
                    {
                        canvas.draw_line(
                            [*p1, *p2],
                            canvas::Stroke::new_solid(1.0, fg),
                            canvas::Highlight::NONE,
                        );
                    }
                }
            }
            BpmnDataKind::DataStore => {
                const ELLIPSE_HEIGHT2: f32 = BpmnDataView::HEIGHT / 8.0;
                const ELLIPSE_RADIUS: egui::Vec2 =
                    egui::Vec2::new(BpmnDataView::WIDTH / 2.0, ELLIPSE_HEIGHT2);
                canvas.draw_ellipse(
                    self.bounds_rect.center_bottom() + (0.0, -ELLIPSE_HEIGHT2).into(),
                    ELLIPSE_RADIUS,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                canvas.draw_ellipse(
                    self.bounds_rect.center_top() + (0.0, 3.0 * ELLIPSE_HEIGHT2).into(),
                    ELLIPSE_RADIUS,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                canvas.draw_ellipse(
                    self.bounds_rect.center_top() + (0.0, 2.0 * ELLIPSE_HEIGHT2).into(),
                    ELLIPSE_RADIUS,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                canvas.draw_ellipse(
                    self.bounds_rect.center_top() + (0.0, ELLIPSE_HEIGHT2).into(),
                    ELLIPSE_RADIUS,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                for x in [self.bounds_rect.left(), self.bounds_rect.right()] {
                    canvas.draw_line(
                        [
                            (x, self.bounds_rect.min.y + ELLIPSE_RADIUS.y).into(),
                            (x, self.bounds_rect.max.y - ELLIPSE_RADIUS.y).into(),
                        ],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        self.highlight,
                    );
                }
            }
        }

        canvas.draw_text(
            self.position + (0.0, (Self::HEIGHT + canvas::CLASS_MIDDLE_FONT_SIZE) / 2.0).into(),
            egui::Align2::CENTER_TOP,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw targetting ellipse
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                t.targetting_for_section(Ok(self.model())),
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
        _settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveBpmnTool>,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
        _diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
                        BpmnPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        BpmnPropChange::DataKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::DataKindChange(model.kind.clone()),
                            ));
                            model.kind = kind.clone();
                        }
                        BpmnPropChange::DataIsCollectionChange(is_collection) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::DataIsCollectionChange(model.is_collection),
                            ));
                            model.is_collection = *is_collection;
                        }
                        BpmnPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::ColorChange(ColorChangeData {
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
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.kind_buffer = model.kind;
        self.is_collection_buffer = model.is_collection;
        self.name_buffer = (*model.name).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, BpmnElementView>,
        c: &mut HashMap<ViewUuid, BpmnElementView>,
        m: &mut HashMap<ModelUuid, BpmnElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(BpmnElement::Data(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            request_focus: false,
            kind_buffer: self.kind_buffer,
            is_collection_buffer: self.is_collection_buffer,
            name_buffer: self.name_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_bpmn_sequenceedge(
    kind: BpmnSequenceEdgeKind,
    condition: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (BpmnNonFinalElement, BpmnElementView),
    target: (BpmnNonInitialElement, BpmnElementView),
) -> (ERef<BpmnSequenceEdge>, ERef<SequenceEdgeViewT>) {
    let link_model = ERef::new(BpmnSequenceEdge::new(
        ModelUuid::now_v7(),
        source.0,
        target.0,
        kind,
        condition.to_owned(),
    ));
    let link_view =
        new_bpmn_sequenceedge_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_bpmn_sequenceedge_view(
    model: ERef<BpmnSequenceEdge>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: BpmnElementView,
    target: BpmnElementView,
) -> ERef<SequenceEdgeViewT> {
    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once((*source.uuid(), target.min_shape())),
        std::iter::once(*target.uuid()),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        BpmnSequenceEdgeAdapter {
            model,
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
pub struct BpmnSequenceEdgeAdapter {
    #[nh_context_serde(entity)]
    model: ERef<BpmnSequenceEdge>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: BpmnSequenceEdgeAdapterTemporaries,
}

#[derive(Clone, Default)]
struct BpmnSequenceEdgeAdapterTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    midpoint_label: Option<Arc<String>>,
    condition_buffer: String,
    kind_buffer: BpmnSequenceEdgeKind,
}

impl MulticonnectionAdapter<BpmnDomain> for BpmnSequenceEdgeAdapter {
    fn model(&self) -> BpmnElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _sources: &Vec<Ending<BpmnElementView>>,
        _targets: &Vec<Ending<BpmnElementView>>,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<BpmnDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<BpmnDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<BpmnDomain as Domain>::ToolT)>,
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
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in BpmnSequenceEdgeKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            BpmnPropChange::SequenceEdgeKindChange(self.temporaries.kind_buffer),
                        ));
                    }
                }
            });
        if ui
            .labeled_text_edit_singleline("Condition:", &mut self.temporaries.condition_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::NameChange(Arc::new(self.temporaries.condition_buffer.clone())),
            ));
        }

        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.separator();

        PropertiesStatus::Shown
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                BpmnPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        BpmnPropChange::NameChange(model.condition.clone()),
                    ));
                    model.condition = name.clone();
                }
                BpmnPropChange::SequenceEdgeKindChange(kind) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        BpmnPropChange::SequenceEdgeKindChange(model.kind),
                    ));
                    model.kind = *kind;
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(
        &mut self,
        _sources: &Vec<Ending<BpmnElementView>>,
        _targets: &Vec<Ending<BpmnElementView>>,
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

        self.temporaries.midpoint_label = match model.condition.is_empty() {
            true => None,
            false => Some(model.condition.clone().into()),
        };
        self.temporaries.condition_buffer = (*model.condition).clone();
        self.temporaries.kind_buffer = model.kind;
    }

    fn deep_copy_init(&self, new_uuid: ModelUuid, m: &mut HashMap<ModelUuid, BpmnElement>) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(BpmnElement::SequenceEdge(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(new_uuid, m)
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, BpmnElement>) {
        self.model.write().deep_copy_relink(m);
    }
}

pub fn new_bpmn_annotation(
    target: (BpmnElement, BpmnElementView),
    text: &str,
    position: egui::Pos2,
) -> (ERef<BpmnAnnotation>, ERef<BpmnAnnotationView>) {
    let comment_model = ERef::new(BpmnAnnotation::new(
        ModelUuid::now_v7(),
        target.0,
        text.to_owned(),
    ));
    let comment_view = new_bpmn_annotation_view(comment_model.clone(), target.1, position);

    (comment_model, comment_view)
}
pub fn new_bpmn_annotation_view(
    model: ERef<BpmnAnnotation>,
    target_view: BpmnElementView,
    position: egui::Pos2,
) -> ERef<BpmnAnnotationView> {
    let m = model.read();
    ERef::new(BpmnAnnotationView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        target_view,

        text_buffer: (*m.text).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color: MGlobalColor::None,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct BpmnAnnotationView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<BpmnAnnotation>,
    #[nh_context_serde(entity)]
    target_view: BpmnElementView,

    #[nh_context_serde(skip_and_default)]
    text_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl BpmnAnnotationView {
    const PADDING_SIZE: f32 = 10.0;
}

impl Entity for BpmnAnnotationView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for BpmnAnnotationView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<BpmnElement> for BpmnAnnotationView {
    fn model(&self) -> BpmnElement {
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

impl ElementControllerGen2<BpmnDomain> for BpmnAnnotationView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        >,
    ) -> PropertiesStatus<BpmnDomain> {
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
                BpmnPropChange::NameChange(Arc::new(self.text_buffer.clone())),
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

        ui.label("Background color:");
        if let Some(new_color) =
            crate::common::controller::mglobalcolor_edit_button(gdc, ui, &self.background_color)
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                BpmnPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<BpmnDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &<BpmnDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveBpmnTool)>,
    ) -> TargettingStatus {
        self.bounds_rect = canvas
            .measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &self.text_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .expand2(egui::Vec2::splat(Self::PADDING_SIZE));
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            self.highlight,
        );

        let self_shape = NHShape::Rect {
            inner: self.bounds_rect,
        };
        let target_shape = self.target_view.min_shape();
        let nice_midpoint = target_shape.nice_midpoint(&self_shape);
        let intersect = self_shape
            .orthogonal_intersect(nice_midpoint)
            .unwrap_or_else(|| self_shape.center_intersect(nice_midpoint));
        let target_intersect = target_shape
            .orthogonal_intersect(nice_midpoint)
            .unwrap_or_else(|| target_shape.center_intersect(nice_midpoint));

        // Draw borders
        if nice_midpoint.x < self.bounds_rect.min.x {
            canvas.draw_line(
                [self.bounds_rect.left_top(), self.bounds_rect.left_bottom()],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );
        } else if self.bounds_rect.max.x < nice_midpoint.x {
            canvas.draw_line(
                [
                    self.bounds_rect.right_top(),
                    self.bounds_rect.right_bottom(),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );
        } else {
            let start_y = if nice_midpoint.y < self.bounds_rect.center().y {
                self.bounds_rect.top()
            } else {
                self.bounds_rect.bottom() - 2.0 * Self::PADDING_SIZE
            };
            for x in [self.bounds_rect.left(), self.bounds_rect.right()] {
                canvas.draw_line(
                    [
                        (x, start_y).into(),
                        (x, start_y + 2.0 * Self::PADDING_SIZE).into(),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
            }
        }
        if nice_midpoint.y < self.bounds_rect.min.y {
            canvas.draw_line(
                [self.bounds_rect.left_top(), self.bounds_rect.right_top()],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );
        } else if self.bounds_rect.max.y < nice_midpoint.y {
            canvas.draw_line(
                [
                    self.bounds_rect.left_bottom(),
                    self.bounds_rect.right_bottom(),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );
        } else {
            let start_x = if nice_midpoint.x < self.bounds_rect.center().x {
                self.bounds_rect.left()
            } else {
                self.bounds_rect.right() - 2.0 * Self::PADDING_SIZE
            };
            for y in [self.bounds_rect.top(), self.bounds_rect.bottom()] {
                canvas.draw_line(
                    [
                        (start_x, y).into(),
                        (start_x + 2.0 * Self::PADDING_SIZE, y).into(),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
            }
        }

        canvas.draw_line(
            [intersect, target_intersect],
            canvas::Stroke::new_dotted(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
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
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::CornerRadius::ZERO,
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
        _settings: &<BpmnDomain as Domain>::SettingsT,
        q: &<BpmnDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveBpmnTool>,
        commands: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
        _diagram_model: &ERef<BpmnDiagram>,
        command: &InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<BpmnOrdinalMovement, BpmnElementOrVertex, BpmnPropChange>,
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
                        BpmnPropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        BpmnPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                BpmnPropChange::ColorChange(ColorChangeData {
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
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.text_buffer = (*model.text).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (BpmnElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        deleting.contains(&self.target_view.uuid())
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, BpmnElementView>,
        c: &mut HashMap<ViewUuid, BpmnElementView>,
        m: &mut HashMap<ModelUuid, BpmnElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(BpmnElement::Annotation(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            target_view: self.target_view.clone(),
            text_buffer: self.text_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, BpmnElementView>,
        m: &HashMap<ModelUuid, BpmnElement>,
    ) {
        self.model.write().deep_copy_relink(m);
        let target_view_uuid = *self.target_view.uuid();
        if let Some(new_target_view) = c.get(&target_view_uuid) {
            self.target_view = new_target_view.clone();
        }
    }
}
