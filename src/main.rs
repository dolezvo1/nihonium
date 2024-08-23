// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};

use eframe::NativeOptions;
use eframe::egui::{
    self,
    vec2, CentralPanel, Frame, Slider, TopBottomPanel, Ui, ViewportBuilder, WidgetText,
};

use egui_dock::{
    AllowedSplits, DockArea, DockState, NodeIndex, Style, SurfaceIndex, TabViewer,
};

mod common;
mod rdf;
mod umlclass;

use crate::common::canvas::SVGCanvas;
use crate::common::controller::DiagramController;

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
    eframe::run_native(
        "113",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
enum NHTab {
    Hierarchy,
    Toolbar,
    Properties,
    Layers,
    StyleEditor,
    
    Diagram { uuid: uuid::Uuid },
}

impl NHTab {
    pub fn name(&self) -> &str {
        match self {
            NHTab::Hierarchy => "Project Hierarchy",
            NHTab::Toolbar => "Toolbar",
            NHTab::Properties => "Properties",
            NHTab::Layers => "Layers",
            NHTab::StyleEditor => "Style Editor",
            NHTab::Diagram{..} => "Diagram",
        }
    }
}

struct MyContext {
    pub diagram_controllers: HashMap<uuid::Uuid, Box<dyn DiagramController>>,
    new_diagram_no: u32,
    
    pub style: Option<Style>,
    
    open_unique_tabs: HashSet<NHTab>,
    last_focused_diagram: Option<uuid::Uuid>,
    
    show_close_buttons: bool,
    show_add_buttons: bool,
    draggable_tabs: bool,
    show_tab_name_on_hover: bool,
    allowed_splits: AllowedSplits,
    show_window_close: bool,
    show_window_collapse: bool,
}

struct MyApp {
    context: MyContext,
    tree: DockState<NHTab>,
}

impl TabViewer for MyContext {
    type Tab = NHTab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            NHTab::Diagram{ uuid } => self.diagram_controllers.get(uuid).unwrap().model_name().into(),
            t => t.name().into(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            NHTab::Hierarchy => self.hierarchy(ui),
            NHTab::Toolbar => self.toolbar(ui),
            NHTab::Properties => self.properties(ui),
            NHTab::Layers => self.layers(ui),
            NHTab::StyleEditor => self.style_editor_tab(ui),
            NHTab::Diagram{uuid} => self.diagram_tab(uuid, ui),
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
        if let NHTab::Diagram{uuid} = tab {
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

impl MyContext {
    fn hierarchy(&mut self, ui: &mut Ui) {
        for (_uiid, controller) in &self.diagram_controllers {
            controller.list_in_project_hierarchy(ui);
        }
    }
    
    fn toolbar(&mut self, ui: &mut Ui) {
        if let Some(last_focused_diagram) = &self.last_focused_diagram {
            self.diagram_controllers.get_mut(last_focused_diagram)
                .map(|c| c.show_toolbar(ui));
        }
    }
    
    fn properties(&mut self, ui: &mut Ui) {
        if let Some(last_focused_diagram) = &self.last_focused_diagram {
            self.diagram_controllers.get_mut(last_focused_diagram)
                .map(|c| c.show_properties(ui));
        }
    }
    
    fn layers(&mut self, ui: &mut Ui) {
        if let Some(last_focused_diagram) = &self.last_focused_diagram {
            self.diagram_controllers.get_mut(last_focused_diagram)
                .map(|c| c.show_layers(ui));
        }
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
                egui::color_picker::color_edit_button_srgba(ui, &mut style.separator.color_idle, egui::color_picker::Alpha::OnlyBlend);
                ui.end_row();

                ui.label("Hovered color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut style.separator.color_hovered, egui::color_picker::Alpha::OnlyBlend);
                ui.end_row();

                ui.label("Dragged color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut style.separator.color_dragged, egui::color_picker::Alpha::OnlyBlend);
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
                    egui::color_picker::color_edit_button_srgba(ui, &mut tab_style.text_color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Outline color:")
                        .on_hover_text("The outline around the active tab name.");
                    egui::color_picker::color_edit_button_srgba(ui, &mut tab_style.outline_color, egui::color_picker::Alpha::OnlyBlend);
                    ui.end_row();

                    ui.label("Background color:");
                    egui::color_picker::color_edit_button_srgba(ui, &mut tab_style.bg_fill, egui::color_picker::Alpha::OnlyBlend);
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
                egui::color_picker::color_edit_button_srgba(ui, &mut style.buttons.close_tab_color, egui::color_picker::Alpha::OnlyBlend);
                ui.end_row();

                ui.label("Close button color focused:");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut style.buttons.close_tab_active_color,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                ui.end_row();

                ui.label("Close button background color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut style.buttons.close_tab_bg_fill, egui::color_picker::Alpha::OnlyBlend);
                ui.end_row();

                ui.label("Bar background color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut style.tab_bar.bg_fill, egui::color_picker::Alpha::OnlyBlend);
                ui.end_row();

                ui.label("Horizontal line color:").on_hover_text(
                    "The line separating the tab name area from the tab content area",
                );
                egui::color_picker::color_edit_button_srgba(ui, &mut style.tab_bar.hline_color, egui::color_picker::Alpha::OnlyBlend);
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
                egui::color_picker::color_edit_button_srgba(ui, &mut style.tab.tab_body.stroke.color, egui::color_picker::Alpha::OnlyBlend);
                ui.end_row();

                ui.label("Background color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut style.tab.tab_body.bg_fill, egui::color_picker::Alpha::OnlyBlend);
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
            })
        });
    }
    
    // In general it should draw first and handle input second, right?
    fn diagram_tab(&mut self, uuid: &uuid::Uuid, ui: &mut Ui) {
        let diagram_controller = self.diagram_controllers.get_mut(uuid).unwrap();
        
        let (mut ui_canvas, response, pos) = diagram_controller.new_ui_canvas(ui);
        
        if response.clicked() || response.drag_started() {
            self.last_focused_diagram = Some(uuid.clone());
        }
        
        diagram_controller.draw_in(ui_canvas.as_mut(), pos);
        
        diagram_controller.handle_input(ui, &response);
        
        response.context_menu(|ui| diagram_controller.context_menu(ui));
    }
    
    fn last_focused_diagram(&mut self) -> Option<&mut Box<dyn DiagramController>> {
        self.last_focused_diagram.as_ref().and_then(|e| self.diagram_controllers.get_mut(e))
    }
}

impl Default for MyApp {
    fn default() -> Self {
        let (rdf_uuid, rdf_demo) = crate::rdf::rdf_controllers::demo(1);
        let (umlclass_uuid, umlclass_demo) = crate::umlclass::umlclass_controllers::demo(2);
        
        let mut diagram_controllers = HashMap::new();
        diagram_controllers.insert(rdf_uuid.clone(), rdf_demo);
        diagram_controllers.insert(umlclass_uuid.clone(), umlclass_demo);
        
        let mut dock_state =
            DockState::new(vec![NHTab::StyleEditor,
                                NHTab::Diagram{uuid: rdf_uuid},
                                NHTab::Diagram{uuid: umlclass_uuid}]);
        "Undock".clone_into(&mut dock_state.translations.tab_context_menu.eject_button);
        
        let mut open_unique_tabs = HashSet::new();
        open_unique_tabs.insert(NHTab::StyleEditor);
        
        let [a, b] = dock_state.main_surface_mut().split_left(
            NodeIndex::root(),
            0.2,
            vec![NHTab::Hierarchy],
        );
        open_unique_tabs.insert(NHTab::Hierarchy);
        let [_, _] = dock_state.main_surface_mut().split_right(
            a,
            0.7,
            vec![NHTab::Properties],
        );
        open_unique_tabs.insert(NHTab::Properties);
        let [_, _] = dock_state.main_surface_mut().split_below(
            b,
            0.7,
            vec![NHTab::Toolbar],
        );
        open_unique_tabs.insert(NHTab::Toolbar);
        
        let context = MyContext {
            diagram_controllers,
            new_diagram_no: 3,
            style: None,
            
            open_unique_tabs,
            last_focused_diagram: None,

            show_window_close: true,
            show_window_collapse: true,
            show_close_buttons: true,
            show_add_buttons: false,
            draggable_tabs: true,
            show_tab_name_on_hover: false,
            allowed_splits: AllowedSplits::default(),
        };

        Self {
            context,
            tree: dock_state,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("egui_dock::MenuBar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    /*
                    if ui.button("New Project").clicked() {
                        println!("yes");
                    }
                    if ui.button("Open Project").clicked() {
                        println!("yes");
                    }
                    ui.menu_button("Recent Projects", |ui| {
                        if ui.button("asdf").clicked() {
                            println!("yes");
                        }
                    });
                    ui.separator();
                    */
                    
                    ui.menu_button("Add New Diagram", |ui| {
                        type NDC = fn(u32) -> (uuid::Uuid, Box<(dyn DiagramController + 'static)>);
                        for (label, fun) in [("Add New UML class diagram", crate::umlclass::umlclass_controllers::new as NDC),
                                             //("Add New OntoUML diagram"),
                                             ("Add New RDF diagram", crate::rdf::rdf_controllers::new as NDC),] {
                            if ui.button(label).clicked() {
                                let (uuid, diagram_controller) = fun(self.context.new_diagram_no);
                                self.context.new_diagram_no += 1;
                                self.context.diagram_controllers.insert(uuid.clone(), diagram_controller);
                                
                                let tab = NHTab::Diagram{uuid};
                                
                                self.tree[SurfaceIndex::main()]
                                    .push_to_focused_leaf(tab);
                                // TODO: set as last_focused_diagram
                                
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
                
                ui.menu_button("View", |_ui| {
                    /*
                    if ui.button("Reset").clicked() {
                        println!("no");
                    }
                    */
                });
                
                ui.menu_button("Diagram", |ui| {
                    self.context.last_focused_diagram().map(|e| {
                        ui.menu_button(format!("Export Diagram `{}` to", e.model_name()), |ui| {
                            if ui.button("SVG").clicked() {
                                // NOTE: This does not work on WASM, and in its current state it never will.
                                //       This will be possible to fix once this is fixed on rfd side (#128).
                                if let Some(path) = rfd::FileDialog::new()
                                                    .set_directory(std::env::current_dir().unwrap())
                                                    .add_filter("SVG files", &["svg"])
                                                    .add_filter("All files", &["*"])
                                                    .save_file() {
                                    let mut canvas = SVGCanvas::new(ui.painter());
                                    e.draw_in(&mut canvas, None);
                                    let _ = canvas.save_to(path);
                                }
                                ui.close_menu();
                            }
                            /*
                            if ui.button("PNG").clicked() {
                                println!("yes");
                                ui.close_menu();
                            }
                            if ui.button("PDF").clicked() {
                                println!("yes");
                                ui.close_menu();
                            }
                            */
                        })
                    });
                });
                
                ui.menu_button("Windows", |ui| {
                    // allow certain tabs to be toggled
                    for tab in &[NHTab::Hierarchy, NHTab::Toolbar, NHTab::Properties,
                                 NHTab::Layers, NHTab::StyleEditor] {
                        if ui
                            .selectable_label(
                                self.context.open_unique_tabs.contains(tab),
                                tab.name()
                            )
                            .clicked()
                        {
                            if let Some(index) = self.tree.find_tab(tab) {
                                self.tree.remove_tab(index);
                                self.context.open_unique_tabs.remove(tab);
                            } else {
                                self.tree[SurfaceIndex::main()]
                                    .push_to_focused_leaf(tab.clone());
                                self.context.open_unique_tabs.insert(tab.clone());
                            }

                            ui.close_menu();
                        }
                    }
                });
            })
        });
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
                    .show_window_close_buttons(self.context.show_window_close)
                    .show_window_collapse_buttons(self.context.show_window_collapse)
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
