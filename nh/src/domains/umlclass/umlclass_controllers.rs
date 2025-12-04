use super::umlclass_models::{
    UmlClass, UmlClassDiagram, UmlClassElement, UmlClassGeneralization, UmlClassPackage, UmlClassCommentLink, UmlClassAssociation, UmlClassClassifier, UmlClassComment, UmlClassInstance, UmlClassAssociationAggregation, UmlClassAssociationNavigability, UmlClassDependency,
};
use crate::common::canvas::{self, Highlight, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, CachingLabelDeriver, ColorBundle, ColorChangeData, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider, MGlobalColor, Model, PositionNoT, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, View
};
use crate::common::ufoption::UFOption;
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{self, ArrowData, Ending, FlipMulticonnection, MULTICONNECTION_SOURCE_BUCKET, MULTICONNECTION_TARGET_BUCKET, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::domains::umlclass::umlclass_models::{UmlClassVisibilityKind, UmlClassOperation, UmlClassProperty};
use crate::{CustomModal, CustomModalResult, CustomTab};
use eframe::egui;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub trait UmlClassPalette<P: UmlClassProfile>: Default + Clone {
    fn iter_mut(&mut self) -> impl Iterator<
        Item = (&str, &mut Vec<(UmlClassToolStage, &'static str, UmlClassElementView<P>)>)
    >;
}

pub trait StereotypeController: Default + Clone + Send + Sync + 'static {
    fn show(&mut self, ui: &mut egui::Ui) -> bool;
    fn get(&mut self) -> Arc<String>;
    fn is_valid(&self, value: &str) -> bool {
        true
    }
    fn refresh(&mut self, new_value: &str);
}

pub trait UmlClassProfile: Default + Clone + Send + Sync + 'static {
    type Palette: UmlClassPalette<Self>;
    type InstanceStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type ClassStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type ClassPropertyStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type ClassOperationStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type DependencyStereotypeController: StereotypeController = UnrestrictedStereotypeController;
    type AssociationStereotypeController: StereotypeController = UnrestrictedStereotypeController;

    fn view_type() -> &'static str;
    fn menubar_options_fun(
        model: &ERef<UmlClassDiagram>,
        view_uuid: &ViewUuid,
        label_provider: &ERef<dyn LabelProvider>,
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

    fn allows_class_attributes() -> bool {
        true
    }
    fn allows_class_operations() -> bool {
        true
    }
}

#[derive(Clone, Default)]
pub struct UnrestrictedStereotypeController {
    buffer: String,
}
impl StereotypeController for UnrestrictedStereotypeController {
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.buffer),
        ).changed()
    }
    fn get(&mut self) -> Arc<String> {
        self.buffer.clone().into()
    }
    fn refresh(&mut self, new_value: &str) {
        self.buffer.replace_range(.., new_value);
    }
}

#[derive(Clone, Default)]
pub struct UmlClassNullProfile;
impl UmlClassProfile for UmlClassNullProfile {
    type Palette = UmlClassNullPalette;

    fn view_type() -> &'static str {
        "umlclass-diagram-view"
    }
}

pub struct UmlClassDomain<P: UmlClassProfile> {
    _profile: PhantomData<P>,
}
impl<P: UmlClassProfile> Domain for UmlClassDomain<P> {
    type CommonElementT = UmlClassElement;
    type DiagramModelT = UmlClassDiagram;
    type CommonElementViewT = UmlClassElementView<P>;
    type ViewTargettingSectionT = UmlClassElement;
    type QueryableT<'a> = UmlClassQueryable<'a, P>;
    type LabelProviderT = UmlClassLabelProvider;
    type ToolT = NaiveUmlClassTool<P>;
    type AddCommandElementT = UmlClassElementOrVertex<P>;
    type PropChangeT = UmlClassPropChange;
}

type PackageViewT<P> = PackageView<UmlClassDomain<P>, UmlClassPackageAdapter<P>>;
type GeneralizationViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassGeneralizationAdapter>;
type DependencyViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassDependencyAdapter<P>>;
type AssociationViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassAssocationAdapter<P>>;
type CommentLinkViewT<P> = MulticonnectionView<UmlClassDomain<P>, UmlClassCommentLinkAdapter>;

pub struct UmlClassQueryable<'a, P: UmlClassProfile> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, UmlClassElementView<P>>,
}

impl<'a, P: UmlClassProfile> Queryable<'a, UmlClassDomain<P>> for UmlClassQueryable<'a, P> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, UmlClassElementView<P>>,
    ) -> Self {
        Self { models_to_views, flattened_views }
    }

    fn get_view(&self, m: &ModelUuid) -> Option<UmlClassElementView<P>> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).cloned()
    }
}

#[derive(Default)]
pub struct UmlClassLabelProvider {
    cache: HashMap<ModelUuid, Arc<String>>,
}

impl LabelProvider for UmlClassLabelProvider {
    fn get(&self, uuid: &ModelUuid) -> Arc<String> {
        self.cache.get(uuid).cloned()
            .unwrap_or_else(|| Arc::new(format!("{:?}", uuid)))
    }
}

impl CachingLabelDeriver<UmlClassElement> for UmlClassLabelProvider {
    fn update(&mut self, e: &UmlClassElement) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let r = inner.read();
                self.cache.insert(*r.uuid, r.name.clone());
            },
            UmlClassElement::UmlClassInstance(inner) => {
                let r = inner.read();
                let s = if r.instance_name.is_empty() {
                    format!(":{}", r.instance_type)
                } else {
                    format!("{}: {}", r.instance_name, r.instance_type)
                };
                self.cache.insert(*r.uuid, Arc::new(s));
            },
            UmlClassElement::UmlClass(inner) => {
                let r = inner.read();
                let s = if r.stereotype.is_empty() {
                    r.name.clone()
                } else {
                    Arc::new(format!("{} «{}»", r.name, r.stereotype))
                };
                self.cache.insert(*r.uuid, s);
            },
            UmlClassElement::UmlClassProperty(inner) => {
                let r = inner.read();
                self.cache.insert(*r.uuid, r.name.clone());
            }
            UmlClassElement::UmlClassOperation(inner) => {
                let r = inner.read();
                self.cache.insert(*r.uuid, r.name.clone());
            }
            UmlClassElement::UmlClassGeneralization(inner) => {
                let r = inner.read();
                let s = if r.set_name.is_empty() {
                    "Generalization".to_owned()
                } else {
                    format!("Generalization ({})", r.set_name)
                };
                self.cache.insert(*r.uuid, Arc::new(s));
            },
            UmlClassElement::UmlClassDependency(inner) => {
                let r = inner.read();
                let s = if r.stereotype.is_empty() {
                    "Dependency".to_owned()
                } else {
                    format!("Dependency ({})", r.stereotype)
                };
                self.cache.insert(*r.uuid, Arc::new(s));
            }
            UmlClassElement::UmlClassAssociation(inner) => {
                let r = inner.read();
                let s = if r.stereotype.is_empty() {
                    "Association".to_owned()
                } else {
                    format!("Association «{}»", r.stereotype)
                };
                self.cache.insert(*r.uuid, Arc::new(s));
            },
            UmlClassElement::UmlClassComment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", Self::filter_and_elipsis(&r.text))
                };
                self.cache.insert(*r.uuid, Arc::new(s));
            },
            UmlClassElement::UmlClassCommentLink(inner) => {
                let r = inner.read();
                self.cache.insert(*r.uuid, Arc::new(format!("Comment Link")));
            },
        }
    }

    fn insert(&mut self, k: ModelUuid, v: Arc<String>) {
        self.cache.insert(k, v);
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

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
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
    Generalization(ERef<GeneralizationViewT<P>>),
    Dependency(ERef<DependencyViewT<P>>),
    Association(ERef<AssociationViewT<P>>),
    Comment(ERef<UmlClassCommentView<P>>),
    CommentLink(ERef<CommentLinkViewT<P>>),
}


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassDiagramAdapter<P: UmlClassProfile> {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: UmlClassDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    placeholders: P::Palette,
}

#[derive(Clone, Default)]
struct UmlClassDiagramBuffer {
    name: String,
    comment: String,
}

#[derive(Clone)]
pub struct UmlClassNullPalette {
    views: [(&'static str, Vec<(UmlClassToolStage, &'static str, UmlClassElementView<UmlClassNullProfile>)>); 3],
}

impl Default for UmlClassNullPalette {
    fn default() -> Self {
        let (_instance, instance_view) = new_umlclass_instance("o", "Type", "", "", egui::Pos2::ZERO);
        instance_view.write().refresh_buffers();
        let (class_m, class_view) = new_umlclass_class("ClassName", "class", false, Vec::new(), Vec::new(), egui::Pos2::ZERO);
        class_view.write().refresh_buffers();
        let class_1 = (class_m.clone(), class_view.clone().into());
        let class_2 = (class_m.clone().into(), class_view.into());
        let (d, dv) = new_umlclass_class("dummy", "class", false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 75.0));
        let dummy_1 = (d.clone(), dv.clone().into());
        let dummy_2 = (d.clone().into(), dv.into());

        let (_property, property_view) = new_umlclass_property(UFOption::None, "property", "Type", "", "", "");
        property_view.write().refresh_buffers();
        let (_operation, operation_view) = new_umlclass_operation(UFOption::None, "operation", "", "ReturnType", "");
        operation_view.write().refresh_buffers();

        let (_gen, gen_view) = new_umlclass_generalization(None, class_1, dummy_1);
        let (assoc, assoc_view) = new_umlclass_association("", "", None, class_2.clone(), dummy_2.clone());
        assoc.write().source_label_multiplicity = Arc::new("".to_owned());
        assoc.write().target_label_multiplicity = Arc::new("".to_owned());
        assoc_view.write().refresh_buffers();
        let (_intreal, intreal_view) = new_umlclass_dependency("", "", false, None, class_2.clone(), dummy_2.clone());
        let (_usage, usage_view) = new_umlclass_dependency("use", "", true, None, class_2.clone(), dummy_2.clone());

        let (_package, package_view) = new_umlclass_package("a package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });
        let (comment, comment_view) = new_umlclass_comment("a comment", egui::Pos2::new(-100.0, -75.0));
        let comment = (comment, comment_view.into());
        let commentlink = new_umlclass_commentlink(None, comment.clone(), (class_m.into(), class_2.1.clone()));

        Self {
            views: [
                ("Elements", vec![
                    (UmlClassToolStage::Instance, "Instance", instance_view.into()),
                    (UmlClassToolStage::Class { name: "ClassName", stereotype: "class" }, "Class", class_2.1),
                    (UmlClassToolStage::ClassProperty, "Property", property_view.into()),
                    (UmlClassToolStage::ClassOperation, "Operation", operation_view.into()),
                ]),
                ("Relationships", vec![
                    (UmlClassToolStage::LinkStart { link_type: LinkType::Generalization }, "Generalization (Set)", gen_view.into()),
                    (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "" } }, "Association", assoc_view.into()),
                    (UmlClassToolStage::LinkStart { link_type: LinkType::Dependency { target_arrow_open: false, stereotype: "", } }, "IntReal", intreal_view.into()),
                    (UmlClassToolStage::LinkStart { link_type: LinkType::Dependency { target_arrow_open: true, stereotype: "use", } }, "Usage", usage_view.into()),
                ]),
                ("Other", vec![
                    (UmlClassToolStage::PackageStart, "Package", package_view.into()),
                    (UmlClassToolStage::Comment, "Comment", comment.1),
                    (UmlClassToolStage::CommentLinkStart, "Comment Link", commentlink.1.into()),
                ]),
            ]
        }
    }
}

impl UmlClassPalette<UmlClassNullProfile> for UmlClassNullPalette {
    fn iter_mut(&mut self) -> impl Iterator<Item = (&str, &mut Vec<(UmlClassToolStage, &'static str, UmlClassElementView<UmlClassNullProfile>)>)> {
        self.views.iter_mut().map(|e| (e.0, &mut e.1))
    }
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
            placeholders: Default::default(),
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
    fn view_type(&self) -> &'static str {
        P::view_type()
    }

    fn create_new_view_for(
        &self,
        q: &UmlClassQueryable<'_, P>,
        element: UmlClassElement,
    ) -> Result<UmlClassElementView<P>, HashSet<ModelUuid>> {
        let v = match element {
            UmlClassElement::UmlClassPackage(inner) => {
                UmlClassElementView::from(
                    new_umlclass_package_view(
                        inner,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            },
            UmlClassElement::UmlClassInstance(inner) => {
                UmlClassElementView::from(
                    new_umlclass_instance_view(inner, egui::Pos2::ZERO)
                )
            }
            UmlClassElement::UmlClass(inner) => {
                let (properties_views, operations_views) = {
                    let r = inner.read();
                    (
                        r.properties.iter().map(|e| new_umlclass_property_view(e.clone())).collect(),
                        r.operations.iter().map(|e| new_umlclass_operation_view(e.clone())).collect(),
                    )
                };

                UmlClassElementView::from(
                    new_umlclass_class_view(inner, properties_views, operations_views, egui::Pos2::ZERO)
                )
            },
            UmlClassElement::UmlClassProperty(..)
            | UmlClassElement::UmlClassOperation(..) => {
                unreachable!()
            }
            UmlClassElement::UmlClassGeneralization(inner) => {
                let m = inner.read();
                let (Some(sv), Some(tv)) = (m.sources.iter().map(|e| q.get_view(&e.read().uuid)).collect(),
                                            m.targets.iter().map(|e| q.get_view(&e.read().uuid)).collect()) else {
                    return Err(m.sources.iter().map(|e| *e.read().uuid)
                        .chain(m.targets.iter().map(|e| *e.read().uuid)).collect())
                };
                UmlClassElementView::from(
                    new_umlclass_generalization_view(inner.clone(), None, sv, tv)
                )
            },
            UmlClassElement::UmlClassDependency(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(
                    new_umlclass_dependency_view(inner.clone(), None, source_view, target_view)
                )
            }
            UmlClassElement::UmlClassAssociation(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(
                    new_umlclass_association_view(inner.clone(), None, source_view, target_view)
                )
            },
            UmlClassElement::UmlClassComment(inner) => {
                UmlClassElementView::from(
                    new_umlclass_comment_view(inner, egui::Pos2::ZERO)
                )
            },
            UmlClassElement::UmlClassCommentLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(
                    new_umlclass_commentlink_view(inner.clone(), None, source_view, target_view)
                )
            },
        };

        Ok(v)
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
    ) -> PropertiesStatus<UmlClassDomain<P>> {
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
    fn show_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
                    vec![UmlClassPropChange::NameChange(Arc::new(
                        self.buffer.name.clone(),
                    ))],
                )
                .into(),
            );
        }

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
                    vec![UmlClassPropChange::CommentChange(Arc::new(
                        self.buffer.comment.clone(),
                    ))],
                )
                .into(),
            );
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    UmlClassPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                        ));
                        self.background_color = *color;
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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
        tool: &mut Option<NaiveUmlClassTool<P>>,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) {
        let button_height = drawing_context.tool_palette_item_height as f32;
        let width = ui.available_width();
        let selected_background_color = if ui.style().visuals.dark_mode {
            egui::Color32::BLUE
        } else {
            egui::Color32::LIGHT_BLUE
        };
        let button_background_color = ui.style().visuals.extreme_bg_color;

        let stage = tool.as_ref().map(|e| e.initial_stage());
        let c = |s: UmlClassToolStage| -> egui::Color32 {
            if stage.is_some_and(|e| e == s) {
                selected_background_color
            } else {
                button_background_color
            }
        };

        if ui
            .add_sized(
                [width, button_height],
                egui::Button::new("Select/Move").fill(if stage == None {
                    selected_background_color
                } else {
                    button_background_color
                }),
            )
            .clicked()
        {
            *tool = None;
        }
        ui.separator();

        let (empty_a, empty_b) = (HashMap::new(), HashMap::new());
        let empty_q = UmlClassQueryable::new(&empty_a, &empty_b);
        for (label, items) in self.placeholders.iter_mut() {
            egui::CollapsingHeader::new(label)
                .default_open(true)
                .show(ui, |ui| {
                    let width = ui.available_width();
                    for (stage, name, view) in items.iter_mut() {
                        let response = ui.add_sized([width, button_height], egui::Button::new(*name).fill(c(*stage)));
                        if response.clicked() {
                            if let Some(t) = &tool && t.initial_stage == *stage {
                                *tool = None;
                            } else {
                                *tool = Some(NaiveUmlClassTool::new(*stage));
                            }
                        }

                        let icon_rect = egui::Rect::from_min_size(response.rect.min, egui::Vec2::splat(button_height));
                        let painter = ui.painter().with_clip_rect(icon_rect);
                        let mut mc = canvas::MeasuringCanvas::new(&painter);
                        view.draw_in(&empty_q, drawing_context, &mut mc, &None);
                        let (scale, offset) = mc.scale_offset_to_fit(egui::Vec2::new(button_height, button_height));
                        let mut c = canvas::UiCanvas::new(false, painter, icon_rect, offset, scale, None, Highlight::NONE);
                        c.clear(egui::Color32::GRAY);
                        view.draw_in(&empty_q, drawing_context, &mut c, &None);
                    }
                });
        }
    }

    fn menubar_options_fun(
        &self,
        view_uuid: &ViewUuid,
        label_provider: &ERef<dyn LabelProvider>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        P::menubar_options_fun(&self.model, view_uuid, label_provider, ui, commands);
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

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let models = super::umlclass_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }
}

pub struct PlantUmlTab {
    diagram: ERef<UmlClassDiagram>,
    plantuml_description: String,
}

impl PlantUmlTab {
    pub fn new(diagram: ERef<UmlClassDiagram>) -> Self {
        Self { diagram, plantuml_description: String::new() }
    }
}

impl CustomTab for PlantUmlTab {
    fn title(&self) -> String {
        "PlantUML description".to_owned()
    }

    fn show(&mut self, ui: &mut egui::Ui, _commands: &mut Vec<ProjectCommand>) {
        if ui.button("Refresh").clicked() {
            self.plantuml_description = self.diagram.read().plantuml();
        }

        ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.plantuml_description.as_str()),
        );
    }
}

pub fn new(no: u32) -> ERef<dyn DiagramController> {
    let name = format!("New UML class diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        ViewUuid::now_v7().into(),
        name.clone().into(),
        UmlClassDiagramAdapter::<UmlClassNullProfile>::new(diagram.clone()),
        Vec::new(),
    )
}

pub fn demo(no: u32) -> ERef<dyn DiagramController> {
    // https://www.uml-diagrams.org/class-diagrams-overview.html
    // https://www.uml-diagrams.org/design-pattern-abstract-factory-uml-class-diagram-example.html

    fn af_operations() -> Vec<(ERef<UmlClassOperation>, ERef<UmlClassOperationView<UmlClassNullProfile>>)>{
        vec![
            new_umlclass_operation(UFOption::Some(UmlClassVisibilityKind::Public), "createProductA", "", "ProductA", ""),
            new_umlclass_operation(UFOption::Some(UmlClassVisibilityKind::Public), "createProductB", "", "ProductB", ""),
        ]
    }

    let (class_af, class_af_view) = new_umlclass_class(
        "AbstractFactory",
        "interface",
        false,
        Vec::new(),
        af_operations(),
        egui::Pos2::new(200.0, 150.0),
    );

    let (class_cfx, class_cfx_view) = new_umlclass_class(
        "ConcreteFactoryX",
        "class",
        false,
        Vec::new(),
        af_operations(),
        egui::Pos2::new(100.0, 250.0),
    );

    let (class_cfy, class_cfy_view) = new_umlclass_class(
        "ConcreteFactoryY",
        "class",
        false,
        Vec::new(),
        af_operations(),
        egui::Pos2::new(300.0, 250.0),
    );

    let (realization_cfx, realization_cfx_view) = new_umlclass_dependency(
        "", "",
        false,
        None,
        (class_cfx.clone().into(), class_cfx_view.clone().into()),
        (class_af.clone().into(), class_af_view.clone().into()),
    );

    let (realization_cfy, realization_cfy_view) = new_umlclass_dependency(
        "", "",
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
    );

    let (usage_client_af, usage_client_af_view) = new_umlclass_dependency(
        "use", "",
        true,
        Some((ViewUuid::now_v7(), egui::Pos2::new(200.0, 50.0))),
        (class_client.clone().into(), class_client_view.clone().into()),
        (class_af.clone().into(), class_af_view.clone().into()),
    );

    let (class_producta, class_producta_view) = new_umlclass_class(
        "ProductA",
        "interface",
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(450.0, 150.0),
    );

    let (usage_client_producta, usage_client_producta_view) =
        new_umlclass_dependency(
            "use", "",
            true,
            Some((ViewUuid::now_v7(), egui::Pos2::new(450.0, 52.0))),
            (class_client.clone().into(), class_client_view.clone().into()),
            (class_producta.clone().into(), class_producta_view.clone().into()),
        );

    let (class_productb, class_productb_view) = new_umlclass_class(
        "ProductB",
        "interface",
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(650.0, 150.0),
    );

    let (usage_client_productb, usage_client_productb_view) =
        new_umlclass_dependency(
            "use", "",
            true,
            Some((ViewUuid::now_v7(), egui::Pos2::new(650.0, 48.0))),
            (class_client.clone().into(), class_client_view.clone().into()),
            (class_productb.clone().into(), class_productb_view.clone().into()),
        );

    let shape_operations = {
        let d = new_umlclass_operation(UFOption::Some(UmlClassVisibilityKind::Public), "draw", "", "", "");
        let m = new_umlclass_operation(UFOption::Some(UmlClassVisibilityKind::Public), "move", "", "", "");
        vec![d, m]
    };
    let (shape_model, shape_view) = new_umlclass_class("Shape", "entity", true, Vec::new(), shape_operations, egui::Pos2::new(200.0, 400.0));
    let (polygon_model, polygon_view) = new_umlclass_class("Polygon", "entity", false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 550.0));
    let circle_properties = {
        let r = new_umlclass_property(UFOption::Some(UmlClassVisibilityKind::Private), "radius", "float", "", "", "");
        let c = new_umlclass_property(UFOption::Some(UmlClassVisibilityKind::Private), "center", "Point", "", "", "");
        vec![r, c]
    };
    let (circle_model, circle_view) = new_umlclass_class("Circle", "entity", false, circle_properties, Vec::new(), egui::Pos2::new(300.0, 550.0));
    let (gen_model, gen_view) = new_umlclass_generalization(
        Some((ViewUuid::now_v7(), egui::Pos2::new(200.0, 490.0))),
        (polygon_model.clone(), polygon_view.clone().into()),
        (shape_model.clone(), shape_view.clone().into())
    );
    gen_model.write().set_is_covering = true;
    gen_model.write().set_is_disjoint = true;
    let gen_uuid = *gen_view.read().uuid();
    gen_view.write().apply_command(
        &InsensitiveCommand::AddDependency(gen_uuid, 0, None, UmlClassElementOrVertex::Element(circle_view.clone().into()), true),
        &mut Vec::new(),
        &mut HashSet::new(),
    );
    let point_properties = {
        let x = new_umlclass_property(UFOption::Some(UmlClassVisibilityKind::Private), "x", "float", "", "", "");
        let y = new_umlclass_property(UFOption::Some(UmlClassVisibilityKind::Private), "y", "float", "", "", "");
        vec![x, y]
    };
    let (point_model, point_view) = new_umlclass_class("Point", "struct", false, point_properties, Vec::new(), egui::Pos2::new(100.0, 700.0));
    let (point_assoc_model, point_assoc_view) = new_umlclass_association(
        "", "", None,
        (polygon_model.clone().into(), polygon_view.clone().into()),
        (point_model.clone().into(), point_view.clone().into())
    );
    point_assoc_model.write().source_label_multiplicity = Arc::new("0..*".to_owned());
    point_assoc_model.write().target_label_multiplicity = Arc::new("3..*".to_owned());
    point_assoc_model.write().target_navigability = UmlClassAssociationNavigability::Navigable;

    let (comment, comment_view) = new_umlclass_comment("This is a comment\nwith multiple lines", egui::Pos2::new(650.0, 250.0));
    let (commentlink1, commentlink1_view) = new_umlclass_commentlink(
        None,
        (comment.clone(), comment_view.clone().into()),
        (class_producta.clone().into(), class_producta_view.clone().into()),
    );
    let (commentlink2, commentlink2_view) = new_umlclass_commentlink(
        None,
        (comment.clone(), comment_view.clone().into()),
        (class_productb.clone().into(), class_productb_view.clone().into()),
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
            shape_model.into(),
            polygon_model.into(),
            circle_model.into(),
            gen_model.into(),
            point_model.into(),
            point_assoc_model.into(),
            comment.into(),
            commentlink1.into(),
            commentlink2.into(),
        ],
    ));
    DiagramControllerGen2::new(
        ViewUuid::now_v7().into(),
        name.clone().into(),
        UmlClassDiagramAdapter::new(diagram2.clone()),
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
            shape_view.into(),
            polygon_view.into(),
            circle_view.into(),
            gen_view.into(),
            point_view.into(),
            point_assoc_view.into(),
            comment_view.into(),
            commentlink1_view.into(),
            commentlink2_view.into(),
        ],
    )
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<DiagramControllerGen2<UmlClassDomain<UmlClassNullProfile>, UmlClassDiagramAdapter<UmlClassNullProfile>>>(&uuid)?)
}

#[derive(Clone, Copy, PartialEq)]
pub enum LinkType {
    Generalization,
    Dependency {
        target_arrow_open: bool,
        stereotype: &'static str,
    },
    Association {
        stereotype: &'static str,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Instance,
    Class { name: &'static str, stereotype: &'static str },
    ClassProperty,
    ClassOperation,
    LinkStart { link_type: LinkType },
    LinkEnd,
    LinkAddEnding { source: bool },
    PackageStart,
    PackageEnd,
    Comment,
    CommentLinkStart,
    CommentLinkEnd,
}

enum PartialUmlClassElement<P: UmlClassProfile> {
    None,
    Some(UmlClassElementView<P>),
    Link {
        link_type: LinkType,
        source: UmlClassClassifier,
        dest: Option<UmlClassClassifier>,
    },
    LinkEnding {
        source: bool,
        gen_model: ERef<UmlClassGeneralization>,
        new_model: Option<ModelUuid>,
    },
    Package {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    CommentLink {
        source: ERef<UmlClassComment>,
        dest: Option<UmlClassElement>,
    },
}

pub struct NaiveUmlClassTool<P: UmlClassProfile> {
    initial_stage: UmlClassToolStage,
    current_stage: UmlClassToolStage,
    result: PartialUmlClassElement<P>,
    event_lock: bool,
}

impl<P: UmlClassProfile> NaiveUmlClassTool<P> {
    pub fn new(initial_stage: UmlClassToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialUmlClassElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl<P: UmlClassProfile> Tool<UmlClassDomain<P>> for NaiveUmlClassTool<P> {
    type Stage = UmlClassToolStage;

    fn initial_stage(&self) -> Self::Stage {
        self.initial_stage
    }

    fn targetting_for_section(&self, element: Option<UmlClassElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                UmlClassToolStage::Instance
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::Comment
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::ClassProperty
                | UmlClassToolStage::ClassOperation
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkStart | UmlClassToolStage::CommentLinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClassPackage(..)) => match self.current_stage {
                UmlClassToolStage::Instance
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkEnd => TARGETTABLE_COLOR,

                UmlClassToolStage::ClassProperty
                | UmlClassToolStage::ClassOperation
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkStart => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClassInstance(..)) => match self.current_stage {
                UmlClassToolStage::Instance
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::ClassProperty
                | UmlClassToolStage::ClassOperation
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkStart
                | UmlClassToolStage::LinkStart { link_type: LinkType::Generalization }
                | UmlClassToolStage::LinkAddEnding { .. } => NON_TARGETTABLE_COLOR,

                UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::CommentLinkEnd => TARGETTABLE_COLOR
            },
            Some(UmlClassElement::UmlClass(..)) => match self.current_stage {
                UmlClassToolStage::ClassProperty
                | UmlClassToolStage::ClassOperation
                | UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::Instance
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkStart => NON_TARGETTABLE_COLOR,
            },
            Some(UmlClassElement::UmlClassProperty(..)
                | UmlClassElement::UmlClassOperation(..)) => NON_TARGETTABLE_COLOR,
            Some(UmlClassElement::UmlClassComment(..)) => match self.current_stage {
                UmlClassToolStage::CommentLinkStart => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::Instance
                | UmlClassToolStage::Class { .. }
                | UmlClassToolStage::ClassProperty
                | UmlClassToolStage::ClassOperation
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkEnd => NON_TARGETTABLE_COLOR,
            },
            Some(UmlClassElement::UmlClassGeneralization(..)
                | UmlClassElement::UmlClassDependency(..)
                | UmlClassElement::UmlClassAssociation(..)
                | UmlClassElement::UmlClassCommentLink(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &UmlClassQueryable<P>,  canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialUmlClassElement::Link {
                source,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlClassElement::LinkEnding { gen_model, .. } => {
                if let Some(view) = q.get_view(&gen_model.read().uuid) {
                    canvas.draw_line(
                        [pos, view.position()],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialUmlClassElement::CommentLink {
                source,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.read().uuid()) {
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

        match (self.current_stage, &mut self.result) {
            (UmlClassToolStage::Instance, _) => {
                let (_object_model, object_view) =
                    new_umlclass_instance("o", "Type", "", "", pos);
                self.result = PartialUmlClassElement::Some(object_view.into());
                self.event_lock = true;
            }
            (UmlClassToolStage::Class { name, stereotype }, _) => {
                let (_class_model, class_view) =
                    new_umlclass_class(name, stereotype, false, Vec::new(), Vec::new(), pos);
                self.result = PartialUmlClassElement::Some(class_view.into());
                self.event_lock = true;
            }
            (UmlClassToolStage::PackageStart, _) => {
                self.result = PartialUmlClassElement::Package {
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
            (UmlClassToolStage::Comment, _) => {
                let (_comment_model, comment_view) =
                    new_umlclass_comment("a comment", pos);
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
            UmlClassElement::UmlClassPackage(inner) => {
                match (self.current_stage, &mut self.result) {
                    (
                        UmlClassToolStage::CommentLinkEnd,
                        PartialUmlClassElement::CommentLink { dest, .. },
                    ) => {
                        *dest = Some(inner.into());
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            UmlClassElement::UmlClassInstance(inner) => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::Link {
                            link_type,
                            source: inner.into(),
                            dest: None,
                        };
                        self.current_stage = UmlClassToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        UmlClassToolStage::LinkEnd,
                        PartialUmlClassElement::Link { dest, .. },
                    ) => {
                        *dest = Some(inner.into());
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
                }
            }
            UmlClassElement::UmlClass(inner) => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::ClassProperty, PartialUmlClassElement::None) => {
                        let (_property, property_view) = new_umlclass_property(UFOption::None, "property", "Type", "", "", "");
                        self.result = PartialUmlClassElement::Some(property_view.into());
                        self.event_lock = true;
                    }
                    (UmlClassToolStage::ClassOperation, PartialUmlClassElement::None) => {
                        let (_operation, operation_view) = new_umlclass_operation(UFOption::None, "operation", "", "ReturnType", "");
                        self.result = PartialUmlClassElement::Some(operation_view.into());
                        self.event_lock = true;
                    }
                    (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::Link {
                            link_type,
                            source: inner.into(),
                            dest: None,
                        };
                        self.current_stage = UmlClassToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        UmlClassToolStage::LinkEnd,
                        PartialUmlClassElement::Link { dest, .. },
                    ) => {
                        *dest = Some(inner.into());
                        self.event_lock = true;
                    }
                    (UmlClassToolStage::LinkAddEnding { source }, &mut PartialUmlClassElement::LinkEnding { ref gen_model, ref mut new_model, .. }) => {
                        let inner_uuid = *inner.read().uuid;
                        let gen_model = gen_model.read();

                        if (source && !gen_model.sources.iter().any(|e| *e.read().uuid == inner_uuid))
                            || (!source && !gen_model.targets.iter().any(|e| *e.read().uuid == inner_uuid)) {
                            *new_model = Some(inner_uuid);
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
                }
            }
            UmlClassElement::UmlClassProperty(..)
            | UmlClassElement::UmlClassOperation(..) => {}
            UmlClassElement::UmlClassComment(inner) => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::CommentLinkStart, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::CommentLink {
                            source: inner,
                            dest: None,
                        };
                        self.current_stage = UmlClassToolStage::CommentLinkEnd;
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            UmlClassElement::UmlClassGeneralization(..)
            | UmlClassElement::UmlClassDependency(..)
            | UmlClassElement::UmlClassAssociation(..)
            | UmlClassElement::UmlClassCommentLink(..) => {}
        }
    }

    fn try_additional_dependency(&mut self) -> Option<(BucketNoT, ModelUuid, ModelUuid)> {
        match &mut self.result {
            PartialUmlClassElement::LinkEnding { source, gen_model, new_model } if new_model.is_some() => {
                let r = Some((if *source { 0 } else { 1 }, *gen_model.read().uuid, new_model.unwrap()));
                *new_model = None;
                r
            }
            _ => {
                None
            }
        }
    }

    fn try_construct_view(
        &mut self,
        into: &dyn ContainerGen2<UmlClassDomain<P>>,
    ) -> Option<(UmlClassElementView<P>, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                let esm: Option<Box<dyn CustomModal>> = match &x {
                    UmlClassElementView::Instance(inner) => Some(Box::new(UmlClassInstanceSetupModal::<P::InstanceStereotypeController>::from(&inner.read().model))),
                    UmlClassElementView::Class(inner) => Some(Box::new(UmlClassSetupModal::<P::ClassStereotypeController>::from(&inner.read().model))),
                    UmlClassElementView::ClassProperty(inner) => Some(Box::new(UmlClassPropertySetupModal::<P::ClassPropertyStereotypeController>::from(&inner.read().model))),
                    UmlClassElementView::ClassOperation(inner) => Some(Box::new(UmlClassOperationSetupModal::<P::ClassOperationStereotypeController>::from(&inner.read().model))),
                    _ => None,
                };
                self.result = PartialUmlClassElement::None;
                Some((x, esm))
            }
            PartialUmlClassElement::Link {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.uuid(), *dest.uuid());
                if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = UmlClassToolStage::LinkStart {
                        link_type: *link_type,
                    };

                    let link_view = match link_type {
                        LinkType::Generalization => {
                            if let (UmlClassClassifier::UmlClass(source), UmlClassClassifier::UmlClass(dest)) = (source, dest) {
                                new_umlclass_generalization(
                                    None,
                                    (source.clone(), source_controller),
                                    (dest.clone(), dest_controller),
                                ).1.into()
                            } else {
                                return None;
                            }
                        },
                        LinkType::Dependency { target_arrow_open, stereotype } => {
                            new_umlclass_dependency(
                                *stereotype,
                                "",
                                *target_arrow_open,
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        },
                        LinkType::Association { stereotype } => {
                            new_umlclass_association(
                                *stereotype,
                                "",
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        },
                    };

                    self.result = PartialUmlClassElement::None;

                    Some((link_view, None))
                } else {
                    None
                }
            }
            PartialUmlClassElement::CommentLink { source, dest: Some(dest) } => {
                let source_uuid = *source.read().uuid();
                if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&dest.uuid()),
                ) {
                    self.current_stage = UmlClassToolStage::CommentLinkStart;

                    let (_link_model, link_view) = new_umlclass_commentlink(
                        None,
                        (source.clone(), source_controller),
                        (dest.clone(), dest_controller),
                    );

                    self.result = PartialUmlClassElement::None;

                    Some((link_view.into(), None))
                } else {
                    None
                }
            }
            PartialUmlClassElement::Package { a, b: Some(b), .. } => {
                self.current_stage = UmlClassToolStage::PackageStart;

                let (_package_model, package_view) =
                    new_umlclass_package("a package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialUmlClassElement::None;
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
pub struct UmlClassPackageAdapter<P: UmlClassProfile> {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassPackage>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
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

    fn insert_element(&mut self, position: Option<PositionNoT>, element: UmlClassElement) -> Result<PositionNoT, ()> {
        self.model.write().insert_element(0, position, element).map_err(|_| ())
    }
    fn delete_element(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        self.model.write().remove_element(uuid).map(|e| e.1)
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>
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
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    UmlClassPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UmlClassPackage(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlClassPackage::new(new_uuid, (*old_model.name).clone(), old_model.contained_elements.clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            name_buffer: self.name_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            _profile: PhantomData,
        }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()) {
                *e = new_model.clone();
            }
        }
    }
}

pub fn new_umlclass_package<P: UmlClassProfile>(
    name: &str,
    bounds_rect: egui::Rect,
) -> (ERef<UmlClassPackage>, ERef<PackageViewT<P>>) {
    let package_model = ERef::new(UmlClassPackage::new(
        ModelUuid::now_v7(),
        name.to_owned(),
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
            name_buffer: (*m.name).clone(),
            comment_buffer: (*m.comment).clone(),
            _profile: PhantomData,
        },
        Vec::new(),
        bounds_rect,
    )
}


fn new_umlclass_instance<P: UmlClassProfile>(
    instance_name: &str,
    instance_type: &str,
    stereotype: &str,
    instance_slots: &str,
    position: egui::Pos2,
) -> (ERef<UmlClassInstance>, ERef<UmlClassInstanceView<P>>) {
    let instance_model = ERef::new(UmlClassInstance::new(
        ModelUuid::now_v7(),
        instance_name.to_owned(),
        instance_type.to_owned(),
        stereotype.to_owned(),
        instance_slots.to_owned(),
    ));
    let instance_view = new_umlclass_instance_view(instance_model.clone(), position);

    (instance_model, instance_view)
}
fn new_umlclass_instance_view<P: UmlClassProfile>(
    model: ERef<UmlClassInstance>,
    position: egui::Pos2,
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
        background_color: MGlobalColor::None,
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
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Name:");
        let r = ui.text_edit_singleline(&mut self.name_buffer);
        ui.label("Type:");
        ui.text_edit_singleline(&mut self.type_buffer);
        ui.label("Stereotype:");
        self.stereotype_controller.show(ui);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.instance_name = Arc::new(self.name_buffer.clone());
                m.instance_type = Arc::new(self.type_buffer.clone());
                m.stereotype = self.stereotype_controller.get();
                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button("Cancel").clicked() {
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
    fn association_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_radius = 8.0;
        let b_center = self.bounds_rect.right_top() + egui::Vec2::splat(b_radius / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * b_radius / ui_scale),
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

impl<P: UmlClassProfile> ContainerGen2<UmlClassDomain<P>> for UmlClassInstanceView<P> {}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassInstanceView<P> {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        _q: &UmlClassQueryable<P>,
        _lp: &UmlClassLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::InstanceName(Arc::new(self.name_buffer.clone())),
            ]));
        }

        ui.label("Type:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.type_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::InstanceType(Arc::new(self.type_buffer.clone())),
            ]));
        }

        ui.label("Stereotype:");
        if self.stereotype_controller.show(ui) {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get()),
            ]));
        }

        ui.label("Slots:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.slots_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::InstanceSlots(Arc::new(self.slots_buffer.clone())),
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
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        _: &UmlClassQueryable<P>,
        context: &GlobalDrawingContext,
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
            context.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE),
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
            let b_rect = self.association_button_rect(ui_scale);
            canvas.draw_rectangle(
                b_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b_rect.center(), egui::Align2::CENTER_CENTER, "↘", 14.0 / ui_scale, egui::Color32::BLACK);
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
        tool: &mut Option<NaiveUmlClassTool<P>>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
            InputEvent::Click(pos) if self.highlight.selected && self.association_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveUmlClassTool {
                    initial_stage: UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "" } },
                    current_stage: UmlClassToolStage::LinkEnd,
                    result: PartialUmlClassElement::Link {
                        link_type: LinkType::Association { stereotype: "" },
                        source: self.model.clone().into(),
                        dest: None,
                    },
                    event_lock: true,
                });

                EventHandlingStatus::HandledByElement
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
                let coerced_delta = coerced_pos - self.bounds_rect.center();

                if self.highlight.selected {
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
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
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            UmlClassPropChange::InstanceName(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::InstanceName(model.instance_name.clone())],
                                ));
                                model.instance_name = name.clone();
                            }
                            UmlClassPropChange::InstanceType(t) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::InstanceType(model.instance_type.clone())],
                                ));
                                model.instance_type = t.clone();
                            }
                            UmlClassPropChange::InstanceSlots(s) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::InstanceSlots(model.instance_slots.clone())],
                                ));
                                model.instance_slots = s.clone();
                            }
                            UmlClassPropChange::StereotypeChange(stereotype) => {
                                if !self.stereotype_controller.is_valid(&stereotype) {
                                    continue;
                                }

                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::StereotypeChange(model.stereotype.clone())],
                                ));
                                model.stereotype = stereotype.clone();
                            }
                            UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                                ));
                                self.background_color = *color;
                            }
                            UmlClassPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
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

        let modelish = if let Some(UmlClassElement::UmlClassInstance(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
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
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Stereotype:");
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
            .selected_text(self.visibility_buffer.as_ref().map(|e| e.name()).unwrap_or("Unspecified"))
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    ui.selectable_value(&mut self.visibility_buffer, e, e.as_ref().map(|e| e.name()).unwrap_or("Unspecified"));
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
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();

                m.stereotype = self.stereotype_controller.get();
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
            if ui.button("Cancel").clicked() {
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
        q: &UmlClassQueryable<P>,
        context: &GlobalDrawingContext,
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
        if let Some((pos, tool)) = tool && self.bounds_rect.contains(*pos) {
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

impl<P: UmlClassProfile> ContainerGen2<UmlClassDomain<P>> for UmlClassPropertyView<P> {}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassPropertyView<P> {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        lp: &<UmlClassDomain<P> as Domain>::LabelProviderT,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>>,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Stereotype:");
        if self.stereotype_controller.show(ui) {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get()),
            ]));
        }

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ]));
        }

        ui.label("Type:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.value_type_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::PropertyTypeChange(Arc::new(self.value_type_buffer.clone())),
            ]));
        }

        ui.label("Multiplicity:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.multiplicity_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::PropertyMultiplicityChange(Arc::new(self.multiplicity_buffer.clone())),
            ]));
        }

        ui.label("Default value:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.default_value_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::PropertyDefaultValueChange(Arc::new(self.default_value_buffer.clone())),
            ]));
        }

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(self.visibility_buffer.as_ref().map(|e| e.name()).unwrap_or("Unspecified"))
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    if ui.selectable_value(&mut self.visibility_buffer, e, e.as_ref().map(|e| e.name()).unwrap_or("Unspecified")).clicked() {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::VisibilityChange(e),
                        ]));
                    }
                }
            });

        if ui.checkbox(&mut self.is_static_buffer, "isStatic").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsStaticChange(self.is_static_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_derived_buffer, "isDerived").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsDerivedChange(self.is_derived_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_read_only_buffer, "isReadOnly").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsReadOnlyChange(self.is_read_only_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_ordered_buffer, "isOrdered").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsOrderedChange(self.is_ordered_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_unique_buffer, "isUnique").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsUniqueChange(self.is_unique_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_id_buffer, "isID").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsIdChange(self.is_id_buffer),
            ]));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(self.bounds_rect.left_top(), q, context, canvas, tool).1
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<<UmlClassDomain<P> as Domain>::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                    self.highlight.selected = true;
                } else {
                    self.highlight.selected = !self.highlight.selected;
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>>,
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
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(..)
            | InsensitiveCommand::MoveSpecificElements(..)
            | InsensitiveCommand::MoveAllElements(..)
            | InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            UmlClassPropChange::StereotypeChange(stereotype) => {
                                if !self.stereotype_controller.is_valid(&stereotype) {
                                    continue;
                                }

                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::StereotypeChange(
                                        model.stereotype.clone(),
                                    )],
                                ));
                                model.stereotype = stereotype.clone();
                            }
                            UmlClassPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::NameChange(model.name.clone())],
                                ));
                                model.name = name.clone();
                            }
                            UmlClassPropChange::PropertyTypeChange(value_type) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::PropertyTypeChange(model.value_type.clone())],
                                ));
                                model.value_type = value_type.clone();
                            }
                            UmlClassPropChange::PropertyMultiplicityChange(multiplicity) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::PropertyMultiplicityChange(model.multiplicity.clone())],
                                ));
                                model.multiplicity = multiplicity.clone();
                            }
                            UmlClassPropChange::PropertyDefaultValueChange(default_value) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::PropertyDefaultValueChange(model.default_value.clone())],
                                ));
                                model.default_value = default_value.clone();
                            }
                            UmlClassPropChange::VisibilityChange(visibility) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::VisibilityChange(model.visibility.clone())],
                                ));
                                model.visibility = visibility.clone();
                            }
                            UmlClassPropChange::IsStaticChange(is_static) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsStaticChange(model.is_static.clone())],
                                ));
                                model.is_static = is_static.clone();
                            }
                            UmlClassPropChange::IsDerivedChange(is_derived) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsDerivedChange(model.is_derived.clone())],
                                ));
                                model.is_derived = is_derived.clone();
                            }
                            UmlClassPropChange::IsReadOnlyChange(is_read_only) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsReadOnlyChange(model.is_read_only.clone())],
                                ));
                                model.is_read_only = is_read_only.clone();
                            }
                            UmlClassPropChange::IsOrderedChange(is_ordered) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsOrderedChange(model.is_ordered.clone())],
                                ));
                                model.is_ordered = is_ordered.clone();
                            }
                            UmlClassPropChange::IsUniqueChange(is_unique) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsUniqueChange(model.is_unique.clone())],
                                ));
                                model.is_unique = is_unique.clone();
                            }
                            UmlClassPropChange::IsIdChange(is_id) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsIdChange(model.is_id.clone())],
                                ));
                                model.is_id = is_id.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
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
                t.push_str(vis.char());
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
        flattened_views: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
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

        let modelish = if let Some(UmlClassElement::UmlClassProperty(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
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
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Stereotype:");
        self.stereotype_controller.show(ui);
        ui.label("Name:");
        let r = ui.text_edit_singleline(&mut self.name_buffer);
        ui.label("Parameters:");
        ui.text_edit_singleline(&mut self.parameters_buffer);
        ui.label("Return type:");
        ui.text_edit_singleline(&mut self.return_type_buffer);

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(self.visibility_buffer.as_ref().map(|e| e.name()).unwrap_or("Unspecified"))
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    ui.selectable_value(&mut self.visibility_buffer, e, e.as_ref().map(|e| e.name()).unwrap_or("Unspecified"));
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
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.stereotype = self.stereotype_controller.get();
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
            if ui.button("Cancel").clicked() {
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
        q: &UmlClassQueryable<P>,
        context: &GlobalDrawingContext,
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
        if let Some((pos, tool)) = tool && self.bounds_rect.contains(*pos) {
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

impl<P: UmlClassProfile> ContainerGen2<UmlClassDomain<P>> for UmlClassOperationView<P> {}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassOperationView<P> {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        lp: &<UmlClassDomain<P> as Domain>::LabelProviderT,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>>,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Stereotype:");
        if self.stereotype_controller.show(ui) {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get()),
            ]));
        }

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ]));
        }

        ui.label("Parameters:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.parameters_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::OperationParametersChange(Arc::new(self.parameters_buffer.clone())),
            ]));
        }

        ui.label("Return type:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.return_type_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::OperationReturnTypeChange(Arc::new(self.return_type_buffer.clone())),
            ]));
        }

        ui.label("Visibility:");
        egui::ComboBox::from_id_salt("Visibility:")
            .selected_text(self.visibility_buffer.as_ref().map(|e| e.name()).unwrap_or("Unspecified"))
            .show_ui(ui, |ui| {
                for e in [
                    UFOption::None,
                    UFOption::Some(UmlClassVisibilityKind::Public),
                    UFOption::Some(UmlClassVisibilityKind::Package),
                    UFOption::Some(UmlClassVisibilityKind::Protected),
                    UFOption::Some(UmlClassVisibilityKind::Private),
                ] {
                    if ui.selectable_value(&mut self.visibility_buffer, e, e.as_ref().map(|e| e.name()).unwrap_or("Unspecified")).clicked() {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::VisibilityChange(e),
                        ]));
                    }
                }
            });

        if ui.checkbox(&mut self.is_static_buffer, "isStatic").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsStaticChange(self.is_static_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_abstract_buffer, "isAbstract").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsAbstractChange(self.is_abstract_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_query_buffer, "isQuery").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsQueryChange(self.is_query_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_ordered_buffer, "isOrdered").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsOrderedChange(self.is_ordered_buffer),
            ]));
        }
        if ui.checkbox(&mut self.is_unique_buffer, "isUnique").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::IsUniqueChange(self.is_unique_buffer),
            ]));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<UmlClassDomain<P> as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &<UmlClassDomain<P> as Domain>::ToolT)>,
    ) -> TargettingStatus {
        self.draw_inner(self.bounds_rect.left_top(), q, context, canvas, tool).1
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<<UmlClassDomain<P> as Domain>::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                    self.highlight.selected = true;
                } else {
                    self.highlight.selected = !self.highlight.selected;
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<<UmlClassDomain<P> as Domain>::AddCommandElementT, <UmlClassDomain<P> as Domain>::PropChangeT>>,
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
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
            }
            InsensitiveCommand::MoveSpecificElements(..)
            | InsensitiveCommand::MoveSpecificElements(..)
            | InsensitiveCommand::MoveAllElements(..)
            | InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::DeleteSpecificElements(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            UmlClassPropChange::StereotypeChange(stereotype) => {
                                if !self.stereotype_controller.is_valid(&stereotype) {
                                    continue;
                                }

                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::StereotypeChange(
                                        model.stereotype.clone(),
                                    )],
                                ));
                                model.stereotype = stereotype.clone();
                            }
                            UmlClassPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::NameChange(model.name.clone())],
                                ));
                                model.name = name.clone();
                            }
                            UmlClassPropChange::OperationParametersChange(parameters) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::OperationParametersChange(model.parameters.clone())],
                                ));
                                model.parameters = parameters.clone();
                            }
                            UmlClassPropChange::OperationReturnTypeChange(return_type) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::OperationReturnTypeChange(model.return_type.clone())],
                                ));
                                model.return_type = return_type.clone();
                            }
                            UmlClassPropChange::VisibilityChange(visibility) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::VisibilityChange(model.visibility.clone())],
                                ));
                                model.visibility = visibility.clone();
                            }
                            UmlClassPropChange::IsStaticChange(is_static) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsStaticChange(model.is_static.clone())],
                                ));
                                model.is_static = is_static.clone();
                            }
                            UmlClassPropChange::IsAbstractChange(is_abstract) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsAbstractChange(model.is_abstract.clone())],
                                ));
                                model.is_abstract = is_abstract.clone();
                            }
                            UmlClassPropChange::IsQueryChange(is_query) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsQueryChange(model.is_query.clone())],
                                ));
                                model.is_query = is_query.clone();
                            }
                            UmlClassPropChange::IsOrderedChange(is_ordered) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsOrderedChange(model.is_ordered.clone())],
                                ));
                                model.is_ordered = is_ordered.clone();
                            }
                            UmlClassPropChange::IsUniqueChange(is_unique) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::IsUniqueChange(model.is_unique.clone())],
                                ));
                                model.is_unique = is_unique.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
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
                t.push_str(vis.char());
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
        flattened_views: &mut HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
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

        let modelish = if let Some(UmlClassElement::UmlClassOperation(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
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
    );

    (class_model, class_view)
}
pub fn new_umlclass_class_view<P: UmlClassProfile>(
    model: ERef<UmlClass>,
    properties_views: Vec<ERef<UmlClassPropertyView<P>>>,
    operations_views: Vec<ERef<UmlClassOperationView<P>>>,
    position: egui::Pos2,
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
        background_color: MGlobalColor::None,

        suppress_template_parameters: false,
        suppress_properties: false,
        suppress_operations: false,
        comment_indication: CommentIndication::Icon,

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
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Stereotype:");
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
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.stereotype = self.stereotype_controller.get();
                m.name = Arc::new(self.name_buffer.clone());
                result = CustomModalResult::CloseModified(*m.uuid);
            }
            if ui.button("Cancel").clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
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

    suppress_template_parameters: bool,
    suppress_properties: bool,
    suppress_operations: bool,
    comment_indication: CommentIndication,

    #[nh_context_serde(skip_and_default)]
    _profile: PhantomData<P>,
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommentIndication {
    None,
    Icon,
    TextCompartment,
}

impl CommentIndication {
    fn char(&self) -> &'static str {
        match self {
            CommentIndication::None => "None",
            CommentIndication::Icon => "Icon",
            CommentIndication::TextCompartment => "Text Compartment",
        }
    }
}

impl<P: UmlClassProfile> UmlClassView<P> {
    const BUTTON_RADIUS: f32 = 8.0;
    fn association_button_rect(&self, ui_scale: f32) ->  egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::splat(Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
    fn property_button_rect(&self, ui_scale: f32) ->  egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::new(Self::BUTTON_RADIUS / ui_scale, 3.0 * Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
    fn operation_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::new(Self::BUTTON_RADIUS / ui_scale, 5.0 * Self::BUTTON_RADIUS / ui_scale);
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

impl<P: UmlClassProfile> ContainerGen2<UmlClassDomain<P>> for UmlClassView<P> {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<UmlClassElementView<P>> {
        for e in &self.properties_views {
            if *uuid == *e.read().model_uuid() {
                return Some(e.clone().into());
            }
        }
        for e in &self.operations_views {
            if *uuid == *e.read().model_uuid() {
                return Some(e.clone().into());
            }
        }
        None
    }
}

pub fn draw_uml_class<'a>(
    canvas: &'a mut dyn canvas::NHCanvas,
    position: egui::Pos2,
    top_label: Option<Arc<String>>,
    main_label: &str,
    bottom_label: Option<Arc<String>>,
    compartments: &[(egui::Vec2, Box<dyn Fn(&mut dyn canvas::NHCanvas, egui::Pos2) + 'a>)],
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
        canvas.draw_rectangle(rect, egui::CornerRadius::ZERO, fill, stroke.into(), highlight);

        (
            offsets,
            global_offset,
            max_width,
            category_separators,
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

            (c.1)(canvas, egui::Pos2::new(
                position.x - max_width / 2.0,
                position.y - global_offset + offsets[offset_counter],
            ));
            offset_counter += 1;
        }
    }

    rect
}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassView<P> {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &UmlClassQueryable<P>,
        lp: &UmlClassLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        let properties_status = self.properties_views.iter()
            .flat_map(|e| e.write().show_properties(drawing_context, q, lp, ui, commands).to_non_default())
            .next();
        if let Some(status) = properties_status.or_else(|| self.operations_views.iter()
                .flat_map(|e| e.write().show_properties(drawing_context, q, lp, ui, commands).to_non_default())
                .next()) {
            return status;
        }

        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Stereotype:");
        if self.stereotype_controller.show(ui) {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::StereotypeChange(self.stereotype_controller.get()),
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
                UmlClassPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ]));
        }

        ui.label("Template parameters:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.template_parameters_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::TemplateParametersChange(Arc::new(self.template_parameters_buffer.clone())),
            ]));
        }

        if ui.checkbox(&mut self.is_abstract_buffer, "isAbstract").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::ClassAbstractChange(self.is_abstract_buffer),
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
                UmlClassPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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

        ui.label("Background color:");
        if crate::common::controller::mglobalcolor_edit_button(
            &drawing_context.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        ui.checkbox(&mut self.suppress_template_parameters, "suppress template parameters");
        ui.checkbox(&mut self.suppress_properties, "suppress properties");
        ui.checkbox(&mut self.suppress_operations, "suppress operations");

        ui.label("Comment indication");
        egui::ComboBox::from_id_salt("comment indication")
            .selected_text(self.comment_indication.char())
            .show_ui(ui, |ui| {
                for e in [CommentIndication::None, CommentIndication::Icon, CommentIndication::TextCompartment] {
                    ui.selectable_value(&mut self.comment_indication, e, e.char());
                }
            });

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &UmlClassQueryable<P>,
        context: &GlobalDrawingContext,
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
        let mut body = Vec::<(egui::Vec2, Box<dyn Fn(&mut dyn canvas::NHCanvas, egui::Pos2)>)>::new();
        if !self.suppress_properties && !self.properties_views.is_empty() {
            body.push((
                rect_union_fold(self.properties_views.iter().map(|e| e.read().bounding_box())).size(),
                Box::new(|c, at| {
                    self.properties_views.iter().fold(at, |s, e| {
                        let r = e.write().draw_inner(s, q, context, c, tool);
                        if r.1 != TargettingStatus::NotDrawn {
                            *child_status.write().unwrap() = r.1;
                        }
                        r.0.left_bottom()
                    });
                })
            ));
        }
        if !self.suppress_operations && !self.operations_views.is_empty() {
            body.push((
                rect_union_fold(self.operations_views.iter().map(|e| e.read().bounding_box())).size(),
                Box::new(|c, at| {
                    self.operations_views.iter().fold(at, |s, e| {
                        let r = e.write().draw_inner(s, q, context, c, tool);
                        if r.1 != TargettingStatus::NotDrawn {
                            *child_status.write().unwrap() = r.1;
                        }
                        r.0.left_bottom()
                    });
                })
            ));
        }
        if self.comment_indication == CommentIndication::TextCompartment && !read.comment.is_empty() {
            let comment = read.comment.clone();
            body.push((
                canvas.measure_text(self.position, egui::Align2::LEFT_TOP, &*read.comment, canvas::CLASS_ITEM_FONT_SIZE).size(),
                Box::new(move |c, at| {
                    c.draw_text(at, egui::Align2::LEFT_TOP, &*comment, canvas::CLASS_ITEM_FONT_SIZE, egui::Color32::BLACK);
                })
            ));
        }

        self.bounds_rect = draw_uml_class(
            canvas,
            self.position,
            self.stereotype_in_guillemets.clone(),
            &read.name,
            None,

            &body,
            context.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE),
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        if !self.suppress_template_parameters && !read.template_parameters.is_empty() {
            let text_bounds = canvas.measure_text(
                self.bounds_rect.right_top(),
                egui::Align2::CENTER_CENTER,
                &read.template_parameters,
                canvas::CLASS_TOP_FONT_SIZE,
            ).expand(2.0);
            canvas.draw_rectangle(
                text_bounds,
                egui::CornerRadius::ZERO,
                context.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE),
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
            let b1 = self.association_button_rect(ui_scale);
            canvas.draw_rectangle(
                b1,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b1.center(), egui::Align2::CENTER_CENTER, "↘", 14.0 / ui_scale, egui::Color32::BLACK);

            let b2 = self.property_button_rect(ui_scale);
            canvas.draw_rectangle(
                b2,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b2.center(), egui::Align2::CENTER_CENTER, "P", 14.0 / ui_scale, egui::Color32::BLACK);

            let b3 = self.operation_button_rect(ui_scale);
            canvas.draw_rectangle(
                b3,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b3.center(), egui::Align2::CENTER_CENTER, "O", 14.0 / ui_scale, egui::Color32::BLACK);
        }

        if canvas.ui_scale().is_some() {
            if self.comment_indication == CommentIndication::Icon && !read.comment.is_empty() {
                canvas.draw_polygon(
                    {
                        let b = self.bounds_rect.left_top() + egui::Vec2::splat(2.5);
                        canvas::COMMENT_INDICATOR.iter()
                            .map(|e| b + e.to_vec2()).collect()
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
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
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
        tool: &mut Option<NaiveUmlClassTool<P>>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
            InputEvent::Click(pos) if self.highlight.selected && self.association_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveUmlClassTool {
                    initial_stage: UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "" } },
                    current_stage: UmlClassToolStage::LinkEnd,
                    result: PartialUmlClassElement::Link {
                        link_type: LinkType::Association { stereotype: "" },
                        source: self.model.clone().into(),
                        dest: None,
                    },
                    event_lock: true,
                });

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.highlight.selected && self.property_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveUmlClassTool {
                    initial_stage: UmlClassToolStage::ClassProperty,
                    current_stage: UmlClassToolStage::ClassProperty,
                    result: PartialUmlClassElement::None,
                    event_lock: false,
                });

                if let Some(tool) = tool {
                    tool.add_section(self.model());
                    if let Some((view, esm)) = tool.try_construct_view(self)
                        && matches!(view, UmlClassElementView::ClassProperty(_)) {
                        commands.push(InsensitiveCommand::AddDependency(*self.uuid, 0, None, view.into(), true).into());
                        if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            *element_setup_modal = esm;
                        }
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.highlight.selected && self.operation_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveUmlClassTool {
                    initial_stage: UmlClassToolStage::ClassOperation,
                    current_stage: UmlClassToolStage::ClassOperation,
                    result: PartialUmlClassElement::None,
                    event_lock: false,
                });

                if let Some(tool) = tool {
                    tool.add_section(self.model());
                    if let Some((view, esm)) = tool.try_construct_view(self)
                        && matches!(view, UmlClassElementView::ClassOperation(_)) {
                        commands.push(InsensitiveCommand::AddDependency(*self.uuid, 1, None, view.into(), true).into());
                        if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            *element_setup_modal = esm;
                        }
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                let child = self.properties_views.iter()
                    .map(|e| {
                        let mut w = e.write();
                        (*w.uuid, w.highlight.selected, w.handle_event(event, ehc, tool, element_setup_modal, commands))
                    })
                    .find(|e| e.2 != EventHandlingStatus::NotHandled)
                    .or_else(|| self.operations_views.iter()
                        .map(|e| {
                            let mut w = e.write();
                            (*w.uuid, w.highlight.selected, w.handle_event(event, ehc, tool, element_setup_modal, commands))
                        })
                        .find(|e| e.2 != EventHandlingStatus::NotHandled));

                match child {
                    Some((uuid, selected, EventHandlingStatus::HandledByElement)) => {
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(uuid).collect(),
                                    true,
                                    Highlight::SELECTED,
                                )
                                .into(),
                            );
                        } else {
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(uuid).collect(),
                                    !selected,
                                    Highlight::SELECTED,
                                )
                                .into(),
                            );
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

                    if let Some((view, esm)) = tool.try_construct_view(self)
                        && matches!(view, UmlClassElementView::ClassProperty(_) | UmlClassElementView::ClassOperation(_)) {
                        let b = match view {
                            UmlClassElementView::ClassProperty(_) => 0,
                            UmlClassElementView::ClassOperation(_) => 1,
                            _ => unreachable!()
                        };
                        commands.push(InsensitiveCommand::AddDependency(*self.uuid, b, None, view.into(), true).into());
                        if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            *element_setup_modal = esm;
                        }
                    }
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
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
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
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            ($self:expr) => {
                $self.properties_views.iter()
                    .for_each(|e| e.write().apply_command(command, undo_accumulator, affected_models));
                $self.operations_views.iter()
                    .for_each(|e| e.write().apply_command(command, undo_accumulator, affected_models));
            };
        }

        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
                recurse!(self);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }
                recurse!(self);
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
                recurse!(self);
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
            | InsensitiveCommand::ResizeSpecificElementsTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(uuids, into_model) => {
                if *into_model {
                    let mut removed_any = false;
                    self.properties_views.retain(
                        |e| {
                            let r = e.read();
                            if uuids.contains(&r.uuid)
                                && let Some((b, pos)) = self.model.write().remove_element(&r.model_uuid()) {
                                undo_accumulator.push(InsensitiveCommand::AddDependency(
                                    *self.uuid,
                                    b,
                                    Some(pos),
                                    UmlClassElementOrVertex::Element(e.clone().into()),
                                    true,
                                ));
                                removed_any = true;
                                false
                            } else {
                                true
                            }
                        }
                    );
                    self.operations_views.retain(
                        |e| {
                            let r = e.read();
                            if uuids.contains(&r.uuid)
                                && let Some((b, pos)) = self.model.write().remove_element(&r.model_uuid()){
                                undo_accumulator.push(InsensitiveCommand::AddDependency(
                                    *self.uuid,
                                    b,
                                    Some(pos),
                                    UmlClassElementOrVertex::Element(e.clone().into()),
                                    true,
                                ));
                                removed_any = true;
                                false
                            } else {
                                true
                            }
                        }
                    );

                    if removed_any {
                        affected_models.insert(*self.model.read().uuid);
                    }
                }
            }
            InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..) => {}
            InsensitiveCommand::AddDependency(v, b, pos, e, into_model) => {
                if *v == *self.uuid && *into_model {
                    let mut w = self.model.write();
                    if let UmlClassElementOrVertex::Element(e) = e
                        && let Ok(pos) = w.insert_element(*b, *pos, e.model()) {
                        match e {
                            UmlClassElementView::ClassProperty(inner) => {
                                self.properties_views.insert(pos.try_into().unwrap(), inner.clone());
                            },
                            UmlClassElementView::ClassOperation(inner) => {
                                self.operations_views.insert(pos.try_into().unwrap(), inner.clone());
                            },
                            _ => return,
                        }

                        let uuid = *e.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                            *self.uuid,
                            *b,
                            uuid,
                            true,
                        ));
                        affected_models.insert(*w.uuid);
                    }
                }
            }
            InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            UmlClassPropChange::StereotypeChange(stereotype) => {
                                if !self.stereotype_controller.is_valid(&stereotype) {
                                    continue;
                                }

                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::StereotypeChange(
                                        model.stereotype.clone(),
                                    )],
                                ));
                                model.stereotype = stereotype.clone();
                            }
                            UmlClassPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::NameChange(model.name.clone())],
                                ));
                                model.name = name.clone();
                            }
                            UmlClassPropChange::TemplateParametersChange(template_parameters) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::TemplateParametersChange(model.template_parameters.clone())],
                                ));
                                model.template_parameters = template_parameters.clone();
                            }
                            UmlClassPropChange::ClassAbstractChange(is_abstract) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ClassAbstractChange(
                                        model.is_abstract,
                                    )],
                                ));
                                model.is_abstract = *is_abstract;
                            }
                            UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                                ));
                                self.background_color = *color;
                            }
                            UmlClassPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::CommentChange(model.comment.clone())],
                                ));
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
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        for e in &self.properties_views {
            let mut w = e.write();
            w.head_count(flattened_views, flattened_views_status, flattened_represented_models);
            flattened_views.insert(*w.uuid(), e.clone().into());
        }
        for e in &self.operations_views {
            let mut w = e.write();
            w.head_count(flattened_views, flattened_views_status, flattened_represented_models);
            flattened_views.insert(*w.uuid(), e.clone().into());
        }
    }
    fn collect_model_uuids(&self, into: &mut HashSet<ModelUuid>) {
        into.insert(*self.model_uuid());
        for e in &self.properties_views {
            e.read().collect_model_uuids(into);
        }
        for e in &self.operations_views {
            e.read().collect_model_uuids(into);
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

        let modelish = if let Some(UmlClassElement::UmlClass(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut dev_null = HashMap::new();
        self.properties_views.iter().for_each(|e| e.read().deep_copy_clone(uuid_present, &mut dev_null, c, m));
        self.operations_views.iter().for_each(|e| e.read().deep_copy_clone(uuid_present, &mut dev_null, c, m));

        let mut cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            properties_views: self.properties_views.clone(),
            operations_views: self.operations_views.clone(),
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
            suppress_template_parameters: self.suppress_template_parameters,
            suppress_properties: self.suppress_properties,
            suppress_operations: self.suppress_operations,
            comment_indication: self.comment_indication,
            _profile: PhantomData,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, <UmlClassDomain<P> as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <UmlClassDomain<P> as Domain>::CommonElementT>,
    ) {
        for e in self.properties_views.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlClassElementView::ClassProperty(new_property)) = c.get(&uuid) {
                *e = new_property.clone();
            }
        }
        for e in self.operations_views.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlClassElementView::ClassOperation(new_operation)) = c.get(&uuid) {
                *e = new_operation.clone();
            }
        }

        let mut w = self.model.write();
        for e in w.properties.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlClassElement::UmlClassProperty(new_property)) = m.get(&uuid) {
                *e = new_property.clone();
            }
        }
        for e in w.operations.iter_mut() {
            let uuid = *e.read().uuid;
            if let Some(UmlClassElement::UmlClassOperation(new_operation)) = m.get(&uuid) {
                *e = new_operation.clone();
            }
        }
    }
}


pub fn new_umlclass_generalization<P: UmlClassProfile>(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClass>, UmlClassElementView<P>),
    target: (ERef<UmlClass>, UmlClassElementView<P>),
) -> (ERef<UmlClassGeneralization>, ERef<GeneralizationViewT<P>>) {
    let link_model = ERef::new(UmlClassGeneralization::new(
        ModelUuid::now_v7(),
        vec![source.0],
        vec![target.0],
    ));
    let link_view = new_umlclass_generalization_view(link_model.clone(), center_point, vec![source.1], vec![target.1]);
    (link_model, link_view)
}
pub fn new_umlclass_generalization_view<P: UmlClassProfile>(
    model: ERef<UmlClassGeneralization>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    sources: Vec<UmlClassElementView<P>>,
    targets: Vec<UmlClassElementView<P>>,
) -> ERef<GeneralizationViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(m.sources.iter().map(|e| *e.read().uuid), *m.targets[0].read().uuid, targets[0].min_shape(), center_point);

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlClassGeneralizationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        sources.into_iter().zip(sp.into_iter()).map(|e| Ending::new_p(e.0, e.1)).collect(),
        targets.into_iter().zip(tp.into_iter()).map(|e| Ending::new_p(e.0, e.1)).collect(),
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>> for UmlClassGeneralizationAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        self.temporaries.midpoint_label.clone()
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
    fn insert_source(&mut self, position: Option<PositionNoT>, e: <UmlClassDomain<P> as Domain>::CommonElementT) -> Result<PositionNoT, ()> {
        self.model.write().insert_element(MULTICONNECTION_SOURCE_BUCKET, position, e).map_err(|_| ())
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
    fn insert_target(&mut self, position: Option<PositionNoT>, e: <UmlClassDomain<P> as Domain>::CommonElementT) -> Result<PositionNoT, ()> {
        self.model.write().insert_element(MULTICONNECTION_TARGET_BUCKET, position, e).map_err(|_| ())
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
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if ui.add_enabled(self.model.read().targets.len() <= 1, egui::Button::new("Add source")).clicked() {
            return PropertiesStatus::ToolRequest(
                Some(NaiveUmlClassTool {
                    initial_stage: UmlClassToolStage::LinkAddEnding { source: true },
                    current_stage: UmlClassToolStage::LinkAddEnding { source: true },
                    result: PartialUmlClassElement::LinkEnding {
                        source: true,
                        gen_model: self.model.clone(),
                        new_model: None,
                    },
                    event_lock: false,
                })
            );
        }
        if ui.add_enabled(self.model.read().sources.len() <= 1, egui::Button::new("Add target")).clicked() {
            return PropertiesStatus::ToolRequest(
                Some(NaiveUmlClassTool {
                    initial_stage: UmlClassToolStage::LinkAddEnding { source: false },
                    current_stage: UmlClassToolStage::LinkAddEnding { source: false },
                    result: PartialUmlClassElement::LinkEnding {
                        source: false,
                        gen_model: self.model.clone(),
                        new_model: None,
                    },
                    event_lock: false,
                })
            );
        }

        if ui.add_enabled(true, egui::Button::new("Switch source and target")).clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ]));
        }

        ui.label("Generalization set name:");
        if ui.text_edit_singleline(&mut self.temporaries.set_name_buffer).changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::SetNameChange(Arc::new(self.temporaries.set_name_buffer.clone())),
            ]));
        }
        if ui.checkbox(&mut self.temporaries.set_is_covering_buffer, "isCovering").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::SetCoveringChange(self.temporaries.set_is_covering_buffer),
            ]));
        }
        if ui.checkbox(&mut self.temporaries.set_is_disjoint_buffer, "isDisjoint").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::SetDisjointChange(self.temporaries.set_is_disjoint_buffer),
            ]));
        }
        ui.separator();

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    UmlClassPropChange::SetNameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::SetNameChange(model.set_name.clone())],
                        ));
                        model.set_name = name.clone();
                    }
                    UmlClassPropChange::SetCoveringChange(is_covering) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::SetCoveringChange(model.set_is_covering.clone())],
                        ));
                        model.set_is_covering = is_covering.clone();
                    }
                    UmlClassPropChange::SetDisjointChange(is_disjoint) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::SetDisjointChange(model.set_is_disjoint.clone())],
                        ));
                        model.set_is_disjoint = is_disjoint.clone();
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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

        let set_props_label = if model.sources.len() > 1 || model.targets.len() > 1 {
            Some(format!("{{{}, {}}}",
                if model.set_is_covering {"complete"} else {"incomplete"},
                if model.set_is_disjoint {"disjoint"} else {"overlapping"},
            ))
        } else { None };
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
                ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::EmptyTriangle),
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
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UmlClassGeneralization(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut model = self.model.write();

        for e in model.sources.iter_mut() {
            let sid = *e.read().uuid;
            if let Some(UmlClassElement::UmlClass(new_source)) = m.get(&sid) {
                *e = new_source.clone();
            }
        }
        for e in model.targets.iter_mut() {
            let tid = *e.read().uuid;
            if let Some(UmlClassElement::UmlClass(new_target)) = m.get(&tid) {
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
    source: (UmlClassClassifier, UmlClassElementView<P>),
    target: (UmlClassClassifier, UmlClassElementView<P>),
) -> (ERef<UmlClassDependency>, ERef<DependencyViewT<P>>) {
    let link_model = ERef::new(UmlClassDependency::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        source.0,
        target.0,
        target_arrow_open,
    ));
    let link_view = new_umlclass_dependency_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlclass_dependency_view<P: UmlClassProfile>(
    model: ERef<UmlClassDependency>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView<P>,
    target: UmlClassElementView<P>,
) -> ERef<DependencyViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.source.uuid()), *m.target.uuid(), target.min_shape(), center_point);

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

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>> for UmlClassDependencyAdapter<P> {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        self.temporaries.midpoint_label.clone()
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
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        ui.label("Stereotype:");
        if self.temporaries.stereotype_controller.show(ui) {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::StereotypeChange(self.temporaries.stereotype_controller.get()),
            ]));
        }

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ]));
        }
        ui.separator();

        ui.label("Target arrow open:");
        if ui.checkbox(&mut self.temporaries.target_arrow_open_buffer, "").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::DependencyArrowOpenChange(
                    self.temporaries.target_arrow_open_buffer,
                ),
            ]));
        }
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ]));
        }
        ui.separator();

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    UmlClassPropChange::StereotypeChange(stereotype) => {
                        if !self.temporaries.stereotype_controller.is_valid(&stereotype) {
                            continue;
                        }

                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::StereotypeChange(
                                model.stereotype.clone(),
                            )],
                        ));
                        model.stereotype = stereotype.clone();
                    }
                    UmlClassPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::StereotypeChange(
                                model.name.clone(),
                            )],
                        ));
                        model.name = name.clone();
                    }
                    UmlClassPropChange::DependencyArrowOpenChange(open) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::DependencyArrowOpenChange(
                                model.target_arrow_open,
                            )],
                        ));
                        model.target_arrow_open = *open;
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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

        fn ah(
            n: UmlClassAssociationNavigability,
            a: UmlClassAssociationAggregation,
        ) -> canvas::ArrowheadType {
            match a {
                UmlClassAssociationAggregation::None => match n {
                    UmlClassAssociationNavigability::Unspecified => canvas::ArrowheadType::None,
                    UmlClassAssociationNavigability::NonNavigable => canvas::ArrowheadType::None,
                    UmlClassAssociationNavigability::Navigable => canvas::ArrowheadType::OpenTriangle,
                }
                UmlClassAssociationAggregation::Shared => canvas::ArrowheadType::EmptyRhombus,
                UmlClassAssociationAggregation::Composite => canvas::ArrowheadType::FullRhombus,
            }
        }
        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.uuid()),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None)
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData::new_labelless(
                canvas::LineType::Dashed,
                if model.target_arrow_open {
                    canvas::ArrowheadType::OpenTriangle
                } else {
                    canvas::ArrowheadType::EmptyTriangle
                }
            )
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.midpoint_label = stereotype_name_format(&*model.stereotype, &*model.name);
        self.temporaries.stereotype_controller.refresh(&*model.stereotype);
        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.target_arrow_open_buffer = model.target_arrow_open;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UmlClassDependency(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.source.uuid();
        if let Some(new_source) = m.get(&source_uuid).and_then(|e| e.as_classifier()) {
            model.source = new_source;
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_classifier()) {
            model.target = new_target;
        }
    }
}


pub fn new_umlclass_association<P: UmlClassProfile>(
    stereotype: &str,
    name: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (UmlClassClassifier, UmlClassElementView<P>),
    target: (UmlClassClassifier, UmlClassElementView<P>),
) -> (ERef<UmlClassAssociation>, ERef<AssociationViewT<P>>) {
    let link_model = ERef::new(UmlClassAssociation::new(
        ModelUuid::now_v7(),
        stereotype.to_owned(),
        name.to_owned(),
        source.0,
        target.0,
    ));
    let link_view = new_umlclass_association_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlclass_association_view<P: UmlClassProfile>(
    model: ERef<UmlClassAssociation>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView<P>,
    target: UmlClassElementView<P>,
) -> ERef<AssociationViewT<P>> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.source.uuid()), *m.target.uuid(), target.min_shape(), center_point);

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

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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

impl<P: UmlClassProfile> MulticonnectionAdapter<UmlClassDomain<P>> for UmlClassAssocationAdapter<P> {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        self.temporaries.midpoint_label.clone()
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
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        ui.label("Stereotype:");
        if self.temporaries.stereotype_controller.show(ui) {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::StereotypeChange(self.temporaries.stereotype_controller.get()),
            ]));
        }

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(self.temporaries.name_buffer.clone())),
            ]));
        }
        ui.separator();

        ui.label("Source multiplicity:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.source_multiplicity_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::LinkMultiplicityChange(false, Arc::new(
                    self.temporaries.source_multiplicity_buffer.clone(),
                )),
            ]));
        }
        ui.label("Source role:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.source_role_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::LinkRoleChange(false, Arc::new(
                    self.temporaries.source_role_buffer.clone(),
                )),
            ]));
        }
        ui.label("Source reading:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.source_reading_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::LinkReadingChange(false, Arc::new(
                    self.temporaries.source_reading_buffer.clone(),
                )),
            ]));
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
                        .selectable_value(&mut self.temporaries.source_navigability_buffer, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkNavigabilityChange(false, self.temporaries.source_navigability_buffer),
                        ]));
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
                        .selectable_value(&mut self.temporaries.source_aggregation_buffer, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkAggregationChange(false, self.temporaries.source_aggregation_buffer),
                        ]));
                    }
                }
            });
        ui.separator();

        ui.label("Target multiplicity:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.target_multiplicity_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::LinkMultiplicityChange(true, Arc::new(
                    self.temporaries.target_multiplicity_buffer.clone(),
                )),
            ]));
        }
        ui.label("Target role:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.target_role_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::LinkRoleChange(true, Arc::new(
                    self.temporaries.target_role_buffer.clone(),
                )),
            ]));
        }
        ui.label("Target reading:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.target_reading_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::LinkReadingChange(true, Arc::new(
                    self.temporaries.target_reading_buffer.clone(),
                )),
            ]));
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
                        .selectable_value(&mut self.temporaries.target_navigability_buffer, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkNavigabilityChange(true, self.temporaries.target_navigability_buffer),
                        ]));
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
                        .selectable_value(&mut self.temporaries.target_aggregation_buffer, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkAggregationChange(true, self.temporaries.target_aggregation_buffer),
                        ]));
                    }
                }
            });
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ]));
        }
        ui.separator();

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    UmlClassPropChange::StereotypeChange(stereotype) => {
                        if !self.temporaries.stereotype_controller.is_valid(&stereotype) {
                            continue;
                        }

                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::StereotypeChange(
                                model.stereotype.clone(),
                            )],
                        ));
                        model.stereotype = stereotype.clone();
                    }
                    UmlClassPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::StereotypeChange(
                                model.name.clone(),
                            )],
                        ));
                        model.name = name.clone();
                    }
                    UmlClassPropChange::LinkMultiplicityChange(t, multiplicity) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::LinkMultiplicityChange(
                                *t,
                                if !t {
                                    model.source_label_multiplicity.clone()
                                } else {
                                    model.target_label_multiplicity.clone()
                                }
                            )],
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
                            vec![UmlClassPropChange::LinkRoleChange(
                                *t,
                                if !t {
                                    model.source_label_role.clone()
                                } else {
                                    model.target_label_role.clone()
                                }
                            )],
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
                            vec![UmlClassPropChange::LinkRoleChange(
                                *t,
                                if !t {
                                    model.source_label_reading.clone()
                                } else {
                                    model.target_label_reading.clone()
                                }
                            )],
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
                            vec![UmlClassPropChange::LinkNavigabilityChange(
                                *t,
                                if !*t {
                                    model.source_navigability
                                } else {
                                    model.target_navigability
                                })],
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
                            vec![UmlClassPropChange::LinkAggregationChange(
                                *t,
                                if !*t {
                                    model.source_aggregation
                                } else {
                                    model.target_aggregation
                                })],
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
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
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

        fn ah(
            n: UmlClassAssociationNavigability,
            a: UmlClassAssociationAggregation,
        ) -> canvas::ArrowheadType {
            match a {
                UmlClassAssociationAggregation::None => match n {
                    UmlClassAssociationNavigability::Unspecified => canvas::ArrowheadType::None,
                    UmlClassAssociationNavigability::NonNavigable => canvas::ArrowheadType::None,
                    UmlClassAssociationNavigability::Navigable => canvas::ArrowheadType::OpenTriangle,
                }
                UmlClassAssociationAggregation::Shared => canvas::ArrowheadType::EmptyRhombus,
                UmlClassAssociationAggregation::Composite => canvas::ArrowheadType::FullRhombus,
            }
        }
        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert((false, *model.source.uuid()), ArrowData {
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
        });
        self.temporaries.arrow_data.insert((true, *model.target.uuid()), ArrowData {
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
        });

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.midpoint_label = stereotype_name_format(&*model.stereotype, &*model.name);
        self.temporaries.stereotype_controller.refresh(&*model.stereotype);
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
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UmlClassAssociation(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.source.uuid();
        if let Some(new_source) = m.get(&source_uuid).and_then(|e| e.as_classifier()) {
            model.source = new_source;
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_classifier()) {
            model.target = new_target;
        }
    }
}


pub fn new_umlclass_comment<P: UmlClassProfile>(
    text: &str,
    position: egui::Pos2,
) -> (ERef<UmlClassComment>, ERef<UmlClassCommentView<P>>) {
    let comment_model = ERef::new(UmlClassComment::new(
        ModelUuid::now_v7(),
        text.to_owned(),
    ));
    let comment_view = new_umlclass_comment_view(comment_model.clone(), position);

    (comment_model, comment_view)
}
pub fn new_umlclass_comment_view<P: UmlClassProfile>(
    model: ERef<UmlClassComment>,
    position: egui::Pos2,
) -> ERef<UmlClassCommentView<P>> {
    let m = model.read();
    ERef::new(UmlClassCommentView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        text_buffer: (*m.text).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
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
    text_buffer: String,

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

impl<P: UmlClassProfile> ContainerGen2<UmlClassDomain<P>> for UmlClassCommentView<P> {}

impl<P: UmlClassProfile> ElementControllerGen2<UmlClassDomain<P>> for UmlClassCommentView<P> {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        _q: &UmlClassQueryable<P>,
        _lp: &UmlClassLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Text:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.text_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
        _: &UmlClassQueryable<P>,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool<P>)>,
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
        tool: &mut Option<NaiveUmlClassTool<P>>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
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
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            UmlClassPropChange::NameChange(text) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::NameChange(model.text.clone())],
                                ));
                                model.text = text.clone();
                            }
                            UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                                ));
                                self.background_color = *color;
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
        self.text_buffer = (*model.text).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView<P>>,
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

        let modelish = if let Some(UmlClassElement::UmlClassComment(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlClassComment::new(model_uuid, (*old_model.text).clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            text_buffer: self.text_buffer.clone(),
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
    let link_view = new_umlclass_commentlink_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
pub fn new_umlclass_commentlink_view<P: UmlClassProfile>(
    model: ERef<UmlClassCommentLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView<P>,
    target: UmlClassElementView<P>,
) -> ERef<CommentLinkViewT<P>> {
    let m = model.read();
    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        UmlClassCommentLinkAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new(source)],
        vec![Ending::new(target)],
        center_point,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>
    ) -> PropertiesStatus<UmlClassDomain<P>> {
        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex<P>, UmlClassPropChange>>,
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
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UmlClassCommentLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlClassCommentLink::new(new_uuid, old_model.source.clone(), old_model.target.clone()));
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
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.source.read().uuid();
        if let Some(UmlClassElement::UmlClassComment(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }
        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}
