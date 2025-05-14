use crate::common::canvas::{self, NHCanvas, NHShape, UiCanvas};
use crate::CustomTab;
use eframe::{egui, epaint};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::{Arc, RwLock, Weak};

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


pub struct SnapManager {
    input_restriction: egui::Rect,
    max_delta: egui::Vec2,
    guidelines_x: Vec<(f32, egui::Align, uuid::Uuid)>,
    guidelines_y: Vec<(f32, egui::Align, uuid::Uuid)>,
    best_xy: Arc<RwLock<(Option<f32>, Option<f32>)>>,
}

impl SnapManager {
    pub fn new(input_restriction: egui::Rect, max_delta: egui::Vec2) -> Self {
        Self {
            input_restriction, max_delta,
            guidelines_x: Vec::new(), guidelines_y: Vec::new(),
            best_xy: Arc::new(RwLock::new((None, None))),
        }
    }
    pub fn add_shape(&mut self, uuid: uuid::Uuid, shape: canvas::NHShape) {
        let guidelines = shape.guidelines();
        if guidelines.iter().any(|e| self.input_restriction.contains(e.0)) {
            for e in guidelines.into_iter() {
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
    where F: Fn(&uuid::Uuid) -> bool
    {
        *self.best_xy.write().unwrap() = (None, None);
        let (mut least_x, mut least_y): (Option<(f32, f32)>, Option<(f32, f32)>) = (None, None);
        let center = s.center();
        
        // Naive guidelines coordinate matching
        for p in s.guidelines().into_iter() {
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


#[derive(Clone)]
pub enum ProjectCommand {
    OpenAndFocusDiagram(uuid::Uuid),
    UndoImmediate,
    RedoImmediate,
    AddCustomTab(uuid::Uuid, Arc<RwLock<dyn CustomTab>>),
    SetSvgExportMenu(Option<(usize, Arc<RwLock<dyn DiagramController>>, std::path::PathBuf, usize, bool, bool, f32, f32)>),
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
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
}

pub struct DrawingContext<'a> {
    pub profile: &'a ColorProfile,
    pub fluent_bundle: &'a fluent_bundle::FluentBundle<fluent_bundle::FluentResource>,
}

pub trait DiagramController: Any {
    fn uuid(&self) -> Arc<uuid::Uuid>;
    fn model_name(&self) -> Arc<String>;

    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, undo_accumulator: &mut Vec<Arc<String>>);
    
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

    fn show_toolbar(&mut self, ui: &mut egui::Ui);
    fn show_properties(&mut self, ui: &mut egui::Ui, undo_accumulator: &mut Vec<Arc<String>>);
    fn show_layers(&self, ui: &mut egui::Ui);
    fn show_menubar_edit_options(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);
    fn show_menubar_diagram_options(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);
    fn list_in_project_hierarchy(&self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);

    // This hurts me at least as much as it hurts you
    //fn outgoing_for<'a>(&'a self, _uuid: &'a uuid::Uuid) -> Box<dyn Iterator<Item=Arc<RwLock<dyn ElementController>>> + 'a> {
    //    Box::new(std::iter::empty::<Arc<RwLock<dyn ElementController>>>())
    //}

    fn apply_command(&mut self, command: DiagramCommand, global_undo: &mut Vec<Arc<String>>);
}

pub trait ElementController<CommonElementT: ?Sized> {
    fn uuid(&self) -> Arc<uuid::Uuid>;
    fn model_name(&self) -> Arc<String>;
    fn model(&self) -> Arc<RwLock<CommonElementT>>;

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
    DeleteSelectedElements,
    CutSelectedElements,
    PasteClipboardElements,
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
        F: Fn() -> HashSet<uuid::Uuid>,
        G: Fn() -> Vec<ElementT>
    {
        use SensitiveCommand as SC;
        use InsensitiveCommand as IC;
        if let SC::Insensitive(inner) = self {
            return inner;
        }
        if let SC::PasteClipboardElements = self {
            return IC::PasteSpecificElements(uuid::Uuid::nil(), clipboard_elements());
        }
        
        let se = selected_elements();
        match self {
            SC::MoveSelectedElements(delta) => IC::MoveSpecificElements(se, delta),
            SC::ResizeSelectedElementsBy(align, delta) => IC::ResizeSpecificElementsBy(se, align, delta),
            SC::ResizeSelectedElementsTo(align, delta) => IC::ResizeSpecificElementsTo(se, align, delta),
            SC::DeleteSelectedElements => IC::DeleteSpecificElements(se),
            SC::CutSelectedElements => IC::CutSpecificElements(se),
            SC::PropertyChangeSelected(changes) => IC::PropertyChange(se, changes),
            SC::Insensitive(..) | SC::PasteClipboardElements => unreachable!(),
        }
    }
}

/// Selection insensitive command - inherently repeatable
#[derive(Clone, PartialEq, Debug)]
pub enum InsensitiveCommand<ElementT: Clone + Debug, PropChangeT: Clone + Debug> {
    SelectAll(bool),
    SelectSpecific(HashSet<uuid::Uuid>, bool),
    SelectByDrag(egui::Rect),
    MoveAllElements(egui::Vec2),
    MoveSpecificElements(HashSet<uuid::Uuid>, egui::Vec2),
    ResizeSpecificElementsBy(HashSet<uuid::Uuid>, egui::Align2, egui::Vec2),
    ResizeSpecificElementsTo(HashSet<uuid::Uuid>, egui::Align2, egui::Vec2),
    DeleteSpecificElements(HashSet<uuid::Uuid>),
    CutSpecificElements(HashSet<uuid::Uuid>),
    PasteSpecificElements(uuid::Uuid, Vec<ElementT>),
    AddElement(uuid::Uuid, ElementT),
    PropertyChange(HashSet<uuid::Uuid>, Vec<PropChangeT>),
}

impl<ElementT: Clone + Debug, PropChangeT: Clone + Debug> Into<SensitiveCommand<ElementT, PropChangeT>> for InsensitiveCommand<ElementT, PropChangeT> {
    fn into(self) -> SensitiveCommand<ElementT, PropChangeT> {
        SensitiveCommand::Insensitive(self)
    }
}

impl<ElementT: Clone + Debug, PropChangeT: Clone + Debug>
    InsensitiveCommand<ElementT, PropChangeT>
{
    fn info_text(&self) -> Arc<String> {
        match self {
            InsensitiveCommand::SelectAll(..) | InsensitiveCommand::SelectSpecific(..) | InsensitiveCommand::SelectByDrag(..) => {
                Arc::new("Sorry, your undo stack is broken now :/".to_owned())
            }
            InsensitiveCommand::DeleteSpecificElements(uuids) => Arc::new(format!("Delete {} elements", uuids.len())),
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

pub fn arc_to_usize<T: ?Sized>(e: &Arc<T>) -> usize {
    Arc::as_ptr(e) as *const () as usize
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Weak;
    
    #[test]
    fn arc_to_usize_test() {
        let data = "Hello, world!\nThis is a test.\n";
        let cursor = std::io::Cursor::new(data);
        // Create base struct
        let reader = Arc::new(RwLock::new(std::io::BufReader::new(cursor)));
        // Create simple clone
        let clone1 = reader.clone();
        // Create dyn Clone 1
        let clone2: Arc<RwLock<dyn std::io::BufRead>> = clone1.clone();
        // Create dyn Clone 2
        let clone3: Arc<RwLock<dyn std::io::Read>> = clone2.clone();
        // Upgraded weak
        let clone4 = Weak::upgrade(&Arc::downgrade(&reader)).unwrap();
        
    
        // Assert all obtained identifiers are equal
        let base = arc_to_usize(&reader);
        assert_eq!(base, arc_to_usize(&clone1));
        assert_eq!(base, arc_to_usize(&clone2));
        assert_eq!(base, arc_to_usize(&clone3));
        assert_eq!(base, arc_to_usize(&clone4));
    }
}

pub trait Model: 'static {
    fn uuid(&self) -> Arc<uuid::Uuid>;
    fn name(&self) -> Arc<String>;
}

pub trait ContainerModel<ModelT: ?Sized>: Model {
    fn add_element(&mut self, _: Arc<RwLock<ModelT>>);
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>);
}

pub trait Tool<CommonElementT: ?Sized, QueryableT, AddCommandElementT, PropChangeT> {
    type KindedElement<'a>;
    type Stage;

    fn initial_stage(&self) -> Self::Stage;

    fn targetting_for_element(&self, controller: Self::KindedElement<'_>) -> egui::Color32;
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2);

    fn add_position(&mut self, pos: egui::Pos2);
    fn add_element(&mut self, controller: Self::KindedElement<'_>);
    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<CommonElementT, QueryableT, Self, AddCommandElementT, PropChangeT>,
    ) -> Option<(
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    CommonElementT,
                    QueryableT,
                    Self,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    )>;
    fn reset_event_lock(&mut self);
}

pub trait ContainerGen2<CommonElementT: ?Sized, QueryableT, ToolT, AddCommandElementT, PropChangeT>
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    CommonElementT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    >;
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
    pub all_elements: &'a HashMap<uuid::Uuid, SelectionStatus>,
    pub snap_manager: &'a SnapManager,
}

pub trait ElementControllerGen2<
    CommonElementT: ?Sized,
    QueryableT,
    ToolT,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
>: ElementController<CommonElementT> + ContainerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT> + Send + Sync where
    ToolT: Tool<CommonElementT, QueryableT, AddCommandElementT, PropChangeT>,
{
    fn show_properties(
        &mut self,
        _: &QueryableT,
        _ui: &mut egui::Ui,
        _commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> bool {
        false
    }
    fn list_in_project_hierarchy(&self, _: &QueryableT, _ui: &mut egui::Ui) {}

    fn draw_in(
        &mut self,
        _: &QueryableT,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus;
    fn collect_allignment(&mut self, am: &mut SnapManager) {
        am.add_shape(*self.uuid(), self.min_shape());
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> EventHandlingStatus;
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    );
    fn head_count(&mut self, into: &mut HashMap<uuid::Uuid, SelectionStatus>);
    
    // Create a deep copy, including the models
    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<uuid::Uuid>>,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        tlc: &mut HashMap<uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
        >,
        c: &mut HashMap<usize, (uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<CommonElementT>>,
            Arc<dyn Any + Send + Sync>,
        )>)
    {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        tlc: &mut HashMap<uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
        >,
        c: &mut HashMap<usize, (uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<CommonElementT>>,
            Arc<dyn Any + Send + Sync>,
        )>);
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<usize, (uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &HashMap<usize, (
            Arc<RwLock<CommonElementT>>,
            Arc<dyn Any + Send + Sync>,
        )>,
    ) {}
}

/// This is a generic DiagramController implementation.
/// Hopefully it should reduce the amount of code, but nothing prevents creating fully custom DiagramController implementations.
pub struct DiagramControllerGen2<
    DiagramModelT: ContainerModel<ElementModelT>,
    ElementModelT: ?Sized + 'static,
    QueryableT,
    BufferT,
    ToolT,
    AddCommandElementT: Clone + Debug + 'static,
    PropChangeT: Clone + Debug + 'static,
> where
    ToolT: Tool<ElementModelT, QueryableT, AddCommandElementT, PropChangeT>,
{
    model: Arc<RwLock<DiagramModelT>>,
    self_reference: Weak<RwLock<Self>>,
    owned_controllers: HashMap<
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    >,
    event_order: Vec<uuid::Uuid>,
    all_elements: HashMap<uuid::Uuid, SelectionStatus>,
    clipboard_elements: HashMap<uuid::Uuid, Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >>,

    pub _layers: Vec<bool>,

    pub camera_offset: egui::Pos2,
    pub camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_interactive_canvas_rect: egui::Rect,
    snap_manager: SnapManager,
    current_tool: Option<ToolT>,
    select_by_drag: Option<(egui::Pos2, egui::Pos2)>,
    
    last_change_flag: bool,
    undo_stack: Vec<(
        InsensitiveCommand<AddCommandElementT, PropChangeT>,
        Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    )>,
    redo_stack: Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,

    // q: dyn Fn(&Vec<DomainElementT>) -> QueryableT,
    queryable: QueryableT,
    buffer: BufferT,
    show_props_fun: fn(
        &mut BufferT,
        &mut egui::Ui,
        &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ),
    apply_property_change_fun: fn(
        &mut BufferT,
        &mut DiagramModelT,
        &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    ),

    tool_change_fun: fn(&mut Option<ToolT>, &mut egui::Ui),
    menubar_options_fun: fn(&mut Self, &mut egui::Ui, &mut Vec<ProjectCommand>),
}

impl<
        DiagramModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    >
    DiagramControllerGen2<
        DiagramModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    ToolT: for<'a> Tool<
        ElementModelT,
        QueryableT,
        AddCommandElementT,
        PropChangeT,
        KindedElement<'a>: From<&'a Self>,
    >,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )>,
{
    pub fn new(
        model: Arc<RwLock<DiagramModelT>>,
        owned_controllers: HashMap<
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        >,
        queryable: QueryableT,
        buffer: BufferT,
        show_props_fun: fn(
            &mut BufferT,
            &mut egui::Ui,
            &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
        ),
        apply_property_change_fun: fn(
            &mut BufferT,
            &mut DiagramModelT,
            &InsensitiveCommand<AddCommandElementT, PropChangeT>,
            &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
        ),
        tool_change_fun: fn(&mut Option<ToolT>, &mut egui::Ui),
        menubar_options_fun: fn(&mut Self, &mut egui::Ui, &mut Vec<ProjectCommand>),
    ) -> Arc<RwLock<Self>> {
        let event_order = owned_controllers.keys().map(|e| *e).collect();
        let ret = Arc::new(RwLock::new(Self {
            model,
            self_reference: Weak::new(),
            owned_controllers,
            event_order,
            all_elements: HashMap::new(),
            clipboard_elements: HashMap::new(),

            _layers: vec![true],

            camera_offset: egui::Pos2::ZERO,
            camera_scale: 1.0,
            last_unhandled_mouse_pos: None,
            last_interactive_canvas_rect: egui::Rect::ZERO,
            snap_manager: SnapManager::new(egui::Rect::ZERO, egui::Vec2::ZERO),
            current_tool: None,
            select_by_drag: None,
            
            last_change_flag: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),

            queryable,
            buffer,
            show_props_fun,
            apply_property_change_fun,
            tool_change_fun,
            menubar_options_fun,
        }));
        ret.write().unwrap().self_reference = Arc::downgrade(&ret);
        ret
    }

    pub fn model(&self) -> Arc<RwLock<DiagramModelT>> {
        self.model.clone()
    }

    fn self_reference_dyn(&self) -> Arc<RwLock<dyn DiagramController>> {
        self.self_reference.upgrade().unwrap()
    }
    
    fn handle_event(&mut self, event: InputEvent, modifiers: ModifierKeys, undo_accumulator: &mut Vec<Arc<String>>) -> bool {
        // Collect alignment guides
        self.snap_manager = SnapManager::new(self.last_interactive_canvas_rect, egui::Vec2::splat(10.0 / self.camera_scale));
        self.event_order.iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .for_each(|uc| uc.1.write().unwrap().collect_allignment(&mut self.snap_manager));
        self.snap_manager.sort_guidelines();
        
        // Handle events
        let mut commands = Vec::new();
        
        if matches!(event, InputEvent::Click(_)) {
            self.current_tool.as_mut().map(|e| e.reset_event_lock());
        }
        
        let ehc = EventHandlingContext {
            modifiers,
            ui_scale: self.camera_scale,
            all_elements: &self.all_elements,
            snap_manager: &self.snap_manager,
        };
        
        let child = self.event_order
            .iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .map(|uc| (uc.0, uc.1.write().unwrap().handle_event(
                    event,
                    &ehc,
                    &mut self.current_tool,
                    &mut commands,
                )))
            .find(|e| e.1 != EventHandlingStatus::NotHandled)
            .map(|us| {
                match us.1 {
                    EventHandlingStatus::HandledByElement if matches!(event, InputEvent::Click(_)) => {
                        if !modifiers.command {
                            commands.push(InsensitiveCommand::SelectAll(false).into());
                            commands.push(InsensitiveCommand::SelectSpecific(
                                std::iter::once(us.0).collect(),
                                true,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::SelectSpecific(
                                std::iter::once(us.0).collect(),
                                !self.all_elements.get(&us.0).is_some_and(|e| e.selected()),
                            ).into());
                        }
                        EventHandlingStatus::HandledByContainer
                    }
                    a => a,
                }
            });
        
        let handled = match event {
            InputEvent::MouseDown(_) | InputEvent::MouseUp(_) | InputEvent::Drag { .. }
                if child.is_some() || self.current_tool.is_some() => child.is_some(),
            InputEvent::MouseDown(pos) => {
                self.select_by_drag = Some((pos, pos));
                true
            }
            InputEvent::MouseUp(_) => {
                self.select_by_drag = None;
                true
            }
            InputEvent::Drag{ delta, ..} => {
                if let Some((a,b)) = self.select_by_drag {
                    self.select_by_drag = Some((a, b + delta));
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
                    if let Some(t) = self.current_tool.as_mut() {
                        t.add_position(pos);
                    }
                }
                
                let mut tool = self.current_tool.take();
                if let Some(new_a) = tool.as_mut().and_then(|e| e.try_construct(self)) {
                    commands.push(InsensitiveCommand::AddElement(
                        *self.uuid(),
                        AddCommandElementT::from(new_a),
                    ).into());
                    handled = true;
                }
                self.current_tool = tool;

                handled
            },
        };
        
        self.apply_commands(commands, undo_accumulator, true, true);

        handled
    }

    fn set_clipboard(&mut self) {
        let selected = self.all_elements.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect();
        self.clipboard_elements = Self::element_deep_copy(
            Some(&selected),
            self.owned_controllers.iter().map(|e| (*e.0, e.1.clone())),
            |_| false,
        );
    }
    
    fn element_deep_copy<F, P>(requested: Option<&HashSet<uuid::Uuid>>, from: F, uuid_present: P) -> HashMap<uuid::Uuid, Arc<RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >>>
        where
            F: Iterator<Item=(uuid::Uuid, Arc<RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >>)>,
            P: Fn(&uuid::Uuid) -> bool,
    {
        let mut top_level_views = HashMap::new();
        let mut views = HashMap::new();
        let mut models = HashMap::new();
        
        for (_uuid, c) in from {
            let c = c.read().unwrap();
            c.deep_copy_walk(requested, &uuid_present, &mut top_level_views, &mut views, &mut models);
        }
        for (_usize, (_uuid, v1, v2)) in views.iter() {
            let mut v1 = v1.write().unwrap();
            v1.deep_copy_relink(&views, &models);
        }
        
        top_level_views
    }
    
    fn apply_commands(
        &mut self,
        commands: Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
        global_undo_accumulator: &mut Vec<Arc<String>>,
        save_to_undo_stack: bool,
        clear_redo_stack: bool,
    ) {
        for command in commands {
            // TODO: transitive closure of dependency when deleting elements
            let command = command.to_selection_insensitive(
                || self.all_elements.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect(),
                || Self::element_deep_copy(
                    None,
                    self.clipboard_elements.iter().map(|e| (*e.0, e.1.clone())),
                    |e| self.all_elements.get(e).is_some(),
                    ).into_iter().map(|e| e.into()).collect(),
            );

            let mut undo_accumulator = vec![];

            match &command {
                InsensitiveCommand::SelectAll(..)
                | InsensitiveCommand::SelectSpecific(..)
                | InsensitiveCommand::SelectByDrag(..)
                | InsensitiveCommand::MoveSpecificElements(..)
                | InsensitiveCommand::MoveAllElements(..)
                | InsensitiveCommand::ResizeSpecificElementsBy(..)
                | InsensitiveCommand::ResizeSpecificElementsTo(..) => {}
                InsensitiveCommand::DeleteSpecificElements(uuids)
                | InsensitiveCommand::CutSpecificElements(uuids) => {
                    for (uuid, element) in self
                        .owned_controllers
                        .iter()
                        .filter(|e| uuids.contains(&e.0))
                    {
                        undo_accumulator.push(InsensitiveCommand::AddElement(
                            *self.uuid(),
                            AddCommandElementT::from((*uuid, element.clone())),
                        ));
                    }

                    let mut self_m = self.model.write().unwrap();
                    self_m.delete_elements(uuids);

                    self.owned_controllers.retain(|k, _v| !uuids.contains(&k));
                    self.event_order.retain(|e| !uuids.contains(&e));
                }
                InsensitiveCommand::AddElement(target, element) => {
                    if *target == *self.uuid() {
                        if let Ok((uuid, element)) = element.clone().try_into() {
                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(uuid).collect(),
                            ));

                            let new_c = element.read().unwrap();
                            let mut self_m = self.model.write().unwrap();
                            self_m.add_element(new_c.model());
                            drop(new_c);

                            self.owned_controllers.insert(uuid, element);
                            self.event_order.push(uuid);
                        }
                    }
                }
                InsensitiveCommand::PasteSpecificElements(_, elements) => {
                    for element in elements {
                        if let Ok((uuid, element)) = element.clone().try_into() {
                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(uuid).collect(),
                            ));

                            let new_c = element.read().unwrap();
                            let mut self_m = self.model.write().unwrap();
                            self_m.add_element(new_c.model());
                            drop(new_c);

                            self.owned_controllers.insert(uuid, element);
                            self.event_order.push(uuid);
                        }
                    }
                }
                InsensitiveCommand::PropertyChange(uuids, _property) => {
                    if uuids.contains(&*self.uuid()) {
                        let mut m = self.model.write().unwrap();
                        (self.apply_property_change_fun)(
                            &mut self.buffer,
                            &mut m,
                            &command,
                            &mut undo_accumulator,
                        );
                    }
                }
            }

            for e in &self.owned_controllers {
                let mut e = e.1.write().unwrap();
                e.apply_command(&command, &mut undo_accumulator);
            }

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
                | InsensitiveCommand::PropertyChange(..) => false,
            };

            if !undo_accumulator.is_empty() {
                if clear_redo_stack {
                    self.redo_stack.clear();
                }
                if save_to_undo_stack {
                    if let Some(merged) = self.undo_stack.last()
                        .filter(|_| self.last_change_flag)
                        .and_then(|e| e.0.merge(&command))
                    {
                        let last = self.undo_stack.last_mut().unwrap();
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
                        self.undo_stack.push((command, undo_accumulator));
                    }
                }
            }

            if modifies_selection {
                self.all_elements = HashMap::new();
                for (_uuid, c) in &self.owned_controllers {
                    let mut c = c.write().unwrap();
                    c.head_count(&mut self.all_elements);
                }
            }
        }
    }
}

impl<
        DiagramModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > DiagramController
    for DiagramControllerGen2<
        DiagramModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    ToolT: for<'a> Tool<
        ElementModelT,
        QueryableT,
        AddCommandElementT,
        PropChangeT,
        KindedElement<'a>: From<&'a Self>,
    >,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )>,
{
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.model.read().unwrap().uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.model.read().unwrap().name()
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
            self.camera_offset,
            self.camera_scale,
            ui.ctx().pointer_interact_pos().map(|e| {
                ((e - self.camera_offset - painter_response.rect.min.to_vec2()) / self.camera_scale)
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
                ((e - self.camera_offset - canvas_pos.to_vec2()) / self.camera_scale).to_pos2()
            });
        
        self.last_interactive_canvas_rect = egui::Rect::from_min_size(self.camera_offset / -self.camera_scale, canvas_size / self.camera_scale);

        (Box::new(ui_canvas), painter_response, inner_mouse)
    }
    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, undo_accumulator: &mut Vec<Arc<String>>) {
        macro_rules! pos_to_abs {
            ($pos:expr) => {
                (($pos - self.camera_offset - response.rect.min.to_vec2()) / self.camera_scale).to_pos2()
            };
        }
        
        // Handle mouse_down/drag/click/mouse_up
        let modifiers = ui.input(|i| ModifierKeys::from_egui(&i.modifiers));
        ui.input(|is| is.events.iter()
            .for_each(|e| match e {
                egui::Event::PointerButton { pos, button, pressed, .. } if *pressed && *button == egui::PointerButton::Primary => {
                    self.last_unhandled_mouse_pos = Some(pos_to_abs!(*pos));
                    self.handle_event(InputEvent::MouseDown(pos_to_abs!(*pos)), modifiers, undo_accumulator);
                },
                _ => {}
            })
        );
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(old_pos) = self.last_unhandled_mouse_pos {
                let delta = response.drag_delta() / self.camera_scale;
                self.handle_event(InputEvent::Drag { from: old_pos, delta }, modifiers, undo_accumulator);
                self.last_unhandled_mouse_pos = Some(old_pos + delta);
            }
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.handle_event(InputEvent::Click(pos_to_abs!(pos)), modifiers, undo_accumulator);
            }
        }
        ui.input(|is| is.events.iter()
            .for_each(|e| match e {
                egui::Event::PointerButton { pos, button, pressed, .. } if !*pressed && *button == egui::PointerButton::Primary => {
                    self.handle_event(InputEvent::MouseUp(pos_to_abs!(*pos)), modifiers, undo_accumulator);
                    self.last_unhandled_mouse_pos = None;
                },
                _ => {}
            })
        );
        
        // Handle diagram drag
        if response.dragged_by(egui::PointerButton::Middle) {
            self.camera_offset += response.drag_delta();
        }

        // Handle diagram zoom
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta);

            let factor = if scroll_delta.y > 0.0 && self.camera_scale < 10.0 {
                1.5
            } else if scroll_delta.y < 0.0 && self.camera_scale > 0.01 {
                0.66
            } else {
                0.0
            };

            if factor != 0.0 {
                if let Some(cursor_pos) = ui.ctx().pointer_interact_pos() {
                    let old_factor = self.camera_scale;
                    self.camera_scale *= factor;
                    self.camera_offset -=
                        ((cursor_pos - self.camera_offset - response.rect.min.to_vec2())
                            / old_factor)
                            * (self.camera_scale - old_factor);
                }
            }
        }
    }
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        ui.label("asdf");
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        (self.tool_change_fun)(&mut self.current_tool, ui);
    }
    fn show_properties(&mut self, ui: &mut egui::Ui, undo_accumulator: &mut Vec<Arc<String>>) {
        let mut commands = Vec::new();

        if self
            .owned_controllers
            .iter()
            .find(|e| {
                e.1.write()
                    .unwrap()
                    .show_properties(&self.queryable, ui, &mut commands)
            })
            .is_none()
        {
            (self.show_props_fun)(&mut self.buffer, ui, &mut commands);
        }

        self.apply_commands(commands, undo_accumulator, true, true);
    }
    fn show_layers(&self, _ui: &mut egui::Ui) {
        // TODO: Layers???
    }
    fn show_menubar_edit_options(&mut self, _ui: &mut egui::Ui, _commands: &mut Vec<ProjectCommand>) {}
    fn show_menubar_diagram_options(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
        (self.menubar_options_fun)(self, ui, commands);
        
        if ui.button("Layout selected elements").clicked() {
            todo!();
        }
        ui.separator();
    }

    fn list_in_project_hierarchy(&self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
        let model = self.model.read().unwrap();

        let r = egui::CollapsingHeader::new(format!("{} ({})", model.name(), model.uuid())).show(
            ui,
            |ui| {
                for uc in &self.owned_controllers {
                    uc.1.read()
                        .unwrap()
                        .list_in_project_hierarchy(&self.queryable, ui);
                }
            },
        );
        
        // React to user interaction
        if r.header_response.double_clicked() {
            commands.push(ProjectCommand::OpenAndFocusDiagram(*model.uuid()));
        }
        
        r.header_response.context_menu(|ui| {
            if ui.button("Open").clicked() {
                commands.push(ProjectCommand::OpenAndFocusDiagram(*model.uuid()));
                ui.close_menu();
            } else if ui.button("Delete").clicked() {
                todo!("implement view deletion");
            }
        });
    }

    fn apply_command(
        &mut self,
        command: DiagramCommand,
        global_undo: &mut Vec<Arc<String>>,
    ) {
        match command {
            DiagramCommand::DropRedoStackAndLastChangeFlag => {
                self.redo_stack.clear();
                self.last_change_flag = false;
            },
            DiagramCommand::SetLastChangeFlag => {
                self.last_change_flag = true;
            },
            DiagramCommand::UndoImmediate => {
                let Some((og_command, undo_commands)) = self.undo_stack.pop() else {
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
                );
                self.redo_stack.push(og_command);
            },
            DiagramCommand::RedoImmediate => {
                let Some(redo_command) = self.redo_stack.pop() else {
                    return;
                };
                self.apply_commands(vec![redo_command.into()], &mut vec![], true, false);
            }
            DiagramCommand::SelectAllElements(select) => {
                self.apply_commands(vec![InsensitiveCommand::SelectAll(select).into()], &mut vec![], true, false);
            }
            DiagramCommand::InvertSelection => {
                self.apply_commands(vec![
                    InsensitiveCommand::SelectAll(true).into(),
                    InsensitiveCommand::SelectSpecific(self.all_elements.iter().filter(|e| e.1.selected()).map(|e| *e.0).collect(), false).into()
                ], &mut vec![], true, false);
            }
            DiagramCommand::DeleteSelectedElements
            | DiagramCommand::CutSelectedElements
            | DiagramCommand::PasteClipboardElements => {
                if matches!(command, DiagramCommand::CutSelectedElements) {
                    self.set_clipboard();
                }
                
                let mut undo = vec![];
                self.apply_commands(vec![
                    match command {
                        DiagramCommand::DeleteSelectedElements => SensitiveCommand::DeleteSelectedElements,
                        DiagramCommand::CutSelectedElements => SensitiveCommand::CutSelectedElements,
                        DiagramCommand::PasteClipboardElements => SensitiveCommand::PasteClipboardElements,
                        _ => unreachable!(),
                    }
                ], &mut undo, true, true);
                self.last_change_flag = true;
                global_undo.extend(undo.into_iter());
            }
            DiagramCommand::CopySelectedElements => {
                self.set_clipboard();
            },
        }
    }

    fn draw_in(
        &mut self,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        mouse_pos: Option<egui::Pos2>
    ) {
        let tool = if let (Some(pos), Some(stage)) = (mouse_pos, self.current_tool.as_ref()) {
            Some((pos, stage))
        } else {
            None
        };
        let mut drawn_targetting = TargettingStatus::NotDrawn;

        self.event_order
            .iter()
            .rev()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (k, e)))
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| {
                if uc
                    .1
                    .write()
                    .unwrap()
                    .draw_in(&self.queryable, context, canvas, &tool)
                    == TargettingStatus::Drawn
                {
                    drawn_targetting = TargettingStatus::Drawn;
                }
            });

        if canvas.ui_scale().is_some() {
            if let Some((pos, tool)) = tool {
                if drawn_targetting == TargettingStatus::NotDrawn {
                    canvas.draw_rectangle(
                        egui::Rect::EVERYTHING,
                        egui::CornerRadius::ZERO,
                        tool.targetting_for_element(ToolT::KindedElement::from(self)),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );
                    self.event_order
                        .iter()
                        .rev()
                        .flat_map(|k| self.owned_controllers.get(k).map(|e| (k, e)))
                        .filter(|_| true) // TODO: filter by layers
                        .for_each(|uc| {
                            uc.1.write()
                                .unwrap()
                                .draw_in(&self.queryable, context, canvas, &Some((pos, tool)));
                        });
                }
                tool.draw_status_hint(canvas, pos);
            } else if let Some((a, b)) = self.select_by_drag {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a, b),
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_rgba_premultiplied(0, 0, 255, 7),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLUE),
                    canvas::Highlight::NONE,
                );
            }
            
            self.snap_manager.draw_best(canvas, context.profile, self.last_interactive_canvas_rect);
        }
    }
}

impl<
        DiagramModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    > ContainerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for DiagramControllerGen2<
        DiagramModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    ToolT: Tool<ElementModelT, QueryableT, AddCommandElementT, PropChangeT>,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )>,
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    > {
        self.owned_controllers.get(uuid).cloned().or_else(|| self.owned_controllers.iter().flat_map(|e| e.1.read().unwrap().controller_for(uuid)).nth(0))
    }
}


pub trait PackageAdapter<
    ElementModelT: ?Sized + 'static,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
>: Send + Sync + 'static {
    fn model(&self) -> Arc<RwLock<ElementModelT>>;
    fn model_uuid(&self) -> Arc<uuid::Uuid>;
    fn model_name(&self) -> Arc<String>;
    
    fn add_element(&mut self, _: Arc<RwLock<ElementModelT>>);
    fn delete_elements(&mut self, uuids: &HashSet<uuid::Uuid>);

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>
    );
    fn apply_change(
        &self,
        command: &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    );
    
    fn deep_copy_init(
        &self,
        uuid: uuid::Uuid,
        m: &mut HashMap<usize, (Arc<RwLock<ElementModelT>>, Arc<dyn Any + Send + Sync>)>,
    ) -> Self where Self: Sized;
    fn deep_copy_finish(
        &mut self,
        m: &HashMap<usize, (Arc<RwLock<ElementModelT>>, Arc<dyn Any + Send + Sync>)>
    );
}

#[derive(Clone, Copy, PartialEq)]
pub enum PackageDragType {
    Move,
    Resize(egui::Align2),
}

pub struct PackageView<
    AdapterT: PackageAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
    ElementModelT: ?Sized + 'static,
    QueryableT: 'static,
    ToolT: 'static,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
> where
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + Clone
        + 'static,
{
    adapter: AdapterT,
    self_reference: Weak<RwLock<Self>>,
    owned_controllers: HashMap<
        uuid::Uuid,
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    >,
    event_order: Vec<uuid::Uuid>,
    all_elements: HashMap<uuid::Uuid, SelectionStatus>,
    selected_direct_elements: HashSet<uuid::Uuid>,

    dragged_type_and_shape: Option<(PackageDragType, egui::Rect)>,
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,
}

impl<
        AdapterT: PackageAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    >
    PackageView<AdapterT, ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
where
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + Clone
        + 'static,
{
    pub fn new(
        adapter: AdapterT,
        owned_controllers: HashMap<
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        >,
        bounds_rect: egui::Rect,
    ) -> Arc<RwLock<Self>> {
        let event_order = owned_controllers.keys().map(|e| *e).collect();
        let c = Arc::new(RwLock::new(
        Self {
            adapter,
            self_reference: Weak::new(),
            owned_controllers,
            event_order,
            all_elements: HashMap::new(),
            selected_direct_elements: HashSet::new(),

            dragged_type_and_shape: None,
            highlight: canvas::Highlight::NONE,
            bounds_rect,
        }));
        c.write().unwrap().self_reference = Arc::downgrade(&c);
        c
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

impl<
        AdapterT: PackageAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ElementController<ElementModelT>
    for PackageView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    ToolT: for<'a> Tool<
        ElementModelT,
        QueryableT,
        AddCommandElementT,
        PropChangeT,
        KindedElement<'a>: From<&'a Self>,
    >,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )>,
{
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.adapter.model_uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.adapter.model_name()
    }
    fn model(&self) -> Arc<RwLock<ElementModelT>> {
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

impl<
        AdapterT: PackageAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ContainerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for PackageView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )>,
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    > {
        self.owned_controllers.get(uuid).cloned().or_else(|| self.owned_controllers.iter().flat_map(|e| e.1.read().unwrap().controller_for(uuid)).nth(0))
    }
}

impl<
        AdapterT: PackageAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for PackageView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    ToolT: for<'a> Tool<
        ElementModelT,
        QueryableT,
        AddCommandElementT,
        PropChangeT,
        KindedElement<'a>: From<&'a Self>,
    >,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<
                        ElementModelT,
                        QueryableT,
                        ToolT,
                        AddCommandElementT,
                        PropChangeT,
                    >,
                >,
            >,
        )>,
{
    fn show_properties(
        &mut self,
        parent: &QueryableT,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> bool {
        if self
            .owned_controllers
            .iter()
            .find(|e| e.1.write().unwrap().show_properties(parent, ui, commands))
            .is_some()
        {
            true
        } else if self.highlight.selected {
            self.adapter.show_properties(ui, commands);
            true
        } else {
            false
        }
    }
    fn list_in_project_hierarchy(&self, parent: &QueryableT, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(format!("{} ({})", *self.adapter.model_name(), *self.adapter.model_uuid())).show(
            ui,
            |ui| {
                for (_uuid, c) in &self.owned_controllers {
                    let c = c.read().unwrap();
                    c.list_in_project_hierarchy(parent, ui);
                }
            },
        );
    }
    fn draw_in(
        &mut self,
        q: &QueryableT,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &ToolT)>,
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

        self.event_order
            .iter()
            .rev()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (k, e)))
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| {
                if uc.1.write().unwrap().draw_in(q, context, canvas, &tool) == TargettingStatus::Drawn
                {
                    drawn_child_targetting = TargettingStatus::Drawn;
                }
            });

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
                        t.targetting_for_element(ToolT::KindedElement::from(&*self)),
                        canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                        canvas::Highlight::NONE,
                    );

                    self.event_order
                        .iter()
                        .rev()
                        .flat_map(|k| self.owned_controllers.get(k).map(|e| (k, e)))
                        .filter(|_| true) // TODO: filter by layers
                        .for_each(|uc| {
                            uc.1.write().unwrap().draw_in(q, context, canvas, &tool);
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
        am.add_shape(*self.uuid(), self.min_shape());
        
        self.event_order.iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .for_each(|uc| uc.1.write().unwrap().collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> EventHandlingStatus {
        let uc_status = self
            .event_order
            .iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .map(|uc| {
                (
                    uc,
                    uc.1.write()
                        .unwrap()
                        .handle_event(event, ehc, tool, commands),
                )
            })
            .find(|e| e.1 != EventHandlingStatus::NotHandled);
        
        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if uc_status.is_some() => {
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
                        tool.add_element(ToolT::KindedElement::from(self));
                        
                        if let Some(new_a) = tool.try_construct(self) {
                            commands.push(InsensitiveCommand::AddElement(*self.uuid(), new_a.into()).into());
                        }
                        
                        EventHandlingStatus::HandledByContainer
                    } else if let Some((uc, status)) = uc_status {
                        if status == EventHandlingStatus::HandledByElement {
                            if !ehc.modifiers.command {
                                commands.push(InsensitiveCommand::SelectAll(false).into());
                                commands.push(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(uc.0).collect(),
                                    true,
                                ).into());
                            } else {
                                commands.push(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(uc.0).collect(),
                                    !self.selected_direct_elements.contains(&uc.0),
                                ).into());
                            }
                        }
                        EventHandlingStatus::HandledByContainer
                    } else {
                        EventHandlingStatus::HandledByElement
                    }
                } else {
                    uc_status.map(|e| e.1).unwrap_or(EventHandlingStatus::NotHandled)
                }
            },
            InputEvent::Drag { delta, .. } => match self.dragged_type_and_shape {
                Some((PackageDragType::Move, real_bounds)) => {
                    let translated_bounds = real_bounds.translate(delta);
                    self.dragged_type_and_shape = Some((PackageDragType::Move, translated_bounds));
                    let translated_real_shape = NHShape::Rect { inner: translated_bounds };
                    let coerced_pos = ehc.snap_manager.coerce(translated_real_shape,
                        |e| !self.all_elements.get(e).is_some() && !if self.highlight.selected { ehc.all_elements.get(e).is_some_and(|e| *e != SelectionStatus::NotSelected) } else {*e == *self.uuid()}
                    );
                    let coerced_delta = coerced_pos - self.position();
                    
                    if self.highlight.selected {
                        commands.push(SensitiveCommand::MoveSelectedElements(coerced_delta));
                    } else {
                        commands.push(InsensitiveCommand::MoveSpecificElements(
                            std::iter::once(*self.uuid()).collect(),
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
                    let new_real_bounds = real_bounds + epaint::Marginf { left, right, top, bottom };
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
        command: &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) {
        macro_rules! recurse {
            ($self:ident) => {
                for e in &$self.owned_controllers {
                    let mut e = e.1.write().unwrap();
                    e.apply_command(command, undo_accumulator);
                }
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
                
                let r = self.bounds_rect + epaint::Marginf{left, right, top, bottom};
                
                undo_accumulator.push(InsensitiveCommand::ResizeSpecificElementsTo(
                    std::iter::once(*self.uuid()).collect(),
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
                            self.owned_controllers.iter().map(|e| *e.0).collect()
                    }
                    false => self.selected_direct_elements.clear(),
                }
                recurse!(self);
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }

                for uuid in self.owned_controllers.keys().filter(|k| uuids.contains(k)) {
                    match select {
                        true => self.selected_direct_elements.insert(*uuid),
                        false => self.selected_direct_elements.remove(uuid),
                    };
                }

                recurse!(self);
            }
            InsensitiveCommand::SelectByDrag(rect) => {
                self.highlight.selected = self.min_shape().contained_within(*rect);
                
                recurse!(self);
            }
            InsensitiveCommand::MoveSpecificElements(uuids, _) if !uuids.contains(&*self.uuid()) => {
                recurse!(self);
            }
            InsensitiveCommand::MoveSpecificElements(_, delta) | InsensitiveCommand::MoveAllElements(delta) => {
                self.bounds_rect.set_center(self.position() + *delta);
                undo_accumulator.push(InsensitiveCommand::MoveSpecificElements(
                    std::iter::once(*self.uuid()).collect(),
                    -*delta,
                ));
                for e in &self.owned_controllers {
                    let mut e = e.1.write().unwrap();
                    e.apply_command(&InsensitiveCommand::MoveAllElements(*delta), &mut vec![]);
                }
            }
            InsensitiveCommand::ResizeSpecificElementsBy(uuids, align, delta) => {
                if uuids.contains(&self.uuid()) {
                    resize_by!(align, delta);
                }
                
                recurse!(self);
            }
            InsensitiveCommand::ResizeSpecificElementsTo(uuids, align, size) => {
                if uuids.contains(&self.uuid()) {
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
            InsensitiveCommand::DeleteSpecificElements(uuids)
            | InsensitiveCommand::CutSpecificElements(uuids) => {
                for (uuid, element) in self
                    .owned_controllers
                    .iter()
                    .filter(|e| uuids.contains(&e.0))
                {
                    undo_accumulator.push(InsensitiveCommand::AddElement(
                        *self.uuid(),
                        AddCommandElementT::from((*uuid, element.clone())),
                    ));
                }

                self.adapter.delete_elements(&uuids);
                self.owned_controllers.retain(|k, _v| !uuids.contains(&k));
                self.event_order.retain(|e| !uuids.contains(&e));

                recurse!(self);
            }
            InsensitiveCommand::AddElement(target, element) => {
                if *target == *self.uuid() {
                    if let Ok((uuid, element)) = element.clone().try_into() {
                        undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                            std::iter::once(uuid).collect(),
                        ));

                        let new_c = element.read().unwrap();
                        self.adapter.add_element(new_c.model());
                        drop(new_c);

                        self.owned_controllers.insert(uuid, element);
                        self.event_order.push(uuid);
                    }
                }

                recurse!(self);
            }
            InsensitiveCommand::PasteSpecificElements(target, _elements) => {
                if *target == *self.uuid() {
                    todo!("undo = delete")
                }
                
                recurse!(self);
            },
            InsensitiveCommand::PropertyChange(uuids, _property) => {
                if uuids.contains(&*self.uuid()) {
                    self.adapter.apply_change(
                        command,
                        undo_accumulator,
                    );
                }

                recurse!(self);
            }
        }
    }

    fn head_count(&mut self, into: &mut HashMap<uuid::Uuid, SelectionStatus>) {
        into.insert(*self.uuid(), self.highlight.selected.into());

        self.all_elements.clear();
        for e in &self.owned_controllers {
            let mut e = e.1.write().unwrap();
            e.head_count(&mut self.all_elements);
        }
        for e in &self.all_elements {
            into.insert(*e.0, match *e.1 {
                SelectionStatus::NotSelected if self.highlight.selected => SelectionStatus::TransitivelySelected,
                e => e,
            });
        }
    }
    
    fn deep_copy_walk(
        &self,
        requested: Option<&HashSet<uuid::Uuid>>,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        tlc: &mut HashMap<uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
        >,
        c: &mut HashMap<usize, (uuid::Uuid, 
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<ElementModelT>>,
            Arc<dyn Any + Send + Sync>,
        )>)
    {
        if requested.is_none_or(|e| e.contains(&self.uuid())) {
            self.deep_copy_clone(uuid_present, tlc, c, m);
        } else {
            self.owned_controllers.iter()
                .for_each(|e| e.1.read().unwrap().deep_copy_walk(requested, uuid_present, tlc, c, m));
        }
    }
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        tlc: &mut HashMap<uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
        >,
        c: &mut HashMap<usize, (uuid::Uuid, 
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<ElementModelT>>,
            Arc<dyn Any + Send + Sync>,
        )>
    ) {
        let uuid = if uuid_present(&*self.uuid()) { uuid::Uuid::now_v7() } else { *self.uuid() };
        
        let mut inner = HashMap::new();
        self.owned_controllers.iter().for_each(|e| e.1.read().unwrap().deep_copy_clone(uuid_present, &mut inner, c, m));
        
        let cloneish = Arc::new(RwLock::new(Self {
            adapter: self.adapter.deep_copy_init(uuid, m),
            self_reference: Weak::new(),
            owned_controllers: inner.iter().map(|e| (*e.0, e.1.clone())).collect(),
            event_order: inner.iter().map(|e| *e.0).collect(),
            all_elements: HashMap::new(),
            selected_direct_elements: self.selected_direct_elements.clone(),
            dragged_type_and_shape: None,
            highlight: self.highlight,
            bounds_rect: self.bounds_rect,
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        tlc.insert(uuid, cloneish.clone());
        c.insert(arc_to_usize(&Weak::upgrade(&self.self_reference).unwrap()),
            (uuid, cloneish.clone(), cloneish));
    }
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<usize, (uuid::Uuid, 
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &HashMap<usize, (
            Arc<RwLock<ElementModelT>>,
            Arc<dyn Any + Send + Sync>,
        )>,
    ) {
        self.owned_controllers.iter().for_each(|e| e.1.write().unwrap().deep_copy_relink(c, m));
    }
}

pub trait MulticonnectionAdapter<
    ElementModelT: ?Sized + 'static,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
>: Send + Sync {
    fn model(&self) -> Arc<RwLock<ElementModelT>>;
    fn model_uuid(&self) -> Arc<uuid::Uuid>;
    fn model_name(&self) -> Arc<String>;

    fn source_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>);
    fn destination_arrow(&self) -> (canvas::LineType, canvas::ArrowheadType, Option<Arc<String>>);

    fn show_properties(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>
    );
    fn apply_change(
        &self,
        command: &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    );
    
    fn deep_copy_init(
        &self,
        uuid: uuid::Uuid,
        m: &mut HashMap<usize, (Arc<RwLock<ElementModelT>>, Arc<dyn Any + Send + Sync>)>,
    ) -> Self where Self: Sized;
    fn deep_copy_finish(
        &mut self,
        m: &HashMap<usize, (Arc<RwLock<ElementModelT>>, Arc<dyn Any + Send + Sync>)>
    );
}

#[derive(Clone, Debug)]
pub struct VertexInformation {
    after: uuid::Uuid,
    id: uuid::Uuid,
    position: egui::Pos2,
}
#[derive(Clone, Debug)]
pub struct FlipMulticonnection {}

pub struct MulticonnectionView<
    AdapterT: MulticonnectionAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
    ElementModelT: ?Sized + 'static,
    QueryableT,
    ToolT,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
> where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    adapter: AdapterT,
    self_reference: Weak<RwLock<Self>>,

    pub source: Arc<
        RwLock<
            dyn ElementControllerGen2<
                ElementModelT,
                QueryableT,
                ToolT,
                AddCommandElementT,
                PropChangeT,
            >,
        >,
    >,
    pub destination: Arc<
        RwLock<
            dyn ElementControllerGen2<
                ElementModelT,
                QueryableT,
                ToolT,
                AddCommandElementT,
                PropChangeT,
            >,
        >,
    >,

    dragged_node: Option<(uuid::Uuid, egui::Pos2)>,
    highlight: canvas::Highlight,
    selected_vertices: HashSet<uuid::Uuid>,
    center_point: Option<(uuid::Uuid, egui::Pos2)>,
    source_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
    dest_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
    point_to_origin: HashMap<uuid::Uuid, (bool, usize)>,
}

impl<
        AdapterT: MulticonnectionAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT,
        ToolT,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    >
    MulticonnectionView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    pub fn new(
        adapter: AdapterT,
        source: Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
        destination: Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,

        center_point: Option<(uuid::Uuid, egui::Pos2)>,
        source_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
        dest_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
    ) -> Arc<RwLock<Self>> {
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
        
        let c = Arc::new(RwLock::new(
            Self {
                adapter,
                self_reference: Weak::new(),
                source,
                destination,
                dragged_node: None,
                highlight: canvas::Highlight::NONE,
                selected_vertices: HashSet::new(),

                center_point,
                source_points,
                dest_points,
                point_to_origin,
            }
        ));
        c.write().unwrap().self_reference = Arc::downgrade(&c);
        c
    }
    
    const VERTEX_RADIUS: f32 = 5.0;
    fn all_vertices(&self) -> impl Iterator<Item = &(uuid::Uuid, egui::Pos2)> {
        self.center_point.iter()
            .chain(self.source_points.iter().flat_map(|e| e.iter()))
            .chain(self.dest_points.iter().flat_map(|e| e.iter()))
    }
}

impl<
        AdapterT: MulticonnectionAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT,
        ToolT,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    > ElementController<ElementModelT>
    for MulticonnectionView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    fn uuid(&self) -> Arc<uuid::Uuid> {
        self.adapter.model_uuid()
    }
    fn model_name(&self) -> Arc<String> {
        self.adapter.model_name()
    }
    fn model(&self) -> Arc<RwLock<ElementModelT>> {
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
            Some(point) => point.1,
            None => (self.source_points[0][0].1 + self.dest_points[0][0].1.to_vec2()) / 2.0,
        }
    }
}

impl<
        AdapterT: MulticonnectionAdapter<ElementModelT, AddCommandElementT, PropChangeT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ContainerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for MulticonnectionView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    fn controller_for(
        &self,
        _uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<
                    ElementModelT,
                    QueryableT,
                    ToolT,
                    AddCommandElementT,
                    PropChangeT,
                >,
            >,
        >,
    > {
        None
    }
}

impl<
        AdapterT: MulticonnectionAdapter<ElementModelT, AddCommandElementT, PropChangeT> +'static,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for MulticonnectionView<
        AdapterT,
        ElementModelT,
        QueryableT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    ToolT: Tool<ElementModelT, QueryableT, AddCommandElementT, PropChangeT>,
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    fn show_properties(
        &mut self,
        _parent: &QueryableT,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> bool {
        if !self.highlight.selected {
            return false;
        }

        self.adapter.show_properties(ui, commands);

        true
    }

    fn draw_in(
        &mut self,
        _: &QueryableT,
        context: &DrawingContext,
        canvas: &mut dyn NHCanvas,
        _tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus {
        let source_bounds = self.source.read().unwrap().min_shape();
        let dest_bounds = self.destination.read().unwrap().min_shape();
        
        let (source_next_point, dest_next_point) = match (
            self.source_points[0].iter().skip(1)
                .map(|e| *e)
                .chain(self.center_point)
                .find(|p| !source_bounds.contains(p.1))
                .map(|e| e.1),
            self.dest_points[0].iter().skip(1)
                .map(|e| *e)
                .chain(self.center_point)
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
                Some(point) => *point,
                None => (
                    uuid::Uuid::nil(),
                    (self.source_points[0][0].1 + self.dest_points[0][0].1.to_vec2()) / 2.0,
                ),
            },
            None,
            self.highlight,
        );

        TargettingStatus::NotDrawn
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        for p in self.center_point.iter()
            .chain(self.source_points.iter().flat_map(|e| e.iter().skip(1)))
            .chain(self.dest_points.iter().flat_map(|e| e.iter().skip(1)))
        {
            am.add_shape(*self.uuid(), NHShape::Rect { inner: egui::Rect::from_min_size(p.1, egui::Vec2::ZERO) });
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
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
                    Some((uuid, pos2)) if is_over(pos, pos2) => {
                        self.dragged_node = Some((uuid, pos));
                        return EventHandlingStatus::HandledByContainer;
                    }
                    // TODO: this is generally wrong (why??)
                    None if is_over(pos, self.position()) => {
                        self.dragged_node = Some((uuid::Uuid::now_v7(), pos));
                        commands.push(InsensitiveCommand::AddElement(
                            *self.uuid(),
                            VertexInformation {
                                after: uuid::Uuid::nil(),
                                id: self.dragged_node.unwrap().0,
                                position: self.position(),
                            }
                            .into(),
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
                            .chain(self.center_point)
                            .peekable();
                            while let Some(u) = iter.next() {
                                let v = if let Some(v) = iter.peek() {
                                    *v
                                } else {
                                    break;
                                };
                                
                                let midpoint = (u.1 + v.1.to_vec2()) / 2.0;
                                if is_over(pos, midpoint) {
                                    self.dragged_node = Some((uuid::Uuid::now_v7(), pos));
                                    commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::AddElement(
                                        *self.uuid(),
                                        VertexInformation {
                                            after: u.0,
                                            id: self.dragged_node.unwrap().0,
                                            position: pos,
                                        }
                                        .into(),
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
                            let mut iter = path.iter().map(|e| *e).chain(self.center_point).peekable();
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
                if self.center_point == None {
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
                    ehc.snap_manager.coerce(translated_real_shape, |e| *e != *self.uuid())
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
        command: &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
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
                if uuids.contains(&*self.uuid()) {
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
            InsensitiveCommand::MoveSpecificElements(uuids, delta) if !uuids.contains(&*self.uuid()) => {
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
            | InsensitiveCommand::PasteSpecificElements(..) => {}
            InsensitiveCommand::DeleteSpecificElements(uuids) => {
                let self_uuid = *self.uuid();
                if let Some(center_point) =
                    self.center_point.as_mut().filter(|e| uuids.contains(&e.0))
                {
                    undo_accumulator.push(InsensitiveCommand::AddElement(
                        self_uuid,
                        AddCommandElementT::from(VertexInformation {
                            after: uuid::Uuid::nil(),
                            id: center_point.0,
                            position: center_point.1,
                        }),
                    ));
                    
                    // Move any last point to the center
                    self.center_point = 'a: {
                        if let Some(path) = self.source_points.iter_mut().filter(|p| p.len() > 1).nth(0) {
                            break 'a path.pop();
                        }
                        if let Some(path) = self.dest_points.iter_mut().filter(|p| p.len() > 1).nth(0) {
                            break 'a path.pop();
                        }
                        None
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
                                        AddCommandElementT::from(VertexInformation {
                                            after: a.0,
                                            id: b.0,
                                            position: b.1,
                                        }),
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
            InsensitiveCommand::AddElement(target, element) => {
                if *target == *self.uuid() {
                    if let Ok(VertexInformation {
                        after,
                        id,
                        position,
                    }) = element.clone().try_into()
                    {
                        if after.is_nil() {
                            // Push popped center point point back to its original path
                            if let Some(o) = self.center_point.and_then(|e| self.point_to_origin.get(&e.0)) {
                                if !o.0 {
                                    self.source_points[o.1].push(self.center_point.unwrap());
                                } else {
                                    self.dest_points[o.1].push(self.center_point.unwrap());
                                }
                            }
                            
                            self.center_point = Some((id, position));

                            undo_accumulator.push(InsensitiveCommand::DeleteSpecificElements(
                                std::iter::once(id).collect(),
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
                if uuids.contains(&*self.uuid()) {
                    for property in properties {
                        if let Ok(FlipMulticonnection {}) = property.try_into() {}
                    }
                    self.adapter.apply_change(command, undo_accumulator);
                }
            }
        }
    }

    fn head_count(&mut self, into: &mut HashMap<uuid::Uuid, SelectionStatus>) {
        into.insert(*self.uuid(), self.highlight.selected.into());
        
        for e in self.all_vertices() {
            into.insert(e.0, match self.selected_vertices.contains(&e.0) {
                true => SelectionStatus::Selected,
                false if self.highlight.selected => SelectionStatus::TransitivelySelected,
                false => SelectionStatus::NotSelected,
            });
        }
    }
    
    fn deep_copy_clone(
        &self,
        uuid_present: &dyn Fn(&uuid::Uuid) -> bool,
        tlc: &mut HashMap<uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
        >,
        c: &mut HashMap<usize, (uuid::Uuid, 
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &mut HashMap<usize, (
            Arc<RwLock<ElementModelT>>,
            Arc<dyn Any + Send + Sync>,
        )>
    ) {
        let uuid = if uuid_present(&*self.uuid()) { uuid::Uuid::now_v7() } else { *self.uuid() };
        
        let cloneish = Arc::new(RwLock::new(Self {
            adapter: self.adapter.deep_copy_init(uuid, m),
            self_reference: Weak::new(),
            source: self.source.clone(),
            destination: self.destination.clone(),
            dragged_node: None,
            highlight: self.highlight,
            selected_vertices: self.selected_vertices.clone(),
            center_point: self.center_point.clone(),
            source_points: self.source_points.clone(),
            dest_points: self.dest_points.clone(),
            point_to_origin: self.point_to_origin.clone(),
        }));
        cloneish.write().unwrap().self_reference = Arc::downgrade(&cloneish);
        tlc.insert(uuid, cloneish.clone());
        c.insert(arc_to_usize(&Weak::upgrade(&self.self_reference).unwrap()), (uuid, cloneish.clone(), cloneish));
    }
    
    fn deep_copy_relink(
        &mut self,
        c: &HashMap<usize, (uuid::Uuid, 
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>>>,
            Arc<dyn Any + Send + Sync>,
        )>,
        m: &HashMap<usize, (
            Arc<RwLock<ElementModelT>>,
            Arc<dyn Any + Send + Sync>,
        )>
    ) {
        self.adapter.deep_copy_finish(m);
    
        if let Some((_u, s, _)) = c.get(&arc_to_usize(&self.source)) {
            self.source = s.clone();
        }
        if let Some((_u, d, _)) = c.get(&arc_to_usize(&self.destination)) {
            self.destination = d.clone();
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
