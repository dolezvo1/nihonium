use eframe::egui;

use std::collections::HashSet;
use std::io::Write;

use super::uuid::ViewUuid;

// rect intersection between segment from p to the center of rect
// based on https://stackoverflow.com/a/31254199 by TWiStErRob
fn segment_rect_point(p: egui::Pos2, rect: &egui::Rect) -> Option<egui::Pos2> {
    if rect.contains(p) {
        return None;
    }
    let m = (rect.center().y - p.y) / (rect.center().x - p.x);
    if p.x <= rect.center().x {
        // check "left" side
        let min_xy = m * (rect.left() - p.x) + p.y;
        if rect.top() <= min_xy && min_xy <= rect.bottom() {
            return Some(egui::Pos2 {
                x: rect.left(),
                y: min_xy,
            });
        }
    }

    if p.x >= rect.center().x {
        // check "right" side
        let max_xy = m * (rect.right() - p.x) + p.y;
        if rect.top() <= max_xy && max_xy <= rect.bottom() {
            return Some(egui::Pos2 {
                x: rect.right(),
                y: max_xy,
            });
        }
    }

    if p.y <= rect.center().y {
        // check "top" side
        let min_yx = (rect.top() - p.y) / m + p.x;
        if rect.left() <= min_yx && min_yx <= rect.right() {
            return Some(egui::Pos2 {
                x: min_yx,
                y: rect.top(),
            });
        }
    }

    if p.y >= rect.center().y {
        // check "bottom" side
        let max_yx = (rect.bottom() - p.y) / m + p.x;
        if rect.left() <= max_yx && max_yx <= rect.right() {
            return Some(egui::Pos2 {
                x: max_yx,
                y: rect.bottom(),
            });
        }
    }

    return None;
}

// based on https://stackoverflow.com/a/25704033 by Danial Esmaeili
fn segment_ellipse_point(
    p: egui::Pos2,
    (center, radius): (&egui::Pos2, &egui::Vec2),
) -> Option<egui::Pos2> {
    let theta = (center.y - p.y).atan2(center.x - p.x);
    let r = ((center.y - p.y).powf(2.0) + (center.x - p.x).powf(2.0)).sqrt()
        - ((radius.x * radius.y)
            / ((radius.y * theta.cos()).powf(2.0) + (radius.x * theta.sin()).powf(2.0)).sqrt());
    return Some(egui::Pos2::new(
        p.x + r * theta.cos(),
        p.y + r * theta.sin(),
    ));
}

fn ellipse_orthogonal_intersection(
    point: egui::Pos2,
    (center, radius): (&egui::Pos2, &egui::Vec2),
) -> Option<egui::Pos2> {
    fn slv_px(px: f32, py: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> f32 {
        let sqrt = ((1.0 - (py - cy).powf(2.0) / ry.powf(2.0)) * rx.powf(2.0)).sqrt();
        px.clamp(cx - sqrt, cx + sqrt)
    }

    if center.x - radius.x < point.x && point.x < center.x + radius.x {
        Some(egui::Pos2::new(
            point.x,
            slv_px(point.y, point.x, center.y, center.x, radius.y, radius.x),
        ))
    } else if center.y - radius.y < point.y && point.y < center.y + radius.y {
        Some(egui::Pos2::new(
            slv_px(point.x, point.y, center.x, center.y, radius.x, radius.y),
            point.y,
        ))
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub enum NHShape {
    Rect {
        inner: egui::Rect,
    },
    Ellipse {
        position: egui::Pos2,
        bounds_radius: egui::Vec2,
    },
}

impl NHShape {
    pub const ELLIPSE_ZERO: Self = Self::Ellipse {
        position: egui::Pos2::ZERO,
        bounds_radius: egui::Vec2::ZERO,
    };

    pub fn translate(&self, delta: egui::Vec2) -> Self {
        match self {
            NHShape::Rect { inner } => NHShape::Rect {
                inner: inner.translate(delta),
            },
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => NHShape::Ellipse {
                position: *position + delta,
                bounds_radius: *bounds_radius,
            },
        }
    }

    pub fn center(&self) -> egui::Pos2 {
        match &self {
            NHShape::Rect { inner } => inner.center(),
            NHShape::Ellipse { position, .. } => *position,
        }
    }

    pub fn center_intersect(&self, point: egui::Pos2) -> egui::Pos2 {
        match &self {
            NHShape::Rect { inner } => segment_rect_point(point, inner).unwrap_or(point),
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => segment_ellipse_point(point, (position, bounds_radius)).unwrap_or(point),
        }
    }
    /// returns None iff point is not orthogonally aligned
    pub fn orthogonal_intersect(&self, point: egui::Pos2) -> Option<egui::Pos2> {
        match &self {
            NHShape::Rect { inner } => match point {
                egui::Pos2 { x, y } if inner.left() < x && x < inner.right() => Some(egui::Pos2 {
                    x,
                    y: y.clamp(inner.top(), inner.bottom()),
                }),
                egui::Pos2 { x, y } if inner.top() < y && y < inner.bottom() => Some(egui::Pos2 {
                    x: x.clamp(inner.left(), inner.right()),
                    y,
                }),
                _ => None,
            },
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => ellipse_orthogonal_intersection(point, (position, bounds_radius)),
        }
    }
    pub fn bounding_box(&self) -> egui::Rect {
        match self {
            NHShape::Rect { inner } => *inner,
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => egui::Rect::from_center_size(*position, 2.0 * *bounds_radius),
        }
    }
    pub fn nice_midpoint(&self, other: &NHShape) -> egui::Pos2 {
        let (a, b) = (self.bounding_box(), other.bounding_box());

        if a.left() < b.right() && b.left() < a.right() {
            egui::Pos2::new(
                (a.left().clamp(b.left(), b.right()) + a.right().clamp(b.left(), b.right())) / 2.0,
                (a.bottom().min(b.bottom()) + a.top().max(b.top())) / 2.0,
            )
        } else if a.top() < b.bottom() && b.top() < a.bottom() {
            egui::Pos2::new(
                (a.right().min(b.right()) + a.left().max(b.left())) / 2.0,
                (a.top().clamp(b.top(), b.bottom()) + a.bottom().clamp(b.top(), b.bottom())) / 2.0,
            )
        } else {
            (self.center_intersect(other.center())
                + other.center_intersect(self.center()).to_vec2())
                / 2.0
        }
    }
    pub fn border_distance(&self, point: egui::Pos2) -> f32 {
        match &self {
            NHShape::Rect { inner } => {
                if inner.contains(point) {
                    (point.x - inner.left())
                        .abs()
                        .min((point.x - inner.right()).abs())
                        .min((point.y - inner.top()).abs())
                        .min((point.y - inner.bottom()).abs())
                } else {
                    let clamped_point = egui::Pos2::new(
                        point.x.min(inner.right()).max(inner.left()),
                        point.y.min(inner.bottom()).max(inner.top()),
                    );
                    point.distance(clamped_point)
                }
            }
            NHShape::Ellipse { .. } => {
                // TODO: This is actually hard to do.
                todo!()
            }
        }
    }
    pub fn contains(&self, point: egui::Pos2) -> bool {
        match &self {
            NHShape::Rect { inner } => inner.contains(point),
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => {
                let d = (point.x - position.x).powf(2.0) / (bounds_radius.x).powf(2.0)
                    + (point.y - position.y).powf(2.0) / (bounds_radius.y).powf(2.0);
                d <= 1.0
            }
        }
    }
    pub fn contained_within(&self, rect: egui::Rect) -> bool {
        match &self {
            NHShape::Rect { inner } => rect.contains_rect(*inner),
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => rect.contains_rect(egui::Rect::from_center_size(
                *position,
                2.0 * *bounds_radius,
            )),
        }
    }

    pub fn guidelines_anchors(&self) -> Vec<(egui::Pos2, egui::Align)> {
        match self {
            NHShape::Rect { inner } => vec![
                (inner.min, egui::Align::Min),
                (inner.center(), egui::Align::Center),
                (inner.max, egui::Align::Max),
            ],
            NHShape::Ellipse {
                position,
                bounds_radius,
            } => vec![
                (*position - *bounds_radius, egui::Align::Min),
                (*position, egui::Align::Center),
                (*position + *bounds_radius, egui::Align::Max),
            ],
        }
    }

    pub fn place_labels(&self, around: egui::Pos2, sizes: [egui::Vec2; 2]) -> [egui::Pos2; 2] {
        const PADDING: f32 = 5.0;
        let center = self.center();
        // TODO: doesn't actually work for both labels, but close enough
        let [x0, x1] = if around.x < center.x {
            [
                around.x - sizes[0].x / 2.0 - PADDING,
                around.x - sizes[1].x / 2.0 - PADDING,
            ]
        } else {
            [
                around.x + sizes[0].x / 2.0 + PADDING,
                around.x + sizes[1].x / 2.0 + PADDING,
            ]
        };
        let [y0, y1] = if around.y < center.y {
            [
                around.y - sizes[0].y / 2.0 - PADDING,
                around.y - sizes[1].y / 2.0 - PADDING,
            ]
        } else {
            [
                around.y + sizes[0].y / 2.0 + PADDING,
                around.y + sizes[1].y / 2.0 + PADDING,
            ]
        };
        [egui::Pos2::new(x0, y0), egui::Pos2::new(x1, y1)]
    }
}

// TODO: circle, embedded circle (ArchiMate)
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ArrowheadType {
    None,
    OpenTriangle,
    EmptyTriangle,
    FullTriangle,
    EmptyRhombus,
    FullRhombus,
}

fn atan2(a: egui::Pos2, b: egui::Pos2) -> f32 {
    (b.y - a.y).atan2(b.x - a.x)
}

const ARROWHEAD_SIDE_LENGTH: f32 = 15.0;
const ARROWHEAD_INNER_ANGLE: f32 = 35.0;

impl ArrowheadType {
    pub fn _name(&self) -> &str {
        match self {
            ArrowheadType::None => "None",
            ArrowheadType::OpenTriangle => "Open Triangle",
            ArrowheadType::EmptyTriangle => "Empty Triangle",
            ArrowheadType::FullTriangle => "Full Triangle",
            ArrowheadType::EmptyRhombus => "Empty Rhombus",
            ArrowheadType::FullRhombus => "Full Rhombus",
        }
    }

    // Get intersection of line between focal_point and other point
    // that is the furthest from the focal_point
    pub fn get_intersect(&self, focal_point: egui::Pos2, other: egui::Pos2) -> egui::Pos2 {
        match self {
            ArrowheadType::None | ArrowheadType::OpenTriangle => focal_point,
            ArrowheadType::EmptyTriangle
            | ArrowheadType::FullTriangle
            | ArrowheadType::EmptyRhombus
            | ArrowheadType::FullRhombus => {
                let outward_angle = atan2(focal_point, other);
                let [p1, p2] = [-ARROWHEAD_INNER_ANGLE, ARROWHEAD_INNER_ANGLE]
                    .map(|e| e * std::f32::consts::PI / 180.0 + outward_angle)
                    .map(|a| {
                        focal_point + egui::Vec2::new(a.cos(), a.sin()) * ARROWHEAD_SIDE_LENGTH
                    });
                let p3 = p2 + (p1 - focal_point);

                if *self == ArrowheadType::EmptyRhombus || *self == ArrowheadType::FullRhombus {
                    p3
                } else {
                    (p3 + focal_point.to_vec2()) / 2.0
                }
            }
        }
    }

    pub fn draw_in(
        &self,
        canvas: &mut (impl NHCanvas + ?Sized),
        focal_point: egui::Pos2,
        other: egui::Pos2,
        highlight: Highlight,
    ) {
        let outward_angle = atan2(focal_point, other);
        if matches!(self, ArrowheadType::OpenTriangle) {
            //println!("{:?}, {:?}, {:?}, {:?}", self, outward_angle, focal_point, other);
        }
        let [p1, p2] = [-ARROWHEAD_INNER_ANGLE, ARROWHEAD_INNER_ANGLE]
            .map(|e| e * std::f32::consts::PI / 180.0 + outward_angle)
            .map(|a| focal_point + egui::Vec2::new(a.cos(), a.sin()) * ARROWHEAD_SIDE_LENGTH);
        match self {
            ArrowheadType::None => {}
            ArrowheadType::OpenTriangle => {
                canvas.draw_line(
                    [focal_point, p1],
                    Stroke::new_solid(1.0, egui::Color32::BLACK),
                    highlight,
                );
                canvas.draw_line(
                    [focal_point, p2],
                    Stroke::new_solid(1.0, egui::Color32::BLACK),
                    highlight,
                );
            }
            ArrowheadType::EmptyTriangle | ArrowheadType::FullTriangle => {
                canvas.draw_polygon(
                    vec![focal_point, p1, p2],
                    if *self == ArrowheadType::EmptyTriangle {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::BLACK
                    },
                    Stroke::new_solid(1.0, egui::Color32::BLACK),
                    highlight,
                );
            }
            ArrowheadType::EmptyRhombus | ArrowheadType::FullRhombus => {
                let p3 = p2 + (p1 - focal_point);
                canvas.draw_polygon(
                    vec![focal_point, p1, p3, p2],
                    if *self == ArrowheadType::EmptyRhombus {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::BLACK
                    },
                    Stroke::new_solid(1.0, egui::Color32::BLACK),
                    highlight,
                );
            }
        }
    }
}

// TODO: dotted, double, squiggly
#[derive(Clone, Copy, PartialEq)]
pub enum LineType {
    Solid,
    Dashed,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: egui::Color32,
    pub line_type: LineType,
}

impl Stroke {
    pub const NONE: Self = Self {
        width: 0.0,
        color: egui::Color32::TRANSPARENT,
        line_type: LineType::Solid,
    };

    pub fn new_solid(width: f32, color: egui::Color32) -> Self {
        Self {
            width,
            color,
            line_type: LineType::Solid,
        }
    }

    pub fn new_dashed(width: f32, color: egui::Color32) -> Self {
        Self {
            width,
            color,
            line_type: LineType::Dashed,
        }
    }
}

impl From<Stroke> for egui::Stroke {
    fn from(value: Stroke) -> egui::Stroke {
        egui::Stroke {
            width: value.width,
            color: value.color,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Highlight {
    pub selected: bool, // "blue"
    pub valid: bool,    // "green"
    pub invalid: bool,  // "red"
    pub warning: bool,  // "yellow"
}

impl Highlight {
    pub const NONE: Self = Self {
        selected: false, // "blue"
        valid: false,    // "green"
        invalid: false,  // "red"
        warning: false,  // "yellow"
    };
    pub const SELECTED: Self = Self {
        selected: true, // "blue"
        valid: false,   // "green"
        invalid: false, // "red"
        warning: false, // "yellow"
    };
}

pub const CLASS_TOP_FONT_SIZE: f32 = 15.0;
pub const CLASS_MIDDLE_FONT_SIZE: f32 = 15.0;
pub const CLASS_BOTTOM_FONT_SIZE: f32 = 15.0;
pub const CLASS_ITEM_FONT_SIZE: f32 = 10.0;

pub trait NHCanvas {
    // These functions are must haves
    /// None if not interactive
    fn ui_scale(&self) -> Option<f32>;

    fn draw_line(&mut self, points: [egui::Pos2; 2], stroke: Stroke, highlight: Highlight);
    fn draw_rectangle(
        &mut self,
        rect: egui::Rect,
        corner_radius: egui::CornerRadius,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    );
    fn draw_ellipse(
        &mut self,
        position: egui::Pos2,
        radius: egui::Vec2,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    );
    fn draw_ellipse_proximity(
        &mut self,
        _position: egui::Pos2,
        _radius: egui::Vec2,
        _color: egui::Color32,
        _stroke: Stroke,
        _max_distance: f32,
        _highlight: Highlight,
    ) {
    }
    fn draw_polygon(
        &mut self,
        vertices: Vec<egui::Pos2>,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    );

    fn measure_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
    ) -> egui::Rect;
    fn draw_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
        text_color: egui::Color32,
    );

    fn draw_class(
        &mut self,
        position: egui::Pos2,
        top_label: Option<&str>,
        main_label: &str,
        bottom_label: Option<&str>,
        items: &[&[(&str, &str)]],
        fill: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) -> egui::Rect {
        // Measure phase
        let (offsets, global_offset, max_width, itemalign, category_separators, rect) = {
            let mut offsets = vec![0.0];
            let mut max_width: f32 = 0.0;
            let mut category_separators = vec![];
            let itemalign = items
                .iter()
                .flat_map(|c| c.iter())
                .map(|e| {
                    self.measure_text(
                        position,
                        egui::Align2::LEFT_CENTER,
                        e.0,
                        CLASS_ITEM_FONT_SIZE,
                    )
                    .width()
                })
                .fold(0.0 as f32, |a, b| a.max(b));

            if let Some(top_label) = top_label {
                let r = self.measure_text(
                    egui::Pos2::ZERO,
                    egui::Align2::CENTER_TOP,
                    &top_label,
                    CLASS_TOP_FONT_SIZE,
                );
                offsets.push(r.height());
                max_width = max_width.max(r.width());
            }

            {
                let r = self.measure_text(
                    egui::Pos2::ZERO,
                    egui::Align2::CENTER_TOP,
                    &main_label,
                    CLASS_MIDDLE_FONT_SIZE,
                );
                offsets.push(r.height());
                max_width = max_width.max(r.width());
            }

            if let Some(bottom_label) = bottom_label {
                let r = self.measure_text(
                    egui::Pos2::ZERO,
                    egui::Align2::CENTER_TOP,
                    &bottom_label,
                    CLASS_BOTTOM_FONT_SIZE,
                );
                offsets.push(r.height());
                max_width = max_width.max(r.width());
            }

            for category in items.iter().filter(|e| e.len() > 0) {
                category_separators.push(offsets.iter().sum::<f32>());

                for (_center, left) in *category {
                    let r = self.measure_text(
                        egui::Pos2::ZERO,
                        egui::Align2::LEFT_TOP,
                        left,
                        CLASS_ITEM_FONT_SIZE,
                    );
                    offsets.push(r.height());
                    max_width = max_width.max(itemalign + r.width());
                }
            }

            // Process, draw bounds
            offsets.iter_mut().fold(0.0, |acc, x| {
                *x += acc;
                *x
            });
            let global_offset = offsets.last().unwrap() / 2.0;
            let rect = egui::Rect::from_center_size(
                position,
                egui::Vec2::new(max_width + 4.0, 2.0 * global_offset),
            );
            self.draw_rectangle(rect, egui::CornerRadius::ZERO, fill, stroke.into(), highlight);

            (
                offsets,
                global_offset,
                max_width,
                itemalign,
                category_separators,
                rect,
            )
        };

        // Draw phase
        {
            let mut offset_counter = 0;

            if let Some(top_label) = top_label {
                self.draw_text(
                    position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                    egui::Align2::CENTER_TOP,
                    &top_label,
                    CLASS_TOP_FONT_SIZE,
                    egui::Color32::BLACK,
                );
                offset_counter += 1;
            }

            {
                self.draw_text(
                    position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                    egui::Align2::CENTER_TOP,
                    &main_label,
                    CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );
                offset_counter += 1;
            }

            if let Some(bottom_label) = bottom_label {
                self.draw_text(
                    position - egui::Vec2::new(0.0, global_offset - offsets[offset_counter]),
                    egui::Align2::CENTER_TOP,
                    &bottom_label,
                    CLASS_BOTTOM_FONT_SIZE,
                    egui::Color32::BLACK,
                );
                offset_counter += 1;
            }

            for (idx, category) in items.iter().filter(|e| e.len() > 0).enumerate() {
                if let Some(catline_offset) = category_separators.get(idx) {
                    self.draw_line(
                        [
                            egui::Pos2::new(
                                position.x - rect.width() / 2.0,
                                position.y - global_offset + catline_offset,
                            ),
                            egui::Pos2::new(
                                position.x + rect.width() / 2.0,
                                position.y - global_offset + catline_offset,
                            ),
                        ],
                        Stroke::new_solid(1.0, egui::Color32::BLACK),
                        highlight,
                    );
                }

                for (center, left) in *category {
                    self.draw_text(
                        egui::Pos2::new(
                            position.x - max_width / 2.0 + itemalign / 2.0,
                            position.y - global_offset + offsets[offset_counter],
                        ),
                        egui::Align2::CENTER_TOP,
                        center,
                        CLASS_ITEM_FONT_SIZE,
                        egui::Color32::BLACK,
                    );
                    self.draw_text(
                        egui::Pos2::new(
                            position.x - max_width / 2.0 + itemalign,
                            position.y - global_offset + offsets[offset_counter],
                        ),
                        egui::Align2::LEFT_TOP,
                        left,
                        CLASS_ITEM_FONT_SIZE,
                        egui::Color32::BLACK,
                    );
                    offset_counter += 1;
                }
            }
        }

        rect
    }

    // TODO: refactor to allow for line types (solid/dotted/dashed/double/squiggly)
    fn draw_multiconnection<'a>(
        &mut self,
        selected_vertices: &HashSet<ViewUuid>,
        sources: &[(
            ArrowheadType,
            Stroke,
            &Vec<(ViewUuid, egui::Pos2)>,
            Option<(egui::Pos2, &'a str)>,
        )],
        destinations: &[(
            ArrowheadType,
            Stroke,
            &Vec<(ViewUuid, egui::Pos2)>,
            Option<(egui::Pos2, &'a str)>,
        )],
        central_point: (ViewUuid, egui::Pos2),
        mid_label: Option<&str>,
        highlight: Highlight,
    ) {
        fn a<'a>(
            central_point: (ViewUuid, egui::Pos2),
            e: &'a (
                ArrowheadType,
                Stroke,
                &'a Vec<(ViewUuid, egui::Pos2)>,
                Option<(egui::Pos2, &'a str)>,
            ),
        ) -> (
            ArrowheadType,
            Stroke,
            egui::Pos2,
            impl Iterator<Item = (ViewUuid, egui::Pos2)> + 'a,
            Option<(egui::Pos2, &'a str)>,
        ) {
            let focal_point = e.2.first().unwrap();
            let path = std::iter::once((
                uuid::Uuid::nil().into(),
                e.0.get_intersect(focal_point.1, e.2.get(1).unwrap_or(&central_point).1),
            ))
            .chain(e.2.iter().skip(1).map(|e| *e))
            .chain(std::iter::once(central_point));
            (e.0, e.1, focal_point.1, path, e.3)
        }

        for (ah, ls, fp, iter, label) in sources
            .iter()
            .map(|e| a(central_point, e))
            .chain(destinations.iter().map(|e| a(central_point, e)))
        {
            let mut iter_peekable = iter.peekable();
            let mut first = true;

            while let Some(u) = iter_peekable.next() {
                let v = if let Some(v) = iter_peekable.peek() {
                    *v
                } else {
                    break;
                };
                let (u, v, v_uuid) = (u.1, v.1, v.0);

                if first {
                    ah.draw_in(self, fp, v, highlight);
                }

                self.draw_line([u, v], ls, highlight);

                const HANDLE_PROXIMITY: f32 = 20.0;

                if !central_point.0.is_nil() {
                    self.draw_ellipse_proximity(
                        (if first { fp } else { u } + v.to_vec2()) / 2.0,
                        egui::Vec2::new(1.0, 1.0),
                        egui::Color32::BLACK,
                        Stroke::new_solid(1.0, egui::Color32::BLACK),
                        HANDLE_PROXIMITY,
                        Highlight::NONE,
                    );
                }

                if selected_vertices.contains(&v_uuid) {
                    self.draw_ellipse(
                        v,
                        egui::Vec2::new(1.0, 1.0),
                        egui::Color32::BLACK,
                        Stroke::new_solid(1.0, egui::Color32::BLACK),
                        Highlight::SELECTED,
                    );
                } else {
                    self.draw_ellipse_proximity(
                        v,
                        egui::Vec2::new(1.0, 1.0),
                        egui::Color32::BLACK,
                        Stroke::new_solid(1.0, egui::Color32::BLACK),
                        HANDLE_PROXIMITY,
                        Highlight::NONE,
                    );
                }

                first = false;
            }

            if let Some(label) = label {
                self.draw_text(
                    label.0,
                    egui::Align2::CENTER_CENTER,
                    label.1,
                    CLASS_MIDDLE_FONT_SIZE,
                    egui::Color32::BLACK,
                );
            }
        }

        // TODO: Blur the line around center to make the mid label more readable?
        //       Alternatively labels could have an angle to fit it better.
        if let Some(mid_label) = mid_label {
            self.draw_text(
                central_point.1,
                egui::Align2::CENTER_CENTER,
                mid_label,
                CLASS_MIDDLE_FONT_SIZE,
                egui::Color32::BLACK,
            );
        }
    }
}

pub struct UiCanvas {
    is_interactive: bool,
    highlight_colors: [egui::Color32; 4],
    painter: egui::Painter,
    canvas: egui::Rect,
    camera_offset: egui::Pos2,
    camera_scale: f32,
    cursor: Option<egui::Pos2>,
}

impl UiCanvas {
    pub fn new(
        is_interactive: bool,
        painter: egui::Painter,
        canvas: egui::Rect,
        camera_offset: egui::Pos2,
        camera_scale: f32,
        cursor: Option<egui::Pos2>,
    ) -> Self {
        Self {
            is_interactive,
            highlight_colors: [
                egui::Color32::BLUE,
                egui::Color32::GREEN,
                egui::Color32::RED,
                egui::Color32::YELLOW,
            ],
            painter,
            canvas,
            camera_offset,
            camera_scale,
            cursor,
        }
    }

    pub fn clear(&self, color: egui::Color32) {
        self.painter
            .rect(self.canvas, egui::CornerRadius::ZERO, color, egui::Stroke::NONE, egui::StrokeKind::Middle);
    }

    pub fn draw_gridlines(
        &self,
        vertical: Option<(f32, egui::Color32)>,
        horizontal: Option<(f32, egui::Color32)>,
    ) {
        let canvas_size_scaled = (self.canvas.max - self.canvas.min) / self.camera_scale;

        if let Some((distance_x, color)) = vertical {
            for x in
                (0..((canvas_size_scaled.x / distance_x) as u32 + 2)).map(|e| distance_x * e as f32)
            {
                self.painter.vline(
                    self.canvas.min.x
                        + self.camera_offset.x % (distance_x * self.camera_scale)
                        + x * self.camera_scale,
                    egui::Rangef::new(self.canvas.min.y, self.canvas.max.y),
                    egui::Stroke::new(1.0, color),
                );
            }
        }
        if let Some((distance_y, color)) = horizontal {
            for y in
                (0..((canvas_size_scaled.y / distance_y) as u32 + 2)).map(|e| distance_y * e as f32)
            {
                self.painter.hline(
                    egui::Rangef::new(self.canvas.min.x, self.canvas.max.x),
                    self.canvas.min.y
                        + self.camera_offset.y % (distance_y * self.camera_scale)
                        + y * self.camera_scale,
                    egui::Stroke::new(1.0, color),
                );
            }
        }
    }

    pub fn sc_tr(&self, pos: egui::Pos2) -> egui::Pos2 {
        (pos * self.camera_scale) + self.canvas.min.to_vec2() + self.camera_offset.to_vec2()
    }
}

impl NHCanvas for UiCanvas {
    fn ui_scale(&self) -> Option<f32> {
        Some(self.camera_scale).filter(|_| self.is_interactive)
    }

    fn draw_line(&mut self, points: [egui::Pos2; 2], stroke: Stroke, highlight: Highlight) {
        let offset = self.canvas.min.to_vec2() + self.camera_offset.to_vec2();
        let (p1, p2) = (
            points[0] * self.camera_scale + offset,
            points[1] * self.camera_scale + offset,
        );

        if highlight.selected {
            self.painter.line_segment(
                [p1, p2],
                egui::Stroke::from(Stroke::new_solid(
                    stroke.width + 1.0,
                    self.highlight_colors[0],
                )),
            );
        }

        match stroke.line_type {
            LineType::Solid => {
                self.painter
                    .line_segment([p1, p2], egui::Stroke::from(stroke));
            }
            LineType::Dashed => {
                self.painter.add(eframe::epaint::Shape::dashed_line(
                    &[p1, p2],
                    egui::Stroke::from(stroke),
                    10.0,
                    10.0,
                ));
            }
        }
    }

    fn draw_rectangle(
        &mut self,
        rect: egui::Rect,
        corner_radius: egui::CornerRadius,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) {
        if highlight.selected {
            for (p1, p2) in [
                (
                    rect.left_top() + egui::Vec2::new(-1.0, -1.0),
                    rect.right_top() + egui::Vec2::new(1.0, -1.0),
                ),
                (
                    rect.left_bottom() + egui::Vec2::new(-1.0, 1.0),
                    rect.right_bottom() + egui::Vec2::new(1.0, 1.0),
                ),
                (
                    rect.right_top() + egui::Vec2::new(1.0, -1.0),
                    rect.right_bottom() + egui::Vec2::new(1.0, 1.0),
                ),
                (
                    rect.left_top() + egui::Vec2::new(-1.0, -1.0),
                    rect.left_bottom() + egui::Vec2::new(-1.0, 1.0),
                ),
            ] {
                self.draw_line(
                    [p1, p2],
                    Stroke::new_solid(stroke.width, self.highlight_colors[0]),
                    Highlight::NONE,
                );
            }
        }

        if color == egui::Color32::TRANSPARENT && stroke.line_type != LineType::Solid {
            for (p1, p2) in [
                (rect.left_top(), rect.right_top()),
                (rect.left_bottom(), rect.right_bottom()),
                (rect.right_top(), rect.right_bottom()),
                (rect.left_top(), rect.left_bottom()),
            ] {
                self.draw_line([p1, p2], stroke, highlight);
            }
        } else {
            self.painter.rect(
                (rect * self.camera_scale)
                    .translate(self.canvas.min.to_vec2() + self.camera_offset.to_vec2())
                    .intersect(self.canvas),
                corner_radius,
                color,
                // TODO: shouldn't stroke be recalculated?
                egui::Stroke::from(stroke),
                egui::StrokeKind::Middle,
            );
        }
    }

    fn draw_ellipse(
        &mut self,
        position: egui::Pos2,
        radius: egui::Vec2,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) {
        if highlight.selected {
            self.painter.add(eframe::epaint::EllipseShape {
                center: self.sc_tr(position),
                radius: (radius + egui::Vec2::new(1.0, 1.0)) * self.camera_scale,
                fill: egui::Color32::TRANSPARENT,
                stroke: egui::Stroke::from(Stroke::new_solid(
                    stroke.width,
                    self.highlight_colors[0],
                )),
            });
        }

        self.painter.add(eframe::epaint::EllipseShape {
            center: self.sc_tr(position),
            radius: radius * self.camera_scale,
            fill: color,
            stroke: egui::Stroke::from(stroke),
        });
    }

    fn draw_ellipse_proximity(
        &mut self,
        position: egui::Pos2,
        radius: egui::Vec2,
        color: egui::Color32,
        stroke: Stroke,
        max_distance: f32,
        highlight: Highlight,
    ) {
        if self
            .cursor
            .filter(|e| e.distance(position) <= max_distance)
            .is_some()
        {
            self.draw_ellipse(position, radius, color, stroke, highlight);
        }
    }

    fn draw_polygon(
        &mut self,
        vertices: Vec<egui::Pos2>,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) {
        let vertices = vertices.into_iter().map(|p| self.sc_tr(p)).collect();
        self.painter.add(egui::Shape::convex_polygon(
            vertices,
            color,
            egui::Stroke::from(stroke),
        ));
    }

    fn measure_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
    ) -> egui::Rect {
        self.painter
            .text(
                self.sc_tr(position),
                anchor,
                text,
                egui::FontId::proportional(font_size * self.camera_scale),
                egui::Color32::TRANSPARENT,
            )
            .translate(-self.canvas.min.to_vec2() - self.camera_offset.to_vec2())
            / self.camera_scale
    }
    fn draw_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
        text_color: egui::Color32,
    ) {
        if font_size * self.camera_scale >= 4.0 {
            self.painter.text(
                self.sc_tr(position),
                anchor,
                text,
                egui::FontId::proportional(font_size * self.camera_scale),
                text_color,
            );
        } else {
            let size = egui::Vec2::new(font_size * text.len() as f32 / 2.0, font_size);
            let pos = egui::Pos2::new(
                position.x
                    - match anchor.x() {
                        egui::Align::Min => -size.x / 2.0,
                        egui::Align::Center => 0.0,
                        egui::Align::Max => size.x / 2.0,
                    },
                position.y
                    - match anchor.y() {
                        egui::Align::Min => -size.y / 2.0,
                        egui::Align::Center => 0.0,
                        egui::Align::Max => size.y / 2.0,
                    },
            );
            self.draw_rectangle(
                egui::Rect::from_center_size(pos, size),
                egui::CornerRadius::ZERO,
                text_color,
                Stroke::new_solid(1.0, text_color.gamma_multiply(0.25)),
                Highlight::NONE,
            );
        }
    }
}

pub struct MeasuringCanvas<'a> {
    painter: &'a egui::Painter,
    bounds: egui::Rect,
}

impl<'a> MeasuringCanvas<'a> {
    pub fn new(painter: &'a egui::Painter) -> Self {
        Self {
            painter,
            bounds: egui::Rect::NOTHING,
        }
    }

    pub fn bounds(&self) -> egui::Rect {
        self.bounds
    }
}

impl<'a> NHCanvas for MeasuringCanvas<'a> {
    fn ui_scale(&self) -> Option<f32> {
        None
    }

    fn draw_line(&mut self, points: [egui::Pos2; 2], _stroke: Stroke, highlight: Highlight) {
        self.bounds.extend_with(points[0]);
        self.bounds.extend_with(points[1]);
    }

    fn draw_rectangle(
        &mut self,
        rect: egui::Rect,
        _corner_radius: egui::CornerRadius,
        _color: egui::Color32,
        _stroke: Stroke,
        highlight: Highlight,
    ) {
        self.bounds = self.bounds.union(rect);
    }

    fn draw_ellipse(
        &mut self,
        position: egui::Pos2,
        radius: egui::Vec2,
        _color: egui::Color32,
        _stroke: Stroke,
        highlight: Highlight,
    ) {
        let rect = egui::Rect::from_center_size(position, 2.0 * radius);
        self.bounds = self.bounds.union(rect);
    }

    fn draw_polygon(
        &mut self,
        vertices: Vec<egui::Pos2>,
        _color: egui::Color32,
        _stroke: Stroke,
        highlight: Highlight,
    ) {
        for p in vertices {
            self.bounds.extend_with(p);
        }
    }

    fn measure_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
    ) -> egui::Rect {
        self.painter.text(
            position,
            anchor,
            text,
            egui::FontId::proportional(font_size),
            egui::Color32::TRANSPARENT,
        )
    }
    fn draw_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
        _text_color: egui::Color32,
    ) {
        let rect = self.measure_text(position, anchor, text, font_size);
        self.bounds = self.bounds.union(rect);
    }
}

pub struct SVGCanvas<'a> {
    camera_offset: egui::Pos2,
    export_size: egui::Vec2,
    painter: &'a egui::Painter,
    element_buffer: Vec<String>,
}

impl<'a> SVGCanvas<'a> {
    pub fn new(painter: &'a egui::Painter, offset: egui::Pos2, size: egui::Vec2) -> Self {
        Self {
            camera_offset: offset,
            export_size: size,
            painter,
            element_buffer: Vec::new(),
        }
    }

    pub fn save_to(&self, path: &std::path::PathBuf) -> Result<(), std::io::Error> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        file.write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
"#,
                self.export_size.x, self.export_size.y
            )
            .as_bytes(),
        )?;

        for line in &self.element_buffer {
            file.write_all(line.as_bytes())?;
        }

        file.write_all(
            r#"</svg>
"#
            .as_bytes(),
        )?;

        Ok(())
    }
}

impl<'a> NHCanvas for SVGCanvas<'a> {
    fn ui_scale(&self) -> Option<f32> {
        None
    }

    fn draw_line(&mut self, points: [egui::Pos2; 2], stroke: Stroke, highlight: Highlight) {
        let stroke_dasharray = match stroke.line_type {
            LineType::Solid => "none",
            LineType::Dashed => "10,5",
        };

        self.element_buffer.push(format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-dasharray="{}"/>
"#,
            points[0].x + self.camera_offset.x,
            points[0].y + self.camera_offset.y,
            points[1].x + self.camera_offset.x,
            points[1].y + self.camera_offset.y,
            stroke.color.to_hex(),
            stroke_dasharray
        ));
    }

    fn draw_rectangle(
        &mut self,
        rect: egui::Rect,
        _corner_radius: egui::CornerRadius,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) {
        // TODO: implement rounding (not directly supported by SVG, potentially hard)
        let top_left = rect.left_top();
        self.element_buffer.push(format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>
"#,
            top_left.x + self.camera_offset.x,
            top_left.y + self.camera_offset.y,
            rect.width(),
            rect.height(),
            color.to_hex(),
            stroke.color.to_hex()
        ));
    }

    fn draw_ellipse(
        &mut self,
        position: egui::Pos2,
        radius: egui::Vec2,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) {
        self.element_buffer.push(format!(
            r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="{}" stroke="{}"/>
"#,
            position.x + self.camera_offset.x,
            position.y + self.camera_offset.y,
            radius.x,
            radius.y,
            color.to_hex(),
            stroke.color.to_hex()
        ));
    }

    fn draw_polygon(
        &mut self,
        vertices: Vec<egui::Pos2>,
        color: egui::Color32,
        stroke: Stroke,
        highlight: Highlight,
    ) {
        let polygon_points = vertices
            .iter()
            .map(|&p| {
                format!(
                    "{},{}",
                    p.x + self.camera_offset.x,
                    p.y + self.camera_offset.y
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        self.element_buffer.push(format!(
            r#"<polygon points="{}" fill="{}" stroke="{}"/>
"#,
            polygon_points,
            color.to_hex(),
            stroke.color.to_hex()
        ));
    }

    fn measure_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
    ) -> egui::Rect {
        self.painter.text(
            position,
            anchor,
            text,
            egui::FontId::proportional(font_size),
            egui::Color32::TRANSPARENT,
        )
    }
    fn draw_text(
        &mut self,
        position: egui::Pos2,
        anchor: egui::Align2,
        text: &str,
        font_size: f32,
        text_color: egui::Color32,
    ) {
        // TODO: use SVG alignment to minimize differences in fonts
        let rect = self.painter.text(
            position,
            anchor,
            text,
            egui::FontId::proportional(font_size),
            egui::Color32::TRANSPARENT,
        );

        let escaped = text
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("'", "&apos;")
            .replace("\"", "&quot;");
        let escaped_lines: Vec<_> = escaped.split("\n").collect();
        let initial_dx = (1.0 - escaped_lines.len() as f32) / 2.0;
        let mut tspans = String::new();
        let mut next_offset_bonus = 0.0;
        for (idx, line) in escaped_lines.into_iter().enumerate() {
            if line.is_empty() {
                next_offset_bonus += 1.0;
            } else {
                tspans += &format!(
                    r#"<tspan x="{}" dy="{}em">{}</tspan>"#,
                    rect.center().x + self.camera_offset.x,
                    if idx == 0 {
                        initial_dx
                    } else {
                        1.0 + next_offset_bonus
                    },
                    line
                );
                next_offset_bonus = 0.0;
            }
        }
        self.element_buffer.push(format!(r#"<text x="{}" y="{}" font-size="{}" fill="{}" text-anchor="middle" dominant-baseline="middle">{}</text>
"#, rect.center().x + self.camera_offset.x, rect.center().y + self.camera_offset.y, font_size, text_color.to_hex(), tspans));
    }
}
