use super::demoofd_models::{
    DemoOfdElement, DemoOfdDiagram, DemoOfdPackage, DemoOfdEntityType, DemoOfdEventType, DemoOfdPropertyType,
};
use crate::common::canvas::{self, Highlight, NHCanvas, NHShape};
use crate::common::controller::{
    CachingLabelDeriver, ColorBundle, ColorChangeData, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider, MGlobalColor, Model, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, View
};
use crate::common::ufoption::UFOption;
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{self, ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::domains::demoofd::demoofd_models::{DemoOfdAggregation, DemoOfdExclusion, DemoOfdPrecedence, DemoOfdSpecialization, DemoOfdType};
use crate::{CustomModal, CustomModalResult, CustomTab};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};
use super::super::demo::{
    INTERNAL_ROLE_BACKGROUND, EXTERNAL_ROLE_BACKGROUND,
    PERFORMA_DETAIL, INFORMA_DETAIL, FORMA_DETAIL,
    DemoTransactionKind,
};

pub struct DemoOfdDomain;
impl Domain for DemoOfdDomain {
    type CommonElementT = DemoOfdElement;
    type DiagramModelT = DemoOfdDiagram;
    type CommonElementViewT = DemoOfdElementView;
    type QueryableT<'a> = DemoOfdQueryable<'a>;
    type LabelProviderT = DemoOfdLabelProvider;
    type ToolT = NaiveDemoOfdTool;
    type AddCommandElementT = DemoOfdElementOrVertex;
    type PropChangeT = DemoOfdPropChange;
}

type PackageViewT = PackageView<DemoOfdDomain, DemoOfdPackageAdapter>;
type PropertyTypeViewT = MulticonnectionView<DemoOfdDomain, DemoOfdPropertyTypeAdapter>;
type SpecializationViewT = MulticonnectionView<DemoOfdDomain, DemoOfdSpecializationAdapter>;
type AggregationViewT = MulticonnectionView<DemoOfdDomain, DemoOfdAggregationAdapter>;
type PrecedenceViewT = MulticonnectionView<DemoOfdDomain, DemoOfdPrecedenceAdapter>;
type ExclusionViewT = MulticonnectionView<DemoOfdDomain, DemoOfdExclusionAdapter>;

pub struct DemoOfdQueryable<'a> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, DemoOfdElementView>,
}

impl<'a> Queryable<'a, DemoOfdDomain> for DemoOfdQueryable<'a> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, DemoOfdElementView>,
    ) -> Self {
        Self { models_to_views, flattened_views }
    }

    fn get_view(&self, m: &ModelUuid) -> Option<DemoOfdElementView> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).cloned()
    }
}

#[derive(Default)]
pub struct DemoOfdLabelProvider {
    cache: HashMap<ModelUuid, Arc<String>>,
}

impl LabelProvider for DemoOfdLabelProvider {
    fn get(&self, uuid: &ModelUuid) -> Arc<String> {
        self.cache.get(uuid).unwrap().clone()
    }
}

impl CachingLabelDeriver<DemoOfdElement> for DemoOfdLabelProvider {
    fn update(&mut self, e: &DemoOfdElement) {
        match e {
            DemoOfdElement::DemoOfdPackage(inner) => {
                self.cache.insert(*inner.read().uuid, Arc::new("Package".to_owned()));
            }
            DemoOfdElement::DemoOfdEntityType(inner) => {
                let r = inner.read();
                self.cache.insert(*r.uuid, r.name.clone());
            },
            DemoOfdElement::DemoOfdEventType(inner) => {
                let r = inner.read();
                self.cache.insert(*r.uuid, r.name.clone());
                // TODO: rework
                if let UFOption::Some(s) = &r.specialization_entity_type {
                    self.update(&s.clone().into());
                }
            },
            DemoOfdElement::DemoOfdPropertyType(inner) => {
                self.cache.insert(*inner.read().uuid, Arc::new("Property".to_owned()));
            },
            DemoOfdElement::DemoOfdSpecialization(inner) => {
                self.cache.insert(*inner.read().uuid, Arc::new("Specialization".to_owned()));
            },
            DemoOfdElement::DemoOfdAggregation(inner) => {
                self.cache.insert(*inner.read().uuid, Arc::new("Aggregation".to_owned()));
            },
            DemoOfdElement::DemoOfdPrecedence(inner) => {
                self.cache.insert(*inner.read().uuid, Arc::new("Precedence".to_owned()));
            },
            DemoOfdElement::DemoOfdExclusion(inner) => {
                self.cache.insert(*inner.read().uuid, Arc::new("Exclusion".to_owned()));
            },
        }
    }

    fn insert(&mut self, k: ModelUuid, v: Arc<String>) {
        self.cache.insert(k, v);
    }
}

#[derive(Clone)]
pub enum DemoOfdPropChange {
    NameChange(Arc<String>),

    EntityPropertiesChange(Arc<String>),
    EntityInternalChange(bool),

    EventKindChange(DemoTransactionKind),
    EventIdentifierChange(Arc<String>),

    LinkMultiplicityChange(/*target?*/ bool, Arc<String>),
    AggregationKindChange(bool),
    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
}

impl Debug for DemoOfdPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UmlClassPropChange::???")
    }
}

impl TryFrom<&DemoOfdPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &DemoOfdPropChange) -> Result<Self, Self::Error> {
        match value {
            DemoOfdPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for DemoOfdPropChange {
    fn from(value: ColorChangeData) -> Self {
        DemoOfdPropChange::ColorChange(value)
    }
}
impl TryFrom<DemoOfdPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: DemoOfdPropChange) -> Result<Self, Self::Error> {
        match value {
            DemoOfdPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Clone, derive_more::From)]
pub enum DemoOfdElementOrVertex {
    Element(DemoOfdElementView),
    Vertex(VertexInformation),
}

impl Debug for DemoOfdElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DemoOfdElementOrVertex::???")
    }
}

impl TryFrom<DemoOfdElementOrVertex> for VertexInformation {
    type Error = ();

    fn try_from(value: DemoOfdElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            DemoOfdElementOrVertex::Vertex(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<DemoOfdElementOrVertex> for DemoOfdElementView {
    type Error = ();

    fn try_from(value: DemoOfdElementOrVertex) -> Result<Self, Self::Error> {
        match value {
            DemoOfdElementOrVertex::Element(v) => Ok(v),
            _ => Err(()),
        }
    }
}


#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = DemoOfdDomain)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum DemoOfdElementView {
    Package(ERef<PackageViewT>),
    EntityType(ERef<DemoOfdEntityView>),
    EventType(ERef<DemoOfdEventView>),
    PropertyType(ERef<PropertyTypeViewT>),
    Specialization(ERef<SpecializationViewT>),
    Aggregation(ERef<AggregationViewT>),
    Precedence(ERef<PrecedenceViewT>),
    Exclusion(ERef<ExclusionViewT>),
}


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoOfdDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: DemoOfdDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    placeholders: UmlClassPlaceholderViews,
}

#[derive(Clone, Default)]
struct DemoOfdDiagramBuffer {
    name: String,
    comment: String,
}

#[derive(Clone)]
struct UmlClassPlaceholderViews {
    views: [DemoOfdElementView; 8],
}

impl Default for UmlClassPlaceholderViews {
    fn default() -> Self {
        let (entity_m, entity_view) = new_demoofd_entitytype("Membership", "", true, egui::Pos2::ZERO);
        let (event, event_view) = new_demoofd_eventtype(
            "01", "is started",
            (entity_m.clone(), entity_view.clone().into()),
            None, egui::Pos2::new(100.0, 0.0),
        );
        let entity_2 = (entity_m.clone().into(), entity_view.into());
        let (d, dv) = new_demoofd_entitytype("dummy", "", false, egui::Pos2::new(100.0, 75.0));
        let (dummy_event, dummy_event_view) = new_demoofd_eventtype(
            "01", "is started",
            entity_2.clone(),
            None, egui::Pos2::new(200.0, 50.0),
        );

        let (prop, prop_view) = new_demoofd_propertytype("", None, entity_2.clone(), (d.clone(), dv.clone().into()));
        prop.write().domain_multiplicity = Arc::new("".to_owned());
        prop.write().range_multiplicity = Arc::new("".to_owned());
        prop_view.write().refresh_buffers();
        let (_spec, spec_view) = new_demoofd_specialization(None, entity_2.clone(), (d.clone(), dv.clone().into()));
        let (_aggr, aggr_view) = new_demoofd_aggregation(None, entity_2.clone(), (d.clone(), dv.clone().into()));
        let (_prec, prec_view) = new_demoofd_precedence(None, (event.clone(), event_view.clone().into()), (dummy_event.clone(), dummy_event_view.clone().into()));
        let (_excl, excl_view) = new_demoofd_exclusion(None, (event.clone().into(), event_view.clone().into()), (dummy_event.clone().into(), dummy_event_view.clone().into()));

        let (_package, package_view) = new_demoofd_package("A package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });

        Self {
            views: [
                entity_2.1.into(),
                event_view.into(),
                prop_view.into(),
                spec_view.into(),
                aggr_view.into(),
                prec_view.into(),
                excl_view.into(),
                package_view.into(),
            ]
        }
    }
}

impl DemoOfdDiagramAdapter {
    fn new(model: ERef<DemoOfdDiagram>) -> Self {
        let m = model.read();
         Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: DemoOfdDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
            placeholders: Default::default(),
        }
    }
}

impl DiagramAdapter<DemoOfdDomain> for DemoOfdDiagramAdapter {
    fn model(&self) -> ERef<DemoOfdDiagram> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }
    fn view_type(&self) -> &'static str {
        "demoofd-diagram-view"
    }

    fn create_new_view_for(
        &self,
        q: &DemoOfdQueryable<'_>,
        element: DemoOfdElement,
    ) -> Result<DemoOfdElementView, HashSet<ModelUuid>> {
        let v = match element {
            DemoOfdElement::DemoOfdPackage(inner) => {
                DemoOfdElementView::from(
                    new_demoofd_package_view(
                        inner,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            }
            DemoOfdElement::DemoOfdEntityType(inner) => {
                DemoOfdElementView::from(
                    new_demoofd_entitytype_view(inner, egui::Pos2::ZERO)
                )
            },
            DemoOfdElement::DemoOfdEventType(inner) => {
                let m = inner.read();
                let bid = *m.base_entity_type.read().uuid;
                let Some(base_view) = q.get_view(&bid) else {
                    return Err(HashSet::from([bid]));
                };
                DemoOfdElementView::from(
                    new_demoofd_eventtype_view(inner.clone(), base_view, None, egui::Pos2::ZERO)
                )
            },
            DemoOfdElement::DemoOfdPropertyType(inner) => {
                let m = inner.read();
                let (sid, tid) = (*m.domain_element.read().uuid, *m.range_element.read().uuid);
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([sid, tid])),
                };
                DemoOfdElementView::from(
                    new_demoofd_propertytype_view(inner.clone(), None, source_view, target_view)
                )
            },
            DemoOfdElement::DemoOfdSpecialization(inner) => {
                let m = inner.read();
                let (sid, tid) = (*m.domain_element.read().uuid, *m.range_element.read().uuid);
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([sid, tid])),
                };
                DemoOfdElementView::from(
                    new_demoofd_specialization_view(inner.clone(), None, source_view, target_view)
                )
            },
            DemoOfdElement::DemoOfdAggregation(inner) => {
                let m = inner.read();
                let (Some(sv), Some(tv)) = (m.domain_elements.iter().map(|e| q.get_view(&e.read().uuid)).collect(),
                                            q.get_view(&m.range_element.read().uuid)) else {
                    return Err(m.domain_elements.iter().map(|e| *e.read().uuid)
                        .chain(std::iter::once(*m.range_element.read().uuid)).collect())
                };
                DemoOfdElementView::from(
                    new_demoofd_aggregation_view(inner.clone(), None, sv, tv)
                )
            },
            DemoOfdElement::DemoOfdPrecedence(inner) => {
                let m = inner.read();
                let (sid, tid) = (*m.domain_element.read().uuid, *m.range_element.read().uuid);
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([sid, tid])),
                };
                DemoOfdElementView::from(
                    new_demoofd_precedence_view(inner.clone(), None, source_view, target_view)
                )
            },
            DemoOfdElement::DemoOfdExclusion(inner) => {
                let m = inner.read();
                let (sid, tid) = (*m.domain_element.uuid(), *m.range_element.uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([sid, tid])),
                };
                DemoOfdElementView::from(
                    new_demoofd_exclusion_view(inner.clone(), None, source_view, target_view)
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
    ) -> PropertiesStatus<DemoOfdDomain> {
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
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
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
                    vec![DemoOfdPropChange::NameChange(Arc::new(
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
                    vec![DemoOfdPropChange::CommentChange(Arc::new(
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
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    DemoOfdPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                        ));
                        self.background_color = *color;
                    }
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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
        tool: &mut Option<NaiveDemoOfdTool>,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) {
        let button_height = 60.0;
        let width = ui.available_width();

        let stage = tool.as_ref().map(|e| e.initial_stage());
        let c = |s: DemoOfdToolStage| -> egui::Color32 {
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
        let empty_q = DemoOfdQueryable::new(&empty_a, &empty_b);
        let mut icon_counter = 0;
        for cat in [
            &[
                (DemoOfdToolStage::Entity, "Entity Type"),
                (DemoOfdToolStage::EventStart { with_specialization: false }, "Event Type"),
            ][..],
            &[
                (
                    DemoOfdToolStage::LinkStart {
                        link_type: LinkType::PropertyType,
                    },
                    "Property Type",
                ),
                (
                    DemoOfdToolStage::LinkStart {
                        link_type: LinkType::Specialization,
                    },
                    "Specialization",
                ),
                (
                    DemoOfdToolStage::LinkStart {
                        link_type: LinkType::Aggregation,
                    },
                    "Aggregation/Generalization",
                ),
                (
                    DemoOfdToolStage::LinkStart {
                        link_type: LinkType::Precedence,
                    },
                    "Precedence",
                ),
                (
                    DemoOfdToolStage::LinkStart {
                        link_type: LinkType::Exclusion,
                    },
                    "Exclusion",
                ),
            ][..],
            &[(DemoOfdToolStage::PackageStart, "Package")][..],
        ] {
            for (stage, name) in cat {
                let response = ui.add_sized([width, button_height], egui::Button::new(*name).fill(c(*stage)));
                if response.clicked() {
                    if let Some(t) = &tool && t.initial_stage == *stage {
                        *tool = None;
                    } else {
                        *tool = Some(NaiveDemoOfdTool::new(*stage));
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
        _view_uuid: &ViewUuid,
        _label_provider: &ERef<dyn LabelProvider>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {}

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DemoOfdElement>) {
        let (new_model, models) = super::demoofd_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, DemoOfdElement>) {
        let models = super::demoofd_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }
}

pub fn new(no: u32) -> ERef<dyn DiagramController> {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New DEMO OFD diagram {}", no);
    let diagram = ERef::new(DemoOfdDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        DemoOfdDiagramAdapter::new(diagram.clone()),
        Vec::new(),
    )
}

pub fn demo(no: u32) -> ERef<dyn DiagramController> {
    let (entity_membership, entity_membership_view) = new_demoofd_entitytype(
        "MEMBERSHIP",
        &"\n".repeat(21),
        true,
        egui::Pos2::new(120.0, 50.0),
    );

    let (entity_started_membership, entity_started_membership_view) = new_demoofd_entitytype(
        "STARTED MEMBERSHIP",
        "starting day [DAY]",
        true,
        egui::Pos2::new(325.0, 80.0),
    );

    let (event_started, event_started_view) = new_demoofd_eventtype(
        "01", "is started",
        (entity_membership.clone(), entity_membership_view.clone().into()),
        Some((entity_started_membership.clone(), entity_started_membership_view.clone())),
        egui::Pos2::new(325.0, 80.0),
    );

    let (entity_person, entity_person_view) = new_demoofd_entitytype(
        "PERSON",
        "day of birth [DAY]",
        false,
        egui::Pos2::new(550.0, 50.0),
    );

    let (prop_member, prop_member_view) = new_demoofd_propertytype(
        "", None,
        (entity_started_membership.clone(), entity_started_membership_view.clone().into()),
        (entity_person.clone(), entity_person_view.clone().into()),
    );

    let (entity_year, entity_year_view) = new_demoofd_entitytype(
        "[YEAR]",
        "minimal age [NUMBER]\nannual fee [MONEY]\nmax members [NUMBER]",
        false,
        egui::Pos2::new(550.0, 250.0),
    );

    let diagram_view_uuid = uuid::Uuid::now_v7().into();
    let diagram_model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("Demo DEMO OFD diagram {}", no);
    let diagram2 = ERef::new(DemoOfdDiagram::new(
        diagram_model_uuid,
        name.clone(),
        vec![
            entity_membership.into(),
            event_started.into(),
            entity_person.into(),
            prop_member.into(),
            entity_year.into(),
        ],
    ));
    DiagramControllerGen2::new(
        Arc::new(diagram_view_uuid),
        name.clone().into(),
        DemoOfdDiagramAdapter::new(diagram2.clone()),
        vec![
            entity_membership_view.into(),
            event_started_view.into(),
            entity_person_view.into(),
            prop_member_view.into(),
            entity_year_view.into(),
        ],
    )
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<DiagramControllerGen2<DemoOfdDomain, DemoOfdDiagramAdapter>>(&uuid)?)
}

#[derive(Clone, Copy, PartialEq)]
pub enum LinkType {
    PropertyType,
    Specialization,
    Aggregation,
    Precedence,
    Exclusion,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DemoOfdToolStage {
    Entity,
    EventStart { with_specialization: bool },
    EventEnd,
    LinkStart { link_type: LinkType },
    LinkEnd,
    LinkAddEnding { source: bool },
    PackageStart,
    PackageEnd,
}

enum PartialDemoOfdElement {
    None,
    Some(DemoOfdElementView),
    Event {
        with_specialization: bool,
        source: ERef<DemoOfdEntityType>,
        pos: Option<egui::Pos2>,
    },
    EntityLink {
        link_type: LinkType,
        source: ERef<DemoOfdEntityType>,
        dest: Option<ERef<DemoOfdEntityType>>,
    },
    AggregationEnding {
        gen_model: ERef<DemoOfdAggregation>,
        new_model: Option<ModelUuid>,
    },
    EventLink {
        link_type: LinkType,
        source: ERef<DemoOfdEventType>,
        dest: Option<ERef<DemoOfdEventType>>,
    },
    TypeLink {
        link_type: LinkType,
        source: DemoOfdType,
        dest: Option<DemoOfdType>,
    },
    Package {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveDemoOfdTool {
    initial_stage: DemoOfdToolStage,
    current_stage: DemoOfdToolStage,
    result: PartialDemoOfdElement,
    event_lock: bool,
}

impl NaiveDemoOfdTool {
    pub fn new(initial_stage: DemoOfdToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialDemoOfdElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<DemoOfdDomain> for NaiveDemoOfdTool {
    type Stage = DemoOfdToolStage;

    fn initial_stage(&self) -> Self::Stage {
        self.initial_stage
    }

    fn targetting_for_element(&self, element: Option<DemoOfdElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                DemoOfdToolStage::Entity
                | DemoOfdToolStage::EventEnd
                | DemoOfdToolStage::PackageStart
                | DemoOfdToolStage::PackageEnd => TARGETTABLE_COLOR,
                DemoOfdToolStage::EventStart { .. }
                | DemoOfdToolStage::LinkStart { .. }
                | DemoOfdToolStage::LinkEnd
                | DemoOfdToolStage::LinkAddEnding { .. } => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(DemoOfdElement::DemoOfdPackage(..)) => match self.current_stage {
                DemoOfdToolStage::Entity
                | DemoOfdToolStage::EventEnd
                | DemoOfdToolStage::PackageStart
                | DemoOfdToolStage::PackageEnd => TARGETTABLE_COLOR,

                DemoOfdToolStage::EventStart { .. }
                | DemoOfdToolStage::LinkStart { .. }
                | DemoOfdToolStage::LinkEnd
                | DemoOfdToolStage::LinkAddEnding { .. } => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(DemoOfdElement::DemoOfdEntityType(inner)) => match self.current_stage {
                DemoOfdToolStage::LinkEnd => match &self.result {
                    PartialDemoOfdElement::EntityLink { link_type, source, .. } => {
                        if *link_type == LinkType::PropertyType
                            || (*link_type == LinkType::Specialization && *source.read().uuid != *inner.read().uuid)
                            || (*link_type == LinkType::Aggregation && *source.read().uuid != *inner.read().uuid){
                            TARGETTABLE_COLOR
                        } else {
                            NON_TARGETTABLE_COLOR
                        }
                    },
                    PartialDemoOfdElement::TypeLink { link_type, source, .. } => {
                        if *link_type == LinkType::Exclusion && *source.uuid() != *inner.read().uuid {
                            TARGETTABLE_COLOR
                        } else {
                            NON_TARGETTABLE_COLOR
                        }
                    },
                    _ => NON_TARGETTABLE_COLOR
                }
                DemoOfdToolStage::EventStart { .. }
                | DemoOfdToolStage::LinkStart { link_type: LinkType::PropertyType | LinkType::Specialization | LinkType::Aggregation | LinkType::Exclusion }
                | DemoOfdToolStage::LinkAddEnding { .. } => {
                    TARGETTABLE_COLOR
                }
                DemoOfdToolStage::EventEnd
                | DemoOfdToolStage::Entity
                | DemoOfdToolStage::LinkStart { link_type: LinkType::Precedence }
                | DemoOfdToolStage::PackageStart
                | DemoOfdToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            Some(DemoOfdElement::DemoOfdEventType(inner)) => match self.current_stage {
                DemoOfdToolStage::Entity => {
                    if inner.read().specialization_entity_type.is_some() {
                        NON_TARGETTABLE_COLOR
                    } else {
                        TARGETTABLE_COLOR
                    }
                }
                DemoOfdToolStage::EventStart { .. }
                | DemoOfdToolStage::EventEnd
                | DemoOfdToolStage::PackageStart
                | DemoOfdToolStage::PackageEnd
                | DemoOfdToolStage::LinkStart { link_type: LinkType::PropertyType | LinkType::Specialization | LinkType::Aggregation }
                | DemoOfdToolStage::LinkAddEnding { .. } => NON_TARGETTABLE_COLOR,

                DemoOfdToolStage::LinkStart { link_type: LinkType::Precedence | LinkType::Exclusion } => TARGETTABLE_COLOR,
                DemoOfdToolStage::LinkEnd => match &self.result {
                    PartialDemoOfdElement::EventLink { link_type, source, dest }
                        if *source.read().uuid != *inner.read().uuid => {
                        TARGETTABLE_COLOR
                    },
                    PartialDemoOfdElement::TypeLink { link_type, source, .. } => {
                        if *link_type == LinkType::Exclusion && *source.uuid() != *inner.read().uuid {
                            TARGETTABLE_COLOR
                        } else {
                            NON_TARGETTABLE_COLOR
                        }
                    },
                    _ => NON_TARGETTABLE_COLOR
                }
            },
            Some(DemoOfdElement::DemoOfdPropertyType(..)
                | DemoOfdElement::DemoOfdSpecialization(..)
                | DemoOfdElement::DemoOfdAggregation(..)
                | DemoOfdElement::DemoOfdPrecedence(..)
                | DemoOfdElement::DemoOfdExclusion(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &DemoOfdQueryable, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialDemoOfdElement::Event { source, .. } => {
                if let Some(source_view) = q.get_view(&*source.read().uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialDemoOfdElement::EntityLink { source, .. } => {
                if let Some(source_view) = q.get_view(&*source.read().uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialDemoOfdElement::AggregationEnding { gen_model, new_model } => {
                if let Some(source_view) = q.get_view(&*gen_model.read().uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialDemoOfdElement::EventLink { source, .. } => {
                if let Some(source_view) = q.get_view(&*source.read().uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialDemoOfdElement::TypeLink { source, .. } => {
                if let Some(source_view) = q.get_view(&*source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialDemoOfdElement::Package { a, .. } => {
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
            (DemoOfdToolStage::Entity, _) => {
                let (_class_model, class_view) =
                    new_demoofd_entitytype("Membership", "", true, pos);
                self.result = PartialDemoOfdElement::Some(class_view.into());
                self.event_lock = true;
            }
            (DemoOfdToolStage::EventEnd, PartialDemoOfdElement::Event { pos: p, .. }) => {
                *p = Some(pos);
                self.event_lock = true;
            }
            (DemoOfdToolStage::PackageStart, _) => {
                self.result = PartialDemoOfdElement::Package { a: pos, b: None };
                self.current_stage = DemoOfdToolStage::PackageEnd;
                self.event_lock = true;
            }
            (DemoOfdToolStage::PackageEnd, PartialDemoOfdElement::Package { b, .. }) => {
                *b = Some(pos)
            }
            _ => {}
        }
    }
    fn add_element(&mut self, element: DemoOfdElement) {
        if self.event_lock {
            return;
        }

        match element {
            DemoOfdElement::DemoOfdEntityType(inner) => {
                match (self.current_stage, &mut self.result) {
                    (DemoOfdToolStage::EventStart { with_specialization }, PartialDemoOfdElement::None) => {
                        self.result = PartialDemoOfdElement::Event { with_specialization, source: inner, pos: None };
                        self.current_stage = DemoOfdToolStage::EventEnd;
                        self.event_lock = true;
                    }
                    (DemoOfdToolStage::LinkStart { link_type: link_type @ (LinkType::PropertyType | LinkType::Specialization | LinkType::Aggregation) }, PartialDemoOfdElement::None) => {
                        self.result = PartialDemoOfdElement::EntityLink {
                            link_type,
                            source: inner.into(),
                            dest: None,
                        };
                        self.current_stage = DemoOfdToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (DemoOfdToolStage::LinkStart { link_type: link_type @ LinkType::Exclusion }, PartialDemoOfdElement::None) => {
                        self.result = PartialDemoOfdElement::TypeLink {
                            link_type,
                            source: inner.into(),
                            dest: None,
                        };
                        self.current_stage = DemoOfdToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        DemoOfdToolStage::LinkEnd,
                        PartialDemoOfdElement::EntityLink { link_type, source, dest },
                    ) => {
                        if (*link_type == LinkType::PropertyType)
                            || (*link_type == LinkType::Specialization && *source.read().uuid != *inner.read().uuid)
                            || (*link_type == LinkType::Aggregation && *source.read().uuid != *inner.read().uuid) {
                            *dest = Some(inner.into());
                        }
                        self.event_lock = true;
                    }
                    (
                        DemoOfdToolStage::LinkEnd,
                        PartialDemoOfdElement::TypeLink { link_type, source, dest },
                    ) => {
                        if *link_type == LinkType::Exclusion && *source.uuid() != *inner.read().uuid {
                            *dest = Some(inner.into());
                        }
                        self.event_lock = true;
                    }
                    (DemoOfdToolStage::LinkAddEnding { source }, &mut PartialDemoOfdElement::AggregationEnding { ref gen_model, ref mut new_model }) => {
                        let inner_uuid = *inner.read().uuid;
                        let gen_model = gen_model.read();

                        if source && !gen_model.domain_elements.iter().any(|e| *e.read().uuid == inner_uuid)
                            && *gen_model.range_element.read().uuid != inner_uuid {
                            *new_model = Some(inner_uuid);
                        }
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            DemoOfdElement::DemoOfdEventType(inner) => {
                match (self.current_stage, &mut self.result) {
                    (DemoOfdToolStage::Entity, _) => {
                        if !inner.read().specialization_entity_type.is_some() {
                            let (_class_model, class_view) =
                                new_demoofd_entitytype("Membership", "", true, egui::Pos2::ZERO);
                            self.result = PartialDemoOfdElement::Some(class_view.into());
                        }
                        self.event_lock = true;
                    }
                    (DemoOfdToolStage::LinkStart { link_type: link_type @ LinkType::Precedence }, PartialDemoOfdElement::None) => {
                        self.result = PartialDemoOfdElement::EventLink {
                            link_type,
                            source: inner.into(),
                            dest: None,
                        };
                        self.current_stage = DemoOfdToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (DemoOfdToolStage::LinkStart { link_type: link_type @ LinkType::Exclusion }, PartialDemoOfdElement::None) => {
                        self.result = PartialDemoOfdElement::TypeLink {
                            link_type,
                            source: inner.into(),
                            dest: None,
                        };
                        self.current_stage = DemoOfdToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        DemoOfdToolStage::LinkEnd,
                        PartialDemoOfdElement::EventLink { link_type, source, dest },
                    ) => {
                        if *link_type == LinkType::Precedence && *source.read().uuid != *inner.read().uuid {
                            *dest = Some(inner.into());
                        }
                        self.event_lock = true;
                    }
                    (
                        DemoOfdToolStage::LinkEnd,
                        PartialDemoOfdElement::TypeLink { link_type, source, dest },
                    ) => {
                        if *link_type == LinkType::Exclusion && *source.uuid() != *inner.read().uuid {
                            *dest = Some(inner.into());
                        }
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }

            DemoOfdElement::DemoOfdPackage(..)
            | DemoOfdElement::DemoOfdPropertyType(..)
            | DemoOfdElement::DemoOfdSpecialization(..)
            | DemoOfdElement::DemoOfdAggregation(..)
            | DemoOfdElement::DemoOfdPrecedence(..)
            | DemoOfdElement::DemoOfdExclusion(..) => {}
        }
    }

    fn try_additional_dependency(&mut self) -> Option<(u8, ModelUuid, ModelUuid)> {
        match &mut self.result {
            PartialDemoOfdElement::AggregationEnding { gen_model, new_model } if new_model.is_some() => {
                let r = Some((0, *gen_model.read().uuid, new_model.unwrap()));
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
        into: &dyn ContainerGen2<DemoOfdDomain>,
    ) -> Option<(DemoOfdElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialDemoOfdElement::Some(x) => {
                let x = x.clone();
                let esm: Option<Box<dyn CustomModal>> = match &x {
                    DemoOfdElementView::EntityType(inner) => Some(Box::new(DemoOfdEntityTypeSetupModal::from(&inner.read().model))),
                    DemoOfdElementView::EventType(inner) => Some(Box::new(DemoOfdEventTypeSetupModal::from(&inner.read().model))),
                    _ => None,
                };
                self.result = PartialDemoOfdElement::None;
                Some((x, esm))
            }
            PartialDemoOfdElement::Event { with_specialization, source, pos: Some(p) } => {
                let base_uuid = *source.read().uuid;
                if let Some(base_view) = into.controller_for(&base_uuid) {
                    self.current_stage = DemoOfdToolStage::EventStart { with_specialization: *with_specialization };

                    let spec = if *with_specialization {
                        Some(new_demoofd_entitytype("STARTED MEMBERSHIP", "starting day [DAY]", true, *p))
                    } else {
                        None
                    };
                    let (_event_model, event_view) =
                        new_demoofd_eventtype("01", "is started", (source.clone(), base_view), spec, *p);

                    self.result = PartialDemoOfdElement::None;
                    Some((event_view.into(), None))
                } else {
                    None
                }
            }
            PartialDemoOfdElement::EntityLink {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid, *dest.read().uuid);
                if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = DemoOfdToolStage::LinkStart {
                        link_type: *link_type,
                    };

                    let link_view = match link_type {
                        LinkType::PropertyType => {
                            new_demoofd_propertytype(
                                "",
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        },
                        LinkType::Specialization => {
                            new_demoofd_specialization(
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        },
                        LinkType::Aggregation => {
                            new_demoofd_aggregation(
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        }
                        LinkType::Precedence
                        | LinkType::Exclusion => unreachable!()
                    };

                    self.result = PartialDemoOfdElement::None;

                    Some((link_view, None))
                } else {
                    None
                }
            }
            PartialDemoOfdElement::EventLink {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid, *dest.read().uuid);
                if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = DemoOfdToolStage::LinkStart {
                        link_type: *link_type,
                    };

                    let link_view = match link_type {
                        LinkType::PropertyType
                        | LinkType::Specialization
                        | LinkType::Aggregation => unreachable!(),
                        LinkType::Precedence => {
                            new_demoofd_precedence(
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        }
                        LinkType::Exclusion => unreachable!()
                    };

                    self.result = PartialDemoOfdElement::None;

                    Some((link_view, None))
                } else {
                    None
                }
            }
            PartialDemoOfdElement::TypeLink {
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
                    self.current_stage = DemoOfdToolStage::LinkStart {
                        link_type: *link_type,
                    };

                    let link_view = match link_type {
                        LinkType::PropertyType
                        | LinkType::Specialization
                        | LinkType::Aggregation
                        | LinkType::Precedence => unreachable!(),
                        LinkType::Exclusion => {
                            new_demoofd_exclusion(
                                None,
                                (source.clone(), source_controller),
                                (dest.clone(), dest_controller),
                            ).1.into()
                        }
                    };

                    self.result = PartialDemoOfdElement::None;

                    Some((link_view, None))
                } else {
                    None
                }
            }
            PartialDemoOfdElement::Package { a, b: Some(b) } => {
                self.current_stage = DemoOfdToolStage::PackageStart;

                let (_package_model, package_view) =
                    new_demoofd_package("A package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialDemoOfdElement::None;
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
pub struct DemoOfdPackageAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdPackage>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<DemoOfdDomain> for DemoOfdPackageAdapter {
    fn model(&self) -> DemoOfdElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
    }

    fn add_element(&mut self, e: DemoOfdElement) {
        self.model.write().add_element(e);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.model.write().delete_elements(uuids);
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>
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
                DemoOfdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self where Self: Sized {
        let model_uuid = *self.model.read().uuid;
        let model = if let Some(DemoOfdElement::DemoOfdPackage(m)) = m.get(&model_uuid) {
            m.clone()
        } else {
            let model = self.model.read().clone_with(new_uuid);
            m.insert(model_uuid, model.clone().into());
            model
        };
        Self { model, name_buffer: self.name_buffer.clone(), comment_buffer: self.comment_buffer.clone() }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DemoOfdElement>,
    ) {
        todo!()
    }
}

fn new_demoofd_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (ERef<DemoOfdPackage>, ERef<PackageViewT>) {
    let graph_model = ERef::new(DemoOfdPackage::new(
        uuid::Uuid::now_v7().into(),
        name.to_owned(),
        vec![],
    ));
    let graph_view = new_demoofd_package_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_demoofd_package_view(
    model: ERef<DemoOfdPackage>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT> {
    let m = model.read();
    PackageViewT::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoOfdPackageAdapter {
            model: model.clone(),
            name_buffer: (*m.name).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}


fn new_demoofd_entitytype(
    name: &str,
    properties: &str,
    is_internal: bool,
    position: egui::Pos2,
) -> (ERef<DemoOfdEntityType>, ERef<DemoOfdEntityView>) {
    let class_model = ERef::new(DemoOfdEntityType::new(
        uuid::Uuid::now_v7().into(),
        name.to_owned(),
        properties.to_owned(),
        is_internal,
    ));
    let class_view = new_demoofd_entitytype_view(class_model.clone(), position);

    (class_model, class_view)
}
fn new_demoofd_entitytype_view(
    model: ERef<DemoOfdEntityType>,
    position: egui::Pos2,
) -> ERef<DemoOfdEntityView> {
    let m = model.read();
    ERef::new(DemoOfdEntityView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),

        name_buffer: (*m.name).clone(),
        properties_buffer: (*m.properties).clone(),
        internal_buffer: m.internal,
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
    })
}

struct DemoOfdEntityTypeSetupModal {
    model: ERef<DemoOfdEntityType>,

    name_buffer: String,
    internal_buffer: bool,
}

impl From<&ERef<DemoOfdEntityType>> for DemoOfdEntityTypeSetupModal {
    fn from(model: &ERef<DemoOfdEntityType>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            name_buffer: (*m.name).clone(),
            internal_buffer: m.internal,
        }
    }
}

impl CustomModal for DemoOfdEntityTypeSetupModal {
    fn show(
        &mut self,
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Name:");
        ui.text_edit_singleline(&mut self.name_buffer);
        ui.label("Internal:");
        ui.checkbox(&mut self.internal_buffer, "");
        ui.separator();

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.name = Arc::new(self.name_buffer.clone());
                m.internal = self.internal_buffer;
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
pub struct DemoOfdEntityView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<DemoOfdEntityType>,

    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    properties_buffer: String,
    #[nh_context_serde(skip_and_default)]
    internal_buffer: bool,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl DemoOfdEntityView {
    const BUTTON_RADIUS: f32 = 8.0;
    fn event_button_rect(&self, ui_scale: f32) ->  egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::splat(Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
    fn event_spec_button_rect(&self, ui_scale: f32) ->  egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::new(Self::BUTTON_RADIUS / ui_scale, 3.0 * Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }
    fn property_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_center = self.bounds_rect.right_top()
            + egui::Vec2::new(Self::BUTTON_RADIUS / ui_scale, 5.0 * Self::BUTTON_RADIUS / ui_scale);
        egui::Rect::from_center_size(
            b_center,
            egui::Vec2::splat(2.0 * Self::BUTTON_RADIUS / ui_scale),
        )
    }

    fn draw_inner(
        &mut self,
        q: &DemoOfdQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoOfdTool)>,
        event: Option<(NHShape, egui::Rect)>,
    ) -> TargettingStatus {
        const CORNER_RADIUS: egui::CornerRadius = egui::CornerRadius::same(10);
        let read = self.model.read();

        let event_size = if let Some(e) = event {
            e.0.bounding_box().union(e.1)
        } else {
            egui::Rect::from_center_size(self.position, egui::Vec2::ZERO)
        };
        let name_size = canvas.measure_text(
            event_size.center_top(),
            egui::Align2::CENTER_BOTTOM,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        let props_size = canvas.measure_text(
            event_size.center_bottom(),
            egui::Align2::CENTER_TOP,
            &read.properties,
            canvas::CLASS_ITEM_FONT_SIZE,
        );
        self.bounds_rect = name_size.union(event_size).union(props_size).expand(5.0);

        canvas.draw_rectangle(
            self.bounds_rect,
            CORNER_RADIUS,
            if read.internal {
                INTERNAL_ROLE_BACKGROUND
            } else {
                EXTERNAL_ROLE_BACKGROUND
            },
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_line(
            [
                egui::Pos2::new(self.bounds_rect.left(), event_size.bottom()),
                egui::Pos2::new(self.bounds_rect.right(), event_size.bottom()),
            ],
            canvas::Stroke::new_dotted(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );

        canvas.draw_text(
            event_size.center_top(),
            egui::Align2::CENTER_BOTTOM,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        canvas.draw_text(
            event_size.center_bottom(),
            egui::Align2::CENTER_TOP,
            &read.properties,
            canvas::CLASS_ITEM_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            let b1_rect = self.event_button_rect(ui_scale);
            canvas.draw_rectangle(
                b1_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b1_rect.center(), egui::Align2::CENTER_CENTER, "◊", 14.0 / ui_scale, egui::Color32::BLACK);

            let b2_rect = self.event_spec_button_rect(ui_scale);
            canvas.draw_rectangle(
                b2_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b2_rect.center(), egui::Align2::CENTER_CENTER, "▣", 14.0 / ui_scale, egui::Color32::BLACK);

            let b3_rect = self.property_button_rect(ui_scale);
            canvas.draw_rectangle(
                b3_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_text(b3_rect.center(), egui::Align2::CENTER_CENTER, "↘", 14.0 / ui_scale, egui::Color32::BLACK);
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
                .filter(|t| self.min_shape().contains(t.0))
                .filter(|t| event.is_none_or(|e| !e.0.contains(t.0)))
                .map(|t| t.1)
            {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    CORNER_RADIUS,
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
}

impl Entity for DemoOfdEntityView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for DemoOfdEntityView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<DemoOfdElement> for DemoOfdEntityView {
    fn model(&self) -> DemoOfdElement {
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

impl ContainerGen2<DemoOfdDomain> for DemoOfdEntityView {}

impl ElementControllerGen2<DemoOfdDomain> for DemoOfdEntityView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        _q: &DemoOfdQueryable,
        _lp: &DemoOfdLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) -> PropertiesStatus<DemoOfdDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                DemoOfdPropChange::EntityPropertiesChange(Arc::new(self.properties_buffer.clone())),
            ]));
        }

        if ui.checkbox(&mut self.internal_buffer, "internal").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::EntityInternalChange(self.internal_buffer),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        q: &DemoOfdQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoOfdTool)>,
    ) -> TargettingStatus {
        self.draw_inner(q, context, canvas, tool, None)
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoOfdTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
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
            InputEvent::Click(pos) if self.highlight.selected && self.event_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveDemoOfdTool {
                    initial_stage: DemoOfdToolStage::EventStart { with_specialization: false },
                    current_stage: DemoOfdToolStage::EventEnd,
                    result: PartialDemoOfdElement::Event {
                        with_specialization: false,
                        source: self.model.clone(),
                        pos: None,
                    },
                    event_lock: true,
                });

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.highlight.selected && self.event_spec_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveDemoOfdTool {
                    initial_stage: DemoOfdToolStage::EventStart { with_specialization: true },
                    current_stage: DemoOfdToolStage::EventEnd,
                    result: PartialDemoOfdElement::Event {
                        with_specialization: true,
                        source: self.model.clone(),
                        pos: None,
                    },
                    event_lock: true,
                });

                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.highlight.selected && self.property_button_rect(ehc.ui_scale).contains(pos) => {
                *tool = Some(NaiveDemoOfdTool {
                    initial_stage: DemoOfdToolStage::LinkStart { link_type: LinkType::PropertyType },
                    current_stage: DemoOfdToolStage::LinkEnd,
                    result: PartialDemoOfdElement::EntityLink {
                        link_type: LinkType::PropertyType,
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
                let coerced_delta = coerced_pos - self.min_shape().center();

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
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
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
                            DemoOfdPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::NameChange(model.name.clone())],
                                ));
                                model.name = name.clone();
                            }
                            DemoOfdPropChange::EntityInternalChange(internal) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::EntityInternalChange(
                                        model.internal,
                                    )],
                                ));
                                model.internal = *internal;
                            }
                            DemoOfdPropChange::EntityPropertiesChange(properties) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::EntityPropertiesChange(
                                        model.properties.clone(),
                                    )],
                                ));
                                model.properties = properties.clone();
                            }
                            DemoOfdPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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
        self.name_buffer = (*model.name).clone();
        self.properties_buffer = (*model.properties).clone();
        self.internal_buffer = model.internal;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoOfdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoOfdElementView>,
        c: &mut HashMap<ViewUuid, DemoOfdElementView>,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoOfdElement::DemoOfdEntityType(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            name_buffer: self.name_buffer.clone(),
            properties_buffer: self.properties_buffer.clone(),
            internal_buffer: self.internal_buffer,
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


fn new_demoofd_eventtype(
    identifier: &str,
    name: &str,
    base_entity_type: (ERef<DemoOfdEntityType>, DemoOfdElementView),
    specialization_entity_type: Option<(ERef<DemoOfdEntityType>, ERef<DemoOfdEntityView>)>,
    position: egui::Pos2,
) -> (ERef<DemoOfdEventType>, ERef<DemoOfdEventView>) {
    let (spec_model, spec_view) = specialization_entity_type
        .map(|e| (Some(e.0), Some(e.1)))
        .unwrap_or((None, None));

    let instance_model = ERef::new(DemoOfdEventType::new(
        uuid::Uuid::now_v7().into(),
        DemoTransactionKind::Performa,
        identifier.to_owned(),
        name.to_owned(),
        base_entity_type.0,
        spec_model,
    ));
    let instance_view = new_demoofd_eventtype_view(instance_model.clone(), base_entity_type.1, spec_view, position);

    (instance_model, instance_view)
}
fn new_demoofd_eventtype_view(
    model: ERef<DemoOfdEventType>,
    base_entity_type: DemoOfdElementView,
    specialization_entity_type: Option<ERef<DemoOfdEntityView>>,
    position: egui::Pos2,
) -> ERef<DemoOfdEventView> {
    let m = model.read();
    ERef::new(DemoOfdEventView {
        uuid: Arc::new(uuid::Uuid::now_v7().into()),
        model: model.clone(),
        base_entity_type,
        specialization_view: UFOption::from(specialization_entity_type),
        kind_buffer: m.kind,
        identifier_buffer: (*m.identifier).clone(),
        name_buffer: (*m.name).clone(),
        comment_buffer: (*m.comment).clone(),
        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
    })
}

struct DemoOfdEventTypeSetupModal {
    model: ERef<DemoOfdEventType>,

    name_buffer: String,
}

impl From<&ERef<DemoOfdEventType>> for DemoOfdEventTypeSetupModal {
    fn from(model: &ERef<DemoOfdEventType>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            name_buffer: (*m.name).clone(),
        }
    }
}

impl CustomModal for DemoOfdEventTypeSetupModal {
    fn show(
        &mut self,
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Name:");
        ui.text_edit_singleline(&mut self.name_buffer);
        ui.separator();

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
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
pub struct DemoOfdEventView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<DemoOfdEventType>,
    #[nh_context_serde(entity)]
    base_entity_type: DemoOfdElementView,
    #[nh_context_serde(entity)]
    specialization_view: UFOption<ERef<DemoOfdEntityView>>,

    #[nh_context_serde(skip_and_default)]
    kind_buffer: DemoTransactionKind,
    #[nh_context_serde(skip_and_default)]
    identifier_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl DemoOfdEventView {
    const RADIUS: f32 = 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE;
}

impl Entity for DemoOfdEventView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for DemoOfdEventView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<DemoOfdElement> for DemoOfdEventView {
    fn model(&self) -> DemoOfdElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rhombus {
            position: self.position,
            bounds_radius: egui::Vec2::splat(Self::RADIUS),
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ContainerGen2<DemoOfdDomain> for DemoOfdEventView {}

impl ElementControllerGen2<DemoOfdDomain> for DemoOfdEventView {
    fn show_properties(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        q: &DemoOfdQueryable,
        lp: &DemoOfdLabelProvider,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) -> PropertiesStatus<DemoOfdDomain> {
        if let Some(child) = self.specialization_view.as_mut()
                .and_then(|t| t.write().show_properties(drawing_context, q, lp, ui, commands).to_non_default()) {
            return child;
        }

        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Transaction Kind:");
        egui::ComboBox::from_id_salt("Transaction Kind:")
            .selected_text(self.kind_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoTransactionKind::Performa,
                    DemoTransactionKind::Informa,
                    DemoTransactionKind::Forma,
                ] {
                    if ui
                        .selectable_value(&mut self.kind_buffer, value, value.char())
                        .clicked() {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoOfdPropChange::EventKindChange(self.kind_buffer),
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
                DemoOfdPropChange::EventIdentifierChange(Arc::new(self.identifier_buffer.clone())),
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
                DemoOfdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        q: &DemoOfdQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoOfdTool)>,
    ) -> TargettingStatus {
        let read = self.model.read();

        let name_pos = canvas.measure_text(
            self.position + egui::Vec2::new(0.0, 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE),
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );

        let child_status = if let UFOption::Some(s) = &self.specialization_view {
            s.write().draw_inner(q, context, canvas, tool, Some((self.min_shape(), name_pos)))
        } else {
            TargettingStatus::NotDrawn
        };

        self.bounds_rect = egui::Rect::from_min_max(self.position, self.position);

        // Draw link to base entity
        let p = self.base_entity_type.min_shape().center_intersect(self.position);
        canvas.draw_line([p, self.position], canvas::Stroke::new_solid(1.0, egui::Color32::BLACK), canvas::Highlight::NONE);

        // Draw diamond
        {
            let pos = self.position;
            canvas.draw_polygon(
                [
                    pos - egui::Vec2::new(0.0, Self::RADIUS),
                    pos + egui::Vec2::new(Self::RADIUS, 0.0),
                    pos + egui::Vec2::new(0.0, Self::RADIUS),
                    pos - egui::Vec2::new(Self::RADIUS, 0.0),
                ].to_vec(),
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, match read.kind {
                    DemoTransactionKind::Performa => PERFORMA_DETAIL,
                    DemoTransactionKind::Informa => INFORMA_DETAIL,
                    DemoTransactionKind::Forma => FORMA_DETAIL,
                }),
                self.highlight,
            );

            canvas.draw_text(
                self.position,
                egui::Align2::CENTER_CENTER,
                &read.identifier,
                canvas::CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, 2.0 * canvas::CLASS_MIDDLE_FONT_SIZE),
            egui::Align2::CENTER_TOP,
            &read.name,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        if canvas.ui_scale().is_some() {
            // Draw targetting rectangle
            if let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
            {
                let pos = self.position;
                canvas.draw_polygon(
                    [
                        pos - egui::Vec2::new(0.0, Self::RADIUS),
                        pos + egui::Vec2::new(Self::RADIUS, 0.0),
                        pos + egui::Vec2::new(0.0, Self::RADIUS),
                        pos - egui::Vec2::new(Self::RADIUS, 0.0),
                    ].to_vec(),
                    t.targetting_for_element(Some(self.model())),
                    canvas::Stroke::new_solid(1.0, match read.kind {
                        DemoTransactionKind::Performa => PERFORMA_DETAIL,
                        DemoTransactionKind::Informa => INFORMA_DETAIL,
                        DemoTransactionKind::Forma => FORMA_DETAIL,
                    }),
                    canvas::Highlight::NONE,
                );
                return TargettingStatus::Drawn;
            }
        }

        child_status
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());

        if let UFOption::Some(s) = &self.specialization_view {
            s.write().collect_allignment(am);
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoOfdTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) -> EventHandlingStatus {
        match event {
            InputEvent::MouseDown(pos) if self.min_shape().contains(pos) => {
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::MouseUp(_) if self.dragged_shape.is_some() => {
                self.dragged_shape = None;
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(pos) if self.min_shape().contains(pos) => {
                if let Some(tool) = tool {
                    tool.add_element(self.model());

                    if !self.specialization_view.is_some()
                        && let Some((DemoOfdElementView::EntityType(new_e), esm)) = tool.try_construct_view(self) {
                        new_e.write().position = self.position;
                        commands.push(InsensitiveCommand::AddElement(*self.uuid, DemoOfdElementView::from(new_e).into(), true).into());
                        if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            *element_setup_modal = esm;
                        }
                    }

                    EventHandlingStatus::HandledByContainer
                } else {
                    if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                        self.highlight.selected = true;
                    } else {
                        self.highlight.selected = !self.highlight.selected;
                    }

                    EventHandlingStatus::HandledByElement
                }
            }
            InputEvent::Click(pos) if !self.min_shape().contains(pos) => {
                if let UFOption::Some(s) = &self.specialization_view {
                    let r = s.write().handle_event(event, ehc, tool, element_setup_modal, commands);
                    match r {
                        EventHandlingStatus::HandledByElement => {
                            let s = s.read();
                            if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                                commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                                commands.push(
                                    InsensitiveCommand::HighlightSpecific(
                                        std::iter::once(*s.uuid()).collect(),
                                        true,
                                        Highlight::SELECTED,
                                    )
                                    .into(),
                                );
                            } else {
                                commands.push(
                                    InsensitiveCommand::HighlightSpecific(
                                        std::iter::once(*s.uuid()).collect(),
                                        !s.highlight.selected,
                                        Highlight::SELECTED,
                                    )
                                    .into(),
                                );
                            }
                            EventHandlingStatus::HandledByContainer
                        },
                        a => a,
                    }
                } else {
                    EventHandlingStatus::NotHandled
                }
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
            _ => self
                .specialization_view
                .as_ref()
                .map(|t| t.write().handle_event(event, ehc, tool, element_setup_modal, commands))
                .unwrap_or(EventHandlingStatus::NotHandled)
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            ($self:ident) => {
                if let UFOption::Some(s) = &$self.specialization_view {
                    s.write().apply_command(command, undo_accumulator, affected_models);
                }
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
            InsensitiveCommand::MoveSpecificElements(uuids, delta)
                if !uuids.contains(&*self.uuid())
                    && !self
                        .specialization_view
                        .as_ref()
                        .is_some_and(|e| uuids.contains(&e.read().uuid())) => {}
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
                if let UFOption::Some(s) = &self.specialization_view {
                    s.write().apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut vec![], affected_models);
                }
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::DeleteSpecificElements(uuids, into_model) => {
                if let Some(e) = self.specialization_view.as_ref()
                    && uuids.contains(&*e.read().uuid) {
                    undo_accumulator.push(InsensitiveCommand::AddElement(
                        *self.uuid,
                        DemoOfdElementOrVertex::Element(e.clone().into()),
                        *into_model,
                    ));
                    if *into_model {
                        self.model.write().specialization_entity_type = UFOption::None;
                    }
                    self.specialization_view = UFOption::None;
                }
                recurse!(self);
            }
            InsensitiveCommand::AddElement(v, e, into_model) => {
                if *v == *self.uuid
                    && self.specialization_view.as_ref().is_none()
                    && let DemoOfdElementOrVertex::Element(DemoOfdElementView::EntityType(e)) = e
                    {
                    undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                        std::iter::once(*e.read().uuid).collect(),
                        *into_model,
                    ));

                    if *into_model {
                        self.model.write().specialization_entity_type = UFOption::Some(e.read().model.clone());
                        affected_models.insert(*self.model_uuid());
                    }
                    affected_models.insert(*e.read().model_uuid());

                    self.specialization_view = UFOption::Some(e.clone());
                }
                recurse!(self);
            }
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            DemoOfdPropChange::EventKindChange(kind) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::EventKindChange(
                                        model.kind,
                                    )],
                                ));
                                model.kind = *kind;
                            }
                            DemoOfdPropChange::EventIdentifierChange(identifier) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::EventIdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                model.identifier = identifier.clone();
                            }
                            DemoOfdPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::NameChange(model.name.clone())],
                                ));
                                model.name = name.clone();
                            }
                            DemoOfdPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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
        self.kind_buffer = model.kind;
        self.identifier_buffer = (*model.identifier).clone();
        self.name_buffer = (*model.name).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoOfdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        if let UFOption::Some(s) = &self.specialization_view {
            let mut views_s = HashMap::new();
            let mut sl = s.write();
            sl.head_count(flattened_views, &mut views_s, flattened_represented_models);

            for e in views_s {
                flattened_views_status.insert(e.0, match e.1 {
                    SelectionStatus::NotSelected if self.highlight.selected => SelectionStatus::TransitivelySelected,
                    e => e,
                });
            }

            flattened_views.insert(*sl.uuid(), s.clone().into());
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        deleting.contains(&self.base_entity_type.uuid())
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoOfdElementView>,
        c: &mut HashMap<ViewUuid, DemoOfdElementView>,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoOfdElement::DemoOfdEventType(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            base_entity_type: self.base_entity_type.clone(),
            specialization_view: self.specialization_view.clone(),
            kind_buffer: self.kind_buffer,
            identifier_buffer: self.identifier_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
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


fn new_demoofd_propertytype(
    name: &str,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<DemoOfdEntityType>, DemoOfdElementView),
    target: (ERef<DemoOfdEntityType>, DemoOfdElementView),
) -> (ERef<DemoOfdPropertyType>, ERef<PropertyTypeViewT>) {
    let link_model = ERef::new(DemoOfdPropertyType::new(
        uuid::Uuid::now_v7().into(),
        name.to_owned(),
        source.0,
        target.0,
    ));
    let link_view = new_demoofd_propertytype_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_demoofd_propertytype_view(
    model: ERef<DemoOfdPropertyType>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: DemoOfdElementView,
    target: DemoOfdElementView,
) -> ERef<PropertyTypeViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.domain_element.read().uuid), *m.range_element.read().uuid, target.min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoOfdPropertyTypeAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoOfdPropertyTypeAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdPropertyType>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: DemoOfdPropertyTypeTemporaries,
}

#[derive(Clone, Default)]
struct DemoOfdPropertyTypeTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    name_buffer: String,
    domain_multiplicity_buffer: String,
    range_multiplicity_buffer: String,

    comment_buffer: String,
}

impl MulticonnectionAdapter<DemoOfdDomain> for DemoOfdPropertyTypeAdapter {
    fn model(&self) -> DemoOfdElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        let r = self.model.read();
        if r.name.is_empty() {
            None
        } else {
            Some(r.name.clone())
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
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>
    ) -> PropertiesStatus<DemoOfdDomain> {
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::NameChange(Arc::new(
                    self.temporaries.name_buffer.clone(),
                )),
            ]));
        }
        ui.separator();

        ui.label("Domain multiplicity:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.domain_multiplicity_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::LinkMultiplicityChange(false, Arc::new(
                    self.temporaries.domain_multiplicity_buffer.clone(),
                )),
            ]));
        }
        ui.separator();

        ui.label("Range multiplicity:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.range_multiplicity_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::LinkMultiplicityChange(true, Arc::new(
                    self.temporaries.range_multiplicity_buffer.clone(),
                )),
            ]));
        }
        ui.separator();

        if ui.button("Switch source and destination").clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::FlipMulticonnection(FlipMulticonnection {}),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::NameChange(
                                model.name.clone(),
                            )],
                        ));
                        model.name = name.clone();
                    }
                    DemoOfdPropChange::LinkMultiplicityChange(t, multiplicity) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::LinkMultiplicityChange(
                                *t,
                                if !t {
                                    model.domain_multiplicity.clone()
                                } else {
                                    model.range_multiplicity.clone()
                                }
                            )],
                        ));
                        if !t {
                            model.domain_multiplicity = multiplicity.clone();
                        } else {
                            model.range_multiplicity = multiplicity.clone();
                        }
                    }
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert((false, *model.domain_element.read().uuid), ArrowData {
            line_type: canvas::LineType::Solid,
            arrowhead_type: canvas::ArrowheadType::None,
            multiplicity: if !model.domain_multiplicity.is_empty() {
                Some(model.domain_multiplicity.clone())
            } else {
                None
            },
            role: None,
            reading: None,
        });
        self.temporaries.arrow_data.insert((true, *model.range_element.read().uuid), ArrowData {
            line_type: canvas::LineType::Solid,
            arrowhead_type: canvas::ArrowheadType::OpenTriangle,
            multiplicity: if !model.range_multiplicity.is_empty() {
                Some(model.range_multiplicity.clone())
            } else {
                None
            },
            role: None,
            reading: None,
        });

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.domain_element.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.range_element.read().uuid);

        self.temporaries.name_buffer = (*model.name).clone();
        self.temporaries.domain_multiplicity_buffer = (*model.domain_multiplicity).clone();
        self.temporaries.range_multiplicity_buffer = (*model.range_multiplicity).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(DemoOfdElement::DemoOfdPropertyType(m)) = m.get(&old_model.uuid) {
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
        m: &HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.domain_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEntityType(new_source)) = m.get(&source_uuid) {
            model.domain_element = new_source.clone();
        }
        let target_uuid = *model.range_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEntityType(new_target)) = m.get(&target_uuid) {
            model.range_element = new_target.clone();
        }
    }
}


fn new_demoofd_specialization(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<DemoOfdEntityType>, DemoOfdElementView),
    target: (ERef<DemoOfdEntityType>, DemoOfdElementView),
) -> (ERef<DemoOfdSpecialization>, ERef<SpecializationViewT>) {
    let link_model = ERef::new(DemoOfdSpecialization::new(
        uuid::Uuid::now_v7().into(),
        source.0,
        target.0,
    ));
    let link_view = new_demoofd_specialization_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_demoofd_specialization_view(
    model: ERef<DemoOfdSpecialization>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: DemoOfdElementView,
    target: DemoOfdElementView,
) -> ERef<SpecializationViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.domain_element.read().uuid), *m.range_element.read().uuid, target.min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoOfdSpecializationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoOfdSpecializationAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdSpecialization>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: DemoOfdSpecializationTemporaries,
}

#[derive(Clone, Default)]
struct DemoOfdSpecializationTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    comment_buffer: String,
}

impl MulticonnectionAdapter<DemoOfdDomain> for DemoOfdSpecializationAdapter {
    fn model(&self) -> DemoOfdElement {
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

    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        self.model.write().flip_multiconnection();
        Ok(())
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>
    ) -> PropertiesStatus<DemoOfdDomain> {
        if ui.button("Switch source and destination").clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::FlipMulticonnection(FlipMulticonnection {}),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.domain_element.read().uuid),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.range_element.read().uuid),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::FullTriangle),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.domain_element.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.range_element.read().uuid);

        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(DemoOfdElement::DemoOfdSpecialization(m)) = m.get(&old_model.uuid) {
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
        m: &HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.domain_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEntityType(new_source)) = m.get(&source_uuid) {
            model.domain_element = new_source.clone();
        }
        let target_uuid = *model.range_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEntityType(new_target)) = m.get(&target_uuid) {
            model.range_element = new_target.clone();
        }
    }
}


fn new_demoofd_aggregation(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<DemoOfdEntityType>, DemoOfdElementView),
    target: (ERef<DemoOfdEntityType>, DemoOfdElementView),
) -> (ERef<DemoOfdAggregation>, ERef<AggregationViewT>) {
    let link_model = ERef::new(DemoOfdAggregation::new(
        uuid::Uuid::now_v7().into(),
        vec![source.0],
        target.0,
        false,
    ));
    let link_view = new_demoofd_aggregation_view(link_model.clone(), center_point, vec![source.1], target.1);
    (link_model, link_view)
}
fn new_demoofd_aggregation_view(
    model: ERef<DemoOfdAggregation>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    sources: Vec<DemoOfdElementView>,
    target: DemoOfdElementView,
) -> ERef<AggregationViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(m.domain_elements.iter().map(|e| *e.read().uuid), *m.range_element.read().uuid, target.min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoOfdAggregationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        sources.into_iter().zip(sp.into_iter()).map(|e| Ending::new_p(e.0, e.1)).collect(),
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoOfdAggregationAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdAggregation>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: DemoOfdAggregationTemporaries,
}

#[derive(Clone, Default)]
struct DemoOfdAggregationTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    is_generalization_buffer: bool,
    comment_buffer: String,
}

impl MulticonnectionAdapter<DemoOfdDomain> for DemoOfdAggregationAdapter {
    fn model(&self) -> DemoOfdElement {
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

    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        self.model.write().flip_multiconnection()
    }

    fn push_source(&mut self, e: <DemoOfdDomain as Domain>::CommonElementT) -> Result<(), ()> {
        if let DemoOfdElement::DemoOfdEntityType(c) = e {
            self.model.write().domain_elements.push(c);
            Ok(())
        } else {
            Err(())
        }
    }
    fn remove_source(&mut self, uuid: &ModelUuid) -> Result<(), ()> {
        let mut w = self.model.write();
        if w.domain_elements.len() == 1 {
            return Err(())
        }
        let original_count = w.domain_elements.len();
        w.domain_elements.retain(|e| *uuid != *e.read().uuid);
        if w.domain_elements.len() != original_count {
            Ok(())
        } else {
            Err(())
        }
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>
    ) -> PropertiesStatus<DemoOfdDomain> {
        let r = self.model.read();

        if ui.button("Add source").clicked() {
            return PropertiesStatus::ToolRequest(
                Some(NaiveDemoOfdTool {
                    initial_stage: DemoOfdToolStage::LinkAddEnding { source: true },
                    current_stage: DemoOfdToolStage::LinkAddEnding { source: true },
                    result: PartialDemoOfdElement::AggregationEnding {
                        gen_model: self.model.clone(),
                        new_model: None,
                    },
                    event_lock: false,
                })
            );
        }

        if ui.add_enabled(r.domain_elements.len() <= 1, egui::Button::new("Switch source and destination")).clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ]));
        }
        ui.separator();

        ui.label("Type:");
        egui::ComboBox::from_id_salt("Type")
            .selected_text(if r.is_generalization { "Generalization" } else { "Aggregation" })
            .show_ui(ui, |ui| {
                for value in [
                    (false, "Aggregation"),
                    (true, "Generalization"),
                ] {
                    if ui
                        .selectable_value(&mut self.temporaries.is_generalization_buffer, value.0, value.1)
                        .clicked() {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoOfdPropChange::AggregationKindChange(self.temporaries.is_generalization_buffer),
                        ]));
                    }
                }
            });

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::AggregationKindChange(is_generalization) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::AggregationKindChange(model.is_generalization)],
                        ));
                        model.is_generalization = *is_generalization;
                    }
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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

        self.temporaries.arrow_data.clear();
        for e in &model.domain_elements {
            self.temporaries.arrow_data.insert(
                (false, *e.read().uuid),
                ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
            );
        }
        self.temporaries.arrow_data.insert(
            (true, *model.range_element.read().uuid),
            ArrowData::new_labelless(
                canvas::LineType::Dashed,
                canvas::ArrowheadType::EmptyTriangleWith(
                    if model.is_generalization { '+' } else { '*' }
                ),
            ),
        );

        self.temporaries.source_uuids.clear();
        for e in &model.domain_elements {
            self.temporaries.source_uuids.push(*e.read().uuid);
        }
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.range_element.read().uuid);

        self.temporaries.is_generalization_buffer = model.is_generalization;
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(DemoOfdElement::DemoOfdAggregation(m)) = m.get(&old_model.uuid) {
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
        m: &HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let mut model = self.model.write();

        for e in model.domain_elements.iter_mut() {
            let sid = *e.read().uuid;
            if let Some(DemoOfdElement::DemoOfdEntityType(new_source)) = m.get(&sid) {
                *e = new_source.clone();
            }
        }
        let target_uuid = *model.range_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEntityType(new_target)) = m.get(&target_uuid) {
            model.range_element = new_target.clone();
        }
    }
}


fn new_demoofd_precedence(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<DemoOfdEventType>, DemoOfdElementView),
    target: (ERef<DemoOfdEventType>, DemoOfdElementView),
) -> (ERef<DemoOfdPrecedence>, ERef<PrecedenceViewT>) {
    let link_model = ERef::new(DemoOfdPrecedence::new(
        uuid::Uuid::now_v7().into(),
        source.0,
        target.0,
    ));
    let link_view = new_demoofd_precedence_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_demoofd_precedence_view(
    model: ERef<DemoOfdPrecedence>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: DemoOfdElementView,
    target: DemoOfdElementView,
) -> ERef<PrecedenceViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.domain_element.read().uuid), *m.range_element.read().uuid, target.min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoOfdPrecedenceAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoOfdPrecedenceAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdPrecedence>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: DemoOfdPrecedenceTemporaries,
}

#[derive(Clone, Default)]
struct DemoOfdPrecedenceTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    comment_buffer: String,
}

impl MulticonnectionAdapter<DemoOfdDomain> for DemoOfdPrecedenceAdapter {
    fn model(&self) -> DemoOfdElement {
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

    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        self.model.write().flip_multiconnection();
        Ok(())
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>
    ) -> PropertiesStatus<DemoOfdDomain> {
        if ui.button("Switch source and destination").clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::FlipMulticonnection(FlipMulticonnection {}),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.domain_element.read().uuid),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.range_element.read().uuid),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::OpenTriangle),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.domain_element.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.range_element.read().uuid);

        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(DemoOfdElement::DemoOfdPrecedence(m)) = m.get(&old_model.uuid) {
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
        m: &HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.domain_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEventType(new_source)) = m.get(&source_uuid) {
            model.domain_element = new_source.clone();
        }
        let target_uuid = *model.range_element.read().uuid;
        if let Some(DemoOfdElement::DemoOfdEventType(new_target)) = m.get(&target_uuid) {
            model.range_element = new_target.clone();
        }
    }
}


fn new_demoofd_exclusion(
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (DemoOfdType, DemoOfdElementView),
    target: (DemoOfdType, DemoOfdElementView),
) -> (ERef<DemoOfdExclusion>, ERef<ExclusionViewT>) {
    let link_model = ERef::new(DemoOfdExclusion::new(
        uuid::Uuid::now_v7().into(),
        source.0,
        target.0,
    ));
    let link_view = new_demoofd_exclusion_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_demoofd_exclusion_view(
    model: ERef<DemoOfdExclusion>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: DemoOfdElementView,
    target: DemoOfdElementView,
) -> ERef<ExclusionViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.domain_element.uuid()), *m.range_element.uuid(), target.min_shape(), center_point);

    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        DemoOfdExclusionAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoOfdExclusionAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoOfdExclusion>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: DemoOfdExclusionTemporaries,
}

#[derive(Clone, Default)]
struct DemoOfdExclusionTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    midpoint_label: Arc<String>,

    comment_buffer: String,
}

impl MulticonnectionAdapter<DemoOfdDomain> for DemoOfdExclusionAdapter {
    fn model(&self) -> DemoOfdElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.temporaries.midpoint_label.clone())
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
        commands: &mut Vec<SensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>
    ) -> PropertiesStatus<DemoOfdDomain> {
        if ui.button("Switch source and destination").clicked() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoOfdPropChange::FlipMulticonnection(FlipMulticonnection {}),
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
                DemoOfdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoOfdElementOrVertex, DemoOfdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoOfdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoOfdPropChange::CommentChange(model.comment.clone())],
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

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.domain_element.uuid()),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.range_element.uuid()),
            ArrowData::new_labelless(canvas::LineType::Dashed, canvas::ArrowheadType::None),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.domain_element.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.range_element.uuid());

        self.temporaries.midpoint_label = Arc::new("X".to_owned());
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoOfdElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(DemoOfdElement::DemoOfdExclusion(m)) = m.get(&old_model.uuid) {
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
        m: &HashMap<ModelUuid, DemoOfdElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.domain_element.uuid();
        if let Some(new_source) = m.get(&source_uuid).and_then(|e| e.clone().as_type()) {
            model.domain_element = new_source.clone();
        }
        let target_uuid = *model.range_element.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.clone().as_type()) {
            model.range_element = new_target.clone();
        }
    }
}
