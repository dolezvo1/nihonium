use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerModel, ControllerAdapter, DiagramAdapter,
    DiagramController, DiagramControllerGen2, DiagramSettings, DiagramSettings2, Domain,
    ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus,
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
use crate::common::views::package_view::{PackageAdapter, PackageView};
use crate::domains::network::network_models::{
    NetworkAssociation, NetworkAssociationArrowheadType, NetworkAssociationLineType,
    NetworkComment, NetworkContainer, NetworkDiagram, NetworkElement, NetworkFile, NetworkFileKind,
    NetworkNode, NetworkNodeKind, NetworkUser, NetworkUserKind,
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

pub struct NetworkDomain;
impl Domain for NetworkDomain {
    type SettingsT = NetworkSettings;
    type CommonElementT = NetworkElement;
    type DiagramModelT = NetworkDiagram;
    type CommonElementViewT = NetworkElementView;
    type ViewTargettingSectionT = NetworkElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveNetworkTool;
    type OrdinalMovementT = NetworkOrdinalMovement;
    type AddCommandElementT = NetworkElementOrVertex;
    type PropChangeT = NetworkPropChange;
}

type PackageViewT = PackageView<NetworkDomain, NetworkContainerAdapter>;
type LinkViewT = MulticonnectionView<NetworkDomain, NetworkAssociationAdapter>;

#[derive(Clone, Copy, Debug)]
pub struct NetworkOrdinalMovement {}

#[derive(Clone)]
pub enum NetworkPropChange {
    NameChange(Arc<String>),

    NodeKindChange(NetworkNodeKind),
    UserKindChange(NetworkUserKind),
    FileKindChange(NetworkFileKind),

    AssociationLineTypeChange(NetworkAssociationLineType),
    AssociationArrowheadTypeChange(/*target?*/ bool, NetworkAssociationArrowheadType),
    AssociationMultiplicityChange(/*target?*/ bool, Arc<String>),
    AssociationRoleChange(/*target?*/ bool, Arc<String>),
    AssociationReadingChange(/*target?*/ bool, Arc<String>),
    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    CommentAlignChange(Option<egui::Align>, Option<egui::Align>),
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
                Self::FileKindChange(_kind) => format!("FileKindChange(..)"),

                Self::AssociationLineTypeChange(_) => format!("AssociationLineTypeChange(..)"),
                Self::AssociationArrowheadTypeChange(..) =>
                    format!("AssociationArrowheadTypeChange(..)"),
                Self::AssociationMultiplicityChange(..) =>
                    format!("AssociationMultiplicityChange(..)"),
                Self::AssociationRoleChange(..) => format!("AssociationRoleChange(..)"),
                Self::AssociationReadingChange(..) => format!("AssociationReadingChange(..)"),
                Self::FlipMulticonnection(_) => format!("FlipMulticonnection"),

                Self::ColorChange(_color) => format!("ColorChange(..)"),
                Self::CommentChange(comment) => format!("CommentChange({})", comment),
                Self::CommentAlignChange(..) => format!("CommentAlignChange(..)"),
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
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            (
                Self::AssociationMultiplicityChange(b1, _),
                newer @ Self::AssociationMultiplicityChange(b2, _),
            )
            | (Self::AssociationRoleChange(b1, _), newer @ Self::AssociationRoleChange(b2, _))
            | (
                Self::AssociationReadingChange(b1, _),
                newer @ Self::AssociationReadingChange(b2, _),
            ) if b1 == b2 => Some(newer.clone()),
            _ => None,
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
    File(ERef<NetworkFileView>),
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

    fn insert_element(
        &mut self,
        parent: ModelUuid,
        element: NetworkElement,
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
        undo: &mut Vec<(ModelUuid, NetworkElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
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

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
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

    fn get_element_pos_in(
        &self,
        parent: &ModelUuid,
        model_uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)> {
        self.model.read().get_element_pos_in(parent, model_uuid)
    }

    fn create_new_view_for(
        &self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        element: NetworkElement,
    ) -> Result<NetworkElementView, HashSet<ModelUuid>> {
        let v = match element {
            NetworkElement::Container(inner) => new_network_container_view(
                inner,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 100.0),
                },
            )
            .into(),
            NetworkElement::Node(inner) => new_network_node_view(inner, egui::Pos2::ZERO).into(),
            NetworkElement::User(inner) => {
                new_network_user_view(inner, egui::Pos2::ZERO, MGlobalColor::None).into()
            }
            NetworkElement::File(inner) => {
                new_network_file_view(inner, egui::Pos2::ZERO, MGlobalColor::None).into()
            }
            NetworkElement::Association(inner) => {
                let m = inner.read();
                let (sid, tid) = (*m.source.uuid(), *m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([sid, tid])),
                };
                new_network_association_view(inner.clone(), source_view, target_view).into()
            }
            NetworkElement::Comment(inner) => {
                new_network_comment_view(inner, egui::Pos2::ZERO, egui::Align2::CENTER_CENTER)
                    .into()
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &NetworkElement) -> Arc<String> {
        match e {
            NetworkElement::Container(inner) => format!(
                "Container ({})",
                LabelProvider::filter_and_elipsis(&inner.read().name)
            )
            .into(),
            NetworkElement::Node(inner) => format!(
                "Node ({})",
                LabelProvider::filter_and_elipsis(&inner.read().name)
            )
            .into(),
            NetworkElement::User(inner) => format!(
                "User ({})",
                LabelProvider::filter_and_elipsis(&inner.read().name)
            )
            .into(),
            NetworkElement::File(inner) => format!(
                "File ({})",
                LabelProvider::filter_and_elipsis(&inner.read().name)
            )
            .into(),
            NetworkElement::Association(_inner) => "Association".to_owned().into(),
            NetworkElement::Comment(inner) => {
                let r = inner.read();
                let s = if r.text.is_empty() {
                    "Comment".to_owned()
                } else {
                    format!("Comment ({})", LabelProvider::filter_and_elipsis(&r.text))
                };
                Arc::new(s)
            }
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
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
                NetworkPropChange::ColorChange((0, new_color).into()),
            ));
        }
    }
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        _drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                NetworkPropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        };

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                NetworkPropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
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
                        NetworkPropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
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
    ) {
    }
    fn try_handle_custom_shortcut(
        &mut self,
        settings: &NetworkSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<NetworkDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveNetworkTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialNetworkElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

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

    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, NetworkElement>) {
        let models = super::network_models::enumerate_diagram(&self.model.read());
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
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            NetworkControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                NetworkDiagramAdapter::new(model),
                elements,
            )],
        )),
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
    let (internet, internet_view) = new_network_node(
        "Cloud",
        NetworkNodeKind::Cloud,
        egui::Pos2::new(200.0, 200.0),
    );
    let (router, router_view) = new_network_node(
        "Router",
        NetworkNodeKind::Router,
        egui::Pos2::new(300.0, 400.0),
    );
    let (swtch, swtch_view) = new_network_node(
        "Switch",
        NetworkNodeKind::Switch,
        egui::Pos2::new(400.0, 200.0),
    );
    let (workstation, workstation_view) = new_network_node(
        "Workstation",
        NetworkNodeKind::Workstation,
        egui::Pos2::new(500.0, 400.0),
    );
    let (user, user_view) = new_network_user(
        "User",
        NetworkUserKind::Normal,
        egui::Pos2::new(600.0, 200.0),
        MGlobalColor::None,
    );
    let (file, file_view) = new_network_file(
        "File",
        NetworkFileKind::Certificate,
        egui::Pos2::new(700.0, 400.0),
        MGlobalColor::None,
    );

    let (e1, e1_view) = new_network_association(
        NetworkAssociationLineType::Solid,
        (internet.clone().into(), internet_view.clone().into()),
        NetworkAssociationArrowheadType::None,
        (router.clone().into(), router_view.clone().into()),
        NetworkAssociationArrowheadType::OpenTriangle,
    );
    let (e2, e2_view) = new_network_association(
        NetworkAssociationLineType::Solid,
        (router.clone().into(), router_view.clone().into()),
        NetworkAssociationArrowheadType::None,
        (swtch.clone().into(), swtch_view.clone().into()),
        NetworkAssociationArrowheadType::OpenTriangle,
    );
    let (e3, e3_view) = new_network_association(
        NetworkAssociationLineType::Solid,
        (swtch.clone().into(), swtch_view.clone().into()),
        NetworkAssociationArrowheadType::None,
        (workstation.clone().into(), workstation_view.clone().into()),
        NetworkAssociationArrowheadType::OpenTriangle,
    );
    let (e4, e4_view) = new_network_association(
        NetworkAssociationLineType::Solid,
        (workstation.clone().into(), workstation_view.clone().into()),
        NetworkAssociationArrowheadType::None,
        (file.clone().into(), file_view.clone().into()),
        NetworkAssociationArrowheadType::OpenTriangle,
    );
    let (e5, e5_view) = new_network_association(
        NetworkAssociationLineType::Dashed,
        (user.clone().into(), user_view.clone().into()),
        NetworkAssociationArrowheadType::None,
        (file.clone().into(), file_view.clone().into()),
        NetworkAssociationArrowheadType::OpenTriangle,
    );

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
            file.into(),
            e1.into(),
            e2.into(),
            e3.into(),
            e4.into(),
            e5.into(),
        ],
    ));
    new_controlller(
        diagram,
        name,
        vec![
            internet_view.into(),
            router_view.into(),
            swtch_view.into(),
            workstation_view.into(),
            user_view.into(),
            file_view.into(),
            e1_view.into(),
            e2_view.into(),
            e3_view.into(),
            e4_view.into(),
            e5_view.into(),
        ],
    )
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        NetworkDomain,
        NetworkControllerAdapter,
        DiagramControllerGen2<NetworkDomain, NetworkDiagramAdapter>,
    >>(&uuid)?)
}

pub struct NetworkSettings {
    palette: RwLock<ToolPalette<NetworkToolStage, NetworkDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<NetworkToolStage, NetworkElementView>>,
}
impl DiagramSettings for NetworkSettings {
    fn serialize(&self) -> Result<toml::Value, ()> {
        let mut table = toml::Table::new();
        table.insert(
            "palette".to_owned(),
            self.palette.read().unwrap().serialize()?.into(),
        );
        Ok(table.into())
    }
}
impl DiagramSettings2<NetworkDomain> for NetworkSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    NetworkToolStage,
                    String,
                    NetworkElementView,
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
            "Nodes",
            vec![
                (
                    NetworkToolStage::Node {
                        name: "Workstation".to_owned(),
                        kind: NetworkNodeKind::Workstation,
                    },
                    "Workstation",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num1,
                    )),
                ),
                (
                    NetworkToolStage::Node {
                        name: "Laptop".to_owned(),
                        kind: NetworkNodeKind::Laptop,
                    },
                    "Laptop",
                    None,
                ),
                (
                    NetworkToolStage::Node {
                        name: "Router".to_owned(),
                        kind: NetworkNodeKind::Router,
                    },
                    "Router",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num2,
                    )),
                ),
                (
                    NetworkToolStage::Node {
                        name: "Switch".to_owned(),
                        kind: NetworkNodeKind::Switch,
                    },
                    "Switch",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num3,
                    )),
                ),
            ],
        ),
        (
            "Users",
            vec![
                (
                    NetworkToolStage::User {
                        name: "User".to_owned(),
                        kind: NetworkUserKind::Normal,
                        background_color: MGlobalColor::None,
                    },
                    "User",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num4,
                    )),
                ),
                (
                    NetworkToolStage::User {
                        name: "Developer".to_owned(),
                        kind: NetworkUserKind::Developer,
                        background_color: MGlobalColor::None,
                    },
                    "Developer",
                    None,
                ),
                (
                    NetworkToolStage::User {
                        name: "Audit".to_owned(),
                        kind: NetworkUserKind::Audit,
                        background_color: MGlobalColor::None,
                    },
                    "Audit",
                    None,
                ),
                (
                    NetworkToolStage::User {
                        name: "Black Hat".to_owned(),
                        kind: NetworkUserKind::BlackHat,
                        background_color: MGlobalColor::None,
                    },
                    "Black Hat",
                    None,
                ),
            ],
        ),
        (
            "Files",
            vec![(
                NetworkToolStage::File {
                    name: "File".to_owned(),
                    kind: NetworkFileKind::Unspecified,
                    background_color: MGlobalColor::None,
                },
                "File",
                Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Num5,
                )),
            )],
        ),
        (
            "Relationships",
            vec![
                (
                    NetworkToolStage::AssociationStart {
                        line_type: NetworkAssociationLineType::Solid,
                        source_arrowhead: NetworkAssociationArrowheadType::None,
                        target_arrowhead: NetworkAssociationArrowheadType::None,
                    },
                    "Association (solid)",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num6,
                    )),
                ),
                (
                    NetworkToolStage::AssociationStart {
                        line_type: NetworkAssociationLineType::Solid,
                        source_arrowhead: NetworkAssociationArrowheadType::None,
                        target_arrowhead: NetworkAssociationArrowheadType::OpenTriangle,
                    },
                    "Association (solid, arrow)",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num7,
                    )),
                ),
                (
                    NetworkToolStage::AssociationStart {
                        line_type: NetworkAssociationLineType::Dashed,
                        source_arrowhead: NetworkAssociationArrowheadType::None,
                        target_arrowhead: NetworkAssociationArrowheadType::None,
                    },
                    "Association (dashed)",
                    None,
                ),
            ],
        ),
        (
            "Other",
            vec![
                (
                    NetworkToolStage::ContainerStart {
                        name: "Subnet".to_owned(),
                    },
                    "Container",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num8,
                    )),
                ),
                (
                    NetworkToolStage::Comment {
                        text: "a comment".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                    },
                    "Comment",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num9,
                    )),
                ),
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

    Box::new(NetworkSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
    })
}

fn view_for_stage(s: &NetworkToolStage) -> NetworkElementView {
    match s {
        NetworkToolStage::Node { name, kind } => {
            let node_view = new_network_node(name, *kind, egui::Pos2::ZERO).1;
            node_view.into()
        }
        NetworkToolStage::User {
            name,
            kind,
            background_color,
        } => {
            let user_view = new_network_user(name, *kind, egui::Pos2::ZERO, *background_color).1;
            user_view.into()
        }
        NetworkToolStage::File {
            name,
            kind,
            background_color,
        } => {
            let file_view = new_network_file(name, *kind, egui::Pos2::ZERO, *background_color).1;
            file_view.into()
        }
        NetworkToolStage::AssociationStart {
            line_type,
            source_arrowhead,
            target_arrowhead,
        } => {
            let d1 = new_network_user(
                "dummy",
                NetworkUserKind::Normal,
                egui::Pos2::ZERO,
                MGlobalColor::None,
            );
            let d2 = new_network_node(
                "dummy",
                NetworkNodeKind::Workstation,
                egui::Pos2::new(100.0, 75.0),
            );

            let association_view = new_network_association(
                *line_type,
                (d1.0.into(), d1.1.into()),
                *source_arrowhead,
                (d2.0.into(), d2.1.into()),
                *target_arrowhead,
            )
            .1;
            association_view.into()
        }
        NetworkToolStage::ContainerStart { name } => {
            let container_view = new_network_container(
                name,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 50.0),
                },
            )
            .1;
            container_view.into()
        }
        NetworkToolStage::Comment { text, align } => {
            let comment_view = new_network_comment(text, egui::Pos2::ZERO, *align).1;
            comment_view.into()
        }
        NetworkToolStage::AssociationEnd | NetworkToolStage::ContainerEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(NetworkSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
    }))
}

pub fn settings_function(
    gdc: &mut GlobalDrawingContext,
    ui: &mut egui::Ui,
    s: &mut Box<dyn DiagramSettings>,
) {
    let Some(s) = (s.as_mut() as &mut dyn Any).downcast_mut::<NetworkSettings>() else {
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
                    NetworkToolStage::Node { name, kind } => {
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();

                        columns[1].label("Kind");
                        egui::ComboBox::from_id_salt("kind")
                            .selected_text(kind.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in NetworkNodeKind::VARIANTS {
                                    modified |= ui.selectable_value(kind, e, e.as_str()).clicked();
                                }
                            });
                    },
                    NetworkToolStage::User { name, kind, background_color } => {
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();

                        columns[1].label("Kind");
                        egui::ComboBox::from_id_salt("kind")
                            .selected_text(kind.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in NetworkUserKind::VARIANTS {
                                    modified |= ui.selectable_value(kind, e, e.as_str()).clicked();
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
                    },
                    NetworkToolStage::File { name, kind, background_color } => {
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();

                        columns[1].label("Kind");
                        egui::ComboBox::from_id_salt("kind")
                            .selected_text(kind.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in NetworkFileKind::VARIANTS {
                                    modified |= ui.selectable_value(kind, e, e.as_str()).clicked();
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
                    },
                    NetworkToolStage::AssociationStart { line_type, source_arrowhead, target_arrowhead } => {
                        columns[1].label("Line type");
                        egui::ComboBox::from_id_salt("line type")
                            .selected_text(line_type.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in NetworkAssociationLineType::VARIANTS {
                                    modified |= ui.selectable_value(line_type, e, e.as_str()).clicked();
                                }
                            });

                        columns[1].label("Source arrowhead");
                        egui::ComboBox::from_id_salt("source arrowhead")
                            .selected_text(source_arrowhead.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in NetworkAssociationArrowheadType::VARIANTS {
                                    modified |= ui.selectable_value(source_arrowhead, e, e.as_str()).clicked();
                                }
                            });

                        columns[1].label("Target arrowhead");
                        egui::ComboBox::from_id_salt("target arrowhead")
                            .selected_text(target_arrowhead.as_str())
                            .show_ui(&mut columns[1], |ui| {
                                for e in NetworkAssociationArrowheadType::VARIANTS {
                                    modified |= ui.selectable_value(target_arrowhead, e, e.as_str()).clicked();
                                }
                            });
                    },
                    NetworkToolStage::ContainerStart { name } => {
                        modified |= columns[1].labeled_text_edit_singleline("Name", name).changed();
                    },
                    NetworkToolStage::Comment { text, align } => {
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
                    },
                    NetworkToolStage::AssociationEnd
                    | NetworkToolStage::ContainerEnd => unreachable!(),
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
    type_indentifier: "network",
    pretty_name: "Network diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    show_settings_function: &(settings_function as ShowSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "",
        description: "Network diagram (network nodes, users, etc.)",
        constructors: &[
            ("empty", &(new as DiagramConstructorF)),
            ("demo", &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkToolStage {
    Node {
        name: String,
        kind: NetworkNodeKind,
    },
    User {
        name: String,
        kind: NetworkUserKind,
        background_color: MGlobalColor,
    },
    File {
        name: String,
        kind: NetworkFileKind,
        background_color: MGlobalColor,
    },
    AssociationStart {
        line_type: NetworkAssociationLineType,
        source_arrowhead: NetworkAssociationArrowheadType,
        target_arrowhead: NetworkAssociationArrowheadType,
    },
    AssociationEnd,
    ContainerStart {
        name: String,
    },
    ContainerEnd,
    Comment {
        text: String,
        align: egui::Align2,
    },
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
        name: String,
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveNetworkTool {
    uuid: uuid::Uuid,
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

    fn new(uuid: uuid::Uuid, initial_stage: NetworkToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialNetworkElement::None,
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

    fn targetting_for_section(&self, element: Option<NetworkElement>) -> egui::Color32 {
        match element {
            None | Some(NetworkElement::Container(_)) => match self.current_stage {
                NetworkToolStage::Node { .. }
                | NetworkToolStage::User { .. }
                | NetworkToolStage::File { .. }
                | NetworkToolStage::ContainerStart { .. }
                | NetworkToolStage::ContainerEnd
                | NetworkToolStage::Comment { .. } => TARGETTABLE_COLOR,
                NetworkToolStage::AssociationStart { .. } | NetworkToolStage::AssociationEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(
                NetworkElement::Node(_)
                | NetworkElement::User(_)
                | NetworkElement::File(_)
                | NetworkElement::Comment(_),
            ) => match self.current_stage {
                NetworkToolStage::AssociationStart { .. } | NetworkToolStage::AssociationEnd => {
                    TARGETTABLE_COLOR
                }
                NetworkToolStage::Node { .. }
                | NetworkToolStage::User { .. }
                | NetworkToolStage::File { .. }
                | NetworkToolStage::ContainerStart { .. }
                | NetworkToolStage::ContainerEnd
                | NetworkToolStage::Comment { .. } => NON_TARGETTABLE_COLOR,
            },
            Some(NetworkElement::Association(_)) => todo!(),
        }
    }
    fn draw_status_hint(
        &self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
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

        match (&self.current_stage, &mut self.result) {
            (NetworkToolStage::Node { name, kind }, _) => {
                let (_, node_view) = new_network_node(name, *kind, pos);
                self.result = PartialNetworkElement::Some(node_view.into());
                self.event_lock = true;
            }
            (
                NetworkToolStage::User {
                    name,
                    kind,
                    background_color,
                },
                _,
            ) => {
                let (_, user_view) = new_network_user(name, *kind, pos, *background_color);
                self.result = PartialNetworkElement::Some(user_view.into());
                self.event_lock = true;
            }
            (
                NetworkToolStage::File {
                    name,
                    kind,
                    background_color,
                },
                _,
            ) => {
                let (_, file_view) = new_network_file(name, *kind, pos, *background_color);
                self.result = PartialNetworkElement::Some(file_view.into());
                self.event_lock = true;
            }
            (NetworkToolStage::ContainerStart { name }, _) => {
                self.result = PartialNetworkElement::Container {
                    name: name.clone(),
                    a: pos,
                    b: None,
                };
                self.current_stage = NetworkToolStage::ContainerEnd;
                self.event_lock = true;
            }
            (NetworkToolStage::ContainerEnd, PartialNetworkElement::Container { b, .. }) => {
                *b = Some(pos)
            }
            (NetworkToolStage::Comment { text, align }, _) => {
                let (_, comment_view) = new_network_comment(text, pos, *align);

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
            NetworkElement::Node(_)
            | NetworkElement::User(_)
            | NetworkElement::File(_)
            | NetworkElement::Comment(_) => match (&self.current_stage, &mut self.result) {
                (
                    NetworkToolStage::AssociationStart {
                        line_type,
                        source_arrowhead,
                        target_arrowhead,
                    },
                    PartialNetworkElement::None,
                ) => {
                    self.result = PartialNetworkElement::Association {
                        line_type: *line_type,
                        source_arrowhead: *source_arrowhead,
                        target_arrowhead: *target_arrowhead,
                        source: section,
                        dest: None,
                    };
                    self.current_stage = NetworkToolStage::AssociationEnd;
                    self.event_lock = true;
                }
                (
                    NetworkToolStage::AssociationEnd,
                    PartialNetworkElement::Association { dest, .. },
                ) => {
                    *dest = Some(section);
                    self.event_lock = true;
                }
                _ => {}
            },
            NetworkElement::Association(..) => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <NetworkDomain as Domain>::OrdinalMovementT,
                <NetworkDomain as Domain>::AddCommandElementT,
                <NetworkDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &self.result {
            PartialNetworkElement::Some(element) => {
                let element = element.clone();
                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: NetworkElementView::from(element).into(),
                    into_model: true,
                });
                Ok(None)
            }
            PartialNetworkElement::Association {
                line_type,
                source_arrowhead,
                target_arrowhead,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.uuid(), *dest.uuid());
                if let (Some(source_controller), Some(dest_controller)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_controller.uuid(), preferred_container)
                    && q.is_contained(&dest_controller.uuid(), preferred_container)
                {
                    self.current_stage = self.initial_stage.clone();

                    let association_view = new_network_association(
                        *line_type,
                        (source.clone(), source_controller),
                        *source_arrowhead,
                        (dest.clone(), dest_controller),
                        *target_arrowhead,
                    )
                    .1;

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: NetworkElementView::from(association_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialNetworkElement::Container {
                name,
                a,
                b: Some(b),
            } => {
                self.current_stage = self.initial_stage.clone();

                let container_view =
                    new_network_container(name, egui::Rect::from_two_pos(*a, *b)).1;

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: NetworkElementView::from(container_view).into(),
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

fn new_network_container(
    name: &str,
    bounds_rect: egui::Rect,
) -> (ERef<NetworkContainer>, ERef<PackageViewT>) {
    let container_model = ERef::new(NetworkContainer::new(
        ModelUuid::now_v7(),
        name.to_owned(),
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

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
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
    fn insert_element(
        &mut self,
        position: Option<PositionNoT>,
        element: NetworkElement,
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

    fn show_model_properties(
        &mut self,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
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
    ) -> Option<ColorChangeData> {
        ui.label("Background color:");
        crate::common::controller::mglobalcolor_edit_button(context, ui, &self.background_color)
            .map(|e| (0, e).into())
    }
    fn apply_change(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
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
                        NetworkPropChange::ColorChange(ColorChangeData {
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
        self.name_buffer = (*model.name).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> Self
    where
        Self: Sized,
    {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, NetworkElement>) {
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
    let node_model = ERef::new(NetworkNode::new(ModelUuid::now_v7(), name.to_owned(), kind));
    let node_view = new_network_node_view(node_model.clone(), position);
    (node_model, node_view)
}
fn new_network_node_view(model: ERef<NetworkNode>, position: egui::Pos2) -> ERef<NetworkNodeView> {
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
        bounds_rect: egui::Rect::from_pos(position),
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
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) -> PropertiesStatus<NetworkDomain> {
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
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in NetworkNodeKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::NodeKindChange(self.kind_buffer),
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
                NetworkPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                    canvas::Highlight::NONE,
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
            }
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
                        canvas::Highlight::NONE,
                    );
                    let modifier = if e % 2 == 0 {
                        egui::Vec2::new(-5.0, 0.0)
                    } else {
                        egui::Vec2::new(5.0, 0.0)
                    };
                    canvas.draw_line(
                        [r.center_top() + modifier, r.center_bottom() + modifier],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
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
            }
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
            }
            NetworkNodeKind::Server => {
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-18.0, -18.0),
                        self.position + egui::Vec2::new(10.0, -18.0),
                        self.position + egui::Vec2::new(18.0, -8.0),
                        self.position + egui::Vec2::new(-10.0, -8.0),
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
                    egui::Color32::GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            NetworkNodeKind::Workstation => {
                let screen_rect =
                    egui::Rect::from_center_size(self.position, egui::Vec2::new(32.0, 18.0));
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
                    ]
                    .to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
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
                    ]
                    .to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            NetworkNodeKind::Tablet => {
                let screen_rect =
                    egui::Rect::from_center_size(self.position, egui::Vec2::new(32.0, 18.0));
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
            }
            NetworkNodeKind::CellularPhone => {
                let screen_rect =
                    egui::Rect::from_center_size(self.position, egui::Vec2::new(18.0, 32.0));
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
            }
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
            }
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
            }
        }

        // Draw targetting rectangle
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
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
        _settings: &<NetworkDomain as Domain>::SettingsT,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
                        NetworkPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        NetworkPropChange::NodeKindChange(kind) => {
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
            InsensitiveCommand::Macro(..) => unreachable!(),
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
    background_color: MGlobalColor,
) -> (ERef<NetworkUser>, ERef<NetworkUserView>) {
    let user_model = ERef::new(NetworkUser::new(ModelUuid::now_v7(), name.to_owned(), kind));
    let user_view = new_network_user_view(user_model.clone(), position, background_color);
    (user_model, user_view)
}
fn new_network_user_view(
    model: ERef<NetworkUser>,
    position: egui::Pos2,
    background_color: MGlobalColor,
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
        bounds_rect: egui::Rect::from_pos(position),
        background_color,
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
            inner: self.bounds_rect,
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
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) -> PropertiesStatus<NetworkDomain> {
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
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in NetworkUserKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::UserKindChange(self.kind_buffer),
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
                NetworkPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                NetworkPropChange::ColorChange((0, new_color).into()),
            ));
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
        let background_color = gdc
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
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
            ]
            .to_vec(),
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );

        match self.kind_buffer {
            NetworkUserKind::Normal => {}
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
                    ]
                    .to_vec(),
                    egui::Color32::LIGHT_GRAY,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            NetworkUserKind::Tie => {
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(1.0, 7.0),
                        self.position + egui::Vec2::new(3.0, 16.0),
                        self.position + egui::Vec2::new(0.0, 18.0),
                        self.position + egui::Vec2::new(-3.0, 16.0),
                        self.position + egui::Vec2::new(-1.0, 7.0),
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
                    HARD_HAT_COLOR,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            NetworkUserKind::BlackHat | NetworkUserKind::GrayHat | NetworkUserKind::WhiteHat => {
                let (hat_main, hat_detail) = match self.kind_buffer {
                    NetworkUserKind::BlackHat => (egui::Color32::BLACK, egui::Color32::WHITE),
                    NetworkUserKind::GrayHat => {
                        (egui::Color32::LIGHT_GRAY, egui::Color32::DARK_GRAY)
                    }
                    NetworkUserKind::WhiteHat => (egui::Color32::WHITE, egui::Color32::BLACK),
                    _ => unreachable!(),
                };
                canvas.draw_polygon(
                    [
                        self.position + egui::Vec2::new(-15.0, -8.0),
                        self.position + egui::Vec2::new(-7.0, -12.0),
                        self.position + egui::Vec2::new(7.0, -12.0),
                        self.position + egui::Vec2::new(15.0, -8.0),
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
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
                    ]
                    .to_vec(),
                    hat_detail,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
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
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
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
        _settings: &<NetworkDomain as Domain>::SettingsT,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
                        NetworkPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        NetworkPropChange::UserKindChange(kind) => {
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
                                NetworkPropChange::ColorChange(ColorChangeData {
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

fn new_network_file(
    name: &str,
    kind: NetworkFileKind,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> (ERef<NetworkFile>, ERef<NetworkFileView>) {
    let user_model = ERef::new(NetworkFile::new(ModelUuid::now_v7(), name.to_owned(), kind));
    let user_view = new_network_file_view(user_model.clone(), position, background_color);
    (user_model, user_view)
}
fn new_network_file_view(
    model: ERef<NetworkFile>,
    position: egui::Pos2,
    background_color: MGlobalColor,
) -> ERef<NetworkFileView> {
    let m = model.read();
    ERef::new(NetworkFileView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        name_buffer: (*m.name).to_owned(),
        kind_buffer: m.kind.clone(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position: position,
        bounds_rect: egui::Rect::ZERO,
        background_color,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct NetworkFileView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<NetworkFile>,

    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: NetworkFileKind,
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

impl Entity for NetworkFileView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for NetworkFileView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<NetworkElement> for NetworkFileView {
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

impl ElementControllerGen2<NetworkDomain> for NetworkFileView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) -> PropertiesStatus<NetworkDomain> {
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
                NetworkPropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in NetworkFileKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::FileKindChange(self.kind_buffer),
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
                NetworkPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                NetworkPropChange::ColorChange((0, new_color).into()),
            ));
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
        let background_color = gdc
            .global_colors
            .get(&self.background_color)
            .unwrap_or(egui::Color32::WHITE);
        canvas.draw_rectangle(
            inner_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );
        const FILE_SIZE: egui::Vec2 = egui::Vec2::new(21.0, 29.0);
        let file_rect = egui::Rect::from_center_size(self.position, FILE_SIZE);
        const CORNER_BEVEL: f32 = 5.0;
        canvas.draw_polygon(
            [
                file_rect.left_top(),
                file_rect.right_top() - egui::Vec2::new(CORNER_BEVEL, 0.0),
                file_rect.right_top() + egui::Vec2::new(0.0, CORNER_BEVEL),
                file_rect.right_bottom(),
                file_rect.left_bottom(),
            ]
            .to_vec(),
            background_color,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );

        match self.kind_buffer {
            NetworkFileKind::Unspecified => {}
            NetworkFileKind::Document => {
                const MARGIN: f32 = 3.0;
                for k in 1..6 {
                    canvas.draw_line(
                        [
                            egui::Pos2::new(
                                file_rect.left() + MARGIN,
                                file_rect.top() + k as f32 * 5.0,
                            ),
                            egui::Pos2::new(
                                file_rect.right() - MARGIN,
                                file_rect.top() + k as f32 * 5.0,
                            ),
                        ],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            NetworkFileKind::Binary => {
                for (e, a) in [
                    ("01", egui::Align2::CENTER_BOTTOM),
                    ("10", egui::Align2::CENTER_TOP),
                ] {
                    canvas.draw_text(
                        self.position,
                        a,
                        e,
                        canvas::CLASS_TOP_FONT_SIZE,
                        egui::Color32::BLACK,
                    );
                }
            }
            k => {
                let c = match k {
                    NetworkFileKind::Unspecified
                    | NetworkFileKind::Document
                    | NetworkFileKind::Binary => unreachable!(),
                    NetworkFileKind::SourceCode => "</>",
                    NetworkFileKind::Certificate => "🔑",
                    NetworkFileKind::Audio => "🔊",
                    NetworkFileKind::Image => "🖼",
                    NetworkFileKind::Video => "📽",
                    NetworkFileKind::Archive => "📦",
                };
                canvas.draw_text(
                    self.position,
                    egui::Align2::CENTER_CENTER,
                    c,
                    canvas::CLASS_TOP_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
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
        if canvas.ui_scale().is_some()
            && let Some(t) = tool
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
        _settings: &<NetworkDomain as Domain>::SettingsT,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
                        NetworkPropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        NetworkPropChange::FileKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::FileKindChange(model.kind.clone()),
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
                                NetworkPropChange::ColorChange(ColorChangeData {
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

        let modelish = if let Some(NetworkElement::File(m)) = m.get(&old_model.uuid) {
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
    let predicate_view = new_network_association_view(predicate_model.clone(), source.1, target.1);

    (predicate_model, predicate_view)
}
fn new_network_association_view(
    model: ERef<NetworkAssociation>,
    source: NetworkElementView,
    target: NetworkElementView,
) -> ERef<LinkViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once(*m.source.uuid()),
        *m.target.uuid(),
        target.min_shape(),
        None,
    );

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

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
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
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) -> PropertiesStatus<NetworkDomain> {
        ui.label("Line type:");
        egui::ComboBox::from_id_salt("line type")
            .selected_text(self.temporaries.line_type_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in NetworkAssociationLineType::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.line_type_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::AssociationLineTypeChange(
                                self.temporaries.line_type_buffer,
                            ),
                        ));
                    }
                }
            });

        ui.label("Source arrowhead type:");
        egui::ComboBox::from_id_salt("source arrohead type")
            .selected_text(self.temporaries.source_arrowhead_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in NetworkAssociationArrowheadType::VARIANTS {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.source_arrowhead_buffer,
                            e,
                            e.as_str(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::AssociationArrowheadTypeChange(
                                false,
                                self.temporaries.source_arrowhead_buffer,
                            ),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline(
                "Source multiplicity:",
                &mut self.temporaries.source_multiplicity_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationMultiplicityChange(
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
                NetworkPropChange::AssociationRoleChange(
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
                NetworkPropChange::AssociationReadingChange(
                    false,
                    Arc::new(self.temporaries.source_reading_buffer.clone()),
                ),
            ));
        }

        ui.label("Target arrowhead type:");
        egui::ComboBox::from_id_salt("target arrohead type")
            .selected_text(self.temporaries.target_arrowhead_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in NetworkAssociationArrowheadType::VARIANTS {
                    if ui
                        .selectable_value(
                            &mut self.temporaries.target_arrowhead_buffer,
                            e,
                            e.as_str(),
                        )
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            NetworkPropChange::AssociationArrowheadTypeChange(
                                true,
                                self.temporaries.target_arrowhead_buffer,
                            ),
                        ));
                    }
                }
            });

        if ui
            .labeled_text_edit_singleline(
                "Target multiplicity:",
                &mut self.temporaries.target_multiplicity_buffer,
            )
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                NetworkPropChange::AssociationMultiplicityChange(
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
                NetworkPropChange::AssociationRoleChange(
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
                NetworkPropChange::AssociationReadingChange(
                    true,
                    Arc::new(self.temporaries.target_reading_buffer.clone()),
                ),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
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
        command: &InsensitiveCommand<
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
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
                            },
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
                            },
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
                            },
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
                            },
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
                NetworkAssociationArrowheadType::OpenTriangle => {
                    canvas::ArrowheadType::OpenTriangle
                }
                NetworkAssociationArrowheadType::EmptyTriangle => {
                    canvas::ArrowheadType::EmptyTriangle
                }
            };
            let multiplicity = if multiplicity.is_empty() {
                None
            } else {
                Some(multiplicity.clone())
            };
            let role = if role.is_empty() {
                None
            } else {
                Some(role.clone())
            };
            let reading = if reading.is_empty() {
                None
            } else {
                Some(reading.clone())
            };
            ArrowData {
                line_type,
                arrowhead_type,
                multiplicity,
                role,
                reading,
            }
        }

        self.temporaries.arrow_data.clear();
        self.temporaries.arrow_data.insert(
            (false, *model.source.uuid()),
            ah(
                model.line_type,
                model.source_arrowhead,
                &model.source_label_multiplicity,
                &model.source_label_role,
                &model.source_label_reading,
            ),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ah(
                model.line_type,
                model.target_arrowhead,
                &model.target_label_multiplicity,
                &model.target_label_role,
                &model.target_label_reading,
            ),
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
        self.temporaries.target_role_buffer = (*model.target_label_role).clone();
        self.temporaries.target_reading_buffer = (*model.target_label_reading).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, NetworkElement>,
    ) -> Self
    where
        Self: Sized,
    {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, NetworkElement>) {
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
    align: egui::Align2,
) -> (ERef<NetworkComment>, ERef<NetworkCommentView>) {
    let comment_model = ERef::new(NetworkComment::new(ModelUuid::now_v7(), text.to_owned()));
    let comment_view = new_network_comment_view(comment_model.clone(), position, align);

    (comment_model, comment_view)
}
pub fn new_network_comment_view(
    model: ERef<NetworkComment>,
    position: egui::Pos2,
    align: egui::Align2,
) -> ERef<NetworkCommentView> {
    let m = model.read();
    ERef::new(NetworkCommentView {
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
    align: egui::Align2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
}

impl NetworkCommentView {
    const CORNER_SIZE: f32 = 10.0;
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
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
        >,
    ) -> PropertiesStatus<NetworkDomain> {
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
                NetworkPropChange::NameChange(Arc::new(self.text_buffer.clone())),
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
                            NetworkPropChange::CommentAlignChange(Some(tmp_x), None),
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
                            NetworkPropChange::CommentAlignChange(None, Some(tmp_y)),
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
                NetworkPropChange::ColorChange((0, new_color).into()),
            ));
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
        _settings: &<NetworkDomain as Domain>::SettingsT,
        q: &<NetworkDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveNetworkTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
            NetworkOrdinalMovement,
            NetworkElementOrVertex,
            NetworkPropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<NetworkOrdinalMovement, NetworkElementOrVertex, NetworkPropChange>,
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
                                NetworkPropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
                        }
                        NetworkPropChange::CommentAlignChange(x, y) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                NetworkPropChange::CommentAlignChange(
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
            align: self.align,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}
