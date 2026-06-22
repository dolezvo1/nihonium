use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerModel, ControllerAdapter, DeleteKind,
    DiagramAdapter, DiagramController, DiagramControllerGen2, DiagramSettings, DiagramSettings2,
    Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus,
    GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider,
    MGlobalColor, Model, MultiDiagramController, PaletteEditBuffer, PositionNoT, ProjectCommand,
    PropertiesStatus, Queryable, SelectionStatus, SnapManager, TargettingStatus, Tool, ToolPalette,
    TryMerge, View,
};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer};
use crate::common::ui_ext::UiExt;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::views::multiconnection_view::{
    self, ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView,
    VertexInformation,
};
use crate::common::views::ordered_views::OrderedViews;
use crate::common::views::package_view::{PackageAdapter, PackageDragType, PackageView};
use crate::domains::umlactivity::umlactivity_models::{
    UmlActivity, UmlActivityActionKind, UmlActivityActionNode, UmlActivityComment,
    UmlActivityCommentLink, UmlActivityDecisionNode, UmlActivityDiagram, UmlActivityEdgeKind,
    UmlActivityElement, UmlActivityFinalNode, UmlActivityFinalNodeKind, UmlActivityFlowEdge,
    UmlActivityForkNode, UmlActivityInitialNode, UmlActivityInterruptibleRegion,
    UmlActivityNonFinalNode, UmlActivityNonInitialNode, UmlActivityObjectNode,
    UmlActivityPartition, UmlActivityPartitionSection,
};
use crate::{
    CustomModal, DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF,
    DiagramConstructorF, DiagramCreationData, DiagramInfo, ShowSettingsF,
};
use eframe::egui;
use std::any::Any;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct UmlActivityDomain;
impl Domain for UmlActivityDomain {
    type SettingsT = UmlActivitySettings;
    type CommonElementT = UmlActivityElement;
    type DiagramModelT = UmlActivityDiagram;
    type CommonElementViewT = UmlActivityElementView;
    type ViewTargettingSectionT = UmlActivityElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveUmlActivityTool;
    type OrdinalMovementT = UmlActivityOrdinalMovement;
    type AddCommandElementT = UmlActivityElementOrVertex;
    type PropChangeT = UmlActivityPropChange;
}

type ActivityViewT = PackageView<UmlActivityDomain, UmlActivityAdapter>;
type InterruptibleRegionViewT =
    PackageView<UmlActivityDomain, UmlActivityInterruptibleRegionAdapter>;
type FlowEdgeViewT = MulticonnectionView<UmlActivityDomain, UmlActivityEdgeAdapter>;
type CommentLinkViewT = MulticonnectionView<UmlActivityDomain, UmlActivityCommentLinkAdapter>;

#[derive(Clone, Copy, Debug)]
pub enum UmlActivityOrdinalMovement {
    Left,
    Right,
}

impl UmlActivityOrdinalMovement {
    pub fn inverse(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Clone)]
pub enum UmlActivityPropChange {
    NameChange(Arc<String>),
    StereotypeChange(Arc<String>),

    ActivityParametersChange(Arc<String>),
    ActionKindChange(UmlActivityActionKind),
    FinalNodeKindChange(UmlActivityFinalNodeKind),

    ForkVerticalChange(bool),
    ForkLengthChange(f32),

    EdgeKindChange(UmlActivityEdgeKind),
    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    CommentAlignChange(Option<egui::Align>, Option<egui::Align>),
}

impl Debug for UmlActivityPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlActivityPropChange::???")
    }
}

impl TryFrom<&UmlActivityPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &UmlActivityPropChange) -> Result<Self, Self::Error> {
        match value {
            UmlActivityPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for UmlActivityPropChange {
    fn from(value: ColorChangeData) -> Self {
        UmlActivityPropChange::ColorChange(value)
    }
}
impl TryFrom<UmlActivityPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: UmlActivityPropChange) -> Result<Self, Self::Error> {
        match value {
            UmlActivityPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for UmlActivityPropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::StereotypeChange(_), newer @ Self::StereotypeChange(_))
            | (Self::ActivityParametersChange(_), newer @ Self::ActivityParametersChange(_))
            | (Self::ForkLengthChange(_), newer @ Self::ForkLengthChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum UmlActivityElementOrVertex {
    Element(UmlActivityElementView),
    Vertex(VertexInformation),
}

impl Debug for UmlActivityElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlActivityElementOrVertex::???")
    }
}

impl TryFrom<UmlActivityElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: UmlActivityElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlActivityElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<UmlActivityElementOrVertex> for UmlActivityElementView {
    type Error = ();

    fn try_from(value: UmlActivityElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlActivityElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "UmlActivityDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlActivityElementView {
    Activity(ERef<ActivityViewT>),
    InterruptibleRegion(ERef<InterruptibleRegionViewT>),
    Partition(ERef<UmlActivityPartitionView>),
    PartitionSection(ERef<UmlActivityPartitionSectionView>),
    ActionNode(ERef<UmlActivityActionNodeView>),
    InitialNode(ERef<UmlActivityInitialNodeView>),
    FinalNode(ERef<UmlActivityFinalNodeView>),
    DecisionNode(ERef<UmlActivityDecisionNodeView>),
    ForkNode(ERef<UmlActivityForkNodeView>),
    ObjectNode(ERef<UmlActivityObjectNodeView>),
    Association(ERef<FlowEdgeViewT>),
    Comment(ERef<UmlActivityCommentView>),
    CommentLink(ERef<CommentLinkViewT>),
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlActivityControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityDiagram>,
}

impl ControllerAdapter<UmlActivityDomain> for UmlActivityControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<UmlActivityDomain, UmlActivityDiagramAdapter>;

    fn model(&self) -> ERef<UmlActivityDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<UmlActivityDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "umlactivity"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::umlactivity_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn insert_element(
        &mut self,
        parent: ModelUuid,
        element: UmlActivityElement,
        b: BucketNoT,
        p: Option<PositionNoT>,
    ) -> Result<(), ()> {
        self.model
            .write()
            .insert_element_into(parent, element, b, p)
    }

    fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, UmlActivityElement, BucketNoT, PositionNoT)>,
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
                UmlActivityDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlActivityDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: UmlActivityDiagramBuffer,
}

#[derive(Clone, Default)]
struct UmlActivityDiagramBuffer {
    name: String,
    comment: String,
}

impl UmlActivityDiagramAdapter {
    pub fn new(model: ERef<UmlActivityDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: UmlActivityDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<UmlActivityDomain> for UmlActivityDiagramAdapter {
    fn model(&self) -> ERef<UmlActivityDiagram> {
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
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        element: UmlActivityElement,
    ) -> Result<UmlActivityElementView, HashSet<ModelUuid>> {
        let v = match element {
            UmlActivityElement::Activity(inner) => new_umlactivity_activity_view(
                inner,
                egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0),
            )
            .into(),
            UmlActivityElement::InterruptibleRegion(inner) => {
                new_umlactivity_interruptibleregion_view(
                    inner,
                    egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0),
                )
                .into()
            }
            UmlActivityElement::Partition(inner) => {
                let r = inner.read();
                let section_views: Result<Vec<_>, _> = r
                    .sections
                    .iter()
                    .map(|e| {
                        self.create_new_view_for(q, e.clone().into())
                            .map(|e| match e {
                                UmlActivityElementView::PartitionSection(inner) => inner,
                                _ => unreachable!(),
                            })
                    })
                    .collect();
                new_umlactivity_partition_view(inner.clone(), section_views?).into()
            }
            UmlActivityElement::PartitionSection(inner) => new_umlactivity_partitionsection_view(
                inner,
                egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0),
            )
            .into(),
            UmlActivityElement::ActionNode(inner) => {
                new_umlactivity_actionnode_view(inner, egui::Pos2::ZERO, MGlobalColor::None).into()
            }
            UmlActivityElement::InitialNode(inner) => {
                new_umlactivity_initialnode_view(inner, egui::Pos2::ZERO).into()
            }
            UmlActivityElement::FinalNode(inner) => {
                new_umlactivity_finalnode_view(inner, egui::Pos2::ZERO).into()
            }
            UmlActivityElement::DecisionNode(inner) => {
                new_umlactivity_decisionnode_view(inner, egui::Pos2::ZERO).into()
            }
            UmlActivityElement::ForkNode(inner) => {
                new_umlactivity_forknode_view(inner, egui::Pos2::ZERO, true, 100.0).into()
            }
            UmlActivityElement::ObjectNode(inner) => {
                new_umlactivity_objectnode_view(inner, egui::Pos2::ZERO, MGlobalColor::None).into()
            }
            UmlActivityElement::Edge(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                new_umlactivity_edge_view(inner.clone(), None, source_view, target_view).into()
            }
            UmlActivityElement::Comment(inner) => {
                new_umlactivity_comment_view(inner, egui::Pos2::ZERO, egui::Align2::CENTER_CENTER)
                    .into()
            }
            UmlActivityElement::CommentLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                new_umlactivity_commentlink_view(inner.clone(), None, source_view, target_view)
                    .into()
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &UmlActivityElement) -> Arc<String> {
        match e {
            UmlActivityElement::Activity(inner) => {
                let r = inner.read();
                let mut s = "Activity (".to_owned();
                s.push_str(&r.name);
                if !r.stereotype.is_empty() {
                    s.push_str("«");
                    s.push_str(&r.stereotype);
                    s.push_str("»");
                }
                s.push_str(")");
                s.into()
            }
            UmlActivityElement::InterruptibleRegion(inner) => {
                let r = inner.read();
                let mut s = "Interruptible Region (".to_owned();
                s.push_str(&r.name);
                if !r.stereotype.is_empty() {
                    s.push_str("«");
                    s.push_str(&r.stereotype);
                    s.push_str("»");
                }
                s.push_str(")");
                s.into()
            }
            UmlActivityElement::Partition(_inner) => "Partition".to_owned().into(),
            UmlActivityElement::PartitionSection(inner) => {
                let r = inner.read();
                let mut s = "Partition Section (".to_owned();
                s.push_str(&r.name);
                if !r.stereotype.is_empty() {
                    s.push_str("«");
                    s.push_str(&r.stereotype);
                    s.push_str("»");
                }
                s.push_str(")");
                s.into()
            }
            UmlActivityElement::ActionNode(inner) => {
                let r = inner.read();
                let mut s = "Action Node (".to_owned();
                s.push_str(&LabelProvider::filter_and_elipsis(&r.name));
                if !r.stereotype.is_empty() {
                    s.push_str("«");
                    s.push_str(&r.stereotype);
                    s.push_str("»");
                }
                s.push_str(")");
                s.into()
            }
            UmlActivityElement::InitialNode(..) => "Initial Node".to_owned().into(),
            UmlActivityElement::FinalNode(inner) => {
                format!("{} Node", inner.read().kind.as_str()).into()
            }
            UmlActivityElement::DecisionNode(inner) => {
                let r = inner.read();
                let s = if r.name.is_empty() {
                    "Decision/Merge Node".to_owned()
                } else {
                    format!(
                        "Decision/Merge Node ({})",
                        LabelProvider::filter_and_elipsis(&r.name)
                    )
                };
                Arc::new(s)
            }
            UmlActivityElement::ForkNode(..) => "Fork/Join Node".to_owned().into(),
            UmlActivityElement::ObjectNode(inner) => {
                let r = inner.read();
                let mut s = "Object Node (".to_owned();
                s.push_str(&LabelProvider::filter_and_elipsis(&r.name));
                if !r.stereotype.is_empty() {
                    s.push_str("«");
                    s.push_str(&r.stereotype);
                    s.push_str("»");
                }
                s.push_str(")");
                s.into()
            }
            UmlActivityElement::Edge(inner) => {
                let r = inner.read();
                let mut s = String::new();
                s.push_str(r.kind.as_str());
                s.push_str(" Edge");
                if !r.name.is_empty() {
                    s.push_str(" (");
                    s.push_str(&r.name);
                    s.push_str(")");
                }
                Arc::new(s)
            }
            UmlActivityElement::Comment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
            UmlActivityElement::CommentLink(_inner) => Arc::new(format!("Comment Link")),
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
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                UmlActivityPropChange::ColorChange((0, new_color).into()),
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
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlActivityPropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlActivityPropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlActivityPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlActivityPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                UmlActivityPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::CommentChange(model.comment.clone()),
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

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, UmlActivityElement>) {
        let (new_model, models) = super::umlactivity_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, UmlActivityElement>) {
        let models = super::umlactivity_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
}

fn new_controlller(
    model: ERef<UmlActivityDiagram>,
    name: String,
    elements: Vec<UmlActivityElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            UmlActivityControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                UmlActivityDiagramAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New UML activity diagram {}", no);
    let diagram = ERef::new(UmlActivityDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (initial, initial_view) = new_umlactivity_initialnode(egui::Pos2::new(200.0, 200.0));
    let (object, object_view) = new_umlactivity_objectnode(
        "Order data",
        "",
        egui::Pos2::new(350.0, 200.0),
        MGlobalColor::None,
    );
    let (decision1, decision1_view) =
        new_umlactivity_decisionnode("", egui::Pos2::new(500.0, 200.0));
    let (ship, ship_view) = new_umlactivity_actionnode(
        "Ship items",
        "",
        UmlActivityActionKind::CallAction,
        egui::Pos2::new(750.0, 200.0),
        MGlobalColor::None,
    );

    let (comment, comment_view) = new_umlactivity_comment(
        "all items available",
        "decisionInput",
        egui::Pos2::new(300.0, 350.0),
        egui::Align2::CENTER_CENTER,
    );
    let (procure, procure_view) = new_umlactivity_actionnode(
        "Procure items",
        "",
        UmlActivityActionKind::CallAction,
        egui::Pos2::new(500.0, 350.0),
        MGlobalColor::None,
    );
    let (r#final, final_view) = new_umlactivity_finalnode(
        UmlActivityFinalNodeKind::ActivityFinal,
        egui::Pos2::new(750.0, 350.0),
    );

    let (decision2, decision2_view) =
        new_umlactivity_decisionnode("", egui::Pos2::new(500.0, 500.0));
    let (signal, signal_view) = new_umlactivity_actionnode(
        "Notify user",
        "",
        UmlActivityActionKind::SendSignalAction,
        egui::Pos2::new(750.0, 500.0),
        MGlobalColor::None,
    );

    let (_e1, e1_view) = new_umlactivity_edge(
        "",
        UmlActivityEdgeKind::Regular,
        None,
        (initial.into(), initial_view.clone().into()),
        (object.clone().into(), object_view.clone().into()),
    );
    let (_e2, e2_view) = new_umlactivity_edge(
        "",
        UmlActivityEdgeKind::Regular,
        None,
        (object.into(), object_view.clone().into()),
        (decision1.clone().into(), decision1_view.clone().into()),
    );
    let (_e3, e3_view) = new_umlactivity_edge(
        "[true]",
        UmlActivityEdgeKind::Regular,
        None,
        (decision1.clone().into(), decision1_view.clone().into()),
        (ship.clone().into(), ship_view.clone().into()),
    );
    let (_cl, cl_view) = new_umlactivity_commentlink(
        None,
        (comment, comment_view.clone().into()),
        (decision1.clone().into(), decision1_view.clone().into()),
    );
    let (_e4, e4_view) = new_umlactivity_edge(
        "[false]",
        UmlActivityEdgeKind::Regular,
        None,
        (decision1.clone().into(), decision1_view.clone().into()),
        (procure.clone().into(), procure_view.clone().into()),
    );
    let (_e5, e5_view) = new_umlactivity_edge(
        "",
        UmlActivityEdgeKind::Regular,
        None,
        (ship.clone().into(), ship_view.clone().into()),
        (r#final.clone().into(), final_view.clone().into()),
    );
    let (_e6, e6_view) = new_umlactivity_edge(
        "",
        UmlActivityEdgeKind::Regular,
        None,
        (procure.clone().into(), procure_view.clone().into()),
        (decision2.clone().into(), decision2_view.clone().into()),
    );
    let (_e7, e7_view) = new_umlactivity_edge(
        "",
        UmlActivityEdgeKind::Regular,
        None,
        (signal.clone().into(), signal_view.clone().into()),
        (r#final.into(), final_view.clone().into()),
    );
    let (_e8, e8_view) = new_umlactivity_edge(
        "[success]",
        UmlActivityEdgeKind::Regular,
        None,
        (decision2.clone().into(), decision2_view.clone().into()),
        (ship.into(), ship_view.clone().into()),
    );
    let (_e9, e9_view) = new_umlactivity_edge(
        "[failure]",
        UmlActivityEdgeKind::Regular,
        None,
        (decision2.into(), decision2_view.clone().into()),
        (signal.into(), signal_view.clone().into()),
    );

    let (activity, activity_view) = new_umlactivity_activity(
        "Order",
        "",
        "",
        egui::Rect::from_x_y_ranges(100.0..=950.0, 100.0..=600.0),
    );
    {
        let mut w = activity_view.write();
        let activity_uuid = *w.uuid();
        let (mut u, mut a) = Default::default();
        for e in [
            initial_view.into(),
            object_view.into(),
            decision1_view.into(),
            ship_view.into(),
            comment_view.into(),
            procure_view.into(),
            final_view.into(),
            decision2_view.into(),
            signal_view.into(),
            e1_view.into(),
            e2_view.into(),
            e3_view.into(),
            cl_view.into(),
            e4_view.into(),
            e5_view.into(),
            e6_view.into(),
            e7_view.into(),
            e8_view.into(),
            e9_view.into(),
        ] {
            w.apply_command(
                &InsensitiveCommand::AddDependency {
                    target: activity_uuid,
                    bucket: 0,
                    position: None,
                    element: UmlActivityElementOrVertex::Element(e),
                    into_model: true,
                },
                &mut u,
                &mut a,
            );
        }
    }

    let name = format!("Demo UML activity diagram {}", no);
    let diagram = ERef::new(UmlActivityDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![activity.into()],
    ));
    new_controlller(diagram, name, vec![activity_view.into()])
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        UmlActivityDomain,
        UmlActivityControllerAdapter,
        DiagramControllerGen2<UmlActivityDomain, UmlActivityDiagramAdapter>,
    >>(&uuid)?)
}

pub struct UmlActivitySettings {
    palette: RwLock<ToolPalette<UmlActivityToolStage, UmlActivityDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<UmlActivityToolStage, UmlActivityElementView>>,
    nonfinal_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            UmlActivityNonFinalNode,
        ) -> (
            UmlActivityToolStage,
            UmlActivityToolStage,
            PartialUmlActivityElement,
            bool,
        ),
    )>,
}

impl DiagramSettings for UmlActivitySettings {
    fn serialize(&self) -> Result<toml::Value, ()> {
        let mut table = toml::Table::new();
        table.insert(
            "palette".to_owned(),
            self.palette.read().unwrap().serialize()?.into(),
        );
        Ok(table.into())
    }
}
impl DiagramSettings2<UmlActivityDomain> for UmlActivitySettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    UmlActivityToolStage,
                    String,
                    UmlActivityElementView,
                )>,
            ),
        ),
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

type NonFinalNodeButtonF = dyn Fn(
    UmlActivityNonFinalNode,
) -> (
    UmlActivityToolStage,
    UmlActivityToolStage,
    PartialUmlActivityElement,
    bool,
);
mod buttons {
    use super::*;
    use std::sync::LazyLock;

    fn nonfinal_edge(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let link_type = LinkType::Edge {
            name: "".to_owned(),
            kind: UmlActivityEdgeKind::Regular,
        };
        (
            UmlActivityToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlActivityToolStage::LinkEnd,
            PartialUmlActivityElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    fn nonfinal_action(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::ActionNode {
            stereotype: "".to_owned(),
            name: "Action".to_owned(),
            kind: UmlActivityActionKind::Basic,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    fn nonfinal_call(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::ActionNode {
            stereotype: "".to_owned(),
            name: "Call Action".to_owned(),
            kind: UmlActivityActionKind::CallAction,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    fn nonfinal_waittime(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::ActionNode {
            stereotype: "".to_owned(),
            name: "Wait Time Action".to_owned(),
            kind: UmlActivityActionKind::WaitTimeAction,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    fn nonfinal_decision(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::DecisionNode {
            name: "".to_owned(),
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    fn nonfinal_object(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::ObjectNode {
            stereotype: "".to_owned(),
            name: "Object".to_owned(),
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    fn nonfinal_flowfinal(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::FinalNode {
            kind: UmlActivityFinalNodeKind::FlowFinal,
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    fn nonfinal_activityfinal(
        m: UmlActivityNonFinalNode,
    ) -> (
        UmlActivityToolStage,
        UmlActivityToolStage,
        PartialUmlActivityElement,
        bool,
    ) {
        let stage = UmlActivityToolStage::FinalNode {
            kind: UmlActivityFinalNodeKind::ActivityFinal,
            with_edge_from: Some(*m.uuid()),
        };
        (stage.clone(), stage, PartialUmlActivityElement::None, true)
    }
    pub const NONFINAL_BUTTONS: LazyLock<
        Vec<(usize, usize, &'static str, &'static NonFinalNodeButtonF)>,
    > = LazyLock::new(|| {
        vec![
            (0, 0, "↘", &nonfinal_edge as &NonFinalNodeButtonF),
            (1, 0, "A", &nonfinal_action as &NonFinalNodeButtonF),
            (1, 1, "C", &nonfinal_call as &NonFinalNodeButtonF),
            (1, 2, "W", &nonfinal_waittime as &NonFinalNodeButtonF),
            (2, 0, "◊", &nonfinal_decision as &NonFinalNodeButtonF),
            (2, 1, "O", &nonfinal_object as &NonFinalNodeButtonF),
            (3, 0, "⊗", &nonfinal_flowfinal as &NonFinalNodeButtonF),
            (
                3,
                1,
                "◎", // Does not work: ⊙⊚⨀⨁⨂◉⯄
                &nonfinal_activityfinal as &NonFinalNodeButtonF,
            ),
        ]
    });
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let palette_items = vec![
        (
            "Action Nodes",
            vec![
                (
                    UmlActivityToolStage::ActionNode {
                        stereotype: "".to_owned(),
                        name: "basic action".to_owned(),
                        kind: UmlActivityActionKind::Basic,
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Basic Action Node",
                ),
                (
                    UmlActivityToolStage::ActionNode {
                        stereotype: "".to_owned(),
                        name: "call action".to_owned(),
                        kind: UmlActivityActionKind::CallAction,
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Call Action Node",
                ),
                (
                    UmlActivityToolStage::ActionNode {
                        stereotype: "".to_owned(),
                        name: "send signal".to_owned(),
                        kind: UmlActivityActionKind::SendSignalAction,
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Send Signal Node",
                ),
                (
                    UmlActivityToolStage::ActionNode {
                        stereotype: "".to_owned(),
                        name: "accept signal".to_owned(),
                        kind: UmlActivityActionKind::AcceptSignalAction,
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Accept Signal Node",
                ),
                (
                    UmlActivityToolStage::ActionNode {
                        stereotype: "".to_owned(),
                        name: "wait time".to_owned(),
                        kind: UmlActivityActionKind::WaitTimeAction,
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Wait Time Node",
                ),
            ],
        ),
        (
            "Other Nodes",
            vec![
                (UmlActivityToolStage::InitialNode {}, "Initial Node"),
                (
                    UmlActivityToolStage::FinalNode {
                        kind: UmlActivityFinalNodeKind::FlowFinal,
                        with_edge_from: None,
                    },
                    "Flow Final Node",
                ),
                (
                    UmlActivityToolStage::FinalNode {
                        kind: UmlActivityFinalNodeKind::ActivityFinal,
                        with_edge_from: None,
                    },
                    "Activity Final Node",
                ),
                (
                    UmlActivityToolStage::DecisionNode {
                        name: "".to_owned(),
                        with_edge_from: None,
                    },
                    "Decision/Merge Node",
                ),
                (UmlActivityToolStage::ForkNodeStart {}, "Fork/Join Node"),
                (
                    UmlActivityToolStage::ObjectNode {
                        stereotype: "".to_owned(),
                        name: "object node".to_owned(),
                        background_color: MGlobalColor::None,
                        with_edge_from: None,
                    },
                    "Object Node",
                ),
            ],
        ),
        (
            "Relationships",
            vec![
                (
                    UmlActivityToolStage::LinkStart {
                        link_type: LinkType::Edge {
                            name: "".to_owned(),
                            kind: UmlActivityEdgeKind::Regular,
                        },
                    },
                    "Regular Edge",
                ),
                (
                    UmlActivityToolStage::LinkStart {
                        link_type: LinkType::Edge {
                            name: "".to_owned(),
                            kind: UmlActivityEdgeKind::Interrupting,
                        },
                    },
                    "Interrupting Edge",
                ),
            ],
        ),
        (
            "Containers",
            vec![
                (
                    UmlActivityToolStage::ActivityStart {
                        name: "activity".to_owned(),
                        stereotype: "".to_owned(),
                        parameters: "".to_owned(),
                    },
                    "Activity",
                ),
                (
                    UmlActivityToolStage::InterruptibleRegionStart {
                        stereotype: "".to_owned(),
                        name: "InterruptibleRegion".to_owned(),
                    },
                    "InterruptibleRegion",
                ),
                (
                    UmlActivityToolStage::PartitionStart {
                        section_stereotype: "".to_owned(),
                        section_name: "Partition Section".to_owned(),
                    },
                    "Partition",
                ),
            ],
        ),
        (
            "Other",
            vec![
                (
                    UmlActivityToolStage::Comment {
                        stereotype: "".to_owned(),
                        text: "a comment".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                    },
                    "Comment",
                ),
                (UmlActivityToolStage::CommentLinkStart {}, "Comment Link"),
            ],
        ),
    ]
    .into_iter()
    .map(|e| {
        (
            e.0,
            e.1.into_iter()
                .map(|e| {
                    let v = view_for_stage(&e.0);
                    (e.0, e.1, v)
                })
                .collect(),
        )
    })
    .collect();

    Box::new(UmlActivitySettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
        nonfinal_buttons: buttons::NONFINAL_BUTTONS.clone(),
    })
}

fn view_for_stage(s: &UmlActivityToolStage) -> UmlActivityElementView {
    match s {
        UmlActivityToolStage::ActionNode {
            stereotype,
            name,
            kind,
            background_color,
            with_edge_from: _,
        } => {
            let view = new_umlactivity_actionnode(
                name,
                stereotype,
                *kind,
                egui::Pos2::ZERO,
                *background_color,
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::InitialNode {} => {
            let view = new_umlactivity_initialnode(egui::Pos2::ZERO).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::FinalNode {
            kind,
            with_edge_from: _,
        } => {
            let view = new_umlactivity_finalnode(*kind, egui::Pos2::ZERO).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::DecisionNode {
            name,
            with_edge_from: _,
        } => {
            let view = new_umlactivity_decisionnode(name, egui::Pos2::ZERO).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::ForkNodeStart => {
            let view = new_umlactivity_forknode(egui::Pos2::ZERO, true, 100.0).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::ObjectNode {
            stereotype,
            name,
            background_color,
            with_edge_from: _,
        } => {
            let view =
                new_umlactivity_objectnode(name, stereotype, egui::Pos2::ZERO, *background_color).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::LinkStart { link_type } => {
            let (d, dv) = new_umlactivity_initialnode(egui::Pos2::ZERO);
            let dummy_1_nonfinal = (d.into(), dv.into());
            let (d, dv) = new_umlactivity_finalnode(
                UmlActivityFinalNodeKind::FlowFinal,
                egui::Pos2::new(200.0, 150.0),
            );
            let dummy_2_noninitial = (d.clone().into(), dv.clone().into());

            match link_type {
                LinkType::Edge { name, kind } => {
                    let view = new_umlactivity_edge(
                        name,
                        *kind,
                        None,
                        dummy_1_nonfinal.clone(),
                        dummy_2_noninitial.clone(),
                    )
                    .1;
                    view.into()
                }
            }
        }
        UmlActivityToolStage::ActivityStart {
            stereotype,
            name,
            parameters,
        } => {
            let view = new_umlactivity_activity(
                name,
                stereotype,
                parameters,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 50.0),
                },
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::InterruptibleRegionStart { stereotype, name } => {
            let view = new_umlactivity_interruptibleregion(
                name,
                stereotype,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(175.0, 75.0),
                },
            )
            .1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::PartitionStart {
            section_stereotype,
            section_name,
        } => {
            let ps = new_umlactivity_partitionsection(
                section_name,
                section_stereotype,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(175.0, 75.0),
                },
            );
            ps.1.write().refresh_buffers();
            let view = new_umlactivity_partition(vec![ps]).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::Comment {
            stereotype,
            text,
            align,
        } => {
            let view = new_umlactivity_comment(text, stereotype, egui::Pos2::ZERO, *align).1;
            view.write().refresh_buffers();
            view.into()
        }
        UmlActivityToolStage::CommentLinkStart => {
            let (comment, comment_view) =
                new_umlactivity_comment("dummy", "", egui::Pos2::ZERO, egui::Align2::CENTER_CENTER);
            let (d, dv) = new_umlactivity_finalnode(
                UmlActivityFinalNodeKind::FlowFinal,
                egui::Pos2::new(200.0, 150.0),
            );
            let dummy_2_element = (d.into(), dv.into());

            let view = new_umlactivity_commentlink(
                None,
                (comment.clone(), comment_view.clone().into()),
                dummy_2_element.clone(),
            )
            .1;
            view.into()
        }
        UmlActivityToolStage::ForkNodeEnd
        | UmlActivityToolStage::LinkEnd
        | UmlActivityToolStage::ActivityEnd
        | UmlActivityToolStage::InterruptibleRegionEnd
        | UmlActivityToolStage::PartitionEnd
        | UmlActivityToolStage::CommentLinkEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(UmlActivitySettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
        nonfinal_buttons: buttons::NONFINAL_BUTTONS.clone(),
    }))
}

pub fn settings_function(
    gdc: &mut GlobalDrawingContext,
    ui: &mut egui::Ui,
    s: &mut Box<dyn DiagramSettings>,
) {
    let Some(s) = (s.as_mut() as &mut dyn Any).downcast_mut::<UmlActivitySettings>() else {
        return;
    };

    let mut w = s.palette.write().unwrap();
    let mut buffer = s.palette_edit_buffer.write().unwrap();

    ui.columns(2, |columns| {
        w.show_treeview(gdc, &mut columns[0]);

        let selected = w.get_selected();
        if selected.uuid() != buffer.uuid() {
            *buffer = w.get_buffer(selected.uuid().cloned());
        }
        match &mut *buffer {
            PaletteEditBuffer::None => {},
            PaletteEditBuffer::Group(_uuid, name) => {
                if columns[1].labeled_text_edit_singleline("Label", name).changed() {
                    w.set_from_buffer(buffer.clone());
                }
            },
            PaletteEditBuffer::Tool(_uuid, name, tool, view) => {
                let mut modified = false;
                modified |= columns[1].labeled_text_edit_singleline("Label", name).changed();

                match tool {
                    UmlActivityToolStage::ActivityStart { stereotype, name, parameters } => {
                        modified |= columns[1].labeled_text_edit_singleline("Stereotype", stereotype).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Parameters", parameters).changed();
                    }
                    UmlActivityToolStage::InterruptibleRegionStart { stereotype, name } => {
                        modified |= columns[1].labeled_text_edit_singleline("Stereotype", stereotype).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();
                    }
                    UmlActivityToolStage::PartitionStart { section_stereotype, section_name } => {
                        modified |= columns[1].labeled_text_edit_singleline("Section stereotype", section_stereotype).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Section name", section_name).changed();
                    }
                    UmlActivityToolStage::ActionNode { stereotype, name, kind, background_color, with_edge_from: _ } => {
                        modified |= columns[1].labeled_text_edit_singleline("Stereotype", stereotype).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();

                        columns[1].label("Kind:");
                        egui::ComboBox::from_id_salt("kind")
                            .selected_text(kind.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in UmlActivityActionKind::VARIANTS {
                                    modified |= ui.selectable_value(kind, e, e.as_str()).changed();
                                }
                            });

                        if let Some(new_color) = crate::common::controller::mglobalcolor_edit_button(
                            gdc,
                            &mut columns[1],
                            background_color,
                        ) {
                            *background_color = new_color;
                            modified = true;
                        }
                    }
                    UmlActivityToolStage::ObjectNode { stereotype, name, background_color, with_edge_from: _ } => {
                        modified |= columns[1].labeled_text_edit_singleline("Stereotype", stereotype).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();

                        if let Some(new_color) = crate::common::controller::mglobalcolor_edit_button(
                            gdc,
                            &mut columns[1],
                            background_color,
                        ) {
                            *background_color = new_color;
                            modified = true;
                        }
                    }
                    UmlActivityToolStage::LinkStart { link_type: LinkType::Edge { name, kind } } => {
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();

                        columns[1].label("Kind:");
                        egui::ComboBox::from_id_salt("kind")
                            .selected_text(kind.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in UmlActivityEdgeKind::VARIANTS {
                                    modified |= ui.selectable_value(kind, e, e.as_str()).changed();
                                }
                            });
                    }
                    UmlActivityToolStage::Comment { stereotype, text, align } => {
                        modified |= columns[1].labeled_text_edit_singleline("Stereotype", stereotype).changed();
                        modified |= columns[1].labeled_text_edit_singleline("Text", text).changed();

                        egui::ComboBox::new("horizontal align", "Horizontal align")
                            .selected_text(format!("{:?}", align.x()))
                            .show_ui(&mut columns[1], |ui| {
                                for e in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                                    modified |= ui.selectable_value(&mut align.0[0], e, format!("{:?}", e)).changed();
                                }
                            });
                        egui::ComboBox::new("vertical align", "Vertical align")
                            .selected_text(format!("{:?}", align.y()))
                            .show_ui(&mut columns[1], |ui| {
                                for e in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                                    modified |= ui.selectable_value(&mut align.0[1], e, format!("{:?}", e)).changed();
                                }
                            });
                    }
                    _ => {},
                }

                if modified {
                    *view = view_for_stage(tool);
                    w.set_from_buffer(buffer.clone());
                }
            },
        }
    });
}

inventory::submit! {DiagramInfo {
    type_indentifier: "umlactivity",
    pretty_name: "UML Activity diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    show_settings_function: &(settings_function as ShowSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "/Unified Modeling Language",
        description: "UML Activity diagram (actions, activites, decisions, etc.)",
        constructors: &[
            ("empty", &(new as DiagramConstructorF)),
            ("demo", &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkType {
    Edge {
        name: String,
        kind: UmlActivityEdgeKind,
    },
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum UmlActivityToolStage {
    ActionNode {
        stereotype: String,
        name: String,
        kind: UmlActivityActionKind,
        background_color: MGlobalColor,
        with_edge_from: Option<ModelUuid>,
    },
    InitialNode {},
    FinalNode {
        kind: UmlActivityFinalNodeKind,
        with_edge_from: Option<ModelUuid>,
    },
    DecisionNode {
        name: String,
        with_edge_from: Option<ModelUuid>,
    },
    ForkNodeStart,
    ForkNodeEnd,
    ObjectNode {
        stereotype: String,
        name: String,
        background_color: MGlobalColor,
        with_edge_from: Option<ModelUuid>,
    },
    LinkStart {
        link_type: LinkType,
    },
    LinkEnd,
    ActivityStart {
        stereotype: String,
        name: String,
        parameters: String,
    },
    ActivityEnd,
    InterruptibleRegionStart {
        stereotype: String,
        name: String,
    },
    InterruptibleRegionEnd,
    PartitionStart {
        section_stereotype: String,
        section_name: String,
    },
    PartitionEnd,
    Comment {
        stereotype: String,
        text: String,
        align: egui::Align2,
    },
    CommentLinkStart,
    CommentLinkEnd,
}

pub enum PartialUmlActivityElement {
    None,
    Some(UmlActivityElementView),
    ForkNode {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    Link {
        link_type: LinkType,
        source: UmlActivityNonFinalNode,
        dest: Option<UmlActivityNonInitialNode>,
    },
    Activity {
        // TODO: are these necessary?
        name: String,
        stereotype: String,
        parameters: String,
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    InterruptibleRegion {
        // TODO: are these necessary?
        name: String,
        stereotype: String,
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    Partition {
        // TODO: are these necessary?
        section_name: String,
        section_stereotype: String,
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    CommentLink {
        source: ERef<UmlActivityComment>,
        dest: Option<UmlActivityElement>,
    },
}

pub struct NaiveUmlActivityTool {
    uuid: uuid::Uuid,
    initial_stage: UmlActivityToolStage,
    current_stage: UmlActivityToolStage,
    result: PartialUmlActivityElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveUmlActivityTool {
    fn try_spend(&mut self) {
        self.result = PartialUmlActivityElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<UmlActivityDomain> for NaiveUmlActivityTool {
    type Stage = UmlActivityToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: UmlActivityToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialUmlActivityElement::None,
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

    fn targetting_for_section(&self, element: Option<UmlActivityElement>) -> egui::Color32 {
        match element {
            None
            | Some(
                UmlActivityElement::Activity(_)
                | UmlActivityElement::InterruptibleRegion(_)
                | UmlActivityElement::Partition(_)
                | UmlActivityElement::PartitionSection(_),
            ) => match self.current_stage {
                UmlActivityToolStage::LinkStart { .. }
                | UmlActivityToolStage::LinkEnd
                | UmlActivityToolStage::CommentLinkStart
                | UmlActivityToolStage::CommentLinkEnd => NON_TARGETTABLE_COLOR,
                _ => TARGETTABLE_COLOR,
            },
            Some(UmlActivityElement::InitialNode(_)) => match self.current_stage {
                UmlActivityToolStage::LinkStart { .. } | UmlActivityToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(UmlActivityElement::FinalNode(_)) => match self.current_stage {
                UmlActivityToolStage::LinkEnd | UmlActivityToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(
                UmlActivityElement::ActionNode(_)
                | UmlActivityElement::DecisionNode(_)
                | UmlActivityElement::ForkNode(_)
                | UmlActivityElement::ObjectNode(_),
            ) => match self.current_stage {
                UmlActivityToolStage::LinkStart { .. }
                | UmlActivityToolStage::LinkEnd
                | UmlActivityToolStage::CommentLinkEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(UmlActivityElement::Comment(_)) => match self.current_stage {
                UmlActivityToolStage::CommentLinkStart => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(UmlActivityElement::Edge(_) | UmlActivityElement::CommentLink(_)) => {
                unreachable!()
            }
        }
    }
    fn draw_status_hint(
        &self,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match (&self.current_stage, &self.result) {
            (_, PartialUmlActivityElement::ForkNode { a, .. }) => {
                let vertical = (pos.y - a.y).abs() > (pos.x - a.x).abs();
                canvas.draw_line(
                    [
                        *a,
                        if vertical {
                            egui::Pos2::new(a.x, pos.y)
                        } else {
                            egui::Pos2::new(pos.x, a.y)
                        },
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            (_, PartialUmlActivityElement::Link { source, .. }) => {
                if let Some(source_view) = q.get_view_for(&source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                UmlActivityToolStage::ActionNode {
                    with_edge_from: Some(source_uuid),
                    ..
                }
                | UmlActivityToolStage::DecisionNode {
                    with_edge_from: Some(source_uuid),
                    ..
                }
                | UmlActivityToolStage::ObjectNode {
                    with_edge_from: Some(source_uuid),
                    ..
                }
                | UmlActivityToolStage::FinalNode {
                    with_edge_from: Some(source_uuid),
                    ..
                },
                _,
            ) => {
                if let Some(source_view) = q.get_view_for(&source_uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                _,
                PartialUmlActivityElement::Activity { a, .. }
                | PartialUmlActivityElement::InterruptibleRegion { a, .. }
                | PartialUmlActivityElement::Partition { a, .. },
            ) => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a, pos),
                    egui::CornerRadius::ZERO,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            (_, PartialUmlActivityElement::CommentLink { source, .. }) => {
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
                UmlActivityToolStage::ActionNode {
                    stereotype,
                    name,
                    kind,
                    background_color,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) =
                    new_umlactivity_actionnode(name, stereotype, *kind, pos, *background_color);
                self.result = PartialUmlActivityElement::Some(view.into());
                self.event_lock = true;
            }
            (UmlActivityToolStage::InitialNode {}, _) => {
                let (_model, view) = new_umlactivity_initialnode(pos);
                self.result = PartialUmlActivityElement::Some(view.into());
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::FinalNode {
                    kind,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) = new_umlactivity_finalnode(*kind, pos);
                self.result = PartialUmlActivityElement::Some(view.into());
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::DecisionNode {
                    name,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) = new_umlactivity_decisionnode(name, pos);
                self.result = PartialUmlActivityElement::Some(view.into());
                self.event_lock = true;
            }
            (UmlActivityToolStage::ForkNodeStart {}, _) => {
                self.result = PartialUmlActivityElement::ForkNode { a: pos, b: None };
                self.current_stage = UmlActivityToolStage::ForkNodeEnd;
                self.event_lock = true;
            }
            (UmlActivityToolStage::ForkNodeEnd, PartialUmlActivityElement::ForkNode { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::ObjectNode {
                    name,
                    stereotype,
                    background_color,
                    with_edge_from: _,
                },
                _,
            ) => {
                let (_model, view) =
                    new_umlactivity_objectnode(name, stereotype, pos, *background_color);
                self.result = PartialUmlActivityElement::Some(view.into());
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::Comment {
                    stereotype,
                    text,
                    align,
                },
                _,
            ) => {
                let (_model, view) = new_umlactivity_comment(text, stereotype, pos, *align);
                self.result = PartialUmlActivityElement::Some(view.into());
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::ActivityStart {
                    name,
                    stereotype,
                    parameters,
                },
                _,
            ) => {
                self.result = PartialUmlActivityElement::Activity {
                    name: name.clone(),
                    stereotype: stereotype.clone(),
                    parameters: parameters.clone(),
                    a: pos,
                    b: None,
                };
                self.current_stage = UmlActivityToolStage::ActivityEnd;
                self.event_lock = true;
            }
            (UmlActivityToolStage::ActivityEnd, PartialUmlActivityElement::Activity { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (UmlActivityToolStage::InterruptibleRegionStart { name, stereotype }, _) => {
                self.result = PartialUmlActivityElement::InterruptibleRegion {
                    name: name.clone(),
                    stereotype: stereotype.clone(),
                    a: pos,
                    b: None,
                };
                self.current_stage = UmlActivityToolStage::InterruptibleRegionEnd;
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::InterruptibleRegionEnd,
                PartialUmlActivityElement::InterruptibleRegion { b, .. },
            ) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::PartitionStart {
                    section_stereotype,
                    section_name,
                },
                _,
            ) => {
                self.result = PartialUmlActivityElement::Partition {
                    section_name: section_name.clone(),
                    section_stereotype: section_stereotype.clone(),
                    a: pos,
                    b: None,
                };
                self.current_stage = UmlActivityToolStage::PartitionEnd;
                self.event_lock = true;
            }
            (
                UmlActivityToolStage::PartitionEnd,
                PartialUmlActivityElement::Partition { b, .. },
            ) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, element: UmlActivityElement) {
        if self.event_lock {
            return;
        }

        match element {
            e @ (UmlActivityElement::ActionNode(..)
            | UmlActivityElement::InitialNode(..)
            | UmlActivityElement::FinalNode(..)
            | UmlActivityElement::DecisionNode(..)
            | UmlActivityElement::ForkNode(..)
            | UmlActivityElement::ObjectNode(..)) => {
                match (&self.current_stage, &mut self.result) {
                    (
                        UmlActivityToolStage::LinkStart { link_type },
                        PartialUmlActivityElement::None,
                    ) if let Some(e) = e.as_nonfinal() => {
                        self.result = PartialUmlActivityElement::Link {
                            link_type: link_type.clone(),
                            source: e,
                            dest: None,
                        };
                        self.current_stage = UmlActivityToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        UmlActivityToolStage::LinkEnd,
                        PartialUmlActivityElement::Link { dest, .. },
                    ) if let Some(e) = e.as_noninitial() => {
                        *dest = Some(e);
                        self.event_lock = true;
                    }
                    (
                        UmlActivityToolStage::CommentLinkEnd,
                        PartialUmlActivityElement::CommentLink { dest, .. },
                    ) => {
                        *dest = Some(e);
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            UmlActivityElement::Comment(inner) => match &self.current_stage {
                UmlActivityToolStage::CommentLinkStart => {
                    self.result = PartialUmlActivityElement::CommentLink {
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = UmlActivityToolStage::CommentLinkEnd;
                    self.event_lock = true;
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlActivityDomain as Domain>::OrdinalMovementT,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &self.result {
            PartialUmlActivityElement::Some(element) => {
                let element = element.clone();
                let additional_edge = match &self.initial_stage {
                    UmlActivityToolStage::ActionNode {
                        with_edge_from: Some(source_uuid),
                        ..
                    }
                    | UmlActivityToolStage::FinalNode {
                        with_edge_from: Some(source_uuid),
                        ..
                    }
                    | UmlActivityToolStage::DecisionNode {
                        with_edge_from: Some(source_uuid),
                        ..
                    }
                    | UmlActivityToolStage::ObjectNode {
                        with_edge_from: Some(source_uuid),
                        ..
                    } if let Some(source) = q.get_view_for(&source_uuid)
                        && let nearest_common_container = q
                            .find_container(&source.uuid(), |uuid, e| {
                                (uuid == preferred_container
                                    || q.is_contained(preferred_container, uuid))
                                    && !matches!(e, UmlActivityElementView::Partition(_))
                            })
                            .map(|e| e.0)
                            .unwrap_or_else(|| q.get_root())
                        && let source_activity = q
                            .find_container(&source.uuid(), |_, e| {
                                matches!(e, UmlActivityElementView::Activity(_))
                            })
                            .map(|e| e.0)
                        && let target_activity = q
                            .find_container_inclusive(&preferred_container, |_, e| {
                                matches!(e, UmlActivityElementView::Activity(_))
                            })
                            .map(|e| e.0)
                        && source_activity == target_activity =>
                    {
                        let edge_view = new_umlactivity_edge(
                            "",
                            UmlActivityEdgeKind::Regular,
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
                    element: UmlActivityElementView::from(element).into(),
                    into_model: true,
                });
                if let Some((parent, e)) = additional_edge {
                    commands.push(InsensitiveCommand::AddDependency {
                        target: parent,
                        bucket: 0,
                        position: None,
                        element: UmlActivityElementView::from(e).into(),
                        into_model: true,
                    });
                }
                Ok(None)
            }
            PartialUmlActivityElement::ForkNode { a, b: Some(b) } => {
                self.current_stage = self.initial_stage.clone();

                let vertical = (b.y - a.y).abs() > (b.x - a.x).abs();
                let center = if vertical {
                    egui::Pos2::new(a.x, (a.y + b.y) / 2.0)
                } else {
                    egui::Pos2::new((a.x + b.x) / 2.0, a.y)
                };
                let length = if vertical { b.y - a.y } else { b.x - a.x }.abs();
                let fork_view = new_umlactivity_forknode(center, vertical, length).1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlActivityElementView::from(fork_view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlActivityElement::Link {
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
                        matches!(e, UmlActivityElementView::Activity(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlActivityElementView::Activity(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view: UmlActivityElementView = match link_type {
                        LinkType::Edge { name, kind } => new_umlactivity_edge(
                            name,
                            *kind,
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
            PartialUmlActivityElement::Activity {
                name,
                stereotype,
                parameters,
                a,
                b: Some(b),
            } => {
                self.current_stage = self.initial_stage.clone();

                let activity_view = new_umlactivity_activity(
                    name,
                    stereotype,
                    parameters,
                    egui::Rect::from_two_pos(*a, *b),
                )
                .1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlActivityElementView::from(activity_view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlActivityElement::InterruptibleRegion {
                name,
                stereotype,
                a,
                b: Some(b),
            } => {
                self.current_stage = self.initial_stage.clone();

                let interruptible_view = new_umlactivity_interruptibleregion(
                    name,
                    stereotype,
                    egui::Rect::from_two_pos(*a, *b),
                )
                .1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlActivityElementView::from(interruptible_view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlActivityElement::Partition {
                section_name,
                section_stereotype,
                a,
                b: Some(b),
            } => {
                self.current_stage = self.initial_stage.clone();

                let r = egui::Rect::from_two_pos(*a, *b);
                let s = new_umlactivity_partitionsection(section_name, section_stereotype, r);
                let partition_view = new_umlactivity_partition(vec![s]).1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlActivityElementView::from(partition_view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlActivityElement::CommentLink {
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
                        matches!(e, UmlActivityElementView::Activity(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlActivityElementView::Activity(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view = new_umlactivity_commentlink(
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
                        element: UmlActivityElementView::from(link_view).into(),
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

pub fn new_umlactivity_activity(
    name: &str,
    stereotype: &str,
    parameters: &str,
    bounds_rect: egui::Rect,
) -> (ERef<UmlActivity>, ERef<ActivityViewT>) {
    let package_model = ERef::new(UmlActivity::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        parameters.to_owned(),
        Vec::new(),
    ));
    let package_view = new_umlactivity_activity_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_umlactivity_activity_view(
    model: ERef<UmlActivity>,
    bounds_rect: egui::Rect,
) -> ERef<ActivityViewT> {
    let m = model.read();
    PackageView::new(
        ViewUuid::now_v7().into(),
        UmlActivityAdapter {
            model: model.clone(),
            background_color: MGlobalColor::None,
            display_text: Arc::new("".to_owned()),
            stereotype_buffer: (*m.stereotype).clone(),
            name_buffer: (*m.name).clone(),
            parameters_buffer: (*m.parameters).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlActivityAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlActivity>,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    display_text: Arc<String>,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    parameters_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<UmlActivityDomain> for UmlActivityAdapter {
    fn model_section(&self) -> UmlActivityElement {
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
    fn insert_element(
        &mut self,
        position: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, ()> {
        self.model
            .write()
            .insert_element(0, position, element)
            .map_err(|_| ())
    }
    fn delete_element(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        self.model.write().remove_element(uuid).map(|e| e.1)
    }

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32 {
        global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE)
    }
    fn draw_label_or_get_text(
        &self,
        bounds_rect: egui::Rect,
        highlight: canvas::Highlight,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlActivityDomain as Domain>::ToolT)>,
    ) -> Result<egui::Rect, Arc<String>> {
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
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::StereotypeChange(Arc::new(self.stereotype_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Parameters:", &mut self.parameters_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::ActivityParametersChange(Arc::new(
                    self.parameters_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlActivityPropChange::StereotypeChange(stereotype) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::StereotypeChange(model.stereotype.clone()),
                    ));
                    model.stereotype = stereotype.clone();
                }
                UmlActivityPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlActivityPropChange::ActivityParametersChange(parameters) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::ActivityParametersChange(model.parameters.clone()),
                    ));
                    model.parameters = parameters.clone();
                }
                UmlActivityPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                UmlActivityPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::ColorChange(ColorChangeData {
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
            let mut acc = "act: ".to_owned();

            if !model.stereotype.is_empty() {
                acc.push_str("«");
                acc.push_str(&model.stereotype);
                acc.push_str("» ");
            }
            acc.push_str(&model.name);
            if !model.parameters.is_empty() {
                acc.push_str("(");
                acc.push_str(&model.parameters);
                acc.push_str(")");
            }

            acc.into()
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
        self.parameters_buffer = (*model.parameters).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlActivityElement::Activity(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(new_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            background_color: self.background_color.clone(),
            display_text: self.display_text.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            parameters_buffer: self.parameters_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlActivityElement>) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()).and_then(|e| e.as_standalone()) {
                *e = new_model;
            }
        }
    }
}

pub fn new_umlactivity_interruptibleregion(
    name: &str,
    stereotype: &str,
    bounds_rect: egui::Rect,
) -> (
    ERef<UmlActivityInterruptibleRegion>,
    ERef<InterruptibleRegionViewT>,
) {
    let package_model = ERef::new(UmlActivityInterruptibleRegion::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        Vec::new(),
    ));
    let package_view = new_umlactivity_interruptibleregion_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_umlactivity_interruptibleregion_view(
    model: ERef<UmlActivityInterruptibleRegion>,
    bounds_rect: egui::Rect,
) -> ERef<InterruptibleRegionViewT> {
    let m = model.read();
    PackageView::new(
        ViewUuid::now_v7().into(),
        UmlActivityInterruptibleRegionAdapter {
            model: model.clone(),
            display_text: Arc::new("".to_owned()),
            stereotype_buffer: (*m.stereotype).clone(),
            name_buffer: (*m.name).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlActivityInterruptibleRegionAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityInterruptibleRegion>,

    #[nh_context_serde(skip_and_default)]
    display_text: Arc<String>,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
}

impl PackageAdapter<UmlActivityDomain> for UmlActivityInterruptibleRegionAdapter {
    fn model_section(&self) -> UmlActivityElement {
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
    fn insert_element(
        &mut self,
        position: Option<PositionNoT>,
        element: UmlActivityElement,
    ) -> Result<PositionNoT, ()> {
        self.model
            .write()
            .insert_element(0, position, element)
            .map_err(|_| ())
    }
    fn delete_element(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        self.model.write().remove_element(uuid).map(|e| e.1)
    }

    fn background_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::TRANSPARENT
    }
    fn border_stroke(&self, _global_colors: &ColorBundle) -> canvas::Stroke {
        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK)
    }
    fn draw_label_or_get_text(
        &self,
        _bounds_rect: egui::Rect,
        _highlight: canvas::Highlight,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlActivityDomain as Domain>::ToolT)>,
    ) -> Result<egui::Rect, Arc<String>> {
        Err(self.display_text.clone())
    }

    fn show_model_properties(
        &mut self,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::StereotypeChange(Arc::new(self.stereotype_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlActivityPropChange::StereotypeChange(stereotype) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::StereotypeChange(model.stereotype.clone()),
                    ));
                    model.stereotype = stereotype.clone();
                }
                UmlActivityPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.display_text = {
            let mut acc = String::new();
            if !model.stereotype.is_empty() {
                acc.push_str("«");
                acc.push_str(&model.stereotype);
                acc.push_str("» ");
            }
            acc.push_str(&model.name);
            acc.into()
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlActivityElement::InterruptibleRegion(m)) = m.get(&old_model.uuid)
        {
            m.clone()
        } else {
            let modelish = old_model.clone_with(new_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            display_text: self.display_text.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlActivityElement>) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()).and_then(|e| e.as_standalone()) {
                *e = new_model;
            }
        }
    }
}

pub fn new_umlactivity_partition(
    sections: Vec<(
        ERef<UmlActivityPartitionSection>,
        ERef<UmlActivityPartitionSectionView>,
    )>,
) -> (ERef<UmlActivityPartition>, ERef<UmlActivityPartitionView>) {
    let (section_models, section_views) = sections.into_iter().collect();
    let package_model = ERef::new(UmlActivityPartition::new(
        ModelUuid::now_v7(),
        section_models,
    ));
    let package_view = new_umlactivity_partition_view(package_model.clone(), section_views);

    (package_model, package_view)
}
pub fn new_umlactivity_partition_view(
    model: ERef<UmlActivityPartition>,
    section_views: Vec<ERef<UmlActivityPartitionSectionView>>,
) -> ERef<UmlActivityPartitionView> {
    ERef::new(UmlActivityPartitionView {
        uuid: ViewUuid::now_v7().into(),
        model,
        section_views,
        bounds_rect: egui::Rect::ZERO,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityPartitionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityPartition>,
    #[nh_context_serde(entity)]
    section_views: Vec<ERef<UmlActivityPartitionSectionView>>,

    bounds_rect: egui::Rect,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlActivityPartitionViewTemporaries,
}

#[derive(Clone, Default)]
struct UmlActivityPartitionViewTemporaries {
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
}

impl Entity for UmlActivityPartitionView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityPartitionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityPartitionView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityPartitionView {
    fn draw_in(
        &mut self,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlActivityDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        let mut child_targetting_drawn = false;
        let mut r = egui::Rect::ZERO;
        for e in self.section_views.iter() {
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
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
        for (idx, e) in self.section_views.iter().enumerate() {
            let mut w = e.write();
            let child = w.show_properties_inner(gdc, q, ui, commands);

            if let Some(add_sibling) = child.1 {
                let idx = match add_sibling {
                    UmlActivityOrdinalMovement::Left => idx,
                    UmlActivityOrdinalMovement::Right => idx + 1,
                };
                let x_range = match add_sibling {
                    UmlActivityOrdinalMovement::Left => {
                        (w.bounds_rect.left() - 200.0)..=w.bounds_rect.left()
                    }
                    UmlActivityOrdinalMovement::Right => {
                        w.bounds_rect.right()..=(w.bounds_rect.right() + 200.0)
                    }
                };
                let sibling = new_umlactivity_partitionsection(
                    "New Partition Section",
                    "",
                    egui::Rect::from_x_y_ranges(x_range, w.bounds_rect.y_range()),
                );
                commands.push(InsensitiveCommand::AddDependency {
                    target: *self.uuid,
                    bucket: 0,
                    position: Some(idx.try_into().unwrap()),
                    element: UmlActivityElementOrVertex::Element(sibling.1.into()),
                    into_model: true,
                });
            }

            if let Some(child) = child.0.to_non_default() {
                return child;
            }
        }

        return PropertiesStatus::NotShown;
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        self.section_views
            .iter()
            .for_each(|v| v.write().collect_allignment(am));
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<UmlActivityDomain as Domain>::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlActivityDomain as Domain>::OrdinalMovementT,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(_) => {
                let k_status = self
                    .section_views
                    .iter()
                    .map(|e| {
                        let mut w = e.write();
                        (
                            *w.uuid,
                            w.handle_event(
                                event,
                                ehc,
                                settings,
                                q,
                                tool,
                                element_setup_modal,
                                commands,
                            ),
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
                    .section_views
                    .iter()
                    .map(|e| {
                        let mut w = e.write();
                        (
                            *w.uuid,
                            w.handle_event(
                                event,
                                ehc,
                                settings,
                                q,
                                tool,
                                element_setup_modal,
                                commands,
                            ),
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
        command: &InsensitiveCommand<
            <UmlActivityDomain as Domain>::OrdinalMovementT,
            <UmlActivityDomain as Domain>::AddCommandElementT,
            <UmlActivityDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <UmlActivityDomain as Domain>::OrdinalMovementT,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.section_views.iter().for_each(|s| {
                    s.write()
                        .apply_command(command, undo_accumulator, affected_models)
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
                                self.section_views.iter().map(|v| *v.read().uuid).collect();
                        }
                        false => self.temporaries.selected_direct_elements.clear(),
                    }
                }
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if h.selected {
                    for k in self
                        .section_views
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
                        .section_views
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
                    .section_views
                    .iter()
                    .all(|e| e.read().temporaries.highlight.selected);
            }
            InsensitiveCommand::MovePositional(uuids, _)
                if !uuids.contains(&self.uuid)
                    && !self
                        .section_views
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
                self.section_views.iter_mut().for_each(|v| {
                    v.write().apply_command(
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(uuids, align, delta) => {
                if self
                    .section_views
                    .iter()
                    .any(|e| uuids.contains(&e.read().uuid))
                {
                    let mut delta_x = egui::Vec2::ZERO;
                    let (mut u, mut v) = Default::default();

                    let sections_iter: Box<
                        dyn Iterator<Item = &ERef<UmlActivityPartitionSectionView>>,
                    > = match align.x() {
                        egui::Align::Min | egui::Align::Center => {
                            Box::new(self.section_views.iter())
                        }
                        egui::Align::Max => Box::new(self.section_views.iter().rev()),
                    };

                    for e in sections_iter {
                        let mut w = e.write();
                        w.apply_command(
                            &InsensitiveCommand::MovePositionalAll(delta_x),
                            &mut u,
                            &mut v,
                        );
                        let mut new_rect = w.bounds_rect;
                        match align.y() {
                            egui::Align::Min => new_rect.max.y += delta.y,
                            egui::Align::Max => new_rect.min.y += delta.y,
                            _ => {}
                        }
                        if uuids.contains(&w.uuid) {
                            match align.x() {
                                egui::Align::Min => new_rect.max.x += delta.x,
                                egui::Align::Max => new_rect.min.x += delta.x,
                                _ => {}
                            }
                            undo_accumulator
                                .push(InsensitiveCommand::ResizeElementTo(*w.uuid, w.bounds_rect));
                            if new_rect.width() >= 40.0 {
                                w.bounds_rect.min.x = new_rect.min.x;
                                w.bounds_rect.max.x = new_rect.max.x;
                                delta_x.x += delta.x;
                            }
                        }
                        if new_rect.height() >= 40.0 {
                            w.bounds_rect.min.y = new_rect.min.y;
                            w.bounds_rect.max.y = new_rect.max.y;
                        }
                    }
                }

                recurse!();
            }
            InsensitiveCommand::ResizeElementTo(uuid, rect) => {
                if let Some((idx, br)) = self
                    .section_views
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
                        let mut w = self.section_views[idx].write();
                        undo_accumulator
                            .push(InsensitiveCommand::ResizeElementTo(*uuid, w.bounds_rect));
                        w.bounds_rect = *rect;
                    }

                    let (mut u, mut v) = Default::default();
                    macro_rules! adjust {
                        ($w:expr, $dx:expr) => {
                            $w.apply_command(
                                &InsensitiveCommand::MovePositionalAll(egui::Vec2::new($dx, 0.0)),
                                &mut u,
                                &mut v,
                            );
                            $w.bounds_rect.set_height(rect.height());
                        };
                    }

                    let delta_left = rect.min.x - br.min.x;
                    for e in self.section_views.iter().take(idx).rev() {
                        adjust!(e.write(), delta_left);
                    }

                    let delta_right = rect.max.x - br.max.x;
                    for e in self.section_views.iter().skip(idx + 1) {
                        adjust!(e.write(), delta_right);
                    }
                }

                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self
                    .section_views
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
                        element: UmlActivityElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }
                let mut delta = egui::Vec2::ZERO;
                let (mut u, mut m) = Default::default();
                let old_sections = std::mem::take(&mut self.section_views);
                for e in old_sections {
                    let mut w = e.write();
                    if uuids.contains(&w.uuid) {
                        delta.x += w.bounds_rect.width();
                    } else {
                        w.apply_command(
                            &InsensitiveCommand::MovePositionalAll(-delta),
                            &mut u,
                            &mut m,
                        );
                        drop(w);
                        self.section_views.push(e);
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
                    let mut w = self.model.write();
                    if *bucket == 0
                        && let Ok(UmlActivityElementView::PartitionSection(view)) =
                            element.clone().try_into()
                    {
                        let mut vw = view.write();
                        if let Some(model_pos) = w
                            .get_element_pos(&vw.model_uuid())
                            .map(|e| e.1)
                            .or_else(|| {
                                if *into_model {
                                    w.insert_element(*bucket, *position, vw.model()).ok()
                                } else {
                                    None
                                }
                            })
                        {
                            let uuid = *vw.uuid;

                            let (old_uuid, old_rect) = self
                                .section_views
                                .first()
                                .map(|e| {
                                    let r = e.read();
                                    (*r.uuid, r.bounds_rect)
                                })
                                .unwrap();
                            if old_rect.height() >= vw.bounds_rect.height() {
                                vw.bounds_rect.set_height(old_rect.height());
                            } else {
                                for e in &self.section_views {
                                    e.write().bounds_rect.set_height(vw.bounds_rect.height());
                                }
                            }
                            let vertical_delta = old_rect.height() - vw.bounds_rect.height();

                            undo_accumulator.extend([
                                InsensitiveCommand::ResizeElementsBy(
                                    std::iter::once(old_uuid).collect(),
                                    egui::Align2::CENTER_TOP,
                                    egui::Vec2::new(0.0, -vertical_delta),
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
                                    egui::Vec2::new(0.0, vertical_delta),
                                ),
                            ]);

                            if *into_model {
                                affected_models.insert(*w.uuid);
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
                                for e in &self.section_views {
                                    let Some((_b, pos)) = w.get_element_pos(&e.read().model_uuid())
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

                            let old_position = if self.section_views.len() == view_pos.into() {
                                self.section_views
                                    .last()
                                    .map(|e| e.read().bounds_rect.right_top())
                            } else {
                                self.section_views
                                    .iter()
                                    .skip(view_pos.into())
                                    .map(|e| e.read().bounds_rect.min)
                                    .next()
                            }
                            .unwrap_or_default();
                            let delta = (vw.bounds_rect.width(), 0.0).into();
                            let (mut u, mut m) = Default::default();
                            for e in self.section_views.iter().skip(view_pos.into()) {
                                e.write().apply_command(
                                    &InsensitiveCommand::MovePositionalAll(delta),
                                    &mut u,
                                    &mut m,
                                );
                            }
                            let delta = old_position - vw.bounds_rect.min;
                            vw.apply_command(
                                &InsensitiveCommand::MovePositionalAll(delta),
                                &mut u,
                                &mut m,
                            );
                            self.section_views
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
                    let mut w = self.model.write();
                    if *bucket == 0
                        && let Some(view) = self
                            .section_views
                            .iter()
                            .find(|v| *v.read().uuid == *element)
                            .cloned()
                    {
                        let model_uuid = *view.read().model_uuid();

                        if let Some((_b, pos)) = w.remove_element(&model_uuid) {
                            undo_accumulator.push(InsensitiveCommand::AddDependency {
                                target: *self.uuid,
                                bucket: *bucket,
                                position: Some(pos),
                                element: UmlActivityElementView::from(view.clone()).into(),
                                into_model: *including_model,
                            });

                            if *including_model {
                                affected_models.insert(*w.uuid);
                            }

                            let mut delta = egui::Vec2::ZERO;
                            let (mut u, mut m) = Default::default();
                            let old_sections = std::mem::take(&mut self.section_views);
                            for e in old_sections {
                                let mut w = e.write();
                                if *w.uuid == *element {
                                    delta.x += w.bounds_rect.width();
                                } else {
                                    w.apply_command(
                                        &InsensitiveCommand::MovePositionalAll(-delta),
                                        &mut u,
                                        &mut m,
                                    );
                                    drop(w);
                                    self.section_views.push(e);
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
                    UmlActivityOrdinalMovement::Left | UmlActivityOrdinalMovement::Right => {
                        let lifelines_iter: Box<
                            dyn Iterator<Item = &mut ERef<UmlActivityPartitionSectionView>>,
                        > = match direction {
                            UmlActivityOrdinalMovement::Left => {
                                Box::new(self.section_views.iter_mut())
                            }
                            UmlActivityOrdinalMovement::Right => {
                                Box::new(self.section_views.iter_mut().rev())
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
                                        UmlActivityOrdinalMovement::Left => (
                                            (-destw.bounds_rect.width(), 0.0).into(),
                                            (srcw.bounds_rect.width(), 0.0).into(),
                                        ),
                                        UmlActivityOrdinalMovement::Right => (
                                            (destw.bounds_rect.width(), 0.0).into(),
                                            (-srcw.bounds_rect.width(), 0.0).into(),
                                        ),
                                    };
                                    let (mut u, mut m) = Default::default();
                                    srcw.apply_command(
                                        &InsensitiveCommand::MovePositionalAll(src_d),
                                        &mut u,
                                        &mut m,
                                    );
                                    destw.apply_command(
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
        flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.section_views.iter().for_each(|s| {
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
        tlc: &mut HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlActivityDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.section_views
                .iter()
                .for_each(|v| v.read().deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlActivityDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(UmlActivityElement::Partition(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut inner = HashMap::new();
        let new_sections = self
            .section_views
            .iter()
            .map(|v| {
                let v = v.read();
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                let Some(UmlActivityElementView::PartitionSection(s)) = c.get(&v.uuid) else {
                    unreachable!()
                };
                s.clone()
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            section_views: new_sections,

            bounds_rect: self.bounds_rect,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlActivityDomain as Domain>::CommonElementT>,
    ) {
        self.section_views
            .iter_mut()
            .for_each(|v| v.write().deep_copy_relink(c, m));

        let mut w = self.model.write();
        for e in w.sections.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlActivityElement::PartitionSection(new_model)) = m.get(&uuid) {
                *e = new_model.clone();
            }
        }
    }
}

pub fn new_umlactivity_partitionsection(
    name: &str,
    stereotype: &str,
    bounds_rect: egui::Rect,
) -> (
    ERef<UmlActivityPartitionSection>,
    ERef<UmlActivityPartitionSectionView>,
) {
    let package_model = ERef::new(UmlActivityPartitionSection::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        Vec::new(),
    ));
    let package_view = new_umlactivity_partitionsection_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_umlactivity_partitionsection_view(
    model: ERef<UmlActivityPartitionSection>,
    bounds_rect: egui::Rect,
) -> ERef<UmlActivityPartitionSectionView> {
    ERef::new(UmlActivityPartitionSectionView {
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
pub struct UmlActivityPartitionSectionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityPartitionSection>,
    #[nh_context_serde(entity)]
    contained_elements: OrderedViews<UmlActivityElementView>,

    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlActivityPartitionSectionViewTemporaries,
}

#[derive(Clone, Default)]
struct UmlActivityPartitionSectionViewTemporaries {
    stereotype_in_guillemets: String,
    stereotype_buffer: String,
    name_buffer: String,

    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
    all_elements: HashMap<ViewUuid, SelectionStatus>,
}

impl UmlActivityPartitionSectionView {
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
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlActivityDomain as Domain>::OrdinalMovementT,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> (
        PropertiesStatus<UmlActivityDomain>,
        Option<UmlActivityOrdinalMovement>,
    ) {
        let mut add_sibling = None::<UmlActivityOrdinalMovement>;

        if let Some(child) = self.contained_elements.event_order_find_mut(|v| {
            v.show_properties(drawing_context, q, ui, commands)
                .to_non_default()
        }) {
            return (child, add_sibling);
        }

        if !self.temporaries.highlight.selected {
            return (PropertiesStatus::NotShown, add_sibling);
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.temporaries.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::StereotypeChange(Arc::new(
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
                UmlActivityPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
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
                UmlActivityPropChange::ColorChange((0, new_color).into()),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Add sibling left").clicked() {
                add_sibling = Some(UmlActivityOrdinalMovement::Left);
            }
            if ui.button("Add sibling right").clicked() {
                add_sibling = Some(UmlActivityOrdinalMovement::Right);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Move left").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    UmlActivityOrdinalMovement::Left,
                ));
            }
            if ui.button("Move right").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    std::iter::once(*self.uuid).collect(),
                    UmlActivityOrdinalMovement::Right,
                ));
            }
        });

        (PropertiesStatus::Shown, add_sibling)
    }
}

impl Entity for UmlActivityPartitionSectionView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityPartitionSectionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityPartitionSectionView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityPartitionSectionView {
    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                <UmlActivityDomain as Domain>::OrdinalMovementT,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
        unreachable!()
    }

    fn draw_in(
        &mut self,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlActivitySettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
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

        let mut center_top_acc = self.bounds_rect.center_top();
        if !self.temporaries.stereotype_in_guillemets.is_empty() {
            canvas.draw_text(
                center_top_acc,
                egui::Align2::CENTER_TOP,
                &self.temporaries.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
            center_top_acc = canvas
                .measure_text(
                    center_top_acc,
                    egui::Align2::CENTER_TOP,
                    &self.temporaries.stereotype_in_guillemets,
                    canvas::CLASS_TOP_FONT_SIZE,
                )
                .center_bottom();
        }
        canvas.draw_text(
            center_top_acc,
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

        macro_rules! draw_children {
            () => {{
                let mut targetting_drawn = false;
                self.contained_elements.draw_order_foreach_mut(|e| {
                    targetting_drawn |=
                        e.draw_in(q, context, settings, canvas, tool) != TargettingStatus::NotDrawn;
                });
                targetting_drawn
            }};
        }

        if draw_children!() {
            return TargettingStatus::Drawn;
        }

        let Some((_, tool)) = tool.filter(|(pos, _)| self.bounds_rect.contains(*pos)) else {
            return TargettingStatus::NotDrawn;
        };

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            tool.targetting_for_section(Some(self.model.clone().into())),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );
        draw_children!();
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
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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

                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands) {
                        if ehc
                            .modifier_settings
                            .alternative_tool_mode
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            *element_setup_modal = esm;
                        }
                    }

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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.contained_elements.event_order_foreach_mut(|v| {
                    v.apply_command(command, undo_accumulator, affected_models)
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
                    let mut w = self.model.write();
                    if *bucket == 0
                        && let Ok(mut view) = UmlActivityElementView::try_from(element.clone())
                        && (!*into_model
                            || w.insert_element(*bucket, *position, view.model()).is_ok())
                    {
                        let uuid = *view.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            element: uuid,
                            including_model: *into_model,
                        });

                        if *into_model {
                            affected_models.insert(*w.uuid);
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
                    let mut w = self.model.write();
                    if *bucket == 0
                        && let Some(view) = self.contained_elements.get(element)
                        && let Some((_b, pos)) = w.remove_element(&view.model_uuid())
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            position: Some(pos),
                            element: view.clone().into(),
                            into_model: *including_model,
                        });

                        if *including_model {
                            affected_models.insert(*w.uuid);
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
                        UmlActivityPropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlActivityPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlActivityPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ColorChange(ColorChangeData {
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

        self.temporaries.stereotype_in_guillemets.clear();
        if !r.stereotype.is_empty() {
            self.temporaries.stereotype_in_guillemets = format!("«{}»", r.stereotype);
        }
        self.temporaries.stereotype_buffer = (*r.stereotype).clone();
        self.temporaries.name_buffer = (*r.name).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
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
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(UmlActivityElement::PartitionSection(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
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
        c: &HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlActivityDomain as Domain>::CommonElementT>,
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
    settings: &<UmlActivityDomain as Domain>::SettingsT,
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
    settings: &<UmlActivityDomain as Domain>::SettingsT,
    origin: egui::Pos2,
    ui_scale: f32,
    click_pos: egui::Pos2,
) -> Option<&NonFinalNodeButtonF> {
    for (row_idx, col_idx, _l, f) in settings.nonfinal_buttons.iter() {
        let r = nonfinal_node_button_rect(origin, ui_scale, *row_idx, *col_idx);
        if r.contains(click_pos) {
            return Some(f);
        }
    }
    None
}

fn new_umlactivity_actionnode(
    name: &str,
    stereotype: &str,
    kind: UmlActivityActionKind,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> (ERef<UmlActivityActionNode>, ERef<UmlActivityActionNodeView>) {
    let instance_model = ERef::new(UmlActivityActionNode::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        kind,
    ));
    let instance_view =
        new_umlactivity_actionnode_view(instance_model.clone(), position, background_color);

    (instance_model, instance_view)
}
fn new_umlactivity_actionnode_view(
    model: ERef<UmlActivityActionNode>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> ERef<UmlActivityActionNodeView> {
    let m = model.read();
    ERef::new(UmlActivityActionNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        stereotype_in_guillemets: String::new(),
        stereotype_buffer: (*m.stereotype).clone(),
        name_buffer: (*m.name).clone(),
        kind_buffer: m.kind,
        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityActionNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityActionNode>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: String,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: UmlActivityActionKind,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl Entity for UmlActivityActionNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityActionNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityActionNodeView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityActionNodeView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
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
                UmlActivityPropChange::StereotypeChange(Arc::new(self.stereotype_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlActivityActionKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlActivityPropChange::ActionKindChange(e),
                        ));
                    }
                }
            });

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
                UmlActivityPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
    ) -> TargettingStatus {
        let background_color = context
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
        let mut stereotype_bottom = egui::Pos2::ZERO;
        if self.kind_buffer != UmlActivityActionKind::WaitTimeAction {
            self.bounds_rect = canvas.measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &self.name_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            );
            stereotype_bottom = self.bounds_rect.center_top();
            if !self.stereotype_in_guillemets.is_empty() {
                self.bounds_rect = self.bounds_rect.union(canvas.measure_text(
                    stereotype_bottom,
                    egui::Align2::CENTER_BOTTOM,
                    &self.stereotype_in_guillemets,
                    canvas::CLASS_TOP_FONT_SIZE,
                ));
            }
            self.bounds_rect = self.bounds_rect.expand(10.0);
        }
        match self.kind_buffer {
            UmlActivityActionKind::Basic | UmlActivityActionKind::CallAction => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::CornerRadius::same(10),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                if self.kind_buffer == UmlActivityActionKind::CallAction {
                    for e in [
                        [(-6.0, 2.0), (-6.0, 10.0)],
                        [(-10.0, 6.0), (-2.0, 6.0)],
                        [(-10.0, 6.0), (-10.0, 10.0)],
                        [(-2.0, 6.0), (-2.0, 10.0)],
                    ]
                    .map(|e| e.map(|e| self.bounds_rect.right_top() + e.into()))
                    {
                        canvas.draw_line(
                            e,
                            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                            canvas::Highlight::NONE,
                        );
                    }
                }
            }
            UmlActivityActionKind::SendSignalAction => {
                canvas.draw_polygon(
                    [
                        self.bounds_rect.left_top(),
                        self.bounds_rect.right_top(),
                        egui::Pos2::new(
                            self.bounds_rect.right() + self.bounds_rect.height() / 2.0,
                            self.bounds_rect.center().y,
                        ),
                        self.bounds_rect.right_bottom(),
                        self.bounds_rect.left_bottom(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                self.bounds_rect.max.x += self.bounds_rect.height() / 2.0;
            }
            UmlActivityActionKind::AcceptSignalAction => {
                let d = egui::Vec2::new(self.bounds_rect.height() / 2.0, 0.0);
                // Draw background
                canvas.draw_polygon(
                    [
                        self.bounds_rect.left_top() - d,
                        self.bounds_rect.right_top(),
                        self.bounds_rect.right_bottom(),
                        self.bounds_rect.left_center(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        self.bounds_rect.right_top(),
                        self.bounds_rect.right_bottom(),
                        self.bounds_rect.left_bottom() - d,
                        self.bounds_rect.left_center(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                // Draw stroke
                for e in [
                    self.bounds_rect.left_top() - d,
                    self.bounds_rect.right_top(),
                    self.bounds_rect.right_bottom(),
                    self.bounds_rect.left_bottom() - d,
                    self.bounds_rect.left_center(),
                    self.bounds_rect.left_top() - d,
                ]
                .array_windows::<2>()
                {
                    canvas.draw_line(
                        *e,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        self.highlight,
                    );
                }
                self.bounds_rect.min.x -= self.bounds_rect.height() / 2.0;
            }
            UmlActivityActionKind::WaitTimeAction => {
                const WIDTH: f32 = 40.0;
                self.bounds_rect =
                    egui::Rect::from_center_size(self.position, egui::Vec2::splat(WIDTH));
                // Upper and lower triangles
                canvas.draw_polygon(
                    [
                        self.position - egui::Vec2::new(WIDTH / 2.0, WIDTH / 2.0),
                        self.position - egui::Vec2::new(-WIDTH / 2.0, WIDTH / 2.0),
                        self.position,
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                canvas.draw_polygon(
                    [
                        self.position,
                        self.position - egui::Vec2::new(-WIDTH / 2.0, -WIDTH / 2.0),
                        self.position - egui::Vec2::new(WIDTH / 2.0, -WIDTH / 2.0),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                let mut bottom = self.bounds_rect.center_bottom();
                if !self.stereotype_in_guillemets.is_empty() {
                    canvas.draw_text(
                        bottom,
                        egui::Align2::CENTER_TOP,
                        &self.stereotype_in_guillemets,
                        canvas::CLASS_TOP_FONT_SIZE,
                        egui::Color32::BLACK,
                    );
                    bottom.y += canvas
                        .measure_text(
                            bottom,
                            egui::Align2::CENTER_TOP,
                            &self.stereotype_in_guillemets,
                            canvas::CLASS_TOP_FONT_SIZE,
                        )
                        .height();
                }
                canvas.draw_text(
                    bottom,
                    egui::Align2::CENTER_TOP,
                    &self.name_buffer,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
        }
        if self.kind_buffer != UmlActivityActionKind::WaitTimeAction {
            if !self.stereotype_in_guillemets.is_empty() {
                canvas.draw_text(
                    stereotype_bottom,
                    egui::Align2::CENTER_BOTTOM,
                    &self.stereotype_in_guillemets,
                    canvas::CLASS_TOP_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
            canvas.draw_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &self.name_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            draw_nonfinal_node_button_rects(
                settings,
                canvas,
                self.bounds_rect.right_top(),
                ui_scale,
            );
        }

        if canvas.ui_scale().is_some() {
            // Draw targetting rectangle
            if let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
            {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::CornerRadius::ZERO,
                    t.targetting_for_section(Some(self.model())),
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
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlActivityTool {
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                        UmlActivityPropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlActivityPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlActivityPropChange::ActionKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ActionKindChange(model.kind),
                            ));
                            model.kind = *kind;
                        }
                        UmlActivityPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ColorChange(ColorChangeData {
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

        self.stereotype_in_guillemets = if model.stereotype.is_empty() {
            String::new()
        } else {
            format!("«{}»", model.stereotype)
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
        self.kind_buffer = model.kind;
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::ActionNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            stereotype_in_guillemets: self.stereotype_in_guillemets.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            kind_buffer: self.kind_buffer,
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

fn new_umlactivity_initialnode(
    position: egui::Pos2,
) -> (
    ERef<UmlActivityInitialNode>,
    ERef<UmlActivityInitialNodeView>,
) {
    let model = ERef::new(UmlActivityInitialNode::new(ModelUuid::now_v7()));
    let view = new_umlactivity_initialnode_view(model.clone(), position);

    (model, view)
}

fn new_umlactivity_initialnode_view(
    model: ERef<UmlActivityInitialNode>,
    position: egui::Pos2,
) -> ERef<UmlActivityInitialNodeView> {
    ERef::new(UmlActivityInitialNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityInitialNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityInitialNode>,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    position: egui::Pos2,
}

impl UmlActivityInitialNodeView {
    const CIRCLE_RADIUS: f32 = 15.0;
    fn buttons_origin(&self) -> egui::Pos2 {
        self.position + egui::Vec2::new(Self::CIRCLE_RADIUS, -Self::CIRCLE_RADIUS)
    }
}

impl Entity for UmlActivityInitialNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityInitialNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityInitialNodeView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityInitialNodeView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
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
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlActivityDomain as Domain>::ToolT)>,
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
            draw_nonfinal_node_button_rects(settings, canvas, self.buttons_origin(), ui_scale);
        }

        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && self.min_shape().contains(*pos)
        {
            canvas.draw_ellipse(
                self.position,
                egui::Vec2::splat(Self::CIRCLE_RADIUS),
                tool.targetting_for_section(Some(self.model.clone().into())),
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
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<UmlActivityDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
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
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.buttons_origin(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlActivityTool {
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            <UmlActivityDomain as Domain>::AddCommandElementT,
            <UmlActivityDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                <UmlActivityDomain as Domain>::AddCommandElementT,
                <UmlActivityDomain as Domain>::PropChangeT,
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
            (<UmlActivityDomain as Domain>::CommonElementViewT, ViewUuid),
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
        tlc: &mut HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlActivityDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlActivityDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::InitialNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
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

pub fn new_umlactivity_finalnode(
    kind: UmlActivityFinalNodeKind,
    position: egui::Pos2,
) -> (ERef<UmlActivityFinalNode>, ERef<UmlActivityFinalNodeView>) {
    let node_model = ERef::new(UmlActivityFinalNode::new(ModelUuid::now_v7(), kind));
    let node_view = new_umlactivity_finalnode_view(node_model.clone(), position);

    (node_model, node_view)
}
pub fn new_umlactivity_finalnode_view(
    model: ERef<UmlActivityFinalNode>,
    position: egui::Pos2,
) -> ERef<UmlActivityFinalNodeView> {
    let m = model.read();
    ERef::new(UmlActivityFinalNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        kind_buffer: m.kind,

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityFinalNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityFinalNode>,

    #[nh_context_serde(skip_and_default)]
    kind_buffer: UmlActivityFinalNodeKind,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
}

impl UmlActivityFinalNodeView {
    const RADIUS_INCREMENT: f32 = 10.0;
}

impl Entity for UmlActivityFinalNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityFinalNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityFinalNodeView {
    fn model(&self) -> UmlActivityElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse {
            position: self.position,
            bounds_radius: egui::Vec2::splat(
                UmlActivityInitialNodeView::CIRCLE_RADIUS + Self::RADIUS_INCREMENT,
            ),
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityFinalNodeView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlActivityFinalNodeKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlActivityPropChange::FinalNodeKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

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
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
    ) -> TargettingStatus {
        let r = UmlActivityInitialNodeView::CIRCLE_RADIUS + Self::RADIUS_INCREMENT;
        let sin45 = 0.70;

        match self.kind_buffer {
            UmlActivityFinalNodeKind::FlowFinal => {
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(r),
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                for e in [1.0, -1.0, -1.0, 1.0, 1.0].array_windows::<4>() {
                    canvas.draw_line(
                        [
                            self.position + egui::Vec2::new(e[0] * r * sin45, e[1] * r * sin45),
                            self.position + egui::Vec2::new(e[2] * r * sin45, e[3] * r * sin45),
                        ],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            UmlActivityFinalNodeKind::ActivityFinal => {
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(r),
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    self.highlight,
                );
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(UmlActivityInitialNodeView::CIRCLE_RADIUS),
                    egui::Color32::BLACK,
                    canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
            }
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
                    t.targetting_for_section(Some(self.model())),
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
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(..) => {}
            InsensitiveCommand::AddDependency { .. } => {}
            InsensitiveCommand::RemoveDependency { .. } => {}
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        UmlActivityPropChange::FinalNodeKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::FinalNodeKindChange(model.kind),
                            ));
                            model.kind = *kind;
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
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::FinalNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            kind_buffer: self.kind_buffer,
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlactivity_decisionnode(
    name: &str,
    position: egui::Pos2,
) -> (
    ERef<UmlActivityDecisionNode>,
    ERef<UmlActivityDecisionNodeView>,
) {
    let node_model = ERef::new(UmlActivityDecisionNode::new(
        ModelUuid::now_v7(),
        name.to_owned(),
    ));
    let node_view = new_umlactivity_decisionnode_view(node_model.clone(), position);

    (node_model, node_view)
}
pub fn new_umlactivity_decisionnode_view(
    model: ERef<UmlActivityDecisionNode>,
    position: egui::Pos2,
) -> ERef<UmlActivityDecisionNodeView> {
    let m = model.read();
    ERef::new(UmlActivityDecisionNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        name_buffer: (*m.name).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_radius: egui::Vec2::ZERO,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityDecisionNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityDecisionNode>,

    #[nh_context_serde(skip_and_default)]
    name_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_radius: egui::Vec2,
}

impl UmlActivityDecisionNodeView {
    const EMPTY_WIDTH: f32 = 45.0;
    const EMPTY_HEIGHT: f32 = 90.0;

    fn buttons_origin(&self) -> egui::Pos2 {
        self.position + egui::Vec2::new(self.bounds_radius.x, -self.bounds_radius.y)
    }
}

impl Entity for UmlActivityDecisionNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityDecisionNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityDecisionNodeView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityDecisionNodeView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        self.bounds_radius = if self.name_buffer.chars().all(char::is_whitespace) {
            egui::Vec2::new(Self::EMPTY_WIDTH / 2.0, Self::EMPTY_HEIGHT / 2.0)
        } else {
            let name_bounds = canvas.measure_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &self.name_buffer,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            );
            name_bounds.size() / 1.5
        };

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

        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

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
                t.targetting_for_section(Some(self.model())),
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
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.buttons_origin(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlActivityTool {
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                        UmlActivityPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
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
        self.name_buffer = (*model.name).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::DecisionNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            name_buffer: self.name_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_radius: self.bounds_radius,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlactivity_forknode(
    position: egui::Pos2,
    vertical: bool,
    longer_side_size: f32,
) -> (ERef<UmlActivityForkNode>, ERef<UmlActivityForkNodeView>) {
    let node_model = ERef::new(UmlActivityForkNode::new(ModelUuid::now_v7()));

    let node_view =
        new_umlactivity_forknode_view(node_model.clone(), position, vertical, longer_side_size);

    (node_model, node_view)
}
pub fn new_umlactivity_forknode_view(
    model: ERef<UmlActivityForkNode>,
    position: egui::Pos2,
    vertical: bool,
    longer_side_size: f32,
) -> ERef<UmlActivityForkNodeView> {
    ERef::new(UmlActivityForkNodeView {
        uuid: ViewUuid::now_v7().into(),
        model,
        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        vertical,
        longer_side_size,
        bounds_rect: egui::Rect::ZERO,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityForkNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityForkNode>,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    vertical: bool,
    longer_side_size: f32,
    pub bounds_rect: egui::Rect,
}

impl UmlActivityForkNodeView {
    const SHORTER_SIDE: f32 = 10.0;
}

impl Entity for UmlActivityForkNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityForkNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityForkNodeView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityForkNodeView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
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

        let mut vertical = self.vertical;
        if ui.checkbox(&mut vertical, "vertical").changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::ForkVerticalChange(vertical),
            ));
        }

        ui.label("Length:");
        let mut length = self.longer_side_size;
        if ui
            .add(egui::DragValue::new(&mut length).speed(1.0))
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::ForkLengthChange(length),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
    ) -> TargettingStatus {
        self.bounds_rect = egui::Rect::from_center_size(
            self.position,
            if self.vertical {
                egui::Vec2::new(Self::SHORTER_SIDE, self.longer_side_size)
            } else {
                egui::Vec2::new(self.longer_side_size, Self::SHORTER_SIDE)
            },
        );

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::BLACK,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            draw_nonfinal_node_button_rects(
                settings,
                canvas,
                self.bounds_rect.right_top(),
                ui_scale,
            );
        }

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
                t.targetting_for_section(Some(self.model())),
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
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlActivityTool {
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                    match property {
                        UmlActivityPropChange::ForkVerticalChange(vertical) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ForkVerticalChange(self.vertical),
                            ));
                            self.vertical = *vertical;
                        }
                        UmlActivityPropChange::ForkLengthChange(length) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ForkLengthChange(self.longer_side_size),
                            ));
                            self.longer_side_size = *length;
                        }
                        _ => {}
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }
    fn refresh_buffers(&mut self) {}

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::ForkNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            vertical: self.vertical,
            longer_side_size: self.longer_side_size,
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlactivity_objectnode(
    name: &str,
    stereotype: &str,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> (ERef<UmlActivityObjectNode>, ERef<UmlActivityObjectNodeView>) {
    let node_model = ERef::new(UmlActivityObjectNode::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
    ));
    let node_view = new_umlactivity_objectnode_view(node_model.clone(), position, background_color);

    (node_model, node_view)
}
pub fn new_umlactivity_objectnode_view(
    model: ERef<UmlActivityObjectNode>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> ERef<UmlActivityObjectNodeView> {
    let m = model.read();
    ERef::new(UmlActivityObjectNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        stereotype_in_guillemets: String::new(),
        stereotype_buffer: (*m.stereotype).clone(),
        name_buffer: (*m.name).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::ZERO,
        background_color,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityObjectNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityObjectNode>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: String,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
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

impl Entity for UmlActivityObjectNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityObjectNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityObjectNodeView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityObjectNodeView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
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
                UmlActivityPropChange::StereotypeChange(Arc::new(self.stereotype_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                UmlActivityPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        self.bounds_rect = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        let stereotype_bottom = self.bounds_rect.center_top();
        if !self.stereotype_in_guillemets.is_empty() {
            self.bounds_rect = self.bounds_rect.union(canvas.measure_text(
                stereotype_bottom,
                egui::Align2::CENTER_BOTTOM,
                &self.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
            ));
        }
        self.bounds_rect = self.bounds_rect.expand(5.0);

        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            context
                .global_colors
                .get(&self.background_color)
                .unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        if !self.stereotype_in_guillemets.is_empty() {
            canvas.draw_text(
                stereotype_bottom,
                egui::Align2::CENTER_BOTTOM,
                &self.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            draw_nonfinal_node_button_rects(
                settings,
                canvas,
                self.bounds_rect.right_top(),
                ui_scale,
            );
        }

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
                t.targetting_for_section(Some(self.model())),
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
        settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                    && let Some(f) = handle_nonfinal_node_button_click(
                        settings,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveUmlActivityTool {
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                        UmlActivityPropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlActivityPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlActivityPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ColorChange(ColorChangeData {
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

        self.stereotype_in_guillemets = if model.stereotype.is_empty() {
            String::new()
        } else {
            format!("«{}»", model.stereotype)
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::ObjectNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            stereotype_in_guillemets: self.stereotype_in_guillemets.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
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

pub fn new_umlactivity_edge(
    name: &str,
    kind: UmlActivityEdgeKind,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (UmlActivityNonFinalNode, UmlActivityElementView),
    target: (UmlActivityNonInitialNode, UmlActivityElementView),
) -> (ERef<UmlActivityFlowEdge>, ERef<FlowEdgeViewT>) {
    let link_model = ERef::new(UmlActivityFlowEdge::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        kind,
        source.0,
        target.0,
    ));
    let link_view = new_umlactivity_edge_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlactivity_edge_view(
    model: ERef<UmlActivityFlowEdge>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlActivityElementView,
    target: UmlActivityElementView,
) -> ERef<FlowEdgeViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once(*m.source.uuid()),
        *m.target.uuid(),
        target.min_shape(),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlActivityEdgeAdapter {
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
pub struct UmlActivityEdgeAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityFlowEdge>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlActivityEdgeTemporaries,
}

#[derive(Clone, Default)]
struct UmlActivityEdgeTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    midpoint_label: Option<Arc<String>>,
    name_buffer: String,
    kind_buffer: UmlActivityEdgeKind,
}

impl MulticonnectionAdapter<UmlActivityDomain> for UmlActivityEdgeAdapter {
    fn model(&self) -> UmlActivityElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlActivityDomain as Domain>::ToolT)>,
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
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlActivityEdgeKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlActivityPropChange::EdgeKindChange(self.temporaries.kind_buffer),
                        ));
                    }
                }
            });

        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.separator();

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlActivityPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlActivityPropChange::EdgeKindChange(kind) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlActivityPropChange::EdgeKindChange(model.kind),
                    ));
                    model.kind = *kind;
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.uuid()),
            ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData {
                line_type: canvas::LineType::Solid,
                arrowhead_type: canvas::ArrowheadType::OpenTriangle,
                multiplicity: match model.kind {
                    UmlActivityEdgeKind::Regular => None,
                    // Doesn't work: ⌁⤷⦮🢱↸↛⇥⇲➠➦➲⦳⧬⧴⭸⭼⮧⮇↯⭍⇝⦚𝥽⟿⥱∅≷⊩⍻⟴⤼⥸⨘⯢➚
                    // Not great: ↘♂⚦†⏭
                    UmlActivityEdgeKind::Interrupting => Some("↪".to_owned().into()),
                },
                role: None,
                reading: None,
            },
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.midpoint_label = match model.name.is_empty() {
            true => None,
            false => Some(model.name.clone().into()),
        };
        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.kind_buffer = model.kind;
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlActivityElement::Edge(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(new_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlActivityElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.uuid();
        if let Some(new_source) = m.get(&source_uuid).and_then(|e| e.as_nonfinal()) {
            model.source = new_source;
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_noninitial()) {
            model.target = new_target;
        }
    }
}

pub fn new_umlactivity_comment(
    text: &str,
    stereotype: &str,
    position: egui::Pos2,
    align: egui::Align2,
) -> (ERef<UmlActivityComment>, ERef<UmlActivityCommentView>) {
    let comment_model = ERef::new(UmlActivityComment::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        text.to_owned(),
    ));
    let comment_view = new_umlactivity_comment_view(comment_model.clone(), position, align);

    (comment_model, comment_view)
}
pub fn new_umlactivity_comment_view(
    model: ERef<UmlActivityComment>,
    position: egui::Pos2,
    align: egui::Align2,
) -> ERef<UmlActivityCommentView> {
    let m = model.read();
    ERef::new(UmlActivityCommentView {
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
        background_color: MGlobalColor::None,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlActivityCommentView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlActivityComment>,

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

impl UmlActivityCommentView {
    const CORNER_SIZE: f32 = 10.0;
}

impl Entity for UmlActivityCommentView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlActivityCommentView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlActivityElement> for UmlActivityCommentView {
    fn model(&self) -> UmlActivityElement {
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

impl ElementControllerGen2<UmlActivityDomain> for UmlActivityCommentView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
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
                UmlActivityPropChange::StereotypeChange(Arc::new(self.stereotype_buffer.clone())),
            ));
        }
        if ui
            .labeled_text_edit_multiline("Text:", &mut self.text_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlActivityPropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
                            UmlActivityPropChange::CommentAlignChange(Some(tmp_x), None),
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
                            UmlActivityPropChange::CommentAlignChange(None, Some(tmp_y)),
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
                UmlActivityPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlActivityTool)>,
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
                    t.targetting_for_section(Some(self.model())),
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
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlActivityTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
        command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
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
                        UmlActivityPropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlActivityPropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        UmlActivityPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlActivityPropChange::CommentAlignChange(x, y) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlActivityPropChange::CommentAlignChange(
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
                s.push_str("«");
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
        _flattened_views: &mut HashMap<ViewUuid, (UmlActivityElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlActivityElementView>,
        c: &mut HashMap<ViewUuid, UmlActivityElementView>,
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlActivityElement::Comment(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
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

pub fn new_umlactivity_commentlink(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlActivityComment>, UmlActivityElementView),
    target: (UmlActivityElement, UmlActivityElementView),
) -> (ERef<UmlActivityCommentLink>, ERef<CommentLinkViewT>) {
    let link_model = ERef::new(UmlActivityCommentLink::new(
        ModelUuid::now_v7(),
        source.0,
        target.0,
    ));
    let link_view =
        new_umlactivity_commentlink_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlactivity_commentlink_view(
    model: ERef<UmlActivityCommentLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlActivityElementView,
    target: UmlActivityElementView,
) -> ERef<CommentLinkViewT> {
    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlActivityCommentLinkAdapter {
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
pub struct UmlActivityCommentLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlActivityCommentLink>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlActivityCommentLinkTemporaries,
}

#[derive(Clone, Default)]
struct UmlActivityCommentLinkTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
}

impl MulticonnectionAdapter<UmlActivityDomain> for UmlActivityCommentLinkAdapter {
    fn model(&self) -> UmlActivityElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlActivityDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlActivityDomain as Domain>::ToolT)>,
    ) -> Result<(), Arc<String>> {
        Ok(())
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
        _q: &<UmlActivityDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlActivityDomain> {
        PropertiesStatus::NotShown
    }
    fn apply_change(
        &self,
        _view_uuid: &ViewUuid,
        _command: &InsensitiveCommand<
            UmlActivityOrdinalMovement,
            UmlActivityElementOrVertex,
            UmlActivityPropChange,
        >,
        _undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlActivityOrdinalMovement,
                UmlActivityElementOrVertex,
                UmlActivityPropChange,
            >,
        >,
    ) {
    }
    fn refresh_buffers(&mut self) {
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
        m: &mut HashMap<ModelUuid, UmlActivityElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlActivityElement::CommentLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(new_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlActivityElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.read().uuid();
        if let Some(UmlActivityElement::Comment(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}
