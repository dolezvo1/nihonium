use super::super::{
    umlclass::umlclass_controllers::UmlClassLabelProvider,
    umlclass::umlclass_models::{
        UmlClass, UmlClassAssociation, UmlClassComment, UmlClassCommentLink, UmlClassDiagram, UmlClassElement, UmlClassGeneralization, UmlClassPackage
    },
};
use crate::{common::{canvas::{self, Highlight, NHCanvas, NHShape}, controller::LabelProvider}, domains::umlclass::umlclass_models::{UmlClassAssociationAggregation, UmlClassAssociationNavigability}};
use crate::common::controller::{
    ColorBundle, ColorChangeData, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, GlobalDrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, InputEvent, InsensitiveCommand, MGlobalColor, Model, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, View
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{self, ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::{CustomModal, CustomModalResult, CustomTab};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct UmlClassDomain;
impl Domain for UmlClassDomain {
    type CommonElementT = UmlClassElement;
    type DiagramModelT = UmlClassDiagram;
    type CommonElementViewT = UmlClassElementView;
    type QueryableT<'a> = UmlClassQueryable<'a>;
    type LabelProviderT = UmlClassLabelProvider;
    type ToolT = NaiveUmlClassTool;
    type AddCommandElementT = UmlClassElementOrVertex;
    type PropChangeT = UmlClassPropChange;
}

type PackageViewT = PackageView<UmlClassDomain, UmlClassPackageAdapter>;
type GeneralizationViewT = MulticonnectionView<UmlClassDomain, UmlClassGeneralizationAdapter>;
type AssociationViewT = MulticonnectionView<UmlClassDomain, UmlClassAssociationAdapter>;
type CommentLinkViewT = MulticonnectionView<UmlClassDomain, UmlClassCommentLinkAdapter>;

pub struct UmlClassQueryable<'a> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, UmlClassElementView>,
}

impl<'a> Queryable<'a, UmlClassDomain> for UmlClassQueryable<'a> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, UmlClassElementView>,
    ) -> Self {
        Self { models_to_views, flattened_views }
    }

    fn get_view(&self, m: &ModelUuid) -> Option<UmlClassElementView> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).cloned()
    }
}

#[derive(Clone)]
pub enum UmlClassPropChange {
    StereotypeChange(Arc<String>),

    NameChange(Arc<String>),
    ClassAbstractChange(bool),
    ClassPropertiesChange(Arc<String>),
    ClassFunctionsChange(Arc<String>),

    SetNameChange(Arc<String>),
    SetCoveringChange(bool),
    SetDisjointChange(bool),

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
pub enum UmlClassElementOrVertex {
    Element(UmlClassElementView),
    Vertex(VertexInformation),
}

impl Debug for UmlClassElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassElementOrVertex::???")
    }
}

impl TryFrom<UmlClassElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: UmlClassElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlClassElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<UmlClassElementOrVertex> for UmlClassElementView {
    type Error = ();

    fn try_from(value: UmlClassElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            UmlClassElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}


#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = UmlClassDomain)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlClassElementView {
    Package(ERef<PackageViewT>),
    Class(ERef<UmlClassView>),
    Generalization(ERef<GeneralizationViewT>),
    Association(ERef<AssociationViewT>),
    Comment(ERef<UmlClassCommentView>),
    CommentLink(ERef<CommentLinkViewT>),
}


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: UmlClassDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    placeholders: UmlClassPlaceholderViews,
}

#[derive(Clone, Default)]
struct UmlClassDiagramBuffer {
    name: String,
    comment: String,
}

#[derive(Clone)]
struct UmlClassPlaceholderViews {
    views: [UmlClassElementView; 12],
}

impl Default for UmlClassPlaceholderViews {
    fn default() -> Self {
        let (kind, kind_view) = new_umlclass_class("kind", "Animal", false, "", "", egui::Pos2::ZERO);
        let kind = (kind, kind_view.into());
        let (subkind, subkind_view) = new_umlclass_class("subkind", "Human", false, "", "", egui::Pos2::new(100.0, 75.0));
        let subkind = (subkind, subkind_view.into());
        let (phase, phase_view) = new_umlclass_class("phase", "Adult", false, "", "", egui::Pos2::ZERO);
        let phase = (phase, phase_view.into());
        let (role, role_view) = new_umlclass_class("role", "Customer", false, "", "", egui::Pos2::ZERO);
        let role = (role, role_view.into());

        let (_gen, gen_view) = new_umlclass_generalization(None, kind.clone(), subkind.clone());
        let (_assoc, assoc_view) = new_umlclass_association("", None, kind.clone(), subkind.clone());
        let (_mediation, mediation_view) = new_umlclass_association("mediation", None, kind.clone(), subkind.clone());
        let (_char, char_view) = new_umlclass_association("characterization", None, kind.clone(), subkind.clone());
        let (_comp, comp_view) = new_umlclass_association("componentOf", None, kind.clone(), subkind.clone());

        let (_package, package_view) = new_umlclass_package("a package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });
        let (comment, comment_view) = new_umlclass_comment("a comment", egui::Pos2::new(-100.0, -75.0));
        let comment = (comment, comment_view.into());
        let commentlink = new_umlclass_commentlink(None, comment.clone(), (kind.0.into(), kind.1.clone()));

        Self {
            views: [
                kind.1,
                subkind.1,
                phase.1,
                role.1,

                gen_view.into(),
                assoc_view.into(),
                mediation_view.into(),
                char_view.into(),
                comp_view.into(),

                package_view.into(),
                comment.1,
                commentlink.1.into(),
            ]
        }
    }
}

impl UmlClassDiagramAdapter {
    fn new(model: ERef<UmlClassDiagram>) -> Self {
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

impl DiagramAdapter<UmlClassDomain> for UmlClassDiagramAdapter {
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
        "ontouml-diagram-view"
    }

    fn create_new_view_for(
        &self,
        q: &UmlClassQueryable<'_>,
        element: UmlClassElement,
    ) -> Result<UmlClassElementView, HashSet<ModelUuid>> {
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
                unreachable!("hopefully")
            }
            UmlClassElement::UmlClass(inner) => {
                UmlClassElementView::from(
                    new_umlclass_class_view(inner, egui::Pos2::ZERO)
                )
            },
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
                unreachable!("not sure yet")
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
    ) -> PropertiesStatus<UmlClassDomain> {
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
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
        tool: &mut Option<NaiveUmlClassTool>,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) {
        let button_height = 60.0;
        let width = ui.available_width();

        let stage = tool.as_ref().map(|e| e.initial_stage());
        let c = |s: UmlClassToolStage| -> egui::Color32 {
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
        let empty_q = UmlClassQueryable::new(&empty_a, &empty_b);
        let mut icon_counter = 0;
        for cat in [
            &[
                (UmlClassToolStage::Class { class_type: "kind" }, "Kind"),
                (UmlClassToolStage::Class { class_type: "subkind" }, "Subkind"),
                (UmlClassToolStage::Class { class_type: "phase" }, "Phase"),
                (UmlClassToolStage::Class { class_type: "role" }, "Role"),
            ][..],
            &[
                (
                    UmlClassToolStage::LinkStart {
                        link_stereotype: None,
                    },
                    "Generalization (Set)",
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_stereotype: Some(""),
                    },
                    "Association",
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_stereotype: Some("mediation"),
                    },
                    "Mediation",
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_stereotype: Some("characterization"),
                    },
                    "Characterization",
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_stereotype: Some("componentOf"),
                    },
                    "ComponentOf",
                ),
            ][..],
            &[
                (UmlClassToolStage::PackageStart, "Package"),
                (UmlClassToolStage::Comment, "Comment"),
                (UmlClassToolStage::CommentLinkStart, "Comment Link"),
            ][..],
        ] {
            for (stage, name) in cat {
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

    fn menubar_options_fun(
        &self,
        view_uuid: &ViewUuid,
        label_provider: &ERef<dyn LabelProvider>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        if ui.button("PlantUML description").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(PlantUmlTab {
                    diagram: self.model.clone(),
                    plantuml_description: "".to_owned(),
                })),
            ));
        }
        if ui.button("OntoUML Validations").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(super::ontouml_validations::OntoUMLValidationTab::new(self.model.clone(), label_provider.clone(), *view_uuid))),
            ));
        }
        ui.separator();
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let (new_model, models) = super::super::umlclass::umlclass_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let models = super::super::umlclass::umlclass_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }
}

struct PlantUmlTab {
    diagram: ERef<UmlClassDiagram>,
    plantuml_description: String,
}

impl CustomTab for PlantUmlTab {
    fn title(&self) -> String {
        "PlantUML description".to_owned()
    }

    fn show(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
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
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New OntoUML diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        UmlClassDiagramAdapter::new(diagram.clone()),
        Vec::new(),
    )
}

pub fn demo(no: u32) -> ERef<dyn DiagramController> {
    let (animal_model, animal_view) = new_umlclass_class("kind", "Animal", false, "", "", egui::Pos2::new(150.0, 200.0));
    let (human_model, human_view) = new_umlclass_class("subkind", "Human", false, "", "", egui::Pos2::new(150.0, 350.0));
    let (alive_model, alive_view) = new_umlclass_class("phase", "Alive", false, "", "", egui::Pos2::new(350.0, 160.0));
    let (dead_model, dead_view) = new_umlclass_class("phase", "Dead", false, "", "", egui::Pos2::new(350.0, 250.0));
    let (marriage_model, marriage_view) = new_umlclass_class("relator", "Marriage", false, "", "", egui::Pos2::new(350.0, 350.0));

    let (gen_phase_model, gen_phase_view) = new_umlclass_generalization(
        Some((uuid::Uuid::now_v7().into(), egui::Pos2::new(280.0, 200.0))),
        (alive_model.clone(), alive_view.clone().into()),
        (animal_model.clone(), animal_view.clone().into()),
    );
    gen_phase_model.write().set_is_covering = true;
    gen_phase_model.write().set_is_disjoint = true;
    let gen_uuid = *gen_phase_view.read().uuid();
    gen_phase_view.write().apply_command(
        &InsensitiveCommand::AddDependency(gen_uuid, 0, UmlClassElementOrVertex::Element(dead_view.clone().into()), true),
        &mut Vec::new(),
        &mut HashSet::new(),
    );

    let (gen_human_model, gen_human_view) = new_umlclass_generalization(
        None,
        (human_model.clone(), human_view.clone().into()),
        (animal_model.clone(), animal_view.clone().into()),
    );

    let (mediation_model, mediation_view) = new_umlclass_association(
        "mediation", None,
        (human_model.clone(), human_view.clone().into()),
        (marriage_model.clone(), marriage_view.clone().into()),
    );
    mediation_model.write().source_label_multiplicity = Arc::new("2..*".to_owned());
    mediation_model.write().target_label_multiplicity = Arc::new("1..1".to_owned());

    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("Demo OntoUML diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        model_uuid,
        name.clone(),
        vec![
            animal_model.into(),
            human_model.into(),
            alive_model.into(),
            dead_model.into(),
            marriage_model.into(),
            gen_phase_model.into(),
            gen_human_model.into(),
            mediation_model.into(),
        ],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        UmlClassDiagramAdapter::new(diagram.clone()),
        vec![
            animal_view.into(),
            human_view.into(),
            alive_view.into(),
            dead_view.into(),
            marriage_view.into(),
            gen_phase_view.into(),
            gen_human_view.into(),
            mediation_view.into(),
        ],
    )
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<DiagramControllerGen2<UmlClassDomain, UmlClassDiagramAdapter>>(&uuid)?)
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Class { class_type: &'static str },
    LinkStart { link_stereotype: Option<&'static str> },
    LinkEnd,
    LinkAddEnding { source: bool },
    PackageStart,
    PackageEnd,
    Comment,
    CommentLinkStart,
    CommentLinkEnd,
}

enum PartialUmlClassElement {
    None,
    Some(UmlClassElementView),
    Link {
        link_stereotype: Option<&'static str>,
        source: ERef<UmlClass>,
        dest: Option<ERef<UmlClass>>,
    },
    LinkEnding {
        source: bool,
        gen_model: ERef<UmlClassGeneralization>,
        new_model: Option<ModelUuid>,
    },
    Package {
        a: egui::Pos2,
        a_display: egui::Pos2,
        b: Option<egui::Pos2>,
    },
    CommentLink {
        source: ERef<UmlClassComment>,
        dest: Option<UmlClassElement>,
    },
}

pub struct NaiveUmlClassTool {
    initial_stage: UmlClassToolStage,
    current_stage: UmlClassToolStage,
    result: PartialUmlClassElement,
    event_lock: bool,
}

impl NaiveUmlClassTool {
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

impl Tool<UmlClassDomain> for NaiveUmlClassTool {
    type Stage = UmlClassToolStage;

    fn initial_stage(&self) -> Self::Stage {
        self.initial_stage
    }

    fn targetting_for_element(&self, element: Option<UmlClassElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                UmlClassToolStage::Class {..}
                | UmlClassToolStage::Comment
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkStart
                | UmlClassToolStage::CommentLinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClassPackage(..)) => match self.current_stage {
                UmlClassToolStage::Class {..}
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkStart => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClassInstance(..)) => NON_TARGETTABLE_COLOR,
            Some(UmlClassElement::UmlClass(..)) => match self.current_stage {
                UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::Class {..}
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkStart => NON_TARGETTABLE_COLOR,
            },
            Some(UmlClassElement::UmlClassComment(..)) => match self.current_stage {
                UmlClassToolStage::CommentLinkStart => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::LinkStart { .. }
                | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::LinkAddEnding { .. }
                | UmlClassToolStage::Class {..}
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
    fn draw_status_hint(&self, q: &UmlClassQueryable,  canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialUmlClassElement::Link {
                source,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.read().uuid()) {
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
            PartialUmlClassElement::Package { a_display, .. } => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(*a_display, pos),
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
            (UmlClassToolStage::Class { class_type }, _) => {
                let name = match class_type {
                    "kind" => "Animal",
                    "subkind" => "Human",
                    "phase" => "Adult",
                    "role" => "Customer",
                    _ => "a class",
                };

                let (_class_model, class_view) =
                    new_umlclass_class(class_type, name, false, "", "", pos);
                self.result = PartialUmlClassElement::Some(class_view.into());
                self.event_lock = true;
            }
            (UmlClassToolStage::PackageStart, _) => {
                self.result = PartialUmlClassElement::Package {
                    a: pos,
                    a_display: pos,
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
    fn add_element<'a>(&mut self, element: UmlClassElement) {
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
            UmlClassElement::UmlClassInstance(..) => {}
            UmlClassElement::UmlClass(inner) => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::LinkStart { link_stereotype }, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::Link {
                            link_stereotype,
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

    fn try_additional_dependency(&mut self) -> Option<(u8, ModelUuid, ModelUuid)> {
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
        into: &dyn ContainerGen2<UmlClassDomain>,
    ) -> Option<(UmlClassElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                let esm: Option<Box<dyn CustomModal>> = match &x {
                    UmlClassElementView::Class(inner) => Some(Box::new(UmlClassSetupModal::from(&inner.read().model))),
                    _ => None,
                };
                self.result = PartialUmlClassElement::None;
                Some((x, esm))
            }
            PartialUmlClassElement::Link {
                link_stereotype,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = UmlClassToolStage::LinkStart {
                        link_stereotype: *link_stereotype,
                    };

                    let link_view = if let Some(link_stereotype) = link_stereotype {
                        new_umlclass_association(
                            *link_stereotype,
                            None,
                            (source.clone(), source_controller),
                            (dest.clone(), dest_controller),
                        ).1.into()
                    } else {
                        new_umlclass_generalization(
                            None,
                            (source.clone(), source_controller),
                            (dest.clone(), dest_controller),
                        ).1.into()
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
pub struct UmlClassPackageAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassPackage>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<UmlClassDomain> for UmlClassPackageAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }

    fn add_element(&mut self, element: UmlClassElement) {
        self.model.write().add_element(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.model.write().delete_elements(uuids);
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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

        Self { model, name_buffer: self.name_buffer.clone(), comment_buffer: self.comment_buffer.clone() }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        todo!()
    }
}

fn new_umlclass_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (ERef<UmlClassPackage>, ERef<PackageViewT>) {
    let package_model = ERef::new(UmlClassPackage::new(
        uuid::Uuid::now_v7().into(),
        name.to_owned(),
        Vec::new(),
    ));
    let package_view = new_umlclass_package_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
fn new_umlclass_package_view(
    model: ERef<UmlClassPackage>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT> {
    let m = model.read();
    PackageView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        UmlClassPackageAdapter {
            model: model.clone(),
            name_buffer: (*m.name).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}


fn new_umlclass_class(
    stereotype: &str,
    name: &str,
    is_abstract: bool,
    properties: &str,
    functions: &str,
    position: egui::Pos2,
) -> (ERef<UmlClass>, ERef<UmlClassView>) {
    let class_model = ERef::new(UmlClass::new(
        uuid::Uuid::now_v7().into(),
        stereotype.to_owned(),
        name.to_owned(),
        is_abstract,
        properties.to_owned(),
        functions.to_owned(),
    ));
    let class_view = new_umlclass_class_view(class_model.clone(), position);

    (class_model, class_view)
}
fn new_umlclass_class_view(
    model: ERef<UmlClass>,
    position: egui::Pos2,
) -> ERef<UmlClassView> {
    let m = model.read();
    ERef::new(UmlClassView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),

        stereotype_buffer: ontouml_class_stereotype_literal(&*m.stereotype),
        name_buffer: (*m.name).clone(),
        is_abstract_buffer: m.is_abstract,
        properties_buffer: (*m.properties).clone(),
        functions_buffer: (*m.functions).clone(),
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
    })
}
fn ontouml_class_stereotype_literal(e: &str) -> &'static str {
    match e {
        // Sortals
        "kind" => "kind",
        "subkind" => "subkind",
        "phase" => "phase",
        "role" => "role",
        "collective" => "collective",
        "quantity" => "quantity",
        "relator" => "relator",
        // Nonsortals
        "category" => "category",
        "phaseMixin" => "phaseMixin",
        "roleMixin" => "roleMixin",
        "mixin" => "mixin",
        // Aspects
        "mode" => "mode",
        "quality" => "quality",
        _ => unreachable!(),
    }
}

struct UmlClassSetupModal {
    model: ERef<UmlClass>,

    stereotype_buffer: &'static str,
    name_buffer: String,
}

impl From<&ERef<UmlClass>> for UmlClassSetupModal {
    fn from(model: &ERef<UmlClass>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            stereotype_buffer: ontouml_class_stereotype_literal(&*m.stereotype),
            name_buffer: (*m.name).clone(),
        }
    }
}

impl CustomModal for UmlClassSetupModal {
    fn show(
        &mut self,
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Stereotype:");
        egui::ComboBox::from_id_salt("Stereotype:")
            .selected_text(format!("«{}»", self.stereotype_buffer))
            .show_ui(ui, |ui| {
                for e in [
                    // Sortals
                    ("kind", "Kind"),
                    ("subkind", "Subkind"),
                    ("phase", "Phase"),
                    ("role", "Role"),
                    ("collective", "Collective"),
                    ("quantity", "Quantity"),
                    ("relator", "Relator"),
                    // Nonsortals
                    ("category", "Category"),
                    ("phaseMixin", "PhaseMixin"),
                    ("roleMixin", "RoleMixin"),
                    ("mixin", "Mixin"),
                    // Aspects
                    ("mode", "Mode"),
                    ("quality", "Quality"),
                ] {
                    ui.selectable_value(&mut self.stereotype_buffer, e.0, e.1);
                }
            }
        );
        ui.label("Name:");
        ui.text_edit_singleline(&mut self.name_buffer);
        ui.separator();

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.stereotype = Arc::new(self.stereotype_buffer.to_owned());
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
pub struct UmlClassView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClass>,

    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: &'static str,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    is_abstract_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    properties_buffer: String,
    #[nh_context_serde(skip_and_default)]
    functions_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl UmlClassView {
    fn association_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_radius = 8.0;
        let b_center = self.bounds_rect.right_top() + egui::Vec2::splat(b_radius / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * b_radius / ui_scale),
        )
    }
}

impl Entity for UmlClassView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlClassView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlClassElement> for UmlClassView {
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

impl ContainerGen2<UmlClassDomain> for UmlClassView {}

impl ElementControllerGen2<UmlClassDomain> for UmlClassView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        _q: &UmlClassQueryable,
        _lp: &UmlClassLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> PropertiesStatus<UmlClassDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Stereotype:");
        egui::ComboBox::from_id_salt("Stereotype:")
            .selected_text(format!("«{}»", self.stereotype_buffer))
            .show_ui(ui, |ui| {
                for e in [
                    // Sortals
                    ("kind", "Kind"),
                    ("subkind", "Subkind"),
                    ("phase", "Phase"),
                    ("role", "Role"),
                    ("collective", "Collective"),
                    ("quantity", "Quantity"),
                    ("relator", "Relator"),
                    // Nonsortals
                    ("category", "Category"),
                    ("phaseMixin", "PhaseMixin"),
                    ("roleMixin", "RoleMixin"),
                    ("mixin", "Mixin"),
                    // Aspects
                    ("mode", "Mode"),
                    ("quality", "Quality"),
                ] {
                    if ui.selectable_value(&mut self.stereotype_buffer, e.0, e.1).changed() {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::StereotypeChange(Arc::new(self.stereotype_buffer.to_owned())),
                        ]));
                    }
                }
            }
        );

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

        if ui.checkbox(&mut self.is_abstract_buffer, "isAbstract").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::ClassAbstractChange(self.is_abstract_buffer),
            ]));
        }

        ui.label("Properties:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.properties_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::ClassPropertiesChange(Arc::new(self.properties_buffer.clone())),
            ]));
        }

        ui.label("Functions:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.functions_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::ClassFunctionsChange(Arc::new(self.functions_buffer.clone())),
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

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &UmlClassQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
    ) -> TargettingStatus {
        let read = self.model.read();

        let stereotype_guillemets = format!("«{}»", read.stereotype);

        self.bounds_rect = canvas.draw_class(
            self.position,
            if read.stereotype.is_empty() {
                None
            } else {
                Some(&stereotype_guillemets)
            },
            &read.name,
            None,
            &[&read.parse_properties(), &read.parse_functions()],
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

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
                    t.targetting_for_element(Some(self.model())),
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
        tool: &mut Option<NaiveUmlClassTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
                    initial_stage: UmlClassToolStage::LinkStart {
                        link_stereotype: Some(""),
                    },
                    current_stage: UmlClassToolStage::LinkEnd,
                    result: PartialUmlClassElement::Link {
                        link_stereotype: Some(""),
                        source: self.model.clone().into(),
                        dest: None,
                    },
                    event_lock: true,
                });

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_element(self.model());
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
            | InsensitiveCommand::AddElement(..)
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
                            UmlClassPropChange::ClassAbstractChange(is_abstract) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ClassAbstractChange(
                                        model.is_abstract,
                                    )],
                                ));
                                model.is_abstract = *is_abstract;
                            }
                            UmlClassPropChange::ClassPropertiesChange(properties) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ClassPropertiesChange(
                                        model.properties.clone(),
                                    )],
                                ));
                                model.properties = properties.clone();
                            }
                            UmlClassPropChange::ClassFunctionsChange(functions) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::ClassFunctionsChange(
                                        model.functions.clone(),
                                    )],
                                ));
                                model.functions = functions.clone();
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
        self.stereotype_buffer = ontouml_class_stereotype_literal(&*model.stereotype);
        self.name_buffer = (*model.name).clone();
        self.is_abstract_buffer = model.is_abstract;
        self.properties_buffer = (*model.properties).clone();
        self.functions_buffer = (*model.functions).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView>,
        c: &mut HashMap<ViewUuid, UmlClassElementView>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
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

        let mut cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            stereotype_buffer: self.stereotype_buffer,
            name_buffer: self.name_buffer.clone(),
            is_abstract_buffer: self.is_abstract_buffer,
            properties_buffer: self.properties_buffer.clone(),
            functions_buffer: self.functions_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn ontouml_link_stereotype_literal(e: &str) -> &'static str {
    match e {
        "" => "",
        "formal" => "formal",
        "mediation" => "mediation",
        "characterization" => "characterization",
        "structuration" => "Structuration",

        "componentOf" => "componentOf",
        "containment" => "containment",
        "memberOf" => "memberOf",
        "subcollectionOf" => "subcollectionOf",
        "subquantityOf" => "subquantityOf",
        _ => unreachable!(),
    }
}


fn new_umlclass_generalization(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClass>, UmlClassElementView),
    target: (ERef<UmlClass>, UmlClassElementView),
) -> (ERef<UmlClassGeneralization>, ERef<GeneralizationViewT>) {
    let link_model = ERef::new(UmlClassGeneralization::new(
        uuid::Uuid::now_v7().into(),
        vec![source.0],
        vec![target.0],
    ));
    let link_view = new_umlclass_generalization_view(link_model.clone(), center_point, vec![source.1], vec![target.1]);
    (link_model, link_view)
}
fn new_umlclass_generalization_view(
    model: ERef<UmlClassGeneralization>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    sources: Vec<UmlClassElementView>,
    targets: Vec<UmlClassElementView>,
) -> ERef<GeneralizationViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(m.sources.iter().map(|e| *e.read().uuid), *m.targets[0].read().uuid, targets[0].min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
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
    arrow_data: HashMap<ModelUuid, ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    set_name_buffer: String,
    set_is_covering_buffer: bool,
    set_is_disjoint_buffer: bool,
    comment_buffer: String,
}

impl MulticonnectionAdapter<UmlClassDomain> for UmlClassGeneralizationAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        self.temporaries.midpoint_label.clone()
    }

    fn arrow_data(&self) -> &HashMap<ModelUuid, ArrowData> {
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
    fn push_source(&mut self, e: <UmlClassDomain as Domain>::CommonElementT) -> Result<(), ()> {
        if let UmlClassElement::UmlClass(c) = e {
            self.model.write().sources.push(c);
            Ok(())
        } else {
            Err(())
        }
    }
    fn remove_source(&mut self, uuid: &ModelUuid) -> Result<(), ()> {
        let mut w = self.model.write();
        if w.sources.len() == 1 {
            return Err(())
        }
        let original_count = w.sources.len();
        w.sources.retain(|e| *uuid != *e.read().uuid);
        if w.sources.len() != original_count {
            Ok(())
        } else {
            Err(())
        }
    }
    fn push_target(&mut self, e: <UmlClassDomain as Domain>::CommonElementT) -> Result<(), ()> {
        if let UmlClassElement::UmlClass(c) = e {
            self.model.write().targets.push(c);
            Ok(())
        } else {
            Err(())
        }
    }
    fn remove_target(&mut self, uuid: &ModelUuid) -> Result<(), ()> {
        let mut w = self.model.write();
        if w.targets.len() == 1 {
            return Err(())
        }
        let original_count = w.targets.len();
        w.targets.retain(|e| *uuid != *e.read().uuid);
        if w.targets.len() != original_count {
            Ok(())
        } else {
            Err(())
        }
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) -> PropertiesStatus<UmlClassDomain> {
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

        if ui.button("Switch source and target").clicked() {
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
            self.temporaries.arrow_data.insert(*e.read().uuid, ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::None));
        }
        for e in &model.targets {
            self.temporaries.arrow_data.insert(*e.read().uuid, ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::EmptyTriangle));
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


fn new_umlclass_association(
    stereotype: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClass>, UmlClassElementView),
    target: (ERef<UmlClass>, UmlClassElementView),
) -> (ERef<UmlClassAssociation>, ERef<AssociationViewT>) {
    let link_model = ERef::new(UmlClassAssociation::new(
        uuid::Uuid::now_v7().into(),
        stereotype.to_owned(),
        source.0.into(),
        target.0.into(),
    ));
    let link_view = new_umlclass_association_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_umlclass_association_view(
    model: ERef<UmlClassAssociation>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView,
    target: UmlClassElementView,
) -> ERef<AssociationViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.source.uuid()), *m.target.uuid(), target.min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        UmlClassAssociationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassAssociationAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassAssociation>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: UmlClassAssociationTemporaries,
}

#[derive(Clone, Default)]
struct UmlClassAssociationTemporaries {
    arrow_data: HashMap<ModelUuid, ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    stereotype_buffer: &'static str,
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

impl MulticonnectionAdapter<UmlClassDomain> for UmlClassAssociationAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        let r = self.model.read();
        if r.stereotype.is_empty() {
            None
        } else {
            Some(format!("«{}»", r.stereotype).into())
        }
    }

    fn arrow_data(&self) -> &HashMap<ModelUuid, ArrowData> {
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
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) ->PropertiesStatus<UmlClassDomain> {
        ui.label("Association type:");
        egui::ComboBox::from_id_salt("Association type:")
            .selected_text(format!("«{}»", self.temporaries.stereotype_buffer))
            .show_ui(ui, |ui| {
                for sv in [
                    ("formal", "Formal"),
                    ("mediation", "Mediation"),
                    ("characterization", "Characterization"),
                    ("structuration", "Structuration"),

                    ("componentOf", "ComponentOf"),
                    ("containment", "Containment"),
                    ("memberOf", "MemberOf"),
                    ("subcollectionOf", "SubcollectionOf"),
                    ("subquantityOf", "SubquantityOf"),
                ] {
                    if ui
                        .selectable_value(&mut self.temporaries.stereotype_buffer, sv.0, sv.1)
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::StereotypeChange(self.temporaries.stereotype_buffer.to_owned().into()),
                        ]));
                    }
                }
            });
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    UmlClassPropChange::StereotypeChange(stereotype) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::StereotypeChange(
                                model.stereotype.clone(),
                            )],
                        ));
                        model.stereotype = stereotype.clone();
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
        self.temporaries.arrow_data.insert(*model.source.uuid(), ArrowData {
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
        self.temporaries.arrow_data.insert(*model.target.uuid(), ArrowData {
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

        self.temporaries.stereotype_buffer = ontouml_link_stereotype_literal(&*model.stereotype);
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


fn new_umlclass_comment(
    text: &str,
    position: egui::Pos2,
) -> (ERef<UmlClassComment>, ERef<UmlClassCommentView>) {
    let comment_model = ERef::new(UmlClassComment::new(
        uuid::Uuid::now_v7().into(),
        text.to_owned(),
    ));
    let comment_view = new_umlclass_comment_view(comment_model.clone(), position);

    (comment_model, comment_view)
}
fn new_umlclass_comment_view(
    model: ERef<UmlClassComment>,
    position: egui::Pos2,
) -> ERef<UmlClassCommentView> {
    let m = model.read();
    ERef::new(UmlClassCommentView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),

        text_buffer: (*m.text).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct UmlClassCommentView {
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
}

impl Entity for UmlClassCommentView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for UmlClassCommentView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<UmlClassElement> for UmlClassCommentView {
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

impl ContainerGen2<UmlClassDomain> for UmlClassCommentView {}

impl ElementControllerGen2<UmlClassDomain> for UmlClassCommentView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        _q: &UmlClassQueryable,
        _lp: &UmlClassLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> PropertiesStatus<UmlClassDomain> {
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

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &UmlClassQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
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
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_polygon(
            [
                egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y),
            ].into_iter().collect(),
            egui::Color32::WHITE,
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
                    t.targetting_for_element(Some(self.model())),
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
        tool: &mut Option<NaiveUmlClassTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
                    tool.add_element(self.model());
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
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
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
            | InsensitiveCommand::AddElement(..)
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
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView>,
        c: &mut HashMap<ViewUuid, UmlClassElementView>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
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
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}


fn new_umlclass_commentlink(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClassComment>, UmlClassElementView),
    target: (UmlClassElement, UmlClassElementView),
) -> (ERef<UmlClassCommentLink>, ERef<CommentLinkViewT>) {
    let link_model = ERef::new(UmlClassCommentLink::new(
        uuid::Uuid::now_v7().into(),
        source.0,
        target.0,
    ));
    let link_view = new_umlclass_commentlink_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_umlclass_commentlink_view(
    model: ERef<UmlClassCommentLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView,
    target: UmlClassElementView,
) -> ERef<CommentLinkViewT> {
    let m = model.read();
    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
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
    arrow_data: HashMap<ModelUuid, ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
}

impl MulticonnectionAdapter<UmlClassDomain> for UmlClassCommentLinkAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        None
    }

    fn arrow_data(&self) -> &HashMap<ModelUuid, ArrowData> {
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
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) -> PropertiesStatus<UmlClassDomain> {
        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {}
    fn refresh_buffers(&mut self) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(*model.source.read().uuid, ArrowData::new_labelless(
            canvas::LineType::Dashed,
            canvas::ArrowheadType::None,
        ));
        self.temporaries.arrow_data.insert(*model.target.uuid(), ArrowData::new_labelless(
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
