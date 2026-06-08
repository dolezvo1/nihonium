use crate::common::canvas::{self, Highlight, NHCanvas, NHShape, UiCanvas};
use crate::common::search::FullTextSearchable;
use crate::common::ui_ext::UiExt;
use crate::common::uuid::ControllerUuid;
use crate::common::views::ordered_views::OrderedViewRefs;
use crate::{CustomModal, CustomModalResult, CustomTab, NHTab};
use eframe::egui;
use egui_ltreeview::DirPosition;
use fluent_bundle::FluentMessage;
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

    OpenAndFocusTab(NHTab, Option<egui::Pos2>),
    AddCustomTab(uuid::Uuid, Arc<RwLock<dyn CustomTab>>),
    SetNewDiagramNumber(u32),
    AddNewDiagram(/*parent:*/ViewUuid, ViewUuid, ERef<dyn DiagramController>),
    DeleteDiagram(ViewUuid),

    AddNewDocument(ViewUuid, String),
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
    OpenProject(bool),
    SaveProject,
    SaveProjectAs,
    CloseProject(bool),
    Exit(bool),
    SwapTopLanguages,
    CycleShadesProfiles,
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
    DeleteSelectedElements(Option<DeleteKind>),
    CutSelectedElements,
    CopySelectedElements,
    PasteClipboardElements(Option<ModelUuid>),
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
    Diagram(ViewUuid, ERef<dyn DiagramController>),
    Document(ViewUuid),
}

impl HierarchyNode {
    pub fn uuid(&self) -> ViewUuid {
        match self {
            Self::Folder(uuid, ..) => *uuid,
            Self::Diagram(uuid, ..) => *uuid,
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
    gdc: &GlobalDrawingContext,
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
                    egui::Stroke::new(1.0_f32, egui::Color32::RED),
                    egui::StrokeKind::Inside,
                );
                painter.line_segment(
                    [response.rect.left_top(), response.rect.right_bottom()],
                    egui::Stroke::new(1.0_f32, egui::Color32::RED),
                );
                painter.line_segment(
                    [response.rect.right_top(), response.rect.left_bottom()],
                    egui::Stroke::new(1.0_f32, egui::Color32::RED),
                );
                ui.label(gdc.translate_0("nh-modal-colorpicker-nooveridebrackets"));
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
                match gdc.global_colors.colors.get(&uuid) {
                    None => {
                        ui.label(gdc.translate_0("nh-modal-colorpicker-notfoundbrackets"));
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
    pub model_labels: LabelProvider,
}

impl GlobalDrawingContext {
    pub fn shortcut_text(&self, ui: &egui::Ui, c: SimpleProjectCommand) -> Option<String> {
        self.shortcuts
            .get(&c)
            .map(|e| ui.ctx().format_shortcut(&e))
    }

    pub fn get_message<'a, 'b>(&'a self, msg_name: &'b str) -> Result<FluentMessage<'a>, &'b str> {
        self.fluent_bundle.get_message(msg_name).ok_or(msg_name)
    }
    pub fn translate_0(&self, msg_name: &str) -> std::borrow::Cow<'_, str> {
        self.fluent_bundle.format_pattern(
            self.fluent_bundle.get_message(msg_name).unwrap().value().unwrap(),
            None,
            &mut vec![],
        )
    }
}

pub struct LabelProvider {
    labels: HashMap<ModelUuid, Arc<String>>,
}

impl LabelProvider {
    /// Clips string to reasonable size, replacing whitespaces with space
    pub fn filter_and_elipsis(src: &str) -> String {
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

    pub fn new() -> Self {
        Self { labels: HashMap::new(), }
    }

    pub fn get(&self, uuid: &ModelUuid) -> Arc<String> {
        self.labels.get(uuid).cloned().unwrap_or_else(|| format!("{:?}", uuid).into())
    }

    pub fn insert(&mut self, uuid: ModelUuid, label: Arc<String>) {
        self.labels.insert(uuid, label);
    }
}


pub trait View: Entity {
    fn uuid(&self) -> Arc<ViewUuid>;
    fn model_uuid(&self) -> Arc<ModelUuid>;
}

pub trait DiagramView: View {
    fn view_name(&self) -> Arc<String>;
    fn set_view_name(&mut self, new_name: Arc<String>);

    fn represented_models(&self) -> &HashMap<ModelUuid, ViewUuid>;
}

pub trait DiagramView2<DomainT: Domain>: DiagramView {
    fn model(&self) -> ERef<DomainT::DiagramModelT>;

    fn refresh_all_buffers(
        &mut self,
        label_provider: &mut LabelProvider,
    );
    fn refresh_buffers(
        &mut self,
        affected_models: &HashSet<ModelUuid>,
        label_provider: &mut LabelProvider,
    );

    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        modifier_settings: ModifierSettings,
        settings: &Box<dyn DiagramSettings>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn cancel_tool(&mut self);

    fn new_ui_canvas(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        interactive: bool,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);

    fn draw_in(
        &mut self,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
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
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    );
    fn show_properties(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> Option<Box<dyn CustomModal>>;
    fn show_outline(
        &mut self,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    );
    fn show_menubar_edit_options(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_menubar_view_options(
        &mut self,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_menubar_diagram_options(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );

    fn diagram_command_to_sensitives(
        &mut self,
        command: DiagramCommand,
        clipboard: &mut Vec<Box<dyn Any>>,
    ) -> Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>;
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn extend_models_for(&self, views: &HashSet<ViewUuid>, models: &mut HashSet<ModelUuid>);
    fn get_view_for(&self, model: &ModelUuid) -> Option<ViewUuid>;
    fn view_transitive_closure(&self, uuids: &mut HashSet<ViewUuid>);

    /// Create new view with a new model
    fn deep_copy(&self) -> ERef<Self>;
    /// Create new view with the same model
    fn shallow_copy(&self) -> ERef<Self>;
}

pub trait DiagramController: Any + NHContextSerialize {
    fn uuid(&self) -> Arc<ControllerUuid>;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn controller_type(&self) -> &'static str;
    fn view_uuids(&self) -> Vec<ViewUuid>;
    fn view_name(&self, uuid: &ViewUuid) -> Arc<String>;
    fn set_view_name(&self, uuid: &ViewUuid, new_name: Arc<String>);

    fn get(&self, uuid: &ViewUuid) -> Option<ERef<dyn DiagramView>>;
    fn refresh_all_buffers(
        &mut self,
        label_provider: &mut LabelProvider,
    );
    fn refresh_buffers(
        &mut self,
        affected_models: &HashSet<ModelUuid>,
        label_provider: &mut LabelProvider,
    );
    fn cancel_tool(&mut self);

    fn show_model_hierarchy(
        &mut self,
        uuid: &ViewUuid,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn handle_input(
        &mut self,
        uuid: &ViewUuid,
        ui: &mut egui::Ui,
        response: &egui::Response,
        modifier_settings: ModifierSettings,
        settings: &Box<dyn DiagramSettings>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn new_ui_canvas(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        interactive: bool,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);

    fn draw_in(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>,
    );

    fn context_menu(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn show_toolbar(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    );
    fn show_properties(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        affected_models: &mut HashSet<ModelUuid>,
    ) -> Option<Box<dyn CustomModal>>;
    fn show_outline(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    );
    fn show_menubar_edit_options(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_menubar_view_options(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_menubar_diagram_options(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_undo_stack(
        &self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );
    fn show_redo_stack(
        &self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );

    fn apply_diagram_command(
        &mut self,
        uuid: &ViewUuid,
        command: DiagramCommand,
        clipboard: &mut Vec<Box<dyn Any>>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn undo_immediate(
        &mut self,
        commands: &mut Vec<ProjectCommand>,
        affected_models: &mut HashSet<ModelUuid>,
    );
    fn redo_immediate(
        &mut self,
        commands: &mut Vec<ProjectCommand>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn show_duplication_menu(
        &mut self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        uuid: &ViewUuid,
    ) -> Option<(ViewUuid, Option<ERef<dyn DiagramController>>)>;

    fn full_text_search(&self, acc: &mut crate::common::search::Searcher);
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

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub enum DeleteKind {
    #[default]
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

/// Try merging a value with a newer one.
/// For values with relative semantics (e.g. relative position change) this should generally be the sum of the two values.
/// For values with absolute semantics (e.g. absolute position) this should generally return either the newer value or None.
pub trait TryMerge {
    fn try_merge(&self, newer: &Self) -> Option<Self> where Self: Sized;
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaletteEditingSelection {
    None,
    Group(uuid::Uuid),
    Tool(uuid::Uuid),
}

impl PaletteEditingSelection {
    pub fn uuid(&self) -> Option<&uuid::Uuid> {
        match self {
            Self::None => None,
            Self::Group(uuid) => Some(uuid),
            Self::Tool(uuid) => Some(uuid),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum PaletteEditBuffer<T: Clone, V: Clone> {
    None,
    Group(uuid::Uuid, String),
    Tool(uuid::Uuid, String, T, V),
}

impl<T: Clone, V: Clone> PaletteEditBuffer<T, V> {
    pub fn uuid(&self) -> Option<&uuid::Uuid> {
        match self {
            Self::None => None,
            Self::Group(uuid, ..) => Some(uuid),
            Self::Tool(uuid, ..) => Some(uuid),
        }
    }
}

pub struct ToolPalette<S: Clone, DomainT: Domain> {
    elements: Vec<(uuid::Uuid, String, Vec<(uuid::Uuid, S, String, DomainT::CommonElementViewT)>)>,
    selection: PaletteEditingSelection,
}

impl<S: Clone, DomainT: Domain> ToolPalette<S, DomainT> {
    pub fn new(elements: Vec<(&str, Vec<(S, &str, DomainT::CommonElementViewT)>)>) -> Self {
        let elements = elements.into_iter()
            .map(|e| {
                (
                    uuid::Uuid::now_v7(),
                    e.0.to_owned(),
                    e.1.into_iter().map(|e| {
                        (uuid::Uuid::now_v7(), e.0, e.1.to_owned(), e.2)
                    }).collect(),
                )
            })
            .collect();
        Self {
            elements,
            selection: PaletteEditingSelection::None,
        }
    }

    pub fn for_each_mut<F>(&mut self, f: F)
        where F: FnMut(&mut (uuid::Uuid, String, Vec<(uuid::Uuid, S, String, DomainT::CommonElementViewT)>)),
    {
        self.elements.iter_mut().for_each(f);
    }

    pub fn show_treeview(
        &mut self,
        _gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) {
        #[derive(Clone, Eq, Hash, PartialEq)]
        enum TreeElement {
            Root,
            Group(uuid::Uuid),
            Tool(uuid::Uuid),
        }

        enum TreeCommand {
            AddGroup(String),
            Duplicate(uuid::Uuid),
            Delete(uuid::Uuid),
        }
        let mut command = None;

        ui.label("Toolbar items");

        egui::ScrollArea::neither()
            .max_height(400.0)
            .show(ui, |ui| {
                let (_r, a) = egui_ltreeview::TreeView::new(ui.id().with("toolbar items"))
                    .allow_multi_selection(false)
                    .show(ui, |b| {
                        b.dir(TreeElement::Root, "Toolbar root");
                        for (group_id, group_label, elements) in &self.elements {
                            let add_options = |ui: &mut egui::Ui| {
                                ui.menu_button("Add element", |_ui| {
                                    // TODO: show possible element types
                                });
                                if ui.button("Add group").clicked() {
                                    return Some(TreeCommand::AddGroup(group_label.to_owned()));
                                }
                                None
                            };
                            let group_node = egui_ltreeview::NodeBuilder::dir(TreeElement::Group(*group_id))
                                .label(group_label)
                                .context_menu(|ui| {
                                    if ui.button("Edit").clicked() {

                                    }

                                    command = command.take().or(add_options(ui));

                                    if ui.add_enabled(elements.is_empty(), egui::Button::new("Delete")).clicked() {
                                        command = Some(TreeCommand::Delete(*group_id));
                                    }
                                });
                            b.node(group_node);

                            for (tool_id, _s, tool_label, _v) in elements {
                                let tool_node = egui_ltreeview::NodeBuilder::leaf(TreeElement::Tool(*tool_id))
                                    .label(tool_label)
                                    .context_menu(|ui| {
                                        if ui.button("Edit").clicked() {

                                        }

                                        command = command.take().or(add_options(ui));

                                        if ui.button("Duplicate").clicked() {
                                            command = Some(TreeCommand::Duplicate(*tool_id));
                                        }
                                        if ui.button("Delete").clicked() {
                                            command = Some(TreeCommand::Delete(*tool_id));
                                        }
                                    });
                                b.node(tool_node);
                            }
                            b.close_dir();
                        }
                        b.close_dir();
                    });
                for e in a {
                    if let egui_ltreeview::Action::SetSelected(e) = &e {
                        match e.first() {
                            Some(TreeElement::Group(id)) => {
                                self.selection = PaletteEditingSelection::Group(*id);
                            }
                            Some(TreeElement::Tool(id)) => {
                                self.selection = PaletteEditingSelection::Tool(*id);
                            }
                            _ => {
                                self.selection = PaletteEditingSelection::None;
                            }
                        }
                    }
                    if let egui_ltreeview::Action::Move(e) = e {
                        let egui_ltreeview::DragAndDrop { source, target, position, .. } = e;
                        let position = match position {
                            egui_ltreeview::DirPosition::First => egui_ltreeview::DirPosition::First,
                            egui_ltreeview::DirPosition::Last => egui_ltreeview::DirPosition::Last,
                            egui_ltreeview::DirPosition::Before(e) => match e {
                                TreeElement::Root => continue,
                                TreeElement::Group(e) | TreeElement::Tool(e) => egui_ltreeview::DirPosition::Before(e),
                            },
                            egui_ltreeview::DirPosition::After(e) => match e {
                                TreeElement::Root => continue,
                                TreeElement::Group(e) | TreeElement::Tool(e) => egui_ltreeview::DirPosition::After(e),
                            },
                        };

                        for src in source {
                            match src {
                                TreeElement::Root => continue,
                                TreeElement::Group(src) => {
                                    self.move_group(src, position);
                                },
                                TreeElement::Tool(src) => {
                                    let TreeElement::Group(target) = target else { continue; };
                                    self.move_tool(src, target, position);
                                },
                            }
                        }
                    }
                }
            });
        match command {
            None => {},
            Some(TreeCommand::AddGroup(name)) => {
                self.elements.push((uuid::Uuid::now_v7(), name, Vec::new()));
            }
            Some(TreeCommand::Duplicate(id)) => self.duplicate_tool(id),
            Some(TreeCommand::Delete(id)) => self.delete_node(id),
        }
    }
    pub fn get_selected(&self) -> PaletteEditingSelection {
        self.selection
    }
    pub fn get_buffer(&self, s: Option<uuid::Uuid>) -> PaletteEditBuffer<S, DomainT::CommonElementViewT> {
        let Some(id) = s else {
            return PaletteEditBuffer::None;
        };

        if let Some(e) = self.elements.iter().find(|e| e.0 == id) {
            return PaletteEditBuffer::Group(id, e.1.clone());
        }

        if let Some(e) = self.elements.iter().find_map(|e| e.2.iter().find(|e| e.0 == id)) {
            return PaletteEditBuffer::Tool(id, e.2.clone(), e.1.clone(), e.3.clone());
        }

        PaletteEditBuffer::None
    }
    pub fn set_from_buffer(&mut self, b: PaletteEditBuffer<S, DomainT::CommonElementViewT>) {
        match b {
            PaletteEditBuffer::None => {},
            PaletteEditBuffer::Group(uuid, name) => {
                for e in self.elements.iter_mut() {
                    if e.0 == uuid {
                        e.1 = name;
                        return;
                    }
                }
            },
            PaletteEditBuffer::Tool(uuid, name, tool, view) => {
                for e in self.elements.iter_mut().flat_map(|e| e.2.iter_mut()) {
                    if e.0 == uuid {
                        e.2 = name;
                        e.1 = tool;
                        e.3 = view;
                        return;
                    }
                }
            },
        }
    }

    fn move_group(&mut self, src: uuid::Uuid, pos: egui_ltreeview::DirPosition<uuid::Uuid>) {
        let Some(g) = self.elements.iter().position(|e| e.0 == src).map(|p| self.elements.remove(p)) else { return; };
        let pos = match pos {
            DirPosition::First => 0,
            DirPosition::Last => self.elements.len(),
            DirPosition::After(g2)
            | DirPosition::Before(g2) => {
                let idx_bonus = match pos {
                    DirPosition::After(_) => 1,
                    DirPosition::Before(_) => 0,
                    _ => unreachable!(),
                };

                self.elements.iter().position(|e| e.0 == g2 || e.2.iter().find(|e| e.0 == g2).is_some()).unwrap() + idx_bonus
            },
        };
        self.elements.insert(pos, g);
    }
    fn move_tool(&mut self, src: uuid::Uuid, target: uuid::Uuid, pos: egui_ltreeview::DirPosition<uuid::Uuid>) {
        let mut t = None;
        for (_, _, elements) in self.elements.iter_mut() {
            if let Some(pos) = elements.iter().position(|e| e.0 == src) {
                t = Some(elements.remove(pos));
                break;
            }
        }
        let Some(t) = t else { return; };

        let (_, _, elements) = self.elements.iter_mut().find(|e| e.0 == target).unwrap();
        let pos = match pos {
            DirPosition::First => 0,
            DirPosition::Last => elements.len(),
            DirPosition::After(t2)
            | DirPosition::Before(t2) => {
                let idx_bonus = match pos {
                    DirPosition::After(_) => 1,
                    DirPosition::Before(_) => 0,
                    _ => unreachable!(),
                };

                elements.iter().position(|e| e.0 == t2).unwrap() + idx_bonus
            },
        };
        elements.insert(pos, t);
    }
    fn duplicate_tool(&mut self, target: uuid::Uuid) {
        for (_, _, elements) in self.elements.iter_mut() {
            if let Some(e) = elements.iter().find(|e| e.0 == target) {
                let new_view = {
                    let (mut tlc, mut c, mut m) = Default::default();
                    e.3.deep_copy_clone(&|_| false, &mut tlc, &mut c, &mut m);
                    tlc.iter_mut().for_each(|e| {
                        e.1.deep_copy_relink(&c, &m);
                    });
                    tlc.get(&e.3.uuid()).cloned().unwrap()
                };

                let new_e = (uuid::Uuid::now_v7(), e.1.clone(), e.2.to_owned(), new_view);
                elements.push(new_e);
            }
        }
    }
    fn delete_node(&mut self, target: uuid::Uuid) {
        self.elements.retain(|e| e.0 != target);
        self.elements.iter_mut().for_each(|e| e.2.retain(|e| e.0 != target));
    }
}

pub trait DiagramSettings: Any {}
pub trait DiagramSettings2<DomainT: Domain>: DiagramSettings {
    fn palette_for_each_mut<'a, F>(&'a self, f: F)
        where F: FnMut(&mut (uuid::Uuid, String, Vec<(uuid::Uuid, <<DomainT as Domain>::ToolT as Tool<DomainT>>::Stage, String, DomainT::CommonElementViewT)>));
}


/// Index of a container partition. Note that 0 means "any owning partition"
/// and thus should not be used if container has multiple and/or non-owning buckets.
pub type BucketNoT = u8;
pub type PositionNoT = usize;
/// Selection insensitive command - inherently repeatable
#[derive(Clone, PartialEq, Debug)]
pub enum InsensitiveCommand<OrdinalMovementT: Clone + Debug, AddElementT: Clone + Debug, PropChangeT: TryMerge + Clone + Debug> {
    HighlightAll(/*set:*/ bool, Highlight),
    SelectByDrag(egui::Rect, bool),
    MovePositionalAll(egui::Vec2),

    HighlightSpecific(HashSet<ViewUuid>, /*set:*/ bool, Highlight),
    MovePositional(HashSet<ViewUuid>, egui::Vec2),
    MoveOrdinal(HashSet<ViewUuid>, OrdinalMovementT),
    ResizeSpecificElementsBy(HashSet<ViewUuid>, egui::Align2, egui::Vec2),
    ResizeSpecificElementsTo(HashSet<ViewUuid>, egui::Align2, egui::Vec2),
    DeleteSpecificElements(HashSet<ViewUuid>, DeleteKind),
    ArrangeSpecificElements(HashSet<ViewUuid>, Arrangement),
    AddDependency {
        target: ViewUuid,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: AddElementT,
        into_model: bool,
    },
    RemoveDependency {
        target: ViewUuid,
        bucket: BucketNoT,
        element: ViewUuid,
        including_model: bool,
    },
    PropertyChange(HashSet<ViewUuid>, PropChangeT),
    Macro(Arc<String>, usize, Arc<Vec<Self>>),
}

impl<OrdinalMovementT: Clone + Debug, AddElementT: Clone + Debug, PropChangeT: TryMerge + Clone + Debug>
    InsensitiveCommand<OrdinalMovementT, AddElementT, PropChangeT>
{
    fn info_text<'a, F, T>(
        &self,
        gdc: &'a GlobalDrawingContext,
        diagram_name: &str,
        f: F,
    ) -> T
        where F: FnOnce(&str) -> T,
    {
        let (msg, count) = match self {
            InsensitiveCommand::DeleteSpecificElements(uuids, b) => if *b == DeleteKind::DeleteView {
                (gdc.get_message("nh-viewcommand-deleteelementsfrom"), uuids.len())
            } else {
                (gdc.get_message("nh-viewcommand-deleteelements"), uuids.len())
            },
            InsensitiveCommand::MovePositional(uuids, ..)
            | InsensitiveCommand::MoveOrdinal(uuids, ..)
                => (gdc.get_message("nh-viewcommand-moveelements"), uuids.len()),
            InsensitiveCommand::MovePositionalAll(_delta)
                => (gdc.get_message("nh-viewcommand-moveallelements"), 0),
            InsensitiveCommand::ResizeSpecificElementsBy(uuids, _, _)
            | InsensitiveCommand::ResizeSpecificElementsTo(uuids, _, _)
                => (gdc.get_message("nh-viewcommand-resizeelements"), uuids.len()),
            InsensitiveCommand::ArrangeSpecificElements(uuids, _)
                => (gdc.get_message("nh-viewcommand-arrangeelements"), uuids.len()),
            InsensitiveCommand::AddDependency { into_model, .. } => if *into_model {
                (gdc.get_message("nh-viewcommand-addelements"), 1)
            } else {
                (gdc.get_message("nh-viewcommand-addelementsinto"), 1)
            },
            InsensitiveCommand::RemoveDependency { including_model, .. } => if *including_model {
                (gdc.get_message("nh-viewcommand-removeelements"), 1)
            } else {
                (gdc.get_message("nh-viewcommand-removeelementsfrom"), 1)
            },
            InsensitiveCommand::PropertyChange(uuids, ..)
                => (gdc.get_message("nh-viewcommand-modifyelements"), uuids.len()),
            InsensitiveCommand::Macro(msg, arg, _)
                => (gdc.get_message(&msg), *arg),
            InsensitiveCommand::HighlightAll(..) | InsensitiveCommand::HighlightSpecific(..) | InsensitiveCommand::SelectByDrag(..) => {
                unreachable!()
            }
        };

        match msg {
            Err(msg_name) => f(msg_name),
            Ok(msg) => {
                let pattern = msg.value().unwrap();
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("count", count);
                args.set("diagram", diagram_name);
                let mut errors = Vec::new();
                let msg = gdc.fluent_bundle.format_pattern(&pattern, Some(&args), &mut errors);
                f(&msg)
            },
        }
    }
}

impl<OrdinalMovementT: Clone + Debug, AddElementT: Clone + Debug, PropChangeT: TryMerge + Clone + Debug> TryMerge for InsensitiveCommand<OrdinalMovementT, AddElementT, PropChangeT>
{
    fn try_merge(&self, newer: &Self) -> Option<Self> {
        match (self, newer) {
            (
                InsensitiveCommand::MovePositional(uuids1, delta1),
                InsensitiveCommand::MovePositional(uuids2, delta2),
            ) if uuids1 == uuids2 => Some(InsensitiveCommand::MovePositional(
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
                InsensitiveCommand::PropertyChange(uuids1, change1),
                InsensitiveCommand::PropertyChange(uuids2, change2),
            ) if uuids1 == uuids2 => change1.try_merge(change2)
                .map(|e| InsensitiveCommand::PropertyChange(uuids1.clone(), e)),
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
    type SettingsT: DiagramSettings2<Self>;
    type CommonElementT: Model + VisitableElement + Clone;
    type DiagramModelT: ContainerModel<ElementT = Self::CommonElementT> + NHContextSerialize + NHContextDeserialize + VisitableDiagram + FullTextSearchable;
    type CommonElementViewT: ElementControllerGen2<Self> + serde::Serialize + NHContextSerialize + NHContextDeserialize + Clone;
    type ViewTargettingSectionT: Into<Self::CommonElementT>;
    type QueryableT<'a>: Queryable<'a, Self>;
    type ToolT: Tool<Self>;
    type OrdinalMovementT: Clone + Debug;
    type AddCommandElementT: From<Self::CommonElementViewT> + TryInto<Self::CommonElementViewT> + Clone + Debug;
    type PropChangeT: From<ColorChangeData> + TryInto<ColorChangeData> + TryMerge + Clone + Debug;
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
    fn get_element_pos(&self, _uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
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
        flattened_views: &'a HashMap<ViewUuid, (DomainT::CommonElementViewT, ViewUuid)>,
        flattened_views_status: &'a HashMap<ViewUuid, SelectionStatus>,
    ) -> Self;

    fn is_contained(&self, v: &ViewUuid, within: &ViewUuid) -> bool;
    fn are_siblings(&self, a: &ViewUuid, b: &ViewUuid) -> bool;
    fn find_parent<P>(&self, child: &ViewUuid, predicate: P) -> Option<(ViewUuid, DomainT::CommonElementViewT)>
        where P: FnMut(&ViewUuid, &DomainT::CommonElementViewT) -> bool;

    fn get_view_for(&self, m: &ModelUuid) -> Option<DomainT::CommonElementViewT>;
    fn selected_views(&self) -> HashSet<ViewUuid>;
}

pub struct GenericQueryable<'a, DomainT: Domain> {
    models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
    flattened_views: &'a HashMap<ViewUuid, (DomainT::CommonElementViewT, ViewUuid)>,
    flattened_views_status: &'a HashMap<ViewUuid, SelectionStatus>,
}

impl<'a, DomainT: Domain> Queryable<'a, DomainT> for GenericQueryable<'a, DomainT> {
    fn new(
        models_to_views: &'a HashMap<ModelUuid, ViewUuid>,
        flattened_views: &'a HashMap<ViewUuid, (DomainT::CommonElementViewT, ViewUuid)>,
        flattened_views_status: &'a HashMap<ViewUuid, SelectionStatus>,
    ) -> Self {
        Self { models_to_views, flattened_views, flattened_views_status }
    }

    fn is_contained(&self, v: &ViewUuid, within: &ViewUuid) -> bool {
        let mut v = *v;
        loop {
            let Some((_, parent)) = self.flattened_views.get(&v) else {
                return false;
            };
            if parent == within {
                return true;
            }
            v = *parent;
        }
    }
    fn are_siblings(&self, a: &ViewUuid, b: &ViewUuid) -> bool {
        self.flattened_views.get(a)
            .and_then(|(_, pa)| self.flattened_views.get(b).map(|(_, pb)| pa == pb))
            .unwrap_or(false)
    }
    fn find_parent<P>(&self, child: &ViewUuid, mut predicate: P) -> Option<(ViewUuid, DomainT::CommonElementViewT)>
        where P: FnMut(&ViewUuid, &DomainT::CommonElementViewT) -> bool,
    {
        let mut v = self.flattened_views.get(child)?.1;
        loop {
            let (parent, parent2) = self.flattened_views.get(&v)?;
            if predicate(&v, parent) {
                return Some((v, parent.clone()));
            }
            v = *parent2;
        }
    }

    fn get_view_for(&self, m: &ModelUuid) -> Option<DomainT::CommonElementViewT> {
        self.models_to_views.get(m).and_then(|e| self.flattened_views.get(e)).map(|e| e.0.clone())
    }

    fn selected_views(&self) -> HashSet<ViewUuid> {
        self.flattened_views_status.iter()
            .filter(|e| e.1.selected())
            .map(|e| *e.0)
            .collect()
    }
}

pub trait Tool<DomainT: Domain> {
    type Stage: Clone + PartialEq + 'static;

    fn new(uuid: uuid::Uuid, initial_stage: Self::Stage, repeat: bool) -> Self;
    fn initial_stage_uuid(&self) -> &uuid::Uuid;
    fn repeats(&self) -> bool;
    fn is_spent(&self) -> bool;

    fn targetting_for_section(&self, element: Option<DomainT::ViewTargettingSectionT>) -> egui::Color32;
    fn draw_status_hint(&self, q: &DomainT::QueryableT<'_>, canvas: &mut dyn NHCanvas, pos: egui::Pos2);

    fn add_position(&mut self, pos: egui::Pos2);
    fn add_section(&mut self, element: DomainT::ViewTargettingSectionT);

    fn try_additional_dependency(&mut self) -> Option<(BucketNoT, ModelUuid, ModelUuid)>;
    fn try_construct_view(
        &mut self,
        q: &DomainT::QueryableT<'_>,
        into: &ViewUuid,
    ) -> Option<(DomainT::CommonElementViewT, Option<Box<dyn CustomModal>>)>;

    fn reset_event_lock(&mut self);
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

pub trait ElementControllerGen2<DomainT: Domain>: ElementController<DomainT::CommonElementT> + NHContextSerialize + Send + Sync {
    fn show_properties(
        &mut self,
        _drawing_context: &GlobalDrawingContext,
        _q: &DomainT::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> PropertiesStatus<DomainT> {
        PropertiesStatus::NotShown
    }
    fn draw_in(
        &mut self,
        _q: &DomainT::QueryableT<'_>,
        context: &GlobalDrawingContext,
        settings: &DomainT::SettingsT,
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
        settings: &DomainT::SettingsT,
        q: &DomainT::QueryableT<'_>,
        tool: &mut Option<DomainT::ToolT>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> EventHandlingStatus;
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    );
    /// Refresh view's fields from model (recursing over children is unnecessary)
    fn refresh_buffers(&mut self);
    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, (DomainT::CommonElementViewT, ViewUuid)>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    );
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


pub trait ControllerAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + 'static {
    type DiagramViewT;

    fn model(&self) -> ERef<DomainT::DiagramModelT>;
    fn clone_with_model(&self, new_model: ERef<DomainT::DiagramModelT>) -> Self;
    fn controller_type(&self) -> &'static str;

    /// Must return all ModelUuids that are to be deleted, including children of deleted containers
    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid>;

    fn insert_element(&mut self, parent: ModelUuid, e: DomainT::CommonElementT, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()>;
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, DomainT::CommonElementT, BucketNoT, PositionNoT)>);

    fn show_add_shared_diagram_menu(&self, gdc: &GlobalDrawingContext, ui: &mut egui::Ui) -> Option<ERef<Self::DiagramViewT>>;
}

#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity, is_subset_with = Self::depends_on)]
pub struct MultiDiagramController<DomainT: Domain, AdapterT: ControllerAdapter<DomainT, DiagramViewT = DiagramViewT>, DiagramViewT>
where DiagramViewT: DiagramView2<DomainT> + NHContextSerialize + NHContextDeserialize + 'static
{
    uuid: Arc<ControllerUuid>,

    #[nh_context_serde(entity)]
    adapter: AdapterT,

    #[nh_context_serde(entity)]
    views: OrderedViewRefs<DiagramViewT>,

    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    undo_stack: Vec<(
        ViewUuid,
        InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        Vec<(ModelUuid, DomainT::CommonElementT, BucketNoT, PositionNoT)>,
    )>,
    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    redo_stack: Vec<(ViewUuid, InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>)>,

    #[serde(skip)]
    #[nh_context_serde(skip_and_default)]
    tree_view_state: egui_ltreeview::TreeViewState<ModelUuid>,
}

impl<DomainT: Domain, AdapterT: ControllerAdapter<DomainT, DiagramViewT = DiagramViewT>, DiagramViewT> MultiDiagramController<DomainT, AdapterT, DiagramViewT>
where DiagramViewT: DiagramView2<DomainT> + NHContextSerialize + NHContextDeserialize + 'static
{
    pub fn new(
        uuid: ControllerUuid,
        adapter: AdapterT,
        views: Vec<ERef<DiagramViewT>>,
    ) -> Self {
        Self {
            uuid: uuid.into(),
            adapter,
            views: OrderedViewRefs::new(views),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            tree_view_state: Default::default(),
        }
    }

    fn depends_on(&self) -> Vec<EntityUuid> {
        self.views.keys().map(|e| (*e).into()).collect()
    }

    fn recurse_delete(
        &self,
        view: &ERef<DiagramViewT>,
        mut c: InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        u: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        m: &mut HashSet<ModelUuid>,
    ) -> InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT> {
        match c {
            InsensitiveCommand::Macro(lbl, arg, cmds) => {
                let cmds = cmds.iter().map(|c| self.recurse_delete(view, c.clone(), u, m)).collect::<Vec<_>>();
                InsensitiveCommand::Macro(lbl, arg, cmds.into())
            }
            InsensitiveCommand::DeleteSpecificElements(ref mut uuids, delete_kind) => {
                let mut original_models = HashSet::new();
                self.views.draw_order_foreach(|e| e.extend_models_for(&uuids, &mut original_models));

                match delete_kind {
                    DeleteKind::DeleteView => {
                        let model_uuids = self.adapter.model_transitive_closure(original_models);
                        let r = view.read();
                        uuids.extend(model_uuids.iter().flat_map(|m| r.get_view_for(m)));
                    }
                    DeleteKind::DeleteModelIfOnlyView => {
                        let mut view_counts = HashMap::<ModelUuid, Vec<ViewUuid>>::new();

                        self.views.draw_order_foreach(|v| {
                            original_models.iter()
                                .for_each(|m| if let Some(v2) = v.get_view_for(m) {
                                    view_counts.entry(*m).or_default().push(v2);
                                });
                        });
                        m.extend(original_models.iter().filter(|e| view_counts.get(*e).is_none_or(|e| e.len() <= 1)).copied());
                        *m = self.adapter.model_transitive_closure(m.clone());
                        self.views.draw_order_foreach(|v| uuids.extend(m.iter().flat_map(|m| v.get_view_for(m))));
                    }
                    DeleteKind::DeleteAll => {
                        *m = self.adapter.model_transitive_closure(original_models);
                        self.views.draw_order_foreach(|v| uuids.extend(m.iter().flat_map(|m| v.get_view_for(m))));
                    }
                }
                self.views.draw_order_foreach(|v| v.view_transitive_closure(uuids));
                c
            },
            c => c,
        }
    }

    fn apply_commands(
        &mut self,
        view_uuid: &ViewUuid,
        commands: Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        push_to_undo_stack: bool,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        let view = self.views.get(view_uuid).cloned().unwrap();

        let mut changed = false;
        for mut c in commands {
            let mut undo_accumulator = Vec::new();
            let mut models_to_remove = HashSet::new();

            c = self.recurse_delete(&view, c, &mut undo_accumulator, &mut models_to_remove);

            if matches!(c, InsensitiveCommand::HighlightAll(..)
                            | InsensitiveCommand::SelectByDrag(..)
                            | InsensitiveCommand::MovePositionalAll(_)) {
                view.write().apply_command(&c, &mut undo_accumulator, affected_models);
            } else {
                self.views.draw_order_foreach_mut(|e| e.apply_command(&c, &mut undo_accumulator, affected_models));
            }

            let mut removed_models = Vec::new();
            if !models_to_remove.is_empty() {
                self.adapter.delete_elements(&models_to_remove, &mut removed_models);
            }

            if !undo_accumulator.is_empty() || !removed_models.is_empty() {
                if !changed {
                    self.redo_stack.clear();
                }
                if push_to_undo_stack {
                    'outer: {
                        let unmerged = 'unmerged: {
                            let Some(last) = self.undo_stack.last_mut().filter(|e| e.0 == *view_uuid) else {
                                break 'unmerged (*view_uuid, c, undo_accumulator, removed_models);
                            };
                            let Some(merged) = last.1.try_merge(&c) else {
                                break 'unmerged (*view_uuid, c, undo_accumulator, removed_models);
                            };
                            last.1 = merged;
                            last.2.extend(undo_accumulator);
                            last.3.extend(removed_models);
                            break 'outer;
                        };

                        self.undo_stack.push(unmerged);
                    }
                }
                changed = true;
            }
        }
    }
}

impl<DomainT: Domain, AdapterT: ControllerAdapter<DomainT, DiagramViewT = DiagramViewT>, DiagramViewT> Entity for MultiDiagramController<DomainT, AdapterT, DiagramViewT>
where DiagramViewT: DiagramView2<DomainT> + NHContextSerialize + NHContextDeserialize + 'static
{
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
    }
}

impl<DomainT: Domain, AdapterT: ControllerAdapter<DomainT, DiagramViewT = DiagramViewT>, DiagramViewT> DiagramController for MultiDiagramController<DomainT, AdapterT, DiagramViewT>
where DiagramViewT: DiagramView2<DomainT> + NHContextSerialize + NHContextDeserialize + 'static
{
    fn uuid(&self) -> Arc<ControllerUuid> {
        self.uuid.clone()
    }

    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.adapter.model().read().uuid()
    }

    fn controller_type(&self) -> &'static str {
        self.adapter.controller_type()
    }

    fn view_uuids(&self) -> Vec<ViewUuid> {
        self.views.keys().cloned().collect()
    }

    fn view_name(&self, uuid: &ViewUuid) -> Arc<String> {
        self.views.get(uuid).map(|e| e.read().view_name()).unwrap()
    }

    fn set_view_name(&self, uuid: &ViewUuid, new_name: Arc<String>) {
        let Some(view) = self.views.get(uuid).cloned() else {
            return;
        };
        view.write().set_view_name(new_name);
    }

    fn show_model_hierarchy(
        &mut self,
        uuid: &ViewUuid,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
        _affected_models: &mut HashSet<ModelUuid>,
    ) {
        let Some(view) = self.views.get(uuid).cloned() else {
            return;
        };

        struct HierarchyViewVisitor<'a, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            gdc: &'a GlobalDrawingContext,
            diagram_uuid: ViewUuid,
            commands: &'a mut Vec<ProjectCommand>,
            is_represented: &'a dyn Fn(ModelUuid) -> bool,
            builder: &'a mut egui_ltreeview::TreeViewBuilder<'ui, ModelUuid>,
            model: PhantomData<ModelT>,
        }
        impl<'a, 'ui, ModelT> HierarchyViewVisitor<'a, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            fn repr_glyph(&self, m: &ModelUuid) -> &'static str {
                if (self.is_represented)(*m) {"[x]"} else {"[ ]"}
            }
            fn show_element(&mut self, is_dir: bool, model_uuid: &ModelUuid) {
                macro_rules! push_dia {
                    ($c:expr) => {
                        self.commands.push(ProjectCommand::SimpleProjectCommand(
                            SimpleProjectCommand::SpecificDiagramCommand(
                                self.diagram_uuid,
                                $c,
                            )
                        ))
                    }
                }
                self.builder.node(
                    if is_dir {
                        egui_ltreeview::NodeBuilder::dir(*model_uuid).activatable(true)
                    } else {
                        egui_ltreeview::NodeBuilder::leaf(*model_uuid)
                    }.label(format!("{} {}", self.repr_glyph(model_uuid), self.gdc.model_labels.get(model_uuid)))
                        .context_menu(|ui| {
                            ui.set_min_width(crate::MIN_MENU_WIDTH);

                            let is_represented = (self.is_represented)(*model_uuid);
                            if is_represented {
                                if ui.button(self.gdc.translate_0("nh-tab-modelhierarchy-jumpto")).clicked() {
                                    let model_uuid = (*model_uuid).into();
                                    push_dia!(DiagramCommand::HighlightAllElements(false, Highlight::SELECTED));
                                    push_dia!(DiagramCommand::HighlightElement(model_uuid, true, Highlight::SELECTED));
                                    push_dia!(DiagramCommand::PanToElement(model_uuid, true));
                                    ui.close();
                                }
                                ui.separator();
                            }

                            if ui.button(self.gdc.translate_0("nh-edit-cut")).clicked() {
                                let model_uuid = (*model_uuid).into();
                                push_dia!(DiagramCommand::HighlightAllElements(false, Highlight::SELECTED));
                                push_dia!(DiagramCommand::HighlightElement(model_uuid, true, Highlight::SELECTED));
                                push_dia!(DiagramCommand::CutSelectedElements);
                                ui.close();
                            }

                            if ui.button(self.gdc.translate_0("nh-edit-copy")).clicked() {
                                let model_uuid = (*model_uuid).into();
                                push_dia!(DiagramCommand::HighlightAllElements(false, Highlight::SELECTED));
                                push_dia!(DiagramCommand::HighlightElement(model_uuid, true, Highlight::SELECTED));
                                push_dia!(DiagramCommand::CopySelectedElements);
                                ui.close();
                            }

                            if ui.button(self.gdc.translate_0("nh-edit-pastehere")).clicked() {
                                push_dia!(DiagramCommand::PasteClipboardElements(Some(*model_uuid)));
                                ui.close();
                            }

                            ui.separator();

                            if !is_represented && ui.button(self.gdc.translate_0("nh-tab-modelhierarchy-createview")).clicked() {
                                push_dia!(DiagramCommand::CreateViewFor(*model_uuid));
                                ui.close();
                            }

                            if is_represented && ui.button(self.gdc.translate_0("nh-tab-modelhierarchy-deleteview")).clicked() {
                                push_dia!(DiagramCommand::DeleteViewFor(*model_uuid, false));
                                ui.close();
                            }

                            if ui.button(self.gdc.translate_0("nh-tab-modelhierarchy-deletemodel")).clicked() {
                                push_dia!(DiagramCommand::DeleteViewFor(*model_uuid, true));
                                ui.close();
                            }
                        })
                );
            }
        }
        impl<'a, 'ui, ModelT> ElementVisitor<<ModelT as ContainerModel>::ElementT> for HierarchyViewVisitor<'a, 'ui, ModelT>
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
        impl<'a, 'ui, ModelT> DiagramVisitor<ModelT> for HierarchyViewVisitor<'a, 'ui, ModelT>
            where ModelT: VisitableDiagram,
                <ModelT as ContainerModel>::ElementT: VisitableElement,
        {
            fn open_diagram(&mut self, e: &ModelT) {
                let model_uuid = *e.uuid();
                self.builder.node(
                    egui_ltreeview::NodeBuilder::dir(model_uuid)
                        .label(&*self.gdc.model_labels.get(&model_uuid))
                        .context_menu(|ui| {
                            if ui.button("Paste here").clicked() {
                                self.commands.push(ProjectCommand::SimpleProjectCommand(
                                    SimpleProjectCommand::SpecificDiagramCommand(
                                        self.diagram_uuid,
                                        DiagramCommand::PasteClipboardElements(Some(model_uuid)),
                                    )
                                ));
                                ui.close();
                            }
                        })

                );
            }

            fn close_diagram(&mut self, _e: &ModelT) {
                self.builder.close_dir();
            }
        }

        let mut set_state = None;
        ui.horizontal(|ui| {
            if ui.button(gdc.translate_0("nh-tab-projecthierarchy-collapseall")).clicked() {
                set_state = Some(false);
            }
            if ui.button(gdc.translate_0("nh-tab-projecthierarchy-uncollapseall")).clicked() {
                set_state = Some(true);
            }
        });

        let (_r, a) = egui_ltreeview::TreeView::new(ui.make_persistent_id("model_hierarchy_view")).show_state(ui, &mut self.tree_view_state, |builder| {
            let r = view.read();
            let represented_models = r.represented_models();
            let is_represented = |e: ModelUuid| represented_models.contains_key(&e);

            let mut hvv = HierarchyViewVisitor {
                gdc,
                diagram_uuid: *uuid,
                commands,
                is_represented: &is_represented,
                builder,
                model: PhantomData,
            };

            self.adapter.model().read().accept(&mut hvv);
        });

        for e in a {
            if let egui_ltreeview::Action::Activate(activate) = e {
                let e = activate.selected[0].into();
                commands.extend([
                    DiagramCommand::HighlightAllElements(false, Highlight::SELECTED),
                    DiagramCommand::HighlightElement(e, true, Highlight::SELECTED),
                    DiagramCommand::PanToElement(e, true),
                ].map(|e| ProjectCommand::SimpleProjectCommand(SimpleProjectCommand::FocusedDiagramCommand(e))));
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

            self.adapter.model().read().accept(&mut StateChangeVisitor {
                set_open: b,
                state: &mut self.tree_view_state,
                model: PhantomData,
            });
        }
    }

    fn get(&self, uuid: &ViewUuid) -> Option<ERef<dyn DiagramView>> {
        self.views.get(uuid).map(|e| e.clone() as ERef<dyn DiagramView>)
    }

    fn refresh_all_buffers(
        &mut self,
        label_provider: &mut LabelProvider,
    ) {
        self.views.draw_order_foreach_mut(|e| e.refresh_all_buffers(label_provider));
    }

    fn refresh_buffers(
        &mut self,
        affected_models: &HashSet<ModelUuid>,
        label_provider: &mut LabelProvider,
    ) {
        self.views.draw_order_foreach_mut(|e| e.refresh_buffers(affected_models, label_provider));
    }

    fn handle_input(
        &mut self,
        uuid: &ViewUuid,
        ui: &mut egui::Ui,
        response: &egui::Response,
        modifier_settings: ModifierSettings,
        settings: &Box<dyn DiagramSettings>,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        let view = self.views.get(uuid).unwrap();
        let mut commands = Vec::new();
        view.write().handle_input(ui, response, modifier_settings, settings, element_setup_modal, &mut commands);
        self.apply_commands(uuid, commands, true, affected_models);
    }

    fn cancel_tool(&mut self) {
        self.views.draw_order_foreach_mut(|e| e.cancel_tool());
    }

    fn new_ui_canvas(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        interactive: bool,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>) {
        let view = self.views.get(uuid).unwrap();
        view.write().new_ui_canvas(context, ui, interactive)
    }

    fn draw_in(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().draw_in(context, settings, canvas, mouse_pos);
    }

    fn context_menu(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
        _affected_models: &mut HashSet<ModelUuid>,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().context_menu(context, ui, commands);
    }

    fn show_toolbar(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().show_toolbar(context, settings, ui);
    }

    fn show_properties(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        affected_models: &mut HashSet<ModelUuid>,
    ) -> Option<Box<dyn CustomModal>> {
        let view = self.views.get(uuid).unwrap();
        let mut commands = Vec::new();
        let r = view.write().show_properties(context, ui, &mut commands);
        self.apply_commands(uuid, commands, true, affected_models);
        r
    }

    fn show_outline(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().show_outline(context, settings, ui)
    }

    fn show_menubar_edit_options(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().show_menubar_edit_options(context, ui, commands);
    }

    fn show_menubar_view_options(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().show_menubar_view_options(context, settings, ui, commands);
    }

    fn show_menubar_diagram_options(
        &mut self,
        uuid: &ViewUuid,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        let view = self.views.get(uuid).unwrap();
        view.write().show_menubar_diagram_options(context, ui, commands);
    }

    fn apply_diagram_command(
        &mut self,
        uuid: &ViewUuid,
        command: DiagramCommand,
        clipboard: &mut Vec<Box<dyn Any>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        let view = self.views.get(uuid).unwrap();
        let commands = view.write().diagram_command_to_sensitives(command, clipboard);
        self.apply_commands(uuid, commands, true, affected_models);
    }

    fn undo_immediate(
        &mut self,
        commands: &mut Vec<ProjectCommand>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        self.cancel_tool();
        let Some((original_view, original_command, undo_commands, removed_models)) = self.undo_stack.pop() else {
            return;
        };
        for (parent, e, b, p) in removed_models {
            let _ = self.adapter.insert_element(parent, e, b, Some(p));
        }
        let redo_stack = std::mem::take(&mut self.redo_stack);
        self.apply_commands(
            &original_view,
            undo_commands
                .into_iter().rev()
                .map(|c| c.into())
                .collect(),
            false,
            affected_models,
        );
        self.redo_stack = redo_stack;
        self.redo_stack.push((original_view, original_command));
        commands.push(ProjectCommand::OpenAndFocusTab(NHTab::Diagram { uuid: original_view }, None));
    }
    fn redo_immediate(
        &mut self,
        commands: &mut Vec<ProjectCommand>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        self.cancel_tool();
        let Some((original_view, redo_command)) = self.redo_stack.pop() else {
            return;
        };
        let redo_stack = std::mem::take(&mut self.redo_stack);
        self.apply_commands(&original_view, vec![redo_command.into()], true, affected_models);
        self.redo_stack = redo_stack;
        commands.push(ProjectCommand::OpenAndFocusTab(NHTab::Diagram { uuid: original_view }, None));
    }

    fn show_undo_stack(
        &self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        let shortcut_text = gdc.shortcut_text(ui, DiagramCommand::UndoImmediate.into());

        if self.undo_stack.is_empty() {
            let mut button = egui::Button::new(gdc.translate_0("nh-edit-undo-nothingtoundo"));
            if let Some(shortcut_text) = shortcut_text {
                button = button.shortcut_text(shortcut_text);
            }
            let _ = ui.add_enabled(false, button);
        } else {
            for (ii, (v, c, _, _)) in self.undo_stack.iter().rev().enumerate() {
                let mut button = c.info_text(gdc, &self.views.get(v).unwrap().read().view_name(), |e| egui::Button::new(e));
                if let Some(shortcut_text) = shortcut_text.as_ref().filter(|_| ii == 0) {
                    button = button.shortcut_text(shortcut_text);
                }

                if ui.add(button).clicked() {
                    for _ in 0..=ii {
                        commands.push(SimpleProjectCommand::FocusedDiagramCommand(DiagramCommand::UndoImmediate).into());
                    }
                    break;
                }
            }
        }
    }

    fn show_redo_stack(
        &self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        let shortcut_text = gdc.shortcut_text(ui, DiagramCommand::RedoImmediate.into());

        if self.redo_stack.is_empty() {
            let mut button = egui::Button::new(gdc.translate_0("nh-edit-redo-nothingtoredo"));
            if let Some(shortcut_text) = shortcut_text {
                button = button.shortcut_text(shortcut_text);
            }
            let _ = ui.add_enabled(false, button);
        } else {
            for (ii, (v, c)) in self.redo_stack.iter().rev().enumerate() {
                let mut button = c.info_text(gdc, &self.views.get(v).unwrap().read().view_name(), |e| egui::Button::new(e));
                if let Some(shortcut_text) = shortcut_text.as_ref().filter(|_| ii == 0) {
                    button = button.shortcut_text(shortcut_text);
                }

                if ui.add(button).clicked() {
                    for _ in 0..=ii {
                        commands.push(SimpleProjectCommand::FocusedDiagramCommand(DiagramCommand::RedoImmediate).into());
                    }
                    break;
                }
            }
        }
    }

    fn show_duplication_menu(
        &mut self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        uuid: &ViewUuid,
    ) -> Option<(ViewUuid, Option<ERef<dyn DiagramController>>)> {
        if ui.button(gdc.translate_0("nh-tab-projecthierarchy-duplicate")).clicked() {
            let view = self.views.get(uuid).unwrap();
            let new_view = view.read().deep_copy();
            let new_view_uuid = *new_view.read().uuid();
            let new_view_model = new_view.read().model();
            return Some((
                new_view_uuid,
                Some(ERef::new(
                    Self::new(
                        ControllerUuid::now_v7().into(),
                        self.adapter.clone_with_model(new_view_model),
                        vec![new_view],
                    )
                )),
            ));
        }
        if ui.button(gdc.translate_0("nh-tab-projecthierarchy-duplicateshared")).clicked() {
            let view = self.views.get(uuid).unwrap();

            // TODO: make undoable
            self.undo_stack.clear();

            let new_view = view.read().shallow_copy();
            let new_view_uuid = *new_view.read().uuid();
            self.views.push(new_view_uuid, new_view);
            return Some((new_view_uuid, None));
        }

        let response = ui.menu_button(gdc.translate_0("nh-tab-projecthierarchy-addnewshareddiagram"), |ui| {
            ui.set_min_width(crate::MIN_MENU_WIDTH);

            self.adapter.show_add_shared_diagram_menu(gdc, ui)
        });
        if let Some(new_diagram) = response.inner.flatten() {
            let new_uuid = *new_diagram.read().uuid();
            self.views.push(new_uuid, new_diagram);
            return Some((new_uuid, None));
        }

        None
    }

    fn full_text_search(&self, acc: &mut crate::common::search::Searcher) {
        acc.open_component(*self.model_uuid());
        self.adapter.model().read().full_text_search(acc);
        acc.close_component(self.views.keys().cloned().collect());
    }
}


pub trait DiagramAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + 'static {
    fn model(&self) -> ERef<DomainT::DiagramModelT>;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;

    fn find_element(&self, model_uuid: &ModelUuid) -> Option<(DomainT::CommonElementT, ModelUuid)> {
        self.model().read().find_element(model_uuid)
    }
    fn get_element_pos(&self, model_uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        self.model().read().get_element_pos(model_uuid)
    }
    fn get_element_pos_in(&self, parent: &ModelUuid, model_uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)>;
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
    fn label_for(&self, element: &DomainT::CommonElementT) -> Arc<String>;

    fn background_color(&self, global_colors: &ColorBundle) -> egui::Color32;
    fn gridlines_color(&self, global_colors: &ColorBundle) -> egui::Color32;
    fn enable_headers(&self) -> (bool, bool) {
        (false, false)
    }
    fn show_view_props_fun(
        &mut self,
        drawing_context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> PropertiesStatus<DomainT>;
    fn show_model_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn apply_property_change_fun(
        &mut self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn refresh_buffers(&mut self);
    fn menubar_options_fun(
        &self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    );

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DomainT::CommonElementT>);
    fn enumerate_models(&self) -> (Self, HashMap<ModelUuid, DomainT::CommonElementT>);
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

    flattened_views: HashMap<ViewUuid, (DomainT::CommonElementViewT, ViewUuid)>,
    flattened_views_status: HashMap<ViewUuid, SelectionStatus>,
    flattened_represented_models: HashMap<ModelUuid, ViewUuid>,
    _layers: Vec<bool>,

    camera_offset: egui::Pos2,
    camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_interactive_canvas_rect: egui::Rect,
    snap_manager: SnapManager,
    current_tool: Option<DomainT::ToolT>,
    select_by_drag: Option<(egui::Pos2, egui::Pos2)>,

    last_change_flag: bool,
}

impl<DomainT: Domain> Default for DiagramControllerGen2Temporaries<DomainT> {
    fn default() -> Self {
        Self {
            name_buffer: Default::default(),
            flattened_views: Default::default(),
            flattened_views_status: Default::default(),
            flattened_represented_models: Default::default(),
            _layers: Default::default(),
            camera_offset: Default::default(),
            camera_scale: 1.0,
            last_unhandled_mouse_pos: Default::default(),
            last_interactive_canvas_rect: egui::Rect::ZERO,
            snap_manager: Default::default(),
            current_tool: Default::default(),
            select_by_drag: Default::default(),
            last_change_flag: Default::default(),
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
        settings: &DomainT::SettingsT,
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands_accumulator: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> bool {
        // Collect alignment guides
        self.temporaries.snap_manager = SnapManager::new(self.temporaries.last_interactive_canvas_rect, egui::Vec2::splat(10.0 / self.temporaries.camera_scale));
        self.owned_views.event_order_foreach_mut(|v| v.collect_allignment(&mut self.temporaries.snap_manager));
        self.temporaries.snap_manager.sort_guidelines();

        // Handle events
        let mut commands = Vec::new();

        let ehc = EventHandlingContext {
            modifier_settings,
            modifiers,
            ui_scale: self.temporaries.camera_scale,
            all_elements: &self.temporaries.flattened_views_status,
            snap_manager: &self.temporaries.snap_manager,
        };
        let q = DomainT::QueryableT::new(
            &self.temporaries.flattened_represented_models,
            &self.temporaries.flattened_views,
            &self.temporaries.flattened_views_status,
        );

        let child = self.owned_views.event_order_find_mut(|v| {
            let r = v.handle_event(event, &ehc, settings, &q, &mut self.temporaries.current_tool, element_setup_modal, &mut commands);
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
                    commands.push(InsensitiveCommand::SelectByDrag(
                        egui::Rect::from_two_pos(a, b + delta),
                        ehc.modifier_settings.hold_selection.is_some_and(|e| ehc.modifiers.is_superset_of(e)),
                    ).into());
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
                if let Some((bucket, target_id, dependency_id)) = tool.as_mut().and_then(|e| e.try_additional_dependency()) {
                    if let (Some(target_view_id), Some((dependency_view, _)))
                        = (self.temporaries.flattened_represented_models.get(&target_id),
                            self.temporaries.flattened_represented_models.get(&dependency_id)
                            .and_then(|e| self.temporaries.flattened_views.get(e))) {
                        commands.push(InsensitiveCommand::AddDependency {
                            target: *target_view_id,
                            bucket,
                            position: None,
                            element: dependency_view.clone().into(),
                            into_model: true,
                        }.into());
                        handled = true;
                    };
                }
                if let Some((new_e, esm)) = tool.as_mut().and_then(|e| e.try_construct_view(&q, &self.uuid)) {
                    commands.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid(),
                        bucket: 0,
                        position: None,
                        element: DomainT::AddCommandElementT::from(new_e),
                        into_model: true,
                     }.into());
                    if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                        *element_setup_modal = esm;
                    }
                    handled = true;
                }
                self.temporaries.current_tool = tool;

                handled
            },
        };

        if matches!(event, InputEvent::Click(_)) {
            if let Some(t) = &self.temporaries.current_tool && t.is_spent() {
                self.temporaries.current_tool = None;
            }
            self.temporaries.current_tool.as_mut().map(|e| e.reset_event_lock());
        }

        commands_accumulator.extend(commands.into_iter());

        handled
    }

    fn set_clipboard_from_selected(&self, clipboard: &mut Vec<Box<dyn Any>>) {
        let selected = self.temporaries.flattened_views_status.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect();
        *clipboard = Self::elements_deep_copy(
            Some(&selected),
            |_| false,
            HashMap::new(),
            self.owned_views.iter_event_order_pairs().map(|e| (e.0, e.1.clone())),
        ).into_values().map(|e| Box::new(e) as Box<dyn Any>).collect();
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
            self.temporaries.flattened_views.insert(k, (v.clone(), *self.uuid));
        }
        self.temporaries.flattened_represented_models.insert(*self.adapter.model_uuid(), *self.uuid);
    }

    fn apply_command_inner(
        &mut self,
        command: &InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        match command {
            InsensitiveCommand::HighlightAll(..)
            | InsensitiveCommand::HighlightSpecific(..)
            | InsensitiveCommand::SelectByDrag(..)
            | InsensitiveCommand::MovePositional(..)
            | InsensitiveCommand::MovePositionalAll(..)
            | InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::MoveOrdinal(..) => {}
            InsensitiveCommand::AddDependency { target, bucket, position, element, into_model } => {
                if *target == *self.uuid && *bucket == 0 {
                    if let Ok(mut view) = element.clone().try_into()
                        && (!*into_model || self.adapter.insert_element(*bucket, *position, view.model()).is_ok()){
                        let uuid = *view.uuid();
                        undo_accumulator.push(InsensitiveCommand::RemoveDependency {
                            target: *self.uuid,
                            bucket: *bucket,
                            element: uuid,
                            including_model: *into_model,
                         });

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
            InsensitiveCommand::RemoveDependency { target, bucket, element, including_model } => {
                if *target == *self.uuid && *bucket == 0 {
                    for (_uuid, element) in self
                        .owned_views
                        .iter_event_order_pairs()
                        .filter(|e| e.0 == *element)
                    {
                        let pos = if !*including_model {
                            None
                        } else if let Some((_b, pos)) = self.adapter.remove_element(&element.model_uuid()) {
                            Some(pos)
                        } else {
                            continue;
                        };
                        undo_accumulator.push(InsensitiveCommand::AddDependency {
                            target: *self.uuid(),
                            bucket: *bucket,
                            position: pos,
                            element: element.clone().into(),
                            into_model: *including_model,
                        });
                    }
                    self.owned_views.retain(|k, _v| *k != *element);
                }
            }
            InsensitiveCommand::DeleteSpecificElements(uuids, delete_kind) => {
                for (_uuid, element) in self
                    .owned_views
                    .iter_event_order_pairs()
                    .filter(|e| uuids.contains(&e.0))
                {
                    let (b, pos) = if *delete_kind == DeleteKind::DeleteView {
                        (0, None)
                    } else if let Some((b, pos)) = self.adapter.get_element_pos(&element.model_uuid()) {
                        (b, Some(pos))
                    } else {
                        continue;
                    };
                    undo_accumulator.push(InsensitiveCommand::AddDependency {
                        target: *self.uuid(),
                        bucket: b,
                        position: pos,
                        element: element.clone().into(),
                        into_model: false,
                    });
                }
                self.owned_views.retain(|k, _v| !uuids.contains(k));
            }
            InsensitiveCommand::ArrangeSpecificElements(uuids, arr) => {
                self.owned_views.apply_arrangement(uuids, *arr);
            },
            InsensitiveCommand::PropertyChange(uuids, _property) => {
                if uuids.is_empty() || uuids.contains(&*self.uuid) {
                    affected_models.insert(*self.model_uuid());
                    self.adapter.apply_property_change_fun(
                        &self.uuid,
                        &command,
                        undo_accumulator,
                    );
                }
            }
            InsensitiveCommand::Macro(_, _, cmds) => {
                for e in cmds.iter() {
                    self.apply_command_inner(e, undo_accumulator, affected_models);
                }
            }
        }

        if !matches!(command, InsensitiveCommand::Macro(..)) {
            self.owned_views.event_order_foreach_mut(|v| {
                v.apply_command(&command, undo_accumulator, affected_models);
            });
        }

        let modifies_selection = match command {
            InsensitiveCommand::HighlightAll(..)
            | InsensitiveCommand::HighlightSpecific(..)
            | InsensitiveCommand::SelectByDrag(..)
            | InsensitiveCommand::DeleteSpecificElements(..) => true,
            InsensitiveCommand::MovePositional(..)
            | InsensitiveCommand::MovePositionalAll(..)
            | InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::MoveOrdinal(..)
            | InsensitiveCommand::ArrangeSpecificElements(..)
            | InsensitiveCommand::AddDependency { .. }
            | InsensitiveCommand::RemoveDependency { .. }
            | InsensitiveCommand::PropertyChange(..)
            | InsensitiveCommand::Macro(..) => false,
        };

        if modifies_selection {
            self.head_count();
        }
    }

    fn some_kind_of_copy(
        &self,
        new_adapter: DiagramAdapterT,
        models: HashMap<ModelUuid, DomainT::CommonElementT>,
    ) -> ERef<Self> {
        Self::new(
            ViewUuid::now_v7().into(),
            format!("{} (copy)", self.name).into(),
            new_adapter,
            Self::elements_deep_copy(
                None,
                |_| true,
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
> DiagramView for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn view_name(&self) -> Arc<String> {
        self.name.clone()
    }

    fn set_view_name(&mut self, new_name: Arc<String>) {
        self.temporaries.name_buffer = (*new_name).clone();
        self.name = new_name;
    }

    fn represented_models(&self) -> &HashMap<ModelUuid, ViewUuid> {
        &self.temporaries.flattened_represented_models
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> DiagramView2<DomainT> for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn model(&self) -> ERef<<DomainT as Domain>::DiagramModelT> {
        self.adapter.model()
    }

    fn refresh_buffers(
        &mut self,
        affected_models: &HashSet<ModelUuid>,
        lp: &mut LabelProvider,
    ) {
        // TODO: only do head_count when new model was added
        self.head_count();

        if affected_models.contains(&self.adapter.model_uuid()) {
            self.adapter.refresh_buffers();
            lp.insert(*self.adapter.model_uuid(), self.adapter.model_name());
        }

        for mk in affected_models.iter() {
            if let Some(vk) = self.temporaries.flattened_represented_models.get(mk)
                && let Some((v, _)) = self.temporaries.flattened_views.get_mut(vk) {
                v.refresh_buffers();
                lp.insert(*v.model_uuid(), self.adapter.label_for(&v.model()));
            }
        }
    }
    fn refresh_all_buffers(
        &mut self,
        label_provider: &mut LabelProvider,
    ) {
        // Full label_provider update
        struct V<'a, DomainT: Domain> {
            label_provider: &'a mut LabelProvider,
            label_f: &'a dyn Fn(&DomainT::CommonElementT) -> Arc<String>,
            domain: PhantomData<DomainT>,
        }

        impl<'a, DomainT: Domain> ElementVisitor<<DomainT as Domain>::CommonElementT> for V<'a, DomainT> {
            fn open_complex(&mut self, e: &<DomainT as Domain>::CommonElementT) {
                self.label_provider.insert(*e.uuid(), (self.label_f)(e));
            }
            fn close_complex(&mut self, _e: &<DomainT as Domain>::CommonElementT) {}
            fn visit_simple(&mut self, e: &<DomainT as Domain>::CommonElementT) {
                self.label_provider.insert(*e.uuid(), (self.label_f)(e));
            }
        }

        impl<'a, DomainT: Domain> DiagramVisitor<<DomainT as Domain>::DiagramModelT> for V<'a, DomainT> {
            fn open_diagram(&mut self, _e: &<DomainT as Domain>::DiagramModelT) {}
            fn close_diagram(&mut self, _e: &<DomainT as Domain>::DiagramModelT) {}
        }

        let mut v: V<DomainT> = V {
            label_provider,
            label_f: &|e| self.adapter.label_for(e),
            domain: PhantomData,
        };
        self.model().read().accept(&mut v);
        label_provider.insert(*self.adapter.model_uuid(), self.adapter.model_name());

        // Refresh buffers
        self.temporaries.name_buffer = (*self.name).clone();

        for (v, _) in self.temporaries.flattened_views.values_mut() {
            v.refresh_buffers();
        }
        self.adapter.refresh_buffers();
    }

    fn new_ui_canvas(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        interactive: bool,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>) {
        let canvas_pos = ui.next_widget_position();
        let canvas_size = ui.available_size();
        let canvas_rect = egui::Rect::from_min_size(canvas_pos, canvas_size);

        let (painter_response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let ui_canvas = UiCanvas::new(
            interactive,
            painter,
            canvas_rect,
            self.temporaries.camera_offset,
            self.temporaries.camera_scale,
            ui.ctx().pointer_interact_pos().map(|e| {
                ((e - self.temporaries.camera_offset - painter_response.rect.min.to_vec2()) / self.temporaries.camera_scale)
                    .to_pos2()
            }),
            Highlight::ALL,
            self.adapter.enable_headers(),
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
        settings: &Box<dyn DiagramSettings>,
        // TODO: remove, handle as a command
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) {
        let Some(settings) = (settings.as_ref() as &dyn Any).downcast_ref::<DomainT::SettingsT>() else { return; };

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
                    self.handle_event(InputEvent::MouseDown(pos_to_abs!(*pos)), modifier_settings, modifiers, settings, element_setup_modal, commands);
                },
                _ => {}
            })
        );
        if response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.multi_touch().is_none()) {
            if let Some(old_pos) = self.temporaries.last_unhandled_mouse_pos {
                let delta = response.drag_delta() / self.temporaries.camera_scale;
                self.handle_event(InputEvent::Drag { from: old_pos, delta }, modifier_settings, modifiers, settings, element_setup_modal, commands);
                self.temporaries.last_unhandled_mouse_pos = Some(old_pos + delta);
            }
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.handle_event(InputEvent::Click(pos_to_abs!(pos)), modifier_settings, modifiers, settings, element_setup_modal, commands);
            }
        }
        ui.input(|is| is.events.iter()
            .for_each(|e| match e {
                egui::Event::PointerButton { pos, button, pressed, .. } if !*pressed && *button == egui::PointerButton::Primary => {
                    self.handle_event(InputEvent::MouseUp(pos_to_abs!(*pos)), modifier_settings, modifiers, settings, element_setup_modal, commands);
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
        if response.hovered()
            && let Some(cursor_pos) = ui.ctx().pointer_interact_pos() {
            macro_rules! apply_zoom {
                ($factor:expr, $cursor_pos:expr) => {
                    let old_factor = self.temporaries.camera_scale;
                    self.temporaries.camera_scale *= $factor;
                    self.temporaries.camera_offset -=
                        (($cursor_pos - self.temporaries.camera_offset - response.rect.min.to_vec2()) / old_factor)
                        * (self.temporaries.camera_scale - old_factor);
                };
            }

            ui.input(|i| i.events.iter().for_each(|e| match e {
                egui::Event::MouseWheel { delta, .. } => {
                    let factor = if delta.y > 0.0 && self.temporaries.camera_scale < 10.0 {
                        1.5
                    } else if delta.y < 0.0 && self.temporaries.camera_scale > 0.01 {
                        0.66
                    } else {
                        0.0
                    };

                    if factor != 0.0 {
                        apply_zoom!(factor, cursor_pos);
                    }
                },
                _ => {},
            }));

            ui.input(|i| {
                if let Some(mti) = i.multi_touch() {
                    apply_zoom!(i.zoom_delta(), mti.center_pos);
                    self.temporaries.camera_offset += i.translation_delta();
                }
            });
        }
    }
    fn cancel_tool(&mut self) {
        self.temporaries.current_tool = None;
    }
    fn context_menu(
        &mut self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        macro_rules! button {
            ($ui:expr, $msg_name:expr, $simple_project_command:expr) => {
                {
                    let mut button = egui::Button::new(gdc.translate_0($msg_name));
                    if let Some(shortcut_text) = gdc.shortcut_text($ui, $simple_project_command) {
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
        button!(ui, "nh-edit-paste", SimpleProjectCommand::from(DiagramCommand::PasteClipboardElements(None)));
        ui.separator();

        ui.menu_button(gdc.translate_0("nh-edit-delete"), |ui| {
            ui.set_min_width(crate::MIN_MENU_WIDTH);

            button!(ui, "nh-generic-deletemodel-view", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(Some(DeleteKind::DeleteView))));
            button!(ui, "nh-generic-deletemodel-modelif", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(Some(DeleteKind::DeleteModelIfOnlyView))));
            button!(ui, "nh-generic-deletemodel-all", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(Some(DeleteKind::DeleteAll))));
        });
        ui.separator();

        button!(ui, "nh-edit-clearhighlight", SimpleProjectCommand::from(DiagramCommand::HighlightAllElements(false, Highlight::ALL)));
        ui.menu_button(gdc.translate_0("nh-edit-arrange"), |ui| {
            ui.set_min_width(crate::MIN_MENU_WIDTH);

            button!(ui, "nh-edit-arrange-bringtofront", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::BringToFront)));
            button!(ui, "nh-edit-arrange-forwardone", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::ForwardOne)));
            button!(ui, "nh-edit-arrange-backwardone", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::BackwardOne)));
            button!(ui, "nh-edit-arrange-sendtoback", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::SendToBack)));
        });
    }

    fn show_toolbar(
        &mut self,
        gdc: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    ) {
        let Some(settings) = (settings.as_ref() as &dyn Any).downcast_ref::<DomainT::SettingsT>() else { return; };

        let button_height = gdc.tool_palette_item_height as f32;
        let width = ui.available_width();
        let selected_background_color = if ui.style().visuals.dark_mode {
            egui::Color32::BLUE
        } else {
            egui::Color32::LIGHT_BLUE
        };
        let button_background_color = ui.style().visuals.extreme_bg_color;

        let stage = self.temporaries.current_tool.as_ref().map(|e| e.initial_stage_uuid()).cloned();
        let c = |s: &uuid::Uuid| -> egui::Color32 {
            if stage.as_ref().is_some_and(|e| *e == *s) {
                selected_background_color
            } else {
                button_background_color
            }
        };

        if ui
            .add_sized(
                [width, button_height],
                egui::Button::new(gdc.translate_0("nh-tab-toolbar-selectmove")).fill(if stage == None {
                    selected_background_color
                } else {
                    button_background_color
                }),
            )
            .clicked()
        {
            self.temporaries.current_tool = None;
        }
        ui.separator();

        let (empty_a, empty_b, empty_c) = (HashMap::new(), HashMap::new(), HashMap::new());
        let empty_q = DomainT::QueryableT::new(&empty_a, &empty_b, &empty_c);

        settings.palette_for_each_mut(|(gid, label, items)| {
            egui::CollapsingHeader::new(&*label)
                .id_salt(gid)
                .default_open(true)
                .show(ui, |ui| {
                    let width = ui.available_width();
                    for (tid, stage, name, view) in items.iter_mut() {
                        let response = ui.add_sized([width, button_height], egui::Button::new(&*name).fill(c(tid)));
                        if let Some(t) = &self.temporaries.current_tool && *t.initial_stage_uuid() == *tid {
                            ui.painter().text(
                                response.rect.right_bottom(),
                                egui::Align2::RIGHT_BOTTOM,
                                if t.repeats() { " ∞ " } else { " 1 " },
                                egui::FontId::proportional(20.0),
                                ui.style().visuals.text_color(),
                            );
                        }

                        if response.clicked() {
                            if let Some(t) = &self.temporaries.current_tool && *t.initial_stage_uuid() == *tid && t.repeats() {
                                self.temporaries.current_tool = None;
                            } else {
                                self.temporaries.current_tool = Some(DomainT::ToolT::new(*tid, stage.clone(), true));
                            }
                        }
                        if response.secondary_clicked() {
                            if let Some(t) = &self.temporaries.current_tool && *t.initial_stage_uuid() == *tid && !t.repeats() {
                                self.temporaries.current_tool = None;
                            } else {
                                self.temporaries.current_tool = Some(DomainT::ToolT::new(*tid, stage.clone(), false));
                            }
                        }

                        let icon_rect = egui::Rect::from_min_size(response.rect.min, egui::Vec2::splat(button_height));
                        let painter = ui.painter().with_clip_rect(icon_rect);
                        let mut mc = canvas::MeasuringCanvas::new(&painter);
                        view.draw_in(&empty_q, gdc, settings, &mut mc, &None);
                        let (scale, offset) = mc.scale_offset_to_fit(egui::Vec2::splat(button_height));
                        let mut c = canvas::UiCanvas::new(false, painter, icon_rect, offset, scale, None, Highlight::NONE, (false, false));
                        c.clear(egui::Color32::GRAY);
                        view.draw_in(&empty_q, gdc, settings, &mut c, &None);
                    }
                });
        });
    }
    fn show_properties(
        &mut self,
        context: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> Option<Box<dyn CustomModal>> {
        let req = 'req: {
            let queryable = DomainT::QueryableT::new(
                &self.temporaries.flattened_represented_models,
                &self.temporaries.flattened_views,
                &self.temporaries.flattened_views_status,
            );

            let child = self
                .owned_views
                .event_order_find_mut(|v| v.show_properties(context, &queryable, ui, commands).to_non_default());
            if let Some(child) = child {
                child
            } else {
                ui.label("View properties:");
                if ui.labeled_text_edit_singleline("Name:", &mut self.temporaries.name_buffer).changed() {
                    self.name = Arc::new(self.temporaries.name_buffer.clone());
                }
                match self.adapter.show_view_props_fun(context, ui) {
                    PropertiesStatus::NotShown | PropertiesStatus::Shown => {},
                    a => break 'req a,
                }

                ui.add_space(super::views::VIEW_MODEL_PROPERTIES_BLOCK_SPACING);

                ui.label("Model properties:");
                self.adapter.show_model_props_fun(&self.uuid, ui, commands);

                PropertiesStatus::Shown
            }
        };

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
                        gdc: &mut GlobalDrawingContext,
                        ui: &mut egui::Ui,
                        commands: &mut Vec<ProjectCommand>,
                    ) -> CustomModalResult {
                        ui.style_mut().spacing.indent += 20.0;
                        ui.heading(gdc.translate_0("nh-modal-colorpicker"));

                        ui.radio_value(&mut self.selected_color_type, MGlobalColorType::None, gdc.translate_0("nh-modal-colorpicker-nooverride"));

                        ui.radio_value(&mut self.selected_color_type, MGlobalColorType::Local, gdc.translate_0("nh-modal-colorpicker-localcolor"));
                        ui.add_enabled_ui(self.selected_color_type == MGlobalColorType::Local, |ui| {
                            ui.indent("local color", |ui| {
                                egui::widgets::color_picker::color_picker_color32(
                                    ui,
                                    &mut self.local_color,
                                    egui::widgets::color_picker::Alpha::OnlyBlend
                                );
                            });
                        });

                        ui.radio_value(&mut self.selected_color_type, MGlobalColorType::Global, gdc.translate_0("nh-modal-colorpicker-globalcolor"));
                        ui.add_enabled_ui(self.selected_color_type == MGlobalColorType::Global, |ui| {
                            ui.indent("global color", |ui| {
                                {
                                    let gc = &mut gdc.global_colors;
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
                                }

                                ui.horizontal(|ui| {
                                    let r = ui.text_edit_singleline(&mut self.new_global_color_name);

                                    if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(gdc.translate_0("nh-modal-colorpicker-addnewglobal")).clicked() {
                                        let new_uuid = uuid::Uuid::now_v7();
                                        gdc.global_colors.colors_order.push(new_uuid);
                                        gdc.global_colors.colors.insert(new_uuid, (std::mem::take(&mut self.new_global_color_name), egui::Color32::WHITE));
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
                            if ui.add_enabled(is_valid, egui::Button::new(gdc.translate_0("nh-generic-ok"))).clicked() {
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
                            if ui.button(gdc.translate_0("nh-generic-cancel")).clicked() {
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
    fn show_outline(
        &mut self,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        ui: &mut egui::Ui,
    ) {
        let mut measuring_canvas = canvas::MeasuringCanvas::new(ui.painter());
        self.draw_in(&context, settings, &mut measuring_canvas, None);
        let diagram_bounds = measuring_canvas.bounds();
        drop(measuring_canvas);

        let outline_size = ui.available_size();
        let camera_scale = (outline_size.x / diagram_bounds.width())
                            .min(outline_size.y / diagram_bounds.height())
                            .min(0.5);
        let remainders = outline_size / camera_scale - diagram_bounds.size();
        let diagram_bounds = diagram_bounds.expand2(remainders / 2.0);

        // Draw the outline
        let canvas_rect = egui::Rect::from_min_size(ui.next_widget_position(), outline_size);
        let (painter_response, painter) = ui.allocate_painter(outline_size, egui::Sense::click_and_drag());
        painter.rect(
            canvas_rect,
            egui::CornerRadius::ZERO,
            self.adapter.background_color(&context.global_colors),
            egui::Stroke::NONE,
            egui::StrokeKind::Middle,
        );
        let mut ui_canvas = UiCanvas::new(
            false,
            painter.clone(),
            canvas_rect,
            diagram_bounds.min * -camera_scale,
            camera_scale,
            None,
            crate::common::canvas::Highlight::NONE,
            (false, false),
        );
        self.draw_in(&context, settings, &mut ui_canvas, None);

        // Draw viewport location hint
        {
            let padding = 20.0 / camera_scale;
            let mut intersects = true;
            let licr = self.temporaries.last_interactive_canvas_rect;
            let range_x = if licr.min.x < diagram_bounds.max.x && licr.max.x > diagram_bounds.min.x {
                licr.min.x..=licr.max.x
            } else if licr.max.x <= diagram_bounds.min.x {
                intersects = false;
                diagram_bounds.min.x..=(diagram_bounds.min.x + padding)
            } else {
                intersects = false;
                (diagram_bounds.max.x - padding)..=diagram_bounds.max.x
            };
            let range_y = if licr.min.y < diagram_bounds.max.y && licr.max.y > diagram_bounds.min.y {
                licr.min.y..=licr.max.y
            } else if licr.max.y <= diagram_bounds.min.y {
                intersects = false;
                diagram_bounds.min.y..=(diagram_bounds.min.y + padding)
            } else {
                intersects = false;
                (diagram_bounds.max.y - padding)..=diagram_bounds.max.y
            };
            let stroke_color = if intersects {
                egui::Color32::BLUE
            } else {
                egui::Color32::RED
            };
            ui_canvas.draw_rectangle(
                egui::Rect::from_x_y_ranges(range_x, range_y),
                egui::CornerRadius::ZERO,
                stroke_color.gamma_multiply(0.3),
                canvas::Stroke::new_solid(1.0, stroke_color),
                crate::common::canvas::Highlight::NONE,
            );
        }

        // Handle events
        if (painter_response.clicked() || painter_response.dragged())
            && let Some(hover_pos) = ui.pointer_hover_pos() {
            let pos = (hover_pos - painter_response.rect.min.to_vec2()) / camera_scale + diagram_bounds.min.to_vec2();
            self.temporaries.camera_offset = pos * -self.temporaries.camera_scale + self.temporaries.last_interactive_canvas_rect.size() / 2.0 * self.temporaries.camera_scale;
        }
    }
    fn show_menubar_edit_options(
        &mut self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        if ui.button(gdc.translate_0("nh-edit-clearhighlight")).clicked() {
            commands.push(SimpleProjectCommand::SpecificDiagramCommand(
                *self.uuid,
                DiagramCommand::HighlightAllElements(false, Highlight::ALL),
            ).into());
        }
    }
    fn show_menubar_view_options(
        &mut self,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
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

        const PADDING: egui::Vec2 = egui::Vec2::splat(10.0);
        if ui.button(translate!("nh-view-resetposition")).clicked() {
            self.temporaries.camera_offset = egui::Pos2::ZERO;
        }
        if ui.button(translate!("nh-view-resetscale")).clicked() {
            self.temporaries.camera_offset = self.temporaries.camera_offset / self.temporaries.camera_scale;
            self.temporaries.camera_scale = 1.0;
        }
        if ui.button(translate!("nh-view-zoomtofit")).clicked() {
            let mut mc = canvas::MeasuringCanvas::new(ui.painter());
            self.draw_in(context, settings, &mut mc, None);

            let rect = mc.bounds();
            let ratio = self.temporaries.last_interactive_canvas_rect.size() * self.temporaries.camera_scale / (rect.size() + PADDING);
            self.temporaries.camera_scale = ratio.x.min(ratio.y);
            self.temporaries.camera_offset = rect.min * -self.temporaries.camera_scale + PADDING / 2.0;
        }
        if ui.button(translate!("nh-view-zoomtofitselected")).clicked() {
            let mut area = egui::Rect::NOTHING;
            for e in self.temporaries.flattened_views_status.iter()
                .filter(|e| *e.1 != SelectionStatus::NotSelected)
                .flat_map(|e| self.temporaries.flattened_views.get(e.0))
            {
                area = area.union(e.0.bounding_box());
            }

            if area.is_positive() {
                let ratio = self.temporaries.last_interactive_canvas_rect.size() * self.temporaries.camera_scale / (area.size() + PADDING);
                self.temporaries.camera_scale = ratio.x.min(ratio.y);
                self.temporaries.camera_offset = area.min * -self.temporaries.camera_scale + PADDING / 2.0;
            }
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
            ui,
            commands,
        );
    }

    fn diagram_command_to_sensitives(
        &mut self,
        command: DiagramCommand,
        clipboard: &mut Vec<Box<dyn Any>>,
    ) -> Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>> {
        macro_rules! se {
            () => {
                self.temporaries.flattened_views_status.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect()
            };
        }

        match command {
            DiagramCommand::DropRedoStackAndLastChangeFlag => {
                self.temporaries.last_change_flag = false;
            },
            DiagramCommand::SetLastChangeFlag => {
                self.temporaries.last_change_flag = true;
            },
            DiagramCommand::UndoImmediate
            | DiagramCommand::RedoImmediate => {}
            DiagramCommand::InvertSelection => {
                return vec![
                    InsensitiveCommand::HighlightAll(true, Highlight::SELECTED).into(),
                    InsensitiveCommand::HighlightSpecific(
                        se!(),
                        false,
                        Highlight::SELECTED,
                    ).into(),
                ];
            }
            DiagramCommand::DeleteSelectedElements(_)
            | DiagramCommand::CutSelectedElements
            | DiagramCommand::PasteClipboardElements(_)
            | DiagramCommand::ArrangeSelected(_) => {
                if matches!(command, DiagramCommand::CutSelectedElements) {
                    self.set_clipboard_from_selected(clipboard);
                }

                return match command {
                        DiagramCommand::DeleteSelectedElements(b)
                            => vec![InsensitiveCommand::DeleteSpecificElements(se!(), b.unwrap_or_default())],
                        DiagramCommand::CutSelectedElements => {
                            let se: HashSet<_> = se!();
                            vec![
                                InsensitiveCommand::Macro(
                                    "nh-viewcommand-cutelements".to_owned().into(),
                                    se.len(),
                                    vec![
                                        InsensitiveCommand::DeleteSpecificElements(se, DeleteKind::DeleteAll)
                                    ].into(),
                                )
                            ]
                        },
                        DiagramCommand::PasteClipboardElements(target) => {
                            let target = target.and_then(|e| self.get_view_for(&e)).unwrap_or(*self.uuid);

                            let mut cmds = Vec::new();
                            cmds.push(InsensitiveCommand::HighlightAll(false, canvas::Highlight::SELECTED));

                            let mut add_commands = Vec::new();
                            let elements = Self::elements_deep_copy(
                                None,
                                |_| true,
                                HashMap::new(),
                                clipboard.iter()
                                    .filter_map(|e| if let Some(e) = e.downcast_ref::<DomainT::CommonElementViewT>() {
                                        Some((*e.uuid(), e.clone()))
                                    } else { None }),
                            );
                            let mut new_elements_area = egui::Rect::NOTHING;
                            for (_k, v) in elements.iter() {
                                new_elements_area = new_elements_area.union(v.bounding_box());
                            }
                            let offset = if target == *self.uuid {
                                -self.temporaries.camera_offset.to_vec2() / self.temporaries.camera_scale
                            } else {
                                self.temporaries.flattened_views
                                    .get(&target).map(|e| e.0.bounding_box().min.to_vec2())
                                    .unwrap_or_default()
                            };
                            let (mut u, mut m) = Default::default();
                            let (mut a, mut b, mut frm) = Default::default();
                            for (_k, mut v) in elements.into_iter() {
                                v.apply_command(
                                    &InsensitiveCommand::MovePositionalAll(
                                        -new_elements_area.min.to_vec2()
                                        + offset
                                        + egui::Vec2::splat(10.0)
                                    ),
                                    &mut u,
                                    &mut m,
                                );
                                v.head_count(&mut a, &mut b, &mut frm);
                                add_commands.push(InsensitiveCommand::AddDependency {
                                    target,
                                    bucket: 0,
                                    position: None,
                                    element: v.into(),
                                    into_model: true,
                                });
                            }
                            cmds.push(InsensitiveCommand::Macro(
                                "nh-viewcommand-pasteelements".to_owned().into(),
                                frm.len(),
                                add_commands.into(),
                            ));

                            cmds
                        },
                        DiagramCommand::ArrangeSelected(arr) => vec![InsensitiveCommand::ArrangeSpecificElements(se!(), arr)],
                        _ => unreachable!(),
                    };
            }
            DiagramCommand::ColorSelected(slot, color) => {
                let ccd = ColorChangeData {
                    slot,
                    color,
                };
                return vec![InsensitiveCommand::PropertyChange(se!(), ccd.into())];
            }
            DiagramCommand::CopySelectedElements => {
                self.set_clipboard_from_selected(clipboard);
            },
            DiagramCommand::HighlightAllElements(set, h) => {
                return vec![InsensitiveCommand::HighlightAll(set, h).into()];
            },
            DiagramCommand::HighlightElement(e, set, h) => {
                let view_uuid = match e {
                    EntityUuid::Model(model_uuid) => self.temporaries.flattened_represented_models.get(&model_uuid).cloned(),
                    EntityUuid::View(view_uuid) => Some(view_uuid),
                    EntityUuid::Controller(_) => return vec![],
                };
                if let Some(view_uuid) = view_uuid {
                    return vec![
                        InsensitiveCommand::HighlightSpecific(std::iter::once(view_uuid).collect(), set, h).into()
                    ];
                }
            },
            DiagramCommand::PanToElement(e, force) => {
                let view_uuid = match e {
                    EntityUuid::Model(model_uuid) => self.temporaries.flattened_represented_models.get(&model_uuid).cloned(),
                    EntityUuid::View(view_uuid) => Some(view_uuid),
                    EntityUuid::Controller(_) => return vec![],
                };
                if let Some((v, _)) = view_uuid.and_then(|e| self.temporaries.flattened_views.get(&e)) {
                    let bb = v.bounding_box();
                    if force || !self.temporaries.last_interactive_canvas_rect.contains_rect(bb) {
                        let lir = self.temporaries.last_interactive_canvas_rect.size() / 2.0 * self.temporaries.camera_scale;
                        self.temporaries.camera_scale = 1.0;
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
                        let mut pseudo_fvs = self.temporaries.flattened_views_status.clone();

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
                            let Some((b, pos)) = self.adapter.get_element_pos_in(&parent_uuid, &model_uuid) else {
                                unreachable!()
                            };

                            let r = {
                                let q = DomainT::QueryableT::new(&pseudo_frm, &pseudo_fv, &pseudo_fvs);
                                self.adapter.create_new_view_for(&q, model.clone())
                            };

                            match r {
                                Ok(new_view) => {
                                    pseudo_fv.insert(*new_view.uuid(), (new_view.clone(), parent_view_uuid));
                                    pseudo_frm.insert(*model.uuid(), *new_view.uuid());
                                    pseudo_fvs.insert(*new_view.uuid(), SelectionStatus::NotSelected);
                                    cmds.push(InsensitiveCommand::AddDependency {
                                        target: parent_view_uuid,
                                        bucket: b,
                                        position: Some(pos),
                                        element: new_view.into(),
                                        into_model: false,
                                    }.into());
                                    models_to_create_views_for.pop();
                                },
                                Err(mut prerequisites) => models_to_create_views_for.extend(prerequisites.drain()),
                            }
                        }
                    }

                    return cmds;
                }
            }
            DiagramCommand::DeleteViewFor(model_uuid, including_model) => {
                if let Some(view_uuid) = self.temporaries.flattened_represented_models.get(&model_uuid) {
                    return vec![
                        InsensitiveCommand::DeleteSpecificElements(
                            std::iter::once(*view_uuid).collect(),
                            if !including_model { DeleteKind::DeleteView } else { DeleteKind::DeleteAll },
                        ).into(),
                    ];
                }
            }
        };
        vec![]
    }
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::OrdinalMovementT, DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        self.apply_command_inner(command, undo_accumulator, affected_models);
    }

    fn draw_in(
        &mut self,
        context: &GlobalDrawingContext,
        settings: &Box<dyn DiagramSettings>,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>
    ) {
        let Some(settings) = (settings.as_ref() as &dyn Any).downcast_ref::<DomainT::SettingsT>() else { return; };

        let tool = if let (Some(pos), Some(stage)) = (mouse_pos, self.temporaries.current_tool.as_ref()) {
            Some((pos, stage))
        } else {
            None
        };
        let mut drawn_targetting = TargettingStatus::NotDrawn;
        let queryable = DomainT::QueryableT::new(
            &self.temporaries.flattened_represented_models,
            &self.temporaries.flattened_views,
            &self.temporaries.flattened_views_status,
        );

        self.owned_views.draw_order_foreach_mut(|v|
            if v.draw_in(&queryable, context, settings, canvas, &tool) == TargettingStatus::Drawn {
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
                        v.draw_in(&queryable, context, settings, canvas, &Some((pos, tool)));
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

    fn extend_models_for(&self, views: &HashSet<ViewUuid>, models: &mut HashSet<ModelUuid>) {
        models.extend(
            views.iter()
                .flat_map(|e| self.temporaries.flattened_views.get(e))
                .map(|(v, _)| *v.model_uuid())
        );
    }
    fn get_view_for(&self, model: &ModelUuid) -> Option<ViewUuid> {
        self.temporaries.flattened_represented_models.get(model).copied()
    }
    fn view_transitive_closure(&self, uuids: &mut HashSet<ViewUuid>) {
        let mut temp = HashSet::new();
        loop {
            for (k, (v, _)) in &self.temporaries.flattened_views {
                if !uuids.contains(k) && v.delete_when(uuids) {
                    temp.insert(*k);
                }
            }
            if temp.is_empty() {
                break;
            }
            uuids.extend(temp.drain());
        }
    }

    fn deep_copy(&self) -> ERef<Self> {
        let (new_adapter, models) = self.adapter.deep_copy();
        self.some_kind_of_copy(new_adapter, models)
    }

    fn shallow_copy(&self) -> ERef<Self> {
        let (new_adapter, models) = self.adapter.enumerate_models();
        self.some_kind_of_copy(new_adapter, models)
    }
}
