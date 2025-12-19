
use std::{collections::{HashMap, HashSet}, sync::Arc};
use eframe::egui;

use crate::{CustomModal, common::{canvas::{self, ArrowDataPos, Highlight}, controller::{BucketNoT, ContainerGen2, Domain, ElementController, ElementControllerGen2, EventHandlingContext, EventHandlingStatus, GlobalDrawingContext, InputEvent, InsensitiveCommand, LabelProvider, PositionNoT, PropertiesStatus, Queryable, SelectionStatus, SnapManager, TargettingStatus, View}, entity::{Entity, EntityUuid}, eref::ERef, project_serde::{NHContextDeserialize, NHContextSerialize}, ufoption::UFOption, uuid::{ModelUuid, ViewUuid}}};

#[derive(Clone)]
pub struct ArrowData {
    pub line_type: canvas::LineType,
    pub arrowhead_type: canvas::ArrowheadType,
    pub multiplicity: Option<Arc<String>>,
    pub role: Option<Arc<String>>,
    pub reading: Option<Arc<String>>,
}

impl ArrowData {
    pub fn new_labelless(
        line_type: canvas::LineType,
        arrowhead_type: canvas::ArrowheadType,
    ) -> Self {
        Self { line_type, arrowhead_type, multiplicity: None, role: None, reading: None }
    }
}

pub fn init_points(
    mut source_uuid: impl Iterator<Item = ModelUuid>,
    target_uuid: ModelUuid,
    target_shape: canvas::NHShape,
    center_point: Option<(ViewUuid, egui::Pos2)>,
) -> (Vec<Vec<(ViewUuid, egui::Pos2)>>, Option<(ViewUuid, egui::Pos2)>, Vec<Vec<(ViewUuid, egui::Pos2)>>) {
    if source_uuid.any(|e| e == target_uuid) {
        let (min, quarter_size) = match target_shape {
            canvas::NHShape::Rect { inner } => (inner.min, inner.size() / 4.0),
            canvas::NHShape::Ellipse { position, bounds_radius }
            | canvas::NHShape::Rhombus { position, bounds_radius }
                => (position - bounds_radius, bounds_radius / 2.0),
        };

        (
            vec![vec![
                (ViewUuid::now_v7(), egui::Pos2::ZERO),
                (ViewUuid::now_v7(), min + egui::Vec2::new(quarter_size.x, -quarter_size.y)),
            ]],
            Some((ViewUuid::now_v7(), min - quarter_size)),
            vec![vec![
                (ViewUuid::now_v7(), egui::Pos2::ZERO),
                (ViewUuid::now_v7(), min + egui::Vec2::new(-quarter_size.x, quarter_size.y)),
            ]],
        )
    } else {
        (
            vec![vec![(ViewUuid::now_v7(), egui::Pos2::ZERO)]],
            center_point,
            vec![vec![(ViewUuid::now_v7(), egui::Pos2::ZERO)]],
        )
    }
}

pub trait MulticonnectionAdapter<DomainT: Domain>: serde::Serialize + NHContextSerialize + NHContextDeserialize + Send + Sync {
    fn model(&self) -> DomainT::CommonElementT;
    fn model_uuid(&self) -> Arc<ModelUuid>;

    fn background_color(&self) -> egui::Color32 {
        egui::Color32::WHITE
    }
    fn foreground_color(&self) -> egui::Color32 {
        egui::Color32::BLACK
    }
    fn midpoint_label(&self) -> Option<Arc<String>> { None }
    fn arrow_data(&self) -> &HashMap<(bool, ModelUuid), ArrowData>;
    fn source_uuids(&self) -> &[ModelUuid];
    fn target_uuids(&self) -> &[ModelUuid];
    fn flip_multiconnection(&mut self) -> Result<(), ()> {
        Err(())
    }
    fn insert_source(&mut self, _position: Option<PositionNoT>, _e: DomainT::CommonElementT) -> Result<PositionNoT, ()> {
        Err(())
    }
    fn remove_source(&mut self, _uuid: &ModelUuid) -> Option<PositionNoT> {
        None
    }
    fn insert_target(&mut self, _position: Option<PositionNoT>, _e: DomainT::CommonElementT) -> Result<PositionNoT, ()> {
        Err(())
    }
    fn remove_target(&mut self, _uuid: &ModelUuid) -> Option<PositionNoT> {
        None
    }

    fn show_properties(
        &mut self,
        q: &DomainT::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>
    ) -> PropertiesStatus<DomainT>;
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

pub const MULTICONNECTION_SOURCE_BUCKET: BucketNoT = 0;
pub const MULTICONNECTION_TARGET_BUCKET: BucketNoT = 1;
pub const MULTICONNECTION_VERTEX_BUCKET: BucketNoT = 2;

#[derive(Clone, Debug)]
pub struct VertexInformation {
    after: ViewUuid,
    id: ViewUuid,
    position: egui::Pos2,
}
#[derive(Clone, Copy, Debug)]
pub struct FlipMulticonnection {}


#[derive(Clone, serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct Ending<T> where T: serde::Serialize + NHContextSerialize + NHContextDeserialize + Clone {
    #[nh_context_serde(entity)]
    element: T,
    points: Vec<(ViewUuid, egui::Pos2)>,
}

impl<T> Ending<T> where T: serde::Serialize + NHContextSerialize + NHContextDeserialize + Clone {
    pub fn new(e: T) -> Self {
        Self::new_p(e, vec![(ViewUuid::now_v7(), egui::Pos2::ZERO)])
    }

    pub fn new_p(e: T, p: Vec<(ViewUuid, egui::Pos2)>,) -> Self {
        Self { element: e, points: p, }
    }
}

#[derive(nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
#[nh_context_serde(is_entity)]
pub struct MulticonnectionView<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> {
    uuid: Arc<ViewUuid>,
    #[nh_context_serde(entity)]
    adapter: AdapterT,

    #[nh_context_serde(entity)]
    sources: Vec<Ending<DomainT::CommonElementViewT>>,
    #[nh_context_serde(entity)]
    targets: Vec<Ending<DomainT::CommonElementViewT>>,

    #[nh_context_serde(skip_and_default)]
    dragged_node: Option<(ViewUuid, egui::Pos2)>,
    #[nh_context_serde(skip_and_default)]
    highlight: canvas::Highlight,
    #[nh_context_serde(skip_and_default)]
    selected_vertices: HashSet<ViewUuid>,
    center_point: UFOption<(ViewUuid, egui::Pos2)>,
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
        mut adapter: AdapterT,
        sources: Vec<Ending<DomainT::CommonElementViewT>>,
        targets: Vec<Ending<DomainT::CommonElementViewT>>,
        center_point: Option<(ViewUuid, egui::Pos2)>,
    ) -> ERef<Self> {
        let mut point_to_origin = HashMap::new();
        for (idx, e) in sources.iter().enumerate() {
            for p in &e.points {
                point_to_origin.insert(p.0, (false, idx));
            }
        }
        for (idx, e) in targets.iter().enumerate() {
            for p in &e.points {
                point_to_origin.insert(p.0, (true, idx));
            }
        }
        adapter.refresh_buffers();

        ERef::new(
            Self {
                uuid,
                adapter,
                sources,
                targets,
                dragged_node: None,
                highlight: canvas::Highlight::NONE,
                selected_vertices: HashSet::new(),

                center_point: center_point.into(),
                point_to_origin,
            }
        )
    }

    const VERTEX_RADIUS: f32 = 5.0;
    fn all_vertices(&self) -> impl Iterator<Item = &(ViewUuid, egui::Pos2)> {
        self.center_point.as_ref().into_iter()
            .chain(self.sources.iter().flat_map(|e| e.points.iter()))
            .chain(self.targets.iter().flat_map(|e| e.points.iter()))
    }
}

impl<DomainT: Domain, AdapterT: MulticonnectionAdapter<DomainT>> Entity for MulticonnectionView<DomainT, AdapterT> {
    fn tagged_uuid(&self) -> EntityUuid {
        (*self.uuid).into()
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
    fn bounding_box(&self) -> egui::Rect {
        let mut r = egui::Rect::NOTHING;

        for e in self.all_vertices() {
            let p = e.1;
            if r == egui::Rect::NOTHING {
                r = egui::Rect::from_min_max(p, p);
            }
            if !r.contains(p) {
                if p.x < r.min.x {
                    r.min.x = p.x;
                } else if p.x > r.max.x {
                    r.max.x = p.x;
                }
                if p.y < r.min.y {
                    r.min.y = p.y;
                } else if p.y > r.max.y {
                    r.max.y = p.y;
                }
            }
        }

        r
    }

    fn position(&self) -> egui::Pos2 {
        match &self.center_point {
            UFOption::Some(point) => point.1,
            UFOption::None => (self.sources[0].points[0].1 + self.targets[0].points[0].1.to_vec2()) / 2.0,
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
        gdc: &GlobalDrawingContext,
        q: &DomainT::QueryableT<'_>,
        ui: &mut egui::Ui,
        commands: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> PropertiesStatus<DomainT> {
        if !self.highlight.selected {
            return PropertiesStatus::NotShown;
        }

        fn display_endings_info<'a, DomainT: Domain>(
            q: &'a DomainT::QueryableT<'_>,
            lp: &'a LabelProvider,
            ui: &mut egui::Ui,
            commands: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
            models: &'a [ModelUuid],
            views: &'a [Ending<DomainT::CommonElementViewT>],
            self_uuid: ViewUuid,
            b: BucketNoT,
        ) {

            for model_uuid in models {
                let e = views.iter().find(|e| *e.element.model_uuid() == *model_uuid);

                ui.horizontal(|ui| {
                    ui.label(&*lp.get(model_uuid));
                    if let Some(e) = e {
                        if ui.add_enabled(views.len() > 1, egui::Button::new("Remove from view")).clicked() {
                            commands.push(InsensitiveCommand::RemoveDependency(self_uuid, b, *e.element.uuid(), false).into());
                        }
                        if ui.add_enabled(models.len() > 1, egui::Button::new("Remove from model")).clicked() {
                            commands.push(InsensitiveCommand::RemoveDependency(self_uuid, b, *e.element.uuid(), true).into());
                        }
                    } else {
                        if ui.button("Add to view").clicked() {
                            if let Some(v) = q.get_view(model_uuid) {
                                commands.push(InsensitiveCommand::AddDependency(self_uuid, b, None, v.into(), false).into());
                            }
                        }
                        if ui.add_enabled(models.len() > 1, egui::Button::new("Remove from model")).clicked() {

                        }
                    }
                });
            }
        }
        ui.label("Sources:");
        display_endings_info::<DomainT>(q, &gdc.model_labels, ui, commands, self.adapter.source_uuids(), &self.sources, *self.uuid, 0);
        ui.label("Targets:");
        display_endings_info::<DomainT>(q, &gdc.model_labels, ui, commands, self.adapter.target_uuids(), &self.targets, *self.uuid, 1);

        return self.adapter.show_properties(q, ui, commands);
    }

    fn draw_in(
        &mut self,
        _: &DomainT::QueryableT<'_>,
        _context: &GlobalDrawingContext,
        canvas: &mut dyn canvas::NHCanvas,
        _tool: &Option<(egui::Pos2, &DomainT::ToolT)>,
    ) -> TargettingStatus {
        let center_point = if let UFOption::Some(center_point) = &self.center_point {
            center_point.1
        } else {
            self.sources[0].element.min_shape().nice_midpoint(&self.targets[0].element.min_shape())
        };

        for e in self.sources.iter_mut().chain(self.targets.iter_mut()) {
            let shape = e.element.min_shape();
            let next_point = e.points.iter().skip(1).next().map(|p| p.1).unwrap_or(center_point);
            let intersect = shape.orthogonal_intersect(next_point)
                    .unwrap_or_else(|| shape.center_intersect(next_point));
            e.points[0].1 = intersect;
        }

        //canvas.draw_ellipse(source_next_point, egui::Vec2::splat(5.0), egui::Color32::RED, canvas::Stroke::new_solid(1.0, egui::Color32::RED), canvas::Highlight::NONE);
        //canvas.draw_ellipse(dest_next_point, egui::Vec2::splat(5.0), egui::Color32::GREEN, canvas::Stroke::new_solid(1.0, egui::Color32::GREEN), canvas::Highlight::NONE);

        //canvas.draw_ellipse((self.source_points[0][0].1 + self.dest_points[0][0].1.to_vec2()) / 2.0, egui::Vec2::splat(5.0), egui::Color32::BROWN, canvas::Stroke::new_solid(1.0, egui::Color32::BROWN), canvas::Highlight::NONE);

        let ad = self.adapter.arrow_data();
        let midpoint_label = self.adapter.midpoint_label();

        canvas.draw_multiconnection(
            &self.selected_vertices,
            self.sources.iter().map(|e| {
                let d = ad.get(&(false, *e.element.model_uuid())).unwrap();
                ArrowDataPos {
                    points: &e.points,
                    stroke: crate::common::canvas::Stroke {
                        width: 1.0,
                        color: self.adapter.foreground_color(),
                        line_type: d.line_type,
                    },
                    arrowhead_type: d.arrowhead_type,
                }
            }).collect(),
            self.targets.iter().map(|e| {
                let d = ad.get(&(true, *e.element.model_uuid())).unwrap();
                ArrowDataPos {
                    points: &e.points,
                    stroke: crate::common::canvas::Stroke {
                        width: 1.0,
                        color: self.adapter.foreground_color(),
                        line_type: d.line_type,
                    },
                    arrowhead_type: d.arrowhead_type,
                }
            }).collect(),
            match &self.center_point {
                UFOption::Some(point) => *point,
                UFOption::None => (
                    ViewUuid::nil(),
                    (self.sources[0].points[0].1 + self.targets[0].points[0].1.to_vec2()) / 2.0,
                ),
            },
            midpoint_label.as_ref().map(|e| e.as_str()),
            self.highlight,
        );

        fn draw_arrow_data(canvas: &mut dyn canvas::NHCanvas, shape: canvas::NHShape, shape_intersect: egui::Pos2, next_point: egui::Pos2, data: &ArrowData) {
            fn draw_small_labels(canvas: &mut dyn canvas::NHCanvas, bounds: canvas::NHShape, pos: egui::Pos2, labels: [Option<&str>; 2]) {
                let mut m = |e| canvas.measure_text(pos, egui::Align2::CENTER_CENTER, e, canvas::CLASS_TOP_FONT_SIZE).size();
                let sizes = [
                    labels[0].map(&mut m).unwrap_or(egui::Vec2::ZERO),
                    labels[1].map(&mut m).unwrap_or(egui::Vec2::ZERO),
                ];
                for p in bounds.place_labels(pos, sizes, 10.0).iter().zip(labels.iter()) {
                    if let Some(l) = p.1 {
                        canvas.draw_text(
                            *p.0,
                            egui::Align2::CENTER_CENTER,
                            *l,
                            canvas::CLASS_TOP_FONT_SIZE,
                            egui::Color32::BLACK,
                        );
                    }
                }
            }
            draw_small_labels(canvas, shape, shape_intersect, [data.multiplicity.as_ref().map(|e| e.as_str()), data.role.as_ref().map(|e| e.as_str())]);

            fn draw_reading(canvas: &mut dyn canvas::NHCanvas, intersect: egui::Pos2, next: egui::Pos2, reading_text: &str) {
                const PADDING: f32 = 10.0;
                const TRIANGLE_LONGEST_SIDE: f32 = 10.0;
                const TRIANGLE_PERPENDICULAR: f32 = 7.0;
                let size = canvas.measure_text(intersect, egui::Align2::CENTER_CENTER, reading_text, canvas::CLASS_TOP_FONT_SIZE).size();
                let mid = (intersect + next.to_vec2()) / 2.0;
                let (dx, dy) = (next.x - intersect.x, next.y - intersect.y);
                let angle = f32::atan2(dx, dy);
                let pos = mid + egui::Vec2::new(
                    f32::cos(angle) * (size.x / 2.0 + PADDING),
                    -f32::sin(angle) * (size.y / 2.0 + PADDING),
                );
                canvas.draw_text(
                    pos,
                    egui::Align2::CENTER_CENTER,
                    reading_text,
                    canvas::CLASS_TOP_FONT_SIZE,
                    egui::Color32::BLACK,
                );

                let points = if dx.abs() > dy.abs() {
                    let sign = if intersect.x < next.x {
                        -1.0 // "◀",
                    } else {
                        1.0 // "▶"
                    };
                    vec![
                        egui::Pos2::new(pos.x + sign * size.x / 2.0 + sign * 2.0, pos.y - TRIANGLE_LONGEST_SIDE / 2.0),
                        egui::Pos2::new(pos.x + sign * size.x / 2.0 + sign * (TRIANGLE_PERPENDICULAR + 2.0), pos.y),
                        egui::Pos2::new(pos.x + sign * size.x / 2.0 + sign * 2.0, pos.y + TRIANGLE_LONGEST_SIDE / 2.0),
                    ]
                } else {
                    let sign = if intersect.y < next.y {
                        -1.0 // "⏶"
                    } else {
                        1.0 // "⏷"
                    };
                    vec![
                        egui::Pos2::new(pos.x - TRIANGLE_LONGEST_SIDE / 2.0, pos.y + sign * size.y / 2.0),
                        egui::Pos2::new(pos.x, pos.y + sign * size.y / 2.0 + sign * TRIANGLE_PERPENDICULAR),
                        egui::Pos2::new(pos.x + TRIANGLE_LONGEST_SIDE / 2.0, pos.y + sign * size.y / 2.0),
                    ]
                };
                canvas.draw_polygon(
                    points,
                    egui::Color32::BLACK,
                    canvas::Stroke::NONE,
                    canvas::Highlight::NONE,
                );
            }
            if let Some(reading) = &data.reading {
                draw_reading(canvas, shape_intersect, next_point, &reading);
            }
        }
        for (target, e) in self.sources.iter().map(|e| (false, e)).chain(self.targets.iter().map(|e| (true, e))) {
            let Some(data) = ad.get(&(target, *e.element.model_uuid())) else {
                continue;
            };
            draw_arrow_data(canvas, e.element.min_shape(), e.points[0].1, e.points.get(1).map(|e| e.1).unwrap_or_else(|| self.position()), data);
        }

        TargettingStatus::NotDrawn
    }

    fn collect_allignment(&mut self, am: &mut SnapManager) {
        for p in self.center_point.as_ref().into_iter()
            .chain(self.sources.iter().flat_map(|e| e.points.iter().skip(1)))
            .chain(self.targets.iter().flat_map(|e| e.points.iter().skip(1)))
        {
            am.add_shape(*self.uuid, canvas::NHShape::Rect { inner: egui::Rect::from_min_size(p.1, egui::Vec2::ZERO) });
        }
    }
    fn handle_event(
        &mut self,
        event: InputEvent,
        ehc: &EventHandlingContext,
        q: &DomainT::QueryableT<'_>,
        _tool: &mut Option<DomainT::ToolT>,
        _element_setup_modal: &mut Option<Box<dyn CustomModal>>,
        commands: &mut Vec<InsensitiveCommand<DomainT::AddCommandElementT, DomainT::PropChangeT>>,
    ) -> EventHandlingStatus {
        let segment_distance_threshold = 3.0 / ehc.ui_scale;
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
                    UFOption::None if is_over(pos, self.position()) => {
                        self.dragged_node = Some((ViewUuid::now_v7(), pos));
                        commands.push(InsensitiveCommand::AddDependency(
                            *self.uuid,
                            3,
                            None,
                            VertexInformation {
                                after: ViewUuid::nil(),
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
                        for e in &mut self.$v {
                            // Iterates over 2-windows
                            let mut iter = e
                            .points
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
                                    self.dragged_node = Some((ViewUuid::now_v7(), pos));
                                    commands.push(InsensitiveCommand::AddDependency(
                                        *self.uuid,
                                        MULTICONNECTION_VERTEX_BUCKET,
                                        None,
                                        VertexInformation {
                                            after: u.0,
                                            id: self.dragged_node.unwrap().0,
                                            position: pos,
                                        }
                                        .into(),
                                        false,
                                    ).into());

                                    return EventHandlingStatus::HandledByContainer;
                                }
                            }
                        }
                    };
                }
                check_midpoints!(sources);
                check_midpoints!(targets);

                // Check whether over a joint, if so drag it
                macro_rules! check_joints {
                    ($v:ident) => {
                        for e in &mut self.$v {
                            let stop_idx = e.points.len();
                            for joint in &mut e.points[1..stop_idx] {
                                if is_over(pos, joint.1) {
                                    self.dragged_node = Some((joint.0, pos));

                                    return EventHandlingStatus::HandledByContainer;
                                }
                            }
                        }
                    };
                }
                check_joints!(sources);
                check_joints!(targets);

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
                            commands.push(InsensitiveCommand::HighlightAll(false, Highlight::SELECTED).into());
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*$uuid).collect(),
                                true,
                                Highlight::SELECTED,
                            ).into());
                        } else {
                            commands.push(InsensitiveCommand::HighlightSpecific(
                                std::iter::once(*$uuid).collect(),
                                !self.selected_vertices.contains($uuid),
                                Highlight::SELECTED,
                            ).into());
                        }
                        return EventHandlingStatus::HandledByContainer;
                    };
                }

                if let Some((uuid, _)) = self.center_point.as_ref().filter(|e| is_over(pos, e.1)) {
                    handle_vertex_click!(uuid);
                }

                macro_rules! check_joints_click {
                    ($pos:expr, $v:ident) => {
                        for e in &self.$v {
                            let stop_idx = e.points.len();
                            for joint in &e.points[1..stop_idx] {
                                if is_over($pos, joint.1) {
                                    handle_vertex_click!(&joint.0);
                                }
                            }
                        }
                    };
                }

                check_joints_click!(pos, sources);
                check_joints_click!(pos, targets);

                // Check segments on paths
                macro_rules! check_path_segments {
                    ($v:ident) => {
                        let p = self.position();
                        for e in &self.$v {
                            // Iterates over 2-windows
                            let mut iter = e.points.iter().map(|e| e.1).chain(std::iter::once(p)).peekable();
                            while let Some(u) = iter.next() {
                                let v = if let Some(v) = iter.peek() {
                                    *v
                                } else {
                                    break;
                                };

                                if dist_to_line_segment(pos, u, v) <= segment_distance_threshold {
                                    return EventHandlingStatus::HandledByElement;
                                }
                            }
                        }
                    };
                }
                check_path_segments!(sources);
                check_path_segments!(targets);

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
                    commands.push(InsensitiveCommand::MoveSpecificElements(q.selected_views(), coerced_delta));
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
                    .chain($self.sources.iter_mut().map(|e| e.points.iter_mut()).flatten())
                    .chain($self.targets.iter_mut().map(|e| e.points.iter_mut()).flatten())
            };
        }
        match command {
            InsensitiveCommand::HighlightAll(set, h) => {
                self.highlight = self.highlight.combine(*set, *h);
                if h.selected {
                    match set {
                        false => self.selected_vertices.clear(),
                        true => {
                            for p in all_pts_mut!(self) {
                                self.selected_vertices.insert(p.0);
                            }
                        }
                    }
                }
            }
            InsensitiveCommand::HighlightSpecific(uuids, set, h) => {
                if uuids.contains(&*self.uuid) {
                    self.highlight = self.highlight.combine(*set, *h);
                }
                if h.selected {
                    match set {
                        false => self.selected_vertices.retain(|e| !uuids.contains(e)),
                        true => {
                            for p in all_pts_mut!(self).filter(|e| uuids.contains(&e.0)) {
                                self.selected_vertices.insert(p.0);
                            }
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
                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                        self_uuid,
                        MULTICONNECTION_VERTEX_BUCKET,
                        None,
                        DomainT::AddCommandElementT::from(VertexInformation {
                            after: ViewUuid::nil(),
                            id: center_point.0,
                            position: center_point.1,
                        }),
                        false,
                    ));

                    // Move any last point to the center
                    self.center_point = 'a: {
                        if let Some(e) = self.sources.iter_mut().filter(|p| p.points.len() > 1).next() {
                            break 'a e.points.pop().into();
                        }
                        if let Some(e) = self.targets.iter_mut().filter(|p| p.points.len() > 1).next() {
                            break 'a e.points.pop().into();
                        }
                        None.into()
                    };
                }

                macro_rules! delete_vertices {
                    ($self:ident, $v:ident) => {
                        for e in $self.$v.iter_mut() {
                            // 2-windows over vertices
                            let mut iter = e.points.iter().peekable();
                            while let Some(a) = iter.next() {
                                let Some(b) = iter.peek() else {
                                    break;
                                };
                                if uuids.contains(&b.0) {
                                    undo_accumulator.push(InsensitiveCommand::AddDependency(
                                        self_uuid,
                                        MULTICONNECTION_VERTEX_BUCKET,
                                        None,
                                        DomainT::AddCommandElementT::from(VertexInformation {
                                            after: a.0,
                                            id: b.0,
                                            position: b.1,
                                        }),
                                        false,
                                    ));
                                }
                            }

                            e.points.retain(|e| !uuids.contains(&e.0));
                        }
                    };
                }
                delete_vertices!(self, sources);
                delete_vertices!(self, targets);

                // Handle dependencies being deleted
                let mut rtin = |e: &Ending<DomainT::CommonElementViewT>| if uuids.contains(&e.element.uuid()) {
                    undo_accumulator.push(InsensitiveCommand::AddDependency(self_uuid, 0, None, e.element.clone().into(), false));
                    false
                } else { true };
                self.sources.retain(&mut rtin);
                self.targets.retain(&mut rtin);
            }
            InsensitiveCommand::AddDependency(target, b, pos, element, into_model) => {
                if *target == *self.uuid {
                    // source/target
                    if let Ok(e) = TryInto::<DomainT::CommonElementViewT>::try_into(element.clone()) {
                        if *b == MULTICONNECTION_SOURCE_BUCKET
                            && (!into_model || self.adapter.insert_source(*pos, e.model()).is_ok()) {
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(*self.uuid, *b, *e.uuid(), *into_model));

                            self.sources.push(Ending {
                                element: e,
                                points: vec![(ViewUuid::now_v7(), egui::Pos2::ZERO)],
                            });

                            affected_models.insert(*self.adapter.model_uuid());
                        } else if *b == MULTICONNECTION_TARGET_BUCKET
                            && (!into_model || self.adapter.insert_target(*pos, e.model()).is_ok()) {
                            undo_accumulator.push(InsensitiveCommand::RemoveDependency(*self.uuid, *b, *e.uuid(), *into_model));

                            self.targets.push(Ending {
                                element: e,
                                points: vec![(ViewUuid::now_v7(), egui::Pos2::ZERO)],
                            });

                            affected_models.insert(*self.adapter.model_uuid());
                        }
                    }
                    // vertex
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
                                    self.sources[o.1].points.push(self.center_point.unwrap());
                                } else {
                                    self.targets[o.1].points.push(self.center_point.unwrap());
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
                                    for (idx1, e) in $self.$v.iter_mut().enumerate() {
                                        for (idx2, p) in e.points.iter().enumerate() {
                                            if p.0 == after {
                                                $self.point_to_origin.insert(id, ($b, idx1));
                                                e.points.insert(idx2 + 1, (id, position));
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
                            insert_vertex!(self, sources, false);
                            insert_vertex!(self, targets, true);
                        }
                    }
                }
            }
            InsensitiveCommand::RemoveDependency(uuid, b, duuid, including_model) => {
                if *uuid == *self.uuid {
                    if *b == MULTICONNECTION_SOURCE_BUCKET && self.sources.len() > 1 {
                        self.sources.retain(|e| if *duuid == *e.element.uuid() {
                            let pos = if !including_model {
                                None
                            } else if let Some(pos) = self.adapter.remove_source(&*e.element.model_uuid()) {
                                Some(pos)
                            } else {
                                return true;
                            };

                            undo_accumulator.push(InsensitiveCommand::AddDependency(*self.uuid, *b, pos, e.element.clone().into(), *including_model));
                            false
                        } else { true });

                        affected_models.insert(*self.adapter.model_uuid());
                    } else if *b == MULTICONNECTION_TARGET_BUCKET && self.targets.len() > 1 {
                        self.targets.retain(|e| if *duuid == *e.element.uuid() {
                            let pos = if !including_model {
                                None
                            } else if let Some(pos) = self.adapter.remove_target(&*e.element.model_uuid()) {
                                Some(pos)
                            } else {
                                return true;
                            };

                            undo_accumulator.push(InsensitiveCommand::AddDependency(*self.uuid, *b, pos, e.element.clone().into(), *including_model));
                            false
                        } else { true });

                        affected_models.insert(*self.adapter.model_uuid());
                    }
                }
            }
            InsensitiveCommand::PropertyChange(uuids, property) => {
                if uuids.contains(&*self.uuid) {
                    if let Ok(FlipMulticonnection {}) = property.try_into()
                        && self.adapter.flip_multiconnection().is_ok() {
                        undo_accumulator.push(InsensitiveCommand::PropertyChange(
                            std::iter::once(*self.uuid).collect(),
                            property.clone(),
                        ));
                        std::mem::swap(&mut self.sources, &mut self.targets);
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
        _flattened_views: &mut HashMap<ViewUuid, DomainT::CommonElementViewT>,
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

        let mut f = |e: &Ending<DomainT::CommonElementViewT>| {
            Ending::new_p(
                e.element.clone(),
                e.points.iter().map(|e| (ViewUuid::now_v7(), e.1)).collect(),
            )
        };
        let sources = self.sources.iter().map(&mut f).collect();
        let targets = self.targets.iter().map(&mut f).collect();
        let center_point = if let UFOption::Some(e) = &self.center_point {
            UFOption::Some((ViewUuid::now_v7(), e.1))
        } else {
            UFOption::None
        };

        let cloneish = ERef::new(Self {
            uuid: view_uuid.into(),
            adapter: self.adapter.deep_copy_init(model_uuid, m),
            sources,
            targets,
            dragged_node: None,
            highlight: self.highlight,
            selected_vertices: self.selected_vertices.clone(),
            center_point,

            // There is no need to keep it (undo would destroy the whole clone first)
            point_to_origin: HashMap::new(),
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

        for e in self.sources.iter_mut() {
            if let Some(s) = c.get(&e.element.uuid()) {
                e.element = s.clone();
            }
        }
        for e in self.targets.iter_mut() {
            if let Some(t) = c.get(&e.element.uuid()) {
                e.element = t.clone();
            }
        }
    }
}
