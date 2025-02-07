use crate::common::canvas::{self, NHCanvas, NHShape, UiCanvas};
use crate::NHApp;
use eframe::egui;
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


pub struct AlignmentManager {
    input_restriction: egui::Rect,
    max_delta: egui::Vec2,
    guidelines_x: Vec<(f32, egui::Align, uuid::Uuid)>,
    guidelines_y: Vec<(f32, egui::Align, uuid::Uuid)>,
    best_x: Option<f32>,
    best_y: Option<f32>,
}

impl AlignmentManager {
    pub fn new(input_restriction: egui::Rect, max_delta: egui::Vec2) -> Self {
        Self {
            input_restriction, max_delta,
            guidelines_x: Vec::new(), guidelines_y: Vec::new(),
            best_x: None, best_y: None,
        }
    }
    pub fn add_shape(&mut self, uuid: uuid::Uuid, shape: canvas::NHShape) {
        for e in shape.guidelines().into_iter().filter(|e| self.input_restriction.contains(e.0)) {
            self.guidelines_x.push((e.0.x, e.1, uuid));
            self.guidelines_y.push((e.0.y, e.1, uuid));
        }
    }
    pub fn sort_guidelines(&mut self) {
        self.guidelines_x.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.guidelines_y.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    
    pub fn coerce(&mut self, uuid: &uuid::Uuid, s: canvas::NHShape) -> egui::Pos2 {
        (self.best_x, self.best_y) = (None, None);
        let center = s.center();
        
        // Naive guidelines coordinate matching
        let start_x = self.guidelines_x.binary_search_by(|probe| probe.0.total_cmp(&(center.x - self.max_delta.x))).unwrap_or_else(|e| e);
        let end_x = self.guidelines_x.binary_search_by(|probe| probe.0.total_cmp(&(center.x + self.max_delta.x))).unwrap_or_else(|e| e);
        for g in self.guidelines_x[start_x..end_x].iter().filter(|e| e.2 != *uuid) {
            if self.best_x.is_none_or(|b| (g.0 - center.x).abs() < (b - center.x).abs()) {
                self.best_x = Some(g.0);
            }
        }
        let start_y = self.guidelines_y.binary_search_by(|probe| probe.0.total_cmp(&(center.y - self.max_delta.y))).unwrap_or_else(|e| e);
        let end_y = self.guidelines_y.binary_search_by(|probe| probe.0.total_cmp(&(center.y + self.max_delta.y))).unwrap_or_else(|e| e);
        for g in self.guidelines_y[start_y..end_y].iter().filter(|e| e.2 != *uuid) {
            if self.best_y.is_none_or(|b| (g.0 - center.y).abs() < (b - center.y).abs())  {
                self.best_y = Some(g.0);
            }
        }
        
        // TODO: try pairwise projection of guidelines with matching Align
        
        egui::Pos2::new(self.best_x.unwrap_or(center.x), self.best_y.unwrap_or(center.y))
    }
}


#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum ProjectCommand {
    OpenAndFocusDiagram(uuid::Uuid),
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
}

pub trait DiagramController: Any {
    fn uuid(&self) -> Arc<uuid::Uuid>;
    fn model_name(&self) -> Arc<String>;

    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, undo_accumulator: &mut Vec<Arc<String>>);
    
    fn new_ui_canvas(
        &mut self,
        ui: &mut egui::Ui,
        profile: &ColorProfile,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);
    
    fn draw_in(
        &mut self,
        canvas: &mut dyn NHCanvas,
        profile: &ColorProfile,
        mouse_pos: Option<egui::Pos2>,
    );
    
    fn context_menu(&mut self, ui: &mut egui::Ui);

    fn show_toolbar(&mut self, ui: &mut egui::Ui);
    fn show_properties(&mut self, ui: &mut egui::Ui, undo_accumulator: &mut Vec<Arc<String>>);
    fn show_layers(&self, ui: &mut egui::Ui);
    fn show_menubar_edit_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui);
    fn show_menubar_diagram_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui);
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
    PropertyChangeSelected(Vec<PropChangeT>),
    Insensitive(InsensitiveCommand<ElementT, PropChangeT>)
}

impl<ElementT: Clone + Debug, PropChangeT: Clone + Debug> SensitiveCommand<ElementT, PropChangeT> {
    // TODO: I'm not sure whether this isn't actually the responsibility of the diagram itself
    fn to_selection_insensitive(
        self,
        selected_elements: &HashSet<uuid::Uuid>,
    ) -> InsensitiveCommand<ElementT, PropChangeT> {
        match self {
            SensitiveCommand::MoveSelectedElements(delta) => {
                InsensitiveCommand::MoveSpecificElements(selected_elements.clone(), delta)
            }
            SensitiveCommand::ResizeSelectedElementsBy(align, delta) => {
                InsensitiveCommand::ResizeSpecificElementsBy(selected_elements.clone(), align, delta)
            }
            SensitiveCommand::ResizeSelectedElementsTo(align, delta) => {
                InsensitiveCommand::ResizeSpecificElementsTo(selected_elements.clone(), align, delta)
            }
            SensitiveCommand::DeleteSelectedElements => {
                InsensitiveCommand::DeleteSpecificElements(selected_elements.clone())
            }
            SensitiveCommand::PropertyChangeSelected(changes) => {
                InsensitiveCommand::PropertyChange(selected_elements.clone(), changes)
            }
            SensitiveCommand::Insensitive(inner) => inner,
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

pub trait ElementControllerGen2<
    CommonElementT: ?Sized,
    QueryableT,
    ToolT,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
>: ElementController<CommonElementT> + ContainerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT, PropChangeT> where
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
        canvas: &mut dyn NHCanvas,
        profile: &ColorProfile,
        tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus;
    fn collect_allignment(&mut self, am: &mut AlignmentManager) {
        am.add_shape(*self.uuid(), self.min_shape());
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
        tool: &mut Option<ToolT>,
        am: &mut AlignmentManager,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> EventHandlingStatus;
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    );
    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>);
    
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
    all_selected_elements: HashSet<uuid::Uuid>,

    pub _layers: Vec<bool>,

    pub camera_offset: egui::Pos2,
    pub camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    last_interactive_canvas_rect: egui::Rect,
    alignment_manager: AlignmentManager,
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
    menubar_options_fun: fn(&mut Self, &mut NHApp, &mut egui::Ui),
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
        menubar_options_fun: fn(&mut Self, &mut NHApp, &mut egui::Ui),
    ) -> Arc<RwLock<Self>> {
        let event_order = owned_controllers.keys().map(|e| *e).collect();
        let ret = Arc::new(RwLock::new(Self {
            model,
            self_reference: Weak::new(),
            owned_controllers,
            event_order,
            all_selected_elements: HashSet::new(),

            _layers: vec![true],

            camera_offset: egui::Pos2::ZERO,
            camera_scale: 1.0,
            last_unhandled_mouse_pos: None,
            last_interactive_canvas_rect: egui::Rect::ZERO,
            alignment_manager: AlignmentManager::new(egui::Rect::ZERO, egui::Vec2::ZERO),
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
        self.alignment_manager = AlignmentManager::new(self.last_interactive_canvas_rect, egui::Vec2::splat(10.0 / self.camera_scale));
        self.event_order.iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .for_each(|uc| uc.1.write().unwrap().collect_allignment(&mut self.alignment_manager));
        self.alignment_manager.sort_guidelines();
        
        // Handle events
        let mut commands = Vec::new();
        
        if matches!(event, InputEvent::Click(_)) {
            self.current_tool.as_mut().map(|e| e.reset_event_lock());
        }
        
        let child = self.event_order
            .iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .map(|uc| (uc.0, uc.1.write().unwrap().handle_event(
                    event,
                    modifiers,
                    &mut self.current_tool,
                    &mut self.alignment_manager,
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
                                !self.all_selected_elements.contains(&us.0),
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

    fn apply_commands(
        &mut self,
        commands: Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
        global_undo_accumulator: &mut Vec<Arc<String>>,
        save_to_undo_stack: bool,
        clear_redo_stack: bool,
    ) {
        for command in commands {
            // TODO: transitive closure of dependency when deleting elements
            let command = command.to_selection_insensitive(&self.all_selected_elements);

            let mut undo_accumulator = vec![];

            match &command {
                InsensitiveCommand::SelectAll(..)
                | InsensitiveCommand::SelectSpecific(..)
                | InsensitiveCommand::SelectByDrag(..)
                | InsensitiveCommand::MoveSpecificElements(..)
                | InsensitiveCommand::MoveAllElements(..)
                | InsensitiveCommand::ResizeSpecificElementsBy(..)
                | InsensitiveCommand::ResizeSpecificElementsTo(..) => {}
                InsensitiveCommand::DeleteSpecificElements(uuids) => {
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
                | InsensitiveCommand::AddElement(..) => true,
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
                self.all_selected_elements = HashSet::new();
                for (_uuid, c) in &self.owned_controllers {
                    let mut c = c.write().unwrap();
                    c.collect_all_selected_elements(&mut self.all_selected_elements);
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
        ui: &mut egui::Ui,
        profile: &ColorProfile,
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
        ui_canvas.clear(profile.backgrounds[0]);
        ui_canvas.draw_gridlines(
            Some((50.0, profile.foregrounds[0])),
            Some((50.0, profile.foregrounds[0])),
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
    fn show_menubar_edit_options(&mut self, _context: &mut NHApp, _ui: &mut egui::Ui) {}
    fn show_menubar_diagram_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui) {
        (self.menubar_options_fun)(self, context, ui);
        
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
                    InsensitiveCommand::SelectSpecific(self.all_selected_elements.clone(), false).into()
                ], &mut vec![], true, false);
            }
            DiagramCommand::DeleteSelectedElements => {
                let mut undo = vec![];
                self.apply_commands(vec![SensitiveCommand::DeleteSelectedElements], &mut undo, true, true);
                self.last_change_flag = true;
                global_undo.extend(undo.into_iter());
            }
        }
    }

    fn draw_in(
        &mut self,
        canvas: &mut dyn NHCanvas,
        profile: &ColorProfile,
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
                    .draw_in(&self.queryable, canvas, profile, &tool)
                    == TargettingStatus::Drawn
                {
                    drawn_targetting = TargettingStatus::Drawn;
                }
            });

        if canvas.is_interactive() {
            if let Some((pos, tool)) = tool {
                if drawn_targetting == TargettingStatus::NotDrawn {
                    canvas.draw_rectangle(
                        egui::Rect::EVERYTHING,
                        egui::Rounding::ZERO,
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
                                .draw_in(&self.queryable, canvas, profile, &Some((pos, tool)));
                        });
                }
                tool.draw_status_hint(canvas, pos);
            } else if let Some((a, b)) = self.select_by_drag {
                canvas.draw_rectangle(
                    egui::Rect::from_two_pos(a, b),
                    egui::Rounding::ZERO,
                    egui::Color32::from_rgba_premultiplied(0, 0, 255, 7),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLUE),
                    canvas::Highlight::NONE,
                );
            }
            
            // Draw alignment hint
            if let Some(bx) = self.alignment_manager.best_x {
                canvas.draw_line([
                    egui::Pos2::new(bx, self.camera_offset.y / -self.camera_scale), egui::Pos2::new(bx, self.camera_offset.y / -self.camera_scale + self.last_interactive_canvas_rect.height())
                ], canvas::Stroke::new_solid(1.0, profile.auxiliary[0]), canvas::Highlight::NONE);
            }
            if let Some(by) = self.alignment_manager.best_y {
                canvas.draw_line([
                    egui::Pos2::new(self.camera_offset.x / -self.camera_scale, by), egui::Pos2::new(self.camera_offset.x / -self.camera_scale + self.last_interactive_canvas_rect.width(), by)
                ], canvas::Stroke::new_solid(1.0, profile.auxiliary[0]), canvas::Highlight::NONE);
            }
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

#[derive(Clone, Copy, PartialEq)]
pub enum DragType {
    Move,
    Resize(egui::Align2),
}

pub struct PackageView<
    ModelT: ContainerModel<ElementModelT>,
    ElementModelT: ?Sized + 'static,
    QueryableT: 'static,
    BufferT: 'static,
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
    model: Arc<RwLock<ModelT>>,
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
    selected_elements: HashSet<uuid::Uuid>,

    buffer: BufferT,

    dragged: Option<DragType>,
    highlight: canvas::Highlight,
    bounds_rect: egui::Rect,

    model_to_element_shim: fn(Arc<RwLock<ModelT>>) -> Arc<RwLock<ElementModelT>>,

    show_properties_fun: fn(
        &mut BufferT,
        &mut egui::Ui,
        &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ),
    apply_property_change_fun: fn(
        &mut BufferT,
        &mut ModelT,
        &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    ),
}

impl<
        ModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    >
    PackageView<ModelT, ElementModelT, QueryableT, BufferT, ToolT, AddCommandElementT, PropChangeT>
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
        model: Arc<RwLock<ModelT>>,
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
        buffer: BufferT,
        bounds_rect: egui::Rect,
        model_to_element_shim: fn(Arc<RwLock<ModelT>>) -> Arc<RwLock<ElementModelT>>,
        show_properties_fun: fn(
            &mut BufferT,
            &mut egui::Ui,
            &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
        ),
        apply_property_change_fun: fn(
            &mut BufferT,
            &mut ModelT,
            &InsensitiveCommand<AddCommandElementT, PropChangeT>,
            &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
        ),
    ) -> Self {
        let event_order = owned_controllers.keys().map(|e| *e).collect();
        Self {
            model,
            owned_controllers,
            event_order,
            selected_elements: HashSet::new(),

            buffer,
            dragged: None,
            highlight: canvas::Highlight::NONE,
            bounds_rect,

            model_to_element_shim,
            show_properties_fun,
            apply_property_change_fun,
        }
    }
}

impl<
        ModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ElementController<ElementModelT>
    for PackageView<
        ModelT,
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
    fn model(&self) -> Arc<RwLock<ElementModelT>> {
        (self.model_to_element_shim)(self.model.clone())
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
        ModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ContainerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for PackageView<
        ModelT,
        ElementModelT,
        QueryableT,
        BufferT,
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
        ModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for PackageView<
        ModelT,
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
            (self.show_properties_fun)(&mut self.buffer, ui, commands);
            true
        } else {
            false
        }
    }
    fn list_in_project_hierarchy(&self, parent: &QueryableT, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        egui::CollapsingHeader::new(format!("{} ({})", *model.name(), *model.name())).show(
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
        canvas: &mut dyn NHCanvas,
        profile: &ColorProfile,
        tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus {
        // Draw shape and text
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::Rounding::ZERO,
            profile.backgrounds[1],
            canvas::Stroke::new_solid(1.0, profile.foregrounds[1]),
            self.highlight,
        );

        canvas.draw_text(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            &self.model.read().unwrap().name(),
            canvas::CLASS_MIDDLE_FONT_SIZE,
            profile.foregrounds[1],
        );
        
        // Draw resize/drag handles
        // TODO: the handles should probably scale?
        if self.highlight.selected && canvas.is_interactive() {
            for h in [self.bounds_rect.left_top(), self.bounds_rect.center_top(), self.bounds_rect.right_top(),
                      self.bounds_rect.left_center(), self.bounds_rect.right_center(), 
                      self.bounds_rect.left_bottom(), self.bounds_rect.center_bottom(), self.bounds_rect.right_bottom()]
            {
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(h, egui::Vec2::splat(5.0)),
                    egui::Rounding::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
            }
            
            canvas.draw_rectangle(
                egui::Rect::from_center_size(self.bounds_rect.right_top() - egui::Vec2::new(10.0, 0.0), egui::Vec2::splat(5.0)),
                egui::Rounding::ZERO,
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
                if uc.1.write().unwrap().draw_in(q, canvas, profile, &tool) == TargettingStatus::Drawn
                {
                    drawn_child_targetting = TargettingStatus::Drawn;
                }
            });

        match (drawn_child_targetting, tool) {
            (TargettingStatus::NotDrawn, Some((pos, t))) if self.min_shape().contains(*pos) => {
                canvas.draw_rectangle(
                    self.bounds_rect,
                    egui::Rounding::ZERO,
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
                        uc.1.write().unwrap().draw_in(q, canvas, profile, &tool);
                    });

                TargettingStatus::Drawn
            }
            _ => drawn_child_targetting,
        }
    }

    fn collect_allignment(&mut self, am: &mut AlignmentManager) {
        am.add_shape(*self.uuid(), self.min_shape());
        
        self.event_order.iter()
            .flat_map(|k| self.owned_controllers.get(k).map(|e| (*k, e)))
            .for_each(|uc| uc.1.write().unwrap().collect_allignment(am));
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
        tool: &mut Option<ToolT>,
        am: &mut AlignmentManager,
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
                        .handle_event(event, modifiers, tool, am, commands),
                )
            })
            .find(|e| e.1 != EventHandlingStatus::NotHandled);
        
        match event {
            InputEvent::MouseDown(_pos) | InputEvent::MouseUp(_pos) if uc_status.is_some() => {
                EventHandlingStatus::HandledByContainer
            }
            InputEvent::MouseDown(pos) => {
                for (a,h) in [(egui::Align2::RIGHT_BOTTOM, self.bounds_rect.left_top()),
                              (egui::Align2::CENTER_BOTTOM, self.bounds_rect.center_top()),
                              (egui::Align2::LEFT_BOTTOM, self.bounds_rect.right_top()),
                              (egui::Align2::RIGHT_CENTER, self.bounds_rect.left_center()),
                              (egui::Align2::LEFT_CENTER, self.bounds_rect.right_center()),
                              (egui::Align2::RIGHT_TOP, self.bounds_rect.left_bottom()),
                              (egui::Align2::CENTER_TOP, self.bounds_rect.center_bottom()),
                              (egui::Align2::LEFT_TOP, self.bounds_rect.right_bottom())]
                {
                    if egui::Rect::from_center_size(h, egui::Vec2::splat(5.0)).contains(pos) {
                        self.dragged = Some(DragType::Resize(a));
                        return EventHandlingStatus::HandledByElement;
                    }
                }
                
                if self.min_shape().border_distance(pos) <= 2.0
                    || egui::Rect::from_center_size(self.bounds_rect.right_top() - egui::Vec2::new(10.0, 0.0), egui::Vec2::splat(5.0)).contains(pos) {
                    self.dragged = Some(DragType::Move);
                    EventHandlingStatus::HandledByElement
                } else {
                    EventHandlingStatus::NotHandled
                }
            },
            InputEvent::MouseUp(pos) => {
                if self.dragged.is_some() {
                    self.dragged = None;
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
                            if !modifiers.command {
                                commands.push(InsensitiveCommand::SelectAll(false).into());
                                commands.push(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(uc.0).collect(),
                                    true,
                                ).into());
                            } else {
                                commands.push(InsensitiveCommand::SelectSpecific(
                                    std::iter::once(uc.0).collect(),
                                    !self.selected_elements.contains(&uc.0),
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
            InputEvent::Drag { delta, .. } => match self.dragged {
                Some(dt) => match dt {
                    DragType::Move => {
                        if self.highlight.selected {
                            commands.push(SensitiveCommand::MoveSelectedElements(delta));
                        } else {
                            commands.push(InsensitiveCommand::MoveSpecificElements(
                                std::iter::once(*self.uuid()).collect(),
                                delta,
                            ).into());
                        }
                        EventHandlingStatus::HandledByElement
                    },
                    DragType::Resize(align2) => {
                        commands.push(SensitiveCommand::ResizeSelectedElementsBy(align2, delta));
                        EventHandlingStatus::HandledByElement
                    },
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
                
                let r = self.bounds_rect + egui::Margin{left, right, top, bottom};
                
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
                        self.selected_elements =
                            self.owned_controllers.iter().map(|e| *e.0).collect()
                    }
                    false => self.selected_elements.clear(),
                }
                recurse!(self);
            }
            InsensitiveCommand::SelectSpecific(uuids, select) => {
                if uuids.contains(&*self.uuid()) {
                    self.highlight.selected = *select;
                }

                for uuid in self.owned_controllers.keys().filter(|k| uuids.contains(k)) {
                    match select {
                        true => self.selected_elements.insert(*uuid),
                        false => self.selected_elements.remove(uuid),
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
                    let mut delta_naive = *size - self.bounds_rect.size();
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
            InsensitiveCommand::DeleteSpecificElements(uuids) => {
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
                self_m.delete_elements(&uuids);

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
                        let mut self_m = self.model.write().unwrap();
                        self_m.add_element(new_c.model());
                        drop(new_c);

                        self.owned_controllers.insert(uuid, element);
                        self.event_order.push(uuid);
                    }
                }

                recurse!(self);
            }
            InsensitiveCommand::PropertyChange(uuids, _property) => {
                if uuids.contains(&*self.uuid()) {
                    let mut m = self.model.write().unwrap();
                    (self.apply_property_change_fun)(
                        &mut self.buffer,
                        &mut m,
                        command,
                        undo_accumulator,
                    );
                }

                recurse!(self);
            }
        }
    }

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }

        for e in &self.owned_controllers {
            let mut e = e.1.write().unwrap();
            e.collect_all_selected_elements(into);
        }
    }
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
    ModelT,
    ElementModelT: ?Sized + 'static,
    QueryableT,
    BufferT,
    ToolT,
    AddCommandElementT: Clone + Debug,
    PropChangeT: Clone + Debug,
> where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    model: Arc<RwLock<ModelT>>,
    buffer: BufferT,

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

    dragged_node: Option<uuid::Uuid>,
    highlight: canvas::Highlight,
    selected_vertices: HashSet<uuid::Uuid>,
    center_point: Option<(uuid::Uuid, egui::Pos2)>,
    source_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
    dest_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
    point_to_origin: HashMap<uuid::Uuid, (bool, usize)>,

    model_to_element_shim: fn(Arc<RwLock<ModelT>>) -> Arc<RwLock<ElementModelT>>,

    show_properties_fun: fn(
        &mut BufferT,
        &mut egui::Ui,
        &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ),
    apply_property_change_fun: fn(
        &mut BufferT,
        &mut ModelT,
        &InsensitiveCommand<AddCommandElementT, PropChangeT>,
        &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
    ),

    model_to_uuid: fn(&ModelT) -> Arc<uuid::Uuid>,
    model_to_name: fn(&ModelT) -> Arc<String>,
    model_to_line_type: fn(&ModelT) -> canvas::LineType,
    model_to_source_arrowhead_type: fn(&ModelT) -> canvas::ArrowheadType,
    model_to_destination_arrowhead_type: fn(&ModelT) -> canvas::ArrowheadType,
    model_to_source_arrowhead_label: fn(&ModelT) -> Option<&str>,
    model_to_destination_arrowhead_label: fn(&ModelT) -> Option<&str>,
}

impl<
        ModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    >
    MulticonnectionView<
        ModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    pub fn new(
        model: Arc<RwLock<ModelT>>,
        buffer: BufferT,
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

        model_to_element_shim: fn(Arc<RwLock<ModelT>>) -> Arc<RwLock<ElementModelT>>,

        show_properties_fun: fn(
            &mut BufferT,
            &mut egui::Ui,
            &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
        ),
        apply_property_change_fun: fn(
            &mut BufferT,
            &mut ModelT,
            &InsensitiveCommand<AddCommandElementT, PropChangeT>,
            &mut Vec<InsensitiveCommand<AddCommandElementT, PropChangeT>>,
        ),

        model_to_uuid: fn(&ModelT) -> Arc<uuid::Uuid>,
        model_to_name: fn(&ModelT) -> Arc<String>,
        model_to_line_type: fn(&ModelT) -> canvas::LineType,
        model_to_source_arrowhead_type: fn(&ModelT) -> canvas::ArrowheadType,
        model_to_destination_arrowhead_type: fn(&ModelT) -> canvas::ArrowheadType,
        model_to_source_arrowhead_label: fn(&ModelT) -> Option<&str>,
        model_to_destination_arrowhead_label: fn(&ModelT) -> Option<&str>,
    ) -> Self {
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
        
        Self {
            model,
            buffer,
            source,
            destination,
            dragged_node: None,
            highlight: canvas::Highlight::NONE,
            selected_vertices: HashSet::new(),

            center_point,
            source_points,
            dest_points,
            point_to_origin,

            model_to_element_shim,

            show_properties_fun,
            apply_property_change_fun,

            model_to_uuid,
            model_to_name,
            model_to_line_type,
            model_to_source_arrowhead_type,
            model_to_destination_arrowhead_type,
            model_to_source_arrowhead_label,
            model_to_destination_arrowhead_label,
        }
    }
}

impl<
        ModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT: Clone + Debug,
        PropChangeT: Clone + Debug,
    > ElementController<ElementModelT>
    for MulticonnectionView<
        ModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
        PropChangeT,
    >
where
    AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a PropChangeT: TryInto<FlipMulticonnection>,
{
    fn uuid(&self) -> Arc<uuid::Uuid> {
        (self.model_to_uuid)(&self.model.read().unwrap())
    }
    fn model_name(&self) -> Arc<String> {
        (self.model_to_name)(&self.model.read().unwrap())
    }
    fn model(&self) -> Arc<RwLock<ElementModelT>> {
        (self.model_to_element_shim)(self.model.clone())
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
        ModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ContainerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for MulticonnectionView<
        ModelT,
        ElementModelT,
        QueryableT,
        BufferT,
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
        ModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + Debug + 'static,
        PropChangeT: Clone + Debug + 'static,
    > ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT, PropChangeT>
    for MulticonnectionView<
        ModelT,
        ElementModelT,
        QueryableT,
        BufferT,
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

        (self.show_properties_fun)(&mut self.buffer, ui, commands);

        true
    }

    fn draw_in(
        &mut self,
        _: &QueryableT,
        canvas: &mut dyn NHCanvas,
        profile: &ColorProfile,
        _tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus {
        let model = self.model.read().unwrap();
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
        
        canvas.draw_multiconnection(
            &self.selected_vertices,
            &[(
                (self.model_to_source_arrowhead_type)(&*model),
                crate::common::canvas::Stroke {
                    width: 1.0,
                    color: profile.foregrounds[2],
                    line_type: (self.model_to_line_type)(&*model),
                },
                &self.source_points[0],
                (self.model_to_source_arrowhead_label)(&*model),
            )],
            &[(
                (self.model_to_destination_arrowhead_type)(&*model),
                crate::common::canvas::Stroke {
                    width: 1.0,
                    color: profile.foregrounds[2],
                    line_type: (self.model_to_line_type)(&*model),
                },
                &self.dest_points[0],
                (self.model_to_destination_arrowhead_label)(&*model),
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

    fn collect_allignment(&mut self, am: &mut AlignmentManager) {
        // TODO: add vertices as zero sized rectangles?
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        modifiers: ModifierKeys,
        _tool: &mut Option<ToolT>,
        _am: &mut AlignmentManager,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT, PropChangeT>>,
    ) -> EventHandlingStatus {
        const DISTANCE_THRESHOLD: f32 = 3.0;
        
        fn is_over(a: egui::Pos2, b: egui::Pos2) -> bool {
            a.distance(b) <= DISTANCE_THRESHOLD
        }
        
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
                        self.dragged_node = Some(uuid);
                        return EventHandlingStatus::HandledByContainer;
                    }
                    // TODO: this is generally wrong (why??)
                    None if is_over(pos, self.position()) => {
                        self.dragged_node = Some(uuid::Uuid::now_v7());
                        commands.push(InsensitiveCommand::AddElement(
                            *self.uuid(),
                            VertexInformation {
                                after: uuid::Uuid::nil(),
                                id: self.dragged_node.unwrap(),
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
                                    self.dragged_node = Some(uuid::Uuid::now_v7());
                                    commands.push(SensitiveCommand::Insensitive(InsensitiveCommand::AddElement(
                                        *self.uuid(),
                                        VertexInformation {
                                            after: u.0,
                                            id: self.dragged_node.unwrap(),
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
                                    self.dragged_node = Some(joint.0);
                                    
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
                        if !modifiers.command {
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
                                
                                if dist_to_line_segment(pos, u.1, v.1) <= DISTANCE_THRESHOLD {
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
                            if dist_to_line_segment(pos, u.1, v.1) <= DISTANCE_THRESHOLD {
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
                
                if self.selected_vertices.contains(&dragged_node) {
                    commands.push(SensitiveCommand::MoveSelectedElements(delta));
                } else {
                    commands.push(InsensitiveCommand::MoveSpecificElements(
                        std::iter::once(dragged_node).collect(),
                        delta,
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
            | InsensitiveCommand::ResizeSpecificElementsTo(..) => {}
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
                    let mut m = self.model.write().unwrap();
                    (self.apply_property_change_fun)(
                        &mut self.buffer,
                        &mut m,
                        command,
                        undo_accumulator,
                    );
                }
            }
        }
    }

    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>) {
        if self.highlight.selected {
            into.insert(*self.uuid());
        }
        for e in &self.selected_vertices {
            into.insert(*e);
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
