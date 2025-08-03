use super::umlclass_models::{
    UmlClass, UmlClassDiagram, UmlClassElement, UmlClassLink, UmlClassLinkType, UmlClassPackage,
    UmlClassStereotype, UmlClassCommentLink,
};
use crate::common::canvas::{self, NHCanvas, NHShape};
use crate::common::controller::{
    ColorLabels, ColorProfile, ContainerGen2, ContainerModel, DiagramAdapter, DiagramController, DiagramControllerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, FlipMulticonnection, InputEvent, InsensitiveCommand, Model, ModelHierarchyView, MulticonnectionAdapter, MulticonnectionView, PackageAdapter, PackageView, ProjectCommand, Queryable, SelectionStatus, SensitiveCommand, SimpleModelHierarchyView, SnapManager, TargettingStatus, Tool, VertexInformation, View
};
use crate::common::entity::{Entity, EntityUuid};
use crate::common::eref::ERef;
use crate::common::uuid::{ModelUuid, ViewUuid};
use crate::common::project_serde::{NHDeserializer, NHDeserializeError, NHDeserializeInstantiator};
use crate::umlclass::umlclass_models::UmlClassComment;
use crate::CustomTab;
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
    type ToolT = NaiveUmlClassTool;
    type AddCommandElementT = UmlClassElementOrVertex;
    type PropChangeT = UmlClassPropChange;
}

type PackageViewT = PackageView<UmlClassDomain, UmlClassPackageAdapter>;
type LinkViewT = MulticonnectionView<UmlClassDomain, UmlClassLinkAdapter>;
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
         ("Class background",      [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),
         ("Comment background",    [egui::Color32::WHITE, egui::Color32::from_rgb(159, 159, 159),]),],
        [("Diagram gridlines",     [egui::Color32::from_rgb(220, 220, 220), egui::Color32::from_rgb(127, 127, 127),]),
         ("Package foreground",    [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Connection foreground", [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Class foreground",      [egui::Color32::BLACK, egui::Color32::BLACK,]),
         ("Comment foreground",    [egui::Color32::BLACK, egui::Color32::BLACK,]),],
        [("Selection",             [egui::Color32::BLUE,  egui::Color32::LIGHT_BLUE,]),],
    );
    ("UML Class diagram".to_owned(), c.0, c.1)
}

#[derive(Clone, derive_more::From, nh_derive::NHContextSerDeTag)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub enum UmlClassElementView {
    Package(ERef<PackageViewT>),
    Class(ERef<UmlClassView>),
    Link(ERef<LinkViewT>),
    Comment(ERef<UmlClassCommentView>),
    CommentLink(ERef<CommentLinkViewT>),
}

impl Entity for UmlClassElementView {
    fn tagged_uuid(&self) -> EntityUuid {
        match self {
            Self::Package(inner) => inner.read().tagged_uuid(),
            Self::Class(inner) => inner.read().tagged_uuid(),
            Self::Link(inner) => inner.read().tagged_uuid(),
            Self::Comment(inner) => inner.read().tagged_uuid(),
            Self::CommentLink(inner) => inner.read().tagged_uuid(),
        }
    }
}

impl View for UmlClassElementView {
    fn uuid(&self) -> Arc<ViewUuid> {
        match self {
            Self::Package(inner) => inner.read().uuid(),
            Self::Class(inner) => inner.read().uuid(),
            Self::Link(inner) => inner.read().uuid(),
            Self::Comment(inner) => inner.read().uuid(),
            Self::CommentLink(inner) => inner.read().uuid(),
        }
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        match self {
            Self::Package(inner) => inner.read().model_uuid(),
            Self::Class(inner) => inner.read().model_uuid(),
            Self::Link(inner) => inner.read().model_uuid(),
            Self::Comment(inner) => inner.read().model_uuid(),
            Self::CommentLink(inner) => inner.read().model_uuid(),
        }
    }
    fn model_name(&self) -> Arc<String> {
        match self {
            Self::Package(inner) => inner.read().model_name(),
            Self::Class(inner) => inner.read().model_name(),
            Self::Link(inner) => inner.read().model_name(),
            Self::Comment(inner) => inner.read().model_name(),
            Self::CommentLink(inner) => inner.read().model_name(),
        }
    }
}
impl ElementController<UmlClassElement> for UmlClassElementView {
    fn model(&self) -> UmlClassElement {
        match self {
            Self::Package(inner) => inner.read().model(),
            Self::Class(inner) => inner.read().model(),
            Self::Link(inner) => inner.read().model(),
            Self::Comment(inner) => inner.read().model(),
            Self::CommentLink(inner) => inner.read().model(),
        }
    }
    fn min_shape(&self) -> NHShape {
        match self {
            Self::Package(inner) => inner.read().min_shape(),
            Self::Class(inner) => inner.read().min_shape(),
            Self::Link(inner) => inner.read().min_shape(),
            Self::Comment(inner) => inner.read().min_shape(),
            Self::CommentLink(inner) => inner.read().min_shape(),
        }
    }
    fn max_shape(&self) -> NHShape {
        match self {
            Self::Package(inner) => inner.read().max_shape(),
            Self::Class(inner) => inner.read().max_shape(),
            Self::Link(inner) => inner.read().max_shape(),
            Self::Comment(inner) => inner.read().max_shape(),
            Self::CommentLink(inner) => inner.read().max_shape(),
        }
    }
    fn position(&self) -> egui::Pos2 {
        match self {
            Self::Package(inner) => inner.read().position(),
            Self::Class(inner) => inner.read().position(),
            Self::Link(inner) => inner.read().position(),
            Self::Comment(inner) => inner.read().position(),
            Self::CommentLink(inner) => inner.read().position(),
        }
    }
}
impl ContainerGen2<UmlClassDomain> for UmlClassElementView {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<<UmlClassDomain as Domain>::CommonElementViewT> {
        match self {
            Self::Package(inner) => inner.read().controller_for(uuid),
            Self::Class(inner) => inner.read().controller_for(uuid),
            Self::Link(inner) => inner.read().controller_for(uuid),
            Self::Comment(inner) => inner.read().controller_for(uuid),
            Self::CommentLink(inner) => inner.read().controller_for(uuid),
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
            Self::Package(inner) => inner.write().show_properties(q, ui, commands),
            Self::Class(inner) => inner.write().show_properties(q, ui, commands),
            Self::Link(inner) => inner.write().show_properties(q, ui, commands),
            Self::Comment(inner) => inner.write().show_properties(q, ui, commands),
            Self::CommentLink(inner) => inner.write().show_properties(q, ui, commands),
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
            Self::Package(inner) => inner.write().draw_in(q, context, canvas, tool),
            Self::Class(inner) => inner.write().draw_in(q, context, canvas, tool),
            Self::Link(inner) => inner.write().draw_in(q, context, canvas, tool),
            Self::Comment(inner) => inner.write().draw_in(q, context, canvas, tool),
            Self::CommentLink(inner) => inner.write().draw_in(q, context, canvas, tool),
        }
    }
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        match self {
            Self::Package(inner) => inner.write().collect_allignment(am),
            Self::Class(inner) => inner.write().collect_allignment(am),
            Self::Link(inner) => inner.write().collect_allignment(am),
            Self::Comment(inner) => inner.write().collect_allignment(am),
            Self::CommentLink(inner) => inner.write().collect_allignment(am),
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
            Self::Package(inner) => inner.write().handle_event(event, ehc, tool, commands),
            Self::Class(inner) => inner.write().handle_event(event, ehc, tool, commands),
            Self::Link(inner) => inner.write().handle_event(event, ehc, tool, commands),
            Self::Comment(inner) => inner.write().handle_event(event, ehc, tool, commands),
            Self::CommentLink(inner) => inner.write().handle_event(event, ehc, tool, commands),
        }
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match self {
            Self::Package(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            Self::Class(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            Self::Link(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            Self::Comment(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
            Self::CommentLink(inner) => inner.write().apply_command(command, undo_accumulator, affected_models),
        }
    }
    fn refresh_buffers(&mut self) {
        match self {
            Self::Package(inner) => inner.write().refresh_buffers(),
            Self::Class(inner) => inner.write().refresh_buffers(),
            Self::Link(inner) => inner.write().refresh_buffers(),
            Self::Comment(inner) => inner.write().refresh_buffers(),
            Self::CommentLink(inner) => inner.write().refresh_buffers(),
        }
    }
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, UmlClassElementView>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        match self {
            Self::Package(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            Self::Class(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            Self::Link(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            Self::Comment(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
            Self::CommentLink(inner) => inner.write().head_count(flattened_views, flattened_views_status, flattened_represented_models),
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        match self {
            Self::Package(inner) => inner.read().delete_when(deleting),
            Self::Class(inner) => inner.read().delete_when(deleting),
            Self::Link(inner) => inner.read().delete_when(deleting),
            Self::Comment(inner) => inner.read().delete_when(deleting),
            Self::CommentLink(inner) => inner.read().delete_when(deleting),
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
            Self::Package(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            Self::Class(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            Self::Link(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            Self::Comment(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
            Self::CommentLink(inner) => inner.read().deep_copy_walk(requested, uuid_present, tlc, c, m),
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
            Self::Package(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            Self::Class(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            Self::Link(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            Self::Comment(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
            Self::CommentLink(inner) => inner.read().deep_copy_clone(uuid_present, tlc, c, m),
        }
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, UmlClassElementView>,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        match self {
            Self::Package(inner) => inner.write().deep_copy_relink(c, m),
            Self::Class(inner) => inner.write().deep_copy_relink(c, m),
            Self::Link(inner) => inner.write().deep_copy_relink(c, m),
            Self::Comment(inner) => inner.write().deep_copy_relink(c, m),
            Self::CommentLink(inner) => inner.write().deep_copy_relink(c, m),
        }
    }
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassDiagramAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
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
    views: [UmlClassElementView; 7],
}

impl Default for UmlClassPlaceholderViews {
    fn default() -> Self {
        let (class, class_view) = new_umlclass_class(UmlClassStereotype::Class, "a class", "", "", egui::Pos2::ZERO);
        let class = (class, class_view.into());
        let (d, dv) = new_umlclass_class(UmlClassStereotype::Class, "dummy", "", "", egui::Pos2::new(100.0, 75.0));
        let dummy = (d, dv.into());
        let (_package, package_view) = new_umlclass_package("a package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });

        let (_assoc, assoc_view) = new_umlclass_link(UmlClassLinkType::Association, "", None, class.clone(), dummy.clone());
        let (_intreal, intreal_view) = new_umlclass_link(UmlClassLinkType::InterfaceRealization, "", None, class.clone(), dummy.clone());
        let (_usage, usage_view) = new_umlclass_link(UmlClassLinkType::Usage, "", None, class.clone(), dummy.clone());

        let (comment, comment_view) = new_umlclass_comment("a comment", egui::Pos2::new(-100.0, -75.0));
        let comment = (comment, comment_view.into());
        let commentlink = new_umlclass_commentlink(None, comment.clone(), (class.0.into(), class.1.clone()));

        Self {
            views: [
                class.1,
                package_view.into(),
                assoc_view.into(),
                intreal_view.into(),
                usage_view.into(),
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
        self.model.read().name()
    }
    fn view_type(&self) -> &'static str {
        "umlclass-diagram-view"
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
            UmlClassElement::UmlClass(inner) => {
                UmlClassElementView::from(
                    new_umlclass_class_view(inner, egui::Pos2::ZERO)
                )
            },
            UmlClassElement::UmlClassLink(inner) => {
                let m = inner.read();
                let (sid, tid) = (m.source.read().uuid(), m.target.read().uuid());
                let (source_view, target_view) = match (q.get_view(&sid), q.get_view(&tid)) {
                    (Some(sv), Some(tv)) => (sv, tv),
                    _ => return Err(HashSet::from([*sid, *tid])),
                };
                UmlClassElementView::from(
                    new_umlclass_link_view(inner.clone(), None, source_view, target_view)
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
        self.buffer.name = (*model.name).clone();
        self.buffer.comment = (*model.comment).clone();
    }

    fn show_tool_palette(
        &mut self,
        tool: &mut Option<NaiveUmlClassTool>,
        drawing_context: &DrawingContext,
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
            &[
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
                let mut painter = ui.painter().with_clip_rect(icon_rect);
                let mut mc = canvas::MeasuringCanvas::new(&painter);
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut mc, &None);
                let (scale, offset) = mc.scale_offset_to_fit(egui::Vec2::new(button_height, button_height));
                let mut c = canvas::UiCanvas::new(false, painter, icon_rect, offset, scale, None);
                c.clear(drawing_context.profile.backgrounds[0].gamma_multiply(0.75));
                self.placeholders.views[icon_counter].draw_in(&empty_q, drawing_context, &mut c, &None);
                icon_counter += 1;
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

struct PlantUmlTab {
    diagram: ERef<UmlClassDiagram>,
    plantuml_description: String,
}

impl CustomTab for PlantUmlTab {
    fn title(&self) -> String {
        "PlantUML description".to_owned()
    }

    fn show(&mut self, /*context: &mut NHApp,*/ ui: &mut egui::Ui) {
        if ui.button("Refresh").clicked() {
            self.plantuml_description = self.diagram.read().plantuml();
        }

        ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.plantuml_description.as_str()),
        );
    }
}

pub fn new(no: u32) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>) {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New UML class diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    ));
    (
        DiagramControllerGen2::new(
            Arc::new(view_uuid),
            name.clone().into(),
            UmlClassDiagramAdapter::new(diagram.clone()),
            Vec::new(),
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram)),
    )
}

pub fn demo(no: u32) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>) {
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
    let diagram2 = ERef::new(UmlClassDiagram::new(
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
    ));
    (
        DiagramControllerGen2::new(
            Arc::new(diagram_view_uuid),
            name.clone().into(),
            UmlClassDiagramAdapter::new(diagram2.clone()),
            owned_controllers,
        ),
        Arc::new(SimpleModelHierarchyView::new(diagram2)),
    )
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<(ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>), NHDeserializeError> {
    let v = d.get_entity::<DiagramControllerGen2<UmlClassDomain, UmlClassDiagramAdapter>>(&uuid)?;
    let mhv = Arc::new(SimpleModelHierarchyView::new(v.read().model()));
    Ok((v, mhv))
}

#[derive(Clone, Copy, PartialEq)]
pub enum UmlClassToolStage {
    Class,
    LinkStart { link_type: UmlClassLinkType },
    LinkEnd,
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
        link_type: UmlClassLinkType,
        source: ERef<UmlClass>,
        dest: Option<ERef<UmlClass>>,
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
                UmlClassToolStage::Class
                | UmlClassToolStage::Comment
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::CommentLinkStart | UmlClassToolStage::CommentLinkEnd => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClassPackage(..)) => match self.current_stage {
                UmlClassToolStage::Class
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkEnd => TARGETTABLE_COLOR,
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::CommentLinkStart => {
                    NON_TARGETTABLE_COLOR
                }
            },
            Some(UmlClassElement::UmlClass(..)) => match self.current_stage {
                UmlClassToolStage::LinkStart { .. } | UmlClassToolStage::LinkEnd
                | UmlClassToolStage::CommentLinkEnd => {
                    TARGETTABLE_COLOR
                }
                UmlClassToolStage::Class
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
                | UmlClassToolStage::Class
                | UmlClassToolStage::PackageStart
                | UmlClassToolStage::PackageEnd
                | UmlClassToolStage::Comment
                | UmlClassToolStage::CommentLinkEnd => NON_TARGETTABLE_COLOR,
            },
            Some(UmlClassElement::UmlClassLink(..) | UmlClassElement::UmlClassCommentLink(..)) => todo!(),
        }
    }
    fn draw_status_hint(&self, q: &UmlClassQueryable,  canvas: &mut dyn NHCanvas, pos: egui::Pos2) {
        match &self.result {
            PartialUmlClassElement::Link {
                source,
                link_type,
                ..
            } => {
                if let Some(source_view) = q.get_view(&source.read().uuid()) {
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
            UmlClassElement::UmlClassLink(..) => {}
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
            UmlClassElement::UmlClassCommentLink(..) => {}
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
                let (source_uuid, target_uuid) = (*source.read().uuid(), *dest.read().uuid());
                if let (Some(source_controller), Some(dest_controller)) = (
                    into.controller_for(&source_uuid),
                    into.controller_for(&target_uuid),
                ) {
                    self.current_stage = UmlClassToolStage::LinkStart {
                        link_type: *link_type,
                    };

                    let (_link_model, link_view) = new_umlclass_link(
                        *link_type,
                        "",
                        None,
                        (source.clone(), source_controller),
                        (dest.clone(), dest_controller),
                    );

                    self.result = PartialUmlClassElement::None;

                    Some(link_view.into())
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

                    Some(link_view.into())
                } else {
                    None
                }
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
    stereotype: UmlClassStereotype,
    name: &str,
    properties: &str,
    functions: &str,
    position: egui::Pos2,
) -> (ERef<UmlClass>, ERef<UmlClassView>) {
    let class_model = ERef::new(UmlClass::new(
        uuid::Uuid::now_v7().into(),
        stereotype,
        name.to_owned(),
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

        stereotype_buffer: m.stereotype,
        name_buffer: (*m.name).clone(),
        properties_buffer: (*m.properties).clone(),
        functions_buffer: (*m.functions).clone(),
        comment_buffer: (*m.comment).clone(),

        dragged_shape: None,
        highlight: canvas::Highlight::NONE,
        position,
        bounds_rect: egui::Rect::from_min_max(position, position),
    })
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub struct UmlClassView {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    pub model: ERef<UmlClass>,

    #[nh_context_serde(skip_and_default)]
    stereotype_buffer: UmlClassStereotype,
    #[nh_context_serde(skip_and_default)]
    name_buffer: String,
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

impl Entity for UmlClassView {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl View for UmlClassView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().name.clone()
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
        let read = self.model.read();

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
        affected_models: &mut HashSet<ModelUuid>,
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
            | InsensitiveCommand::PasteSpecificElements(..)
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
                            UmlClassPropChange::PropertiesChange(properties) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::PropertiesChange(
                                        model.properties.clone(),
                                    )],
                                ));
                                model.properties = properties.clone();
                            }
                            UmlClassPropChange::FunctionsChange(functions) => {
                                undo_accumulator.push(InsensitiveCommand::PropertyChange(
                                    std::iter::once(*self.uuid).collect(),
                                    vec![UmlClassPropChange::FunctionsChange(
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
        self.stereotype_buffer = model.stereotype.clone();
        self.name_buffer = (*model.name).clone();
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
            let modelish = ERef::new(UmlClass::new(model_uuid, old_model.stereotype, (*old_model.name).clone(), (*old_model.properties).clone(), (*old_model.functions).clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        let mut cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            model: modelish,
            stereotype_buffer: self.stereotype_buffer.clone(),
            name_buffer: self.name_buffer.clone(),
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

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassLink>,
    #[nh_context_serde(skip_and_default)]
    link_type_buffer: UmlClassLinkType,
    #[nh_context_serde(skip_and_default)]
    sal_buffer: String,
    #[nh_context_serde(skip_and_default)]
    dal_buffer: String,
    #[nh_context_serde(skip_and_default)]
    comment_buffer: String,
}

impl MulticonnectionAdapter<UmlClassDomain> for UmlClassLinkAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        self.model.read().link_type.name()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        Some(self.model.read().description.clone())
    }

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        let model = self.model.read();
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
        let model = self.model.read();
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
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) {
        ui.label("Link type:");
        egui::ComboBox::from_id_salt("link type")
            .selected_text(&*self.link_type_buffer.name())
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
                        .selectable_value(&mut self.link_type_buffer, sv, &*sv.name())
                        .changed()
                    {
                        commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                            UmlClassPropChange::LinkTypeChange(self.link_type_buffer),
                        ]));
                    }
                }
            });

        ui.label("Source:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.sal_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::SourceArrowheadLabelChange(Arc::new(
                    self.sal_buffer.clone(),
                )),
            ]));
        }
        ui.separator();

        ui.label("Destination:");
        if ui
            .add_sized(
                (ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.dal_buffer),
            )
            .changed()
        {
            commands.push(SensitiveCommand::PropertyChangeSelected(vec![
                UmlClassPropChange::DestinationArrowheadLabelChange(Arc::new(
                    self.dal_buffer.clone(),
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
    fn refresh_buffers(&mut self) {
        let model = self.model.read();
        self.link_type_buffer = model.link_type;
        self.sal_buffer = (*model.source_arrowhead_label).clone();
        self.dal_buffer = (*model.target_arrowhead_label).clone();
        self.comment_buffer = (*model.comment).clone();
    }

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, UmlClassElement>,
    ) -> Self where Self: Sized {
        let old_model = self.model.read();

        let model = if let Some(UmlClassElement::UmlClassLink(m)) = m.get(&old_model.uuid) {
            m.clone()
        } else {
            let modelish = ERef::new(UmlClassLink::new(new_uuid, old_model.link_type, (*old_model.description).clone(), old_model.source.clone(), old_model.target.clone()));
            m.insert(*old_model.uuid, modelish.clone().into());
            modelish
        };

        Self {
            model,
            link_type_buffer: self.link_type_buffer.clone(),
            sal_buffer: self.sal_buffer.clone(),
            dal_buffer: self.dal_buffer.clone(),
            comment_buffer: self.comment_buffer.clone(),
        }
    }

    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, UmlClassElement>,
    ) {
        let mut model = self.model.write();
        
        let source_uuid = *model.source.read().uuid();
        if let Some(UmlClassElement::UmlClass(new_source)) = m.get(&source_uuid) {
            model.source = new_source.clone();
        }
        let target_uuid = *model.target.read().uuid();
        if let Some(UmlClassElement::UmlClass(new_target)) = m.get(&target_uuid) {
            model.target = new_target.clone();
        }
    }
}

fn new_umlclass_link(
    link_type: UmlClassLinkType,
    description: impl Into<String>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: (ERef<UmlClass>, UmlClassElementView),
    target: (ERef<UmlClass>, UmlClassElementView),
) -> (ERef<UmlClassLink>, ERef<LinkViewT>) {
    let link_model = ERef::new(UmlClassLink::new(
        uuid::Uuid::now_v7().into(),
        link_type,
        description,
        source.0,
        target.0,
    ));
    let link_view = new_umlclass_link_view(link_model.clone(), center_point, source.1, target.1);
    (link_model, link_view)
}
fn new_umlclass_link_view(
    model: ERef<UmlClassLink>,
    center_point: Option<(ViewUuid, egui::Pos2)>,
    source: UmlClassElementView,
    target: UmlClassElementView,
) -> ERef<LinkViewT> {
    let m = model.read();
    MulticonnectionView::new(
        Arc::new(uuid::Uuid::now_v7().into()),
        UmlClassLinkAdapter {
            model: model.clone(),
            link_type_buffer: m.link_type,
            sal_buffer: (*m.source_arrowhead_label).clone(),
            dal_buffer: (*m.target_arrowhead_label).clone(),
            comment_buffer: (*m.comment).clone(),
        },
        source,
        target,
        center_point,
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
    )
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
#[nh_context_serde(uuid_type = ViewUuid)]
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
        EntityUuid::View(*self.uuid)
    }
}

impl View for UmlClassCommentView {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().text.clone()
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
        _parent: &UmlClassQueryable,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
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

        true
    }

    fn draw_in(
        &mut self,
        _: &UmlClassQueryable,
        context: &DrawingContext,
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
            context.profile.backgrounds[4],
            canvas::Stroke::new_solid(1.0, context.profile.foregrounds[4]),
            self.highlight,
        );
        canvas.draw_polygon(
            [
                egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y + corner_size),
                egui::Pos2::new(self.bounds_rect.max.x - corner_size, self.bounds_rect.min.y),
            ].into_iter().collect(),
            context.profile.backgrounds[4],
            canvas::Stroke::new_solid(1.0, context.profile.foregrounds[4]),
            self.highlight,
        );
        canvas.draw_text(
            self.position,
            egui::Align2::CENTER_CENTER,
            &read.text,
            canvas::CLASS_MIDDLE_FONT_SIZE,
            context.profile.foregrounds[4],
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
        affected_models: &mut HashSet<ModelUuid>,
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
            | InsensitiveCommand::PasteSpecificElements(..)
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
        },
        source,
        target,
        center_point,
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
        vec![vec![(uuid::Uuid::now_v7().into(), egui::Pos2::ZERO)]],
    )
}

#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct UmlClassCommentLinkAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassCommentLink>,
}

impl MulticonnectionAdapter<UmlClassDomain> for UmlClassCommentLinkAdapter {
    fn model(&self) -> UmlClassElement {
        self.model.clone().into()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.model.read().uuid.clone()
    }

    fn model_name(&self) -> Arc<String> {
        self.model.read().name()
    }

    fn midpoint_label(&self) -> Option<Arc<String>> {
        None
    }

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        let model = self.model.read();
        (
            canvas::LineType::Dashed,
            canvas::ArrowheadType::None,
            None
        )
    }

    fn destination_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>) {
        (
            canvas::LineType::Dashed,
            canvas::ArrowheadType::None,
            None
        )
    }

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>
    ) {}
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>,
        undo_accumulator: &mut Vec<InsensitiveCommand<UmlClassElementOrVertex, UmlClassPropChange>>,
    ) {}
    fn refresh_buffers(&mut self) {}

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

        Self { model }
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
