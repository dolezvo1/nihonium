use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ColorBundle, ColorChangeData, ControllerAdapter, DeleteKind, DiagramAdapter, DiagramController,
    DiagramControllerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext,
    EventHandlingStatus, GenericQueryable, GlobalDrawingContext, InputEvent, InsensitiveCommand,
    MGlobalColor, MultiDiagramController, ProjectCommand, PropertiesStatus, Queryable,
    SelectionStatus, SnapManager, TargettingStatus, Tool, TryMerge, View,
};
use crate::common::diagram_settings::{
    DiagramSettings, DiagramSettings2, GroupDisplayStyle, PaletteEditBuffer, ShortCutStatus,
    ShowSettingsResult, ToolPalette,
};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::model::{BucketNoT, ContainerModel, DiagramModel, Model, PositionNoT};
use crate::common::project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer};
use crate::common::ui_ext::UiExt;
use crate::common::uuid::{ControllerUuid, ModelUuid, ViewUuid};
use crate::common::views::multiconnection_view::{
    self, ArrowData, Ending, FlipMulticonnection, MULTICONNECTION_SOURCE_BUCKET,
    MULTICONNECTION_TARGET_BUCKET, MulticonnectionAdapter, MulticonnectionView, VertexInformation,
};
use crate::common::views::ordered_views::OrderedViews;
use crate::domains::archimate::archimate_models::{
    ArchiMateConcept, ArchiMateConceptKind, ArchiMateConceptKindColorGroup,
    ArchiMateConceptKindShapeGroup, ArchiMateDiagram, ArchiMateElement, ArchiMateJunctionKind,
    ArchiMateRelationship, ArchiMateRelationshipEnding, ArchiMateRelationshipKind,
};
use crate::{
    CustomModal, DefaultNameF, DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF,
    DiagramConstructorF, DiagramCreationData, DiagramInfo, SetShortcut,
};
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock},
};

pub struct ArchiMateDomain;
impl Domain for ArchiMateDomain {
    type SettingsT = ArchiMateSettings;
    type CommonElementT = ArchiMateElement;
    type DiagramModelT = ArchiMateDiagram;
    type CommonElementViewT = ArchiMateElementView;
    type ViewTargettingSectionT = ArchiMateElement;
    type QueryableT<'a> = GenericQueryable<'a, Self>;
    type ToolT = NaiveArchiMateTool;
    type OrdinalMovementT = ArchiMateOrdinalMovement;
    type AddCommandElementT = ArchiMateElementOrVertex;
    type PropChangeT = ArchiMatePropChange;
}

type RelationshipViewT = MulticonnectionView<ArchiMateDomain, ArchiMateRelationshipAdapter>;

#[derive(Clone, Copy, Debug)]
pub struct ArchiMateOrdinalMovement {}

#[derive(Clone)]
pub enum ArchiMatePropChange {
    NameChange(Arc<String>),
    StereotypeChange(Arc<String>),

    ConceptKindChange(ArchiMateConceptKind),
    RelationshipKindChange(ArchiMateRelationshipKind),
    RelationshipJunctionKindChange(ArchiMateJunctionKind),
    RelationshipMultiplicityChange(bool, ModelUuid, Arc<String>),
    FlipMulticonnection(FlipMulticonnection),

    ColorChange(ColorChangeData),
    CommentChange(Arc<String>),
}

impl Debug for ArchiMatePropChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "ArchiMatePropChange::???(..)",)
    }
}

impl TryFrom<&ArchiMatePropChange> for FlipMulticonnection {
    type Error = ();

    fn try_from(value: &ArchiMatePropChange) -> Result<Self, Self::Error> {
        match value {
            ArchiMatePropChange::FlipMulticonnection(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl From<ColorChangeData> for ArchiMatePropChange {
    fn from(value: ColorChangeData) -> Self {
        ArchiMatePropChange::ColorChange(value)
    }
}
impl TryFrom<ArchiMatePropChange> for ColorChangeData {
    type Error = ();

    fn try_from(value: ArchiMatePropChange) -> Result<Self, Self::Error> {
        match value {
            ArchiMatePropChange::ColorChange(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryMerge for ArchiMatePropChange {
    fn try_merge(&self, newer: &Self) -> Option<Self>
    where
        Self: Sized,
    {
        match (self, newer) {
            (Self::NameChange(_), newer @ Self::NameChange(_))
            | (Self::StereotypeChange(_), newer @ Self::StereotypeChange(_))
            | (Self::CommentChange(_), newer @ Self::CommentChange(_)) => Some(newer.clone()),
            (
                Self::RelationshipMultiplicityChange(b1, m1, _),
                newer @ Self::RelationshipMultiplicityChange(b2, m2, _),
            ) if b1 == b2 && m1 == m2 => Some(newer.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, derive_more::From, derive_more::TryInto)]
pub enum ArchiMateElementOrVertex {
    Element(ArchiMateElementView),
    Vertex(VertexInformation),
}

impl Debug for ArchiMateElementOrVertex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "ArchiMateElementOrVertex::???")
    }
}

#[derive(Clone, derive_more::From, nh_derive::View, nh_derive::NHContextSerDeTag)]
#[view(default_passthrough = "eref", domain = "ArchiMateDomain")]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum ArchiMateElementView {
    Concept(ERef<ArchiMateConceptView>),
    Relationship(ERef<RelationshipViewT>),
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct ArchiMateControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<ArchiMateDiagram>,
}

impl ControllerAdapter<ArchiMateDomain> for ArchiMateControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<ArchiMateDomain, ArchiMateDiagramAdapter>;

    fn model(&self) -> ERef<ArchiMateDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<ArchiMateDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "archimate"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::archimate_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, ArchiMateElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("ArchiMate Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New ArchiMate Diagram".to_owned().into(),
                ArchiMateDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct ArchiMateDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<ArchiMateDiagram>,
    background_color: MGlobalColor,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    buffer: ArchiMateDiagramBuffer,
}

#[derive(Clone, Default)]
struct ArchiMateDiagramBuffer {
    name: String,
    comment: String,
}

impl ArchiMateDiagramAdapter {
    fn new(model: ERef<ArchiMateDiagram>) -> Self {
        let m = model.read();
        Self {
            model: model.clone(),
            background_color: MGlobalColor::None,
            buffer: ArchiMateDiagramBuffer {
                name: (*m.name).clone(),
                comment: (*m.comment).clone(),
            },
        }
    }
}

impl DiagramAdapter<ArchiMateDomain> for ArchiMateDiagramAdapter {
    fn model(&self) -> ERef<ArchiMateDiagram> {
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
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        element: ArchiMateElement,
    ) -> Result<ArchiMateElementView, HashSet<ModelUuid>> {
        let v = match element {
            ArchiMateElement::Concept(inner) => new_archimate_concept_view(
                inner,
                egui::Pos2::ZERO,
                ArchiMateConceptRenderStyle::BoxWithIcon,
                MGlobalColor::None,
            )
            .into(),
            ArchiMateElement::Relationship(inner) => {
                let m = inner.read();
                let (Some(sv), Some(tv)) = (
                    m.sources
                        .iter()
                        .map(|e| q.get_view_for(&e.concept.read().uuid))
                        .collect(),
                    m.targets
                        .iter()
                        .map(|e| q.get_view_for(&e.concept.read().uuid))
                        .collect(),
                ) else {
                    return Err(m
                        .sources
                        .iter()
                        .map(|e| *e.concept.read().uuid)
                        .chain(m.targets.iter().map(|e| *e.concept.read().uuid))
                        .collect());
                };
                new_archimate_relationship_view(inner.clone(), None, sv, tv).into()
            }
        };

        Ok(v)
    }
    fn label_for(&self, e: &ArchiMateElement) -> Arc<String> {
        match e {
            ArchiMateElement::Concept(inner) => {
                let r = inner.read();
                let mut s = String::new();
                s.push_str(r.kind.as_str());
                s.push_str(" (");
                s.push_str(&r.name);
                s.push(')');
                s.into()
            }
            ArchiMateElement::Relationship(inner) => inner.read().kind.as_str().to_owned().into(),
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
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
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
                ArchiMatePropChange::ColorChange((0, new_color).into()),
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
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
    ) {
        if ui
            .labeled_text_edit_singleline("Name:", &mut self.buffer.name)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                ArchiMatePropChange::NameChange(Arc::new(self.buffer.name.clone())),
            ));
        };

        if ui
            .labeled_text_edit_multiline("Comment:", &mut self.buffer.comment)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                std::iter::once(*view_uuid).collect(),
                ArchiMatePropChange::CommentChange(Arc::new(self.buffer.comment.clone())),
            ));
        }
    }

    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            ArchiMateOrdinalMovement,
            ArchiMateElementOrVertex,
            ArchiMatePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                ArchiMatePropChange::NameChange(name) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        ArchiMatePropChange::NameChange(model.name.clone()),
                    ));
                    model.name = name.clone();
                }
                ArchiMatePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        ArchiMatePropChange::ColorChange(ColorChangeData {
                            slot: 0,
                            color: self.background_color,
                        }),
                    ));
                    self.background_color = *color;
                }
                ArchiMatePropChange::CommentChange(comment) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        ArchiMatePropChange::CommentChange(model.comment.clone()),
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
        settings: &ArchiMateSettings,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> PropertiesStatus<ArchiMateDomain> {
        if let Some((uuid, ts)) = settings
            .palette
            .read()
            .unwrap()
            .find_matching_tool_stage(modifiers, key)
        {
            PropertiesStatus::ToolRequest(Some(NaiveArchiMateTool {
                uuid,
                initial_stage: ts.clone(),
                current_stage: ts,
                result: PartialArchiMateElement::None,
                event_lock: false,
                is_spent: None,
            }))
        } else {
            PropertiesStatus::Shown
        }
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, ArchiMateElement>) {
        let (new_model, models) = super::archimate_models::deep_copy_diagram(&self.model.read());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }
    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, ArchiMateElement>) {
        let models = super::archimate_models::enumerate_diagram(&self.model.read());
        (self.clone(), models)
    }
    fn top_sort_info(
        &self,
        m: &<ArchiMateDomain as Domain>::CommonElementT,
    ) -> crate::common::model::ModelTopSortInfo {
        super::archimate_models::top_sort_info(m)
    }
}

fn new_controlller(
    model: ERef<ArchiMateDiagram>,
    name: String,
    elements: Vec<ArchiMateElementView>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            ArchiMateControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                ArchiMateDiagramAdapter::new(model),
                elements,
            )],
        )),
    )
}

pub fn new(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let diagram = ERef::new(ArchiMateDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![],
    ));
    new_controlller(diagram, name.to_owned(), vec![])
}

pub fn demo(name: &str) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (createreservation, createreservation_view) = new_archimate_concept(
        "Create a Reservation",
        "Objective",
        ArchiMateConceptKind::Goal,
        egui::Pos2::new(200.0, 100.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );
    createreservation_view.write().refresh_buffers();

    let (bookingcreation, bookingcreation_view) = new_archimate_concept(
        "Booking Creation",
        "",
        ArchiMateConceptKind::Capability,
        egui::Pos2::new(400.0, 100.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (phone, phone_view) = new_archimate_concept(
        "Phone",
        "",
        ArchiMateConceptKind::BusinessInterface,
        egui::Pos2::new(300.0, 300.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (client, client_view) = new_archimate_concept(
        "Client",
        "",
        ArchiMateConceptKind::BusinessActor,
        egui::Pos2::new(200.0, 500.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (bookb, bookb_view) = new_archimate_concept(
        "Book",
        "",
        ArchiMateConceptKind::Service,
        egui::Pos2::new(400.0, 500.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (web, web_view) = new_archimate_concept(
        "Web Front-End",
        "",
        ArchiMateConceptKind::ApplicationInterface,
        egui::Pos2::new(200.0, 700.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (booka, booka_view) = new_archimate_concept(
        "Book",
        "",
        ArchiMateConceptKind::Service,
        egui::Pos2::new(400.0, 700.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (bookingsystem, bookingsystem_view) = new_archimate_concept(
        "Booking System",
        "",
        ArchiMateConceptKind::ApplicationComponent,
        egui::Pos2::new(200.0, 900.0),
        ArchiMateConceptRenderStyle::Icon,
        MGlobalColor::None,
    );

    let (node, node_view) = new_archimate_concept(
        "Server",
        "",
        ArchiMateConceptKind::Node,
        egui::Pos2::new(400.0, 900.0),
        ArchiMateConceptRenderStyle::Icon,
        MGlobalColor::None,
    );

    let (location, location_view) = new_archimate_concept(
        "Headquarters",
        "",
        ArchiMateConceptKind::Location,
        egui::Pos2::new(600.0, 900.0),
        ArchiMateConceptRenderStyle::BoxWithIcon,
        MGlobalColor::None,
    );

    let (e1, e1_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Serving,
        None,
        (phone.clone(), phone_view.clone().into()),
        (client.clone(), client_view.clone().into()),
    );
    let (e2, e2_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Assignment,
        None,
        (phone.clone(), phone_view.clone().into()),
        (bookb.clone(), bookb_view.clone().into()),
    );
    let (e3, e3_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Serving,
        None,
        (web.clone(), web_view.clone().into()),
        (client.clone(), client_view.clone().into()),
    );
    let (e4, e4_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Assignment,
        None,
        (web.clone(), web_view.clone().into()),
        (booka.clone(), booka_view.clone().into()),
    );
    let (e5, e5_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Realization,
        None,
        (booka.clone(), booka_view.clone().into()),
        (bookb.clone(), bookb_view.clone().into()),
    );
    let (e6, e6_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Composition,
        None,
        (bookingsystem.clone(), bookingsystem_view.clone().into()),
        (web.clone(), web_view.clone().into()),
    );
    let (e7, e7_view) = new_archimate_relationship(
        ArchiMateRelationshipKind::Realization,
        None,
        (bookingsystem.clone(), bookingsystem_view.clone().into()),
        (booka.clone(), booka_view.clone().into()),
    );

    let diagram = ERef::new(ArchiMateDiagram::new(
        ModelUuid::now_v7(),
        name.to_owned(),
        vec![
            createreservation.into(),
            bookingcreation.into(),
            phone.into(),
            client.into(),
            bookb.into(),
            web.into(),
            booka.into(),
            bookingsystem.into(),
            node.into(),
            location.into(),
            e1.into(),
            e2.into(),
            e3.into(),
            e4.into(),
            e5.into(),
            e6.into(),
            e7.into(),
        ],
    ));
    new_controlller(
        diagram,
        name.to_owned(),
        vec![
            createreservation_view.into(),
            bookingcreation_view.into(),
            phone_view.into(),
            client_view.into(),
            bookb_view.into(),
            web_view.into(),
            booka_view.into(),
            bookingsystem_view.into(),
            node_view.into(),
            location_view.into(),
            e1_view.into(),
            e2_view.into(),
            e3_view.into(),
            e4_view.into(),
            e5_view.into(),
            e6_view.into(),
            e7_view.into(),
        ],
    )
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        ArchiMateDomain,
        ArchiMateControllerAdapter,
        DiagramControllerGen2<ArchiMateDomain, ArchiMateDiagramAdapter>,
    >>(&uuid)?)
}

pub struct ArchiMateSettings {
    palette: RwLock<ToolPalette<ArchiMateToolStage, ArchiMateDomain>>,
    palette_edit_buffer: RwLock<PaletteEditBuffer<ArchiMateToolStage, ArchiMateElementView>>,
    element_buttons: Vec<(usize, usize, &'static str, &'static ElementButtonF)>,
}
impl DiagramSettings for ArchiMateSettings {
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
                PaletteEditBuffer::Group(_uuid, name, display_style) => {
                    let mut modified = false;

                    modified |= columns[1]
                        .labeled_text_edit_singleline("Label", name)
                        .changed();

                    columns[1].label("Display style");
                    egui::ComboBox::from_id_salt("group display style")
                        .selected_text(display_style.as_str())
                        .show_ui(&mut columns[1], |ui| {
                            for e in GroupDisplayStyle::VARIANTS {
                                modified |=
                                    ui.selectable_value(display_style, e, e.as_str()).clicked();
                            }
                        });

                    if modified {
                        w.set_from_buffer(buffer.clone());
                    }
                }
                PaletteEditBuffer::Tool(uuid, name, tool, view, ksc) => {
                    let mut modified = false;
                    modified |= columns[1]
                        .labeled_text_edit_singleline("Label", name)
                        .changed();

                    match crate::common::diagram_settings::show_shortcut(
                        &mut columns[1],
                        ksc,
                        shortcut_being_set
                            .as_ref()
                            .is_some_and(|e| e.is_diagram(uuid)),
                    ) {
                        ShortCutStatus::NoChange => {}
                        ShortCutStatus::Cleared => modified = true,
                        ShortCutStatus::Set => {
                            ret = ShowSettingsResult::SetShortcut(*uuid);
                        }
                        ShortCutStatus::CancelSet => {
                            ret = ShowSettingsResult::CancelShortcutSetting;
                        }
                    }

                    match tool {
                        ArchiMateToolStage::Concept {
                            stereotype,
                            name,
                            kind,
                            background_color,
                            with_edge_from: _,
                        } => {
                            modified |= columns[1]
                                .labeled_text_edit_singleline("Stereotype", stereotype)
                                .changed();

                            modified |= columns[1]
                                .labeled_text_edit_multiline("Name", name)
                                .changed();

                            columns[1].label("Kind");
                            egui::ComboBox::from_id_salt("concept kind")
                                .selected_text(kind.as_str())
                                .show_ui(&mut columns[1], |ui| {
                                    for e in ArchiMateConceptKind::VARIANTS {
                                        modified |=
                                            ui.selectable_value(kind, e, e.as_str()).clicked();
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
                        ArchiMateToolStage::RelationshipStart { kind } => {
                            columns[1].label("Line type");
                            egui::ComboBox::from_id_salt("line type")
                                .selected_text(kind.as_str())
                                .show_ui(&mut columns[1], |ui| {
                                    for e in ArchiMateRelationshipKind::VARIANTS {
                                        modified |=
                                            ui.selectable_value(kind, e, e.as_str()).clicked();
                                    }
                                });
                        }
                        ArchiMateToolStage::RelationshipEnd
                        | ArchiMateToolStage::RelationshipAddEnding { .. } => {
                            unreachable!()
                        }
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
impl DiagramSettings2<ArchiMateDomain> for ArchiMateSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                GroupDisplayStyle,
                Vec<(
                    uuid::Uuid,
                    ArchiMateToolStage,
                    String,
                    ArchiMateElementView,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        ),
    {
        self.palette.write().unwrap().for_each_mut(f);
    }
}

type ElementButtonF = dyn Fn(
    ERef<ArchiMateConcept>,
) -> (
    ArchiMateToolStage,
    ArchiMateToolStage,
    PartialArchiMateElement,
    bool,
);
mod buttons {
    use super::*;
    use std::sync::LazyLock;

    fn element_association(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        (
            ArchiMateToolStage::RelationshipStart {
                kind: ArchiMateRelationshipKind::AssociationUndirected,
            },
            ArchiMateToolStage::RelationshipEnd,
            PartialArchiMateElement::Relationship {
                source: m,
                dest: None,
            },
            true,
        )
    }
    fn element_serving(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        (
            ArchiMateToolStage::RelationshipStart {
                kind: ArchiMateRelationshipKind::Serving,
            },
            ArchiMateToolStage::RelationshipEnd,
            PartialArchiMateElement::Relationship {
                source: m,
                dest: None,
            },
            true,
        )
    }
    fn element_specialization(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        (
            ArchiMateToolStage::RelationshipStart {
                kind: ArchiMateRelationshipKind::Specialization,
            },
            ArchiMateToolStage::RelationshipEnd,
            PartialArchiMateElement::Relationship {
                source: m,
                dest: None,
            },
            true,
        )
    }

    fn element_motivation(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        let stage = ArchiMateToolStage::Concept {
            stereotype: "".to_owned(),
            name: "Stakeholder".to_owned(),
            kind: ArchiMateConceptKind::Stakeholder,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.read().uuid),
        };
        (stage.clone(), stage, PartialArchiMateElement::None, true)
    }
    fn element_strategy(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        let stage = ArchiMateToolStage::Concept {
            stereotype: "".to_owned(),
            name: "Resource".to_owned(),
            kind: ArchiMateConceptKind::Resource,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.read().uuid),
        };
        (stage.clone(), stage, PartialArchiMateElement::None, true)
    }
    fn element_business(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        let stage = ArchiMateToolStage::Concept {
            stereotype: "".to_owned(),
            name: "Business Actor".to_owned(),
            kind: ArchiMateConceptKind::BusinessActor,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.read().uuid),
        };
        (stage.clone(), stage, PartialArchiMateElement::None, true)
    }
    fn element_application(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        let stage = ArchiMateToolStage::Concept {
            stereotype: "".to_owned(),
            name: "Application Component".to_owned(),
            kind: ArchiMateConceptKind::ApplicationComponent,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.read().uuid),
        };
        (stage.clone(), stage, PartialArchiMateElement::None, true)
    }
    fn element_technology(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        let stage = ArchiMateToolStage::Concept {
            stereotype: "".to_owned(),
            name: "Node".to_owned(),
            kind: ArchiMateConceptKind::Node,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.read().uuid),
        };
        (stage.clone(), stage, PartialArchiMateElement::None, true)
    }
    fn element_integration(
        m: ERef<ArchiMateConcept>,
    ) -> (
        ArchiMateToolStage,
        ArchiMateToolStage,
        PartialArchiMateElement,
        bool,
    ) {
        let stage = ArchiMateToolStage::Concept {
            stereotype: "".to_owned(),
            name: "Deliverable".to_owned(),
            kind: ArchiMateConceptKind::Deliverable,
            background_color: MGlobalColor::None,
            with_edge_from: Some(*m.read().uuid),
        };
        (stage.clone(), stage, PartialArchiMateElement::None, true)
    }

    pub const ELEMENT_BUTTONS: LazyLock<
        Vec<(usize, usize, &'static str, &'static ElementButtonF)>,
    > = LazyLock::new(|| {
        vec![
            (0, 0, "\\", &element_association as &ElementButtonF),
            (0, 1, "↘", &element_serving as &ElementButtonF),
            (0, 2, "↘", &element_specialization as &ElementButtonF),
            (1, 0, "M", &element_motivation as &ElementButtonF),
            (1, 1, "S", &element_strategy as &ElementButtonF),
            (1, 2, "B", &element_business as &ElementButtonF),
            (1, 3, "A", &element_application as &ElementButtonF),
            (1, 4, "T", &element_technology as &ElementButtonF),
            (1, 5, "I", &element_integration as &ElementButtonF),
        ]
    });
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let relationships = ArchiMateRelationshipKind::VARIANTS
        .iter()
        .map(|e| {
            (
                ArchiMateToolStage::RelationshipStart { kind: *e },
                e.as_str(),
                None,
            )
        })
        .collect::<Vec<_>>();

    let es = |e: ArchiMateConceptKind| {
        (
            ArchiMateToolStage::Concept {
                stereotype: "".to_owned(),
                name: e.as_str().to_owned(),
                kind: e,
                background_color: MGlobalColor::None,
                with_edge_from: None,
            },
            e.as_str(),
            None,
        )
    };

    let palette_items = vec![
        ("Relationships", relationships),
        (
            "Common Domain",
            [
                ArchiMateConceptKind::Role,
                ArchiMateConceptKind::Collaboration,
                ArchiMateConceptKind::Path,
                ArchiMateConceptKind::Process,
                ArchiMateConceptKind::Function,
                ArchiMateConceptKind::Service,
                ArchiMateConceptKind::Event,
                ArchiMateConceptKind::Grouping,
                ArchiMateConceptKind::Location,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
        (
            "Motivation Domain",
            [
                ArchiMateConceptKind::Stakeholder,
                ArchiMateConceptKind::Driver,
                ArchiMateConceptKind::Assessment,
                ArchiMateConceptKind::Goal,
                ArchiMateConceptKind::Outcome,
                ArchiMateConceptKind::Principle,
                ArchiMateConceptKind::Requirement,
                ArchiMateConceptKind::Meaning,
                ArchiMateConceptKind::Value,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
        (
            "Strategy Domain",
            [
                ArchiMateConceptKind::Resource,
                ArchiMateConceptKind::Capability,
                ArchiMateConceptKind::ValueStream,
                ArchiMateConceptKind::CourseOfAction,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
        (
            "Business Domain",
            [
                ArchiMateConceptKind::BusinessActor,
                ArchiMateConceptKind::BusinessInterface,
                ArchiMateConceptKind::BusinessObject,
                ArchiMateConceptKind::Product,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
        (
            "Application Domain",
            [
                ArchiMateConceptKind::ApplicationComponent,
                ArchiMateConceptKind::ApplicationInterface,
                ArchiMateConceptKind::DataObject,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
        (
            "Technology Domain",
            [
                ArchiMateConceptKind::Node,
                ArchiMateConceptKind::TechnologyInterface,
                ArchiMateConceptKind::Device,
                ArchiMateConceptKind::SystemSoftware,
                ArchiMateConceptKind::Equipment,
                ArchiMateConceptKind::Facility,
                ArchiMateConceptKind::CommunicationNetwork,
                ArchiMateConceptKind::DistributionNetwork,
                ArchiMateConceptKind::Artifact,
                ArchiMateConceptKind::Material,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
        (
            "Implementation and Migration Domain",
            [
                ArchiMateConceptKind::WorkPackage,
                ArchiMateConceptKind::Deliverable,
                ArchiMateConceptKind::Plateau,
            ]
            .into_iter()
            .map(es)
            .collect(),
        ),
    ]
    .into_iter()
    .map(|e| {
        (
            e.0,
            GroupDisplayStyle::Grid,
            e.1.into_iter()
                .map(|e| {
                    let v = view_for_stage(&e.0);
                    (e.0, e.1, v, e.2)
                })
                .collect(),
        )
    })
    .collect();

    Box::new(ArchiMateSettings {
        palette: RwLock::new(ToolPalette::new(palette_items)),
        palette_edit_buffer: RwLock::new(PaletteEditBuffer::None),
        element_buttons: buttons::ELEMENT_BUTTONS.clone(),
    })
}

fn view_for_stage(s: &ArchiMateToolStage) -> ArchiMateElementView {
    match s {
        ArchiMateToolStage::Concept {
            stereotype,
            name,
            kind,
            background_color,
            with_edge_from: _,
        } => {
            let node_view = new_archimate_concept(
                name,
                stereotype,
                *kind,
                egui::Pos2::ZERO,
                ArchiMateConceptRenderStyle::Icon,
                *background_color,
            )
            .1;
            node_view.write().refresh_buffers();
            node_view.into()
        }
        ArchiMateToolStage::RelationshipStart { kind } => {
            let d1 = new_archimate_concept(
                "dummy",
                "",
                ArchiMateConceptKind::Node,
                egui::Pos2::ZERO,
                ArchiMateConceptRenderStyle::BoxWithIcon,
                MGlobalColor::None,
            );
            let d2 = new_archimate_concept(
                "dummy",
                "",
                ArchiMateConceptKind::Node,
                egui::Pos2::new(100.0, 75.0),
                ArchiMateConceptRenderStyle::BoxWithIcon,
                MGlobalColor::None,
            );

            let association_view = new_archimate_relationship(
                *kind,
                None,
                (d1.0.into(), d1.1.into()),
                (d2.0.into(), d2.1.into()),
            )
            .1;
            association_view.into()
        }
        ArchiMateToolStage::RelationshipEnd | ArchiMateToolStage::RelationshipAddEnding { .. } => {
            unreachable!()
        }
    }
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    let toml::Value::Table(value) = value else {
        return Err(());
    };
    Ok(Box::new(ArchiMateSettings {
        palette: ToolPalette::deserialize(value.get("palette").unwrap().clone(), view_for_stage)?
            .into(),
        palette_edit_buffer: PaletteEditBuffer::None.into(),
        element_buttons: buttons::ELEMENT_BUTTONS.clone(),
    }))
}

inventory::submit! {DiagramInfo {
    type_indentifier: "archimate",
    pretty_name: "ArchiMate diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "",
        description: "ArchiMate diagram (motivation elements, strategy layer elements, business layer elements, application layer elements, technology layer elements, implementation & migration elements, etc.)",
        constructors: &[
            ("empty", &((|no| format!("New ArchiMate diagram {}", no)) as DefaultNameF), &(new as DiagramConstructorF)),
            ("demo", &((|no| format!("Demo ArchiMate diagram {}", no)) as DefaultNameF), &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ArchiMateToolStage {
    Concept {
        stereotype: String,
        name: String,
        kind: ArchiMateConceptKind,
        background_color: MGlobalColor,
        with_edge_from: Option<ModelUuid>,
    },
    RelationshipStart {
        kind: ArchiMateRelationshipKind,
    },
    RelationshipEnd,
    RelationshipAddEnding {
        source: bool,
    },
}

enum PartialArchiMateElement {
    None,
    Some(ArchiMateElementView),
    Relationship {
        source: ERef<ArchiMateConcept>,
        dest: Option<ERef<ArchiMateConcept>>,
    },
    RelationshipEnding {
        relationship_model: ERef<ArchiMateRelationship>,
        new_model: Option<ModelUuid>,
    },
}

pub struct NaiveArchiMateTool {
    uuid: uuid::Uuid,
    initial_stage: ArchiMateToolStage,
    current_stage: ArchiMateToolStage,
    result: PartialArchiMateElement,
    event_lock: bool,
    is_spent: Option<bool>,
}

impl NaiveArchiMateTool {
    fn try_spend(&mut self) {
        self.result = PartialArchiMateElement::None;
        self.is_spent = self.is_spent.map(|_| true);
    }
    fn references(&self, uuid: &ModelUuid) -> bool {
        match &self.result {
            PartialArchiMateElement::None | PartialArchiMateElement::Some(_) => false,
            PartialArchiMateElement::Relationship { source, dest } => {
                *source.read().uuid == *uuid
                    || dest.as_ref().is_some_and(|e| *e.read().uuid == *uuid)
            }
            PartialArchiMateElement::RelationshipEnding {
                relationship_model,
                new_model,
            } => *relationship_model.read().uuid == *uuid || new_model.is_some_and(|e| e == *uuid),
        }
    }
}

const TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 255, 0, 31);
const NON_TARGETTABLE_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 0, 0, 31);

impl Tool<ArchiMateDomain> for NaiveArchiMateTool {
    type Stage = ArchiMateToolStage;

    fn new(uuid: uuid::Uuid, initial_stage: ArchiMateToolStage, repeat: bool) -> Self {
        Self {
            uuid,
            current_stage: initial_stage.clone(),
            initial_stage,
            result: PartialArchiMateElement::None,
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

    fn targetting_for_section(
        &self,
        element: Result<ArchiMateElement, ERef<ArchiMateDiagram>>,
    ) -> egui::Color32 {
        match element {
            Err(_) => match self.current_stage {
                ArchiMateToolStage::Concept { .. } => TARGETTABLE_COLOR,
                _ => NON_TARGETTABLE_COLOR,
            },
            Ok(ArchiMateElement::Relationship(_)) => unreachable!(),
            _ => TARGETTABLE_COLOR,
        }
    }
    fn draw_status_hint(
        &self,
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        canvas: &mut dyn NHCanvas,
        pos: egui::Pos2,
    ) {
        match (&self.result, &self.initial_stage) {
            (PartialArchiMateElement::Relationship { source, .. }, _) => {
                if let Some(source_view) = q.get_view_for(&source.read().uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                PartialArchiMateElement::RelationshipEnding {
                    relationship_model, ..
                },
                _,
            ) => {
                if let Some(view) = q.get_view_for(&relationship_model.read().uuid) {
                    canvas.draw_line(
                        [pos, view.position()],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            (
                _,
                ArchiMateToolStage::Concept {
                    with_edge_from: Some(source_uuid),
                    ..
                },
            ) => {
                if let Some(source_view) = q.get_view_for(source_uuid) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
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
                ArchiMateToolStage::Concept {
                    stereotype,
                    name,
                    kind,
                    background_color,
                    with_edge_from: _,
                },
                _,
            ) => {
                let view = new_archimate_concept(
                    name,
                    stereotype,
                    *kind,
                    pos,
                    ArchiMateConceptRenderStyle::BoxWithIcon,
                    *background_color,
                )
                .1;
                self.result = PartialArchiMateElement::Some(view.into());
                self.event_lock = true;
            }
            _ => {}
        }
    }
    fn add_section(&mut self, section: ArchiMateElement) {
        if self.event_lock {
            return;
        }

        match section {
            ArchiMateElement::Concept(c) => match (&self.current_stage, &mut self.result) {
                (ArchiMateToolStage::RelationshipStart { .. }, PartialArchiMateElement::None) => {
                    self.result = PartialArchiMateElement::Relationship {
                        source: c,
                        dest: None,
                    };
                    self.current_stage = ArchiMateToolStage::RelationshipEnd;
                    self.event_lock = true;
                }
                (
                    ArchiMateToolStage::RelationshipEnd,
                    PartialArchiMateElement::Relationship { dest, .. },
                ) => {
                    *dest = Some(c);
                    self.event_lock = true;
                }
                (
                    ArchiMateToolStage::RelationshipAddEnding { source },
                    &mut PartialArchiMateElement::RelationshipEnding {
                        ref relationship_model,
                        ref mut new_model,
                    },
                ) => {
                    let concept_uuid = *c.read().uuid;
                    let r = relationship_model.read();

                    if (*source
                        && !r
                            .sources
                            .iter()
                            .any(|e| *e.concept.read().uuid == concept_uuid))
                        || (!source
                            && !r
                                .targets
                                .iter()
                                .any(|e| *e.concept.read().uuid == concept_uuid))
                    {
                        *new_model = Some(concept_uuid);
                    }
                    self.event_lock = true;
                }
                _ => {}
            },
            ArchiMateElement::Relationship(..) => {}
        }
    }

    fn try_flush(
        &mut self,
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        preferred_container: &ViewUuid,
        preferred_bucket: BucketNoT,
        preferred_position: Option<PositionNoT>,
        commands: &mut Vec<
            InsensitiveCommand<
                <ArchiMateDomain as Domain>::OrdinalMovementT,
                <ArchiMateDomain as Domain>::AddCommandElementT,
                <ArchiMateDomain as Domain>::PropChangeT,
            >,
        >,
    ) -> Result<Option<Box<dyn CustomModal>>, ()> {
        match &mut self.result {
            PartialArchiMateElement::Some(element) => {
                let element = element.clone();
                let additional_edge = match &self.initial_stage {
                    ArchiMateToolStage::Concept {
                        with_edge_from: Some(source_uuid),
                        ..
                    } if let Some(source) = q.get_view_for(source_uuid)
                        && let ArchiMateElement::Concept(source_model) = source.model()
                        && let ArchiMateElement::Concept(target_model) = element.model()
                        && let nearest_common_container = q
                            .find_container(&source.uuid(), |uuid, _| {
                                uuid == preferred_container
                                    || q.is_contained(preferred_container, uuid)
                            })
                            .map(|e| e.0)
                            .unwrap_or_else(|| q.get_root()) =>
                    {
                        let edge_view = new_archimate_relationship(
                            ArchiMateRelationshipKind::AssociationUndirected,
                            None,
                            (source_model, source.clone().into()),
                            (target_model, element.clone()),
                        )
                        .1;
                        Some((nearest_common_container, edge_view))
                    }
                    _ => None,
                };

                self.try_spend();

                commands.push(InsensitiveCommand::AddDependency {
                    target: *preferred_container,
                    bucket: preferred_bucket,
                    position: preferred_position,
                    element: element.into(),
                    into_model: true,
                });
                if let Some((parent, e)) = additional_edge {
                    commands.push(InsensitiveCommand::AddDependency {
                        target: parent,
                        bucket: 0,
                        position: None,
                        element: ArchiMateElementView::from(e).into(),
                        into_model: true,
                    });
                }
                Ok(None)
            }
            PartialArchiMateElement::Relationship {
                source,
                dest: Some(dest),
                ..
            } if let ArchiMateToolStage::RelationshipStart { kind } = &self.initial_stage => {
                let (source_uuid, target_uuid) = (*source.read().uuid, *dest.read().uuid);
                if let (Some(source_view), Some(dest_view)) =
                    (q.get_view_for(&source_uuid), q.get_view_for(&target_uuid))
                    && q.is_contained(&source_view.uuid(), preferred_container)
                    && q.is_contained(&dest_view.uuid(), preferred_container)
                {
                    self.current_stage = self.initial_stage.clone();

                    let association_view = new_archimate_relationship(
                        *kind,
                        None,
                        (source.clone(), source_view),
                        (dest.clone(), dest_view),
                    )
                    .1;

                    self.try_spend();
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *preferred_container,
                        bucket: preferred_bucket,
                        position: preferred_position,
                        element: ArchiMateElementView::from(association_view).into(),
                        into_model: true,
                    });
                    Ok(None)
                } else {
                    Err(())
                }
            }
            PartialArchiMateElement::RelationshipEnding {
                relationship_model,
                new_model,
            } if let ArchiMateToolStage::RelationshipAddEnding { source } = &self.initial_stage
                && let Some(target) = q.get_viewuuid_for(&relationship_model.read().uuid())
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
            _ => Err(()),
        }
    }

    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ArchiMateConceptRenderStyle {
    BoxWithIcon,
    Icon,
}

fn element_button_rect(
    origin: egui::Pos2,
    ui_scale: f32,
    row_index: usize,
    column_index: usize,
) -> egui::Rect {
    const BUTTON_RADIUS: f32 = 8.0;
    let b_center = origin
        + egui::Vec2::new(
            (1.0 + 2.0 * column_index as f32) * BUTTON_RADIUS / ui_scale,
            (1.0 + 2.0 * row_index as f32) * BUTTON_RADIUS / ui_scale,
        );
    egui::Rect::from_center_size(b_center, egui::Vec2::splat(2.0 * BUTTON_RADIUS / ui_scale))
}
fn draw_element_button_rects(
    settings: &<ArchiMateDomain as Domain>::SettingsT,
    canvas: &mut dyn NHCanvas,
    origin: egui::Pos2,
    ui_scale: f32,
) {
    for (row_idx, col_idx, l, _f) in settings.element_buttons.iter() {
        let r = element_button_rect(origin, ui_scale, *row_idx, *col_idx);
        canvas.draw_rectangle(
            r,
            egui::CornerRadius::ZERO,
            egui::Color32::WHITE,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );
        canvas.draw_text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            l,
            14.0 / ui_scale,
            egui::Color32::BLACK,
        );
    }
}
fn handle_element_button_click(
    settings: &<ArchiMateDomain as Domain>::SettingsT,
    origin: egui::Pos2,
    ui_scale: f32,
    click_pos: egui::Pos2,
) -> Option<&ElementButtonF> {
    for (row_idx, col_idx, _l, f) in settings.element_buttons.iter() {
        let r = element_button_rect(origin, ui_scale, *row_idx, *col_idx);
        if r.contains(click_pos) {
            return Some(f);
        }
    }
    None
}

fn new_archimate_concept(
    name: &str,
    stereotype: &str,
    kind: ArchiMateConceptKind,
    position: egui::Pos2,
    render_style: ArchiMateConceptRenderStyle,
    background_color: MGlobalColor,
) -> (ERef<ArchiMateConcept>, ERef<ArchiMateConceptView>) {
    let model = ERef::new(ArchiMateConcept::new(
        ModelUuid::now_v7(),
        kind,
        stereotype.to_owned(),
        name.to_owned(),
        Vec::new(),
    ));
    let view = new_archimate_concept_view(model.clone(), position, render_style, background_color);
    (model, view)
}
fn new_archimate_concept_view(
    model: ERef<ArchiMateConcept>,
    position: egui::Pos2,
    render_style: ArchiMateConceptRenderStyle,
    background_color: MGlobalColor,
) -> ERef<ArchiMateConceptView> {
    let m = model.read();
    ERef::new(ArchiMateConceptView {
        uuid: ViewUuid::now_v7().into(),
        model: model.clone(),
        owned_views: OrderedViews::new(Vec::new()),
        all_elements: HashMap::new(),
        selected_direct_elements: HashSet::new(),

        stereotype_in_guillemets: String::new(),
        stereotype_buffer: (*m.stereotype).to_owned(),
        name_buffer: (*m.name).to_owned(),
        kind_buffer: m.kind,
        comment_buffer: (*m.comment).to_owned(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_pos(position),
        background_color,
        render_style,
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct ArchiMateConceptView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<ArchiMateConcept>,
    #[nh_context_serde(entity)]
    owned_views: OrderedViews<ArchiMateElementView>,
    #[nh_context_serde(skip_and_default)]
    all_elements: HashMap<ViewUuid, SelectionStatus>,
    #[nh_context_serde(skip_and_default)]
    selected_direct_elements: HashSet<ViewUuid>,

    #[nh_context_serde(skip_and_default)]
    stereotype_in_guillemets: String,
    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: String,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
    #[nh_context_serde(skip_and_default)]
    kind_buffer: ArchiMateConceptKind,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,

    #[nh_context_serde(skip_and_default)]
    dragged_shape: Option<NHShape>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
    background_color: MGlobalColor,
    render_style: ArchiMateConceptRenderStyle,
}

impl Entity for ArchiMateConceptView {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl View for ArchiMateConceptView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
}

impl ElementController<ArchiMateElement> for ArchiMateConceptView {
    fn model(&self) -> ArchiMateElement {
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

impl ElementControllerGen2<ArchiMateDomain> for ArchiMateConceptView {
    fn show_properties(
        &mut self,
        gdc: &GlobalDrawingContext,
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
    ) -> PropertiesStatus<ArchiMateDomain> {
        let child = self
            .owned_views
            .event_order_find_mut(|v| v.show_properties(gdc, q, ui, commands).non_default());

        if let Some(child) = child {
            return child;
        } else if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        ui.label("Model properties");

        if ui
            .labeled_text_edit_singleline("Stereotype:", &mut self.stereotype_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                ArchiMatePropChange::StereotypeChange(Arc::new(self.stereotype_buffer.clone())),
            ));
        }

        if ui
            .labeled_text_edit_multiline("Name:", &mut self.name_buffer)
            .changed()
        {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                ArchiMatePropChange::NameChange(Arc::new(self.name_buffer.clone())),
            ));
        }

        ui.label("Kind:");
        egui::ComboBox::from_id_salt("concept kind")
            .selected_text(self.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in ArchiMateConceptKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            ArchiMatePropChange::ConceptKindChange(self.kind_buffer),
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
                ArchiMatePropChange::CommentChange(Arc::new(self.comment_buffer.clone())),
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
                ArchiMatePropChange::ColorChange((0, new_color).into()),
            ));
        }

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        gdc: &GlobalDrawingContext,
        settings: &ArchiMateSettings,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveArchiMateTool)>,
    ) -> TargettingStatus {
        let name_bounds = canvas.measure_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
        );
        let mut content_bounds = name_bounds;
        if !self.stereotype_buffer.is_empty() {
            content_bounds = content_bounds.union(canvas.measure_text(
                name_bounds.center_top(),
                egui::Align2::CENTER_BOTTOM,
                &self.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
            ));
        }
        self.owned_views.draw_order_foreach_mut(|e| {
            content_bounds = content_bounds.union(e.min_shape().bounding_box())
        });
        self.bounds_rect = content_bounds.expand2((30.0, 25.0).into());

        // Draw shape and text
        let background_color = gdc
            .global_colors
            .get(&self.background_color)
            .unwrap_or_else(|| match self.kind_buffer.color_group() {
                ArchiMateConceptKindColorGroup::Common
                    if self.kind_buffer == ArchiMateConceptKind::Grouping =>
                {
                    egui::Color32::TRANSPARENT
                }
                ArchiMateConceptKindColorGroup::Common => egui::Color32::from_rgb(0xDF, 0xDA, 0xD0),
                ArchiMateConceptKindColorGroup::Motivation => {
                    egui::Color32::from_rgb(0xCC, 0xCC, 0xFF)
                }
                ArchiMateConceptKindColorGroup::Strategy => {
                    egui::Color32::from_rgb(0xF5, 0xDE, 0xAA)
                }
                ArchiMateConceptKindColorGroup::Business => {
                    egui::Color32::from_rgb(0xFF, 0xFF, 0xAE)
                }
                ArchiMateConceptKindColorGroup::Application => {
                    egui::Color32::from_rgb(0xB2, 0xFF, 0xFF)
                }
                ArchiMateConceptKindColorGroup::Technology => {
                    egui::Color32::from_rgb(0xAF, 0xFF, 0xAF)
                }
                ArchiMateConceptKindColorGroup::ImplementationAndMigration => {
                    egui::Color32::from_rgb(0xFF, 0xDF, 0xDF)
                }
            });
        let stroke = match self.kind_buffer {
            ArchiMateConceptKind::Grouping => {
                canvas::Stroke::new_dashed(1.0, egui::Color32::DARK_GRAY)
            }
            _ => canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
        };

        match self.kind_buffer.rectangle_shape_group() {
            ArchiMateConceptKindShapeGroup::Motivational => {
                const CHAMFER: f32 = 7.0;
                canvas.draw_polygon(
                    [
                        self.bounds_rect.left_top() + (CHAMFER, 0.0).into(),
                        self.bounds_rect.right_top() + (-CHAMFER, 0.0).into(),
                        self.bounds_rect.right_top() + (0.0, CHAMFER).into(),
                        self.bounds_rect.right_bottom() + (0.0, -CHAMFER).into(),
                        self.bounds_rect.right_bottom() + (-CHAMFER, 0.0).into(),
                        self.bounds_rect.left_bottom() + (CHAMFER, 0.0).into(),
                        self.bounds_rect.left_bottom() + (0.0, -CHAMFER).into(),
                        self.bounds_rect.left_top() + (0.0, CHAMFER).into(),
                    ]
                    .to_vec(),
                    background_color,
                    stroke,
                    self.highlight,
                );
            }
            ArchiMateConceptKindShapeGroup::Structural => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::CornerRadius::ZERO,
                    background_color,
                    stroke,
                    self.highlight,
                );
            }
            ArchiMateConceptKindShapeGroup::Behavioral => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::CornerRadius::same(10),
                    background_color,
                    stroke,
                    self.highlight,
                );
            }
        }
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &self.name_buffer,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            egui::Color32::BLACK,
        );
        if !self.stereotype_buffer.is_empty() {
            canvas.draw_text(
                name_bounds.center_top(),
                egui::Align2::CENTER_BOTTOM,
                &self.stereotype_in_guillemets,
                canvas::CLASS_TOP_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }

        let icon_rect = egui::Rect::from_min_size(
            egui::Pos2::new(
                self.bounds_rect.right() - 27.0,
                self.bounds_rect.top() + 7.0,
            ),
            egui::Vec2::new(20.0, 20.0),
        );
        /*
        canvas.draw_rectangle(
            icon_rect,
            egui::CornerRadius::ZERO,
            egui::Color32::TRANSPARENT,
            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
            canvas::Highlight::NONE,
        );
        */

        // draw icons based on kind
        match self.kind_buffer {
            ArchiMateConceptKind::Role | ArchiMateConceptKind::Stakeholder => {
                let body_rect = icon_rect.shrink2((5.0, 5.0).into());
                canvas.draw_ellipse(
                    icon_rect.center() + (-5.0, 0.0).into(),
                    egui::Vec2::new(3.0, 5.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    body_rect,
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::TRANSPARENT),
                    canvas::Highlight::NONE,
                );
                for e in [
                    (body_rect.left_top(), body_rect.right_top()),
                    (body_rect.left_bottom(), body_rect.right_bottom()),
                ] {
                    canvas.draw_line(
                        [e.0, e.1],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                canvas.draw_ellipse(
                    icon_rect.center() + (5.0, 0.0).into(),
                    egui::Vec2::new(3.0, 5.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Collaboration => {
                for (bg, fg) in [
                    (background_color, egui::Color32::TRANSPARENT),
                    (egui::Color32::TRANSPARENT, egui::Color32::BLACK),
                ] {
                    for offset in [-3.5, 3.5] {
                        canvas.draw_ellipse(
                            icon_rect.center() + (offset, 0.0).into(),
                            egui::Vec2::splat(6.5),
                            bg,
                            canvas::Stroke::new_solid(1.0, fg),
                            canvas::Highlight::NONE,
                        );
                    }
                }
            }
            ArchiMateConceptKind::Path => {
                let (slice_x, slice_y) = (3.0, 7.0);
                for e in [
                    icon_rect.center() + (-slice_x, 0.0).into(),
                    icon_rect.center() + (slice_x, 0.0).into(),
                ] {
                    canvas.draw_rectangle(
                        egui::Rect::from_center_size(e, (slice_x, slice_x).into()),
                        egui::CornerRadius::ZERO,
                        egui::Color32::BLACK,
                        canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                let (slice_y, refund) = (icon_rect.width() / 2.0 - slice_y, 2.0);
                for e in [
                    (
                        icon_rect.left_center(),
                        icon_rect.left_center() + (refund * slice_x, refund * slice_y).into(),
                    ),
                    (
                        icon_rect.left_center(),
                        icon_rect.left_center() + (refund * slice_x, refund * -slice_y).into(),
                    ),
                    (
                        icon_rect.right_center(),
                        icon_rect.right_center() + (refund * -slice_x, refund * -slice_y).into(),
                    ),
                    (
                        icon_rect.right_center(),
                        icon_rect.right_center() + (refund * -slice_x, refund * slice_y).into(),
                    ),
                ] {
                    canvas.draw_line(
                        [e.0, e.1],
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Process => {
                let ah_offset = 3.0;
                canvas.draw_polygon(
                    [
                        (icon_rect.center().x + ah_offset, icon_rect.top()).into(),
                        icon_rect.right_center(),
                        (icon_rect.center().x + ah_offset, icon_rect.bottom()).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let tail_width = 8.0;
                let mut tail_rect = egui::Rect::from_min_size(
                    icon_rect.left_top() + (0.0, (icon_rect.height() - tail_width) / 2.0).into(),
                    (11.0 + ah_offset, tail_width).into(),
                );
                canvas.draw_rectangle(
                    tail_rect,
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                tail_rect.max.x -= 1.0;
                for e in [
                    (tail_rect.right_bottom(), tail_rect.left_bottom()),
                    (tail_rect.left_bottom(), tail_rect.left_top()),
                    (tail_rect.left_top(), tail_rect.right_top()),
                ] {
                    canvas.draw_line(
                        [e.0, e.1],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Function => {
                let slice = icon_rect.width() / 3.0;
                let right_side = [
                    (icon_rect.center().x, icon_rect.top()).into(),
                    (icon_rect.right(), icon_rect.top() + slice).into(),
                    (icon_rect.right(), icon_rect.bottom()).into(),
                    (icon_rect.center().x, icon_rect.bottom() - slice).into(),
                ]
                .to_vec();
                let left_side = [
                    (icon_rect.center().x, icon_rect.bottom() - slice).into(),
                    (icon_rect.left(), icon_rect.bottom()).into(),
                    (icon_rect.left(), icon_rect.top() + slice).into(),
                    (icon_rect.center().x, icon_rect.top()).into(),
                ]
                .to_vec();
                canvas.draw_polygon(
                    right_side.clone(),
                    background_color,
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    right_side.clone(),
                    background_color,
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let all_pts = right_side
                    .into_iter()
                    .chain(left_side.into_iter())
                    .collect();
                canvas.draw_polygon(
                    all_pts,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Service | ArchiMateConceptKind::Event => {
                let mut pts = Vec::new();
                const SAMPLES: usize = 10;
                let radius = 5.0;
                let sc = |t0: f32, ox| {
                    (0..=SAMPLES).map(move |i| {
                        let t = i as f32 / SAMPLES as f32;
                        let theta = t0 + std::f32::consts::PI * (1.0 - t);
                        icon_rect.center()
                            + (ox, 0.0).into()
                            + (radius * theta.sin(), radius * theta.cos()).into()
                    })
                };

                pts.extend(sc(0.0, 5.0));
                match self.kind_buffer {
                    ArchiMateConceptKind::Service => pts.extend(sc(180.0_f32.to_radians(), -5.0)),
                    ArchiMateConceptKind::Event => {
                        pts.push((icon_rect.left(), icon_rect.center().y + radius).into());
                        pts.push((icon_rect.left() + 5.0, icon_rect.center().y).into());
                        pts.push((icon_rect.left(), icon_rect.center().y - radius).into());
                    }
                    _ => unreachable!(),
                }

                canvas.draw_polygon(
                    pts,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Grouping => {
                let slice = icon_rect.width() / 3.0;
                let top_rect =
                    egui::Rect::from_min_size(icon_rect.left_top(), (2.0 * slice, slice).into());
                let bottom_rect = egui::Rect::from_min_size(
                    top_rect.left_bottom(),
                    (3.0 * slice, 2.0 * slice).into(),
                );
                for e in [top_rect, bottom_rect] {
                    canvas.draw_rectangle(
                        e,
                        egui::CornerRadius::ZERO,
                        egui::Color32::TRANSPARENT,
                        stroke,
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Location => {
                let mut pts = Vec::new();
                const SAMPLES: usize = 10;
                let radius = 7.0;
                let sc = || {
                    (0..=SAMPLES).map(move |i| {
                        let t = i as f32 / SAMPLES as f32;
                        let theta = std::f32::consts::PI * (1.0 - t);
                        icon_rect.center()
                            + (0.0, -3.0).into()
                            + (radius * theta.cos(), -radius * theta.sin()).into()
                    })
                };

                pts.extend(sc());
                pts.push(icon_rect.center_bottom());

                canvas.draw_polygon(
                    pts,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Driver => {
                for (radius, bg) in [
                    (7.5, egui::Color32::BLACK),
                    (6.5, background_color),
                    (2.5, egui::Color32::BLACK),
                ] {
                    canvas.draw_ellipse(
                        icon_rect.center(),
                        egui::Vec2::splat(radius),
                        bg,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                for e in [0.0, 45.0, 90.0, 135.0]
                    .map(|e| (e as f32).to_radians())
                    .map(|e| egui::Vec2::new(e.cos(), e.sin()))
                {
                    canvas.draw_line(
                        [icon_rect.center() + e * 10.0, icon_rect.center() - e * 10.0],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Assessment => {
                canvas.draw_line(
                    [
                        icon_rect.left_bottom() + (3.0, -3.0).into(),
                        icon_rect.center(),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    icon_rect.center() + (3.0, -3.0).into(),
                    egui::Vec2::new(6.0, 6.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Goal => {
                for (radius, bg) in [
                    (9.5, background_color),
                    (7.0, background_color),
                    (4.0, egui::Color32::BLACK),
                ] {
                    canvas.draw_ellipse(
                        icon_rect.center(),
                        egui::Vec2::splat(radius),
                        bg,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Outcome => {
                for (radius, bg) in [
                    (8.0, background_color),
                    (5.0, background_color),
                    (2.0, background_color),
                ] {
                    canvas.draw_ellipse(
                        icon_rect.center(),
                        egui::Vec2::splat(radius),
                        bg,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                for p in [(icon_rect.center(), icon_rect.right_top())]
                    .into_iter()
                    .chain(
                        [(0.0, -5.0), (5.0, 0.0)]
                            .map(|e| (icon_rect.center() + e.into(), icon_rect.center())),
                    )
                {
                    canvas.draw_line(
                        [p.0, p.1],
                        canvas::Stroke::new_solid(3.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Principle => {
                canvas.draw_rectangle(
                    icon_rect.shrink2((3.5, 3.5).into()),
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "!",
                    canvas::CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
            ArchiMateConceptKind::Requirement => {
                let tilt = 3.0;
                let box_rect = icon_rect.shrink2((2.0, 5.0).into());
                canvas.draw_polygon(
                    [
                        box_rect.left_top() + (tilt, 0.0).into(),
                        box_rect.right_top(),
                        box_rect.right_bottom() + (-tilt, 0.0).into(),
                        box_rect.left_bottom(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Meaning => {
                canvas.draw_ellipse(
                    icon_rect.center() + (2.0, -5.0).into(),
                    egui::Vec2::new(8.0, 5.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    icon_rect.center() + (-3.5, 1.5).into(),
                    egui::Vec2::new(3.0, 2.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    icon_rect.center() + (-5.0, 5.0).into(),
                    egui::Vec2::new(1.0, 1.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Value => {
                canvas.draw_ellipse(
                    icon_rect.center(),
                    egui::Vec2::new(10.0, 6.0),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Resource => {
                let box_rect = icon_rect.shrink2((3.0, 6.0).into());
                canvas.draw_rectangle(
                    box_rect,
                    egui::CornerRadius::same(5),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    egui::Rect::from_min_size(
                        (box_rect.right(), box_rect.top() + 2.0).into(),
                        (3.0, 4.0).into(),
                    ),
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                for e in 0..=2 {
                    let x = box_rect.left() + 3.0 + e as f32 * 2.0;
                    canvas.draw_line(
                        [
                            (x, box_rect.top() + 2.0).into(),
                            (x, box_rect.bottom() - 2.0).into(),
                        ],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Capability => {
                let side_len = icon_rect.width() / 3.0;
                for (x, y) in (0..=2)
                    .into_iter()
                    .flat_map(|x| (0..=2).into_iter().map(move |e| (x as f32, e as f32)))
                    .filter(|(x, y)| *y >= 2.0 - *x)
                {
                    canvas.draw_rectangle(
                        egui::Rect::from_min_size(
                            icon_rect.left_top() + (x * side_len, y * side_len).into(),
                            (side_len, side_len).into(),
                        ),
                        egui::CornerRadius::ZERO,
                        background_color,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::ValueStream => {
                let slice = icon_rect.width() / 3.0;
                let bottom_side = [
                    (icon_rect.right(), icon_rect.center().y).into(),
                    (icon_rect.right() - slice, icon_rect.bottom()).into(),
                    (icon_rect.left(), icon_rect.bottom()).into(),
                    (icon_rect.left() + slice, icon_rect.center().y).into(),
                ]
                .to_vec();
                let top_side = [
                    (icon_rect.left() + slice, icon_rect.center().y).into(),
                    (icon_rect.left(), icon_rect.top()).into(),
                    (icon_rect.right() - slice, icon_rect.top()).into(),
                    (icon_rect.right(), icon_rect.center().y).into(),
                ]
                .to_vec();
                canvas.draw_polygon(
                    bottom_side.clone(),
                    background_color,
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    top_side.clone(),
                    background_color,
                    canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let all_pts = bottom_side
                    .into_iter()
                    .chain(top_side.into_iter())
                    .collect();
                canvas.draw_polygon(
                    all_pts,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::CourseOfAction => {
                for (bg, radius) in [
                    (background_color, 5.0),
                    (background_color, 3.0),
                    (egui::Color32::BLACK, 1.0),
                ] {
                    canvas.draw_ellipse(
                        icon_rect.center() + (5.0, -5.0).into(),
                        egui::Vec2::splat(radius),
                        bg,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                let p1 = egui::Pos2::new(icon_rect.left() + 5.0, icon_rect.bottom());
                let p2 = egui::Pos2::new(icon_rect.left() + 5.0, icon_rect.bottom() - 5.0);
                let p3 = icon_rect.center() + (1.0, -1.0).into();
                for e in [(p1, p2), (p2, p3)].into_iter().chain(
                    [(0.0, 5.0), (-5.0, 0.0)]
                        .into_iter()
                        .map(|e| (p3 + e.into(), p3)),
                ) {
                    canvas.draw_line(
                        [e.0, e.1],
                        canvas::Stroke::new_solid(3.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::BusinessActor => {
                let head_radius = 4.0;
                canvas.draw_ellipse(
                    (icon_rect.center().x, icon_rect.top() + head_radius).into(),
                    egui::Vec2::splat(head_radius),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let right_hand = egui::Pos2::new(
                    icon_rect.center().x + head_radius,
                    icon_rect.center().y + 2.0,
                );
                let left_hand = egui::Pos2::new(
                    icon_rect.center().x - head_radius,
                    icon_rect.center().y + 2.0,
                );
                let pelvis =
                    egui::Pos2::new(icon_rect.center().x, icon_rect.top() + 3.5 * head_radius);
                for (p1, p2) in [
                    (
                        (icon_rect.center().x, icon_rect.top() + 2.0 * head_radius).into(),
                        pelvis,
                    ),
                    (right_hand, left_hand),
                    (pelvis, (right_hand.x, icon_rect.bottom()).into()),
                    (pelvis, (left_hand.x, icon_rect.bottom()).into()),
                ] {
                    canvas.draw_line(
                        [p1, p2],
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::BusinessInterface
            | ArchiMateConceptKind::ApplicationInterface
            | ArchiMateConceptKind::TechnologyInterface => {
                canvas.draw_line(
                    [icon_rect.left_center(), icon_rect.center()],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_ellipse(
                    icon_rect.center() + (3.5, 0.0).into(),
                    egui::Vec2::new(6.5, 6.5),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::BusinessObject | ArchiMateConceptKind::DataObject => {
                let strip_height = 4.0;
                let box_rect = icon_rect.shrink2((0.0, 4.0).into());
                canvas.draw_rectangle(
                    box_rect,
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_line(
                    [
                        (box_rect.min.x, box_rect.top() + strip_height).into(),
                        (box_rect.max.x, box_rect.top() + strip_height).into(),
                    ],
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Product => {
                let strip_height = 4.0;
                let box_rect = icon_rect.shrink2((0.0, 4.0).into());
                canvas.draw_rectangle(
                    box_rect,
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_rectangle(
                    egui::Rect::from_min_size(box_rect.left_top(), (10.0, strip_height).into()),
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::ApplicationComponent => {
                let overhang = 6.0;
                let box_rect = icon_rect.with_min_x(icon_rect.min.x + overhang);
                canvas.draw_rectangle(
                    box_rect,
                    egui::CornerRadius::ZERO,
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                for e in [-4.0, 4.0] {
                    canvas.draw_rectangle(
                        egui::Rect::from_center_size(
                            (box_rect.left(), box_rect.center().y + e).into(),
                            (2.0 * overhang, 3.0).into(),
                        ),
                        egui::CornerRadius::ZERO,
                        background_color,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Node => {
                let slice = icon_rect.width() / 3.0;
                canvas.draw_polygon(
                    [
                        icon_rect.left_top() + (0.0, slice).into(),
                        icon_rect.right_top() + (-slice, slice).into(),
                        icon_rect.right_bottom() + (-slice, 0.0).into(),
                        icon_rect.left_bottom(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        icon_rect.left_top() + (slice, 0.0).into(),
                        icon_rect.right_top(),
                        icon_rect.right_top() + (-slice, slice).into(),
                        icon_rect.left_top() + (0.0, slice).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        icon_rect.right_top(),
                        icon_rect.right_bottom() + (0.0, -slice).into(),
                        icon_rect.right_bottom() + (-slice, 0.0).into(),
                        icon_rect.right_top() + (-slice, slice).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Device => {
                let slice = icon_rect.width() / 5.0;
                let slice2 = slice / 4.0;
                canvas.draw_polygon(
                    [
                        icon_rect.left_top() + (slice, 0.0).into(),
                        icon_rect.right_top() + (-slice, 0.0).into(),
                        icon_rect.right_top() + (-slice2, slice2).into(),
                        icon_rect.right_top() + (0.0, slice).into(),
                        icon_rect.right_bottom() + (0.0, -2.0 * slice).into(),
                        icon_rect.right_bottom() + (-slice2, -slice - slice2).into(),
                        icon_rect.right_bottom() + (-slice, -slice).into(),
                        icon_rect.left_bottom() + (slice, -slice).into(),
                        icon_rect.left_bottom() + (slice2, -slice - slice2).into(),
                        icon_rect.left_bottom() + (0.0, -2.0 * slice).into(),
                        icon_rect.left_top() + (0.0, slice).into(),
                        icon_rect.left_top() + (slice2, slice2).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        icon_rect.right_bottom() + (-slice, -slice).into(),
                        icon_rect.right_bottom(),
                        icon_rect.left_bottom(),
                        icon_rect.left_bottom() + (slice, -slice).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::SystemSoftware => {
                for offset in [2.0, -2.0] {
                    canvas.draw_ellipse(
                        icon_rect.center() + (offset, -offset).into(),
                        egui::Vec2::splat(8.0),
                        background_color,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Equipment => {
                for (offset, radii) in [((-3.0, 3.0), [7.0, 5.0]), ((5.0, -5.0), [5.0, 3.0])] {
                    for radius in radii {
                        canvas.draw_ellipse(
                            icon_rect.center() + offset.into(),
                            egui::Vec2::splat(radius),
                            background_color,
                            canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                            canvas::Highlight::NONE,
                        );
                    }
                }
            }
            ArchiMateConceptKind::Facility => {
                let slice = icon_rect.width() / 4.0;
                canvas.draw_polygon(
                    [
                        icon_rect.left_top(),
                        (icon_rect.left() + slice, icon_rect.top()).into(),
                        (icon_rect.left() + slice, icon_rect.center().y + slice).into(),
                        icon_rect.center(),
                        icon_rect.center() + (0.0, slice).into(),
                        icon_rect.center() + (slice, 0.0).into(),
                        icon_rect.center() + (slice, slice).into(),
                        (icon_rect.right(), icon_rect.center().y).into(),
                        icon_rect.right_bottom(),
                        icon_rect.left_bottom(),
                    ]
                    .to_vec(),
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::CommunicationNetwork => {
                let tilt = 3.0;
                let box_rect = icon_rect.shrink2((3.0, 5.0).into());
                let circuit_pts = [
                    box_rect.left_bottom(),
                    box_rect.left_top() + (tilt, 0.0).into(),
                    box_rect.right_top(),
                    box_rect.right_bottom() + (-tilt, 0.0).into(),
                    box_rect.left_bottom(),
                ];
                for p in circuit_pts.iter().take(4) {
                    canvas.draw_ellipse(
                        *p,
                        egui::Vec2::splat(2.0),
                        egui::Color32::BLACK,
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                for e in circuit_pts.array_windows::<2>() {
                    canvas.draw_line(
                        *e,
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::DistributionNetwork => {
                let (slice_x, slice_y) = (3.0, 7.0);
                canvas.draw_polygon(
                    [
                        icon_rect.right_top() + (-slice_x, slice_y).into(),
                        icon_rect.right_center(),
                        icon_rect.right_bottom() + (-slice_x, -slice_y).into(),
                        icon_rect.left_bottom() + (slice_x, -slice_y).into(),
                        icon_rect.left_center(),
                        icon_rect.left_top() + (slice_x, slice_y).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let (slice_y, refund) = (icon_rect.width() / 2.0 - slice_y, 2.0);
                for e in [
                    (
                        icon_rect.left_center(),
                        icon_rect.left_center() + (refund * slice_x, refund * slice_y).into(),
                    ),
                    (
                        icon_rect.left_center(),
                        icon_rect.left_center() + (refund * slice_x, refund * -slice_y).into(),
                    ),
                    (
                        icon_rect.right_center(),
                        icon_rect.right_center() + (refund * -slice_x, refund * -slice_y).into(),
                    ),
                    (
                        icon_rect.right_center(),
                        icon_rect.right_center() + (refund * -slice_x, refund * slice_y).into(),
                    ),
                ] {
                    canvas.draw_line(
                        [e.0, e.1],
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Artifact => {
                let chamfer = 6.0;
                let box_rect = icon_rect.shrink2((3.0, 0.0).into());
                canvas.draw_polygon(
                    [
                        box_rect.left_top(),
                        box_rect.right_top() + (-chamfer, 0.0).into(),
                        box_rect.right_top() + (0.0, chamfer).into(),
                        box_rect.right_bottom(),
                        box_rect.left_bottom(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_polygon(
                    [
                        box_rect.right_top() + (-chamfer, 0.0).into(),
                        box_rect.right_top() + (0.0, chamfer).into(),
                        box_rect.right_top() + (-chamfer, chamfer).into(),
                    ]
                    .to_vec(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Material => {
                let hexagon = |radius: f32| {
                    (0..=6).map(move |i| {
                        let theta = (i as f32 * 60.0).to_radians();
                        icon_rect.center() + (radius * theta.cos(), -radius * theta.sin()).into()
                    })
                };
                canvas.draw_polygon(
                    hexagon(9.0).collect(),
                    background_color,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                let mut iter = hexagon(5.0);
                while let Some(p1) = iter.next()
                    && let Some(p2) = iter.next()
                {
                    canvas.draw_line(
                        [p1, p2],
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::WorkPackage => {
                const SAMPLES: usize = 15;
                let radius = 7.0;
                let mut iter = (0..=SAMPLES)
                    .map(move |i| {
                        let t = i as f32 / SAMPLES as f32;
                        let theta = 270.0_f32.to_radians() * (1.0 - t);
                        icon_rect.center() + (radius * theta.cos(), -radius * theta.sin()).into()
                    })
                    .peekable();
                while let Some(p1) = iter.next() {
                    let Some(p2) = iter.peek() else {
                        break;
                    };
                    canvas.draw_line(
                        [p1, *p2],
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
                let arrow = 4.0;
                let afp = icon_rect.center() + (radius, radius).into();
                for (p1, p2) in [
                    (icon_rect.center() + (0.0, radius).into(), afp),
                    (afp, afp + (-arrow, -arrow).into()),
                    (afp, afp + (-arrow, arrow).into()),
                ] {
                    canvas.draw_line(
                        [p1, p2],
                        canvas::Stroke::new_solid(2.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
            ArchiMateConceptKind::Deliverable => {
                let box_rect = icon_rect.shrink2((0.0, 4.0).into());
                let mut pts = Vec::new();
                const SAMPLES: usize = 10;
                let amplitude = box_rect.height() / 4.0;
                let sy = || {
                    (0..=SAMPLES).map(move |i| {
                        let t = i as f32 / SAMPLES as f32;
                        let theta = 2.0 * std::f32::consts::PI * t;
                        egui::Pos2::new(
                            box_rect.right() - t * box_rect.width(),
                            box_rect.center().y + amplitude - amplitude * theta.sin(),
                        )
                    })
                };

                pts.push(box_rect.left_top());
                pts.push(box_rect.right_top());
                pts.extend(sy());

                canvas.draw_polygon(
                    pts,
                    egui::Color32::TRANSPARENT,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            ArchiMateConceptKind::Plateau => {
                let slice = icon_rect.width() / 4.0;
                let size = egui::Vec2::new(2.0 * slice, slice);
                for e in [
                    egui::Rect::from_min_size(icon_rect.center_top(), size),
                    egui::Rect::from_center_size(icon_rect.center(), size),
                    egui::Rect::from_min_size(icon_rect.left_bottom() + (0.0, -slice).into(), size),
                ] {
                    canvas.draw_rectangle(
                        e,
                        egui::CornerRadius::ZERO,
                        egui::Color32::BLACK,
                        canvas::Stroke::new_solid(0.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                }
            }
        }

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;
        self.owned_views.draw_order_foreach_mut(|v| {
            if v.draw_in(q, gdc, settings, canvas, tool) == TargettingStatus::Drawn {
                drawn_child_targetting = TargettingStatus::Drawn;
            }
        });

        // Draw buttons
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            draw_element_button_rects(settings, canvas, self.bounds_rect.right_top(), ui_scale);
        }

        // Draw targetting rectangle
        if canvas.ui_scale().is_some()
            && drawn_child_targetting != TargettingStatus::Drawn
            && let Some(t) = tool
                .as_ref()
                .filter(|e| self.min_shape().contains(e.0))
                .map(|e| e.1)
        {
            canvas.draw_rectangle(
                self.bounds_rect,
                egui::CornerRadius::ZERO,
                t.targetting_for_section(Ok(self.model())),
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            TargettingStatus::Drawn
        } else {
            drawn_child_targetting
        }
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid, self.min_shape());

        self.owned_views
            .event_order_foreach_mut(|v| v.collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        settings: &<ArchiMateDomain as Domain>::SettingsT,
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        tool: &mut Option<NaiveArchiMateTool>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<
            InsensitiveCommand<
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
    ) -> EventHandlingStatus {
        let k_status = self.owned_views.event_order_find_mut(|v| {
            let s = v.handle_event(event, ehc, settings, q, tool, element_setup_modal, commands);
            if s != EventHandlingStatus::NotHandled {
                Some((*v.uuid(), s))
            } else {
                None
            }
        });

        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if k_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
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
                    && let Some(f) = handle_element_button_click(
                        settings,
                        self.bounds_rect.right_top(),
                        ehc.ui_scale,
                        pos,
                    ) =>
            {
                let (initial_stage, current_stage, result, event_lock) =
                    f(self.model.clone().into());
                *tool = Some(NaiveArchiMateTool {
                    uuid: uuid::Uuid::nil(),
                    initial_stage,
                    current_stage,
                    result,
                    event_lock,
                    is_spent: Some(false),
                });
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::Click(pos) => {
                if !self.bounds_rect.contains(pos) {
                    return k_status
                        .map(|e| e.1)
                        .unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.model.clone().into());

                    if !tool.references(&self.model.read().uuid)
                        && let Ok(esm) = tool.try_flush(q, &self.uuid, 0, None, commands)
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
                            commands.push(InsensitiveCommand::HighlightAll(
                                false,
                                canvas::Highlight::SELECTED,
                            ));
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                canvas::Highlight::SELECTED,
                            ));
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.selected_direct_elements.contains(&k),
                                canvas::Highlight::SELECTED,
                            ));
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
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
        diagram_model: &ERef<ArchiMateDiagram>,
        command: &InsensitiveCommand<
            ArchiMateOrdinalMovement,
            ArchiMateElementOrVertex,
            ArchiMatePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            () => {
                self.owned_views.event_order_foreach_mut(|v| {
                    v.apply_command(diagram_model, command, undo_accumulator, affected_models)
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
            InsensitiveCommand::MovePositional(uuids, _) if !uuids.contains(&*self.uuid) => {
                recurse!();
            }
            InsensitiveCommand::MovePositional(_, delta)
            | InsensitiveCommand::MovePositionalAll(delta) => {
                self.position += *delta;
                undo_accumulator.push(InsensitiveCommand::MovePositional(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                let mut void = vec![];
                self.owned_views.event_order_foreach_mut(|v| {
                    v.apply_command(
                        diagram_model,
                        &InsensitiveCommand::MovePositionalAll(*delta),
                        &mut void,
                        affected_models,
                    );
                });
            }
            InsensitiveCommand::ResizeElementsBy(..) | InsensitiveCommand::ResizeElementTo(..) => {}
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for (_uuid, element) in self
                    .owned_views
                    .iter_event_order_pairs()
                    .filter(|e| uuids.contains(&e.0))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
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
                        element: element.clone().into(),
                        into_model: false,
                    });
                }

                self.owned_views.retain(|k, _v| !uuids.contains(k));

                recurse!();
            }
            InsensitiveCommand::AddDependency {
                target,
                bucket,
                position,
                element,
                into_model,
            } => {
                let model_uuid = *self.model.read().uuid;
                if *target == *self.uuid
                    && *bucket == 0
                    && let ArchiMateElementOrVertex::Element(mut view) = element.clone()
                    && (!*into_model
                        || diagram_model
                            .write()
                            .insert_element_into(model_uuid, 0, *position, view.model())
                            .is_ok())
                {
                    let uuid = *view.uuid();
                    undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                        target: *self.uuid,
                        bucket: *bucket,
                        element: uuid,
                        including_model: *into_model,
                    });

                    if *into_model {
                        affected_models.insert(model_uuid);
                    }
                    let mut model_transitives = HashMap::new();
                    view.head_count(
                        &mut HashMap::new(),
                        &mut HashMap::new(),
                        &mut model_transitives,
                    );
                    affected_models.extend(model_transitives.into_keys());

                    self.owned_views.push(uuid, view);
                }

                recurse!();
            }
            InsensitiveCommand::RemoveDependency {
                target,
                bucket,
                element,
                including_model,
            } => {
                let model_uuid = *self.model.read().uuid;
                if *target == *self.uuid
                    && *bucket == 0
                    && let Some(view) = self.owned_views.get(element)
                    && let Some((_, pos)) = diagram_model
                        .write()
                        .remove_element_from(model_uuid, &view.model_uuid())
                {
                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid,
                        bucket: *bucket,
                        position: Some(pos),
                        element: view.clone().into(),
                        into_model: *including_model,
                    });

                    if *including_model {
                        affected_models.insert(model_uuid);
                    }

                    self.owned_views.retain(|k, _v| *k != *element);
                }
                recurse!();
            }
            InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model.read().uuid);
                    let mut model = self.model.write();
                    match property {
                        ArchiMatePropChange::StereotypeChange(stereotype) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                ArchiMatePropChange::StereotypeChange(model.stereotype.clone()),
                            ));
                            model.stereotype = stereotype.clone();
                        }
                        ArchiMatePropChange::NameChange(name) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                ArchiMatePropChange::NameChange(model.name.clone()),
                            ));
                            model.name = name.clone();
                        }
                        ArchiMatePropChange::ConceptKindChange(kind) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                ArchiMatePropChange::ConceptKindChange(model.kind),
                            ));
                            model.kind = *kind;
                        }
                        ArchiMatePropChange::CommentChange(comment) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                ArchiMatePropChange::CommentChange(model.comment.clone()),
                            ));
                            model.comment = comment.clone();
                        }
                        ArchiMatePropChange::ColorChange(ColorChangeData { slot: 0, color }) => {
                            undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                std::iter::once(*self.uuid).collect(),
                                ArchiMatePropChange::ColorChange(ColorChangeData {
                                    slot: 0,
                                    color: self.background_color,
                                }),
                            ));
                            self.background_color = *color;
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
        self.stereotype_in_guillemets = if !model.stereotype.is_empty() {
            format!("«{}»", model.stereotype)
        } else {
            String::new()
        };
        self.stereotype_buffer = (*model.stereotype).clone();
        self.name_buffer = (*model.name).clone();
        self.kind_buffer = model.kind;
        self.comment_buffer = (*model.comment).clone();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (ArchiMateElementView, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid(), self.highlight.selected.into());
        flattened_represented_models.insert(*self.model_uuid(), *self.uuid);

        self.all_elements.clear();
        self.owned_views.event_order_foreach_mut(|v| {
            v.head_count(
                flattened_views,
                &mut self.all_elements,
                flattened_represented_models,
            )
        });
        for e in &self.all_elements {
            flattened_views_status.insert(
                *e.0,
                match *e.1 {
                    SelectionStatus::NotSelected if self.highlight.selected => {
                        SelectionStatus::TransitivelySelected
                    }
                    e => e,
                },
            );
        }

        self.owned_views.event_order_foreach_mut(|v| {
            flattened_views.insert(*v.uuid(), (v.clone(), *self.uuid));
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, ArchiMateElementView>,
        c: &mut HashMap<ViewUuid, ArchiMateElementView>,
        m: &mut HashMap<ModelUuid, ArchiMateElement>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.owned_views
                .event_order_foreach(|v| v.deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, ArchiMateElementView>,
        c: &mut HashMap<ViewUuid, ArchiMateElementView>,
        m: &mut HashMap<ModelUuid, ArchiMateElement>,
    ) {
        let old_model = self.model.read();

        let (view_uuid, model_uuid) = if uuid_present(&self.uuid) {
            (ViewUuid::now_v7(), ModelUuid::now_v7())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(ArchiMateElement::Concept(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(model_uuid, m)
        };
        let mut inner = HashMap::new();
        self.owned_views
            .event_order_foreach(|v| v.deep_copy_clone(uuid_present, &mut inner, c, m));
        let owned_views = OrderedViews::new(
            self.owned_views
                .iter_event_order_keys()
                .flat_map(|e| inner.get(&e).cloned())
                .collect(),
        );

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            owned_views,
            all_elements: HashMap::new(),
            selected_direct_elements: HashSet::new(),
            stereotype_in_guillemets: self.stereotype_in_guillemets.clone(),
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            kind_buffer: self.kind_buffer,
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
            background_color: self.background_color,
            render_style: self.render_style,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

fn new_archimate_relationship(
    kind: ArchiMateRelationshipKind,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<ArchiMateConcept>, ArchiMateElementView),
    target: (ERef<ArchiMateConcept>, ArchiMateElementView),
) -> (ERef<ArchiMateRelationship>, ERef<RelationshipViewT>) {
    let model = ERef::new(ArchiMateRelationship::new(
        ModelUuid::now_v7(),
        kind,
        vec![ArchiMateRelationshipEnding {
            concept: source.0,
            multiplicity: Arc::new("".to_owned()),
        }],
        vec![ArchiMateRelationshipEnding {
            concept: target.0,
            multiplicity: Arc::new("".to_owned()),
        }],
    ));
    let view = new_archimate_relationship_view(
        model.clone(),
        center_point,
        vec![source.1],
        vec![target.1],
    );

    (model, view)
}
fn new_archimate_relationship_view(
    model: ERef<ArchiMateRelationship>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    sources: Vec<ArchiMateElementView>,
    targets: Vec<ArchiMateElementView>,
) -> ERef<RelationshipViewT> {
    let m = model.read();

    let (sp, mp, tp) = multiconnection_view::init_points(
        m.sources.iter().map(|e| *e.concept.read().uuid),
        *m.targets[0].concept.read().uuid,
        targets[0].min_shape(),
        center_point,
    );

    MulticonnectionView::new(
        ViewUuid::now_v7().into(),
        ArchiMateRelationshipAdapter {
            model: model.clone(),
            temporaries: Default::default(),
        },
        sources
            .into_iter()
            .zip(sp)
            .map(|e| Ending::new_p(e.0, e.1))
            .collect(),
        targets
            .into_iter()
            .zip(tp)
            .map(|e| Ending::new_p(e.0, e.1))
            .collect(),
        mp,
    )
}

#[derive(
    Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize,
)]
pub struct ArchiMateRelationshipAdapter {
    #[nh_context_serde(entity)]
    model: ERef<ArchiMateRelationship>,
    #[serde(skip_serializing)]
    #[nh_context_serde(skip_and_default)]
    temporaries: ArchiMateRelationshipAdapterTemporaries,
}

#[derive(Clone, Default)]
struct ArchiMateRelationshipAdapterTemporaries {
    arrow_data: HashMap<(bool, ModelUuid), ArrowData>,
    source_uuids: Vec<ModelUuid>,
    target_uuids: Vec<ModelUuid>,

    kind_buffer: ArchiMateRelationshipKind,
    junction_kind_buffer: ArchiMateJunctionKind,
    source_multiplicities_buffers: Vec<(ModelUuid, String)>,
    target_multiplicities_buffers: Vec<(ModelUuid, String)>,
    is_junction: bool,
}

impl MulticonnectionAdapter<ArchiMateDomain> for ArchiMateRelationshipAdapter {
    fn model(&self) -> ArchiMateElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn draw_center_or_get_label(
        &self,
        sources: &Vec<Ending<ArchiMateElementView>>,
        targets: &Vec<Ending<ArchiMateElementView>>,
        center: egui::Pos2,
        highlight: canvas::Highlight,
        _q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        _settings: &<ArchiMateDomain as Domain>::SettingsT,
        canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &<ArchiMateDomain as Domain>::ToolT)>,
    ) -> Result<(), Arc<String>> {
        if self.temporaries.is_junction {
            let (bg, prox) = match self.temporaries.junction_kind_buffer {
                ArchiMateJunctionKind::AndJunction => (egui::Color32::BLACK, egui::Color32::WHITE),
                ArchiMateJunctionKind::OrJunction => (egui::Color32::WHITE, egui::Color32::BLACK),
            };

            let radius = egui::Vec2::splat(10.0);
            canvas.draw_ellipse(
                center,
                radius,
                bg,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                highlight,
            );
            let shape = NHShape::Ellipse {
                position: center,
                bounds_radius: radius,
            };
            let c = (egui::Color32::BLACK, egui::Color32::WHITE);

            let ah_from_sources = self
                .temporaries
                .arrow_data
                .iter()
                .find(|e| e.0.0)
                .unwrap()
                .1
                .without_text();
            for e in sources {
                let p = e.points.last().unwrap().1;
                let fp = shape.center_intersect(p);
                ah_from_sources
                    .arrowhead_type
                    .draw_in(canvas, fp, p, c, highlight);
            }
            let ah_from_targets = self
                .temporaries
                .arrow_data
                .iter()
                .find(|e| !e.0.0)
                .unwrap()
                .1
                .without_text();
            for e in targets {
                let p = e.points.last().unwrap().1;
                let fp = shape.center_intersect(p);
                ah_from_targets
                    .arrowhead_type
                    .draw_in(canvas, fp, p, c, highlight);
            }

            canvas.draw_ellipse_proximity(
                center,
                egui::Vec2::new(1.0, 1.0),
                prox,
                canvas::Stroke::new_solid(1.0, prox),
                canvas::MULTICONNECTION_HANDLE_PROXIMITY,
                canvas::Highlight::NONE,
            );
        }
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
        gdc: &GlobalDrawingContext,
        q: &<ArchiMateDomain as Domain>::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<
            InsensitiveCommand<
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
    ) -> PropertiesStatus<ArchiMateDomain> {
        ui.label("Kind:");
        egui::ComboBox::from_id_salt("relationship kind")
            .selected_text(self.temporaries.kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in ArchiMateRelationshipKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            ArchiMatePropChange::RelationshipKindChange(
                                self.temporaries.kind_buffer,
                            ),
                        ));
                    }
                }
            });

        let mut junction_kind_label = egui::RichText::new("Junction kind:");
        if !self.temporaries.is_junction {
            junction_kind_label = junction_kind_label.strikethrough();
        }
        ui.label(junction_kind_label);
        egui::ComboBox::from_id_salt("junction kind")
            .selected_text(self.temporaries.junction_kind_buffer.as_str())
            .show_ui(ui, |ui| {
                for e in ArchiMateJunctionKind::VARIANTS {
                    if ui
                        .selectable_value(&mut self.temporaries.junction_kind_buffer, e, e.as_str())
                        .changed()
                    {
                        commands.push(InsensitiveCommand::PropertyChange(
                            q.selected_views(),
                            ArchiMatePropChange::RelationshipJunctionKindChange(
                                self.temporaries.junction_kind_buffer,
                            ),
                        ));
                    }
                }
            });

        ui.label("Source multiplicities:");
        for e in self.temporaries.source_multiplicities_buffers.iter_mut() {
            if ui
                .labeled_text_edit_singleline(&*gdc.model_labels.get(&e.0), &mut e.1)
                .changed()
            {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    ArchiMatePropChange::RelationshipMultiplicityChange(
                        false,
                        e.0,
                        e.1.clone().into(),
                    ),
                ));
            }
        }

        ui.label("Targets multiplicities:");
        for e in self.temporaries.target_multiplicities_buffers.iter_mut() {
            if ui
                .labeled_text_edit_singleline(&*gdc.model_labels.get(&e.0), &mut e.1)
                .changed()
            {
                commands.push(InsensitiveCommand::PropertyChange(
                    q.selected_views(),
                    ArchiMatePropChange::RelationshipMultiplicityChange(
                        true,
                        e.0,
                        e.1.clone().into(),
                    ),
                ));
            }
        }

        if ui.button("Add source").clicked() {
            return PropertiesStatus::ToolRequest(Some(NaiveArchiMateTool {
                uuid: uuid::Uuid::nil(),
                initial_stage: ArchiMateToolStage::RelationshipAddEnding { source: true },
                current_stage: ArchiMateToolStage::RelationshipAddEnding { source: true },
                result: PartialArchiMateElement::RelationshipEnding {
                    relationship_model: self.model.clone().into(),
                    new_model: None,
                },
                event_lock: false,
                is_spent: Some(false),
            }));
        }
        if ui.button("Add target").clicked() {
            return PropertiesStatus::ToolRequest(Some(NaiveArchiMateTool {
                uuid: uuid::Uuid::nil(),
                initial_stage: ArchiMateToolStage::RelationshipAddEnding { source: false },
                current_stage: ArchiMateToolStage::RelationshipAddEnding { source: false },
                result: PartialArchiMateElement::RelationshipEnding {
                    relationship_model: self.model.clone().into(),
                    new_model: None,
                },
                event_lock: false,
                is_spent: Some(false),
            }));
        }

        if ui.button("Switch source and destination").clicked() {
            commands.push(InsensitiveCommand::PropertyChange(
                q.selected_views(),
                ArchiMatePropChange::FlipMulticonnection(FlipMulticonnection {}),
            ));
        }

        PropertiesStatus::Shown
    }
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<
            ArchiMateOrdinalMovement,
            ArchiMateElementOrVertex,
            ArchiMatePropChange,
        >,
        undo_accumulator: &mut Vec<
            InsensitiveCommand<
                ArchiMateOrdinalMovement,
                ArchiMateElementOrVertex,
                ArchiMatePropChange,
            >,
        >,
    ) {
        if let InsensitiveCommand::PropertyChange(_, property) = command {
            let mut model = self.model.write();
            match property {
                ArchiMatePropChange::RelationshipKindChange(kind) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        ArchiMatePropChange::RelationshipKindChange(model.kind),
                    ));
                    model.kind = *kind;
                }
                ArchiMatePropChange::RelationshipJunctionKindChange(kind) => {
                    undo_accumulator.push(InsensitiveCommand::PropertyChange(
                        std::iter::once(*view_uuid).collect(),
                        ArchiMatePropChange::RelationshipJunctionKindChange(model.junction_kind),
                    ));
                    model.junction_kind = *kind;
                }
                ArchiMatePropChange::RelationshipMultiplicityChange(b, m, multiplicity) => {
                    if !b
                        && let Some(e) = model
                            .sources
                            .iter_mut()
                            .find(|e| *e.concept.read().uuid == *m)
                    {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            ArchiMatePropChange::RelationshipMultiplicityChange(
                                *b,
                                *m,
                                e.multiplicity.clone(),
                            ),
                        ));
                        e.multiplicity = multiplicity.clone();
                    } else if *b
                        && let Some(e) = model
                            .targets
                            .iter_mut()
                            .find(|e| *e.concept.read().uuid == *m)
                    {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            ArchiMatePropChange::RelationshipMultiplicityChange(
                                *b,
                                *m,
                                e.multiplicity.clone(),
                            ),
                        ));
                        e.multiplicity = multiplicity.clone();
                    }
                }
                _ => {}
            }
        }
    }
    fn refresh_buffers(
        &mut self,
        _sources: &Vec<Ending<ArchiMateElementView>>,
        _targets: &Vec<Ending<ArchiMateElementView>>,
    ) {
        let model = self.model.read();

        self.temporaries.arrow_data.clear();
        self.temporaries.source_uuids.clear();
        self.temporaries.target_uuids.clear();

        let (lt, sah, tah) = match model.kind {
            ArchiMateRelationshipKind::Composition => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::FullRhombus,
                canvas::ArrowheadType::None,
            ),
            ArchiMateRelationshipKind::Aggregation => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::EmptyRhombus,
                canvas::ArrowheadType::None,
            ),
            ArchiMateRelationshipKind::Assignment => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::FullCircleSmall,
                canvas::ArrowheadType::FullTriangleSmall,
            ),
            ArchiMateRelationshipKind::Realization => (
                canvas::LineType::Dotted,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::EmptyTriangle,
            ),
            ArchiMateRelationshipKind::Serving => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::OpenTriangle,
            ),
            ArchiMateRelationshipKind::AccessUnspecified => (
                canvas::LineType::Dotted,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::None,
            ),
            ArchiMateRelationshipKind::AccessUnidirectional => (
                canvas::LineType::Dotted,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::OpenTriangle,
            ),
            ArchiMateRelationshipKind::AccessBidirectional => (
                canvas::LineType::Dotted,
                canvas::ArrowheadType::OpenTriangle,
                canvas::ArrowheadType::OpenTriangle,
            ),
            ArchiMateRelationshipKind::Influence => (
                canvas::LineType::Dashed,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::OpenTriangle,
            ),
            ArchiMateRelationshipKind::AssociationUndirected => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::None,
            ),
            ArchiMateRelationshipKind::AssociationDirected => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::HalfOpenTriangle,
            ),
            ArchiMateRelationshipKind::Triggering => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::FullTriangleSmall,
            ),
            ArchiMateRelationshipKind::Flow => (
                canvas::LineType::Dashed,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::FullTriangleSmall,
            ),
            ArchiMateRelationshipKind::Specialization => (
                canvas::LineType::Solid,
                canvas::ArrowheadType::None,
                canvas::ArrowheadType::EmptyTriangle,
            ),
        };
        for e in &model.sources {
            let uuid = *e.concept.read().uuid;
            self.temporaries.arrow_data.insert(
                (false, uuid),
                ArrowData {
                    line_type: lt,
                    arrowhead_type: sah,
                    multiplicity: Some(e.multiplicity.clone()).filter(|e| !e.is_empty()),
                    role: None,
                    reading: None,
                },
            );
            self.temporaries.source_uuids.push(uuid);
        }
        for e in &model.targets {
            let uuid = *e.concept.read().uuid;
            self.temporaries.arrow_data.insert(
                (true, uuid),
                ArrowData {
                    line_type: lt,
                    arrowhead_type: tah,
                    multiplicity: Some(e.multiplicity.clone()).filter(|e| !e.is_empty()),
                    role: None,
                    reading: None,
                },
            );
            self.temporaries.target_uuids.push(uuid);
        }

        self.temporaries.kind_buffer = model.kind;
        self.temporaries.junction_kind_buffer = model.junction_kind;
        self.temporaries.source_multiplicities_buffers = model
            .sources
            .iter()
            .map(|e| (*e.concept.read().uuid, (*e.multiplicity).clone()))
            .collect();
        self.temporaries.target_multiplicities_buffers = model
            .targets
            .iter()
            .map(|e| (*e.concept.read().uuid, (*e.multiplicity).clone()))
            .collect();
        self.temporaries.is_junction = model.sources.len() > 1 || model.targets.len() > 1;
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, ArchiMateElement>,
    ) -> Self
    where
        Self: Sized,
    {
        let old_model = self.model.read();

        let model = if let Some(ArchiMateElement::Relationship(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            old_model.deep_copy_clone_inner(new_uuid, m)
        };

        Self {
            model,
            temporaries: self.temporaries.clone(),
        }
    }

    fn deep_copy_finish(&mut self, m: &HashMap<ModelUuid, ArchiMateElement>) {
        self.model.write().deep_copy_relink(m);
    }
}
