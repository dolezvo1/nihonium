// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use common::canvas::UiCanvas;
use common::controller::{ColorLabels, ColorProfile};
use eframe::egui::{
    self, vec2, CentralPanel, Frame, Slider, TopBottomPanel, Ui, ViewportBuilder, WidgetText,
};
use eframe::NativeOptions;

use egui_dock::{AllowedSplits, DockArea, DockState, NodeIndex, Style, SurfaceIndex, TabViewer};

mod common;
mod democsd;
mod rdf;
mod umlclass;

use crate::common::canvas::{MeasuringCanvas, SVGCanvas};
use crate::common::controller::{DiagramCommand, DiagramController};

/// Adds a widget with a label next to it, can be given an extra parameter in order to show a hover text
macro_rules! labeled_widget {
    ($ui:expr, $x:expr, $l:expr) => {
        $ui.horizontal(|ui| {
            ui.add($x);
            ui.label($l);
        });
    };
    ($ui:expr, $x:expr, $l:expr, $d:expr) => {
        $ui.horizontal(|ui| {
            ui.add($x).on_hover_text($d);
            ui.label($l).on_hover_text($d);
        });
    };
}

// Creates a slider which has a unit attached to it
// When given an extra parameter it will be used as a multiplier (e.g 100.0 when working with percentages)
macro_rules! unit_slider {
    ($val:expr, $range:expr) => {
        egui::Slider::new($val, $range)
    };
    ($val:expr, $range:expr, $unit:expr) => {
        egui::Slider::new($val, $range).custom_formatter(|value, decimal_range| {
            egui::emath::format_with_decimals_in_range(value, decimal_range) + $unit
        })
    };
    ($val:expr, $range:expr, $unit:expr, $mul:expr) => {
        egui::Slider::new($val, $range)
            .custom_formatter(|value, decimal_range| {
                egui::emath::format_with_decimals_in_range(value * $mul, decimal_range) + $unit
            })
            .custom_parser(|string| string.parse::<f64>().ok().map(|valid| valid / $mul))
    };
}

fn main() -> eframe::Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");
    let options = NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size(vec2(1024.0, 1024.0)),
        ..Default::default()
    };
    eframe::run_native("113", options, Box::new(|_cc| Ok(Box::<NHApp>::default())))
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
enum NHTab {
    StyleEditor,

    ProjectHierarchy,

    Toolbar,
    Properties,
    Layers,

    Diagram { uuid: uuid::Uuid },
    CustomTab { uuid: uuid::Uuid },
}

impl NHTab {
    pub fn name(&self) -> &str {
        match self {
            NHTab::StyleEditor => "Style Editor",

            NHTab::ProjectHierarchy => "Project Hierarchy",

            NHTab::Toolbar => "Toolbar",
            NHTab::Properties => "Properties",
            NHTab::Layers => "Layers",

            NHTab::Diagram { .. } => "Diagram",
            NHTab::CustomTab { .. } => todo!(),
        }
    }
}

pub trait CustomTab {
    fn title(&self) -> String;
    fn show(&mut self, /*context: &mut NHApp,*/ ui: &mut egui::Ui);
    //fn on_close(&mut self, context: &mut NHApp);
}

struct NHContext {
    pub diagram_controllers: HashMap<uuid::Uuid, (usize, Arc<RwLock<dyn DiagramController>>)>,
    hierarchy_order: Vec<uuid::Uuid>,
    new_diagram_no: u32,
    pub custom_tabs: HashMap<uuid::Uuid, Arc<RwLock<dyn CustomTab>>>,

    pub style: Option<Style>,
    color_profiles: Vec<(String, ColorLabels, Vec<ColorProfile>)>,
    selected_color_profiles: Vec<usize>,

    undo_stack: Vec<(Arc<String>, uuid::Uuid)>,
    redo_stack: Vec<(Arc<String>, uuid::Uuid)>,
    
    shortcuts: HashMap<DiagramCommand, egui::KeyboardShortcut>,
    shortcut_top_order: Vec<(DiagramCommand, egui::KeyboardShortcut)>,

    open_unique_tabs: HashSet<NHTab>,
    last_focused_diagram: Option<uuid::Uuid>,
    svg_export_menu: Option<(usize, Arc<RwLock<dyn DiagramController>>, std::path::PathBuf, usize, bool, bool, f32, f32)>,

    show_close_buttons: bool,
    show_add_buttons: bool,
    draggable_tabs: bool,
    show_tab_name_on_hover: bool,
    allowed_splits: AllowedSplits,
    show_window_close: bool,
    show_window_collapse: bool,
}

impl TabViewer for NHContext {
    type Tab = NHTab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            NHTab::Diagram { uuid } => {
                let c = self.diagram_controllers.get(uuid).unwrap().1.read().unwrap();
                (&*c.model_name()).into()
            }
            NHTab::CustomTab { uuid } => {
                let c = self.custom_tabs.get(uuid).unwrap().read().unwrap();
                c.title().into()
            }
            t => t.name().into(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            NHTab::StyleEditor => self.style_editor_tab(ui),

            NHTab::ProjectHierarchy => self.hierarchy(ui),

            NHTab::Toolbar => self.toolbar(ui),
            NHTab::Properties => self.properties(ui),
            NHTab::Layers => self.layers(ui),

            NHTab::Diagram { uuid } => self.diagram_tab(uuid, ui),
            NHTab::CustomTab { uuid } => self.custom_tab(uuid, ui),
        }
    }

    fn context_menu(
        &mut self,
        ui: &mut Ui,
        _tab: &mut Self::Tab,
        _surface: SurfaceIndex,
        _node: NodeIndex,
    ) {
        ui.label("asdfasdf");
        ui.label("This is a tab context menu");
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        if let NHTab::Diagram { uuid } = tab {
            if response.clicked() || response.drag_started() {
                self.last_focused_diagram = Some(uuid.clone());
            }
        }
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        self.open_unique_tabs.remove(tab);
        true
    }
}

impl NHContext {
    fn sort_shortcuts(&mut self) {
        self.shortcut_top_order = self.shortcuts.iter().map(|(&k,&v)|(k,v)).collect();
        
        fn weight(m: &egui::KeyboardShortcut) -> u32 {
            m.modifiers.alt as u32 + m.modifiers.command as u32 + m.modifiers.shift as u32
        }
        
        self.shortcut_top_order.sort_by(|a, b| weight(&b.1).cmp(&weight(&a.1)));
    }

    fn hierarchy(&self, ui: &mut Ui) {
        for (_t, c) in self.hierarchy_order.iter()
            .flat_map(|e| self.diagram_controllers.get(e))
        {
            let controller_lock = c.write().unwrap();
            controller_lock.list_in_project_hierarchy(ui);
        }
    }

    fn toolbar(&self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else {return;};
        let mut controller_lock = c.write().unwrap();
        controller_lock.show_toolbar(ui);
    }

    fn properties(&mut self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
        let mut controller_lock = c.write().unwrap();
        
        let mut undo_accumulator = Vec::<Arc<String>>::new();
        controller_lock.show_properties(ui, &mut undo_accumulator);
        if !undo_accumulator.is_empty() {
            for (_uuid, (_t, c)) in self.diagram_controllers.iter().filter(|(uuid, _)| *uuid != last_focused_diagram) {
                let mut c = c.write().unwrap();
                c.apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![]);
            }
            
            self.redo_stack.clear();
            controller_lock.apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![]);
            controller_lock.apply_command(DiagramCommand::SetLastChangeFlag, &mut vec![]);
            
            for command_label in undo_accumulator {
                self.undo_stack.push((command_label, *last_focused_diagram));
            }
        }
    }

    fn layers(&self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else {return;};
        let mut controller_lock = c.write().unwrap();
        controller_lock.show_layers(ui);
    }

    fn style_editor_tab(&mut self, ui: &mut Ui) {
        ui.heading("Style Editor");

        ui.collapsing("DockArea Options", |ui| {
            ui.checkbox(&mut self.show_close_buttons, "Show close buttons");
            ui.checkbox(&mut self.show_add_buttons, "Show add buttons");
            ui.checkbox(&mut self.draggable_tabs, "Draggable tabs");
            ui.checkbox(&mut self.show_tab_name_on_hover, "Show tab name on hover");
            ui.checkbox(&mut self.show_window_close, "Show close button on windows");
            ui.checkbox(
                &mut self.show_window_collapse,
                "Show collaspse button on windows",
            );
            egui::ComboBox::new("cbox:allowed_splits", "Split direction(s)")
                .selected_text(format!("{:?}", self.allowed_splits))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.allowed_splits, AllowedSplits::All, "All");
                    ui.selectable_value(
                        &mut self.allowed_splits,
                        AllowedSplits::LeftRightOnly,
                        "LeftRightOnly",
                    );
                    ui.selectable_value(
                        &mut self.allowed_splits,
                        AllowedSplits::TopBottomOnly,
                        "TopBottomOnly",
                    );
                    ui.selectable_value(&mut self.allowed_splits, AllowedSplits::None, "None");
                });
        });

        let style = self.style.as_mut().unwrap();

        ui.collapsing("Border", |ui| {
            egui::Grid::new("border").show(ui, |ui| {
                ui.label("Width:");
                ui.add(Slider::new(
                    &mut style.main_surface_border_stroke.width,
                    1.0..=50.0,
                ));
                ui.end_row();

                ui.label("Color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.main_surface_border_stroke.color,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Rounding:");
                rounding_ui(ui, &mut style.main_surface_border_rounding);
                ui.end_row();
            });
        });

        ui.collapsing("Separator", |ui| {
            egui::Grid::new("separator").show(ui, |ui| {
                ui.label("Width:");
                ui.add(Slider::new(&mut style.separator.width, 1.0..=50.0));
                ui.end_row();

                ui.label("Extra Interact Width:");
                ui.add(Slider::new(
                    &mut style.separator.extra_interact_width,
                    0.0..=50.0,
                ));
                ui.end_row();

                ui.label("Offset limit:");
                ui.add(Slider::new(&mut style.separator.extra, 1.0..=300.0));
                ui.end_row();

                ui.label("Idle color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.separator.color_idle,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Hovered color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.separator.color_hovered,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Dragged color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.separator.color_dragged,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();
            });
        });

        ui.collapsing("Tabs", |ui| {
            ui.separator();

            ui.checkbox(&mut style.tab_bar.fill_tab_bar, "Expand tabs");
            ui.checkbox(
                &mut style.tab_bar.show_scroll_bar_on_overflow,
                "Show scroll bar on tab overflow",
            );
            ui.checkbox(
                &mut style.tab.hline_below_active_tab_name,
                "Show a line below the active tab name",
            );
            ui.horizontal(|ui| {
                ui.add(Slider::new(&mut style.tab_bar.height, 20.0..=50.0));
                ui.label("Tab bar height");
            });

            egui::ComboBox::new("add_button_align", "Add button align")
                .selected_text(format!("{:?}", style.buttons.add_tab_align))
                .show_ui(ui, |ui| {
                    for align in [egui_dock::TabAddAlign::Left, egui_dock::TabAddAlign::Right] {
                        ui.selectable_value(
                            &mut style.buttons.add_tab_align,
                            align,
                            format!("{:?}", align),
                        );
                    }
                });

            ui.separator();

            fn tab_style_editor_ui(ui: &mut Ui, tab_style: &mut egui_dock::TabInteractionStyle) {
                ui.separator();

                ui.label("Rounding");
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.rounding.nw, 0.0..=15.0),
                    "North-West"
                );
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.rounding.ne, 0.0..=15.0),
                    "North-East"
                );
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.rounding.sw, 0.0..=15.0),
                    "South-West"
                );
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.rounding.se, 0.0..=15.0),
                    "South-East"
                );

                ui.separator();

                egui::Grid::new("tabs_colors").show(ui, |ui| {
                    ui.label("Title text color:");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut tab_style.text_color,
                        egui::color_picker::Alpha::OnlyBlend,
                    );
                    ui.end_row();

                    ui.label("Outline color:")
                        .on_hover_text("The outline around the active tab name.");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut tab_style.outline_color,
                        egui::color_picker::Alpha::OnlyBlend,
                    );
                    ui.end_row();

                    ui.label("Background color:");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut tab_style.bg_fill,
                        egui::color_picker::Alpha::OnlyBlend,
                    );
                    ui.end_row();
                });
            }

            ui.collapsing("Active", |ui| {
                tab_style_editor_ui(ui, &mut style.tab.active);
            });

            ui.collapsing("Inactive", |ui| {
                tab_style_editor_ui(ui, &mut style.tab.inactive);
            });

            ui.collapsing("Focused", |ui| {
                tab_style_editor_ui(ui, &mut style.tab.focused);
            });

            ui.collapsing("Hovered", |ui| {
                tab_style_editor_ui(ui, &mut style.tab.hovered);
            });

            ui.separator();

            egui::Grid::new("tabs_colors").show(ui, |ui| {
                ui.label("Close button color unfocused:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.buttons.close_tab_color,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Close button color focused:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.buttons.close_tab_active_color,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Close button background color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.buttons.close_tab_bg_fill,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Bar background color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.tab_bar.bg_fill,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Horizontal line color:").on_hover_text(
                    "The line separating the tab name area from the tab content area",
                );
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.tab_bar.hline_color,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();
            });
        });

        ui.collapsing("Tab body", |ui| {
            ui.separator();

            ui.label("Rounding");
            rounding_ui(ui, &mut style.tab.tab_body.rounding);

            ui.label("Stroke width:");
            ui.add(Slider::new(
                &mut style.tab.tab_body.stroke.width,
                0.0..=10.0,
            ));
            ui.end_row();

            egui::Grid::new("tab_body_colors").show(ui, |ui| {
                ui.label("Stroke color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.tab.tab_body.stroke.color,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Background color:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.tab.tab_body.bg_fill,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();
            });
        });
        ui.collapsing("Overlay", |ui| {
            let selected_text = match style.overlay.overlay_type {
                egui_dock::OverlayType::HighlightedAreas => "Highlighted Areas",
                egui_dock::OverlayType::Widgets => "Widgets",
            };
            ui.label("Overlay Style:");
            egui::ComboBox::new("overlay styles", "")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut style.overlay.overlay_type,
                        egui_dock::OverlayType::HighlightedAreas,
                        "Highlighted Areas",
                    );
                    ui.selectable_value(
                        &mut style.overlay.overlay_type,
                        egui_dock::OverlayType::Widgets,
                        "Widgets",
                    );
                });
            ui.collapsing("Feel", |ui|{
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.feel.center_drop_coverage, 0.0..=1.0, "%", 100.0),
                    "Center drop coverage",
                    "how big the area where dropping a tab into the center of another should be."
                );
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.feel.fade_hold_time, 0.0..=4.0, "s"),
                    "Fade hold time",
                    "How long faded windows should hold their fade before unfading, in seconds."
                );
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.feel.max_preference_time, 0.0..=4.0, "s"),
                    "Max preference time",
                    "How long the overlay may prefer to stick to a surface despite hovering over another, in seconds."
                );
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.feel.window_drop_coverage, 0.0..=1.0, "%", 100.0),
                    "Window drop coverage",
                    "How big the area for undocking a window should be. [is overshadowed by center drop coverage]"
                );
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.feel.interact_expansion, 1.0..=100.0, "ps"),
                    "Interact expansion",
                    "How much extra interaction area should be allocated for buttons on the overlay"
                );
            });

            ui.collapsing("Visuals", |ui|{
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.max_button_size, 10.0..=500.0, "ps"),
                    "Max button size",
                    "The max length of a side on a overlay button in egui points"
                );
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.button_spacing, 0.0..=50.0, "ps"),
                    "Button spacing",
                    "Spacing between buttons on the overlay, in egui units."
                );
                labeled_widget!(
                    ui,
                    unit_slider!(&mut style.overlay.surface_fade_opacity, 0.0..=1.0, "%", 100.0),
                    "Window fade opacity",
                    "how visible windows are when dragging a tab behind them."
                );
                labeled_widget!(
                    ui,
                    egui::Slider::new(&mut style.overlay.selection_stroke_width, 0.0..=50.0),
                    "Selection stroke width",
                    "width of a selection which uses a outline stroke instead of filled rect."
                );
                egui::Grid::new("overlay style preferences").show(ui, |ui| {
                    ui.label("Button color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut style.overlay.button_color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Button border color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut style.overlay.button_border_stroke.color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Selection color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut style.overlay.selection_color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Button stroke color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut style.overlay.button_border_stroke.color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Button stroke width:");
                    ui.add(Slider::new(&mut style.overlay.button_border_stroke.width, 0.0..=50.0));
                    ui.end_row();
                });
            });

            ui.collapsing("Hover highlight", |ui|{
                egui::Grid::new("leaf highlighting prefs").show(ui, |ui|{
                    ui.label("Fill color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut style.overlay.hovered_leaf_highlight.color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Stroke color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut style.overlay.hovered_leaf_highlight.stroke.color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Stroke width:");
                    ui.add(Slider::new(&mut style.overlay.hovered_leaf_highlight.stroke.width, 0.0..=50.0));
                    ui.end_row();

                    ui.label("Expansion:");
                    ui.add(Slider::new(&mut style.overlay.hovered_leaf_highlight.expansion, -50.0..=50.0));
                    ui.end_row();
                });
                ui.label("Rounding:");
                rounding_ui(ui, &mut style.overlay.hovered_leaf_highlight.rounding);
            });
        });
        
        ui.collapsing("Diagram themes", |ui|{
            for (idx1, (name, l, p)) in self.color_profiles.iter().enumerate() {
                egui::ComboBox::from_label(name)
                    .selected_text(&p[self.selected_color_profiles[idx1]].name)
                    .show_ui(ui, |ui| {
                        for (idx2, profile) in p.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_color_profiles[idx1], idx2, &profile.name);
                        }
                        // TODO: allow custom profiles
                    }
                );
            }
        });
    }
    
    // In general it should draw first and handle input second, right?
    fn diagram_tab(&mut self, tab_uuid: &uuid::Uuid, ui: &mut Ui) {
        let Some((t, arc)) = self.diagram_controllers.get(tab_uuid) else { return; };
        let mut diagram_controller = arc.write().unwrap();
        let color_profile = &self.color_profiles[*t].2[self.selected_color_profiles[*t]];
        
        let (mut ui_canvas, response, pos) = diagram_controller.new_ui_canvas(ui, color_profile);

        if response.clicked() || response.drag_started() {
            self.last_focused_diagram = Some(tab_uuid.clone());
        }

        diagram_controller.draw_in(ui_canvas.as_mut(), color_profile, pos);

        let mut undo_accumulator = Vec::<Arc<String>>::new();
        diagram_controller.handle_input(ui, &response, &mut undo_accumulator);
        response.context_menu(|ui| diagram_controller.context_menu(ui));
        if !undo_accumulator.is_empty() {
            for (_uuid, (t, c)) in self.diagram_controllers.iter().filter(|(uuid, _)| *uuid != tab_uuid) {
                let mut c = c.write().unwrap();
                c.apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![]);
            }
            
            self.redo_stack.clear();
            diagram_controller.apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![]);
            diagram_controller.apply_command(DiagramCommand::SetLastChangeFlag, &mut vec![]);
            
            for command_label in undo_accumulator {
                self.undo_stack.push((command_label, *tab_uuid));
            }
        }
    }

    fn custom_tab(&mut self, tab_uuid: &uuid::Uuid, ui: &mut Ui) {
        let x = self.custom_tabs.get(tab_uuid).map(|e| e.clone()).unwrap();
        let mut custom_tab = x.write().unwrap();
        custom_tab.show(/*self,*/ ui);
    }

    fn last_focused_diagram(&mut self) -> Option<(usize, Arc<RwLock<dyn DiagramController>>)> {
        self.last_focused_diagram
            .as_ref()
            .and_then(|e| self.diagram_controllers.get(e).cloned())
    }
}

struct NHApp {
    context: NHContext,
    tree: DockState<NHTab>,
}

impl Default for NHApp {
    fn default() -> Self {
        let mut diagram_controllers = HashMap::new();
        let mut hierarchy_order = vec![];
        let mut tabs = vec![NHTab::StyleEditor];

        for (diagram_type, (uuid, controller)) in [
            (0, crate::rdf::rdf_controllers::demo(1)),
            (1, crate::umlclass::umlclass_controllers::demo(2)),
            (2, crate::democsd::democsd_controllers::demo(3)),
        ] {
            diagram_controllers.insert(uuid, (diagram_type, controller));
            hierarchy_order.push(uuid);
            tabs.push(NHTab::Diagram { uuid });
        }

        let mut dock_state = DockState::new(tabs);
        "Undock".clone_into(&mut dock_state.translations.tab_context_menu.eject_button);

        let mut open_unique_tabs = HashSet::new();
        open_unique_tabs.insert(NHTab::StyleEditor);

        let [a, b] = dock_state.main_surface_mut().split_left(
            NodeIndex::root(),
            0.2,
            vec![NHTab::ProjectHierarchy],
        );
        open_unique_tabs.insert(NHTab::ProjectHierarchy);
        let [_, _] = dock_state
            .main_surface_mut()
            .split_right(a, 0.7, vec![NHTab::Properties]);
        open_unique_tabs.insert(NHTab::Properties);
        let [_, _] = dock_state
            .main_surface_mut()
            .split_below(b, 0.7, vec![NHTab::Toolbar]);
        open_unique_tabs.insert(NHTab::Toolbar);

        let color_profiles = vec![
            crate::rdf::rdf_controllers::colors(),
            crate::umlclass::umlclass_controllers::colors(),
            crate::democsd::democsd_controllers::colors()
        ];
        
        let selected_color_profiles = color_profiles.iter().map(|_| 0).collect();
        
        let mut context = NHContext {
            diagram_controllers,
            hierarchy_order,
            new_diagram_no: 4,
            custom_tabs: HashMap::new(),
            
            style: None,
            color_profiles,
            selected_color_profiles,
            
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            
            shortcuts: HashMap::new(),
            shortcut_top_order: vec![],

            open_unique_tabs,
            last_focused_diagram: None,
            svg_export_menu: None,

            show_window_close: true,
            show_window_collapse: true,
            show_close_buttons: true,
            show_add_buttons: false,
            draggable_tabs: true,
            show_tab_name_on_hover: false,
            allowed_splits: AllowedSplits::default(),
        };
        
        context.shortcuts.insert(DiagramCommand::UndoImmediate, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z));
        context.shortcuts.insert(DiagramCommand::RedoImmediate, egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ));
        context.shortcuts.insert(DiagramCommand::SelectAllElements(true), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A));
        context.shortcuts.insert(DiagramCommand::SelectAllElements(false), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::A,
        ));
        context.shortcuts.insert(DiagramCommand::InvertSelection, egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::I,
        ));
        context.shortcuts.insert(DiagramCommand::DeleteSelectedElements, egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete));
        context.sort_shortcuts();

        Self {
            context,
            tree: dock_state,
        }
    }
}

impl NHApp {
    fn switch_to_tab(&mut self, tab: &NHTab) {
        let Some(t) = self.tree.find_tab(&tab) else { return; };
        self.tree.set_active_tab(t);
        if let NHTab::Diagram { uuid } = tab {
            self.context.last_focused_diagram = Some(*uuid);
        }
    }

    fn undo_immediate(&mut self) {
        let Some(e) = self.context.undo_stack.pop() else { return; };
        
        self.switch_to_tab(&NHTab::Diagram { uuid: e.1 });
        
        {
            let Some((_t, ac)) = self.context.diagram_controllers.get(&e.1) else { return; };
            let mut c = ac.write().unwrap();
            c.apply_command(DiagramCommand::UndoImmediate, &mut vec![]);
        }
        
        self.context.redo_stack.push(e);
    }
    fn redo_immediate(&mut self) {
        let Some(e) = self.context.redo_stack.pop() else { return; };
        
        self.switch_to_tab(&NHTab::Diagram { uuid: e.1 });
        
        {
            let Some((_t, ac)) = self.context.diagram_controllers.get(&e.1) else { return; };
            let mut c = ac.write().unwrap();
            c.apply_command(DiagramCommand::RedoImmediate, &mut vec![]);
        }
        
        self.context.undo_stack.push(e);
    }

    pub fn add_custom_tab(&mut self, uuid: uuid::Uuid, tab: Arc<RwLock<dyn CustomTab>>) {
        self.context.custom_tabs.insert(uuid, tab);

        let tab = NHTab::CustomTab { uuid };

        self.tree[SurfaceIndex::main()].push_to_focused_leaf(tab);
    }
}

fn new_project() -> Result<(), &'static str> {
    // FIXME: closing original project closes the new one as well

    let Ok(executable) = std::env::current_exe() else {
        return Err("Failed to get current executable");
    };

    let Ok(_child) = std::process::Command::new(executable).spawn() else {
        return Err("Failed to start process");
    };

    Ok(())
}

impl eframe::App for NHApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("egui_dock::MenuBar").show(ctx, |ui| {
            macro_rules! send_to_focused_diagram {
                ($command:expr) => {
                    if let Some((_, NHTab::Diagram { uuid })) = self.tree.find_active_focused() {
                        if let Some((_t, ac)) = self.context.diagram_controllers.get(&uuid) {
                            let mut c = ac.write().unwrap();
                            let mut undo = vec![];
                            c.apply_command($command, &mut undo);
                            self.context.undo_stack.extend(undo.into_iter().map(|e| (e, *uuid)));
                        }
                    }
                };
            }
            
            // Check diagram-handled shortcuts
            ui.input(|is|
                'outer: for e in is.events.iter() {
                    let egui::Event::Key { key, pressed, modifiers, .. } = e else { continue; };
                    if !pressed {continue;}
                    'inner: for ksh in &self.context.shortcut_top_order {
                        if !(modifiers.matches_logically(ksh.1.modifiers) && *key == ksh.1.logical_key) {
                            continue 'inner;
                        }
                        
                        match ksh.0 {
                            DiagramCommand::UndoImmediate => self.undo_immediate(),
                            DiagramCommand::RedoImmediate => self.redo_immediate(),
                            DiagramCommand::SelectAllElements(select) => send_to_focused_diagram!(DiagramCommand::SelectAllElements(select)),
                            DiagramCommand::InvertSelection => send_to_focused_diagram!(DiagramCommand::InvertSelection),
                            DiagramCommand::DeleteSelectedElements => send_to_focused_diagram!(DiagramCommand::DeleteSelectedElements),
                            _ => unreachable!(),
                        }
                        
                        break 'outer;
                    }
                }
            );
            
            // Menubar UI
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {
                        let _ = new_project();
                    }
                    // TODO: implement
                    if ui.button("Open Project").clicked() {
                        println!("TODO");
                    }
                    // TODO: implement
                    ui.menu_button("Recent Projects", |ui| {
                        if ui.button("asdf").clicked() {
                            println!("TODO");
                        }
                    });
                    ui.separator();

                    ui.menu_button("Add New Diagram", |ui| {
                        type NDC =
                            fn(u32) -> (uuid::Uuid, Arc<RwLock<(dyn DiagramController + 'static)>>);
                        for (label, diagram_type, fun) in [
                            (
                                "UML Class diagram",
                                0,
                                crate::umlclass::umlclass_controllers::new as NDC,
                            ),
                            //("Add New OntoUML diagram"),
                            (
                                "DEMO CSD diagram",
                                1,
                                crate::democsd::democsd_controllers::new as NDC,
                            ),
                            ("RDF diagram", 2, crate::rdf::rdf_controllers::new as NDC),
                        ] {
                            if ui.button(label).clicked() {
                                let (uuid, diagram_controller) = fun(self.context.new_diagram_no);
                                self.context.new_diagram_no += 1;
                                self.context
                                    .diagram_controllers
                                    .insert(uuid, (diagram_type, diagram_controller));
                                self.context
                                    .hierarchy_order
                                    .push(uuid);

                                let tab = NHTab::Diagram { uuid };

                                self.tree[SurfaceIndex::main()].push_to_focused_leaf(tab);
                                self.context.last_focused_diagram = Some(uuid);

                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();

                    /*
                    if ui.button("Save to").clicked() {
                        println!("yes");
                    }
                    if ui.button("Save as").clicked() {
                        println!("yes");
                    }
                    ui.separator();

                    if ui.button("Exit").clicked() {
                        println!("yes");
                    }
                    */
                });

                ui.menu_button("Edit", |ui| {
                    ui.menu_button("Undo", |ui| {
                        let shortcut_text = self.context.shortcuts.get(&DiagramCommand::UndoImmediate).map(|e| ui.ctx().format_shortcut(&e));
                        
                        if self.context.undo_stack.is_empty() {
                            let mut button = egui::Button::new("(nothing to undo)");
                            if let Some(shortcut_text) = shortcut_text {
                                button = button.shortcut_text(shortcut_text);
                            }
                            let _ = ui.add_enabled(false, button);
                        } else {
                            for (ii, (c, uuid)) in self.context.undo_stack.iter().rev().enumerate() {
                                let Some((_t, ac)) = self.context.diagram_controllers.get(uuid) else {
                                    break;
                                };
                                let mut button = egui::Button::new(format!("{} in '{}'", &*c, ac.read().unwrap().model_name()));
                                if let Some(shortcut_text) = shortcut_text.as_ref().filter(|_| ii == 0) {
                                    button = button.shortcut_text(shortcut_text);
                                }

                                if ui.add(button).clicked() {
                                    for _ in 0..=ii {
                                        self.undo_immediate();
                                    }
                                    break;
                                }
                            }
                        }
                        
                    });
                    
                    ui.menu_button("Redo", |ui| {
                        let shortcut_text = self.context.shortcuts.get(&DiagramCommand::RedoImmediate).map(|e| ui.ctx().format_shortcut(&e));
                        
                        if self.context.redo_stack.is_empty() {
                            let mut button = egui::Button::new("(nothing to redo)");
                            if let Some(shortcut_text) = shortcut_text {
                                button = button.shortcut_text(shortcut_text);
                            }
                            let _ = ui.add_enabled(false, button);
                        } else {
                            for (ii, (c, uuid)) in self.context.redo_stack.iter().rev().enumerate() {
                                let Some((_t, ac)) = self.context.diagram_controllers.get(uuid) else {
                                    break;
                                };
                                let mut button = egui::Button::new(format!("{} in '{}'", &*c, ac.read().unwrap().model_name()));
                                if let Some(shortcut_text) = shortcut_text.as_ref().filter(|_| ii == 0) {
                                    button = button.shortcut_text(shortcut_text);
                                }

                                if ui.add(button).clicked() {
                                    for _ in 0..=ii {
                                        self.redo_immediate();
                                    }
                                    break;
                                }
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
                    ui.separator();

                    if let Some((_t, d)) = self.context.last_focused_diagram() {
                        let mut d = d.write().unwrap();
                        d.show_menubar_edit_options(self, ui);
                    }
                });

                ui.menu_button("View", |_ui| {
                    /*
                    if ui.button("Reset").clicked() {
                        println!("no");
                    }
                    */
                });

                ui.menu_button("Diagram", |ui| {
                    let Some((t, c)) = self.context.last_focused_diagram() else { return; };
                    let mut controller = c.write().unwrap();

                    controller.show_menubar_diagram_options(self, ui);

                    ui.menu_button(
                        format!("Export Diagram `{}` to", controller.model_name()),
                        |ui| {
                            if ui.button("SVG").clicked() {
                                // NOTE: This does not work on WASM, and in its current state it never will.
                                //       This will be possible to fix once this is fixed on rfd side (#128).
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_directory(std::env::current_dir().unwrap())
                                    .add_filter("SVG files", &["svg"])
                                    .add_filter("All files", &["*"])
                                    .save_file()
                                {
                                    self.context.svg_export_menu = Some((t, c.clone(), path, self.context.selected_color_profiles[t], false, false, 10.0, 10.0));
                                }
                                ui.close_menu();
                            }
                        },
                    );
                });

                ui.menu_button("Windows", |ui| {
                    // allow certain tabs to be toggled
                    for tab in &[
                        NHTab::StyleEditor,
                        NHTab::ProjectHierarchy,
                        NHTab::Toolbar,
                        NHTab::Properties,
                        NHTab::Layers,
                    ] {
                        if ui
                            .selectable_label(
                                self.context.open_unique_tabs.contains(tab),
                                tab.name(),
                            )
                            .clicked()
                        {
                            if let Some(index) = self.tree.find_tab(tab) {
                                self.tree.remove_tab(index);
                                self.context.open_unique_tabs.remove(tab);
                            } else {
                                self.tree[SurfaceIndex::main()].push_to_focused_leaf(tab.clone());
                                self.context.open_unique_tabs.insert(tab.clone());
                            }

                            ui.close_menu();
                        }
                    }
                });
            })
        });
        
        // SVG export options modal
        let mut hide_svg_export_modal = false;
        if let Some((t, c, path, profile, background, gridlines, padding_x, padding_y)) = self.context.svg_export_menu.as_mut() {
            let mut controller = c.write().unwrap();
            
            egui::containers::Window::new("SVG export options").show(ctx, |ui| {
                ui.label(format!("Location: `{}`", path.display()));
                
                // Change options
                egui::ComboBox::from_label("Color profile")
                    .selected_text(&self.context.color_profiles[*t].2[*profile].name)
                    .show_ui(ui, |ui| {
                        for (idx2, p) in self.context.color_profiles[*t].2.iter().enumerate() {
                            ui.selectable_value(profile, idx2, &p.name);
                        }
                    }
                );
                ui.checkbox(background, "Solid background");
                ui.checkbox(gridlines, "Gridlines");
                
                ui.add(egui::Slider::new(padding_x, 0.0..=5000.0).text("Horizontal padding"));
                ui.add(egui::Slider::new(padding_y, 0.0..=5000.0).text("Vertical padding"));
                
                ui.separator();
                
                // Show preview
                {
                    let color_profile = &self.context.color_profiles[*t].2[*profile];
                    
                    // Measure the diagram
                    let mut measuring_canvas =
                            MeasuringCanvas::new(ui.painter());
                    controller.draw_in(&mut measuring_canvas, color_profile, None);
                    let diagram_bounds = measuring_canvas.bounds();
                    drop(measuring_canvas);
                    
                    let preview_width = ui.available_width();
                    let camera_scale = preview_width / (diagram_bounds.width() + 2.0 * *padding_x);
                    let preview_height = preview_width * (diagram_bounds.height() + 2.0 * *padding_y)
                        / (diagram_bounds.width() + 2.0 * *padding_x);
                    let preview_size = egui::Vec2::new(preview_width, preview_height);
                    
                    // Draw the diagram
                    let canvas_pos = ui.next_widget_position();
                    let canvas_rect = egui::Rect::from_min_size(canvas_pos, preview_size);
                    
                    let (painter_response, painter) =
                        ui.allocate_painter(preview_size, egui::Sense::focusable_noninteractive());
                    if *background {
                        painter.rect(
                            canvas_rect,
                            egui::Rounding::ZERO,
                            color_profile.backgrounds[0],
                            egui::Stroke::NONE,
                        );
                    } else {
                        let rect_side: f32 = preview_width / 20.0;
                        for ii in 0..20 {
                            for jj in 0..=((preview_height / rect_side) as u32) {
                                painter.rect(
                                    egui::Rect::from_min_size(
                                        egui::Pos2::new(ii as f32 * rect_side, jj as f32 * rect_side)
                                        + canvas_rect.min.to_vec2(),
                                        egui::Vec2::splat(rect_side)
                                    ),
                                    egui::Rounding::ZERO,
                                    if (ii + jj) % 2 == 0 {egui::Color32::GRAY} else {egui::Color32::from_rgb(50, 50, 50)},
                                    egui::Stroke::NONE
                                );
                            }
                        }
                    }
                    let mut ui_canvas = UiCanvas::new(
                        false,
                        painter,
                        canvas_rect,
                        diagram_bounds.min * -camera_scale + egui::Vec2::new(*padding_x, *padding_y) * camera_scale,
                        camera_scale,
                        None,
                    );
                    if *gridlines {
                        ui_canvas.draw_gridlines(
                            Some((50.0, color_profile.foregrounds[0])),
                            Some((50.0, color_profile.foregrounds[0])),
                        );
                    }
                    controller.draw_in(&mut ui_canvas, color_profile, None);
                }
                
                // Cancel or confirm export
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        hide_svg_export_modal = true;
                    }
                    if ui.button("OK").clicked() {
                        let color_profile = &self.context.color_profiles[*t].2[*profile];
                        
                        let mut measuring_canvas =
                            MeasuringCanvas::new(ui.painter());
                        controller.draw_in(&mut measuring_canvas, color_profile, None);

                        let mut svg_canvas = SVGCanvas::new(
                            ui.painter(),
                            -1.0 * measuring_canvas.bounds().min
                                + egui::Vec2::new(*padding_x, *padding_y),
                            measuring_canvas.bounds().size()
                                + egui::Vec2::new(
                                    2.0 * *padding_x,
                                    2.0 * *padding_y,
                                ),
                        );
                        controller.draw_in(&mut svg_canvas, color_profile, None);
                        let _ = svg_canvas.save_to(&path);
                        
                        hide_svg_export_modal = true;
                    }
                });
            });
        }
        if hide_svg_export_modal {
            self.context.svg_export_menu = None;
        }
        
        CentralPanel::default()
            // When displaying a DockArea in another UI, it looks better
            // to set inner margins to 0.
            .frame(Frame::central_panel(&ctx.style()).inner_margin(0.))
            .show(ctx, |ui| {
                let style = self
                    .context
                    .style
                    .get_or_insert(Style::from_egui(ui.style()))
                    .clone();

                DockArea::new(&mut self.tree)
                    .style(style)
                    .show_close_buttons(self.context.show_close_buttons)
                    .show_add_buttons(self.context.show_add_buttons)
                    .draggable_tabs(self.context.draggable_tabs)
                    .show_tab_name_on_hover(self.context.show_tab_name_on_hover)
                    .allowed_splits(self.context.allowed_splits)
                    .show_leaf_close_all_buttons(self.context.show_window_close)
                    .show_leaf_collapse_buttons(self.context.show_window_collapse)
                    .show_inside(ui, &mut self.context);
            });
    }
}

fn rounding_ui(ui: &mut Ui, rounding: &mut egui::Rounding) {
    labeled_widget!(ui, Slider::new(&mut rounding.nw, 0.0..=15.0), "North-West");
    labeled_widget!(ui, Slider::new(&mut rounding.ne, 0.0..=15.0), "North-East");
    labeled_widget!(ui, Slider::new(&mut rounding.sw, 0.0..=15.0), "South-West");
    labeled_widget!(ui, Slider::new(&mut rounding.se, 0.0..=15.0), "South-East");
}
