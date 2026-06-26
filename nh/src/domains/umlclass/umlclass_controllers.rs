use super::umlclass_models::{
    UmlClass, UmlClassAssociable, UmlClassAssociation, UmlClassAssociationAggregation,
    UmlClassAssociationNavigability, UmlClassComment, UmlClassCommentLink, UmlClassDependency,
    UmlClassDiagram, UmlClassElement, UmlClassGeneralization, UmlClassInstance, UmlClassPackage,
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
use crate::common::ufoption::UFOption;
use crate::common::ui_ext::UiExt;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::views::multiconnection_view::{
    self, ArrowData, Ending, FlipMulticonnection, MULTICONNECTION_SOURCE_BUCKET,
    MULTICONNECTION_TARGET_BUCKET, MulticonnectionAdapter, MulticonnectionView, VertexInformation,
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::domains::umlclass::umlclass_models::{
    UmlClassOperation, UmlClassPackageKind, UmlClassProperty, UmlClassVisibilityKind,
    UmlGeneralization, UmlUseCase, UmlUseCaseGeneralization,
};
use crate::{
    CustomModal, CustomModalResult, CustomTab, DefaultSettingsF, DeserializeControllerF,
    DeserializeSettingsF, DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::egui;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub trait StereotypeController: Default + Clone + Send + Sync + 'static {
    fn show(&mut self, ui: &mut egui::Ui) -> bool;
    fn get_raw(&self) -> String;
    fn get_arc(&self) -> Arc<String> {
        self.get_raw().into()
    }
    fn is_valid(&self, _value: &str) -> bool {
        true
    }
    fn refresh(&mut self, new_value: &str);
}

pub trait UmlClassProfile: Default + Clone + Send + Sync + 'static {
    type InstanceStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type ClassStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type ClassPropertyStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type ClassOperationStereotypeController: StereotypeController =
        UnrestrictedStereotypeController;
    type UseCaseStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type DependencyStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type AssociationStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type PackageStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type CommentStereotypeController: StereotypeController = UnrestrictedStereotypeController;

    fn menubar_options_fun(
        model: &ERef<UmlClassDiagram>,
        _view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        if ui.button("PlantUML description").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(PlantUmlTab::new(model.clone()))),
            ));
        }
        ui.separator();
    }

    fn allows_class_rendering_as_stick_figure() -> bool {
        false
    }
}

#[derive(Clone, Default)]
pub struct UnrestrictedStereotypeController {
    buffer: String,
}
impl StereotypeController for UnrestrictedStereotypeController {
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        ui.labeled_text_edit_singleline("Stereotype:", &mut self.buffer)
            .changed()
    }
    fn get_raw(&self) -> String {
        self.buffer.clone()
    }
    fn refresh(&mut self, new_value: &str) {
        self.buffer.replace_range(.., new_value);
    }
}

#[derive(Clone, Default)]
pub struct UmlClassNullProfile;
impl UmlClassProfile for UmlClassNullProfile {}

pub struct UmlClassDomain<P: UmlClassProfile> {
    _profile: PhantomData<P>,
}
impl<P: UmlClassProfile> Domain for UmlClassDomain<P> {
    type SettingsT = UmlClassSettings<P>;
    type CommonElementT = UmlClassElement;
    type DiagramModelT = UmlClassDiagram;
    type CommonElementViewT = UmlClassElementView<P>;
    type ViewTargettingSectionT = UmlClassElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveUmlClassTool<P>;
    type OrdinalMovementT = UmlClassOrdinalMovement;
    type AddCommandElementT = UmlClassElementOrVertex<P>;
    type PropChangeT = UmlClassPropChange;
}

type PackageViewT<P> = PackageView<UmlClassDomain<P>, UmlClassPackageAdapter<P>>;
type GeneralizationViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassGeneralizationAdapter>;
type DependencyViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassDependencyAdapter<P>>;
type AssociationViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassAssocationAdapter<P>>;
type UseCaseGeneralizationViewT<P> =
    MulticonnectionView<UmlClassDomain<P>, UmlUseCaseGeneralizationAdapter>;
type CommentLinkViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassCommentLinkAdapter>;

#[derive(Clone, Copy, Debug)]
pub enum UmlClassOrdinalMovement {
    ClassChildUp,
    ClassChildDown,
}

impl UmlClassOrdinalMovement {
    fn inverse(&self) -> Self {
        match self {
            Self::ClassChildUp => Self::ClassChildDown,
            Self::ClassChildDown => Self::ClassChildUp,
        }
    }
}

#[derive(Clone)]
pub enum UmlClassPropChange {
    StereotypeChange(Arc<String>),

    InstanceName(Arc<String>),
    InstanceType(Arc<String>),
    InstanceSlots(Arc<String>),

    NameChange(Arc<String>),
    TemplateParametersChange(Arc<String>),
    ClassAbstractChange(bool),

    PropertyTypeChange(Arc<String>),
    PropertyMultiplicityChange(Arc<String>),
    PropertyDefaultValueChange(Arc<String>),
    OperationParametersChange(Arc<String>),
    OperationReturnTypeChange(Arc<String>),
    VisibilityChange(UFOption<UmlClassVisibilityKind>),
    IsStaticChange(bool),
    IsDerivedChange(bool),
    IsReadOnlyChange(bool),
    IsOrderedChange(bool),
    IsUniqueChange(bool),
    IsIdChange(bool),
    IsAbstractChange(bool),
    IsQueryChange(bool),

    SetNameChange(Arc<String>),
    SetCoveringChange(bool),
    SetDisjointChange(bool),

    DependencyArrowOpenChange(bool),

    LinkNavigabilityChange(/*target?*/ bool, UmlClassAssociationNavigability),
    LinkAggregationChange(/*target?*/ bool, UmlClassAssociationAggregation),
    LinkMultiplicityChange(/*target?*/ bool, Arc<String>),
    LinkRoleChange(/*target?*/ bool, Arc<String>),
    LinkReadingChange(/*target?*/ bool, Arc<String>),
    FlipMulticonnection(FlipMulticonnection),

    PackageKindChange(UmlClassPackageKind),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    CommentAlignChange(Option<egui::Align>, Option<egui::Align>),
}

impl Debug for UmlClassPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassPropChange::???")
    }
}

impl TryFrom<&UmlClassPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &UmlClassPropChange) -> Result<Self, Self::Error> {
        match value {
            UmlClassPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for UmlClassPropChange {
    fn from(value: ColorChangeData) -> Self {
        UmlClassPropChange::ColorChange(value)
    }
}
impl TryFrom<UmlClassPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: UmlClassPropChange) -> Result<Self, Self::Error> {
        match value {
            UmlClassPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for UmlClassPropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::StereotypeChange(_), newer @ Self::StereotypeChange(_))
            | (Self::InstanceName(_), newer @ Self::InstanceName(_))
            | (Self::InstanceType(_), newer @ Self::InstanceType(_))
            | (Self::InstanceSlots(_), newer @ Self::InstanceSlots(_))
            | (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::TemplateParametersChange(_), newer @ Self::TemplateParametersChange(_))
            | (Self::PropertyTypeChange(_), newer @ Self::PropertyTypeChange(_))
            | (Self::PropertyMultiplicityChange(_), newer @ Self::PropertyMultiplicityChange(_))
            | (Self::PropertyDefaultValueChange(_), newer @ Self::PropertyDefaultValueChange(_))
            | (Self::OperationParametersChange(_), newer @ Self::OperationParametersChange(_))
            | (Self::OperationReturnTypeChange(_), newer @ Self::OperationReturnTypeChange(_))
            | (Self::SetNameChange(_), newer @ Self::SetNameChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            (Self::LinkMultiplicityChange(b1, _), newer @ Self::LinkMultiplicityChange(b2, _))
            | (Self::LinkRoleChange(b1, _), newer @ Self::LinkRoleChange(b2, _))
            | (Self::LinkReadingChange(b1, _), newer @ Self::LinkReadingChange(b2, _))
                if b1 == b2 =>
            {
                Some(newer.clone())
            }
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum UmlClassElementOrVertex<Profile: UmlClassProfile> {
    Element(UmlClassElementView<Profile>),
    Vertex(VertexInformation),
}

impl<P: UmlClassProfile> Debug for UmlClassElementOrVertex<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassElementOrVertex::???")
    }
}

impl<P: UmlClassProfile> TryFrom<UmlClassElementOrVertex<P>> for VertexInformation {
    type Error = ();

    fn try_from(value: UmlClassElementOrVertex<P>) -> Result<Self, Self::Error> {
        match value {
            UmlClassElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl<P: UmlClassProfile> TryFrom<UmlClassElementOrVertex<P>> for UmlClassElementView<P> {
    type Error = ();

    fn try_from(value: UmlClassElementOrVertex<P>) -> Result<Self, Self::Error> {
        match value {
            UmlClassElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "UmlClassDomain<P>")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlClassElementView<P: UmlClassProfile> {
    Package(ERef<PackageViewT<P>>),
    Instance(ERef<UmlClassInstanceView<P>>),
    Class(ERef<UmlClassView<P>>),
    ClassProperty(ERef<UmlClassPropertyView<P>>),
    ClassOperation(ERef<UmlClassOperationView<P>>),
    UseCase(ERef<UmlUseCaseView<P>>),
    Generalization(ERef<GeneralizationViewT<P>>),
    Dependency(ERef<DependencyViewT<P>>),
    Association(ERef<AssociationViewT<P>>),
    UseCaseGeneralization(ERef<UseCaseGeneralizationViewT<P>>),
    Comment(ERef<UmlClassCommentView<P>>),
    CommentLink(ERef<CommentLinkViewT<P>>),
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
}

impl ControllerAdapter<UmlClassDomain<UmlClassNullProfile>> for UmlClassControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<
        UmlClassDomain<UmlClassNullProfile>,
        UmlClassDiagramAdapter<UmlClassNullProfile>,
    >;

    fn model(&self) -> ERef<UmlClassDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<UmlClassDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "umlclass"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::umlclass_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn insert_element(
        &mut self,
        parent: ModelUuid,
        element: UmlClassElement,
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
        undo: &mut Vec<(ModelUuid, UmlClassElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("UML Class Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared UML Class Diagram".to_owned().into(),
                UmlClassDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlClassDiagramAdapter<P: UmlClassProfile> {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: UmlClassDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    profile: PhantomData<P>,
}

#[derive(Clone, Default)]
struct UmlClassDiagramBuffer {
    name: String,
    comment: String,
}

impl<P: UmlClassProfile> UmlClassDiagramAdapter<P> {
    pub fn new(model: ERef<UmlClassDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: UmlClassDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
            profile: PhantomData,
        }
    }
}

impl<P: UmlClassProfile> DiagramAdapter<UmlClassDomain<P>> for UmlClassDiagramAdapter<P> {
    fn model(&self) -> ERef<UmlClassDiagram> {
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
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        element: UmlClassElement,
    ) -> Result<UmlClassElementView<P>, HashSet<ModelUuid>> {
        let v = match element {
            UmlClassElement::Package(inner) => {
                UmlClassElementView::from(new_umlclass_package_view(
                    inner,
                    egui::Rect {
                        min: egui::Pos2::ZERO,
                        max: egui::Pos2::new(100.0, 100.0),
                    },
                ))
            }
            UmlClassElement::Instance(inner) => UmlClassElementView::from(
                new_umlclass_instance_view(inner, egui::Pos2::ZERO, MGlobalColor::None),
            ),
            UmlClassElement::Class(inner) => {
                let (properties_views, operations_views) = {
                    let r = inner.read();
                    (
                        r.properties
                            .iter()
                            .map(|e| new_umlclass_property_view(e.clone()))
                            .collect(),
                        r.operations
                            .iter()
                            .map(|e| new_umlclass_operation_view(e.clone()))
                            .collect(),
                    )
                };

                UmlClassElementView::from(new_umlclass_class_view(
                    inner,
                    properties_views,
                    operations_views,
                    egui::Pos2::ZERO,
                    UmlClassRenderStyle::Class,
                    MGlobalColor::None,
                ))
            }
            UmlClassElement::Property(..) | UmlClassElement::Operation(..) => {
                unreachable!()
            }
            UmlClassElement::UseCase(inner) => UmlClassElementView::from(new_uml_usecase_view(
                inner,
                egui::Pos2::ZERO,
                MGlobalColor::None,
            )),
            UmlClassElement::Generalization(inner) => {
                let m = inner.read();
                let (Some(sv), Some(tv)) = (
                    m.sources
                        .iter()
                        .map(|e| q.get_view_for(&e.read().uuid))
                        .collect(),
                    m.targets
                        .iter()
                        .map(|e| q.get_view_for(&e.read().uuid))
                        .collect(),
                ) else {
                    return Err(m
                        .sources
                        .iter()
                        .map(|e| *e.read().uuid)
                        .chain(m.targets.iter().map(|e| *e.read().uuid))
                        .collect());
                };
                UmlClassElementView::from(new_umlclass_generalization_view(
                    inner.clone(),
                    None,
                    sv,
                    tv,
                ))
            }
            UmlClassElement::Dependency(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(new_umlclass_dependency_view(
                    inner.clone(),
                    None,
                    source_view,
                    target_view,
                ))
            }
            UmlClassElement::Association(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(new_umlclass_association_view(
                    inner.clone(),
                    None,
                    source_view,
                    target_view,
                ))
            }
            UmlClassElement::UseCaseGeneralization(inner) => {
                let m = inner.read();
                let (Some(sv), Some(tv)) = (
                    m.sources
                        .iter()
                        .map(|e| q.get_view_for(&e.read().uuid))
                        .collect(),
                    m.targets
                        .iter()
                        .map(|e| q.get_view_for(&e.read().uuid))
                        .collect(),
                ) else {
                    return Err(m
                        .sources
                        .iter()
                        .map(|e| *e.read().uuid)
                        .chain(m.targets.iter().map(|e| *e.read().uuid))
                        .collect());
                };
                UmlClassElementView::from(new_uml_usecasegeneralization_view(
                    inner.clone(),
                    None,
                    sv,
                    tv,
                ))
            }
            UmlClassElement::Comment(inner) => UmlClassElementView::from(
                new_umlclass_comment_view(inner, egui::Pos2::ZERO, egui::Align2::CENTER_CENTER),
            ),
            UmlClassElement::CommentLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(new_umlclass_commentlink_view(
                    inner.clone(),
                    None,
                    source_view,
                    target_view,
                ))
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &UmlClassElement) -> Arc<String> {
        match e {
            UmlClassElement::Package(inner) => inner.read().name.clone(),
            UmlClassElement::Instance(inner) => {
                let r = inner.read();
                let s = if r.instance_name.is_empty() {
                    format!(":{}", r.instance_type)
                } else {
                    format!("{}: {}", r.instance_name, r.instance_type)
                };
                Arc::new(s)
            }
            UmlClassElement::Class(inner) => {
                let r = inner.read();
                if r.stereotype.is_empty() {
                    r.name.clone()
                } else {
                    Arc::new(format!("{} «{}»", r.name, r.stereotype))
                }
            }
            UmlClassElement::Property(inner) => inner.read().name.clone(),
            UmlClassElement::Operation(inner) => inner.read().name.clone(),
            UmlClassElement::UseCase(inner) => {
                let r = inner.read();
                if r.stereotype.is_empty() {
                    r.name.clone()
                } else {
                    Arc::new(format!("{} «{}»", r.name, r.stereotype))
                }
            }
            UmlClassElement::Generalization(inner) => {
                let r = inner.read();
                let s = if r.set_name.is_empty() {
                    "Generalization".to_owned()
                } else {
                    format!("Generalization ({})", r.set_name)
                };
                Arc::new(s)
            }
            UmlClassElement::Dependency(inner) => {
                let r = inner.read();
                let s = if r.stereotype.is_empty() {
                    "Dependency".to_owned()
                } else {
                    format!("Dependency ({})", r.stereotype)
                };
                Arc::new(s)
            }
            UmlClassElement::Association(inner) => {
                let r = inner.read();
                let s = if r.stereotype.is_empty() {
                    "Association".to_owned()
                } else {
                    format!("Association «{}»", r.stereotype)
                };
                Arc::new(s)
            }
            UmlClassElement::UseCaseGeneralization(inner) => {
                let r = inner.read();
                let s = if r.set_name.is_empty() {
                    "Generalization".to_owned()
                } else {
                    format!("Generalization ({})", r.set_name)
                };
                Arc::new(s)
            }
            UmlClassElement::Comment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
            UmlClassElement::CommentLink(_inner) => Arc::new(format!("Comment Link")),
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
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                UmlClassPropChange::ColorChange((0, new_color).into()),
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
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlClassPropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                UmlClassPropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlClassPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::CommentChange(model.comment.clone()),
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
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        P::menubar_options_fun(&self.model, view_uuid, ui, commands);
    }
    fn try_handle_custom_shortcut(
        &mut self,
        settings: &UmlClassSettings<P>,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveUmlClassTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialUmlClassElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let (new_model, models) = super::umlclass_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let models = super::umlclass_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
}

pub struct PlantUmlTab {
    diagram: ERef<UmlClassDiagram>,
    plantuml_description: String,
}

impl PlantUmlTab {
    pub fn new(diagram: ERef<UmlClassDiagram>) -> Self {
        Self {
            diagram,
            plantuml_description: String::new(),
        }
    }
}

impl CustomTab for PlantUmlTab {
    fn title(&self) -> String {
        "PlantUML description".to_owned()
    }

    fn show(
        &mut self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) {
        if ui.button("Refresh").clicked() {
            self.plantuml_description = self.diagram.read().plantuml();
        }

        ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.plantuml_description.as_str()),
        );
    }
}

fn new_controlller(
    model: ERef<UmlClassDiagram>,
    name: String,
    elements: Vec<UmlClassElementView<UmlClassNullProfile>>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            UmlClassControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                UmlClassDiagramAdapter::<UmlClassNullProfile>::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New UML class diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    // https://www.uml-diagrams.org/class-diagrams-overview.html
    // https://www.uml-diagrams.org/design-pattern-abstract-factory-uml-class-diagram-example.html

    fn af_operations(
        is_abstract: bool,
    ) -> Vec<(
        ERef<UmlClassOperation>,
        ERef<UmlClassOperationView<UmlClassNullProfile>>,
    )> {
        [
            ("createProductA", "ProductA"),
            ("createProductB", "ProductB"),
        ]
        .iter()
        .map(|e| {
            let e = new_umlclass_operation(
                UFOption::Some(UmlClassVisibilityKind::Public),
                e.0,
                "",
                e.1,
                "",
            );
            if is_abstract {
                e.0.write().is_abstract = true;
                e.1.write().refresh_buffers();
            }
            e
        })
        .collect()
    }

    let (class_af, class_af_view) = new_umlclass_class(
        "AbstractFactory",
        "interface",
        false,
        Vec::new(),
        af_operations(true),
        egui::Pos2::new(200.0, 150.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );

    let (class_cfx, class_cfx_view) = new_umlclass_class(
        "ConcreteFactoryX",
        "class",
        false,
        Vec::new(),
        af_operations(false),
        egui::Pos2::new(100.0, 250.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );

    let (class_cfy, class_cfy_view) = new_umlclass_class(
        "ConcreteFactoryY",
        "class",
        false,
        Vec::new(),
        af_operations(false),
        egui::Pos2::new(300.0, 250.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );

    let (realization_cfx, realization_cfx_view) = new_umlclass_dependency(
        "",
        "",
        false,
        None,
        (class_cfx.clone().into(), class_cfx_view.clone().into()),
        (class_af.clone().into(), class_af_view.clone().into()),
    );

    let (realization_cfy, realization_cfy_view) = new_umlclass_dependency(
        "",
        "",
        false,
        None,
        (class_cfy.clone().into(), class_cfy_view.clone().into()),
        (class_af.clone().into(), class_af_view.clone().into()),
    );

    let (class_client, class_client_view) = new_umlclass_class(
        "Client",
        "class",
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(300.0, 50.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );

    let (usage_client_af, usage_client_af_view) = new_umlclass_dependency(
        "use",
        "",
        true,
        Some((ViewUuid::now_v7(), egui::Pos2::new(200.0, 50.0))),
        (
            class_client.clone().into(),
            class_client_view.clone().into(),
        ),
        (class_af.clone().into(), class_af_view.clone().into()),
    );

    let (class_producta, class_producta_view) = new_umlclass_class(
        "ProductA",
        "interface",
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(450.0, 150.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );

    let (usage_client_producta, usage_client_producta_view) = new_umlclass_dependency(
        "use",
        "",
        true,
        Some((ViewUuid::now_v7(), egui::Pos2::new(450.0, 52.0))),
        (
            class_client.clone().into(),
            class_client_view.clone().into(),
        ),
        (
            class_producta.clone().into(),
            class_producta_view.clone().into(),
        ),
    );

    let (class_productb, class_productb_view) = new_umlclass_class(
        "ProductB",
        "interface",
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(650.0, 150.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );

    let (usage_client_productb, usage_client_productb_view) = new_umlclass_dependency(
        "use",
        "",
        true,
        Some((ViewUuid::now_v7(), egui::Pos2::new(650.0, 48.0))),
        (
            class_client.clone().into(),
            class_client_view.clone().into(),
        ),
        (
            class_productb.clone().into(),
            class_productb_view.clone().into(),
        ),
    );

    let (comment, comment_view) = new_umlclass_comment(
        "This is a comment\nwith multiple lines",
        "",
        egui::Pos2::new(650.0, 250.0),
        egui::Align2::CENTER_CENTER,
    );
    let (commentlink1, commentlink1_view) = new_umlclass_commentlink(
        None,
        (comment.clone(), comment_view.clone().into()),
        (
            class_producta.clone().into(),
            class_producta_view.clone().into(),
        ),
    );
    let (commentlink2, commentlink2_view) = new_umlclass_commentlink(
        None,
        (comment.clone(), comment_view.clone().into()),
        (
            class_productb.clone().into(),
            class_productb_view.clone().into(),
        ),
    );

    let shape_operations = {
        let d = new_umlclass_operation(
            UFOption::Some(UmlClassVisibilityKind::Public),
            "draw",
            "",
            "",
            "",
        );
        let m = new_umlclass_operation(
            UFOption::Some(UmlClassVisibilityKind::Public),
            "move",
            "",
            "",
            "",
        );
        vec![d, m]
    };
    let (shape_model, shape_view) = new_umlclass_class(
        "Shape",
        "entity",
        true,
        Vec::new(),
        shape_operations,
        egui::Pos2::new(200.0, 400.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );
    let (polygon_model, polygon_view) = new_umlclass_class(
        "Polygon",
        "entity",
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(100.0, 550.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );
    let circle_properties = {
        let r = new_umlclass_property(
            UFOption::Some(UmlClassVisibilityKind::Private),
            "radius",
            "float",
            "",
            "",
            "",
        );
        let c = new_umlclass_property(
            UFOption::Some(UmlClassVisibilityKind::Private),
            "center",
            "Point",
            "",
            "",
            "",
        );
        vec![r, c]
    };
    let (circle_model, circle_view) = new_umlclass_class(
        "Circle",
        "entity",
        false,
        circle_properties,
        Vec::new(),
        egui::Pos2::new(300.0, 550.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );
    let (gen_model, gen_view) = new_umlclass_generalization(
        "",
        Some((ViewUuid::now_v7(), egui::Pos2::new(200.0, 490.0))),
        (polygon_model.clone(), polygon_view.clone().into()),
        (shape_model.clone(), shape_view.clone().into()),
    );
    gen_model.write().set_is_covering = true;
    gen_model.write().set_is_disjoint = true;
    let gen_uuid = *gen_view.read().uuid();
    gen_view.write().apply_command(
        &InsensitiveCommand::AddDependency {
            target: gen_uuid,
            bucket: MULTICONNECTION_SOURCE_BUCKET,
            position: None,
            element: UmlClassElementOrVertex::Element(circle_view.clone().into()),
            into_model: true,
        },
        &mut Vec::new(),
        &mut HashSet::new(),
    );
    let point_properties = {
        let x = new_umlclass_property(
            UFOption::Some(UmlClassVisibilityKind::Private),
            "x",
            "float",
            "",
            "",
            "",
        );
        let y = new_umlclass_property(
            UFOption::Some(UmlClassVisibilityKind::Private),
            "y",
            "float",
            "",
            "",
            "",
        );
        vec![x, y]
    };
    let (point_model, point_view) = new_umlclass_class(
        "Point",
        "struct",
        false,
        point_properties,
        Vec::new(),
        egui::Pos2::new(100.0, 700.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );
    let (point_assoc_model, point_assoc_view) = new_umlclass_association(
        "",
        "",
        "",
        "",
        None,
        (polygon_model.clone().into(), polygon_view.clone().into()),
        (point_model.clone().into(), point_view.clone().into()),
    );
    point_assoc_model.write().source_label_multiplicity = Arc::new("0..*".to_owned());
    point_assoc_model.write().target_label_multiplicity = Arc::new("3..*".to_owned());
    point_assoc_model.write().target_navigability = UmlClassAssociationNavigability::Navigable;

    let (comment2, comment2_view) = new_umlclass_comment(
        "{radius >= 0}",
        "",
        egui::Pos2::new(300.0, 650.0),
        egui::Align2::CENTER_CENTER,
    );
    let (commentlink3, commentlink3_view) = new_umlclass_commentlink(
        None,
        (comment2.clone(), comment2_view.clone().into()),
        (circle_model.clone().into(), circle_view.clone().into()),
    );

    let (instance, instance_view) = new_umlclass_instance(
        "d",
        "Human",
        "person",
        "firstName = \"Vojtěch\"\nlastName = \"Doležal\"",
        egui::Pos2::new(650.0, 400.0),
        MGlobalColor::None,
    );

    let name = format!("Demo UML class diagram {}", no);
    let diagram2 = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            class_af.into(),
            class_cfx.into(),
            class_cfy.into(),
            realization_cfx.into(),
            realization_cfy.into(),
            class_client.into(),
            usage_client_af.into(),
            class_producta.into(),
            usage_client_producta.into(),
            class_productb.into(),
            usage_client_productb.into(),
            comment.into(),
            commentlink1.into(),
            commentlink2.into(),
            shape_model.into(),
            polygon_model.into(),
            circle_model.into(),
            gen_model.into(),
            point_model.into(),
            point_assoc_model.into(),
            comment2.into(),
            commentlink3.into(),
            instance.into(),
        ],
    ));
    new_controlller(
        diagram2,
        name,
        vec![
            class_af_view.into(),
            class_cfx_view.into(),
            class_cfy_view.into(),
            realization_cfx_view.into(),
            realization_cfy_view.into(),
            class_client_view.into(),
            usage_client_af_view.into(),
            class_producta_view.into(),
            usage_client_producta_view.into(),
            class_productb_view.into(),
            usage_client_productb_view.into(),
            comment_view.into(),
            commentlink1_view.into(),
            commentlink2_view.into(),
            shape_view.into(),
            polygon_view.into(),
            circle_view.into(),
            gen_view.into(),
            point_view.into(),
            point_assoc_view.into(),
            comment2_view.into(),
            commentlink3_view.into(),
            instance_view.into(),
        ],
    )
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        UmlClassDomain<UmlClassNullProfile>,
        UmlClassControllerAdapter,
        DiagramControllerGen2<
            UmlClassDomain<UmlClassNullProfile>,
            UmlClassDiagramAdapter<UmlClassNullProfile>,
        >,
    >>(&uuid)?)
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommentIndication {
    None,
    Icon,
    TextCompartment,
}

impl CommentIndication {
    const VARIANTS: [Self; 3] = [Self::None, Self::Icon, Self::TextCompartment];

    fn as_str(&self) -> &'static str {
        match self {
            CommentIndication::None => "None",
            CommentIndication::Icon => "Icon",
            CommentIndication::TextCompartment => "Text Compartment",
        }
    }
}

pub struct UmlClassSettings<P: UmlClassProfile> {
    palette: RwLock<ToolPalette<UmlClassToolStage, UmlClassDomain<P>>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<UmlClassToolStage, UmlClassElementView<P>>>,
    comment_indication: CommentIndication,
    instance_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            ERef<UmlClassInstance>,
        ) -> (
            UmlClassToolStage,
            UmlClassToolStage,
            PartialUmlClassElement<P>,
            bool,
        ),
    )>,
    class_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            ERef<UmlClass>,
        ) -> (
            UmlClassToolStage,
            UmlClassToolStage,
            PartialUmlClassElement<P>,
            bool,
        ),
    )>,
}

impl<P: UmlClassProfile> DiagramSettings for UmlClassSettings<P> {
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

                    // TODO: make the stereotypes more efficient
                    match tool {
                        UmlClassToolStage::Instance {
                            instance_name,
                            instance_type,
                            stereotype,
                            background_color,
                        } => {
                            let mut sc = P::InstanceStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Instance name", instance_name)
                                .changed();
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Instance type", instance_type)
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
                        UmlClassToolStage::Class {
                            name,
                            stereotype,
                            is_abstract,
                            render_style: _,
                            background_color,
                        } => {
                            let mut sc = P::ClassStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Name", name)
                                .changed();
                            modified |= columns[1].checkbox(is_abstract, "isAbstract").changed();

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
                        UmlClassToolStage::ClassProperty {
                            name,
                            property_type,
                            stereotype,
                        } => {
                            let mut sc = P::ClassPropertyStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Name", name)
                                .changed();
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Property type", property_type)
                                .changed();
                        }
                        UmlClassToolStage::ClassOperation {
                            name,
                            return_type,
                            stereotype,
                        } => {
                            let mut sc = P::ClassOperationStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Name", name)
                                .changed();
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Return type", return_type)
                                .changed();
                        }
                        UmlClassToolStage::UseCase {
                            name,
                            stereotype,
                            is_abstract,
                            background_color,
                        } => {
                            let mut sc = P::UseCaseStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Name", name)
                                .changed();
                            modified |= columns[1].checkbox(is_abstract, "isAbstract").changed();

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
                        UmlClassToolStage::LinkStart { link_type } => match link_type {
                            LinkType::Generalization { set_name } => {
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Set name", set_name)
                                    .changed();
                            }
                            LinkType::Dependency {
                                target_arrow_open,
                                stereotype,
                                name,
                            } => {
                                let mut sc = P::DependencyStereotypeController::default();
                                sc.refresh(&stereotype);
                                if sc.show(&mut columns[1]) {
                                    modified = true;
                                    *stereotype = sc.get_raw();
                                }
                                modified |= columns[1]
                                    .labeled_text_edit_singleline("Name", name)
                                    .changed();
                                modified |= columns[1]
                                    .checkbox(target_arrow_open, "target arrow open")
                                    .changed();
                            }
                            LinkType::Association {
                                stereotype,
                                source_multiplicity,
                                target_multiplicity,
                            } => {
                                let mut sc = P::AssociationStereotypeController::default();
                                sc.refresh(&stereotype);
                                if sc.show(&mut columns[1]) {
                                    modified = true;
                                    *stereotype = sc.get_raw();
                                }
                                modified |= columns[1]
                                    .labeled_text_edit_singleline(
                                        "Source multiplicity",
                                        source_multiplicity,
                                    )
                                    .changed();
                                modified |= columns[1]
                                    .labeled_text_edit_singleline(
                                        "Target multiplicity",
                                        target_multiplicity,
                                    )
                                    .changed();
                            }
                        },
                        UmlClassToolStage::PackageStart {
                            name,
                            stereotype,
                            kind,
                        } => {
                            let mut sc = P::PackageStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Name", name)
                                .changed();

                            columns[1].label("Package kind");
                            egui::ComboBox::from_id_salt("package kind")
                                .selected_text(kind.as_str())
                                .show_ui(&mut columns[1], |ui| {
                                    for e in UmlClassPackageKind::VARIANTS {
                                        modified |=
                                            ui.selectable_value(kind, e, e.as_str()).clicked();
                                    }
                                });
                        }
                        UmlClassToolStage::Comment {
                            stereotype,
                            text,
                            align,
                        } => {
                            let mut sc = P::CommentStereotypeController::default();
                            sc.refresh(&stereotype);
                            if sc.show(&mut columns[1]) {
                                modified = true;
                                *stereotype = sc.get_raw();
                            }
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
                        UmlClassToolStage::CommentLinkStart => {}
                        UmlClassToolStage::LinkEnd
                        | UmlClassToolStage::LinkAddEnding { .. }
                        | UmlClassToolStage::PackageEnd
                        | UmlClassToolStage::CommentLinkEnd => unreachable!(),
                    }

                    if modified {
                        *view = view_for_stage(tool);
                        w.set_from_buffer(buffer.clone());
                    }
                }
            }
        });

        ui.label("Comment indication");
        egui::ComboBox::from_id_salt("comment indication")
            .selected_text(self.comment_indication.as_str())
            .show_ui(ui, |ui| {
                for e in CommentIndication::VARIANTS {
                    ui.selectable_value(&mut self.comment_indication, e, e.as_str());
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
            self.palette.read().unwrap().serialize()?.into(),
        );
        table.insert(
            "comment_indication".to_owned(),
            toml::Value::try_from(self.comment_indication).map_err(|_| ())?,
        );
        Ok(table.into())
    }
}
impl<P: UmlClassProfile> DiagramSettings2<UmlClassDomain<P>> for UmlClassSettings<P> {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    UmlClassToolStage,
                    String,
                    UmlClassElementView<P>,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        ),
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

pub fn default_settings_helper<P: UmlClassProfile>(
    palette_items: Vec<(
        &'static str,
        Vec<(
            UmlClassToolStage,
            &'static str,
            Option<egui::KeyboardShortcut>,
        )>,
    )>,
    instance_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            ERef<UmlClassInstance>,
        ) -> (
            UmlClassToolStage,
            UmlClassToolStage,
            PartialUmlClassElement<P>,
            bool,
        ),
    )>,
    class_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            ERef<UmlClass>,
        ) -> (
            UmlClassToolStage,
            UmlClassToolStage,
            PartialUmlClassElement<P>,
            bool,
        ),
    )>,
) -> Box<UmlClassSettings<P>> {
    let palette_items = palette_items
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

    Box::new(UmlClassSettings {
        comment_indication: CommentIndication::Icon,
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
        instance_buttons,
        class_buttons,
    })
}

fn view_for_stage<P: UmlClassProfile>(s: &UmlClassToolStage) -> UmlClassElementView<P> {
    match s {
        UmlClassToolStage::Instance {
            instance_name,
            instance_type,
            stereotype,
            background_color: color,
        } => {
            let instance_view = new_umlclass_instance(
                instance_name,
                instance_type,
                stereotype,
                "",
                egui::Pos2::ZERO,
                *color,
            )
            .1;
            instance_view.write().refresh_buffers();
            instance_view.into()
        }
        UmlClassToolStage::Class {
            name,
            stereotype,
            is_abstract,
            render_style,
            background_color: color,
        } => {
            let class_view = new_umlclass_class(
                name,
                stereotype,
                *is_abstract,
                Vec::new(),
                Vec::new(),
                egui::Pos2::ZERO,
                *render_style,
                *color,
            )
            .1;
            class_view.write().refresh_buffers();
            class_view.into()
        }
        UmlClassToolStage::ClassProperty {
            name,
            property_type,
            stereotype,
        } => {
            let property_view =
                new_umlclass_property(UFOption::None, name, property_type, "", "", stereotype).1;
            property_view.write().refresh_buffers();
            property_view.into()
        }
        UmlClassToolStage::ClassOperation {
            name,
            return_type,
            stereotype,
        } => {
            let operation_view =
                new_umlclass_operation(UFOption::None, name, "", return_type, stereotype).1;
            operation_view.write().refresh_buffers();
            operation_view.into()
        }
        UmlClassToolStage::UseCase {
            name,
            stereotype,
            is_abstract,
            background_color: color,
        } => {
            let uc_view =
                new_uml_usecase(name, stereotype, *is_abstract, egui::Pos2::ZERO, *color).1;
            uc_view.write().refresh_buffers();
            uc_view.into()
        }
        UmlClassToolStage::LinkStart { link_type } => {
            let d1 = new_umlclass_class(
                "dummy",
                "",
                false,
                Vec::new(),
                Vec::new(),
                egui::Pos2::ZERO,
                UmlClassRenderStyle::Class,
                MGlobalColor::None,
            );
            let d2 = new_umlclass_class(
                "dummy",
                "",
                false,
                Vec::new(),
                Vec::new(),
                egui::Pos2::new(100.0, 50.0),
                UmlClassRenderStyle::Class,
                MGlobalColor::None,
            );

            match link_type {
                LinkType::Generalization { set_name } => {
                    let g = new_umlclass_generalization(
                        set_name,
                        None,
                        (d1.0, d1.1.into()),
                        (d2.0, d2.1.into()),
                    )
                    .1;
                    g.into()
                }
                LinkType::Dependency {
                    target_arrow_open,
                    stereotype,
                    name,
                } => {
                    let d = new_umlclass_dependency(
                        stereotype,
                        name,
                        *target_arrow_open,
                        None,
                        (d1.0.into(), d1.1.into()),
                        (d2.0.into(), d2.1.into()),
                    )
                    .1;
                    d.into()
                }
                LinkType::Association {
                    stereotype,
                    source_multiplicity,
                    target_multiplicity,
                } => {
                    let a = new_umlclass_association(
                        stereotype,
                        "",
                        source_multiplicity,
                        target_multiplicity,
                        None,
                        (d1.0.into(), d1.1.into()),
                        (d2.0.into(), d2.1.into()),
                    )
                    .1;
                    a.into()
                }
            }
        }
        UmlClassToolStage::PackageStart {
            name,
            stereotype,
            kind,
        } => {
            let package_view = new_umlclass_package(
                name,
                stereotype,
                *kind,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 50.0),
                },
            )
            .1;
            package_view.write().refresh_buffers();
            package_view.into()
        }
        UmlClassToolStage::Comment {
            stereotype,
            text,
            align,
        } => {
            let comment_view = new_umlclass_comment(text, stereotype, egui::Pos2::ZERO, *align).1;
            comment_view.write().refresh_buffers();
            comment_view.into()
        }
        UmlClassToolStage::CommentLinkStart => {
            let d1 = new_umlclass_comment("", "", egui::Pos2::ZERO, egui::Align2::CENTER_CENTER);
            let d2 = new_umlclass_class(
                "dummy",
                "",
                false,
                Vec::new(),
                Vec::new(),
                egui::Pos2::new(100.0, 50.0),
                UmlClassRenderStyle::Class,
                MGlobalColor::None,
            );
            let commentlink =
                new_umlclass_commentlink(None, (d1.0, d1.1.into()), (d2.0.into(), d2.1.into())).1;
            commentlink.into()
        }
        UmlClassToolStage::LinkEnd
        | UmlClassToolStage::LinkAddEnding { .. }
        | UmlClassToolStage::PackageEnd
        | UmlClassToolStage::CommentLinkEnd => unreachable!(),
    }
}

mod buttons {
    use super::*;
    use std::sync::LazyLock;
    fn instance_association(
        m: ERef<UmlClassInstance>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    ) {
        let link_type = LinkType::Association {
            stereotype: "".to_owned(),
            source_multiplicity: "0..*".to_owned(),
            target_multiplicity: "1..1".to_owned(),
        };
        (
            UmlClassToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlClassToolStage::LinkEnd,
            PartialUmlClassElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    type InstanceButtonF = dyn Fn(
        ERef<UmlClassInstance>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    );
    pub const INSTANCE_BUTTONS: LazyLock<
        Vec<(usize, usize, &'static str, &'static InstanceButtonF)>,
    > = LazyLock::new(|| vec![(0, 0, "\\", &instance_association as &InstanceButtonF)]);
    fn class_association(
        m: ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    ) {
        let link_type = LinkType::Association {
            stereotype: "".to_owned(),
            source_multiplicity: "0..*".to_owned(),
            target_multiplicity: "1..1".to_owned(),
        };
        (
            UmlClassToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlClassToolStage::LinkEnd,
            PartialUmlClassElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    fn class_generalization(
        m: ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    ) {
        let link_type = LinkType::Generalization {
            set_name: "".to_owned(),
        };
        (
            UmlClassToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlClassToolStage::LinkEnd,
            PartialUmlClassElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    fn class_property(
        _m: ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    ) {
        let stage = UmlClassToolStage::ClassProperty {
            name: "property".to_owned(),
            property_type: "PropertyType".to_owned(),
            stereotype: "".to_owned(),
        };
        (stage.clone(), stage, PartialUmlClassElement::None, false)
    }
    fn class_operation(
        _m: ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    ) {
        let stage = UmlClassToolStage::ClassOperation {
            name: "operation".to_owned(),
            return_type: "ReturnType".to_owned(),
            stereotype: "".to_owned(),
        };
        (stage.clone(), stage, PartialUmlClassElement::None, false)
    }
    type ClassButtonF = dyn Fn(
        ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UmlClassNullProfile>,
        bool,
    );
    pub const CLASS_BUTTONS: LazyLock<Vec<(usize, usize, &'static str, &'static ClassButtonF)>> =
        LazyLock::new(|| {
            vec![
                (0, 0, "\\", &class_association as &ClassButtonF),
                (0, 1, "↘", &class_generalization as &ClassButtonF),
                (1, 0, "P", &class_property as &ClassButtonF),
                (1, 1, "O", &class_operation as &ClassButtonF),
            ]
        });
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let palette_items = vec![
        (
            "Elements",
            vec![
                (
                    UmlClassToolStage::Instance {
                        instance_name: "o".to_owned(),
                        instance_type: "Type".to_owned(),
                        stereotype: "".to_owned(),
                        background_color: MGlobalColor::None,
                    },
                    "Instance",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num1,
                    )),
                ),
                (
                    UmlClassToolStage::Class {
                        name: "ClassName".to_owned(),
                        stereotype: "class".to_owned(),
                        is_abstract: false,
                        render_style: UmlClassRenderStyle::Class,
                        background_color: MGlobalColor::None,
                    },
                    "Class",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num2,
                    )),
                ),
                (
                    UmlClassToolStage::ClassProperty {
                        name: "property".to_owned(),
                        property_type: "PropertyType".to_owned(),
                        stereotype: "".to_owned(),
                    },
                    "Property",
                    None,
                ),
                (
                    UmlClassToolStage::ClassOperation {
                        name: "operation".to_owned(),
                        return_type: "ReturnType".to_owned(),
                        stereotype: "".to_owned(),
                    },
                    "Operation",
                    None,
                ),
            ],
        ),
        (
            "Relationships",
            vec![
                (
                    UmlClassToolStage::LinkStart {
                        link_type: LinkType::Generalization {
                            set_name: "".to_owned(),
                        },
                    },
                    "Generalization (Set)",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num5,
                    )),
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_type: LinkType::Association {
                            stereotype: "".to_owned(),
                            source_multiplicity: "0..*".to_owned(),
                            target_multiplicity: "1..1".to_owned(),
                        },
                    },
                    "Association",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num6,
                    )),
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_type: LinkType::Dependency {
                            target_arrow_open: false,
                            stereotype: "".to_owned(),
                            name: "".to_owned(),
                        },
                    },
                    "Interface Realization",
                    None,
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_type: LinkType::Dependency {
                            target_arrow_open: true,
                            stereotype: "use".to_owned(),
                            name: "".to_owned(),
                        },
                    },
                    "Usage",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num7,
                    )),
                ),
            ],
        ),
        (
            "Other",
            vec![
                (
                    UmlClassToolStage::PackageStart {
                        name: "a package".to_owned(),
                        stereotype: "".to_owned(),
                        kind: UmlClassPackageKind::Package,
                    },
                    "Package",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num8,
                    )),
                ),
                (
                    UmlClassToolStage::Comment {
                        stereotype: "".to_owned(),
                        text: "a comment".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                    },
                    "Comment",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num9,
                    )),
                ),
                (UmlClassToolStage::CommentLinkStart, "Comment Link", None),
            ],
        ),
    ];

    default_settings_helper::<UmlClassNullProfile>(
        palette_items,
        buttons::INSTANCE_BUTTONS.clone(),
        buttons::CLASS_BUTTONS.clone(),
    )
}

pub fn settings_deserializer_helper<P: UmlClassProfile>(
    value: toml::Value,
    instance_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            ERef<UmlClassInstance>,
        ) -> (
            UmlClassToolStage,
            UmlClassToolStage,
            PartialUmlClassElement<P>,
            bool,
        ),
    )>,
    class_buttons: Vec<(
        usize,
        usize,
        &'static str,
        &'static dyn Fn(
            ERef<UmlClass>,
        ) -> (
            UmlClassToolStage,
            UmlClassToolStage,
            PartialUmlClassElement<P>,
            bool,
        ),
    )>,
) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(UmlClassSettings::<P> {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
        comment_indication: value
            .get("comment_indication")
            .unwrap()
            .clone()
            .try_into()
            .unwrap(),
        instance_buttons,
        class_buttons,
    }))
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    settings_deserializer_helper(
        value,
        buttons::INSTANCE_BUTTONS.clone(),
        buttons::CLASS_BUTTONS.clone(),
    )
}

inventory::submit! {DiagramInfo {
    type_indentifier: "umlclass",
    pretty_name: "UML Class diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "/Unified Modeling Language",
        description: "UML Class diagram (classes, objects, packages, etc.)",
        constructors: &[
            ("empty", &(new as DiagramConstructorF)),
            ("demo", &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkType {
    Generalization {
        set_name: String,
    },
    Dependency {
        target_arrow_open: bool,
        stereotype: String,
        name: String,
    },
    Association {
        stereotype: String,
        source_multiplicity: String,
        target_multiplicity: String,
    },
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlClassToolStage {
    Instance {
        instance_name: String,
        instance_type: String,
        stereotype: String,
        background_color: MGlobalColor,
    },
    Class {
        name: String,
        stereotype: String,
        is_abstract: bool,
        render_style: UmlClassRenderStyle,
        background_color: MGlobalColor,
    },
    ClassProperty {
        name: String,
        property_type: String,
        stereotype: String,
    },
    ClassOperation {
        name: String,
        return_type: String,
        stereotype: String,
    },
    UseCase {
        name: String,
        stereotype: String,
        is_abstract: bool,
        background_color: MGlobalColor,
    },
    LinkStart {
        link_type: LinkType,
    },
    LinkEnd,
    LinkAddEnding {
        source: bool,
    },
    PackageStart {
        name: String,
        stereotype: String,
        kind: UmlClassPackageKind,
    },
    PackageEnd,
    Comment {
        stereotype: String,
        text: String,
        align: egui::Align2,
    },
    CommentLinkStart,
    CommentLinkEnd,
}

pub enum PartialUmlClassElement<P: UmlClassProfile> {
    None,
    Some(UmlClassElementView<P>),
    Link {
        link_type: LinkType,
        source: UmlClassAssociable,
        dest: Option<UmlClassAssociable>,
    },
    LinkEnding {
        source: bool,
        gen_model: UmlGeneralization,
        new_model: Option<ModelUuid>,
    },
    Package {
        // TODO: are these necessary?
        name: String,
        stereotype: String,
        kind: UmlClassPackageKind,
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    CommentLink {
        source: ERef<UmlClassComment>,
        dest: Option<UmlClassElement>,
    },
}

pub struct NaiveUmlClassTool<P: UmlClassProfile> {
    uuid: uuid::Uuid,
    initial_stage: UmlClassToolStage,
    current_stage: UmlClassToolStage,
    result: PartialUmlClassElement<P>,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl<P: UmlClassProfile> NaiveUmlClassTool<P> {
    fn try_spend(&mut self) {
        self.result = PartialUmlClassElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl<P: UmlClassProfile> Tool<UmlClassDomain<P>> for NaiveUmlClassTool<P> {
    type Stage = UmlClassToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: UmlClassToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialUmlClassElement::None,
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

    fn targetting_for_section(&self, element: Option<UmlClassElement>) -> egui::Color32 {
        match element {
            None | Some(UmlClassElement::Package(..)) => match self.current_stage {
                UmlClassToolStage::Instance { .. }
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::UseCase { .. }
                | UmlClassToolStage::PackageStart { .. }
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment { .. }
                | UmlClassToolStage::CommentLinkEnd => TARGETTABLE_COLOR,

                UmlClassToolStage::ClassProperty { .. }
                | UmlClassToolStage::ClassOperation { .. }
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkStart => NON_TARGETTABLE_COLOR,
            },
            Some(UmlClassElement::Instance(..)) => match self.current_stage {
                UmlClassToolStage::Instance { .. }
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::ClassProperty { .. }
                | UmlClassToolStage::ClassOperation { .. }
                | UmlClassToolStage::UseCase { .. }
                | UmlClassToolStage::PackageStart { .. }
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment { .. }
                | UmlClassToolStage::CommentLinkStart
                | UmlClassToolStage::LinkStart {
                    link_type: LinkType::Generalization { .. },
                }
                | UmlClassToolStage::LinkAddEnding { .. } => NON_TARGETTABLE_COLOR,

                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::LinkEnd => match &self.result {
                    PartialUmlClassElement::Link { link_type, .. }
                        if !matches!(link_type, LinkType::Generalization { .. }) =>
                    {
                        TARGETTABLE_COLOR
                    }
                    _ => NON_TARGETTABLE_COLOR,
                },
            },
            Some(UmlClassElement::Class(..)) => match self.current_stage {
                UmlClassToolStage::ClassProperty { .. }
                | UmlClassToolStage::ClassOperation { .. }
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::CommentLinkEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::Instance { .. }
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::UseCase { .. }
                | UmlClassToolStage::PackageStart { .. }
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment { .. }
                | UmlClassToolStage::CommentLinkStart => NON_TARGETTABLE_COLOR,

                UmlClassToolStage::LinkAddEnding { .. } | UmlClassToolStage::LinkEnd => {
                    match &self.result {
                        PartialUmlClassElement::Link {
                            link_type: LinkType::Generalization { .. },
                            source: UmlClassAssociable::Class(_),
                            ..
                        }
                        | PartialUmlClassElement::LinkEnding {
                            gen_model: UmlGeneralization::Generalization(_),
                            ..
                        } => TARGETTABLE_COLOR,
                        PartialUmlClassElement::Link { link_type, .. }
                            if !matches!(link_type, LinkType::Generalization { .. }) =>
                        {
                            TARGETTABLE_COLOR
                        }
                        _ => NON_TARGETTABLE_COLOR,
                    }
                }
            },
            Some(UmlClassElement::Property(..) | UmlClassElement::Operation(..)) => {
                NON_TARGETTABLE_COLOR
            }
            Some(UmlClassElement::UseCase(..)) => match self.current_stage {
                UmlClassToolStage::Instance { .. }
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::ClassProperty { .. }
                | UmlClassToolStage::ClassOperation { .. }
                | UmlClassToolStage::UseCase { .. }
                | UmlClassToolStage::PackageStart { .. }
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment { .. }
                | UmlClassToolStage::CommentLinkStart => NON_TARGETTABLE_COLOR,

                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }

                UmlClassToolStage::LinkAddEnding { .. } | UmlClassToolStage::LinkEnd => {
                    match &self.result {
                        PartialUmlClassElement::Link {
                            link_type: LinkType::Generalization { .. },
                            source: UmlClassAssociable::UseCase(_),
                            ..
                        }
                        | PartialUmlClassElement::LinkEnding {
                            gen_model: UmlGeneralization::UseCaseGeneralization(_),
                            ..
                        } => TARGETTABLE_COLOR,
                        PartialUmlClassElement::Link { link_type, .. }
                            if !matches!(link_type, LinkType::Generalization { .. }) =>
                        {
                            TARGETTABLE_COLOR
                        }
                        _ => NON_TARGETTABLE_COLOR,
                    }
                }
            },
            Some(UmlClassElement::Comment(..)) => match self.current_stage {
                UmlClassToolStage::CommentLinkStart => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::Instance { .. }
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::ClassProperty { .. }
                | UmlClassToolStage::ClassOperation { .. }
                | UmlClassToolStage::UseCase { .. }
                | UmlClassToolStage::PackageStart { .. }
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment { .. }
                | UmlClassToolStage::CommentLinkEnd => NON_TARGETTABLE_COLOR,
            },
            Some(
                UmlClassElement::Generalization(..)
                | UmlClassElement::Dependency(..)
                | UmlClassElement::Association(..)
                | UmlClassElement::UseCaseGeneralization(..)
                | UmlClassElement::CommentLink(..),
            ) => todo!(),
        }
    }
    fn draw_status_hint(
        &self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match &self.result {
            PartialUmlClassElement::Link { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlClassElement::LinkEnding { gen_model, .. } => {
                if let Some(view) = q.get_view_for(&gen_model.uuid()) {
                    canvas.draw_line(
                        [pos, view.position()],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlClassElement::CommentLink { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlClassElement::Package { a, .. } => {
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
                UmlClassToolStage::Instance {
                    instance_name,
                    instance_type,
                    stereotype,
                    background_color,
                },
                _,
            ) => {
                let (_object_model, object_view) = new_umlclass_instance(
                    instance_name,
                    instance_type,
                    stereotype,
                    "",
                    pos,
                    *background_color,
                );
                self.result = PartialUmlClassElement::Some(object_view.into());
                self.event_lock = true;
            }
            (
                UmlClassToolStage::Class {
                    name,
                    stereotype,
                    is_abstract,
                    render_style,
                    background_color,
                },
                _,
            ) => {
                let (_class_model, class_view) = new_umlclass_class(
                    &name,
                    &stereotype,
                    *is_abstract,
                    Vec::new(),
                    Vec::new(),
                    pos,
                    *render_style,
                    *background_color,
                );
                self.result = PartialUmlClassElement::Some(class_view.into());
                self.event_lock = true;
            }
            (
                UmlClassToolStage::UseCase {
                    name,
                    stereotype,
                    is_abstract,
                    background_color,
                },
                _,
            ) => {
                let (_usecase_model, usecase_view) =
                    new_uml_usecase(&name, &stereotype, *is_abstract, pos, *background_color);
                self.result = PartialUmlClassElement::Some(usecase_view.into());
                self.event_lock = true;
            }
            (
                UmlClassToolStage::PackageStart {
                    name,
                    stereotype,
                    kind,
                },
                _,
            ) => {
                self.result = PartialUmlClassElement::Package {
                    name: name.clone(),
                    stereotype: stereotype.clone(),
                    kind: *kind,
                    a: pos,
                    b: None,
                };
                self.current_stage = UmlClassToolStage::PackageEnd;
                self.event_lock = true;
            }
            (UmlClassToolStage::PackageEnd, PartialUmlClassElement::Package { b, .. }) => {
                *b = Some(pos);
                self.event_lock = true;
            }
            (
                UmlClassToolStage::Comment {
                    stereotype,
                    text,
                    align,
                },
                _,
            ) => {
                let (_comment_model, comment_view) =
                    new_umlclass_comment(text, stereotype, pos, *align);
                self.result = PartialUmlClassElement::Some(comment_view.into());
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, element: UmlClassElement) {
        if self.event_lock {
            return;
        }

        match element {
            UmlClassElement::Package(inner) => match (&self.current_stage, &mut self.result) {
                (
                    UmlClassToolStage::CommentLinkEnd,
                    PartialUmlClassElement::CommentLink { dest, .. },
                ) => {
                    *dest = Some(inner.into());
                    self.event_lock = true;
                }
                _ => {}
            },
            UmlClassElement::Instance(inner) => match (&self.current_stage, &mut self.result) {
                (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None)
                    if !matches!(link_type, LinkType::Generalization { .. }) =>
                {
                    self.result = PartialUmlClassElement::Link {
                        link_type: link_type.clone(),
                        source: inner.into(),
                        dest: None,
                    };
                    self.current_stage = UmlClassToolStage::LinkEnd;
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::LinkEnd,
                    PartialUmlClassElement::Link {
                        link_type, dest, ..
                    },
                ) => {
                    if !matches!(link_type, LinkType::Generalization { .. }) {
                        *dest = Some(inner.into());
                    }
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::CommentLinkEnd,
                    PartialUmlClassElement::CommentLink { dest, .. },
                ) => {
                    *dest = Some(inner.into());
                    self.event_lock = true;
                }
                _ => {}
            },
            UmlClassElement::Class(inner) => match (&self.current_stage, &mut self.result) {
                (
                    UmlClassToolStage::ClassProperty {
                        name,
                        property_type,
                        stereotype,
                    },
                    PartialUmlClassElement::None,
                ) => {
                    let (_property, property_view) = new_umlclass_property(
                        UFOption::None,
                        name,
                        property_type,
                        "",
                        "",
                        stereotype,
                    );
                    self.result = PartialUmlClassElement::Some(property_view.into());
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::ClassOperation {
                        name,
                        return_type,
                        stereotype,
                    },
                    PartialUmlClassElement::None,
                ) => {
                    let (_operation, operation_view) =
                        new_umlclass_operation(UFOption::None, name, "", return_type, stereotype);
                    self.result = PartialUmlClassElement::Some(operation_view.into());
                    self.event_lock = true;
                }
                (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                    self.result = PartialUmlClassElement::Link {
                        link_type: link_type.to_owned(),
                        source: inner.into(),
                        dest: None,
                    };
                    self.current_stage = UmlClassToolStage::LinkEnd;
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::LinkEnd,
                    PartialUmlClassElement::Link {
                        link_type,
                        source,
                        dest,
                    },
                ) => {
                    if !matches!(link_type, LinkType::Generalization { .. })
                        || matches!(source, UmlClassAssociable::Class(_))
                    {
                        *dest = Some(inner.into());
                    }
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::LinkAddEnding { source },
                    &mut PartialUmlClassElement::LinkEnding {
                        ref gen_model,
                        ref mut new_model,
                        ..
                    },
                ) => {
                    let inner_uuid = *inner.read().uuid;
                    if let UmlGeneralization::Generalization(inner2) = gen_model {
                        let r = inner2.read();

                        if (*source && !r.sources.iter().any(|e| *e.read().uuid == inner_uuid))
                            || (!source && !r.targets.iter().any(|e| *e.read().uuid == inner_uuid))
                        {
                            *new_model = Some(inner_uuid);
                        }
                        self.event_lock = true;
                    }
                }
                (
                    UmlClassToolStage::CommentLinkEnd,
                    PartialUmlClassElement::CommentLink { dest, .. },
                ) => {
                    *dest = Some(inner.into());
                    self.event_lock = true;
                }
                _ => {}
            },
            UmlClassElement::Property(..) | UmlClassElement::Operation(..) => {}
            UmlClassElement::UseCase(inner) => match (&self.current_stage, &mut self.result) {
                (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                    self.result = PartialUmlClassElement::Link {
                        link_type: link_type.to_owned(),
                        source: inner.into(),
                        dest: None,
                    };
                    self.current_stage = UmlClassToolStage::LinkEnd;
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::LinkEnd,
                    PartialUmlClassElement::Link {
                        link_type,
                        source,
                        dest,
                    },
                ) => {
                    if !matches!(link_type, LinkType::Generalization { .. })
                        || matches!(source, UmlClassAssociable::UseCase(_))
                    {
                        *dest = Some(inner.into());
                    }
                    self.event_lock = true;
                }
                (
                    UmlClassToolStage::LinkAddEnding { source },
                    &mut PartialUmlClassElement::LinkEnding {
                        ref gen_model,
                        ref mut new_model,
                        ..
                    },
                ) => {
                    let inner_uuid = *inner.read().uuid;
                    if let UmlGeneralization::UseCaseGeneralization(inner2) = gen_model {
                        let r = inner2.read();

                        if (*source && !r.sources.iter().any(|e| *e.read().uuid == inner_uuid))
                            || (!source && !r.targets.iter().any(|e| *e.read().uuid == inner_uuid))
                        {
                            *new_model = Some(inner_uuid);
                        }
                        self.event_lock = true;
                    }
                }
                (
                    UmlClassToolStage::CommentLinkEnd,
                    PartialUmlClassElement::CommentLink { dest, .. },
                ) => {
                    *dest = Some(inner.into());
                    self.event_lock = true;
                }
                _ => {}
            },
            UmlClassElement::Comment(inner) => match (&self.current_stage, &mut self.result) {
                (UmlClassToolStage::CommentLinkStart, PartialUmlClassElement::None) => {
                    self.result = PartialUmlClassElement::CommentLink {
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = UmlClassToolStage::CommentLinkEnd;
                    self.event_lock = true;
                }
                _ => {}
            },
            UmlClassElement::Generalization(..)
            | UmlClassElement::Dependency(..)
            | UmlClassElement::Association(..)
            | UmlClassElement::UseCaseGeneralization(..)
            | UmlClassElement::CommentLink(..) => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <UmlClassDomain<P> as Domain>::OrdinalMovementT,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &mut self.result {
            PartialUmlClassElement::LinkEnding {
                source,
                gen_model,
                new_model,
            } if new_model.is_some()
                && let Some(target) = q.get_viewuuid_for(&gen_model.uuid())
                && let Some(element) = q.get_view_for(&new_model.unwrap()) =>
            {
                commands.push(InsensitiveCommand::AddDependency {
                    target,
                    bucket: if *source {
                        MULTICONNECTION_SOURCE_BUCKET
                    } else {
                        MULTICONNECTION_TARGET_BUCKET
                    },
                    position: None,
                    element: element.into(),
                    into_model: true,
                });
                *new_model = None;
                Ok(None)
            }
            PartialUmlClassElement::Some(element) => {
                let element = element.clone();
                let esm: Option<Box<dyn CustomModal>> = match &element {
                    UmlClassElementView::Instance(inner) => {
                        Some(Box::new(UmlClassInstanceSetupModal::<
                            P::InstanceStereotypeController,
                        >::from(
                            &inner.read().model
                        )))
                    }
                    UmlClassElementView::Class(inner) => Some(Box::new(UmlClassSetupModal::<
                        P::ClassStereotypeController,
                    >::from(
                        &inner.read().model
                    ))),
                    UmlClassElementView::ClassProperty(inner) => {
                        Some(Box::new(UmlClassPropertySetupModal::<
                            P::ClassPropertyStereotypeController,
                        >::from(
                            &inner.read().model
                        )))
                    }
                    UmlClassElementView::ClassOperation(inner) => {
                        Some(Box::new(UmlClassOperationSetupModal::<
                            P::ClassOperationStereotypeController,
                        >::from(
                            &inner.read().model
                        )))
                    }
                    _ => None,
                };
                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlClassElementView::from(element).into(),
                    into_model: true,
                });
                Ok(esm)
            }
            PartialUmlClassElement::Link {
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
                {
                    self.current_stage = UmlClassToolStage::LinkStart {
                        link_type: link_type.clone(),
                    };

                    let link_view: UmlClassElementView<_> = match link_type {
                        LinkType::Generalization { set_name } => {
                            if let (
                                UmlClassAssociable::Class(source),
                                UmlClassAssociable::Class(dest),
                            ) = (&source, &dest)
                            {
                                new_umlclass_generalization(
                                    set_name,
                                    None,
                                    (source.clone(), source_view),
                                    (dest.clone(), target_view),
                                )
                                .1
                                .into()
                            } else if let (
                                UmlClassAssociable::UseCase(source),
                                UmlClassAssociable::UseCase(dest),
                            ) = (&source, &dest)
                            {
                                new_uml_usecasegeneralization(
                                    None,
                                    (source.clone(), source_view),
                                    (dest.clone(), target_view),
                                )
                                .1
                                .into()
                            } else {
                                return Err(());
                            }
                        }
                        LinkType::Dependency {
                            target_arrow_open,
                            stereotype,
                            name,
                        } => new_umlclass_dependency(
                            stereotype,
                            name,
                            *target_arrow_open,
                            None,
                            (source.clone(), source_view),
                            (dest.clone(), target_view),
                        )
                        .1
                        .into(),
                        LinkType::Association {
                            stereotype,
                            source_multiplicity,
                            target_multiplicity,
                        } => new_umlclass_association(
                            stereotype,
                            "",
                            source_multiplicity,
                            target_multiplicity,
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
            PartialUmlClassElement::CommentLink {
                source,
                dest: Some(dest),
            } => {
                let source_uuid = *source.read().uuid();
                if let (Some(source_view), Some(target_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&dest.uuid()))
                    && q.is_contained(&source_view.uuid(), preferred_container)
                    && q.is_contained(&target_view.uuid(), preferred_container)
                {
                    self.current_stage = UmlClassToolStage::CommentLinkStart;

                    let link_view = new_umlclass_commentlink(
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
                        element: UmlClassElementView::from(link_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialUmlClassElement::Package {
                name,
                stereotype,
                kind,
                a,
                b: Some(b),
            } => {
                self.current_stage = self.initial_stage.clone();

                let (_package_model, package_view) =
                    new_umlclass_package(name, stereotype, *kind, egui::Rect::from_two_pos(*a, *b));

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: UmlClassElementView::from(package_view).into(),
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

pub fn new_umlclass_package<P: UmlClassProfile>(
    name: &str,
    stereotype: &str,
    kind: UmlClassPackageKind,
    bounds_rect: egui::Rect,
) -> (ERef<UmlClassPackage>, ERef<PackageViewT<P>>) {
    let package_model = ERef::new(UmlClassPackage::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        stereotype.to_owned(),
        kind,
        Vec::new(),
    ));
    let package_view = new_umlclass_package_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
pub fn new_umlclass_package_view<P: UmlClassProfile>(
    model: ERef<UmlClassPackage>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT<P>> {
    let m = model.read();
    PackageView::new(
        ViewUuid::now_v7().into(),
        UmlClassPackageAdapter {
            model: model.clone(),
            background_color: MGlobalColor::None,
            display_text: Arc::new("".to_owned()),
            name_buffer: (*m.name).clone(),
            stereotype_controller: Default::default(),
            kind_buffer: m.kind.clone(),
            comment_buffer: (*m.comment).clone(),
            _profile: PhantomData,
        },
        Vec::new(),
        bounds_rect,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlClassPackageAdapter<P: UmlClassProfile> {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassPackage>,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    display_text: Arc<String>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::PackageStereotypeController,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: UmlClassPackageKind,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> PackageAdapter<UmlClassDomain<P>> for UmlClassPackageAdapter<P> {
    fn model_section(&self) -> UmlClassElement {
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
        element: UmlClassElement,
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
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
    ) -> Result<egui::Rect, Arc<String>> {
        match self.kind_buffer {
            UmlClassPackageKind::Package => {
                const PADDING: f32 = 4.0;
                let background_color = self.background_color(&context.global_colors);
                let foreground_color = self.text_color(&context.global_colors);
                let r = canvas.measure_text(
                    bounds_rect.left_top() + egui::Vec2::new(PADDING, -PADDING),
                    egui::Align2::LEFT_BOTTOM,
                    &self.display_text,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                );
                canvas.draw_rectangle(
                    r.expand(PADDING),
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, foreground_color),
                    highlight,
                );
                canvas.draw_text(
                    bounds_rect.left_top() + egui::Vec2::new(PADDING, -PADDING),
                    egui::Align2::LEFT_BOTTOM,
                    &self.display_text,
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    foreground_color,
                );
                Ok(r.expand(PADDING))
            }
            UmlClassPackageKind::Boundary => Err(self.display_text.clone()),
        }
    }

    fn show_model_properties(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Package kind:");
        egui::ComboBox::from_id_salt("package kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in UmlClassPackageKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::PackageKindChange(self.kind_buffer),
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
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlClassPropChange::StereotypeChange(stereotype) => {
                    if !self.stereotype_controller.is_valid(&stereotype) {
                        return;
                    }

                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                    ));
                    model.stereotype = stereotype.clone();
                }
                UmlClassPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlClassPropChange::PackageKindChange(kind) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::PackageKindChange(model.kind),
                    ));
                    model.kind = kind.clone();
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::ColorChange(ColorChangeData {
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

        self.display_text = if model.stereotype.is_empty() {
            model.name.clone()
        } else {
            format!("«{}» {}", model.stereotype, model.name).into()
        };
        self.stereotype_controller.refresh(&model.stereotype);
        self.name_buffer = (*model.name).clone();
        self.kind_buffer = model.kind;
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::Package(m)) = m.get(&old_model.uuid) {
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
            stereotype_controller: self.stereotype_controller.clone(),
            name_buffer: self.name_buffer.clone(),
            kind_buffer: self.kind_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            _profile: PhantomData,
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlClassElement>) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()) {
                *e = new_model.clone();
            }
        }
    }
}

fn new_umlclass_instance<P: UmlClassProfile>(
    instance_name: &str,
    instance_type: &str,
    stereotype: &str,
    instance_slots: &str,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> (ERef<UmlClassInstance>, ERef<UmlClassInstanceView<P>>) {
    let instance_model = ERef::new(UmlClassInstance::new(
        ModelUuid::now_v7(),
        instance_name.to_owned(),
        instance_type.to_owned(),
        stereotype.to_owned(),
        instance_slots.to_owned(),
    ));
    let instance_view =
        new_umlclass_instance_view(instance_model.clone(), position, background_color);

    (instance_model, instance_view)
}
fn new_umlclass_instance_view<P: UmlClassProfile>(
    model: ERef<UmlClassInstance>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> ERef<UmlClassInstanceView<P>> {
    let m = model.read();
    ERef::new(UmlClassInstanceView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        stereotype_in_guillemets: String::new(),
        main_text: String::new(),
        name_buffer: (*m.instance_name).clone(),
        type_buffer: (*m.instance_type).clone(),
        stereotype_controller: Default::default(),
        slots_buffer: (*m.instance_slots).clone(),
        comment_buffer: (*m.comment).clone(),
        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color,
        _profile: PhantomData,
    })
}

struct UmlClassInstanceSetupModal<SC: StereotypeController> {
    model: ERef<UmlClassInstance>,
    first_frame: bool,
    name_buffer: String,
    type_buffer: String,
    stereotype_controller: SC,
}

impl<SC: StereotypeController> From<&ERef<UmlClassInstance>> for UmlClassInstanceSetupModal<SC> {
    fn from(model: &ERef<UmlClassInstance>) -> Self {
        let m = model.read();
        let mut stereotype_controller: SC = Default::default();
        stereotype_controller.refresh(&*m.stereotype);
        Self {
            model: model.clone(),
            first_frame: true,
            name_buffer: (*m.instance_name).clone(),
            type_buffer: (*m.instance_type).clone(),
            stereotype_controller,
        }
    }
}

impl<SC: StereotypeController> CustomModal for UmlClassInstanceSetupModal<SC> {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        self.stereotype_controller.show(ui);
        ui.label("Name:");
        let r = ui.text_edit_singleline(&mut self.name_buffer);
        ui.label("Type:");
        ui.text_edit_singleline(&mut self.type_buffer);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let mut m = self.model.write();
                m.instance_name = Arc::new(self.name_buffer.clone());
                m.instance_type = Arc::new(self.type_buffer.clone());
                m.stereotype = self.stereotype_controller.get_arc();
                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button(gdc.translate_0("nh-generic-cancel")).clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassInstanceView<P: UmlClassProfile> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClassInstance>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: String,
    #[nh_context_serde(skip_and_default)]
    main_text: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    type_buffer: String,
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::InstanceStereotypeController,
    #[nh_context_serde(skip_and_default)]
    slots_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> UmlClassInstanceView<P> {
    const BUTTON_RADIUS: f32 = 8.0;
    fn button_rect(&self, ui_scale: f32, row_index: usize, column_index: usize) -> egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::new(
                (1.0 + 2.0 * column_index as f32) * Self::BUTTON_RADIUS / ui_scale,
                (1.0 + 2.0 * row_index as f32) * Self::BUTTON_RADIUS / ui_scale,
            );
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
}

impl<P: UmlClassProfile> Entity for UmlClassInstanceView<P> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<P: UmlClassProfile> View for UmlClassInstanceView<P> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl<P: UmlClassProfile> ElementController<UmlClassElement> for UmlClassInstanceView<P> {
    fn model(&self) -> UmlClassElement {
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

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassInstanceView<P> {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::InstanceName(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Type:", &mut self.type_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::InstanceType(Arc::new(self.type_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Slots:", &mut self.slots_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::InstanceSlots(Arc::new(self.slots_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                UmlClassPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
    ) -> TargettingStatus {
        let read = self.model.read();

        let mut min = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.main_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        let stereotype_botton = min.center_top();
        if !self.stereotype_in_guillemets.is_empty() {
            min = min.union(canvas.measure_text(
                stereotype_botton,
                egui::Align2::CENTER_BOTTOM,
                &self.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
            ));
        }
        let slots_top = min.center_bottom();
        if !read.instance_slots.is_empty() {
            min = min.union(canvas.measure_text(
                slots_top,
                egui::Align2::CENTER_TOP,
                &read.instance_slots,
                canvas::CLASS_ITEM_FONT_SIZE,
            ));
        }
        self.bounds_rect = min.expand(5.0);

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
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.main_text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        if !self.stereotype_in_guillemets.is_empty() {
            canvas.draw_text(
                stereotype_botton,
                egui::Align2::CENTER_BOTTOM,
                &self.stereotype_in_guillemets,
                canvas::CLASS_ITEM_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }
        if !read.instance_slots.is_empty() {
            canvas.draw_line(
                [self.bounds_rect.left(), self.bounds_rect.right()]
                    .map(|e| egui::Pos2::new(e, slots_top.y)),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );
            canvas.draw_text(
                slots_top,
                egui::Align2::CENTER_TOP,
                &read.instance_slots,
                canvas::CLASS_ITEM_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            for (row_idx, col_idx, l, _f) in settings.instance_buttons.iter() {
                let b_rect = self.button_rect(ui_scale, *row_idx, *col_idx);
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
                    l,
                    14.0 / ui_scale,
                    egui::Color32::BLACK,
                );
            }
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
        settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlClassTool<P>>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                    && let Some(e) =
                        settings
                            .instance_buttons
                            .iter()
                            .find(|(row_idx, col_idx, ..)| {
                                self.button_rect(ehc.ui_scale, *row_idx, *col_idx)
                                    .contains(pos)
                            }) =>
            {
                let (initial_stage, current_stage, result, event_lock) = e.3(self.model.clone());
                *tool = Some(NaiveUmlClassTool {
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
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                        UmlClassPropChange::InstanceName(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::InstanceName(model.instance_name.clone()),
                            ));
                            model.instance_name = name.clone();
                        }
                        UmlClassPropChange::InstanceType(t) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::InstanceType(model.instance_type.clone()),
                            ));
                            model.instance_type = t.clone();
                        }
                        UmlClassPropChange::InstanceSlots(s) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::InstanceSlots(model.instance_slots.clone()),
                            ));
                            model.instance_slots = s.clone();
                        }
                        UmlClassPropChange::StereotypeChange(stereotype) => {
                            if !self.stereotype_controller.is_valid(&stereotype) {
                                return;
                            }

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlClassPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::CommentChange(model.comment.clone()),
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
            String::new()
        } else {
            format!("«{}»", model.stereotype)
        };
        self.main_text = if model.instance_name.is_empty() {
            format!(":{}", model.instance_type)
        } else {
            format!("{}: {}", model.instance_name, model.instance_type)
        };

        self.name_buffer = (*model.instance_name).clone();
        self.type_buffer = (*model.instance_type).clone();
        self.stereotype_controller.refresh(&*model.stereotype);
        self.slots_buffer = (*model.instance_slots).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlClassElementView<P>, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        c: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::Instance(m)) = m.get(&old_model.uuid) {
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
            main_text: self.main_text.clone(),
            name_buffer: self.name_buffer.clone(),
            type_buffer: self.type_buffer.clone(),
            stereotype_controller: self.stereotype_controller.clone(),
            slots_buffer: self.slots_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_umlclass_property<P: UmlClassProfile>(
    visibility_modifier: UFOption<UmlClassVisibilityKind>,
    name: &str,
    value_type: &str,
    multiplicity: &str,
    default_value: &str,
    stereotype: &str,
) -> (ERef<UmlClassProperty>, ERef<UmlClassPropertyView<P>>) {
    let model = ERef::new(UmlClassProperty::new(
        ModelUuid::now_v7(),
        visibility_modifier,
        name.to_owned(),
        value_type.to_owned(),
        multiplicity.to_owned(),
        default_value.to_owned(),
        stereotype.to_owned(),
    ));
    let view = new_umlclass_property_view(model.clone());

    (model, view)
}

fn new_umlclass_property_view<P: UmlClassProfile>(
    model: ERef<UmlClassProperty>,
) -> ERef<UmlClassPropertyView<P>> {
    let m = model.read();
    ERef::new(UmlClassPropertyView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        display_text: String::new(),
        visibility_buffer: m.visibility,
        stereotype_controller: Default::default(),
        name_buffer: (*m.name).clone(),
        value_type_buffer: (*m.value_type).clone(),
        multiplicity_buffer: (*m.multiplicity).clone(),
        default_value_buffer: (*m.default_value).clone(),

        is_static_buffer: m.is_static,
        is_derived_buffer: m.is_derived,
        is_read_only_buffer: m.is_read_only,
        is_ordered_buffer: m.is_ordered,
        is_unique_buffer: m.is_unique,
        is_id_buffer: m.is_id,

        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::ZERO,
        _profile: PhantomData,
    })
}

struct UmlClassPropertySetupModal<SC: StereotypeController> {
    model: ERef<UmlClassProperty>,
    first_frame: bool,

    stereotype_controller: SC,
    name_buffer: String,
    value_type_buffer: String,
    multiplicity_buffer: String,
    default_value_buffer: String,

    visibility_buffer: UFOption<UmlClassVisibilityKind>,
    is_static_buffer: bool,
    is_derived_buffer: bool,
    is_read_only_buffer: bool,
    is_ordered_buffer: bool,
    is_unique_buffer: bool,
    is_id_buffer: bool,
}

impl<SC: StereotypeController> From<&ERef<UmlClassProperty>> for UmlClassPropertySetupModal<SC> {
    fn from(model: &ERef<UmlClassProperty>) -> Self {
        let m = model.read();
        let mut stereotype_controller: SC = Default::default();
        stereotype_controller.refresh(&*m.stereotype);
        Self {
            model: model.clone(),
            first_frame: true,

            stereotype_controller,
            name_buffer: (*m.name).clone(),
            value_type_buffer: (*m.value_type).clone(),
            multiplicity_buffer: (*m.multiplicity).clone(),
            default_value_buffer: (*m.default_value).clone(),

            visibility_buffer: m.visibility,
            is_static_buffer: m.is_static,
            is_derived_buffer: m.is_derived,
            is_read_only_buffer: m.is_read_only,
            is_ordered_buffer: m.is_ordered,
            is_unique_buffer: m.is_unique,
            is_id_buffer: m.is_id,
        }
    }
}

impl<SC: StereotypeController> CustomModal for UmlClassPropertySetupModal<SC> {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        self.stereotype_controller.show(ui);
        ui.label("Name:");
        let r = ui.text_edit_singleline(&mut self.name_buffer);
        ui.label("Type:");
        ui.text_edit_singleline(&mut self.value_type_buffer);
        ui.label("Multiplicity:");
        ui.text_edit_singleline(&mut self.multiplicity_buffer);
        ui.label("Default value:");
        ui.text_edit_singleline(&mut self.default_value_buffer);

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(
                self.visibility_buffer
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("Unspecified"),
            )
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    ui.selectable_value(
                        &mut self.visibility_buffer,
                        e,
                        e.as_ref().map(|e| e.as_str()).unwrap_or("Unspecified"),
                    );
                }
            });

        ui.checkbox(&mut self.is_static_buffer, "isStatic");
        ui.checkbox(&mut self.is_derived_buffer, "isDerived");
        ui.checkbox(&mut self.is_read_only_buffer, "isReadOnly");
        ui.checkbox(&mut self.is_ordered_buffer, "isOrdered");
        ui.checkbox(&mut self.is_unique_buffer, "isUnique");
        ui.checkbox(&mut self.is_id_buffer, "isID");

        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let mut m = self.model.write();

                m.stereotype = self.stereotype_controller.get_arc();
                m.name = Arc::new(self.name_buffer.clone());
                m.value_type = Arc::new(self.value_type_buffer.clone());
                m.multiplicity = Arc::new(self.multiplicity_buffer.clone());
                m.default_value = Arc::new(self.default_value_buffer.clone());

                m.visibility = self.visibility_buffer;
                m.is_static = self.is_static_buffer;
                m.is_derived = self.is_derived_buffer;
                m.is_read_only = self.is_read_only_buffer;
                m.is_ordered = self.is_ordered_buffer;
                m.is_unique = self.is_unique_buffer;
                m.is_id = self.is_id_buffer;

                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button(gdc.translate_0("nh-generic-cancel")).clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassPropertyView<P: UmlClassProfile> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClassProperty>,

    #[nh_context_serde(skip_and_default)]
    display_text: String,
    #[nh_context_serde(skip_and_default)]
    visibility_buffer: UFOption<UmlClassVisibilityKind>,
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::ClassPropertyStereotypeController,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    value_type_buffer: String,
    #[nh_context_serde(skip_and_default)]
    multiplicity_buffer: String,
    #[nh_context_serde(skip_and_default)]
    default_value_buffer: String,
    #[nh_context_serde(skip_and_default)]
    is_static_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_derived_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_read_only_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_ordered_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_unique_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_id_buffer: bool,

    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> UmlClassPropertyView<P> {
    fn draw_inner(
        &mut self,
        at: egui::Pos2,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
    ) -> (egui::Rect, TargettingStatus) {
        self.bounds_rect = canvas.measure_text(
            at,
            egui::Align2::LEFT_TOP,
            &self.display_text,
            canvas::CLASS_ITEM_FONT_SIZE,
        );
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            self.highlight,
        );
        canvas.draw_text(
            at,
            egui::Align2::LEFT_TOP,
            &self.display_text,
            canvas::CLASS_ITEM_FONT_SIZE,
            egui::Color32::BLACK,
        );
        if self.is_static_buffer {
            let d = egui::Vec2::new(0.0, 1.0);
            canvas.draw_line(
                [
                    self.bounds_rect.left_bottom() - d,
                    self.bounds_rect.right_bottom() - d,
                ],
                canvas::Stroke::new_solid(STATIC_UNDERLINE_WIDTH, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
        }
        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && self.bounds_rect.contains(*pos)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                tool.targetting_for_section(Some(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                canvas::Highlight::NONE,
            );

            (self.bounds_rect, TargettingStatus::Drawn)
        } else {
            (self.bounds_rect, TargettingStatus::NotDrawn)
        }
    }
}

impl<P: UmlClassProfile> Entity for UmlClassPropertyView<P> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<P: UmlClassProfile> View for UmlClassPropertyView<P> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl<P: UmlClassProfile> ElementController<UmlClassElement> for UmlClassPropertyView<P> {
    fn model(&self) -> UmlClassElement {
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

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassPropertyView<P> {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Type:", &mut self.value_type_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::PropertyTypeChange(Arc::new(self.value_type_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Multiplicity:", &mut self.multiplicity_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::PropertyMultiplicityChange(Arc::new(
                    self.multiplicity_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Default value:", &mut self.default_value_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::PropertyDefaultValueChange(Arc::new(
                    self.default_value_buffer.clone(),
                )),
            ));
        }

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(
                self.visibility_buffer
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("Unspecified"),
            )
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    if ui
                        .selectable_value(
                            &mut self.visibility_buffer,
                            e,
                            e.as_ref().map(|e| e.as_str()).unwrap_or("Unspecified"),
                        )
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::VisibilityChange(e),
                        ));
                    }
                }
            });

        if ui
            .checkbox(&mut self.is_static_buffer, "isStatic")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsStaticChange(self.is_static_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_derived_buffer, "isDerived")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsDerivedChange(self.is_derived_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_read_only_buffer, "isReadOnly")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsReadOnlyChange(self.is_read_only_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_ordered_buffer, "isOrdered")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsOrderedChange(self.is_ordered_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_unique_buffer, "isUnique")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsUniqueChange(self.is_unique_buffer),
            ));
        }
        if ui.checkbox(&mut self.is_id_buffer, "isID").changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsIdChange(self.is_id_buffer),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlClassOrdinalMovement::ClassChildUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlClassOrdinalMovement::ClassChildDown,
                ));
            }
        });

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(
            self.bounds_rect.left_top(),
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
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlClassDomain<P> as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
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
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            <UmlClassDomain<P> as Domain>::AddCommandElementT,
            <UmlClassDomain<P> as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
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
                        UmlClassPropChange::StereotypeChange(stereotype) => {
                            if !self.stereotype_controller.is_valid(&stereotype) {
                                return;
                            }

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlClassPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlClassPropChange::PropertyTypeChange(value_type) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::PropertyTypeChange(model.value_type.clone()),
                            ));
                            model.value_type = value_type.clone();
                        }
                        UmlClassPropChange::PropertyMultiplicityChange(multiplicity) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::PropertyMultiplicityChange(
                                    model.multiplicity.clone(),
                                ),
                            ));
                            model.multiplicity = multiplicity.clone();
                        }
                        UmlClassPropChange::PropertyDefaultValueChange(default_value) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::PropertyDefaultValueChange(
                                    model.default_value.clone(),
                                ),
                            ));
                            model.default_value = default_value.clone();
                        }
                        UmlClassPropChange::VisibilityChange(visibility) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::VisibilityChange(model.visibility.clone()),
                            ));
                            model.visibility = visibility.clone();
                        }
                        UmlClassPropChange::IsStaticChange(is_static) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsStaticChange(model.is_static.clone()),
                            ));
                            model.is_static = is_static.clone();
                        }
                        UmlClassPropChange::IsDerivedChange(is_derived) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsDerivedChange(model.is_derived.clone()),
                            ));
                            model.is_derived = is_derived.clone();
                        }
                        UmlClassPropChange::IsReadOnlyChange(is_read_only) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsReadOnlyChange(model.is_read_only.clone()),
                            ));
                            model.is_read_only = is_read_only.clone();
                        }
                        UmlClassPropChange::IsOrderedChange(is_ordered) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsOrderedChange(model.is_ordered.clone()),
                            ));
                            model.is_ordered = is_ordered.clone();
                        }
                        UmlClassPropChange::IsUniqueChange(is_unique) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsUniqueChange(model.is_unique.clone()),
                            ));
                            model.is_unique = is_unique.clone();
                        }
                        UmlClassPropChange::IsIdChange(is_id) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsIdChange(model.is_id.clone()),
                            ));
                            model.is_id = is_id.clone();
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
            let mut t = String::new();

            if !m.stereotype.is_empty() {
                t.push_str("«");
                t.push_str(&*m.stereotype);
                t.push_str("» ");
            }

            if let UFOption::Some(vis) = m.visibility {
                t.push_str(vis.as_char());
            }

            if m.is_derived {
                t.push_str("/");
            }
            t.push_str(&*m.name);

            if !m.value_type.is_empty() {
                t.push_str(": ");
                t.push_str(&*m.value_type);
            }

            if !m.multiplicity.is_empty() {
                if m.value_type.is_empty() {
                    t.push_str(" ");
                }
                t.push_str("[");
                t.push_str(&*m.multiplicity);
                t.push_str("]");
            }

            if !m.default_value.is_empty() {
                t.push_str(" = ");
                t.push_str(&*m.default_value);
            }

            if m.is_read_only || m.is_ordered || m.is_unique || m.is_id {
                t.push_str(" {");
                let mut first = true;
                for e in [
                    (m.is_id, "id"),
                    (m.is_read_only, "readOnly"),
                    (m.is_unique, "unique"),
                    (m.is_ordered, "ordered"),
                ] {
                    if !e.0 {
                        continue;
                    }
                    if first {
                        first = false;
                    } else {
                        t.push_str(", ");
                    }
                    t.push_str(e.1);
                }
                t.push_str("}");
            }

            t
        };

        self.visibility_buffer = m.visibility;
        self.stereotype_controller.refresh(&*m.stereotype);
        self.name_buffer = (*m.name).clone();
        self.value_type_buffer = (*m.value_type).clone();
        self.multiplicity_buffer = (*m.multiplicity).clone();
        self.default_value_buffer = (*m.default_value).clone();

        self.is_static_buffer = m.is_static;
        self.is_derived_buffer = m.is_derived;
        self.is_read_only_buffer = m.is_read_only;
        self.is_ordered_buffer = m.is_ordered;
        self.is_unique_buffer = m.is_unique;
        self.is_id_buffer = m.is_id;
    }
    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<
            ViewUuid,
            (<UmlClassDomain<P> as Domain>::CommonElementViewT, ViewUuid),
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
        tlc: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlClassDomain<P> as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::Property(m)) = m.get(&old_model.uuid) {
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
            visibility_buffer: self.visibility_buffer,
            stereotype_controller: self.stereotype_controller.clone(),
            name_buffer: self.name_buffer.clone(),
            value_type_buffer: self.value_type_buffer.clone(),
            multiplicity_buffer: self.multiplicity_buffer.clone(),
            default_value_buffer: self.default_value_buffer.clone(),

            is_static_buffer: self.is_static_buffer,
            is_derived_buffer: self.is_derived_buffer,
            is_read_only_buffer: self.is_read_only_buffer,
            is_ordered_buffer: self.is_ordered_buffer,
            is_unique_buffer: self.is_unique_buffer,
            is_id_buffer: self.is_id_buffer,

            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_umlclass_operation<P: UmlClassProfile>(
    visibility_modifier: UFOption<UmlClassVisibilityKind>,
    name: &str,
    parameters: &str,
    return_type: &str,
    stereotype: &str,
) -> (ERef<UmlClassOperation>, ERef<UmlClassOperationView<P>>) {
    let model = ERef::new(UmlClassOperation::new(
        ModelUuid::now_v7(),
        visibility_modifier,
        name.to_owned(),
        parameters.to_owned(),
        return_type.to_owned(),
        stereotype.to_owned(),
    ));
    let view = new_umlclass_operation_view(model.clone());

    (model, view)
}

fn new_umlclass_operation_view<P: UmlClassProfile>(
    model: ERef<UmlClassOperation>,
) -> ERef<UmlClassOperationView<P>> {
    let m = model.read();
    ERef::new(UmlClassOperationView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        display_text: String::new(),
        visibility_buffer: m.visibility,
        stereotype_controller: Default::default(),
        name_buffer: (*m.name).clone(),
        parameters_buffer: (*m.parameters).clone(),
        return_type_buffer: (*m.return_type).clone(),

        is_static_buffer: m.is_static,
        is_abstract_buffer: m.is_abstract,
        is_query_buffer: m.is_query,
        is_ordered_buffer: m.is_ordered,
        is_unique_buffer: m.is_unique,

        highlight: canvas::Highlight::NONE,
        bounds_rect: egui::Rect::ZERO,

        _profile: PhantomData,
    })
}

struct UmlClassOperationSetupModal<SC: StereotypeController> {
    model: ERef<UmlClassOperation>,
    first_frame: bool,

    stereotype_controller: SC,
    name_buffer: String,
    parameters_buffer: String,
    return_type_buffer: String,

    visibility_buffer: UFOption<UmlClassVisibilityKind>,
    is_static_buffer: bool,
    is_abstract_buffer: bool,
    is_query_buffer: bool,
    is_ordered_buffer: bool,
    is_unique_buffer: bool,
}

impl<SC: StereotypeController> From<&ERef<UmlClassOperation>> for UmlClassOperationSetupModal<SC> {
    fn from(model: &ERef<UmlClassOperation>) -> Self {
        let m = model.read();
        let mut stereotype_controller: SC = Default::default();
        stereotype_controller.refresh(&*m.stereotype);
        Self {
            model: model.clone(),
            first_frame: true,

            stereotype_controller,
            name_buffer: (*m.name).clone(),
            parameters_buffer: (*m.parameters).clone(),
            return_type_buffer: (*m.return_type).clone(),

            visibility_buffer: m.visibility,
            is_static_buffer: m.is_static,
            is_abstract_buffer: m.is_abstract,
            is_query_buffer: m.is_query,
            is_ordered_buffer: m.is_ordered,
            is_unique_buffer: m.is_unique,
        }
    }
}

impl<SC: StereotypeController> CustomModal for UmlClassOperationSetupModal<SC> {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        self.stereotype_controller.show(ui);
        ui.label("Name:");
        let r = ui.text_edit_singleline(&mut self.name_buffer);
        ui.label("Parameters:");
        ui.text_edit_singleline(&mut self.parameters_buffer);
        ui.label("Return type:");
        ui.text_edit_singleline(&mut self.return_type_buffer);

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(
                self.visibility_buffer
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("Unspecified"),
            )
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    ui.selectable_value(
                        &mut self.visibility_buffer,
                        e,
                        e.as_ref().map(|e| e.as_str()).unwrap_or("Unspecified"),
                    );
                }
            });

        ui.checkbox(&mut self.is_static_buffer, "isStatic");
        ui.checkbox(&mut self.is_abstract_buffer, "isAbstract");
        ui.checkbox(&mut self.is_query_buffer, "isQuery");
        ui.checkbox(&mut self.is_ordered_buffer, "isOrdered");
        ui.checkbox(&mut self.is_unique_buffer, "isUnique");

        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let mut m = self.model.write();
                m.stereotype = self.stereotype_controller.get_arc();
                m.name = Arc::new(self.name_buffer.clone());
                m.parameters = Arc::new(self.parameters_buffer.clone());
                m.return_type = Arc::new(self.return_type_buffer.clone());

                m.visibility = self.visibility_buffer;
                m.is_static = self.is_static_buffer;
                m.is_abstract = self.is_abstract_buffer;
                m.is_query = self.is_query_buffer;
                m.is_ordered = self.is_ordered_buffer;
                m.is_unique = self.is_unique_buffer;

                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button(gdc.translate_0("nh-generic-cancel")).clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassOperationView<P: UmlClassProfile> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClassOperation>,

    #[nh_context_serde(skip_and_default)]
    display_text: String,
    #[nh_context_serde(skip_and_default)]
    visibility_buffer: UFOption<UmlClassVisibilityKind>,
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::ClassOperationStereotypeController,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    parameters_buffer: String,
    #[nh_context_serde(skip_and_default)]
    return_type_buffer: String,
    #[nh_context_serde(skip_and_default)]
    is_static_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_abstract_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_query_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_ordered_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    is_unique_buffer: bool,

    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> UmlClassOperationView<P> {
    fn draw_inner(
        &mut self,
        at: egui::Pos2,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
    ) -> (egui::Rect, TargettingStatus) {
        self.bounds_rect = canvas.measure_text(
            at,
            egui::Align2::LEFT_TOP,
            &self.display_text,
            canvas::CLASS_ITEM_FONT_SIZE,
        );
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
            self.highlight,
        );
        let text_color = if !self.is_abstract_buffer {
            egui::Color32::BLACK
        } else {
            IS_ABSTRACT_COLOR
        };
        canvas.draw_text(
            at,
            egui::Align2::LEFT_TOP,
            &self.display_text,
            canvas::CLASS_ITEM_FONT_SIZE,
            text_color,
        );
        if self.is_static_buffer {
            let d = egui::Vec2::new(0.0, 1.0);
            canvas.draw_line(
                [
                    self.bounds_rect.left_bottom() - d,
                    self.bounds_rect.right_bottom() - d,
                ],
                canvas::Stroke::new_solid(STATIC_UNDERLINE_WIDTH, text_color),
                canvas::Highlight::NONE,
            );
        }
        if canvas.ui_scale().is_some()
            && let Some((pos, tool)) = tool
            && self.bounds_rect.contains(*pos)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                tool.targetting_for_section(Some(self.model.clone().into())),
                canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                canvas::Highlight::NONE,
            );

            (self.bounds_rect, TargettingStatus::Drawn)
        } else {
            (self.bounds_rect, TargettingStatus::NotDrawn)
        }
    }
}

impl<P: UmlClassProfile> Entity for UmlClassOperationView<P> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<P: UmlClassProfile> View for UmlClassOperationView<P> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl<P: UmlClassProfile> ElementController<UmlClassElement> for UmlClassOperationView<P> {
    fn model(&self) -> UmlClassElement {
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

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassOperationView<P> {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Parameters:", &mut self.parameters_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::OperationParametersChange(Arc::new(
                    self.parameters_buffer.clone(),
                )),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Return type:", &mut self.return_type_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::OperationReturnTypeChange(Arc::new(
                    self.return_type_buffer.clone(),
                )),
            ));
        }

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(
                self.visibility_buffer
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("Unspecified"),
            )
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    if ui
                        .selectable_value(
                            &mut self.visibility_buffer,
                            e,
                            e.as_ref().map(|e| e.as_str()).unwrap_or("Unspecified"),
                        )
                        .clicked()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::VisibilityChange(e),
                        ));
                    }
                }
            });

        if ui
            .checkbox(&mut self.is_static_buffer, "isStatic")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsStaticChange(self.is_static_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_abstract_buffer, "isAbstract")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsAbstractChange(self.is_abstract_buffer),
            ));
        }
        if ui.checkbox(&mut self.is_query_buffer, "isQuery").changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsQueryChange(self.is_query_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_ordered_buffer, "isOrdered")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsOrderedChange(self.is_ordered_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.is_unique_buffer, "isUnique")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::IsUniqueChange(self.is_unique_buffer),
            ));
        }

        ui.horizontal(|ui| {
            if ui.button("Move up").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlClassOrdinalMovement::ClassChildUp,
                ));
            }
            if ui.button("Move down").clicked() {
                commands.push(InsensitiveCommand::MoveOrdinal(
                    q.selected_views(),
                    UmlClassOrdinalMovement::ClassChildDown,
                ));
            }
        });

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(
            self.bounds_rect.left_top(),
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
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _tool: &mut Option<<UmlClassDomain<P> as Domain>::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
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
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            <UmlClassDomain<P> as Domain>::AddCommandElementT,
            <UmlClassDomain<P> as Domain>::PropChangeT,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                <UmlClassDomain<P> as Domain>::AddCommandElementT,
                <UmlClassDomain<P> as Domain>::PropChangeT,
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
                        UmlClassPropChange::StereotypeChange(stereotype) => {
                            if !self.stereotype_controller.is_valid(&stereotype) {
                                return;
                            }

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlClassPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlClassPropChange::OperationParametersChange(parameters) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::OperationParametersChange(
                                    model.parameters.clone(),
                                ),
                            ));
                            model.parameters = parameters.clone();
                        }
                        UmlClassPropChange::OperationReturnTypeChange(return_type) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::OperationReturnTypeChange(
                                    model.return_type.clone(),
                                ),
                            ));
                            model.return_type = return_type.clone();
                        }
                        UmlClassPropChange::VisibilityChange(visibility) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::VisibilityChange(model.visibility.clone()),
                            ));
                            model.visibility = visibility.clone();
                        }
                        UmlClassPropChange::IsStaticChange(is_static) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsStaticChange(model.is_static.clone()),
                            ));
                            model.is_static = is_static.clone();
                        }
                        UmlClassPropChange::IsAbstractChange(is_abstract) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsAbstractChange(model.is_abstract.clone()),
                            ));
                            model.is_abstract = is_abstract.clone();
                        }
                        UmlClassPropChange::IsQueryChange(is_query) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsQueryChange(model.is_query.clone()),
                            ));
                            model.is_query = is_query.clone();
                        }
                        UmlClassPropChange::IsOrderedChange(is_ordered) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsOrderedChange(model.is_ordered.clone()),
                            ));
                            model.is_ordered = is_ordered.clone();
                        }
                        UmlClassPropChange::IsUniqueChange(is_unique) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::IsUniqueChange(model.is_unique.clone()),
                            ));
                            model.is_unique = is_unique.clone();
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
            let mut t = String::new();

            if !m.stereotype.is_empty() {
                t.push_str("«");
                t.push_str(&*m.stereotype);
                t.push_str("» ");
            }

            if let UFOption::Some(vis) = m.visibility {
                t.push_str(vis.as_char());
            }

            t.push_str(&*m.name);
            t.push_str("(");
            t.push_str(&*m.parameters);
            t.push_str(")");

            if !m.return_type.is_empty() {
                t.push_str(": ");
                t.push_str(&*m.return_type);
            }

            if m.is_query || m.is_ordered || m.is_unique {
                t.push_str(" {");
                let mut first = true;
                for e in [
                    (m.is_query, "query"),
                    (m.is_unique, "unique"),
                    (m.is_ordered, "ordered"),
                ] {
                    if !e.0 {
                        continue;
                    }
                    if first {
                        first = false;
                    } else {
                        t.push_str(", ");
                    }
                    t.push_str(e.1);
                }
                t.push_str("}");
            }

            t
        };

        self.visibility_buffer = m.visibility;
        self.stereotype_controller.refresh(&*m.stereotype);
        self.name_buffer = (*m.name).clone();
        self.parameters_buffer = (*m.parameters).clone();
        self.return_type_buffer = (*m.return_type).clone();

        self.is_static_buffer = m.is_static;
        self.is_abstract_buffer = m.is_abstract;
        self.is_query_buffer = m.is_query;
        self.is_ordered_buffer = m.is_ordered;
        self.is_unique_buffer = m.is_unique;
    }
    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<
            ViewUuid,
            (<UmlClassDomain<P> as Domain>::CommonElementViewT, ViewUuid),
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
        tlc: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlClassDomain<P> as Domain>::CommonElementT>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::Operation(m)) = m.get(&old_model.uuid) {
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
            visibility_buffer: self.visibility_buffer,
            stereotype_controller: self.stereotype_controller.clone(),
            name_buffer: self.name_buffer.clone(),
            parameters_buffer: self.parameters_buffer.clone(),
            return_type_buffer: self.return_type_buffer.clone(),

            is_static_buffer: self.is_static_buffer,
            is_abstract_buffer: self.is_abstract_buffer,
            is_query_buffer: self.is_query_buffer,
            is_ordered_buffer: self.is_ordered_buffer,
            is_unique_buffer: self.is_unique_buffer,

            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlclass_class<P: UmlClassProfile>(
    name: &str,
    stereotype: &str,
    is_abstract: bool,
    properties: Vec<(ERef<UmlClassProperty>, ERef<UmlClassPropertyView<P>>)>,
    operations: Vec<(ERef<UmlClassOperation>, ERef<UmlClassOperationView<P>>)>,
    position: egui::Pos2,
    render_style: UmlClassRenderStyle,
    background_color: MGlobalColor,
) -> (ERef<UmlClass>, ERef<UmlClassView<P>>) {
    let class_model = ERef::new(UmlClass::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        stereotype.to_owned(),
        "".to_owned(),
        is_abstract,
        properties.iter().map(|e| e.0.clone()).collect(),
        operations.iter().map(|e| e.0.clone()).collect(),
    ));
    let class_view = new_umlclass_class_view(
        class_model.clone(),
        properties.iter().map(|e| e.1.clone()).collect(),
        operations.iter().map(|e| e.1.clone()).collect(),
        position,
        render_style,
        background_color,
    );

    (class_model, class_view)
}
pub fn new_umlclass_class_view<P: UmlClassProfile>(
    model: ERef<UmlClass>,
    properties_views: Vec<ERef<UmlClassPropertyView<P>>>,
    operations_views: Vec<ERef<UmlClassOperationView<P>>>,
    position: egui::Pos2,
    render_style: UmlClassRenderStyle,
    background_color: MGlobalColor,
) -> ERef<UmlClassView<P>> {
    let m = model.read();
    ERef::new(UmlClassView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        properties_views,
        operations_views,

        stereotype_in_guillemets: None,
        stereotype_controller: Default::default(),
        name_buffer: (*m.name).clone(),
        template_parameters_buffer: (*m.template_parameters).clone(),
        is_abstract_buffer: m.is_abstract,
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color,

        render_style,
        suppress_template_parameters: false,
        suppress_properties: false,
        suppress_operations: false,

        _profile: PhantomData,
    })
}

struct UmlClassSetupModal<SC: StereotypeController> {
    model: ERef<UmlClass>,
    first_frame: bool,
    stereotype_controller: SC,
    name_buffer: String,
}

impl<SC: StereotypeController> From<&ERef<UmlClass>> for UmlClassSetupModal<SC> {
    fn from(model: &ERef<UmlClass>) -> Self {
        let m = model.read();
        let mut stereotype_controller: SC = Default::default();
        stereotype_controller.refresh(&*m.stereotype);
        Self {
            model: model.clone(),
            first_frame: true,
            stereotype_controller,
            name_buffer: (*m.name).clone(),
        }
    }
}

impl<SC: StereotypeController> CustomModal for UmlClassSetupModal<SC> {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        self.stereotype_controller.show(ui);
        ui.label("Name:");
        let r = ui.text_edit_singleline(&mut self.name_buffer);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let mut m = self.model.write();
                m.stereotype = self.stereotype_controller.get_arc();
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

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UmlClassRenderStyle {
    Class,
    StickFigure,
}

impl UmlClassRenderStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            UmlClassRenderStyle::Class => "Class",
            UmlClassRenderStyle::StickFigure => "Stick Figure",
        }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassView<P: UmlClassProfile> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClass>,
    #[nh_context_serde(entity)]
    pub properties_views: Vec<ERef<UmlClassPropertyView<P>>>,
    #[nh_context_serde(entity)]
    pub operations_views: Vec<ERef<UmlClassOperationView<P>>>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: Option<Arc<String>>,
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::ClassStereotypeController,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    template_parameters_buffer: String,
    #[nh_context_serde(skip_and_default)]
    is_abstract_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,

    render_style: UmlClassRenderStyle,
    suppress_template_parameters: bool,
    suppress_properties: bool,
    suppress_operations: bool,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> UmlClassView<P> {
    const BUTTON_RADIUS: f32 = 8.0;
    fn button_rect(&self, ui_scale: f32, row_index: usize, column_index: usize) -> egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::new(
                (1.0 + 2.0 * column_index as f32) * Self::BUTTON_RADIUS / ui_scale,
                (1.0 + 2.0 * row_index as f32) * Self::BUTTON_RADIUS / ui_scale,
            );
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
}

impl<P: UmlClassProfile> Entity for UmlClassView<P> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<P: UmlClassProfile> View for UmlClassView<P> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl<P: UmlClassProfile> ElementController<UmlClassElement> for UmlClassView<P> {
    fn model(&self) -> UmlClassElement {
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

pub const IS_ABSTRACT_COLOR: egui::Color32 = egui::Color32::from_rgb(130, 130, 130);
pub const STATIC_UNDERLINE_WIDTH: f32 = 2.0;
pub fn draw_uml_class<'a>(
    canvas: &'a mut dyn canvas::NHCanvas,
    position: egui::Pos2,
    top_label: Option<Arc<String>>,
    main_label: &str,
    bottom_label: Option<Arc<String>>,
    is_abstract: bool,
    compartments: &[(
        egui::Vec2,
        Box<dyn Fn(&mut dyn canvas::NHCanvas, egui::Pos2) + 'a>,
    )],
    fill: egui::Color32,
    stroke: canvas::Stroke,
    highlight: canvas::Highlight,
) -> egui::Rect {
    // Measure phase
    let (offsets, global_offset, max_width, category_separators, rect) = {
        let mut offsets = vec![0.0];
        let mut max_width: f32 = 0.0;
        let mut category_separators = vec![];

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

        for c in compartments.iter() {
            category_separators.push(offsets.iter().sum::<f32>());
            offsets.push(c.0.y);
            max_width = max_width.max(c.0.x);
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
        canvas.draw_rectangle(
            rect,
            egui::CornerRadius::ZERO,
            fill,
            stroke.into(),
            highlight,
        );

        (offsets, global_offset, max_width, category_separators, rect)
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
                if !is_abstract {
                    egui::Color32::BLACK
                } else {
                    IS_ABSTRACT_COLOR
                },
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
            offset_counter += 1;
        }

        for (idx, c) in compartments.iter().enumerate() {
            if let Some(catline_offset) = category_separators.get(idx) {
                canvas.draw_line(
                    [
                        egui::Pos2::new(
                            position.x - rect.width() / 2.0,
                            position.y - global_offset + catline_offset,
                        ),
                        egui::Pos2::new(
                            position.x + rect.width() / 2.0,
                            position.y - global_offset + catline_offset,
                        ),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    highlight,
                );
            }

            (c.1)(
                canvas,
                egui::Pos2::new(
                    position.x - max_width / 2.0,
                    position.y - global_offset + offsets[offset_counter],
                ),
            );
            offset_counter += 1;
        }
    }

    rect
}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassView<P> {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        let properties_status = self
            .properties_views
            .iter()
            .flat_map(|e| {
                e.write()
                    .show_properties(gdc, q, ui, commands)
                    .to_non_default()
            })
            .next();
        if let Some(status) = properties_status.or_else(|| {
            self.operations_views
                .iter()
                .flat_map(|e| {
                    e.write()
                        .show_properties(gdc, q, ui, commands)
                        .to_non_default()
                })
                .next()
        }) {
            return status;
        }

        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline(
                "Template parameters:",
                &mut self.template_parameters_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::TemplateParametersChange(Arc::new(
                    self.template_parameters_buffer.clone(),
                )),
            ));
        }

        if ui
            .checkbox(&mut self.is_abstract_buffer, "isAbstract")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::ClassAbstractChange(self.is_abstract_buffer),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                UmlClassPropChange::ColorChange((0, new_color).into()),
            ));
        }

        ui.label("Render style");
        egui::ComboBox::from_id_salt("render style")
            .selected_text(self.render_style.as_str())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.render_style,
                    UmlClassRenderStyle::Class,
                    UmlClassRenderStyle::Class.as_str(),
                );
                if P::allows_class_rendering_as_stick_figure() {
                    ui.selectable_value(
                        &mut self.render_style,
                        UmlClassRenderStyle::StickFigure,
                        UmlClassRenderStyle::StickFigure.as_str(),
                    );
                }
            });

        ui.checkbox(
            &mut self.suppress_template_parameters,
            "suppress template parameters",
        );
        ui.checkbox(&mut self.suppress_properties, "suppress properties");
        ui.checkbox(&mut self.suppress_operations, "suppress operations");

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
    ) -> TargettingStatus {
        fn rect_union_fold<I: Iterator<Item = egui::Rect>>(elements: I) -> egui::Rect {
            let mut acc = egui::Rect::NOTHING;

            for e in elements {
                acc = acc.union(e);
            }

            if acc == egui::Rect::NOTHING {
                egui::Rect::ZERO
            } else {
                acc
            }
        }

        let read = self.model.read();
        let child_status = RwLock::new(TargettingStatus::NotDrawn);
        let body_color = context
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);

        if self.render_style == UmlClassRenderStyle::StickFigure {
            let p = self.position;
            let s = canvas::Stroke::new_solid(1.0, egui::Color32::BLACK);
            let h = self.highlight;
            canvas.draw_ellipse(
                p - egui::Vec2::new(0.0, 20.0),
                egui::Vec2::splat(10.0),
                body_color,
                s,
                h,
            );
            canvas.draw_line(
                [
                    p - egui::Vec2::new(20.0, 4.0),
                    p - egui::Vec2::new(-20.0, 4.0),
                ],
                s,
                h,
            ); // hands
            canvas.draw_line(
                [
                    p - egui::Vec2::new(0.0, 10.0),
                    p - egui::Vec2::new(0.0, -8.0),
                ],
                s,
                h,
            ); // torso
            canvas.draw_line(
                [
                    p - egui::Vec2::new(16.0, -28.0),
                    p - egui::Vec2::new(0.0, -8.0),
                ],
                s,
                h,
            ); // / leg
            canvas.draw_line(
                [
                    p - egui::Vec2::new(-16.0, -28.0),
                    p - egui::Vec2::new(0.0, -8.0),
                ],
                s,
                h,
            ); // \ leg
            self.bounds_rect = egui::Rect::from_min_max(
                p - egui::Vec2::new(20.0, 30.0),
                p + egui::Vec2::new(20.0, 28.0),
            );
            canvas.draw_text(
                p - egui::Vec2::new(0.0, -28.0),
                egui::Align2::CENTER_TOP,
                &read.name,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
        } else {
            let mut body = Vec::<(
                egui::Vec2,
                Box<dyn Fn(&mut dyn canvas::NHCanvas, egui::Pos2)>,
            )>::new();
            if !self.suppress_properties && !self.properties_views.is_empty() {
                body.push((
                    rect_union_fold(
                        self.properties_views
                            .iter()
                            .map(|e| e.read().bounding_box()),
                    )
                    .size(),
                    Box::new(|c, at| {
                        self.properties_views.iter().fold(at, |s, e| {
                            let r = e.write().draw_inner(s, q, context, settings, c, tool);
                            if r.1 != TargettingStatus::NotDrawn {
                                *child_status.write().unwrap() = r.1;
                            }
                            r.0.left_bottom()
                        });
                    }),
                ));
            }
            if !self.suppress_operations && !self.operations_views.is_empty() {
                body.push((
                    rect_union_fold(
                        self.operations_views
                            .iter()
                            .map(|e| e.read().bounding_box()),
                    )
                    .size(),
                    Box::new(|c, at| {
                        self.operations_views.iter().fold(at, |s, e| {
                            let r = e.write().draw_inner(s, q, context, settings, c, tool);
                            if r.1 != TargettingStatus::NotDrawn {
                                *child_status.write().unwrap() = r.1;
                            }
                            r.0.left_bottom()
                        });
                    }),
                ));
            }
            if settings.comment_indication == CommentIndication::TextCompartment
                && !read.comment.is_empty()
            {
                let comment = read.comment.clone();
                body.push((
                    canvas
                        .measure_text(
                            self.position,
                            egui::Align2::LEFT_TOP,
                            &*read.comment,
                            canvas::CLASS_ITEM_FONT_SIZE,
                        )
                        .size(),
                    Box::new(move |c, at| {
                        c.draw_text(
                            at,
                            egui::Align2::LEFT_TOP,
                            &*comment,
                            canvas::CLASS_ITEM_FONT_SIZE,
                            egui::Color32::BLACK,
                        );
                    }),
                ));
            }

            self.bounds_rect = draw_uml_class(
                canvas,
                self.position,
                self.stereotype_in_guillemets.clone(),
                &read.name,
                None,
                read.is_abstract,
                &body,
                body_color,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                self.highlight,
            );

            if !self.suppress_template_parameters && !read.template_parameters.is_empty() {
                let text_bounds = canvas
                    .measure_text(
                        self.bounds_rect.right_top(),
                        egui::Align2::CENTER_CENTER,
                        &read.template_parameters,
                        canvas::CLASS_TOP_FONT_SIZE,
                    )
                    .expand(2.0);
                canvas.draw_rectangle(
                    text_bounds,
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_dotted(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_text(
                    self.bounds_rect.right_top(),
                    egui::Align2::CENTER_CENTER,
                    &read.template_parameters,
                    canvas::CLASS_TOP_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }

            // Draw buttons
            if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
                for (row_idx, col_idx, l, _f) in settings.class_buttons.iter() {
                    let b1 = self.button_rect(ui_scale, *row_idx, *col_idx);
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
                        l,
                        14.0 / ui_scale,
                        egui::Color32::BLACK,
                    );
                }
            }
        }

        if canvas.ui_scale().is_some() {
            if settings.comment_indication == CommentIndication::Icon && !read.comment.is_empty() {
                canvas.draw_polygon(
                    {
                        let b = self.bounds_rect.left_top() + egui::Vec2::splat(2.5);
                        canvas::COMMENT_INDICATOR
                            .iter()
                            .map(|e| b + e.to_vec2())
                            .collect()
                    },
                    egui::Color32::LIGHT_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLUE),
                    canvas::Highlight::NONE,
                );
            }

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

            let child_status = *child_status.read().unwrap();

            // Draw targetting rectangle
            if let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
                && child_status == TargettingStatus::NotDrawn
            {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::CornerRadius::ZERO,
                    t.targetting_for_section(Some(self.model())),
                    canvas::Stroke::new_solid(
                        match self.render_style {
                            UmlClassRenderStyle::Class => 1.0,
                            UmlClassRenderStyle::StickFigure => 0.0,
                        },
                        egui::Color32::BLACK,
                    ),
                    canvas::Highlight::NONE,
                );
                TargettingStatus::Drawn
            } else {
                child_status
            }
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlClassTool<P>>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                    && let Some(e) =
                        settings
                            .class_buttons
                            .iter()
                            .find(|(row_idx, col_idx, ..)| {
                                self.button_rect(ehc.ui_scale, *row_idx, *col_idx)
                                    .contains(pos)
                            }) =>
            {
                let (initial_stage, current_stage, result, event_lock) = e.3(self.model.clone());
                *tool = Some(NaiveUmlClassTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage,
                    current_stage,
                    result,
                    event_lock,
                    is_spent: Some(false),
                });

                if let Some(tool) = tool {
                    tool.add_section(self.model());
                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands) {
                        if ehc
                            .modifier_settings
                            .alternative_tool_mode
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            *element_setup_modal = esm;
                        }
                    }
                }

                EventHandlingStatus::HandledByContainer
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                let child = self
                    .properties_views
                    .iter()
                    .map(|e| {
                        let mut w = e.write();
                        (
                            *w.uuid,
                            w.highlight.selected,
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
                    .find(|e| e.2 != EventHandlingStatus::NotHandled)
                    .or_else(|| {
                        self.operations_views
                            .iter()
                            .map(|e| {
                                let mut w = e.write();
                                (
                                    *w.uuid,
                                    w.highlight.selected,
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
                            .find(|e| e.2 != EventHandlingStatus::NotHandled)
                    });

                match child {
                    Some((uuid, selected, EventHandlingStatus::HandledByElement)) => {
                        if ehc
                            .modifier_settings
                            .hold_selection
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            commands
                                .push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED));
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(uuid).collect(),
                                true,
                                Highlight::SELECTED,
                            ));
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(uuid).collect(),
                                !selected,
                                Highlight::SELECTED,
                            ));
                        }
                        return EventHandlingStatus::HandledByContainer;
                    }
                    Some((.., EventHandlingStatus::HandledByContainer)) => {
                        return EventHandlingStatus::HandledByContainer;
                    }
                    _ => {}
                }

                if let Some(tool) = tool {
                    tool.add_section(self.model());

                    if let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands) {
                        if ehc
                            .modifier_settings
                            .alternative_tool_mode
                            .is_none_or(|e| !ehc.modifiers.is_superset_of(e))
                        {
                            *element_setup_modal = esm;
                        }
                    }
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
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.properties_views.iter().for_each(|e| {
                    e.write()
                        .apply_command(command, undo_accumulator, affected_models)
                });
                self.operations_views.iter().for_each(|e| {
                    e.write()
                        .apply_command(command, undo_accumulator, affected_models)
                });
            };
        }

        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
                recurse!();
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }
                recurse!();
            }
            InsensitiveCommand::SelectByDrag(rect, retain) => {
                self.highlight.selected = (self.highlight.selected && *retain)
                    || self.min_shape().contained_within(*rect);
                recurse!();
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
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                if *delete_kind != DeleteKind::DeleteView {
                    let mut removed_any = false;
                    self.properties_views.retain(|e| {
                        let r = e.read();
                        if uuids.contains(&r.uuid)
                            && let Some((b, pos)) =
                                self.model.write().get_element_pos(&r.model_uuid())
                        {
                            undo_accumulator.push(InsensitiveCommand::AddDependency {
                                target: *self.uuid,
                                bucket: b,
                                position: Some(pos),
                                element: UmlClassElementOrVertex::Element(e.clone().into()),
                                into_model: false,
                            });
                            removed_any = true;
                            false
                        } else {
                            true
                        }
                    });
                    self.operations_views.retain(|e| {
                        let r = e.read();
                        if uuids.contains(&r.uuid)
                            && let Some((b, pos)) =
                                self.model.write().get_element_pos(&r.model_uuid())
                        {
                            undo_accumulator.push(InsensitiveCommand::AddDependency {
                                target: *self.uuid,
                                bucket: b,
                                position: Some(pos),
                                element: UmlClassElementOrVertex::Element(e.clone().into()),
                                into_model: false,
                            });
                            removed_any = true;
                            false
                        } else {
                            true
                        }
                    });

                    if removed_any {
                        affected_models.insert(*self.model.read().uuid);
                    }
                }
            }
            InsensitiveCommand::AddDependency {
                target,
                bucket,
                position,
                element,
                into_model,
            } => {
                if *target == *self.uuid
                    && let UmlClassElementOrVertex::Element(e) = element
                {
                    let mut w = self.model.write();
                    if let Some(model_pos) =
                        w.get_element_pos(&e.model_uuid()).map(|e| e.1).or_else(|| {
                            if *into_model {
                                w.insert_element(*bucket, *position, e.model()).ok()
                            } else {
                                None
                            }
                        })
                    {
                        let mut model_transitives = HashMap::new();
                        e.clone().head_count(
                            &mut HashMap::new(),
                            &mut HashMap::new(),
                            &mut model_transitives,
                        );
                        affected_models.extend(model_transitives.into_keys());

                        match e {
                            UmlClassElementView::ClassProperty(inner) => {
                                let view_pos = |arr: &Vec<ERef<UmlClassPropertyView<_>>>| {
                                    let mut view_pos: PositionNoT = 0;
                                    for e in arr {
                                        let Some((_b, pos)) =
                                            w.get_element_pos(&e.read().model_uuid())
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
                                let view_pos = view_pos(&self.properties_views);
                                self.properties_views.insert(view_pos, inner.clone());
                            }
                            UmlClassElementView::ClassOperation(inner) => {
                                let view_pos = |arr: &Vec<ERef<UmlClassOperationView<_>>>| {
                                    let mut view_pos: PositionNoT = 0;
                                    for e in arr {
                                        let Some((_b, pos)) =
                                            w.get_element_pos(&e.read().model_uuid())
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
                                let view_pos = view_pos(&self.operations_views);
                                self.operations_views.insert(view_pos, inner.clone());
                            }
                            _ => return,
                        }

                        let uuid = *e.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            element: uuid,
                            including_model: *into_model,
                        });
                        affected_models.insert(*w.uuid);
                    }
                }
            }
            InsensitiveCommand::RemoveDependency {
                target,
                bucket,
                element,
                including_model,
            } => {
                if *target == *self.uuid && *including_model {
                    let mut removed_any = false;
                    if *bucket == 0 || *bucket == UmlClass::PROPERTIES_BUCKET {
                        self.properties_views.retain(|e| {
                            let r = e.read();
                            if *r.uuid == *element
                                && let Some((b, pos)) =
                                    self.model.write().remove_element(&r.model_uuid())
                            {
                                undo_accumulator.push(InsensitiveCommand::AddDependency {
                                    target: *self.uuid,
                                    bucket: b,
                                    position: Some(pos),
                                    element: UmlClassElementOrVertex::Element(e.clone().into()),
                                    into_model: true,
                                });
                                removed_any = true;
                                false
                            } else {
                                true
                            }
                        });
                    }
                    if *bucket == 0 || *bucket == UmlClass::OPERATIONS_BUCKET {
                        self.operations_views.retain(|e| {
                            let r = e.read();
                            if *r.uuid == *element
                                && let Some((b, pos)) =
                                    self.model.write().remove_element(&r.model_uuid())
                            {
                                undo_accumulator.push(InsensitiveCommand::AddDependency {
                                    target: *self.uuid,
                                    bucket: b,
                                    position: Some(pos),
                                    element: UmlClassElementOrVertex::Element(e.clone().into()),
                                    into_model: true,
                                });
                                removed_any = true;
                                false
                            } else {
                                true
                            }
                        });
                    }

                    if removed_any {
                        affected_models.insert(*self.model.read().uuid);
                    }
                }
            }
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(uuids, direction) => {
                let mut undo_uuids = HashSet::new();
                {
                    let properties_iter: Box<
                        dyn Iterator<Item = &mut ERef<UmlClassPropertyView<P>>>,
                    > = match direction {
                        UmlClassOrdinalMovement::ClassChildUp => {
                            Box::new(self.properties_views.iter_mut())
                        }
                        UmlClassOrdinalMovement::ClassChildDown => {
                            Box::new(self.properties_views.iter_mut().rev())
                        }
                    };
                    let mut properties_iter = properties_iter.peekable();
                    while let Some(dest) = properties_iter.next()
                        && let Some(src) = properties_iter.peek_mut()
                    {
                        if uuids.contains(&src.read().uuid) && !uuids.contains(&dest.read().uuid) {
                            let mut w = self.model.write();
                            let Some(new_pos) = w.get_element_pos(&dest.read().model_uuid()) else {
                                continue;
                            };
                            w.move_element(
                                &src.read().model_uuid(),
                                UmlClass::PROPERTIES_BUCKET,
                                new_pos.1,
                            );
                            undo_uuids.insert(*src.read().uuid);
                            std::mem::swap(dest, *src);
                        }
                    }
                }
                {
                    let operations_iter: Box<
                        dyn Iterator<Item = &mut ERef<UmlClassOperationView<P>>>,
                    > = match direction {
                        UmlClassOrdinalMovement::ClassChildUp => {
                            Box::new(self.operations_views.iter_mut())
                        }
                        UmlClassOrdinalMovement::ClassChildDown => {
                            Box::new(self.operations_views.iter_mut().rev())
                        }
                    };
                    let mut operations_iter = operations_iter.peekable();
                    while let Some(dest) = operations_iter.next()
                        && let Some(src) = operations_iter.peek_mut()
                    {
                        if uuids.contains(&src.read().uuid) && !uuids.contains(&dest.read().uuid) {
                            let mut w = self.model.write();
                            let Some(new_pos) = w.get_element_pos(&dest.read().model_uuid()) else {
                                continue;
                            };
                            w.move_element(
                                &src.read().model_uuid(),
                                UmlClass::OPERATIONS_BUCKET,
                                new_pos.1,
                            );
                            undo_uuids.insert(*src.read().uuid);
                            std::mem::swap(dest, *src);
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
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        UmlClassPropChange::StereotypeChange(stereotype) => {
                            if !self.stereotype_controller.is_valid(&stereotype) {
                                return;
                            }

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlClassPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlClassPropChange::TemplateParametersChange(template_parameters) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::TemplateParametersChange(
                                    model.template_parameters.clone(),
                                ),
                            ));
                            model.template_parameters = template_parameters.clone();
                        }
                        UmlClassPropChange::ClassAbstractChange(is_abstract) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::ClassAbstractChange(model.is_abstract),
                            ));
                            model.is_abstract = *is_abstract;
                        }
                        UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlClassPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::CommentChange(model.comment.clone()),
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

        self.stereotype_in_guillemets = if model.stereotype.is_empty() {
            None
        } else {
            Some(format!("«{}»", model.stereotype).into())
        };

        self.stereotype_controller.refresh(&*model.stereotype);
        self.name_buffer = (*model.name).clone();
        self.template_parameters_buffer = (*model.template_parameters).clone();
        self.is_abstract_buffer = model.is_abstract;
        self.comment_buffer = (*model.comment).clone();

        for e in &self.properties_views {
            e.write().refresh_buffers();
        }
        for e in &self.operations_views {
            e.write().refresh_buffers();
        }
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (UmlClassElementView<P>, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        for e in &self.properties_views {
            let mut w = e.write();
            w.head_count(
                flattened_views,
                flattened_views_status,
                flattened_represented_models,
            );
            flattened_views.insert(*w.uuid(), (e.clone().into(), *self.uuid));
        }
        for e in &self.operations_views {
            let mut w = e.write();
            w.head_count(
                flattened_views,
                flattened_views_status,
                flattened_represented_models,
            );
            flattened_views.insert(*w.uuid(), (e.clone().into(), *self.uuid));
        }
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, <UmlClassDomain<P> as Domain>::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            for e in &self.properties_views {
                e.read().deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
            for e in &self.operations_views {
                e.read().deep_copy_walk(requested, uuid_present, tlc, c, m);
            }
        }
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        c: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::Class(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut dev_null = HashMap::new();
        let properties_views = self
            .properties_views
            .iter()
            .flat_map(|e| {
                e.read().deep_copy_clone(uuid_present, &mut dev_null, c, m);
                if let Some(UmlClassElementView::ClassProperty(new_view)) = c.get(&e.read().uuid) {
                    Some(new_view.clone())
                } else {
                    None
                }
            })
            .collect();
        let operations_views = self
            .operations_views
            .iter()
            .flat_map(|e| {
                e.read().deep_copy_clone(uuid_present, &mut dev_null, c, m);
                if let Some(UmlClassElementView::ClassOperation(new_view)) = c.get(&e.read().uuid) {
                    Some(new_view.clone())
                } else {
                    None
                }
            })
            .collect();

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            properties_views,
            operations_views,
            stereotype_in_guillemets: self.stereotype_in_guillemets.clone(),
            stereotype_controller: self.stereotype_controller.clone(),
            name_buffer: self.name_buffer.clone(),
            template_parameters_buffer: self.template_parameters_buffer.clone(),
            is_abstract_buffer: self.is_abstract_buffer,
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            render_style: self.render_style.clone(),
            suppress_template_parameters: self.suppress_template_parameters,
            suppress_properties: self.suppress_properties,
            suppress_operations: self.suppress_operations,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        _c: &HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlClassDomain<P> as Domain>::CommonElementT>,
    ) {
        let mut w = self.model.write();
        for e in w.properties.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlClassElement::Property(new_property)) = m.get(&uuid) {
                *e = new_property.clone();
            }
        }
        for e in w.operations.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlClassElement::Operation(new_operation)) = m.get(&uuid) {
                *e = new_operation.clone();
            }
        }
    }
}

pub fn new_uml_usecase<P: UmlClassProfile>(
    name: &str,
    stereotype: &str,
    is_abstract: bool,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> (ERef<UmlUseCase>, ERef<UmlUseCaseView<P>>) {
    let usecase_model = ERef::new(UmlUseCase::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        stereotype.to_owned(),
        is_abstract,
    ));
    let usecase_view = new_uml_usecase_view(usecase_model.clone(), position, background_color);

    (usecase_model, usecase_view)
}
pub fn new_uml_usecase_view<P: UmlClassProfile>(
    model: ERef<UmlUseCase>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> ERef<UmlUseCaseView<P>> {
    let m = model.read();
    ERef::new(UmlUseCaseView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        stereotype_in_guillemets: None,
        stereotype_controller: Default::default(),
        name_buffer: (*m.name).clone(),
        is_abstract_buffer: m.is_abstract,
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_radius: egui::Vec2::ZERO,
        background_color,

        _profile: PhantomData,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlUseCaseView<P: UmlClassProfile> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlUseCase>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: Option<Arc<String>>,
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::UseCaseStereotypeController,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    is_abstract_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_radius: egui::Vec2,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> Entity for UmlUseCaseView<P> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<P: UmlClassProfile> View for UmlUseCaseView<P> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl<P: UmlClassProfile> ElementController<UmlClassElement> for UmlUseCaseView<P> {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Ellipse {
            position: self.position,
            bounds_radius: self.bounds_radius,
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlUseCaseView<P> {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .checkbox(&mut self.is_abstract_buffer, "isAbstract")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::ClassAbstractChange(self.is_abstract_buffer),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                UmlClassPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
    ) -> TargettingStatus {
        // Draw shape and text
        let name_bounds = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        let mut text_bounds = name_bounds;
        if let Some(s) = &self.stereotype_in_guillemets {
            let stereotype_bounds = canvas.measure_text(
                name_bounds.center_top(),
                egui::Align2::CENTER_BOTTOM,
                &s,
                canvas::CLASS_TOP_FONT_SIZE,
            );
            text_bounds = text_bounds.union(stereotype_bounds);
        }

        self.bounds_radius = text_bounds.size() / 1.5;

        canvas.draw_ellipse(
            self.position,
            self.bounds_radius,
            context
                .global_colors
                .get(&self.background_color)
                .unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            if !self.is_abstract_buffer {
                egui::Color32::BLACK
            } else {
                IS_ABSTRACT_COLOR
            },
        );
        if let Some(s) = &self.stereotype_in_guillemets {
            canvas.draw_text(
                name_bounds.center_top(),
                egui::Align2::CENTER_BOTTOM,
                &s,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        // Draw targetting ellipse
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
        {
            canvas.draw_ellipse(
                self.position,
                self.bounds_radius,
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
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlClassTool<P>>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                        UmlClassPropChange::StereotypeChange(stereotype) => {
                            if !self.stereotype_controller.is_valid(&stereotype) {
                                return;
                            }

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlClassPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        UmlClassPropChange::ClassAbstractChange(is_abstract) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::ClassAbstractChange(model.is_abstract),
                            ));
                            model.is_abstract = *is_abstract;
                        }
                        UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlClassPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::CommentChange(model.comment.clone()),
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

        self.stereotype_controller.refresh(&*model.stereotype);
        self.name_buffer = (*model.name).clone();
        self.is_abstract_buffer = model.is_abstract;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlClassElementView<P>, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        c: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::UseCase(m)) = m.get(&old_model.uuid) {
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
            stereotype_controller: self.stereotype_controller.clone(),
            name_buffer: self.name_buffer.clone(),
            is_abstract_buffer: self.is_abstract_buffer,
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_radius: self.bounds_radius,
            background_color: self.background_color,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlclass_generalization<P: UmlClassProfile>(
    set_name: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClass>, UmlClassElementView<P>),
    target: (ERef<UmlClass>, UmlClassElementView<P>),
) -> (ERef<UmlClassGeneralization>, ERef<GeneralizationViewT<P>>) {
    let link_model = ERef::new(UmlClassGeneralization::new(
        ModelUuid::now_v7(),
        set_name.to_owned(),
        vec![source.0],
        vec![target.0],
    ));
    let link_view = new_umlclass_generalization_view(
        link_model.clone(),
        center_point,
        vec![source.1],
        vec![target.1],
    );
    (link_model, link_view)
}
pub fn new_umlclass_generalization_view<P: UmlClassProfile>(
    model: ERef<UmlClassGeneralization>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    sources: Vec<UmlClassElementView<P>>,
    targets: Vec<UmlClassElementView<P>>,
) -> ERef<GeneralizationViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        m.sources.iter().map(|e| *e.read().uuid),
        *m.targets[0].read().uuid,
        targets[0].min_shape(),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlClassGeneralizationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        sources
            .into_iter()
            .zip(sp.into_iter())
            .map(|e| Ending::new_p(e.0, e.1))
            .collect(),
        targets
            .into_iter()
            .zip(tp.into_iter())
            .map(|e| Ending::new_p(e.0, e.1))
            .collect(),
        mp,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlClassGeneralizationAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassGeneralization>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlClassGeneralizationTemporaries,
}

#[derive(Clone, Default)]
struct UmlClassGeneralizationTemporaries {
    midpoint_label: Option<Arc<String>>,
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    set_name_buffer: String,
    set_is_covering_buffer: bool,
    set_is_disjoint_buffer: bool,
    comment_buffer: String,
}

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>>
    for UmlClassGeneralizationAdapter
{
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
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
        self.model.write().flip_multiconnection();
        Ok(())
    }
    fn insert_source(
        &mut self,
        position: Option<PositionNoT>,
        e: <UmlClassDomain<P> as Domain>::CommonElementT,
    ) -> Result<PositionNoT, ()> {
        self.model
            .write()
            .insert_element(MULTICONNECTION_SOURCE_BUCKET, position, e)
            .map_err(|_| ())
    }
    fn remove_source(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        let mut w = self.model.write();
        if w.sources.len() == 1 {
            return None;
        }
        for (idx, e) in w.sources.iter().enumerate() {
            if *e.read().uuid == *uuid {
                w.sources.remove(idx);
                return Some(idx.try_into().unwrap());
            }
        }
        None
    }
    fn insert_target(
        &mut self,
        position: Option<PositionNoT>,
        e: <UmlClassDomain<P> as Domain>::CommonElementT,
    ) -> Result<PositionNoT, ()> {
        self.model
            .write()
            .insert_element(MULTICONNECTION_TARGET_BUCKET, position, e)
            .map_err(|_| ())
    }
    fn remove_target(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        let mut w = self.model.write();
        if w.targets.len() == 1 {
            return None;
        }
        for (idx, e) in w.targets.iter().enumerate() {
            if *e.read().uuid == *uuid {
                w.targets.remove(idx);
                return Some(idx.try_into().unwrap());
            }
        }
        None
    }

    fn show_properties(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if ui
            .add_enabled(
                self.model.read().targets.len() <= 1,
                egui::Button::new("Add source"),
            )
            .clicked()
        {
            return PropertiesStatus::ToolRequest(Some(NaiveUmlClassTool {
                uuid: uuid::Uuid::nil(),
                initial_stage: UmlClassToolStage::LinkAddEnding { source: true },
                current_stage: UmlClassToolStage::LinkAddEnding { source: true },
                result: PartialUmlClassElement::LinkEnding {
                    source: true,
                    gen_model: self.model.clone().into(),
                    new_model: None,
                },
                event_lock: false,
                is_spent: Some(false),
            }));
        }
        if ui
            .add_enabled(
                self.model.read().sources.len() <= 1,
                egui::Button::new("Add target"),
            )
            .clicked()
        {
            return PropertiesStatus::ToolRequest(Some(NaiveUmlClassTool {
                uuid: uuid::Uuid::nil(),
                initial_stage: UmlClassToolStage::LinkAddEnding { source: false },
                current_stage: UmlClassToolStage::LinkAddEnding { source: false },
                result: PartialUmlClassElement::LinkEnding {
                    source: false,
                    gen_model: self.model.clone().into(),
                    new_model: None,
                },
                event_lock: false,
                is_spent: Some(false),
            }));
        }

        if ui
            .add_enabled(true, egui::Button::new("Switch source and target"))
            .clicked()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }

        ui.label("Generalization set name:");
        if ui
            .text_edit_singleline(&mut self.temporaries.set_name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::SetNameChange(Arc::new(
                    self.temporaries.set_name_buffer.clone(),
                )),
            ));
        }
        if ui
            .checkbox(&mut self.temporaries.set_is_covering_buffer, "isCovering")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::SetCoveringChange(self.temporaries.set_is_covering_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.temporaries.set_is_disjoint_buffer, "isDisjoint")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::SetDisjointChange(self.temporaries.set_is_disjoint_buffer),
            ));
        }
        ui.separator();

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(
                    self.temporaries.comment_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlClassPropChange::SetNameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::SetNameChange(model.set_name.clone()),
                    ));
                    model.set_name = name.clone();
                }
                UmlClassPropChange::SetCoveringChange(is_covering) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::SetCoveringChange(model.set_is_covering.clone()),
                    ));
                    model.set_is_covering = is_covering.clone();
                }
                UmlClassPropChange::SetDisjointChange(is_disjoint) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::SetDisjointChange(model.set_is_disjoint.clone()),
                    ));
                    model.set_is_disjoint = is_disjoint.clone();
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        let set_props_label = if model.sources.len() > 1 || model.targets.len() > 1 {
            Some(format!(
                "{{{}, {}}}",
                if model.set_is_covering {
                    "complete"
                } else {
                    "incomplete"
                },
                if model.set_is_disjoint {
                    "disjoint"
                } else {
                    "overlapping"
                },
            ))
        } else {
            None
        };
        self.temporaries.midpoint_label = if let Some(spl) = set_props_label {
            Some(Arc::new(format!("{}\n{}", model.set_name, spl)))
        } else if !model.set_name.is_empty() {
            Some(model.set_name.clone())
        } else {
            None
        };

        self.temporaries.arrow_data.clear();
        for e in &model.sources {
            self.temporaries.arrow_data.insert(
                (false, *e.read().uuid),
                ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::None),
            );
        }
        for e in &model.targets {
            self.temporaries.arrow_data.insert(
                (true, *e.read().uuid),
                ArrowData::new_labelless(
                    canvas::LineType::Solid,
                    canvas::ArrowheadType::EmptyTriangle,
                ),
            );
        }

        self.temporaries.source_uuids.clear();
        for e in &model.sources {
            self.temporaries.source_uuids.push(*e.read().uuid);
        }
        self.temporaries.target_uuids.clear();
        for e in &model.targets {
            self.temporaries.target_uuids.push(*e.read().uuid);
        }

        self.temporaries.set_name_buffer = (*model.set_name).clone();
        self.temporaries.set_is_covering_buffer = model.set_is_covering;
        self.temporaries.set_is_disjoint_buffer = model.set_is_disjoint;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::Generalization(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlClassElement>) {
        let mut model = self.model.write();

        for e in model.sources.iter_mut() {
            let sid = *e.read().uuid;
            if let Some(UmlClassElement::Class(new_source)) = m.get(&sid) {
                *e = new_source.clone();
            }
        }
        for e in model.targets.iter_mut() {
            let tid = *e.read().uuid;
            if let Some(UmlClassElement::Class(new_target)) = m.get(&tid) {
                *e = new_target.clone();
            }
        }
    }
}

pub fn stereotype_name_format(stereotype: &str, name: &str) -> Option<Arc<String>> {
    if stereotype.is_empty() && name.is_empty() {
        None
    } else {
        let mut label = String::new();
        if !stereotype.is_empty() {
            label.push_str("«");
            label.push_str(stereotype);
            label.push_str("»");
        }
        if !name.is_empty() {
            if !label.is_empty() {
                label.push_str("\n");
            }
            label.push_str(name);
        }
        Some(label.into())
    }
}

pub fn new_umlclass_dependency<P: UmlClassProfile>(
    stereotype: &str,
    name: &str,
    target_arrow_open: bool,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (UmlClassAssociable, UmlClassElementView<P>),
    target: (UmlClassAssociable, UmlClassElementView<P>),
) -> (ERef<UmlClassDependency>, ERef<DependencyViewT<P>>) {
    let link_model = ERef::new(UmlClassDependency::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        source.0,
        target.0,
        target_arrow_open,
    ));
    let link_view =
        new_umlclass_dependency_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlclass_dependency_view<P: UmlClassProfile>(
    model: ERef<UmlClassDependency>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView<P>,
    target: UmlClassElementView<P>,
) -> ERef<DependencyViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once(*m.source.uuid()),
        *m.target.uuid(),
        target.min_shape(),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlClassDependencyAdapter {
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
pub struct UmlClassDependencyAdapter<P: UmlClassProfile> {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDependency>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlClassDependencyTemporaries<P>,
}

#[derive(Clone, Default)]
struct UmlClassDependencyTemporaries<P: UmlClassProfile> {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    midpoint_label: Option<Arc<String>>,
    stereotype_controller: P::DependencyStereotypeController,
    name_buffer: String,
    target_arrow_open_buffer: bool,
    comment_buffer: String,
}

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>>
    for UmlClassDependencyAdapter<P>
{
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
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
        self.model.write().flip_multiconnection();
        Ok(())
    }

    fn show_properties(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if self.temporaries.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(
                    self.temporaries.stereotype_controller.get_arc(),
                ),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }
        ui.separator();

        ui.label("Target arrow open:");
        if ui
            .checkbox(&mut self.temporaries.target_arrow_open_buffer, "")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::DependencyArrowOpenChange(
                    self.temporaries.target_arrow_open_buffer,
                ),
            ));
        }
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.separator();

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(
                    self.temporaries.comment_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlClassPropChange::StereotypeChange(stereotype) => {
                    if !self.temporaries.stereotype_controller.is_valid(&stereotype) {
                        return;
                    }

                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                    ));
                    model.stereotype = stereotype.clone();
                }
                UmlClassPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlClassPropChange::DependencyArrowOpenChange(open) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::DependencyArrowOpenChange(model.target_arrow_open),
                    ));
                    model.target_arrow_open = *open;
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
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
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData::new_labelless(
                canvas::LineType::Dashed,
                if model.target_arrow_open {
                    canvas::ArrowheadType::OpenTriangle
                } else {
                    canvas::ArrowheadType::EmptyTriangle
                },
            ),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.midpoint_label = stereotype_name_format(&*model.stereotype, &*model.name);
        self.temporaries
            .stereotype_controller
            .refresh(&*model.stereotype);
        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.target_arrow_open_buffer = model.target_arrow_open;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::Dependency(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlClassElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.uuid();
        if let Some(new_source) = m.get(&source_uuid).and_then(|e| e.as_associable()) {
            model.source = new_source;
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_associable()) {
            model.target = new_target;
        }
    }
}

pub fn new_umlclass_association<P: UmlClassProfile>(
    stereotype: &str,
    name: &str,
    source_label_multiplicity: &str,
    target_label_multiplicity: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (UmlClassAssociable, UmlClassElementView<P>),
    target: (UmlClassAssociable, UmlClassElementView<P>),
) -> (ERef<UmlClassAssociation>, ERef<AssociationViewT<P>>) {
    let link_model = ERef::new(UmlClassAssociation::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        source.0,
        source_label_multiplicity.to_owned(),
        target.0,
        target_label_multiplicity.to_owned(),
    ));
    let link_view =
        new_umlclass_association_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlclass_association_view<P: UmlClassProfile>(
    model: ERef<UmlClassAssociation>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView<P>,
    target: UmlClassElementView<P>,
) -> ERef<AssociationViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once(*m.source.uuid()),
        *m.target.uuid(),
        target.min_shape(),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlClassAssocationAdapter {
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
pub struct UmlClassAssocationAdapter<P: UmlClassProfile> {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassAssociation>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlClassAssociationTemporaries<P>,
}

#[derive(Clone, Default)]
struct UmlClassAssociationTemporaries<P: UmlClassProfile> {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    midpoint_label: Option<Arc<String>>,
    stereotype_controller: P::AssociationStereotypeController,
    name_buffer: String,
    source_multiplicity_buffer: String,
    source_role_buffer: String,
    source_reading_buffer: String,
    source_navigability_buffer: UmlClassAssociationNavigability,
    source_aggregation_buffer: UmlClassAssociationAggregation,
    target_multiplicity_buffer: String,
    target_role_buffer: String,
    target_reading_buffer: String,
    target_navigability_buffer: UmlClassAssociationNavigability,
    target_aggregation_buffer: UmlClassAssociationAggregation,
    comment_buffer: String,
}

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>>
    for UmlClassAssocationAdapter<P>
{
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
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
        self.model.write().flip_multiconnection();
        Ok(())
    }

    fn show_properties(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if self.temporaries.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(
                    self.temporaries.stereotype_controller.get_arc(),
                ),
            ));
        }

        if ui
            .labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ));
        }
        ui.separator();

        if ui
            .labeled_text_edit_singleline(
                "Source multiplicity:",
                &mut self.temporaries.source_multiplicity_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::LinkMultiplicityChange(
                    false,
                    Arc::new(self.temporaries.source_multiplicity_buffer.clone()),
                ),
            ));
        }
        if ui
            .labeled_text_edit_singleline("Source role:", &mut self.temporaries.source_role_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::LinkRoleChange(
                    false,
                    Arc::new(self.temporaries.source_role_buffer.clone()),
                ),
            ));
        }
        if ui
            .labeled_text_edit_singleline(
                "Source reading:",
                &mut self.temporaries.source_reading_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::LinkReadingChange(
                    false,
                    Arc::new(self.temporaries.source_reading_buffer.clone()),
                ),
            ));
        }
        ui.label("Source navigability:");
        egui::ComboBox::from_id_salt("source navigability")
            .selected_text(&*self.temporaries.source_navigability_buffer.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassAssociationNavigability::Unspecified,
                    UmlClassAssociationNavigability::NonNavigable,
                    UmlClassAssociationNavigability::Navigable,
                ] {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.source_navigability_buffer,
                            sv,
                            &*sv.name(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::LinkNavigabilityChange(
                                false,
                                self.temporaries.source_navigability_buffer,
                            ),
                        ));
                    }
                }
            });
        ui.label("Source aggregation:");
        egui::ComboBox::from_id_salt("source aggregation")
            .selected_text(&*self.temporaries.source_aggregation_buffer.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassAssociationAggregation::None,
                    UmlClassAssociationAggregation::Shared,
                    UmlClassAssociationAggregation::Composite,
                ] {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.source_aggregation_buffer,
                            sv,
                            &*sv.name(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::LinkAggregationChange(
                                false,
                                self.temporaries.source_aggregation_buffer,
                            ),
                        ));
                    }
                }
            });
        ui.separator();

        if ui
            .labeled_text_edit_singleline(
                "Target multiplicity:",
                &mut self.temporaries.target_multiplicity_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::LinkMultiplicityChange(
                    true,
                    Arc::new(self.temporaries.target_multiplicity_buffer.clone()),
                ),
            ));
        }
        if ui
            .labeled_text_edit_singleline("Target role:", &mut self.temporaries.target_role_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::LinkRoleChange(
                    true,
                    Arc::new(self.temporaries.target_role_buffer.clone()),
                ),
            ));
        }
        if ui
            .labeled_text_edit_singleline(
                "Target reading:",
                &mut self.temporaries.target_reading_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::LinkReadingChange(
                    true,
                    Arc::new(self.temporaries.target_reading_buffer.clone()),
                ),
            ));
        }
        ui.label("Target navigability:");
        egui::ComboBox::from_id_salt("target navigability")
            .selected_text(&*self.temporaries.target_navigability_buffer.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassAssociationNavigability::Unspecified,
                    UmlClassAssociationNavigability::NonNavigable,
                    UmlClassAssociationNavigability::Navigable,
                ] {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.target_navigability_buffer,
                            sv,
                            &*sv.name(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::LinkNavigabilityChange(
                                true,
                                self.temporaries.target_navigability_buffer,
                            ),
                        ));
                    }
                }
            });
        ui.label("Target aggregation:");
        egui::ComboBox::from_id_salt("target aggregation")
            .selected_text(&*self.temporaries.target_aggregation_buffer.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassAssociationAggregation::None,
                    UmlClassAssociationAggregation::Shared,
                    UmlClassAssociationAggregation::Composite,
                ] {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.target_aggregation_buffer,
                            sv,
                            &*sv.name(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            UmlClassPropChange::LinkAggregationChange(
                                true,
                                self.temporaries.target_aggregation_buffer,
                            ),
                        ));
                    }
                }
            });
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }
        ui.separator();

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(
                    self.temporaries.comment_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlClassPropChange::StereotypeChange(stereotype) => {
                    if !self.temporaries.stereotype_controller.is_valid(&stereotype) {
                        return;
                    }

                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                    ));
                    model.stereotype = stereotype.clone();
                }
                UmlClassPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                UmlClassPropChange::LinkMultiplicityChange(t, multiplicity) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::LinkMultiplicityChange(
                            *t,
                            if !t {
                                model.source_label_multiplicity.clone()
                            } else {
                                model.target_label_multiplicity.clone()
                            },
                        ),
                    ));
                    if !t {
                        model.source_label_multiplicity = multiplicity.clone();
                    } else {
                        model.target_label_multiplicity = multiplicity.clone();
                    }
                }
                UmlClassPropChange::LinkRoleChange(t, role) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::LinkRoleChange(
                            *t,
                            if !t {
                                model.source_label_role.clone()
                            } else {
                                model.target_label_role.clone()
                            },
                        ),
                    ));
                    if !t {
                        model.source_label_role = role.clone();
                    } else {
                        model.target_label_role = role.clone();
                    }
                }
                UmlClassPropChange::LinkReadingChange(t, reading) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::LinkRoleChange(
                            *t,
                            if !t {
                                model.source_label_reading.clone()
                            } else {
                                model.target_label_reading.clone()
                            },
                        ),
                    ));
                    if !t {
                        model.source_label_reading = reading.clone();
                    } else {
                        model.target_label_reading = reading.clone();
                    }
                }
                UmlClassPropChange::LinkNavigabilityChange(t, navigability) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::LinkNavigabilityChange(
                            *t,
                            if !*t {
                                model.source_navigability
                            } else {
                                model.target_navigability
                            },
                        ),
                    ));
                    if !*t {
                        model.source_navigability = *navigability;
                    } else {
                        model.target_navigability = *navigability;
                    }
                }
                UmlClassPropChange::LinkAggregationChange(t, aggregation) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::LinkAggregationChange(
                            *t,
                            if !*t {
                                model.source_aggregation
                            } else {
                                model.target_aggregation
                            },
                        ),
                    ));
                    if !*t {
                        model.source_aggregation = *aggregation;
                    } else {
                        model.target_aggregation = *aggregation;
                    }
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        fn ah(
            n: UmlClassAssociationNavigability,
            a: UmlClassAssociationAggregation,
        ) -> canvas::ArrowheadType {
            match a {
                UmlClassAssociationAggregation::None => match n {
                    UmlClassAssociationNavigability::Unspecified => canvas::ArrowheadType::None,
                    UmlClassAssociationNavigability::NonNavigable => canvas::ArrowheadType::None,
                    UmlClassAssociationNavigability::Navigable => {
                        canvas::ArrowheadType::OpenTriangle
                    }
                },
                UmlClassAssociationAggregation::Shared => canvas::ArrowheadType::EmptyRhombus,
                UmlClassAssociationAggregation::Composite => canvas::ArrowheadType::FullRhombus,
            }
        }
        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.uuid()),
            ArrowData {
                line_type: canvas::LineType::Solid,
                arrowhead_type: ah(model.source_navigability, model.source_aggregation),
                multiplicity: if !model.source_label_multiplicity.is_empty() {
                    Some(model.source_label_multiplicity.clone())
                } else {
                    None
                },
                role: if !model.source_label_role.is_empty() {
                    Some(model.source_label_role.clone())
                } else {
                    None
                },
                reading: if !model.source_label_reading.is_empty() {
                    Some(model.source_label_reading.clone())
                } else {
                    None
                },
            },
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData {
                line_type: canvas::LineType::Solid,
                arrowhead_type: ah(model.target_navigability, model.target_aggregation),
                multiplicity: if !model.target_label_multiplicity.is_empty() {
                    Some(model.target_label_multiplicity.clone())
                } else {
                    None
                },
                role: if !model.target_label_role.is_empty() {
                    Some(model.target_label_role.clone())
                } else {
                    None
                },
                reading: if !model.target_label_reading.is_empty() {
                    Some(model.target_label_reading.clone())
                } else {
                    None
                },
            },
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.midpoint_label = stereotype_name_format(&*model.stereotype, &*model.name);
        self.temporaries
            .stereotype_controller
            .refresh(&*model.stereotype);
        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.source_multiplicity_buffer = (*model.source_label_multiplicity).clone();
        self.temporaries.source_role_buffer = (*model.source_label_role).clone();
        self.temporaries.source_reading_buffer = (*model.source_label_reading).clone();
        self.temporaries.source_navigability_buffer = model.source_navigability;
        self.temporaries.source_aggregation_buffer = model.source_aggregation;
        self.temporaries.target_multiplicity_buffer = (*model.target_label_multiplicity).clone();
        self.temporaries.target_role_buffer = (*model.target_label_role).clone();
        self.temporaries.target_reading_buffer = (*model.target_label_reading).clone();
        self.temporaries.target_navigability_buffer = model.target_navigability;
        self.temporaries.target_aggregation_buffer = model.target_aggregation;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::Association(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlClassElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.uuid();
        if let Some(new_source) = m.get(&source_uuid).and_then(|e| e.as_associable()) {
            model.source = new_source;
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_associable()) {
            model.target = new_target;
        }
    }
}

pub fn new_uml_usecasegeneralization<P: UmlClassProfile>(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlUseCase>, UmlClassElementView<P>),
    target: (ERef<UmlUseCase>, UmlClassElementView<P>),
) -> (
    ERef<UmlUseCaseGeneralization>,
    ERef<UseCaseGeneralizationViewT<P>>,
) {
    let link_model = ERef::new(UmlUseCaseGeneralization::new(
        ModelUuid::now_v7(),
        vec![source.0],
        vec![target.0],
    ));
    let link_view = new_uml_usecasegeneralization_view(
        link_model.clone(),
        center_point,
        vec![source.1],
        vec![target.1],
    );
    (link_model, link_view)
}
pub fn new_uml_usecasegeneralization_view<P: UmlClassProfile>(
    model: ERef<UmlUseCaseGeneralization>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    sources: Vec<UmlClassElementView<P>>,
    targets: Vec<UmlClassElementView<P>>,
) -> ERef<UseCaseGeneralizationViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        m.sources.iter().map(|e| *e.read().uuid),
        *m.targets[0].read().uuid,
        targets[0].min_shape(),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlUseCaseGeneralizationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        sources
            .into_iter()
            .zip(sp.into_iter())
            .map(|e| Ending::new_p(e.0, e.1))
            .collect(),
        targets
            .into_iter()
            .zip(tp.into_iter())
            .map(|e| Ending::new_p(e.0, e.1))
            .collect(),
        mp,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct UmlUseCaseGeneralizationAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlUseCaseGeneralization>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlUseCaseGeneralizationTemporaries,
}

#[derive(Clone, Default)]
struct UmlUseCaseGeneralizationTemporaries {
    midpoint_label: Option<Arc<String>>,
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    set_name_buffer: String,
    set_is_covering_buffer: bool,
    set_is_disjoint_buffer: bool,
    comment_buffer: String,
}

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>>
    for UmlUseCaseGeneralizationAdapter
{
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
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
        self.model.write().flip_multiconnection();
        Ok(())
    }
    fn insert_source(
        &mut self,
        position: Option<PositionNoT>,
        e: <UmlClassDomain<P> as Domain>::CommonElementT,
    ) -> Result<PositionNoT, ()> {
        self.model
            .write()
            .insert_element(MULTICONNECTION_SOURCE_BUCKET, position, e)
            .map_err(|_| ())
    }
    fn remove_source(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        let mut w = self.model.write();
        if w.sources.len() == 1 {
            return None;
        }
        for (idx, e) in w.sources.iter().enumerate() {
            if *e.read().uuid == *uuid {
                w.sources.remove(idx);
                return Some(idx.try_into().unwrap());
            }
        }
        None
    }
    fn insert_target(
        &mut self,
        position: Option<PositionNoT>,
        e: <UmlClassDomain<P> as Domain>::CommonElementT,
    ) -> Result<PositionNoT, ()> {
        self.model
            .write()
            .insert_element(MULTICONNECTION_TARGET_BUCKET, position, e)
            .map_err(|_| ())
    }
    fn remove_target(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        let mut w = self.model.write();
        if w.targets.len() == 1 {
            return None;
        }
        for (idx, e) in w.targets.iter().enumerate() {
            if *e.read().uuid == *uuid {
                w.targets.remove(idx);
                return Some(idx.try_into().unwrap());
            }
        }
        None
    }

    fn show_properties(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if ui
            .add_enabled(
                self.model.read().targets.len() <= 1,
                egui::Button::new("Add source"),
            )
            .clicked()
        {
            return PropertiesStatus::ToolRequest(Some(NaiveUmlClassTool {
                uuid: uuid::Uuid::nil(),
                initial_stage: UmlClassToolStage::LinkAddEnding { source: true },
                current_stage: UmlClassToolStage::LinkAddEnding { source: true },
                result: PartialUmlClassElement::LinkEnding {
                    source: true,
                    gen_model: self.model.clone().into(),
                    new_model: None,
                },
                event_lock: false,
                is_spent: Some(false),
            }));
        }
        if ui
            .add_enabled(
                self.model.read().sources.len() <= 1,
                egui::Button::new("Add target"),
            )
            .clicked()
        {
            return PropertiesStatus::ToolRequest(Some(NaiveUmlClassTool {
                uuid: uuid::Uuid::nil(),
                initial_stage: UmlClassToolStage::LinkAddEnding { source: false },
                current_stage: UmlClassToolStage::LinkAddEnding { source: false },
                result: PartialUmlClassElement::LinkEnding {
                    source: false,
                    gen_model: self.model.clone().into(),
                    new_model: None,
                },
                event_lock: false,
                is_spent: Some(false),
            }));
        }

        if ui
            .add_enabled(true, egui::Button::new("Switch source and target"))
            .clicked()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }

        ui.label("Generalization set name:");
        if ui
            .text_edit_singleline(&mut self.temporaries.set_name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::SetNameChange(Arc::new(
                    self.temporaries.set_name_buffer.clone(),
                )),
            ));
        }
        if ui
            .checkbox(&mut self.temporaries.set_is_covering_buffer, "isCovering")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::SetCoveringChange(self.temporaries.set_is_covering_buffer),
            ));
        }
        if ui
            .checkbox(&mut self.temporaries.set_is_disjoint_buffer, "isDisjoint")
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::SetDisjointChange(self.temporaries.set_is_disjoint_buffer),
            ));
        }
        ui.separator();

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::CommentChange(Arc::new(
                    self.temporaries.comment_buffer.clone(),
                )),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                UmlClassPropChange::SetNameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::SetNameChange(model.set_name.clone()),
                    ));
                    model.set_name = name.clone();
                }
                UmlClassPropChange::SetCoveringChange(is_covering) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::SetCoveringChange(model.set_is_covering.clone()),
                    ));
                    model.set_is_covering = is_covering.clone();
                }
                UmlClassPropChange::SetDisjointChange(is_disjoint) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::SetDisjointChange(model.set_is_disjoint.clone()),
                    ));
                    model.set_is_disjoint = is_disjoint.clone();
                }
                UmlClassPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        UmlClassPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        let set_props_label = if model.sources.len() > 1 || model.targets.len() > 1 {
            Some(format!(
                "{{{}, {}}}",
                if model.set_is_covering {
                    "complete"
                } else {
                    "incomplete"
                },
                if model.set_is_disjoint {
                    "disjoint"
                } else {
                    "overlapping"
                },
            ))
        } else {
            None
        };
        self.temporaries.midpoint_label = if let Some(spl) = set_props_label {
            Some(Arc::new(format!("{}\n{}", model.set_name, spl)))
        } else if !model.set_name.is_empty() {
            Some(model.set_name.clone())
        } else {
            None
        };

        self.temporaries.arrow_data.clear();
        for e in &model.sources {
            self.temporaries.arrow_data.insert(
                (false, *e.read().uuid),
                ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::None),
            );
        }
        for e in &model.targets {
            self.temporaries.arrow_data.insert(
                (true, *e.read().uuid),
                ArrowData::new_labelless(
                    canvas::LineType::Solid,
                    canvas::ArrowheadType::EmptyTriangle,
                ),
            );
        }

        self.temporaries.source_uuids.clear();
        for e in &model.sources {
            self.temporaries.source_uuids.push(*e.read().uuid);
        }
        self.temporaries.target_uuids.clear();
        for e in &model.targets {
            self.temporaries.target_uuids.push(*e.read().uuid);
        }

        self.temporaries.set_name_buffer = (*model.set_name).clone();
        self.temporaries.set_is_covering_buffer = model.set_is_covering;
        self.temporaries.set_is_disjoint_buffer = model.set_is_disjoint;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UseCaseGeneralization(m)) = m.get(&old_model.uuid)
        {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlClassElement>) {
        let mut model = self.model.write();

        for e in model.sources.iter_mut() {
            let sid = *e.read().uuid;
            if let Some(UmlClassElement::UseCase(new_source)) = m.get(&sid) {
                *e = new_source.clone();
            }
        }
        for e in model.targets.iter_mut() {
            let tid = *e.read().uuid;
            if let Some(UmlClassElement::UseCase(new_target)) = m.get(&tid) {
                *e = new_target.clone();
            }
        }
    }
}

pub fn new_umlclass_comment<P: UmlClassProfile>(
    text: &str,
    stereotype: &str,
    position: egui::Pos2,
    align: egui::Align2,
) -> (ERef<UmlClassComment>, ERef<UmlClassCommentView<P>>) {
    let comment_model = ERef::new(UmlClassComment::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        text.to_owned(),
    ));
    let comment_view = new_umlclass_comment_view(comment_model.clone(), position, align);

    (comment_model, comment_view)
}
pub fn new_umlclass_comment_view<P: UmlClassProfile>(
    model: ERef<UmlClassComment>,
    position: egui::Pos2,
    align: egui::Align2,
) -> ERef<UmlClassCommentView<P>> {
    let m = model.read();
    ERef::new(UmlClassCommentView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        display_text: String::new(),
        stereotype_controller: Default::default(),
        text_buffer: (*m.text).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        align,
        bounds_rect: egui::Rect::from_min_max(position, position),
        background_color: MGlobalColor::None,
        _profile: PhantomData,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassCommentView<P: UmlClassProfile> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClassComment>,

    #[nh_context_serde(skip_and_default)]
    display_text: String,
    #[nh_context_serde(skip_and_default)]
    stereotype_controller: P::CommentStereotypeController,
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

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

impl<P: UmlClassProfile> UmlClassCommentView<P> {
    const CORNER_SIZE: f32 = 10.0;

    fn comment_link_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_radius = 8.0;
        let b_center = self.bounds_rect.right_top() + egui::Vec2::splat(b_radius / ui_scale);
        egui::Rect::from_center_size(b_center, egui::Vec2::splat(2.0 * b_radius / ui_scale))
    }
}

impl<P: UmlClassProfile> Entity for UmlClassCommentView<P> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<P: UmlClassProfile> View for UmlClassCommentView<P> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl<P: UmlClassProfile> ElementController<UmlClassElement> for UmlClassCommentView<P> {
    fn model(&self) -> UmlClassElement {
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

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassCommentView<P> {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if self.stereotype_controller.show(ui) {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get_arc()),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Text:", &mut self.text_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                UmlClassPropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
                            UmlClassPropChange::CommentAlignChange(Some(tmp_x), None),
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
                            UmlClassPropChange::CommentAlignChange(None, Some(tmp_y)),
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
                UmlClassPropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
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
                "\\", // Does not work: ┄𜸍┊┄⤍⇢┈┇┋╏╲
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
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveUmlClassTool<P>>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                *tool = Some(NaiveUmlClassTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage: UmlClassToolStage::CommentLinkStart,
                    current_stage: UmlClassToolStage::CommentLinkEnd,
                    result: PartialUmlClassElement::CommentLink {
                        source: self.model.clone(),
                        dest: None,
                    },
                    event_lock: true,
                    is_spent: Some(false),
                });

                return EventHandlingStatus::HandledByElement;
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
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
                        UmlClassPropChange::StereotypeChange(stereotype) => {
                            if !self.stereotype_controller.is_valid(&stereotype) {
                                return;
                            }

                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        UmlClassPropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        UmlClassPropChange::CommentAlignChange(x, y) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                UmlClassPropChange::CommentAlignChange(
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
            let mut s = String::new();
            if !model.stereotype.is_empty() {
                s.push_str("«");
                s.push_str(&model.stereotype);
                s.push_str("»\n");
            }
            s.push_str(&model.text);
            s
        };
        self.stereotype_controller.refresh(&model.stereotype);
        self.text_buffer = (*model.text).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (UmlClassElementView<P>, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        c: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::Comment(m)) = m.get(&old_model.uuid) {
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
            stereotype_controller: self.stereotype_controller.clone(),
            text_buffer: self.text_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            align: self.align,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

pub fn new_umlclass_commentlink<P: UmlClassProfile>(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClassComment>, UmlClassElementView<P>),
    target: (UmlClassElement, UmlClassElementView<P>),
) -> (ERef<UmlClassCommentLink>, ERef<CommentLinkViewT<P>>) {
    let link_model = ERef::new(UmlClassCommentLink::new(
        ModelUuid::now_v7(),
        source.0,
        target.0,
    ));
    let link_view =
        new_umlclass_commentlink_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlclass_commentlink_view<P: UmlClassProfile>(
    model: ERef<UmlClassCommentLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView<P>,
    target: UmlClassElementView<P>,
) -> ERef<CommentLinkViewT<P>> {
    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlClassCommentLinkAdapter {
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
pub struct UmlClassCommentLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassCommentLink>,
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

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>> for UmlClassCommentLinkAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<UmlClassDomain<P> as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
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
        _q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
            >,
        >,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        PropertiesStatus::NotShown
    }
    fn apply_change(
        &self,
        _view_uuid: &ViewUuid,
        _command: &InsensitiveCommand<
            UmlClassOrdinalMovement,
            UmlClassElementOrVertex<P>,
            UmlClassPropChange,
        >,
        _undo_accumulator: &mut Vec<
            InsensitiveCommand<
                UmlClassOrdinalMovement,
                UmlClassElementOrVertex<P>,
                UmlClassPropChange,
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
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::CommentLink(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, UmlClassElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.read().uuid();
        if let Some(UmlClassElement::Comment(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}
