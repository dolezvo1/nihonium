use super::umlsequence_models::{
    UmlSequenceCombinedFragment, UmlSequenceComment, UmlSequenceCommentLink, UmlSequenceDiagram,
    UmlSequenceElement, UmlSequenceHorizontalElement, UmlSequenceLifeline, UmlSequenceMessage,
};
use crate::common::canvas::{self, Highlight, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerModel, ControllerAdapter, DeleteKind,
    DiagramAdapter, DiagramController, DiagramControllerGen2, DiagramSettings, DiagramSettings2,
    Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus,
    GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider,
    MGlobalColor, Model, MultiDiagramController, PaletteEditBuffer, PositionNoT, ProjectCommand,
    PropertiesStatus, Queryable, SelectionStatus, ShowSettingsResult, SnapManager,
    TargettingStatus, Tool, ToolPalette, TryMerge, View,
};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer};
use crate::common::ui_ext::UiExt;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::views::multiconnection_view::{
    ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView,
    VertexInformation,
};
use crate::common::views::package_view::PackageDragType;
use crate::domains::umlsequence::umlsequence_models::{
    HORIZONTALS_BUCKET, UmlSequenceActivationBehaviour, UmlSequenceCombinedFragmentKind,
    UmlSequenceCombinedFragmentSection, UmlSequenceDiagramBoard, UmlSequenceMessageLifecycleKind,
    UmlSequenceMessageSynchronicityKind, UmlSequenceRef, VERTICALS_BUCKET,
};
use crate::{
    CustomModal, DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF,
    DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::{egui, epaint};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct UmlSequenceDomain;
impl Domain for UmlSequenceDomain {
    type SettingsT = UmlSequenceSettings;
    type CommonElementT = UmlSequenceElement;
    type DiagramModelT = UmlSequenceDiagramBoard;
    type CommonElementViewT = UmlSequenceElementView;
    type ViewTargettingSectionT = UmlSequenceElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveUmlSequenceTool;
    type OrdinalMovementT = UmlSequenceOrdinalMovement;
    type AddCommandElementT = UmlSequenceElementOrVertex;
    type PropChangeT = UmlSequencePropChange;
}

type CommentLinkViewT = MulticonnectionView<UmlSequenceDomain, UmlSequenceCommentLinkAdapter>;

#[derive(Clone, Copy, Debug)]
pub enum UmlSequenceOrdinalMovement {
    LifelineLeft,
    LifelineRight,
    HorizontalUp,
    HorizontalDown,
}

impl UmlSequenceOrdinalMovement {
    fn inverse(&self) -> Self {
        match self {
            Self::LifelineLeft => Self::LifelineRight,
            Self::LifelineRight => Self::LifelineLeft,
            Self::HorizontalUp => Self::HorizontalDown,
            Self::HorizontalDown => Self::HorizontalUp,
        }
    }
}

#[derive(Clone)]
pub enum UmlSequencePropChange {
    NameChange(Arc<String>),
    StereotypeChange(Arc<String>),

    ShowActivationsChange(bool),

    SynchronicityKindChange(UmlSequenceMessageSynchronicityKind),
    LifecycleKindChange(UmlSequenceMessageLifecycleKind),
    IsReturnChange(bool),
    DurationChange(f32),
    StateInvariantChange(Arc<String>),
    FlipMulticonnection(FlipMulticonnection),

    CombinedFragmentKindChange(UmlSequenceCombinedFragmentKind),
    CombinedFragmentKindArgumentChange(Arc<String>),
    CombinedFragmentSectionGuardChange(Arc<String>),
    ActivationsBehaviourChange(UmlSequenceActivationBehaviour),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    CommentAlignChange(Option<egui::Align>, Option<egui::Align>),
}

impl Debug for UmlSequencePropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlSequencePropChange::???")
    }
}

impl TryFrom<&UmlSequencePropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &UmlSequencePropChange) -> Result<Self, Self::Error> {
        match value {
            UmlSequencePropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for UmlSequencePropChange {
    fn from(value: ColorChangeData) -> Self {
        UmlSequencePropChange::ColorChange(value)
    }
}
impl TryFrom<UmlSequencePropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: UmlSequencePropChange) -> Result<Self, Self::Error> {
        match value {
            UmlSequencePropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for UmlSequencePropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::StereotypeChange(_), newer @ Self::StereotypeChange(_))
            | (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::DurationChange(_), newer @ Self::DurationChange(_))
            | (Self::StateInvariantChange(_), newer @ Self::StateInvariantChange(_))
            | (
                Self::CombinedFragmentKindArgumentChange(_),
                newer @ Self::CombinedFragmentKindArgumentChange(_),
            )
            | (
                Self::CombinedFragmentSectionGuardChange(_),
                newer @ Self::CombinedFragmentSectionGuardChange(_),
            )
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum UmlSequenceElementOrVertex {
    Element(UmlSequenceElementView),
    Vertex(VertexInformation),
}

impl Debug for UmlSequenceElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassElementOrVertex::???")
    }
}

impl TryFrom<UmlSequenceElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: UmlSequenceElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlSequenceElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<UmlSequenceElementOrVertex> for UmlSequenceElementView {
    type Error = ();

    fn try_from(value: UmlSequenceElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlSequenceElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "UmlSequenceDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlSequenceElementView {
    Diagram(ERef<UmlSequenceDiagramView>),
    CombinedFragment(ERef<UmlSequenceCombinedFragmentView>),
    CombinedFragmentSection(ERef<UmlSequenceCombinedFragmentSectionView>),
    Lifeline(ERef<UmlSequenceLifelineView>),
    Message(ERef<UmlSequenceMessageView>),
    Ref(ERef<UmlSequenceRefView>),
    Comment(ERef<UmlSequenceCommentView>),
    CommentLink(ERef<CommentLinkViewT>),
}

impl UmlSequenceElementView {
    fn as_horizontal(self) -> Option<UmlSequenceHorizontalElementView> {
        match self {
            UmlSequenceElementView::CombinedFragment(inner) => Some(inner.into()),
            UmlSequenceElementView::Message(inner) => Some(inner.into()),
            UmlSequenceElementView::Ref(inner) => Some(inner.into()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "UmlSequenceDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlSequenceHorizontalElementView {
    CombinedFragment(ERef<UmlSequenceCombinedFragmentView>),
    Message(ERef<UmlSequenceMessageView>),
    Ref(ERef<UmlSequenceRefView>),
}

impl UmlSequenceHorizontalElementView {
    fn as_element_view(self) -> UmlSequenceElementView {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => inner.into(),
            UmlSequenceHorizontalElementView::Message(inner) => inner.into(),
            UmlSequenceHorizontalElementView::Ref(inner) => inner.into(),
        }
    }

    fn count_activations(&self, ac: &mut ActivationsCounter) {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => {
                ac.add_combined_fragment(inner);
            }
            UmlSequenceHorizontalElementView::Message(inner) => {
                ac.add_message(inner);
            }
            UmlSequenceHorizontalElementView::Ref(_inner) => {}
        }
    }

    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        message_offsets: &HashMap<ViewUuid, (usize, usize)>,
        pos_y: f32,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => inner.write().draw_inner(
                lifeline_views,
                message_offsets,
                pos_y,
                q,
                context,
                settings,
                canvas,
                tool,
            ),
            UmlSequenceHorizontalElementView::Message(inner) => {
                inner
                    .write()
                    .draw_inner(message_offsets, pos_y, q, context, settings, canvas, tool)
            }
            UmlSequenceHorizontalElementView::Ref(inner) => {
                inner
                    .write()
                    .draw_inner(lifeline_views, pos_y, q, context, settings, canvas, tool)
            }
        }
    }
    fn handle_event_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => {
                inner.write().handle_event_inner(
                    lifeline_views,
                    event,
                    ehc,
                    settings,
                    q,
                    tool,
                    element_setup_modal,
                    commands,
                )
            }
            UmlSequenceHorizontalElementView::Message(inner) => inner.write().handle_event(
                event,
                ehc,
                settings,
                q,
                tool,
                element_setup_modal,
                commands,
            ),
            UmlSequenceHorizontalElementView::Ref(inner) => inner.write().handle_event_inner(
                lifeline_views,
                event,
                ehc,
                q,
                tool,
                element_setup_modal,
                commands,
            ),
        }
    }
}

#[derive(Default)]
struct ActivationsCounter {
    current_counts: HashMap<ViewUuid, usize>,
    message_offsets: HashMap<ViewUuid, (usize, usize)>,
    last_message_y: f32,
    open_activations: HashMap<ViewUuid, Vec<(usize, f32, MGlobalColor)>>,
    closed_activations: Vec<(ViewUuid, usize, f32, f32, MGlobalColor)>,
}

impl ActivationsCounter {
    pub const ACTIVATION_WIDTH: f32 = 10.0;
    pub const ACTIVATION_OFFSET: f32 = 7.0;

    fn add_combined_fragment(&mut self, combined_fragment: &ERef<UmlSequenceCombinedFragmentView>) {
        let r = combined_fragment.read();

        // Clone counts, close currently opened activations
        let previous_counts = self.current_counts.clone();
        let outside: HashMap<ViewUuid, Vec<_>> = self
            .open_activations
            .iter()
            .filter(|e| !r.temporaries.spanned_lifelines.contains(e.0))
            .map(|e| (*e.0, e.1.clone()))
            .collect();
        let mut inner_previous_open = HashMap::new();
        for (v, o) in self
            .open_activations
            .iter_mut()
            .filter(|e| r.temporaries.spanned_lifelines.contains(e.0))
        {
            let previous = std::mem::take(o);
            for e in previous.iter() {
                self.closed_activations
                    .push((*v, e.0, e.1, r.bounds_rect.min.y, e.2));
            }
            inner_previous_open.insert(*v, previous);
        }

        // Call count_activations on child elements
        let mut first_variant = None;
        for e in r.sections.iter() {
            let r2 = e.read();

            self.current_counts = previous_counts.clone();
            self.open_activations = outside
                .iter()
                .map(|e| (*e.0, e.1.clone()))
                .chain(inner_previous_open.iter().map(|e| {
                    (
                        *e.0,
                        e.1.iter()
                            .map(|e| (e.0, r2.bounds_rect.min.y, e.2))
                            .collect(),
                    )
                }))
                .collect();

            for e in r2.horizontal_element_views.iter() {
                e.count_activations(self);
            }

            if r.temporaries.end_behaviour_buffer
                == UmlSequenceActivationBehaviour::ContinueFirstVariant
                && first_variant.is_none()
            {
                first_variant = Some((self.current_counts.clone(), self.open_activations.clone()));
            }

            for (v, o) in self
                .open_activations
                .iter_mut()
                .filter(|e| r.temporaries.spanned_lifelines.contains(e.0))
            {
                for e in o.drain(..) {
                    self.closed_activations
                        .push((*v, e.0, e.1, r2.bounds_rect.max.y, e.2));
                }
            }
        }

        // Set final state based on selected end_behaviour
        match r.temporaries.end_behaviour_buffer {
            UmlSequenceActivationBehaviour::ContinueFirstVariant => {
                let (c, o) = first_variant.unwrap();
                self.current_counts = c;
                self.open_activations = o
                    .into_iter()
                    .map(|e| {
                        if r.temporaries.spanned_lifelines.contains(&e.0) {
                            (
                                e.0,
                                e.1.iter()
                                    .map(|e| (e.0, r.bounds_rect.max.y, e.2))
                                    .collect(),
                            )
                        } else {
                            e
                        }
                    })
                    .collect();
            }
            UmlSequenceActivationBehaviour::ResetToInitialState => {
                self.current_counts = previous_counts;
                self.open_activations = outside
                    .iter()
                    .map(|e| (*e.0, e.1.clone()))
                    .chain(inner_previous_open.iter().map(|e| {
                        (
                            *e.0,
                            e.1.iter()
                                .map(|e| (e.0, r.bounds_rect.max.y, e.2))
                                .collect(),
                        )
                    }))
                    .collect();
            }
            UmlSequenceActivationBehaviour::TerminateActivations => {
                self.current_counts = previous_counts
                    .into_iter()
                    .filter(|e| !r.temporaries.spanned_lifelines.contains(&e.0))
                    .collect();
                self.open_activations = outside;
            }
        }
    }

    fn add_message(&mut self, message: &ERef<UmlSequenceMessageView>) {
        let r = message.read();
        let source_uuid = *r.source.uuid();
        let target_uuid = *r.target.uuid();

        self.message_offsets.insert(
            *r.uuid,
            (
                self.current_counts
                    .get(&source_uuid)
                    .map(|e| e.saturating_sub(1))
                    .unwrap_or(0),
                self.current_counts
                    .get(&target_uuid)
                    .map(|e| {
                        if r.temporaries.is_return_buffer {
                            e.saturating_sub(1)
                        } else {
                            *e
                        }
                    })
                    .unwrap_or(0),
            ),
        );

        if !r.temporaries.is_return_buffer {
            if self
                .current_counts
                .get(&source_uuid)
                .is_none_or(|e| *e == 0)
            {
                self.current_counts.entry(source_uuid).or_insert(1);
                self.open_activations.entry(source_uuid).or_default().push((
                    0,
                    r.temporaries.source_y,
                    r.found_activation_color,
                ));
            }
            let target_count = self.current_counts.entry(target_uuid).or_default();
            self.open_activations.entry(target_uuid).or_default().push((
                *target_count,
                r.temporaries.target_y,
                r.new_activation_color,
            ));
            *target_count += 1;
        } else {
            let source_count = self.current_counts.entry(source_uuid).or_default();
            *source_count = source_count.saturating_sub(1);
            if let Some(a) = self.open_activations.entry(source_uuid).or_default().pop() {
                self.closed_activations
                    .push((source_uuid, a.0, a.1, r.temporaries.source_y, a.2));
            }
        }
        self.last_message_y = r.temporaries.source_y.max(r.temporaries.target_y);
    }

    fn finish(
        mut self,
    ) -> (
        HashMap<ViewUuid, (usize, usize)>,
        impl Iterator<Item = (ViewUuid, usize, f32, f32, MGlobalColor)>,
    ) {
        for open in self.open_activations.iter_mut() {
            self.closed_activations.extend(
                open.1
                    .drain(..)
                    .map(|e| (*open.0, e.0, e.1, self.last_message_y, e.2)),
            );
        }

        self.closed_activations.sort_by_key(|e| e.1);

        (
            self.message_offsets,
            self.closed_activations
                .into_iter()
                .map(|e| (e.0, e.1, e.2, e.3, e.4)),
        )
    }
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlSequenceControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceDiagramBoard>,
}

impl ControllerAdapter<UmlSequenceDomain> for UmlSequenceControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<UmlSequenceDomain, UmlSequenceDiagramBoardAdapter>;

    fn model(&self) -> ERef<UmlSequenceDiagramBoard> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<UmlSequenceDiagramBoard>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "umlsequence"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::umlsequence_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn insert_element(
        &mut self,
        parent: ModelUuid,
        element: UmlSequenceElement,
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
        undo: &mut Vec<(ModelUuid, UmlSequenceElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("UML Sequence Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared UML Sequence Diagram".to_owned().into(),
                UmlSequenceDiagramBoardAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlSequenceDiagramBoardAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceDiagramBoard>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: UmlSequenceDiagramBoardBuffer,
}

#[derive(Clone, Default)]
struct UmlSequenceDiagramBoardBuffer {
    name: String,
    comment: String,
}

impl UmlSequenceDiagramBoardAdapter {
    pub fn new(model: ERef<UmlSequenceDiagramBoard>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: UmlSequenceDiagramBoardBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<UmlSequenceDomain> for UmlSequenceDiagramBoardAdapter {
    fn model(&self) -> ERef<UmlSequenceDiagramBoard> {
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
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        element: UmlSequenceElement,
    ) -> Result<UmlSequenceElementView, HashSet<ModelUuid>> {
        let v = match element {
            UmlSequenceElement::Diagram(inner) => {
                // TODO: Diagram elements cannot currently be instantiated at the same time? :/
                // let r = inner.read();
                new_umlsequence_diagram_view(
                    inner.clone(),
                    Vec::new(),
                    Vec::new(),
                    egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0),
                    true,
                )
                .into()
            }
            UmlSequenceElement::CombinedFragment(inner) => {
                let r = inner.read();
                let section_views: Result<Vec<_>, _> = r
                    .sections
                    .iter()
                    .map(|e| {
                        self.create_new_view_for(q, e.clone().into())
                            .map(|e| match e {
                                UmlSequenceElementView::CombinedFragmentSection(inner) => inner,
                                _ => unreachable!(),
                            })
                    })
                    .collect();
                new_umlsequence_combinedfragment_view(inner.clone(), section_views?).into()
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let r = inner.read();
                let horizontal_element_views: Result<Vec<_>, _> = r
                    .horizontal_elements
                    .iter()
                    .map(|e| {
                        self.create_new_view_for(q, e.clone().to_element())
                            .map(|e| e.as_horizontal().unwrap())
                    })
                    .collect();
                new_umlsequence_combinedfragmentsection_view(
                    inner.clone(),
                    horizontal_element_views?,
                )
                .into()
            }
            UmlSequenceElement::Lifeline(inner) => new_umlsequence_lifeline_view(
                inner.clone(),
                UmlSequenceLifelineRenderStyle::Object,
                MGlobalColor::None,
            )
            .into(),
            UmlSequenceElement::Message(inner) => {
                let r = inner.read();
                let source_uuid = *r.source.read().uuid;
                let target_uuid = *r.target.read().uuid;
                let (Some(s), Some(t)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                else {
                    return Err([source_uuid, target_uuid].into_iter().collect());
                };
                new_umlsequence_message_view(
                    inner.clone(),
                    MGlobalColor::None,
                    MGlobalColor::None,
                    s,
                    t,
                )
                .into()
            }
            UmlSequenceElement::Ref(inner) => new_umlsequence_ref_view(inner.clone()).into(),
            UmlSequenceElement::Comment(inner) => {
                new_umlsequence_comment_view(inner, egui::Pos2::ZERO, egui::Align2::CENTER_CENTER)
                    .into()
            }
            UmlSequenceElement::CommentLink(_inner) => todo!(),
        };

        Ok(v)
    }
    fn label_for(&self, e: &UmlSequenceElement) -> Arc<String> {
        match e {
            UmlSequenceElement::Diagram(inner) => inner.read().name.clone(),
            UmlSequenceElement::CombinedFragment(inner) => {
                let r = inner.read();
                Arc::new(combined_fragment_display_text(r.kind, &r.kind_argument))
            }
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let r = inner.read();
                let s = if r.guard.is_empty() {
                    "Section".to_owned()
                } else {
                    format!("Section ([{}])", r.guard)
                };
                Arc::new(s)
            }
            UmlSequenceElement::Lifeline(inner) => inner.read().name.clone(),
            UmlSequenceElement::Message(inner) => {
                let r = inner.read();
                let s = if r.name.is_empty() {
                    "Message".to_owned()
                } else {
                    format!("Message ({})", r.name)
                };
                Arc::new(s)
            }
            UmlSequenceElement::Ref(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Ref".to_owned()
                } else {
                    format!("Ref ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
            UmlSequenceElement::Comment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
            UmlSequenceElement::CommentLink(_inner) => Arc::new("Comment Link".to_string()),
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
    fn enable_headers(&self) -> (bool, bool) {
        (true, false)
    }
    fn show_view_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
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
                UmlSequencePropChange::ColorChange((0, new_color).into()),
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
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlSequencePropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlSequencePropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlSequenceOrdinalMovement,
            UmlSequenceElementOrVertex,
            UmlSequencePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlSequencePropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlSequencePropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlSequencePropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                UmlSequencePropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlSequencePropChange::CommentChange(model.comment.clone()),
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
        settings: &UmlSequenceSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveUmlSequenceTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialUmlSequenceElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, UmlSequenceElement>) {
        let (new_model, models) = super::umlsequence_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, UmlSequenceElement>) {
        let models = super::umlsequence_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
}

fn new_controlller(
    model: ERef<UmlSequenceDiagramBoard>,
    name: String,
    elements: Vec<UmlSequenceElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            UmlSequenceControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                UmlSequenceDiagramBoardAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let model_uuid = ModelUuid::now_v7();
    let name = format!("New UML sequence diagram {}", no);
    let diagram = ERef::new(UmlSequenceDiagramBoard::new(
        model_uuid,
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (user_model, user_view) = new_umlsequence_lifeline(
        "User",
        "",
        UmlSequenceLifelineRenderStyle::StickFigure,
        MGlobalColor::None,
    );
    let (service1_model, service1_view) = new_umlsequence_lifeline(
        "Auth server",
        "",
        UmlSequenceLifelineRenderStyle::Object,
        MGlobalColor::None,
    );
    let (service2_model, service2_view) = new_umlsequence_lifeline(
        "Database",
        "",
        UmlSequenceLifelineRenderStyle::Database,
        MGlobalColor::None,
    );

    let (message1_model, message1_view) = new_umlsequence_message(
        "request",
        "",
        UmlSequenceMessageSynchronicityKind::Synchronous,
        UmlSequenceMessageLifecycleKind::None,
        false,
        0.0,
        MGlobalColor::None,
        MGlobalColor::None,
        (user_model.clone(), user_view.clone().into()),
        (service1_model.clone(), service1_view.clone().into()),
    );
    let (message2_model, message2_view) = new_umlsequence_message(
        "database query",
        "",
        UmlSequenceMessageSynchronicityKind::Synchronous,
        UmlSequenceMessageLifecycleKind::None,
        false,
        0.0,
        MGlobalColor::None,
        MGlobalColor::None,
        (service1_model.clone(), service1_view.clone().into()),
        (service2_model.clone(), service2_view.clone().into()),
    );

    let (ref_model, ref_view) = new_umlsequence_ref(
        "Request revalidation or terminate",
        [
            *user_model.read().uuid,
            *service1_model.read().uuid,
            *service2_model.read().uuid,
        ]
        .into_iter()
        .collect(),
    );

    let (combined_fragment_section1_model, combined_fragment_section1_view) =
        new_umlsequence_combinedfragmentsection(
            "invalid token",
            vec![(ref_model.into(), ref_view.into())],
        );
    let (combined_fragment_section2_model, combined_fragment_section2_view) =
        new_umlsequence_combinedfragmentsection(
            "token valid",
            vec![(message2_model.into(), message2_view.into())],
        );
    let (combined_fragment_model, combined_fragment_view) = new_umlsequence_combinedfragment(
        UmlSequenceCombinedFragmentKind::Alt,
        "",
        UmlSequenceActivationBehaviour::ContinueFirstVariant,
        [
            *user_model.read().uuid,
            *service1_model.read().uuid,
            *service2_model.read().uuid,
        ]
        .into_iter()
        .collect(),
        vec![
            (
                combined_fragment_section1_model,
                combined_fragment_section1_view,
            ),
            (
                combined_fragment_section2_model,
                combined_fragment_section2_view,
            ),
        ],
    );

    let (diagram_model, diagram_view) = new_umlsequence_diagram(
        "Diagram",
        vec![
            (user_model, user_view),
            (service1_model, service1_view),
            (service2_model, service2_view),
        ],
        vec![
            (message1_model.into(), message1_view.into()),
            (
                combined_fragment_model.into(),
                combined_fragment_view.into(),
            ),
        ],
        egui::Rect::from_min_size(egui::Pos2::new(100.0, 100.0), egui::Vec2::splat(500.0)),
        true,
    );

    let model_uuid = ModelUuid::now_v7();
    let name = format!("Demo UML sequence diagram {}", no);
    let diagram = ERef::new(UmlSequenceDiagramBoard::new(
        model_uuid,
        name.clone(),
        vec![diagram_model.into()],
    ));
    new_controlller(diagram, name, vec![diagram_view.into()])
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        UmlSequenceDomain,
        UmlSequenceControllerAdapter,
        DiagramControllerGen2<UmlSequenceDomain, UmlSequenceDiagramBoardAdapter>,
    >>(&uuid)?)
}

pub struct UmlSequenceSettings {
    palette: RwLock<ToolPalette<UmlSequenceToolStage, UmlSequenceDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<UmlSequenceToolStage, UmlSequenceElementView>>,
}

impl DiagramSettings for UmlSequenceSettings {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        shortcut_being_set: &Option<SetShortcut>,
    ) -> ShowSettingsResult {
        let mut w = self.palette.write().unwrap();
        let mut buffer = self.palette_edit_buffer.write().unwrap();
        let mut ret = ShowSettingsResult::None;

        ui.columns(2, |columns| {
            w.show_treeview(gdc, &mut columns[0]);

            let selected = w.get_selected();
            if selected.uuid() != buffer.uuid() {
                *buffer = w.get_buffer(selected.uuid().cloned());
            }
            match &mut *buffer {
                PaletteEditBuffer::None => {}
                PaletteEditBuffer::Group(_uuid, name) => {
                    if columns[1]
                        .labeled_text_edit_singleline("Label", name)
                        .changed()
                    {
                        w.set_from_buffer(buffer.clone());
                    }
                }
                PaletteEditBuffer::Tool(uuid, name, tool, view, ksc) => {
                    let mut modified = false;
                    modified |= columns[1]
                        .labeled_text_edit_singleline("Label", name)
                        .changed();

                    match crate::common::controller::show_shortcut(
                        &mut columns[1],
                        ksc,
                        shortcut_being_set
                            .as_ref()
                            .is_some_and(|e| e.is_diagram(uuid)),
                    ) {
                        crate::common::controller::ShortCutStatus::NoChange => {}
                        crate::common::controller::ShortCutStatus::Cleared => modified = true,
                        crate::common::controller::ShortCutStatus::Set => {
                            ret = ShowSettingsResult::SetShortcut(*uuid);
                        }
                        crate::common::controller::ShortCutStatus::CancelSet => {
                            ret = ShowSettingsResult::CancelShortcutSetting;
                        }
                    }

                    match tool {
                        UmlSequenceToolStage::CombinedFragmentStart {
                            kind,
                            end_behaviour,
                        } => {
                            columns[1].label("Kind:");
                            egui::ComboBox::from_id_salt("Kind:")
                                .selected_text(kind.as_str())
                                .show_ui(&mut columns[1], |ui| {
                                    for e in UmlSequenceCombinedFragmentKind::VARIANTS {
                                        modified |=
                                            ui.selectable_value(kind, e, e.as_str()).clicked();
                                    }
                                });

                            columns[1].label("End behaviour:");
                            egui::ComboBox::from_id_salt("End behaviour:")
                                .selected_text(end_behaviour.as_str())
                                .show_ui(&mut columns[1], |ui| {
                                    for e in UmlSequenceActivationBehaviour::VARIANTS {
                                        modified |= ui
                                            .selectable_value(end_behaviour, e, e.as_str())
                                            .clicked();
                                    }
                                });
                        }
                        UmlSequenceToolStage::Lifeline {
                            name,
                            stereotype,
                            render_style,
                            background_color,
                        } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Stereotype", stereotype)
                                .changed();
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Name", name)
                                .changed();

                            columns[1].label("Render style");
                            egui::ComboBox::from_id_salt("render style")
                                .selected_text(render_style.as_str())
                                .show_ui(&mut columns[1], |ui| {
                                    for e in UmlSequenceLifelineRenderStyle::VARIANTS {
                                        modified |= ui
                                            .selectable_value(render_style, e, e.as_str())
                                            .clicked();
                                    }
                                });
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
                        UmlSequenceToolStage::RefStart { text } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Text", text)
                                .changed();
                        }
                        UmlSequenceToolStage::Comment { text, align } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Text", text)
                                .changed();

                            egui::ComboBox::new("horizontal align", "Horizontal align")
                                .selected_text(format!("{:?}", align.x()))
                                .show_ui(&mut columns[1], |ui| {
                                    for e in
                                        [egui::Align::Min, egui::Align::Center, egui::Align::Max]
                                    {
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
                                    for e in
                                        [egui::Align::Min, egui::Align::Center, egui::Align::Max]
                                    {
                                        modified |= ui
                                            .selectable_value(
                                                &mut align.0[1],
                                                e,
                                                format!("{:?}", e),
                                            )
                                            .changed();
                                    }
                                });
                        }
                        UmlSequenceToolStage::LinkStart { link_type } => match link_type {
                            LinkType::Message {
                                synchronicity_kind,
                                is_return,
                                name,
                                duration,
                                found_activation_color,
                                new_activation_color,
                                state_invariant,
                            } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();

                                columns[1].label("Synchronicity:");
                                egui::ComboBox::from_id_salt("synchronicity")
                                    .selected_text(synchronicity_kind.as_str())
                                    .show_ui(&mut columns[1], |ui| {
                                        for e in UmlSequenceMessageSynchronicityKind::VARIANTS {
                                            modified |= ui
                                                .selectable_value(synchronicity_kind, e, e.as_str())
                                                .changed();
                                        }
                                    });

                                modified |= columns[1].checkbox(is_return, "isReturn").changed();
                                modified |= columns[1]
                                    .add(egui::DragValue::new(duration).speed(1.0))
                                    .changed();

                                columns[1].label("Found activation color");
                                if let Some(new_color) =
                                    crate::common::controller::mglobalcolor_edit_button(
                                        gdc,
                                        &mut columns[1],
                                        found_activation_color,
                                    )
                                {
                                    *found_activation_color = new_color;
                                    modified = true;
                                }

                                columns[1].label("New activation color");
                                if let Some(new_color) =
                                    crate::common::controller::mglobalcolor_edit_button(
                                        gdc,
                                        &mut columns[1],
                                        new_activation_color,
                                    )
                                {
                                    *new_activation_color = new_color;
                                    modified = true;
                                }

                                modified |= columns[1]
                                    .labeled_text_edit_singleline(
                                        "State invariant",
                                        state_invariant,
                                    )
                                    .changed();
                            }
                        },
                        _ => {}
                    }

                    if modified {
                        *view = view_for_stage(tool);
                        w.set_from_buffer(buffer.clone());
                    }
                }
            }
        });

        ret
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
impl DiagramSettings2<UmlSequenceDomain> for UmlSequenceSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    UmlSequenceToolStage,
                    String,
                    UmlSequenceElementView,
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
            "Containers",
            vec![
                (
                    UmlSequenceToolStage::DiagramStart,
                    "Diagram",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num7,
                    )),
                ),
                (
                    UmlSequenceToolStage::CombinedFragmentStart {
                        kind: UmlSequenceCombinedFragmentKind::Opt,
                        end_behaviour: UmlSequenceActivationBehaviour::ContinueFirstVariant,
                    },
                    "Combined Fragment",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num8,
                    )),
                ),
            ],
        ),
        (
            "Elements",
            vec![
                (
                    UmlSequenceToolStage::Lifeline {
                        name: "User".to_owned(),
                        stereotype: "".to_owned(),
                        render_style: UmlSequenceLifelineRenderStyle::StickFigure,
                        background_color: MGlobalColor::None,
                    },
                    "Actor Lifeline",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num4,
                    )),
                ),
                (
                    UmlSequenceToolStage::Lifeline {
                        name: "s: Service".to_owned(),
                        stereotype: "".to_owned(),
                        render_style: UmlSequenceLifelineRenderStyle::Object,
                        background_color: MGlobalColor::None,
                    },
                    "Object Lifeline",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num5,
                    )),
                ),
            ],
        ),
        (
            "Messages",
            vec![
                (
                    UmlSequenceToolStage::LinkStart {
                        link_type: LinkType::Message {
                            synchronicity_kind: UmlSequenceMessageSynchronicityKind::Synchronous,
                            is_return: false,
                            name: "".to_owned(),
                            duration: 0.0,
                            found_activation_color: MGlobalColor::None,
                            new_activation_color: MGlobalColor::None,
                            state_invariant: "".to_owned(),
                        },
                    },
                    "Synchronous Message",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num1,
                    )),
                ),
                (
                    UmlSequenceToolStage::LinkStart {
                        link_type: LinkType::Message {
                            synchronicity_kind: UmlSequenceMessageSynchronicityKind::Synchronous,
                            is_return: true,
                            name: "".to_owned(),
                            duration: 0.0,
                            found_activation_color: MGlobalColor::None,
                            new_activation_color: MGlobalColor::None,
                            state_invariant: "".to_owned(),
                        },
                    },
                    "Synchronous Return",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num2,
                    )),
                ),
                (
                    UmlSequenceToolStage::LinkStart {
                        link_type: LinkType::Message {
                            synchronicity_kind:
                                UmlSequenceMessageSynchronicityKind::AsynchronousCall,
                            is_return: false,
                            name: "".to_owned(),
                            duration: 0.0,
                            found_activation_color: MGlobalColor::None,
                            new_activation_color: MGlobalColor::None,
                            state_invariant: "".to_owned(),
                        },
                    },
                    "Asynchronous Call",
                    None,
                ),
                (
                    UmlSequenceToolStage::LinkStart {
                        link_type: LinkType::Message {
                            synchronicity_kind:
                                UmlSequenceMessageSynchronicityKind::AsynchronousSignal,
                            is_return: false,
                            name: "".to_owned(),
                            duration: 0.0,
                            found_activation_color: MGlobalColor::None,
                            new_activation_color: MGlobalColor::None,
                            state_invariant: "".to_owned(),
                        },
                    },
                    "Asynchronous Signal",
                    None,
                ),
            ],
        ),
        (
            "Other",
            vec![
                (
                    UmlSequenceToolStage::RefStart {
                        text: "Checkout".to_owned(),
                    },
                    "Interaction Fragment",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num3,
                    )),
                ),
                (
                    UmlSequenceToolStage::Comment {
                        text: "a comment".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                    },
                    "Comment",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num9,
                    )),
                ),
                //(UmlSequenceToolStage::CommentLinkStart, "Comment Link", commentlink.1.into()),
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
                    (e.0, e.1, v, e.2)
                })
                .collect(),
        )
    })
    .collect();

    Box::new(UmlSequenceSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
    })
}

fn view_for_stage(s: &UmlSequenceToolStage) -> UmlSequenceElementView {
    match s {
        UmlSequenceToolStage::DiagramStart => {
            let diagram_view = new_umlsequence_diagram(
                "Diagram",
                Vec::new(),
                Vec::new(),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(100.0, 75.0)),
                true,
            )
            .1;
            diagram_view.write().refresh_buffers();
            diagram_view.into()
        }
        UmlSequenceToolStage::CombinedFragmentStart {
            kind,
            end_behaviour,
        } => {
            let combined_fragment_view = {
                let section = new_umlsequence_combinedfragmentsection("no errors", Vec::new());
                section.1.write().refresh_buffers();
                new_umlsequence_combinedfragment(
                    *kind,
                    "",
                    *end_behaviour,
                    HashSet::new(),
                    vec![section],
                )
                .1
            };
            combined_fragment_view.write().refresh_buffers();
            combined_fragment_view.into()
        }
        UmlSequenceToolStage::Lifeline {
            name,
            stereotype,
            render_style,
            background_color,
        } => {
            let lifeline_view =
                new_umlsequence_lifeline(name, stereotype, *render_style, *background_color).1;
            lifeline_view.into()
        }
        UmlSequenceToolStage::LinkStart { link_type } => {
            let d1 = new_umlsequence_lifeline(
                "dummy",
                "",
                UmlSequenceLifelineRenderStyle::StickFigure,
                MGlobalColor::None,
            );
            let d2 = new_umlsequence_lifeline(
                "dummy",
                "",
                UmlSequenceLifelineRenderStyle::Object,
                MGlobalColor::None,
            );
            d2.1.write().bounds_rect = egui::Rect::from_x_y_ranges(150.0..=150.0, 0.0..=0.0);

            match link_type {
                LinkType::Message {
                    synchronicity_kind,
                    is_return,
                    name,
                    duration,
                    found_activation_color,
                    new_activation_color,
                    state_invariant,
                } => {
                    let message_view = new_umlsequence_message(
                        name,
                        state_invariant,
                        *synchronicity_kind,
                        UmlSequenceMessageLifecycleKind::None,
                        *is_return,
                        *duration,
                        *found_activation_color,
                        *new_activation_color,
                        (d1.0, d1.1.into()),
                        (d2.0, d2.1.into()),
                    )
                    .1;
                    message_view.write().refresh_buffers();
                    message_view.into()
                }
            }
        }
        UmlSequenceToolStage::RefStart { text } => {
            let ref_view = new_umlsequence_ref(text, HashSet::new()).1;
            ref_view.write().refresh_buffers();
            ref_view.into()
        }
        UmlSequenceToolStage::Comment { text, align } => {
            let comment_view = new_umlsequence_comment(text, egui::Pos2::ZERO, *align).1;
            comment_view.into()
        }
        UmlSequenceToolStage::CommentLinkStart => todo!(),

        UmlSequenceToolStage::DiagramEnd
        | UmlSequenceToolStage::CombinedFragmentEnd
        | UmlSequenceToolStage::LinkEnd
        | UmlSequenceToolStage::RefEnd
        | UmlSequenceToolStage::CommentLinkEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(UmlSequenceSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
    }))
}

inventory::submit! {DiagramInfo {
    type_indentifier: "umlsequence",
    pretty_name: "UML Sequence diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "/Unified Modeling Language",
        description: "UML Sequence diagram (lifelines, messages, etc.)",
        constructors: &[
            ("empty", &(new as DiagramConstructorF)),
            ("demo", &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkType {
    Message {
        synchronicity_kind: UmlSequenceMessageSynchronicityKind,
        is_return: bool,
        name: String,
        duration: f32,
        found_activation_color: MGlobalColor,
        new_activation_color: MGlobalColor,
        state_invariant: String,
    },
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlSequenceToolStage {
    DiagramStart,
    DiagramEnd,
    CombinedFragmentStart {
        kind: UmlSequenceCombinedFragmentKind,
        end_behaviour: UmlSequenceActivationBehaviour,
    },
    CombinedFragmentEnd,
    Lifeline {
        name: String,
        stereotype: String,
        render_style: UmlSequenceLifelineRenderStyle,
        background_color: MGlobalColor,
    },
    LinkStart {
        link_type: LinkType,
    },
    LinkEnd,
    RefStart {
        text: String,
    },
    RefEnd,
    Comment {
        text: String,
        align: egui::Align2,
    },
    CommentLinkStart,
    CommentLinkEnd,
}

enum PartialUmlSequenceElement {
    None,
    Some(UmlSequenceElementView),
    Diagram {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    CombinedFragment {
        kind: UmlSequenceCombinedFragmentKind,
        end_behaviour: UmlSequenceActivationBehaviour,
        source: ERef<UmlSequenceLifeline>,
        dest: Option<ERef<UmlSequenceLifeline>>,
    },
    Link {
        link_type: LinkType,
        source: ERef<UmlSequenceLifeline>,
        dest: Option<ERef<UmlSequenceLifeline>>,
    },
    Ref {
        text: String,
        source: ERef<UmlSequenceLifeline>,
        dest: Option<ERef<UmlSequenceLifeline>>,
    },
    CommentLink {
        source: ERef<UmlSequenceComment>,
        dest: Option<UmlSequenceElement>,
    },
}

pub struct NaiveUmlSequenceTool {
    uuid: uuid::Uuid,
    initial_stage: UmlSequenceToolStage,
    current_stage: UmlSequenceToolStage,
    result: PartialUmlSequenceElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveUmlSequenceTool {
    fn try_spend(&mut self) {
        self.result = PartialUmlSequenceElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<UmlSequenceDomain> for NaiveUmlSequenceTool {
    type Stage = UmlSequenceToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: UmlSequenceToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialUmlSequenceElement::None,
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

    fn targetting_for_section(&self, element: Option<UmlSequenceElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                UmlSequenceToolStage::DiagramStart
                | UmlSequenceToolStage::DiagramEnd
                | UmlSequenceToolStage::Comment { .. } => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(UmlSequenceElement::Diagram(_)) => match self.current_stage {
                UmlSequenceToolStage::Lifeline { .. }
                | UmlSequenceToolStage::LinkStart { .. }
                | UmlSequenceToolStage::LinkEnd
                | UmlSequenceToolStage::CombinedFragmentStart { .. }
                | UmlSequenceToolStage::CombinedFragmentEnd
                | UmlSequenceToolStage::RefStart { .. }
                | UmlSequenceToolStage::RefEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(UmlSequenceElement::CombinedFragmentSection(_)) => match self.current_stage {
                UmlSequenceToolStage::LinkStart { .. }
                | UmlSequenceToolStage::LinkEnd
                | UmlSequenceToolStage::CombinedFragmentStart { .. }
                | UmlSequenceToolStage::CombinedFragmentEnd
                | UmlSequenceToolStage::RefStart { .. }
                | UmlSequenceToolStage::RefEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            _ => NON_TARGETTABLE_COLOR,
        }
    }
    fn draw_status_hint(
        &self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match &self.result {
            PartialUmlSequenceElement::Link { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_line(
                        [egui::Pos2::new(source_view.position().x, pos.y), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlSequenceElement::CombinedFragment { source, .. }
            | PartialUmlSequenceElement::Ref { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_rectangle(
                        egui::Rect::from_two_pos(
                            egui::Pos2::new(source_view.position().x, pos.y),
                            pos,
                        )
                        .expand(UmlSequenceCombinedFragmentSectionView::SECTION_PADDING_X / 2.0),
                        egui::CornerRadius::ZERO,
                        egui::Color32::TRANSPARENT,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlSequenceElement::CommentLink { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlSequenceElement::Diagram { a, .. } => {
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
            (
                UmlSequenceToolStage::Lifeline {
                    name,
                    stereotype,
                    render_style,
                    background_color,
                },
                _,
            ) => {
                let (_class_model, class_view) =
                    new_umlsequence_lifeline(name, stereotype, *render_style, *background_color);
                self.result = PartialUmlSequenceElement::Some(class_view.into());
                self.event_lock = true;
            }
            (UmlSequenceToolStage::DiagramStart, _) => {
                self.result = PartialUmlSequenceElement::Diagram { a: pos, b: None };
                self.current_stage = UmlSequenceToolStage::DiagramEnd;
                self.event_lock = true;
            }
            (UmlSequenceToolStage::DiagramEnd, PartialUmlSequenceElement::Diagram { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (UmlSequenceToolStage::Comment { text, align }, PartialUmlSequenceElement::None) => {
                let (_comment_model, comment_view) = new_umlsequence_comment(text, pos, *align);
                self.result = PartialUmlSequenceElement::Some(comment_view.into());
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, element: UmlSequenceElement) {
        if self.event_lock {
            return;
        }

        if let UmlSequenceElement::Lifeline(inner) = element {
            match (&self.current_stage, &mut self.result) {
                (
                    UmlSequenceToolStage::LinkStart { link_type },
                    PartialUmlSequenceElement::None,
                ) => {
                    self.result = PartialUmlSequenceElement::Link {
                        link_type: link_type.clone(),
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = UmlSequenceToolStage::LinkEnd;
                    self.event_lock = true;
                }
                (UmlSequenceToolStage::LinkEnd, PartialUmlSequenceElement::Link { dest, .. }) => {
                    *dest = Some(inner);
                    self.event_lock = true;
                }
                (
                    UmlSequenceToolStage::CombinedFragmentStart {
                        kind,
                        end_behaviour,
                    },
                    PartialUmlSequenceElement::None,
                ) => {
                    self.result = PartialUmlSequenceElement::CombinedFragment {
                        kind: *kind,
                        end_behaviour: *end_behaviour,
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = UmlSequenceToolStage::CombinedFragmentEnd;
                    self.event_lock = true;
                }
                (
                    UmlSequenceToolStage::CombinedFragmentEnd,
                    PartialUmlSequenceElement::CombinedFragment { dest, .. },
                ) => {
                    *dest = Some(inner);
                    self.event_lock = true;
                }
                (UmlSequenceToolStage::RefStart { text }, PartialUmlSequenceElement::None) => {
                    self.result = PartialUmlSequenceElement::Ref {
                        text: text.clone(),
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = UmlSequenceToolStage::RefEnd;
                    self.event_lock = true;
                }
                (UmlSequenceToolStage::RefEnd, PartialUmlSequenceElement::Ref { dest, .. }) => {
                    *dest = Some(inner);
                    self.event_lock = true;
                }
                _ => {}
            }
        }
    }

    fn try_flush(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &self.result {
            PartialUmlSequenceElement::Some(element) => {
                let element = element.clone();
                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: element.into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlSequenceElement::Diagram { a, b: Some(b), .. } => {
                self.current_stage = UmlSequenceToolStage::DiagramStart;

                let (_diagram_model, diagram_view) = new_umlsequence_diagram(
                    "Diagram",
                    Vec::new(),
                    Vec::new(),
                    egui::Rect::from_two_pos(*a, *b),
                    true,
                );

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlSequenceElementView::from(diagram_view).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialUmlSequenceElement::CombinedFragment {
                kind,
                end_behaviour,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.find_container(&source_view.uuid(), |_, e| {
                        matches!(e, UmlSequenceElementView::Diagram(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlSequenceElementView::Diagram(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let section = new_umlsequence_combinedfragmentsection("", Vec::new());
                    let cf_view = new_umlsequence_combinedfragment(
                        *kind,
                        "",
                        *end_behaviour,
                        [source_uuid, target_uuid].into_iter().collect(),
                        vec![section],
                    )
                    .1;

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: UmlSequenceElementView::from(cf_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialUmlSequenceElement::Link {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.find_container(&source_view.uuid(), |_, e| {
                        matches!(e, UmlSequenceElementView::Diagram(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlSequenceElementView::Diagram(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let link_view: UmlSequenceElementView = match link_type {
                        LinkType::Message {
                            synchronicity_kind,
                            is_return,
                            name,
                            duration,
                            found_activation_color,
                            new_activation_color,
                            state_invariant,
                        } => new_umlsequence_message(
                            name,
                            state_invariant,
                            *synchronicity_kind,
                            UmlSequenceMessageLifecycleKind::None,
                            *is_return,
                            *duration,
                            *found_activation_color,
                            *new_activation_color,
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
            PartialUmlSequenceElement::Ref {
                text,
                source,
                dest: Some(dest),
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.find_container(&source_view.uuid(), |_, e| {
                        matches!(e, UmlSequenceElementView::Diagram(_))
                    })
                    .map(|e| e.0)
                        == q.find_container(&target_view.uuid(), |_, e| {
                            matches!(e, UmlSequenceElementView::Diagram(_))
                        })
                        .map(|e| e.0)
                {
                    self.current_stage = self.initial_stage.clone();

                    let ref_view =
                        new_umlsequence_ref(text, [source_uuid, target_uuid].into_iter().collect())
                            .1;

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: UmlSequenceElementView::from(ref_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialUmlSequenceElement::CommentLink {
                source,
                dest: Some(dest),
            } => {
                let source_uuid = *source.read().uuid();
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&dest.uuid()))
                {
                    self.current_stage = UmlSequenceToolStage::CommentLinkStart;

                    let (_link_model, link_view) = new_umlsequence_commentlink(
                        None,
                        (source.clone(), source_view),
                        (dest.clone(), target_view),
                    );

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: UmlSequenceElementView::from(link_view).into(),
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

pub fn new_umlsequence_diagram(
    name: &str,
    lifelines: Vec<(ERef<UmlSequenceLifeline>, ERef<UmlSequenceLifelineView>)>,
    horizontals: Vec<(
        UmlSequenceHorizontalElement,
        UmlSequenceHorizontalElementView,
    )>,
    bounds_rect: egui::Rect,
    show_activations: bool,
) -> (ERef<UmlSequenceDiagram>, ERef<UmlSequenceDiagramView>) {
    let (lifeline_models, lifeline_views) = lifelines.into_iter().collect();
    let (horizontal_models, horizontal_views) = horizontals.into_iter().collect();
    let diagram_model = ERef::new(UmlSequenceDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        lifeline_models,
        horizontal_models,
    ));
    let package_view = new_umlsequence_diagram_view(
        diagram_model.clone(),
        lifeline_views,
        horizontal_views,
        bounds_rect,
        show_activations,
    );

    (diagram_model, package_view)
}
pub fn new_umlsequence_diagram_view(
    model: ERef<UmlSequenceDiagram>,
    lifeline_views: Vec<ERef<UmlSequenceLifelineView>>,
    horizontal_element_views: Vec<UmlSequenceHorizontalElementView>,
    bounds_rect: egui::Rect,
    show_activations: bool,
) -> ERef<UmlSequenceDiagramView> {
    ERef::new(UmlSequenceDiagramView {
        uuid: ViewUuid::now_v7().into(),
        model,
        lifeline_views,
        horizontal_element_views,
        temporaries: Default::default(),
        bounds_rect,
        show_activations,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceDiagramView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceDiagram>,

    #[nh_context_serde(entity)]
    lifeline_views: Vec<ERef<UmlSequenceLifelineView>>,
    #[nh_context_serde(entity)]
    horizontal_element_views: Vec<UmlSequenceHorizontalElementView>,

    #[nh_context_serde(skip_and_default)]
    temporaries: UmlSequenceDiagramViewTemporaries,
    bounds_rect: egui::Rect,
    show_activations: bool,
}

#[derive(Clone, Default)]
struct UmlSequenceDiagramViewTemporaries {
    display_text: String,
    name_buffer: String,
    comment_buffer: String,

    all_elements: HashMap<ViewUuid, SelectionStatus>,
    selected_direct_elements: HashSet<ViewUuid>,
    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
}

impl UmlSequenceDiagramView {
    const MIN_SIZE: egui::Vec2 = egui::Vec2::new(100.0, 200.0);

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

    fn lifeline_insertion_place(&self, pos: egui::Pos2) -> (PositionNoT, egui::Rect) {
        let lifelines_total = self.lifeline_views.len();
        let lifeline_width = self.bounds_rect.width() / (lifelines_total.max(1) as f32);

        let selected_lifeline_idx =
            ((pos.x - self.bounds_rect.min.x + lifeline_width / 2.0) / lifeline_width).floor();
        let selected_lifeline_start_x = if selected_lifeline_idx <= 0.0 {
            self.bounds_rect.min.x
        } else {
            self.bounds_rect.min.x + (selected_lifeline_idx - 0.5) * lifeline_width
        };
        let selected_lifeline_width =
            if selected_lifeline_idx <= 0.0 || selected_lifeline_idx >= lifelines_total as f32 {
                lifeline_width / 2.0
            } else {
                lifeline_width
            };

        (
            (selected_lifeline_idx as usize).try_into().unwrap(),
            egui::Rect::from_min_size(
                egui::Pos2::new(selected_lifeline_start_x, self.bounds_rect.min.y),
                egui::Vec2::new(selected_lifeline_width, self.bounds_rect.height()),
            ),
        )
    }

    fn horizontal_insertion_place(
        &self,
        pos: egui::Pos2,
    ) -> Option<(
        Option<PositionNoT>,
        ERef<UmlSequenceLifeline>,
        egui::Rect,
        egui::Rect,
    )> {
        let (lifeline_center, lifeline) = 'a: {
            for e in &self.lifeline_views {
                let r = e.read();
                if r.bounds_rect.min.x < pos.x && pos.x < r.bounds_rect.max.x {
                    break 'a (r.bounds_rect.center().x, r.model.clone());
                }
            }
            return None;
        };

        let mut insertion_index = Option::<PositionNoT>::None;
        let mut nearest_before = Option::<f32>::None;
        let mut nearest_after = Option::<f32>::None;

        for (idx, v) in self.horizontal_element_views.iter().enumerate() {
            let shape_bb = v.min_shape().bounding_box();
            let (min_y, max_y) = (shape_bb.min.y, shape_bb.max.y);
            if min_y < pos.y && pos.y < max_y {
                return None;
            }
            if max_y < pos.y && nearest_before.is_none_or(|e| e < max_y) {
                nearest_before = Some(max_y);
            }
            if min_y > pos.y && nearest_after.is_none_or(|e| e > min_y) {
                insertion_index = Some(idx.try_into().unwrap());
                nearest_after = Some(min_y);
            }
        }

        let nearest_before = nearest_before.unwrap_or(self.bounds_rect.min.y);
        let nearest_after = nearest_after.unwrap_or(self.bounds_rect.max.y);
        let nearest_average = (nearest_before + nearest_after) / 2.0;

        const WIDTH: f32 = 20.0;
        let lifeline_rect = egui::Rect::from_center_size(
            egui::Pos2::new(lifeline_center, nearest_average),
            egui::Vec2::new(WIDTH, nearest_after - nearest_before),
        );
        let horizontal_rect = self
            .bounds_rect
            .with_min_y(nearest_average - WIDTH / 2.0)
            .with_max_y(nearest_average + WIDTH / 2.0);

        Some((insertion_index, lifeline, lifeline_rect, horizontal_rect))
    }
}

impl Entity for UmlSequenceDiagramView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceDiagramView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceDiagramView {
    fn model(&self) -> UmlSequenceElement {
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

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceDiagramView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        let child = self
            .lifeline_views
            .iter_mut()
            .flat_map(|v| {
                v.write()
                    .show_properties(gdc, q, ui, commands)
                    .non_default()
            })
            .next()
            .or_else(|| {
                self.horizontal_element_views
                    .iter_mut()
                    .flat_map(|v| v.show_properties(gdc, q, ui, commands).non_default())
                    .next()
            });

        if let Some(child) = child {
            child
        } else if self.temporaries.highlight.selected {
            ui.label("Model properties");

            if ui
                .labeled_text_edit_multiline("Name:", &mut self.temporaries.name_buffer)
                .changed()
            {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    UmlSequencePropChange::NameChange(Arc::new(
                        self.temporaries.name_buffer.clone(),
                    )),
                ));
            }

            if ui
                .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
                .changed()
            {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    UmlSequencePropChange::CommentChange(Arc::new(
                        self.temporaries.comment_buffer.clone(),
                    )),
                ));
            }

            ui.add_space(crate::common::views::VIEW_MODEL_PROPERTIES_BLOCK_SPACING);
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
                    let egui::Vec2 { mut x, .. } = self.bounds_rect.size();

                    ui.label("width");
                    if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                        commands.push(InsensitiveCommand::ResizeElementsBy(
                            q.selected_views(),
                            egui::Align2::LEFT_CENTER,
                            egui::Vec2::new(x - self.bounds_rect.width(), 0.0),
                        ));
                    }
                    ui.end_row();
                }
            });

            let mut show_activations = self.show_activations;
            if ui
                .checkbox(&mut show_activations, "show activations")
                .changed()
            {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    UmlSequencePropChange::ShowActivationsChange(show_activations),
                ));
            }

            PropertiesStatus::Shown
        } else {
            PropertiesStatus::NotShown
        }
    }
    fn draw_in(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> TargettingStatus {
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );

        fn draw_children(
            s: &mut UmlSequenceDiagramView,
            q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
            context: &GlobalDrawingContext,
            settings: &<UmlSequenceDomain as Domain>::SettingsT,
            canvas: &mut dyn canvas::NHCanvas,
            tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
        ) -> TargettingStatus {
            let mut drawn_child_targetting = TargettingStatus::NotDrawn;

            let lifelines_no = s.lifeline_views.len();
            let sliver_x = s.bounds_rect.width() / lifelines_no as f32 / 2.0;
            let max_object_height = s
                .lifeline_views
                .iter()
                .map(|v| v.read().min_shape().bounding_box().height())
                .max_by(|l, r| l.partial_cmp(r).unwrap())
                .unwrap_or(0.0);
            let lifelines_y =
                s.bounds_rect.top() + max_object_height + canvas::CLASS_MIDDLE_FONT_SIZE;
            for (idx, v) in s.lifeline_views.iter().enumerate() {
                let x = s.bounds_rect.min.x + (2 * idx + 1) as f32 * sliver_x;
                let t = v.write().draw_inner(
                    egui::Pos2::new(x, lifelines_y),
                    s.bounds_rect.max.y,
                    q,
                    context,
                    settings,
                    canvas,
                    tool,
                );
                if t != TargettingStatus::NotDrawn {
                    drawn_child_targetting = t;
                }
            }

            let message_offsets = if s.show_activations {
                let mut ac = ActivationsCounter::default();
                for e in s.horizontal_element_views.iter() {
                    e.count_activations(&mut ac);
                }

                let (offsets, activations) = ac.finish();
                for (id, no, start_y, end_y, color) in activations {
                    let shift = ActivationsCounter::ACTIVATION_OFFSET * no as f32;
                    let lifeline_center_x = s
                        .lifeline_views
                        .iter()
                        .find(|e| *e.read().uuid == id)
                        .map(|e| e.read().bounds_rect.center().x)
                        .unwrap_or(0.0);
                    canvas.draw_rectangle(
                        egui::Rect::from_x_y_ranges(
                            (lifeline_center_x - ActivationsCounter::ACTIVATION_WIDTH / 2.0 + shift)
                                ..=(lifeline_center_x
                                    + ActivationsCounter::ACTIVATION_WIDTH / 2.0
                                    + shift),
                            start_y..=end_y,
                        ),
                        egui::CornerRadius::ZERO,
                        context
                            .global_colors
                            .get(&color)
                            .unwrap_or(egui::Color32::WHITE),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                offsets
            } else {
                HashMap::new()
            };

            const PADDING_Y: f32 = 2.0;
            let mut counter_y = s.bounds_rect.min.y + 2.0 * max_object_height + PADDING_Y;
            for v in s.horizontal_element_views.iter_mut() {
                let (t, r) = v.draw_inner(
                    &s.lifeline_views,
                    &message_offsets,
                    counter_y,
                    q,
                    context,
                    settings,
                    canvas,
                    tool,
                );
                if t != TargettingStatus::NotDrawn {
                    drawn_child_targetting = t;
                }
                counter_y = r.max.y;
            }
            s.bounds_rect.set_bottom(
                counter_y.max(s.bounds_rect.min.y + UmlSequenceDiagramView::MIN_SIZE.y),
            );
            drawn_child_targetting
        }
        let drawn_child_targetting = draw_children(self, q, context, settings, canvas, tool);

        // Draw top left pentagon
        const PENTAGON_PADDING: f32 = 4.0;
        let pentagon_bg = egui::Color32::WHITE;
        let left_top_pentagon_rect = canvas
            .measure_text(
                self.bounds_rect.left_top() + egui::Vec2::splat(PENTAGON_PADDING),
                egui::Align2::LEFT_TOP,
                &self.temporaries.display_text,
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
            self.temporaries.highlight,
        );
        canvas.draw_text(
            self.bounds_rect.left_top() + egui::Vec2::splat(PENTAGON_PADDING),
            egui::Align2::LEFT_TOP,
            &self.temporaries.display_text,
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
                (self.bounds_rect.left_center(), "<"),
                (self.bounds_rect.right_center(), ">"),
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
                    match t.current_stage {
                        UmlSequenceToolStage::Lifeline { .. } => {
                            canvas.draw_rectangle(
                                self.lifeline_insertion_place(*pos).1,
                                egui::CornerRadius::ZERO,
                                t.targetting_for_section(Some(self.model.clone().into())),
                                canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                                canvas::Highlight::NONE,
                            );
                        }
                        UmlSequenceToolStage::LinkStart { .. }
                        | UmlSequenceToolStage::LinkEnd
                        | UmlSequenceToolStage::CombinedFragmentStart { .. }
                        | UmlSequenceToolStage::CombinedFragmentEnd
                        | UmlSequenceToolStage::RefStart { .. }
                        | UmlSequenceToolStage::RefEnd => {
                            if let Some((.., lr, hr)) = self.horizontal_insertion_place(*pos) {
                                canvas.draw_rectangle(
                                    lr,
                                    egui::CornerRadius::ZERO,
                                    t.targetting_for_section(Some(self.model.clone().into())),
                                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                                    canvas::Highlight::NONE,
                                );
                                canvas.draw_rectangle(
                                    hr,
                                    egui::CornerRadius::ZERO,
                                    t.targetting_for_section(Some(self.model.clone().into())),
                                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                                    canvas::Highlight::NONE,
                                );
                            }
                        }
                        _ => {
                            canvas.draw_rectangle(
                                self.bounds_rect,
                                egui::CornerRadius::ZERO,
                                t.targetting_for_section(Some(self.model.clone().into())),
                                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                                canvas::Highlight::NONE,
                            );
                        }
                    }

                    draw_children(self, q, context, settings, canvas, tool);

                    TargettingStatus::Drawn
                }
                _ => drawn_child_targetting,
            }
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid, self.min_shape());

        self.lifeline_views
            .iter_mut()
            .for_each(|v| v.write().collect_allignment(am));
        self.horizontal_element_views
            .iter_mut()
            .for_each(|v| v.collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        let k_status = self
            .lifeline_views
            .iter_mut()
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
                self.horizontal_element_views
                    .iter_mut()
                    .flat_map(|v| {
                        let s = v.handle_event_inner(
                            &self.lifeline_views,
                            event,
                            ehc,
                            settings,
                            q,
                            tool,
                            element_setup_modal,
                            commands,
                        );
                        if s != EventHandlingStatus::NotHandled {
                            Some((*v.uuid(), s))
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
                        (egui::Align2::RIGHT_CENTER, self.bounds_rect.left_center()),
                        (egui::Align2::LEFT_CENTER, self.bounds_rect.right_center()),
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
            InputEvent::Click(pos) => {
                if !self.min_shape().contains(pos) {
                    return k_status
                        .map(|e| e.1)
                        .unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    let horizontal_place = self.horizontal_insertion_place(pos);
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());
                    if let Some(h) = &horizontal_place {
                        tool.add_section(h.1.clone().into());
                    }

                    let pos = if matches!(tool.initial_stage, UmlSequenceToolStage::Lifeline { .. })
                    {
                        Some(self.lifeline_insertion_place(pos).0)
                    } else {
                        horizontal_place.and_then(|e| e.0)
                    };
                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, pos, commands)
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
                                !self.temporaries.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
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
                        !self.temporaries.all_elements.get(e).is_some()
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
                            !self.temporaries.all_elements.get(e).is_some()
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
        command: &InsensitiveCommand<
            UmlSequenceOrdinalMovement,
            UmlSequenceElementOrVertex,
            UmlSequencePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.lifeline_views.iter_mut().for_each(|v| {
                    v.write()
                        .apply_command(command, undo_accumulator, affected_models)
                });
                self.horizontal_element_views
                    .iter_mut()
                    .for_each(|v| v.apply_command(command, undo_accumulator, affected_models));
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
                    match set {
                        true => {
                            self.temporaries.selected_direct_elements =
                                self.lifeline_views.iter().map(|v| *v.read().uuid).collect();
                            self.horizontal_element_views.iter().for_each(|v| {
                                self.temporaries.selected_direct_elements.insert(*v.uuid());
                            });
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
                        .lifeline_views
                        .iter()
                        .map(|v| *v.read().uuid)
                        .chain(self.horizontal_element_views.iter().map(|v| *v.uuid()))
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
                self.lifeline_views.iter_mut().for_each(|v| {
                    v.write().apply_command(
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
                self.horizontal_element_views.iter_mut().for_each(|v| {
                    v.apply_command(
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    let min_delta_x = Self::MIN_SIZE.x - self.bounds_rect.width();
                    let (left, right) = match align.x() {
                        egui::Align::Min => (0.0, delta.x.max(min_delta_x)),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => ((-delta.x).max(min_delta_x), 0.0),
                    };
                    let min_delta_y = Self::MIN_SIZE.y - self.bounds_rect.height();
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
                for element in self
                    .lifeline_views
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
                        element: UmlSequenceElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }
                self.lifeline_views
                    .retain(|v| !uuids.contains(&v.read().uuid));

                for element in self
                    .horizontal_element_views
                    .iter()
                    .filter(|v| uuids.contains(&v.uuid()))
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
                        element: element.clone().as_element_view().into(),
                        into_model: false,
                    });
                }
                self.horizontal_element_views
                    .retain(|v| !uuids.contains(&v.uuid()));

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
                    if (*bucket == 0 || *bucket == VERTICALS_BUCKET)
                        && let Ok(UmlSequenceElementView::Lifeline(view)) =
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
                            let uuid = *vw.uuid();
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
                            vw.head_count(
                                &mut HashMap::new(),
                                &mut HashMap::new(),
                                &mut model_transitives,
                            );
                            affected_models.extend(model_transitives.into_keys());

                            let view_pos = {
                                let mut view_pos: PositionNoT = 0;
                                for e in &self.lifeline_views {
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
                            self.lifeline_views
                                .insert(view_pos.try_into().unwrap(), view.clone());
                        }
                    }
                    if (*bucket == 0 || *bucket == HORIZONTALS_BUCKET)
                        && let Ok(mut view) = UmlSequenceElementView::try_from(element.clone())
                            .and_then(|v| v.as_horizontal().ok_or(()))
                        && let Some(model_pos) = w
                            .get_element_pos(&view.model_uuid())
                            .map(|e| e.1)
                            .or_else(|| {
                                if *into_model {
                                    w.insert_element(*bucket, *position, view.model()).ok()
                                } else {
                                    None
                                }
                            })
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

                        let view_pos = {
                            let mut view_pos: PositionNoT = 0;
                            for e in &self.horizontal_element_views {
                                let Some((_b, pos)) = w.get_element_pos(&e.model_uuid()) else {
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
                        self.horizontal_element_views
                            .insert(view_pos.try_into().unwrap(), view);
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
                    if (*bucket == 0 || *bucket == VERTICALS_BUCKET)
                        && let Some(view) = self
                            .lifeline_views
                            .iter()
                            .find(|v| *v.read().uuid == *element)
                            .cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.read().model_uuid())
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            position: Some(pos),
                            element: UmlSequenceElementView::from(view.clone()).into(),
                            into_model: *including_model,
                        });

                        if *including_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.lifeline_views.retain(|v| *v.read().uuid != *element);
                    }
                    if (*bucket == 0 || *bucket == HORIZONTALS_BUCKET)
                        && let Some(view) = self
                            .horizontal_element_views
                            .iter()
                            .find(|v| *v.uuid() == *element)
                            .cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.model_uuid())
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            position: Some(pos),
                            element: view.clone().as_element_view().into(),
                            into_model: *including_model,
                        });

                        if *including_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.horizontal_element_views
                            .retain(|v| *v.uuid() != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {}
            InsensitiveCommand::MoveOrdinal(uuids, direction) => {
                let mut undo_uuids = HashSet::new();
                match direction {
                    UmlSequenceOrdinalMovement::LifelineLeft
                    | UmlSequenceOrdinalMovement::LifelineRight => {
                        let lifelines_iter: Box<
                            dyn Iterator<Item = &mut ERef<UmlSequenceLifelineView>>,
                        > = match direction {
                            UmlSequenceOrdinalMovement::LifelineLeft => {
                                Box::new(self.lifeline_views.iter_mut())
                            }
                            UmlSequenceOrdinalMovement::LifelineRight => {
                                Box::new(self.lifeline_views.iter_mut().rev())
                            }
                            _ => unreachable!(),
                        };
                        let mut lifelines_iter = lifelines_iter.peekable();
                        while let Some(dest) = lifelines_iter.next()
                            && let Some(src) = lifelines_iter.peek_mut()
                        {
                            if uuids.contains(&src.read().uuid)
                                && !uuids.contains(&dest.read().uuid)
                            {
                                let mut w = self.model.write();
                                let Some(new_pos) = w.get_element_pos(&dest.read().model_uuid())
                                else {
                                    continue;
                                };
                                w.move_element(&src.read().model_uuid(), 0, new_pos.1);
                                undo_uuids.insert(*src.read().uuid);
                                std::mem::swap(dest, *src);
                            }
                        }
                    }
                    UmlSequenceOrdinalMovement::HorizontalUp
                    | UmlSequenceOrdinalMovement::HorizontalDown => {
                        let horizontal_elements_iter: Box<
                            dyn Iterator<Item = &mut UmlSequenceHorizontalElementView>,
                        > = match direction {
                            UmlSequenceOrdinalMovement::HorizontalUp => {
                                Box::new(self.horizontal_element_views.iter_mut())
                            }
                            UmlSequenceOrdinalMovement::HorizontalDown => {
                                Box::new(self.horizontal_element_views.iter_mut().rev())
                            }
                            _ => unreachable!(),
                        };
                        let mut horizontal_elements_iter = horizontal_elements_iter.peekable();
                        while let Some(dest) = horizontal_elements_iter.next()
                            && let Some(src) = horizontal_elements_iter.peek_mut()
                        {
                            if uuids.contains(&src.uuid()) && !uuids.contains(&dest.uuid()) {
                                let mut w = self.model.write();
                                let Some(new_pos) = w.get_element_pos(&dest.model_uuid()) else {
                                    continue;
                                };
                                w.move_element(&src.model_uuid(), 1, new_pos.1);
                                undo_uuids.insert(*src.uuid());
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
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    match property {
                        UmlSequencePropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlSequencePropChange::ShowActivationsChange(b) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::ShowActivationsChange(self.show_activations),
                            ));
                            self.show_activations = *b;
                        }
                        UmlSequencePropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
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

        self.temporaries.display_text = format!("sd: {}", r.name);
        self.temporaries.name_buffer = (*r.name).clone();
        self.temporaries.comment_buffer = (*r.comment).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model.read().uuid, *self.uuid);

        self.temporaries.all_elements.clear();
        self.lifeline_views.iter_mut().for_each(|v| {
            v.write().head_count(
                flattened_views,
                &mut self.temporaries.all_elements,
                flattened_represented_models,
            )
        });
        self.horizontal_element_views.iter_mut().for_each(|v| {
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

        self.lifeline_views.iter().for_each(|v| {
            flattened_views.insert(*v.read().uuid, (v.clone().into(), *self.uuid));
        });
        self.horizontal_element_views.iter().for_each(|v| {
            flattened_views.insert(*v.uuid(), (v.clone().as_element_view(), *self.uuid));
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        c: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.lifeline_views
                .iter()
                .for_each(|v| v.read().deep_copy_walk(requested, uuid_present, tlc, c, m));
            self.horizontal_element_views
                .iter()
                .for_each(|v| v.deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        c: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlSequenceElement::Diagram(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut inner = HashMap::new();
        let lifeline_views = self
            .lifeline_views
            .iter()
            .map(|v| {
                let r = v.read();
                r.deep_copy_clone(uuid_present, &mut inner, c, m);
                match c.get(&r.uuid) {
                    Some(UmlSequenceElementView::Lifeline(l)) => l.clone(),
                    _ => v.clone(),
                }
            })
            .collect();
        let horizontal_element_views = self
            .horizontal_element_views
            .iter()
            .map(|v| {
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                match c.get(&v.uuid()).and_then(|v| v.clone().as_horizontal()) {
                    Some(e) => e.clone(),
                    _ => v.clone(),
                }
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,

            lifeline_views,
            horizontal_element_views,

            temporaries: self.temporaries.clone(),
            bounds_rect: self.bounds_rect,
            show_activations: self.show_activations,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, UmlSequenceElementView>,
        m: &HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        self.lifeline_views
            .iter_mut()
            .for_each(|v| v.write().deep_copy_relink(c, m));
        self.horizontal_element_views
            .iter_mut()
            .for_each(|v| v.deep_copy_relink(c, m));

        let mut w = self.model.write();
        for e in w.vertical_elements.iter_mut() {
            let id = *e.read().uuid();
            if let Some(UmlSequenceElement::Lifeline(new_model)) = m.get(&id) {
                *e = new_model.clone();
            }
        }
        for e in w.horizontal_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()).and_then(|e| e.clone().as_horizontal()) {
                *e = new_model;
            }
        }
    }
}

pub fn new_umlsequence_combinedfragment(
    kind: UmlSequenceCombinedFragmentKind,
    kind_argument: &str,
    end_behaviour: UmlSequenceActivationBehaviour,
    horizontal_span: HashSet<ModelUuid>,
    sections: Vec<(
        ERef<UmlSequenceCombinedFragmentSection>,
        ERef<UmlSequenceCombinedFragmentSectionView>,
    )>,
) -> (
    ERef<UmlSequenceCombinedFragment>,
    ERef<UmlSequenceCombinedFragmentView>,
) {
    let (section_models, section_views) = sections.into_iter().collect();
    let package_model = ERef::new(UmlSequenceCombinedFragment::new(
        ModelUuid::now_v7(),
        kind,
        kind_argument.to_owned(),
        end_behaviour,
        horizontal_span,
        section_models,
    ));
    let package_view = new_umlsequence_combinedfragment_view(package_model.clone(), section_views);

    (package_model, package_view)
}
pub fn new_umlsequence_combinedfragment_view(
    model: ERef<UmlSequenceCombinedFragment>,
    sections: Vec<ERef<UmlSequenceCombinedFragmentSectionView>>,
) -> ERef<UmlSequenceCombinedFragmentView> {
    ERef::new(UmlSequenceCombinedFragmentView {
        uuid: ViewUuid::now_v7().into(),
        model,
        sections,
        bounds_rect: egui::Rect::ZERO,
        left_top_pentagon_rect: egui::Rect::ZERO,
        background_color: MGlobalColor::None,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceCombinedFragmentView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceCombinedFragment>,
    #[nh_context_serde(entity)]
    sections: Vec<ERef<UmlSequenceCombinedFragmentSectionView>>,

    pub bounds_rect: egui::Rect,
    left_top_pentagon_rect: egui::Rect,
    background_color: MGlobalColor,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlSequenceCombinedFragmentViewTemporaries,
}

#[derive(Clone, Default)]
struct UmlSequenceCombinedFragmentViewTemporaries {
    display_text: String,
    kind_buffer: UmlSequenceCombinedFragmentKind,
    kind_argument_buffer: String,
    end_behaviour_buffer: UmlSequenceActivationBehaviour,
    comment_buffer: String,

    spanned_lifelines: HashSet<ViewUuid>,
    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
}

impl UmlSequenceCombinedFragmentView {
    const COMBINED_FRAGMENT_MARGIN_BOTTOM: f32 = 10.0;
    const BUTTON_RADIUS: f32 = 8.0;

    fn new_section_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_center =
            self.bounds_rect.right_top() + egui::Vec2::splat(Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
    fn new_section_command(
        &self,
    ) -> InsensitiveCommand<
        UmlSequenceOrdinalMovement,
        UmlSequenceElementOrVertex,
        UmlSequencePropChange,
    > {
        let v: UmlSequenceElementView = new_umlsequence_combinedfragmentsection("", Vec::new())
            .1
            .into();
        InsensitiveCommand::AddDependency {
            target: *self.uuid,
            bucket: HORIZONTALS_BUCKET,
            position: None,
            element: v.into(),
            into_model: true,
        }
    }

    fn spanned_lifeline_views<'b>(
        &self,
        lifeline_views: &'b [ERef<UmlSequenceLifelineView>],
    ) -> &'b [ERef<UmlSequenceLifelineView>] {
        let r = self.model.read();
        let start = lifeline_views
            .iter()
            .enumerate()
            .find(|e| r.horizontal_span.contains(&e.1.read().model_uuid()))
            .map(|e| e.0)
            .unwrap_or(0);
        let end = lifeline_views
            .iter()
            .enumerate()
            .rev()
            .find(|e| r.horizontal_span.contains(&e.1.read().model_uuid()))
            .map(|e| e.0 + 1)
            .unwrap_or(lifeline_views.len());
        &lifeline_views[start..end]
    }

    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        message_offsets: &HashMap<ViewUuid, (usize, usize)>,
        pos_y: f32,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        let spanned_lifeline_views = self.spanned_lifeline_views(lifeline_views);
        self.temporaries.spanned_lifelines = spanned_lifeline_views
            .iter()
            .map(|e| *e.read().uuid)
            .collect();
        let span_x = (
            spanned_lifeline_views
                .first()
                .map(|e| e.read().bounds_rect.center().x)
                .unwrap_or(0.0),
            spanned_lifeline_views
                .last()
                .map(|e| e.read().bounds_rect.center().x)
                .unwrap_or(0.0),
        );

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;
        let mut section_offsets = vec![pos_y];
        let mut acc = egui::Rect::from_min_size(egui::Pos2::new(span_x.0, pos_y), egui::Vec2::ZERO);
        for e in self.sections.iter_mut() {
            let (t, r) = e.write().draw_inner(
                spanned_lifeline_views,
                span_x,
                message_offsets,
                acc.max.y,
                q,
                context,
                settings,
                canvas,
                tool,
            );
            if t != TargettingStatus::NotDrawn {
                drawn_child_targetting = t;
            }
            section_offsets.push(r.max.y);
            acc = acc.union(r);
        }

        for (idx, e) in self.sections.iter_mut().enumerate() {
            let mut w = e.write();
            w.bounds_rect = acc
                .with_min_y(section_offsets[idx])
                .with_max_y(section_offsets[idx + 1]);

            canvas.draw_line(
                [
                    egui::Pos2::new(acc.min.x, section_offsets[idx + 1]),
                    egui::Pos2::new(acc.max.x, section_offsets[idx + 1]),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );

            if w.temporaries.highlight != canvas::Highlight::NONE {
                canvas.draw_rectangle(
                    w.bounds_rect.shrink(2.0),
                    egui::CornerRadius::same(2),
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    w.temporaries.highlight,
                );
            }
        }

        self.bounds_rect = acc;
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );

        // Draw top left pentagon
        const PENTAGON_PADDING: f32 = 4.0;
        let pentagon_bg = egui::Color32::WHITE;
        self.left_top_pentagon_rect = canvas
            .measure_text(
                egui::Pos2::new(
                    self.bounds_rect.min.x + PENTAGON_PADDING,
                    pos_y + PENTAGON_PADDING,
                ),
                egui::Align2::LEFT_TOP,
                &self.temporaries.display_text,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .expand(PENTAGON_PADDING);
        canvas.draw_polygon(
            [
                self.left_top_pentagon_rect.left_top(),
                self.left_top_pentagon_rect.right_top(),
                self.left_top_pentagon_rect.right_bottom() - egui::Vec2::new(0.0, PENTAGON_PADDING),
                self.left_top_pentagon_rect.right_bottom() - egui::Vec2::new(PENTAGON_PADDING, 0.0),
                self.left_top_pentagon_rect.left_bottom(),
            ]
            .to_vec(),
            pentagon_bg,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );
        canvas.draw_text(
            egui::Pos2::new(
                self.bounds_rect.min.x + PENTAGON_PADDING,
                pos_y + PENTAGON_PADDING,
            ),
            egui::Align2::LEFT_TOP,
            &self.temporaries.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw buttons
        if self.temporaries.highlight.selected
            && let Some(ui_scale) = canvas.ui_scale()
        {
            let b1 = self.new_section_button_rect(ui_scale);
            canvas.draw_rectangle(
                b1,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(
                b1.center(),
                egui::Align2::CENTER_CENTER,
                "+",
                14.0 / ui_scale,
                egui::Color32::BLACK,
            );
        }

        (
            drawn_child_targetting,
            self.bounds_rect
                .with_max_y(self.bounds_rect.max.y + Self::COMBINED_FRAGMENT_MARGIN_BOTTOM),
        )
    }

    fn handle_event_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos)
                if self.temporaries.highlight.selected
                    && self.new_section_button_rect(ehc.ui_scale).contains(pos) =>
            {
                commands.push(self.new_section_command());

                EventHandlingStatus::HandledByContainer
            }
            InputEvent::Click(pos) if self.left_top_pentagon_rect.contains(pos) => {
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.bounds_rect.contains(pos) => {
                let spanned_lifeline_views = self.spanned_lifeline_views(lifeline_views);
                let k_status = self
                    .sections
                    .iter()
                    .map(|e| {
                        let mut w = e.write();
                        (
                            *w.uuid,
                            w.handle_event_inner(
                                spanned_lifeline_views,
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
                                !self.temporaries.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ));
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }
}

impl Entity for UmlSequenceCombinedFragmentView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceCombinedFragmentView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceCombinedFragmentView {
    fn model(&self) -> UmlSequenceElement {
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

fn combined_fragment_display_text(
    kind: UmlSequenceCombinedFragmentKind,
    kind_argument: &str,
) -> String {
    match kind {
        UmlSequenceCombinedFragmentKind::Loop if !kind_argument.is_empty() => {
            format!("{}({})", kind.as_str(), kind_argument)
        }
        UmlSequenceCombinedFragmentKind::Ignore | UmlSequenceCombinedFragmentKind::Consider
            if !kind_argument.is_empty() =>
        {
            format!("{}{{{}}}", kind.as_str(), kind_argument)
        }
        a => a.as_str().to_owned(),
    }
}

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceCombinedFragmentView {
    fn draw_in(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlSequenceDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(
            &Vec::new(),
            &HashMap::new(),
            0.0,
            q,
            context,
            settings,
            canvas,
            tool,
        );
        TargettingStatus::NotDrawn
    }

    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if let Some(child) = self
            .sections
            .iter_mut()
            .filter_map(|v| {
                v.write()
                    .show_properties(drawing_context, q, ui, commands)
                    .non_default()
            })
            .next()
        {
            return child;
        }

        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("Kind:")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlSequenceCombinedFragmentKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::CombinedFragmentKindChange(
                                self.temporaries.kind_buffer,
                            ),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline(
                "Kind argument:",
                &mut self.temporaries.kind_argument_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CombinedFragmentKindArgumentChange(Arc::new(
                    self.temporaries.kind_argument_buffer.clone(),
                )),
            ));
        }

        ui.label("End behaviour:");
        egui::ComboBox::from_id_salt("End behaviour:")
            .selected_text(self.temporaries.end_behaviour_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlSequenceActivationBehaviour::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.end_behaviour_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::ActivationsBehaviourChange(
                                self.temporaries.end_behaviour_buffer,
                            ),
                        ));
                    }
                }
            });

        if ui.button("Add section").clicked() {
            commands.push(self.new_section_command());
        }
        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalDown,
                ));
            }
        });

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CommentChange(Arc::new(
                    self.temporaries.comment_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }

    fn handle_event(
        &mut self,
        _event: InputEvent,
        _ehc: &EventHandlingContext,
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        unreachable!()
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<
            <UmlSequenceDomain as Domain>::OrdinalMovementT,
            <UmlSequenceDomain as Domain>::AddCommandElementT,
            <UmlSequenceDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.sections.iter().for_each(|s| {
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
                                self.sections.iter().map(|v| *v.read().uuid).collect();
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
                        .sections
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
                self.sections.iter_mut().for_each(|v| {
                    v.write().apply_command(
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
                for element in self
                    .sections
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
                        element: UmlSequenceElementView::from(element.clone()).into(),
                        into_model: false,
                    });
                }
                self.sections.retain(|v| !uuids.contains(&v.read().uuid));

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
                    if (*bucket == 0 || *bucket == HORIZONTALS_BUCKET)
                        && let Ok(UmlSequenceElementView::CombinedFragmentSection(view)) =
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
                            vw.head_count(
                                &mut HashMap::new(),
                                &mut HashMap::new(),
                                &mut model_transitives,
                            );
                            affected_models.extend(model_transitives.into_keys());

                            let view_pos = {
                                let mut view_pos: PositionNoT = 0;
                                for e in &self.sections {
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
                            self.sections
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
                    if (*bucket == 0 || *bucket == HORIZONTALS_BUCKET)
                        && let Some(view) = self
                            .sections
                            .iter()
                            .find(|v| *v.read().uuid == *element)
                            .cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.read().model_uuid())
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            position: Some(pos),
                            element: UmlSequenceElementView::from(view.clone()).into(),
                            into_model: *including_model,
                        });

                        if *including_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.sections.retain(|v| *v.read().uuid != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {}
            InsensitiveCommand::MoveOrdinal(uuids, direction) => {
                if let UmlSequenceOrdinalMovement::HorizontalUp
                | UmlSequenceOrdinalMovement::HorizontalDown = direction
                {
                    let mut undo_uuids = HashSet::new();
                    let sections_iter: Box<
                        dyn Iterator<Item = &mut ERef<UmlSequenceCombinedFragmentSectionView>>,
                    > = match direction {
                        UmlSequenceOrdinalMovement::HorizontalUp => {
                            Box::new(self.sections.iter_mut())
                        }
                        UmlSequenceOrdinalMovement::HorizontalDown => {
                            Box::new(self.sections.iter_mut().rev())
                        }
                        _ => unreachable!(),
                    };
                    let mut sections_iter = sections_iter.peekable();
                    while let Some(dest) = sections_iter.next()
                        && let Some(src) = sections_iter.peek_mut()
                    {
                        if uuids.contains(&src.read().uuid) && !uuids.contains(&dest.read().uuid) {
                            let mut w = self.model.write();
                            let Some(new_pos) = w.get_element_pos(&dest.read().model_uuid()) else {
                                continue;
                            };
                            w.move_element(&src.read().model_uuid(), 1, new_pos.1);
                            undo_uuids.insert(*src.read().uuid);
                            std::mem::swap(dest, *src);
                        }
                    }
                    if !undo_uuids.is_empty() {
                        undo_accumulator.push(InsensitiveCommand::MoveOrdinal(
                            undo_uuids,
                            direction.inverse(),
                        ));
                    }
                }

                recurse!();
            }
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    match property {
                        UmlSequencePropChange::CombinedFragmentKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CombinedFragmentKindChange(model.kind),
                            ));
                            model.kind = *kind;
                        }
                        UmlSequencePropChange::CombinedFragmentKindArgumentChange(
                            kind_argument,
                        ) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CombinedFragmentKindArgumentChange(
                                    model.kind_argument.clone(),
                                ),
                            ));
                            model.kind_argument = kind_argument.clone();
                        }
                        UmlSequencePropChange::ActivationsBehaviourChange(ab) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::ActivationsBehaviourChange(
                                    model.end_behaviour,
                                ),
                            ));
                            model.end_behaviour = *ab;
                        }
                        UmlSequencePropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
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

        self.temporaries.display_text =
            combined_fragment_display_text(model.kind, &model.kind_argument);
        self.temporaries.kind_buffer = model.kind;
        self.temporaries.kind_argument_buffer = (*model.kind_argument).clone();
        self.temporaries.end_behaviour_buffer = model.end_behaviour;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.sections.iter().for_each(|s| {
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
        tlc: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.sections
                .iter()
                .for_each(|v| v.read().deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(UmlSequenceElement::CombinedFragment(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut inner = HashMap::new();
        let new_sections = self
            .sections
            .iter()
            .map(|v| {
                let v = v.read();
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                let Some(UmlSequenceElementView::CombinedFragmentSection(s)) = c.get(&v.uuid)
                else {
                    unreachable!()
                };
                s.clone()
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            sections: new_sections,

            bounds_rect: self.bounds_rect,
            left_top_pentagon_rect: self.left_top_pentagon_rect,
            background_color: self.background_color,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        self.sections
            .iter_mut()
            .for_each(|v| v.write().deep_copy_relink(c, m));

        let mut w = self.model.write();
        for e in w.sections.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlSequenceElement::CombinedFragmentSection(new_model)) = m.get(&uuid) {
                *e = new_model.clone();
            }
        }
    }
}

pub fn new_umlsequence_combinedfragmentsection(
    guard: &str,
    horizontals: Vec<(
        UmlSequenceHorizontalElement,
        UmlSequenceHorizontalElementView,
    )>,
) -> (
    ERef<UmlSequenceCombinedFragmentSection>,
    ERef<UmlSequenceCombinedFragmentSectionView>,
) {
    let (child_models, child_views) = horizontals.into_iter().collect();
    let section_model = ERef::new(UmlSequenceCombinedFragmentSection::new(
        ModelUuid::now_v7(),
        guard.to_owned(),
        child_models,
    ));
    let section_view =
        new_umlsequence_combinedfragmentsection_view(section_model.clone(), child_views);

    (section_model, section_view)
}
pub fn new_umlsequence_combinedfragmentsection_view(
    model: ERef<UmlSequenceCombinedFragmentSection>,
    horizontal_element_views: Vec<UmlSequenceHorizontalElementView>,
) -> ERef<UmlSequenceCombinedFragmentSectionView> {
    ERef::new(UmlSequenceCombinedFragmentSectionView {
        uuid: ViewUuid::now_v7().into(),
        model,
        horizontal_element_views,
        bounds_rect: egui::Rect::ZERO,
        background_color: MGlobalColor::None,
        temporaries: Default::default(),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceCombinedFragmentSectionView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceCombinedFragmentSection>,
    #[nh_context_serde(entity)]
    horizontal_element_views: Vec<UmlSequenceHorizontalElementView>,

    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlSequenceCombinedFragmentSectionViewTemporaries,
}

#[derive(Clone, Default)]
struct UmlSequenceCombinedFragmentSectionViewTemporaries {
    highlight: Highlight,
    selected_direct_elements: HashSet<ViewUuid>,

    display_text: String,
    guard_buffer: String,
}

impl UmlSequenceCombinedFragmentSectionView {
    const SECTION_PADDING_X: f32 = 20.0;
    const SECTION_EMPTY_SIZE_Y: f32 = 60.0;
    const SECTION_PADDING_Y: f32 = 30.0;

    // Does not draw the outer rectangle, because it doesn't know the sizes of sibling sections
    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        (min_lifeline_x, max_lifeline_x): (f32, f32),
        message_offsets: &HashMap<ViewUuid, (usize, usize)>,
        pos_y: f32,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        canvas.draw_text(
            egui::Pos2::new(self.bounds_rect.center().x, pos_y + 2.0),
            egui::Align2::CENTER_TOP,
            &self.temporaries.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        let (drawn_child_targetting, rect) = if self.horizontal_element_views.is_empty() {
            (
                TargettingStatus::NotDrawn,
                egui::Rect::from_two_pos(
                    egui::Pos2::new(min_lifeline_x - Self::SECTION_PADDING_X, pos_y),
                    egui::Pos2::new(
                        max_lifeline_x + Self::SECTION_PADDING_X,
                        pos_y + (Self::SECTION_PADDING_Y + Self::SECTION_EMPTY_SIZE_Y),
                    ),
                ),
            )
        } else {
            let mut drawn_child_targetting = TargettingStatus::NotDrawn;
            let mut acc = egui::Rect::from_min_max(
                egui::Pos2::new(min_lifeline_x, pos_y),
                egui::Pos2::new(max_lifeline_x, pos_y + Self::SECTION_PADDING_Y),
            );
            for e in self.horizontal_element_views.iter_mut() {
                let (t, r) = e.draw_inner(
                    lifeline_views,
                    message_offsets,
                    acc.max.y,
                    q,
                    context,
                    settings,
                    canvas,
                    tool,
                );
                if t != TargettingStatus::NotDrawn {
                    drawn_child_targetting = t;
                }
                acc = acc.union(r);
            }
            (
                drawn_child_targetting,
                acc.expand2(egui::Vec2::new(Self::SECTION_PADDING_X, 0.0)),
            )
        };

        if canvas.ui_scale().is_none() {
            return (drawn_child_targetting, rect);
        }

        match (drawn_child_targetting, tool) {
            (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                match t.current_stage {
                    UmlSequenceToolStage::LinkStart { .. }
                    | UmlSequenceToolStage::LinkEnd
                    | UmlSequenceToolStage::CombinedFragmentStart { .. }
                    | UmlSequenceToolStage::CombinedFragmentEnd
                    | UmlSequenceToolStage::RefStart { .. }
                    | UmlSequenceToolStage::RefEnd => {
                        if let Some((.., lr, hr)) =
                            self.horizontal_insertion_place(lifeline_views, *pos)
                        {
                            canvas.draw_rectangle(
                                lr,
                                egui::CornerRadius::ZERO,
                                t.targetting_for_section(Some(self.model.clone().into())),
                                canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                                canvas::Highlight::NONE,
                            );
                            canvas.draw_rectangle(
                                hr,
                                egui::CornerRadius::ZERO,
                                t.targetting_for_section(Some(self.model.clone().into())),
                                canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                                canvas::Highlight::NONE,
                            );
                        }
                    }
                    _ => {
                        canvas.draw_rectangle(
                            self.bounds_rect,
                            egui::CornerRadius::ZERO,
                            t.targetting_for_section(Some(self.model.clone().into())),
                            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                            canvas::Highlight::NONE,
                        );
                    }
                }

                (TargettingStatus::Drawn, rect)
            }
            _ => (drawn_child_targetting, rect),
        }
    }

    fn handle_event_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.bounds_rect.contains(pos) => {
                let k_status = self
                    .horizontal_element_views
                    .iter_mut()
                    .map(|e| {
                        (
                            *e.uuid(),
                            e.handle_event_inner(
                                lifeline_views,
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

                if let Some(tool) = tool {
                    let horizontal_place = self.horizontal_insertion_place(lifeline_views, pos);
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());
                    if let Some(h) = &horizontal_place {
                        tool.add_section(h.1.clone().into());

                        if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, h.0, commands)
                            && ehc
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
                                !self.temporaries.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ));
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    // TODO: deduplicate?
    fn horizontal_insertion_place(
        &self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        pos: egui::Pos2,
    ) -> Option<(
        Option<PositionNoT>,
        ERef<UmlSequenceLifeline>,
        egui::Rect,
        egui::Rect,
    )> {
        let (lifeline_center, lifeline) = 'a: {
            for e in lifeline_views.iter() {
                let r = e.read();
                let rect = r.min_shape().bounding_box();
                if rect.min.x < pos.x && pos.x < rect.max.x {
                    break 'a (r.bounds_rect.center().x, r.model.clone());
                }
            }
            return None;
        };

        let mut insertion_index = Option::<PositionNoT>::None;
        let mut nearest_before = Option::<f32>::None;
        let mut nearest_after = Option::<f32>::None;

        for (idx, v) in self.horizontal_element_views.iter().enumerate() {
            let shape_bb = v.min_shape().bounding_box();
            let (min_y, max_y) = (shape_bb.min.y, shape_bb.max.y);
            if min_y < pos.y && pos.y < max_y {
                return None;
            }
            if max_y < pos.y && nearest_before.is_none_or(|e| e < max_y) {
                nearest_before = Some(max_y);
            }
            if min_y > pos.y && nearest_after.is_none_or(|e| e > min_y) {
                insertion_index = Some(idx.try_into().unwrap());
                nearest_after = Some(min_y);
            }
        }

        let nearest_before = nearest_before.unwrap_or(self.bounds_rect.min.y);
        let nearest_after = nearest_after.unwrap_or(self.bounds_rect.max.y);
        let nearest_average = (nearest_before + nearest_after) / 2.0;

        const WIDTH: f32 = 20.0;
        let lifeline_rect = egui::Rect::from_center_size(
            egui::Pos2::new(lifeline_center, nearest_average),
            egui::Vec2::new(WIDTH, nearest_after - nearest_before),
        );
        let horizontal_rect = self
            .bounds_rect
            .with_min_y(nearest_average - WIDTH / 2.0)
            .with_max_y(nearest_average + WIDTH / 2.0);

        Some((insertion_index, lifeline, lifeline_rect, horizontal_rect))
    }
}

impl Entity for UmlSequenceCombinedFragmentSectionView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceCombinedFragmentSectionView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceCombinedFragmentSectionView {
    fn model(&self) -> UmlSequenceElement {
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

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceCombinedFragmentSectionView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if let Some(child) = self
            .horizontal_element_views
            .iter_mut()
            .filter_map(|v| {
                v.show_properties(drawing_context, q, ui, commands)
                    .non_default()
            })
            .next()
        {
            return child;
        }

        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if ui
            .labeled_text_edit_singleline("Guard:", &mut self.temporaries.guard_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CombinedFragmentSectionGuardChange(
                    self.temporaries.guard_buffer.clone().into(),
                ),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalDown,
                ));
            }
        });

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> TargettingStatus {
        self.draw_inner(
            &Vec::new(),
            (0.0, 100.0),
            &HashMap::new(),
            0.0,
            q,
            context,
            settings,
            canvas,
            tool,
        )
        .0
    }

    fn handle_event(
        &mut self,
        _event: InputEvent,
        _ehc: &EventHandlingContext,
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<NaiveUmlSequenceTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        unreachable!()
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<
            UmlSequenceOrdinalMovement,
            UmlSequenceElementOrVertex,
            UmlSequencePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.horizontal_element_views
                    .iter_mut()
                    .for_each(|v| v.apply_command(command, undo_accumulator, affected_models));
            };
        }
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                if h.selected {
                    match set {
                        true => {
                            self.temporaries.selected_direct_elements = self
                                .horizontal_element_views
                                .iter()
                                .map(|v| *v.uuid())
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
                        .horizontal_element_views
                        .iter()
                        .map(|v| *v.uuid())
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
                self.horizontal_element_views.iter_mut().for_each(|v| {
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
                for element in self
                    .horizontal_element_views
                    .iter()
                    .filter(|v| uuids.contains(&v.uuid()))
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
                        element: element.clone().as_element_view().into(),
                        into_model: false,
                    });
                }
                self.horizontal_element_views
                    .retain(|v| !uuids.contains(&v.uuid()));

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
                    if (*bucket == 0 || *bucket == HORIZONTALS_BUCKET)
                        && let Ok(mut view) = UmlSequenceElementView::try_from(element.clone())
                            .and_then(|v| v.as_horizontal().ok_or(()))
                        && let Some(model_pos) = w
                            .get_element_pos(&view.model_uuid())
                            .map(|e| e.1)
                            .or_else(|| {
                                if *into_model {
                                    w.insert_element(*bucket, *position, view.model()).ok()
                                } else {
                                    None
                                }
                            })
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

                        let view_pos = {
                            let mut view_pos: PositionNoT = 0;
                            for e in &self.horizontal_element_views {
                                let Some((_b, pos)) = w.get_element_pos(&e.model_uuid()) else {
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
                        self.horizontal_element_views
                            .insert(view_pos.try_into().unwrap(), view.clone());
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
                    if (*bucket == 0 || *bucket == HORIZONTALS_BUCKET)
                        && let Some(view) = self
                            .horizontal_element_views
                            .iter()
                            .find(|v| *v.uuid() == *element)
                            .cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.model_uuid())
                    {
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            position: Some(pos),
                            element: view.clone().as_element_view().into(),
                            into_model: *including_model,
                        });

                        if *including_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.horizontal_element_views
                            .retain(|v| *v.uuid() != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {}
            InsensitiveCommand::MoveOrdinal(uuids, direction) => {
                if let UmlSequenceOrdinalMovement::HorizontalUp
                | UmlSequenceOrdinalMovement::HorizontalDown = direction
                {
                    let mut undo_uuids = HashSet::new();
                    let horizontal_elements_iter: Box<
                        dyn Iterator<Item = &mut UmlSequenceHorizontalElementView>,
                    > = match direction {
                        UmlSequenceOrdinalMovement::HorizontalUp => {
                            Box::new(self.horizontal_element_views.iter_mut())
                        }
                        UmlSequenceOrdinalMovement::HorizontalDown => {
                            Box::new(self.horizontal_element_views.iter_mut().rev())
                        }
                        _ => unreachable!(),
                    };
                    let mut horizontal_elements_iter = horizontal_elements_iter.peekable();
                    while let Some(dest) = horizontal_elements_iter.next()
                        && let Some(src) = horizontal_elements_iter.peek_mut()
                    {
                        if uuids.contains(&src.uuid()) && !uuids.contains(&dest.uuid()) {
                            let mut w = self.model.write();
                            let Some(new_pos) = w.get_element_pos(&dest.model_uuid()) else {
                                continue;
                            };
                            w.move_element(&src.model_uuid(), 1, new_pos.1);
                            undo_uuids.insert(*src.uuid());
                            std::mem::swap(dest, *src);
                        }
                    }
                    if !undo_uuids.is_empty() {
                        undo_accumulator.push(InsensitiveCommand::MoveOrdinal(
                            undo_uuids,
                            direction.inverse(),
                        ));
                    }
                }

                recurse!();
            }
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    if let UmlSequencePropChange::CombinedFragmentSectionGuardChange(guard) =
                        property
                    {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::CombinedFragmentSectionGuardChange(
                                model.guard.clone(),
                            ),
                        ));
                        model.guard = guard.clone();
                    }
                }
                recurse!();
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {
        let r = self.model.read();

        self.temporaries.display_text = if r.guard.is_empty() {
            "".to_owned()
        } else {
            format!("[{}]", r.guard)
        };
        self.temporaries.guard_buffer = (*r.guard).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.horizontal_element_views.iter_mut().for_each(|v| {
            v.head_count(
                flattened_views,
                flattened_views_status,
                flattened_represented_models,
            );
            flattened_views.insert(*v.uuid(), (v.clone().as_element_view(), *self.uuid));
        });
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        c: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model =
            if let Some(UmlSequenceElement::CombinedFragmentSection(m)) = m.get(&old_model.uuid) {
                m.clone()
            } else {
                let modelish = old_model.clone_with(model_uuid);
                m.insert(*old_model.uuid, modelish.clone().into());
                modelish
            };

        let mut inner = HashMap::new();
        let horizontal_element_views = self
            .horizontal_element_views
            .iter()
            .map(|v| {
                v.deep_copy_clone(uuid_present, &mut inner, c, m);
                let Some(e) = c.get(&v.uuid()).and_then(|e| e.clone().as_horizontal()) else {
                    unreachable!()
                };
                e.clone()
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            horizontal_element_views,

            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        self.horizontal_element_views
            .iter_mut()
            .for_each(|v| v.deep_copy_relink(c, m));

        let mut w = self.model.write();
        for e in w.horizontal_elements.iter_mut() {
            let uuid = *e.uuid();
            if let Some(new_model) = m.get(&uuid).and_then(|e| e.as_horizontal()) {
                *e = new_model.clone();
            }
        }
    }
}

pub fn new_umlsequence_lifeline(
    name: &str,
    stereotype: &str,
    render_style: UmlSequenceLifelineRenderStyle,
    background_color: MGlobalColor,
) -> (ERef<UmlSequenceLifeline>, ERef<UmlSequenceLifelineView>) {
    let class_model = ERef::new(UmlSequenceLifeline::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        stereotype.to_owned(),
    ));
    let class_view =
        new_umlsequence_lifeline_view(class_model.clone(), render_style, background_color);

    (class_model, class_view)
}
pub fn new_umlsequence_lifeline_view(
    model: ERef<UmlSequenceLifeline>,
    render_style: UmlSequenceLifelineRenderStyle,
    background_color: MGlobalColor,
) -> ERef<UmlSequenceLifelineView> {
    let m = model.read();
    ERef::new(UmlSequenceLifelineView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        stereotype_in_guillemets: None,
        stereotype_buffer: (*m.stereotype).clone(),
        name_buffer: (*m.name).clone(),
        comment_buffer: (*m.comment).clone(),

        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::ZERO,
        background_color,
        render_style,
    })
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlSequenceLifelineRenderStyle {
    StickFigure,
    Object,
    Boundary,
    Control,
    Entity,
    Database,
}

impl UmlSequenceLifelineRenderStyle {
    const VARIANTS: [Self; 6] = [
        Self::StickFigure,
        Self::Object,
        Self::Boundary,
        Self::Control,
        Self::Entity,
        Self::Database,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            UmlSequenceLifelineRenderStyle::StickFigure => "Stick Figure",
            UmlSequenceLifelineRenderStyle::Object => "Object",
            UmlSequenceLifelineRenderStyle::Boundary => "Boundary",
            UmlSequenceLifelineRenderStyle::Control => "Control",
            UmlSequenceLifelineRenderStyle::Entity => "Entity",
            UmlSequenceLifelineRenderStyle::Database => "Database",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceLifelineView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlSequenceLifeline>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: Option<Arc<String>>,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
    render_style: UmlSequenceLifelineRenderStyle,
}

impl UmlSequenceLifelineView {
    fn draw_inner(
        &mut self,
        pos: egui::Pos2,
        max_y: f32,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> TargettingStatus {
        let read = self.model.read();

        let body_color = context
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
        let s = canvas::Stroke::new_solid(1.0, egui::Color32::BLACK);
        let h = self.highlight;
        match self.render_style {
            UmlSequenceLifelineRenderStyle::StickFigure => {
                canvas.draw_ellipse(
                    pos - egui::Vec2::new(0.0, 20.0),
                    egui::Vec2::splat(10.0),
                    body_color,
                    s,
                    h,
                );
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(20.0, 4.0),
                        pos - egui::Vec2::new(-20.0, 4.0),
                    ],
                    s,
                    h,
                ); // hands
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(0.0, 10.0),
                        pos - egui::Vec2::new(0.0, -8.0),
                    ],
                    s,
                    h,
                ); // torso
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(16.0, -28.0),
                        pos - egui::Vec2::new(0.0, -8.0),
                    ],
                    s,
                    h,
                ); // / leg
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(-16.0, -28.0),
                        pos - egui::Vec2::new(0.0, -8.0),
                    ],
                    s,
                    h,
                ); // \ leg

                self.bounds_rect = egui::Rect::from_min_max(
                    pos - egui::Vec2::new(20.0, 30.0),
                    pos + egui::Vec2::new(20.0, 28.0),
                );
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0),
                    egui::Align2::CENTER_TOP,
                    &read.name,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );

                canvas.draw_line(
                    [
                        pos + egui::Vec2::new(
                            0.0,
                            self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE,
                        ),
                        egui::Pos2::new(pos.x, max_y),
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    h,
                );
            }
            UmlSequenceLifelineRenderStyle::Object => {
                self.bounds_rect = draw_simple_uml_class(
                    canvas,
                    pos,
                    self.stereotype_in_guillemets.clone(),
                    &read.name,
                    None,
                    body_color,
                    s,
                    h,
                );

                canvas.draw_line(
                    [
                        self.bounds_rect.center_bottom(),
                        egui::Pos2::new(pos.x, max_y),
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    h,
                );
            }
            UmlSequenceLifelineRenderStyle::Boundary => {
                const CIRCLE_RADIUS: f32 = 15.0;
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, CIRCLE_RADIUS),
                        pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, -CIRCLE_RADIUS),
                    ],
                    s,
                    h,
                ); // vertical
                canvas.draw_line([pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, 0.0), pos], s, h); // horizontal
                canvas.draw_ellipse(
                    pos + egui::Vec2::new(CIRCLE_RADIUS, 0.0),
                    egui::Vec2::splat(CIRCLE_RADIUS),
                    body_color,
                    s,
                    h,
                );

                self.bounds_rect = egui::Rect::from_min_max(
                    pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, CIRCLE_RADIUS),
                    pos + egui::Vec2::new(2.0 * CIRCLE_RADIUS, CIRCLE_RADIUS),
                );
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0),
                    egui::Align2::CENTER_TOP,
                    &read.name,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );

                canvas.draw_line(
                    [
                        pos + egui::Vec2::new(
                            0.0,
                            self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE,
                        ),
                        egui::Pos2::new(pos.x, max_y),
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    h,
                );
            }
            UmlSequenceLifelineRenderStyle::Control => {
                const CIRCLE_RADIUS: f32 = 15.0;
                const ARROW_LENGTH: f32 = 5.0;
                canvas.draw_ellipse(pos, egui::Vec2::splat(CIRCLE_RADIUS), body_color, s, h);
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(0.0, CIRCLE_RADIUS),
                        pos - egui::Vec2::new(-ARROW_LENGTH, CIRCLE_RADIUS + ARROW_LENGTH),
                    ],
                    s,
                    h,
                ); // up
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(0.0, CIRCLE_RADIUS),
                        pos - egui::Vec2::new(-ARROW_LENGTH, CIRCLE_RADIUS - ARROW_LENGTH),
                    ],
                    s,
                    h,
                ); // down

                self.bounds_rect = egui::Rect::from_center_size(
                    pos,
                    egui::Vec2::new(2.0 * CIRCLE_RADIUS, 2.0 * CIRCLE_RADIUS),
                );
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0),
                    egui::Align2::CENTER_TOP,
                    &read.name,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );

                canvas.draw_line(
                    [
                        pos + egui::Vec2::new(
                            0.0,
                            self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE,
                        ),
                        egui::Pos2::new(pos.x, max_y),
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    h,
                );
            }
            UmlSequenceLifelineRenderStyle::Entity => {
                const CIRCLE_RADIUS: f32 = 15.0;
                canvas.draw_ellipse(pos, egui::Vec2::splat(CIRCLE_RADIUS), body_color, s, h);
                canvas.draw_line(
                    [
                        pos - egui::Vec2::new(CIRCLE_RADIUS, -CIRCLE_RADIUS - 1.0),
                        pos - egui::Vec2::new(-CIRCLE_RADIUS, -CIRCLE_RADIUS - 1.0),
                    ],
                    s,
                    h,
                ); // horizontal

                self.bounds_rect = egui::Rect::from_center_size(
                    pos,
                    egui::Vec2::new(2.0 * CIRCLE_RADIUS, 2.0 * CIRCLE_RADIUS),
                );
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0),
                    egui::Align2::CENTER_TOP,
                    &read.name,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );

                canvas.draw_line(
                    [
                        pos + egui::Vec2::new(
                            0.0,
                            self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE,
                        ),
                        egui::Pos2::new(pos.x, max_y),
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    h,
                );
            }
            UmlSequenceLifelineRenderStyle::Database => {
                const ELLIPSE_RADIUS: egui::Vec2 = egui::Vec2::new(20.0, 10.0);
                canvas.draw_ellipse(
                    pos + egui::Vec2::new(0.0, ELLIPSE_RADIUS.y),
                    ELLIPSE_RADIUS,
                    body_color,
                    s,
                    h,
                ); // bottom
                canvas.draw_rectangle(
                    egui::Rect::from_min_max(pos - ELLIPSE_RADIUS, pos + ELLIPSE_RADIUS),
                    egui::CornerRadius::ZERO,
                    body_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                ); // fill
                canvas.draw_line(
                    [
                        pos - ELLIPSE_RADIUS,
                        pos - egui::Vec2::new(ELLIPSE_RADIUS.x, -ELLIPSE_RADIUS.y),
                    ],
                    s,
                    h,
                ); // left
                canvas.draw_line(
                    [
                        pos + egui::Vec2::new(ELLIPSE_RADIUS.x, -ELLIPSE_RADIUS.y),
                        pos + ELLIPSE_RADIUS,
                    ],
                    s,
                    h,
                ); // right
                canvas.draw_ellipse(
                    pos - egui::Vec2::new(0.0, ELLIPSE_RADIUS.y),
                    ELLIPSE_RADIUS,
                    body_color,
                    s,
                    h,
                ); // top

                self.bounds_rect = egui::Rect::from_center_size(pos, 2.0 * ELLIPSE_RADIUS);
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0),
                    egui::Align2::CENTER_TOP,
                    &read.name,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );

                canvas.draw_line(
                    [
                        pos + egui::Vec2::new(
                            0.0,
                            self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE,
                        ),
                        egui::Pos2::new(pos.x, max_y),
                    ],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    h,
                );
            }
        }

        canvas.draw_header_text(
            canvas::HeaderLocation::Horizontal(self.bounds_rect.min.x..=self.bounds_rect.max.x),
            &self.name_buffer,
        );

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
}

impl Entity for UmlSequenceLifelineView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceLifelineView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceLifelineView {
    fn model(&self) -> UmlSequenceElement {
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

pub fn draw_simple_uml_class(
    canvas: &mut dyn canvas::NHCanvas,
    position: egui::Pos2,
    top_label: Option<Arc<String>>,
    main_label: &str,
    bottom_label: Option<Arc<String>>,
    fill: egui::Color32,
    stroke: canvas::Stroke,
    highlight: canvas::Highlight,
) -> egui::Rect {
    // Measure phase
    let (offsets, global_offset, rect) = {
        let mut offsets = vec![0.0];
        let mut max_width: f32 = 0.0;

        if let Some(top_label) = &top_label {
            let r = canvas.measure_text(
                egui::Pos2::ZERO,
                egui::Align2::CENTER_TOP,
                top_label,
                canvas::CLASS_TOP_FONT_SIZE,
            );
            offsets.push(r.height());
            max_width = max_width.max(r.width());
        }

        {
            let r = canvas.measure_text(
                egui::Pos2::ZERO,
                egui::Align2::CENTER_TOP,
                main_label,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            );
            offsets.push(r.height());
            max_width = max_width.max(r.width());
        }

        if let Some(bottom_label) = &bottom_label {
            let r = canvas.measure_text(
                egui::Pos2::ZERO,
                egui::Align2::CENTER_TOP,
                bottom_label,
                canvas::CLASS_BOTTOM_FONT_SIZE,
            );
            offsets.push(r.height());
            max_width = max_width.max(r.width());
        }

        // Process, draw bounds
        offsets.iter_mut().fold(0.0, |acc, x| {
            *x += acc;
            *x
        });
        let global_offset = offsets.last().unwrap() / 2.0;
        let rect = egui::Rect::from_center_size(
            position,
            egui::Vec2::new(max_width + 14.0, 2.0 * global_offset + 14.0),
        );
        canvas.draw_rectangle(rect, egui::CornerRadius::ZERO, fill, stroke, highlight);

        (offsets, global_offset, rect)
    };

    // Draw phase
    {
        let mut offset_counter = 0;

        if let Some(top_label) = &top_label {
            canvas.draw_text(
                position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                egui::Align2::CENTER_TOP,
                top_label,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
            offset_counter += 1;
        }

        {
            canvas.draw_text(
                position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                egui::Align2::CENTER_TOP,
                main_label,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
            offset_counter += 1;
        }

        if let Some(bottom_label) = &bottom_label {
            canvas.draw_text(
                position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                egui::Align2::CENTER_TOP,
                bottom_label,
                canvas::CLASS_BOTTOM_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }
    }

    rect
}

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceLifelineView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
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
                UmlSequencePropChange::StereotypeChange(self.stereotype_buffer.clone().into()),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move left").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::LifelineLeft,
                ));
            }
            if ui.button("Move right").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::LifelineRight,
                ));
            }
        });

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }

        ui.label("View properties");

        ui.label("Background color:");
        if let Some(new_color) = crate::common::controller::mglobalcolor_edit_button(
            drawing_context,
            ui,
            &self.background_color,
        ) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::ColorChange((0, new_color).into()),
            ));
        }

        ui.label("Render style");
        egui::ComboBox::from_id_salt("render style")
            .selected_text(self.render_style.as_str())
            .show_ui(ui, |ui| {
                for e in UmlSequenceLifelineRenderStyle::VARIANTS {
                    ui.selectable_value(&mut self.render_style, e, e.as_str());
                }
            });

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> TargettingStatus {
        self.draw_inner(
            self.bounds_rect.center(),
            self.bounds_rect.bottom() + self.bounds_rect.height(),
            q,
            context,
            settings,
            canvas,
            tool,
        )
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
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
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<
            UmlSequenceOrdinalMovement,
            UmlSequenceElementOrVertex,
            UmlSequencePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
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
                        UmlSequencePropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlSequencePropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlSequencePropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CommentChange(model.comment.clone()),
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

        self.stereotype_in_guillemets = if model.stereotype.is_empty() {
            None
        } else {
            Some(format!("«{}»", model.stereotype).into())
        };

        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        c: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlSequenceElement::Lifeline(m)) = m.get(&old_model.uuid) {
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
            comment_buffer: self.comment_buffer.clone(),
            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            render_style: self.render_style,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlsequence_message(
    name: &str,
    state_invariant: &str,
    synchronicity: UmlSequenceMessageSynchronicityKind,
    lifecycle: UmlSequenceMessageLifecycleKind,
    is_return: bool,
    duration: f32,
    found_activation_color: MGlobalColor,
    new_activation_color: MGlobalColor,
    source: (ERef<UmlSequenceLifeline>, UmlSequenceElementView),
    target: (ERef<UmlSequenceLifeline>, UmlSequenceElementView),
) -> (ERef<UmlSequenceMessage>, ERef<UmlSequenceMessageView>) {
    let link_model = ERef::new(UmlSequenceMessage::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        state_invariant.to_owned(),
        synchronicity,
        lifecycle,
        is_return,
        duration,
        source.0,
        target.0,
    ));
    let link_view = new_umlsequence_message_view(
        link_model.clone(),
        found_activation_color,
        new_activation_color,
        source.1,
        target.1,
    );
    (link_model, link_view)
}
pub fn new_umlsequence_message_view(
    model: ERef<UmlSequenceMessage>,
    found_activation_color: MGlobalColor,
    new_activation_color: MGlobalColor,
    source: UmlSequenceElementView,
    target: UmlSequenceElementView,
) -> ERef<UmlSequenceMessageView> {
    ERef::new(UmlSequenceMessageView {
        uuid: ViewUuid::now_v7().into(),
        model,
        source,
        target,
        bounds_rect: egui::Rect::ZERO,
        found_activation_color,
        new_activation_color,
        temporaries: Default::default(),
    })
}

#[derive(Clone, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceMessageView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceMessage>,

    #[nh_context_serde(entity)]
    source: UmlSequenceElementView,
    #[nh_context_serde(entity)]
    target: UmlSequenceElementView,

    bounds_rect: egui::Rect,
    found_activation_color: MGlobalColor,
    new_activation_color: MGlobalColor,
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlSequenceMessageTemporaries,
}

#[derive(Clone, Default)]
struct UmlSequenceMessageTemporaries {
    line_type: canvas::LineType,
    target_arrow_type: canvas::ArrowheadType,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    state_invariant_in_curly_brackets: String,
    source_y: f32,
    target_y: f32,

    display_text: String,
    name_buffer: String,
    synchronicity_kind_buffer: UmlSequenceMessageSynchronicityKind,
    lifecycle_kind_buffer: UmlSequenceMessageLifecycleKind,
    is_return_buffer: bool,
    duration_buffer: f32,
    state_invariant_buffer: String,
    comment_buffer: String,
    highlight: canvas::Highlight,
}

impl UmlSequenceMessageView {
    const MESSAGE_SPACING: f32 = 60.0;

    fn draw_inner(
        &mut self,
        message_offsets: &HashMap<ViewUuid, (usize, usize)>,
        pos_y: f32,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        let s = canvas::Stroke {
            width: 1.0,
            color: egui::Color32::BLACK,
            line_type: self.temporaries.line_type,
        };

        let (source_x, target_x) = (self.source.position().x, self.target.position().x);
        let (start, second, penultimate, end) = if source_x == target_x {
            let offset = message_offsets.get(&self.uuid).map(|e| {
                e.0 as f32 * ActivationsCounter::ACTIVATION_OFFSET
                    + ActivationsCounter::ACTIVATION_WIDTH / 2.0
            });
            let start_offset = offset.unwrap_or(0.0);
            let end_offset = offset
                .map(|e| {
                    e + match self.temporaries.is_return_buffer {
                        false => ActivationsCounter::ACTIVATION_OFFSET,
                        true => -ActivationsCounter::ACTIVATION_OFFSET,
                    }
                })
                .unwrap_or(0.0);

            const WIDTH: f32 = 25.0;
            let start = egui::Pos2::new(
                source_x + start_offset,
                pos_y + Self::MESSAGE_SPACING / 2.0 - WIDTH,
            );
            let end = egui::Pos2::new(
                source_x + end_offset,
                pos_y + Self::MESSAGE_SPACING / 2.0 + WIDTH + self.temporaries.duration_buffer,
            );
            let second = egui::Pos2::new(start.x + WIDTH, start.y);
            let penultimate = egui::Pos2::new(second.x, end.y);

            canvas.draw_line([start, second], s, self.temporaries.highlight);
            canvas.draw_line([second, penultimate], s, self.temporaries.highlight);

            self.bounds_rect = egui::Rect::from_two_pos(start, penultimate);
            (start, second, penultimate, end)
        } else {
            let (wos, wot) = match source_x < target_x {
                true => (
                    ActivationsCounter::ACTIVATION_WIDTH / 2.0,
                    -ActivationsCounter::ACTIVATION_WIDTH / 2.0,
                ),
                false => (
                    -ActivationsCounter::ACTIVATION_WIDTH / 2.0,
                    ActivationsCounter::ACTIVATION_WIDTH / 2.0,
                ),
            };
            let (start_offset, end_offset) = message_offsets
                .get(&self.uuid)
                .map(|e| {
                    (
                        e.0 as f32 * ActivationsCounter::ACTIVATION_OFFSET + wos,
                        e.1 as f32 * ActivationsCounter::ACTIVATION_OFFSET + wot,
                    )
                })
                .unwrap_or((0.0, 0.0));

            let start =
                egui::Pos2::new(source_x + start_offset, pos_y + Self::MESSAGE_SPACING / 2.0);
            let end = egui::Pos2::new(
                target_x + end_offset,
                pos_y + Self::MESSAGE_SPACING / 2.0 + self.temporaries.duration_buffer,
            );
            self.bounds_rect = egui::Rect::from_two_pos(start, end);
            (start, end, start, end)
        };
        self.temporaries.source_y = start.y;
        self.temporaries.target_y = end.y;

        let end_intersect = self
            .temporaries
            .target_arrow_type
            .get_intersect(end, penultimate);
        canvas.draw_line([penultimate, end_intersect], s, self.temporaries.highlight);
        self.temporaries.target_arrow_type.draw_in(
            canvas,
            end,
            penultimate,
            (egui::Color32::BLACK, egui::Color32::WHITE),
            self.temporaries.highlight,
        );

        canvas.draw_text(
            (start + second.to_vec2()) / 2.0,
            egui::Align2::CENTER_BOTTOM,
            &self.temporaries.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        canvas.draw_text(
            end + egui::Vec2::new(0.0, 10.0),
            egui::Align2::CENTER_TOP,
            &self.temporaries.state_invariant_in_curly_brackets,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        let (lifeline_min_x, lifeline_diff_x) = if source_x <= target_x {
            (source_x, target_x - source_x)
        } else {
            (target_x, source_x - target_x)
        };
        let r = egui::Rect::from_min_size(
            egui::Pos2::new(lifeline_min_x, pos_y),
            egui::Vec2::new(lifeline_diff_x, Self::MESSAGE_SPACING),
        );
        (TargettingStatus::NotDrawn, r)
    }
}

impl Entity for UmlSequenceMessageView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceMessageView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceMessageView {
    fn model(&self) -> UmlSequenceElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rect {
            inner: self.bounds_rect,
        }
    }

    fn position(&self) -> egui::Pos2 {
        todo!()
    }
}

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceMessageView {
    fn draw_in(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlSequenceDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(&HashMap::new(), 0.0, q, context, settings, canvas, tool);
        TargettingStatus::NotDrawn
    }

    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }

        ui.label("Synchronicity:");
        egui::ComboBox::from_id_salt("synchronicity")
            .selected_text(self.temporaries.synchronicity_kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlSequenceMessageSynchronicityKind::VARIANTS {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.synchronicity_kind_buffer,
                            e,
                            e.as_str(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::SynchronicityKindChange(
                                self.temporaries.synchronicity_kind_buffer,
                            ),
                        ));
                    }
                }
            });

        ui.label("Lifecycle:");
        egui::ComboBox::from_id_salt("lifecycle")
            .selected_text(self.temporaries.lifecycle_kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlSequenceMessageLifecycleKind::VARIANTS {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.lifecycle_kind_buffer,
                            e,
                            e.as_str(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::LifecycleKindChange(
                                self.temporaries.lifecycle_kind_buffer,
                            ),
                        ));
                    }
                }
            });

        ui.label("isReturn:");
        if ui
            .checkbox(&mut self.temporaries.is_return_buffer, "isReturn")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::IsReturnChange(self.temporaries.is_return_buffer),
            ));
        }

        ui.label("Duration:");
        let mut duration = self.temporaries.duration_buffer;
        if ui
            .add(egui::DragValue::new(&mut duration).speed(1.0))
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::DurationChange(duration),
            ));
        }

        ui.label("Found activation color");
        if let Some(new_color) = crate::common::controller::mglobalcolor_edit_button(
            gdc,
            ui,
            &self.found_activation_color,
        ) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::ColorChange((2, new_color).into()),
            ));
        }

        ui.label("New activation color");
        if let Some(new_color) =
            crate::common::controller::mglobalcolor_edit_button(gdc, ui, &self.new_activation_color)
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::ColorChange((0, new_color).into()),
            ));
        }

        if ui
            .labeled_text_edit_multiline(
                "State invariant:",
                &mut self.temporaries.state_invariant_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::StateInvariantChange(Arc::new(
                    self.temporaries.state_invariant_buffer.clone(),
                )),
            ));
        }
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalDown,
                ));
            }
        });
        ui.separator();

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CommentChange(Arc::new(
                    self.temporaries.comment_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        _ehc: &EventHandlingContext,
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.bounds_rect.expand(5.0).contains(pos) => {
                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<
            <UmlSequenceDomain as Domain>::OrdinalMovementT,
            <UmlSequenceDomain as Domain>::AddCommandElementT,
            <UmlSequenceDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
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
            InsensitiveCommand::PropertyChange(uuids, property) if uuids.contains(&*self.uuid) => {
                let mut model = self.model.write();
                affected_models.insert(*model.uuid);
                match property {
                    UmlSequencePropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::NameChange(model.name.clone()),
                        ));
                        model.name = name.clone();
                    }
                    UmlSequencePropChange::SynchronicityKindChange(synchronicity) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::SynchronicityKindChange(model.synchronicity),
                        ));
                        model.synchronicity = *synchronicity;
                    }
                    UmlSequencePropChange::LifecycleKindChange(lifecycle) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::LifecycleKindChange(model.lifecycle),
                        ));
                        model.lifecycle = *lifecycle;
                    }
                    UmlSequencePropChange::IsReturnChange(is_return) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::IsReturnChange(model.is_return),
                        ));
                        model.is_return = *is_return;
                    }
                    UmlSequencePropChange::DurationChange(duration) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::DurationChange(model.duration),
                        ));
                        model.duration = *duration;
                    }
                    UmlSequencePropChange::StateInvariantChange(invariant) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::StateInvariantChange(
                                model.state_invariant.clone(),
                            ),
                        ));
                        model.state_invariant = invariant.clone();
                    }
                    UmlSequencePropChange::FlipMulticonnection(_) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::FlipMulticonnection(FlipMulticonnection {}),
                        ));
                        std::mem::swap(&mut self.source, &mut self.target);
                        model.flip_multiconnection();
                    }
                    UmlSequencePropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::CommentChange(model.comment.clone()),
                        ));
                        model.comment = comment.clone();
                    }
                    UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::ColorChange(
                                (0, self.new_activation_color).into(),
                            ),
                        ));
                        self.new_activation_color = *color;
                    }
                    UmlSequencePropChange::ColorChange(ColorChangeData { slot: 2, color }) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::ColorChange(
                                (2, self.found_activation_color).into(),
                            ),
                        ));
                        self.found_activation_color = *color;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        let (line_type, target_arrow_type) =
            match (&model.synchronicity, model.lifecycle, model.is_return) {
                (_, UmlSequenceMessageLifecycleKind::Create, _) | (_, _, true) => (
                    canvas::LineType::Dashed,
                    canvas::ArrowheadType::OpenTriangle,
                ),
                (synchronicity, _, _) => (
                    canvas::LineType::Solid,
                    match synchronicity {
                        UmlSequenceMessageSynchronicityKind::Synchronous => {
                            canvas::ArrowheadType::FullTriangle
                        }
                        UmlSequenceMessageSynchronicityKind::AsynchronousCall
                        | UmlSequenceMessageSynchronicityKind::AsynchronousSignal => {
                            canvas::ArrowheadType::OpenTriangle
                        }
                    },
                ),
            };

        self.temporaries.line_type = line_type;
        self.temporaries.target_arrow_type = target_arrow_type;

        self.temporaries.source_uuids.clear();
        self.temporaries
            .source_uuids
            .push(*model.source.read().uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries
            .target_uuids
            .push(*model.target.read().uuid());

        self.temporaries.display_text = match model.lifecycle {
            UmlSequenceMessageLifecycleKind::None => (*model.name).clone(),
            UmlSequenceMessageLifecycleKind::Create => {
                format!("«create»\n{}", model.name).trim().to_string()
            }
            UmlSequenceMessageLifecycleKind::Delete => {
                format!("«destroy»\n{}", model.name).trim().to_string()
            }
        };
        self.temporaries.state_invariant_in_curly_brackets = if model.state_invariant.is_empty() {
            "".to_owned()
        } else {
            format!("{{{}}}", model.state_invariant)
        };
        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.synchronicity_kind_buffer = model.synchronicity;
        self.temporaries.lifecycle_kind_buffer = model.lifecycle;
        self.temporaries.is_return_buffer = model.is_return;
        self.temporaries.duration_buffer = model.duration;
        self.temporaries.state_invariant_buffer = (*model.state_invariant).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model.read().uuid, *self.uuid);
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        deleting.contains(&self.source.uuid()) || deleting.contains(&self.target.uuid())
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *self.model_uuid())
        };

        let old_model = self.model.read();

        let model = if let Some(UmlSequenceElement::Message(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut void = HashMap::new();
        let source = if let Some(s) = c.get(&self.source.uuid()) {
            s.clone()
        } else {
            self.source.deep_copy_clone(uuid_present, &mut void, c, m);
            c.get(&*self.source.uuid()).unwrap().clone()
        };
        let target = if let Some(t) = c.get(&self.target.uuid()) {
            t.clone()
        } else {
            self.target.deep_copy_clone(uuid_present, &mut void, c, m);
            c.get(&*self.target.uuid()).unwrap().clone()
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            source,
            target,
            bounds_rect: self.bounds_rect,
            found_activation_color: self.found_activation_color,
            new_activation_color: self.new_activation_color,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        _c: &HashMap<ViewUuid, UmlSequenceElementView>,
        m: &HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        let mut model = self.model.write();

        let source_model_uuid = *model.source.read().uuid();
        if let Some(UmlSequenceElement::Lifeline(new_source)) = m.get(&source_model_uuid) {
            model.source = new_source.clone();
        }
        let target_model_uuid = *model.target.read().uuid();
        if let Some(UmlSequenceElement::Lifeline(new_target)) = m.get(&target_model_uuid) {
            model.target = new_target.clone();
        }
    }
}

pub fn new_umlsequence_ref(
    text: &str,
    horizontal_span: HashSet<ModelUuid>,
) -> (ERef<UmlSequenceRef>, ERef<UmlSequenceRefView>) {
    let comment_model = ERef::new(UmlSequenceRef::new(
        ModelUuid::now_v7(),
        text.to_owned(),
        horizontal_span,
    ));
    let comment_view = new_umlsequence_ref_view(comment_model.clone());

    (comment_model, comment_view)
}
pub fn new_umlsequence_ref_view(model: ERef<UmlSequenceRef>) -> ERef<UmlSequenceRefView> {
    ERef::new(UmlSequenceRefView {
        uuid: ViewUuid::now_v7().into(),
        model,

        temporaries: Default::default(),
        bounds_rect: egui::Rect::ZERO,
        background_color: MGlobalColor::None,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlSequenceRefView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlSequenceRef>,

    #[nh_context_serde(skip_and_default)]
    temporaries: UmlSequenceRefViewTemporaries,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

#[derive(Clone, Default)]
struct UmlSequenceRefViewTemporaries {
    text_buffer: String,

    highlight: canvas::Highlight,
}

impl UmlSequenceRefView {
    const REF_MARGIN_BOTTOM: f32 = 10.0;
    const REF_PADDING_X: f32 = 20.0;

    fn spanned_lifeline_views<'b>(
        &self,
        lifeline_views: &'b [ERef<UmlSequenceLifelineView>],
    ) -> &'b [ERef<UmlSequenceLifelineView>] {
        let r = self.model.read();
        let start = lifeline_views
            .iter()
            .enumerate()
            .find(|e| r.horizontal_span.contains(&e.1.read().model_uuid()))
            .map(|e| e.0)
            .unwrap_or(0);
        let end = lifeline_views
            .iter()
            .enumerate()
            .rev()
            .find(|e| r.horizontal_span.contains(&e.1.read().model_uuid()))
            .map(|e| e.0 + 1)
            .unwrap_or(lifeline_views.len());
        &lifeline_views[start..end]
    }

    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        pos_y: f32,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        let spanned_lifeline_views = self.spanned_lifeline_views(lifeline_views);
        let span_x = (
            spanned_lifeline_views
                .first()
                .map(|e| e.read().bounds_rect.center().x)
                .unwrap_or(0.0),
            spanned_lifeline_views
                .last()
                .map(|e| e.read().bounds_rect.center().x)
                .unwrap_or(0.0),
        );

        let background_color = context
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
        const PADDING_Y: f32 = 5.0;
        let r = canvas.measure_text(
            egui::Pos2::new((span_x.0 + span_x.1) / 2.0, pos_y + PADDING_Y),
            egui::Align2::CENTER_TOP,
            &self.temporaries.text_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );

        self.bounds_rect = r
            .with_min_x(r.min.x.min(span_x.0))
            .with_max_x(r.max.x.max(span_x.1))
            .expand2(egui::Vec2::new(Self::REF_PADDING_X, PADDING_Y));
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );
        canvas.draw_text(
            egui::Pos2::new((span_x.0 + span_x.1) / 2.0, pos_y + PADDING_Y),
            egui::Align2::CENTER_TOP,
            &self.temporaries.text_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw top left pentagon
        const PENTAGON_PADDING: f32 = 4.0;
        let pentagon_bg = egui::Color32::WHITE;
        let r = canvas
            .measure_text(
                egui::Pos2::new(
                    self.bounds_rect.min.x + PENTAGON_PADDING,
                    pos_y + PENTAGON_PADDING,
                ),
                egui::Align2::LEFT_TOP,
                "ref",
                canvas::CLASS_MIDDLE_FONT_SIZE,
            )
            .expand(PENTAGON_PADDING);
        canvas.draw_polygon(
            [
                r.left_top(),
                r.right_top(),
                r.right_bottom() - egui::Vec2::new(0.0, PENTAGON_PADDING),
                r.right_bottom() - egui::Vec2::new(PENTAGON_PADDING, 0.0),
                r.left_bottom(),
            ]
            .to_vec(),
            pentagon_bg,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );
        canvas.draw_text(
            egui::Pos2::new(
                self.bounds_rect.min.x + PENTAGON_PADDING,
                pos_y + PENTAGON_PADDING,
            ),
            egui::Align2::LEFT_TOP,
            "ref",
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        let r = self
            .bounds_rect
            .with_max_y(self.bounds_rect.max.y + Self::REF_MARGIN_BOTTOM);
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
                t.targetting_for_section(Some(self.model())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            (TargettingStatus::Drawn, r)
        } else {
            (TargettingStatus::NotDrawn, r)
        }
    }

    fn handle_event_inner(
        &mut self,
        _lifeline_views: &[ERef<UmlSequenceLifelineView>],
        event: InputEvent,
        _ehc: &EventHandlingContext,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.bounds_rect.contains(pos) => {
                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }
}

impl Entity for UmlSequenceRefView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceRefView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceRefView {
    fn model(&self) -> UmlSequenceElement {
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

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceRefView {
    fn draw_in(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlSequenceDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlSequenceDomain as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(&Vec::new(), 0.0, q, context, settings, canvas, tool);
        TargettingStatus::NotDrawn
    }

    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if ui
            .labeled_text_edit_multiline("Text:", &mut self.temporaries.text_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::NameChange(Arc::new(self.temporaries.text_buffer.clone())),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlSequenceOrdinalMovement::HorizontalDown,
                ));
            }
        });

        PropertiesStatus::Shown
    }

    fn handle_event(
        &mut self,
        _event: InputEvent,
        _ehc: &EventHandlingContext,
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> EventHandlingStatus {
        unreachable!()
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<
            <UmlSequenceDomain as Domain>::OrdinalMovementT,
            <UmlSequenceDomain as Domain>::AddCommandElementT,
            <UmlSequenceDomain as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                <UmlSequenceDomain as Domain>::OrdinalMovementT,
                <UmlSequenceDomain as Domain>::AddCommandElementT,
                <UmlSequenceDomain as Domain>::PropChangeT,
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
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::AddDependency { .. }
            | InsensitiveCommand::RemoveDependency { .. }
            | InsensitiveCommand::ArrangeSpecificElements(..)
            | InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    if let UmlSequencePropChange::NameChange(name) = property {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::NameChange(model.text.clone()),
                        ));
                        model.text = name.clone();
                    }
                }
            }
            InsensitiveCommand::Macro(..) => unreachable!(),
        }
    }

    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.temporaries.text_buffer = (*model.text).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.temporaries.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(UmlSequenceElement::Ref(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,

            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            temporaries: self.temporaries.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlsequence_comment(
    text: &str,
    position: egui::Pos2,
    align: egui::Align2,
) -> (ERef<UmlSequenceComment>, ERef<UmlSequenceCommentView>) {
    let comment_model = ERef::new(UmlSequenceComment::new(
        ModelUuid::now_v7(),
        text.to_owned(),
    ));
    let comment_view = new_umlsequence_comment_view(comment_model.clone(), position, align);

    (comment_model, comment_view)
}
pub fn new_umlsequence_comment_view(
    model: ERef<UmlSequenceComment>,
    position: egui::Pos2,
    align: egui::Align2,
) -> ERef<UmlSequenceCommentView> {
    let m = model.read();
    ERef::new(UmlSequenceCommentView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

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
pub struct UmlSequenceCommentView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlSequenceComment>,

    #[nh_context_serde(skip_and_default)]
    text_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    align: egui::Align2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl UmlSequenceCommentView {
    const CORNER_SIZE: f32 = 10.0;
}

impl Entity for UmlSequenceCommentView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlSequenceCommentView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlSequenceElement> for UmlSequenceCommentView {
    fn model(&self) -> UmlSequenceElement {
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

impl ElementControllerGen2<UmlSequenceDomain> for UmlSequenceCommentView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
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
                UmlSequencePropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
                            UmlSequencePropChange::CommentAlignChange(Some(tmp_x), None),
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
                            UmlSequencePropChange::CommentAlignChange(None, Some(tmp_y)),
                        ));
                    }
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
                UmlSequencePropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &UmlSequenceSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> TargettingStatus {
        let read = self.model.read();

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
                &read.text,
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
            &read.text,
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
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
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
            UmlSequenceOrdinalMovement,
            UmlSequenceElementOrVertex,
            UmlSequencePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
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
                        UmlSequencePropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlSequencePropChange::CommentAlignChange(x, y) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CommentAlignChange(
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
        _flattened_views: &mut HashMap<ViewUuid, (UmlSequenceElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        c: &mut HashMap<ViewUuid, UmlSequenceElementView>,
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlSequenceElement::Comment(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlSequenceComment::new(
                model_uuid,
                (*old_model.text).clone(),
            ));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
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

pub fn new_umlsequence_commentlink(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlSequenceComment>, UmlSequenceElementView),
    target: (UmlSequenceElement, UmlSequenceElementView),
) -> (ERef<UmlSequenceCommentLink>, ERef<CommentLinkViewT>) {
    let link_model = ERef::new(UmlSequenceCommentLink::new(
        ModelUuid::now_v7(),
        source.0,
        target.0,
    ));
    let link_view =
        new_umlsequence_commentlink_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlsequence_commentlink_view(
    model: ERef<UmlSequenceCommentLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlSequenceElementView,
    target: UmlSequenceElementView,
) -> ERef<CommentLinkViewT> {
    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlSequenceCommentLinkAdapter {
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
pub struct UmlSequenceCommentLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlSequenceCommentLink>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlClassCommentLinkTemporaries,
}

#[derive(Clone, Default)]
struct UmlClassCommentLinkTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
}

impl MulticonnectionAdapter<UmlSequenceDomain> for UmlSequenceCommentLinkAdapter {
    fn model(&self) -> UmlSequenceElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlSequenceDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlSequenceDomain as Domain>::ToolT)>,
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
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        _view_uuid: &ViewUuid,
        _command: &InsensitiveCommand<
            UmlSequenceOrdinalMovement,
            UmlSequenceElementOrVertex,
            UmlSequencePropChange,
        >,
        _undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlSequenceOrdinalMovement,
                UmlSequenceElementOrVertex,
                UmlSequencePropChange,
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
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlSequenceElement::CommentLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlSequenceCommentLink::new(
                new_uuid,
                old_model.source.clone(),
                old_model.target.clone(),
            ));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlSequenceElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.read().uuid();
        if let Some(UmlSequenceElement::Comment(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}
