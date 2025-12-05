use std::{collections::{HashMap, HashSet}, sync::Arc};

use eframe::{egui, epaint};

use crate::{CustomModal, common::{canvas::{self, Highlight}, controller::{ColorBundle, ContainerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GlobalDrawingContext, InputEvent, InsensitiveCommand, PositionNoT, PropertiesStatus, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, Tool, View}, entity::{Entity, EntityUuid}, eref::ERef, project_serde::{NHContextDeserialize, NHContextSerialize}, uuid::{ModelUuid, ViewUuid}, views::ordered_views::OrderedViews}};


pub trait PackageAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + Send + Sync + 'static {
    fn model_section(&self) -> DomainT::ViewTargettingSectionT;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;

    fn insert_element(&mut self, position: Option<PositionNoT>, element: DomainT::CommonElementT) -> Result<PositionNoT, ()>;
    fn delete_element(&mut self, uuids: &ModelUuid) -> Option<PositionNoT>;

    fn background_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::WHITE
    }
    fn foreground_color(&self, _global_colors: &ColorBundle) -> egui::Color32 {
        egui::Color32::BLACK
    }
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
#[nh_context_serde(is_entity)]
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
        (*self.uuid).into()
    }
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> View for PackageView<DomainT, AdapterT> {
    fn uuid(&self) -> Arc<ViewUuid> {
        self.uuid.clone()
    }
    fn model_uuid(&self) -> Arc<ModelUuid> {
        self.adapter.model_uuid()
    }
}

impl<DomainT: Domain, AdapterT: PackageAdapter<DomainT>> ElementController<DomainT::CommonElementT> for PackageView<DomainT, AdapterT> {
    fn model(&self) -> DomainT::CommonElementT {
        self.adapter.model_section().into()
    }

    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Rect {
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
        drawing_context: &GlobalDrawingContext,
        parent: &DomainT::QueryableT<'_>,
        lp: &DomainT::LabelProviderT,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> PropertiesStatus<DomainT> {
        let child = self
            .owned_views
            .event_order_find_mut(|v| v.show_properties(drawing_context, parent, lp, ui, commands).to_non_default());

        if let Some(child) = child {
            child
        } else if self.highlight.selected {
            ui.label("Model properties");

            self.adapter.show_properties(ui, commands);

            ui.add_space(super::VIEW_MODEL_PROPERTIES_BLOCK_SPACING);
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

            PropertiesStatus::Shown
        } else {
            PropertiesStatus::NotShown
        }
    }
    fn draw_in(
        &mut self,
        q: &DomainT::QueryableT<'_>,
        context: &GlobalDrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        tool: &Option<(egui::Pos2, &DomainT::ToolT)>,
    ) -> TargettingStatus {
        // Draw shape and text
        canvas.draw_rectangle(
            self.bounds_rect,
            egui::CornerRadius::ZERO,
            self.adapter.background_color(&context.global_colors),
            canvas::Stroke::new_solid(1.0, self.adapter.foreground_color(&context.global_colors)),
            self.highlight,
        );

        canvas.draw_text(
            self.bounds_rect.center_top(),
            egui::Align2::CENTER_TOP,
            &self.adapter.model_name(),
            canvas::CLASS_MIDDLE_FONT_SIZE,
            self.adapter.foreground_color(&context.global_colors),
        );

        // Draw resize/drag handles
        if let Some(ui_scale) = canvas.ui_scale().filter(|_| self.highlight.selected) {
            let handle_size = self.handle_size(ui_scale);
            //compile_error!("icons")
            for (h, c) in [
                (self.bounds_rect.left_top(), "↖"),
                (self.bounds_rect.center_top(), "^"),
                (self.bounds_rect.right_top(), "↗"),
                (self.bounds_rect.left_center(), "<"),
                (self.bounds_rect.right_center(), ">"),
                (self.bounds_rect.left_bottom(), "↙"),
                (self.bounds_rect.center_bottom(), "v"),
                (self.bounds_rect.right_bottom(), "↘"),
            ] {
                canvas.draw_rectangle(
                    egui::Rect::from_center_size(h, egui::Vec2::splat(handle_size / ui_scale)),
                    egui::CornerRadius::ZERO,
                    egui::Color32::WHITE,
                    canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                    canvas::Highlight::NONE,
                );
                canvas.draw_text(
                    h,
                    egui::Align2::CENTER_CENTER,
                    c,
                    10.0 / ui_scale,
                    egui::Color32::BLACK,
                );
            }

            let dc = self.drag_handle_position(ui_scale);
            canvas.draw_rectangle(
                egui::Rect::from_center_size(
                    dc,
                    egui::Vec2::splat(handle_size / ui_scale),
                ),
                egui::CornerRadius::ZERO,
                egui::Color32::WHITE,
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );

            let da_radius = (handle_size / 2.0 - 1.0) / ui_scale;
            canvas.draw_line(
                [
                    dc - egui::Vec2::new(0.0, da_radius),
                    dc + egui::Vec2::new(0.0, da_radius),
                ],
                canvas::Stroke::new_solid(1.0, egui::Color32::BLACK),
                canvas::Highlight::NONE,
            );
            canvas.draw_line(
                [
                    dc - egui::Vec2::new(da_radius, 0.0),
                    dc + egui::Vec2::new(da_radius, 0.0),
                ],
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
                        t.targetting_for_section(Some(self.adapter.model_section())),
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
        element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> EventHandlingStatus {
        let k_status = self.owned_views.event_order_find_mut(|v| {
            let s = v.handle_event(event, ehc, tool, element_setup_modal, commands);
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
                if self.highlight.selected {
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
                if !self.min_shape().contains(pos) {
                    return k_status.map(|e| e.1).unwrap_or(EventHandlingStatus::NotHandled);
                }

                if let Some(tool) = tool {
                    tool.add_position(*event.mouse_position());
                    tool.add_section(self.adapter.model_section());

                    if let Some((new_e, esm)) = tool.try_construct_view(self) {
                        commands.push(InsensitiveCommand::AddDependency(*self.uuid, 0, None, new_e.into(), true).into());
                        if ehc.modifier_settings.alternative_tool_mode.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            *element_setup_modal = esm;
                        }
                    }

                    EventHandlingStatus::HandledByContainer
                } else if let Some((k, status)) = k_status {
                    if status == EventHandlingStatus::HandledByElement {
                        if ehc.modifier_settings.hold_selection.is_none_or(|e| !ehc.modifiers.is_superset_of(e)) {
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                true,
                                Highlight::SELECTED,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(k).collect(),
                                !self.selected_direct_elements.contains(&k),
                                Highlight::SELECTED,
                            ).into());
                        }
                    }
                    EventHandlingStatus::HandledByContainer
                } else {
                    EventHandlingStatus::HandledByElement
                }
            },
            InputEvent::Drag { delta, .. } => match self.dragged_type_and_shape {
                Some((PackageDragType::Move, real_bounds)) => {
                    let translated_bounds = real_bounds.translate(delta);
                    self.dragged_type_and_shape = Some((PackageDragType::Move, translated_bounds));
                    let translated_real_shape = canvas::NHShape::Rect { inner: translated_bounds };
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
                        canvas::NHShape::Rect { inner: egui::Rect::from_min_size(egui::Pos2::new(handle_x.0, handle_y.0), egui::Vec2::ZERO) },
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
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
                if h.selected {
                    match set {
                        true => {
                            self.selected_direct_elements =
                                self.owned_views.iter_event_order_keys().collect()
                        }
                        false => self.selected_direct_elements.clear(),
                    }
                }
                recurse!(self);
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }

                if h.selected {
                    for k in self.owned_views.iter_event_order_keys().filter(|k| uuids.contains(k)) {
                        match set {
                            true => self.selected_direct_elements.insert(k),
                            false => self.selected_direct_elements.remove(&k),
                        };
                    }
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
                for (_uuid, element) in self
                    .owned_views
                    .iter_event_order_pairs()
                    .filter(|e| uuids.contains(&e.0))
                {
                    let pos = if !from_model {
                        None
                    } else if let Some(pos) = self.adapter.delete_element(&element.model_uuid()) {
                        Some(pos)
                    } else {
                        continue;
                    };

                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                        *self.uuid,
                        0,
                        pos,
                        element.clone().into(),
                        from_model,
                    ));
                }

                self.owned_views.retain(|k, _v| !uuids.contains(k));

                recurse!(self);
            }
            InsensitiveCommand::PasteSpecificElements(target, _elements) => {
                if *target == *self.uuid {
                    todo!("undo = delete")
                }

                recurse!(self);
            },
            InsensitiveCommand::AddDependency(target, b, pos, element, into_model) => {
                if *target == *self.uuid && *b == 0 {
                    if let Ok(mut view) = element.clone().try_into()
                        && (!*into_model || self.adapter.insert_element(*pos, view.model()).is_ok()){
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

                recurse!(self);
            }
            InsensitiveCommand::RemoveDependency(..) => {
                recurse!(self);
            }
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
            (ViewUuid::now_v7(), ModelUuid::now_v7())
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
        self.adapter.deep_copy_finish(m);
    }
}
