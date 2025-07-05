// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::io::Write;

use common::canvas::{NHCanvas, UiCanvas};
use common::controller::{ColorLabels, ColorProfile, DrawingContext, HierarchyNode, ModelHierarchyView, ProjectCommand, SimpleModelHierarchyView, SimpleProjectCommand};
use common::project_serde::{NHProjectHierarchyNodeDTO, NHSerializeError, NHSerializer};
use common::uuid::{ModelUuid, ViewUuid};
use eframe::egui::{
    self, vec2, CentralPanel, Frame, Slider, TopBottomPanel, Ui, ViewportBuilder, WidgetText,
};
use eframe::NativeOptions;

use egui_dock::{AllowedSplits, DockArea, DockState, NodeIndex, Style, SurfaceIndex, TabViewer};
use egui_ltreeview::{NodeBuilder, TreeView, TreeViewState};

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
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let options = NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size(vec2(1024.0, 1024.0)),
        ..Default::default()
    };
    eframe::run_native("113", options, Box::new(|_cc| Ok(Box::<NHApp>::default())))
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
enum NHTab {
    RecentlyUsed,
    StyleEditor,

    ProjectHierarchy,
    ModelHierarchy,

    Toolbar,
    Properties,
    Layers,

    Diagram { uuid: ViewUuid },
    CustomTab { uuid: uuid::Uuid },
}

impl NHTab {
    pub fn name(&self) -> &str {
        match self {
            NHTab::RecentlyUsed => "Recently Used",
            NHTab::StyleEditor => "Style Editor",

            NHTab::ProjectHierarchy => "Project Hierarchy",
            NHTab::ModelHierarchy => "Model Hierarchy",

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
    project_path: Option<std::path::PathBuf>,
    pub diagram_controllers: HashMap<ViewUuid, (usize, Arc<RwLock<dyn DiagramController>>)>,
    project_hierarchy: HierarchyNode,
    tree_view_state: TreeViewState<ViewUuid>,
    model_hierarchy_views: HashMap<ModelUuid, Arc<dyn ModelHierarchyView>>,
    new_diagram_no: u32,
    pub custom_tabs: HashMap<uuid::Uuid, Arc<RwLock<dyn CustomTab>>>,

    pub style: Option<Style>,
    color_profiles: Vec<(String, ColorLabels, Vec<ColorProfile>)>,
    selected_color_profiles: Vec<usize>,
    selected_language: usize,
    languages_order: Vec<unic_langid::LanguageIdentifier>,
    fluent_bundle: fluent_bundle::FluentBundle<fluent_bundle::FluentResource>,

    undo_stack: Vec<(Arc<String>, ViewUuid)>,
    redo_stack: Vec<(Arc<String>, ViewUuid)>,
    unprocessed_commands: Vec<ProjectCommand>,
    has_unsaved_changes: bool,
    
    shortcuts: HashMap<SimpleProjectCommand, egui::KeyboardShortcut>,
    shortcut_top_order: Vec<(SimpleProjectCommand, egui::KeyboardShortcut)>,

    open_unique_tabs: HashSet<NHTab>,
    last_focused_diagram: Option<ViewUuid>,
    svg_export_menu: Option<(usize, Arc<RwLock<dyn DiagramController>>, std::path::PathBuf, usize, bool, bool, f32, f32)>,
    confirm_modal_reason: Option<SimpleProjectCommand>,
    shortcut_being_set: Option<SimpleProjectCommand>,

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
            NHTab::RecentlyUsed => {
                // TODO: show recently used projects
                ui.heading("Recently used");
                ui.label("[no recently used]");
            },
            NHTab::StyleEditor => self.style_editor_tab(ui),

            NHTab::ProjectHierarchy => self.project_hierarchy(ui),
            NHTab::ModelHierarchy => self.model_hierarchy(ui),

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

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        self.open_unique_tabs.remove(tab);
        true
    }
}

impl NHContext {
    fn export_project(&self) -> Result<String, NHSerializeError> {
        let HierarchyNode::Folder(.., children) = &self.project_hierarchy else {
            return Err(NHSerializeError::StructureError("invalid hierarchy root".into()))
        };

        fn h(e: &HierarchyNode) -> NHProjectHierarchyNodeDTO {
            match e {
                HierarchyNode::Folder(uuid, _, children)
                    => NHProjectHierarchyNodeDTO::Folder { uuid: *uuid, hierarchy: children.iter().map(h).collect() },
                HierarchyNode::Diagram(rw_lock)
                    => NHProjectHierarchyNodeDTO::Diagram { uuid: *rw_lock.read().unwrap().uuid() },
            }
        }

        let mut serializer = NHSerializer::new();
        for e in self.diagram_controllers.iter() {
            e.1.1.read().unwrap().serialize_into(&mut serializer)?;
        }

        let project = common::project_serde::NHProjectDTO::new(
            children.iter().map(h).collect(),
            serializer,
        );

        Ok(toml::to_string(&project)?)
    }
    fn clear_project_data(&mut self) {
        self.project_path = None;
        self.diagram_controllers.clear();
        self.project_hierarchy = HierarchyNode::Folder(uuid::Uuid::nil().into(), "root".to_owned().into(), vec![]);
        self.new_diagram_no = 1;
        self.custom_tabs.clear();

        self.undo_stack.clear();
        self.redo_stack.clear();
        self.unprocessed_commands.clear();
        self.has_unsaved_changes = false;

        self.last_focused_diagram = None;
        self.svg_export_menu = None;
        self.confirm_modal_reason = None;
    }

    fn sort_shortcuts(&mut self) {
        self.shortcut_top_order = self.shortcuts.iter().map(|(&k,&v)|(k,v)).collect();
        
        fn weight(m: &egui::KeyboardShortcut) -> u32 {
            m.modifiers.alt as u32 + m.modifiers.command as u32 + m.modifiers.shift as u32
        }
        
        self.shortcut_top_order.sort_by(|a, b| weight(&b.1).cmp(&weight(&a.1)));
    }

    fn project_hierarchy(&mut self, ui: &mut Ui) {
        enum ContextMenuAction {
            NewFolder(ViewUuid),
            RecCollapseAt(bool, ViewUuid),
            DeleteFolder(ViewUuid),
            OpenDiagram(ViewUuid),
            DuplicateDeep(ViewUuid),
            DuplicateShallow(ViewUuid),
            DeleteDiagram(ViewUuid),
        }

        let mut context_menu_action = None;

        ui.horizontal(|ui| {
            if ui.button("New folder").clicked() {
                context_menu_action = Some(ContextMenuAction::NewFolder(uuid::Uuid::nil().into()));
            }
            if ui.button("Collapse all").clicked() {
                context_menu_action = Some(ContextMenuAction::RecCollapseAt(true, uuid::Uuid::nil().into()));
            }
            if ui.button("Uncollapse all").clicked() {
                context_menu_action = Some(ContextMenuAction::RecCollapseAt(false, uuid::Uuid::nil().into()));
            }
        });

        fn hierarchy(builder: &mut egui_ltreeview::TreeViewBuilder<ViewUuid>, hn: &HierarchyNode, cma: &mut Option<ContextMenuAction>) {
            match hn {
                HierarchyNode::Folder(uuid, name, children) => {
                    builder.node(
                        NodeBuilder::dir(*uuid)
                            .label(format!("{} ({})", name, uuid.to_string()))
                            .context_menu(|ui| {
                                if ui.button("New Folder").clicked() {
                                    *cma = Some(ContextMenuAction::NewFolder(*uuid));
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Collapse children").clicked() {
                                    *cma = Some(ContextMenuAction::RecCollapseAt(true, *uuid));
                                    ui.close_menu();
                                }
                                if ui.button("Uncollapse children").clicked() {
                                    *cma = Some(ContextMenuAction::RecCollapseAt(false, *uuid));
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    *cma = Some(ContextMenuAction::DeleteFolder(*uuid));
                                    ui.close_menu();
                                }
                            })
                    );

                    for c in children {
                        hierarchy(builder, c, cma);
                    }

                    builder.close_dir();
                },
                HierarchyNode::Diagram(rw_lock) => {
                    let hm = rw_lock.read().unwrap();
                    builder.node(
                        NodeBuilder::leaf(*hm.uuid())
                            .label(format!("{} ({})", hm.model_name(), hm.uuid().to_string()))
                            .context_menu(|ui| {
                                if ui.button("Open").clicked() {
                                    *cma = Some(ContextMenuAction::OpenDiagram(*hm.uuid()));
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Duplicate (deep)").clicked() {
                                    *cma = Some(ContextMenuAction::DuplicateDeep(*hm.uuid()));
                                    ui.close_menu();
                                }
                                if ui.button("Duplicate (shallow)").clicked() {
                                    *cma = Some(ContextMenuAction::DuplicateShallow(*hm.uuid()));
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    *cma = Some(ContextMenuAction::DeleteDiagram(*hm.uuid()));
                                    ui.close_menu();
                                }
                            })
                    );
                },
            }
        }

        let mut commands = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let id = ui.make_persistent_id("Project Hierarchy Tree View");
                let (response, actions) = TreeView::new(id).show_state(
                    ui,
                    &mut self.tree_view_state,
                    |builder| {
                        hierarchy(builder, &self.project_hierarchy, &mut context_menu_action);
                    }
                );

                for action in actions.into_iter() {
                    match action {
                        egui_ltreeview::Action::Move(dnd) => {
                            let target_is_folder = matches!(self.project_hierarchy.get(&dnd.target), Some((HierarchyNode::Folder(..), _)));

                            for source_id in &dnd.source {
                                if let Some((source_node, source_node_parent)) = self.project_hierarchy.get(source_id) {
                                    if (target_is_folder && matches!(source_node, HierarchyNode::Folder(..) | HierarchyNode::Diagram(..)))
                                        || dnd.target == source_node_parent.uuid() {
                                        if let Some(source) = self.project_hierarchy.remove(source_id) {
                                            _ = self.project_hierarchy.insert(&dnd.target, dnd.position, source);
                                        }
                                    }
                                }
                            }
                        }
                        egui_ltreeview::Action::Activate(a) => {
                            for selected in &a.selected {
                                match self.project_hierarchy.get(selected) {
                                    Some((HierarchyNode::Diagram(..), _)) => {
                                        commands.push(ProjectCommand::OpenAndFocusDiagram(*selected));
                                    }
                                    // TODO: jump to activated Element/CompositeElement
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });

        if let Some(c) = context_menu_action {
            match c {
                ContextMenuAction::NewFolder(view_uuid) => {
                    self.project_hierarchy.insert(
                        &view_uuid,
                        egui_ltreeview::DirPosition::Last,
                        HierarchyNode::Folder(uuid::Uuid::now_v7().into(), Arc::new("New folder".into()), vec![]),
                    );
                },
                ContextMenuAction::RecCollapseAt(b, view_uuid) => {
                    if let Some(e) = self.project_hierarchy.get(&view_uuid) {
                        e.0.for_each(|e| self.tree_view_state.set_openness(&e.uuid(), !b));
                    }
                },
                ContextMenuAction::DeleteFolder(view_uuid) => {
                    self.project_hierarchy.remove(&view_uuid);
                },
                ContextMenuAction::OpenDiagram(view_uuid) => commands.push(ProjectCommand::OpenAndFocusDiagram(view_uuid)),
                ContextMenuAction::DuplicateDeep(view_uuid) => commands.push(ProjectCommand::CopyDiagram(view_uuid, true)),
                ContextMenuAction::DuplicateShallow(view_uuid) => commands.push(ProjectCommand::CopyDiagram(view_uuid, false)),
                ContextMenuAction::DeleteDiagram(view_uuid) => commands.push(ProjectCommand::DeleteDiagram(view_uuid)),
            }
        }

        self.unprocessed_commands.extend(commands.into_iter());
    }

    fn model_hierarchy(&mut self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
        let model_uuid = c.read().unwrap().model_uuid();
        let Some(model_hierarchy_view) = self.model_hierarchy_views.get(&model_uuid) else { return; };
        model_hierarchy_view.show_model_hierarchy(ui, c.read().unwrap().represented_models());
    }

    fn toolbar(&self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
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
            self.has_unsaved_changes = true;
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
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
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
                corner_radius_ui(ui, &mut style.main_surface_border_rounding);
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
                    Slider::new(&mut tab_style.corner_radius.nw, 0..=15),
                    "North-West"
                );
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.corner_radius.ne, 0..=15),
                    "North-East"
                );
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.corner_radius.sw, 0..=15),
                    "South-West"
                );
                labeled_widget!(
                    ui,
                    Slider::new(&mut tab_style.corner_radius.se, 0..=15),
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
            corner_radius_ui(ui, &mut style.tab.tab_body.corner_radius);

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
            ui.collapsing("Feel", |ui| {
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

            ui.collapsing("Visuals", |ui| {
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

            ui.collapsing("Hover highlight", |ui| {
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
                corner_radius_ui(ui, &mut style.overlay.hovered_leaf_highlight.corner_radius);
            });
        });
        
        ui.collapsing("Diagram themes", |ui| {
            for (idx1, (name, l, p)) in self.color_profiles.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label(&*name)
                        .selected_text(&p[self.selected_color_profiles[idx1]].name)
                        .show_ui(ui, |ui| {
                            for (idx2, profile) in p.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_color_profiles[idx1], idx2, &profile.name);
                            }
                        }
                    );
                    if ui.button("Duplicate as a new color profile").clicked() {
                        let current = &p[self.selected_color_profiles[idx1]];
                        p.push(ColorProfile {
                            name: format!("{} (copy)", current.name),
                            backgrounds: current.backgrounds.clone(),
                            foregrounds: current.foregrounds.clone(),
                            auxiliary: current.auxiliary.clone(),
                        });
                    }
                });

                egui::CollapsingHeader::new("Color editor").id_salt(("Color editor", idx1)).show(ui, |ui| {
                    let current = &mut p[self.selected_color_profiles[idx1]];
                    let color_editor_block = |ui: &mut Ui, name: &str, labels: &[Option<String>], colors: &mut [egui::Color32]| {
                        egui::CollapsingHeader::new(name).id_salt((name, idx1)).show(ui, |ui| {
                            egui::Grid::new((name, idx1, "grid")).show(ui, |ui| {
                                for (l, c) in labels.iter().flatten().zip(colors.iter_mut()) {
                                    ui.label(l);
                                    ui.horizontal(|ui| {
                                        egui::widgets::color_picker::color_edit_button_srgba(
                                            ui,
                                            c,
                                            egui::widgets::color_picker::Alpha::OnlyBlend
                                        );
                                    });
                                    ui.end_row();
                                }
                            });
                        });
                    };

                    color_editor_block(ui, "Background colors", &l.backgrounds, &mut current.backgrounds);
                    color_editor_block(ui, "Foreground colors", &l.foregrounds, &mut current.foregrounds);
                    color_editor_block(ui, "Auxiliary colors", &l.auxiliary, &mut current.auxiliary);
                });
            }
        });

        ui.collapsing("Languages", |ui| {
            for (idx, l) in self.languages_order.iter().enumerate() {
                let text = if idx == self.selected_language { format!("[{}]", l) } else { l.to_string() };
                if ui.add(egui::Label::new(text).sense(egui::Sense::click())).clicked() {
                    self.selected_language = idx;
                };
            }

            if ui.add_enabled(self.selected_language > 0, egui::Button::new("Up")).clicked() {
                self.languages_order.swap(self.selected_language, self.selected_language - 1);
                self.selected_language -= 1;
                self.fluent_bundle = common::fluent::create_fluent_bundle(&self.languages_order).unwrap();
            }

            if ui.add_enabled(self.selected_language + 1 < self.languages_order.len(), egui::Button::new("Down")).clicked() {
                self.languages_order.swap(self.selected_language, self.selected_language + 1);
                self.selected_language += 1;
                self.fluent_bundle = common::fluent::create_fluent_bundle(&self.languages_order).unwrap();
            }
        });

        ui.collapsing("Keyboard shortcuts", |ui| {
            egui::Grid::new("shortcut editor grid").show(ui, |ui| {
                for (l, c) in &[("Swap top languages:", SimpleProjectCommand::SwapTopLanguages),
                                ("Save project:", SimpleProjectCommand::SaveProject),
                                ("Save project as:", SimpleProjectCommand::SaveProjectAs),
                               ] {
                    ui.label(*l);
                    let sc = self.shortcuts.get(c);
                    ui.horizontal(|ui| {
                        if let Some(sc) = sc {
                            ui.label(ui.ctx().format_shortcut(sc));
                        }
                    });

                    if self.shortcut_being_set.is_none_or(|e| e != *c) {
                        if ui.button("Set").clicked() {
                            self.shortcut_being_set = Some(*c);
                        }
                    } else {
                        if ui.button("Cancel").clicked() {
                            self.shortcut_being_set = None;
                        }
                    }

                    if sc.is_some() && ui.button("Delete").clicked() {
                        self.shortcuts.remove(c);
                    }
                    ui.end_row();
                }
            });
        });
    }
    
    // In general it should draw first and handle input second, right?
    fn diagram_tab(&mut self, tab_uuid: &ViewUuid, ui: &mut Ui) {
        let Some((t, arc)) = self.diagram_controllers.get(tab_uuid) else { return; };
        let mut diagram_controller = arc.write().unwrap();

        let drawing_context = DrawingContext {
            profile: &self.color_profiles[*t].2[self.selected_color_profiles[*t]],
            fluent_bundle: &self.fluent_bundle,
        };
        let (mut ui_canvas, response, pos) = diagram_controller.new_ui_canvas(&drawing_context, ui);

        diagram_controller.draw_in(&drawing_context, ui_canvas.as_mut(), pos);

        let mut undo_accumulator = Vec::<Arc<String>>::new();
        diagram_controller.handle_input(ui, &response, &mut undo_accumulator);
        response.context_menu(|ui| diagram_controller.context_menu(ui));
        if !undo_accumulator.is_empty() {
            self.has_unsaved_changes = true;
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

    fn last_focused_diagram(&self) -> Option<(usize, Arc<RwLock<dyn DiagramController>>)> {
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
        let mut hierarchy = vec![];
        let mut model_hierarchy_views = HashMap::<_, Arc<dyn ModelHierarchyView>>::new();
        let mut tabs = vec![NHTab::RecentlyUsed, NHTab::StyleEditor];

        for (diagram_type, (controller, mhview)) in [
            (0, crate::rdf::rdf_controllers::demo(1)),
            (1, crate::umlclass::umlclass_controllers::demo(2)),
            (2, crate::democsd::democsd_controllers::demo(3)),
        ] {
            let uuid = *controller.read().unwrap().uuid();
            hierarchy.push(HierarchyNode::Diagram(controller.clone()));
            diagram_controllers.insert(uuid, (diagram_type, controller.clone()));
            model_hierarchy_views.insert(*controller.read().unwrap().model_uuid(), mhview);
            tabs.push(NHTab::Diagram { uuid });
        }

        let mut dock_state = DockState::new(tabs);
        "Undock".clone_into(&mut dock_state.translations.tab_context_menu.eject_button);

        let mut open_unique_tabs = HashSet::new();
        open_unique_tabs.insert(NHTab::RecentlyUsed);
        open_unique_tabs.insert(NHTab::StyleEditor);

        let [a, b] = dock_state.main_surface_mut().split_left(
            NodeIndex::root(),
            0.2,
            vec![NHTab::ProjectHierarchy, NHTab::ModelHierarchy],
        );
        open_unique_tabs.insert(NHTab::ProjectHierarchy);
        open_unique_tabs.insert(NHTab::ModelHierarchy);
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
        let languages_order = common::fluent::AVAILABLE_LANGUAGES.iter().map(|e| e.0.clone()).collect();
        let fluent_bundle = common::fluent::create_fluent_bundle(&languages_order)
            .expect("Could not establish base FluentBundle");
        
        let mut context = NHContext {
            project_path: None,
            diagram_controllers,
            project_hierarchy: HierarchyNode::Folder(uuid::Uuid::nil().into(), Arc::new("root".to_owned()), hierarchy),
            tree_view_state: TreeViewState::default(),
            model_hierarchy_views,
            new_diagram_no: 4,
            custom_tabs: HashMap::new(),
            
            style: None,
            color_profiles,
            selected_color_profiles,
            selected_language: 0,
            languages_order,
            fluent_bundle,
            
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            unprocessed_commands: Vec::new(),
            has_unsaved_changes: true,
            
            shortcuts: HashMap::new(),
            shortcut_top_order: vec![],

            open_unique_tabs,
            last_focused_diagram: None,
            svg_export_menu: None,
            confirm_modal_reason: None,
            shortcut_being_set: None,

            show_window_close: true,
            show_window_collapse: true,
            show_close_buttons: true,
            show_add_buttons: false,
            draggable_tabs: true,
            show_tab_name_on_hover: false,
            allowed_splits: AllowedSplits::default(),
        };
        
        context.shortcuts.insert(SimpleProjectCommand::SwapTopLanguages, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L));
        context.shortcuts.insert(SimpleProjectCommand::OpenProject(false), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O));
        context.shortcuts.insert(SimpleProjectCommand::SaveProject, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S));
        context.shortcuts.insert(SimpleProjectCommand::SaveProjectAs, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S));
        context.shortcuts.insert(DiagramCommand::UndoImmediate.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z));
        context.shortcuts.insert(DiagramCommand::RedoImmediate.into(), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ));
        context.shortcuts.insert(DiagramCommand::SelectAllElements(true).into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A));
        context.shortcuts.insert(DiagramCommand::SelectAllElements(false).into(), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::A,
        ));
        context.shortcuts.insert(DiagramCommand::InvertSelection.into(), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::I,
        ));
        context.shortcuts.insert(DiagramCommand::CutSelectedElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::X));
        context.shortcuts.insert(DiagramCommand::CopySelectedElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::C));
        context.shortcuts.insert(DiagramCommand::PasteClipboardElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::V));
        context.shortcuts.insert(DiagramCommand::DeleteSelectedElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete));
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
        self.context.has_unsaved_changes = true;
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
        self.context.has_unsaved_changes = true;
    }

    fn add_diagram(
        &mut self,
        diagram_type: usize,
        diagram: Arc<RwLock<dyn DiagramController>>,
        hierarchy_view: Arc<dyn ModelHierarchyView>,
    ) {
        let view_uuid = *diagram.read().unwrap().uuid();
        let model_uuid = *diagram.read().unwrap().model_uuid();
        if let HierarchyNode::Folder(.., children) = &mut self.context.project_hierarchy {
            children.push(HierarchyNode::Diagram(diagram.clone()));
        }
        self.context
            .diagram_controllers
            .insert(view_uuid, (diagram_type, diagram));
        self.context.model_hierarchy_views.insert(model_uuid, hierarchy_view);

        let tab = NHTab::Diagram { uuid: view_uuid };
        self.tree[SurfaceIndex::main()].push_to_focused_leaf(tab);
    }
    pub fn add_custom_tab(&mut self, uuid: uuid::Uuid, tab: Arc<RwLock<dyn CustomTab>>) {
        self.context.custom_tabs.insert(uuid, tab);

        let tab = NHTab::CustomTab { uuid };

        self.tree[SurfaceIndex::main()].push_to_focused_leaf(tab);
    }
}

fn new_project() -> Result<(), &'static str> {
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
        // Process ProjectCommands
        let mut commands = vec![];
        for c in self.context.unprocessed_commands.drain(..) {
            match c {
                ProjectCommand::OpenAndFocusDiagram(uuid) => {
                    let target_tab = NHTab::Diagram { uuid };
                    if let Some(t) = self.tree.find_tab(&target_tab) {
                        self.tree.set_focused_node_and_surface((t.0, t.1));
                        self.tree.set_active_tab(t);
                    } else {
                        if let Some(t) = self.context.last_focused_diagram
                            .and_then(|e| self.tree.find_tab(&NHTab::Diagram { uuid: e })) {
                            self.tree.set_focused_node_and_surface((t.0, t.1));
                            self.tree.set_active_tab(t);
                        }
                        self.tree[SurfaceIndex::main()].push_to_focused_leaf(target_tab);
                    }
                },
                other => commands.push(other),
            }
        }
        
        // Set self.context.last_focused_diagram
        if let Some((_, NHTab::Diagram { uuid })) = self.tree.find_active_focused() {
            self.context.last_focused_diagram = Some(*uuid);
        }

        // Set window title depending on the project path
        let modified = if self.context.has_unsaved_changes { "*" } else { "" };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            if let Some(project_path) = &self.context.project_path {
                format!("113{} - {}", modified, project_path.to_string_lossy())
            } else {
                format!("113{}", modified)
            }
        ));

        macro_rules! translate {
            ($msg_name:expr) => {
                self.context.fluent_bundle.format_pattern(
                    self.context.fluent_bundle.get_message($msg_name).unwrap().value().unwrap(),
                    None,
                    &mut vec![],
                )
            };
        }

        macro_rules! shortcut_text {
            ($ui:expr, $simple_project_command:expr) => {
                self.context.shortcuts.get(&$simple_project_command).map(|e| $ui.ctx().format_shortcut(&e))
            };
        }

        macro_rules! button {
            ($ui:expr, $msg_name:expr, $simple_project_command:expr) => {
                {
                    let mut button = egui::Button::new(translate!($msg_name));
                    if let Some(shortcut_text) = shortcut_text!($ui, $simple_project_command) {
                        button = button.shortcut_text(shortcut_text);
                    }
                    if $ui.add(button).clicked() {
                        commands.push($simple_project_command.into());
                        $ui.close_menu();
                    }
                }
            };
        }

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

        // Show ui
        TopBottomPanel::top("egui_dock::MenuBar").show(ctx, |ui| {
            // Check diagram-handled shortcuts
            ui.input(|is|
                'outer: for e in is.events.iter() {
                    match e {
                        egui::Event::Cut => send_to_focused_diagram!(DiagramCommand::CutSelectedElements),
                        egui::Event::Copy => send_to_focused_diagram!(DiagramCommand::CopySelectedElements),
                        egui::Event::Paste(a) => send_to_focused_diagram!(DiagramCommand::PasteClipboardElements),
                        egui::Event::Key { key, pressed, modifiers, .. } => {
                            if !pressed {continue;}

                            if let Some(sc) = &self.context.shortcut_being_set {
                                self.context.shortcuts.insert(*sc, egui::KeyboardShortcut { logical_key: *key, modifiers: *modifiers });
                                self.context.shortcut_being_set = None;
                                self.context.sort_shortcuts();
                                continue;
                            }

                            'inner: for ksh in &self.context.shortcut_top_order {
                                if !(modifiers.matches_logically(ksh.1.modifiers) && *key == ksh.1.logical_key) {
                                    continue 'inner;
                                }
                                
                                match ksh.0 {
                                    e @ SimpleProjectCommand::DiagramCommand(dc) => match dc {
                                        DiagramCommand::DropRedoStackAndLastChangeFlag
                                        | DiagramCommand::SetLastChangeFlag => unreachable!(),
                                        DiagramCommand::UndoImmediate => self.undo_immediate(),
                                        DiagramCommand::RedoImmediate => self.redo_immediate(),
                                        _ => commands.push(e.into())
                                    },
                                    other => commands.push(other.into()),
                                }
                                
                                break 'outer;
                            }
                        }
                        _ => {}
                    }
                    
                }
            );
            
            // Menubar UI
            egui::menu::bar(ui, |ui| {
                ui.menu_button(translate!("nh-project"), |ui| {
                    if ui.button(translate!("nh-project-newproject")).clicked() {
                        let _ = new_project();
                    }

                    button!(ui, "nh-project-openproject", SimpleProjectCommand::OpenProject(false));

                    // TODO: implement
                    ui.menu_button(translate!("nh-project-recentprojects"), |ui| {
                        if ui.button("asdf").clicked() {
                            println!("TODO");
                        }
                    });
                    ui.separator();

                    ui.menu_button(translate!("nh-project-addnewdiagram"), |ui| {
                        type NDC =
                            fn(u32) -> (Arc<RwLock<(dyn DiagramController + 'static)>>, Arc<dyn ModelHierarchyView>);
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
                                let (diagram_controller, mhview) = fun(self.context.new_diagram_no);
                                commands.push(ProjectCommand::SetNewDiagramNumber(self.context.new_diagram_no + 1));
                                commands.push(ProjectCommand::AddNewDiagram(diagram_type, diagram_controller, mhview));
                                // TODO: use mhview
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();

                    button!(ui, "nh-project-save", SimpleProjectCommand::SaveProject);
                    button!(ui, "nh-project-saveas", SimpleProjectCommand::SaveProjectAs);
                    ui.separator();
                    button!(ui, "nh-project-closeproject", SimpleProjectCommand::CloseProject(false));
                    #[cfg(not(target_arch = "wasm32"))]
                    button!(ui, "nh-project-exit", SimpleProjectCommand::Exit(false));
                });

                ui.menu_button(translate!("nh-edit"), |ui| {
                    ui.menu_button(translate!("nh-edit-undo"), |ui| {
                        let shortcut_text = shortcut_text!(ui, DiagramCommand::UndoImmediate.into());
                        
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
                                        commands.push(SimpleProjectCommand::DiagramCommand(DiagramCommand::UndoImmediate).into());
                                    }
                                    break;
                                }
                            }
                        }
                        
                    });
                    
                    ui.menu_button(translate!("nh-edit-redo"), |ui| {
                        let shortcut_text = shortcut_text!(ui, DiagramCommand::RedoImmediate.into());
                        
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
                                        commands.push(SimpleProjectCommand::DiagramCommand(DiagramCommand::RedoImmediate).into());
                                    }
                                    break;
                                }
                            }
                        }
                    });
                    ui.separator();

                    button!(ui, "nh-edit-cut", SimpleProjectCommand::from(DiagramCommand::CutSelectedElements));
                    button!(ui, "nh-edit-copy", SimpleProjectCommand::from(DiagramCommand::CopySelectedElements));
                    button!(ui, "nh-edit-paste", SimpleProjectCommand::from(DiagramCommand::PasteClipboardElements));
                    ui.separator();

                    if let Some((_t, d)) = self.context.last_focused_diagram() {
                        let mut d = d.write().unwrap();
                        d.show_menubar_edit_options(ui, &mut commands);
                    }
                });

                ui.menu_button(translate!("nh-view"), |_ui| {
                    /*
                    if ui.button("Reset").clicked() {
                        println!("no");
                    }
                    */
                });

                ui.menu_button(translate!("nh-diagram"), |ui| {
                    let Some((t, c)) = self.context.last_focused_diagram() else { return; };
                    let mut controller = c.write().unwrap();

                    controller.show_menubar_diagram_options(ui, &mut commands);

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
                                    commands.push(
                                        ProjectCommand::SetSvgExportMenu(
                                            Some((t, c.clone(), path, self.context.selected_color_profiles[t], false, false, 10.0, 10.0))
                                        )
                                    );
                                }
                                ui.close_menu();
                            }
                        },
                    );
                });

                ui.menu_button(translate!("nh-windows"), |ui| {
                    // allow certain tabs to be toggled
                    for tab in &[
                        NHTab::RecentlyUsed,
                        NHTab::StyleEditor,
                        NHTab::ProjectHierarchy,
                        NHTab::ModelHierarchy,
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
                
                ui.spacing_mut().slider_width = (ui.available_width() / 2.0).max(50.0);
                ui.add(egui::Slider::new(padding_x, 0.0..=500.0).text("Horizontal padding"));
                ui.add(egui::Slider::new(padding_y, 0.0..=500.0).text("Vertical padding"));
                
                ui.separator();
                
                // Show preview
                {
                    let color_profile = &self.context.color_profiles[*t].2[*profile];
                    let drawing_context = DrawingContext {
                        profile: color_profile,
                        fluent_bundle: &self.context.fluent_bundle,
                    };
                    
                    // Measure the diagram
                    let mut measuring_canvas =
                            MeasuringCanvas::new(ui.painter());
                    controller.draw_in(&drawing_context, &mut measuring_canvas, None);
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
                            egui::CornerRadius::ZERO,
                            color_profile.backgrounds[0],
                            egui::Stroke::NONE,
                            egui::StrokeKind::Middle,
                        );
                    } else {
                        const rect_side: f32 = 20.0;
                        for ii in 0..((preview_width / rect_side) as u32) {
                            for jj in 0..=((preview_height / rect_side) as u32) {
                                painter.rect(
                                    egui::Rect::from_min_size(
                                        egui::Pos2::new(ii as f32 * rect_side, jj as f32 * rect_side)
                                        + canvas_rect.min.to_vec2(),
                                        egui::Vec2::splat(rect_side)
                                    ),
                                    egui::CornerRadius::ZERO,
                                    if (ii + jj) % 2 == 0 {egui::Color32::GRAY} else {egui::Color32::from_rgb(50, 50, 50)},
                                    egui::Stroke::NONE,
                                    egui::StrokeKind::Middle,
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
                    controller.draw_in(&drawing_context, &mut ui_canvas, None);
                }
                
                // Cancel or confirm export
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        hide_svg_export_modal = true;
                    }
                    if ui.button("OK").clicked() {
                        let color_profile = &self.context.color_profiles[*t].2[*profile];
                        let drawing_context = DrawingContext {
                            profile: color_profile,
                            fluent_bundle: &self.context.fluent_bundle,
                        };
                        
                        let mut measuring_canvas =
                            MeasuringCanvas::new(ui.painter());
                        controller.draw_in(&drawing_context, &mut measuring_canvas, None);

                        let canvas_offset = -1.0 * measuring_canvas.bounds().min
                            + egui::Vec2::new(*padding_x, *padding_y);
                        let canvas_size = measuring_canvas.bounds().size()
                            + egui::Vec2::new(
                                2.0 * *padding_x,
                                2.0 * *padding_y,
                            );
                        let mut svg_canvas = SVGCanvas::new(
                            ui.painter(),
                            canvas_offset,
                            canvas_size,
                        );
                        if *background {
                            svg_canvas.draw_rectangle(
                                egui::Rect::from_min_size(
                                    -1.0 * canvas_offset,
                                    canvas_size,
                                ),
                                egui::CornerRadius::ZERO,
                                color_profile.backgrounds[0],
                                common::canvas::Stroke::NONE,
                                common::canvas::Highlight::NONE,
                            );
                        }
                        controller.draw_in(&drawing_context, &mut svg_canvas, None);
                        let _ = svg_canvas.save_to(&path);
                        
                        hide_svg_export_modal = true;
                    }
                });
            });
        }
        if hide_svg_export_modal {
            self.context.svg_export_menu = None;
        }

        if let Some(confirm_reason) = self.context.confirm_modal_reason.clone() {
            egui::Modal::new("Modal Window".into())
                .show(ctx, |ui| {

                    ui.label(translate!("nh-generic-unsavedchanges-warning"));

                    match confirm_reason {
                        SimpleProjectCommand::OpenProject(_) => {
                            ui.label(translate!("nh-project-openproject-confirm"));
                        },
                        SimpleProjectCommand::CloseProject(_) => {
                            ui.label(translate!("nh-project-closeproject-confirm"));
                        },
                        SimpleProjectCommand::Exit(_) => {
                            ui.label(translate!("nh-project-exit-confirm"));
                        },
                        _ => unreachable!("Unexpected confirm modal reason"),
                    }

                    ui.horizontal(|ui| {
                        if ui.button(translate!("nh-generic-yes")).clicked() {
                            match confirm_reason {
                                SimpleProjectCommand::OpenProject(_) => {
                                    commands.push(SimpleProjectCommand::OpenProject(true).into());
                                },
                                SimpleProjectCommand::CloseProject(_) => {
                                    commands.push(SimpleProjectCommand::CloseProject(true).into());
                                },
                                SimpleProjectCommand::Exit(_) => {
                                    commands.push(SimpleProjectCommand::Exit(true).into());
                                },
                                _ => unreachable!("Unexpected confirm modal reason"),
                            }
                            self.context.confirm_modal_reason = None;
                        }
                        if ui.button(translate!("nh-generic-unsavedchanges-saveandproceed")).clicked() {
                            commands.push(SimpleProjectCommand::SaveProject.into());
                            match confirm_reason {
                                SimpleProjectCommand::OpenProject(_) => {
                                    commands.push(SimpleProjectCommand::OpenProject(false).into());
                                },
                                SimpleProjectCommand::CloseProject(_) => {
                                    commands.push(SimpleProjectCommand::CloseProject(false).into());
                                },
                                SimpleProjectCommand::Exit(_) => {
                                    commands.push(SimpleProjectCommand::Exit(false).into());
                                },
                                _ => unreachable!("Unexpected confirm modal reason"),
                            }
                            self.context.confirm_modal_reason = None;
                        }
                        if ui.button(translate!("nh-generic-cancel")).clicked() {
                            self.context.confirm_modal_reason = None;
                        }
                    });
                });
        }

        for c in commands {
            match c {
                ProjectCommand::SimpleProjectCommand(spc) => match spc {
                    SimpleProjectCommand::DiagramCommand(dc) => match dc {
                        DiagramCommand::UndoImmediate => self.undo_immediate(),
                        DiagramCommand::RedoImmediate => self.redo_immediate(),
                        dc => send_to_focused_diagram!(dc),
                    },
                    SimpleProjectCommand::SwapTopLanguages => {
                        if self.context.languages_order.len() > 1 {
                            self.context.languages_order.swap(0, 1);
                        }
                        self.context.fluent_bundle = common::fluent::create_fluent_bundle(&self.context.languages_order).unwrap();
                    }
                    SimpleProjectCommand::OpenProject(b) => if !self.context.has_unsaved_changes || b {
                        todo!("TODO: Open project");
                    } else {
                        self.context.confirm_modal_reason = Some(SimpleProjectCommand::OpenProject(b));
                    }
                    SimpleProjectCommand::SaveProject => {
                        if let Some(project_path) = self.context.project_path.clone()
                            .or_else(|| rfd::FileDialog::new()
                                .set_directory(std::env::current_dir().unwrap())
                                .add_filter("Nihonium Project files", &["nhp"])
                                .add_filter("All files", &["*"])
                                .save_file()) {
                            match self.context.export_project() {
                                Err(e) => println!("Error exporting: {:?}", e),
                                Ok(project) => {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .truncate(true)
                                        .write(true)
                                        .open(&project_path)
                                        .unwrap();
                                    file.write_all(project.as_bytes());
                                    self.context.project_path = Some(project_path);
                                    self.context.has_unsaved_changes = false;
                                }
                            }
                        }
                    }
                    SimpleProjectCommand::SaveProjectAs => {
                        // NOTE: This does not work on WASM, and in its current state it never will.
                        //       This will be possible to fix once this is fixed on rfd side (#128).
                        if let Some(path) = rfd::FileDialog::new()
                            .set_directory(std::env::current_dir().unwrap())
                            .add_filter("Nihonium Project files", &["nhp"])
                            .add_filter("All files", &["*"])
                            .save_file()
                        {
                            match self.context.export_project() {
                                Err(e) => println!("Error exporting: {:?}", e),
                                Ok(project) => {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .truncate(true)
                                        .write(true)
                                        .open(&path)
                                        .unwrap();
                                    file.write_all(project.as_bytes());
                                    self.context.project_path = Some(path);
                                    self.context.has_unsaved_changes = false;
                                }
                            }
                        }
                    }
                    SimpleProjectCommand::CloseProject(b) => if !self.context.has_unsaved_changes || b {
                        self.context.clear_project_data();
                        self.tree = self.tree.filter_tabs(|e| !matches!(e, NHTab::Diagram { .. } | NHTab::CustomTab { .. }))
                    } else {
                        self.context.confirm_modal_reason = Some(SimpleProjectCommand::CloseProject(b));
                    }
                    SimpleProjectCommand::Exit(b) => if !self.context.has_unsaved_changes || b {
                        std::process::exit(0);
                    } else {
                        self.context.confirm_modal_reason = Some(SimpleProjectCommand::Exit(b));
                    }
                }
                ProjectCommand::OpenAndFocusDiagram(_) => unreachable!("this should not happen"),
                ProjectCommand::AddCustomTab(uuid, tab) => self.add_custom_tab(uuid, tab),
                ProjectCommand::SetSvgExportMenu(sem) => self.context.svg_export_menu = sem,
                ProjectCommand::SetNewDiagramNumber(no) => self.context.new_diagram_no = no,
                ProjectCommand::AddNewDiagram(diagram_type, diagram_controller, hierarchy_view) => {
                    self.add_diagram(diagram_type, diagram_controller, hierarchy_view);
                },
                ProjectCommand::CopyDiagram(view_uuid, deep_copy) => {
                    let Some((t, c)) = self.context.diagram_controllers.get(&view_uuid) else {
                        continue;
                    };
                    let (new_diagram, hmview) = if deep_copy {
                        c.read().unwrap().deep_copy()
                    } else {
                        c.read().unwrap().shallow_copy()
                    };

                    self.add_diagram(*t, new_diagram.clone(), hmview);
                }
                ProjectCommand::DeleteDiagram(view_uuid) => {
                    self.context.project_hierarchy.remove(&view_uuid);
                    self.context.diagram_controllers.remove(&view_uuid);
                    self.context.last_focused_diagram.take_if(|e| *e == view_uuid);
                    if let Some(snt) = self.tree.find_tab(&NHTab::Diagram { uuid: view_uuid }) {
                        self.tree.remove_tab(snt);
                    }
                },
            }
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

fn corner_radius_ui(ui: &mut Ui, corner_radius: &mut egui::CornerRadius) {
    labeled_widget!(ui, Slider::new(&mut corner_radius.nw, 0..=15), "North-West");
    labeled_widget!(ui, Slider::new(&mut corner_radius.ne, 0..=15), "North-East");
    labeled_widget!(ui, Slider::new(&mut corner_radius.sw, 0..=15), "South-West");
    labeled_widget!(ui, Slider::new(&mut corner_radius.se, 0..=15), "South-East");
}
