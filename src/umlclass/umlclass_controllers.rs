use super::umlclass_models::{
    UmlClass, UmlClassDiagram, UmlClassElement, UmlClassLink, UmlClassLinkType, UmlClassPackage,
    UmlClassStereotype,
};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ColorLabels, ColorProfile, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, Model, ModelHierarchyView, MulticonnectionAdapter, MulticonnectionView, PackageAdapter, PackageView, ProjectCommand, Queryable, SelectionStatus, SensitiveCommand, SimpleModelHierarchyView, SnapManager, TargettingStatus, Tool, VertexInformation, View
};
use crate::common::project_serde::{NHDeserializeError, NHDeserializeScalar, NHDeserializer, NHSerialize, NHSerializeError, NHSerializeToScalar, NHSerializer};
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::CustomTab;
use eframe::egui;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::{Arc, RwLock, Weak},
};

struct UmlClassDomain;
impl Domain for UmlClassDomain {
    type CommonElementT = UmlClassElement;
    type CommonElementViewT = UmlClassElementView;
    type QueryableT<'a> = UmlClassQueryable<'a>;
    type ToolT = NaiveUmlClassTool;
    type AddCommandElementT = UmlClassElementOrVertex;
    type PropChangeT = UmlClassPropChange;
}

type PackageViewT = PackageView<UmlClassDomain, UmlClassPackageAdapter>;
type LinkViewT = MulticonnectionView<UmlClassDomain, UmlClassLinkAdapter>;

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
    NameChange(Arc<String>),

    StereotypeChange(UmlClassStereotype),
    PropertiesChange(Arc<String>),
    FunctionsChange(Arc<String>),

    LinkTypeChange(UmlClassLinkType),
    SourceArrowheadLabelChange(Arc<String>),
    DestinationArrowheadLabelChange(Arc<String>),

    CommentChange(Arc<String>),
    FlipMulticonnection,
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
            UmlClassPropChange::FlipMulticonnection => Ok(FlipMulticonnection {}),
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

pub fn colors() -> (String, ColorLabels, Vec<ColorProfile>) {
    #[rustfmt::skip]
    let c = crate::common::controller::build_colors!(
                                   ["Light",              "Darker"],
        [("Diagram background",    [egui::Color32::WHITE, egui::Color32::GRAY,]),
         ("Package background",    [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),
         ("Connection background", [egui::Color32::WHITE, egui::Color32::WHITE,]),
         ("Class background",      [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),],
        [("Diagram gridlines",     [egui::Color32::from_rgb(220, 220, 220), egui::Color32::from_rgb(127, 127, 127),]),
         ("Package foreground",    [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Connection foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Class foreground",      [egui::Color32::BLACK, egui::Color32::BLACK,]),],
        [("Selection",             [egui::Color32::BLUE,  egui::Color32::LIGHT_BLUE,]),],
    );
    ("UML Class diagram".to_owned(), c.0, c.1)
}

#[derive(Clone, derive_more::From)]
pub enum UmlClassElementView {
    Package(Arc<RwLock<PackageViewT>>),
    Class(Arc<RwLock<UmlClassController>>),
    Link(Arc<RwLock<LinkViewT>>),
}

impl NHSerialize for UmlClassElementView {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            Self::Package(inner) => inner.read().unwrap().serialize_into(into),
            Self::Class(inner) => inner.read().unwrap().serialize_into(into),
            Self::Link(inner) => inner.read().unwrap().serialize_into(into),
        }
    }
}
// impl NHDeserialize for UmlClassElementView {}
impl View for UmlClassElementView {
    fn uuid(&self) -> Arc<ViewUuid> {
        match self {
            Self::Package(inner) => inner.read().unwrap().uuid(),
            Self::Class(inner) => inner.read().unwrap().uuid(),
            Self::Link(inner) => inner.read().unwrap().uuid(),
        }
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        match self {
            Self::Package(inner) => inner.read().unwrap().model_uuid(),
            Self::Class(inner) => inner.read().unwrap().model_uuid(),
            Self::Link(inner) => inner.read().unwrap().model_uuid(),
        }
    }
    fn model_name(&self) -> Arc<String> {
        match self {
            Self::Package(inner) => inner.read().unwrap().model_name(),
            Self::Class(inner) => inner.read().unwrap().model_name(),
            Self::Link(inner) => inner.read().unwrap().model_name(),
        }
    }
}
impl ElementController<UmlClassElement> for UmlClassElementView {
    fn model(&self) -> UmlClassElement {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().model(),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().model(),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().model(),
        }
    }
    fn min_shape(&self) -> NHShape {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().min_shape(),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().min_shape(),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().min_shape(),
        }
    }
    fn max_shape(&self) -> NHShape {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().max_shape(),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().max_shape(),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().max_shape(),
        }
    }
    fn position(&self) -> egui::Pos2 {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().position(),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().position(),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().position(),
        }
    }
}
impl ContainerGen2<UmlClassDomain> for UmlClassElementView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<<UmlClassDomain as Domain>::CommonElementViewT> {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().controller_for(uuid),
        }
    }
}
impl ElementControllerGen2<UmlClassDomain> for UmlClassElementView {
    fn show_properties(
        &mut self,
        q: &UmlClassQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> bool {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().show_properties(q, ui, commands),
        }
    }
    fn draw_in(
        &mut self,
        q: &UmlClassQueryable,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
    ) -> TargettingStatus {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().draw_in(q, context, canvas, tool),
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().collect_allignment(am),
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<NaiveUmlClassTool>,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> EventHandlingStatus {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().handle_event(event, ehc, tool, commands),
        }
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().apply_command(command, undo_accumulator),
        }
    }
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().head_count(flattened_views, flattened_views_status, flattened_represented_models),
        }
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView>,
        c: &mut HashMap<ViewUuid, UmlClassElementView>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, UmlClassElementView>,
        c: &mut HashMap<ViewUuid, UmlClassElementView>,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            UmlClassElementView::Class(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
            UmlClassElementView::Link(rw_lock) => rw_lock.read().unwrap().deep_copy_clone(uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, UmlClassElementView>,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        match self {
            UmlClassElementView::Package(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            UmlClassElementView::Class(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
            UmlClassElementView::Link(rw_lock) => rw_lock.write().unwrap().deep_copy_relink(c, m),
        }
    }
}

#[derive(Clone)]
pub struct UmlClassDiagramAdapter {
    model: Arc<RwLock<UmlClassDiagram>>,
    name_buffer: String,
    comment_buffer: String,
}

impl DiagramAdapter<UmlClassDomain, UmlClassDiagram> for UmlClassDiagramAdapter {
    fn model(&self) -> Arc<RwLock<UmlClassDiagram>> {
        self.model.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name()
    }
    fn view_type(&self) -> &'static str {
        "umlclass-diagram-view"
    }

    fn create_new_view_for(
        &self,
        q: &UmlClassQueryable<'_>,
        element: UmlClassElement,
    ) -> UmlClassElementOrVertex {
        let v = match element {
            UmlClassElement::UmlClassPackage(rw_lock) => {
                UmlClassElementView::from(
                    new_umlclass_package_view(
                        rw_lock,
                        egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 100.0) },
                    )
                )
            },
            UmlClassElement::UmlClass(rw_lock) => {
                UmlClassElementView::from(
                    new_umlclass_class_view(rw_lock, egui::Pos2::ZERO)
                )
            },
            UmlClassElement::UmlClassLink(rw_lock) => {
                let m = rw_lock.read().unwrap();
                let source_view = q.get_view(&m.source.read().unwrap().uuid).unwrap();
                let target_view = q.get_view(&m.target.read().unwrap().uuid).unwrap();
                UmlClassElementView::from(
                    new_umlclass_link_view(rw_lock.clone(), None, source_view, target_view)
                )
            },
        };

        UmlClassElementOrVertex::from(v)
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
                egui::TextEdit::singleline(&mut self.name_buffer),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![UmlClassPropChange::NameChange(Arc::new(
                        self.name_buffer.clone(),
                    ))],
                )
                .into(),
            );
        }

        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.comment_buffer),
            )
            .changed()
        {
            commands.push(
                InsensitiveCommand::PropertyChange(
                    std::iter::once(*view_uuid).collect(),
                    vec![UmlClassPropChange::CommentChange(Arc::new(
                        self.comment_buffer.clone(),
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
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    UmlClassPropChange::NameChange(name) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::NameChange(model.name.clone())],
                        ));
                        self.name_buffer = (**name).clone();
                        model.name = name.clone();
                    }
                    UmlClassPropChange::CommentChange(comment) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(model.comment.clone())],
                        ));
                        self.comment_buffer = (**comment).clone();
                        model.comment = comment.clone();
                    }
                    _ => {}
                }
            }
        }
    }

    fn tool_change_fun(&self, tool: &mut Option<NaiveUmlClassTool>, ui: &mut egui::Ui) {
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
                [width, 20.0],
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

        for cat in [
            &[
                (UmlClassToolStage::Class, "Class"),
                (UmlClassToolStage::PackageStart, "Package"),
            ][..],
            &[
                (
                    UmlClassToolStage::LinkStart {
                        link_type: UmlClassLinkType::Association,
                    },
                    "Association",
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_type: UmlClassLinkType::InterfaceRealization,
                    },
                    "IntReal",
                ),
                (
                    UmlClassToolStage::LinkStart {
                        link_type: UmlClassLinkType::Usage,
                    },
                    "Usage",
                ),
            ][..],
            &[(UmlClassToolStage::Note, "Note")][..],
        ] {
            for (stage, name) in cat {
                if ui
                    .add_sized([width, 20.0], egui::Button::new(*name).fill(c(*stage)))
                    .clicked()
                {
                    *tool = Some(NaiveUmlClassTool::new(*stage));
                }
            }
            ui.separator();
        }
    }

    fn menubar_options_fun(&self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
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
        ui.separator();
    }

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let (new_model, models) = super::umlclass_models::deep_copy_diagram(&self.model.read().unwrap());
        (
            Self {
                model: new_model,
                ..self.clone()
            },
            models,
        )
    }

    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, UmlClassElement>) {
        let models = super::umlclass_models::fake_copy_diagram(&self.model.read().unwrap());
        (self.clone(), models)
    }
}

impl NHSerializeToScalar for UmlClassDiagramAdapter {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<toml::Value, NHSerializeError> {
        self.model.read().unwrap().serialize_into(into)?;

        Ok(toml::Value::String(self.model.read().unwrap().uuid().to_string()))
    }
}

impl NHDeserializeScalar for UmlClassDiagramAdapter {
    fn deserialize(
        source: &toml::Value,
        deserializer: &NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        let toml::Value::String(s) = source else {
            return Err(NHDeserializeError::StructureError(format!("expected string, got {:?}", source)));
        };
        let uuid = uuid::Uuid::parse_str(s)?.into();
        let model = deserializer.get_or_instantiate_model::<UmlClassDiagram>(&uuid)?;
        let name_buffer = (*model.read().unwrap().name).clone();
        let comment_buffer = (*model.read().unwrap().comment).clone();
        Ok(Self { model, name_buffer, comment_buffer })
    }
}

struct PlantUmlTab {
    diagram: Arc<RwLock<UmlClassDiagram>>,
    plantuml_description: String,
}

impl CustomTab for PlantUmlTab {
    fn title(&self) -> String {
        "PlantUML description".to_owned()
    }

    fn show(&mut self, /*context: &mut NHApp,*/ ui: &mut egui::Ui) {
        if ui.button("Refresh").clicked() {
            let diagram = self.diagram.read().unwrap();
            self.plantuml_description = diagram.plantuml();
        }

        ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.plantuml_description.as_str()),
        );
    }
}

pub fn new(no: u32) -> (Arc<RwLock<dyn DiagramController>>, Arc<dyn ModelHierarchyView>) {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New UML class diagram {}", no);
    let diagram = Arc::new(RwLock::new(UmlClassDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    )));
    (
        DiagramControllerGen2::new(
            Arc::new(view_uuid),
            UmlClassDiagramAdapter {
                model: diagram.clone(),
                name_buffer: name,
                comment_buffer: "".to_owned(),
            },
            Vec::new(),
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram)),
    )
}

pub fn demo(no: u32) -> (Arc<RwLock<dyn DiagramController>>, Arc<dyn ModelHierarchyView>) {
    // https://www.uml-diagrams.org/class-diagrams-overview.html
    // https://www.uml-diagrams.org/design-pattern-abstract-factory-uml-class-diagram-example.html

    let (class_af, class_af_view) = new_umlclass_class(
        UmlClassStereotype::Interface,
        "AbstractFactory",
        "",
        "+createProductA(): ProductA\n+createProductB(): ProductB\n",
        egui::Pos2::new(200.0, 150.0),
    );

    let (class_cfx, class_cfx_view) = new_umlclass_class(
        UmlClassStereotype::Class,
        "ConcreteFactoryX",
        "",
        "+createProductA(): ProductA\n+createProductB(): ProductB\n",
        egui::Pos2::new(100.0, 250.0),
    );

    let (class_cfy, class_cfy_view) = new_umlclass_class(
        UmlClassStereotype::Class,
        "ConcreteFactoryY",
        "",
        "+createProductA(): ProductA\n+createProductB(): ProductB\n",
        egui::Pos2::new(300.0, 250.0),
    );

    let (realization_cfx, realization_cfx_view) = new_umlclass_link(
        UmlClassLinkType::InterfaceRealization,
        "",
        None,
        (class_cfx.clone(), class_cfx_view.clone().into()),
        (class_af.clone(), class_af_view.clone().into()),
    );

    let (realization_cfy, realization_cfy_view) = new_umlclass_link(
        UmlClassLinkType::InterfaceRealization,
        "",
        None,
        (class_cfy.clone(), class_cfy_view.clone().into()),
        (class_af.clone(), class_af_view.clone().into()),
    );

    let (class_client, class_client_view) = new_umlclass_class(
        UmlClassStereotype::Class,
        "Client",
        "",
        "",
        egui::Pos2::new(300.0, 50.0),
    );

    let (usage_client_af, usage_client_af_view) = new_umlclass_link(
        UmlClassLinkType::Usage,
        "<<use>>",
        Some((uuid::Uuid::now_v7().into(), egui::Pos2::new(200.0, 50.0))),
        (class_client.clone(), class_client_view.clone().into()),
        (class_af.clone(), class_af_view.clone().into()),
    );

    let (class_producta, class_producta_view) = new_umlclass_class(
        UmlClassStereotype::Interface,
        "ProductA",
        "",
        "",
        egui::Pos2::new(450.0, 150.0),
    );

    let (usage_client_producta, usage_client_producta_view) =
        new_umlclass_link(
            UmlClassLinkType::Usage,
            "<<use>>",
            Some((uuid::Uuid::now_v7().into(), egui::Pos2::new(450.0, 52.0))),
            (class_client.clone(), class_client_view.clone().into()),
            (class_producta.clone(), class_producta_view.clone().into()),
        );

    let (class_productb, class_productb_view) = new_umlclass_class(
        UmlClassStereotype::Interface,
        "ProductB",
        "",
        "",
        egui::Pos2::new(650.0, 150.0),
    );

    let (usage_client_productb, usage_client_productb_view) =
        new_umlclass_link(
            UmlClassLinkType::Usage,
            "<<use>>",
            Some((uuid::Uuid::now_v7().into(), egui::Pos2::new(650.0, 48.0))),
            (class_client.clone(), class_client_view.clone().into()),
            (class_productb.clone(), class_productb_view.clone().into()),
        );

    let mut owned_controllers = Vec::<UmlClassElementView>::new();
    owned_controllers.push(class_af_view.into());
    owned_controllers.push(class_cfx_view.into());
    owned_controllers.push(class_cfy_view.into());
    owned_controllers.push(realization_cfx_view.into());
    owned_controllers.push(realization_cfy_view.into());
    owned_controllers.push(class_client_view.into());
    owned_controllers.push(usage_client_af_view.into());
    owned_controllers.push(class_producta_view.into());
    owned_controllers.push(usage_client_producta_view.into());
    owned_controllers.push(class_productb_view.into());
    owned_controllers.push(usage_client_productb_view.into());

    let diagram_view_uuid = uuid::Uuid::now_v7().into();
    let diagram_model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("Demo UML class diagram {}", no);
    let diagram2 = Arc::new(RwLock::new(UmlClassDiagram::new(
        diagram_model_uuid,
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
        ],
    )));
    (
        DiagramControllerGen2::new(
            Arc::new(diagram_view_uuid),
            UmlClassDiagramAdapter {
                model: diagram2.clone(),
                name_buffer: name,
                comment_buffer: "".to_owned(),
            },
            owned_controllers,
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram2)),
    )
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Class,
    LinkStart { link_type: UmlClassLinkType },
    LinkEnd,
    PackageStart,
    PackageEnd,
    Note,
}

enum PartialUmlClassElement {
    None,
    Some(UmlClassElementView),
    Link {
        link_type: UmlClassLinkType,
        source: Arc<RwLock<UmlClass>>,
        dest: Option<Arc<RwLock<UmlClass>>>,
    },
    Package {
        a: egui::Pos2,
        a_display: egui::Pos2,
        b: Option<egui::Pos2>,
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
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClassPackage(..)) => match self.current_stage {
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClass(..)) => match self.current_stage {
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::Class
                | UmlClassToolStage::Note
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => NON_TARGETTABLE_COLOR,
            },
            Some(UmlClassElement::UmlClassLink(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &UmlClassQueryable,  canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialUmlClassElement::Link {
                source,
                link_type,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.read().unwrap().uuid()) {
                    canvas.draw_line(
                        [source_view.position(), pos],
                        match link_type.line_type() {
                            canvas::LineType::Solid => {
                                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK)
                            }
                            canvas::LineType::Dashed => {
                                canvas::Stroke::new_dashed(1.0, egui::Color32::BLACK)
                            }
                        },
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
            (UmlClassToolStage::Class, _) => {
                let (_class_model, class_view) =
                    new_umlclass_class(UmlClassStereotype::Class, "a class", "", "", pos);
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
            (UmlClassToolStage::Note, _) => {}
            _ => {}
        }
    }
    fn add_element<'a>(&mut self, controller: UmlClassElement) {
        if self.event_lock {
            return;
        }

        match controller {
            UmlClassElement::UmlClassPackage(..) => {}
            UmlClassElement::UmlClass(inner) => {
                match (self.current_stage, &mut self.result) {
                    (UmlClassToolStage::LinkStart { link_type }, PartialUmlClassElement::None) => {
                        self.result = PartialUmlClassElement::Link {
                            link_type,
                            source: inner,
                            dest: None,
                        };
                        self.current_stage = UmlClassToolStage::LinkEnd;
                        self.event_lock = true;
                    }
                    (
                        UmlClassToolStage::LinkEnd,
                        PartialUmlClassElement::Link { dest, .. },
                    ) => {
                        *dest = Some(inner);
                        self.event_lock = true;
                    }
                    _ => {}
                }
            }
            UmlClassElement::UmlClassLink(..) => {}
        }
    }
    fn try_construct(&mut self, into: &dyn ContainerGen2<UmlClassDomain>) -> Option<UmlClassElementView> {
        match &self.result {
            PartialUmlClassElement::Some(x) => {
                let x = x.clone();
                self.result = PartialUmlClassElement::None;
                Some(x)
            }
            PartialUmlClassElement::Link {
                link_type,
                source,
                dest: Some(dest),
                ..
            } => {
                self.current_stage = UmlClassToolStage::LinkStart {
                    link_type: *link_type,
                };

                let association_view: Option<UmlClassElementView>
                    = if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source.read().unwrap().uuid()),
                    into.controller_for(&dest.read().unwrap().uuid()),
                ) {
                    let (_link_model, link_view) = new_umlclass_link(
                        *link_type,
                        "",
                        None,
                        (source.clone(), source_controller),
                        (dest.clone(), dest_controller),
                    );

                    Some(link_view.into())
                } else {
                    None
                };

                self.result = PartialUmlClassElement::None;
                association_view
            }
            PartialUmlClassElement::Package { a, b: Some(b), .. } => {
                self.current_stage = UmlClassToolStage::PackageStart;

                let (_package_model, package_view) =
                    new_umlclass_package("a package", egui::Rect::from_two_pos(*a, *b));

                self.result = PartialUmlClassElement::None;
                Some(package_view.into())
            }
            _ => None,
        }
    }
    fn reset_event_lock(&mut self) {
        self.event_lock = false;
    }
}

#[derive(Clone)]
pub struct UmlClassPackageAdapter {
    model: Arc<RwLock<UmlClassPackage>>,
}

impl PackageAdapter<UmlClassDomain> for UmlClassPackageAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }

    fn view_type(&self) -> &'static str {
        "umlclass-package-view"
    }
    
    fn add_element(&mut self, element: UmlClassElement) {
        self.model.write().unwrap().add_element(element);
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) {
        self.model.write().unwrap().delete_elements(uuids);
    }

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) {
        let model = self.model.read().unwrap();
        let mut name_buffer = (*model.name).clone();
        ui.label("Name:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut name_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::NameChange(Arc::new(name_buffer)),
            ]));
        }

        let mut comment_buffer = (*model.comment).clone();
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::CommentChange(Arc::new(comment_buffer)),
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
            let mut model = self.model.write().unwrap();
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

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read().unwrap();

        let model = if let Some(UmlClassElement::UmlClassPackage(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(UmlClassPackage::new(new_uuid, (*old_model.name).clone(), old_model.contained_elements.clone())));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self { model }
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
) -> (Arc<RwLock<UmlClassPackage>>, Arc<RwLock<PackageViewT>>) {
    let model_uuid = uuid::Uuid::now_v7().into();
    let package_model = Arc::new(RwLock::new(UmlClassPackage::new(
        model_uuid,
        name.to_owned(),
        vec![],
    )));
    let package_view = new_umlclass_package_view(package_model.clone(), bounds_rect);

    (package_model, package_view)
}
fn new_umlclass_package_view(
    model: Arc<RwLock<UmlClassPackage>>,
    bounds_rect: egui::Rect,
) -> Arc<RwLock<PackageViewT>> {
    let view_uuid = uuid::Uuid::now_v7().into();
    PackageView::new(
        Arc::new(view_uuid),
        UmlClassPackageAdapter {
            model,
        },
        HashMap::new(),
        bounds_rect,
    )
}

fn new_umlclass_class(
    stereotype: UmlClassStereotype,
    name: &str,
    properties: &str,
    functions: &str,
    position: egui::Pos2,
) -> (Arc<RwLock<UmlClass>>, Arc<RwLock<UmlClassController>>) {
    let class_model_uuid = uuid::Uuid::now_v7().into();
    let class_model = Arc::new(RwLock::new(UmlClass::new(
        class_model_uuid,
        stereotype,
        name.to_owned(),
        properties.to_owned(),
        functions.to_owned(),
    )));
    let class_view = new_umlclass_class_view(class_model.clone(), position);

    (class_model, class_view)
}
fn new_umlclass_class_view(
    model: Arc<RwLock<UmlClass>>,
    position: egui::Pos2,
) -> Arc<RwLock<UmlClassController>> {
    let m = model.read().unwrap();
    let class_view_uuid = uuid::Uuid::now_v7().into();
    let class_view = Arc::new(RwLock::new(UmlClassController {
        uuid: Arc::new(class_view_uuid),
        model: model.clone(),
        self_reference: Weak::new(),

        stereotype_buffer: m.stereotype,
        name_buffer: (*m.name).clone(),
        properties_buffer: (*m.properties).clone(),
        functions_buffer: (*m.functions).clone(),
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::ZERO,
    }));
    class_view.write().unwrap().self_reference = Arc::downgrade(&class_view);
    class_view
}

pub struct UmlClassController {
    uuid: Arc<ViewUuid>,
    pub model: Arc<RwLock<UmlClass>>,
    self_reference: Weak<RwLock<Self>>,

    stereotype_buffer: UmlClassStereotype,
    name_buffer: String,
    properties_buffer: String,
    functions_buffer: String,
    comment_buffer: String,

    dragged_shape: Option<NHShape>,
    highlight: canvas::Highlight,
    pub position: egui::Pos2,
    pub bounds_rect: egui::Rect,
}

impl View for UmlClassController {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name.clone()
    }
}

impl NHSerialize for UmlClassController {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        let mut element = toml::Table::new();
        element.insert("uuid".to_owned(), toml::Value::String(self.uuid.to_string()));
        element.insert("type".to_owned(), toml::Value::String("umlclass-class-view".to_owned()));
        element.insert("position".to_owned(), toml::Value::Array(vec![toml::Value::Float(self.position.x as f64), toml::Value::Float(self.position.y as f64)]));
        into.insert_view(*self.uuid, element);

        Ok(())
    }
}

impl ElementController<UmlClassElement> for UmlClassController {
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

impl ContainerGen2<UmlClassDomain> for UmlClassController {}

impl ElementControllerGen2<UmlClassDomain> for UmlClassController {
    fn show_properties(
        &mut self,
        _parent: &UmlClassQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        ui.label("Model properties");

        ui.label("Stereotype:");
        egui::ComboBox::from_id_salt("Stereotype:")
            .selected_text(self.stereotype_buffer.char())
            .show_ui(ui, |ui| {
                for value in [
                    UmlClassStereotype::Abstract,
                    UmlClassStereotype::AbstractClass,
                    UmlClassStereotype::Class,
                    UmlClassStereotype::Entity,
                    UmlClassStereotype::Enum,
                    UmlClassStereotype::Interface,
                ] {
                    if ui
                        .selectable_value(&mut self.stereotype_buffer, value, value.char())
                        .clicked()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::StereotypeChange(self.stereotype_buffer),
                        ]));
                    }
                }
            });

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

        ui.label("Properties:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut self.properties_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::PropertiesChange(Arc::new(self.properties_buffer.clone())),
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
                UmlClassPropChange::FunctionsChange(Arc::new(self.functions_buffer.clone())),
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

        true
    }

    fn draw_in(
        &mut self,
        _: &UmlClassQueryable,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &NaiveUmlClassTool)>,
    ) -> TargettingStatus {
        let read = self.model.read().unwrap();

        self.bounds_rect = canvas.draw_class(
            self.position,
            Some(read.stereotype.char()),
            &read.name,
            None,
            &[&read.parse_properties(), &read.parse_functions()],
            context.profile.backgrounds[3],
            canvas::Stroke::new_solid(1.0, context.profile.foregrounds[3]),
            self.highlight,
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
                    if !ehc.modifiers.command {
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
    ) {
        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight.selected = *select;
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
            | InsensitiveCommand::PasteSpecificElements(..) => {}
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    for property in properties {
                        match property {
                            UmlClassPropChange::StereotypeChange(stereotype) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::StereotypeChange(
                                        model.stereotype.clone(),
                                    )],
                                ));
                                self.stereotype_buffer = stereotype.clone();
                                model.stereotype = stereotype.clone();
                            }
                            UmlClassPropChange::NameChange(name) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::NameChange(model.name.clone())],
                                ));
                                self.name_buffer = (**name).clone();
                                model.name = name.clone();
                            }
                            UmlClassPropChange::PropertiesChange(properties) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::PropertiesChange(
                                        model.properties.clone(),
                                    )],
                                ));
                                self.properties_buffer = (**properties).clone();
                                model.properties = properties.clone();
                            }
                            UmlClassPropChange::FunctionsChange(functions) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::FunctionsChange(
                                        model.functions.clone(),
                                    )],
                                ));
                                self.functions_buffer = (**functions).clone();
                                model.functions = functions.clone();
                            }
                            UmlClassPropChange::CommentChange(comment) => {
                                let mut model = self.model.write().unwrap();
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::CommentChange(model.comment.clone())],
                                ));
                                self.comment_buffer = (**comment).clone();
                                model.comment = comment.clone();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
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
        let old_model = self.model.read().unwrap();

        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *old_model.uuid)
        };

        let modelish = if let Some(UmlClassElement::UmlClass(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(UmlClass::new(model_uuid, old_model.stereotype, (*old_model.name).clone(), (*old_model.properties).clone(), (*old_model.functions).clone())));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = Arc::new(RwLock::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            self_reference: Weak::new(),
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
            properties_buffer: self.properties_buffer.clone(),
            functions_buffer: self.functions_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
            dragged_shape: None,
            highlight: self.highlight,
            position: self.position,
            bounds_rect: self.bounds_rect,
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
}

#[derive(Clone)]
pub struct UmlClassLinkAdapter {
    model: Arc<RwLock<UmlClassLink>>,
}

impl MulticonnectionAdapter<UmlClassDomain> for UmlClassLinkAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().unwrap().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().link_type.name()
    }

    fn view_type(&self) -> &'static str {
        "umlclass-link-view"
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.model.read().unwrap().description.clone())
    }

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        let model = self.model.read().unwrap();
        (
            model.link_type.line_type(),
            model.link_type.source_arrowhead_type(),
            if !model.source_arrowhead_label.is_empty() {
                Some(model.source_arrowhead_label.clone())
            } else {
                None
            }
        )
    }

    fn destination_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        let model = self.model.read().unwrap();
        (
            model.link_type.line_type(),
            model.link_type.destination_arrowhead_type(),
            if !model.target_arrowhead_label.is_empty() {
                Some(model.target_arrowhead_label.clone())
            } else {
                None
            }
        )
    }

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) {
        let model = self.model.read().unwrap();
        let mut link_type_buffer = model.link_type.clone();
        ui.label("Link type:");
        egui::ComboBox::from_id_salt("link type")
            .selected_text(&*link_type_buffer.name())
            .show_ui(ui, |ui| {
                for sv in [
                    UmlClassLinkType::Association,
                    UmlClassLinkType::Aggregation,
                    UmlClassLinkType::Composition,
                    UmlClassLinkType::Generalization,
                    UmlClassLinkType::InterfaceRealization,
                    UmlClassLinkType::Usage,
                ] {
                    if ui
                        .selectable_value(&mut link_type_buffer, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkTypeChange(link_type_buffer),
                        ]));
                    }
                }
            });

        let mut sal_buffer = (*model.source_arrowhead_label).clone();
        ui.label("Source:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut sal_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::SourceArrowheadLabelChange(Arc::new(
                    sal_buffer,
                )),
            ]));
        }
        ui.separator();

        let mut dal_buffer = (*model.target_arrowhead_label).clone();
        ui.label("Destination:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut dal_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::DestinationArrowheadLabelChange(Arc::new(
                    dal_buffer,
                )),
            ]));
        }
        ui.separator();

        ui.label("Swap source and destination:");
        if ui.button("Swap").clicked() {
            // (model.source, model.destination) = (model.destination.clone(), model.source.clone());
            /* TODO:
            (self.source, self.destination) = (self.destination.clone(), self.source.clone());
            (self.source_points, self.dest_points) =
                (self.dest_points.clone(), self.source_points.clone());
                */
        }
        ui.separator();

        let mut comment_buffer = (*model.comment).clone();
        ui.label("Comment:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::multiline(&mut comment_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::CommentChange(Arc::new(comment_buffer)),
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
            let mut model = self.model.write().unwrap();
            for property in properties {
                match property {
                    UmlClassPropChange::LinkTypeChange(link_type) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::LinkTypeChange(model.link_type.clone())],
                        ));
                        model.link_type = link_type.clone();
                    }
                    UmlClassPropChange::SourceArrowheadLabelChange(source_arrowhead_label) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(
                                model.source_arrowhead_label.clone(),
                            )],
                        ));
                        model.source_arrowhead_label = source_arrowhead_label.clone();
                    }
                    UmlClassPropChange::DestinationArrowheadLabelChange(
                        destination_arrowhead_label,
                    ) => {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*view_uuid).collect(),
                            vec![UmlClassPropChange::CommentChange(
                                model.target_arrowhead_label.clone(),
                            )],
                        ));
                        model.target_arrowhead_label = destination_arrowhead_label.clone();
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

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read().unwrap();

        let model = if let Some(UmlClassElement::UmlClassLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = Arc::new(RwLock::new(UmlClassLink::new(new_uuid, old_model.link_type, (*old_model.description).clone(), old_model.source.clone(), old_model.target.clone())));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self { model }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut model = self.model.write().unwrap();
        
        let source_uuid = *model.source.read().unwrap().uuid;
        if let Some(UmlClassElement::UmlClass(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }
        let target_uuid = *model.source.read().unwrap().uuid;
        if let Some(UmlClassElement::UmlClass(new_target)) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}

fn new_umlclass_link(
    link_type: UmlClassLinkType,
    description: impl Into<String>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (Arc<RwLock<UmlClass>>, UmlClassElementView),
    target: (Arc<RwLock<UmlClass>>, UmlClassElementView),
) -> (Arc<RwLock<UmlClassLink>>, Arc<RwLock<LinkViewT>>) {
    let link_model_uuid = uuid::Uuid::now_v7().into();
    let link_model = Arc::new(RwLock::new(UmlClassLink::new(
        link_model_uuid,
        link_type,
        description,
        source.0,
        target.0,
    )));
    let link_view = new_umlclass_link_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_umlclass_link_view(
    model: Arc<RwLock<UmlClassLink>>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView,
    target: UmlClassElementView,
) -> Arc<RwLock<LinkViewT>> {
    let link_view_uuid = uuid::Uuid::now_v7().into();
    MulticonnectionView::new(
        Arc::new(link_view_uuid),
        UmlClassLinkAdapter {
            model,
        },
        source,
        target,
        center_point,
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
    )
}
