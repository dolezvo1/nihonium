use eframe::egui;
use std::sync::{Arc, RwLock};
use crate::common::canvas::{NHCanvas, NHShape};

pub trait DiagramController {
    fn uuid(&self) -> uuid::Uuid;
    fn model_name(&self) -> String;
    
    fn new_ui_canvas(&self, ui: &mut egui::Ui) -> (Box<dyn NHCanvas>, egui::Response, Option<egui::Pos2>);
    fn handle_input(&mut self, ui: &mut egui::Ui, response: &egui::Response);
    fn click(&mut self, pos: egui::Pos2) -> bool;
    fn drag(&mut self, last_pos: egui::Pos2, delta: egui::Vec2) -> bool;
    fn context_menu(&mut self, ui: &mut egui::Ui);
    
    fn show_toolbar(&mut self, ui: &mut egui::Ui);
    fn show_properties(&mut self, ui: &mut egui::Ui);
    fn show_layers(&self, ui: &mut egui::Ui);
    fn list_in_project_hierarchy(&self, ui: &mut egui::Ui);
    
    // This hurts me at least as much as it hurts you
    fn outgoing_for<'a>(&'a self, _uuid: &'a uuid::Uuid) -> Box<dyn Iterator<Item=Arc<RwLock<dyn ElementController>>> + 'a> {
        Box::new(std::iter::empty::<Arc<RwLock<dyn ElementController>>>())
    }
    
    fn draw_in(&mut self, canvas: &mut dyn NHCanvas, mouse_pos: Option<egui::Pos2>);
}

pub trait ElementController {
    fn uuid(&self) -> uuid::Uuid;
    fn model_name(&self) -> String;
    
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

#[derive(Clone, Copy, PartialEq)]
pub struct ModifierKeys {
    pub control: bool,
}

impl ModifierKeys {
    pub const NONE: Self = Self { control: false };
}

#[derive(Clone, Copy, PartialEq)]
pub enum ClickHandlingStatus {
    NotHandled,
    Handled,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DragHandlingStatus {
    NotHandled,
    Handled,
}

// TODO: a generic DiagramController implementation

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
            let (source_next_point, dest_next_point)
                = match ($self.source_points[0].get(1).map(|e| *e).or($self.center_point),
                        $self.dest_points[0].get(1).map(|e| *e).or($self.center_point)) {
                    (None, None) => {
                        let pos_avg = (source_pos + dest_pos.to_vec2()) / 2.0;
                        (pos_avg, pos_avg)
                    },
                    (source_next_point, dest_next_point) => {
                        (source_next_point.unwrap_or(dest_pos), dest_next_point.unwrap_or(source_pos))
                    },
                };
            
            match (source_bounds.orthogonal_intersect(source_next_point)
                    .or_else(|| source_bounds.center_intersect(source_next_point)),
                dest_bounds.orthogonal_intersect(dest_next_point)
                    .or_else(|| dest_bounds.center_intersect(dest_next_point))) {
                (Some(source_intersect), Some(dest_intersect)) => {
                    $self.source_points[0][0] = source_intersect;
                    $self.dest_points[0][0] = dest_intersect;
                    $canvas.draw_multiconnection(
                        &[(model.link_type.source_arrowhead_type(),
                            crate::common::canvas::Stroke { width: 1.0, color: egui::Color32::BLACK, line_type: model.link_type.line_type() },
                            &$self.source_points[0],
                            Some(&model.source_arrowhead_label))],
                        &[(model.link_type.destination_arrowhead_type(),
                            crate::common::canvas::Stroke { width: 1.0, color: egui::Color32::BLACK, line_type: model.link_type.line_type() },
                            &$self.dest_points[0],
                            Some(&model.destination_arrowhead_label),)],
                        $self.position(),
                        None,
                    );
                },
                _ => {},
            }
        }
    }
    pub(crate) use multiconnection_draw_in;
    
    // center_point: Option<egui::Pos2>
    // fn sources(&mut self) -> &mut [Vec<egui::Pos2>];
    // fn destinations(&mut self) -> &mut [Vec<egui::Pos2>];
    macro_rules! multiconnection_element_drag {
        ($self:ident, $last_pos:ident, $delta:ident, $center_point:ident, $sources:ident, $destinations:ident, $ret:expr) => {
            const DISTANCE_THRESHOLD: f32 = 3.0;
            
            fn is_over(a: egui::Pos2, b: egui::Pos2) -> bool {
                a.distance(b) <= DISTANCE_THRESHOLD
            }
            
            match $self.center_point {
                // Check whether over center point, if so move it
                Some(pos) => if is_over($last_pos, pos) {
                    $self.center_point = Some(pos + $delta);
                    return $ret;
                },
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
                        let mut iter = path.iter().map(|e| *e).chain(center_point).enumerate().peekable();
                        while let Some((idx, u)) = iter.next() {
                            let v = if let Some((_, v)) = iter.peek() { *v } else { break; };
                        
                            let midpoint = (u + v.to_vec2()) / 2.0;
                            if is_over($last_pos, midpoint) {
                                path.insert(idx+1, midpoint + $delta);
                                return $ret;
                            }
                        }
                    }
                };
            }
            check_midpoints!(sources);
            check_midpoints!(destinations);
            
            fn dist_to_line_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
                fn dist2(a: egui::Pos2, b: egui::Pos2) -> f32 {
                    (a.x - b.x).powf(2.0) + (a.y - b.y).powf(2.0)
                }
                let l2 = dist2(a, b);
                let distance_squared = if l2 == 0.0 { dist2(p, a) } else {
                    let t = (((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2).clamp(0.0,1.0);
                    dist2(p, egui::Pos2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)))
                };
                return distance_squared.sqrt();
            }
            
            // TODO: this doesn't actually work in the situation where there is no center_point
            macro_rules! check_segments {
                ($v:ident) => {
                    let center_point = $self.center_point.clone();
                    for path in $self.$v() {
                        
                        // Iterates over 2-windows
                        let mut iter = path.iter().map(|e| *e).chain(center_point).peekable();
                        while let Some(u) = iter.next() {
                            let v = if let Some(v) = iter.peek() { *v } else { break; };
                            
                            if dist_to_line_segment($last_pos, u, v) <= DISTANCE_THRESHOLD {
                                return $ret;
                            }
                        }
                    }
                };
            }
            check_segments!(sources);
            check_segments!(destinations);
        }
    }
    pub(crate) use multiconnection_element_drag;
}
