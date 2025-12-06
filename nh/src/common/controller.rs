use crate::common::canvas::{self, Highlight, NHCanvas, NHShape, UiCanvas};
use crate::{CustomModal, CustomModalResult, CustomTab};
use eframe::egui;
use egui_ltreeview::DirPosition;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use super::project_serde::{NHContextDeserialize, NHContextSerialize};
use super::uuid::{ModelUuid, ViewUuid};
use super::views::ordered_views::OrderedViews;
use super::entity::{Entity, EntityUuid};
use super::eref::ERef;

pub struct SnapManager {
    input_restriction: egui::Rect,
    max_delta: egui::Vec2,
    guidelines_x: Vec<(f32, egui::Align, ViewUuid)>,
    guidelines_y: Vec<(f32, egui::Align, ViewUuid)>,
    best_xy: RwLock<(Option<f32>, Option<f32>)>,
}

impl SnapManager {
    pub fn new(input_restriction: egui::Rect, max_delta: egui::Vec2) -> Self {
        Self {
            input_restriction, max_delta,
            guidelines_x: Vec::new(), guidelines_y: Vec::new(),
            best_xy: RwLock::new((None, None)),
        }
    }
    pub fn add_shape(&mut self, uuid: ViewUuid, shape: canvas::NHShape) {
        if shape.bounding_box().intersects(self.input_restriction) {
            for e in shape.guidelines_anchors().into_iter() {
                self.guidelines_x.push((e.0.x, e.1, uuid));
                self.guidelines_y.push((e.0.y, e.1, uuid));
            }
        }
    }
    pub fn sort_guidelines(&mut self) {
        self.guidelines_x.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.guidelines_y.sort_by(|a, b| a.0.total_cmp(&b.0));
    }

    pub fn coerce<F>(&self, s: canvas::NHShape, uuids_filter: F) -> egui::Pos2
    where F: Fn(&ViewUuid) -> bool
    {
        *self.best_xy.write().unwrap() = (None, None);
        let (mut least_x, mut least_y): (Option<(f32, f32)>, Option<(f32, f32)>) = (None, None);
        let center = s.center();

        // Naive guidelines coordinate matching
        for p in s.guidelines_anchors().into_iter() {
            let start_x = self.guidelines_x.binary_search_by(|probe| probe.0.total_cmp(&(p.0.x - self.max_delta.x))).unwrap_or_else(|e| e);
            let end_x = self.guidelines_x.binary_search_by(|probe| probe.0.total_cmp(&(p.0.x + self.max_delta.x))).unwrap_or_else(|e| e);
            for g in self.guidelines_x[start_x..end_x].iter().filter(|e| uuids_filter(&e.2)) {
                if least_x.is_none_or(|b| (p.0.x - g.0).abs() < b.0.abs()) {
                    least_x = Some((p.0.x - g.0, g.0));
                }
            }
            let start_y = self.guidelines_y.binary_search_by(|probe| probe.0.total_cmp(&(p.0.y - self.max_delta.y))).unwrap_or_else(|e| e);
            let end_y = self.guidelines_y.binary_search_by(|probe| probe.0.total_cmp(&(p.0.y + self.max_delta.y))).unwrap_or_else(|e| e);
            for g in self.guidelines_y[start_y..end_y].iter().filter(|e| uuids_filter(&e.2)) {
                if least_y.is_none_or(|b| (p.0.y - g.0).abs() < b.0.abs()) {
                    least_y = Some((p.0.y - g.0, g.0));
                }
            }
        }

        // TODO: try pairwise projection of guidelines with matching Align

        least_x = least_x.filter(|e| e.0.abs() < self.max_delta.x);
        least_y = least_y.filter(|e| e.0.abs() < self.max_delta.y);
        *self.best_xy.write().unwrap() = (least_x.map(|e| e.1), least_y.map(|e| e.1));
        egui::Pos2::new(center.x - least_x.map(|e| e.0).unwrap_or(0.0), center.y - least_y.map(|e| e.0).unwrap_or(0.0))
    }

    pub fn draw_best(&self, canvas: &mut dyn NHCanvas, color: egui::Color32, rect: egui::Rect) {
        let (best_x, best_y) = *self.best_xy.read().unwrap();
        if let Some(bx) = best_x {
            canvas.draw_line([
                egui::Pos2::new(bx, rect.min.y), egui::Pos2::new(bx, rect.max.y)
            ], canvas::Stroke::new_solid(1.0, color), canvas::Highlight::NONE);
        }
        if let Some(by) = best_y {
            canvas.draw_line([
                egui::Pos2::new(rect.min.x, by), egui::Pos2::new(rect.max.x, by)
            ], canvas::Stroke::new_solid(1.0, color), canvas::Highlight::NONE);
        }
    }
}

impl Default for SnapManager {
    fn default() -> Self {
        Self {
            input_restriction: egui::Rect::ZERO,
            max_delta: egui::Vec2::ZERO,
            guidelines_x: Vec::new(),
            guidelines_y: Vec::new(),
            best_xy: RwLock::new((None, None)),
        }
    }
}

#[derive(Clone)]
pub enum ProjectCommand {
    SimpleProjectCommand(SimpleProjectCommand),
    RenameElement(ViewUuid, String),

    OpenAndFocusDiagram(ViewUuid, Option<egui::Pos2>),
    AddCustomTab(uuid::Uuid, Arc<RwLock<dyn CustomTab>>),
    SetNewDiagramNumber(u32),
    AddNewDiagram(usize, ERef<dyn DiagramController>),
    CopyDiagram(ViewUuid, /*deep:*/ bool),
    DeleteDiagram(ViewUuid),

    AddNewDocument(ViewUuid, String),
    OpenAndFocusDocument(ViewUuid, Option<egui::Pos2>),
    DuplicateDocument(ViewUuid),
    DeleteDocument(ViewUuid),
}

impl From<SimpleProjectCommand> for ProjectCommand {
    fn from(value: SimpleProjectCommand) -> ProjectCommand {
        ProjectCommand::SimpleProjectCommand(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SimpleProjectCommand {
    FocusedDiagramCommand(DiagramCommand),
    SpecificDiagramCommand(ViewUuid, DiagramCommand),
    DeleteSelectedElements(Option<DeleteKind>),
    OpenProject(bool),
    SaveProject,
    SaveProjectAs,
    CloseProject(bool),
    Exit(bool),
    SwapTopLanguages,
}

impl From<DiagramCommand> for SimpleProjectCommand {
    fn from(value: DiagramCommand) -> Self {
        SimpleProjectCommand::FocusedDiagramCommand(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DiagramCommand {
    DropRedoStackAndLastChangeFlag,
    SetLastChangeFlag,
    UndoImmediate,
    RedoImmediate,
    InvertSelection,
    DeleteSelectedElements(/*including_models:*/ bool),
    CutSelectedElements,
    CopySelectedElements,
    PasteClipboardElements,
    ArrangeSelected(Arrangement),
    ColorSelected(u8, MGlobalColor),
    HighlightAllElements(/*set: */bool, Highlight),
    HighlightElement(EntityUuid, /*set: */bool, Highlight),
    PanToElement(EntityUuid, /*force:*/bool),
    CreateViewFor(ModelUuid),
    DeleteViewFor(ModelUuid, /*including_model:*/ bool),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Arrangement {
    BringToFront,
    ForwardOne,
    BackwardOne,
    SendToBack,
}

pub enum HierarchyNode {
    Folder(ViewUuid, /*name:*/ Arc<String>, /*children:*/ Vec<HierarchyNode>),
    Diagram(ERef<dyn TopLevelView>),
    Document(ViewUuid),
}

impl HierarchyNode {
    pub fn uuid(&self) -> ViewUuid {
        match self {
            Self::Folder(uuid, ..) => *uuid,
            Self::Diagram(inner) => *inner.read().uuid(),
            Self::Document(uuid) => *uuid,
        }
    }

    pub fn get(&self, id: &ViewUuid) -> Option<(&HierarchyNode, &HierarchyNode)> {
        match self {
            Self::Folder(.., children) => {
                for c in children {
                    if c.uuid() == *id {
                        return Some((c, self));
                    }
                    if let Some(e) = c.get(id) {
                        return Some(e);
                    }
                }
            }
            Self::Diagram(..) | Self::Document(..) => {}
        }
        None
    }
    pub fn remove(&mut self, id: &ViewUuid) -> Option<HierarchyNode> {
        match self {
            Self::Folder(.., children) => {
                if let Some(index) = children.iter().position(|e| e.uuid() == *id) {
                    Some(children.remove(index))
                } else {
                    for node in children.iter_mut() {
                        let r = node.remove(id);
                        if r.is_some() {
                            return r;
                        }
                    }
                    None
                }
            }
            Self::Diagram(..) | Self::Document(..) => None,
        }
    }
    pub fn insert(
        &mut self,
        id: &ViewUuid,
        position: DirPosition<ViewUuid>,
        value: HierarchyNode,
    ) -> Result<(), HierarchyNode> {
        let self_uuid = self.uuid();
        match self {
            Self::Folder(.., children) => {
                if self_uuid == *id {
                    match position {
                        DirPosition::First => children.insert(0, value),
                        DirPosition::Last => children.push(value),
                        DirPosition::Before(id2) | DirPosition::After(id2) => {
                            if let Some(index) =
                                children.iter().position(|n| n.uuid() == id2)
                            {
                                children.insert(index + if matches!(position, DirPosition::After(_)) {1} else {0}, value);
                            }
                        }
                    }
                    Ok(())
                } else {
                    let mut value = Err(value);
                    for node in children.iter_mut() {
                        if let Err(v) = value {
                            value = node.insert(id, position, v);
                        }
                    }
                    value
                }
            }
            Self::Diagram(..) | Self::Document(..) => Err(value),
        }
    }
    pub fn for_each(&self, mut f: impl FnMut(&Self)) {
        f(self);
        match self {
            Self::Folder(.., children) => {
                children.iter().for_each(f);
            },
            Self::Diagram(..) | Self::Document(..) => {},
        }
    }
}

pub trait ModelHierarchyView {
    fn show_model_hierarchy(
        &self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        is_represented: &dyn Fn(&ModelUuid) -> bool,
    ) -> Vec<DiagramCommand>;
}

pub struct SimpleModelHierarchyView<ModelT>
    where ModelT: VisitableDiagram,
        <ModelT as ContainerModel>::ElementT: VisitableElement,
{
    model: ERef<ModelT>,
    label_provider: ERef<dyn LabelProvider>,
    state: RwLock<egui_ltreeview::TreeViewState<ModelUuid>>,
}

impl<ModelT> SimpleModelHierarchyView<ModelT>
    where ModelT: VisitableDiagram,
        <ModelT as ContainerModel>::ElementT: VisitableElement,
{
    pub fn new(
        model: ERef<ModelT>,
        label_provider: ERef<dyn LabelProvider>,
    ) -> Self {
        Self {
            model,
            label_provider,
            state: Default::default(),
        }
    }
}

impl<ModelT> ModelHierarchyView for SimpleModelHierarchyView<ModelT>
    where ModelT: VisitableDiagram,
        <ModelT as ContainerModel>::ElementT: VisitableElement,
{
    fn show_model_hierarchy(
        &self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        is_represented: &dyn Fn(&ModelUuid) -> bool
    ) -> Vec<DiagramCommand> {
        macro_rules! translate {
            ($context:expr, $msg_name:expr) => {
                $context.fluent_bundle.format_pattern(
                    $context.fluent_bundle.get_message($msg_name).unwrap().value().unwrap(),
                    None,
                    &mut vec![],
                )
            };
        }

        struct HierarchyViewVisitor<'data, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            gdc: &'data GlobalDrawingContext,
            commands: Vec<DiagramCommand>,
            is_represented: &'data dyn Fn(&ModelUuid) -> bool,
            label_provider: &'data dyn LabelProvider,
            builder: &'data mut egui_ltreeview::TreeViewBuilder<'ui, ModelUuid>,
            model: PhantomData<ModelT>,
        }
        impl<'data, 'ui, ModelT> HierarchyViewVisitor<'data, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            fn repr_glyph(&self, m: &ModelUuid) -> &'static str {
                if (self.is_represented)(m) {"[x]"} else {"[ ]"}
            }
            fn show_element(&mut self, is_dir: bool, model_uuid: &ModelUuid) {
                self.builder.node(
                    if is_dir {
                        egui_ltreeview::NodeBuilder::dir(*model_uuid).activatable(true)
                    } else {
                        egui_ltreeview::NodeBuilder::leaf(*model_uuid)
                    }.label(format!("{} {}", self.repr_glyph(model_uuid), self.label_provider.get(model_uuid)))
                        .context_menu(|ui| {
                            ui.set_min_width(crate::MIN_MENU_WIDTH);

                            let is_represented = (self.is_represented)(model_uuid);
                            if is_represented {
                                if ui.button(translate!(self.gdc, "nh-tab-modelhierarchy-jumpto")).clicked() {
                                    let model_uuid = (*model_uuid).into();
                                    self.commands.push(DiagramCommand::HighlightAllElements(false, Highlight::SELECTED));
                                    self.commands.push(DiagramCommand::HighlightElement(model_uuid, true, Highlight::SELECTED));
                                    self.commands.push(DiagramCommand::PanToElement(model_uuid, true));
                                    ui.close();
                                }
                                ui.separator();
                            }

                            if !is_represented && ui.button(translate!(self.gdc, "nh-tab-modelhierarchy-createview")).clicked() {
                                self.commands.push(DiagramCommand::CreateViewFor(*model_uuid));
                                ui.close();
                            }

                            if is_represented && ui.button(translate!(self.gdc, "nh-tab-modelhierarchy-deleteview")).clicked() {
                                self.commands.push(DiagramCommand::DeleteViewFor(*model_uuid, false));
                                ui.close();
                            }

                            if ui.button(translate!(self.gdc, "nh-tab-modelhierarchy-deletemodel")).clicked() {
                                self.commands.push(DiagramCommand::DeleteViewFor(*model_uuid, true));
                                ui.close();
                            }
                        })
                );
            }
        }
        impl<'data, 'ui, ModelT> ElementVisitor<<ModelT as ContainerModel>::ElementT> for HierarchyViewVisitor<'data, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            fn open_complex(&mut self, e: &<ModelT as ContainerModel>::ElementT) {
                self.show_element(true, &*e.uuid());
            }

            fn close_complex(&mut self, _e: &<ModelT as ContainerModel>::ElementT) {
                self.builder.close_dir();
            }

            fn visit_simple(&mut self, e: &<ModelT as ContainerModel>::ElementT) {
                self.show_element(false, &*e.uuid());
            }
        }
        impl<'data, 'ui, ModelT> DiagramVisitor<ModelT> for HierarchyViewVisitor<'data, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            fn open_diagram(&mut self, e: &ModelT) {
                let model_uuid = *e.uuid();
                self.builder.node(
                    egui_ltreeview::NodeBuilder::dir(model_uuid)
                        .label(&*self.label_provider.get(&model_uuid))
                );
            }

            fn close_diagram(&mut self, _e: &ModelT) {
                self.builder.close_dir();
            }
        }

        let mut set_state = None;
        let mut c = vec![];
        ui.horizontal(|ui| {
            if ui.button(translate!(gdc, "nh-tab-projecthierarchy-collapseall")).clicked() {
                set_state = Some(false);
            }
            if ui.button(translate!(gdc, "nh-tab-projecthierarchy-uncollapseall")).clicked() {
                set_state = Some(true);
            }
        });

        let mut state = self.state.write().unwrap();
        let (_r, a) = egui_ltreeview::TreeView::new(ui.make_persistent_id("model_hierarchy_view")).show_state(ui, &mut state, |builder| {
            let mut hvv = HierarchyViewVisitor {
                gdc,
                commands: vec![],
                is_represented, builder,
                label_provider: &*self.label_provider.read(),
                model: PhantomData,
            };

            self.model.read().accept(&mut hvv);

            c = hvv.commands;
        });

        for e in a {
            if let egui_ltreeview::Action::Activate(activate) = e {
                let e = activate.selected[0].into();
                c.push(DiagramCommand::HighlightAllElements(false, Highlight::SELECTED));
                c.push(DiagramCommand::HighlightElement(e, true, Highlight::SELECTED));
                c.push(DiagramCommand::PanToElement(e, true));
            }
        }

        if let Some(b) = set_state {
            struct StateChangeVisitor<'a, ModelT> {
                set_open: bool,
                state: &'a mut egui_ltreeview::TreeViewState<ModelUuid>,
                model: PhantomData<ModelT>,
            }

            impl<'a, ModelT> ElementVisitor<<ModelT as ContainerModel>::ElementT> for StateChangeVisitor<'a, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
            {
                fn open_complex(&mut self, e: &<ModelT as ContainerModel>::ElementT) {
                    self.state.set_openness(*e.uuid(), self.set_open);
                }

                fn close_complex(&mut self, _e: &<ModelT as ContainerModel>::ElementT) {}

                fn visit_simple(&mut self, e: &<ModelT as ContainerModel>::ElementT) {
                    self.state.set_openness(*e.uuid(), self.set_open);
                }
            }

            impl<'a, ModelT> DiagramVisitor<ModelT> for StateChangeVisitor<'a, ModelT>
                where ModelT: VisitableDiagram,
                    <ModelT as ContainerModel>::ElementT: VisitableElement,
            {
                fn open_diagram(&mut self, e: &ModelT) {
                    self.state.set_openness(*e.uuid(), self.set_open);
                }

                fn close_diagram(&mut self, _e: &ModelT) {}
            }

            self.model.read().accept(&mut StateChangeVisitor {
                set_open: b,
                state: &mut state,
                model: PhantomData,
            });
        }

        c
    }
}

pub trait LabelProvider {
    fn get(&self, uuid: &ModelUuid) -> Arc<String>;
}

pub trait CachingLabelDeriver<ModelT: Model>: LabelProvider + Default {
    fn filter_and_elipsis(src: &str) -> String {
        const CUTOFF: usize = 40;
        let mut s: String = src.chars()
            .map(|c| if c.is_whitespace() { ' ' } else { c } )
            .take(CUTOFF)
            .collect();
        if src.len() > CUTOFF {
            s.push_str("...");
        }
        s
    }

    fn update(&mut self, e: &ModelT);
    fn insert(&mut self, k: ModelUuid, v: Arc<String>);
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, derive_more::From, serde::Serialize, serde::Deserialize)]
pub enum MGlobalColor {
    /// None means no override compared to "standard element color"
    /// (which is usually white for background, black for foreground)
    /// i.e. None is very distinct from Local(Color32::Transparent)
    None,
    Local(egui::Color32),
    Global(uuid::Uuid),
}

pub fn mglobalcolor_edit_button(
    global_colors: &ColorBundle,
    ui: &mut egui::Ui,
    color: &mut MGlobalColor,
) -> bool {
    ui.horizontal(|ui| {
        let (response, painter) = ui.allocate_painter(egui::Vec2::new(30.0, 20.0), egui::Sense::click());

        match color {
            MGlobalColor::None => {
                painter.rect(
                    response.rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.0, egui::Color32::RED),
                    egui::StrokeKind::Inside,
                );
                painter.line_segment(
                    [response.rect.left_top(), response.rect.right_bottom()],
                    egui::Stroke::new(1.0, egui::Color32::RED),
                );
                painter.line_segment(
                    [response.rect.right_top(), response.rect.left_bottom()],
                    egui::Stroke::new(1.0, egui::Color32::RED),
                );
                ui.label("[no override]");
            },
            MGlobalColor::Local(color) => {
                painter.rect(
                    response.rect,
                    egui::CornerRadius::ZERO,
                    *color,
                    egui::Stroke::NONE,
                    egui::StrokeKind::Inside,
                );
                ui.label(color.to_hex());
            },
            MGlobalColor::Global(uuid) => {
                match global_colors.colors.get(&uuid) {
                    None => {
                        ui.label("[not found]");
                    },
                    Some((desc, color)) => {
                        painter.rect(
                            response.rect,
                            egui::CornerRadius::ZERO,
                            *color,
                            egui::Stroke::NONE,
                            egui::StrokeKind::Inside,
                        );
                        ui.label(desc);
                    },
                }
            },
        }

        if response.clicked() {
            true
        } else {
            false
        }
    }).inner
}

#[derive(Clone, Debug)]
pub struct ColorBundle {
    pub colors_order: Vec<uuid::Uuid>,
    pub colors: HashMap<uuid::Uuid, (String, egui::Color32)>,
}

impl ColorBundle {
    pub fn new() -> Self {
        Self {
            colors_order: Vec::new(),
            colors: HashMap::new(),
        }
    }
    pub fn get(&self, c: &MGlobalColor) -> Option<egui::Color32> {
        match c {
            MGlobalColor::None => None,
            MGlobalColor::Local(color32) => Some(*color32),
            MGlobalColor::Global(uuid) => {
                self.colors.get(uuid).map(|e| e.1)
            },
        }
    }
    pub fn clear(&mut self) {
        self.colors_order.clear();
        self.colors.clear();
    }
}

pub const TOOL_PALETTE_MIN_HEIGHT: u32 = 15;
pub const TOOL_PALETTE_MAX_HEIGHT: u32 = 200;
pub struct GlobalDrawingContext {
    pub global_colors: ColorBundle,
    pub fluent_bundle: fluent_bundle::FluentBundle<fluent_bundle::FluentResource>,
    pub shortcuts: HashMap<SimpleProjectCommand, egui::KeyboardShortcut>,
    pub tool_palette_item_height: u32,
}


pub trait View: Entity {
    fn uuid(&self) -> Arc<ViewUuid>;
    fn model_uuid(&self) -> Arc<ModelUuid>;
}

pub trait TopLevelView: View {
    fn view_name(&self) -> Arc<String>;
    fn set_view_name(&mut self, new_name: Arc<String>);
    fn view_type(&self) -> String;
}

pub trait DiagramController: Any + TopLevelView + NHContextSerialize {
    fn new_hierarchy_view(&self) -> Arc<dyn ModelHierarchyView>;
    fn represented_models(&self) -> &HashMap<ModelUuid, ViewUuid>;
    fn refresh_buffers(&mut self, affected_models: &HashSet<ModelUuid>);

    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        modifier_settings: ModifierSettings,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    );
    fn cancel_tool(&mut self);

    fn new_ui_canvas(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);

    fn draw_in(
        &mut self,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>,
    );

    fn context_menu(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );

    fn show_toolbar(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    );
    fn show_properties(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) -> Option<Box<dyn CustomModal>>;
    fn show_menubar_edit_options(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_menubar_view_options(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_menubar_diagram_options(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );

    fn apply_command(
        &mut self,
        command: DiagramCommand,
        global_undo: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    /// Create new view with new model
    fn deep_copy(&self) -> ERef<dyn DiagramController>;
    /// Create new view with the same model
    fn shallow_copy(&self) -> ERef<dyn DiagramController>;
}

pub trait ElementController<CommonElementT>: View {
    fn model(&self) -> CommonElementT;

    fn min_shape(&self) -> NHShape;
    fn bounding_box(&self) -> egui::Rect {
        self.min_shape().bounding_box()
    }

    // Position makes sense even for elements such as connections,
    // e.g. when a connection is a target of a connection
    fn position(&self) -> egui::Pos2;
}

#[derive(Clone, Copy, PartialEq)]
pub enum TargettingStatus {
    NotDrawn,
    Drawn,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DeleteKind {
    DeleteView,
    DeleteModelIfOnlyView,
    DeleteAll,
}

#[derive(Clone, Copy)]
pub struct ModifierSettings {
    pub default_delete_kind: Option<DeleteKind>,
    pub delete_view_modifier: Option<ModifierKeys>,
    pub delete_model_if_modifier: Option<ModifierKeys>,
    pub delete_all_modifier: Option<ModifierKeys>,
    sorted_delete_kinds: [(Option<ModifierKeys>, DeleteKind); 3],

    pub hold_selection: Option<ModifierKeys>,
    pub alternative_tool_mode: Option<ModifierKeys>,
}

impl ModifierSettings {
    pub fn sort_delete_kinds(&mut self) {
        self.sorted_delete_kinds[0] = (self.delete_view_modifier, DeleteKind::DeleteView);
        self.sorted_delete_kinds[1] = (self.delete_model_if_modifier, DeleteKind::DeleteModelIfOnlyView);
        self.sorted_delete_kinds[2] = (self.delete_all_modifier, DeleteKind::DeleteAll);
        self.sorted_delete_kinds.sort_by_key(|e| e.0.map(|e| e.set_bits()).unwrap_or(0));
    }
    pub fn get_delete_kind(&self, modifiers: ModifierKeys) -> Option<DeleteKind> {
        self.sorted_delete_kinds.iter()
            .find(|e| e.0.is_some_and(|e| modifiers.is_superset_of(e)))
            .map(|e| e.1)
            .or(self.default_delete_kind)
    }
}

impl Default for ModifierSettings {
    fn default() -> Self {
        Self {
            default_delete_kind: None,
            delete_view_modifier: None,
            delete_model_if_modifier: None,
            delete_all_modifier: Some(ModifierKeys::SHIFT),
            sorted_delete_kinds: [
                (Some(ModifierKeys::SHIFT), DeleteKind::DeleteAll),
                (None, DeleteKind::DeleteView),
                (None, DeleteKind::DeleteModelIfOnlyView),
            ],

            hold_selection: Some(ModifierKeys::COMMAND),
            alternative_tool_mode: Some(ModifierKeys::ALT),
        }
    }
}


#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ModifierKeys {
    pub alt: bool,
    pub command: bool, // mac_cmd || win_ctrl || linux_ctrl
    pub shift: bool,
}

impl ModifierKeys {
    pub const NONE: Self = Self {
        alt: false,
        command: false,
        shift: false,
    };
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    pub const COMMAND: Self = Self {
        command: true,
        ..Self::NONE
    };
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };

    pub fn from_egui(source: &egui::Modifiers) -> Self {
        Self {
            alt: source.alt,
            command: source.command,
            shift: source.shift,
        }
    }

    pub fn set_bits(&self) -> u8 {
        (if self.alt { 1 } else { 0 })
        + (if self.command { 1 } else { 0 })
        + (if self.shift { 1 } else { 0 })
    }

    pub fn is_superset_of(&self, other: Self) -> bool {
        (self.alt || !other.alt)
        && (self.command || !other.command)
        && (self.shift || !other.shift)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum InputEvent {
    MouseDown(egui::Pos2),
    MouseUp(egui::Pos2),
    Click(egui::Pos2),
    Drag {from: egui::Pos2, delta: egui::Vec2},
}

impl InputEvent {
    pub fn mouse_position(&self) -> &egui::Pos2 {
        match self {
            InputEvent::MouseDown(pos2) => pos2,
            InputEvent::MouseUp(pos2) => pos2,
            InputEvent::Click(pos2) => pos2,
            InputEvent::Drag { from, .. } => from,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum EventHandlingStatus {
    NotHandled, // = other element must handle it
    HandledByElement, // = handled by element only
    HandledByContainer, // = fully handled
}

/// Selection sensitive command - not inherently repeatable
#[derive(Clone, PartialEq, Debug)]
pub enum SensitiveCommand<AddElementT: Clone + Debug, PropChangeT: Clone + Debug> {
    MoveSelectedElements(egui::Vec2),
    ResizeSelectedElementsBy(egui::Align2, egui::Vec2),
    DeleteSelectedElements(/*including_models:*/ bool),
    CutSelectedElements,
    PasteClipboardElements,
    ArrangeSelected(Arrangement),
    PropertyChangeSelected(Vec<PropChangeT>),
    Insensitive(InsensitiveCommand<AddElementT, PropChangeT>)
}

impl<AddElementT: Clone + Debug, PropChangeT: Clone + Debug> SensitiveCommand<AddElementT, PropChangeT> {
    // TODO: I'm not sure whether this isn't actually the responsibility of the diagram itself
    fn to_selection_insensitive<F, G>(
        self,
        selected_elements: F,
        clipboard_elements: G,
    ) -> InsensitiveCommand<AddElementT, PropChangeT>
    where
        F: Fn() -> HashSet<ViewUuid>,
        G: Fn() -> Vec<AddElementT>
    {
        use SensitiveCommand as SC;
        use InsensitiveCommand as IC;
        if let SC::Insensitive(inner) = self {
            return inner;
        }
        if let SC::PasteClipboardElements = self {
            return IC::PasteSpecificElements(ViewUuid::nil(), clipboard_elements());
        }

        let se = selected_elements();
        match self {
            SC::MoveSelectedElements(delta) => IC::MoveSpecificElements(se, delta),
            SC::ResizeSelectedElementsBy(align, delta) => IC::ResizeSpecificElementsBy(se, align, delta),
            SC::DeleteSelectedElements(including_models) => IC::DeleteSpecificElements(se, including_models),
            SC::CutSelectedElements => IC::CutSpecificElements(se),
            SC::ArrangeSelected(arr) => IC::ArrangeSpecificElements(se, arr),
            SC::PropertyChangeSelected(changes) => IC::PropertyChange(se, changes),
            SC::Insensitive(..) | SC::PasteClipboardElements => unreachable!(),
        }
    }
}

impl<AddElementT: Clone + Debug, PropChangeT: Clone + Debug> From<InsensitiveCommand<AddElementT, PropChangeT>> for SensitiveCommand<AddElementT, PropChangeT> {
    fn from(value: InsensitiveCommand<AddElementT, PropChangeT>) -> Self {
        Self::Insensitive(value)
    }
}

pub type BucketNoT = u8;
pub type PositionNoT = u16;
/// Selection insensitive command - inherently repeatable
#[derive(Clone, PartialEq, Debug)]
pub enum InsensitiveCommand<AddElementT: Clone + Debug, PropChangeT: Clone + Debug> {
    HighlightAll(bool, Highlight),
    HighlightSpecific(HashSet<ViewUuid>, bool, Highlight),
    SelectByDrag(egui::Rect),
    MoveAllElements(egui::Vec2),
    MoveSpecificElements(HashSet<ViewUuid>, egui::Vec2),
    ResizeSpecificElementsBy(HashSet<ViewUuid>, egui::Align2, egui::Vec2),
    ResizeSpecificElementsTo(HashSet<ViewUuid>, egui::Align2, egui::Vec2),
    DeleteSpecificElements(HashSet<ViewUuid>, /*including_models:*/ bool),
    CutSpecificElements(HashSet<ViewUuid>),
    PasteSpecificElements(ViewUuid, Vec<AddElementT>),
    ArrangeSpecificElements(HashSet<ViewUuid>, Arrangement),
    AddDependency(ViewUuid, /*bucket:*/ BucketNoT, /*model pos:*/ Option<PositionNoT>, AddElementT, /*into_model:*/ bool),
    RemoveDependency(ViewUuid, BucketNoT, ViewUuid, /*including_model:*/ bool),
    PropertyChange(HashSet<ViewUuid>, Vec<PropChangeT>),
}

impl<AddElementT: Clone + Debug, PropChangeT: Clone + Debug>
    InsensitiveCommand<AddElementT, PropChangeT>
{
    fn info_text(&self) -> Arc<String> {
        match self {
            InsensitiveCommand::HighlightAll(..) | InsensitiveCommand::HighlightSpecific(..) | InsensitiveCommand::SelectByDrag(..) => {
                Arc::new("Sorry, your undo stack is broken now :/".to_owned())
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, false) => Arc::new(format!("Delete {} elements from view", uuids.len())),
            InsensitiveCommand::DeleteSpecificElements(uuids, true) => Arc::new(format!("Delete {} elements", uuids.len())),
            InsensitiveCommand::MoveSpecificElements(uuids, _delta) => {
                Arc::new(format!("Move {} elements", uuids.len()))
            }
            InsensitiveCommand::MoveAllElements(_delta) => {
                Arc::new(format!("Move all elements"))
            }
            InsensitiveCommand::ResizeSpecificElementsBy(uuids, _, _)
            | InsensitiveCommand::ResizeSpecificElementsTo(uuids, _, _) => {
                Arc::new(format!("Resize {} elements", uuids.len()))
            }
            InsensitiveCommand::CutSpecificElements(uuids) => Arc::new(format!("Cut {} elements", uuids.len())),
            InsensitiveCommand::PasteSpecificElements(_, uuids) => Arc::new(format!("Paste {} elements", uuids.len())),
            InsensitiveCommand::ArrangeSpecificElements(uuids, _) => Arc::new(format!("Arranged {} elements", uuids.len())),
            InsensitiveCommand::AddDependency(..)
            | InsensitiveCommand::RemoveDependency(..) => Arc::new(format!("Modify 1 element")),
            InsensitiveCommand::PropertyChange(uuids, ..) => {
                Arc::new(format!("Modify {} elements", uuids.len()))
            }
        }
    }

    // for purposes of repeatability - keep only last relevant
    fn merge(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (
                InsensitiveCommand::MoveSpecificElements(uuids1, delta1),
                InsensitiveCommand::MoveSpecificElements(uuids2, delta2),
            ) if uuids1 == uuids2 => Some(InsensitiveCommand::MoveSpecificElements(
                uuids1.clone(),
                *delta1 + *delta2,
            )),
            (
                InsensitiveCommand::ResizeSpecificElementsBy(uuids1, align1, delta1),
                InsensitiveCommand::ResizeSpecificElementsBy(uuids2, align2, delta2),
            ) if uuids1 == uuids2 && align1 == align2 => Some(InsensitiveCommand::ResizeSpecificElementsBy(
                uuids1.clone(),
                *align1,
                *delta1 + *delta2,
            )),
            (
                InsensitiveCommand::PropertyChange(uuids1, changes1),
                InsensitiveCommand::PropertyChange(uuids2, changes2),
            ) if uuids1 == uuids2 => Some(InsensitiveCommand::PropertyChange(
                uuids1.clone(),
                changes2.iter().rev().chain(changes1.iter().rev()).fold(
                    Vec::new(),
                    |mut uniques, e| {
                        if uniques
                            .iter()
                            .find(|u| std::mem::discriminant(*u) == std::mem::discriminant(e))
                            .is_none()
                        {
                            uniques.push(e.clone());
                        }
                        uniques
                    },
                ),
            )),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct ColorChangeData {
    pub slot: u8,
    pub color: MGlobalColor,
}

pub trait Domain: Sized + 'static {
    type CommonElementT: Model + VisitableElement + Clone;
    type DiagramModelT: ContainerModel<ElementT = Self::CommonElementT> + VisitableDiagram;
    type CommonElementViewT: ElementControllerGen2<Self> + serde::Serialize + NHContextSerialize + NHContextDeserialize + Clone;
    type ViewTargettingSectionT: Into<Self::CommonElementT>;
    type QueryableT<'a>: Queryable<'a, Self>;
    type LabelProviderT: CachingLabelDeriver<Self::CommonElementT>;
    type ToolT: Tool<Self>;
    type AddCommandElementT: From<Self::CommonElementViewT> + TryInto<Self::CommonElementViewT> + Clone + Debug;
    type PropChangeT: From<ColorChangeData> + TryInto<ColorChangeData> + Clone + Debug;
}

pub trait ElementVisitor<T: ?Sized> {
    fn open_complex(&mut self, e: &T);
    fn close_complex(&mut self, e: &T);
    fn visit_simple(&mut self, e: &T);
}
pub trait DiagramVisitor<T: ContainerModel>: ElementVisitor<T::ElementT> {
    fn open_diagram(&mut self, e: &T);
    fn close_diagram(&mut self, e: &T);
}

pub trait Model: Entity + 'static {
    fn uuid(&self) -> Arc<ModelUuid>;
}

pub trait VisitableElement: Model {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>) where Self: Sized {
        v.visit_simple(self);
    }
}
pub trait VisitableDiagram: ContainerModel where <Self as ContainerModel>::ElementT: VisitableElement {
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>);
}

pub trait ContainerModel: Model {
    type ElementT: Model;

    fn find_element(&self, _uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        None
    }
    fn insert_element(&mut self, _bucket: BucketNoT, _position: Option<PositionNoT>, element: Self::ElementT) -> Result<PositionNoT, Self::ElementT> {
        Err(element)
    }
    fn remove_element(&mut self, _uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        None
    }
}

pub trait Queryable<'a, DomainT: Domain> {
    // TODO: This is actually not a very good idea. Constructor should only be required where instantiated.
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, DomainT::CommonElementViewT>,
    ) -> Self;
    fn get_view(&self, m: &ModelUuid) -> Option<DomainT::CommonElementViewT>;
}

pub trait Tool<DomainT: Domain> {
    type Stage: 'static;

    fn initial_stage(&self) -> Self::Stage;

    fn targetting_for_section(&self, element: Option<DomainT::ViewTargettingSectionT>) -> egui::Color32;
    fn draw_status_hint(&self, q: &DomainT::QueryableT<'_>, canvas: &mut dyn NHCanvas, pos: egui::Pos2);

    fn add_position(&mut self, pos: egui::Pos2);
    fn add_section(&mut self, element: DomainT::ViewTargettingSectionT);

    fn try_additional_dependency(&mut self) -> Option<(BucketNoT, ModelUuid, ModelUuid)>;
    fn try_construct_view(
        &mut self,
        into: &dyn ContainerGen2<DomainT>,
    ) -> Option<(DomainT::CommonElementViewT, Option<Box<dyn CustomModal>>)>;

    fn reset_event_lock(&mut self);
}

pub trait ContainerGen2<DomainT: Domain> {
    fn controller_for(&self, _uuid: &ModelUuid) -> Option<DomainT::CommonElementViewT> {
        None
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SelectionStatus {
    NotSelected,
    TransitivelySelected,
    Selected,
}

impl SelectionStatus {
    pub fn selected(&self) -> bool {
        match self {
            Self::Selected => true,
            _ => false,
        }
    }
}

impl From<bool> for SelectionStatus {
    fn from(value: bool) -> Self {
        if value {
            SelectionStatus::Selected
        } else {
            SelectionStatus::NotSelected
        }
    }
}

pub struct EventHandlingContext<'a> {
    pub modifier_settings: ModifierSettings,
    pub modifiers: ModifierKeys,
    pub ui_scale: f32,
    pub all_elements: &'a HashMap<ViewUuid, SelectionStatus>,
    pub snap_manager: &'a SnapManager,
}

pub enum RequestType {
    ChangeColor(u8, MGlobalColor),
}

pub enum PropertiesStatus<DomainT: Domain> {
    NotShown,
    Shown,
    PromptRequest(RequestType),
    ToolRequest(Option<DomainT::ToolT>),
}

impl<DomainT: Domain> PropertiesStatus<DomainT> {
    pub fn to_non_default(self) -> Option<Self> {
        match self {
            Self::NotShown => None,
            e => Some(e),
        }
    }
}

pub trait ElementControllerGen2<DomainT: Domain>: ElementController<DomainT::CommonElementT> + NHContextSerialize + ContainerGen2<DomainT> + Send + Sync {
    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        _q: &DomainT::QueryableT<'_>,
        _lp: &DomainT::LabelProviderT,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> PropertiesStatus<DomainT> {
        PropertiesStatus::NotShown
    }
    fn draw_in(
        &mut self,
        _: &DomainT::QueryableT<'_>,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &DomainT::ToolT)>,
    ) -> TargettingStatus;
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<DomainT::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> EventHandlingStatus;
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    );
    fn refresh_buffers(&mut self);
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    );
    /// Must return true when a view cannot exist without these views, e.g. a link cannot exist without it's target
    fn delete_when(&self, _deleting: &HashSet<ViewUuid>) -> bool {
        false
    }

    // Create a deep copy, including the models
    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    );
    fn deep_copy_relink(
        &mut self,
        _c: &HashMap<ViewUuid, DomainT::CommonElementViewT>,
        _m: &HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {}
}

pub trait DiagramAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + 'static {
    fn model(&self) -> ERef<DomainT::DiagramModelT>;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;

    fn view_type(&self) -> &'static str;

    fn find_element(&self, model_uuid: &ModelUuid) -> Option<(DomainT::CommonElementT, ModelUuid)> {
        self.model().read().find_element(model_uuid)
    }
    fn insert_element(&mut self, bucket: BucketNoT, position: Option<PositionNoT>, element: DomainT::CommonElementT) -> Result<PositionNoT, DomainT::CommonElementT> {
        self.model().write().insert_element(bucket, position, element)
    }
    fn remove_element(&mut self, uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        self.model().write().remove_element(uuid)
    }
    fn create_new_view_for(
        &self,
        q: &DomainT::QueryableT<'_>,
        element: DomainT::CommonElementT,
    ) -> Result<DomainT::CommonElementViewT, HashSet<ModelUuid>>;

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32;
    fn gridlines_color(&self, global_colors: &ColorBundle) -> egui::Color32;
    fn show_view_props_fun(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> PropertiesStatus<DomainT>;
    fn show_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn refresh_buffers(&mut self);
    fn show_tool_palette(
        &mut self,
        tool: &mut Option<DomainT::ToolT>,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    );
    fn menubar_options_fun(
        &self,
        view_uuid: &ViewUuid,
        label_provider: &ERef<dyn LabelProvider>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DomainT::CommonElementT>);
    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, DomainT::CommonElementT>);
}

/// This is a generic DiagramController implementation.
/// Hopefully it should reduce the amount of code, but nothing prevents creating fully custom DiagramController implementations.
#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = Self::depends_on, initialize_with = Self::initialize)]
pub struct DiagramControllerGen2<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>,
> {
    uuid: Arc<ViewUuid>,
    name: Arc<String>,
    #[nh_context_serde(entity)]
    adapter: DiagramAdapterT,
    #[nh_context_serde(entity)]
    owned_views: OrderedViews<DomainT::CommonElementViewT>,
    #[nh_context_serde(skip_and_default)]
    temporaries: DiagramControllerGen2Temporaries<DomainT>,
}

struct DiagramControllerGen2Temporaries<DomainT: Domain> {
    name_buffer: String,

    flattened_views: HashMap<ViewUuid, DomainT::CommonElementViewT>,
    flattened_views_status: HashMap<ViewUuid, SelectionStatus>,
    flattened_represented_models: HashMap<ModelUuid, ViewUuid>,
    clipboard_elements: HashMap<ViewUuid, DomainT::CommonElementViewT>,
    label_provider: ERef<DomainT::LabelProviderT>,
    _layers: Vec<bool>,

    camera_offset: egui::Pos2,
    camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_interactive_canvas_rect: egui::Rect,
    snap_manager: SnapManager,
    current_tool: Option<DomainT::ToolT>,
    select_by_drag: Option<(egui::Pos2, egui::Pos2)>,

    last_change_flag: bool,
    undo_stack: Vec<(
        InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    )>,
    redo_stack: Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
}

impl<DomainT: Domain> Default for DiagramControllerGen2Temporaries<DomainT> {
    fn default() -> Self {
        Self {
            name_buffer: Default::default(),
            flattened_views: Default::default(),
            flattened_views_status: Default::default(),
            flattened_represented_models: Default::default(),
            clipboard_elements: Default::default(),
            label_provider: ERef::new(Default::default()),
            _layers: Default::default(),
            camera_offset: Default::default(),
            camera_scale: 1.0,
            last_unhandled_mouse_pos: Default::default(),
            last_interactive_canvas_rect: egui::Rect::ZERO,
            snap_manager: Default::default(),
            current_tool: Default::default(),
            select_by_drag: Default::default(),
            last_change_flag: Default::default(),
            undo_stack: Default::default(),
            redo_stack: Default::default(),
        }
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> DiagramControllerGen2<DomainT, DiagramAdapterT> {
    pub fn new(
        uuid: Arc<ViewUuid>,
        name: Arc<String>,
        adapter: DiagramAdapterT,
        owned_views: Vec<DomainT::CommonElementViewT>,
    ) -> ERef<Self> {
        let ret = ERef::new(Self {
            uuid,
            name,
            adapter,
            owned_views: OrderedViews::new(owned_views),
            temporaries: DiagramControllerGen2Temporaries::default(),
        });
        ret.write().initialize();

        ret
    }

    fn initialize(&mut self) {
        // Initialize flattened_* fields, etc.
        self.head_count();
        // Refresh all buffers to reflect model state
        self.refresh_all_buffers();
    }

    fn refresh_all_buffers(&mut self) {
        // Full label_provider update
        struct V<DomainT: Domain> {
            label_provider: DomainT::LabelProviderT,
        }

        impl<DomainT: Domain> ElementVisitor<<DomainT as Domain>::CommonElementT> for V<DomainT> {
            fn open_complex(&mut self, e: &<DomainT as Domain>::CommonElementT) {
                self.label_provider.update(e);
            }
            fn close_complex(&mut self, _e: &<DomainT as Domain>::CommonElementT) {}
            fn visit_simple(&mut self, e: &<DomainT as Domain>::CommonElementT) {
                self.label_provider.update(e);
            }
        }

        impl<DomainT: Domain> DiagramVisitor<<DomainT as Domain>::DiagramModelT> for V<DomainT> {
            fn open_diagram(&mut self, _e: &<DomainT as Domain>::DiagramModelT) {}
            fn close_diagram(&mut self, _e: &<DomainT as Domain>::DiagramModelT) {}
        }

        let mut v: V<DomainT> = V { label_provider: Default::default() };
        self.model().read().accept(&mut v);

        let mut label_provider = v.label_provider;
        label_provider.insert(*self.adapter.model_uuid(), self.adapter.model_name());
        self.temporaries.label_provider = ERef::new(label_provider);

        // Refresh buffers
        self.temporaries.name_buffer = (*self.name).clone();

        for v in self.temporaries.flattened_views.values_mut() {
            v.refresh_buffers();
        }
        self.adapter.refresh_buffers();
    }

    fn depends_on(&self) -> Vec<EntityUuid> {
        std::iter::once(self.model().read().tagged_uuid()).collect()
    }

    pub fn model(&self) -> ERef<DomainT::DiagramModelT> {
        self.adapter.model()
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        modifier_settings: ModifierSettings,
        modifiers: ModifierKeys,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) -> bool {
        // Collect alignment guides
        self.temporaries.snap_manager = SnapManager::new(self.temporaries.last_interactive_canvas_rect, egui::Vec2::splat(10.0 / self.temporaries.camera_scale));
        self.owned_views.event_order_foreach_mut(|v| v.collect_allignment(&mut self.temporaries.snap_manager));
        self.temporaries.snap_manager.sort_guidelines();

        // Handle events
        let mut commands = Vec::new();

        if matches!(event, InputEvent::Click(_)) {
            self.temporaries.current_tool.as_mut().map(|e| e.reset_event_lock());
        }

        let ehc = EventHandlingContext {
            modifier_settings,
            modifiers,
            ui_scale: self.temporaries.camera_scale,
            all_elements: &self.temporaries.flattened_views_status,
            snap_manager: &self.temporaries.snap_manager,
        };

        let child = self.owned_views.event_order_find_mut(|v| {
            let r = v.handle_event(event, &ehc, &mut self.temporaries.current_tool, element_setup_modal, &mut commands);
            if r != EventHandlingStatus::NotHandled {
                let k = v.uuid();
                Some((*k, match r {
                    EventHandlingStatus::HandledByElement if matches!(event, InputEvent::Click(_)) => {
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*k).collect(),
                                true,
                                Highlight::SELECTED,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*k).collect(),
                                !self.temporaries.flattened_views_status.get(&k).is_some_and(|e| e.selected()),
                                Highlight::SELECTED,
                            ).into());
                        }
                        EventHandlingStatus::HandledByContainer
                    }
                    a => a,
                }))
            } else {
                None
            }
        });

        let handled = match event {
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) | InputEvent::Drag { .. }
                if child.is_some() || self.temporaries.current_tool.is_some() => child.is_some(),
            InputEvent::MouseDown(pos) => {
                self.temporaries.select_by_drag = Some((pos, pos));
                true
            }
            InputEvent::MouseUp(_) => {
                self.temporaries.select_by_drag = None;
                true
            }
            InputEvent::Drag{ delta, ..} => {
                if let Some((a,b)) = self.temporaries.select_by_drag {
                    self.temporaries.select_by_drag = Some((a, b + delta));
                    commands.push(InsensitiveCommand::SelectByDrag(egui::Rect::from_two_pos(a, b + delta)).into());
                }
                true
            }
            InputEvent::Click(pos) => {
                let mut handled = child
                    .ok_or_else(|| {
                        commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                    })
                    .is_ok();

                if !handled {
                    if let Some(t) = self.temporaries.current_tool.as_mut() {
                        t.add_position(pos);
                    }
                }

                let mut tool = self.temporaries.current_tool.take();
                if let Some((t, target_id, dependency_id)) = tool.as_mut().and_then(|e| e.try_additional_dependency()) {
                    if let (Some(target_view_id), Some(dependency_view))
                        = (self.temporaries.flattened_represented_models.get(&target_id),
                            self.temporaries.flattened_represented_models.get(&dependency_id)
                            .and_then(|e| self.temporaries.flattened_views.get(e))) {
                        commands.push(InsensitiveCommand::AddDependency(*target_view_id, t, None, dependency_view.clone().into(), true).into());
                        handled = true;
                    };
                }
                if let Some((new_e, esm)) = tool.as_mut().and_then(|e| e.try_construct_view(self)) {
                    commands.push(InsensitiveCommand::AddDependency(
                        *self.uuid(),
                        0,
                        None,
                        DomainT::AddCommandElementT::from(new_e),
                        true,
                    ).into());
                    if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                        *element_setup_modal = esm;
                    }
                    handled = true;
                }
                self.temporaries.current_tool = tool;

                handled
            },
        };

        self.apply_commands(commands, undo_accumulator, true, true, affected_models);

        handled
    }

    fn set_clipboard_from_selected(&mut self) {
        let selected = self.temporaries.flattened_views_status.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect();
        self.temporaries.clipboard_elements = Self::elements_deep_copy(
            Some(&selected),
            |_| false,
            HashMap::new(),
            self.owned_views.iter_event_order_pairs().map(|e| (e.0, e.1.clone())),
        );
    }

    fn elements_deep_copy<VI>(
        requested: Option<&HashSet<ViewUuid>>,
        view_uuid_present: impl Fn(&ViewUuid) -> bool,
        existing_models: HashMap<ModelUuid, DomainT::CommonElementT>,
        source_views: VI,
    ) -> HashMap<ViewUuid, DomainT::CommonElementViewT>
        where
            VI: Iterator<Item=(ViewUuid, DomainT::CommonElementViewT)>,
    {
        let mut top_level_views = HashMap::new();
        let mut views = HashMap::new();
        let mut models = existing_models;

        for (_uuid, c) in source_views {
            c.deep_copy_walk(requested, &view_uuid_present, &mut top_level_views, &mut views, &mut models);
        }
        for (_usize, v) in top_level_views.iter_mut() {
            v.deep_copy_relink(&views, &models);
        }

        top_level_views
    }

    fn head_count(&mut self) {
        self.temporaries.flattened_views.clear();
        self.temporaries.flattened_views_status.clear();
        self.temporaries.flattened_represented_models.clear();
        self.owned_views.event_order_foreach_mut(|v|
            v.head_count(
                &mut self.temporaries.flattened_views,
                &mut self.temporaries.flattened_views_status,
                &mut self.temporaries.flattened_represented_models,
            )
        );
        for (k, v) in self.owned_views.iter_event_order_pairs() {
            self.temporaries.flattened_views.insert(k, v.clone());
        }
        self.temporaries.flattened_represented_models.insert(*self.adapter.model_uuid(), *self.uuid);
    }

    fn apply_commands(
        &mut self,
        commands: Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        global_undo_accumulator: &mut Vec<Arc<String>>,
        save_to_undo_stack: bool,
        clear_redo_stack: bool,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        for command in commands {
            let command = command.to_selection_insensitive(
                || self.temporaries.flattened_views_status.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect(),
                || Self::elements_deep_copy(
                    None,
                    |e| self.temporaries.flattened_views_status.contains_key(e),
                    HashMap::new(),
                    self.temporaries.clipboard_elements.iter().map(|e| (*e.0, e.1.clone())),
                    ).into_iter().map(|e| e.1.into()).collect(),
            );

            // compute transitive closure of dependency when deleting or cutting elements
            fn tr_closure<E: ElementControllerGen2<D>, D: Domain>(
                all: &mut HashMap<ViewUuid, E>,
                mut deleting: HashSet<ViewUuid>
            ) -> HashSet<ViewUuid> {
                // Calculate transitives of `deleting`
                let mut deleting_transitives = HashMap::new();
                for (_uuid, e1) in all.iter_mut()
                    .filter(|e| deleting.contains(e.0))
                {
                    e1.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut deleting_transitives);
                }
                deleting.extend(deleting_transitives.into_values());

                // Calculate transitive closure
                let mut found_uuids = HashSet::new();
                loop {
                    for (_uuid, e1) in all.iter_mut().filter(|e| !deleting.contains(e.0)) {
                        if e1.delete_when(&deleting) {
                            let mut including_transitives = HashMap::new();
                            e1.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut including_transitives);
                            for (_m, v) in including_transitives {
                                found_uuids.insert(v);
                            }
                        }
                    }

                    if found_uuids.is_empty() {
                        break;
                    }
                    deleting.extend(found_uuids.drain());
                }
                deleting
            }
            let command = match command {
                InsensitiveCommand::DeleteSpecificElements(uuids, b)
                => InsensitiveCommand::DeleteSpecificElements(tr_closure(&mut self.temporaries.flattened_views, uuids), b),
                InsensitiveCommand::CutSpecificElements(uuids)
                => InsensitiveCommand::CutSpecificElements(tr_closure(&mut self.temporaries.flattened_views, uuids)),
                c => c,
            };

            let mut undo_accumulator = vec![];

            match &command {
                InsensitiveCommand::HighlightAll(..)
                | InsensitiveCommand::HighlightSpecific(..)
                | InsensitiveCommand::SelectByDrag(..)
                | InsensitiveCommand::MoveSpecificElements(..)
                | InsensitiveCommand::MoveAllElements(..)
                | InsensitiveCommand::ResizeSpecificElementsBy(..)
                | InsensitiveCommand::ResizeSpecificElementsTo(..) => {}
                InsensitiveCommand::AddDependency(t, b, pos, element, into_model) => {
                    if *t == *self.uuid && *b == 0 {
                        if let Ok(mut view) = element.clone().try_into()
                            && (!*into_model || self.adapter.insert_element(*b, *pos, view.model()).is_ok()){
                            let uuid = *view.uuid();
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(
                                *self.uuid,
                                *b,
                                uuid,
                                *into_model,
                            ));

                            if *into_model {
                                affected_models.insert(*self.adapter.model_uuid());
                            }
                            let mut model_transitives = HashMap::new();
                            view.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut model_transitives);
                            affected_models.extend(model_transitives.into_keys());

                            self.owned_views.push(uuid, view);
                        }
                    }
                }
                InsensitiveCommand::RemoveDependency(t, b, elm, from_model) => {
                    if *t == *self.uuid && *b == 0 {
                        for (_uuid, element) in self
                            .owned_views
                            .iter_event_order_pairs()
                            .filter(|e| e.0 == *elm)
                        {
                            let pos = if !*from_model {
                                None
                            } else if let Some((_b, pos)) = self.adapter.remove_element(&element.model_uuid()) {
                                Some(pos)
                            } else {
                                continue;
                            };
                            undo_accumulator.push(InsensitiveCommand::AddDependency(*self.uuid(), *b, pos, element.clone().into(), *from_model));
                        }
                        self.owned_views.retain(|k, _v| *k != *elm);
                    }
                }
                InsensitiveCommand::DeleteSpecificElements(uuids, _)
                | InsensitiveCommand::CutSpecificElements(uuids) => {
                    let from_model = matches!(
                        command,
                        InsensitiveCommand::DeleteSpecificElements(_, true) | InsensitiveCommand::CutSpecificElements(..)
                    );

                    for (_uuid, element) in self
                        .owned_views
                        .iter_event_order_pairs()
                        .filter(|e| uuids.contains(&e.0))
                    {
                        let pos = if !from_model {
                            None
                        } else if let Some((_b, pos)) = self.adapter.remove_element(&element.model_uuid()) {
                            Some(pos)
                        } else {
                            continue;
                        };
                        undo_accumulator.push(InsensitiveCommand::AddDependency(*self.uuid(), 0, pos, element.clone().into(), from_model));
                    }
                    self.owned_views.retain(|k, _v| !uuids.contains(k));
                }
                InsensitiveCommand::PasteSpecificElements(_, elements) => {
                    for element in elements {
                        if let Ok(mut view) = element.clone().try_into()
                            && let Ok(_) = self.adapter.insert_element(0, None, view.model()) {
                            let uuid = *view.uuid();
                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(uuid).collect(),
                                true,
                            ));
                            let mut model_transitives = HashMap::new();
                            view.head_count(&mut HashMap::new(), &mut HashMap::new(), &mut model_transitives);
                            affected_models.extend(model_transitives.into_keys());
                            self.owned_views.push(uuid, view);
                        }
                    }
                }
                InsensitiveCommand::ArrangeSpecificElements(uuids, arr) => {
                    self.owned_views.apply_arrangement(uuids, *arr);
                },
                InsensitiveCommand::PropertyChange(uuids, _property) => {
                    if uuids.is_empty() || uuids.contains(&*self.uuid) {
                        self.adapter.apply_property_change_fun(
                            &self.uuid,
                            &command,
                            &mut undo_accumulator,
                        );
                        affected_models.insert(*self.adapter.model_uuid());
                    }
                }
            }

            self.owned_views.event_order_foreach_mut(|v|
                v.apply_command(&command, &mut undo_accumulator, affected_models)
            );

            let modifies_selection = match command {
                InsensitiveCommand::HighlightAll(..)
                | InsensitiveCommand::HighlightSpecific(..)
                | InsensitiveCommand::SelectByDrag(..)
                | InsensitiveCommand::DeleteSpecificElements(..)
                | InsensitiveCommand::CutSpecificElements(..)
                | InsensitiveCommand::PasteSpecificElements(..) => true,
                InsensitiveCommand::MoveSpecificElements(..)
                | InsensitiveCommand::MoveAllElements(..)
                | InsensitiveCommand::ResizeSpecificElementsBy(..)
                | InsensitiveCommand::ResizeSpecificElementsTo(..)
                | InsensitiveCommand::ArrangeSpecificElements(..)
                | InsensitiveCommand::AddDependency(..)
                | InsensitiveCommand::RemoveDependency(..)
                | InsensitiveCommand::PropertyChange(..) => false,
            };

            if !undo_accumulator.is_empty() {
                if clear_redo_stack {
                    self.temporaries.redo_stack.clear();
                }
                if save_to_undo_stack {
                    if let Some(merged) = self.temporaries.undo_stack.last()
                        .filter(|_| self.temporaries.last_change_flag)
                        .and_then(|e| e.0.merge(&command))
                    {
                        let last = self.temporaries.undo_stack.last_mut().unwrap();
                        last.0 = merged;
                        let unique_prop_changes: Vec<_> = last
                            .1
                            .iter()
                            .chain(undo_accumulator.iter())
                            .fold(Vec::new(), |mut uniques, e| {
                                if let InsensitiveCommand::PropertyChange(uuids, properties) = e {
                                    for property in properties {
                                        if uniques
                                            .iter()
                                            .find(|(u, p)| {
                                                *u == uuids
                                                    && std::mem::discriminant(*p)
                                                        == std::mem::discriminant(property)
                                            })
                                            .is_none()
                                        {
                                            uniques.push((uuids, property));
                                        }
                                    }
                                }
                                uniques
                            })
                            .into_iter()
                            .map(|(u, c)| {
                                InsensitiveCommand::PropertyChange(u.clone(), vec![c.clone()])
                            })
                            .collect();
                        last.1.extend(undo_accumulator);
                        last.1
                            .retain(|e| !matches!(e, InsensitiveCommand::PropertyChange(_uuids, _x)));
                        last.1.extend(unique_prop_changes);
                    } else {
                        global_undo_accumulator.push(command.info_text());
                        self.temporaries.undo_stack.push((command, undo_accumulator));
                    }
                }
            }

            if modifies_selection {
                self.head_count();
            }
        }
    }

    fn some_kind_of_copy(
        &self,
        new_adapter: DiagramAdapterT,
        models: HashMap<ModelUuid, DomainT::CommonElementT>
    ) -> ERef<dyn DiagramController> {
        Self::new(
            ViewUuid::now_v7().into(),
            format!("{} (copy)", self.name).into(),
            new_adapter,
            Self::elements_deep_copy(
                None,
                |_| false,
                models,
                self.owned_views.iter_event_order_pairs().map(|e| (e.0, e.1.clone())),
            ).into_iter().map(|e| e.1).collect(),
        )
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> Entity for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> View for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.adapter.model_uuid()
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> TopLevelView for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn view_name(&self) -> Arc<String> {
        self.name.clone()
    }

    fn set_view_name(&mut self, new_name: Arc<String>) {
        self.temporaries.name_buffer = (*new_name).clone();
        self.name = new_name;
    }

    fn view_type(&self) -> String {
        self.adapter.view_type().to_owned()
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> DiagramController for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn new_hierarchy_view(&self) -> Arc<dyn ModelHierarchyView> {
        Arc::new(SimpleModelHierarchyView::new(self.adapter.model(), self.temporaries.label_provider.clone()))
    }

    fn represented_models(&self) -> &HashMap<ModelUuid, ViewUuid> {
        &self.temporaries.flattened_represented_models
    }

    fn refresh_buffers(&mut self, affected_models: &HashSet<ModelUuid>) {
        // TODO: only do head_count when new model was added
        self.head_count();

        let mut lp = self.temporaries.label_provider.write();
        if affected_models.contains(&self.adapter.model_uuid()) {
            self.adapter.refresh_buffers();
            lp.insert(*self.adapter.model_uuid(), self.adapter.model_name());
        }

        for mk in affected_models.iter() {
            if let Some(vk) = self.temporaries.flattened_represented_models.get(mk)
                && let Some(v) = self.temporaries.flattened_views.get_mut(vk) {
                v.refresh_buffers();
                lp.update(&v.model());
            }
        }
    }

    fn new_ui_canvas(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>) {
        let canvas_pos = ui.next_widget_position();
        let canvas_size = ui.available_size();
        let canvas_rect = egui::Rect::from_min_size(canvas_pos, canvas_size);

        let (painter_response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let ui_canvas = UiCanvas::new(
            true,
            painter,
            canvas_rect,
            self.temporaries.camera_offset,
            self.temporaries.camera_scale,
            ui.ctx().pointer_interact_pos().map(|e| {
                ((e - self.temporaries.camera_offset - painter_response.rect.min.to_vec2()) / self.temporaries.camera_scale)
                    .to_pos2()
            }),
            Highlight::ALL,
        );
        ui_canvas.clear(self.adapter.background_color(&context.global_colors));
        ui_canvas.draw_gridlines(
            Some((50.0, self.adapter.gridlines_color(&context.global_colors))),
            Some((50.0, self.adapter.gridlines_color(&context.global_colors))),
        );

        let inner_mouse = ui
            .ctx()
            .pointer_interact_pos()
            .filter(|e| canvas_rect.contains(*e))
            .map(|e| {
                ((e - self.temporaries.camera_offset - canvas_pos.to_vec2()) / self.temporaries.camera_scale).to_pos2()
            });

        self.temporaries.last_interactive_canvas_rect = egui::Rect::from_min_size(self.temporaries.camera_offset / -self.temporaries.camera_scale, canvas_size / self.temporaries.camera_scale);

        (Box::new(ui_canvas), painter_response, inner_mouse)
    }
    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        modifier_settings: ModifierSettings,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! pos_to_abs {
            ($pos:expr) => {
                (($pos - self.temporaries.camera_offset - response.rect.min.to_vec2()) / self.temporaries.camera_scale).to_pos2()
            };
        }

        // Handle mouse_down/drag/click/mouse_up
        let modifiers = ui.input(|i| ModifierKeys::from_egui(&i.modifiers));
        ui.input(|is| is.events.iter()
            .for_each(|e| match e {
                egui::Event::PointerButton { pos, button, pressed, .. } if *pressed && *button == egui::PointerButton::Primary => {
                    self.temporaries.last_unhandled_mouse_pos = Some(pos_to_abs!(*pos));
                    self.handle_event(InputEvent::MouseDown(pos_to_abs!(*pos)), modifier_settings, modifiers, element_setup_modal, undo_accumulator, affected_models);
                },
                _ => {}
            })
        );
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(old_pos) = self.temporaries.last_unhandled_mouse_pos {
                let delta = response.drag_delta() / self.temporaries.camera_scale;
                self.handle_event(InputEvent::Drag { from: old_pos, delta }, modifier_settings, modifiers, element_setup_modal, undo_accumulator, affected_models);
                self.temporaries.last_unhandled_mouse_pos = Some(old_pos + delta);
            }
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.handle_event(InputEvent::Click(pos_to_abs!(pos)), modifier_settings, modifiers, element_setup_modal, undo_accumulator, affected_models);
            }
        }
        ui.input(|is| is.events.iter()
            .for_each(|e| match e {
                egui::Event::PointerButton { pos, button, pressed, .. } if !*pressed && *button == egui::PointerButton::Primary => {
                    self.handle_event(InputEvent::MouseUp(pos_to_abs!(*pos)), modifier_settings, modifiers, element_setup_modal, undo_accumulator, affected_models);
                    self.temporaries.last_unhandled_mouse_pos = None;
                },
                _ => {}
            })
        );

        // Handle diagram drag
        if response.dragged_by(egui::PointerButton::Middle) {
            self.temporaries.camera_offset += response.drag_delta();
        }

        // Handle diagram zoom
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta);

            let factor = if scroll_delta.y > 0.0 && self.temporaries.camera_scale < 10.0 {
                1.5
            } else if scroll_delta.y < 0.0 && self.temporaries.camera_scale > 0.01 {
                0.66
            } else {
                0.0
            };

            if factor != 0.0 {
                if let Some(cursor_pos) = ui.ctx().pointer_interact_pos() {
                    let old_factor = self.temporaries.camera_scale;
                    self.temporaries.camera_scale *= factor;
                    self.temporaries.camera_offset -=
                        ((cursor_pos - self.temporaries.camera_offset - response.rect.min.to_vec2())
                            / old_factor)
                            * (self.temporaries.camera_scale - old_factor);
                }
            }
        }
    }
    fn cancel_tool(&mut self) {
        self.temporaries.current_tool = None;
    }
    fn context_menu(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        macro_rules! translate {
            ($msg_name:expr) => {
                context.fluent_bundle.format_pattern(
                    context.fluent_bundle.get_message($msg_name).unwrap().value().unwrap(),
                    None,
                    &mut vec![],
                )
            };
        }
        macro_rules! shortcut_text {
            ($ui:expr, $simple_project_command:expr) => {
                context.shortcuts.get(&$simple_project_command).map(|e| $ui.ctx().format_shortcut(&e))
            };
        }
        macro_rules! button {
            ($ui:expr, $msg_name:expr, $simple_project_command:expr) => {
                {
                    let mut button = egui::Button::new(translate!($msg_name));
                    if let Some(shortcut_text) = shortcut_text!($ui, $simple_project_command) {
                        button = button.shortcut_text(shortcut_text);
                    }
                    if $ui.add(button).clicked() {
                        commands.push($simple_project_command.into());
                        $ui.close();
                    }
                }
            };
        }

        ui.set_min_width(crate::MIN_MENU_WIDTH);

        button!(ui, "nh-edit-cut", SimpleProjectCommand::from(DiagramCommand::CutSelectedElements));
        button!(ui, "nh-edit-copy", SimpleProjectCommand::from(DiagramCommand::CopySelectedElements));
        button!(ui, "nh-edit-paste", SimpleProjectCommand::from(DiagramCommand::PasteClipboardElements));
        ui.separator();

        ui.menu_button(translate!("nh-edit-delete"), |ui| {
            ui.set_min_width(crate::MIN_MENU_WIDTH);

            button!(ui, "nh-generic-deletemodel-view", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(false)));
            button!(ui, "nh-generic-deletemodel-modelif", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(true)));
            button!(ui, "nh-generic-deletemodel-all", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(true)));
        });
        ui.separator();

        button!(ui, "nh-edit-clearhighlight", SimpleProjectCommand::from(DiagramCommand::HighlightAllElements(false, Highlight::ALL)));
        ui.menu_button(translate!("nh-edit-arrange"), |ui| {
            ui.set_min_width(crate::MIN_MENU_WIDTH);

            button!(ui, "nh-edit-arrange-bringtofront", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::BringToFront)));
            button!(ui, "nh-edit-arrange-forwardone", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::ForwardOne)));
            button!(ui, "nh-edit-arrange-backwardone", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::BackwardOne)));
            button!(ui, "nh-edit-arrange-sendtoback", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::SendToBack)));
        });
    }

    fn show_toolbar(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) {
        self.adapter.show_tool_palette(&mut self.temporaries.current_tool, context, ui);
    }
    fn show_properties(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) -> Option<Box<dyn CustomModal>> {
        let mut commands = Vec::new();
        let req = 'req: {
            let queryable = DomainT::QueryableT::new(&self.temporaries.flattened_represented_models, &self.temporaries.flattened_views);
            let lp = self.temporaries.label_provider.read();

            let child = self
                .owned_views
                .event_order_find_mut(|v| v.show_properties(context, &queryable, &lp, ui, &mut commands).to_non_default());
            if let Some(child) = child {
                child
            } else {
                ui.label("View properties:");
                ui.label("Name:");
                if ui
                    .add_sized(
                        (ui.available_width(), 20.0),
                        egui::TextEdit::singleline(&mut self.temporaries.name_buffer),
                    )
                    .changed()
                {
                    self.name = Arc::new(self.temporaries.name_buffer.clone());
                }
                match self.adapter.show_view_props_fun(context, ui) {
                    PropertiesStatus::NotShown | PropertiesStatus::Shown => {},
                    a => break 'req a,
                }

                ui.add_space(super::views::VIEW_MODEL_PROPERTIES_BLOCK_SPACING);

                ui.label("Model properties:");
                self.adapter.show_props_fun(&self.uuid, ui, &mut commands);

                PropertiesStatus::Shown
            }
        };

        self.apply_commands(commands, undo_accumulator, true, true, affected_models);

        match req {
            PropertiesStatus::NotShown | PropertiesStatus::Shown => None,
            PropertiesStatus::ToolRequest(t) => {
                self.temporaries.current_tool = t;
                None
            }
            PropertiesStatus::PromptRequest(RequestType::ChangeColor(t, c)) => {
                #[derive(PartialEq)]
                enum MGlobalColorType {
                    None,
                    Local,
                    Global,
                }
                struct ColorChangeModal {
                    diagram_uuid: ViewUuid,
                    diagram_color_slot: u8,
                    selected_color_type: MGlobalColorType,
                    local_color: egui::Color32,
                    global_color: uuid::Uuid,
                    new_global_color_name: String,
                }
                impl CustomModal for ColorChangeModal {
                    fn show(
                        &mut self,
                        d: &mut GlobalDrawingContext,
                        ui: &mut egui::Ui,
                        commands: &mut Vec<ProjectCommand>,
                    ) -> CustomModalResult {
                        ui.style_mut().spacing.indent += 20.0;
                        ui.heading("Color picker");

                        ui.radio_value(&mut self.selected_color_type, MGlobalColorType::None, "No override");

                        ui.radio_value(&mut self.selected_color_type, MGlobalColorType::Local, "Local color");
                        ui.add_enabled_ui(self.selected_color_type == MGlobalColorType::Local, |ui| {
                            ui.indent("local color", |ui| {
                                egui::widgets::color_picker::color_picker_color32(
                                    ui,
                                    &mut self.local_color,
                                    egui::widgets::color_picker::Alpha::OnlyBlend
                                );
                            });
                        });

                        ui.radio_value(&mut self.selected_color_type, MGlobalColorType::Global, "Global color");
                        ui.add_enabled_ui(self.selected_color_type == MGlobalColorType::Global, |ui| {
                            ui.indent("global color", |ui| {
                                let gc = &mut d.global_colors;
                                for id in gc.colors_order.iter() {
                                    ui.horizontal(|ui| {
                                        if let Some(c) = gc.colors.get_mut(id) {
                                            egui::widgets::color_picker::color_edit_button_srgba(
                                                ui,
                                                &mut c.1,
                                                egui::widgets::color_picker::Alpha::OnlyBlend
                                            );

                                            let text = if *id == self.global_color {
                                                &format!("[{}]", c.0)
                                            } else {
                                                &c.0
                                            };
                                            if ui.label(text).clicked() {
                                                self.global_color = *id;
                                            }
                                        }
                                    });
                                }

                                ui.horizontal(|ui| {
                                    let r = ui.text_edit_singleline(&mut self.new_global_color_name);

                                    if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Add new").clicked() {
                                        let new_uuid = uuid::Uuid::now_v7();
                                        gc.colors_order.push(new_uuid);
                                        gc.colors.insert(new_uuid, (std::mem::take(&mut self.new_global_color_name), egui::Color32::WHITE));
                                    }
                                });
                            });
                        });

                        ui.separator();

                        let mut result = CustomModalResult::KeepOpen;
                        ui.horizontal(|ui| {
                            let is_valid = match self.selected_color_type {
                                MGlobalColorType::Global => !self.global_color.is_nil(),
                                _ => true,
                            };
                            if ui.add_enabled(is_valid, egui::Button::new("Ok")).clicked() {
                                let c = match self.selected_color_type {
                                    MGlobalColorType::None => MGlobalColor::None,
                                    MGlobalColorType::Local => MGlobalColor::Local(self.local_color),
                                    MGlobalColorType::Global => MGlobalColor::Global(self.global_color),
                                };
                                commands.push(ProjectCommand::SimpleProjectCommand(
                                    SimpleProjectCommand::SpecificDiagramCommand(
                                        self.diagram_uuid,
                                        DiagramCommand::ColorSelected(self.diagram_color_slot, c),
                                    )
                                ));
                                result = CustomModalResult::CloseUnmodified;
                            }
                            if ui.button("Cancel").clicked() {
                                result = CustomModalResult::CloseUnmodified;
                            }
                        });

                        result
                    }
                }
                Some(Box::new(ColorChangeModal {
                    diagram_uuid: *self.uuid,
                    diagram_color_slot: t,
                    selected_color_type: match c {
                        MGlobalColor::None => MGlobalColorType::None,
                        MGlobalColor::Local(_color32) => MGlobalColorType::Local,
                        MGlobalColor::Global(_uuid) => MGlobalColorType::Global,
                    },
                    local_color: if let MGlobalColor::Local(color) = c { color } else { egui::Color32::WHITE },
                    global_color: if let MGlobalColor::Global(uuid) = c { uuid } else { uuid::Uuid::nil() },
                    new_global_color_name: String::new(),
                }))
            },
        }
    }
    fn show_menubar_edit_options(
        &mut self,
        _context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        if ui.button("Clear highlights").clicked() {
            commands.push(SimpleProjectCommand::SpecificDiagramCommand(
                *self.uuid,
                DiagramCommand::HighlightAllElements(false, Highlight::ALL),
            ).into());
        }
    }
    fn show_menubar_view_options(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) {
        macro_rules! translate {
            ($msg_name:expr) => {
                context.fluent_bundle.format_pattern(
                    context.fluent_bundle.get_message($msg_name).unwrap().value().unwrap(),
                    None,
                    &mut vec![],
                )
            };
        }

        if ui.button(translate!("nh-view-resetposition")).clicked() {
            self.temporaries.camera_offset = egui::Pos2::ZERO;
        }
        if ui.button(translate!("nh-view-resetscale")).clicked() {
            self.temporaries.camera_offset = self.temporaries.camera_offset / self.temporaries.camera_scale;
            self.temporaries.camera_scale = 1.0;
        }
        if ui.button(translate!("nh-view-zoomtofit")).clicked() {
            const PADDING: egui::Vec2 = egui::Vec2::splat(10.0);

            let mut mc = canvas::MeasuringCanvas::new(ui.painter());
            self.draw_in(context, &mut mc, None);

            let rect = mc.bounds();
            let ratio = self.temporaries.last_interactive_canvas_rect.size() * self.temporaries.camera_scale / (rect.size() + PADDING);
            self.temporaries.camera_scale = ratio.x.min(ratio.y);
            self.temporaries.camera_offset = rect.min * -self.temporaries.camera_scale + PADDING / 2.0;
        }
    }
    fn show_menubar_diagram_options(
        &mut self,
        _context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        self.adapter.menubar_options_fun(
            &*self.uuid,
            &(self.temporaries.label_provider.clone() as ERef<dyn LabelProvider>),
            ui,
            commands,
        );
    }

    fn apply_command(
        &mut self,
        command: DiagramCommand,
        global_undo: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match command {
            DiagramCommand::DropRedoStackAndLastChangeFlag => {
                self.temporaries.redo_stack.clear();
                self.temporaries.last_change_flag = false;
            },
            DiagramCommand::SetLastChangeFlag => {
                self.temporaries.last_change_flag = true;
            },
            DiagramCommand::UndoImmediate => {
                let Some((og_command, undo_commands)) = self.temporaries.undo_stack.pop() else {
                    return;
                };
                self.apply_commands(
                    undo_commands
                        .into_iter().rev()
                        .map(|c| c.into())
                        .collect(),
                    &mut vec![],
                    false,
                    false,
                    affected_models,
                );
                self.temporaries.redo_stack.push(og_command);
            },
            DiagramCommand::RedoImmediate => {
                let Some(redo_command) = self.temporaries.redo_stack.pop() else {
                    return;
                };
                self.apply_commands(vec![redo_command.into()], &mut vec![], true, false, affected_models);
            }
            DiagramCommand::InvertSelection => {
                self.apply_commands(vec![
                    InsensitiveCommand::HighlightAll(true, Highlight::SELECTED).into(),
                    InsensitiveCommand::HighlightSpecific(
                        self.temporaries.flattened_views_status.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect(),
                        false,
                        Highlight::SELECTED,
                    ).into()
                ], &mut vec![], true, false, affected_models);
            }
            DiagramCommand::DeleteSelectedElements(_)
            | DiagramCommand::CutSelectedElements
            | DiagramCommand::PasteClipboardElements
            | DiagramCommand::ArrangeSelected(_) => {
                if matches!(command, DiagramCommand::CutSelectedElements) {
                    self.set_clipboard_from_selected();
                }

                let mut undo = vec![];
                self.apply_commands(vec![
                    match command {
                        DiagramCommand::DeleteSelectedElements(b) => SensitiveCommand::DeleteSelectedElements(b),
                        DiagramCommand::CutSelectedElements => SensitiveCommand::CutSelectedElements,
                        DiagramCommand::PasteClipboardElements => SensitiveCommand::PasteClipboardElements,
                        DiagramCommand::ArrangeSelected(arr) => SensitiveCommand::ArrangeSelected(arr),
                        _ => unreachable!(),
                    }
                ], &mut undo, true, true, affected_models);
                self.temporaries.last_change_flag = true;
                global_undo.extend(undo.into_iter());
            }
            DiagramCommand::ColorSelected(slot, color) => {
                let ccd = ColorChangeData {
                    slot,
                    color,
                };
                let mut undo = vec![];
                self.apply_commands(vec![
                    SensitiveCommand::PropertyChangeSelected(vec![ccd.into()]),
                ], &mut undo, true, true, affected_models);
                self.temporaries.last_change_flag = true;
                global_undo.extend(undo.into_iter());
            }
            DiagramCommand::CopySelectedElements => {
                self.set_clipboard_from_selected();
            },
            DiagramCommand::HighlightAllElements(set, h) => {
                self.apply_commands(vec![InsensitiveCommand::HighlightAll(set, h).into()], &mut vec![], true, false, affected_models);
            },
            DiagramCommand::HighlightElement(e, set, h) => {
                let view_uuid = match e {
                    EntityUuid::Model(model_uuid) => self.temporaries.flattened_represented_models.get(&model_uuid).cloned(),
                    EntityUuid::View(view_uuid) => Some(view_uuid),
                };
                if let Some(view_uuid) = view_uuid {
                    self.apply_commands(
                        vec![InsensitiveCommand::HighlightSpecific(std::iter::once(view_uuid).collect(), set, h).into()],
                        &mut vec![], true, false, affected_models
                    );
                }
            },
            DiagramCommand::PanToElement(e, force) => {
                let view_uuid = match e {
                    EntityUuid::Model(model_uuid) => self.temporaries.flattened_represented_models.get(&model_uuid).cloned(),
                    EntityUuid::View(view_uuid) => Some(view_uuid),
                };
                if let Some(v) = view_uuid.and_then(|e| self.temporaries.flattened_views.get(&e)) {
                    let bb = v.bounding_box();
                    if force || !self.temporaries.last_interactive_canvas_rect.contains_rect(bb) {
                        self.temporaries.camera_scale = 1.0;
                        let lir = self.temporaries.last_interactive_canvas_rect.size() / 2.0;
                        let lir = egui::Pos2::new(lir.x.max(10.0), lir.y.max(10.0));
                        self.temporaries.camera_offset = lir - bb.center().to_vec2();
                    }
                }
            }
            DiagramCommand::CreateViewFor(model_uuid) => {
                if self.adapter.find_element(&model_uuid).is_some() {
                    let mut cmds = vec![];

                    // create all necessary views, such as parents or elements targetted by a link
                    {
                        let mut models_to_create_views_for = vec![model_uuid];
                        let mut pseudo_fv = self.temporaries.flattened_views.clone();
                        let mut pseudo_frm = self.temporaries.flattened_represented_models.clone();

                        loop {
                            let Some(model_uuid) = models_to_create_views_for.last().cloned() else {
                                break;
                            };
                            if pseudo_frm.contains_key(&model_uuid) {
                                models_to_create_views_for.pop();
                                continue;
                            }
                            let (model, parent_uuid) = self.adapter.find_element(&model_uuid).unwrap();
                            let Some(parent_view_uuid) = pseudo_frm.get(&parent_uuid).cloned() else {
                                models_to_create_views_for.push(parent_uuid);
                                continue;
                            };

                            let r = {
                                let q = DomainT::QueryableT::new(&pseudo_frm, &pseudo_fv);
                                self.adapter.create_new_view_for(&q, model.clone())
                            };

                            match r {
                                Ok(new_view) => {
                                    pseudo_fv.insert(*new_view.uuid(), new_view.clone());
                                    pseudo_frm.insert(*model.uuid(), *new_view.uuid());
                                    cmds.push(InsensitiveCommand::AddDependency(parent_view_uuid, 0, None, new_view.into(), false).into());
                                    models_to_create_views_for.pop();
                                },
                                Err(mut prerequisites) => models_to_create_views_for.extend(prerequisites.drain()),
                            }
                        }
                    }

                    // apply commands
                    let mut undo = vec![];
                    self.apply_commands(cmds, &mut undo, true, true, affected_models);
                    self.temporaries.last_change_flag = true;
                    global_undo.extend(undo.into_iter());
                }
            }
            DiagramCommand::DeleteViewFor(model_uuid, including_model) => {
                if let Some(view_uuid) = self.temporaries.flattened_represented_models.get(&model_uuid) {
                    let mut undo = vec![];
                    self.apply_commands(vec![
                        InsensitiveCommand::DeleteSpecificElements(std::iter::once(*view_uuid).collect(), including_model).into()
                    ], &mut undo, true, true, affected_models);
                    self.temporaries.last_change_flag = true;
                    global_undo.extend(undo.into_iter());
                }
            }
        }
    }

    fn draw_in(
        &mut self,
        context: &GlobalDrawingContext,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>
    ) {
        let tool = if let (Some(pos), Some(stage)) = (mouse_pos, self.temporaries.current_tool.as_ref()) {
            Some((pos, stage))
        } else {
            None
        };
        let mut drawn_targetting = TargettingStatus::NotDrawn;
        let queryable = DomainT::QueryableT::new(&self.temporaries.flattened_represented_models, &self.temporaries.flattened_views);

        self.owned_views.draw_order_foreach_mut(|v|
            if v.draw_in(&queryable, context, canvas, &tool) == TargettingStatus::Drawn {
                drawn_targetting = TargettingStatus::Drawn;
            }
        );

        if canvas.ui_scale().is_some() {
            if let Some((pos, tool)) = tool {
                if drawn_targetting == TargettingStatus::NotDrawn {
                    canvas.draw_rectangle(
                        egui::Rect::EVERYTHING,
                        egui::CornerRadius::ZERO,
                        tool.targetting_for_section(None),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                    self.owned_views.draw_order_foreach_mut(|v| {
                        v.draw_in(&queryable, context, canvas, &Some((pos, tool)));
                    });
                }
                tool.draw_status_hint(&queryable, canvas, pos);
            } else if let Some((a, b)) = self.temporaries.select_by_drag {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a, b),
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_rgba_premultiplied(0, 0, 255, 7),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLUE),
                    canvas::Highlight::NONE,
                );
            }

            self.temporaries.snap_manager.draw_best(canvas, egui::Color32::BLUE, self.temporaries.last_interactive_canvas_rect);
        }
    }

    fn deep_copy(&self) -> ERef<dyn DiagramController> {
        let (new_adapter, models) = self.adapter.deep_copy();
        self.some_kind_of_copy(new_adapter, models)
    }

    fn shallow_copy(&self) -> ERef<dyn DiagramController> {
        let (new_adapter, models) = self.adapter.fake_copy();
        self.some_kind_of_copy(new_adapter, models)
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> ContainerGen2<DomainT> for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<DomainT::CommonElementViewT> {
        self.temporaries.flattened_represented_models.get(uuid).and_then(|e| self.temporaries.flattened_views.get(e)).cloned()
    }
}
