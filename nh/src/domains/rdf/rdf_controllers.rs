use super::rdf_models::{
    RdfDiagram, RdfElement, RdfGraph, RdfLiteral, RdfNode, RdfPredicate, RdfTargettableElement,
};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    BucketNoT, ColorBundle, ColorChangeData, ContainerModel, ControllerAdapter, DiagramAdapter,
    DiagramController, DiagramControllerGen2, DiagramSettings, DiagramSettings2, Domain,
    ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus,
    GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand, MGlobalColor, Model,
    MultiDiagramController, PaletteEditBuffer, PositionNoT, ProjectCommand, PropertiesStatus,
    Queryable, SelectionStatus, ShowSettingsResult, SnapManager, TargettingStatus, Tool,
    ToolPalette, TryMerge, View,
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
use crate::{
    CustomModal, CustomModalResult, DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF,
    DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct RdfDomain;
impl Domain for RdfDomain {
    type SettingsT = RdfSettings;
    type CommonElementT = RdfElement;
    type DiagramModelT = RdfDiagram;
    type CommonElementViewT = RdfElementView;
    type ViewTargettingSectionT = RdfElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveRdfTool;
    type OrdinalMovementT = RdfOrdinalMovement;
    type AddCommandElementT = RdfElementOrVertex;
    type PropChangeT = RdfPropChange;
}

type PackageViewT = PackageView<RdfDomain, RdfGraphAdapter>;
type LinkViewT = MulticonnectionView<RdfDomain, RdfPredicateAdapter>;

#[derive(Clone, Copy, Debug)]
pub struct RdfOrdinalMovement {}

#[derive(Clone)]
pub enum RdfPropChange {
    NameChange(Arc<String>),
    IriChange(Arc<String>),

    ContentChange(Arc<String>),
    DataTypeChange(Arc<String>),
    LangTagChange(Arc<String>),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
    FlipMulticonnection(FlipMulticonnection),
}

impl Debug for RdfPropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "RdfPropChange::{}",
            match self {
                Self::NameChange(name) => format!("NameChange({})", name),
                Self::IriChange(iri) => format!("IriChange({})", iri),

                Self::ContentChange(content) => format!("ContentChange({})", content),
                Self::DataTypeChange(datatype) => format!("DataTypeChange({})", datatype),
                Self::LangTagChange(langtag) => format!("LangTagChange({})", langtag),

                Self::ColorChange(_color) => format!("ColorChange(..)"),
                Self::CommentChange(comment) => format!("CommentChange({})", comment),
                Self::FlipMulticonnection(_) => format!("FlipMulticonnection"),
            }
        )
    }
}

impl TryFrom<&RdfPropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &RdfPropChange) -> Result<Self, Self::Error> {
        match value {
            RdfPropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for RdfPropChange {
    fn from(value: ColorChangeData) -> Self {
        RdfPropChange::ColorChange(value)
    }
}
impl TryFrom<RdfPropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: RdfPropChange) -> Result<Self, Self::Error> {
        match value {
            RdfPropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for RdfPropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::IriChange(_), newer @ Self::IriChange(_))
            | (Self::ContentChange(_), newer @ Self::ContentChange(_))
            | (Self::DataTypeChange(_), newer @ Self::DataTypeChange(_))
            | (Self::LangTagChange(_), newer @ Self::LangTagChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From, derive_more::TryInto)]
pub enum RdfElementOrVertex {
    Element(RdfElementView),
    Vertex(VertexInformation),
}

impl Debug for RdfElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "RdfElementOrVertex::???")
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "RdfDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum RdfElementView {
    Graph(ERef<PackageViewT>),
    Literal(ERef<RdfLiteralView>),
    Node(ERef<RdfNodeView>),
    Predicate(ERef<LinkViewT>),
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct RdfControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfDiagram>,
}

impl ControllerAdapter<RdfDomain> for RdfControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<RdfDomain, RdfDiagramAdapter>;

    fn model(&self) -> ERef<RdfDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<RdfDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "rdf"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::rdf_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn insert_element(
        &mut self,
        parent: ModelUuid,
        element: RdfElement,
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
        undo: &mut Vec<(ModelUuid, RdfElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("RDF Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New RDF Diagram".to_owned().into(),
                RdfDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct RdfDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: RdfDiagramBuffer,
}

#[derive(Clone, Default)]
struct RdfDiagramBuffer {
    name: String,
    comment: String,
}

impl RdfDiagramAdapter {
    fn new(model: ERef<RdfDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: RdfDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<RdfDomain> for RdfDiagramAdapter {
    fn model(&self) -> ERef<RdfDiagram> {
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
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        element: RdfElement,
    ) -> Result<RdfElementView, HashSet<ModelUuid>> {
        let v = match element {
            RdfElement::RdfGraph(rw_lock) => RdfElementView::from(new_rdf_graph_view(
                rw_lock,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 100.0),
                },
            )),
            RdfElement::RdfLiteral(rw_lock) => {
                RdfElementView::from(new_rdf_literal_view(rw_lock, egui::Pos2::ZERO))
            }
            RdfElement::RdfNode(rw_lock) => {
                RdfElementView::from(new_rdf_node_view(rw_lock, egui::Pos2::ZERO))
            }
            RdfElement::RdfPredicate(rw_lock) => {
                let m = rw_lock.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.uuid());
                let (source_view, target_view) = match (q.get_view_for(&sid), q.get_view_for(&tid))
                {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                RdfElementView::from(new_rdf_predicate_view(
                    rw_lock.clone(),
                    source_view,
                    target_view,
                ))
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &RdfElement) -> Arc<String> {
        match e {
            RdfElement::RdfGraph(inner) => inner.read().iri.clone(),
            RdfElement::RdfLiteral(inner) => inner.read().content.clone(),
            RdfElement::RdfNode(inner) => inner.read().iri.clone(),
            RdfElement::RdfPredicate(inner) => inner.read().iri.clone(),
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
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
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
                RdfPropChange::ColorChange((0, new_color).into()),
            ));
        }
    }
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        _drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                RdfPropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        };

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                RdfPropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                RdfPropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                RdfPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                RdfPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::CommentChange(model.comment.clone()),
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
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        // TODO: re-enable when sophia's getrandom dependency gets updated
        #[cfg(not(target_arch = "wasm32"))]
        {
            /* TODO: RDF import & export
            if ui.button("Import RDF data").clicked() {}
            */
            if ui.button("SPARQL Queries").clicked() {
                let uuid = uuid::Uuid::now_v7();
                commands.push(ProjectCommand::AddCustomTab(
                    uuid,
                    Arc::new(RwLock::new(super::rdf_queries::SparqlQueriesTab::new(
                        self.model.clone(),
                    ))),
                ));
            }
            ui.separator();
        }
    }
    fn try_handle_custom_shortcut(
        &mut self,
        settings: &RdfSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<RdfDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveRdfTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialRdfElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, RdfElement>) {
        let (new_model, models) = super::rdf_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, RdfElement>) {
        let models = super::rdf_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
}

fn new_controlller(
    model: ERef<RdfDiagram>,
    name: String,
    elements: Vec<RdfElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            RdfControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                RdfDiagramAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New RDF diagram {}", no);

    let diagram = ERef::new(RdfDiagram::new(ModelUuid::now_v7(), name.clone(), vec![]));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (node, node_view) = new_rdf_node(
        "http://www.w3.org/People/EM/contact#me",
        egui::Pos2::new(300.0, 100.0),
    );

    let (literal_model, literal_view) = new_rdf_literal(
        "Eric Miller",
        "http://www.w3.org/2001/XMLSchema#string",
        "en",
        egui::Pos2::new(300.0, 200.0),
    );

    let (predicate, predicate_view) = new_rdf_predicate(
        "http://www.w3.org/2000/10/swap/pim/contact#fullName",
        (node.clone(), node_view.clone().into()),
        (literal_model.clone().into(), literal_view.clone().into()),
    );

    let (graph, graph_view) = new_rdf_graph(
        "http://subgraph",
        egui::Rect::from_x_y_ranges(100.0..=500.0, 300.0..=500.0),
    );

    let name = format!("Demo RDF diagram {}", no);
    let diagram = ERef::new(RdfDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            node.into(),
            literal_model.into(),
            predicate.into(),
            graph.into(),
        ],
    ));
    new_controlller(
        diagram,
        name,
        vec![
            node_view.into(),
            literal_view.into(),
            predicate_view.into(),
            graph_view.into(),
        ],
    )
}

pub fn stress_test<const N1: usize, const DX: u32, const DY: u32>(
    no: u32,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (mut models, mut views): (Vec<_>, Vec<_>) = Default::default();

    for xx in 0..N1 {
        for yy in 0..100 {
            let (node_st, node_st_view) = new_rdf_node(
                "http://www.w3.org/People/EM/contact#me",
                egui::Pos2::new(100.0 + xx as f32 * DX as f32, 200.0 + yy as f32 * DY as f32),
            );
            models.push(node_st.into());
            views.push(node_st_view.into());
        }
    }

    let name = format!("Stress RDF diagram {}", no);
    let diagram = ERef::new(RdfDiagram::new(ModelUuid::now_v7(), name.clone(), models));
    new_controlller(diagram, name, views)
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        RdfDomain,
        RdfControllerAdapter,
        DiagramControllerGen2<RdfDomain, RdfDiagramAdapter>,
    >>(&uuid)?)
}

pub struct RdfSettings {
    palette: RwLock<ToolPalette<RdfToolStage, RdfDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<RdfToolStage, RdfElementView>>,
}
impl DiagramSettings for RdfSettings {
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
                        RdfToolStage::Literal {
                            content,
                            datatype,
                            language,
                        } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Content", content)
                                .changed();
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Datatype", datatype)
                                .changed();
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Language", language)
                                .changed();
                        }
                        RdfToolStage::Node { iri } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("IRI", iri)
                                .changed();
                        }
                        RdfToolStage::PredicateStart { iri } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("IRI", iri)
                                .changed();
                        }
                        RdfToolStage::GraphStart { iri } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("IRI", iri)
                                .changed();
                        }
                        RdfToolStage::PredicateEnd | RdfToolStage::GraphEnd => unreachable!(),
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
            self.palette.read().unwrap().serialize()?.into(),
        );
        Ok(table.into())
    }
}
impl DiagramSettings2<RdfDomain> for RdfSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    RdfToolStage,
                    String,
                    RdfElementView,
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
            "Elements",
            vec![
                (
                    RdfToolStage::Literal {
                        content: "Eric Miller".to_owned(),
                        datatype: "http://www.w3.org/2001/XMLSchema#string".to_owned(),
                        language: "en".to_owned(),
                    },
                    "Literal",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num1,
                    )),
                ),
                (
                    RdfToolStage::Node {
                        iri: "http://iri".to_owned(),
                    },
                    "Node",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num2,
                    )),
                ),
            ],
        ),
        (
            "Relationships",
            vec![(
                RdfToolStage::PredicateStart {
                    iri: "http://iri".to_owned(),
                },
                "Predicate",
                Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Num3,
                )),
            )],
        ),
        (
            "Other",
            vec![(
                RdfToolStage::GraphStart {
                    iri: "http://graph".to_owned(),
                },
                "Graph",
                Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Num4,
                )),
            )],
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

    Box::new(RdfSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
    })
}

fn view_for_stage(s: &RdfToolStage) -> RdfElementView {
    match s {
        RdfToolStage::Literal {
            content,
            datatype,
            language,
        } => {
            let literal_view = new_rdf_literal(content, datatype, language, egui::Pos2::ZERO).1;
            literal_view.into()
        }
        RdfToolStage::Node { iri } => {
            let node_view = new_rdf_node(iri, egui::Pos2::ZERO).1;
            node_view.into()
        }
        RdfToolStage::PredicateStart { iri } => {
            let d1 = new_rdf_node("dummy", egui::Pos2::ZERO);
            let d2 = new_rdf_literal("dummy", "", "", egui::Pos2::new(100.0, 75.0));
            let predicate_view =
                new_rdf_predicate(iri, (d1.0, d1.1.into()), (d2.0.into(), d2.1.into())).1;
            predicate_view.into()
        }
        RdfToolStage::GraphStart { iri } => {
            let graph_view = new_rdf_graph(
                iri,
                egui::Rect {
                    min: egui::Pos2::ZERO,
                    max: egui::Pos2::new(100.0, 50.0),
                },
            )
            .1;
            graph_view.into()
        }
        RdfToolStage::PredicateEnd | RdfToolStage::GraphEnd => unreachable!(),
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(RdfSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
    }))
}

inventory::submit! {DiagramInfo {
    type_indentifier: "rdf",
    pretty_name: "Resource Description Framework",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "",
        description: "Resource Description Framework (RDF)",
        constructors: &[
            ("empty", &(new as DiagramConstructorF)),
            ("demo", &(demo as DiagramConstructorF)),
            ("stress test 2k", &(stress_test::<20, 20, 20> as DiagramConstructorF)),
            ("stress test 5k", &(stress_test::<50, 20, 20> as DiagramConstructorF)),
            ("stress test 10k", &(stress_test::<100, 20, 20> as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RdfToolStage {
    Literal {
        content: String,
        datatype: String,
        language: String,
    },
    Node {
        iri: String,
    },
    PredicateStart {
        iri: String,
    },
    PredicateEnd,
    GraphStart {
        iri: String,
    },
    GraphEnd,
}

enum PartialRdfElement {
    None,
    Some(RdfElementView),
    Predicate {
        iri: String,
        source: ERef<RdfNode>,
        dest: Option<RdfTargettableElement>,
    },
    Graph {
        iri: String,
        a: egui::Pos2,
        b: Option<egui::Pos2>,
    },
}

pub struct NaiveRdfTool {
    uuid: uuid::Uuid,
    initial_stage: RdfToolStage,
    current_stage: RdfToolStage,
    result: PartialRdfElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveRdfTool {
    fn try_spend(&mut self) {
        self.result = PartialRdfElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<RdfDomain> for NaiveRdfTool {
    type Stage = RdfToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: RdfToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialRdfElement::None,
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

    fn targetting_for_section(&self, element: Option<RdfElement>) -> egui::Color32 {
        match element {
            None => match self.current_stage {
                RdfToolStage::Literal { .. }
                | RdfToolStage::Node { .. }
                | RdfToolStage::GraphStart { .. }
                | RdfToolStage::GraphEnd => TARGETTABLE_COLOR,
                RdfToolStage::PredicateStart { .. } | RdfToolStage::PredicateEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(RdfElement::RdfGraph(..)) => match self.current_stage {
                RdfToolStage::Literal { .. } | RdfToolStage::Node { .. } => TARGETTABLE_COLOR,
                RdfToolStage::PredicateStart { .. }
                | RdfToolStage::PredicateEnd
                | RdfToolStage::GraphStart { .. }
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfLiteral(..)) => match self.current_stage {
                RdfToolStage::PredicateEnd => TARGETTABLE_COLOR,
                RdfToolStage::Literal { .. }
                | RdfToolStage::Node { .. }
                | RdfToolStage::PredicateStart { .. }
                | RdfToolStage::GraphStart { .. }
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfNode(..)) => match self.current_stage {
                RdfToolStage::PredicateStart { .. } | RdfToolStage::PredicateEnd => {
                    TARGETTABLE_COLOR
                }
                RdfToolStage::Literal { .. }
                | RdfToolStage::Node { .. }
                | RdfToolStage::GraphStart { .. }
                | RdfToolStage::GraphEnd => NON_TARGETTABLE_COLOR,
            },
            Some(RdfElement::RdfPredicate(..)) => todo!(),
        }
    }
    fn draw_status_hint(
        &self,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match &self.result {
            PartialRdfElement::Predicate { source, .. } => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            PartialRdfElement::Graph { a, .. } => {
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
                RdfToolStage::Literal {
                    content,
                    datatype,
                    language,
                },
                _,
            ) => {
                let (_literal_model, literal_view) =
                    new_rdf_literal(content, datatype, language, pos);

                self.result = PartialRdfElement::Some(literal_view.into());
                self.event_lock = true;
            }
            (RdfToolStage::Node { iri }, _) => {
                let (_node, node_view) = new_rdf_node(iri, pos);
                self.result = PartialRdfElement::Some(node_view.into());
                self.event_lock = true;
            }
            (RdfToolStage::GraphStart { iri }, _) => {
                self.result = PartialRdfElement::Graph {
                    iri: iri.clone(),
                    a: pos,
                    b: None,
                };
                self.current_stage = RdfToolStage::GraphEnd;
                self.event_lock = true;
            }
            (RdfToolStage::GraphEnd, PartialRdfElement::Graph { b, .. }) => *b = Some(pos),
            _ => {}
        }
    }
    fn add_section(&mut self, controller: RdfElement) {
        if self.event_lock {
            return;
        }

        match controller {
            RdfElement::RdfGraph(..) => {}
            RdfElement::RdfLiteral(inner) => match (&self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { dest, .. }) => {
                    *dest = Some(RdfTargettableElement::from(inner));
                    self.event_lock = true;
                }
                _ => {}
            },
            RdfElement::RdfNode(inner) => match (&self.current_stage, &mut self.result) {
                (RdfToolStage::PredicateStart { iri }, PartialRdfElement::None) => {
                    self.result = PartialRdfElement::Predicate {
                        iri: iri.clone(),
                        source: inner,
                        dest: None,
                    };
                    self.current_stage = RdfToolStage::PredicateEnd;
                    self.event_lock = true;
                }
                (RdfToolStage::PredicateEnd, PartialRdfElement::Predicate { dest, .. }) => {
                    *dest = Some(RdfTargettableElement::from(inner));
                }
                _ => {}
            },
            RdfElement::RdfPredicate(..) => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <RdfDomain as Domain>::OrdinalMovementT,
                <RdfDomain as Domain>::AddCommandElementT,
                <RdfDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &self.result {
            PartialRdfElement::Some(element) => {
                let element = element.clone();
                let esm: Option<Box<dyn CustomModal>> = match &element {
                    RdfElementView::Literal(inner) => {
                        Some(Box::new(RdfLiteralSetupModal::from(&inner.read().model)))
                    }
                    RdfElementView::Node(inner) => Some(Box::new(RdfIriBasedSetupModal::from(
                        RdfElement::from(inner.read().model.clone()),
                    ))),
                    RdfElementView::Predicate(..) | RdfElementView::Graph(..) => unreachable!(),
                };
                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: RdfElementView::from(element).into(),
                    into_model: true,
                });
                Ok(esm)
            }
            PartialRdfElement::Predicate {
                iri,
                source,
                dest: Some(dest),
                ..
            } => {
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.uuid());
                if let (Some(source_controller), Some(dest_controller)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_controller.uuid(), preferred_container)
                    && q.is_contained(&dest_controller.uuid(), preferred_container)
                    && q.are_siblings(&source_controller.uuid(), &dest_controller.uuid())
                {
                    self.current_stage = self.initial_stage.clone();

                    let (predicate_model, predicate_view) = new_rdf_predicate(
                        iri,
                        (source.clone(), source_controller),
                        (dest.clone(), dest_controller),
                    );

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: RdfElementView::from(predicate_view).into(),
                        into_model: true,
                    });
                    Ok(Some(Box::new(RdfIriBasedSetupModal::from(
                        RdfElement::from(predicate_model),
                    ))))
                } else {
                    Err(())
                }
            }
            PartialRdfElement::Graph { iri, a, b: Some(b) } => {
                self.current_stage = self.initial_stage.clone();

                let (graph_model, graph_view) =
                    new_rdf_graph(iri, egui::Rect::from_two_pos(*a, *b));

                self.try_spend();
                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: RdfElementView::from(graph_view).into(),
                    into_model: true,
                });
                Ok(Some(Box::new(RdfIriBasedSetupModal::from(
                    RdfElement::from(graph_model),
                ))))
            }
            _ => Err(()),
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

struct RdfIriBasedSetupModal {
    model: RdfElement,
    first_frame: bool,
    iri_buffer: String,
}

impl From<RdfElement> for RdfIriBasedSetupModal {
    fn from(model: RdfElement) -> Self {
        let iri_buffer = match &model {
            RdfElement::RdfGraph(eref) => (*eref.read().iri).clone(),
            RdfElement::RdfNode(eref) => (*eref.read().iri).clone(),
            RdfElement::RdfPredicate(eref) => (*eref.read().iri).clone(),
            RdfElement::RdfLiteral(..) => unreachable!(),
        };
        Self {
            model,
            first_frame: true,
            iri_buffer,
        }
    }
}

impl CustomModal for RdfIriBasedSetupModal {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("IRI:");
        let r = ui.text_edit_singleline(&mut self.iri_buffer);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let iri = Arc::new(self.iri_buffer.clone());
                match &self.model {
                    RdfElement::RdfGraph(inner) => inner.write().iri = iri,
                    RdfElement::RdfNode(inner) => inner.write().iri = iri,
                    RdfElement::RdfPredicate(inner) => inner.write().iri = iri,
                    RdfElement::RdfLiteral(_inner) => unreachable!(),
                }
                result = CustomModalResult::CloseModified(*self.model.uuid());
            }
            if ui.button(gdc.translate_0("nh-generic-cancel")).clicked() {
                result = CustomModalResult::CloseUnmodified;
            }
        });

        result
    }
}

fn new_rdf_graph(iri: &str, bounds_rect: egui::Rect) -> (ERef<RdfGraph>, ERef<PackageViewT>) {
    let graph_model = ERef::new(RdfGraph::new(
        ModelUuid::now_v7(),
        iri.to_owned(),
        Vec::new(),
    ));
    let graph_view = new_rdf_graph_view(graph_model.clone(), bounds_rect);

    (graph_model, graph_view)
}
fn new_rdf_graph_view(model: ERef<RdfGraph>, bounds_rect: egui::Rect) -> ERef<PackageViewT> {
    let m = model.read();
    PackageView::new(
        ViewUuid::now_v7().into(),
        RdfGraphAdapter {
            model: model.clone(),
            background_color: MGlobalColor::None,
            iri_buffer: (*m.iri).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        Vec::new(),
        bounds_rect,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct RdfGraphAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfGraph>,
    background_color: MGlobalColor,

    #[nh_context_serde(skip_and_default)]
    iri_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl PackageAdapter<RdfDomain> for RdfGraphAdapter {
    fn model_section(&self) -> RdfElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().iri.clone()
    }

    fn get_element_pos(&self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        self.model.read().get_element_pos(uuid)
    }
    fn insert_element(
        &mut self,
        position: Option<PositionNoT>,
        element: RdfElement,
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
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("IRI:", &mut self.iri_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::IriChange(Arc::new(self.iri_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        command: &InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                RdfPropChange::IriChange(iri) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::IriChange(model.iri.clone()),
                    ));
                    model.iri = iri.clone();
                }
                RdfPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::CommentChange(model.comment.clone()),
                    ));
                    model.comment = comment.clone();
                }
                RdfPropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::ColorChange(ColorChangeData {
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
        self.iri_buffer = (*model.iri).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(&self, new_uuid: ModelUuid, m: &mut HashMap<ModelUuid, RdfElement>) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(RdfElement::RdfGraph(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(new_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };
        Self {
            model,
            background_color: self.background_color.clone(),
            iri_buffer: self.iri_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, RdfElement>) {
        let mut w = self.model.write();
        for e in w.contained_elements.iter_mut() {
            if let Some(new_model) = m.get(&*e.uuid()) {
                *e = new_model.clone();
            }
        }
    }
}

fn new_rdf_node(iri: &str, position: egui::Pos2) -> (ERef<RdfNode>, ERef<RdfNodeView>) {
    let node_model = ERef::new(RdfNode::new(ModelUuid::now_v7(), iri.to_owned()));
    let node_view = new_rdf_node_view(node_model.clone(), position);
    (node_model, node_view)
}
fn new_rdf_node_view(model: ERef<RdfNode>, position: egui::Pos2) -> ERef<RdfNodeView> {
    let m = model.read();
    let node_view = ERef::new(RdfNodeView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        iri_buffer: (*m.iri).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_radius: egui::Vec2::ZERO,
    });
    node_view
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct RdfNodeView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<RdfNode>,

    #[nh_context_serde(skip_and_default)]
    iri_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    #[nh_context_serde(skip_and_default)]
    pub bounds_radius: egui::Vec2,
}

impl RdfNodeView {
    fn predicate_button_rect(&self, ui_scale: f32) -> egui::Rect {
        let b_radius = 8.0;
        let b_center = self.position
            + egui::Vec2::new(
                self.bounds_radius.x + b_radius / ui_scale,
                -self.bounds_radius.y + b_radius / ui_scale,
            );
        egui::Rect::from_center_size(b_center, egui::Vec2::splat(2.0 * b_radius / ui_scale))
    }
}

impl Entity for RdfNodeView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for RdfNodeView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<RdfElement> for RdfNodeView {
    fn model(&self) -> RdfElement {
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

impl ElementControllerGen2<RdfDomain> for RdfNodeView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) -> PropertiesStatus<RdfDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("IRI:", &mut self.iri_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::IriChange(Arc::new(self.iri_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        _q: &<RdfDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &RdfSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        let text_bounds = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        self.bounds_radius = text_bounds.size() / 1.5;

        canvas.draw_ellipse(
            self.position,
            self.bounds_radius,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.model.read().iri,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            let b_rect = self.predicate_button_rect(ui_scale);
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
                "↘",
                14.0 / ui_scale,
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
        _settings: &<RdfDomain as Domain>::SettingsT,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveRdfTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
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
                    && self.predicate_button_rect(ehc.ui_scale).contains(pos) =>
            {
                *tool = Some(NaiveRdfTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage: RdfToolStage::PredicateStart {
                        iri: "http://www.w3.org/2000/10/swap/pim/contact#fullName".to_owned(),
                    },
                    current_stage: RdfToolStage::PredicateEnd,
                    result: PartialRdfElement::Predicate {
                        iri: "http://www.w3.org/2000/10/swap/pim/contact#fullName".to_owned(),
                        source: self.model.clone(),
                        dest: None,
                    },
                    event_lock: true,
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
        command: &InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
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
                        RdfPropChange::IriChange(iri) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                RdfPropChange::IriChange(model.iri.clone()),
                            ));
                            model.iri = iri.clone();
                        }
                        RdfPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                RdfPropChange::CommentChange(model.comment.clone()),
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
        self.iri_buffer = (*model.iri).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (RdfElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, RdfElementView>,
        c: &mut HashMap<ViewUuid, RdfElementView>,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(RdfElement::RdfNode(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            iri_buffer: self.iri_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_radius: self.bounds_radius,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_rdf_literal(
    content: &str,
    datatype: &str,
    langtag: &str,
    position: egui::Pos2,
) -> (ERef<RdfLiteral>, ERef<RdfLiteralView>) {
    let literal_model = ERef::new(RdfLiteral::new(
        ModelUuid::now_v7(),
        content.to_owned(),
        datatype.to_owned(),
        langtag.to_owned(),
    ));
    let literal_view = new_rdf_literal_view(literal_model.clone(), position);
    (literal_model, literal_view)
}
fn new_rdf_literal_view(model: ERef<RdfLiteral>, position: egui::Pos2) -> ERef<RdfLiteralView> {
    let m = model.read();
    let literal_view = ERef::new(RdfLiteralView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),

        content_buffer: (*m.content).to_owned(),
        datatype_buffer: (*m.datatype).to_owned(),
        langtag_buffer: (*m.langtag).to_owned(),
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_pos(position),
    });
    literal_view
}

struct RdfLiteralSetupModal {
    model: ERef<RdfLiteral>,
    first_frame: bool,
    content_buffer: String,
    datatype_buffer: String,
    langtag_buffer: String,
}

impl From<&ERef<RdfLiteral>> for RdfLiteralSetupModal {
    fn from(model: &ERef<RdfLiteral>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            first_frame: true,
            content_buffer: (*m.content).clone(),
            datatype_buffer: (*m.datatype).clone(),
            langtag_buffer: (*m.langtag).clone(),
        }
    }
}

impl CustomModal for RdfLiteralSetupModal {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label("Content:");
        let r = ui.text_edit_singleline(&mut self.content_buffer);
        ui.label("Datatype:");
        ui.text_edit_singleline(&mut self.datatype_buffer);
        ui.label("Langtag:");
        ui.text_edit_singleline(&mut self.langtag_buffer);
        ui.separator();

        if self.first_frame {
            r.request_focus();
            self.first_frame = false;
        }

        let mut result = CustomModalResult::KeepOpen;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-generic-ok")).clicked() {
                let mut m = self.model.write();
                m.content = Arc::new(self.content_buffer.clone());
                m.datatype = Arc::new(self.datatype_buffer.clone());
                m.langtag = Arc::new(self.langtag_buffer.clone());
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
pub struct RdfLiteralView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<RdfLiteral>,

    #[nh_context_serde(skip_and_default)]
    content_buffer: String,
    #[nh_context_serde(skip_and_default)]
    datatype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    langtag_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl Entity for RdfLiteralView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for RdfLiteralView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<RdfElement> for RdfLiteralView {
    fn model(&self) -> RdfElement {
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

impl ElementControllerGen2<RdfDomain> for RdfLiteralView {
    fn show_properties(
        &mut self,
        _gdc: &GlobalDrawingContext,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) -> PropertiesStatus<RdfDomain> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Content:", &mut self.content_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::ContentChange(Arc::new(self.content_buffer.clone())),
            ));
        }
        if ui
            .labeled_text_edit_singleline("Datatype:", &mut self.datatype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::DataTypeChange(Arc::new(self.datatype_buffer.clone())),
            ));
        };

        if ui
            .labeled_text_edit_singleline("Language:", &mut self.langtag_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::LangTagChange(Arc::new(self.langtag_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
        _q: &<RdfDomain as Domain>::QueryableT<'_>,
        _gdc: &GlobalDrawingContext,
        _settings: &RdfSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveRdfTool)>,
    ) -> TargettingStatus {
        // Draw shape and text
        self.bounds_rect = crate::domains::umlclass::umlclass_controllers::draw_uml_class(
            canvas,
            self.position,
            None,
            &self.model.read().content,
            None,
            false,
            &[],
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            self.highlight,
        );

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
            TargettingStatus::Drawn
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _settings: &<RdfDomain as Domain>::SettingsT,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveRdfTool>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
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
        command: &InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
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
                        RdfPropChange::ContentChange(content) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                RdfPropChange::ContentChange(model.content.clone()),
                            ));
                            model.content = content.clone();
                        }
                        RdfPropChange::DataTypeChange(datatype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                RdfPropChange::DataTypeChange(model.datatype.clone()),
                            ));
                            model.datatype = datatype.clone();
                        }
                        RdfPropChange::LangTagChange(langtag) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                RdfPropChange::LangTagChange(model.langtag.clone()),
                            ));
                            model.langtag = langtag.clone();
                        }
                        RdfPropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                RdfPropChange::CommentChange(model.comment.clone()),
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
        self.content_buffer = (*model.content).clone();
        self.datatype_buffer = (*model.datatype).clone();
        self.langtag_buffer = (*model.langtag).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        _flattened_views: &mut HashMap<ViewUuid, (RdfElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, RdfElementView>,
        c: &mut HashMap<ViewUuid, RdfElementView>,
        m: &mut HashMap<ModelUuid, RdfElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (ViewUuid::now_v7().into(), ModelUuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(RdfElement::RdfLiteral(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = old_model.clone_with(model_uuid);
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            content_buffer: self.content_buffer.clone(),
            datatype_buffer: self.datatype_buffer.clone(),
            langtag_buffer: self.langtag_buffer.clone(),
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

fn new_rdf_predicate(
    iri: &str,
    source: (ERef<RdfNode>, RdfElementView),
    target: (RdfTargettableElement, RdfElementView),
) -> (ERef<RdfPredicate>, ERef<LinkViewT>) {
    let predicate_model = ERef::new(RdfPredicate::new(
        ModelUuid::now_v7(),
        iri.to_owned(),
        source.0,
        target.0,
    ));
    let predicate_view = new_rdf_predicate_view(predicate_model.clone(), source.1, target.1);

    (predicate_model, predicate_view)
}
fn new_rdf_predicate_view(
    model: ERef<RdfPredicate>,
    source: RdfElementView,
    target: RdfElementView,
) -> ERef<LinkViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        std::iter::once(*m.source.read().uuid),
        *m.target.uuid(),
        target.min_shape(),
        None,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        RdfPredicateAdapter {
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
pub struct RdfPredicateAdapter {
    #[nh_context_serde(entity)]
    model: ERef<RdfPredicate>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: RdfPredicateTemporaries,
}

#[derive(Clone, Default)]
struct RdfPredicateTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,
    iri_buffer: String,
    comment_buffer: String,
}

impl MulticonnectionAdapter<RdfDomain> for RdfPredicateAdapter {
    fn model(&self) -> RdfElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        _center: egui::Pos2,
        _highlight: canvas::Highlight,
        _q: &<RdfDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<RdfDomain as Domain>::SettingsT,
        _canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<RdfDomain as Domain>::ToolT)>,
    ) -> Result<(), Arc<String>> {
        Err(self.model.read().iri.clone())
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
        if let RdfTargettableElement::RdfNode(t) = &w.target {
            let tmp = w.source.clone();
            w.source = t.clone();
            w.target = tmp.into();
            Ok(())
        } else {
            Err(())
        }
    }

    fn show_properties(
        &mut self,
        q: &<RdfDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) -> PropertiesStatus<RdfDomain> {
        if ui
            .labeled_text_edit_singleline("IRI:", &mut self.temporaries.iri_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::IriChange(Arc::new(self.temporaries.iri_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.temporaries.comment_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::CommentChange(Arc::new(self.temporaries.comment_buffer.clone())),
            ));
        }

        if ui.button("Switch source and destination").clicked()
            && let RdfTargettableElement::RdfNode(_) = &self.model.read().target
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                RdfPropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<RdfOrdinalMovement, RdfElementOrVertex, RdfPropChange>,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                RdfPropChange::IriChange(iri) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::IriChange(model.iri.clone()),
                    ));
                    model.iri = iri.clone();
                }
                RdfPropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        RdfPropChange::CommentChange(model.comment.clone()),
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
            (false, *model.source.read().uuid),
            ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::None),
        );
        self.temporaries.arrow_data.insert(
            (true, *model.target.uuid()),
            ArrowData::new_labelless(canvas::LineType::Solid, canvas::ArrowheadType::OpenTriangle),
        );

        self.temporaries.source_uuids.clear();
        self.temporaries
            .source_uuids
            .push(*model.source.read().uuid);
        self.temporaries.target_uuids.clear();
        self.temporaries.target_uuids.push(*model.target.uuid());

        self.temporaries.iri_buffer = (*model.iri).clone();
        self.temporaries.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(&self, new_uuid: ModelUuid, m: &mut HashMap<ModelUuid, RdfElement>) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(RdfElement::RdfPredicate(m)) = m.get(&old_model.uuid) {
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

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, RdfElement>) {
        let mut model = self.model.write();

        let source_uuid = *model.source.read().uuid();
        if let Some(RdfElement::RdfNode(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone().into();
        }

        let target_uuid = *model.target.uuid();
        if let Some(new_target) = m.get(&target_uuid).and_then(|e| e.as_targettable_element()) {
            model.target = new_target;
        }
    }
}
