
use std::{collections::{HashMap, HashSet}, sync::Arc};
use eframe::egui;

use crate::{common::{canvas, controller::{ContainerGen2, Domain, DrawingContext, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, InputEvent, InsensitiveCommand, PropertiesStatus, SelectionStatus, SensitiveCommand, SnapManager, TargettingStatus, View}, entity::{Entity, EntityUuid}, eref::ERef, project_serde::{NHContextDeserialize, NHContextSerialize}, ufoption::UFOption, uuid::{ModelUuid, ViewUuid}}, CustomModal};


pub trait MulticonnectionAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + Send + Sync {
    fn model(&self) -> DomainT::CommonElementT;
    fn model_uuid(&self) -> Arc<ModelUuid>;
    fn model_name(&self) -> Arc<String>;

    fn background_color(&self) -> egui::Color32 {
        egui::Color32::WHITE
    }
    fn foreground_color(&self) -> egui::Color32 {
        egui::Color32::BLACK
    }
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
#[derive(Clone, Copy, Debug)]
pub struct FlipMulticonnection {}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
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
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> ElementController<DomainT::CommonElementT> for MulticonnectionView<DomainT, AdapterT>
where
    DomainT::AddCommandElementT: From<VertexInformation> + TryInto<VertexInformation>,
    for<'a> &'a DomainT::PropChangeT: TryInto<FlipMulticonnection>,
{
    fn model(&self) -> DomainT::CommonElementT {
        self.adapter.model()
    }

    fn min_shape(&self) -> canvas::NHShape {
        canvas::NHShape::Rect {
            inner: egui::Rect::NOTHING,
        }
    }
    fn max_shape(&self) -> canvas::NHShape {
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
        _drawing_context: &DrawingContext,
        _parent: &DomainT::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<SensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> PropertiesStatus {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        self.adapter.show_properties(ui, commands);

        PropertiesStatus::Shown
    }

    fn draw_in(
        &mut self,
        _: &DomainT::QueryableT<'_>,
        context: &DrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
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

        fn s_to_p(canvas: &mut dyn canvas::NHCanvas, bounds: canvas::NHShape, pos: egui::Pos2, s: &str) -> egui::Pos2 {
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
                    color: self.adapter.foreground_color(),
                    line_type: source_line_type,
                },
                &self.source_points[0],
                l1,
            )],
            &[(
                dest_arrow_type,
                crate::common::canvas::Stroke {
                    width: 1.0,
                    color: self.adapter.foreground_color(),
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
            am.add_shape(*self.uuid, canvas::NHShape::Rect { inner: egui::Rect::from_min_size(p.1, egui::Vec2::ZERO) });
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        _tool: &mut Option<DomainT::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
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
                let translated_real_shape = canvas::NHShape::Rect { inner: egui::Rect::from_min_size(translated_real_pos, egui::Vec2::ZERO) };
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
                        if let Ok(FlipMulticonnection {}) = property.try_into() {
                            std::mem::swap(&mut self.source, &mut self.target);
                            std::mem::swap(&mut self.source_points, &mut self.dest_points);
                        }
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
