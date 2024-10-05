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
    fn show_menubar_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui);
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

#[derive(PartialEq, Debug)]
pub enum DiagramCommand {
    SelectAll,
    UnselectAll,
    Select(uuid::Uuid),
    Unselect(uuid::Uuid),
    MoveSelectedElements(egui::Vec2),
}

pub trait Model: 'static {
    fn uuid(&self) -> Arc<uuid::Uuid>;
    fn name(&self) -> Arc<String>;
}

pub trait ContainerModel<ModelT: ?Sized>: Model {
    fn add_element(&mut self, _: Arc<RwLock<ModelT>>);
}

pub trait KindedElement<'a> {
    type DiagramType;

    fn diagram(_: &'a Self::DiagramType) -> Self;
    fn package() -> Self;
}

pub trait Tool<CommonElementT: ?Sized, QueryableT> {
    type KindedElement<'a>: KindedElement<'a>;
    type Stage;

    fn initial_stage(&self) -> Self::Stage;

    fn targetting_for_element<'a>(&self, controller: Self::KindedElement<'a>) -> egui::Color32;
    fn draw_status_hint(&self, canvas: &mut dyn NHCanvas, pos: egui::Pos2);

    fn offset_by(&mut self, delta: egui::Vec2);
    fn add_position(&mut self, pos: egui::Pos2);
    fn add_element<'a>(&mut self, controller: Self::KindedElement<'a>, pos: egui::Pos2);
    fn try_construct(
        &mut self,
        into: &dyn ContainerGen2<CommonElementT, QueryableT, Self>,
    ) -> Option<Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, Self>>>>;
    fn reset_event_lock(&mut self);
}

pub trait ElementControllerGen2<CommonElementT: ?Sized, QueryableT, ToolT>:
    ElementController<CommonElementT>
where
    ToolT: Tool<CommonElementT, QueryableT>,
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
        commands: &mut Vec<DiagramCommand>,
        pos: egui::Pos2,
        modifiers: ModifierKeys,
    ) -> ClickHandlingStatus;
    fn drag(
        &mut self,
        tool: &mut Option<ToolT>,
        commands: &mut Vec<DiagramCommand>,
        last_pos: egui::Pos2,
        delta: egui::Vec2,
    ) -> DragHandlingStatus;
    fn apply_command(&mut self, command: &DiagramCommand);
}

pub trait ContainerGen2<CommonElementT: ?Sized, QueryableT, ToolT> {
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<Arc<RwLock<dyn ElementControllerGen2<CommonElementT, QueryableT, ToolT>>>>;
}

/// This is a generic DiagramController implementation.
/// Hopefully it should reduce the amount of code, but nothing prevents creating fully custom DiagramController implementations.
pub struct DiagramControllerGen2<
    DiagramModelT,
    ElementModelT: ?Sized + 'static,
    QueryableT,
    BufferT,
    ToolT,
> where
    DiagramModelT: ContainerModel<ElementModelT>,
    ToolT: Tool<ElementModelT, QueryableT>,
{
    model: Arc<RwLock<DiagramModelT>>,
    owned_controllers: HashMap<
        uuid::Uuid,
        Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT>>>,
    >,

    pub _layers: Vec<bool>,

    pub camera_offset: egui::Pos2,
    pub camera_scale: f32,
    last_unhandled_mouse_pos: Option<egui::Pos2>,
    selected_elements: HashSet<uuid::Uuid>,
    current_tool: Option<ToolT>,

    // q: dyn Fn(&Vec<DomainElementT>) -> QueryableT,
    queryable: QueryableT,
    buffer: BufferT,
    show_props_fun: fn(&mut DiagramModelT, &mut BufferT, &mut egui::Ui),
    tool_change_fun: fn(&mut Option<ToolT>, &mut egui::Ui),
    menubar_options_fun: fn(&mut Self, &mut NHApp, &mut egui::Ui),
}

impl<
        DiagramModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT,
    > DiagramControllerGen2<DiagramModelT, ElementModelT, QueryableT, BufferT, ToolT>
where
    DiagramModelT: ContainerModel<ElementModelT>,
    ToolT: Tool<ElementModelT, QueryableT>,
{
    pub fn new(
        model: Arc<RwLock<DiagramModelT>>,
        owned_controllers: HashMap<
            uuid::Uuid,
            Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT>>>,
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
            current_tool: None,

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

    fn apply_commands(&mut self, commands: Vec<DiagramCommand>) {
        for c in &commands {
            for e in &self.owned_controllers {
                let mut e = e.1.write().unwrap();
                e.apply_command(c);
            }
        }
    }
}

impl<
        DiagramModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT,
    > DiagramController
    for DiagramControllerGen2<DiagramModelT, ElementModelT, QueryableT, BufferT, ToolT>
where
    DiagramModelT: ContainerModel<ElementModelT>,
    ToolT: for<'a> Tool<
            ElementModelT,
            QueryableT,
            KindedElement<'a>: KindedElement<'a, DiagramType = Self>,
        > + 'static,
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
        if response.clicked() {
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
        self.current_tool
            .as_mut()
            .map(|e| e.reset_event_lock());

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
                            commands.push(DiagramCommand::UnselectAll);
                            commands.push(DiagramCommand::Select(*uc.0));
                        } else if self.selected_elements.contains(&uc.0) {
                            commands.push(DiagramCommand::Unselect(*uc.0));
                        } else {
                            commands.push(DiagramCommand::Select(*uc.0));
                        };
                        ClickHandlingStatus::HandledByContainer
                    }
                    a => a,
                }
            })
            .find(|e| *e == ClickHandlingStatus::HandledByContainer)
            .ok_or_else(|| {
                commands.push(DiagramCommand::UnselectAll);
            })
            .is_ok();

        self.apply_commands(commands);

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
                uc.1.write().unwrap().drag(
                    &mut self.current_tool,
                    &mut commands,
                    last_pos,
                    delta,
                ) == DragHandlingStatus::Handled
            })
            .is_some();

        self.apply_commands(commands);

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
    fn show_menubar_options(&mut self, context: &mut NHApp, ui: &mut egui::Ui) {
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
                    tool.targetting_for_element(ToolT::KindedElement::diagram(&self)),
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
        DiagramModelT,
        ElementModelT: ?Sized + 'static,
        QueryableT: 'static,
        BufferT: 'static,
        ToolT,
    > ContainerGen2<ElementModelT, QueryableT, ToolT>
    for DiagramControllerGen2<DiagramModelT, ElementModelT, QueryableT, BufferT, ToolT>
where
    DiagramModelT: ContainerModel<ElementModelT>,
    ToolT: Tool<ElementModelT, QueryableT>,
{
    fn controller_for(
        &self,
        uuid: &uuid::Uuid,
    ) -> Option<Arc<RwLock<dyn ElementControllerGen2<ElementModelT, QueryableT, ToolT>>>> {
        self.owned_controllers.get(uuid).cloned()
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

pub mod macros {
    // TODO: parametrize
    macro_rules! multiconnection_draw_in {
        ($self:ident, $canvas:ident) => {
            let model = $self.model.read().unwrap();
            let (source_pos, source_bounds) = {
                let lock = $self.source.read().unwrap();
                (lock.position(), lock.min_shape())
            };
            let (dest_pos, dest_bounds) = {
                let lock = $self.destination.read().unwrap();
                (lock.position(), lock.min_shape())
            };
            let (source_next_point, dest_next_point) = match (
                $self.source_points[0]
                    .get(1)
                    .map(|e| *e)
                    .or($self.center_point),
                $self.dest_points[0]
                    .get(1)
                    .map(|e| *e)
                    .or($self.center_point),
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
                    $self.source_points[0][0] = source_intersect;
                    $self.dest_points[0][0] = dest_intersect;
                    $canvas.draw_multiconnection(
                        &[(
                            model.link_type.source_arrowhead_type(),
                            crate::common::canvas::Stroke {
                                width: 1.0,
                                color: egui::Color32::BLACK,
                                line_type: model.link_type.line_type(),
                            },
                            &$self.source_points[0],
                            Some(&model.source_arrowhead_label),
                        )],
                        &[(
                            model.link_type.destination_arrowhead_type(),
                            crate::common::canvas::Stroke {
                                width: 1.0,
                                color: egui::Color32::BLACK,
                                line_type: model.link_type.line_type(),
                            },
                            &$self.dest_points[0],
                            Some(&model.destination_arrowhead_label),
                        )],
                        $self.position(),
                        None,
                        $self.highlight,
                    );
                }
                _ => {}
            }
        };
    }
    pub(crate) use multiconnection_draw_in;

    // center_point: Option<egui::Pos2>
    // fn sources(&mut self) -> &mut [Vec<egui::Pos2>];
    // fn destinations(&mut self) -> &mut [Vec<egui::Pos2>];
    macro_rules! multiconnection_element_click {
        ($self:ident, $last_pos:ident, $delta:ident, $center_point:ident, $sources:ident, $destinations:ident, $ret:expr) => {
            const DISTANCE_THRESHOLD: f32 = 3.0;

            fn dist_to_line_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
                fn dist2(a: egui::Pos2, b: egui::Pos2) -> f32 {
                    (a.x - b.x).powf(2.0) + (a.y - b.y).powf(2.0)
                }
                let l2 = dist2(a, b);
                let distance_squared = if l2 == 0.0 {
                    dist2(p, a)
                } else {
                    let t = (((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2)
                        .clamp(0.0, 1.0);
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
                    let center_point = $self.center_point.clone();
                    for path in $self.$v() {
                        // Iterates over 2-windows
                        let mut iter = path.iter().map(|e| *e).chain(center_point).peekable();
                        while let Some(u) = iter.next() {
                            let v = if let Some(v) = iter.peek() {
                                *v
                            } else {
                                break;
                            };

                            if dist_to_line_segment($last_pos, u, v) <= DISTANCE_THRESHOLD {
                                return $ret;
                            }
                        }
                    }
                };
            }
            check_path_segments!(sources);
            check_path_segments!(destinations);

            // In case there is no center_point, also check all-to-all of last points
            if $self.center_point == None {
                // TODO: this shouldn't have to clone, but probably not that big of a deal
                let destinations: Vec<egui::Pos2> = $self
                    .destinations()
                    .iter()
                    .flat_map(|e| e.last().cloned())
                    .collect();
                for u in $self.sources().iter().flat_map(|e| e.last()) {
                    for v in &destinations {
                        if dist_to_line_segment($last_pos, *u, *v) <= DISTANCE_THRESHOLD {
                            return $ret;
                        }
                    }
                }
            }
        };
    }
    pub(crate) use multiconnection_element_click;

    macro_rules! multiconnection_element_drag {
        ($self:ident, $last_pos:ident, $delta:ident, $center_point:ident, $sources:ident, $destinations:ident, $ret:expr) => {
            const DISTANCE_THRESHOLD: f32 = 3.0;

            fn is_over(a: egui::Pos2, b: egui::Pos2) -> bool {
                a.distance(b) <= DISTANCE_THRESHOLD
            }

            match $self.center_point {
                // Check whether over center point, if so move it
                Some(pos) => {
                    if is_over($last_pos, pos) {
                        $self.center_point = Some(pos + $delta);
                        return $ret;
                    }
                }
                // Check whether over a midpoint, if so set center point
                None => {
                    // TODO: this is generally wrong (why??)
                    let midpoint = $self.position();
                    if is_over($last_pos, midpoint) {
                        $self.center_point = Some(midpoint + $delta);
                        return $ret;
                    }
                }
            }

            // Check whether over a joint, if so move it
            macro_rules! check_joints {
                ($v:ident) => {
                    for path in $self.$v() {
                        let stop_idx = path.len();
                        for joint in &mut path[1..stop_idx] {
                            if is_over($last_pos, *joint) {
                                *joint += $delta;
                                return $ret;
                            }
                        }
                    }
                };
            }
            check_joints!(sources);
            check_joints!(destinations);

            // Check whether over midpoint, if so add a new joint
            macro_rules! check_midpoints {
                ($v:ident) => {
                    let center_point = $self.center_point.clone();
                    for path in $self.$v() {
                        // Iterates over 2-windows
                        let mut iter = path
                            .iter()
                            .map(|e| *e)
                            .chain(center_point)
                            .enumerate()
                            .peekable();
                        while let Some((idx, u)) = iter.next() {
                            let v = if let Some((_, v)) = iter.peek() {
                                *v
                            } else {
                                break;
                            };

                            let midpoint = (u + v.to_vec2()) / 2.0;
                            if is_over($last_pos, midpoint) {
                                path.insert(idx + 1, midpoint + $delta);
                                return $ret;
                            }
                        }
                    }
                };
            }
            check_midpoints!(sources);
            check_midpoints!(destinations);
        };
    }
    pub(crate) use multiconnection_element_drag;
}
