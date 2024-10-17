use crate::common::canvas::{self, NHCanvas, NHShape, UiCanvas};
use crate::NHApp;
use eframe::egui;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

pub trait DiagramController: Any {
    fn uuid(&self) -> Arc<uuid::Uuid>;
    fn model_name(&self) -> Arc<String>;

    fn new_ui_canvas(
        &self,
        ui: &mut egui::Ui,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);
    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response);
    fn click(&mut self, pos: egui::Pos2, modifiers: ModifierKeys) -> bool;
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool;
    fn context_menu(&mut self, ui: &mut egui::Ui);

    fn show_toolbar(&mut self, ui: &mut egui::Ui);
    fn show_properties(&mut self, ui: &mut egui::Ui);
    fn show_layers(&self, ui: &mut egui::Ui);
    fn show_menubar_edit_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui);
    fn show_menubar_diagram_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui);
    fn list_in_project_hierarchy(&self, ui: &mut egui::Ui);

    // This hurts me at least as much as it hurts you
    //fn outgoing_for<'a>(&'a self, _uuid: &'a uuid::Uuid) -> Box<dyn Iterator<Item=Arc<RwLock<dyn ElementController>>> + 'a> {
    //    Box::new(std::iter::empty::<Arc<RwLock<dyn ElementController>>>())
    //}

    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, mouse_pos: Option<egui::Pos2>);
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
pub enum ClickHandlingStatus {
    NotHandled,
    HandledByElement,
    HandledByContainer,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DragHandlingStatus {
    NotHandled,
    Handled,
}

#[derive(Clone, PartialEq, Debug)]
pub enum SensitiveCommand<ElementT: Clone> {
    SelectAll(bool),
    Select(HashSet<uuid::Uuid>, bool),
    MoveElements(HashSet<uuid::Uuid>, egui::Vec2),
    MoveSelectedElements(egui::Vec2),
    DeleteElements(HashSet<uuid::Uuid>),
    DeleteSelectedElements,
    AddElement(uuid::Uuid, ElementT),
}

impl<ElementT: Clone> SensitiveCommand<ElementT> {
    fn to_selection_insensitive(
        self,
        selected_elements: &HashSet<uuid::Uuid>,
    ) -> InsensitiveCommand<ElementT> {
        match self {
            SensitiveCommand::SelectAll(select) => InsensitiveCommand::SelectAll(select),
            SensitiveCommand::Select(uuids, select) => InsensitiveCommand::Select(uuids, select),
            SensitiveCommand::MoveElements(uuids, delta) => {
                InsensitiveCommand::MoveElements(uuids, delta)
            }
            SensitiveCommand::MoveSelectedElements(delta) => {
                InsensitiveCommand::MoveElements(selected_elements.clone(), delta)
            }
            SensitiveCommand::DeleteElements(uuids) => InsensitiveCommand::DeleteElements(uuids),
            SensitiveCommand::DeleteSelectedElements => {
                InsensitiveCommand::DeleteElements(selected_elements.clone())
            }
            SensitiveCommand::AddElement(uuid, element) => {
                InsensitiveCommand::AddElement(uuid, element)
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum InsensitiveCommand<ElementT: Clone> {
    SelectAll(bool),
    Select(HashSet<uuid::Uuid>, bool),
    MoveElements(HashSet<uuid::Uuid>, egui::Vec2),
    DeleteElements(HashSet<uuid::Uuid>),
    AddElement(uuid::Uuid, ElementT),
}

impl<ElementT: Clone> InsensitiveCommand<ElementT> {
    fn to_selection_sensitive(self) -> SensitiveCommand<ElementT> {
        match self {
            InsensitiveCommand::SelectAll(select) => SensitiveCommand::SelectAll(select),
            InsensitiveCommand::Select(uuids, select) => SensitiveCommand::Select(uuids, select),
            InsensitiveCommand::MoveElements(uuids, delta) => {
                SensitiveCommand::MoveElements(uuids, delta)
            }
            InsensitiveCommand::DeleteElements(uuids) => SensitiveCommand::DeleteElements(uuids),
            InsensitiveCommand::AddElement(uuid, element) => {
                SensitiveCommand::AddElement(uuid, element)
            }
        }
    }

    fn info_text(&self) -> String {
        match self {
            InsensitiveCommand::SelectAll(..) | InsensitiveCommand::Select(..) => {
                format!("Sorry, your undo stack is broken now :/")
            }
            InsensitiveCommand::DeleteElements(uuids) => format!("Delete {} elements", uuids.len()),
            InsensitiveCommand::MoveElements(uuids, delta) => {
                format!("Move {} elements", uuids.len())
            }
            InsensitiveCommand::AddElement(..) => format!("Add 1 element"),
        }
    }

    fn merge(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (
                InsensitiveCommand::MoveElements(uuids1, delta1),
                InsensitiveCommand::MoveElements(uuids2, delta2),
            ) if uuids1 == uuids2 => Some(InsensitiveCommand::MoveElements(
                uuids1.clone(),
                *delta1 + *delta2,
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
}

pub trait Tool<CommonElementT: ?Sized, QueryableT, AddCommandElementT> {
    type KindedElement<'a>;
    type Stage;

    fn initial_stage(&self) -> Self::Stage;

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32;
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2);

    fn offset_by(&mut self, delta: egui::Vec2);
    fn add_position(&mut self, pos: egui::Pos2);
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>, pos: egui::Pos2);
    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<CommonElementT, QueryableT, Self, AddCommandElementT>,
    ) -> Option<
        Arc<
            RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, Self, AddCommandElementT>>,
        >,
    >;
    fn reset_event_lock(&mut self);
}

pub trait ElementControllerGen2<
    CommonElementT: ?Sized,
    QueryableT,
    ToolT,
    AddCommandElementT: Clone,
>: ElementController<CommonElementT> where
    ToolT: Tool<CommonElementT, QueryableT, AddCommandElementT>,
{
    fn show_properties(&mut self, _: &QueryableT, _ui: &mut egui::Ui) -> bool {
        false
    }
    fn list_in_project_hierarchy(&self, _: &QueryableT, _ui: &mut egui::Ui) {}

    fn draw_in(
        &mut self,
        _: &QueryableT,
        canvas: &mut dyn NHCanvas,
        tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus;
    fn click(
        &mut self,
        tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT>>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus;
    fn drag(
        &mut self,
        tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT>>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus;
    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<AddCommandElementT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT>>,
    );
    fn collect_all_selected_elements(&mut self, into: &mut HashSet<uuid::Uuid>);
}

pub trait ContainerGen2<CommonElementT: ?Sized, QueryableT, ToolT, AddCommandElementT> {
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<
                dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT, AddCommandElementT>,
            >,
        >,
    >;
}

/// This is a generic DiagramController implementation.
/// Hopefully it should reduce the amount of code, but nothing prevents creating fully custom DiagramController implementations.
pub struct DiagramControllerGen2<
    DiagramModelT: ContainerModel<ElementModelT>,
    ElementModelT: ?Sized + 'static,
    QueryableT,
    BufferT,
    ToolT,
    AddCommandElementT: Clone + 'static,
> where
    ToolT: Tool<ElementModelT, QueryableT, AddCommandElementT>,
{
    model: Arc<RwLock<DiagramModelT>>,
    owned_controllers: HashMap<
        uuid::Uuid,
        Arc<
            RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>>,
        >,
    >,

    pub _layers: Vec<bool>,

    pub camera_offset: egui::Pos2,
    pub camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    selected_elements: HashSet<uuid::Uuid>,
    all_selected_elements: HashSet<uuid::Uuid>,
    current_tool: Option<ToolT>,
    undo_stack: Vec<(
        InsensitiveCommand<AddCommandElementT>,
        Vec<InsensitiveCommand<AddCommandElementT>>,
    )>,
    redo_stack: Vec<InsensitiveCommand<AddCommandElementT>>,
    undo_shortcut: egui::KeyboardShortcut,
    redo_shortcut: egui::KeyboardShortcut,
    delete_shortcut: egui::KeyboardShortcut,

    // q: dyn Fn(&Vec<DomainElementT>) -> QueryableT,
    queryable: QueryableT,
    buffer: BufferT,
    show_props_fun: fn(&mut DiagramModelT, &mut BufferT, &mut egui::Ui),
    tool_change_fun: fn(&mut Option<ToolT>, &mut egui::Ui),
    menubar_options_fun: fn(&mut Self, &mut NHApp, &mut egui::Ui),
}

impl<
        DiagramModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + 'static,
    >
    DiagramControllerGen2<
        DiagramModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
    >
where
    ToolT: for<'a> Tool<
        ElementModelT,
        QueryableT,
        AddCommandElementT,
        KindedElement<'a>: From<&'a Self>,
    >,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
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
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
                >,
            >,
        >,
        queryable: QueryableT,
        buffer: BufferT,
        show_props_fun: fn(&mut DiagramModelT, &mut BufferT, &mut egui::Ui),
        tool_change_fun: fn(&mut Option<ToolT>, &mut egui::Ui),
        menubar_options_fun: fn(&mut Self, &mut NHApp, &mut egui::Ui),
    ) -> Self {
        Self {
            model,
            owned_controllers,

            _layers: vec![true],

            camera_offset: egui::Pos2::ZERO,
            camera_scale: 1.0,
            last_unhandled_mouse_pos: None,
            selected_elements: HashSet::new(),
            all_selected_elements: HashSet::new(),
            current_tool: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_shortcut: egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z),
            redo_shortcut: egui::KeyboardShortcut::new(
                egui::Modifiers {
                    alt: false,
                    ctrl: false,
                    shift: true,
                    mac_cmd: false,
                    command: true,
                },
                egui::Key::Z,
            ),
            delete_shortcut: egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete),

            queryable,
            buffer,
            show_props_fun,
            tool_change_fun,
            menubar_options_fun,
        }
    }

    pub fn model(&self) -> Arc<RwLock<DiagramModelT>> {
        self.model.clone()
    }

    fn apply_commands(
        &mut self,
        commands: Vec<SensitiveCommand<AddCommandElementT>>,
        save_to_undo_stack: bool,
        clear_redo_stack: bool,
    ) {
        for command in commands {
            // TODO: transitive closure of dependency when deleting elements
            let command = command.to_selection_insensitive(&self.all_selected_elements);

            let mut undo_accumulator = vec![];

            match &command {
                InsensitiveCommand::SelectAll(select) => match select {
                    true => {
                        self.selected_elements =
                            self.owned_controllers.iter().map(|e| *e.0).collect()
                    }
                    false => self.selected_elements.clear(),
                },
                InsensitiveCommand::Select(uuids, select) => {
                    for uuid in self.owned_controllers.keys().filter(|k| uuids.contains(k)) {
                        match select {
                            true => self.selected_elements.insert(uuid.clone()),
                            false => self.selected_elements.remove(uuid),
                        };
                    }
                }
                InsensitiveCommand::MoveElements(..) => {}
                InsensitiveCommand::DeleteElements(uuids) => {
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

                    self.owned_controllers.retain(|k, v| !uuids.contains(&k));
                }
                InsensitiveCommand::AddElement(target, element) => {
                    if *target == *self.uuid() {
                        if let Ok((uuid, element)) = element.clone().try_into() {
                            self.owned_controllers.insert(uuid, element);
                            undo_accumulator.push(InsensitiveCommand::DeleteElements(
                                std::iter::once(uuid).collect(),
                            ));
                        }
                    }
                }
            }

            for e in &self.owned_controllers {
                let mut e = e.1.write().unwrap();
                e.apply_command(&command, &mut undo_accumulator);
            }

            if !undo_accumulator.is_empty() {
                if clear_redo_stack {
                    self.redo_stack.clear();
                }
                if save_to_undo_stack {
                    if let Some(merged) = self.undo_stack.last().and_then(|e| e.0.merge(&command)) {
                        let last = self.undo_stack.last_mut().unwrap();
                        last.0 = merged;
                        last.1.extend(undo_accumulator);
                    } else {
                        self.undo_stack.push((command.clone(), undo_accumulator));
                    }
                }
            }

            match command {
                InsensitiveCommand::SelectAll(..)
                | InsensitiveCommand::Select(..)
                | InsensitiveCommand::DeleteElements(..)
                | InsensitiveCommand::AddElement(..) => {
                    self.all_selected_elements = HashSet::new();
                    for (_, c) in &self.owned_controllers {
                        let mut c = c.write().unwrap();
                        c.collect_all_selected_elements(&mut self.all_selected_elements);
                    }
                }
                InsensitiveCommand::MoveElements(..) => {}
            }
        }
    }

    fn undo(&mut self, n: usize) {
        let n = n.min(self.undo_stack.len());
        let (commands, undo_commands): (Vec<_>, Vec<Vec<_>>) = self
            .undo_stack
            .drain(self.undo_stack.len() - n..)
            .rev()
            .collect();
        self.apply_commands(
            undo_commands
                .into_iter()
                .flatten()
                .map(|c| c.to_selection_sensitive())
                .collect(),
            false,
            false,
        );
        self.redo_stack.extend(commands);
    }

    fn redo(&mut self, n: usize) {
        let n = n.min(self.redo_stack.len());
        let commands: Vec<_> = self
            .redo_stack
            .drain(self.redo_stack.len() - n..)
            .rev()
            .map(|c| c.to_selection_sensitive())
            .collect();
        self.apply_commands(commands, true, false);
    }
}

impl<
        DiagramModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT: 'static,
        AddCommandElementT: Clone + 'static,
    > DiagramController
    for DiagramControllerGen2<
        DiagramModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
    >
where
    ToolT: for<'a> Tool<
        ElementModelT,
        QueryableT,
        AddCommandElementT,
        KindedElement<'a>: From<&'a Self>,
    >,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
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
        &self,
        ui: &mut egui::Ui,
    ) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>) {
        let canvas_pos = ui.next_widget_position();
        let canvas_size = ui.available_size();
        let canvas_rect = egui::Rect {
            min: canvas_pos,
            max: canvas_pos + canvas_size,
        };

        let (painter_response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let ui_canvas = UiCanvas::new(
            painter,
            canvas_rect,
            self.camera_offset,
            self.camera_scale,
            ui.ctx().pointer_interact_pos().map(|e| {
                ((e - self.camera_offset - painter_response.rect.min.to_vec2()) / self.camera_scale)
                    .to_pos2()
            }),
        );
        ui_canvas.clear(egui::Color32::WHITE);
        ui_canvas.draw_gridlines(
            Some((50.0, egui::Color32::from_rgb(220, 220, 220))),
            Some((50.0, egui::Color32::from_rgb(220, 220, 220))),
        );

        let inner_mouse = ui
            .ctx()
            .pointer_interact_pos()
            .filter(|e| canvas_rect.contains(*e))
            .map(|e| {
                ((e - self.camera_offset - canvas_pos.to_vec2()) / self.camera_scale).to_pos2()
            });

        (Box::new(ui_canvas), painter_response, inner_mouse)
    }
    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // Handle camera and element clicks/drags

        // TODO: This shortcut handling is generally wrong. Depends on redo_shortcut not being a subset of undo_shortcut.
        if ui.input_mut(|i| i.consume_shortcut(&self.redo_shortcut)) {
            self.redo(1);
        } else if ui.input_mut(|i| i.consume_shortcut(&self.undo_shortcut)) {
            self.undo(1);
        } else if ui.input_mut(|i| i.consume_shortcut(&self.delete_shortcut)) {
            self.apply_commands(vec![SensitiveCommand::DeleteSelectedElements], true, true);
        } else if response.clicked() {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.click(
                    ((pos - self.camera_offset - response.rect.min.to_vec2()) / self.camera_scale)
                        .to_pos2(),
                    ui.input(|i| ModifierKeys::from_egui(&i.modifiers)),
                );
            }
        } else if response.dragged_by(egui::PointerButton::Middle) {
            self.camera_offset += response.drag_delta();
        } else if response.drag_started_by(egui::PointerButton::Primary) {
            self.last_unhandled_mouse_pos = ui.ctx().pointer_interact_pos();
        } else if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(cursor_pos) = &self.last_unhandled_mouse_pos {
                let last_down_pos =
                    (*cursor_pos - self.camera_offset - response.rect.min.to_vec2())
                        / self.camera_scale;
                self.drag(
                    last_down_pos.to_pos2(),
                    response.drag_delta() / self.camera_scale,
                );
                self.last_unhandled_mouse_pos = ui.ctx().pointer_interact_pos();
            }
        } else if response.drag_stopped() {
            self.last_unhandled_mouse_pos = None;
        }

        // Handle zoom
        if response.hovered() {
            let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta);

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
    fn click(&mut self, pos: egui::Pos2, modifiers: ModifierKeys) -> bool {
        self.current_tool.as_mut().map(|e| e.reset_event_lock());

        let mut commands = Vec::new();

        let handled = self
            .owned_controllers
            .iter_mut()
            .map(|uc| {
                match uc.1.write().unwrap().click(
                    &mut self.current_tool,
                    &mut commands,
                    pos,
                    modifiers,
                ) {
                    ClickHandlingStatus::HandledByElement => {
                        if !modifiers.command {
                            commands.push(SensitiveCommand::SelectAll(false));
                            commands.push(SensitiveCommand::Select(
                                std::iter::once(*uc.0).collect(),
                                true,
                            ));
                        } else {
                            commands.push(SensitiveCommand::Select(
                                std::iter::once(*uc.0).collect(),
                                !self.selected_elements.contains(&uc.0),
                            ));
                        }
                        ClickHandlingStatus::HandledByContainer
                    }
                    a => a,
                }
            })
            .find(|e| *e == ClickHandlingStatus::HandledByContainer)
            .ok_or_else(|| {
                commands.push(SensitiveCommand::SelectAll(false));
            })
            .is_ok();

        self.apply_commands(commands, true, true);

        if !handled {
            if let Some(t) = self.current_tool.as_mut() {
                t.add_position(pos);
            }
        }
        let mut tool = self.current_tool.take();
        if let Some(new_a) = tool.as_mut().and_then(|e| e.try_construct(self)) {
            let new_c = new_a.read().unwrap();
            let uuid = *new_c.uuid();

            let mut self_m = self.model.write().unwrap();
            self_m.add_element(new_c.model());
            drop(new_c);

            self.owned_controllers.insert(uuid, new_a);
            self.current_tool = tool;
            return true;
        }
        self.current_tool = tool;
        handled
    }
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool {
        let mut commands = Vec::new();

        let ret = self
            .owned_controllers
            .iter_mut()
            .find(|uc| {
                uc.1.write()
                    .unwrap()
                    .drag(&mut self.current_tool, &mut commands, last_pos, delta)
                    == DragHandlingStatus::Handled
            })
            .is_some();

        self.apply_commands(commands, true, true);

        ret
    }
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        ui.label("asdf");
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        (self.tool_change_fun)(&mut self.current_tool, ui);
    }
    fn show_properties(&mut self, ui: &mut egui::Ui) {
        if self
            .owned_controllers
            .iter()
            .find(|e| e.1.write().unwrap().show_properties(&self.queryable, ui))
            .is_none()
        {
            let mut model = self.model.write().unwrap();

            (self.show_props_fun)(&mut model, &mut self.buffer, ui);
        }
    }
    fn show_layers(&self, _ui: &mut egui::Ui) {
        // TODO: Layers???
    }
    fn show_menubar_edit_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui) {
        ui.menu_button("Undo", |ui| {
            ui.set_max_width(200.0);
            for (ii, c) in self.undo_stack.iter().rev().enumerate() {
                let mut button = egui::Button::new(c.0.info_text());
                if ii == 0 {
                    button = button.shortcut_text(ui.ctx().format_shortcut(&self.undo_shortcut));
                }

                if ui.add(button).clicked() {
                    self.undo(ii + 1);
                    break;
                }
            }
        });
        ui.menu_button("Redo", |ui| {
            ui.set_max_width(200.0);
            for (ii, c) in self.redo_stack.iter().rev().enumerate() {
                let mut button = egui::Button::new(c.info_text());
                if ii == 0 {
                    button = button.shortcut_text(ui.ctx().format_shortcut(&self.redo_shortcut));
                }

                if ui.add(button).clicked() {
                    self.redo(ii + 1);
                    break;
                }
            }
        });
        ui.separator();

        // TODO: implement
        if ui.button("Cut").clicked() {
            println!("no");
        }
        // TODO: implement
        if ui.button("Copy").clicked() {
            println!("no");
        }
        // TODO: implement
        if ui.button("Paste").clicked() {
            println!("no");
        }
    }
    fn show_menubar_diagram_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui) {
        (self.menubar_options_fun)(self, context, ui);
    }

    fn list_in_project_hierarchy(&self, ui: &mut egui::Ui) {
        let model = self.model.read().unwrap();

        egui::CollapsingHeader::new(format!("{} ({})", model.name(), model.uuid())).show(
            ui,
            |ui| {
                for uc in &self.owned_controllers {
                    uc.1.read()
                        .unwrap()
                        .list_in_project_hierarchy(&self.queryable, ui);
                }
            },
        );
    }

    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, mouse_pos: Option<egui::Pos2>) {
        let tool = if let (Some(pos), Some(stage)) = (mouse_pos, self.current_tool.as_ref()) {
            Some((pos, stage))
        } else {
            None
        };
        let mut drawn_targetting = TargettingStatus::NotDrawn;

        self.owned_controllers
            .iter_mut()
            .filter(|_| true) // TODO: filter by layers
            .for_each(|uc| {
                if uc
                    .1
                    .write()
                    .unwrap()
                    .draw_in(&self.queryable, canvas, &tool)
                    == TargettingStatus::Drawn
                {
                    drawn_targetting = TargettingStatus::Drawn;
                }
            });

        if let Some((pos, tool)) = tool {
            if drawn_targetting == TargettingStatus::NotDrawn {
                canvas.draw_rectangle(
                    egui::Rect::EVERYTHING,
                    egui::Rounding::ZERO,
                    tool.targetting_for_element(ToolT::KindedElement::from(self)),
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                self.owned_controllers
                    .iter_mut()
                    .filter(|_| true) // TODO: filter by layers
                    .for_each(|uc| {
                        uc.1.write()
                            .unwrap()
                            .draw_in(&self.queryable, canvas, &Some((pos, tool)));
                    });
            }
            tool.draw_status_hint(canvas, pos);
        }
    }
}

impl<
        DiagramModelT: ContainerModel<ElementModelT>,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT,
        AddCommandElementT: Clone,
    > ContainerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>
    for DiagramControllerGen2<
        DiagramModelT,
        ElementModelT,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT,
    >
where
    ToolT: Tool<ElementModelT, QueryableT, AddCommandElementT>,
    AddCommandElementT: From<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
                >,
            >,
        )> + TryInto<(
            uuid::Uuid,
            Arc<
                RwLock<
                    dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>,
                >,
            >,
        )>,
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<
        Arc<
            RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>>,
        >,
    > {
        self.owned_controllers.get(uuid).cloned()
    }
}

pub struct MulticonnectionView<
    ModelT,
    ElementModelT: ?Sized + 'static,
    QueryableT,
    BufferT,
    ToolT,
    AddCommandElementT: Clone,
> where
    AddCommandElementT:
        From<(uuid::Uuid, uuid::Uuid, egui::Pos2)> + TryInto<(uuid::Uuid, uuid::Uuid, egui::Pos2)>,
{
    pub model: Arc<RwLock<ModelT>>,
    pub buffer: BufferT,

    pub source: Arc<
        RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>>,
    >,
    pub destination: Arc<
        RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>>,
    >,

    pub highlight: canvas::Highlight,
    pub selected_vertices: HashSet<uuid::Uuid>,
    pub center_point: Option<(uuid::Uuid, egui::Pos2)>,
    pub source_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,
    pub dest_points: Vec<Vec<(uuid::Uuid, egui::Pos2)>>,

    pub model_to_element_shim: fn(Arc<RwLock<ModelT>>) -> Arc<RwLock<ElementModelT>>,

    pub show_properties_fun: fn(&mut ModelT, &mut BufferT, &mut egui::Ui),

    pub model_to_uuid: fn(&ModelT) -> Arc<uuid::Uuid>,
    pub model_to_name: fn(&ModelT) -> Arc<String>,
    pub model_to_line_type: fn(&ModelT) -> canvas::LineType,
    pub model_to_source_arrowhead_type: fn(&ModelT) -> canvas::ArrowheadType,
    pub model_to_destination_arrowhead_type: fn(&ModelT) -> canvas::ArrowheadType,
    pub model_to_source_arrowhead_label: fn(&ModelT) -> Option<&str>,
    pub model_to_destination_arrowhead_label: fn(&ModelT) -> Option<&str>,
}

impl<
        ModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT: Clone,
    > ElementController<ElementModelT>
    for MulticonnectionView<ModelT, ElementModelT, QueryableT, BufferT, ToolT, AddCommandElementT>
where
    AddCommandElementT:
        From<(uuid::Uuid, uuid::Uuid, egui::Pos2)> + TryInto<(uuid::Uuid, uuid::Uuid, egui::Pos2)>,
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
        QueryableT,
        BufferT,
        ToolT,
        AddCommandElementT: Clone,
    > ElementControllerGen2<ElementModelT, QueryableT, ToolT, AddCommandElementT>
    for MulticonnectionView<ModelT, ElementModelT, QueryableT, BufferT, ToolT, AddCommandElementT>
where
    ToolT: Tool<ElementModelT, QueryableT, AddCommandElementT>,
    AddCommandElementT:
        From<(uuid::Uuid, uuid::Uuid, egui::Pos2)> + TryInto<(uuid::Uuid, uuid::Uuid, egui::Pos2)>,
{
    fn show_properties(&mut self, _parent: &QueryableT, ui: &mut egui::Ui) -> bool {
        if !self.highlight.selected {
            return false;
        }

        let mut c = self.model.write().unwrap();
        (self.show_properties_fun)(&mut c, &mut self.buffer, ui);

        true
    }

    fn draw_in(
        &mut self,
        _: &QueryableT,
        canvas: &mut dyn NHCanvas,
        _tool: &Option<(egui::Pos2, &ToolT)>,
    ) -> TargettingStatus {
        let model = self.model.read().unwrap();
        let (source_pos, source_bounds) = {
            let lock = self.source.read().unwrap();
            (lock.position(), lock.min_shape())
        };
        let (dest_pos, dest_bounds) = {
            let lock = self.destination.read().unwrap();
            (lock.position(), lock.min_shape())
        };
        let (source_next_point, dest_next_point) = match (
            self.source_points[0]
                .get(1)
                .map(|e| *e)
                .or(self.center_point)
                .map(|e| e.1),
            self.dest_points[0]
                .get(1)
                .map(|e| *e)
                .or(self.center_point)
                .map(|e| e.1),
        ) {
            (None, None) => {
                let pos_avg = (source_pos + dest_pos.to_vec2()) / 2.0;
                (pos_avg, pos_avg)
            }
            (source_next_point, dest_next_point) => (
                source_next_point.unwrap_or(dest_pos),
                dest_next_point.unwrap_or(source_pos),
            ),
        };

        match (
            source_bounds
                .orthogonal_intersect(source_next_point)
                .or_else(|| source_bounds.center_intersect(source_next_point)),
            dest_bounds
                .orthogonal_intersect(dest_next_point)
                .or_else(|| dest_bounds.center_intersect(dest_next_point)),
        ) {
            (Some(source_intersect), Some(dest_intersect)) => {
                self.source_points[0][0].1 = source_intersect;
                self.dest_points[0][0].1 = dest_intersect;
                canvas.draw_multiconnection(
                    &self.selected_vertices,
                    &[(
                        (self.model_to_source_arrowhead_type)(&*model),
                        crate::common::canvas::Stroke {
                            width: 1.0,
                            color: egui::Color32::BLACK,
                            line_type: (self.model_to_line_type)(&*model),
                        },
                        &self.source_points[0],
                        (self.model_to_source_arrowhead_label)(&*model),
                    )],
                    &[(
                        (self.model_to_destination_arrowhead_type)(&*model),
                        crate::common::canvas::Stroke {
                            width: 1.0,
                            color: egui::Color32::BLACK,
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
            }
            _ => {}
        }

        TargettingStatus::NotDrawn
    }

    fn click(
        &mut self,
        _tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT>>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus {
        const DISTANCE_THRESHOLD: f32 = 3.0;

        macro_rules! handle_vertex {
            ($uuid:expr) => {
                if !modifiers.command {
                    commands.push(SensitiveCommand::SelectAll(false));
                    commands.push(SensitiveCommand::Select(
                        std::iter::once(*$uuid).collect(),
                        true,
                    ));
                } else {
                    commands.push(SensitiveCommand::Select(
                        std::iter::once(*$uuid).collect(),
                        !self.selected_vertices.contains($uuid),
                    ));
                }
                return ClickHandlingStatus::HandledByContainer;
            };
        }

        fn is_over(a: egui::Pos2, b: egui::Pos2) -> bool {
            a.distance(b) <= DISTANCE_THRESHOLD
        }

        if let Some((uuid, _)) = self.center_point.as_ref().filter(|e| is_over(pos, e.1)) {
            handle_vertex!(uuid);
        }

        macro_rules! check_joints {
            ($v:ident) => {
                for path in &self.$v {
                    let stop_idx = path.len();
                    for joint in &path[1..stop_idx] {
                        if is_over(pos, joint.1) {
                            handle_vertex!(&joint.0);
                        }
                    }
                }
            };
        }
        check_joints!(source_points);
        check_joints!(dest_points);

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
                            return ClickHandlingStatus::HandledByElement;
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
                        return ClickHandlingStatus::HandledByElement;
                    }
                }
            }
        }
        ClickHandlingStatus::NotHandled
    }
    fn drag(
        &mut self,
        _tool: &mut Option<ToolT>,
        commands: &mut Vec<SensitiveCommand<AddCommandElementT>>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus {
        const DISTANCE_THRESHOLD: f32 = 3.0;

        fn is_over(a: egui::Pos2, b: egui::Pos2) -> bool {
            a.distance(b) <= DISTANCE_THRESHOLD
        }

        match self.center_point {
            // Check whether over center point, if so move it
            Some((uuid, pos)) => {
                if is_over(last_pos, pos) {
                    if self.selected_vertices.contains(&uuid) {
                        commands.push(SensitiveCommand::MoveSelectedElements(delta));
                    } else {
                        commands.push(SensitiveCommand::MoveElements(
                            std::iter::once(uuid).collect(),
                            delta,
                        ));
                    }

                    return DragHandlingStatus::Handled;
                }
            }
            // Check whether over a midpoint, if so set center point
            None => {
                // TODO: this is generally wrong (why??)
                let midpoint = self.position();
                if is_over(last_pos, midpoint) {
                    commands.push(SensitiveCommand::AddElement(
                        *self.uuid(),
                        (uuid::Uuid::nil(), uuid::Uuid::now_v7(), midpoint + delta).into(),
                    ));
                    return DragHandlingStatus::Handled;
                }
            }
        }

        // Check whether over a joint, if so move it
        macro_rules! check_joints {
            ($v:ident) => {
                for path in &mut self.$v {
                    let stop_idx = path.len();
                    for joint in &mut path[1..stop_idx] {
                        if is_over(last_pos, joint.1) {
                            if self.selected_vertices.contains(&joint.0) {
                                commands.push(SensitiveCommand::MoveSelectedElements(delta));
                            } else {
                                commands.push(SensitiveCommand::MoveElements(
                                    std::iter::once(joint.0).collect(),
                                    delta,
                                ));
                            }

                            return DragHandlingStatus::Handled;
                        }
                    }
                }
            };
        }
        check_joints!(source_points);
        check_joints!(dest_points);

        // Check whether over midpoint, if so add a new joint
        macro_rules! check_midpoints {
            ($v:ident) => {
                for path in &mut self.$v {
                    // Iterates over 2-windows
                    let mut iter = path
                        .iter()
                        .map(|e| *e)
                        .chain(self.center_point)
                        .enumerate()
                        .peekable();
                    while let Some((idx, u)) = iter.next() {
                        let v = if let Some((_, v)) = iter.peek() {
                            *v
                        } else {
                            break;
                        };

                        let midpoint = (u.1 + v.1.to_vec2()) / 2.0;
                        if is_over(last_pos, midpoint) {
                            commands.push(SensitiveCommand::AddElement(
                                *self.uuid(),
                                (u.0, uuid::Uuid::now_v7(), midpoint + delta).into(),
                            ));
                            return DragHandlingStatus::Handled;
                        }
                    }
                }
            };
        }
        check_midpoints!(source_points);
        check_midpoints!(dest_points);

        DragHandlingStatus::NotHandled
    }

    fn apply_command(
        &mut self,
        command: &InsensitiveCommand<AddCommandElementT>,
        undo_accumulator: &mut Vec<InsensitiveCommand<AddCommandElementT>>,
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
            InsensitiveCommand::Select(uuids, select) => {
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
            InsensitiveCommand::MoveElements(uuids, delta) => {
                let multiconnection_present = uuids.contains(&*self.uuid());
                for p in
                    all_pts_mut!(self).filter(|e| multiconnection_present || uuids.contains(&e.0))
                {
                    p.1 += *delta;
                    undo_accumulator.push(InsensitiveCommand::MoveElements(
                        std::iter::once(p.0).collect(),
                        -*delta,
                    ));
                }
            }
            InsensitiveCommand::DeleteElements(uuids) => {
                let self_uuid = *self.uuid();
                if let Some(center_point) =
                    self.center_point.as_mut().filter(|e| uuids.contains(&e.0))
                {
                    undo_accumulator.push(InsensitiveCommand::AddElement(
                        self_uuid,
                        AddCommandElementT::from((
                            uuid::Uuid::nil(),
                            center_point.0,
                            center_point.1,
                        )),
                    ));
                    self.center_point = None;
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
                                        AddCommandElementT::from((a.0, b.0, b.1)),
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
                    if let Ok((a, b, c)) = element.clone().try_into() {
                        if a.is_nil() {
                            self.center_point = Some((b, c));

                            undo_accumulator.push(InsensitiveCommand::DeleteElements(
                                std::iter::once(b).collect(),
                            ));
                        } else {
                            macro_rules! insert_vertex {
                                ($self:ident, $v:ident) => {
                                    for path in $self.$v.iter_mut() {
                                        for (idx, p) in path.iter().enumerate() {
                                            if p.0 == a {
                                                path.insert(idx + 1, (b, c));
                                                undo_accumulator.push(
                                                    InsensitiveCommand::DeleteElements(
                                                        std::iter::once(b).collect(),
                                                    ),
                                                );
                                                return;
                                            }
                                        }
                                    }
                                };
                            }
                            insert_vertex!(self, source_points);
                            insert_vertex!(self, dest_points);
                        }
                    }
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
    egui::ComboBox::from_id_source(name)
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
