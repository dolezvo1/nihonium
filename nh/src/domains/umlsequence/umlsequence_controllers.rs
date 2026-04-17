use super::umlsequence_models::{
    UmlSequenceDiagram, UmlSequenceElement, UmlSequenceHorizontalElement, UmlSequenceLifeline,
    UmlSequenceCombinedFragment, UmlSequenceComment, UmlSequenceCommentLink, UmlSequenceMessage,
};
use crate::common::canvas::{self, Highlight, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerModel, ControllerAdapter, DeleteKind, DiagramAdapter, DiagramController, DiagramControllerGen2, DiagramSettings, DiagramSettings2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider, MGlobalColor, Model, MultiDiagramController, PositionNoT, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SnapManager, TargettingStatus, Tool, ToolPalette, TryMerge, View
};
use crate::common::ui_ext::UiExt;
use crate::common::views::package_view::PackageDragType;
use crate::common::views::multiconnection_view::{ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::domains::umlsequence::umlsequence_models::{UmlSequenceCombinedFragmentKind, UmlSequenceCombinedFragmentSection, UmlSequenceDiagramBoard, UmlSequenceMessageLifecycleKind, UmlSequenceMessageSynchronicityKind};
use crate::CustomModal;
use eframe::{egui, epaint};
use std::any::Any;
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
    type AddCommandElementT = UmlSequenceElementOrVertex;
    type PropChangeT = UmlSequencePropChange;
}

type CommentLinkViewT = MulticonnectionView<UmlSequenceDomain, UmlSequenceCommentLinkAdapter>;

#[derive(Clone)]
pub enum UmlSequencePropChange {
    NameChange(Arc<String>),
    StereotypeChange(Arc<String>),

    SynchronicityKindChange(UmlSequenceMessageSynchronicityKind),
    LifecycleKindChange(UmlSequenceMessageLifecycleKind),
    IsReturnChange(bool),
    StateInvariantChange(Arc<String>),
    FlipMulticonnection(FlipMulticonnection),

    CombinedFragmentKindChange(UmlSequenceCombinedFragmentKind),
    CombinedFragmentKindArgumentChange(Arc<String>),
    CombinedFragmentSectionGuardChange(Arc<String>),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
}

impl Debug for UmlSequencePropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassPropChange::???")
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
    fn try_merge(&self, newer: &Self) -> Option<Self> where Self: Sized {
        match (self, newer) {
            (Self::StereotypeChange(_), Self::StereotypeChange(newer)) => Some(Self::StereotypeChange(newer.clone())),
            (Self::NameChange(_), Self::NameChange(newer)) => Some(Self::NameChange(newer.clone())),
            (Self::StateInvariantChange(_), Self::StateInvariantChange(newer)) => Some(Self::StateInvariantChange(newer.clone())),
            (Self::CombinedFragmentKindArgumentChange(_), Self::CombinedFragmentKindArgumentChange(newer)) => Some(Self::CombinedFragmentKindArgumentChange(newer.clone())),
            (Self::CombinedFragmentSectionGuardChange(_), Self::CombinedFragmentSectionGuardChange(newer)) => Some(Self::CombinedFragmentSectionGuardChange(newer.clone())),
            (Self::CommentChange(_), Self::CommentChange(newer)) => Some(Self::CommentChange(newer.clone())),
            _ => None
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
    Comment(ERef<UmlSequenceCommentView>),
    CommentLink(ERef<CommentLinkViewT>),
}

impl UmlSequenceElementView {
    fn as_horizontal(self) -> Option<UmlSequenceHorizontalElementView> {
        match self {
            UmlSequenceElementView::CombinedFragment(inner) => Some(inner.into()),
            UmlSequenceElementView::Message(inner) => Some(inner.into()),
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
}

impl UmlSequenceHorizontalElementView {
    fn as_element_view(self) -> UmlSequenceElementView {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => inner.into(),
            UmlSequenceHorizontalElementView::Message(inner) => inner.into(),
        }
    }

    fn theoretical_height(&self) -> f32 {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => inner.read().theoretical_height(),
            UmlSequenceHorizontalElementView::Message(inner) => inner.read().theoretical_height(),
        }
    }
    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        pos_and_scale_y: (f32, f32),
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => inner.write().draw_inner(lifeline_views, pos_and_scale_y, q, context, settings, canvas, tool),
            UmlSequenceHorizontalElementView::Message(inner) => inner.write().draw_inner(pos_and_scale_y, q, context, settings, canvas, tool),
        }
    }
    fn handle_event_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        event: InputEvent,
        ehc: &EventHandlingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> EventHandlingStatus {
        match self {
            UmlSequenceHorizontalElementView::CombinedFragment(inner) => inner.write().handle_event_inner(lifeline_views, event, ehc, q, tool, element_setup_modal, commands),
            UmlSequenceHorizontalElementView::Message(inner) => inner.write().handle_event(event, ehc, q, tool, element_setup_modal, commands),
        }
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

    fn insert_element(&mut self, parent: ModelUuid, element: UmlSequenceElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
        self.model.write().insert_element_into(parent, element, b, p)
    }

    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlSequenceElement, BucketNoT, PositionNoT)>) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(&self, _gdc: &GlobalDrawingContext, ui: &mut egui::Ui) -> Option<ERef<Self::DiagramViewT>> {
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


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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

    fn get_element_pos_in(&self, parent: &ModelUuid, model_uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
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
                new_umlsequence_diagram_view(inner.clone(), Vec::new(), Vec::new(), egui::Rect::from_x_y_ranges(0.0..=100.0, 0.0..=100.0)).into()
            },
            UmlSequenceElement::CombinedFragment(inner) => {
                let r = inner.read();
                let section_views: Result<Vec<_>, _> = r.sections.iter()
                    .map(|e| self.create_new_view_for(q, e.clone().into()).map(|e| match e {
                        UmlSequenceElementView::CombinedFragmentSection(inner) => inner,
                        _ => unreachable!(),
                    }))
                    .collect();
                new_umlsequence_combinedfragment_view(inner.clone(), section_views?).into()
            },
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let r = inner.read();
                let horizontal_element_views: Result<Vec<_>, _> = r.horizontal_elements.iter()
                    .map(|e| self.create_new_view_for(q, e.clone().to_element()).map(|e| e.as_horizontal().unwrap()))
                    .collect();
                new_umlsequence_combinedfragmentsection_view(inner.clone(), horizontal_element_views?).into()
            },
            UmlSequenceElement::Lifeline(inner) => {
                new_umlsequence_lifeline_view(inner.clone(), UmlSequenceLifelineRenderStyle::Object).into()
            },
            UmlSequenceElement::Message(inner) => {
                let r = inner.read();
                let source_uuid = *r.source.read().uuid;
                let target_uuid = *r.target.read().uuid;
                let (Some(s), Some(t)) = (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid)) else {
                    return Err([source_uuid, target_uuid].into_iter().collect());
                };
                new_umlsequence_message_view(inner.clone(), s, t).into()
            },
            UmlSequenceElement::Comment(inner) => todo!(),
            UmlSequenceElement::CommentLink(inner) => todo!(),
        };

        Ok(v)
    }
    fn label_for(&self, e: &UmlSequenceElement) -> Arc<String> {
        match e {
            UmlSequenceElement::Diagram(inner) => inner.read().name.clone(),
            UmlSequenceElement::CombinedFragment(inner) => {
                let r = inner.read();
                Arc::new(combined_fragment_display_text(r.kind, &r.kind_argument))
            },
            UmlSequenceElement::CombinedFragmentSection(inner) => {
                let r = inner.read();
                let s = if r.guard.is_empty() {
                    "Section".to_owned()
                } else {
                    format!("Section ([{}])", r.guard)
                };
                Arc::new(s)
            },
            UmlSequenceElement::Lifeline(inner) => inner.read().name.clone(),
            UmlSequenceElement::Message(inner) => {
                let r = inner.read();
                let s = if r.name.is_empty() {
                    "Message".to_owned()
                } else {
                    format!("Message ({})", r.name)
                };
                Arc::new(s)
            },
            UmlSequenceElement::Comment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            },
            UmlSequenceElement::CommentLink(_inner) => {
                Arc::new(format!("Comment Link"))
            },
        }
    }

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32 {
        global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE)
    }
    fn gridlines_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::from_rgb(220, 220, 220)
    }
    fn show_view_props_fun(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        ui.label("Background color:");
        if crate::common::controller::mglobalcolor_edit_button(
            &drawing_context.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        PropertiesStatus::Shown
    }
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) {
        if ui.labeled_text_edit_singleline("Name:", &mut self.buffer.name).changed() {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    UmlSequencePropChange::NameChange(Arc::new(
                        self.buffer.name.clone(),
                    )),
                ),
            );
        }

        if ui.labeled_text_edit_multiline("Comment:", &mut self.buffer.comment).changed() {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    UmlSequencePropChange::CommentChange(Arc::new(
                        self.buffer.comment.clone(),
                    )),
                ),
            );
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
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
                        UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
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
    ) {}

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

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, UmlSequenceElement>) {
        let models = super::umlsequence_models::fake_copy_diagram(&self.model.read());
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
        ERef::new(
            MultiDiagramController::new(
                ControllerUuid::now_v7(),
                UmlSequenceControllerAdapter { model: model.clone() },
                vec![
                    DiagramControllerGen2::new(
                        uuid.into(),
                        name.into(),
                        UmlSequenceDiagramBoardAdapter::new(model),
                        elements,
                    )
                ]
            )
        )
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
    let (user_model, user_view) = new_umlsequence_lifeline("User", "", UmlSequenceLifelineRenderStyle::StickFigure);
    let (service1_model, service1_view) = new_umlsequence_lifeline("Auth server", "", UmlSequenceLifelineRenderStyle::Object);
    let (service2_model, service2_view) = new_umlsequence_lifeline("Database", "", UmlSequenceLifelineRenderStyle::Database);

    let (message1_model, message1_view) = new_umlsequence_message("request", "", UmlSequenceMessageSynchronicityKind::Synchronous, UmlSequenceMessageLifecycleKind::None, false, (user_model.clone(), user_view.clone().into()), (service1_model.clone(), service1_view.clone().into()));
    let (message2_model, message2_view) = new_umlsequence_message("database query", "", UmlSequenceMessageSynchronicityKind::Synchronous, UmlSequenceMessageLifecycleKind::None, false, (service1_model.clone(), service1_view.clone().into()), (service2_model.clone(), service2_view.clone().into()));

    let (combined_fragment_section1_model, combined_fragment_section1_view) = new_umlsequence_combinedfragmentsection("invalid token", vec![]);
    let (combined_fragment_section2_model, combined_fragment_section2_view) = new_umlsequence_combinedfragmentsection("token valid", vec![
        (message2_model.into(), message2_view.into()),
    ]);
    let (combined_fragment_model, combined_fragment_view) = new_umlsequence_combinedfragment(
        UmlSequenceCombinedFragmentKind::Alt, "",
        [*user_model.read().uuid, *service1_model.read().uuid, *service2_model.read().uuid].into_iter().collect(),
        vec![
            (combined_fragment_section1_model.into(), combined_fragment_section1_view.into()),
            (combined_fragment_section2_model.into(), combined_fragment_section2_view.into()),
        ],
    );

    let (diagram_model, diagram_view) = new_umlsequence_diagram(
        "Diagram",
        vec![(user_model, user_view), (service1_model, service1_view), (service2_model, service2_view)],
        vec![
            (message1_model.into(), message1_view.into()),
            (combined_fragment_model.into(), combined_fragment_view.into()),
        ],
        egui::Rect::from_min_size(egui::Pos2::new(100.0, 100.0), egui::Vec2::splat(500.0)),
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

pub fn deserializer(uuid: ControllerUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<UmlSequenceDomain, UmlSequenceControllerAdapter, DiagramControllerGen2<UmlSequenceDomain, UmlSequenceDiagramBoardAdapter>>>(&uuid)?)
}

pub struct UmlSequenceSettings {
    palette: RwLock<ToolPalette<UmlSequenceToolStage, UmlSequenceElementView>>,
}

impl DiagramSettings for UmlSequenceSettings {}
impl DiagramSettings2<UmlSequenceDomain> for UmlSequenceSettings {
    fn palette_for_each_mut<F>(&self, f: F)
        where F: FnMut(&mut (uuid::Uuid, &'static str, Vec<(uuid::Uuid, UmlSequenceToolStage, &'static str, UmlSequenceElementView)>))
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let (_, diagram_view) = new_umlsequence_diagram("Diagram", Vec::new(), Vec::new(), egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(100.0, 75.0)));
    diagram_view.write().refresh_buffers();
    let (_, combined_fragment_view) = {
        let section = new_umlsequence_combinedfragmentsection("no errors", Vec::new());
        section.1.write().refresh_buffers();
        new_umlsequence_combinedfragment(UmlSequenceCombinedFragmentKind::Opt, "", HashSet::new(), vec![section.into()])
    };
    combined_fragment_view.write().refresh_buffers();
    let (lifeline_model1, lifeline_view1) = new_umlsequence_lifeline("User", "", UmlSequenceLifelineRenderStyle::StickFigure);
    let (lifeline_model2, lifeline_view2) = new_umlsequence_lifeline("Service", "", UmlSequenceLifelineRenderStyle::Object);
    lifeline_view2.write().bounds_rect = egui::Rect::from_x_y_ranges(150.0..=150.0, 0.0..=0.0);
    let (_, message_view1) = new_umlsequence_message("", "", UmlSequenceMessageSynchronicityKind::Synchronous, UmlSequenceMessageLifecycleKind::None, false,
        (lifeline_model1.clone(), lifeline_view1.clone().into()),
        (lifeline_model2.clone(), lifeline_view2.clone().into()));
    message_view1.write().refresh_buffers();
    let (_, message_view2) = new_umlsequence_message("", "", UmlSequenceMessageSynchronicityKind::Synchronous, UmlSequenceMessageLifecycleKind::None, true,
        (lifeline_model1.clone(), lifeline_view1.clone().into()),
        (lifeline_model2.clone(), lifeline_view2.clone().into()));
    message_view2.write().refresh_buffers();
    let (_, message_view3) = new_umlsequence_message("", "", UmlSequenceMessageSynchronicityKind::AsynchronousCall, UmlSequenceMessageLifecycleKind::None, false,
        (lifeline_model1.clone(), lifeline_view1.clone().into()),
        (lifeline_model2.clone(), lifeline_view2.clone().into()));
    message_view3.write().refresh_buffers();
    let (_, message_view4) = new_umlsequence_message("", "", UmlSequenceMessageSynchronicityKind::AsynchronousSignal, UmlSequenceMessageLifecycleKind::None, false,
        (lifeline_model1.clone(), lifeline_view1.clone().into()),
        (lifeline_model2.clone(), lifeline_view2.clone().into()));
    message_view4.write().refresh_buffers();
    let (_, comment_view) = new_umlsequence_comment("Comment", egui::Pos2::ZERO);

    let palette_items = vec![
        ("Containers", vec![
            (UmlSequenceToolStage::DiagramStart, "Diagram", diagram_view.into()),
            (UmlSequenceToolStage::CombinedFragmentStart { kind: UmlSequenceCombinedFragmentKind::Opt }, "Combined Fragment", combined_fragment_view.into()),
        ]),
        ("Elements", vec![
            (UmlSequenceToolStage::Lifeline { name: "User", stereotype: "", render_style: UmlSequenceLifelineRenderStyle::StickFigure }, "Actor Lifeline", lifeline_view1.into()),
            (UmlSequenceToolStage::Lifeline { name: "Service", stereotype: "", render_style: UmlSequenceLifelineRenderStyle::Object }, "Object Lifeline", lifeline_view2.into()),
        ]),
        ("Messages", vec![
            (UmlSequenceToolStage::LinkStart { link_type: LinkType::Message {
                synchronicity_kind: UmlSequenceMessageSynchronicityKind::Synchronous,
                is_return: false,
                name: "",
            } }, "Synchronous Message", message_view1.into()),
            (UmlSequenceToolStage::LinkStart { link_type: LinkType::Message {
                synchronicity_kind: UmlSequenceMessageSynchronicityKind::Synchronous,
                is_return: true,
                name: "",
            } }, "Synchronous Return", message_view2.into()),
            (UmlSequenceToolStage::LinkStart { link_type: LinkType::Message {
                synchronicity_kind: UmlSequenceMessageSynchronicityKind::AsynchronousCall,
                is_return: false,
                name: "",
            } }, "Asynchronous Call", message_view3.into()),
            (UmlSequenceToolStage::LinkStart { link_type: LinkType::Message {
                synchronicity_kind: UmlSequenceMessageSynchronicityKind::AsynchronousSignal,
                is_return: false,
                name: "",
            } }, "Asynchronous Signal", message_view4.into()),
        ]),
        ("Other", vec![
            (UmlSequenceToolStage::Comment, "Comment", comment_view.into()),
            //(UmlSequenceToolStage::CommentLinkStart, "Comment Link", commentlink.1.into()),
        ]),
    ];
    Box::new(UmlSequenceSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
    })
}
pub fn settings_function(gdc: &mut GlobalDrawingContext, ui: &mut egui::Ui, s: &mut Box<dyn DiagramSettings>) {
    let Some(s) = (s.as_mut() as &mut dyn Any).downcast_mut::<UmlSequenceSettings>() else { return; };

    s.palette.write().unwrap().show_treeview(gdc, ui);
}


#[derive(Clone, Copy, PartialEq)]
pub enum LinkType {
    Message {
        synchronicity_kind: UmlSequenceMessageSynchronicityKind,
        is_return: bool,
        name: &'static str,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlSequenceToolStage {
    DiagramStart,
    DiagramEnd,
    CombinedFragmentStart { kind: UmlSequenceCombinedFragmentKind },
    CombinedFragmentEnd,
    CombinedFragmentSection,
    Lifeline { name: &'static str, stereotype: &'static str, render_style: UmlSequenceLifelineRenderStyle },
    LinkStart { link_type: LinkType },
    LinkEnd,
    Comment,
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
        source: ERef<UmlSequenceLifeline>,
        dest: Option<ERef<UmlSequenceLifeline>>,
    },
    Link {
        link_type: LinkType,
        source: ERef<UmlSequenceLifeline>,
        dest: Option<ERef<UmlSequenceLifeline>>,
    },
    CommentLink {
        source: ERef<UmlSequenceComment>,
        dest: Option<UmlSequenceElement>,
    },
}

pub struct NaiveUmlSequenceTool {
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

    fn new(initial_stage: UmlSequenceToolStage, repeat: bool) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialUmlSequenceElement::None,
            event_lock: false,
            is_spent: if repeat { None } else { Some(false) },
        }
    }
    fn initial_stage(&self) -> Self::Stage {
        self.initial_stage
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
                | UmlSequenceToolStage::DiagramEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Some(UmlSequenceElement::Diagram(_)) => match self.current_stage {
                UmlSequenceToolStage::Lifeline { .. }
                | UmlSequenceToolStage::LinkStart { .. }
                | UmlSequenceToolStage::LinkEnd
                | UmlSequenceToolStage::CombinedFragmentStart { .. }
                | UmlSequenceToolStage::CombinedFragmentEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR
            }
            Some(UmlSequenceElement::CombinedFragmentSection(_)) => match self.current_stage {
                UmlSequenceToolStage::LinkStart { .. }
                | UmlSequenceToolStage::LinkEnd
                | UmlSequenceToolStage::CombinedFragmentStart { .. }
                | UmlSequenceToolStage::CombinedFragmentEnd => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            }
            _ => NON_TARGETTABLE_COLOR,
        }
    }
    fn draw_status_hint(&self, q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,  canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialUmlSequenceElement::Link {
                source,
                ..
            } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_line(
                        [egui::Pos2::new(source_view.position().x, pos.y), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlSequenceElement::CombinedFragment {
                source,
                ..
            } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_rectangle(
                        egui::Rect::from_two_pos(egui::Pos2::new(source_view.position().x, pos.y), pos)
                            .expand(UmlSequenceCombinedFragmentSectionView::SECTION_PADDING_X / 2.0),
                        egui::CornerRadius::ZERO,
                        egui::Color32::TRANSPARENT,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlSequenceElement::CommentLink {
                source,
                ..
            } => {
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

        match (self.current_stage, &mut self.result) {
            (UmlSequenceToolStage::Lifeline { name, stereotype, render_style }, _) => {
                let (_class_model, class_view) =
                    new_umlsequence_lifeline(name, stereotype, render_style);
                self.result = PartialUmlSequenceElement::Some(class_view.into());
                self.event_lock = true;
            }
            (UmlSequenceToolStage::DiagramStart, _) => {
                self.result = PartialUmlSequenceElement::Diagram {
                    a: pos,
                    b: None,
                };
                self.current_stage = UmlSequenceToolStage::DiagramEnd;
                self.event_lock = true;
            }
            (UmlSequenceToolStage::DiagramEnd, PartialUmlSequenceElement::Diagram { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (UmlSequenceToolStage::Comment, _) => {
                let (_comment_model, comment_view) =
                    new_umlsequence_comment("a comment", pos);
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

        match element {
            UmlSequenceElement::CombinedFragment(_inner) => {
                match (self.current_stage, &mut self.result) {
                    (
                        UmlSequenceToolStage::CombinedFragmentSection,
                        PartialUmlSequenceElement::None,
                    ) => {
                        self.result = PartialUmlSequenceElement::Some(
                            new_umlsequence_combinedfragmentsection("", Vec::new()).1.into()
                        );
                        self.event_lock = true;
                    }
                    _ => {}
                }
            },
            UmlSequenceElement::Lifeline(inner) => {
                match (self.current_stage, &mut self.result) {
                    (UmlSequenceToolStage::LinkStart { link_type }, PartialUmlSequenceElement::None) => {
                        self.result = PartialUmlSequenceElement::Link {
                            link_type,
                            source: inner,
                            dest: None,
                        };
                        self.current_stage = UmlSequenceToolStage::LinkEnd;
                        self.event_lock = true;
                    },
                    (
                        UmlSequenceToolStage::LinkEnd,
                        PartialUmlSequenceElement::Link { dest, .. },
                    ) => {
                        *dest = Some(inner);
                        self.event_lock = true;
                    }
                    (UmlSequenceToolStage::CombinedFragmentStart { kind }, PartialUmlSequenceElement::None) => {
                        self.result = PartialUmlSequenceElement::CombinedFragment {
                            kind,
                            source: inner,
                            dest: None,
                        };
                        self.current_stage = UmlSequenceToolStage::CombinedFragmentEnd;
                        self.event_lock = true;
                    },
                    (UmlSequenceToolStage::CombinedFragmentEnd, PartialUmlSequenceElement::CombinedFragment { dest, .. },) => {
                        *dest = Some(inner);
                        self.event_lock = true;
                    },
                    _ => {}
                }
            }
            _ => {},
        }
    }

    fn try_additional_dependency(&mut self) -> Option<(u8, ModelUuid, ModelUuid)> {
        None
    }

    fn try_construct_view(
        &mut self,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _into: &ViewUuid,
    ) -> Option<(UmlSequenceElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialUmlSequenceElement::Some(x) => {
                let x = x.clone();
                self.try_spend();
                Some((x, None))
            }
            PartialUmlSequenceElement::Diagram { a, b: Some(b), .. } => {
                self.current_stage = UmlSequenceToolStage::DiagramStart;

                let (_diagram_model, diagram_view) =
                    new_umlsequence_diagram("Diagram", Vec::new(), Vec::new(), egui::Rect::from_two_pos(*a, *b));

                self.try_spend();
                Some((diagram_view.into(), None))
            }
            PartialUmlSequenceElement::CombinedFragment {
                kind,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_view), Some(target_view)) = (
                    q.get_view_for(&source_uuid),
                    q.get_view_for(&target_uuid),
                ) && q.find_parent(&source_view.uuid(), |_, e| matches!(e, UmlSequenceElementView::Diagram(_))).map(|e| e.0)
                    == q.find_parent(&target_view.uuid(), |_, e| matches!(e, UmlSequenceElementView::Diagram(_))).map(|e| e.0) {
                    self.current_stage = self.initial_stage;

                    let section = new_umlsequence_combinedfragmentsection("", Vec::new()).into();
                    let link_view = new_umlsequence_combinedfragment(
                        *kind,
                        "",
                        [source_uuid, target_uuid].into_iter().collect(),
                        vec![section],
                    ).1.into();

                    self.try_spend();
                    Some((link_view, None))
                } else {
                    None
                }
            }
            PartialUmlSequenceElement::Link {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_view), Some(target_view)) = (
                    q.get_view_for(&source_uuid),
                    q.get_view_for(&target_uuid),
                ) && q.find_parent(&source_view.uuid(), |_, e| matches!(e, UmlSequenceElementView::Diagram(_))).map(|e| e.0)
                    == q.find_parent(&target_view.uuid(), |_, e| matches!(e, UmlSequenceElementView::Diagram(_))).map(|e| e.0) {
                    self.current_stage = self.initial_stage;

                    let link_view = match link_type {
                        LinkType::Message { synchronicity_kind, is_return, name } => {
                            new_umlsequence_message(
                                name,
                                "",
                                *synchronicity_kind,
                                UmlSequenceMessageLifecycleKind::None,
                                *is_return,
                                (source.clone(), source_view),
                                (dest.clone(), target_view),
                            ).1.into()
                        },
                    };

                    self.try_spend();
                    Some((link_view, None))
                } else {
                    None
                }
            }
            PartialUmlSequenceElement::CommentLink { source, dest: Some(dest) } => {
                let source_uuid = *source.read().uuid();
                if let (Some(source_view), Some(target_view)) = (
                    q.get_view_for(&source_uuid),
                    q.get_view_for(&dest.uuid()),
                ) {
                    self.current_stage = UmlSequenceToolStage::CommentLinkStart;

                    let (_link_model, link_view) = new_umlsequence_commentlink(
                        None,
                        (source.clone(), source_view),
                        (dest.clone(), target_view),
                    );

                    self.try_spend();
                    Some((link_view.into(), None))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}


pub fn new_umlsequence_diagram(
    name: &str,
    lifelines: Vec<(ERef<UmlSequenceLifeline>, ERef<UmlSequenceLifelineView>)>,
    horizontals: Vec<(UmlSequenceHorizontalElement, UmlSequenceHorizontalElementView)>,
    bounds_rect: egui::Rect,
) -> (ERef<UmlSequenceDiagram>, ERef<UmlSequenceDiagramView>) {
    let (lifeline_models, lifeline_views) = lifelines.into_iter().collect();
    let (horizontal_models, horizontal_views) = horizontals.into_iter().collect();
    let diagram_model = ERef::new(UmlSequenceDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        lifeline_models,
        horizontal_models,
    ));
    let package_view = new_umlsequence_diagram_view(diagram_model.clone(), lifeline_views, horizontal_views, bounds_rect);

    (diagram_model, package_view)
}
pub fn new_umlsequence_diagram_view(
    model: ERef<UmlSequenceDiagram>,
    lifeline_views: Vec<ERef<UmlSequenceLifelineView>>,
    horizontal_element_views: Vec<UmlSequenceHorizontalElementView>,
    bounds_rect: egui::Rect,
) -> ERef<UmlSequenceDiagramView> {
    ERef::new(UmlSequenceDiagramView {
        uuid: ViewUuid::now_v7().into(),
        model,
        lifeline_views,
        horizontal_element_views,
        temporaries: Default::default(),
        bounds_rect,
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
            self.bounds_rect.top()
        )
    }

    fn lifeline_insertion_place(&self, pos: egui::Pos2) -> (PositionNoT, egui::Rect) {
        let lifelines_total = self.lifeline_views.len();
        let lifeline_width = self.bounds_rect.width() / (lifelines_total.max(1) as f32);

        let selected_lifeline_idx = ((pos.x - self.bounds_rect.min.x + lifeline_width / 2.0) / lifeline_width).floor();
        let selected_lifeline_start_x = if selected_lifeline_idx <= 0.0 {
            self.bounds_rect.min.x
        } else {
            self.bounds_rect.min.x + (selected_lifeline_idx - 0.5) * lifeline_width
        };
        let selected_lifeline_width = if selected_lifeline_idx <= 0.0 || selected_lifeline_idx >= lifelines_total as f32 {
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

    fn horizontal_insertion_place(&self, pos: egui::Pos2) -> Option<(Option<PositionNoT>, ERef<UmlSequenceLifeline>, egui::Rect, egui::Rect)> {
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
        let horizontal_rect = self.bounds_rect
            .with_min_y(nearest_average - WIDTH / 2.0)
            .with_max_y(nearest_average + WIDTH / 2.0);

        Some((
            insertion_index,
            lifeline,
            lifeline_rect,
            horizontal_rect,
        ))
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
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        let child = self
            .lifeline_views
            .iter_mut()
            .flat_map(|v| v.write().show_properties(gdc, q, ui, commands).to_non_default())
            .next()
            .or_else(|| self.horizontal_element_views.iter_mut().flat_map(|v| v.show_properties(gdc, q, ui, commands).to_non_default()).next());

        if let Some(child) = child {
            child
        } else if self.temporaries.highlight.selected {
            ui.label("Model properties");

            if ui.labeled_text_edit_multiline("Name:", &mut self.temporaries.name_buffer).changed() {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    UmlSequencePropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
                ));
            }

            if ui.labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer).changed() {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    UmlSequencePropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
                ));
            }

            ui.add_space(crate::common::views::VIEW_MODEL_PROPERTIES_BLOCK_SPACING);
            ui.label("View properties");

            egui::Grid::new("size_grid").show(ui, |ui| {
                {
                    let egui::Pos2 { mut x, mut y } = self.bounds_rect.left_top();

                    ui.label("x");
                    if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                        commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), egui::Vec2::new(x - self.bounds_rect.left(), 0.0)));
                    }
                    ui.label("y");
                    if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                        commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), egui::Vec2::new(0.0, y - self.bounds_rect.top())));
                    }
                    ui.end_row();
                }

                {
                    let egui::Vec2 { mut x, mut y } = self.bounds_rect.size();

                    ui.label("width");
                    if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                        commands.push(InsensitiveCommand::ResizeSpecificElementsBy(q.selected_views(), egui::Align2::LEFT_CENTER, egui::Vec2::new(x - self.bounds_rect.width(), 0.0)));
                    }
                    ui.label("height");
                    if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                        commands.push(InsensitiveCommand::ResizeSpecificElementsBy(q.selected_views(), egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, y - self.bounds_rect.height())));
                    }
                    ui.end_row();
                }
            });

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

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;

        macro_rules! draw_children {
            () => {
                let lifelines_no = self.lifeline_views.len();
                let sliver_x = self.bounds_rect.width() / lifelines_no as f32 / 2.0;
                let max_object_height = self.lifeline_views.iter()
                    .map(|v| v.read().min_shape().bounding_box().height())
                    .max_by(|l, r| l.partial_cmp(r).unwrap())
                    .unwrap_or(0.0);
                let lifelines_y = self.bounds_rect.top() + max_object_height + canvas::CLASS_MIDDLE_FONT_SIZE;
                for (idx, v) in self.lifeline_views.iter().enumerate() {
                    let x = self.bounds_rect.min.x + (2 * idx + 1) as f32 * sliver_x;
                    let t = v.write().draw_inner(egui::Pos2::new(x, lifelines_y), self.bounds_rect.max.y, q, context, settings, canvas, tool);
                    #[allow(unused)]
                    if t != TargettingStatus::NotDrawn {
                        drawn_child_targetting = t;
                    }
                }

                const PADDING_Y: f32 = 2.0;
                let theoretical_height = PADDING_Y + self.horizontal_element_views.iter().map(|v| v.theoretical_height()).sum::<f32>();
                let sliver_y = (self.bounds_rect.height() - 2.0 * max_object_height) / theoretical_height;
                let mut counter_y = self.bounds_rect.min.y + 2.0 * max_object_height + PADDING_Y * sliver_y;
                for v in self.horizontal_element_views.iter_mut() {
                    let (t, r) = v.draw_inner(&self.lifeline_views, (counter_y, sliver_y), q, context, settings, canvas, tool);
                    #[allow(unused)]
                    if t != TargettingStatus::NotDrawn {
                        drawn_child_targetting = t;
                    }
                    counter_y = r.max.y;
                }
            };
        }
        draw_children!();

        // Draw top left pentagon
        const PENTAGON_PADDING: f32 = 4.0;
        let pentagon_bg = egui::Color32::WHITE;
        let left_top_pentagon_rect = canvas.measure_text(self.bounds_rect.left_top() + egui::Vec2::splat(PENTAGON_PADDING), egui::Align2::LEFT_TOP, &self.temporaries.display_text, canvas::CLASS_MIDDLE_FONT_SIZE).expand(PENTAGON_PADDING);
        canvas.draw_polygon([
            left_top_pentagon_rect.left_top(), left_top_pentagon_rect.right_top(),
            left_top_pentagon_rect.right_bottom() - egui::Vec2::new(0.0, PENTAGON_PADDING),
            left_top_pentagon_rect.right_bottom() - egui::Vec2::new(PENTAGON_PADDING, 0.0),
            left_top_pentagon_rect.left_bottom(),
        ].to_vec(), pentagon_bg, canvas::Stroke::new_solid(1.0, egui::Color32::BLACK), self.temporaries.highlight);
        canvas.draw_text(self.bounds_rect.left_top() + egui::Vec2::splat(PENTAGON_PADDING), egui::Align2::LEFT_TOP, &self.temporaries.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK);

        // Draw resize/drag handles
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.temporaries.highlight.selected) {
            let handle_size = self.handle_size(ui_scale);
            for (h, c) in [
                (self.bounds_rect.left_top(), "↖"),
                (self.bounds_rect.center_top(), "^"),
                (self.bounds_rect.right_top(), "↗"),
                (self.bounds_rect.left_center(), "<"),
                (self.bounds_rect.right_center(), ">"),
                (self.bounds_rect.left_bottom(), "↙"),
                (self.bounds_rect.center_bottom(), "v"),
                (self.bounds_rect.right_bottom(), "↘"),
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
                egui::Rect::from_center_size(
                    dc,
                    egui::Vec2::splat(handle_size / ui_scale),
                ),
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
                canvas.draw_line([
                    egui::Pos2::new(self.bounds_rect.min.x, self.bounds_rect.center().y),
                    egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.center().y),
                ], canvas::Stroke::new_solid(1.0, egui::Color32::BLUE), canvas::Highlight::NONE);
                canvas.draw_line([
                    egui::Pos2::new(self.bounds_rect.center().x, self.bounds_rect.min.y),
                    egui::Pos2::new(self.bounds_rect.center().x, self.bounds_rect.max.y),
                ], canvas::Stroke::new_solid(1.0, egui::Color32::BLUE), canvas::Highlight::NONE);
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
                        },
                        UmlSequenceToolStage::LinkStart { .. }
                        | UmlSequenceToolStage::LinkEnd
                        | UmlSequenceToolStage::CombinedFragmentStart { .. }
                        | UmlSequenceToolStage::CombinedFragmentEnd => {
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

                    draw_children!();

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

        self.lifeline_views.iter_mut().for_each(|v| v.write().collect_allignment(am));
        self.horizontal_element_views.iter_mut().for_each(|v| v.collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> EventHandlingStatus {
        let k_status = self.lifeline_views.iter_mut().flat_map(|v| {
            let mut w = v.write();
            let s = w.handle_event(event, ehc, q, tool, element_setup_modal, commands);
            if s != EventHandlingStatus::NotHandled {
                Some((*w.uuid(), s))
            } else {
                None
            }
        }).next().or_else(|| self.horizontal_element_views.iter_mut().flat_map(|v| {
            let s = v.handle_event_inner(&self.lifeline_views, event, ehc, q, tool, element_setup_modal, commands);
            if s != EventHandlingStatus::NotHandled {
                Some((*v.uuid(), s))
            } else {
                None
            }
        }).next());

        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if k_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) => {
                let handle_size = self.handle_size(1.0);
                if self.temporaries.highlight.selected {
                    for (a,h) in [(egui::Align2::RIGHT_BOTTOM, self.bounds_rect.left_top()),
                                (egui::Align2::CENTER_BOTTOM, self.bounds_rect.center_top()),
                                (egui::Align2::LEFT_BOTTOM, self.bounds_rect.right_top()),
                                (egui::Align2::RIGHT_CENTER, self.bounds_rect.left_center()),
                                (egui::Align2::LEFT_CENTER, self.bounds_rect.right_center()),
                                (egui::Align2::RIGHT_TOP, self.bounds_rect.left_bottom()),
                                (egui::Align2::CENTER_TOP, self.bounds_rect.center_bottom()),
                                (egui::Align2::LEFT_TOP, self.bounds_rect.right_bottom())]
                    {
                        if egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size) / ehc.ui_scale).contains(pos) {
                            self.temporaries.dragged_type_and_shape = Some((PackageDragType::Resize(a), self.bounds_rect));
                            return EventHandlingStatus::HandledByElement;
                        }
                    }
                }

                if self.min_shape().border_distance(pos) <= 2.0 / ehc.ui_scale
                    || egui::Rect::from_center_size(
                        self.drag_handle_position(ehc.ui_scale),
                        egui::Vec2::splat(handle_size) / ehc.ui_scale).contains(pos) {
                    self.temporaries.dragged_type_and_shape = Some((PackageDragType::Move, self.bounds_rect));
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            },
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
                    return k_status.map(|e| e.1).unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    let horizontal_place = self.horizontal_insertion_place(pos);
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());
                    if let Some(h) = &horizontal_place {
                        tool.add_section(h.1.clone().into());
                    }

                    if let Some((new_e, esm)) = tool.try_construct_view(q, &self.uuid) {
                        if let UmlSequenceElementView::Lifeline(_) = &new_e {
                            let pos = self.lifeline_insertion_place(pos).0;

                            commands.push(InsensitiveCommand::AddDependency(*self.uuid, 0, Some(pos), new_e.into(), true).into());
                            if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                                *element_setup_modal = esm;
                            }
                        } else if let Some(_) = new_e.clone().as_horizontal()
                                && let Some(h) = horizontal_place {
                            commands.push(InsensitiveCommand::AddDependency(*self.uuid, 1, h.0, new_e.into(), true).into());
                            if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                                *element_setup_modal = esm;
                            }
                        }
                    }

                    EventHandlingStatus::HandledByContainer
                } else if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                Highlight::SELECTED,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.temporaries.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ).into());
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            },
            InputEvent::Drag { delta, .. } => match self.temporaries.dragged_type_and_shape {
                Some((PackageDragType::Move, real_bounds)) => {
                    let translated_bounds = real_bounds.translate(delta);
                    self.temporaries.dragged_type_and_shape = Some((PackageDragType::Move, translated_bounds));
                    let translated_real_shape = canvas::NHShape::Rect { inner: translated_bounds };
                    let coerced_pos = ehc.snap_manager.coerce(translated_real_shape,
                        |e| !self.temporaries.all_elements.get(e).is_some() && !if self.temporaries.highlight.selected { ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected) } else {*e == *self.uuid}
                    );
                    let coerced_delta = coerced_pos - self.position();

                    if self.temporaries.highlight.selected {
                        commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), coerced_delta));
                    } else {
                        commands.push(InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid).collect(),
                            coerced_delta,
                        ));
                    }
                    EventHandlingStatus::HandledByElement
                },
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
                    let new_real_bounds = real_bounds + epaint::MarginF32 { left, right, top, bottom };
                    self.temporaries.dragged_type_and_shape = Some((PackageDragType::Resize(align), new_real_bounds));
                    let handle_x = match align.x() {
                        egui::Align::Min => (new_real_bounds.right(), self.bounds_rect.right()),
                        egui::Align::Center => (new_real_bounds.center().x, self.bounds_rect.center().x),
                        egui::Align::Max => (new_real_bounds.left(), self.bounds_rect.left()),
                    };
                    let handle_y = match align.y() {
                        egui::Align::Min => (new_real_bounds.bottom(), self.bounds_rect.bottom()),
                        egui::Align::Center => (new_real_bounds.center().y, self.bounds_rect.center().y),
                        egui::Align::Max => (new_real_bounds.top(), self.bounds_rect.top()),
                    };
                    let coerced_point = ehc.snap_manager.coerce(
                        canvas::NHShape::Rect { inner: egui::Rect::from_min_size(egui::Pos2::new(handle_x.0, handle_y.0), egui::Vec2::ZERO) },
                        |e| !self.temporaries.all_elements.get(e).is_some() && !ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected)
                    );
                    let coerced_delta = coerced_point - egui::Pos2::new(handle_x.1, handle_y.1);

                    commands.push(InsensitiveCommand::ResizeSpecificElementsBy(q.selected_views(), align, coerced_delta));
                    EventHandlingStatus::HandledByElement
                },
                None => EventHandlingStatus::NotHandled,
            },
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.lifeline_views.iter_mut().for_each(|v| v.write().apply_command(command, undo_accumulator, affected_models));
                self.horizontal_element_views.iter_mut().for_each(|v| v.apply_command(command, undo_accumulator, affected_models));
            };
        }
        macro_rules! resize_by {
            ($align:expr, $delta:expr) => {
                let min_delta_x = Self::MIN_SIZE.x - self.bounds_rect.width();
                let (left, right) = match $align.x() {
                    egui::Align::Min => (0.0, $delta.x.max(min_delta_x)),
                    egui::Align::Center => (0.0, 0.0),
                    egui::Align::Max => ((-$delta.x).max(min_delta_x), 0.0),
                };
                let min_delta_y = Self::MIN_SIZE.y - self.bounds_rect.height();
                let (top, bottom) = match $align.y() {
                    egui::Align::Min => (0.0, $delta.y.max(min_delta_y)),
                    egui::Align::Center => (0.0, 0.0),
                    egui::Align::Max => ((-$delta.y).max(min_delta_y), 0.0),
                };

                let r = self.bounds_rect + epaint::MarginF32{left, right, top, bottom};

                undo_accumulator.push(InsensitiveCommand::ResizeSpecificElementsTo(
                    std::iter::once(*self.uuid).collect(),
                    *$align,
                    self.bounds_rect.size(),
                ));
                self.bounds_rect = r;
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
                            self.horizontal_element_views.iter()
                                .for_each(|v| { self.temporaries.selected_direct_elements.insert(*v.uuid()); });
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
                    for k in self.lifeline_views.iter().map(|v| *v.read().uuid)
                        .chain(self.horizontal_element_views.iter().map(|v| *v.uuid()))
                        .filter(|k| uuids.contains(k)) {
                        match set {
                            true => self.temporaries.selected_direct_elements.insert(k),
                            false => self.temporaries.selected_direct_elements.remove(&k),
                        };
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected =
                    (self.temporaries.highlight.selected && *retain) || self.min_shape().contained_within(*rect);

                recurse!();
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.lifeline_views.iter_mut().for_each(|v| {
                    v.write().apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut void, affected_models);
                });
                self.horizontal_element_views.iter_mut().for_each(|v| {
                    v.apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut void, affected_models);
                });
            }
            InsensitiveCommand::ResizeSpecificElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    resize_by!(align, delta);
                }

                recurse!();
            }
            InsensitiveCommand::ResizeSpecificElementsTo(uuids, align, size) => {
                if uuids.contains(&self.uuid) {
                    let delta_naive = *size - self.bounds_rect.size();
                    let x = match align.x() {
                        egui::Align::Min => delta_naive.x,
                        egui::Align::Center => 0.0,
                        egui::Align::Max => -delta_naive.x,
                    };
                    let y = match align.y() {
                        egui::Align::Min => delta_naive.y,
                        egui::Align::Center => 0.0,
                        egui::Align::Max => -delta_naive.y,
                    };

                    resize_by!(align, egui::Vec2::new(x, y));
                }

                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self
                    .lifeline_views.iter()
                    .filter(|v| uuids.contains(&v.read().uuid))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
                    } else if let Some((b, pos)) = self.model.read().get_element_pos(&element.read().model_uuid()) {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                        *self.uuid,
                        b,
                        pos,
                        UmlSequenceElementView::from(element.clone()).into(),
                        false,
                    ));
                }
                self.lifeline_views.retain(|v| !uuids.contains(&v.read().uuid));

                for element in self
                    .horizontal_element_views.iter()
                    .filter(|v| uuids.contains(&v.uuid()))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (1, None)
                    } else if let Some((b, pos)) = self.model.read().get_element_pos(&element.model_uuid()) {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                        *self.uuid,
                        b,
                        pos,
                        element.clone().as_element_view().into(),
                        false,
                    ));
                }
                self.horizontal_element_views.retain(|v| !uuids.contains(&v.uuid()));

                recurse!();
            }
            InsensitiveCommand::PasteSpecificElements(target, _elements) => {
                if *target == *self.uuid {
                    todo!("undo = delete")
                }

                recurse!();
            },
            InsensitiveCommand::AddDependency(target, b, pos, element, into_model) => {
                if *target == *self.uuid {
                    let mut w = self.model.write();
                    if *b == 0
                        && let Ok(UmlSequenceElementView::Lifeline(view)) = element.clone().try_into() {
                        let mut vw = view.write();
                        if let Some(model_pos) = w.get_element_pos(&vw.model_uuid()).map(|e| e.1)
                            .or_else(|| if *into_model { w.insert_element(*b, *pos, vw.model()).ok() } else { None }) {
                            let uuid = *vw.uuid();
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                                *self.uuid,
                                *b,
                                uuid,
                                *into_model,
                            ));

                            if *into_model {
                                affected_models.insert(*w.uuid);
                            }
                            let mut model_transitives = HashMap::new();
                            vw.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut model_transitives);
                            affected_models.extend(model_transitives.into_keys());

                            let view_pos = {
                                let mut view_pos: PositionNoT = 0;
                                for e in &self.lifeline_views {
                                    let Some((_b, pos)) = w.get_element_pos(&e.read().model_uuid()) else {
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
                            self.lifeline_views.insert(view_pos.try_into().unwrap(), view.clone());
                        }
                    }
                    if *b == 1
                        && let Ok(mut view) = UmlSequenceElementView::try_from(element.clone()).and_then(|v| v.as_horizontal().ok_or(()))
                        && let Some(model_pos) = w.get_element_pos(&view.model_uuid()).map(|e| e.1)
                            .or_else(|| if *into_model { w.insert_element(*b, *pos, view.model()).ok() } else { None }) {
                        let uuid = *view.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                            *self.uuid,
                            *b,
                            uuid,
                            *into_model,
                        ));

                        if *into_model {
                            affected_models.insert(*w.uuid);
                        }
                        let mut model_transitives = HashMap::new();
                        view.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut model_transitives);
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
                        self.horizontal_element_views.insert(view_pos.try_into().unwrap(), view);
                    }
                }

                recurse!();
            }
            InsensitiveCommand::RemoveDependency(target, b, element, into_model) => {
                if *target == *self.uuid {
                    let mut w = self.model.write();
                    if *b == 0
                        && let Some(view) = self.lifeline_views.iter().find(|v| *v.read().uuid == *element).cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.read().model_uuid()) {
                        undo_accumulator.push(InsensitiveCommand::AddDependency(
                            *self.uuid,
                            *b,
                            Some(pos),
                            UmlSequenceElementView::from(view.clone()).into(),
                            *into_model,
                        ));

                        if *into_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.lifeline_views.retain(|v| *v.read().uuid != *element);
                    }
                    if *b == 1
                        && let Some(view) = self.horizontal_element_views.iter().find(|v| *v.uuid() == *element).cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.model_uuid()) {
                        undo_accumulator.push(InsensitiveCommand::AddDependency(
                            *self.uuid,
                            *b,
                            Some(pos),
                            view.clone().as_element_view().into(),
                            *into_model,
                        ));

                        if *into_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.horizontal_element_views.retain(|v| *v.uuid() != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {},
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
        self.lifeline_views.iter_mut().for_each(|v|
            v.write().head_count(flattened_views, &mut self.temporaries.all_elements, flattened_represented_models)
        );
        self.horizontal_element_views.iter_mut().for_each(|v|
            v.head_count(flattened_views, &mut self.temporaries.all_elements, flattened_represented_models)
        );
        for e in &self.temporaries.all_elements {
            flattened_views_status.insert(*e.0, match *e.1 {
                SelectionStatus::NotSelected if self.temporaries.highlight.selected => SelectionStatus::TransitivelySelected,
                e => e,
            });
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
            self.lifeline_views.iter().for_each(|v|
                v.read().deep_copy_walk(requested, uuid_present, tlc, c, m)
            );
            self.horizontal_element_views.iter().for_each(|v|
                v.deep_copy_walk(requested, uuid_present, tlc, c, m)
            );
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

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
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
        let lifeline_views = self.lifeline_views.iter().map(|v| {
            let r = v.read();
            r.deep_copy_clone(uuid_present, &mut inner, c, m);
            match c.get(&r.uuid) {
                Some(UmlSequenceElementView::Lifeline(l)) => l.clone(),
                _ => v.clone(),
            }
        }).collect();
        let horizontal_element_views = self.horizontal_element_views.iter().map(|v| {
            v.deep_copy_clone(uuid_present, &mut inner, c, m);
            match c.get(&v.uuid()).and_then(|v| v.clone().as_horizontal()) {
                Some(e) => e.clone(),
                _ => v.clone(),
            }
        }).collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,

            lifeline_views,
            horizontal_element_views,

            temporaries: self.temporaries.clone(),
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, UmlSequenceElementView>,
        m: &HashMap<ModelUuid, UmlSequenceElement>,
    ) {
        self.lifeline_views.iter_mut().for_each(|v|
            v.write().deep_copy_relink(c, m)
        );
        self.horizontal_element_views.iter_mut().for_each(|v|
            v.deep_copy_relink(c, m)
        );

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
    horizontal_span: HashSet<ModelUuid>,
    sections: Vec<(ERef<UmlSequenceCombinedFragmentSection>, ERef<UmlSequenceCombinedFragmentSectionView>)>,
) -> (ERef<UmlSequenceCombinedFragment>, ERef<UmlSequenceCombinedFragmentView>) {
    let (section_models, section_views) = sections.into_iter().collect();
    let package_model = ERef::new(UmlSequenceCombinedFragment::new(
        ModelUuid::now_v7().into(),
        kind,
        kind_argument.to_owned(),
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
    ERef::new(
        UmlSequenceCombinedFragmentView {
            uuid: ViewUuid::now_v7().into(),
            model,
            sections,
            bounds_rect: egui::Rect::ZERO,
            left_top_pentagon_rect: egui::Rect::ZERO,
            background_color: MGlobalColor::None,
            temporaries: Default::default(),
        }
    )
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
    temporaries: UmlSequenceCombinedFragmentViewTemporaries
}

#[derive(Clone, Default)]
struct UmlSequenceCombinedFragmentViewTemporaries {
    display_text: String,
    kind_buffer: UmlSequenceCombinedFragmentKind,
    kind_argument_buffer: String,
    comment_buffer: String,

    highlight: canvas::Highlight,
    selected_direct_elements: HashSet<ViewUuid>,
}

impl UmlSequenceCombinedFragmentView {
    const COMBINED_FRAGMENT_MARGIN_BOTTOM: f32 = 1.0;

    fn theoretical_height(&self) -> f32 {
        self.sections.iter().map(|e| e.read().theoretical_height()).sum::<f32>() + Self::COMBINED_FRAGMENT_MARGIN_BOTTOM
    }

    const BUTTON_RADIUS: f32 = 8.0;

    fn new_section_button_rect(&self, ui_scale: f32) ->  egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::splat(Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }

    fn spanned_lifeline_views<'a, 'b>(&'a self, lifeline_views: &'b [ERef<UmlSequenceLifelineView>]) -> &'b [ERef<UmlSequenceLifelineView>] {
        let r = self.model.read();
        let start = lifeline_views.iter().enumerate().find(|e| r.horizontal_span.contains(&e.1.read().model_uuid())).map(|e| e.0).unwrap_or(0);
        let end = lifeline_views.iter().enumerate().rev().find(|e| r.horizontal_span.contains(&e.1.read().model_uuid())).map(|e| e.0 + 1).unwrap_or(lifeline_views.len());
        &lifeline_views[start..end]
    }

    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        (pos_y, scale_y): (f32, f32),
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        let spanned_lifeline_views = self.spanned_lifeline_views(lifeline_views);
        let span_x = (
            spanned_lifeline_views.first().map(|e| e.read().bounds_rect.center().x).unwrap_or(0.0),
            spanned_lifeline_views.last().map(|e| e.read().bounds_rect.center().x).unwrap_or(0.0),
        );

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;
        let mut section_offsets = vec![pos_y];
        let mut acc = egui::Rect::from_min_size(egui::Pos2::new(span_x.0, pos_y), egui::Vec2::ZERO);
        for e in self.sections.iter_mut() {
            let (t, r) = e.write().draw_inner(
                spanned_lifeline_views,
                span_x,
                (acc.max.y, scale_y),
                q, context, settings, canvas, tool);
            if t != TargettingStatus::NotDrawn {
                drawn_child_targetting = t;
            }
            section_offsets.push(r.max.y);
            acc = acc.union(r);
        }

        for (idx, e) in self.sections.iter_mut().enumerate() {
            let mut w = e.write();
            w.bounds_rect = acc.with_min_y(section_offsets[idx]).with_max_y(section_offsets[idx+1]);

            canvas.draw_line(
                [egui::Pos2::new(acc.min.x, section_offsets[idx+1]), egui::Pos2::new(acc.max.x, section_offsets[idx+1])],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK), canvas::Highlight::NONE);

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
            self.bounds_rect, egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT, canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.temporaries.highlight,
        );

        // Draw top left pentagon
        const PENTAGON_PADDING: f32 = 4.0;
        let pentagon_bg = egui::Color32::WHITE;
        self.left_top_pentagon_rect = canvas.measure_text(egui::Pos2::new(self.bounds_rect.min.x + PENTAGON_PADDING, pos_y + PENTAGON_PADDING), egui::Align2::LEFT_TOP, &self.temporaries.display_text, canvas::CLASS_MIDDLE_FONT_SIZE).expand(PENTAGON_PADDING);
        canvas.draw_polygon([
            self.left_top_pentagon_rect.left_top(), self.left_top_pentagon_rect.right_top(),
            self.left_top_pentagon_rect.right_bottom() - egui::Vec2::new(0.0, PENTAGON_PADDING),
            self.left_top_pentagon_rect.right_bottom() - egui::Vec2::new(PENTAGON_PADDING, 0.0),
            self.left_top_pentagon_rect.left_bottom(),
        ].to_vec(), pentagon_bg, canvas::Stroke::new_solid(1.0, egui::Color32::BLACK), self.temporaries.highlight);
        canvas.draw_text(egui::Pos2::new(self.bounds_rect.min.x + PENTAGON_PADDING, pos_y + PENTAGON_PADDING), egui::Align2::LEFT_TOP, &self.temporaries.display_text,
            canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK);

        // Draw buttons
        if self.temporaries.highlight.selected && let Some(ui_scale) = canvas.ui_scale() {
            let b1 = self.new_section_button_rect(ui_scale);
            canvas.draw_rectangle(
                b1,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b1.center(), egui::Align2::CENTER_CENTER, "+", 14.0 / ui_scale, egui::Color32::BLACK);
        }

        (
            drawn_child_targetting,
            self.bounds_rect.with_max_y(self.bounds_rect.max.y + Self::COMBINED_FRAGMENT_MARGIN_BOTTOM * scale_y),
        )
    }

    fn handle_event_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        event: InputEvent,
        ehc: &EventHandlingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.temporaries.highlight.selected && self.new_section_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveUmlSequenceTool {
                    initial_stage: UmlSequenceToolStage::CombinedFragmentSection,
                    current_stage: UmlSequenceToolStage::CombinedFragmentSection,
                    result: PartialUmlSequenceElement::None,
                    event_lock: false,
                    is_spent: None,
                });

                if let Some(tool) = tool {
                    tool.add_section(self.model());
                    if let Some((view, esm)) = tool.try_construct_view(q, &self.uuid)
                        && matches!(view, UmlSequenceElementView::CombinedFragmentSection(_)) {
                        commands.push(InsensitiveCommand::AddDependency(*self.uuid, 1, None, view.into(), true).into());
                        if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            *element_setup_modal = esm;
                        }
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.left_top_pentagon_rect.contains(pos) => {
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.bounds_rect.contains(pos) => {
                let spanned_lifeline_views = self.spanned_lifeline_views(lifeline_views);
                let k_status = self.sections.iter()
                        .map(|e| {
                            let mut w = e.write();
                            (*w.uuid, w.handle_event_inner(spanned_lifeline_views, event, ehc, q, tool, element_setup_modal, commands))
                        })
                        .find(|e| e.1 != EventHandlingStatus::NotHandled);

                if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                Highlight::SELECTED,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.temporaries.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ).into());
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            },
            _ => EventHandlingStatus::NotHandled,
        }
    }
}

impl Entity for UmlSequenceCombinedFragmentView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).clone().into()
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
            inner: self.bounds_rect
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.bounds_rect.center()
    }
}

fn combined_fragment_display_text(kind: UmlSequenceCombinedFragmentKind, kind_argument: &str) -> String {
    match kind {
        UmlSequenceCombinedFragmentKind::Loop if !kind_argument.is_empty()
            => format!("{}({})", kind.char(), kind_argument),
        UmlSequenceCombinedFragmentKind::Ignore
        | UmlSequenceCombinedFragmentKind::Consider if !kind_argument.is_empty()
            => format!("{}{{{}}}", kind.char(), kind_argument),
        a => a.char().to_owned(),
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
        self.draw_inner(&Vec::new(), (0.0, 1.0), q, context, settings, canvas, tool);
        TargettingStatus::NotDrawn
    }

    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if let Some(child) = self.sections.iter_mut()
            .filter_map(|v| v.write().show_properties(drawing_context, q, ui, commands).to_non_default())
            .next()
        {
            return child;
        }

        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("Kind:")
            .selected_text(self.temporaries.kind_buffer.name())
            .show_ui(ui, |ui| {
                for e in [
                    UmlSequenceCombinedFragmentKind::Opt,
                    UmlSequenceCombinedFragmentKind::Alt,
                    UmlSequenceCombinedFragmentKind::Loop,
                    UmlSequenceCombinedFragmentKind::Break,
                    UmlSequenceCombinedFragmentKind::Par,
                    UmlSequenceCombinedFragmentKind::Strict,
                    UmlSequenceCombinedFragmentKind::Seq,
                    UmlSequenceCombinedFragmentKind::Critical,
                    UmlSequenceCombinedFragmentKind::Ignore,
                    UmlSequenceCombinedFragmentKind::Consider,
                    UmlSequenceCombinedFragmentKind::Assert,
                    UmlSequenceCombinedFragmentKind::Neg,
                ] {
                    if ui.selectable_value(&mut self.temporaries.kind_buffer, e, e.char()).clicked() {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::CombinedFragmentKindChange(self.temporaries.kind_buffer.clone()),
                        ));
                    }
                }
            });

        if ui.labeled_text_edit_singleline("Kind argument:", &mut self.temporaries.kind_argument_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CombinedFragmentKindArgumentChange(Arc::new(self.temporaries.kind_argument_buffer.clone())),
            ));
        }

        if ui.labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ));
        }

        PropertiesStatus::Shown
    }

    fn handle_event(
        &mut self,
        _event: InputEvent,
        _ehc: &EventHandlingContext,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>>,
    ) -> EventHandlingStatus {
        unreachable!()
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.sections.iter().for_each(|s| s.write().apply_command(command, undo_accumulator, affected_models));
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
                    for k in self.sections.iter().map(|v| *v.read().uuid).filter(|k| uuids.contains(k)) {
                        match set {
                            true => self.temporaries.selected_direct_elements.insert(k),
                            false => self.temporaries.selected_direct_elements.remove(&k),
                        };
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected =
                    (self.temporaries.highlight.selected && *retain) || self.min_shape().contained_within(*rect);

                recurse!();
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.sections.iter_mut().for_each(|v| {
                    v.write().apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut void, affected_models);
                });
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..) => {
                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self.sections.iter().filter(|v| uuids.contains(&v.read().uuid)) {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (1, None)
                    } else if let Some((b, pos)) = self.model.read().get_element_pos(&element.read().model_uuid()) {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                        *self.uuid,
                        b,
                        pos,
                        UmlSequenceElementView::from(element.clone()).into(),
                        false,
                    ));
                }
                self.sections.retain(|v| !uuids.contains(&v.read().uuid));

                recurse!();
            }
            InsensitiveCommand::PasteSpecificElements(target, _elements) => {
                if *target == *self.uuid {
                    todo!("undo = delete")
                }

                recurse!();
            },
            InsensitiveCommand::AddDependency(target, b, pos, element, into_model) => {
                if *target == *self.uuid {
                    let mut w = self.model.write();
                    if *b == 1
                        && let Ok(UmlSequenceElementView::CombinedFragmentSection(view)) = element.clone().try_into() {
                        let mut vw = view.write();
                        if let Some(model_pos) = w.get_element_pos(&vw.model_uuid()).map(|e| e.1)
                            .or_else(|| if *into_model { w.insert_element(*b, *pos, vw.model()).ok() } else { None }) {
                            let uuid = *vw.uuid;
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                                *self.uuid,
                                *b,
                                uuid,
                                *into_model,
                            ));

                            if *into_model {
                                affected_models.insert(*w.uuid);
                            }
                            let mut model_transitives = HashMap::new();
                            vw.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut model_transitives);
                            affected_models.extend(model_transitives.into_keys());

                            let view_pos = {
                                let mut view_pos: PositionNoT = 0;
                                for e in &self.sections {
                                    let Some((_b, pos)) = w.get_element_pos(&e.read().model_uuid()) else {
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
                            self.sections.insert(view_pos.try_into().unwrap(), view.clone());
                        }
                    }
                }

                recurse!();
            }
            InsensitiveCommand::RemoveDependency(target, b, element, into_model) => {
                if *target == *self.uuid {
                    let mut w = self.model.write();
                    if *b == 1
                        && let Some(view) = self.sections.iter().find(|v| *v.read().uuid == *element).cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.read().model_uuid()) {
                        undo_accumulator.push(InsensitiveCommand::AddDependency(
                            *self.uuid,
                            *b,
                            Some(pos),
                            UmlSequenceElementView::from(view.clone()).into(),
                            *into_model,
                        ));

                        if *into_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.sections.retain(|v| *v.read().uuid != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {},
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    match property {
                        UmlSequencePropChange::CombinedFragmentKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CombinedFragmentKindChange(model.kind.clone()),
                            ));
                            model.kind = kind.clone();
                        }
                        UmlSequencePropChange::CombinedFragmentKindArgumentChange(kind_argument) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CombinedFragmentKindArgumentChange(model.kind_argument.clone()),
                            ));
                            model.kind_argument = kind_argument.clone();
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
        }
    }

    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.temporaries.display_text = combined_fragment_display_text(model.kind, &model.kind_argument);
        self.temporaries.kind_buffer = model.kind.clone();
        self.temporaries.kind_argument_buffer = (*model.kind_argument).clone();
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
            w.head_count(flattened_views, flattened_views_status, flattened_represented_models);
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
            self.sections.iter().for_each(|v|
                v.read().deep_copy_walk(requested, uuid_present, tlc, c, m)
            );
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

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
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
        let new_sections = self.sections.iter().map(|v| {
            let v = v.read();
            v.deep_copy_clone(uuid_present, &mut inner, c, m);
            let Some(UmlSequenceElementView::CombinedFragmentSection(s)) = c.get(&v.uuid) else {
                unreachable!()
            };
            s.clone()
        }).collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            sections: new_sections,

            bounds_rect: self.bounds_rect.clone(),
            left_top_pentagon_rect: self.left_top_pentagon_rect.clone(),
            background_color: self.background_color.clone(),
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
        self.sections.iter_mut().for_each(|v|
            v.write().deep_copy_relink(c, m)
        );

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
    horizontals: Vec<(UmlSequenceHorizontalElement, UmlSequenceHorizontalElementView)>,
) -> (ERef<UmlSequenceCombinedFragmentSection>, ERef<UmlSequenceCombinedFragmentSectionView>) {
    let (child_models, child_views) = horizontals.into_iter().collect();
    let section_model = ERef::new(UmlSequenceCombinedFragmentSection::new(
        ModelUuid::now_v7().into(),
        guard.to_owned(),
        child_models,
    ));
    let section_view = new_umlsequence_combinedfragmentsection_view(section_model.clone(), child_views);

    (section_model, section_view)
}
pub fn new_umlsequence_combinedfragmentsection_view(
    model: ERef<UmlSequenceCombinedFragmentSection>,
    horizontal_element_views: Vec<UmlSequenceHorizontalElementView>,
) -> ERef<UmlSequenceCombinedFragmentSectionView> {
    ERef::new(
        UmlSequenceCombinedFragmentSectionView {
            uuid: ViewUuid::now_v7().into(),
            model,
            horizontal_element_views,
            bounds_rect: egui::Rect::ZERO,
            background_color: MGlobalColor::None,
            temporaries: Default::default(),
        }
    )
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
    const SECTION_EMPTY_SIZE_Y: f32 = 4.0;
    const SECTION_PADDING_Y: f32 = 2.0;

    fn theoretical_height(&self) -> f32 {
        if self.horizontal_element_views.is_empty() {
            Self::SECTION_PADDING_Y + Self::SECTION_EMPTY_SIZE_Y
        } else {
            Self::SECTION_PADDING_Y + self.horizontal_element_views.iter().map(|v| v.theoretical_height()).sum::<f32>()
        }
    }

    // Does not draw the outer rectangle, because it doesn't know the sizes of sibling sections
    fn draw_inner(
        &mut self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        (min_lifeline_x, max_lifeline_x): (f32, f32),
        (pos_y, scale_y): (f32, f32),
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &UmlSequenceSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlSequenceTool)>,
    ) -> (TargettingStatus, egui::Rect) {
        canvas.draw_text(egui::Pos2::new(self.bounds_rect.center().x, pos_y + 2.0), egui::Align2::CENTER_TOP,
            &self.temporaries.display_text, canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK);

        let (drawn_child_targetting, rect) = if self.horizontal_element_views.is_empty() {
            (
                TargettingStatus::NotDrawn,
                egui::Rect::from_two_pos(
                    egui::Pos2::new(min_lifeline_x - Self::SECTION_PADDING_X, pos_y),
                    egui::Pos2::new(max_lifeline_x + Self::SECTION_PADDING_X, pos_y + (Self::SECTION_PADDING_Y + Self::SECTION_EMPTY_SIZE_Y) * scale_y),
                ),
            )
        } else {
            let mut drawn_child_targetting = TargettingStatus::NotDrawn;
            let mut acc = egui::Rect::from_min_max(
                egui::Pos2::new(min_lifeline_x, pos_y),
                egui::Pos2::new(max_lifeline_x, pos_y + Self::SECTION_PADDING_Y * scale_y),
            );
            for e in self.horizontal_element_views.iter_mut() {
                let (t, r) = e.draw_inner(lifeline_views, (acc.max.y, scale_y), q, context, settings, canvas, tool);
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
                    | UmlSequenceToolStage::CombinedFragmentEnd => {
                        if let Some((.., lr, hr)) = self.horizontal_insertion_place(lifeline_views, *pos) {
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
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.bounds_rect.contains(pos) => {
                let k_status = self.horizontal_element_views.iter_mut()
                    .map(|e| {
                        (*e.uuid(), e.handle_event_inner(lifeline_views, event, ehc, q, tool, element_setup_modal, commands))
                    })
                    .find(|e| e.1 != EventHandlingStatus::NotHandled);

                if let Some(tool) = tool {
                    let horizontal_place = self.horizontal_insertion_place(lifeline_views, pos);
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());
                    if let Some(h) = &horizontal_place {
                        tool.add_section(h.1.clone().into());
                    }

                    if let Some((new_e, esm)) = tool.try_construct_view(q, &self.uuid) {
                        if let Some(_) = new_e.clone().as_horizontal()
                                && let Some(h) = horizontal_place {
                            commands.push(InsensitiveCommand::AddDependency(*self.uuid, 1, h.0, new_e.into(), true).into());
                            if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                                *element_setup_modal = esm;
                            }
                        }
                    }

                    EventHandlingStatus::HandledByContainer
                } else if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                Highlight::SELECTED,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.temporaries.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ).into());
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            },
            _ => EventHandlingStatus::NotHandled,
        }
    }

    // TODO: deduplicate?
    fn horizontal_insertion_place(
        &self,
        lifeline_views: &[ERef<UmlSequenceLifelineView>],
        pos: egui::Pos2,
    ) -> Option<(Option<PositionNoT>, ERef<UmlSequenceLifeline>, egui::Rect, egui::Rect)> {
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
        let horizontal_rect = self.bounds_rect
            .with_min_y(nearest_average - WIDTH / 2.0)
            .with_max_y(nearest_average + WIDTH / 2.0);

        Some((
            insertion_index,
            lifeline,
            lifeline_rect,
            horizontal_rect,
        ))
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
        commands: &mut Vec<InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>>,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if let Some(child) = self.horizontal_element_views.iter_mut()
            .filter_map(|v| v.show_properties(drawing_context, q, ui, commands).to_non_default())
            .next()
        {
            return child;
        }

        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if ui.labeled_text_edit_singleline("Guard:", &mut self.temporaries.guard_buffer).changed() {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    UmlSequencePropChange::CombinedFragmentSectionGuardChange(self.temporaries.guard_buffer.clone().into()),
                )
            );
        }

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
        self.draw_inner(&Vec::new(), (0.0, 100.0), (0.0, 1.0), q, context, settings, canvas, tool).0
    }

    fn handle_event(
        &mut self,
        _event: InputEvent,
        _ehc: &EventHandlingContext,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<NaiveUmlSequenceTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> EventHandlingStatus {
        unreachable!()
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.horizontal_element_views.iter_mut().for_each(|v| v.apply_command(command, undo_accumulator, affected_models));
            };
        }
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.temporaries.highlight = self.temporaries.highlight.combine(*set, *h);
                if h.selected {
                    match set {
                        true => {
                            self.temporaries.selected_direct_elements =
                                self.horizontal_element_views.iter().map(|v| *v.uuid()).collect();
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
                    for k in self.horizontal_element_views.iter().map(|v| *v.uuid()).filter(|k| uuids.contains(k)) {
                        match set {
                            true => self.temporaries.selected_direct_elements.insert(k),
                            false => self.temporaries.selected_direct_elements.remove(&k),
                        };
                    }
                }

                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.temporaries.highlight.selected =
                    (self.temporaries.highlight.selected && *retain) || self.min_shape().contained_within(*rect);

                recurse!();
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.horizontal_element_views.iter_mut().for_each(|v| {
                    v.apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut void, affected_models);
                });
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..) => {
                recurse!();
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for element in self.horizontal_element_views.iter().filter(|v| uuids.contains(&v.uuid())) {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (1, None)
                    } else if let Some((b, pos)) = self.model.read().get_element_pos(&element.model_uuid()) {
                        (b, Some(pos))
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                        *self.uuid,
                        b,
                        pos,
                        element.clone().as_element_view().into(),
                        false,
                    ));
                }
                self.horizontal_element_views.retain(|v| !uuids.contains(&v.uuid()));

                recurse!();
            }
            InsensitiveCommand::PasteSpecificElements(target, _elements) => {
                if *target == *self.uuid {
                    todo!("undo = delete")
                }

                recurse!();
            },
            InsensitiveCommand::AddDependency(target, b, pos, element, into_model) => {
                if *target == *self.uuid {
                    let mut w = self.model.write();
                    if *b == 1
                        && let Ok(mut view) = UmlSequenceElementView::try_from(element.clone()).and_then(|v| v.as_horizontal().ok_or(()))
                        && let Some(model_pos) = w.get_element_pos(&view.model_uuid()).map(|e| e.1)
                            .or_else(|| if *into_model { w.insert_element(*b, *pos, view.model()).ok() } else { None }) {
                        let uuid = *view.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                            *self.uuid,
                            *b,
                            uuid,
                            *into_model,
                        ));

                        if *into_model {
                            affected_models.insert(*w.uuid);
                        }
                        let mut model_transitives = HashMap::new();
                        view.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut model_transitives);
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
                        self.horizontal_element_views.insert(view_pos.try_into().unwrap(), view.clone());
                    }
                }

                recurse!();
            }
            InsensitiveCommand::RemoveDependency(target, b, element, into_model) => {
                if *target == *self.uuid {
                    let mut w = self.model.write();
                    if *b == 1
                        && let Some(view) = self.horizontal_element_views.iter().find(|v| *v.uuid() == *element).cloned()
                        && let Some((_b, pos)) = w.remove_element(&view.model_uuid()) {
                        undo_accumulator.push(InsensitiveCommand::AddDependency(
                            *self.uuid,
                            *b,
                            Some(pos),
                            view.clone().as_element_view().into(),
                            *into_model,
                        ));

                        if *into_model {
                            affected_models.insert(*w.uuid);
                        }

                        self.horizontal_element_views.retain(|v| *v.uuid() != *element);
                    }
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(_uuids, _arr) => {},
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&self.uuid) {
                    let mut model = self.model.write();
                    affected_models.insert(*model.uuid);
                    match property {
                        UmlSequencePropChange::CombinedFragmentSectionGuardChange(guard) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::CombinedFragmentSectionGuardChange(model.guard.clone()),
                            ));
                            model.guard = guard.clone();
                        }
                        _ => {}
                    }
                }
                recurse!();
            }
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
            v.head_count(flattened_views, flattened_views_status, flattened_represented_models);
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

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let model = if let Some(UmlSequenceElement::CombinedFragmentSection(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut inner = HashMap::new();
        let horizontal_element_views = self.horizontal_element_views.iter().map(|v| {
            v.deep_copy_clone(uuid_present, &mut inner, c, m);
            let Some(e) = c.get(&v.uuid()).and_then(|e| e.clone().as_horizontal()) else {
                unreachable!()
            };
            e.clone()
        }).collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model,
            horizontal_element_views,

            bounds_rect: self.bounds_rect.clone(),
            background_color: self.background_color.clone(),
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
        self.horizontal_element_views.iter_mut().for_each(|v|
            v.deep_copy_relink(c, m)
        );

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
) -> (ERef<UmlSequenceLifeline>, ERef<UmlSequenceLifelineView>) {
    let class_model = ERef::new(UmlSequenceLifeline::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        stereotype.to_owned(),
    ));
    let class_view = new_umlsequence_lifeline_view(
        class_model.clone(),
        render_style,
    );

    (class_model, class_view)
}
pub fn new_umlsequence_lifeline_view(
    model: ERef<UmlSequenceLifeline>,
    render_style: UmlSequenceLifelineRenderStyle,
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
        background_color: MGlobalColor::None,
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
    pub fn char(&self) -> &'static str {
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

        let body_color = context.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE);
        let s = canvas::Stroke::new_solid(1.0, egui::Color32::BLACK);
        let h = self.highlight;
        match self.render_style {
            UmlSequenceLifelineRenderStyle::StickFigure => {
                canvas.draw_ellipse(pos - egui::Vec2::new(0.0, 20.0), egui::Vec2::splat(10.0), body_color, s, h);
                canvas.draw_line([pos - egui::Vec2::new(20.0, 4.0), pos - egui::Vec2::new(-20.0, 4.0)], s, h); // hands
                canvas.draw_line([pos - egui::Vec2::new(0.0, 10.0), pos - egui::Vec2::new(0.0, -8.0)], s, h); // torso
                canvas.draw_line([pos - egui::Vec2::new(16.0, -28.0), pos - egui::Vec2::new(0.0, -8.0)], s, h); // / leg
                canvas.draw_line([pos - egui::Vec2::new(-16.0, -28.0), pos - egui::Vec2::new(0.0, -8.0)], s, h); // \ leg

                self.bounds_rect = egui::Rect::from_min_max(pos - egui::Vec2::new(20.0, 30.0), pos + egui::Vec2::new(20.0, 28.0));
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0), egui::Align2::CENTER_TOP,
                    &read.name, canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK,
                );

                canvas.draw_line([pos + egui::Vec2::new(0.0, self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE), egui::Pos2::new(pos.x, max_y)], canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK), h);
            },
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

                canvas.draw_line([self.bounds_rect.center_bottom(), egui::Pos2::new(pos.x, max_y)], canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK), h);
            },
            UmlSequenceLifelineRenderStyle::Boundary => {
                const CIRCLE_RADIUS: f32 = 15.0;
                canvas.draw_line([pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, CIRCLE_RADIUS), pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, -CIRCLE_RADIUS)], s, h); // vertical
                canvas.draw_line([pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, 0.0), pos], s, h); // horizontal
                canvas.draw_ellipse(pos + egui::Vec2::new(CIRCLE_RADIUS, 0.0), egui::Vec2::splat(CIRCLE_RADIUS), body_color, s, h);

                self.bounds_rect = egui::Rect::from_min_max(pos - egui::Vec2::new(2.0 * CIRCLE_RADIUS, CIRCLE_RADIUS), pos + egui::Vec2::new(2.0 * CIRCLE_RADIUS, CIRCLE_RADIUS));
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0), egui::Align2::CENTER_TOP,
                    &read.name, canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK,
                );

                canvas.draw_line([pos + egui::Vec2::new(0.0, self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE), egui::Pos2::new(pos.x, max_y)], canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK), h);
            },
            UmlSequenceLifelineRenderStyle::Control => {
                const CIRCLE_RADIUS: f32 = 15.0;
                const ARROW_LENGTH: f32 = 5.0;
                canvas.draw_ellipse(pos, egui::Vec2::splat(CIRCLE_RADIUS), body_color, s, h);
                canvas.draw_line([pos - egui::Vec2::new(0.0, CIRCLE_RADIUS), pos - egui::Vec2::new(-ARROW_LENGTH, CIRCLE_RADIUS + ARROW_LENGTH)], s, h); // up
                canvas.draw_line([pos - egui::Vec2::new(0.0, CIRCLE_RADIUS), pos - egui::Vec2::new(-ARROW_LENGTH, CIRCLE_RADIUS - ARROW_LENGTH)], s, h); // down

                self.bounds_rect = egui::Rect::from_center_size(pos, egui::Vec2::new(2.0 * CIRCLE_RADIUS, 2.0 * CIRCLE_RADIUS));
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0), egui::Align2::CENTER_TOP,
                    &read.name, canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK,
                );

                canvas.draw_line([pos + egui::Vec2::new(0.0, self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE), egui::Pos2::new(pos.x, max_y)], canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK), h);
            },
            UmlSequenceLifelineRenderStyle::Entity => {
                const CIRCLE_RADIUS: f32 = 15.0;
                canvas.draw_ellipse(pos, egui::Vec2::splat(CIRCLE_RADIUS), body_color, s, h);
                canvas.draw_line([pos - egui::Vec2::new(CIRCLE_RADIUS, -CIRCLE_RADIUS - 1.0), pos - egui::Vec2::new(-CIRCLE_RADIUS, -CIRCLE_RADIUS - 1.0)], s, h); // horizontal

                self.bounds_rect = egui::Rect::from_center_size(pos, egui::Vec2::new(2.0 * CIRCLE_RADIUS, 2.0 * CIRCLE_RADIUS));
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0), egui::Align2::CENTER_TOP,
                    &read.name, canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK,
                );

                canvas.draw_line([pos + egui::Vec2::new(0.0, self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE), egui::Pos2::new(pos.x, max_y)], canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK), h);
            },
            UmlSequenceLifelineRenderStyle::Database => {
                const ELLIPSE_RADIUS: egui::Vec2 = egui::Vec2::new(20.0, 10.0);
                canvas.draw_ellipse(pos + egui::Vec2::new(0.0, ELLIPSE_RADIUS.y), ELLIPSE_RADIUS, body_color, s, h); // bottom
                canvas.draw_rectangle(
                    egui::Rect::from_min_max(pos - ELLIPSE_RADIUS, pos + ELLIPSE_RADIUS),
                    egui::CornerRadius::ZERO,
                    body_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                ); // fill
                canvas.draw_line([pos - ELLIPSE_RADIUS, pos - egui::Vec2::new(ELLIPSE_RADIUS.x, -ELLIPSE_RADIUS.y)], s, h); // left
                canvas.draw_line([pos + egui::Vec2::new(ELLIPSE_RADIUS.x, -ELLIPSE_RADIUS.y), pos + ELLIPSE_RADIUS], s, h); // right
                canvas.draw_ellipse(pos - egui::Vec2::new(0.0, ELLIPSE_RADIUS.y), ELLIPSE_RADIUS, body_color, s, h); // top

                self.bounds_rect = egui::Rect::from_center_size(pos, 2.0 * ELLIPSE_RADIUS);
                canvas.draw_text(
                    pos - egui::Vec2::new(0.0, -28.0), egui::Align2::CENTER_TOP,
                    &read.name, canvas::CLASS_MIDDLE_FONT_SIZE, egui::Color32::BLACK,
                );

                canvas.draw_line([pos + egui::Vec2::new(0.0, self.bounds_rect.height() / 2.0 + canvas::CLASS_MIDDLE_FONT_SIZE), egui::Pos2::new(pos.x, max_y)], canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK), h);
            },
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

pub fn draw_simple_uml_class<'a>(
    canvas: &'a mut dyn canvas::NHCanvas,
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
                &top_label,
                canvas::CLASS_TOP_FONT_SIZE,
            );
            offsets.push(r.height());
            max_width = max_width.max(r.width());
        }

        {
            let r = canvas.measure_text(
                egui::Pos2::ZERO,
                egui::Align2::CENTER_TOP,
                &main_label,
                canvas::CLASS_MIDDLE_FONT_SIZE,
            );
            offsets.push(r.height());
            max_width = max_width.max(r.width());
        }

        if let Some(bottom_label) = &bottom_label {
            let r = canvas.measure_text(
                egui::Pos2::ZERO,
                egui::Align2::CENTER_TOP,
                &bottom_label,
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
        canvas.draw_rectangle(rect, egui::CornerRadius::ZERO, fill, stroke.into(), highlight);

        (
            offsets,
            global_offset,
            rect,
        )
    };

    // Draw phase
    {
        let mut offset_counter = 0;

        if let Some(top_label) = &top_label {
            canvas.draw_text(
                position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                egui::Align2::CENTER_TOP,
                &top_label,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
            offset_counter += 1;
        }

        {
            canvas.draw_text(
                position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                egui::Align2::CENTER_TOP,
                &main_label,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
            offset_counter += 1;
        }

        if let Some(bottom_label) = &bottom_label {
            canvas.draw_text(
                position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                egui::Align2::CENTER_TOP,
                &bottom_label,
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
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui.labeled_text_edit_singleline("Stereotype:", &mut self.stereotype_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::StereotypeChange(self.stereotype_buffer.clone().into()),
            ));
        }

        if ui.labeled_text_edit_multiline("Name:", &mut self.name_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui.labeled_text_edit_multiline("Comment:", &mut self.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }

        ui.label("View properties");

        ui.label("Background color:");
        if crate::common::controller::mglobalcolor_edit_button(
            &drawing_context.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        ui.label("Render style");
        egui::ComboBox::from_id_salt("render style")
            .selected_text(self.render_style.char())
            .show_ui(ui, |ui| {
                for e in [
                    UmlSequenceLifelineRenderStyle::StickFigure,
                    UmlSequenceLifelineRenderStyle::Object,
                    UmlSequenceLifelineRenderStyle::Boundary,
                    UmlSequenceLifelineRenderStyle::Control,
                    UmlSequenceLifelineRenderStyle::Entity,
                    UmlSequenceLifelineRenderStyle::Database,
                ] {
                    ui.selectable_value(&mut self.render_style, e, e.char());
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
        self.draw_inner(self.bounds_rect.center(), self.bounds_rect.bottom() + self.bounds_rect.height(), q, context, settings, canvas, tool)
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model());
                } else {
                    if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
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
        command: &InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
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
                self.highlight.selected =
                    (self.highlight.selected && *retain) || self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _)
                if !uuids.contains(&*self.uuid) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.bounds_rect = self.bounds_rect.translate(*delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        UmlSequencePropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlSequencePropChange::StereotypeChange(
                                    model.stereotype.clone(),
                                ),
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
                                UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
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

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
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


pub fn message_label_format(lifecycle: UmlSequenceMessageLifecycleKind, name: &str) -> Option<Arc<String>> {
    if lifecycle == UmlSequenceMessageLifecycleKind::None && name.is_empty() {
        None
    } else {
        let mut label = match lifecycle {
            UmlSequenceMessageLifecycleKind::None => "".to_owned(),
            UmlSequenceMessageLifecycleKind::Create => "«create»\n".to_owned(),
            UmlSequenceMessageLifecycleKind::Delete => "«destroy»\n".to_owned(),
        };
        if !name.is_empty() {
            label.push_str(name);
            label.push_str("\n");
        }
        let newlines_count = label.chars().filter(|e| *e == '\n').count();
        for _ in 1..newlines_count {
            label.push_str("\n");
        }
        Some(label.into())
    }
}


pub fn new_umlsequence_message(
    name: &str,
    state_invariant: &str,
    synchronicity: UmlSequenceMessageSynchronicityKind,
    lifecycle: UmlSequenceMessageLifecycleKind,
    is_return: bool,
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
        source.0,
        target.0,
    ));
    let link_view = new_umlsequence_message_view(link_model.clone(), source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlsequence_message_view(
    model: ERef<UmlSequenceMessage>,
    source: UmlSequenceElementView,
    target: UmlSequenceElementView,
) -> ERef<UmlSequenceMessageView> {
    ERef::new(
        UmlSequenceMessageView {
            uuid: ViewUuid::now_v7().into(),
            model,
            source,
            target,
            bounds_rect: egui::Rect::ZERO,
            temporaries: Default::default(),
        },
    )
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

    display_text: String,
    name_buffer: String,
    synchronicity_kind_buffer: UmlSequenceMessageSynchronicityKind,
    lifecycle_kind_buffer: UmlSequenceMessageLifecycleKind,
    is_return_buffer: bool,
    state_invariant_buffer: String,
    comment_buffer: String,
    highlight: canvas::Highlight,
}

impl UmlSequenceMessageView {
    const MESSAGE_SPACING: f32 = 4.0;

    fn theoretical_height(&self) -> f32 {
        Self::MESSAGE_SPACING
    }
    fn draw_inner(
        &mut self,
        (pos_y, scale_y): (f32, f32),
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
            const WIDTH: f32 = 20.0;
            let start = egui::Pos2::new(source_x, pos_y + scale_y * Self::MESSAGE_SPACING / 2.0 - WIDTH);
            let end = egui::Pos2::new(target_x, pos_y + scale_y * Self::MESSAGE_SPACING / 2.0 + WIDTH);
            let second = egui::Pos2::new(start.x + WIDTH, start.y);
            let penultimate = egui::Pos2::new(end.x + WIDTH, end.y);

            canvas.draw_line([start, second], s, self.temporaries.highlight);
            canvas.draw_line([second, penultimate], s, self.temporaries.highlight);

            self.bounds_rect = egui::Rect::from_two_pos(start, penultimate);
            (start, second, penultimate, end)
        } else {
            let start = egui::Pos2::new(source_x, pos_y + scale_y * Self::MESSAGE_SPACING / 2.0);
            let end = egui::Pos2::new(target_x, pos_y + scale_y * Self::MESSAGE_SPACING / 2.0);
            self.bounds_rect = egui::Rect::from_two_pos(start, end);
            (start, end, start, end)
        };

        let end_intersect = self.temporaries.target_arrow_type.get_intersect(end, penultimate);
        canvas.draw_line(
            [penultimate, end_intersect],
            s,
            self.temporaries.highlight,
        );
        self.temporaries.target_arrow_type.draw_in(canvas, end, penultimate, self.temporaries.highlight);

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
            egui::Vec2::new(lifeline_diff_x, scale_y * Self::MESSAGE_SPACING),
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
        NHShape::Rect { inner: self.bounds_rect }
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
        self.draw_inner((0.0, 1.0), q, context, settings, canvas, tool);
        TargettingStatus::NotDrawn
    }

    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if !self.temporaries.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if ui.labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }

        ui.label("Synchronicity:");
        egui::ComboBox::from_id_salt("synchronicity")
            .selected_text(self.temporaries.synchronicity_kind_buffer.char())
            .show_ui(ui, |ui| {
                for e in [UmlSequenceMessageSynchronicityKind::Synchronous, UmlSequenceMessageSynchronicityKind::AsynchronousCall, UmlSequenceMessageSynchronicityKind::AsynchronousSignal] {
                    if ui
                        .selectable_value(&mut self.temporaries.synchronicity_kind_buffer, e, e.char())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::SynchronicityKindChange(self.temporaries.synchronicity_kind_buffer),
                        ));
                    }
                }
            });

        ui.label("Lifecycle:");
        egui::ComboBox::from_id_salt("lifecycle")
            .selected_text(self.temporaries.lifecycle_kind_buffer.char())
            .show_ui(ui, |ui| {
                for e in [UmlSequenceMessageLifecycleKind::None, UmlSequenceMessageLifecycleKind::Create, UmlSequenceMessageLifecycleKind::Delete] {
                    if ui
                        .selectable_value(&mut self.temporaries.lifecycle_kind_buffer, e, e.char())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlSequencePropChange::LifecycleKindChange(self.temporaries.lifecycle_kind_buffer),
                        ));
                    }
                }
            });

        ui.label("isReturn:");
        if ui.checkbox(&mut self.temporaries.is_return_buffer, "isReturn").changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::IsReturnChange(self.temporaries.is_return_buffer),
            ));
        }

        if ui.labeled_text_edit_multiline("State invariant:", &mut self.temporaries.state_invariant_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::StateInvariantChange(Arc::new(self.temporaries.state_invariant_buffer.clone())),
            ));
        }
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.separator();

        if ui.labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlSequencePropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ));
        }

        PropertiesStatus::Shown
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        _ehc: &EventHandlingContext,
        _q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlSequenceDomain as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.bounds_rect.expand(5.0).contains(pos) => EventHandlingStatus::HandledByElement,
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<<UmlSequenceDomain as Domain>::AddCommandElementT, <UmlSequenceDomain as Domain>::PropChangeT>>,
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
                self.temporaries.highlight.selected =
                    (self.temporaries.highlight.selected && *retain) || self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::PropertyChange(uuids, property) if uuids.contains(&*self.uuid) => {
                let mut model = self.model.write();
                affected_models.insert(*model.uuid);
                match property {
                    UmlSequencePropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::NameChange(
                                model.name.clone(),
                            ),
                        ));
                        model.name = name.clone();
                    }
                    UmlSequencePropChange::SynchronicityKindChange(synchronicity) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::SynchronicityKindChange(
                                model.synchronicity.clone(),
                            ),
                        ));
                        model.synchronicity = synchronicity.clone();
                    }
                    UmlSequencePropChange::LifecycleKindChange(lifecycle) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::LifecycleKindChange(
                                model.lifecycle.clone(),
                            ),
                        ));
                        model.lifecycle = lifecycle.clone();
                    }
                    UmlSequencePropChange::IsReturnChange(is_return) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            UmlSequencePropChange::IsReturnChange(
                                model.is_return.clone(),
                            ),
                        ));
                        model.is_return = *is_return;
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
                            UmlSequencePropChange::FlipMulticonnection(FlipMulticonnection { }),
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
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        let (line_type, target_arrow_type) = match (&model.synchronicity, model.lifecycle, model.is_return) {
            (_, UmlSequenceMessageLifecycleKind::Create, _)
            | (_, _, true) => (canvas::LineType::Dashed, canvas::ArrowheadType::OpenTriangle),
            (synchronicity, _, _) => {
                (canvas::LineType::Solid,
                match synchronicity {
                    UmlSequenceMessageSynchronicityKind::Synchronous => canvas::ArrowheadType::FullTriangle,
                    UmlSequenceMessageSynchronicityKind::AsynchronousCall
                    | UmlSequenceMessageSynchronicityKind::AsynchronousSignal => canvas::ArrowheadType::OpenTriangle,
                })
            }
        };

        self.temporaries.line_type = line_type;
        self.temporaries.target_arrow_type = target_arrow_type;

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.read().uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.read().uuid());

        self.temporaries.display_text = match model.lifecycle {
            UmlSequenceMessageLifecycleKind::None => (*model.name).clone(),
            UmlSequenceMessageLifecycleKind::Create => if model.name.is_empty() {
                format!("«create»")
            } else {
                format!("«create»\n{}", model.name)
            },
            UmlSequenceMessageLifecycleKind::Delete => if model.name.is_empty() {
                format!("«destroy»")
            } else {
                format!("«destroy»\n{}", model.name)
            },
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
        deleting.contains(&self.source.uuid())
        || deleting.contains(&self.target.uuid())
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlSequenceDomain as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlSequenceDomain as Domain>::CommonElementT>,
    ) {
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
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


pub fn new_umlsequence_comment(
    text: &str,
    position: egui::Pos2,
) -> (ERef<UmlSequenceComment>, ERef<UmlSequenceCommentView>) {
    let comment_model = ERef::new(UmlSequenceComment::new(
        ModelUuid::now_v7(),
        text.to_owned(),
    ));
    let comment_view = new_umlsequence_comment_view(comment_model.clone(), position);

    (comment_model, comment_view)
}
pub fn new_umlsequence_comment_view(
    model: ERef<UmlSequenceComment>,
    position: egui::Pos2,
) -> ERef<UmlSequenceCommentView> {
    let m = model.read();
    ERef::new(UmlSequenceCommentView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

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
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
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
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> PropertiesStatus<UmlSequenceDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui.labeled_text_edit_multiline("Text:", &mut self.text_buffer).changed() {
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
                commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), egui::Vec2::new(x - self.position.x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), egui::Vec2::new(0.0, y - self.position.y)));
            }
        });

        ui.label("Background color:");
        if crate::common::controller::mglobalcolor_edit_button(
            &drawing_context.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
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

        let corner_size = 10.0;
        self.bounds_rect = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &read.text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        ).expand2(egui::Vec2 { x: corner_size, y: corner_size });

        canvas.draw_polygon(
            [
                self.bounds_rect.min,
                egui::Pos2::new(self.bounds_rect.min.x, self.bounds_rect.max.y),
                self.bounds_rect.max,
                egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y),
            ].into_iter().collect(),
            context.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_polygon(
            [
                egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y),
            ].into_iter().collect(),
            context.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
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
                        egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.min.y + corner_size),
                        egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y),
                    ].into_iter().collect(),
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
        q: &<UmlSequenceDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlSequenceTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::MouseDown(pos) => {
                if !self.min_shape().contains(pos) {
                    return EventHandlingStatus::NotHandled
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
                    if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
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
                    commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), coerced_delta));
                } else {
                    commands.push(
                        InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid).collect(),
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
        command: &InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
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
                self.highlight.selected =
                    (self.highlight.selected && *retain) || self.min_shape().contained_within(*rect);
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
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
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
                                UmlSequencePropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
                            ));
                            self.background_color = *color;
                        }
                        _ => {}
                    }
                }
            }
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

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlSequenceElement::Comment(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlSequenceComment::new(model_uuid, (*old_model.text).clone()));
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
    let link_view = new_umlsequence_commentlink_view(link_model.clone(), center_point, source.1, target.1);
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

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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

    fn midpoint_label(&self) -> Option<Arc<String>> {
        None
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
        _commands: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>
    ) -> PropertiesStatus<UmlSequenceDomain> {
        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        _view_uuid: &ViewUuid,
        _command: &InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>,
        _undo_accumulator: &mut Vec<InsensitiveCommand<UmlSequenceElementOrVertex, UmlSequencePropChange>>,
    ) {}
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert((false, *model.source.read().uuid), ArrowData::new_labelless(
            canvas::LineType::Dashed,
            canvas::ArrowheadType::None,
        ));
        self.temporaries.arrow_data.insert((true, *model.target.uuid()), ArrowData::new_labelless(
            canvas::LineType::Dashed,
            canvas::ArrowheadType::None,
        ));

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlSequenceElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlSequenceElement::CommentLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlSequenceCommentLink::new(new_uuid, old_model.source.clone(), old_model.target.clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlSequenceElement>,
    ) {
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
