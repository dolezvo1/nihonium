
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerModel, ControllerAdapter, DiagramAdapter, DiagramController, DiagramControllerGen2, DiagramSettings, DiagramSettings2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider, MGlobalColor, Model, MultiDiagramController, PositionNoT, ProjectCommand, PropertiesStatus, Queryable, RequestType, SelectionStatus, SnapManager, TargettingStatus, Tool, ToolPalette, TryMerge, View
};
use crate::common::ui_ext::UiExt;
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::common::views::multiconnection_view::{self, ArrowData, Ending, FlipMulticonnection, MulticonnectionAdapter, MulticonnectionView, VertexInformation};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::domains::network::network_models::{NetworkAssociation, NetworkAssociationArrowheadType, NetworkAssociationLineType, NetworkComment, NetworkContainer, NetworkContainerShapeKind, NetworkDiagram, NetworkElement, NetworkNode, NetworkNodeKind, NetworkUser, NetworkUserKind};
use crate::{CustomModal};
use eframe::egui;
use std::any::Any;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct NetworkDomain;
impl Domain for NetworkDomain {
    type SettingsT = NetworkSettings;
    type CommonElementT = NetworkElement;
    type DiagramModelT = NetworkDiagram;
    type CommonElementViewT = NetworkElementView;
    type ViewTargettingSectionT = NetworkElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveNetworkTool;
    type AddCommandElementT = NetworkElementOrVertex;
    type PropChangeT = NetworkPropChange;
}

type PackageViewT = PackageView<NetworkDomain, NetworkContainerAdapter>;
type LinkViewT = MulticonnectionView<NetworkDomain, NetworkAssociationAdapter>;

#[derive(Clone)]
pub enum NetworkPropChange {
    NameChange(Arc<String>),

    NodeKindChange(NetworkNodeKind),
    UserKindChange(NetworkUserKind),

    AssociationLineTypeChange(NetworkAssociationLineType),
    AssociationArrowheadTypeChange(/*target?*/ bool, NetworkAssociationArrowheadType),
    AssociationMultiplicityChange(/*target?*/ bool, Arc<String>),
    AssociationRoleChange(/*target?*/ bool, Arc<String>),
    AssociationReadingChange(/*target?*/ bool, Arc<String>),
    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
}

impl Debug for NetworkPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "NetworkPropChange::{}",
            match self {
                Self::NameChange(name) => format!("NameChange({})", name),
                Self::NodeKindChange(_kind) => format!("NodeKindChange(..)"),
                Self::UserKindChange(_kind) => format!("UserKindChange(..)"),

                Self::AssociationLineTypeChange(_) => format!("AssociationLineTypeChange(..)"),
                Self::AssociationArrowheadTypeChange(..) => format!("AssociationArrowheadTypeChange(..)"),
                Self::AssociationMultiplicityChange(..) => format!("AssociationMultiplicityChange(..)"),
                Self::AssociationRoleChange(..) => format!("AssociationRoleChange(..)"),
                Self::AssociationReadingChange(..) => format!("AssociationReadingChange(..)"),
                Self::FlipMulticonnection(_) => format!("FlipMulticonnection"),

                Self::ColorChange(_color) => format!("ColorChange(..)"),
                Self::CommentChange(comment) => format!("CommentChange({})", comment),
            }
        )
    }
}

impl TryFrom<&NetworkPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &NetworkPropChange) -> Result<Self, Self::Error> {
        match value {
            NetworkPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for NetworkPropChange {
    fn from(value: ColorChangeData) -> Self {
        NetworkPropChange::ColorChange(value)
    }
}
impl TryFrom<NetworkPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: NetworkPropChange) -> Result<Self, Self::Error> {
        match value {
            NetworkPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for NetworkPropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self> where Self: Sized {
        match (self, newer) {
            (Self::NameChange(_), Self::NameChange(newer)) => Some(Self::NameChange(newer.clone())),
            (Self::AssociationMultiplicityChange(b1, _), Self::AssociationMultiplicityChange(b2, newer))
                if b1 == b2 => Some(Self::AssociationMultiplicityChange(*b1, newer.clone())),
            (Self::AssociationRoleChange(b1, _), Self::AssociationRoleChange(b2, newer))
                if b1 == b2 => Some(Self::AssociationRoleChange(*b1, newer.clone())),
            (Self::AssociationReadingChange(b1, _), Self::AssociationReadingChange(b2, newer))
                if b1 == b2 => Some(Self::AssociationReadingChange(*b1, newer.clone())),
            (Self::CommentChange(_), Self::CommentChange(newer)) => Some(Self::CommentChange(newer.clone())),
            _ => None
        }
    }
}

#[derive(Clone, derive_more::From, derive_more::TryInto)]
pub enum NetworkElementOrVertex {
    Element(NetworkElementView),
    Vertex(VertexInformation),
}

impl Debug for NetworkElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "NetworkElementOrVertex::???")
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "NetworkDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum NetworkElementView {
    Container(ERef<PackageViewT>),
    Node(ERef<NetworkNodeView>),
    User(ERef<NetworkUserView>),
    Predicate(ERef<LinkViewT>),
    Comment(ERef<NetworkCommentView>),
}


#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct NetworkControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<NetworkDiagram>,
}

impl ControllerAdapter<NetworkDomain> for NetworkControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<NetworkDomain, NetworkDiagramAdapter>;

    fn model(&self) -> ERef<NetworkDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<NetworkDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "network"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::network_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn insert_element(&mut self, parent: ModelUuid, element: NetworkElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
        self.model.write().insert_element_into(parent, element, b, p)
    }

    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, NetworkElement, BucketNoT, PositionNoT)>) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(&self, _gdc: &GlobalDrawingContext, ui: &mut egui::Ui) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("Network Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Network Diagram".to_owned().into(),
                NetworkDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct NetworkDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<NetworkDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: NetworkDiagramBuffer,
}

#[derive(Clone, Default)]
struct NetworkDiagramBuffer {
    name: String,
    comment: String,
}

impl NetworkDiagramAdapter {
    fn new(model: ERef<NetworkDiagram>) -> Self {
        let m = model.read();
         Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: NetworkDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<NetworkDomain> for NetworkDiagramAdapter {
    fn model(&self) -> ERef<NetworkDiagram> {
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
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        element: NetworkElement,
    ) -> Result<NetworkElementView, HashSet<ModelUuid>> {
        let v = match element {
            NetworkElement::Container(inner) => {
                new_network_container_view(
                    inner,
                    egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                ).into()
            },
            NetworkElement::Node(inner) => {
                new_network_node_view(inner, egui::Pos2::ZERO).into()
            },
            NetworkElement::User(inner) => {
                new_network_user_view(inner, egui::Pos2::ZERO).into()
            },
            NetworkElement::Association(inner) => {
                let m = inner.read();
                let (sid, tid) = (*m.source.uuid(), *m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([sid, tid])),
                };
                new_network_association_view(
                    inner.clone(),
                    source_view,
                    target_view,
                ).into()
            },
            NetworkElement::Comment(inner) => {
                new_network_comment_view(inner, egui::Pos2::ZERO).into()
            },
        };

        Ok(v)
    }
    fn label_for(&self, e: &NetworkElement) -> Arc<String> {
        match e {
            NetworkElement::Container(inner) => {
                format!("Container ({})", LabelProvider::filter_and_elipsis(&inner.read().name)).into()
            },
            NetworkElement::Node(inner) => {
                format!("Node ({})", LabelProvider::filter_and_elipsis(&inner.read().name)).into()
            },
            NetworkElement::User(inner) => {
                format!("User ({})", LabelProvider::filter_and_elipsis(&inner.read().name)).into()
            },
            NetworkElement::Association(_inner) => {
                "Association".to_owned().into()
            },
            NetworkElement::Comment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
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
    ) -> PropertiesStatus<NetworkDomain> {
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
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) {
        if ui.labeled_text_edit_singleline("Name:", &mut self.buffer.name).changed() {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    NetworkPropChange::NameChange(Arc::new(self.buffer.name.clone())),
                ),
            );
        };

        if ui.labeled_text_edit_multiline("Comment:", &mut self.buffer.comment).changed() {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    NetworkPropChange::CommentChange(Arc::new(
                        self.buffer.comment.clone(),
                    )),
                ),
            );
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                NetworkPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
                    ));
                    self.background_color = *color;
                }
                NetworkPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::CommentChange(model.comment.clone()),
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

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, NetworkElement>) {
        let (new_model, models) = super::network_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, NetworkElement>) {
        let models = super::network_models::fake_copy_diagram(&self.model.read());
        (self.clone(), models)
    }
}

fn new_controlller(
    model: ERef<NetworkDiagram>,
    name: String,
    elements: Vec<NetworkElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(
            MultiDiagramController::new(
                ControllerUuid::now_v7(),
                NetworkControllerAdapter { model: model.clone() },
                vec![
                    DiagramControllerGen2::new(
                        uuid.into(),
                        name.into(),
                        NetworkDiagramAdapter::new(model),
                        elements,
                    )
                ]
            )
        )
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New Network diagram {}", no);

    let diagram = ERef::new(NetworkDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (internet, internet_view) = new_network_node("Cloud", NetworkNodeKind::Cloud, egui::Pos2::new(200.0, 200.0));
    let (router, router_view) = new_network_node("Router", NetworkNodeKind::Router, egui::Pos2::new(300.0, 400.0));
    let (swtch, swtch_view) = new_network_node("Switch", NetworkNodeKind::Switch, egui::Pos2::new(400.0, 200.0));
    let (workstation, workstation_view) = new_network_node("Workstation", NetworkNodeKind::Workstation, egui::Pos2::new(500.0, 400.0));
    let (user, user_view) = new_network_user("User", NetworkUserKind::Normal, egui::Pos2::new(600.0, 200.0));

    let name = format!("Demo Network diagram {}", no);
    let diagram = ERef::new(NetworkDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            internet.into(),
            router.into(),
            swtch.into(),
            workstation.into(),
            user.into(),
        ],
    ));
    new_controlller(diagram, name, vec![
        internet_view.into(),
        router_view.into(),
        swtch_view.into(),
        workstation_view.into(),
        user_view.into(),
    ])
}

pub fn deserializer(uuid: ControllerUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<NetworkDomain, NetworkControllerAdapter, DiagramControllerGen2<NetworkDomain, NetworkDiagramAdapter>>>(&uuid)?)
}


pub struct NetworkSettings {
    palette: RwLock<ToolPalette<NetworkToolStage, NetworkElementView>>,
}
impl DiagramSettings for NetworkSettings {}
impl DiagramSettings2<NetworkDomain> for NetworkSettings {
    fn palette_for_each_mut<F>(&self, f: F)
        where F: FnMut(&mut (uuid::Uuid, &'static str, Vec<(uuid::Uuid, NetworkToolStage, &'static str, NetworkElementView)>))
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let (node_model, node_view) = new_network_node("Workstation", NetworkNodeKind::Workstation, egui::Pos2::new(100.0, 75.0));
    let node = (node_model.into(), node_view.into());
    let (_laptop_model, laptop_view) = new_network_node("Laptop", NetworkNodeKind::Laptop, egui::Pos2::ZERO);
    let (_router_model, router_view) = new_network_node("Router", NetworkNodeKind::Router, egui::Pos2::ZERO);
    let (_switch_model, switch_view) = new_network_node("Switch", NetworkNodeKind::Switch, egui::Pos2::ZERO);

    let (user_model, user_view) = new_network_user("User", NetworkUserKind::Normal, egui::Pos2::ZERO);
    let user = (user_model.into(), user_view.into());
    let (_developer_model, developer_view) = new_network_user("Developer", NetworkUserKind::Developer, egui::Pos2::ZERO);
    let (_audit_model, audit_view) = new_network_user("Audit", NetworkUserKind::Audit, egui::Pos2::ZERO);
    let (_blackhat_model, blackhat_view) = new_network_user("Black Hat", NetworkUserKind::BlackHat, egui::Pos2::ZERO);

    let (_association1, association1_view) = new_network_association(
        NetworkAssociationLineType::Solid,
        user.clone(), NetworkAssociationArrowheadType::None,
        node.clone(), NetworkAssociationArrowheadType::None,
    );
    let (_association2, association2_view) = new_network_association(
        NetworkAssociationLineType::Solid,
        user.clone(), NetworkAssociationArrowheadType::None,
        node.clone(), NetworkAssociationArrowheadType::OpenTriangle,
    );
    let (_association3, association3_view) = new_network_association(
        NetworkAssociationLineType::Dashed,
        user.clone(), NetworkAssociationArrowheadType::None,
        node.clone(), NetworkAssociationArrowheadType::None,
    );

    let (_container, container_view) = new_network_container(
        "Subnet", NetworkContainerShapeKind::Rectangle,
        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) },
    );
    let (_comment, comment_view) = new_network_comment("text", egui::Pos2::ZERO);

    let palette_items = vec![
        ("Nodes", vec![
            (NetworkToolStage::Node { name: "Workstation", kind: NetworkNodeKind::Workstation }, "Workstation", node.1),
            (NetworkToolStage::Node { name: "Laptop", kind: NetworkNodeKind::Laptop }, "Laptop", laptop_view.into()),
            (NetworkToolStage::Node { name: "Router", kind: NetworkNodeKind::Router }, "Router", router_view.into()),
            (NetworkToolStage::Node { name: "Switch", kind: NetworkNodeKind::Switch }, "Switch", switch_view.into()),
        ]),
        ("Users", vec![
            (NetworkToolStage::User { name: "User", kind: NetworkUserKind::Normal }, "User", user.1),
            (NetworkToolStage::User { name: "Developer", kind: NetworkUserKind::Developer }, "Developer", developer_view.into()),
            (NetworkToolStage::User { name: "Audit", kind: NetworkUserKind::Audit }, "Audit", audit_view.into()),
            (NetworkToolStage::User { name: "Black Hat", kind: NetworkUserKind::BlackHat }, "Black Hat", blackhat_view.into()),
        ]),
        ("Relationships", vec![
            (NetworkToolStage::AssociationStart {
                line_type: NetworkAssociationLineType::Solid,
                source_arrowhead: NetworkAssociationArrowheadType::None,
                target_arrowhead: NetworkAssociationArrowheadType::None,
            }, "Association (solid)", association1_view.into()),
            (NetworkToolStage::AssociationStart {
                line_type: NetworkAssociationLineType::Solid,
                source_arrowhead: NetworkAssociationArrowheadType::None,
                target_arrowhead: NetworkAssociationArrowheadType::OpenTriangle,
            }, "Association (solid, arrow)", association2_view.into()),
            (NetworkToolStage::AssociationStart {
                line_type: NetworkAssociationLineType::Dashed,
                source_arrowhead: NetworkAssociationArrowheadType::None,
                target_arrowhead: NetworkAssociationArrowheadType::None,
            }, "Association (dashed)", association3_view.into()),
        ]),
        ("Other", vec![
            (NetworkToolStage::ContainerStart, "Container", container_view.into()),
            (NetworkToolStage::Comment, "Comment", comment_view.into()),
        ]),
    ];

    Box::new(NetworkSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
    })
}

pub fn settings_function(gdc: &mut GlobalDrawingContext, ui: &mut egui::Ui, s: &mut Box<dyn DiagramSettings>) {
    let Some(s) = (s.as_mut() as &mut dyn Any).downcast_mut::<NetworkSettings>() else { return; };

    s.palette.write().unwrap().show_treeview(gdc, ui);
}

#[derive(Clone, Copy, PartialEq)]
pub enum NetworkToolStage {
    Node { name: &'static str, kind: NetworkNodeKind },
    User { name: &'static str, kind: NetworkUserKind },
    AssociationStart {
        line_type: NetworkAssociationLineType,
        source_arrowhead: NetworkAssociationArrowheadType,
        target_arrowhead: NetworkAssociationArrowheadType,
    },
    AssociationEnd,
    ContainerStart,
    ContainerEnd,
    Comment,
}

enum PartialNetworkElement {
    None,
    Some(NetworkElementView),
    Association {
        line_type: NetworkAssociationLineType,
        source_arrowhead: NetworkAssociationArrowheadType,
        target_arrowhead: NetworkAssociationArrowheadType,
        source: NetworkElement,
        dest: Option<NetworkElement>,
    },
    Container {
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveNetworkTool {
    initial_stage: NetworkToolStage,
    current_stage: NetworkToolStage,
    result: PartialNetworkElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveNetworkTool {
    fn try_spend(&mut self) {
        self.result = PartialNetworkElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<NetworkDomain> for NaiveNetworkTool {
    type Stage = NetworkToolStage;

    fn new(initial_stage: NetworkToolStage, repeat: bool) -> Self {
        Self {
            initial_stage,
            current_stage: initial_stage,
            result: PartialNetworkElement::None,
            event_lock: false,
            is_spent: if repeat { None } else { Some(false) },
        }
    }
    fn initial_stage(&self) -> NetworkToolStage {
        self.initial_stage
    }
    fn repeats(&self) -> bool {
        self.is_spent.is_none()
    }
    fn is_spent(&self) -> bool {
        self.is_spent.is_some_and(|e| e)
    }

    fn targetting_for_section(&self, element: Option<NetworkElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                NetworkToolStage::Node { .. }
                | NetworkToolStage::User { .. }
                | NetworkToolStage::ContainerStart
                | NetworkToolStage::ContainerEnd
                | NetworkToolStage::Comment => TARGETTABLE_COLOR,
                NetworkToolStage::AssociationStart { .. }
                | NetworkToolStage::AssociationEnd => NON_TARGETTABLE_COLOR,
            },
            Some(NetworkElement::Container(..)) => match self.current_stage {
                NetworkToolStage::Node { .. }
                | NetworkToolStage::User { .. }
                | NetworkToolStage::Comment => TARGETTABLE_COLOR,
                NetworkToolStage::AssociationStart { .. }
                | NetworkToolStage::AssociationEnd
                | NetworkToolStage::ContainerStart
                | NetworkToolStage::ContainerEnd => NON_TARGETTABLE_COLOR,
            },
            Some(NetworkElement::Node(..) | NetworkElement::User(..) | NetworkElement::Comment(..)) => match self.current_stage {
                NetworkToolStage::AssociationStart { .. }
                | NetworkToolStage::AssociationEnd => TARGETTABLE_COLOR,
                NetworkToolStage::Node { .. }
                | NetworkToolStage::User { .. }
                | NetworkToolStage::ContainerStart
                | NetworkToolStage::ContainerEnd
                | NetworkToolStage::Comment => NON_TARGETTABLE_COLOR,
            },
            Some(NetworkElement::Association(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &<NetworkDomain as Domain>::QueryableT<'_>, canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialNetworkElement::Association { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialNetworkElement::Container { a, .. } => {
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
            (NetworkToolStage::Node { name, kind }, _) => {
                let (_, node_view) = new_network_node(name, kind, pos);
                self.result = PartialNetworkElement::Some(node_view.into());
                self.event_lock = true;
            }
            (NetworkToolStage::User { name, kind }, _) => {
                let (_, user_view) = new_network_user(name, kind, pos);
                self.result = PartialNetworkElement::Some(user_view.into());
                self.event_lock = true;
            }
            (NetworkToolStage::ContainerStart, _) => {
                self.result = PartialNetworkElement::Container { a: pos, b: None };
                self.current_stage = NetworkToolStage::ContainerEnd;
                self.event_lock = true;
            }
            (NetworkToolStage::ContainerEnd, PartialNetworkElement::Container { b, .. }) => *b = Some(pos),
            (NetworkToolStage::Comment, _) => {
                let (_, comment_view) = new_network_comment(
                    "text",
                    pos,
                );

                self.result = PartialNetworkElement::Some(comment_view.into());
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, section: NetworkElement) {
        if self.event_lock {
            return;
        }

        match section {
            NetworkElement::Container(..) => {}
            NetworkElement::Node(..) | NetworkElement::User(..)
            | NetworkElement::Comment(..) => match (self.current_stage, &mut self.result) {
                (NetworkToolStage::AssociationStart { line_type, source_arrowhead, target_arrowhead }, PartialNetworkElement::None) => {
                    let source = match section {
                        NetworkElement::Node(inner) => inner.into(),
                        NetworkElement::User(inner) => inner.into(),
                        NetworkElement::Comment(inner) => inner.into(),
                        _ => unreachable!(),
                    };
                    self.result = PartialNetworkElement::Association {
                        line_type,
                        source_arrowhead,
                        target_arrowhead,
                        source,
                        dest: None,
                    };
                    self.current_stage = NetworkToolStage::AssociationEnd;
                    self.event_lock = true;
                }
                (NetworkToolStage::AssociationEnd, PartialNetworkElement::Association { dest, .. }) => {
                    let target = match section {
                        NetworkElement::Node(inner) => inner.into(),
                        NetworkElement::User(inner) => inner.into(),
                        NetworkElement::Comment(inner) => inner.into(),
                        _ => unreachable!(),
                    };
                    *dest = Some(target);
                    self.event_lock = true;
                }
                _ => {}
            },
            NetworkElement::Association(..) => {},
        }
    }

    fn try_additional_dependency(&mut self) -> Option<(BucketNoT, ModelUuid, ModelUuid)> {
        None
    }

    fn try_construct_view(
        &mut self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        into: &ViewUuid,
    ) -> Option<(NetworkElementView, Option<Box<dyn CustomModal>>)> {
        match &self.result {
            PartialNetworkElement::Some(x) => {
                let x = x.clone();
                self.try_spend();
                Some((x, None))
            }
            PartialNetworkElement::Association {
                line_type,
                source_arrowhead,
                target_arrowhead,
                source,
                dest: Some(dest),
                ..
            } => {
                self.current_stage = self.initial_stage;

                let (source_uuid, target_uuid) = (*source.uuid(), *dest.uuid());
                let association_view: Option<(_, Option<Box<dyn CustomModal>>)> =
                    if let (Some(source_controller), Some(dest_controller)) = (
                        q.get_view_for(&source_uuid),
                        q.get_view_for(&target_uuid),
                    ) && q.is_contained(&source_controller.uuid(), into)
                      && q.is_contained(&dest_controller.uuid(), into)
                    {
                        let (_, association_view) = new_network_association(
                            *line_type,
                            (source.clone(), source_controller), *source_arrowhead,
                            (dest.clone(), dest_controller), *target_arrowhead,
                        );

                        Some((association_view.into(), None))
                    } else {
                        None
                    };

                self.try_spend();
                association_view
            }
            PartialNetworkElement::Container { a, b: Some(b) } => {
                self.current_stage = NetworkToolStage::ContainerStart;

                let (_, container_view) = new_network_container(
                    "Subnet", NetworkContainerShapeKind::Rectangle, egui::Rect::from_two_pos(*a, *b),
                );

                self.try_spend();
                Some((container_view.into(), None))
            }
            _ => None,
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}


fn new_network_container(
    name: &str,
    kind: NetworkContainerShapeKind,
    bounds_rect: egui::Rect,
) -> (ERef<NetworkContainer>, ERef<PackageViewT>) {
    let container_model = ERef::new(NetworkContainer::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        kind,
        Vec::new(),
    ));
    let container_view = new_network_container_view(container_model.clone(), bounds_rect);

    (container_model, container_view)
}
fn new_network_container_view(
    model: ERef<NetworkContainer>,
    bounds_rect: egui::Rect,
) -> ERef<PackageViewT> {
    let m = model.read();
    PackageView::new(
        ViewUuid::now_v7().into(),
        NetworkContainerAdapter {
            model: model.clone(),
            background_color: MGlobalColor::None,
            name_buffer: (*m.name).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct NetworkContainerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<NetworkContainer>,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<NetworkDomain> for NetworkContainerAdapter {
    fn model_section(&self) -> NetworkElement {
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
    fn insert_element(&mut self, position: Option<PositionNoT>, element: NetworkElement) -> Result<PositionNoT, ()> {
        self.model.write().insert_element(0, position, element).map_err(|_| ())
    }
    fn delete_element(&mut self, uuid: &ModelUuid) -> Option<PositionNoT> {
        self.model.write().remove_element(uuid).map(|e| e.1)
    }

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32 {
        global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE)
    }

    fn show_model_properties(
        &mut self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>
    ) {
        if ui.labeled_text_edit_singleline("Name:", &mut self.name_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui.labeled_text_edit_multiline("Comment:", &mut self.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
            ));
        }
    }
    fn show_color_property(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> PropertiesStatus<NetworkDomain> {
        ui.label("Background color:");
        if crate::common::controller::mglobalcolor_edit_button(
            &context.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                NetworkPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                NetworkPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
                    ));
                    self.background_color = *color;
                }
                _ => {}
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
        m: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(NetworkElement::Container(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(new_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };
        Self {
            model,
            background_color: self.background_color.clone(),
            name_buffer: self.name_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
        }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, NetworkElement>,
    ) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()) {
                *e = new_model.clone();
            }
        }
    }
}


fn new_network_node(
    name: &str,
    kind: NetworkNodeKind,
    position: egui::Pos2,
) -> (ERef<NetworkNode>, ERef<NetworkNodeView>) {
    let node_model = ERef::new(NetworkNode::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        kind,
    ));
    let node_view = new_network_node_view(node_model.clone(), position);
    (node_model, node_view)
}
fn new_network_node_view(
    model: ERef<NetworkNode>,
    position: egui::Pos2,
) -> ERef<NetworkNodeView> {
    let m = model.read();
    ERef::new(NetworkNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        name_buffer: (*m.name).to_owned(),
        kind_buffer: m.kind.clone(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_rect: egui::Rect::ZERO,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<NetworkNode>,

    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: NetworkNodeKind,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl Entity for NetworkNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for NetworkNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<NetworkElement> for NetworkNodeView {
    fn model(&self) -> NetworkElement {
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

impl ElementControllerGen2<NetworkDomain> for NetworkNodeView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) -> PropertiesStatus<NetworkDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui.labeled_text_edit_multiline("Name:", &mut self.name_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.char())
            .show_ui(ui, |ui| {
                for e in [
                    NetworkNodeKind::Cloud,
                    NetworkNodeKind::Firewall,
                    NetworkNodeKind::Router,
                    NetworkNodeKind::Switch,
                    NetworkNodeKind::Server,
                    NetworkNodeKind::Workstation,
                    NetworkNodeKind::Laptop,
                    NetworkNodeKind::Tablet,
                    NetworkNodeKind::CellularPhone,
                    NetworkNodeKind::UsbDrive,
                    NetworkNodeKind::OpticalMedia
                ] {
                    if ui.selectable_value(&mut self.kind_buffer, e, e.char()).changed() {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::NodeKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

        if ui.labeled_text_edit_multiline("Comment:", &mut self.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _q: &<NetworkDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &NetworkSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveNetworkTool)>,
    ) -> TargettingStatus {
        const INNER_SIZE: egui::Vec2 = egui::Vec2::new(40.0, 40.0);
        const OUTER_SIZE: egui::Vec2 = egui::Vec2::new(50.0, 50.0);
        // Draw shape and text
        let inner_rect = egui::Rect::from_center_size(self.position, INNER_SIZE);
        canvas.draw_rectangle(
            inner_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, OUTER_SIZE.y / 2.0),
            egui::Align2::CENTER_TOP,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        self.bounds_rect = egui::Rect::from_center_size(self.position, OUTER_SIZE);

        // draw icons based on kind
        match self.kind_buffer {
            NetworkNodeKind::Cloud => {
                const SIZE: egui::Vec2 = egui::Vec2::new(15.0, 8.0);
                const SHAPE_COLOR: egui::Color32 = egui::Color32::from_rgb(0xD0, 0xED, 0xEB);
                let stroke = canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT);
                canvas.draw_rectangle(
                    inner_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE.gamma_multiply(0.5),
                    stroke,
                    canvas::Highlight::NONE
                );
                canvas.draw_ellipse(
                    self.position + egui::Vec2::new(-5.0, -5.0),
                    egui::Vec2::new(10.0, 10.0),
                    SHAPE_COLOR,
                    stroke,
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    self.position + egui::Vec2::new(5.0, -5.0),
                    egui::Vec2::new(5.0, 5.0),
                    SHAPE_COLOR,
                    stroke,
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    self.position + egui::Vec2::new(5.0, 5.0),
                    SIZE,
                    SHAPE_COLOR,
                    stroke,
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    self.position + egui::Vec2::new(-5.0, 5.0),
                    SIZE,
                    SHAPE_COLOR,
                    stroke,
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::Firewall => {
                for e in 0..7 {
                    let r = egui::Rect::from_center_size(
                        inner_rect.center_top() + egui::Vec2::new(0.0, 5.0 + e as f32 * 5.0),
                        egui::Vec2::new(20.0, 5.0),
                    );
                    canvas.draw_rectangle(
                        r,
                        egui::CornerRadius::ZERO,
                        egui::Color32::RED,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE
                    );
                    let modifier = if e % 2 == 0 { egui::Vec2::new(-5.0, 0.0) } else { egui::Vec2::new(5.0, 0.0) };
                    canvas.draw_line(
                        [r.center_top() + modifier, r.center_bottom() + modifier],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE
                    );
                }
            },
            NetworkNodeKind::Router => {
                const COLOR: egui::Color32 = egui::Color32::from_rgb(0x08, 0xb8, 0xdb);
                canvas.draw_ellipse(
                    self.position,
                    egui::Vec2::splat(19.0),
                    COLOR,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_text(
                    self.position,
                    egui::Align2::CENTER_BOTTOM,
                    "↘ ↗",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::WHITE,
                );
                canvas.draw_text(
                    self.position,
                    egui::Align2::CENTER_TOP,
                    "↙ ↖",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::WHITE,
                );
            },
            NetworkNodeKind::Switch => {
                const COLOR: egui::Color32 = egui::Color32::from_rgb(0x08, 0xb8, 0xdb);
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(self.position, egui::Vec2::splat(38.0)),
                    egui::CornerRadius::ZERO,
                    COLOR,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_text(
                    self.position,
                    egui::Align2::CENTER_BOTTOM,
                    "↗↗",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::WHITE,
                );
                canvas.draw_text(
                    self.position,
                    egui::Align2::CENTER_TOP,
                    "↙↙",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::WHITE,
                );
            },
            NetworkNodeKind::Server => {
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-18.0, -18.0),
                        self.position + egui::Vec2::new(10.0, -18.0),
                        self.position + egui::Vec2::new(18.0, -8.0),
                        self.position + egui::Vec2::new(-10.0, -8.0),
                    ].to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-10.0, -8.0),
                        self.position + egui::Vec2::new(18.0, -8.0),
                        self.position + egui::Vec2::new(18.0, 18.0),
                        self.position + egui::Vec2::new(-10.0, 18.0),
                    ].to_vec(),
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-10.0, -8.0),
                        self.position + egui::Vec2::new(-10.0, 18.0),
                        self.position + egui::Vec2::new(-18.0, 8.0),
                        self.position + egui::Vec2::new(-18.0, -18.0),
                    ].to_vec(),
                    egui::Color32::GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::Workstation => {
                let screen_rect = egui::Rect::from_center_size(
                    self.position,
                    egui::Vec2::new(32.0, 18.0),
                );
                canvas.draw_rectangle(
                    screen_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    screen_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        screen_rect.center_bottom() + egui::Vec2::new(-8.0, 10.0),
                        screen_rect.center_bottom() + egui::Vec2::new(-4.0, 2.0),
                        screen_rect.center_bottom() + egui::Vec2::new(4.0, 2.0),
                        screen_rect.center_bottom() + egui::Vec2::new(8.0, 10.0),
                    ].to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::Laptop => {
                let screen_rect = egui::Rect::from_two_pos(
                    self.position + egui::Vec2::new(-13.0, -13.0),
                    self.position + egui::Vec2::new(13.0, -2.0),
                );
                canvas.draw_rectangle(
                    screen_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    screen_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        inner_rect.left_bottom() + egui::Vec2::new(2.0, -2.0),
                        screen_rect.left_bottom() + egui::Vec2::new(-2.0, 2.0),
                        screen_rect.right_bottom() + egui::Vec2::new(2.0, 2.0),
                        inner_rect.right_bottom() + egui::Vec2::new(-2.0, -2.0),
                    ].to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::Tablet => {
                let screen_rect = egui::Rect::from_center_size(
                    self.position,
                    egui::Vec2::new(32.0, 18.0),
                );
                canvas.draw_rectangle(
                    screen_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    screen_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::CellularPhone => {
                let screen_rect = egui::Rect::from_center_size(
                    self.position,
                    egui::Vec2::new(18.0, 32.0),
                );
                canvas.draw_rectangle(
                    screen_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    screen_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::UsbDrive => {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(
                        self.position + egui::Vec2::new(-8.0, -18.0),
                        self.position + egui::Vec2::new(8.0, 8.0),
                    ),
                    egui::CornerRadius::ZERO,
                    egui::Color32::DARK_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(
                        self.position + egui::Vec2::new(-5.0, 8.0),
                        self.position + egui::Vec2::new(5.0, 18.0),
                    ),
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(
                        self.position + egui::Vec2::new(-3.0, 12.0),
                        self.position + egui::Vec2::new(-1.0, 14.0),
                    ),
                    egui::CornerRadius::ZERO,
                    egui::Color32::BLACK,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(
                        self.position + egui::Vec2::new(1.0, 12.0),
                        self.position + egui::Vec2::new(3.0, 14.0),
                    ),
                    egui::CornerRadius::ZERO,
                    egui::Color32::BLACK,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkNodeKind::OpticalMedia => {
                canvas.draw_ellipse(
                    self.position,
                    inner_rect.size() / 2.0,
                    egui::Color32::LIGHT_YELLOW,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    self.position,
                    inner_rect.size() / 4.0,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    self.position,
                    inner_rect.size() / 8.0,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
        }

        // Draw targetting rectangle
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_rectangle(
                egui::Rect::from_center_size(self.position, INNER_SIZE),
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
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
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
                    commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), coerced_delta));
                } else {
                    commands.push(
                        InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid).collect(),
                            coerced_delta,
                        ),
                    );
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
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
                        NetworkPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        NetworkPropChange::NodeKindChange(kind)  => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NodeKindChange(model.kind.clone()),
                            ));
                            model.kind = kind.clone();
                        }
                        NetworkPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::CommentChange(model.comment.clone()),
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
        self.name_buffer = (*model.name).clone();
        self.kind_buffer = model.kind.clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (NetworkElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, NetworkElementView>,
        c: &mut HashMap<ViewUuid, NetworkElementView>,
        m: &mut HashMap<ModelUuid, NetworkElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7().into(), ModelUuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(NetworkElement::Node(m)) = m.get(&old_model.uuid) {
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
            kind_buffer: self.kind_buffer.clone(),
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


fn new_network_user(
    name: &str,
    kind: NetworkUserKind,
    position: egui::Pos2,
) -> (ERef<NetworkUser>, ERef<NetworkUserView>) {
    let user_model = ERef::new(NetworkUser::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        kind,
    ));
    let user_view = new_network_user_view(user_model.clone(), position);
    (user_model, user_view)
}
fn new_network_user_view(
    model: ERef<NetworkUser>,
    position: egui::Pos2,
) -> ERef<NetworkUserView> {
    let m = model.read();
    ERef::new(NetworkUserView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        name_buffer: (*m.name).to_owned(),
        kind_buffer: m.kind.clone(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_rect: egui::Rect::ZERO,
        background_color: MGlobalColor::None,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkUserView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<NetworkUser>,

    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: NetworkUserKind,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl Entity for NetworkUserView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for NetworkUserView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<NetworkElement> for NetworkUserView {
    fn model(&self) -> NetworkElement {
        self.model.clone().into()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rect {
            inner: self.bounds_rect
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
    }
}

impl ElementControllerGen2<NetworkDomain> for NetworkUserView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) -> PropertiesStatus<NetworkDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui.labeled_text_edit_multiline("Name:", &mut self.name_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.char())
            .show_ui(ui, |ui| {
                for e in [
                    NetworkUserKind::Normal,
                    NetworkUserKind::Sysadmin,
                    NetworkUserKind::Tie,
                    NetworkUserKind::Audit,
                    NetworkUserKind::Developer,
                    NetworkUserKind::BlackHat,
                    NetworkUserKind::GrayHat,
                    NetworkUserKind::WhiteHat,
                ] {
                    if ui.selectable_value(&mut self.kind_buffer, e, e.char()).changed() {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::UserKindChange(self.kind_buffer),
                        ));
                    }
                }
            });

        if ui.labeled_text_edit_multiline("Comment:", &mut self.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
            &gdc.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        PropertiesStatus::Shown
    }
    fn draw_in(
        &mut self,
        _q: &<NetworkDomain as Domain>::QueryableT<'_>,
        gdc: &GlobalDrawingContext,
        _settings: &NetworkSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveNetworkTool)>,
    ) -> TargettingStatus {
        const INNER_SIZE: egui::Vec2 = egui::Vec2::new(40.0, 40.0);
        const OUTER_SIZE: egui::Vec2 = egui::Vec2::new(50.0, 50.0);
        // Draw shape and text
        let inner_rect = egui::Rect::from_center_size(self.position, INNER_SIZE);
        let background_color = gdc.global_colors.get(&self.background_color).unwrap_or(egui::Color32::WHITE);
        canvas.draw_rectangle(
            inner_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        canvas.draw_ellipse(
            self.position + egui::Vec2::new(0.0, -5.0),
            INNER_SIZE / 4.0,
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );
        canvas.draw_polygon(
            [
                inner_rect.left_bottom(),
                inner_rect.left_center() + egui::Vec2::new(7.0, 5.0),
                inner_rect.center() + egui::Vec2::new(-5.0, 7.0),
                inner_rect.center() + egui::Vec2::new(5.0, 7.0),
                inner_rect.right_center() + egui::Vec2::new(-7.0, 5.0),
                inner_rect.right_bottom(),
            ].to_vec(),
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );

        match self.kind_buffer {
            NetworkUserKind::Normal => {},
            NetworkUserKind::Sysadmin => {
                let screen_rect = egui::Rect::from_two_pos(
                    inner_rect.left_bottom() + egui::Vec2::new(4.0, -10.0),
                    inner_rect.center() + egui::Vec2::new(-6.0, 5.0),
                );
                canvas.draw_rectangle(
                    screen_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    screen_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::LIGHT_BLUE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        screen_rect.center_bottom() + egui::Vec2::new(-4.0, 5.0),
                        screen_rect.center_bottom() + egui::Vec2::new(-2.0, 2.0),
                        screen_rect.center_bottom() + egui::Vec2::new(2.0, 2.0),
                        screen_rect.center_bottom() + egui::Vec2::new(4.0, 5.0),
                    ].to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkUserKind::Tie => {
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(1.0, 7.0),
                        self.position + egui::Vec2::new(3.0, 16.0),
                        self.position + egui::Vec2::new(0.0, 18.0),
                        self.position + egui::Vec2::new(-3.0, 16.0),
                        self.position + egui::Vec2::new(-1.0, 7.0),
                    ].to_vec(),
                    egui::Color32::BLACK,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            NetworkUserKind::Audit => {
                let paper_rect = egui::Rect::from_two_pos(
                    inner_rect.left_bottom() + egui::Vec2::new(4.0, -4.0),
                    inner_rect.center() + egui::Vec2::new(-6.0, 3.0),
                );
                canvas.draw_rectangle(
                    paper_rect.expand(2.0),
                    egui::CornerRadius::ZERO,
                    egui::Color32::BROWN,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    paper_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        paper_rect.center_top() + egui::Vec2::new(-4.0, 2.0),
                        paper_rect.center_top() + egui::Vec2::new(-2.0, -2.0),
                        paper_rect.center_top() + egui::Vec2::new(2.0, -2.0),
                        paper_rect.center_top() + egui::Vec2::new(4.0, 2.0),
                    ].to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            NetworkUserKind::Developer => {
                const HARD_HAT_COLOR: egui::Color32 = egui::Color32::YELLOW;
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-10.0, -10.0),
                        self.position + egui::Vec2::new(-7.0, -15.0),
                        self.position + egui::Vec2::new(-2.0, -18.0),
                        self.position + egui::Vec2::new(2.0, -18.0),
                        self.position + egui::Vec2::new(7.0, -15.0),
                        self.position + egui::Vec2::new(10.0, -10.0),
                    ].to_vec(),
                    HARD_HAT_COLOR,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-10.0, -10.0),
                        self.position + egui::Vec2::new(-5.0, -9.0),
                        self.position + egui::Vec2::new(5.0, -9.0),
                        self.position + egui::Vec2::new(10.0, -10.0),
                    ].to_vec(),
                    HARD_HAT_COLOR,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
            NetworkUserKind::BlackHat
            | NetworkUserKind::GrayHat
            | NetworkUserKind::WhiteHat => {
                let (hat_main, hat_detail) = match self.kind_buffer {
                    NetworkUserKind::BlackHat => (egui::Color32::BLACK, egui::Color32::WHITE),
                    NetworkUserKind::GrayHat => (egui::Color32::LIGHT_GRAY, egui::Color32::DARK_GRAY),
                    NetworkUserKind::WhiteHat => (egui::Color32::WHITE, egui::Color32::BLACK),
                    _ => unreachable!(),
                };
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-15.0, -8.0),
                        self.position + egui::Vec2::new(-7.0, -12.0),
                        self.position + egui::Vec2::new(7.0, -12.0),
                        self.position + egui::Vec2::new(15.0, -8.0),
                    ].to_vec(),
                    hat_main,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-7.0, -12.0),
                        self.position + egui::Vec2::new(-5.0, -15.0),
                        self.position + egui::Vec2::new(-2.0, -18.0),
                        self.position + egui::Vec2::new(2.0, -18.0),
                        self.position + egui::Vec2::new(5.0, -15.0),
                        self.position + egui::Vec2::new(7.0, -12.0),
                    ].to_vec(),
                    hat_main,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-9.0, -11.0),
                        self.position + egui::Vec2::new(-7.0, -13.0),
                        self.position + egui::Vec2::new(7.0, -13.0),
                        self.position + egui::Vec2::new(9.0, -11.0),
                    ].to_vec(),
                    hat_detail,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            },
        }

        canvas.draw_text(
            self.position + egui::Vec2::new(0.0, OUTER_SIZE.y / 2.0),
            egui::Align2::CENTER_TOP,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        self.bounds_rect = egui::Rect::from_center_size(self.position, OUTER_SIZE);

        // Draw targetting rectangle
        if let Some(t) = tool
            .as_ref()
            .filter(|e| self.min_shape().contains(e.0))
            .map(|e| e.1)
        {
            canvas.draw_rectangle(
                inner_rect,
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
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
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
                        ),
                    );
                }
                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
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
                        NetworkPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        NetworkPropChange::UserKindChange(kind)  => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::UserKindChange(model.kind.clone()),
                            ));
                            model.kind = kind.clone();
                        }
                        NetworkPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
                        }
                        NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
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
        self.name_buffer = (*model.name).clone();
        self.kind_buffer = model.kind.clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (NetworkElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, NetworkElementView>,
        c: &mut HashMap<ViewUuid, NetworkElementView>,
        m: &mut HashMap<ModelUuid, NetworkElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(NetworkElement::User(m)) = m.get(&old_model.uuid) {
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
            kind_buffer: self.kind_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}


fn new_network_association(
    line_type: NetworkAssociationLineType,
    source: (NetworkElement, NetworkElementView),
    source_arrowhead: NetworkAssociationArrowheadType,
    target: (NetworkElement, NetworkElementView),
    target_arrowhead: NetworkAssociationArrowheadType,
) -> (ERef<NetworkAssociation>, ERef<LinkViewT>) {
    let predicate_model = ERef::new(NetworkAssociation::new(
        ModelUuid::now_v7(),
        line_type,
        source.0,
        source_arrowhead,
        target.0,
        target_arrowhead,
    ));
    let predicate_view = new_network_association_view(
        predicate_model.clone(),
        source.1,
        target.1
    );

    (predicate_model, predicate_view)
}
fn new_network_association_view(
    model: ERef<NetworkAssociation>,
    source: NetworkElementView,
    target: NetworkElementView,
) -> ERef<LinkViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(std::iter::once(*m.source.uuid()), *m.target.uuid(), target.min_shape(), None);

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        NetworkAssociationAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        vec![Ending::new_p(source, sp[0].clone())],
        vec![Ending::new_p(target, tp[0].clone())],
        mp,
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct NetworkAssociationAdapter {
    #[nh_context_serde(entity)]
    model: ERef<NetworkAssociation>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: NetworkAssociationTemporaries,
}

#[derive(Clone, Default)]
struct NetworkAssociationTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    line_type_buffer: NetworkAssociationLineType,
    source_arrowhead_buffer: NetworkAssociationArrowheadType,
    source_multiplicity_buffer: String,
    source_role_buffer: String,
    source_reading_buffer: String,
    target_arrowhead_buffer: NetworkAssociationArrowheadType,
    target_multiplicity_buffer: String,
    target_role_buffer: String,
    target_reading_buffer: String,
    comment_buffer: String,
}

impl MulticonnectionAdapter<NetworkDomain> for NetworkAssociationAdapter {
    fn model(&self) -> NetworkElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<NetworkDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<NetworkDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<NetworkDomain as Domain>::ToolT)>,
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

    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        self.model.write().flip_multiconnection();
        Ok(())
    }

    fn show_properties(
        &mut self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>
    ) ->PropertiesStatus<NetworkDomain> {
        ui.label("Line type:");
        egui::ComboBox::from_id_salt("line type")
            .selected_text(self.temporaries.line_type_buffer.char())
            .show_ui(ui, |ui| {
                for e in [ NetworkAssociationLineType::Solid, NetworkAssociationLineType::Dashed, ] {
                    if ui.selectable_value(&mut self.temporaries.line_type_buffer, e, e.char()).changed() {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::AssociationLineTypeChange(self.temporaries.line_type_buffer),
                        ));
                    }
                }
            });

        ui.label("Source arrowhead type:");
        egui::ComboBox::from_id_salt("source arrohead type")
            .selected_text(self.temporaries.source_arrowhead_buffer.char())
            .show_ui(ui, |ui| {
                for e in [ NetworkAssociationArrowheadType::None, NetworkAssociationArrowheadType::OpenTriangle, NetworkAssociationArrowheadType::EmptyTriangle, ] {
                    if ui.selectable_value(&mut self.temporaries.source_arrowhead_buffer, e, e.char()).changed() {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::AssociationArrowheadTypeChange(false, self.temporaries.source_arrowhead_buffer),
                        ));
                    }
                }
            });

        if ui.labeled_text_edit_singleline("Source multiplicity:", &mut self.temporaries.source_multiplicity_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationMultiplicityChange(false, Arc::new(
                    self.temporaries.source_multiplicity_buffer.clone(),
                )),
            ));
        }
        if ui.labeled_text_edit_singleline("Source role:", &mut self.temporaries.source_role_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationRoleChange(false, Arc::new(
                    self.temporaries.source_role_buffer.clone(),
                )),
            ));
        }
        if ui.labeled_text_edit_singleline("Source reading:", &mut self.temporaries.source_reading_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationReadingChange(false, Arc::new(
                    self.temporaries.source_reading_buffer.clone(),
                )),
            ));
        }

        ui.label("Target arrowhead type:");
        egui::ComboBox::from_id_salt("target arrohead type")
            .selected_text(self.temporaries.target_arrowhead_buffer.char())
            .show_ui(ui, |ui| {
                for e in [ NetworkAssociationArrowheadType::None, NetworkAssociationArrowheadType::OpenTriangle, NetworkAssociationArrowheadType::EmptyTriangle, ] {
                    if ui.selectable_value(&mut self.temporaries.target_arrowhead_buffer, e, e.char()).changed() {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::AssociationArrowheadTypeChange(true, self.temporaries.target_arrowhead_buffer),
                        ));
                    }
                }
            });

        if ui.labeled_text_edit_singleline("Target multiplicity:", &mut self.temporaries.target_multiplicity_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationMultiplicityChange(true, Arc::new(
                    self.temporaries.target_multiplicity_buffer.clone(),
                )),
            ));
        }
        if ui.labeled_text_edit_singleline("Target role:", &mut self.temporaries.target_role_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationRoleChange(true, Arc::new(
                    self.temporaries.target_role_buffer.clone(),
                )),
            ));
        }
        if ui.labeled_text_edit_singleline("Target reading:", &mut self.temporaries.target_reading_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationReadingChange(true, Arc::new(
                    self.temporaries.target_reading_buffer.clone(),
                )),
            ));
        }

        if ui.labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ));
        }

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                NetworkPropChange::AssociationLineTypeChange(line_type) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::AssociationLineTypeChange(model.line_type.clone()),
                    ));
                    model.line_type = line_type.clone();
                }
                NetworkPropChange::AssociationArrowheadTypeChange(t, arrowhead) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::AssociationArrowheadTypeChange(
                            *t,
                            if !t {
                                model.source_arrowhead.clone()
                            } else {
                                model.target_arrowhead.clone()
                            }
                        ),
                    ));
                    if !t {
                        model.source_arrowhead = arrowhead.clone();
                    } else {
                        model.target_arrowhead = arrowhead.clone();
                    }
                }
                NetworkPropChange::AssociationMultiplicityChange(t, multiplicity) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::AssociationMultiplicityChange(
                            *t,
                            if !t {
                                model.source_label_multiplicity.clone()
                            } else {
                                model.target_label_multiplicity.clone()
                            }
                        ),
                    ));
                    if !t {
                        model.source_label_multiplicity = multiplicity.clone();
                    } else {
                        model.target_label_multiplicity = multiplicity.clone();
                    }
                }
                NetworkPropChange::AssociationRoleChange(t, role) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::AssociationRoleChange(
                            *t,
                            if !t {
                                model.source_label_role.clone()
                            } else {
                                model.target_label_role.clone()
                            }
                        ),
                    ));
                    if !t {
                        model.source_label_role = role.clone();
                    } else {
                        model.target_label_role = role.clone();
                    }
                }
                NetworkPropChange::AssociationReadingChange(t, reading) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::AssociationReadingChange(
                            *t,
                            if !t {
                                model.source_label_reading.clone()
                            } else {
                                model.target_label_reading.clone()
                            }
                        ),
                    ));
                    if !t {
                        model.source_label_reading = reading.clone();
                    } else {
                        model.target_label_reading = reading.clone();
                    }
                }
                NetworkPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        NetworkPropChange::CommentChange(model.comment.clone()),
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
            line_type: NetworkAssociationLineType,
            arrowhead_type: NetworkAssociationArrowheadType,
            multiplicity: &Arc<String>,
            role: &Arc<String>,
            reading: &Arc<String>,
        ) -> ArrowData {
            let line_type = match line_type {
                NetworkAssociationLineType::Solid => canvas::LineType::Solid,
                NetworkAssociationLineType::Dashed => canvas::LineType::Dashed,
            };
            let arrowhead_type = match arrowhead_type {
                NetworkAssociationArrowheadType::None => canvas::ArrowheadType::None,
                NetworkAssociationArrowheadType::OpenTriangle => canvas::ArrowheadType::OpenTriangle,
                NetworkAssociationArrowheadType::EmptyTriangle => canvas::ArrowheadType::EmptyTriangle,
            };
            let multiplicity = if multiplicity.is_empty() { None } else { Some(multiplicity.clone()) };
            let role = if role.is_empty() { None } else { Some(role.clone()) };
            let reading = if reading.is_empty() { None } else { Some(reading.clone()) };
            ArrowData { line_type, arrowhead_type, multiplicity, role, reading }
        }

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.uuid()),
            ah(model.line_type, model.source_arrowhead, &model.source_label_multiplicity, &model.source_label_role, &model.source_label_reading),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ah(model.line_type, model.target_arrowhead, &model.target_label_multiplicity, &model.target_label_role, &model.target_label_reading),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries.source_uuids.push(*model.source.uuid());
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.line_type_buffer = model.line_type.clone();
        self.temporaries.source_arrowhead_buffer = model.source_arrowhead.clone();
        self.temporaries.source_multiplicity_buffer = (*model.source_label_multiplicity).clone();
        self.temporaries.source_role_buffer = (*model.source_label_role).clone();
        self.temporaries.source_reading_buffer = (*model.source_label_reading).clone();
        self.temporaries.target_arrowhead_buffer = model.target_arrowhead.clone();
        self.temporaries.target_multiplicity_buffer = (*model.target_label_multiplicity).clone();
        self.temporaries.target_role_buffer  = (*model.target_label_role).clone();
        self.temporaries.target_reading_buffer = (*model.target_label_reading).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, NetworkElement>
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(NetworkElement::Association(m)) = m.get(&old_model.uuid) {
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
        m: &HashMap<ModelUuid, NetworkElement>,
    ) {
        let mut model = self.model.write();

        let source_uuid = *model.source.uuid();
        if let Some(new_source) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }

        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}


pub fn new_network_comment(
    text: &str,
    position: egui::Pos2,
) -> (ERef<NetworkComment>, ERef<NetworkCommentView>) {
    let comment_model = ERef::new(NetworkComment::new(
        ModelUuid::now_v7(),
        text.to_owned(),
    ));
    let comment_view = new_network_comment_view(comment_model.clone(), position);

    (comment_model, comment_view)
}
pub fn new_network_comment_view(
    model: ERef<NetworkComment>,
    position: egui::Pos2,
) -> ERef<NetworkCommentView> {
    let m = model.read();
    ERef::new(NetworkCommentView {
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
pub struct NetworkCommentView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<NetworkComment>,

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

impl Entity for NetworkCommentView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for NetworkCommentView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<NetworkElement> for NetworkCommentView {
    fn model(&self) -> NetworkElement {
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

impl ElementControllerGen2<NetworkDomain> for NetworkCommentView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
    ) -> PropertiesStatus<NetworkDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui.labeled_text_edit_multiline("Text:", &mut self.text_buffer).changed() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
            &gdc.global_colors,
            ui,
            &mut self.background_color,
        ) {
            return PropertiesStatus::PromptRequest(RequestType::ChangeColor(0, self.background_color))
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &<NetworkDomain as Domain>::QueryableT<'_>,
        context: &GlobalDrawingContext,
        _settings: &<NetworkDomain as Domain>::SettingsT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveNetworkTool)>,
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
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
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
                        ),
                    );
                }

                EventHandlingStatus::HandledByElement
            }
            _ => EventHandlingStatus::NotHandled,
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<NetworkElementOrVertex, NetworkPropChange>>,
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
                        NetworkPropChange::NameChange(text) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NameChange(model.text.clone()),
                            ));
                            model.text = text.clone();
                        }
                        NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::ColorChange(ColorChangeData { slot: 0, color: self.background_color }),
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
        _flattened_views: &mut HashMap<ViewUuid, (NetworkElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, NetworkElementView>,
        c: &mut HashMap<ViewUuid, NetworkElementView>,
        m: &mut HashMap<ModelUuid, NetworkElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(NetworkElement::Comment(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
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
