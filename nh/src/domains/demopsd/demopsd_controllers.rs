use crate::common::canvas::{self, Highlight, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GlobalDrawingContext, InputEvent, InsensitiveCommand, MGlobalColor, Model, MultiDiagramController, PositionNoT, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, View
};
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::ufoption::UFOption;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::domains::demopsd::demopsd_models::{DemoPsdState, DemoPsdStateInfo};
use super::demopsd_models::{
    DemoPsdDiagram, DemoPsdElement, DemoPsdLink, DemoPsdLinkType, DemoPsdPackage, DemoPsdTransaction, DemoPsdAct, DemoPsdFact,
};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::{CustomModal, CustomModalResult};
use eframe::{egui, epaint};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc},
};
use super::super::demo::{
    INTERNAL_ROLE_BACKGROUND, EXTERNAL_ROLE_BACKGROUND,
    PERFORMA_DETAIL, INFORMA_DETAIL, FORMA_DETAIL,
    DemoTransactionKind,
};

pub struct DemoPsdDomain;
impl Domain for DemoPsdDomain {
    type CommonElementT = DemoPsdElement;
    type DiagramModelT = DemoPsdDiagram;
    type CommonElementViewT = DemoPsdElementView;
    type ViewTargettingSectionT = DemoPsdElementTargettingSection;
    type QueryableT<'a> = DemoPsdQueryable<'a>;
    type ToolT = NaiveDemoPsdTool;
    type AddCommandElementT = DemoPsdElementOrVertex;
    type PropChangeT = DemoPsdPropChange;
}

type PackageViewT = PackageView<DemoPsdDomain, DemoPsdPackageAdapter>;
type LinkViewT = MulticonnectionView<DemoPsdDomain, DemoPsdLinkAdapter>;

pub struct DemoPsdQueryable<'a> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, DemoPsdElementView>,
}

impl<'a> Queryable<'a, DemoPsdDomain> for DemoPsdQueryable<'a> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, DemoPsdElementView>,
    ) -> Self {
        Self { models_to_views, flattened_views }
    }

    fn get_view(&self, m: &ModelUuid) -> Option<DemoPsdElementView> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).cloned()
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

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
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
}

impl DemoPsdElementView {
    fn as_state_view(self) -> Option<DemoPsdStateView> {
        match self {
            DemoPsdElementView::Fact(inner) => Some(inner.into()),
            DemoPsdElementView::Act(inner) => Some(inner.into()),
            DemoPsdElementView::Package(..)
            | DemoPsdElementView::Transaction(..)
            | DemoPsdElementView::Link(..) => None,
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
        q: &DemoPsdQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
        pos: egui::Pos2,
        text_align: egui::Align2,
    ) -> TargettingStatus {
        match self {
            DemoPsdStateView::Fact(inner) => inner.write().draw_inner(q, context, canvas, tool, pos, text_align),
            DemoPsdStateView::Act(inner) => inner.write().draw_inner(q, context, canvas, tool, pos, text_align),
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
}

impl Into<DemoPsdElement> for DemoPsdElementTargettingSection {
    fn into(self) -> DemoPsdElement {
        match self {
            DemoPsdElementTargettingSection::Package(inner) => inner.into(),
            DemoPsdElementTargettingSection::Transaction(inner, ..) => inner.into(),
            DemoPsdElementTargettingSection::Fact(inner) => inner.into(),
            DemoPsdElementTargettingSection::Act(inner) => inner.into(),
            DemoPsdElementTargettingSection::Link(inner) => inner.into(),
        }
    }
}


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct DemoPsdDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: DemoPsdDiagramBuffer,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    placeholders: DemoPsdPlaceholderViews,
}

#[derive(Clone, Default)]
struct DemoPsdDiagramBuffer {
    name: String,
    comment: String,
}

#[derive(Clone)]
struct DemoPsdPlaceholderViews {
    views: [DemoPsdElementView; 6],
}

impl Default for DemoPsdPlaceholderViews {
    fn default() -> Self {
        let (ta, ta_view) = new_demopsd_transaction("01", "", vec![], None, vec![], egui::Pos2::new(100.0, 75.0), 200.0);
        let ta = (ta, ta_view.into());

        let (fact, fact_view) = new_demopsd_fact("rq", true, egui::Pos2::ZERO);
        let fact = (fact, fact_view.into());
        let (act, act_view) = new_demopsd_act("rq", true, egui::Pos2::new(100.0, 75.0));
        let act = (act, act_view.into());

        let (_response, response_view) = new_demopsd_link(DemoPsdLinkType::ResponseLink, fact.clone(), act.clone(), None);
        let (_wait, wait_view) = new_demopsd_link(DemoPsdLinkType::WaitLink, fact.clone(), act.clone(), None);

        let (_package, package_view) = new_demopsd_package("A package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });

        Self {
            views: [
                ta.1,
                fact.1,
                act.1,
                response_view.into(),
                wait_view.into(),
                package_view.into(),
            ],
        }
    }
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
            placeholders: Default::default(),
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

    fn create_new_view_for(
        &self,
        q: &DemoPsdQueryable<'_>,
        element: DemoPsdElement,
    ) -> Result<DemoPsdElementView, HashSet<ModelUuid>> {
        let v = match element {
            DemoPsdElement::DemoPsdPackage(inner) => {
                DemoPsdElementView::from(
                    new_demopsd_package_view(
                        inner,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            },
            DemoPsdElement::DemoPsdTransaction(inner) => {
                let r = inner.read();

                let f = |e: &DemoPsdStateInfo| DemoPsdStateViewInfo {
                    view: match &e.state {
                        DemoPsdState::Fact(inner) => new_demopsd_fact_view(inner.clone(), egui::Pos2::ZERO).into(),
                        DemoPsdState::Act(inner) => new_demopsd_act_view(inner.clone(), egui::Pos2::ZERO).into(),
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

                DemoPsdElementView::from(
                    new_demopsd_transaction_view(inner.clone(), before, p_act, after, egui::Pos2::ZERO, 200.0)
                )
            },
            DemoPsdElement::DemoPsdFact(inner) => {
                DemoPsdElementView::from(
                    new_demopsd_fact_view(inner, egui::Pos2::ZERO)
                )
            }
            DemoPsdElement::DemoPsdAct(inner) => {
                DemoPsdElementView::from(
                    new_demopsd_act_view(inner, egui::Pos2::ZERO)
                )
            }
            DemoPsdElement::DemoPsdLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.read().uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                DemoPsdElementView::from(
                    new_demopsd_link_view(
                        inner.clone(),
                        source_view,
                        target_view,
                        None,
                    )
                )
            },
        };

        Ok(v)
    }
    fn label_for(&self, e: &DemoPsdElement) -> Arc<String> {
        match e {
            DemoPsdElement::DemoPsdPackage(inner) => {
                inner.read().name.clone()
            },
            DemoPsdElement::DemoPsdTransaction(inner) => {
                let r = inner.read();
                let mut l = format!("Transaction {}", r.identifier);
                if !r.name.is_empty() {
                    l.push_str(" (");
                    l.push_str(&r.name);
                    l.push_str(&")");
                }

                Arc::new(l)
            },
            DemoPsdElement::DemoPsdFact(inner) => {
                let r = inner.read();
                let mut l = format!("Fact");
                if !r.identifier.is_empty() {
                    l.push_str(" (");
                    l.push_str(&r.identifier);
                    l.push_str(&")");
                }

                Arc::new(l)
            }
            DemoPsdElement::DemoPsdAct(inner) => {
                let r = inner.read();
                let mut l = format!("Act");
                if !r.identifier.is_empty() {
                    l.push_str(" (");
                    l.push_str(&r.identifier);
                    l.push_str(&")");
                }

                Arc::new(l)
            }
            DemoPsdElement::DemoPsdLink(inner) => {
                Arc::new(inner.read().link_type.char().to_owned())
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
    ) -> PropertiesStatus<DemoPsdDomain> {
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
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
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
                    vec![DemoPsdPropChange::NameChange(Arc::new(self.buffer.name.clone()))],
                )
                .into(),
            );
        };

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
                    vec![DemoPsdPropChange::CommentChange(Arc::new(
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
        command: &InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoPsdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    DemoPsdPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color })],
                        ));
                        self.background_color = *color;
                    }
                    DemoPsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::CommentChange(model.comment.clone())],
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
        tool: &mut Option<NaiveDemoPsdTool>,
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
        let c = |s: DemoPsdToolStage| -> egui::Color32 {
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
        let empty_q = DemoPsdQueryable::new(&empty_a, &empty_b);
        let mut icon_counter = 0;
        for cat in [
            &[
                (DemoPsdToolStage::TransactionStart, "Transaction"),
                (DemoPsdToolStage::Fact, "Fact"),
                (DemoPsdToolStage::Act, "Act"),
            ][..],
            &[
                (
                    DemoPsdToolStage::LinkStart {
                        link_type: DemoPsdLinkType::ResponseLink,
                    },
                    "Response Link",
                ),
                (
                    DemoPsdToolStage::LinkStart {
                        link_type: DemoPsdLinkType::WaitLink,
                    },
                    "Wait Link",
                ),
            ][..],
            &[(DemoPsdToolStage::PackageStart, "Package")][..],
        ] {
            for (stage, name) in cat {
                let response = ui.add_sized([width, button_height], egui::Button::new(*name).fill(c(*stage)));
                if response.clicked() {
                    if let Some(t) = &tool && t.initial_stage == *stage {
                        *tool = None;
                    } else {
                        *tool = Some(NaiveDemoPsdTool::new(*stage));
                    }
                }

                let icon_rect = egui::Rect::from_min_size(response.rect.min, egui::Vec2::splat(button_height));
                let painter = ui.painter().with_clip_rect(icon_rect);
                let mut mc = canvas::MeasuringCanvas::new(&painter);
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut mc, &None);
                let (scale, offset) = mc.scale_offset_to_fit(egui::Vec2::new(button_height, button_height));
                let mut c = canvas::UiCanvas::new(false, painter, icon_rect, offset, scale, None, Highlight::NONE);
                c.clear(egui::Color32::GRAY);
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut c, &None);
                icon_counter += 1;
            }
            ui.separator();
        }
    }

    fn menubar_options_fun(
        &self,
        _view_uuid: &ViewUuid,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) {}

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

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, DemoPsdElement>) {
        let models = super::demopsd_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }

    fn transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::demopsd_models::transitive_closure(&self.model.read(), when_deleting)
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
        ERef::new(
            MultiDiagramController::new(
                ControllerUuid::now_v7(),
                "demopsd-diagram",
                model.clone(),
                vec![
                    DiagramControllerGen2::new(
                        uuid.into(),
                        name.into(),
                        DemoPsdDiagramAdapter::new(model),
                        elements,
                    )
                ]
            )
        )
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New DEMO PSD diagram {}", no);

    let diagram = ERef::new(DemoPsdDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let fact = new_demopsd_fact("TK04/ac", false, egui::Pos2::new(375.0, 300.0));
    let act = new_demopsd_act("", false, egui::Pos2::new(200.0, 400.0));
    let wait_link = new_demopsd_link(
        DemoPsdLinkType::WaitLink,
        (fact.0.clone(), fact.1.clone().into()),
        (act.0.clone(), act.1.clone().into()),
        Some((ViewUuid::now_v7(), egui::Pos2::new(300.0, 300.0))),
    );

    let (tx01, tx01_view) = new_demopsd_transaction(
        "01", "usufruct case concluding",
        vec![], None, vec![],
        egui::Pos2::new(200.0, 100.0), 350.0,
    );
    let (tx02, tx02_view) = new_demopsd_transaction(
        "02", "resource seizing",
        vec![], None, vec![],
        egui::Pos2::new(100.0, 200.0), 150.0,
    );
    let (tx03, tx03_view) = new_demopsd_transaction(
        "03", "resource releasing",
        vec![], Some(act.clone()), vec![],
        egui::Pos2::new(300.0, 200.0), 150.0,
    );

    // TODO: states

    let models = vec![
        tx01.into(),
        tx02.into(),
        tx03.into(),
        fact.0.into(),
        wait_link.0.into(),
    ];
    let views = vec![
        tx01_view.into(),
        tx02_view.into(),
        tx03_view.into(),
        fact.1.into(),
        wait_link.1.into(),
    ];

    {
        let name = format!("Demo DEMO PSD diagram {}", no);
        let diagram = ERef::new(DemoPsdDiagram::new(
            ModelUuid::now_v7(),
            name.clone(),
            models,
        ));
        new_controlller(diagram, name, views)
    }
}

pub fn deserializer(uuid: ControllerUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<DemoPsdDomain, DiagramControllerGen2<DemoPsdDomain, DemoPsdDiagramAdapter>>>(&uuid)?)
}

#[derive(Clone, Copy, PartialEq)]
pub enum DemoPsdToolStage {
    TransactionStart,
    TransactionEnd,
    Fact,
    Act,
    LinkStart { link_type: DemoPsdLinkType },
    LinkEnd,
    PackageStart,
    PackageEnd,
}

enum PartialDemoPsdElement {
    None,
    Some(DemoPsdElementView),
    TransactionStart {
        start_pos: egui::Pos2,
    },
    Link {
        link_type: DemoPsdLinkType,
        source: ERef<DemoPsdFact>,
        dest: Option<ERef<DemoPsdAct>>,
    },
    Package {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveDemoPsdTool {
    initial_stage: DemoPsdToolStage,
    current_stage: DemoPsdToolStage,
    result: PartialDemoPsdElement,
    event_lock: bool,
}

impl NaiveDemoPsdTool {
    pub fn new(initial_stage: DemoPsdToolStage) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialDemoPsdElement::None,
            event_lock: false,
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<DemoPsdDomain> for NaiveDemoPsdTool {
    type Stage = DemoPsdToolStage;

    fn initial_stage(&self) -> DemoPsdToolStage {
        self.initial_stage
    }

    fn targetting_for_section(&self, element: Option<DemoPsdElementTargettingSection>) -> egui::Color32 {
        type TS = DemoPsdElementTargettingSection;
        match element {
            None => match self.current_stage {
                DemoPsdToolStage::TransactionStart
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact
                | DemoPsdToolStage::Act
                | DemoPsdToolStage::PackageStart
                | DemoPsdToolStage::PackageEnd => TARGETTABLE_COLOR,
                DemoPsdToolStage::LinkStart { .. }
                | DemoPsdToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(TS::Package(..)) => match self.current_stage {
                DemoPsdToolStage::TransactionStart
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact
                | DemoPsdToolStage::Act
                | DemoPsdToolStage::PackageStart
                | DemoPsdToolStage::PackageEnd => TARGETTABLE_COLOR,
                DemoPsdToolStage::LinkStart { .. }
                | DemoPsdToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(TS::Transaction(tx, align)) => {
                if align == egui::Align2::CENTER_CENTER {
                    return if self.current_stage == DemoPsdToolStage::Act && !tx.read().p_act.is_some() {
                        TARGETTABLE_COLOR
                    } else {
                        NON_TARGETTABLE_COLOR
                    };
                }

                if matches!(self.current_stage, DemoPsdToolStage::Fact | DemoPsdToolStage::Act) {
                    TARGETTABLE_COLOR
                } else {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(TS::Fact(..)) => match self.current_stage {
                DemoPsdToolStage::LinkStart { .. } => TARGETTABLE_COLOR,
                DemoPsdToolStage::TransactionStart
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact
                | DemoPsdToolStage::Act
                | DemoPsdToolStage::LinkEnd
                | DemoPsdToolStage::PackageStart
                | DemoPsdToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            }
            Some(TS::Act(..)) => match self.current_stage {
                DemoPsdToolStage::LinkEnd => TARGETTABLE_COLOR,
                DemoPsdToolStage::TransactionStart
                | DemoPsdToolStage::TransactionEnd
                | DemoPsdToolStage::Fact
                | DemoPsdToolStage::Act
                | DemoPsdToolStage::LinkStart { .. }
                | DemoPsdToolStage::PackageStart
                | DemoPsdToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            }
            Some(TS::Link(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &DemoPsdQueryable, canvas: &mut dyn canvas::NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialDemoPsdElement::TransactionStart { start_pos } => {
                canvas.draw_line(
                    [*start_pos, egui::Pos2::new(pos.x, start_pos.y)],
                    canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            PartialDemoPsdElement::Link {
                source,
                link_type,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.read().uuid()) {
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

        match (self.current_stage, &mut self.result) {
            (DemoPsdToolStage::TransactionStart, _) => {
                self.result = PartialDemoPsdElement::TransactionStart { start_pos: pos };
                self.current_stage = DemoPsdToolStage::TransactionEnd;
                self.event_lock = true;
            }
            (DemoPsdToolStage::TransactionEnd, PartialDemoPsdElement::TransactionStart { start_pos }) => {
                let rect = egui::Rect::from_two_pos(
                    egui::Pos2::new(start_pos.x, start_pos.y),
                    egui::Pos2::new(pos.x, start_pos.y),
                );
                let (_transaction_model, transaction_view) =
                    new_demopsd_transaction("01", "", vec![], None, vec![], rect.center(), rect.width());
                self.result = PartialDemoPsdElement::Some(transaction_view.into());
                self.current_stage = DemoPsdToolStage::TransactionStart;
                self.event_lock = true;
            }
            (DemoPsdToolStage::Fact, _) => {
                let (_fact_model, fact_view) = new_demopsd_fact("", true, pos);
                self.result = PartialDemoPsdElement::Some(fact_view.into());
                self.event_lock = true;
            }
            (DemoPsdToolStage::Act, _) => {
                let (_act_model, act_view) = new_demopsd_act("", true, pos);
                self.result = PartialDemoPsdElement::Some(act_view.into());
                self.event_lock = true;
            }
            (DemoPsdToolStage::PackageStart, _) => {
                self.result = PartialDemoPsdElement::Package { a: pos, b: None };
                self.current_stage = DemoPsdToolStage::PackageEnd;
                self.event_lock = true;
            }
            (DemoPsdToolStage::PackageEnd, PartialDemoPsdElement::Package { b, .. }) => {
                *b = Some(pos)
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
            TS::Package(..)
            | TS::Transaction(..) => {},
            TS::Fact(inner) => if let DemoPsdToolStage::LinkStart { link_type } = self.current_stage {
                self.result = PartialDemoPsdElement::Link { link_type: link_type, source: inner, dest: None };
                self.current_stage = DemoPsdToolStage::LinkEnd;
                self.event_lock = true;
            } else {},
            TS::Act(inner) => if let DemoPsdToolStage::LinkEnd = self.current_stage
                && let PartialDemoPsdElement::Link { dest, .. } = &mut self.result {
                *dest = Some(inner);
                self.event_lock = true;
            } else {},
            TS::Link(..) => {}
        }
    }

    fn try_additional_dependency(&mut self) -> Option<(BucketNoT, ModelUuid, ModelUuid)> {
        None
    }

    fn try_construct_view(
        &mut self,
        into: &dyn ContainerGen2<DemoPsdDomain>,
    ) -> Option<(DemoPsdElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialDemoPsdElement::Some(x) => {
                let x = x.clone();
                self.result = PartialDemoPsdElement::None;
                let esm: Option<Box<dyn CustomModal>> = match &x {
                    DemoPsdElementView::Transaction(inner) => {
                        Some(Box::new(DemoPsdTransactionSetupModal::from(&inner.read().model)))
                    },
                    DemoPsdElementView::Fact(..)
                    | DemoPsdElementView::Act(..) => None,
                    DemoPsdElementView::Package(..)
                    | DemoPsdElementView::Link(..) => unreachable!(),
                };
                Some((x, esm))
            }
            PartialDemoPsdElement::Link {
                source,
                dest: Some(target),
                link_type,
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *target.read().uuid());
                if let (Some(source_view), Some(target_view)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = self.initial_stage;

                    let (_link_model, link_view) = new_demopsd_link(
                        *link_type,
                        (source.clone(), source_view),
                        (target.clone(), target_view),
                        None,
                    );

                    self.result = PartialDemoPsdElement::None;

                    Some((link_view.into(), None))
                } else {
                    None
                }
            }
            PartialDemoPsdElement::Package { a, b: Some(b) } => {
                self.current_stage = DemoPsdToolStage::PackageStart;

                let (_package_model, package_view) =
                    new_demopsd_package("A package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialDemoPsdElement::None;
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
pub struct DemoPsdPackageAdapter {
    #[nh_context_serde(entity)]
    model: ERef<DemoPsdPackage>,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
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

    fn insert_element(&mut self, position: Option<PositionNoT>, e: DemoPsdElement) -> Result<PositionNoT, ()> {
        self.model.write().insert_element(0, position, e).map_err(|_| ())
    }
    fn delete_element(&mut self, uuids: &ModelUuid) -> Option<PositionNoT> {
        self.model.write().remove_element(uuids).map(|e| e.1)
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>
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
                DemoPsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoPsdPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::NameChange(model.name.clone())],
                        ));
                        model.name = name.clone();
                    }
                    DemoPsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::CommentChange(model.comment.clone())],
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
        m: &mut HashMap<ModelUuid, DemoPsdElement>,
    ) -> Self where Self: Sized {
        let model_uuid = *self.model.read().uuid;
        let model = if let Some(DemoPsdElement::DemoPsdPackage(m)) = m.get(&model_uuid) {
            m.clone()
        } else {
            let model = self.model.read();
            let model = ERef::new(DemoPsdPackage::new(new_uuid, (*model.name).clone(), model.contained_elements.clone()));
            m.insert(model_uuid, model.clone().into());
            model
        };
        Self { model, name_buffer: self.name_buffer.clone(), comment_buffer: self.comment_buffer.clone() }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DemoPsdElement>,
    ) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()) {
                *e = new_model.clone();
            }
        }
    }
}

fn new_demopsd_package(
    name: &str,
    bounds_rect: egui::Rect,
) -> (ERef<DemoPsdPackage>, ERef<PackageViewT>) {
    let graph_model = ERef::new(DemoPsdPackage::new(
        ModelUuid::now_v7(),
        name.to_owned(),
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
        DemoTransactionKind::Performa,
        identifier.to_owned(),
        name.to_owned(),
        before_models,
        p_act_model,
        after_models,
    ));
    let tx_view = new_demopsd_transaction_view(tx_model.clone(), before_views, p_act_view, after_views, position, width);
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
        _gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Transaction Kind:");
        egui::ComboBox::from_id_salt("Transaction Kind:")
            .selected_text(self.kind_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoTransactionKind::Performa,
                    DemoTransactionKind::Informa,
                    DemoTransactionKind::Forma,
                ] {
                    ui.selectable_value(&mut self.kind_buffer, value, value.char());
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
            if ui.button("Ok").clicked() {
                let mut m = self.model.write();
                m.kind = self.kind_buffer;
                m.identifier = Arc::new(self.identifier_buffer.clone());
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

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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
            self.tx_outer_rectangle.min.x + self.tx_outer_rectangle.width() * self.tx_mark_percentage,
            self.tx_outer_rectangle.center().y,
        );
        let delta = tx_mark_center - pos;

        if delta.x.abs() + delta.y.abs() <= radius {
            (
                self.model.clone(),
                egui::Align2::CENTER_CENTER,
            )
        } else {
            let quadrant = match (pos.x > tx_mark_center.x, pos.y > tx_mark_center.y) {
                (false, false) => egui::Align2::LEFT_TOP,
                (false, true) => egui::Align2::LEFT_BOTTOM,
                (true, true) => egui::Align2::RIGHT_BOTTOM,
                (true, false) => egui::Align2::RIGHT_TOP,
            };
            (
                self.model.clone(),
                quadrant,
            )
        }
    }

    fn state_insertion_place(&self, quadrant: egui::Align2, pos: egui::Pos2) -> (PositionNoT, egui::Rect) {
        let tx_mark_center = egui::Pos2::new(
            self.tx_outer_rectangle.min.x + self.tx_outer_rectangle.width() * self.tx_mark_percentage,
            self.tx_outer_rectangle.center().y,
        );
        let states_total = match quadrant.x() {
            egui::Align::Min => self.before_views.len(),
            egui::Align::Center => unreachable!(),
            egui::Align::Max => self.after_views.len(),
        };
        let (quadrant_start_x, quadrant_width) = match quadrant.x() {
            egui::Align::Min => (self.tx_outer_rectangle.min.x, self.tx_mark_percentage * self.tx_outer_rectangle.width()),
            egui::Align::Center => unreachable!(),
            egui::Align::Max => (tx_mark_center.x, (1.0 - self.tx_mark_percentage) * self.tx_outer_rectangle.width()),
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
            egui::Align::Min if current_state_idx >= states_total as f32 => state_width + Self::MIN_SIZE.x / 2.0,
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
        canvas::NHShape::Rect { inner: self.tx_outer_rectangle }
    }
    fn position(&self) -> egui::Pos2 {
        self.tx_outer_rectangle.center()
    }
}

impl ContainerGen2<DemoPsdDomain> for DemoPsdTransactionView {
    fn controller_for(&self, _uuid: &ModelUuid) -> Option<<DemoPsdDomain as Domain>::CommonElementViewT> {
        None
    }
}

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdTransactionView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &DemoPsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) -> PropertiesStatus<DemoPsdDomain> {
        // try before
        if let Some(child) = self.before_views.iter_mut()
            .flat_map(|e| e.view.show_properties(gdc, q, ui, commands).to_non_default()).next() {
            return child;
        }
        // try P-act
        if let Some(child) = self.p_act_view.as_mut()
                .and_then(|c| c.write().show_properties(gdc, q, ui, commands).to_non_default()) {
            return child;
        }
        // try after
        if let Some(child) = self.after_views.iter_mut()
            .flat_map(|e| e.view.show_properties(gdc, q, ui, commands).to_non_default()).next() {
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
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoPsdPropChange::TransactionKindChange(self.kind_buffer),
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
                DemoPsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
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
                DemoPsdPropChange::NameChange(Arc::new(self.name_buffer.clone())),
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
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position();

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.position().x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.position().y)));
            }
        });

        ui.horizontal(|ui| {
            let mut width = self.tx_outer_rectangle.width();
            let mark_deadzone = 2500.0 / width;
            let mut mark_percentage = self.tx_mark_percentage * 100.0;

            ui.label("width");
            if ui.add(egui::DragValue::new(&mut width).range(Self::MIN_SIZE.x..=5000.0).speed(1.0)).changed() {
                commands.push(SensitiveCommand::ResizeSelectedElementsBy(
                    egui::Align2::LEFT_CENTER,
                    egui::Vec2::new(width - self.tx_outer_rectangle.width(), 0.0),
                ));
            }
            ui.label("mark percentage");
            if ui.add(egui::DragValue::new(&mut mark_percentage).range(mark_deadzone..=(100.0-mark_deadzone)).speed(1.0)).changed() {
                commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                    DemoPsdPropChange::TransactionPercentageChange(mark_percentage / 100.0),
                ]));
            }
        });

        PropertiesStatus::Shown
    }
    fn draw_in(
        &mut self,
        q: &DemoPsdQueryable,
        context: &GlobalDrawingContext,
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
            self.tx_outer_rectangle.min.x + self.tx_outer_rectangle.width() * self.tx_mark_percentage,
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
        let width_before = self.tx_outer_rectangle.width() * self.tx_mark_percentage - Self::MIN_SIZE.x / 2.0;
        for (idx, e) in self.before_views.iter_mut().enumerate().map(|(idx, e)| ((idx + 1) as f32, e)) {
            let (pos_y, align) = if !e.executor {
                (self.tx_outer_rectangle.min.y, egui::Align2::CENTER_TOP)
            } else {
                (self.tx_outer_rectangle.max.y, egui::Align2::CENTER_BOTTOM)
            };

            child_targetting_drawn |= e.view.draw_inner(
                q, context, canvas, tool,
                egui::Pos2::new(self.tx_outer_rectangle.min.x + width_before * idx / spaces_count, pos_y),
                align,
            ) == TargettingStatus::Drawn;
        }
        // draw P-act
        if let UFOption::Some(e) = &self.p_act_view {
            child_targetting_drawn |= e.write().draw_inner(
                q, context, canvas, tool,
                egui::Pos2::new(tx_mark_center.x, self.tx_outer_rectangle.max.y),
                egui::Align2::LEFT_TOP,
            ) == TargettingStatus::Drawn;
        }
        // draw after
        let spaces_count = (self.after_views.len() + 1) as f32;
        let width_after = self.tx_outer_rectangle.width() * (1.0 - self.tx_mark_percentage) - Self::MIN_SIZE.x / 2.0;
        for (idx, e) in self.after_views.iter_mut().enumerate().map(|(idx, e)| ((idx + 1) as f32, e)) {
            let (pos_y, align) = if !e.executor {
                (self.tx_outer_rectangle.min.y, egui::Align2::CENTER_TOP)
            } else {
                (self.tx_outer_rectangle.max.y, egui::Align2::CENTER_BOTTOM)
            };

            child_targetting_drawn |= e.view.draw_inner(
                q, context, canvas, tool,
                egui::Pos2::new(tx_mark_center.x + Self::MIN_SIZE.x / 2.0 + width_after * idx / spaces_count, pos_y),
                align,
            ) == TargettingStatus::Drawn;
        }

        if let Some((pos, tool)) = tool && !child_targetting_drawn {
            let section = self.section_for(*pos);
            if section.1 == egui::Align2::CENTER_CENTER {
                canvas.draw_polygon(
                    vec![
                        tx_mark_center - egui::Vec2::new(0.0, radius),
                        tx_mark_center + egui::Vec2::new(radius, 0.0),
                        tx_mark_center + egui::Vec2::new(0.0, radius),
                        tx_mark_center - egui::Vec2::new(radius, 0.0),
                        tx_mark_center - egui::Vec2::new(0.0, radius),
                    ],
                    tool.targetting_for_section(Some(section.into())),
                    canvas::Stroke::new_solid(1.0, detail_color),
                    canvas::Highlight::NONE,
                );
                return TargettingStatus::Drawn;
            } else if self.tx_outer_rectangle.contains(*pos) {
                canvas.draw_rectangle(
                    self.state_insertion_place(section.1, *pos).1,
                    egui::CornerRadius::ZERO,
                    tool.targetting_for_section(Some(section.into())),
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                draw_tx_mark(canvas);
                return TargettingStatus::Drawn;
            }
        }

        TargettingStatus::NotDrawn
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoPsdTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) -> EventHandlingStatus {
        let child_status = self.before_views.iter_mut()
            .flat_map(|e| {
                let s = e.view.handle_event(event, ehc, tool, element_setup_modal, commands);
                if s != EventHandlingStatus::NotHandled {
                    Some((*e.view.uuid(), s))
                } else {
                    None
                }
            })
            .next();
        let child_status = child_status.or_else(|| self.p_act_view.as_ref().and_then(|e| {
            let mut w = e.write();
            let s = w.handle_event(event, ehc, tool, element_setup_modal, commands);
            if s != EventHandlingStatus::NotHandled {
                Some((*w.uuid(), s))
            } else {
                None
            }
        }));
        let child_status = child_status.or_else(|| self.after_views.iter_mut()
            .flat_map(|e| {
                let s = e.view.handle_event(event, ehc, tool, element_setup_modal, commands);
                if s != EventHandlingStatus::NotHandled {
                    Some((*e.view.uuid(), s))
                } else {
                    None
                }
            }).next());


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
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(k).collect(),
                                    true,
                                    Highlight::SELECTED,
                                )
                                .into(),
                            );
                        } else {
                            commands.push(
                                InsensitiveCommand::HighlightSpecific(
                                    std::iter::once(k).collect(),
                                    !self.selected_direct_elements.contains(&k),
                                    Highlight::SELECTED,
                                )
                                .into(),
                            );
                        }
                        return EventHandlingStatus::HandledByContainer;
                    }
                    Some((_, EventHandlingStatus::HandledByContainer)) => {
                        return EventHandlingStatus::HandledByContainer;
                    }
                    _ => {}
                }


                if !self.min_shape().contains(pos) {
                    return child_status.map(|e| e.1).unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    let section = self.section_for(pos);
                    let quadrant = section.1;
                    tool.add_section(section.into());

                    if self.p_act_view.as_ref().is_none() || quadrant != egui::Align2::CENTER_CENTER {
                        tool.add_position(pos);
                        if let Some((new_e, esm)) = tool.try_construct_view(self) {
                            if (quadrant == egui::Align2::CENTER_CENTER
                                && !self.model.read().p_act.is_some()
                                && matches!(new_e, DemoPsdElementView::Act(_)))
                               || (quadrant != egui::Align2::CENTER_CENTER
                                   && matches!(new_e, DemoPsdElementView::Act(_) | DemoPsdElementView::Fact(_))) {
                                let quadrant_no = match quadrant {
                                    egui::Align2::CENTER_CENTER => 0,
                                    egui::Align2::LEFT_TOP => 1,
                                    egui::Align2::LEFT_BOTTOM => 2,
                                    egui::Align2::RIGHT_BOTTOM => 3,
                                    egui::Align2::RIGHT_TOP => 4,
                                    _ => unreachable!(),
                                };
                                let pos = self.state_insertion_place(quadrant, pos).0;

                                commands.push(InsensitiveCommand::AddDependency(*self.uuid, quadrant_no, Some(pos), new_e.into(), true).into());
                                if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                                    *element_setup_modal = esm;
                                }
                            }
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
            InputEvent::Drag { delta, .. } if self.dragged_rect.is_some() => {
                let translated_real_rect = self.dragged_rect.unwrap().translate(delta);
                self.dragged_rect = Some(translated_real_rect);
                let translated_shape = canvas::NHShape::Rect { inner: translated_real_rect };
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
        command: &InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            ($self:ident) => {
                for e in &mut self.before_views {
                    e.view.apply_command(
                        command,
                        undo_accumulator,
                        affected_models,
                    );
                }
                if let UFOption::Some(e) = &self.p_act_view {
                    e.write().apply_command(
                        command,
                        undo_accumulator,
                        affected_models,
                    );
                }
                for e in &mut self.after_views {
                    e.view.apply_command(
                        command,
                        undo_accumulator,
                        affected_models,
                    );
                }
            };
        }
        macro_rules! resize_by {
            ($align:expr, $delta:expr) => {
                let min_delta_x = Self::MIN_SIZE.x - self.tx_outer_rectangle.width();
                let (left, right) = match $align.x() {
                    egui::Align::Min => (0.0, $delta.x.max(min_delta_x)),
                    egui::Align::Center => (0.0, 0.0),
                    egui::Align::Max => ((-$delta.x).max(min_delta_x), 0.0),
                };

                let r = self.tx_outer_rectangle + epaint::MarginF32{left, right, top: 0.0, bottom: 0.0};

                undo_accumulator.push(InsensitiveCommand::ResizeSpecificElementsTo(
                    std::iter::once(*self.uuid).collect(),
                    *$align,
                    self.tx_outer_rectangle.size(),
                ));
                self.tx_outer_rectangle = r;
            };
        }

        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);

                if h.selected {
                    match set {
                        true => {
                            // TODO: before
                            if let UFOption::Some(e) = &self.p_act_view {
                                self.selected_direct_elements.insert(*e.read().uuid);
                            }
                            // TODO: after
                        }
                        false => self.selected_direct_elements.clear(),
                    }
                }

                recurse!(self);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }

                if h.selected {
                    // TODO: before
                    if let UFOption::Some(e) = &self.p_act_view {
                        let k = *e.read().uuid;
                        match set {
                            true => self.selected_direct_elements.insert(k),
                            false => self.selected_direct_elements.remove(&k),
                        };
                    }
                    // TODO: after
                }

                recurse!(self);
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
                recurse!(self);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!(self);
            }
            InsensitiveCommand::MoveSpecificElements(_, delta)
            | InsensitiveCommand::MoveAllElements(delta) => {
                self.tx_outer_rectangle = self.tx_outer_rectangle.translate(*delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
            }

            InsensitiveCommand::ResizeSpecificElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    resize_by!(align, delta);
                }
                recurse!(self);
            }
            InsensitiveCommand::ResizeSpecificElementsTo(uuids, align, size) => {
                if uuids.contains(&self.uuid) {
                    let delta_naive = *size - self.tx_outer_rectangle.size();
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
                recurse!(self);
            }

            InsensitiveCommand::DeleteSpecificElements(uuids, from_model) => {
                if *from_model {
                    let mut w = self.model.write();

                    if let Some(e) = self.p_act_view.as_ref()
                        && uuids.contains(&e.read().uuid) {
                        undo_accumulator.push(InsensitiveCommand::AddDependency(
                            *self.uuid,
                            0,
                            None,
                            DemoPsdElementOrVertex::Element(e.clone().into()),
                            true,
                        ));
                        w.p_act = UFOption::None;
                        self.p_act_view = UFOption::None;
                    }

                    let mut closure = |e: &DemoPsdStateViewInfo| if uuids.contains(&e.view.uuid())
                        && let Some((b, pos)) = w.remove_element(&e.view.model_uuid()) {
                            undo_accumulator.push(InsensitiveCommand::AddDependency(
                                *self.uuid,
                                b,
                                Some(pos),
                                DemoPsdElementOrVertex::Element(e.view.clone().as_element_view()),
                                true,
                            ));
                            false
                        } else { true };
                    self.before_views.retain(&mut closure);
                    self.after_views.retain(&mut closure);
                }
                recurse!(self);
            }
            InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..) => {}
            InsensitiveCommand::AddDependency(v, b, pos, e, into_model) => {
                if *v == *self.uuid && *into_model {
                    let mut w = self.model.write();
                    if *b == 0 {
                        if self.p_act_view.as_ref().is_none()
                            && let DemoPsdElementOrVertex::Element(DemoPsdElementView::Act(e)) = e {
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                                *v,
                                *b,
                                *e.read().uuid,
                                true,
                            ));
                            affected_models.insert(*e.read().model_uuid());
                            w.p_act = UFOption::Some(e.read().model.clone());
                            self.p_act_view = UFOption::Some(e.clone());
                        }
                    } else {
                        if let DemoPsdElementOrVertex::Element(e) = e
                            && let Some(e) = e.clone().as_state_view()
                            && let Ok(pos) = w.insert_element(*b, *pos, e.model()) {
                            let after = match b {
                                1 | 2 => false,
                                3 | 4 => true,
                                _ => unreachable!(),
                            };
                            let executor = match b {
                                1 | 4 => false,
                                2 | 3 => true,
                                _ => unreachable!()
                            };

                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                                *v,
                                *b,
                                *e.uuid(),
                                true,
                            ));
                            affected_models.insert(*e.model_uuid());

                            if !after {
                                self.before_views.insert(pos.try_into().unwrap(), DemoPsdStateViewInfo { view: e, executor });
                            } else {
                                self.after_views.insert(pos.try_into().unwrap(), DemoPsdStateViewInfo { view: e, executor });
                            }
                        }
                    }
                }
                recurse!(self);
            }
            InsensitiveCommand::RemoveDependency(v, b, duuid, into_model) => {
                if *v == *self.uuid && *into_model {
                    let mut w = self.model.write();
                    if *b == 0 {
                        if let Some(e) = self.p_act_view.as_ref()
                            && *duuid == *e.read().uuid {
                            undo_accumulator.push(InsensitiveCommand::AddDependency(
                                *v,
                                *b,
                                None,
                                DemoPsdElementOrVertex::Element(e.clone().into()),
                                true,
                            ));
                            w.p_act = UFOption::None;
                            self.p_act_view = UFOption::None;
                        }
                    } else {
                        let closure = |e: &DemoPsdStateViewInfo| if *e.view.uuid() == *duuid
                            && let Some((b, pos)) = w.remove_element(&e.view.model_uuid()) {
                                undo_accumulator.push(InsensitiveCommand::AddDependency(
                                    *v,
                                    b,
                                    Some(pos),
                                    DemoPsdElementOrVertex::Element(e.view.clone().as_element_view()),
                                    true,
                                ));
                                false
                            } else { true };
                        match b {
                            1 | 2 => self.before_views.retain(closure),
                            3 | 4 => self.after_views.retain(closure),
                            _ => unreachable!(),
                        };
                    }
                }
                recurse!(self);
            }
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    for property in properties {
                        match property {
                            DemoPsdPropChange::TransactionKindChange(kind) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::TransactionKindChange(
                                        model.kind,
                                    )],
                                ));
                                model.kind = *kind;
                            }
                            DemoPsdPropChange::IdentifierChange(identifier) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                model.identifier = identifier.clone();
                            }
                            DemoPsdPropChange::NameChange(name) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::NameChange(
                                        model.name.clone(),
                                    )],
                                ));
                                model.name = name.clone();
                            }
                            DemoPsdPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::CommentChange(model.comment.clone())],
                                ));
                                model.comment = comment.clone();
                            }
                            DemoPsdPropChange::TransactionPercentageChange(percentage) => {
                                let w = 25.0 / self.tx_outer_rectangle.width();
                                let new_percentage = percentage.clamp(w, 1.0 - w);

                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::TransactionPercentageChange(
                                        self.tx_mark_percentage,
                                    )],
                                ));
                                self.tx_mark_percentage = new_percentage;
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
        self.comment_buffer = (*model.comment).clone();

        for e in &mut self.before_views {
            e.view.refresh_buffers();
        }
        if let UFOption::Some(e) = &self.p_act_view {
            let mut w = e.write();
            w.refresh_buffers();
        }
        for e in &mut self.after_views {
            e.view.refresh_buffers();
        }
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DemoPsdElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        let mut flattened_status_temp = HashMap::new();
        for e in &mut self.before_views {
            e.view.head_count(flattened_views, &mut flattened_status_temp, flattened_represented_models);
            flattened_views.insert(*e.view.uuid(), e.view.clone().as_element_view());
        }
        if let UFOption::Some(e) = &self.p_act_view {
            let mut w = e.write();
            w.head_count(flattened_views, &mut flattened_status_temp, flattened_represented_models);
            flattened_views.insert(*w.uuid(), e.clone().into());
        }
        for e in &mut self.after_views {
            e.view.head_count(flattened_views, &mut flattened_status_temp, flattened_represented_models);
            flattened_views.insert(*e.view.uuid(), e.view.clone().as_element_view());
        }

        flattened_status_temp.iter().for_each(|e| {
            let s = match e.1 {
                SelectionStatus::NotSelected if self.highlight.selected => SelectionStatus::TransitivelySelected,
                a => *a,
            };
            flattened_views_status.insert(*e.0, s);
        });
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DemoPsdElementView>,
        c: &mut HashMap<ViewUuid, DemoPsdElementView>,
        m: &mut HashMap<ModelUuid, DemoPsdElement>
    ) {
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let new_before_views = self.before_views.iter().map(|e| {
            e.view.deep_copy_clone(uuid_present, &mut HashMap::new(), c, m);
            DemoPsdStateViewInfo {
                view: c.get(&e.view.uuid()).and_then(|e| e.clone().as_state_view()).unwrap(),
                executor: e.executor,
            }
        }).collect();
        let new_p_act_view = if let UFOption::Some(e) = &self.p_act_view {
            e.write().deep_copy_clone(uuid_present, &mut HashMap::new(), c, m);
            if let Some(DemoPsdElementView::Act(e)) = c.get(&e.read().uuid()) {
                Some(e.clone())
            } else { None }
        } else { None }.into();
        let new_after_views = self.after_views.iter().map(|e| {
            e.view.deep_copy_clone(uuid_present, &mut HashMap::new(), c, m);
            DemoPsdStateViewInfo {
                view: c.get(&e.view.uuid()).and_then(|e| e.clone().as_state_view()).unwrap(),
                executor: e.executor,
            }
        }).collect();

        let modelish = if let Some(DemoPsdElement::DemoPsdTransaction(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

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

    fn deep_copy_relink(
        &mut self,
        _c: &HashMap<ViewUuid, <DemoPsdDomain as Domain>::CommonElementViewT>,
        m: &HashMap<ModelUuid, <DemoPsdDomain as Domain>::CommonElementT>,
    ) {
        let mut w = self.model.write();
        for e in w.before.iter_mut() {
            let e_uuid = *e.state.uuid();
            if let Some(new_state) = m.get(&e_uuid).and_then(|e| e.clone().to_state()) {
                e.state = new_state;
            }
        }
        if let UFOption::Some(p_act) = &w.p_act {
            let p_act_uuid = *p_act.read().uuid;
            if let Some(DemoPsdElement::DemoPsdAct(new_p_act)) = m.get(&p_act_uuid) {
                w.p_act = UFOption::Some(new_p_act.clone());
            }
        }
        for e in w.after.iter_mut() {
            let e_uuid = *e.state.uuid();
            if let Some(new_state) = m.get(&e_uuid).and_then(|e| e.clone().to_state()) {
                e.state = new_state;
            }
        }
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
fn new_demopsd_fact_view(
    model: ERef<DemoPsdFact>,
    position: egui::Pos2,
) -> ERef<DemoPsdFactView> {
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
        _q: &DemoPsdQueryable,
        _gdc: &GlobalDrawingContext,
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
            self.position + egui::Vec2::new(
                match text_align.x() {
                    egui::Align::Min => 1.5 * Self::RADIUS.x,
                    egui::Align::Center => 0.0,
                    egui::Align::Max => -1.5 * Self::RADIUS.x,
                },
                match text_align.y() {
                    egui::Align::Min => Self::RADIUS.y,
                    egui::Align::Center => 0.0,
                    egui::Align::Max => -Self::RADIUS.y,
                }
            ),
            text_align,
            &read.identifier,
            canvas::CLASS_BOTTOM_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw targetting rectangle
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_ellipse(
                self.position,
                Self::RADIUS,
                t.targetting_for_section(Some(self.model.clone().into())),
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

impl ContainerGen2<DemoPsdDomain> for DemoPsdFactView {}

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdFactView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        _parent: &DemoPsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) -> PropertiesStatus<DemoPsdDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.identifier_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoPsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ]));
        }

        if ui.checkbox(&mut self.internal_buffer, "Internal").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoPsdPropChange::StateInternalChange(self.internal_buffer),
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
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        q: &DemoPsdQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
    ) -> TargettingStatus {
        self.draw_inner(q, context, canvas, tool, self.position, egui::Align2::LEFT_CENTER)
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoPsdTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
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
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                }
                EventHandlingStatus::NotHandled
            }
            e if !self.min_shape().contains(*e.mouse_position()) => {
                return EventHandlingStatus::NotHandled
            }
            InputEvent::MouseDown(_) => {
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(_) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model.clone().into());
                } else {
                    if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                        commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                        commands.push(
                            InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*self.uuid).collect(),
                                true,
                                Highlight::SELECTED,
                            )
                            .into(),
                        );
                    } else {
                        commands.push(
                            InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*self.uuid).collect(),
                                !self.highlight.selected,
                                Highlight::SELECTED,
                            )
                            .into(),
                        );
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
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
                            DemoPsdPropChange::IdentifierChange(identifier) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                model.identifier = identifier.clone();
                            }
                            DemoPsdPropChange::StateInternalChange(internal) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::StateInternalChange(model.internal)],
                                ));
                                model.internal = *internal;
                            }
                            DemoPsdPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::CommentChange(model.comment.clone())],
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
        self.identifier_buffer = (*model.identifier).clone();
        self.internal_buffer = model.internal;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, DemoPsdElementView>,
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
        m: &mut HashMap<ModelUuid, DemoPsdElement>
    ) {
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoPsdElement::DemoPsdFact(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
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
fn new_demopsd_act_view(
    model: ERef<DemoPsdAct>,
    position: egui::Pos2,
) -> ERef<DemoPsdActView> {
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
        _q: &DemoPsdQueryable,
        _gdc: &GlobalDrawingContext,
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
                }
            ),
            text_align,
            &read.identifier,
            canvas::CLASS_BOTTOM_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw targetting rectangle
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                t.targetting_for_section(Some(self.model.clone().into())),
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

impl ContainerGen2<DemoPsdDomain> for DemoPsdActView {}

impl ElementControllerGen2<DemoPsdDomain> for DemoPsdActView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        _parent: &DemoPsdQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) -> PropertiesStatus<DemoPsdDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        ui.label("Identifier:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.identifier_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoPsdPropChange::IdentifierChange(Arc::new(self.identifier_buffer.clone())),
            ]));
        }

        if ui.checkbox(&mut self.internal_buffer, "Internal").changed() {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoPsdPropChange::StateInternalChange(self.internal_buffer),
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
                DemoPsdPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ]));
        }

        ui.label("View properties");

        ui.horizontal(|ui| {
            let egui::Pos2 { mut x, mut y } = self.position();

            ui.label("x");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.position().x, 0.0)));
            }
            ui.label("y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.position().y)));
            }
        });

        PropertiesStatus::Shown
    }
    fn draw_in(
        &mut self,
        q: &DemoPsdQueryable,
        context: &GlobalDrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveDemoPsdTool)>,
    ) -> TargettingStatus {
        self.draw_inner(q, context, canvas, tool, self.bounds_rect.center(), egui::Align2::LEFT_CENTER)
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveDemoPsdTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
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
            InputEvent::MouseUp(_) => {
                if self.dragged_shape.is_some() {
                    self.dragged_shape = None;
                }
                EventHandlingStatus::NotHandled
            }
            e if !self.min_shape().contains(*e.mouse_position()) => {
                return EventHandlingStatus::NotHandled
            }
            InputEvent::MouseDown(_) => {
                self.dragged_shape = Some(self.min_shape());
                EventHandlingStatus::HandledByElement
            }
            InputEvent::Click(_) => {
                if let Some(tool) = tool {
                    tool.add_section(self.model.clone().into());
                } else {
                    if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                        commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                        commands.push(
                            InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*self.uuid).collect(),
                                true,
                                Highlight::SELECTED,
                            )
                            .into(),
                        );
                    } else {
                        commands.push(
                            InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*self.uuid).collect(),
                                !self.highlight.selected,
                                Highlight::SELECTED,
                            )
                            .into(),
                        );
                    }
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
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
                self.bounds_rect = self.bounds_rect.translate(*delta);
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
                            DemoPsdPropChange::IdentifierChange(identifier) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::IdentifierChange(
                                        model.identifier.clone(),
                                    )],
                                ));
                                model.identifier = identifier.clone();
                            }
                            DemoPsdPropChange::StateInternalChange(internal) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::StateInternalChange(model.internal)],
                                ));
                                model.internal = *internal;
                            }
                            DemoPsdPropChange::CommentChange(comment) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![DemoPsdPropChange::CommentChange(model.comment.clone())],
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
        self.identifier_buffer = (*model.identifier).clone();
        self.internal_buffer = model.internal;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, DemoPsdElementView>,
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
        m: &mut HashMap<ModelUuid, DemoPsdElement>
    ) {
        let old_model = self.model.read();
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(DemoPsdElement::DemoPsdAct(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
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
    source: (
        ERef<DemoPsdFact>,
        DemoPsdElementView,
    ),
    target: (
        ERef<DemoPsdAct>,
        DemoPsdElementView,
    ),
    center_point: Option<(ViewUuid, egui::Pos2)>,
) -> (ERef<DemoPsdLink>, ERef<LinkViewT>) {
    let link_model = ERef::new(DemoPsdLink::new(
        ModelUuid::now_v7(),
        link_type,
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

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
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

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.model.read().multiplicity.clone())
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
        commands: &mut Vec<SensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>
    ) -> PropertiesStatus<DemoPsdDomain> {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("Type:")
            .selected_text(self.temporaries.link_type_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    DemoPsdLinkType::ResponseLink,
                    DemoPsdLinkType::WaitLink,
                ] {
                    if ui
                        .selectable_value(&mut self.temporaries.link_type_buffer, value, value.char())
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            DemoPsdPropChange::LinkTypeChange(self.temporaries.link_type_buffer),
                        ]));
                    }
                }
            });

        ui.label("Multiplicity:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.temporaries.multiplicity_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoPsdPropChange::LinkMultiplicityChange(Arc::new(self.temporaries.multiplicity_buffer.clone())),
            ]));
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.temporaries.comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                DemoPsdPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ]));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DemoPsdElementOrVertex, DemoPsdPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, properties) = command {
            let mut model = self.model.write();
            for property in properties {
                match property {
                    DemoPsdPropChange::LinkTypeChange(link_type) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::LinkTypeChange(model.link_type)],
                        ));
                        model.link_type = *link_type;
                    }
                    DemoPsdPropChange::LinkMultiplicityChange(multiplicity) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::LinkMultiplicityChange(model.multiplicity.clone())],
                        ));
                        model.multiplicity = multiplicity.clone();
                    }
                    DemoPsdPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![DemoPsdPropChange::CommentChange(model.comment.clone())],
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
        let line_type = self.line_type();
        self.temporaries.arrow_data.insert((false, *model.source.read().uuid), ArrowData::new_labelless(
            line_type,
            canvas::ArrowheadType::None,
        ));
        self.temporaries.arrow_data.insert((true, *model.target.read().uuid), ArrowData::new_labelless(
            line_type,
            canvas::ArrowheadType::FullTriangle,
        ));

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.read().uuid);

        self.temporaries.link_type_buffer = model.link_type;
        self.temporaries.multiplicity_buffer = (*model.multiplicity).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DemoPsdElement>
    ) -> Self where Self: Sized {
        let model = self.model.read();
        let model = if let Some(DemoPsdElement::DemoPsdLink(m)) = m.get(&model.uuid) {
            m.clone()
        } else {
            let modelish = model.clone_with(new_uuid);
            m.insert(*model.uuid, modelish.clone().into());
            modelish
        };
        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DemoPsdElement>
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.source.read().uuid();
        if let Some(DemoPsdElement::DemoPsdFact(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }

        let target_uuid = *model.target.read().uuid();
        if let Some(DemoPsdElement::DemoPsdAct(new_target)) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}
