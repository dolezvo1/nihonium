use crate::common::canvas::{self, NHCanvas, NHShape, UiCanvas};
use crate::CustomTab;
use eframe::{egui, epaint};
use egui_ltreeview::DirPosition;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

pub const BACKGROUND_COLORS: usize = 8;
pub const FOREGROUND_COLORS: usize = 8;
pub const AUXILIARY_COLORS: usize = 8;

#[derive(PartialEq)]
pub struct ColorProfile {
    pub name: String,
    pub backgrounds: [egui::Color32; BACKGROUND_COLORS],
    pub foregrounds: [egui::Color32; FOREGROUND_COLORS],
    pub auxiliary: [egui::Color32; AUXILIARY_COLORS],
}

/// Describes ColorProfile usage in a diagram
pub struct ColorLabels {
    pub backgrounds: [Option<String>; BACKGROUND_COLORS],
    pub foregrounds: [Option<String>; FOREGROUND_COLORS],
    pub auxiliary: [Option<String>; AUXILIARY_COLORS],
}

macro_rules! build_colors {
    ([$($profile_names:expr),* $(,)?],
     [$($pair_back:expr),* $(,)?],
     [$($pair_front:expr),* $(,)?],
     [$($pair_aux:expr),* $(,)?] $(,)?) => {{
        let mut vec_profiles = Vec::<crate::common::controller::ColorProfile>::new();

        for name in vec![$($profile_names),*] {
            vec_profiles.push(ColorProfile {
                name: name.to_string(),
                backgrounds: [egui::Color32::PLACEHOLDER; crate::common::controller::BACKGROUND_COLORS],
                foregrounds: [egui::Color32::PLACEHOLDER; crate::common::controller::FOREGROUND_COLORS],
                auxiliary: [egui::Color32::PLACEHOLDER; crate::common::controller::AUXILIARY_COLORS],
            });
        }

        let mut vec_labels_back = Vec::new();
        for (idx1, (label, values)) in vec![$($pair_back),*].into_iter().enumerate() {
            vec_labels_back.push(label);
            for (idx2, v) in values.into_iter().enumerate() {
                vec_profiles[idx2].backgrounds[idx1] = v;
            }
        }

        let mut vec_labels_front = Vec::new();
        for (idx1, (label, values)) in vec![$($pair_front),*].into_iter().enumerate() {
            vec_labels_front.push(label);
            for (idx2, v) in values.into_iter().enumerate() {
                vec_profiles[idx2].foregrounds[idx1] = v;
            }
        }

        let mut vec_labels_aux = Vec::new();
        for (idx1, (label, values)) in vec![$($pair_aux),*].into_iter().enumerate() {
            vec_labels_aux.push(label);
            for (idx2, v) in values.into_iter().enumerate() {
                vec_profiles[idx2].auxiliary[idx1] = v;
            }
        }

        let mut labels_back_iterator = vec_labels_back.into_iter()
            .take(crate::common::controller::BACKGROUND_COLORS).map(|e| Some(e.to_owned()));
        let mut labels_front_iterator = vec_labels_front.into_iter()
            .take(crate::common::controller::FOREGROUND_COLORS).map(|e| Some(e.to_owned()));
        let mut labels_aux_iterator = vec_labels_aux.into_iter()
            .take(crate::common::controller::AUXILIARY_COLORS).map(|e| Some(e.to_owned()));
        let labels = ColorLabels {
            backgrounds: std::array::from_fn(|_| {
                labels_back_iterator.next()
                    .unwrap_or_else(|| None)
            }),
            foregrounds: std::array::from_fn(|_| {
                labels_front_iterator.next()
                    .unwrap_or_else(|| None)
            }),
            auxiliary: std::array::from_fn(|_| {
                labels_aux_iterator.next()
                    .unwrap_or_else(|| None)
            }),
        };

        (labels, vec_profiles)
    }};
}
pub(crate) use build_colors;

use super::project_serde::{NHContextDeserialize, NHDeserializeError, NHDeserializer, NHContextSerialize, NHSerializeStore};
use super::uuid::{ModelUuid, ViewUuid};
use super::views::ordered_views::OrderedViews;
use super::entity::{Entity, EntityUuid};
use super::eref::ERef;
use super::ufoption::UFOption;


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

    pub fn draw_best(&self, canvas: &mut dyn NHCanvas, profile: &ColorProfile, rect: egui::Rect) {
        let (best_x, best_y) = *self.best_xy.read().unwrap();
        if let Some(bx) = best_x {
            canvas.draw_line([
                egui::Pos2::new(bx, rect.min.y), egui::Pos2::new(bx, rect.max.y)
            ], canvas::Stroke::new_solid(1.0, profile.auxiliary[0]), canvas::Highlight::NONE);
        }
        if let Some(by) = best_y {
            canvas.draw_line([
                egui::Pos2::new(rect.min.x, by), egui::Pos2::new(rect.max.x, by)
            ], canvas::Stroke::new_solid(1.0, profile.auxiliary[0]), canvas::Highlight::NONE);
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
    /// Open given diagram wherever
    OpenAndFocusDiagram(ViewUuid),
    /// Open and/or move diagram to be at given position
    OpenAndFocusDiagramAt(ViewUuid, egui::Pos2),
    AddCustomTab(uuid::Uuid, Arc<RwLock<dyn CustomTab>>),
    SetSvgExportMenu(Option<(usize, ERef<dyn DiagramController>, std::path::PathBuf, usize, bool, bool, f32, f32)>),
    SetNewDiagramNumber(u32),
    AddNewDiagram(usize, ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>),
    CopyDiagram(ViewUuid, /*deep:*/ bool),
    DeleteDiagram(ViewUuid),
}

impl From<SimpleProjectCommand> for ProjectCommand {
    fn from(value: SimpleProjectCommand) -> ProjectCommand {
        ProjectCommand::SimpleProjectCommand(value)
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub enum SimpleProjectCommand {
    DiagramCommand(DiagramCommand),
    OpenProject(bool),
    SaveProject,
    SaveProjectAs,
    CloseProject(bool),
    Exit(bool),
    SwapTopLanguages,
}

impl From<DiagramCommand> for SimpleProjectCommand {
    fn from(value: DiagramCommand) -> Self {
        SimpleProjectCommand::DiagramCommand(value)
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub enum DiagramCommand {
    DropRedoStackAndLastChangeFlag,
    SetLastChangeFlag,
    UndoImmediate,
    RedoImmediate,
    SelectAllElements(bool),
    InvertSelection,
    DeleteSelectedElements,
    CutSelectedElements,
    CopySelectedElements,
    PasteClipboardElements,
    ArrangeSelected(Arrangement),
    CreateViewFor(ModelUuid),
    DeleteViewFor(ModelUuid, /*including_model:*/ bool),
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub enum Arrangement {
    BringToFront,
    ForwardOne,
    BackwardOne,
    SendToBack,
}

pub enum HierarchyNode {
    Folder(ViewUuid, Arc<String>, Vec<HierarchyNode>),
    Diagram(ERef<dyn TopLevelView>),
}

impl HierarchyNode {
    pub fn uuid(&self) -> ViewUuid {
        match self {
            HierarchyNode::Folder(uuid, ..) => *uuid,
            HierarchyNode::Diagram(inner) => *inner.read().uuid(),
        }
    }

    pub fn collect_hierarchy(&self) -> HierarchyNode {
        match self {
            HierarchyNode::Folder(uuid, name, children) => {
                HierarchyNode::Folder(
                    *uuid,
                    name.clone(),
                    children.iter().map(|e| e.collect_hierarchy()).collect()
                )
            },
            HierarchyNode::Diagram(inner) => HierarchyNode::Diagram(inner.clone()),
        }
    }

    pub fn get(&self, id: &ViewUuid) -> Option<(&HierarchyNode, &HierarchyNode)> {
        let self_id = self.uuid();
        match self {
            HierarchyNode::Folder(.., children) => {
                for c in children {
                    if c.uuid() == *id {
                        return Some((c, self));
                    }
                    if let Some(e) = c.get(id) {
                        return Some(e);
                    }
                }
            }
            HierarchyNode::Diagram(..) => {}
        }
        None
    }
    pub fn remove(&mut self, id: &ViewUuid) -> Option<HierarchyNode> {
        match self {
            HierarchyNode::Folder(.., children) => {
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
            HierarchyNode::Diagram(..) => None,
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
            HierarchyNode::Folder(.., children) => {
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
            HierarchyNode::Diagram(..) => Err(value),
        }
    }
    pub fn for_each(&self, mut f: impl FnMut(&Self)) {
        f(self);
        match self {
            HierarchyNode::Folder(.., children) => {
                children.iter().for_each(f);
            },
            HierarchyNode::Diagram(..) => {},
        }
    }
}

pub trait ModelHierarchyView {
    fn show_model_hierarchy(&self, ui: &mut egui::Ui, is_represented: &dyn Fn(&ModelUuid) -> bool) -> Option<DiagramCommand>;
}

pub struct SimpleModelHierarchyView<ModelT: Model> {
    model: ERef<ModelT>,
}

impl<ModelT: Model> SimpleModelHierarchyView<ModelT> {
    pub fn new(model: ERef<ModelT>) -> Self {
        Self { model }
    }
}

impl<ModelT: Model> ModelHierarchyView for SimpleModelHierarchyView<ModelT> {
    fn show_model_hierarchy(&self, ui: &mut egui::Ui, is_represented: &dyn Fn(&ModelUuid) -> bool) -> Option<DiagramCommand> {
        struct HierarchyViewVisitor<'data, 'ui> {
            command: Option<DiagramCommand>,
            is_represented: &'data dyn Fn(&ModelUuid) -> bool,
            builder: &'data mut egui_ltreeview::TreeViewBuilder<'ui, ModelUuid>,
        }
        impl<'data, 'ui> HierarchyViewVisitor<'data, 'ui> {
            fn c(&self, m: &ModelUuid) -> &'static str {
                if (self.is_represented)(m) {"[x]"} else {"[ ]"}
            }
            fn show_model(&mut self, is_dir: bool, e: &dyn Model) {
                let model_uuid = *e.uuid();
                self.builder.node(
                    if is_dir {
                        egui_ltreeview::NodeBuilder::dir(model_uuid)
                    } else {
                        egui_ltreeview::NodeBuilder::leaf(model_uuid)
                    }.label(format!("{} {} ({})", self.c(&model_uuid), e.name(), model_uuid.to_string()))
                        .context_menu(|ui| {
                            if !(self.is_represented)(&model_uuid) && ui.button("Create view").clicked() {
                                self.command = Some(DiagramCommand::CreateViewFor(model_uuid));
                                ui.close();
                            }

                            if (self.is_represented)(&model_uuid) && ui.button("Delete view").clicked() {
                                self.command = Some(DiagramCommand::DeleteViewFor(model_uuid, false));
                                ui.close();
                            }

                            if ui.button("Delete model").clicked() {
                                self.command = Some(DiagramCommand::DeleteViewFor(model_uuid, true));
                                ui.close();
                            }
                        })
                );
            }
        }
        impl<'data, 'ui> StructuralVisitor<dyn Model> for HierarchyViewVisitor<'data, 'ui> {
            fn open_complex(&mut self, e: &dyn Model) {
                self.show_model(true, e);
            }

            fn close_complex(&mut self, e: &dyn Model) {
                self.builder.close_dir();
            }

            fn visit_simple(&mut self, e: &dyn Model) {
                self.show_model(false, e);
            }
        }

        let mut c = None;
        egui_ltreeview::TreeView::new(ui.make_persistent_id("model_hierarchy_view")).show(ui, |builder| {
            let mut hvv = HierarchyViewVisitor { command: None, is_represented, builder };

            self.model.read().accept(&mut hvv);

            c = hvv.command;
        });

        c
    }
}


const VIEW_MODEL_PROPERTIES_BLOCK_SPACING: f32 = 10.0;

pub struct DrawingContext<'a> {
    pub profile: &'a ColorProfile,
    pub fluent_bundle: &'a fluent_bundle::FluentBundle<fluent_bundle::FluentResource>,
}

pub trait View: Entity {
    fn uuid(&self) -> Arc<ViewUuid>;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;
}

pub trait TopLevelView: View {
    fn view_name(&self) -> Arc<String>;
    fn view_type(&self) -> String;
}

pub trait DiagramController: Any + TopLevelView + NHContextSerialize {
    fn represented_models(&self) -> &HashMap<ModelUuid, ViewUuid>;
    fn refresh_buffers(&mut self, affected_models: &HashSet<ModelUuid>);

    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    fn new_ui_canvas(
        &mut self,
        context: &DrawingContext,
        ui: &mut egui::Ui,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);

    fn draw_in(
        &mut self,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>,
    );

    fn context_menu(&mut self, ui: &mut egui::Ui);

    fn show_toolbar(
        &mut self,
        context: &DrawingContext,
        ui: &mut egui::Ui,
    );
    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    );
    fn show_layers(&self, ui: &mut egui::Ui);
    fn show_menubar_edit_options(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);
    fn show_menubar_diagram_options(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);

    fn apply_command(
        &mut self,
        command: DiagramCommand,
        global_undo: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    );

    /// Create new view with new model
    fn deep_copy(&self) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>);
    /// Create new view with the same model
    fn shallow_copy(&self) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>);
}

pub trait ElementController<CommonElementT>: View {
    fn model(&self) -> CommonElementT;

    fn min_shape(&self) -> NHShape;
    fn max_shape(&self) -> NHShape {
        self.min_shape()
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
        command: false,
        shift: false,
    };
    pub const COMMAND: Self = Self {
        alt: false,
        command: true,
        shift: false,
    };
    pub const SHIFT: Self = Self {
        alt: false,
        command: false,
        shift: true,
    };

    pub fn from_egui(source: &egui::Modifiers) -> Self {
        Self {
            alt: source.alt,
            command: source.command,
            shift: source.shift,
        }
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
pub enum SensitiveCommand<ElementT: Clone + Debug, PropChangeT: Clone + Debug> {
    MoveSelectedElements(egui::Vec2),
    ResizeSelectedElementsBy(egui::Align2, egui::Vec2),
    ResizeSelectedElementsTo(egui::Align2, egui::Vec2),
    DeleteSelectedElements(/*including_models:*/ bool),
    CutSelectedElements,
    PasteClipboardElements,
    ArrangeSelected(Arrangement),
    PropertyChangeSelected(Vec<PropChangeT>),
    Insensitive(InsensitiveCommand<ElementT, PropChangeT>)
}

impl<ElementT: Clone + Debug, PropChangeT: Clone + Debug> SensitiveCommand<ElementT, PropChangeT> {
    // TODO: I'm not sure whether this isn't actually the responsibility of the diagram itself
    fn to_selection_insensitive<F, G>(
        self,
        selected_elements: F,
        clipboard_elements: G,
    ) -> InsensitiveCommand<ElementT, PropChangeT>
    where
        F: Fn() -> HashSet<ViewUuid>,
        G: Fn() -> Vec<ElementT>
    {
        use SensitiveCommand as SC;
        use InsensitiveCommand as IC;
        if let SC::Insensitive(inner) = self {
            return inner;
        }
        if let SC::PasteClipboardElements = self {
            return IC::PasteSpecificElements(uuid::Uuid::nil().into(), clipboard_elements());
        }

        let se = selected_elements();
        match self {
            SC::MoveSelectedElements(delta) => IC::MoveSpecificElements(se, delta),
            SC::ResizeSelectedElementsBy(align, delta) => IC::ResizeSpecificElementsBy(se, align, delta),
            SC::ResizeSelectedElementsTo(align, delta) => IC::ResizeSpecificElementsTo(se, align, delta),
            SC::DeleteSelectedElements(including_models) => IC::DeleteSpecificElements(se, including_models),
            SC::CutSelectedElements => IC::CutSpecificElements(se),
            SC::ArrangeSelected(arr) => IC::ArrangeSpecificElements(se, arr),
            SC::PropertyChangeSelected(changes) => IC::PropertyChange(se, changes),
            SC::Insensitive(..) | SC::PasteClipboardElements => unreachable!(),
        }
    }
}

impl<ElementT: Clone + Debug, PropChangeT: Clone + Debug> From<InsensitiveCommand<ElementT, PropChangeT>> for SensitiveCommand<ElementT, PropChangeT> {
    fn from(value: InsensitiveCommand<ElementT, PropChangeT>) -> Self {
        Self::Insensitive(value)
    }
}

/// Selection insensitive command - inherently repeatable
#[derive(Clone, PartialEq, Debug)]
pub enum InsensitiveCommand<ElementT: Clone + Debug, PropChangeT: Clone + Debug> {
    SelectAll(bool),
    SelectSpecific(HashSet<ViewUuid>, bool),
    SelectByDrag(egui::Rect),
    MoveAllElements(egui::Vec2),
    MoveSpecificElements(HashSet<ViewUuid>, egui::Vec2),
    ResizeSpecificElementsBy(HashSet<ViewUuid>, egui::Align2, egui::Vec2),
    ResizeSpecificElementsTo(HashSet<ViewUuid>, egui::Align2, egui::Vec2),
    DeleteSpecificElements(HashSet<ViewUuid>, /*including_models:*/ bool),
    CutSpecificElements(HashSet<ViewUuid>),
    PasteSpecificElements(ViewUuid, Vec<ElementT>),
    ArrangeSpecificElements(HashSet<ViewUuid>, Arrangement),
    AddElement(ViewUuid, ElementT, /*into_model:*/ bool),
    PropertyChange(HashSet<ViewUuid>, Vec<PropChangeT>),
}

impl<ElementT: Clone + Debug, PropChangeT: Clone + Debug>
    InsensitiveCommand<ElementT, PropChangeT>
{
    fn info_text(&self) -> Arc<String> {
        match self {
            InsensitiveCommand::SelectAll(..) | InsensitiveCommand::SelectSpecific(..) | InsensitiveCommand::SelectByDrag(..) => {
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
            InsensitiveCommand::AddElement(..) => Arc::new(format!("Add 1 element")),
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

pub trait Domain: Sized + 'static {
    type CommonElementT: Model + Clone;
    type DiagramModelT: ContainerModel<ElementT = Self::CommonElementT>;
    type CommonElementViewT: ElementControllerGen2<Self> + serde::Serialize + NHContextSerialize + NHContextDeserialize + Clone;
    type QueryableT<'a>: Queryable<'a, Self>;
    type ToolT: Tool<Self>;
    type AddCommandElementT: From<Self::CommonElementViewT> + TryInto<Self::CommonElementViewT> + Clone + Debug;
    type PropChangeT: Clone + Debug;
}

pub trait StructuralVisitor<T: ?Sized> {
    fn open_complex(&mut self, e: &T);
    fn close_complex(&mut self, e: &T);
    fn visit_simple(&mut self, e: &T);
}

pub trait Model: Entity + 'static {
    fn uuid(&self) -> Arc<ModelUuid>;
    fn name(&self) -> Arc<String>;
    fn accept(&self, v: &mut dyn StructuralVisitor<dyn Model>) where Self: Sized {
        v.visit_simple(self);
    }
}

pub trait ContainerModel: Model {
    type ElementT: Model;

    fn find_element(&self, uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        None
    }
    fn add_element(&mut self, element: Self::ElementT) -> Result<(), Self::ElementT> {
        Err(element)
    }
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>) -> Result<(), ()> {
        Err(())
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

    fn targetting_for_element(&self, element: Option<DomainT::CommonElementT>) -> egui::Color32;
    fn draw_status_hint(&self, q: &DomainT::QueryableT<'_>, canvas: &mut dyn NHCanvas, pos: egui::Pos2);

    fn add_position(&mut self, pos: egui::Pos2);
    fn add_element(&mut self, element: DomainT::CommonElementT);
    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<DomainT>,
    ) -> Option<DomainT::CommonElementViewT>;
    fn reset_event_lock(&mut self);
}

pub trait ContainerGen2<DomainT: Domain> {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<DomainT::CommonElementViewT> {
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
    pub modifiers: ModifierKeys,
    pub ui_scale: f32,
    pub all_elements: &'a HashMap<ViewUuid, SelectionStatus>,
    pub snap_manager: &'a SnapManager,
}

pub trait ElementControllerGen2<DomainT: Domain>: ElementController<DomainT::CommonElementT> + NHContextSerialize + ContainerGen2<DomainT> + Send + Sync {
    fn show_properties(
        &mut self,
        _: &DomainT::QueryableT<'_>,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> bool {
        false
    }
    fn draw_in(
        &mut self,
        _: &DomainT::QueryableT<'_>,
        context: &DrawingContext,
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
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
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
        c: &HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &HashMap<ModelUuid, DomainT::CommonElementT>,
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
    fn add_element(&mut self, element: DomainT::CommonElementT) {
        self.model().write().add_element(element);
    }
    fn delete_elements(&mut self, elements: &HashSet<ModelUuid>) {
        self.model().write().delete_elements(elements);
    }
    fn create_new_view_for(
        &self,
        q: &DomainT::QueryableT<'_>,
        element: DomainT::CommonElementT,
    ) -> Result<DomainT::CommonElementViewT, HashSet<ModelUuid>>;

    fn show_props_fun(
        &mut self,
        view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn apply_property_change_fun(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn refresh_buffers(&mut self);
    fn show_tool_palette(
        &mut self,
        tool: &mut Option<DomainT::ToolT>,
        drawing_context: &DrawingContext,
        ui: &mut egui::Ui,
    );
    fn menubar_options_fun(&self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);

    fn deep_copy(&self) -> (Self, HashMap<ModelUuid, DomainT::CommonElementT>);
    fn fake_copy(&self) -> (Self, HashMap<ModelUuid, DomainT::CommonElementT>);
}

/// This is a generic DiagramController implementation.
/// Hopefully it should reduce the amount of code, but nothing prevents creating fully custom DiagramController implementations.
#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ViewUuid, initialize_with = Self::initialize)]
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
        for v in self.temporaries.flattened_views.values_mut() {
            v.refresh_buffers();
        }

        self.temporaries.name_buffer = (*self.name).clone();
        self.adapter.refresh_buffers();
    }

    pub fn model(&self) -> ERef<DomainT::DiagramModelT> {
        self.adapter.model()
    }

    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
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
            modifiers,
            ui_scale: self.temporaries.camera_scale,
            all_elements: &self.temporaries.flattened_views_status,
            snap_manager: &self.temporaries.snap_manager,
        };

        let child = self.owned_views.event_order_find_mut(|v| {
            let r = v.handle_event(event, &ehc, &mut self.temporaries.current_tool, &mut commands);
            if r != EventHandlingStatus::NotHandled {
                let k = v.uuid();
                Some((*k, match r {
                    EventHandlingStatus::HandledByElement if matches!(event, InputEvent::Click(_)) => {
                        if !modifiers.command {
                            commands.push(InsensitiveCommand::SelectAll(false).into());
                            commands.push(InsensitiveCommand::SelectSpecific(
                                std::iter::once(*k).collect(),
                                true,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::SelectSpecific(
                                std::iter::once(*k).collect(),
                                !self.temporaries.flattened_views_status.get(&k).is_some_and(|e| e.selected()),
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
                        commands.push(InsensitiveCommand::SelectAll(false).into());
                    })
                    .is_ok();

                if !handled {
                    if let Some(t) = self.temporaries.current_tool.as_mut() {
                        t.add_position(pos);
                    }
                }

                let mut tool = self.temporaries.current_tool.take();
                if let Some(new_a) = tool.as_mut().and_then(|e| e.try_construct(self)) {
                    commands.push(InsensitiveCommand::AddElement(
                        *self.uuid(),
                        DomainT::AddCommandElementT::from(new_a),
                        true,
                    ).into());
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
                    |e| self.temporaries.flattened_views_status.get(e).is_some(),
                    HashMap::new(),
                    self.temporaries.clipboard_elements.iter().map(|e| (*e.0, e.1.clone())),
                    ).into_iter().map(|e| e.1.into()).collect(),
            );

            // compute transitive closure of dependency when deleting elements
            let command = match command {
                InsensitiveCommand::DeleteSpecificElements(uuids, b) => {
                    let mut deleting = uuids;
                    let mut found_uuids = HashSet::new();
                    loop {
                        for (k, e1) in self.temporaries.flattened_views.iter().filter(|e| !deleting.contains(e.0)) {
                            if e1.delete_when(&deleting) {
                                found_uuids.insert(*k);
                            }
                        }

                        if found_uuids.is_empty() {
                            break;
                        }
                        deleting.extend(found_uuids.drain());
                    }

                    InsensitiveCommand::DeleteSpecificElements(deleting, b)
                },
                c => c,
            };

            let mut undo_accumulator = vec![];

            match &command {
                InsensitiveCommand::SelectAll(..)
                | InsensitiveCommand::SelectSpecific(..)
                | InsensitiveCommand::SelectByDrag(..)
                | InsensitiveCommand::MoveSpecificElements(..)
                | InsensitiveCommand::MoveAllElements(..)
                | InsensitiveCommand::ResizeSpecificElementsBy(..)
                | InsensitiveCommand::ResizeSpecificElementsTo(..) => {}
                InsensitiveCommand::DeleteSpecificElements(uuids, _)
                | InsensitiveCommand::CutSpecificElements(uuids) => {
                    let from_model = matches!(
                        command,
                        InsensitiveCommand::DeleteSpecificElements(_, true) | InsensitiveCommand::CutSpecificElements(..)
                    );
                    let mut model_uuids = HashSet::new();
                    for (uuid, element) in self
                        .owned_views
                        .iter_event_order_pairs()
                        .filter(|e| uuids.contains(&e.0))
                    {
                        model_uuids.insert(*element.model_uuid());
                        undo_accumulator.push(InsensitiveCommand::AddElement(*self.uuid(), element.clone().into(), from_model));
                    }
                    if from_model {
                        self.adapter.delete_elements(&model_uuids);
                    }

                    self.owned_views.retain(|k, _v| !uuids.contains(k));
                }
                InsensitiveCommand::AddElement(target, element, into_model) => {
                    if *target == *self.uuid {
                        if let Ok(view) = element.clone().try_into() {
                            let uuid = *view.uuid();
                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(uuid).collect(),
                                *into_model,
                            ));

                            if *into_model {
                                self.adapter.add_element(view.model());
                            }

                            self.owned_views.push(uuid, view);
                        }
                    }
                }
                InsensitiveCommand::PasteSpecificElements(_, elements) => {
                    for element in elements {
                        if let Ok(view) = element.clone().try_into() {
                            let uuid = *view.uuid();
                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(uuid).collect(),
                                true,
                            ));

                            self.adapter.add_element(view.model());

                            self.owned_views.push(uuid, view);
                        }
                    }
                }
                InsensitiveCommand::ArrangeSpecificElements(uuids, arr) => {
                    self.owned_views.apply_arrangement(uuids, *arr);
                },
                InsensitiveCommand::PropertyChange(uuids, _property) => {
                    if uuids.contains(&*self.uuid) {
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
                InsensitiveCommand::SelectAll(..)
                | InsensitiveCommand::SelectSpecific(..)
                | InsensitiveCommand::SelectByDrag(..)
                | InsensitiveCommand::DeleteSpecificElements(..)
                | InsensitiveCommand::AddElement(..)
                | InsensitiveCommand::CutSpecificElements(..)
                | InsensitiveCommand::PasteSpecificElements(..) => true,
                InsensitiveCommand::MoveSpecificElements(..)
                | InsensitiveCommand::MoveAllElements(..)
                | InsensitiveCommand::ResizeSpecificElementsBy(..)
                | InsensitiveCommand::ResizeSpecificElementsTo(..)
                | InsensitiveCommand::ArrangeSpecificElements(..)
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
    ) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>) {
        let new_diagram_view = Self::new(
            Arc::new(uuid::Uuid::now_v7().into()),
            format!("{} (copy)", self.name).into(),
            new_adapter,
            Self::elements_deep_copy(
                None,
                |_| false,
                models,
                self.owned_views.iter_event_order_pairs().map(|e| (e.0, e.1.clone())),
            ).into_iter().map(|e| e.1).collect(),
        );
        let new_model_view = SimpleModelHierarchyView::new(new_diagram_view.read().model());

        (new_diagram_view, Arc::new(new_model_view))
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
    fn model_name(&self) -> Arc<String> {
        self.adapter.model_name()
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> TopLevelView for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn view_name(&self) -> Arc<String> {
        self.name.clone()
    }

    fn view_type(&self) -> String {
        self.adapter.view_type().to_owned()
    }
}

impl<
    DomainT: Domain,
    DiagramAdapterT: DiagramAdapter<DomainT>
> DiagramController for DiagramControllerGen2<DomainT, DiagramAdapterT> {
    fn represented_models(&self) -> &HashMap<ModelUuid, ViewUuid> {
        &self.temporaries.flattened_represented_models
    }

    fn refresh_buffers(&mut self, affected_models: &HashSet<ModelUuid>) {
        if affected_models.contains(&self.adapter.model_uuid()) {
            self.adapter.refresh_buffers();
        }

        for mk in affected_models.iter() {
            if let Some(vk) = self.temporaries.flattened_represented_models.get(mk)
                && let Some(v) = self.temporaries.flattened_views.get_mut(vk) {
                v.refresh_buffers();
            }
        }
    }

    fn new_ui_canvas(
        &mut self,
        context: &DrawingContext,
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
        );
        ui_canvas.clear(context.profile.backgrounds[0]);
        ui_canvas.draw_gridlines(
            Some((50.0, context.profile.foregrounds[0])),
            Some((50.0, context.profile.foregrounds[0])),
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
                    self.handle_event(InputEvent::MouseDown(pos_to_abs!(*pos)), modifiers, undo_accumulator, affected_models);
                },
                _ => {}
            })
        );
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(old_pos) = self.temporaries.last_unhandled_mouse_pos {
                let delta = response.drag_delta() / self.temporaries.camera_scale;
                self.handle_event(InputEvent::Drag { from: old_pos, delta }, modifiers, undo_accumulator, affected_models);
                self.temporaries.last_unhandled_mouse_pos = Some(old_pos + delta);
            }
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.handle_event(InputEvent::Click(pos_to_abs!(pos)), modifiers, undo_accumulator, affected_models);
            }
        }
        ui.input(|is| is.events.iter()
            .for_each(|e| match e {
                egui::Event::PointerButton { pos, button, pressed, .. } if !*pressed && *button == egui::PointerButton::Primary => {
                    self.handle_event(InputEvent::MouseUp(pos_to_abs!(*pos)), modifiers, undo_accumulator, affected_models);
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
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        ui.label("asdf");
    }

    fn show_toolbar(
        &mut self,
        context: &DrawingContext,
        ui: &mut egui::Ui,
    ) {
        self.adapter.show_tool_palette(&mut self.temporaries.current_tool, context, ui);
    }
    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        undo_accumulator: &mut Vec<Arc<String>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        let mut commands = Vec::new();
        {
            let queryable = DomainT::QueryableT::new(&self.temporaries.flattened_represented_models, &self.temporaries.flattened_views);

            if self
                .owned_views
                .event_order_find_mut(|v| if v.show_properties(&queryable, ui, &mut commands) { Some(()) } else { None })
                .is_none()
            {
                ui.label("View properties:");
                ui.label("Name:");
                if ui.text_edit_singleline(&mut self.temporaries.name_buffer).changed() {
                    self.name = Arc::new(self.temporaries.name_buffer.clone());
                }
                ui.add_space(VIEW_MODEL_PROPERTIES_BLOCK_SPACING);

                ui.label("Model properties:");
                self.adapter.show_props_fun(&self.uuid, ui, &mut commands);
            }
        }

        self.apply_commands(commands, undo_accumulator, true, true, affected_models);
    }
    fn show_layers(&self, _ui: &mut egui::Ui) {
        // TODO: Layers???
    }
    fn show_menubar_edit_options(&mut self, _ui: &mut egui::Ui, _commands: &mut Vec<ProjectCommand>) {}
    fn show_menubar_diagram_options(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
        self.adapter.menubar_options_fun(/*self,*/ ui, commands);

        if ui.button("Layout selected elements").clicked() {
            todo!();
        }
        ui.separator();
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
            DiagramCommand::SelectAllElements(select) => {
                self.apply_commands(vec![InsensitiveCommand::SelectAll(select).into()], &mut vec![], true, false, affected_models);
            }
            DiagramCommand::InvertSelection => {
                self.apply_commands(vec![
                    InsensitiveCommand::SelectAll(true).into(),
                    InsensitiveCommand::SelectSpecific(self.temporaries.flattened_views_status.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect(), false).into()
                ], &mut vec![], true, false, affected_models);
            }
            DiagramCommand::DeleteSelectedElements
            | DiagramCommand::CutSelectedElements
            | DiagramCommand::PasteClipboardElements
            | DiagramCommand::ArrangeSelected(_) => {
                if matches!(command, DiagramCommand::CutSelectedElements) {
                    self.set_clipboard_from_selected();
                }

                let mut undo = vec![];
                self.apply_commands(vec![
                    match command {
                        DiagramCommand::DeleteSelectedElements => SensitiveCommand::DeleteSelectedElements(true),
                        DiagramCommand::CutSelectedElements => SensitiveCommand::CutSelectedElements,
                        DiagramCommand::PasteClipboardElements => SensitiveCommand::PasteClipboardElements,
                        DiagramCommand::ArrangeSelected(arr) => SensitiveCommand::ArrangeSelected(arr),
                        _ => unreachable!(),
                    }
                ], &mut undo, true, true, affected_models);
                self.temporaries.last_change_flag = true;
                global_undo.extend(undo.into_iter());
            }
            DiagramCommand::CopySelectedElements => {
                self.set_clipboard_from_selected();
            },
            DiagramCommand::CreateViewFor(model_uuid) => {
                if let Some((model, parent_uuid)) = self.adapter.find_element(&model_uuid) {
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
                                    cmds.push(InsensitiveCommand::AddElement(parent_view_uuid, new_view.into(), false).into());
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
        context: &DrawingContext,
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
                        tool.targetting_for_element(None),
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

            self.temporaries.snap_manager.draw_best(canvas, context.profile, self.temporaries.last_interactive_canvas_rect);
        }
    }

    fn deep_copy(&self) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>) {
        let (new_adapter, models) = self.adapter.deep_copy();
        self.some_kind_of_copy(new_adapter, models)
    }

    fn shallow_copy(&self) -> (ERef<dyn DiagramController>, Arc<dyn ModelHierarchyView>) {
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


pub trait PackageAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + Send + Sync + 'static {
    fn model(&self) -> DomainT::CommonElementT;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;

    fn add_element(&mut self, element: DomainT::CommonElementT);
    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>);

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>
    );
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn refresh_buffers(&mut self);

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    ) -> Self where Self: Sized;
    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DomainT::CommonElementT>
    );
}

#[derive(Clone, Copy, PartialEq)]
pub enum PackageDragType {
    Move,
    Resize(egui::Align2),
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub struct PackageView<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    adapter: AdapterT,
    #[nh_context_serde(entity)]
    owned_views: OrderedViews<DomainT::CommonElementViewT>,
    #[nh_context_serde(skip_and_default)]
    all_elements: HashMap<ViewUuid, SelectionStatus>,
    #[nh_context_serde(skip_and_default)]
    selected_direct_elements: HashSet<ViewUuid>,

    #[nh_context_serde(skip_and_default)]
    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> PackageView<DomainT, AdapterT> {
    pub fn new(
        uuid: Arc<ViewUuid>,
        adapter: AdapterT,
        owned_views: Vec<DomainT::CommonElementViewT>,
        bounds_rect: egui::Rect,
    ) -> ERef<Self> {
        ERef::new(
            Self {
                uuid,
                adapter,
                owned_views: OrderedViews::new(owned_views),
                all_elements: HashMap::new(),
                selected_direct_elements: HashSet::new(),

                dragged_type_and_shape: None,
                highlight: canvas::Highlight::NONE,
                bounds_rect,
            }
        )
    }

    fn handle_size(&self, ui_scale: f32) -> f32 {
        10.0_f32
            .min(self.bounds_rect.width() * ui_scale / 6.0)
            .min(self.bounds_rect.height() * ui_scale / 3.0)
    }
    fn drag_handle_position(&self, ui_scale: f32) -> egui::Pos2 {
        egui::Pos2::new(
            (self.bounds_rect.right() - 2.0 * self.handle_size(ui_scale) / ui_scale)
                .max((self.bounds_rect.center().x + self.bounds_rect.right()) / 2.0),
            self.bounds_rect.top()
        )
    }
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> Entity for PackageView<DomainT, AdapterT> {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> View for PackageView<DomainT, AdapterT> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.adapter.model_uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.adapter.model_name()
    }
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> ElementController<DomainT::CommonElementT> for PackageView<DomainT, AdapterT> {
    fn model(&self) -> DomainT::CommonElementT {
        self.adapter.model()
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

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> ContainerGen2<DomainT> for PackageView<DomainT, AdapterT> {
    fn controller_for(&self, uuid: &ModelUuid) -> Option<DomainT::CommonElementViewT> {
        // TODO: store views by model uuids?
        self.owned_views.iter_event_order_pairs().find(|(_, v)| *v.model_uuid() == *uuid).map(|(_, v)| v.clone())
            .or_else(|| self.owned_views.iter_event_order_pairs().flat_map(|(_, v)| v.controller_for(uuid)).next())
    }
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> ElementControllerGen2<DomainT> for PackageView<DomainT, AdapterT>
where
    DomainT::CommonElementViewT: From<ERef<PackageView<DomainT, AdapterT>>>,
{
    fn show_properties(
        &mut self,
        parent: &DomainT::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> bool {
        if self
            .owned_views
            .event_order_find_mut(|v| if v.show_properties(parent, ui, commands) { Some(()) } else { None })
            .is_some()
        {
            true
        } else if self.highlight.selected {
            ui.label("Model properties");

            self.adapter.show_properties(ui, commands);

            ui.add_space(VIEW_MODEL_PROPERTIES_BLOCK_SPACING);
            ui.label("View properties");

            egui::Grid::new("size_grid").show(ui, |ui| {
                {
                    let egui::Pos2 { mut x, mut y } = self.bounds_rect.left_top();

                    ui.label("x");
                    if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                        commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(x - self.bounds_rect.left(), 0.0)));
                    }
                    ui.label("y");
                    if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                        commands.push(SensitiveCommand::MoveSelectedElements(egui::Vec2::new(0.0, y - self.bounds_rect.top())));
                    }
                    ui.end_row();
                }

                {
                    let egui::Vec2 { mut x, mut y } = self.bounds_rect.size();

                    ui.label("width");
                    if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                        commands.push(SensitiveCommand::ResizeSelectedElementsBy(egui::Align2::LEFT_CENTER, egui::Vec2::new(x - self.bounds_rect.width(), 0.0)));
                    }
                    ui.label("height");
                    if ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed() {
                        commands.push(SensitiveCommand::ResizeSelectedElementsBy(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, y - self.bounds_rect.height())));
                    }
                    ui.end_row();
                }
            });

            true
        } else {
            false
        }
    }
    fn draw_in(
        &mut self,
        q: &DomainT::QueryableT<'_>,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &DomainT::ToolT)>,
    ) -> TargettingStatus {
        // Draw shape and text
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            context.profile.backgrounds[1],
            canvas::Stroke::new_solid(1.0, context.profile.foregrounds[1]),
            self.highlight,
        );

        canvas.draw_text(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            &self.adapter.model_name(),
            canvas::CLASS_MIDDLE_FONT_SIZE,
            context.profile.foregrounds[1],
        );

        // Draw resize/drag handles
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            let handle_size = self.handle_size(ui_scale);
            for h in [self.bounds_rect.left_top(), self.bounds_rect.center_top(), self.bounds_rect.right_top(),
                      self.bounds_rect.left_center(), self.bounds_rect.right_center(),
                      self.bounds_rect.left_bottom(), self.bounds_rect.center_bottom(), self.bounds_rect.right_bottom()]
            {
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size / ui_scale)),
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }

            canvas.draw_rectangle(
                egui::Rect::from_center_size(
                    self.drag_handle_position(ui_scale),
                    egui::Vec2::splat(handle_size / ui_scale),
                ),
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
        }

        let mut drawn_child_targetting = TargettingStatus::NotDrawn;

        self.owned_views.draw_order_foreach_mut(|v|
            if v.draw_in(q, context, canvas, &tool) == TargettingStatus::Drawn {
                drawn_child_targetting = TargettingStatus::Drawn;
            }
        );

        if canvas.ui_scale().is_some() {
            if self.dragged_type_and_shape.is_some() {
                canvas.draw_line([
                    egui::Pos2::new(self.bounds_rect.min.x, self.bounds_rect.center().y),
                    egui::Pos2::new(self.bounds_rect.max.x, self.bounds_rect.center().y),
                ], canvas::Stroke::new_solid(1.0, egui::Color32::BLUE), canvas::Highlight::NONE);
                canvas.draw_line([
                    egui::Pos2::new(self.bounds_rect.center().x, self.bounds_rect.min.y),
                    egui::Pos2::new(self.bounds_rect.center().x, self.bounds_rect.max.y),
                ], canvas::Stroke::new_solid(1.0, egui::Color32::BLUE), canvas::Highlight::NONE);
            }

            match (drawn_child_targetting, tool) {
                (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                    canvas.draw_rectangle(
                        self.bounds_rect,
                        egui::CornerRadius::ZERO,
                        t.targetting_for_element(Some(self.adapter.model())),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );

                    self.owned_views.draw_order_foreach_mut(|v| {
                        v.draw_in(q, context, canvas, &tool);
                    });

                    TargettingStatus::Drawn
                }
                _ => drawn_child_targetting,
            }
        } else {
            TargettingStatus::NotDrawn
        }
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid, self.min_shape());

        self.owned_views.event_order_foreach_mut(|v|
            v.collect_allignment(am)
        );
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<DomainT::ToolT>,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> EventHandlingStatus {
        let k_status = self.owned_views.event_order_find_mut(|v| {
            let s = v.handle_event(event, ehc, tool, commands);
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
                let handle_size = self.handle_size(1.0);
                for (a,h) in [(egui::Align2::RIGHT_BOTTOM, self.bounds_rect.left_top()),
                              (egui::Align2::CENTER_BOTTOM, self.bounds_rect.center_top()),
                              (egui::Align2::LEFT_BOTTOM, self.bounds_rect.right_top()),
                              (egui::Align2::RIGHT_CENTER, self.bounds_rect.left_center()),
                              (egui::Align2::LEFT_CENTER, self.bounds_rect.right_center()),
                              (egui::Align2::RIGHT_TOP, self.bounds_rect.left_bottom()),
                              (egui::Align2::CENTER_TOP, self.bounds_rect.center_bottom()),
                              (egui::Align2::LEFT_TOP, self.bounds_rect.right_bottom())]
                {
                    if egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size) / ehc.ui_scale).contains(pos) {
                        self.dragged_type_and_shape = Some((PackageDragType::Resize(a), self.bounds_rect));
                        return EventHandlingStatus::HandledByElement;
                    }
                }

                if self.min_shape().border_distance(pos) <= 2.0 / ehc.ui_scale
                    || egui::Rect::from_center_size(
                        self.drag_handle_position(ehc.ui_scale),
                        egui::Vec2::splat(handle_size) / ehc.ui_scale).contains(pos) {
                    self.dragged_type_and_shape = Some((PackageDragType::Move, self.bounds_rect));
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            },
            InputEvent::MouseUp(_pos) => {
                if self.dragged_type_and_shape.is_some() {
                    self.dragged_type_and_shape = None;
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            }
            InputEvent::Click(pos) => {
                if self.min_shape().contains(pos) {
                    if let Some(tool) = tool {
                        tool.add_position(*event.mouse_position());
                        tool.add_element(self.adapter.model());

                        if let Some(new_a) = tool.try_construct(self) {
                            commands.push(InsensitiveCommand::AddElement(*self.uuid, new_a.into(), true).into());
                        }

                        EventHandlingStatus::HandledByContainer
                    } else if let Some((k, status)) = k_status {
                        if status == EventHandlingStatus::HandledByElement {
                            if !ehc.modifiers.command {
                                commands.push(InsensitiveCommand::SelectAll(false).into());
                                commands.push(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(k).collect(),
                                    true,
                                ).into());
                            } else {
                                commands.push(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(k).collect(),
                                    !self.selected_direct_elements.contains(&k),
                                ).into());
                            }
                        }
                        EventHandlingStatus::HandledByContainer
                    } else {
                        EventHandlingStatus::HandledByElement
                    }
                } else {
                    k_status.map(|e| e.1).unwrap_or(EventHandlingStatus::NotHandled)
                }
            },
            InputEvent::Drag { delta, .. } => match self.dragged_type_and_shape {
                Some((PackageDragType::Move, real_bounds)) => {
                    let translated_bounds = real_bounds.translate(delta);
                    self.dragged_type_and_shape = Some((PackageDragType::Move, translated_bounds));
                    let translated_real_shape = NHShape::Rect { inner: translated_bounds };
                    let coerced_pos = ehc.snap_manager.coerce(translated_real_shape,
                        |e| !self.all_elements.get(e).is_some() && !if self.highlight.selected { ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected) } else {*e == *self.uuid}
                    );
                    let coerced_delta = coerced_pos - self.position();

                    if self.highlight.selected {
                        commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
                    } else {
                        commands.push(InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid).collect(),
                            coerced_delta,
                        ).into());
                    }
                    EventHandlingStatus::HandledByElement
                },
                Some((PackageDragType::Resize(align), real_bounds)) => {
                    let (left, right) = match align.x() {
                        egui::Align::Min => (0.0, delta.x),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => (-delta.x, 0.0),
                    };
                    let (top, bottom) = match align.y() {
                        egui::Align::Min => (0.0, delta.y),
                        egui::Align::Center => (0.0, 0.0),
                        egui::Align::Max => (-delta.y, 0.0),
                    };
                    let new_real_bounds = real_bounds + epaint::MarginF32 { left, right, top, bottom };
                    self.dragged_type_and_shape = Some((PackageDragType::Resize(align), new_real_bounds));
                    let handle_x = match align.x() {
                        egui::Align::Min => (new_real_bounds.right(), self.bounds_rect.right()),
                        egui::Align::Center => (new_real_bounds.center().x, self.bounds_rect.center().x),
                        egui::Align::Max => (new_real_bounds.left(), self.bounds_rect.left()),
                    };
                    let handle_y = match align.y() {
                        egui::Align::Min => (new_real_bounds.bottom(), self.bounds_rect.bottom()),
                        egui::Align::Center => (new_real_bounds.center().y, self.bounds_rect.center().y),
                        egui::Align::Max => (new_real_bounds.top(), self.bounds_rect.top()),
                    };
                    let coerced_point = ehc.snap_manager.coerce(
                        NHShape::Rect { inner: egui::Rect::from_min_size(egui::Pos2::new(handle_x.0, handle_y.0), egui::Vec2::ZERO) },
                        |e| !self.all_elements.get(e).is_some() && !ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected)
                    );
                    let coerced_delta = coerced_point - egui::Pos2::new(handle_x.1, handle_y.1);

                    commands.push(SensitiveCommand::ResizeSelectedElementsBy(align, coerced_delta));
                    EventHandlingStatus::HandledByElement
                },
                None => EventHandlingStatus::NotHandled,
            },
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! recurse {
            ($self:ident) => {
                $self.owned_views.event_order_foreach_mut(|v|
                    v.apply_command(command, undo_accumulator, affected_models)
                );
            };
        }
        macro_rules! resize_by {
            ($align:expr, $delta:expr) => {
                let min_delta_x = 40.0 - self.bounds_rect.width();
                let (left, right) = match $align.x() {
                    egui::Align::Min => (0.0, $delta.x.max(min_delta_x)),
                    egui::Align::Center => (0.0, 0.0),
                    egui::Align::Max => ((-$delta.x).max(min_delta_x), 0.0),
                };
                let min_delta_y = 20.0 - self.bounds_rect.height();
                let (top, bottom) = match $align.y() {
                    egui::Align::Min => (0.0, $delta.y.max(min_delta_y)),
                    egui::Align::Center => (0.0, 0.0),
                    egui::Align::Max => ((-$delta.y).max(min_delta_y), 0.0),
                };

                let r = self.bounds_rect + epaint::MarginF32{left, right, top, bottom};

                undo_accumulator.push(InsensitiveCommand::ResizeSpecificElementsTo(
                    std::iter::once(*self.uuid).collect(),
                    *$align,
                    self.bounds_rect.size(),
                ));
                self.bounds_rect = r;
            };
        }

        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
                match select {
                    true => {
                        self.selected_direct_elements =
                            self.owned_views.iter_event_order_keys().collect()
                    }
                    false => self.selected_direct_elements.clear(),
                }
                recurse!(self);
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight.selected = *select;
                }

                for k in self.owned_views.iter_event_order_keys().filter(|k| uuids.contains(k)) {
                    match select {
                        true => self.selected_direct_elements.insert(k),
                        false => self.selected_direct_elements.remove(&k),
                    };
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
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid).collect(),
                    -*delta,
                ));
                self.owned_views.event_order_foreach_mut(|v| {
                    v.apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut vec![], affected_models);
                });
            }
            InsensitiveCommand::ResizeSpecificElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid) {
                    resize_by!(align, delta);
                }

                recurse!(self);
            }
            InsensitiveCommand::ResizeSpecificElementsTo(uuids, align, size) => {
                if uuids.contains(&self.uuid) {
                    let delta_naive = *size - self.bounds_rect.size();
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
            InsensitiveCommand::DeleteSpecificElements(uuids, _)
            | InsensitiveCommand::CutSpecificElements(uuids) => {
                let from_model = matches!(
                    command,
                    InsensitiveCommand::DeleteSpecificElements(_, true) | InsensitiveCommand::CutSpecificElements(..)
                );
                let mut model_uuids = HashSet::new();
                for (uuid, element) in self
                    .owned_views
                    .iter_event_order_pairs()
                    .filter(|e| uuids.contains(&e.0))
                {
                    model_uuids.insert(*element.model_uuid());
                    undo_accumulator.push(InsensitiveCommand::AddElement(
                        *self.uuid,
                        element.clone().into(),
                        from_model,
                    ));
                }
                if from_model {
                    self.adapter.delete_elements(&model_uuids);
                }

                self.owned_views.retain(|k, _v| !uuids.contains(k));

                recurse!(self);
            }
            InsensitiveCommand::AddElement(target, element, into_model) => {
                if *target == *self.uuid {
                    if let Ok(view) = element.clone().try_into() {
                        let uuid = *view.uuid();
                        undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                            std::iter::once(uuid).collect(),
                            *into_model,
                        ));

                        if *into_model {
                            self.adapter.add_element(view.model());
                        }

                        self.owned_views.push(uuid, view);
                    }
                }

                recurse!(self);
            }
            InsensitiveCommand::PasteSpecificElements(target, _elements) => {
                if *target == *self.uuid {
                    todo!("undo = delete")
                }

                recurse!(self);
            },
            InsensitiveCommand::ArrangeSpecificElements(uuids, arr) => {
                self.owned_views.apply_arrangement(uuids, *arr);
            },
            InsensitiveCommand::PropertyChange(uuids, _property) => {
                if uuids.contains(&*self.uuid) {
                    self.adapter.apply_change(
                        &self.uuid,
                        command,
                        undo_accumulator,
                    );
                    affected_models.insert(*self.adapter.model_uuid());
                }

                recurse!(self);
            }
        }
    }
    fn refresh_buffers(&mut self) {
        self.adapter.refresh_buffers();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.highlight.selected.into());
        flattened_represented_models.insert(*self.adapter.model_uuid(), *self.uuid);

        self.all_elements.clear();
        self.owned_views.event_order_foreach_mut(|v|
            v.head_count(flattened_views, &mut self.all_elements, flattened_represented_models)
        );
        for e in &self.all_elements {
            flattened_views_status.insert(*e.0, match *e.1 {
                SelectionStatus::NotSelected if self.highlight.selected => SelectionStatus::TransitivelySelected,
                e => e,
            });
        }

        self.owned_views.event_order_foreach_mut(|v| {
            flattened_views.insert(*v.uuid(), v.clone());
        });
    }

    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<ViewUuid>>,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {
        if requested.is_none_or(|e| e.contains(&self.uuid)) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.owned_views.event_order_foreach(|v|
                v.deep_copy_walk(requested, uuid_present, tlc, c, m)
            );
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *self.model_uuid())
        };

        let mut inner = HashMap::new();
        self.owned_views.event_order_foreach(|v|
            v.deep_copy_clone(uuid_present, &mut inner, c, m)
        );

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            adapter: self.adapter.deep_copy_init(model_uuid, m),
            owned_views: OrderedViews::new(inner.into_values().collect()),
            all_elements: HashMap::new(),
            selected_direct_elements: self.selected_direct_elements.clone(),
            dragged_type_and_shape: None,
            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {
        self.owned_views.event_order_foreach_mut(|v|
            v.deep_copy_relink(c, m)
        );
    }
}

pub trait MulticonnectionAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + Send + Sync {
    fn model(&self) -> DomainT::CommonElementT;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;

    fn midpoint_label(&self) -> Option<Arc<String>> { None }
    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>);
    fn destination_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>);

    fn show_properties(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>
    );
    fn apply_change(
        &self,
        view_uuid: &ViewUuid,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    );
    fn refresh_buffers(&mut self);

    fn deep_copy_init(
        &self,
        new_uuid: ModelUuid,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    ) -> Self where Self: Sized;
    fn deep_copy_finish(
        &mut self,
        m: &HashMap<ModelUuid, DomainT::CommonElementT>,
    );
}

#[derive(Clone, Debug)]
pub struct VertexInformation {
    after: ViewUuid,
    id: ViewUuid,
    position: egui::Pos2,
}
#[derive(Clone, Debug)]
pub struct FlipMulticonnection {}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(uuid_type = ViewUuid)]
pub struct MulticonnectionView<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    adapter: AdapterT,

    #[nh_context_serde(entity)]
    pub source: DomainT::CommonElementViewT,
    #[nh_context_serde(entity)]
    pub target: DomainT::CommonElementViewT,

    #[nh_context_serde(skip_and_default)]
    dragged_node: Option<(ViewUuid, egui::Pos2)>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    #[nh_context_serde(skip_and_default)]
    selected_vertices: HashSet<ViewUuid>,
    center_point: UFOption<(ViewUuid, egui::Pos2)>,
    source_points: Vec<Vec<(ViewUuid, egui::Pos2)>>,
    dest_points: Vec<Vec<(ViewUuid, egui::Pos2)>>,
    #[nh_context_serde(skip_and_default)]
    point_to_origin: HashMap<ViewUuid, (bool, usize)>,
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> MulticonnectionView<DomainT, AdapterT>
where
    DomainT::AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a DomainT::PropChangeT: TryInto<FlipMulticonnection>,
{
    pub fn new(
        uuid: Arc<ViewUuid>,
        adapter: AdapterT,
        source: DomainT::CommonElementViewT,
        destination: DomainT::CommonElementViewT,

        center_point: Option<(ViewUuid, egui::Pos2)>,
        source_points: Vec<Vec<(ViewUuid, egui::Pos2)>>,
        dest_points: Vec<Vec<(ViewUuid, egui::Pos2)>>,
    ) -> ERef<Self> {
        let mut point_to_origin = HashMap::new();
        for (idx, path) in source_points.iter().enumerate() {
            for p in path {
                point_to_origin.insert(p.0, (false, idx));
            }
        }
        for (idx, path) in dest_points.iter().enumerate() {
            for p in path {
                point_to_origin.insert(p.0, (true, idx));
            }
        }

        ERef::new(
            Self {
                uuid,
                adapter,
                source,
                target: destination,
                dragged_node: None,
                highlight: canvas::Highlight::NONE,
                selected_vertices: HashSet::new(),

                center_point: center_point.into(),
                source_points,
                dest_points,
                point_to_origin,
            }
        )
    }

    const VERTEX_RADIUS: f32 = 5.0;
    fn all_vertices(&self) -> impl Iterator<Item = &(ViewUuid, egui::Pos2)> {
        self.center_point.as_ref().into_iter()
            .chain(self.source_points.iter().flat_map(|e| e.iter()))
            .chain(self.dest_points.iter().flat_map(|e| e.iter()))
    }
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> Entity for MulticonnectionView<DomainT, AdapterT> {
    fn tagged_uuid(&self) -> EntityUuid {
        EntityUuid::View(*self.uuid)
    }
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> View for MulticonnectionView<DomainT, AdapterT>
where
    DomainT::AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a DomainT::PropChangeT: TryInto<FlipMulticonnection>,
{
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.adapter.model_uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.adapter.model_name()
    }
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> ElementController<DomainT::CommonElementT> for MulticonnectionView<DomainT, AdapterT>
where
    DomainT::AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a DomainT::PropChangeT: TryInto<FlipMulticonnection>,
{
    fn model(&self) -> DomainT::CommonElementT {
        self.adapter.model()
    }

    fn min_shape(&self) -> NHShape {
        NHShape::Rect {
            inner: egui::Rect::NOTHING,
        }
    }
    fn max_shape(&self) -> NHShape {
        todo!()
    }

    fn position(&self) -> egui::Pos2 {
        match &self.center_point {
            UFOption::Some(point) => point.1,
            UFOption::None => (self.source_points[0][0].1 + self.dest_points[0][0].1.to_vec2()) / 2.0,
        }
    }
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> ContainerGen2<DomainT> for MulticonnectionView<DomainT, AdapterT> {}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> ElementControllerGen2<DomainT> for MulticonnectionView<DomainT, AdapterT>
where
    DomainT::CommonElementViewT: From<ERef<MulticonnectionView<DomainT, AdapterT>>>,
    DomainT::AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a DomainT::PropChangeT: TryInto<FlipMulticonnection>,
{
    fn show_properties(
        &mut self,
        _parent: &DomainT::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        self.adapter.show_properties(ui, commands);

        true
    }

    fn draw_in(
        &mut self,
        _: &DomainT::QueryableT<'_>,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        _tool: &Option<(egui::Pos2, &DomainT::ToolT)>,
    ) -> TargettingStatus {
        let source_bounds = self.source.min_shape();
        let dest_bounds = self.target.min_shape();

        let (source_next_point, dest_next_point) = match (
            self.source_points[0].iter().skip(1)
                .map(|e| *e)
                .chain(self.center_point.as_ref().cloned())
                .find(|p| !source_bounds.contains(p.1))
                .map(|e| e.1),
            self.dest_points[0].iter().skip(1)
                .map(|e| *e)
                .chain(self.center_point.as_ref().cloned())
                .find(|p| !dest_bounds.contains(p.1))
                .map(|e| e.1),
        ) {
            (None, None) => {
                let point = source_bounds.nice_midpoint(&dest_bounds);
                (point, point)
            }
            (source_next_point, dest_next_point) => (
                source_next_point.unwrap_or(dest_bounds.center()),
                dest_next_point.unwrap_or(source_bounds.center()),
            ),
        };

        //canvas.draw_ellipse(source_next_point, egui::Vec2::splat(5.0), egui::Color32::RED, canvas::Stroke::new_solid(1.0, egui::Color32::RED), canvas::Highlight::NONE);
        //canvas.draw_ellipse(dest_next_point, egui::Vec2::splat(5.0), egui::Color32::GREEN, canvas::Stroke::new_solid(1.0, egui::Color32::GREEN), canvas::Highlight::NONE);

        // The bounds may use different intersection method only if the target points are not the same or it's the real midpoint
        let (source_intersect, dest_intersect) =
            match (source_bounds.orthogonal_intersect(source_next_point), dest_bounds.orthogonal_intersect(dest_next_point)) {
                (Some(a), Some(b)) => (a, b),
                (a, b) if source_next_point != dest_next_point || self.center_point.is_some() =>
                    (a.unwrap_or_else(|| source_bounds.center_intersect(source_next_point)),
                     b.unwrap_or_else(|| dest_bounds.center_intersect(dest_next_point))),
                _ => (source_bounds.center_intersect(source_next_point), dest_bounds.center_intersect(dest_next_point))
            };

        self.source_points[0][0].1 = source_intersect;
        self.dest_points[0][0].1 = dest_intersect;

        //canvas.draw_ellipse((self.source_points[0][0].1 + self.dest_points[0][0].1.to_vec2()) / 2.0, egui::Vec2::splat(5.0), egui::Color32::BROWN, canvas::Stroke::new_solid(1.0, egui::Color32::BROWN), canvas::Highlight::NONE);

        fn s_to_p(canvas: &mut dyn NHCanvas, bounds: NHShape, pos: egui::Pos2, s: &str) -> egui::Pos2 {
            let size = canvas.measure_text(pos, egui::Align2::CENTER_CENTER, s, canvas::CLASS_MIDDLE_FONT_SIZE).size();
            bounds.place_labels(pos, [size, egui::Vec2::ZERO])[0]
        }
        let (source_line_type, source_arrow_type, source_label) = self.adapter.source_arrow();
        let (dest_line_type, dest_arrow_type, dest_label) = self.adapter.destination_arrow();
        let l1 = source_label.as_ref().map(|e| (s_to_p(canvas, source_bounds, source_intersect, &*e), e.as_str()));
        let l2 = dest_label.as_ref().map(|e| (s_to_p(canvas, dest_bounds, dest_intersect, &*e), e.as_str()));
        let midpoint_label = self.adapter.midpoint_label();

        canvas.draw_multiconnection(
            &self.selected_vertices,
            &[(
                source_arrow_type,
                crate::common::canvas::Stroke {
                    width: 1.0,
                    color: context.profile.foregrounds[2],
                    line_type: source_line_type,
                },
                &self.source_points[0],
                l1,
            )],
            &[(
                dest_arrow_type,
                crate::common::canvas::Stroke {
                    width: 1.0,
                    color: context.profile.foregrounds[2],
                    line_type: dest_line_type,
                },
                &self.dest_points[0],
                l2,
            )],
            match &self.center_point {
                UFOption::Some(point) => *point,
                UFOption::None => (
                    uuid::Uuid::nil().into(),
                    (self.source_points[0][0].1 + self.dest_points[0][0].1.to_vec2()) / 2.0,
                ),
            },
            midpoint_label.as_ref().map(|e| e.as_str()),
            self.highlight,
        );

        TargettingStatus::NotDrawn
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        for p in self.center_point.as_ref().into_iter()
            .chain(self.source_points.iter().flat_map(|e| e.iter().skip(1)))
            .chain(self.dest_points.iter().flat_map(|e| e.iter().skip(1)))
        {
            am.add_shape(*self.uuid, NHShape::Rect { inner: egui::Rect::from_min_size(p.1, egui::Vec2::ZERO) });
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _tool: &mut Option<DomainT::ToolT>,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> EventHandlingStatus {
        const SEGMENT_DISTANCE_THRESHOLD: f32 = 2.0;
        let is_over = |a: egui::Pos2, b: egui::Pos2| -> bool {
            a.distance(b) <= Self::VERTEX_RADIUS / ehc.ui_scale
        };

        fn dist_to_line_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
            fn dist2(a: egui::Pos2, b: egui::Pos2) -> f32 {
                (a.x - b.x).powf(2.0) + (a.y - b.y).powf(2.0)
            }
            let l2 = dist2(a, b);
            let distance_squared = if l2 == 0.0 {
                dist2(p, a)
            } else {
                let t =
                (((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2).clamp(0.0, 1.0);
                dist2(
                    p,
                    egui::Pos2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)),
                )
            };
            return distance_squared.sqrt();
        }

        match event {
            InputEvent::MouseDown(pos) => {
                // Either add a new node and drag it or mark existing node as dragged

                // Check whether over center point
                match self.center_point {
                    UFOption::Some((uuid, pos2)) if is_over(pos, pos2) => {
                        self.dragged_node = Some((uuid, pos));
                        return EventHandlingStatus::HandledByContainer;
                    }
                    // TODO: this is generally wrong (why??)
                    UFOption::None if is_over(pos, self.position()) => {
                        self.dragged_node = Some((uuid::Uuid::now_v7().into(), pos));
                        commands.push(InsensitiveCommand::AddElement(
                            *self.uuid,
                            VertexInformation {
                                after: uuid::Uuid::nil().into(),
                                id: self.dragged_node.unwrap().0,
                                position: self.position(),
                            }
                            .into(),
                            false,
                        ).into());

                        return EventHandlingStatus::HandledByContainer;
                    }
                    _ => {}
                }

                // Check whether over midpoint, if so add a new joint
                macro_rules! check_midpoints {
                    ($v:ident) => {
                        for path in &mut self.$v {
                            // Iterates over 2-windows
                            let mut iter = path
                            .iter()
                            .map(|e| *e)
                            .chain(self.center_point.as_ref().cloned())
                            .peekable();
                            while let Some(u) = iter.next() {
                                let v = if let Some(v) = iter.peek() {
                                    *v
                                } else {
                                    break;
                                };

                                let midpoint = (u.1 + v.1.to_vec2()) / 2.0;
                                if is_over(pos, midpoint) {
                                    self.dragged_node = Some((uuid::Uuid::now_v7().into(), pos));
                                    commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::AddElement(
                                        *self.uuid,
                                        VertexInformation {
                                            after: u.0,
                                            id: self.dragged_node.unwrap().0,
                                            position: pos,
                                        }
                                        .into(),
                                        false,
                                    )));

                                    return EventHandlingStatus::HandledByContainer;
                                }
                            }
                        }
                    };
                }
                check_midpoints!(source_points);
                check_midpoints!(dest_points);

                // Check whether over a joint, if so drag it
                macro_rules! check_joints {
                    ($v:ident) => {
                        for path in &mut self.$v {
                            let stop_idx = path.len();
                            for joint in &mut path[1..stop_idx] {
                                if is_over(pos, joint.1) {
                                    self.dragged_node = Some((joint.0, pos));

                                    return EventHandlingStatus::HandledByContainer;
                                }
                            }
                        }
                    };
                }
                check_joints!(source_points);
                check_joints!(dest_points);

                EventHandlingStatus::NotHandled
            },
            InputEvent::MouseUp(_) => {
                if self.dragged_node.take().is_some() {
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            },
            InputEvent::Click(pos) => {
                macro_rules! handle_vertex_click {
                    ($uuid:expr) => {
                        if !ehc.modifiers.command {
                            commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::SelectAll(false)));
                            commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::SelectSpecific(
                                std::iter::once(*$uuid).collect(),
                                true,
                            )));
                        } else {
                            commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::SelectSpecific(
                                std::iter::once(*$uuid).collect(),
                                !self.selected_vertices.contains($uuid),
                            )));
                        }
                        return EventHandlingStatus::HandledByContainer;
                    };
                }

                if let Some((uuid, _)) = self.center_point.as_ref().filter(|e| is_over(pos, e.1)) {
                    handle_vertex_click!(uuid);
                }

                macro_rules! check_joints_click {
                    ($pos:expr, $v:ident) => {
                        for path in &self.$v {
                            let stop_idx = path.len();
                            for joint in &path[1..stop_idx] {
                                if is_over($pos, joint.1) {
                                    handle_vertex_click!(&joint.0);
                                }
                            }
                        }
                    };
                }

                check_joints_click!(pos, source_points);
                check_joints_click!(pos, dest_points);

                // Check segments on paths
                macro_rules! check_path_segments {
                    ($v:ident) => {
                        //let center_point = self.center_point.clone();
                        for path in &self.$v {
                            // Iterates over 2-windows
                            let mut iter = path.iter().map(|e| *e)
                                .chain(self.center_point.as_ref().cloned()).peekable();
                            while let Some(u) = iter.next() {
                                let v = if let Some(v) = iter.peek() {
                                    *v
                                } else {
                                    break;
                                };

                                if dist_to_line_segment(pos, u.1, v.1) <= SEGMENT_DISTANCE_THRESHOLD {
                                    return EventHandlingStatus::HandledByElement;
                                }
                            }
                        }
                    };
                }
                check_path_segments!(source_points);
                check_path_segments!(dest_points);

                // In case there is no center_point, also check all-to-all of last points
                if self.center_point == UFOption::None {
                    for u in self.source_points.iter().flat_map(|e| e.last()) {
                        for v in self.dest_points.iter().flat_map(|e| e.last()) {
                            if dist_to_line_segment(pos, u.1, v.1) <= SEGMENT_DISTANCE_THRESHOLD {
                                return EventHandlingStatus::HandledByElement;
                            }
                        }
                    }
                }
                EventHandlingStatus::NotHandled
            },
            InputEvent::Drag { delta, .. } => {
                let Some(dragged_node) = self.dragged_node else {
                    return EventHandlingStatus::NotHandled;
                };

                let translated_real_pos = dragged_node.1 + delta;
                self.dragged_node = Some((dragged_node.0, translated_real_pos));
                let translated_real_shape = NHShape::Rect { inner: egui::Rect::from_min_size(translated_real_pos, egui::Vec2::ZERO) };
                let coerced_pos = if self.highlight.selected {
                    ehc.snap_manager.coerce(translated_real_shape, |e| !ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected))
                } else {
                    ehc.snap_manager.coerce(translated_real_shape, |e| *e != *self.uuid)
                };
                let coerced_delta = coerced_pos - self.all_vertices()
                    .find(|e| e.0 == dragged_node.0).unwrap().1;

                if self.selected_vertices.contains(&dragged_node.0) {
                    commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
                } else {
                    commands.push(InsensitiveCommand::MoveSpecificElements(
                        std::iter::once(dragged_node.0).collect(),
                        coerced_delta,
                    ).into());
                }

                EventHandlingStatus::HandledByContainer
            },
        }
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
        affected_models: &mut HashSet<ModelUuid>,
    ) {
        macro_rules! all_pts_mut {
            ($self:ident) => {
                $self
                    .center_point
                    .as_mut()
                    .into_iter()
                    .chain($self.source_points.iter_mut().flatten())
                    .chain($self.dest_points.iter_mut().flatten())
            };
        }
        match command {
            InsensitiveCommand::SelectAll(select) => {
                self.highlight.selected = *select;
                match select {
                    false => self.selected_vertices.clear(),
                    true => {
                        for p in all_pts_mut!(self) {
                            self.selected_vertices.insert(p.0);
                        }
                    }
                }
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight.selected = *select;
                }
                match select {
                    false => self.selected_vertices.retain(|e| !uuids.contains(e)),
                    true => {
                        for p in all_pts_mut!(self).filter(|e| uuids.contains(&e.0)) {
                            self.selected_vertices.insert(p.0);
                        }
                    }
                }
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = all_pts_mut!(self).find(|p| !rect.contains(p.1)).is_none();
            }
            InsensitiveCommand::MoveSpecificElements(uuids, delta) if !uuids.contains(&*self.uuid) => {
                for p in all_pts_mut!(self).filter(|e| uuids.contains(&e.0)) {
                    p.1 += *delta;
                    undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                        std::iter::once(p.0).collect(),
                        -*delta,
                    ));
                }
            }
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                for p in all_pts_mut!(self) {
                    p.1 += *delta;
                    undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                        std::iter::once(p.0).collect(),
                        -*delta,
                    ));
                }
            }
            InsensitiveCommand::ResizeSpecificElementsBy(..)
            | InsensitiveCommand::ResizeSpecificElementsTo(..)
            | InsensitiveCommand::CutSpecificElements(..)
            | InsensitiveCommand::PasteSpecificElements(..)
            | InsensitiveCommand::ArrangeSpecificElements(..) => {}
            InsensitiveCommand::DeleteSpecificElements(uuids, _) => {
                let self_uuid = *self.uuid;
                if let Some(center_point) =
                    self.center_point.as_mut().filter(|e| uuids.contains(&e.0))
                {
                    undo_accumulator.push(InsensitiveCommand::AddElement(
                        self_uuid,
                        DomainT::AddCommandElementT::from(VertexInformation {
                            after: uuid::Uuid::nil().into(),
                            id: center_point.0,
                            position: center_point.1,
                        }),
                        false,
                    ));

                    // Move any last point to the center
                    self.center_point = 'a: {
                        if let Some(path) = self.source_points.iter_mut().filter(|p| p.len() > 1).next() {
                            break 'a path.pop().into();
                        }
                        if let Some(path) = self.dest_points.iter_mut().filter(|p| p.len() > 1).next() {
                            break 'a path.pop().into();
                        }
                        None.into()
                    };
                }

                macro_rules! delete_vertices {
                    ($self:ident, $v:ident) => {
                        for path in $self.$v.iter_mut() {
                            // 2-windows over vertices
                            let mut iter = path.iter().peekable();
                            while let Some(a) = iter.next() {
                                let Some(b) = iter.peek() else {
                                    break;
                                };
                                if uuids.contains(&b.0) {
                                    undo_accumulator.push(InsensitiveCommand::AddElement(
                                        self_uuid,
                                        DomainT::AddCommandElementT::from(VertexInformation {
                                            after: a.0,
                                            id: b.0,
                                            position: b.1,
                                        }),
                                        false,
                                    ));
                                }
                            }

                            path.retain(|e| !uuids.contains(&e.0));
                        }
                    };
                }
                delete_vertices!(self, source_points);
                delete_vertices!(self, dest_points);
            }
            InsensitiveCommand::AddElement(target, element, _) => {
                if *target == *self.uuid {
                    if let Ok(VertexInformation {
                        after,
                        id,
                        position,
                    }) = element.clone().try_into()
                    {
                        if after.is_nil() {
                            // Push popped center point point back to its original path
                            if let Some(o) = self.center_point.as_ref().and_then(|e| self.point_to_origin.get(&e.0)) {
                                if !o.0 {
                                    self.source_points[o.1].push(self.center_point.unwrap());
                                } else {
                                    self.dest_points[o.1].push(self.center_point.unwrap());
                                }
                            }

                            self.center_point = UFOption::Some((id, position));

                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(id).collect(),
                                false,
                            ));
                        } else {
                            macro_rules! insert_vertex {
                                ($self:ident, $v:ident, $b:expr) => {
                                    for (idx1, path) in $self.$v.iter_mut().enumerate() {
                                        for (idx2, p) in path.iter().enumerate() {
                                            if p.0 == after {
                                                $self.point_to_origin.insert(id, ($b, idx1));
                                                path.insert(idx2 + 1, (id, position));
                                                undo_accumulator.push(
                                                    InsensitiveCommand::DeleteSpecificElements(
                                                        std::iter::once(id).collect(),
                                                        false,
                                                    ),
                                                );
                                                return;
                                            }
                                        }
                                    }
                                };
                            }
                            insert_vertex!(self, source_points, false);
                            insert_vertex!(self, dest_points, true);
                        }
                    }
                }
            }
            InsensitiveCommand::PropertyChange(uuids, properties) => {
                if uuids.contains(&*self.uuid) {
                    for property in properties {
                        if let Ok(FlipMulticonnection {}) = property.try_into() {}
                    }
                    self.adapter.apply_change(&self.uuid, command, undo_accumulator);
                    affected_models.insert(*self.adapter.model_uuid());
                }
            }
        }
    }
    fn refresh_buffers(&mut self) {
        self.adapter.refresh_buffers();
    }

    fn head_count(
        &mut self,
        flattened_views: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
        flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
    ) {
        flattened_views_status.insert(*self.uuid, self.highlight.selected.into());
        flattened_represented_models.insert(*self.adapter.model_uuid(), *self.uuid);

        for e in self.all_vertices() {
            flattened_views_status.insert(e.0, match self.selected_vertices.contains(&e.0) {
                true => SelectionStatus::Selected,
                false if self.highlight.selected => SelectionStatus::TransitivelySelected,
                false => SelectionStatus::NotSelected,
            });
        }
    }
    fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
        deleting.contains(&self.source.uuid()) || deleting.contains(&self.target.uuid())
    }

    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&ViewUuid) -> bool,
        tlc: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        c: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &mut HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {
        let (view_uuid, model_uuid) = if uuid_present(&*self.uuid) {
            (uuid::Uuid::now_v7().into(), uuid::Uuid::now_v7().into())
        } else {
            (*self.uuid, *self.model_uuid())
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            adapter: self.adapter.deep_copy_init(model_uuid, m),
            source: self.source.clone(),
            target: self.target.clone(),
            dragged_node: None,
            highlight: self.highlight,
            selected_vertices: self.selected_vertices.clone(),
            center_point: self.center_point.clone(),
            source_points: self.source_points.clone(),
            dest_points: self.dest_points.clone(),
            point_to_origin: self.point_to_origin.clone(),
        });
        tlc.insert(view_uuid, cloneish.clone().into());
        c.insert(*self.uuid, cloneish.clone().into());
    }

    fn deep_copy_relink(
        &mut self,
        c: &HashMap<ViewUuid, DomainT::CommonElementViewT>,
        m: &HashMap<ModelUuid, DomainT::CommonElementT>,
    ) {
        self.adapter.deep_copy_finish(m);

        if let Some(s) = c.get(&self.source.uuid()) {
            self.source = s.clone();
        }
        if let Some(d) = c.get(&self.target.uuid()) {
            self.target = d.clone();
        }
    }
}

/*
fn arrowhead_combo(ui: &mut egui::Ui, name: &str, val: &mut ArrowheadType) -> egui::Response {
    egui::ComboBox::from_id_salt(name)
        .selected_text(val.name())
        .show_ui(ui, |ui| {
            for sv in [ArrowheadType::None, ArrowheadType::OpenTriangle,
                       ArrowheadType::EmptyTriangle, ArrowheadType::FullTriangle,
                       ArrowheadType::EmptyRhombus, ArrowheadType::FullRhombus] {
                ui.selectable_value(val, sv, sv.name());
            }
        }).response
}
*/
