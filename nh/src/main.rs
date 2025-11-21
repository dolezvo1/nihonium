#![feature(unsize, coerce_unsized, associated_type_defaults)]
// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, RwLock};

use common::canvas::{NHCanvas, UiCanvas};
use common::controller::{Arrangement, GlobalDrawingContext, HierarchyNode, ModelHierarchyView, ProjectCommand, SimpleProjectCommand};
use common::project_serde::{NHSerializeError, NHDeserializer, NHDeserializeError};
use common::uuid::{ModelUuid, ViewUuid};
use eframe::egui::{
    self, vec2, CentralPanel, Frame, Slider, TopBottomPanel, Ui, ViewportBuilder, WidgetText,
};

use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{AllowedSplits, DockArea, DockState, NodeIndex, Style, SurfaceIndex, TabViewer};
use egui_ltreeview::{NodeBuilder, TreeView, TreeViewState};
use rfd::FileHandle;

mod common;
mod domains;

use crate::common::eref::ERef;
use crate::common::canvas::{Highlight, MeasuringCanvas, SVGCanvas};
use crate::common::controller::{ColorBundle, DeleteKind, DiagramCommand, DiagramController, ModifierKeys, ModifierSettings, TOOL_PALETTE_MIN_HEIGHT};
use crate::common::project_serde::{FSRawReader, FSRawWriter, FSReadAbstraction, FSWriteAbstraction, ZipFSReader, ZipFSWriter};

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

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size(vec2(1024.0, 1024.0)),
        ..Default::default()
    };
    eframe::run_native("Nihonium", options, Box::new(|cc| {
        Ok(Box::<NHApp>::default())
    }))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                Default::default(),
                Box::new(|cc| {
                    Ok(Box::<NHApp>::default())
                }),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
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
    Document { uuid: ViewUuid },
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
            NHTab::Document { .. } => "Document",
            NHTab::CustomTab { .. } => "Custom Tab",
        }
    }
}

pub trait CustomTab {
    fn title(&self) -> String;
    fn show(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>);
    //fn on_close(&mut self, context: &mut NHApp);
}

pub enum CustomModalResult {
    KeepOpen,
    CloseUnmodified,
    CloseModified(ModelUuid),
}

pub trait CustomModal {
    fn show(
        &mut self,
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult;
}

pub struct ErrorModal {
    message: String
}

impl ErrorModal {
    pub fn new_box(message: String) -> Box<dyn CustomModal> {
        Box::new(Self { message })
    }
}

impl CustomModal for ErrorModal {
    fn show(
        &mut self,
        d: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) -> CustomModalResult {
        ui.label(&self.message);

        if ui.button("Ok").clicked() {
            CustomModalResult::CloseUnmodified
        } else {
            CustomModalResult::KeepOpen
        }
    }
}

type DDes = dyn Fn(ViewUuid, &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError>;

enum FileIOOperation {
    Open(FileHandle),
    OpenContent(Result<Box<dyn FSReadAbstraction + Send>, NHDeserializeError>),
    Save(FileHandle),
    ImageExport(FileHandle, ERef<dyn DiagramController>),
    Error(String),
}

struct NHContext {
    file_io_channel: (Sender<FileIOOperation>, Receiver<FileIOOperation>),
    project_path: Option<std::path::PathBuf>,
    pub diagram_controllers: HashMap<ViewUuid, (usize, ERef<dyn DiagramController>)>,
    project_hierarchy: HierarchyNode,
    tree_view_state: TreeViewState<ViewUuid>,
    model_hierarchy_views: HashMap<ModelUuid, Arc<dyn ModelHierarchyView>>,
    diagram_deserializers: HashMap<String, (usize, &'static DDes)>,
    new_diagram_no: u32,
    documents: HashMap<ViewUuid, (String, String)>,
    pub custom_tabs: HashMap<uuid::Uuid, Arc<RwLock<dyn CustomTab>>>,
    custom_modal: Option<Box<dyn CustomModal>>,

    pub style: Option<Style>,
    zoom_factor: f32,
    zoom_with_keyboard: bool,
    diagram_shades: Vec<(String, Vec<egui::Color32>)>,
    selected_diagram_shades: Vec<usize>,
    selected_language: usize,
    languages_order: Vec<unic_langid::LanguageIdentifier>,
    shortcut_top_order: Vec<(SimpleProjectCommand, egui::KeyboardShortcut)>,
    modifier_settings: ModifierSettings,
    drawing_context: GlobalDrawingContext,

    undo_stack: Vec<(Arc<String>, ViewUuid)>,
    redo_stack: Vec<(Arc<String>, ViewUuid)>,
    unprocessed_commands: Vec<ProjectCommand>,
    should_change_title: bool,
    has_unsaved_changes: bool,

    open_unique_tabs: HashSet<NHTab>,
    last_focused_diagram: Option<ViewUuid>,
    svg_export_menu: Option<(ERef<dyn DiagramController>, Option<FileHandle>, bool, bool, Highlight, f32, f32)>,
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

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            NHTab::Diagram { uuid } => {
                let c = self.diagram_controllers.get(uuid).unwrap().1.read();
                (&*c.view_name()).into()
            }
            NHTab::Document { uuid } => {
                self.documents.get(&uuid).unwrap().0.clone().into()
            }
            NHTab::CustomTab { uuid } => {
                self.custom_tabs.get(uuid).unwrap()
                    .read().unwrap().title().into()
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
            NHTab::Document { uuid } => self.document_tab(uuid, ui),
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

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        self.open_unique_tabs.remove(tab);
        OnCloseResponse::Close
    }
}


fn add_project_element_block(gdc: &GlobalDrawingContext, new_diagram_no: u32, ui: &mut Ui, commands: &mut Vec<ProjectCommand>) {
    macro_rules! translate {
        ($msg_name:expr) => {
            gdc.fluent_bundle.format_pattern(
                gdc.fluent_bundle.get_message($msg_name).unwrap().value().unwrap(),
                None,
                &mut vec![],
            )
        };
    }

    macro_rules! diagram_button {
        ($ui:expr, $label:expr, $diagram_type:expr, $fun:expr) => {
            if $ui.button($label).clicked() {
                let diagram_controller = $fun(new_diagram_no);
                commands.push(ProjectCommand::SetNewDiagramNumber(new_diagram_no + 1));
                commands.push(ProjectCommand::AddNewDiagram($diagram_type, diagram_controller));
                $ui.close();
            }
        }
    }

    if ui.button(translate!("nh-project-addnewdocument")).clicked() {
        commands.push(ProjectCommand::AddNewDocument(uuid::Uuid::now_v7().into(), "New Document".to_owned()));
    }
    ui.menu_button(translate!("nh-project-addnewdiagram"), |ui| {
        ui.set_min_width(MIN_MENU_WIDTH);

        type NDC = fn(u32) -> ERef<dyn DiagramController + 'static>;
        ui.menu_button("UML Class", |ui| {
            ui.set_min_width(MIN_MENU_WIDTH);
            for (label, diagram_type, fun) in [
                (
                    "UML Class diagram",
                    1,
                    crate::domains::umlclass::umlclass_controllers::new as NDC,
                ),
                (
                    "OntoUML diagram",
                    3,
                    crate::domains::ontouml::ontouml_controllers::new as NDC,
                ),
            ] {
                diagram_button!(ui, label, diagram_type, fun);
            }
        });

        for (label, diagram_type, fun) in [
            (
                "DEMO Coordination Structure Diagram",
                2,
                crate::domains::democsd::democsd_controllers::new as NDC,
            ),
            (
                "DEMO Object Fact Diagram",
                4,
                crate::domains::demoofd::demoofd_controllers::new as NDC,
            ),
            ("RDF diagram", 0, crate::domains::rdf::rdf_controllers::new as NDC),
        ] {
            diagram_button!(ui, label, diagram_type, fun);
        }
    });
    ui.menu_button(translate!("nh-project-adddemodiagram"), |ui| {
        ui.set_min_width(MIN_MENU_WIDTH);

        type DDC = fn(u32) -> ERef<dyn DiagramController + 'static>;
        ui.menu_button("UML Class", |ui| {
            ui.set_min_width(MIN_MENU_WIDTH);
            for (label, diagram_type, fun) in [
                (
                    "UML Class diagram",
                    1,
                    crate::domains::umlclass::umlclass_controllers::demo as DDC,
                ),
                (
                    "OntoUML diagram",
                    3,
                    crate::domains::ontouml::ontouml_controllers::demo as DDC,
                ),
            ] {
                diagram_button!(ui, label, diagram_type, fun);
            }
        });

        for (label, diagram_type, fun) in [
            (
                "DEMO Coordination Structure Diagram",
                2,
                crate::domains::democsd::democsd_controllers::demo as DDC,
            ),
            (
                "DEMO Object Fact Diagram",
                4,
                crate::domains::demoofd::demoofd_controllers::demo as DDC,
            ),
            ("RDF diagram", 0, crate::domains::rdf::rdf_controllers::demo as DDC),
        ] {
            diagram_button!(ui, label, diagram_type, fun);
        }
    });
    ui.separator();
}


macro_rules! supported_extensions {
    ($got:expr) => {
        format!("Expected a path with a known extension (.nhp, .nhpz), got {:?}", $got)
    };
}

#[cfg(not(target_arch = "wasm32"))]
fn execute<F: Future<Output = ()> + Send + 'static>(f: F) {
    // this is stupid... use any executor of your choice instead
    std::thread::spawn(move || futures::executor::block_on(f));
}
#[cfg(target_arch = "wasm32")]
fn execute<F: Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}

impl NHContext {
    fn set_project_path(&mut self, project_path: Option<PathBuf>) {
        self.project_path = project_path;
        self.should_change_title = true;
    }
    fn set_has_unsaved_changes(&mut self, has_unsaved_changes: bool) {
        self.has_unsaved_changes = has_unsaved_changes;
        self.should_change_title = true;
    }

    fn export_project(&self, fh: FileHandle) -> Result<(), NHSerializeError> {
        let project_file_name = PathBuf::from(fh.file_name());
        let extension = if cfg!(target_arch = "wasm32") {
            "nhpz"
        } else {
            project_file_name.extension()
                .and_then(|e| e.to_str())
                .ok_or_else(|| supported_extensions!(project_file_name))?
        };
        match extension {
            #[cfg(not(target_arch = "wasm32"))]
            "nhp" => {
                let project_file_path = fh.path().to_path_buf();
                let containing_folder = project_file_path.parent()
                    .ok_or_else(|| format!("Path {:?} does not have a valid parent", project_file_path))?;
                let (sources_folder_name, sources_folder_name_str, file_name) =
                    project_file_path.file_stem()
                    .and_then(|s| project_file_path.file_name().and_then(|n| s.to_str().map(|s2| (s, s2, n))))
                    .ok_or_else(|| supported_extensions!(project_file_path))?;
                let mut wa = FSRawWriter::new(containing_folder, file_name, sources_folder_name)?;
                self.export_project_nhp(&mut wa, sources_folder_name_str)
            },
            "nhpz" => {
                let s = self.file_io_channel.0.clone();
                let mut wa = ZipFSWriter::new("project.nhp", "project");
                if let Err(e) = self.export_project_nhp(&mut wa, "project") {
                    s.send(FileIOOperation::Error(format!("Error exporting: {:?}", e)));
                    return Err(e);
                };
                execute(async move {
                    match ZipFSWriter::into_bytes(wa) {
                        Err(e) => {
                            s.send(FileIOOperation::Error(format!("Error exporting: {:?}", e)));
                        },
                        Ok(bytes) => if let Err(e) = fh.write(&bytes).await {
                            s.send(FileIOOperation::Error(format!("Error saving: {:?}", e)));
                        },
                    }
                });
                Ok(())
            },
            otherwise => Err(supported_extensions!(project_file_name).into())
        }
    }
    fn export_project_nhp<WA: FSWriteAbstraction>(&self, wa: &mut WA, sources_folder_name: &str) -> Result<(), NHSerializeError> {
        let HierarchyNode::Folder(_, project_name, children) = &self.project_hierarchy else {
            return Err(format!("invalid hierarchy root for project export").into())
        };

        Ok(common::project_serde::NHProjectSerialization::write_to(
            wa,
            &*project_name,
            sources_folder_name,
            self.new_diagram_no as usize,
            &children,
            &self.drawing_context.global_colors,
            &self.diagram_controllers,
            &self.documents,
        )?)
    }
    fn import_project(&mut self, fh: FileHandle) -> Result<(), NHDeserializeError> {
        let project_file_name = PathBuf::from(fh.file_name());
        let extension = if cfg!(target_arch = "wasm32") {
            "nhpz"
        } else {
            project_file_name.extension()
                .and_then(|e| e.to_str())
                .ok_or_else(|| supported_extensions!(project_file_name))?
        };
        match extension {
            #[cfg(not(target_arch = "wasm32"))]
            "nhp" => {
                let project_file_path = fh.path().to_path_buf();
                let containing_folder = project_file_path.parent()
                    .ok_or_else(|| format!("Path {:?} does not have a valid parent", project_file_path))?;
                let file_name = project_file_path.file_name()
                    .ok_or_else(|| supported_extensions!(project_file_path))?;
                let rr = FSRawReader::new(containing_folder.to_path_buf(), file_name.to_os_string())
                    .map(|e| Box::new(e) as Box<dyn FSReadAbstraction + Send>).map_err(|e| e.into());
                self.file_io_channel.0.send(FileIOOperation::OpenContent(rr));
                Ok(())
            },
            "nhpz" => {
                let s = self.file_io_channel.0.clone();
                execute(async move {
                    let file_contents = fh.read().await;
                    let zfsr = ZipFSReader::new(file_contents, "project.nhp", "project")
                        .map(|e| Box::new(e) as Box<dyn FSReadAbstraction + Send>).map_err(|e| e.into());
                    s.send(FileIOOperation::OpenContent(zfsr));
                });
                Ok(())
            },
            otherwise => Err(supported_extensions!(project_file_name).into())
        }
    }
    fn import_project_nhp(&mut self, ra: &mut dyn FSReadAbstraction) -> Result<(), NHDeserializeError> {
        let project_file_bytes = ra.read_manifest_file()?;
        let project_file_str = str::from_utf8(&project_file_bytes)?;
        let pdto: common::project_serde::NHProjectSerialization = toml::from_str(&project_file_str)?;
        let (hierarchy, top_level_views, documents) = pdto.deserialize_all(ra, &self.diagram_deserializers)?;

        // All good, clear and set fields
        self.clear_project_data();

        let HierarchyNode::Folder(_, project_name, children) = &mut self.project_hierarchy else {
            unreachable!("clear_project_data set hierarchy root to non-folder value")
        };
        for e in hierarchy {
            children.push(e);
        }
        *project_name = Arc::new(pdto.project_name());
        self.new_diagram_no = pdto.new_diagram_no_counter() as u32;
        for e in &top_level_views {
            let r = e.1.1.read();
            let (uuid, mhv) = (*r.model_uuid(), r.new_hierarchy_view());
            self.model_hierarchy_views.insert(uuid, mhv);
        }
        self.diagram_controllers = top_level_views;
        self.documents = documents;
        self.drawing_context.global_colors = pdto.global_colors();

        Ok(())
    }
    fn clear_project_data(&mut self) {
        self.project_path = None;
        self.diagram_controllers.clear();
        self.project_hierarchy = HierarchyNode::Folder(uuid::Uuid::nil().into(), "New Project".to_owned().into(), vec![]);
        self.model_hierarchy_views.clear();
        self.new_diagram_no = 1;
        self.documents.clear();
        self.custom_tabs.clear();
        self.drawing_context.global_colors.clear();

        self.undo_stack.clear();
        self.redo_stack.clear();
        self.unprocessed_commands.clear();
        self.should_change_title = true;
        self.has_unsaved_changes = false;

        self.last_focused_diagram = None;
        self.svg_export_menu = None;
        self.confirm_modal_reason = None;
    }

    fn sort_shortcuts(&mut self) {
        self.shortcut_top_order = self.drawing_context.shortcuts.iter().map(|(&k,&v)|(k,v)).collect();
        
        fn weight(m: &egui::KeyboardShortcut) -> u32 {
            m.modifiers.alt as u32 + m.modifiers.command as u32 + m.modifiers.shift as u32
        }
        
        self.shortcut_top_order.sort_by(|a, b| weight(&b.1).cmp(&weight(&a.1)));
    }

    fn project_hierarchy(&mut self, ui: &mut Ui) {
        enum ContextMenuAction {
            NewFolder(ViewUuid),
            CollapseAt(/*collapse:*/ Option<bool>, /*recurse:*/ bool, ViewUuid),
            RenameElement(ViewUuid),
            DeleteFolder(ViewUuid),
        }

        let mut context_menu_action = None;

        ui.horizontal(|ui| {
            if ui.button("New folder").clicked() {
                context_menu_action = Some(ContextMenuAction::NewFolder(uuid::Uuid::nil().into()));
            }
            if ui.button("Collapse all").clicked() {
                context_menu_action = Some(ContextMenuAction::CollapseAt(Some(true), true, uuid::Uuid::nil().into()));
            }
            if ui.button("Uncollapse all").clicked() {
                context_menu_action = Some(ContextMenuAction::CollapseAt(Some(false), true, uuid::Uuid::nil().into()));
            }
        });

        fn hierarchy(
            builder: &mut egui_ltreeview::TreeViewBuilder<ViewUuid>,
            gdc: &GlobalDrawingContext,
            new_diagram_no: u32,
            hn: &HierarchyNode,
            docs: &HashMap<ViewUuid, (String, String)>,
            cma: &mut Option<ContextMenuAction>,
            commands: &mut Vec<ProjectCommand>,
        ) {
            match hn {
                HierarchyNode::Folder(uuid, name, children) => {
                    builder.node(
                        NodeBuilder::dir(*uuid)
                            .label(&**name)
                            .context_menu(|ui| {
                                ui.set_min_width(MIN_MENU_WIDTH);

                                if ui.button("Toggle Collapse").clicked() {
                                    *cma = Some(ContextMenuAction::CollapseAt(None, false, *uuid));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("New Folder").clicked() {
                                    *cma = Some(ContextMenuAction::NewFolder(*uuid));
                                    ui.close();
                                }

                                add_project_element_block(gdc, new_diagram_no, ui, commands);

                                if ui.button("Collapse children").clicked() {
                                    *cma = Some(ContextMenuAction::CollapseAt(Some(true), true, *uuid));
                                    ui.close();
                                }
                                if ui.button("Uncollapse children").clicked() {
                                    *cma = Some(ContextMenuAction::CollapseAt(Some(false), true, *uuid));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Rename").clicked() {
                                    *cma = Some(ContextMenuAction::RenameElement(*uuid));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    *cma = Some(ContextMenuAction::DeleteFolder(*uuid));
                                    ui.close();
                                }
                            })
                    );

                    for c in children {
                        hierarchy(builder, gdc, new_diagram_no, c, docs, cma, commands);
                    }

                    builder.close_dir();
                },
                HierarchyNode::Diagram(rw_lock) => {
                    let hm = rw_lock.read();
                    builder.node(
                        NodeBuilder::leaf(*hm.uuid())
                            .label(&*hm.view_name())
                            .context_menu(|ui| {
                                ui.set_min_width(MIN_MENU_WIDTH);

                                if ui.button("Open").clicked() {
                                    commands.push(ProjectCommand::OpenAndFocusDiagram(*hm.uuid(), None));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("New Folder").clicked() {
                                    *cma = Some(ContextMenuAction::NewFolder(uuid::Uuid::nil().into()));
                                    ui.close();
                                }

                                add_project_element_block(gdc, new_diagram_no, ui, commands);

                                if ui.button("Duplicate (deep)").clicked() {
                                    commands.push(ProjectCommand::CopyDiagram(*hm.uuid(), true));
                                    ui.close();
                                }
                                if ui.button("Duplicate (shallow)").clicked() {
                                    commands.push(ProjectCommand::CopyDiagram(*hm.uuid(), false));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Rename").clicked() {
                                    *cma = Some(ContextMenuAction::RenameElement(*hm.uuid()));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    commands.push(ProjectCommand::DeleteDiagram(*hm.uuid()));
                                    ui.close();
                                }
                            })
                    );
                },
                HierarchyNode::Document(uuid) => {
                    builder.node(
                        NodeBuilder::leaf(*uuid)
                            .label(&docs.get(uuid).unwrap().0)
                            .context_menu(|ui| {
                                ui.set_min_width(MIN_MENU_WIDTH);

                                if ui.button("Open").clicked() {
                                    commands.push(ProjectCommand::OpenAndFocusDocument(*uuid, None));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("New Folder").clicked() {
                                    *cma = Some(ContextMenuAction::NewFolder(uuid::Uuid::nil().into()));
                                    ui.close();
                                }

                                add_project_element_block(gdc, new_diagram_no, ui, commands);

                                if ui.button("Duplicate").clicked() {
                                    commands.push(ProjectCommand::DuplicateDocument(*uuid));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Rename").clicked() {
                                    *cma = Some(ContextMenuAction::RenameElement(*uuid));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    commands.push(ProjectCommand::DeleteDocument(*uuid));
                                    ui.close();
                                }
                            })
                    );
                }
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
                        hierarchy(builder, &self.drawing_context, self.new_diagram_no, &self.project_hierarchy, &self.documents, &mut context_menu_action, &mut commands);
                    }
                );

                for action in actions.into_iter() {
                    match action {
                        egui_ltreeview::Action::Activate(a) => {
                            for selected in &a.selected {
                                if let Some((HierarchyNode::Diagram(..), _)) = self.project_hierarchy.get(selected) {
                                    commands.push(ProjectCommand::OpenAndFocusDiagram(*selected, None));
                                } else if let Some((HierarchyNode::Document(..), _)) = self.project_hierarchy.get(selected) {
                                    commands.push(ProjectCommand::OpenAndFocusDocument(*selected, None));
                                }
                            }
                        }
                        egui_ltreeview::Action::MoveExternal(dnde) => {
                            for selected in &dnde.source {
                                if let Some((HierarchyNode::Diagram(..), _)) = self.project_hierarchy.get(selected) {
                                    commands.push(ProjectCommand::OpenAndFocusDiagram(*selected, Some(dnde.position)));
                                } else if let Some((HierarchyNode::Document(..), _)) = self.project_hierarchy.get(selected) {
                                    commands.push(ProjectCommand::OpenAndFocusDocument(*selected, Some(dnde.position)));
                                }
                            }
                        }
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
                ContextMenuAction::CollapseAt(collapse, recurse, view_uuid) => {
                    let mut f = |e: &HierarchyNode| if recurse {
                        e.for_each(|e| self.tree_view_state.set_openness(e.uuid(), !collapse.unwrap_or_else(|| self.tree_view_state.is_open(&e.uuid()).unwrap_or(true))))
                    } else {
                        self.tree_view_state.set_openness(e.uuid(), !collapse.unwrap_or_else(|| self.tree_view_state.is_open(&e.uuid()).unwrap_or(true)));
                    };
                    if view_uuid.is_nil() {
                        f(&self.project_hierarchy);
                    } else if let Some(e) = self.project_hierarchy.get(&view_uuid) {
                        f(&e.0);
                    }
                },
                ContextMenuAction::RenameElement(view_uuid) => 'a: {
                    let f = |e: &HierarchyNode| match e {
                        HierarchyNode::Folder(_, name, _) => (**name).clone(),
                        HierarchyNode::Diagram(eref) => (*eref.read().view_name()).clone(),
                        HierarchyNode::Document(view_uuid) => self.documents.get(view_uuid).map(|e| e.0.clone()).unwrap(),
                    };
                    let original_name = if view_uuid.is_nil() {
                        f(&self.project_hierarchy)
                    } else if let Some(e) = self.project_hierarchy.get(&view_uuid) {
                        f(&e.0)
                    } else {
                        break 'a;
                    };

                    struct ViewRenameModal {
                        first_frame: bool,
                        view_uuid: ViewUuid,
                        name_buffer: String,
                    }

                    impl CustomModal for ViewRenameModal {
                        fn show(
                            &mut self,
                            d: &mut GlobalDrawingContext,
                            ui: &mut egui::Ui,
                            commands: &mut Vec<ProjectCommand>,
                        ) -> CustomModalResult {
                            ui.label("Name:");
                            let r = ui.text_edit_singleline(&mut self.name_buffer);
                            if self.first_frame {
                                r.request_focus();
                                self.first_frame = false;
                            }

                            let mut result = CustomModalResult::KeepOpen;
                            ui.horizontal(|ui| {
                                if ui.button("Ok").clicked() {
                                    commands.push(ProjectCommand::RenameElement(self.view_uuid, self.name_buffer.clone()));
                                    result = CustomModalResult::CloseUnmodified;
                                }
                                if ui.button("Cancel").clicked() {
                                    result = CustomModalResult::CloseUnmodified;
                                }
                            });

                            result
                        }
                    }

                    self.custom_modal = Some(Box::new(
                        ViewRenameModal {
                            first_frame: true,
                            view_uuid: view_uuid,
                            name_buffer: original_name,
                        }
                    ));
                }
                ContextMenuAction::DeleteFolder(view_uuid) => {
                    self.project_hierarchy.remove(&view_uuid);
                },
            }
        }

        self.unprocessed_commands.extend(commands.into_iter());
    }

    fn refresh_buffers(&mut self, affected_models: &HashSet<ModelUuid>) {
        if affected_models.is_empty() {
            return;
        }

        for (_, e) in self.diagram_controllers.values_mut() {
            e.write().refresh_buffers(affected_models);
        }
    }

    fn set_modified_state(&mut self, view_uuid: ViewUuid, undo_accumulator: Vec<Arc<String>>) {
        if !undo_accumulator.is_empty() {
            self.set_has_unsaved_changes(true);
            let Some((_t, target_diagram)) = self.diagram_controllers.get(&view_uuid) else { return; };

            for (_uuid, (_t, c)) in self.diagram_controllers.iter().filter(|(uuid, _)| **uuid != view_uuid) {
                c.write().apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![], &mut HashSet::new());
            }

            self.redo_stack.clear();
            target_diagram.write().apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![], &mut HashSet::new());
            target_diagram.write().apply_command(DiagramCommand::SetLastChangeFlag, &mut vec![], &mut HashSet::new());

            for command_label in undo_accumulator {
                self.undo_stack.push((command_label, view_uuid));
            }
        }
    }

    fn model_hierarchy(&mut self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, lfc)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
        let model_uuid = lfc.read().model_uuid();
        let Some(model_hierarchy_view) = self.model_hierarchy_views.get(&model_uuid) else { return; };

        let cmds = {
            let lock = lfc.read();
            let rm = lock.represented_models();
            let rf = |uuid: &ModelUuid| rm.contains_key(uuid);
            model_hierarchy_view.show_model_hierarchy(ui, &rf)
        };

        if !cmds.is_empty() {
            let mut undo_accumulator = Vec::new();
            let mut affected_models = HashSet::new();
            for c in cmds {
                lfc.write().apply_command(c, &mut undo_accumulator, &mut affected_models);
            }
            self.set_modified_state(*last_focused_diagram, undo_accumulator);
            self.refresh_buffers(&affected_models);
        }
    }

    fn toolbar(&self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
        c.write().show_toolbar(&self.drawing_context, ui);
    }

    fn properties(&mut self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };

        let mut affected_models = HashSet::new();
        let mut undo_accumulator = {
            let mut undo_accumulator = Vec::new();
            if let Some(m) = c.write().show_properties(&self.drawing_context, ui, &mut undo_accumulator, &mut affected_models) {
                self.custom_modal = Some(m);
            };
            undo_accumulator
        };

        self.set_modified_state(*last_focused_diagram, undo_accumulator);
        self.refresh_buffers(&affected_models);
    }

    fn layers(&self, ui: &mut Ui) {
        let Some(last_focused_diagram) = &self.last_focused_diagram else { return; };
        let Some((_t, c)) = self.diagram_controllers.get(last_focused_diagram) else { return; };
        c.write().show_layers(ui);
    }

    fn style_editor_tab(&mut self, ui: &mut Ui) {
        ui.heading("Style Editor");

        if ui.button("Switch light/dark theme").clicked() {
            let (new_theme, new_visuals) = match ui.ctx().theme() {
                egui::Theme::Light => (egui::Theme::Dark, egui::Visuals::dark()),
                egui::Theme::Dark => (egui::Theme::Light, egui::Visuals::light()),
            };
            ui.ctx().set_theme(new_theme);
            ui.ctx().set_visuals(new_visuals);
            self.style = Some(Style::from_egui(&ui.ctx().style()));
        }

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
        
        ui.collapsing("Diagram shades", |ui| {
            for (idx1, (name, values)) in self.diagram_shades.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(&*name);
                    egui::widgets::color_picker::color_edit_button_srgba(
                        ui,
                        values.first_mut().unwrap(),
                        egui::widgets::color_picker::Alpha::OnlyBlend
                    );
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
                self.drawing_context.fluent_bundle = common::fluent::create_fluent_bundle(&self.languages_order).unwrap();
            }

            if ui.add_enabled(self.selected_language + 1 < self.languages_order.len(), egui::Button::new("Down")).clicked() {
                self.languages_order.swap(self.selected_language, self.selected_language + 1);
                self.selected_language += 1;
                self.drawing_context.fluent_bundle = common::fluent::create_fluent_bundle(&self.languages_order).unwrap();
            }
        });

        ui.collapsing("Keys and Shortcuts", |ui| {
            ui.label("Tool palette item height");
            ui.add(egui::Slider::new(&mut self.drawing_context.tool_palette_item_height, TOOL_PALETTE_MIN_HEIGHT..=200));

            ui.label("Modifiers");


            let mut modifier_settings = self.modifier_settings;
            fn delete_kind_name(e: &Option<DeleteKind>) -> &str {
                match e {
                    None => "Ask",
                    Some(DeleteKind::DeleteView) => "Delete View",
                    Some(DeleteKind::DeleteModelIfOnlyView) => "Delete Model If Only View",
                    Some(DeleteKind::DeleteAll) => "Delete All",
                }
            }
            ui.label("Default delete action:");
            egui::ComboBox::from_id_salt("Default delete action")
                .selected_text(delete_kind_name(&modifier_settings.default_delete_kind))
                .show_ui(ui, |ui| {
                    for e in [
                        None, Some(DeleteKind::DeleteView),
                        Some(DeleteKind::DeleteModelIfOnlyView), Some(DeleteKind::DeleteAll),
                    ] {
                        ui.selectable_value(&mut modifier_settings.default_delete_kind, e, delete_kind_name(&e));
                    }
                });
            egui::Grid::new("modifiers grid").show(ui, |ui| {
                ui.label("Enable");
                ui.label("Modifiers");
                ui.label("Alt");
                ui.label("Ctrl");
                ui.label("Shift");
                ui.end_row();

                fn row(ui: &mut Ui, name: &str, m: &mut Option<ModifierKeys>) {
                    let mut b = m.is_some();
                    if ui.checkbox(&mut b, "").changed() {
                        *m = match b {
                            true => Some(ModifierKeys::NONE),
                            false => None,
                        };
                    }
                    ui.label(name);

                    if let Some(m) = m {
                        ui.add_enabled(true, egui::Checkbox::without_text(&mut m.alt));
                        ui.add_enabled(true, egui::Checkbox::without_text(&mut m.command));
                        ui.add_enabled(true, egui::Checkbox::without_text(&mut m.shift));
                    } else {
                        ui.add_enabled(false, egui::Checkbox::without_text(&mut false));
                        ui.add_enabled(false, egui::Checkbox::without_text(&mut false));
                        ui.add_enabled(false, egui::Checkbox::without_text(&mut false));
                    }

                    ui.end_row();
                }

                row(ui, "Delete View", &mut modifier_settings.delete_view_modifier);
                row(ui, "Delete Model If Only View", &mut modifier_settings.delete_model_if_modifier);
                row(ui, "Delete All", &mut modifier_settings.delete_all_modifier);

                row(ui, "Hold selection", &mut modifier_settings.hold_selection);
                row(ui, "Alternative Tool Mode", &mut modifier_settings.alternative_tool_mode);
            });
            self.modifier_settings = modifier_settings;
            self.modifier_settings.sort_delete_kinds();
            ui.separator();

            ui.label("Shortcuts");
            egui::Grid::new("shortcut editor grid").show(ui, |ui| {
                for (l, c) in &[("Swap top languages:", SimpleProjectCommand::SwapTopLanguages),
                                ("Save project:", SimpleProjectCommand::SaveProject),
                                ("Save project as:", SimpleProjectCommand::SaveProjectAs),
                                ("Arrange - Bring to Front:", DiagramCommand::ArrangeSelected(Arrangement::BringToFront).into()),
                                ("Arrange - Forward One:", DiagramCommand::ArrangeSelected(Arrangement::ForwardOne).into()),
                                ("Arrange - Backward One:", DiagramCommand::ArrangeSelected(Arrangement::BackwardOne).into()),
                                ("Arrange - Send to Back:", DiagramCommand::ArrangeSelected(Arrangement::SendToBack).into()),
                               ] {
                    ui.label(*l);
                    let sc = self.drawing_context.shortcuts.get(c);
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
                        self.drawing_context.shortcuts.remove(c);
                        self.sort_shortcuts();
                    }
                    ui.end_row();
                }
            });
        });
    }
    
    // In general it should draw first and handle input second, right?
    fn diagram_tab(&mut self, tab_uuid: &ViewUuid, ui: &mut Ui) {
        let Some((t, v)) = self.diagram_controllers.get(tab_uuid).cloned() else { return; };
        let mut diagram_controller = v.write();

        let (mut ui_canvas, response, pos) = diagram_controller.new_ui_canvas(&self.drawing_context, ui);
        response.context_menu(|ui| {
            diagram_controller.context_menu(&self.drawing_context, ui, &mut self.unprocessed_commands);
        });

        diagram_controller.draw_in(&self.drawing_context, ui_canvas.as_mut(), pos);
        let shade_color = self.diagram_shades[t].1[self.selected_diagram_shades[t]];
        ui_canvas.draw_rectangle(egui::Rect::EVERYTHING, egui::CornerRadius::ZERO, shade_color, common::canvas::Stroke::NONE, Highlight::NONE);

        let mut undo_accumulator = Vec::<Arc<String>>::new();
        let mut affected_models = HashSet::new();
        diagram_controller.handle_input(
            ui, &response, self.modifier_settings,
            &mut self.custom_modal, &mut undo_accumulator, &mut affected_models,
        );

        if !undo_accumulator.is_empty() {
            self.set_has_unsaved_changes(true);
            for (_uuid, (t, c)) in self.diagram_controllers.iter().filter(|(uuid, _)| *uuid != tab_uuid) {
                c.write().apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![], &mut HashSet::new());
            }
            
            self.redo_stack.clear();
            diagram_controller.apply_command(DiagramCommand::DropRedoStackAndLastChangeFlag, &mut vec![], &mut HashSet::new());
            diagram_controller.apply_command(DiagramCommand::SetLastChangeFlag, &mut vec![], &mut HashSet::new());
            
            for command_label in undo_accumulator {
                self.undo_stack.push((command_label, *tab_uuid));
            }
        }

        drop(diagram_controller);
        self.refresh_buffers(&affected_models);
    }

    fn document_tab(&mut self, uuid: &ViewUuid, ui: &mut Ui) {
        let c = self.documents.get_mut(uuid).unwrap();
        if ui.add_sized(ui.available_size(), egui::TextEdit::multiline(&mut c.1)).changed() {
            c.0 = c.1.lines().next().unwrap_or("empty document").to_owned();
            self.set_has_unsaved_changes(true);
        }
    }

    fn custom_tab(&mut self, tab_uuid: &uuid::Uuid, ui: &mut Ui) {
        let x = self.custom_tabs.get(tab_uuid).map(|e| e.clone()).unwrap();
        let mut custom_tab = x.write().unwrap();
        custom_tab.show(ui, &mut self.unprocessed_commands);
    }

    fn last_focused_diagram(&self) -> Option<(usize, ERef<dyn DiagramController>)> {
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
        let mut new_diagram_no = 1;
        let mut diagram_controllers = HashMap::new();
        let mut hierarchy = vec![];
        let mut model_hierarchy_views = HashMap::<_, Arc<dyn ModelHierarchyView>>::new();
        let mut tabs = vec![NHTab::RecentlyUsed, NHTab::StyleEditor];

        let documents = {
            let mut d = HashMap::<ViewUuid, (String, String)>::new();
            let document_uuid = uuid::Uuid::now_v7().into();
            hierarchy.push(HierarchyNode::Document(document_uuid));
            tabs.push(NHTab::Document { uuid: document_uuid });
            d.insert(
                document_uuid,
                (
                    "Example Document".to_owned(),
                    "Example Document\n\nDocuments may store additional text descriptions.\n\nDocuments may span many, many lines, with the first one serving as the name.".to_owned(),
                )
            );
            d
        };

        for (diagram_type, view) in [
            (1, crate::domains::umlclass::umlclass_controllers::demo(1)),
        ] {
            let r = view.read();
            let mhview = r.new_hierarchy_view();
            let (view_uuid, model_uuid) = (*r.uuid(), *r.model_uuid());
            drop(r);

            hierarchy.push(HierarchyNode::Diagram(view.clone()));
            diagram_controllers.insert(view_uuid, (diagram_type, view));
            model_hierarchy_views.insert(model_uuid, mhview);
            tabs.push(NHTab::Diagram { uuid: view_uuid });
            new_diagram_no += 1;
        }

        let mut diagram_deserializers = HashMap::new();
        diagram_deserializers.insert("rdf-diagram-view".to_string(), (0, &crate::domains::rdf::rdf_controllers::deserializer as &DDes));
        diagram_deserializers.insert("umlclass-diagram-view".to_string(), (1, &crate::domains::umlclass::umlclass_controllers::deserializer as &DDes));
        diagram_deserializers.insert("democsd-diagram-view".to_string(), (2, &crate::domains::democsd::democsd_controllers::deserializer as &DDes));
        diagram_deserializers.insert("umlclass-diagram-view-ontouml".to_string(), (3, &crate::domains::ontouml::ontouml_controllers::deserializer as &DDes));
        diagram_deserializers.insert("demoofd-diagram-view".to_string(), (4, &crate::domains::demoofd::demoofd_controllers::deserializer as &DDes));

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

        let diagram_shades = vec![
            ("RDF".to_owned(), vec![egui::Color32::TRANSPARENT]),
            ("UML Class".to_owned(), vec![egui::Color32::TRANSPARENT]),
            ("DEMO Coordination Structure Diagram".to_owned(), vec![egui::Color32::TRANSPARENT]),
            ("OntoUML".to_owned(), vec![egui::Color32::TRANSPARENT]),
            ("DEMO Object Fact Diagram".to_owned(), vec![egui::Color32::TRANSPARENT]),
        ];
        
        let selected_diagram_shades = diagram_shades.iter().map(|_| 0).collect();
        let languages_order = common::fluent::AVAILABLE_LANGUAGES.iter().map(|e| e.0.clone()).collect();
        let fluent_bundle = common::fluent::create_fluent_bundle(&languages_order)
            .expect("Could not establish base FluentBundle");
        
        let mut context = NHContext {
            file_io_channel: std::sync::mpsc::channel(),
            project_path: None,
            diagram_controllers,
            project_hierarchy: HierarchyNode::Folder(uuid::Uuid::nil().into(), Arc::new("New Project".to_owned()), hierarchy),
            tree_view_state: TreeViewState::default(),
            model_hierarchy_views,
            diagram_deserializers,
            new_diagram_no,
            documents,
            custom_tabs: HashMap::new(),
            custom_modal: None,
            
            style: None,
            zoom_factor: 1.0,
            zoom_with_keyboard: false,
            diagram_shades,
            selected_diagram_shades,
            selected_language: 0,
            languages_order,
            modifier_settings: Default::default(),
            drawing_context: GlobalDrawingContext {
                global_colors: ColorBundle::new(),
                fluent_bundle,
                shortcuts: HashMap::new(),
                tool_palette_item_height: 60,
            },
            
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            unprocessed_commands: Vec::new(),
            should_change_title: true,
            has_unsaved_changes: true,
            
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
        
        context.drawing_context.shortcuts.insert(SimpleProjectCommand::SwapTopLanguages, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L));
        context.drawing_context.shortcuts.insert(SimpleProjectCommand::OpenProject(false), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O));
        context.drawing_context.shortcuts.insert(SimpleProjectCommand::SaveProject, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S));
        context.drawing_context.shortcuts.insert(SimpleProjectCommand::SaveProjectAs, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S));
        context.drawing_context.shortcuts.insert(DiagramCommand::UndoImmediate.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z));
        context.drawing_context.shortcuts.insert(DiagramCommand::RedoImmediate.into(), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ));
        context.drawing_context.shortcuts.insert(DiagramCommand::HighlightAllElements(true, Highlight::SELECTED).into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A));
        context.drawing_context.shortcuts.insert(DiagramCommand::HighlightAllElements(false, Highlight::SELECTED).into(), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::A,
        ));
        context.drawing_context.shortcuts.insert(DiagramCommand::InvertSelection.into(), egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::I,
        ));
        context.drawing_context.shortcuts.insert(DiagramCommand::CutSelectedElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::X));
        context.drawing_context.shortcuts.insert(DiagramCommand::CopySelectedElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::C));
        context.drawing_context.shortcuts.insert(DiagramCommand::PasteClipboardElements.into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::V));
        context.drawing_context.shortcuts.insert(SimpleProjectCommand::DeleteSelectedElements(None), egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete));

        context.drawing_context.shortcuts.insert(DiagramCommand::ArrangeSelected(Arrangement::BringToFront).into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Plus));
        context.drawing_context.shortcuts.insert(DiagramCommand::ArrangeSelected(Arrangement::ForwardOne).into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Plus));
        context.drawing_context.shortcuts.insert(DiagramCommand::ArrangeSelected(Arrangement::BackwardOne).into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Minus));
        context.drawing_context.shortcuts.insert(DiagramCommand::ArrangeSelected(Arrangement::SendToBack).into(), egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Minus));
        context.sort_shortcuts();

        Self {
            context,
            tree: dock_state,
        }
    }
}

// Push to the node of the last diagram or the largest node
macro_rules! push_tab_to_best {
    ($self:expr, $tab:expr) => {
        if let Some(lfd_uuid) = &$self.context.last_focused_diagram
            && let Some((si, ni, _ti)) = $self.tree.find_tab(&NHTab::Diagram { uuid: *lfd_uuid }) {
            $self.tree.set_focused_node_and_surface((si, ni));
            $self.tree.push_to_focused_leaf($tab);
        } else {
            let mut current_largest_leaf = None;
            let mut current_max_area = None;
            for (_si, ln) in $self.tree.iter_leaves() {
                let leaf_node_area = ln.viewport.area();
                if current_max_area.is_none_or(|e| leaf_node_area > e) {
                    if let Some(tab) = ln.tabs.get(ln.active.0)
                        && let Some((si, ni, _ti)) = $self.tree.find_tab(tab) {
                        current_largest_leaf = Some((si, ni));
                        current_max_area = Some(leaf_node_area);
                    }
                }
            }
            if let Some((si, ni)) = current_largest_leaf {
                $self.tree.set_focused_node_and_surface((si, ni));
            }

            $self.tree[SurfaceIndex::main()].push_to_focused_leaf($tab);
        }
    };
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
            let mut affected_models = HashSet::new();
            ac.write().apply_command(DiagramCommand::UndoImmediate, &mut vec![], &mut affected_models);
            self.context.refresh_buffers(&affected_models);
        }
        
        self.context.redo_stack.push(e);
        self.context.set_has_unsaved_changes(true);
    }
    fn redo_immediate(&mut self) {
        let Some(e) = self.context.redo_stack.pop() else { return; };
        
        self.switch_to_tab(&NHTab::Diagram { uuid: e.1 });
        
        {
            let Some((_t, ac)) = self.context.diagram_controllers.get(&e.1) else { return; };
            let mut affected_models = HashSet::new();
            ac.write().apply_command(DiagramCommand::RedoImmediate, &mut vec![], &mut affected_models);
            self.context.refresh_buffers(&affected_models);
        }
        
        self.context.undo_stack.push(e);
        self.context.set_has_unsaved_changes(true);
    }

    fn add_diagram(
        &mut self,
        diagram_type: usize,
        diagram_view: ERef<dyn DiagramController>,
    ) {
        let r = diagram_view.read();
        let (view_uuid, model_uuid) = (*r.uuid(), *r.model_uuid());
        let hierarchy_view = r.new_hierarchy_view();
        drop(r);

        if let HierarchyNode::Folder(.., children) = &mut self.context.project_hierarchy {
            children.push(HierarchyNode::Diagram(diagram_view.clone()));
        }
        self.context
            .diagram_controllers
            .insert(view_uuid, (diagram_type, diagram_view));
        self.context.model_hierarchy_views.insert(model_uuid, hierarchy_view);
        push_tab_to_best!(self, NHTab::Diagram { uuid: view_uuid });
    }

    pub fn add_custom_tab(&mut self, uuid: uuid::Uuid, tab: Arc<RwLock<dyn CustomTab>>) {
        self.context.custom_tabs.insert(uuid, tab);

        let tab = NHTab::CustomTab { uuid };

        self.tree[SurfaceIndex::main()].push_to_focused_leaf(tab);
    }

    pub fn clear_nonstatic_tabs(&mut self) {
        self.tree.retain_tabs(
            |e| !matches!(e, NHTab::Diagram { .. } | NHTab::Document { .. } | NHTab::CustomTab { .. })
        );
        for e in self.tree.iter_leaves_mut() {
            if e.1.active.0 > e.1.tabs.len() {
                e.1.active.0 = 0;
            }
        }
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

// TODO: remove when egui/#5138 is fixed
pub const MIN_MENU_WIDTH: f32 = 250.0;

impl eframe::App for NHApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(e) = self.context.file_io_channel.1.try_recv() {
            match e {
                FileIOOperation::Open(fh) => {
                    let file_name = fh.file_name();
                    match self.context.import_project(fh) {
                        Err(e) => self.context.custom_modal = Some(ErrorModal::new_box(format!("Error opening: {:?}", e))),
                        Ok(_) => {
                            self.context.set_project_path(Some(file_name.into()));
                            self.clear_nonstatic_tabs();
                        }
                    }
                },
                FileIOOperation::OpenContent(r) => match r {
                    Err(e) => self.context.custom_modal = Some(ErrorModal::new_box(format!("Error opening: {:?}", e))),
                    Ok(mut r) => match self.context.import_project_nhp(&mut *r) {
                        Err(e) => self.context.custom_modal = Some(ErrorModal::new_box(format!("Error opening: {:?}", e))),
                        Ok(_) => {}
                    },
                }
                FileIOOperation::Save(fh) => {
                    let file_name = fh.file_name();
                    match self.context.export_project(fh) {
                        Err(e) => self.context.custom_modal = Some(ErrorModal::new_box(format!("Error exporting: {:?}", e))),
                        Ok(_) => {
                            self.context.set_project_path(Some(file_name.into()));
                            self.context.set_has_unsaved_changes(false);
                        }
                    }
                },
                FileIOOperation::ImageExport(fh, v) => {
                    self.context.svg_export_menu =
                        Some((
                            v, Some(fh),
                            false, false, Highlight::NONE,
                            10.0, 10.0,
                        ));
                }
                FileIOOperation::Error(e) => {
                    self.context.custom_modal = Some(ErrorModal::new_box(format!("Error opening: {:?}", e)));
                }
            }
        }

        // Set context state
        ctx.options_mut(|op| {
            op.zoom_factor = self.context.zoom_factor;
            op.zoom_with_keyboard = self.context.zoom_with_keyboard;
        });

        let mut commands = vec![];

        // Check for exit request, cancel if unsaved changes
        if ctx.input(|i| i.viewport().close_requested()) && self.context.has_unsaved_changes {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            commands.push(SimpleProjectCommand::Exit(false).into());
        }

        // Process ProjectCommands
        for c in self.context.unprocessed_commands.drain(..) {
            macro_rules! push_tab_to_cursor {
                ($self:expr, $tab:expr, $pos:expr) => {
                    if let Some(t) = $self.tree.find_tab(&$tab)
                        && let egui_dock::Node::Leaf(ln) = &$self.tree[t.0][t.1]
                        && ln.rect.contains($pos) {
                        $self.tree.set_focused_node_and_surface((t.0, t.1));
                        $self.tree.set_active_tab(t);
                    } else {
                        $self.tree.retain_tabs(|e| *e != $tab);
                        let mut it = $self.tree.iter_leaves();
                        while let Some((_si, ln)) = it.next() {
                            if ln.rect.contains($pos)
                                && let Some(tab) = ln.tabs.get(ln.active.0)
                                && let Some(t) = $self.tree.find_tab(tab) {
                                    drop(it);
                                    $self.tree.set_focused_node_and_surface((t.0, t.1));
                                    $self.tree[t.0].push_to_focused_leaf($tab);
                                    break;
                            }
                        }
                    }
                };
            }

            match c {
                ProjectCommand::OpenAndFocusDiagram(uuid, pos) => {
                    let target_tab = NHTab::Diagram { uuid };
                    if let Some(pos) = pos {
                        push_tab_to_cursor!(self, target_tab, pos);
                    } else {
                        if let Some(t) = self.tree.find_tab(&target_tab) {
                            self.tree.set_focused_node_and_surface((t.0, t.1));
                            self.tree.set_active_tab(t);
                        } else {
                            push_tab_to_best!(self, target_tab);
                        }
                    }
                },
                ProjectCommand::OpenAndFocusDocument(uuid, pos) => {
                    let target_tab = NHTab::Document { uuid };
                    if let Some(pos) = pos {
                        push_tab_to_cursor!(self, target_tab, pos);
                    } else {
                        if let Some(t) = self.tree.find_tab(&target_tab) {
                            self.tree.set_focused_node_and_surface((t.0, t.1));
                            self.tree.set_active_tab(t);
                        } else {
                            push_tab_to_best!(self, target_tab);
                        }
                    }
                }
                other => commands.push(other),
            }
        }
        
        // Set self.context.last_focused_diagram
        if let Some((_, NHTab::Diagram { uuid })) = self.tree.find_active_focused() {
            self.context.last_focused_diagram = Some(*uuid);
        }

        // Set window title depending on the project path
        if self.context.should_change_title {
            let modified = if self.context.has_unsaved_changes { "*" } else { "" };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                if let Some(project_path) = &self.context.project_path {
                    format!("Nihonium{} - {}", modified, project_path.to_string_lossy())
                } else {
                    format!("Nihonium{}", modified)
                }
            ));
            self.context.should_change_title = false;
        }

        macro_rules! translate {
            ($msg_name:expr) => {
                self.context.drawing_context.fluent_bundle.format_pattern(
                    self.context.drawing_context.fluent_bundle.get_message($msg_name).unwrap().value().unwrap(),
                    None,
                    &mut vec![],
                )
            };
        }

        macro_rules! shortcut_text {
            ($ui:expr, $simple_project_command:expr) => {
                self.context.drawing_context.shortcuts.get(&$simple_project_command).map(|e| $ui.ctx().format_shortcut(&e))
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
                        $ui.close();
                    }
                }
            };
        }

        macro_rules! send_to_diagram {
            ($uuid:expr, $command:expr) => {
                if let Some((_t, ac)) = self.context.diagram_controllers.get($uuid) {
                    let mut undo = vec![];
                    let mut affected_models = HashSet::new();
                    ac.write().apply_command($command, &mut undo, &mut affected_models);
                    self.context.undo_stack.extend(undo.into_iter().map(|e| (e, *$uuid)));
                    self.context.refresh_buffers(&affected_models);
                }
            };
        }

        macro_rules! send_to_focused_diagram {
            ($command:expr) => {
                if let Some((_, NHTab::Diagram { uuid })) = self.tree.find_active_focused() {
                    send_to_diagram!(uuid, $command);
                }
            };
        }

        // Show ui
        TopBottomPanel::top("egui_dock::MenuBar").show(ctx, |ui| {
            // Check diagram-handled shortcuts
            let interact_pos = ui.ctx().pointer_interact_pos();
            ui.input(|is|
                'outer: for e in is.events.iter() {
                    match e {
                        egui::Event::Cut => send_to_focused_diagram!(DiagramCommand::CutSelectedElements),
                        egui::Event::Copy => send_to_focused_diagram!(DiagramCommand::CopySelectedElements),
                        egui::Event::Paste(a) => send_to_focused_diagram!(DiagramCommand::PasteClipboardElements),
                        egui::Event::Key { key, pressed, modifiers, .. } => {
                            if !pressed {continue;}

                            if let Some(sc) = &self.context.shortcut_being_set {
                                self.context.drawing_context.shortcuts.insert(*sc, egui::KeyboardShortcut { logical_key: *key, modifiers: *modifiers });
                                self.context.shortcut_being_set = None;
                                self.context.sort_shortcuts();
                                continue;
                            }

                            if *key == egui::Key::Escape {
                                if self.context.confirm_modal_reason.is_some() {
                                    self.context.confirm_modal_reason = None;
                                } else if self.context.custom_modal.is_some() {
                                    self.context.custom_modal = None;
                                } else {
                                    if let Some(e) = self.context.last_focused_diagram
                                        .and_then(|e| self.context.diagram_controllers.get(&e)) {
                                        e.1.write().cancel_tool();
                                    }
                                }
                            }

                            'inner: for ksh in &self.context.shortcut_top_order {
                                if !(modifiers.matches_logically(ksh.1.modifiers) && *key == ksh.1.logical_key) {
                                    continue 'inner;
                                }
                                
                                match ksh.0 {
                                    e @ SimpleProjectCommand::FocusedDiagramCommand(dc) => match dc {
                                        DiagramCommand::DropRedoStackAndLastChangeFlag
                                        | DiagramCommand::SetLastChangeFlag => unreachable!(),
                                        DiagramCommand::UndoImmediate => {
                                            if matches!(self.tree.find_active_focused(), Some((_, NHTab::Diagram { .. }))) {
                                                self.undo_immediate()
                                            }
                                        },
                                        DiagramCommand::RedoImmediate => {
                                            if matches!(self.tree.find_active_focused(), Some((_, NHTab::Diagram { .. }))) {
                                                self.redo_immediate()
                                            }
                                        },
                                        _ => commands.push(e.into())
                                    },
                                    other => commands.push(other.into()),
                                }
                                
                                break 'outer;
                            }
                        }
                        egui::Event::MouseWheel { unit, delta, modifiers }
                            if modifiers.matches_logically(egui::Modifiers::COMMAND) => {
                            if let Some(pos) = &interact_pos
                                && let Some(t) = self.tree.find_tab(&NHTab::Toolbar)
                                && let Some(r) = self.tree[t.0][t.1].rect()
                                && r.contains(*pos)
                            {
                                self.context.drawing_context.tool_palette_item_height =
                                    (self.context.drawing_context.tool_palette_item_height as f32 + delta.y * 5.0)
                                        .max(TOOL_PALETTE_MIN_HEIGHT as f32) as u32;
                            }
                        }
                        _ => {}
                    }
                    
                }
            );
            
            // Menubar UI
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(translate!("nh-project"), |ui| {
                    ui.set_min_width(MIN_MENU_WIDTH);

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

                    add_project_element_block(&self.context.drawing_context, self.context.new_diagram_no, ui, &mut commands);

                    #[cfg(not(target_arch = "wasm32"))]
                    button!(ui, "nh-project-save", SimpleProjectCommand::SaveProject);
                    button!(ui, "nh-project-saveas", SimpleProjectCommand::SaveProjectAs);
                    ui.separator();
                    button!(ui, "nh-project-closeproject", SimpleProjectCommand::CloseProject(false));
                    #[cfg(not(target_arch = "wasm32"))]
                    button!(ui, "nh-project-exit", SimpleProjectCommand::Exit(false));
                });

                ui.menu_button(translate!("nh-edit"), |ui| {
                    ui.set_min_width(MIN_MENU_WIDTH);

                    ui.menu_button(translate!("nh-edit-undo"), |ui| {
                        ui.set_min_width(MIN_MENU_WIDTH);

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
                                let mut button = egui::Button::new(format!("{} in '{}'", &*c, ac.read().view_name()));
                                if let Some(shortcut_text) = shortcut_text.as_ref().filter(|_| ii == 0) {
                                    button = button.shortcut_text(shortcut_text);
                                }

                                if ui.add(button).clicked() {
                                    for _ in 0..=ii {
                                        commands.push(SimpleProjectCommand::FocusedDiagramCommand(DiagramCommand::UndoImmediate).into());
                                    }
                                    break;
                                }
                            }
                        }
                        
                    });
                    
                    ui.menu_button(translate!("nh-edit-redo"), |ui| {
                        ui.set_min_width(MIN_MENU_WIDTH);

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
                                let mut button = egui::Button::new(format!("{} in '{}'", &*c, ac.read().view_name()));
                                if let Some(shortcut_text) = shortcut_text.as_ref().filter(|_| ii == 0) {
                                    button = button.shortcut_text(shortcut_text);
                                }

                                if ui.add(button).clicked() {
                                    for _ in 0..=ii {
                                        commands.push(SimpleProjectCommand::FocusedDiagramCommand(DiagramCommand::RedoImmediate).into());
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

                    ui.menu_button(translate!("nh-edit-delete"), |ui| {
                        ui.set_min_width(MIN_MENU_WIDTH);
                        button!(ui, "nh-generic-deletemodel-view", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(false)));
                        button!(ui, "nh-generic-deletemodel-modelif", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(false)));
                        button!(ui, "nh-generic-deletemodel-all", SimpleProjectCommand::from(DiagramCommand::DeleteSelectedElements(true)));
                    });
                    ui.separator();

                    if let Some((_t, d)) = self.context.last_focused_diagram() {
                        d.write().show_menubar_edit_options(&self.context.drawing_context, ui, &mut commands);
                    }

                    ui.menu_button(translate!("nh-edit-arrange"), |ui| {
                        ui.set_min_width(MIN_MENU_WIDTH);
                        button!(ui, "nh-edit-arrange-bringtofront", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::BringToFront)));
                        button!(ui, "nh-edit-arrange-forwardone", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::ForwardOne)));
                        button!(ui, "nh-edit-arrange-backwardone", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::BackwardOne)));
                        button!(ui, "nh-edit-arrange-sendtoback", SimpleProjectCommand::from(DiagramCommand::ArrangeSelected(Arrangement::SendToBack)));
                    });
                });

                ui.menu_button(translate!("nh-view"), |ui| {
                    ui.set_min_width(MIN_MENU_WIDTH);

                    let Some((_t, v)) = self.context.last_focused_diagram() else { return; };
                    let mut view = v.write();

                    view.show_menubar_view_options(&self.context.drawing_context, ui, &mut commands);
                });

                ui.menu_button(translate!("nh-diagram"), |ui| {
                    ui.set_min_width(MIN_MENU_WIDTH);

                    let Some((_t, v)) = self.context.last_focused_diagram() else { return; };
                    let mut view = v.write();

                    view.show_menubar_diagram_options(&self.context.drawing_context, ui, &mut commands);

                    ui.menu_button(
                        format!("Export Diagram `{}` to", view.view_name()),
                        |ui| {
                            ui.set_min_width(MIN_MENU_WIDTH);

                            if ui.button("SVG").clicked() {
                                let d = rfd::AsyncFileDialog::new()
                                    .set_file_name(format!("{}.svg", view.view_name()))
                                    .add_filter("SVG files", &["svg"])
                                    .add_filter("All files", &["*"])
                                    .save_file();
                                let s = self.context.file_io_channel.0.clone();
                                let v = v.clone();
                                execute(async move {
                                    if let Some(fh) = d.await {
                                        let _ = s.send(FileIOOperation::ImageExport(fh, v));
                                    }
                                });

                                ui.close();
                            }
                        },
                    );
                });

                ui.menu_button(translate!("nh-windows"), |ui| {
                    ui.set_min_width(MIN_MENU_WIDTH);

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

                            ui.close();
                        }
                    }
                });

                ui.menu_button(translate!("nh-about"), |ui| {
                    ui.add(
                        egui::Hyperlink::from_label_and_url(
                            translate!("nh-about-mainpage"),
                            "https://github.com/dolezvo1/nihonium"
                        ).open_in_new_tab(true)
                    );
                    ui.add(
                        egui::Hyperlink::from_label_and_url(
                            translate!("nh-about-bugtracker"),
                            "https://github.com/dolezvo1/nihonium/issues"
                        ).open_in_new_tab(true)
                    );
                    ui.label(format!("Code version: {}", env!("COMMIT_IDENTIFIER")));
                });
            })
        });

        // SVG export options modal
        let mut hide_svg_export_modal = false;
        if let Some((c, fh, background, gridlines, highlight, padding_x, padding_y)) = self.context.svg_export_menu.as_mut() {
            let mut controller = c.write();
            
            egui::containers::Window::new("SVG export options").show(ctx, |ui| {
                // Change options
                ui.checkbox(background, "Solid background");
                ui.checkbox(gridlines, "Gridlines");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut highlight.selected, "Select");
                    ui.checkbox(&mut highlight.valid, "Valid");
                    ui.checkbox(&mut highlight.warning, "Warning");
                    ui.checkbox(&mut highlight.invalid, "Invalid");
                });
                
                ui.spacing_mut().slider_width = (ui.available_width() / 2.0).max(50.0);
                ui.add(egui::Slider::new(padding_x, 0.0..=500.0).text("Horizontal padding"));
                ui.add(egui::Slider::new(padding_y, 0.0..=500.0).text("Vertical padding"));
                
                ui.separator();
                
                // Show preview
                {
                    // Measure the diagram
                    let mut measuring_canvas =
                            MeasuringCanvas::new(ui.painter());
                    controller.draw_in(&self.context.drawing_context, &mut measuring_canvas, None);
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
                            egui::Color32::WHITE, // TODO: load the actual background color
                            egui::Stroke::NONE,
                            egui::StrokeKind::Middle,
                        );
                    } else {
                        const RECT_SIDE: f32 = 20.0;
                        for ii in 0..((preview_width / RECT_SIDE) as u32) {
                            for jj in 0..=((preview_height / RECT_SIDE) as u32) {
                                painter.rect(
                                    egui::Rect::from_min_size(
                                        egui::Pos2::new(ii as f32 * RECT_SIDE, jj as f32 * RECT_SIDE)
                                        + canvas_rect.min.to_vec2(),
                                        egui::Vec2::splat(RECT_SIDE)
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
                        *highlight,
                    );
                    if *gridlines {
                        ui_canvas.draw_gridlines(
                            Some((50.0, egui::Color32::from_rgb(220, 220, 220))),
                            Some((50.0, egui::Color32::from_rgb(220, 220, 220))),
                        );
                    }
                    controller.draw_in(&self.context.drawing_context, &mut ui_canvas, None);
                }

                ui.separator();
                
                // Cancel or confirm export
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        hide_svg_export_modal = true;
                    }
                    if ui.button("OK").clicked() {
                        let mut measuring_canvas =
                            MeasuringCanvas::new(ui.painter());
                        controller.draw_in(&self.context.drawing_context, &mut measuring_canvas, None);

                        let canvas_offset = -1.0 * measuring_canvas.bounds().min
                            + egui::Vec2::new(*padding_x, *padding_y);
                        let canvas_size = measuring_canvas.bounds().size()
                            + egui::Vec2::new(
                                2.0 * *padding_x,
                                2.0 * *padding_y,
                            );
                        let mut svg_canvas = SVGCanvas::new(
                            canvas_offset,
                            canvas_size,
                            *highlight,
                            ui.painter(),
                        );
                        if *background {
                            svg_canvas.draw_rectangle(
                                egui::Rect::from_min_size(
                                    -1.0 * canvas_offset,
                                    canvas_size,
                                ),
                                egui::CornerRadius::ZERO,
                                egui::Color32::WHITE, // TODO: load the actual background color
                                common::canvas::Stroke::NONE,
                                common::canvas::Highlight::NONE,
                            );
                        }
                        controller.draw_in(&self.context.drawing_context, &mut svg_canvas, None);

                        let fh = fh.take().unwrap();
                        match svg_canvas.into_bytes() {
                            Err(_) => todo!(),
                            Ok(bytes) => execute(async move {
                                let _ = fh.write(&bytes).await;
                            })
                        }

                        hide_svg_export_modal = true;
                    }
                });
            });
        }
        if hide_svg_export_modal {
            self.context.svg_export_menu = None;
        }

        if let Some(element_setup_modal) = self.context.custom_modal.as_mut() {
            let result = egui::Modal::new("Custom Modal".into())
                .show(ctx,
                    |ui| element_setup_modal.show(
                        &mut self.context.drawing_context,
                        ui,
                        &mut commands,
                    )
                ).inner;

            match result {
                CustomModalResult::KeepOpen => {},
                CustomModalResult::CloseUnmodified => {
                    self.context.custom_modal = None;
                },
                CustomModalResult::CloseModified(model_uuid) => {
                    self.context.custom_modal = None;
                    self.context.refresh_buffers(&std::iter::once(model_uuid).collect());
                },
            }
        }

        if let Some(confirm_reason) = self.context.confirm_modal_reason.clone() {
            egui::Modal::new("Confirm Modal Window".into())
                .show(ctx, |ui| {

                    if let SimpleProjectCommand::DeleteSelectedElements(k) = confirm_reason {
                        ui.label(translate!("nh-generic-deletemodel-title"));

                        let mut b = k.is_some();
                        if ui.checkbox(&mut b, translate!("nh-generic-dontaskagain")).changed() {
                            self.context.confirm_modal_reason = Some(SimpleProjectCommand::DeleteSelectedElements(
                                match b {
                                    true => Some(DeleteKind::DeleteView),
                                    false => None,
                                }
                            ));
                        }

                        ui.horizontal(|ui| {
                            if ui.button(translate!("nh-generic-deletemodel-view")).clicked() {
                                commands.push(SimpleProjectCommand::DeleteSelectedElements(Some(DeleteKind::DeleteView)).into());
                                if k.is_some() {
                                    self.context.modifier_settings.default_delete_kind = Some(DeleteKind::DeleteView);
                                }
                                self.context.confirm_modal_reason = None;
                            }
                            if ui.button(translate!("nh-generic-deletemodel-modelif")).clicked() {
                                commands.push(SimpleProjectCommand::DeleteSelectedElements(Some(DeleteKind::DeleteModelIfOnlyView)).into());
                                if k.is_some() {
                                    self.context.modifier_settings.default_delete_kind = Some(DeleteKind::DeleteModelIfOnlyView);
                                }
                                self.context.confirm_modal_reason = None;
                            }
                            if ui.button(translate!("nh-generic-deletemodel-all")).clicked() {
                                commands.push(SimpleProjectCommand::DeleteSelectedElements(Some(DeleteKind::DeleteAll)).into());
                                if k.is_some() {
                                    self.context.modifier_settings.default_delete_kind = Some(DeleteKind::DeleteAll);
                                }
                                self.context.confirm_modal_reason = None;
                            }
                            if ui.button(translate!("nh-generic-cancel")).clicked() {
                                self.context.confirm_modal_reason = None;
                            }
                        });
                    } else {
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
                    }
                });
        }

        for c in commands {
            match c {
                ProjectCommand::SimpleProjectCommand(spc) => match spc {
                    SimpleProjectCommand::FocusedDiagramCommand(dc) => match dc {
                        DiagramCommand::UndoImmediate => self.undo_immediate(),
                        DiagramCommand::RedoImmediate => self.redo_immediate(),
                        dc => send_to_focused_diagram!(dc),
                    },
                    SimpleProjectCommand::SpecificDiagramCommand(v, dc) => {
                        send_to_diagram!(&v, dc);
                    }
                    SimpleProjectCommand::DeleteSelectedElements(k) => match k.or(self.context.modifier_settings.default_delete_kind) {
                        None => {
                            self.context.confirm_modal_reason = Some(SimpleProjectCommand::DeleteSelectedElements(None));
                        },
                        Some(k) => match k {
                            DeleteKind::DeleteView => {
                                send_to_focused_diagram!(DiagramCommand::DeleteSelectedElements(false))
                            },
                            // TODO: implement delete model if only view
                            DeleteKind::DeleteModelIfOnlyView
                            | DeleteKind::DeleteAll => {
                                send_to_focused_diagram!(DiagramCommand::DeleteSelectedElements(true))
                            },
                        },
                    }
                    SimpleProjectCommand::SwapTopLanguages => {
                        if self.context.languages_order.len() > 1 {
                            self.context.languages_order.swap(0, 1);
                        }
                        self.context.drawing_context.fluent_bundle = common::fluent::create_fluent_bundle(&self.context.languages_order).unwrap();
                    }
                    SimpleProjectCommand::OpenProject(b) => if !self.context.has_unsaved_changes || b {
                        let mut dialog = rfd::AsyncFileDialog::new();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            dialog = dialog.add_filter("Nihonium Project files", &["nhp"]);
                        }
                        dialog = dialog
                            .add_filter("Nihonium Project Zip files", &["nhpz"])
                            .add_filter("All files", &["*"]);

                        let s = self.context.file_io_channel.0.clone();

                        execute(async move {
                            let file = dialog.pick_file().await;
                            if let Some(file) = file {
                                s.send(FileIOOperation::Open(file));
                            }
                        });
                    } else {
                        self.context.confirm_modal_reason = Some(SimpleProjectCommand::OpenProject(b));
                    }
                    SimpleProjectCommand::SaveProject
                    | SimpleProjectCommand::SaveProjectAs => {
                        let mut dialog = rfd::AsyncFileDialog::new();
                        #[cfg(target_arch = "wasm32")]
                        {
                            let HierarchyNode::Folder(_, name, _) = &self.context.project_hierarchy else {
                                continue;
                            };
                            dialog = dialog.set_file_name(format!("{}.nhpz", name));
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            dialog = dialog.add_filter("Nihonium Project files", &["nhp"]);
                        }
                        dialog = dialog
                            .add_filter("Nihonium Project Zip files", &["nhpz"])
                            .add_filter("All files", &["*"]);

                        let current_path = self.context.project_path.clone()
                            .filter(|_| spc == SimpleProjectCommand::SaveProject)
                            .filter(|_| cfg!(not(target_arch = "wasm32")));
                        let s = self.context.file_io_channel.0.clone();

                        execute(async move {
                            let file = dialog.save_file().await;
                            if let Some(file) = file {
                                s.send(FileIOOperation::Save(file));
                            }
                        });
                    }
                    SimpleProjectCommand::CloseProject(b) => if !self.context.has_unsaved_changes || b {
                        self.context.clear_project_data();
                        self.clear_nonstatic_tabs();
                    } else {
                        self.context.confirm_modal_reason = Some(SimpleProjectCommand::CloseProject(b));
                    }
                    SimpleProjectCommand::Exit(b) => if !self.context.has_unsaved_changes || b {
                        std::process::exit(0);
                    } else {
                        self.context.confirm_modal_reason = Some(SimpleProjectCommand::Exit(b));
                    }
                }
                ProjectCommand::RenameElement(view_uuid, new_name) => {
                    fn h(
                        e: &mut HierarchyNode,
                        uuid: ViewUuid,
                        new_name: &str,
                        docs: &mut HashMap<ViewUuid, (String, String)>,
                    ) {
                        match e {
                            HierarchyNode::Folder(view_uuid, name, children) => {
                                if uuid == *view_uuid {
                                    *name = new_name.to_owned().into();
                                }

                                for e in children.iter_mut() {
                                    h(e, uuid, new_name, docs);
                                }
                            },
                            HierarchyNode::Diagram(eref) => {
                                eref.write().set_view_name(new_name.to_owned().into());
                            },
                            HierarchyNode::Document(view_uuid) => {
                                if uuid == *view_uuid
                                    && let Some(e) = docs.get_mut(&view_uuid) {
                                    e.0 = new_name.to_owned();
                                    let mut lines: Vec<&str> = e.1.lines().collect();
                                    if !lines.is_empty() {
                                        lines[0] = new_name;
                                    } else {
                                        lines.push(new_name);
                                    }
                                    e.1 = lines.join("\n");
                                }
                            },
                        }
                    }

                    h(
                        &mut self.context.project_hierarchy,
                        view_uuid,
                        &new_name,
                        &mut self.context.documents,
                    );
                }
                ProjectCommand::OpenAndFocusDiagram(..)
                | ProjectCommand::OpenAndFocusDocument(..) => unreachable!("this really should not happen"),
                ProjectCommand::AddCustomTab(uuid, tab) => self.add_custom_tab(uuid, tab),
                ProjectCommand::SetNewDiagramNumber(no) => self.context.new_diagram_no = no,
                ProjectCommand::AddNewDiagram(diagram_type, diagram) => {
                    self.add_diagram(diagram_type, diagram);
                },
                ProjectCommand::CopyDiagram(view_uuid, deep_copy) => {
                    let Some((t, c)) = self.context.diagram_controllers.get(&view_uuid) else {
                        continue;
                    };
                    let new_diagram = if deep_copy {
                        c.read().deep_copy()
                    } else {
                        c.read().shallow_copy()
                    };

                    self.add_diagram(*t, new_diagram);
                }
                ProjectCommand::DeleteDiagram(view_uuid) => {
                    self.context.project_hierarchy.remove(&view_uuid);
                    self.context.diagram_controllers.remove(&view_uuid);
                    self.context.last_focused_diagram.take_if(|e| *e == view_uuid);
                    if let Some(snt) = self.tree.find_tab(&NHTab::Diagram { uuid: view_uuid }) {
                        self.tree.remove_tab(snt);
                    }
                },
                ProjectCommand::AddNewDocument(uuid, content) => {
                    let first_line = content.lines().next().unwrap_or("empty document").to_owned();
                    self.context.documents.insert(uuid, (first_line, content));
                    if let HierarchyNode::Folder(.., children) = &mut self.context.project_hierarchy {
                        children.push(HierarchyNode::Document(uuid));
                    }
                    push_tab_to_best!(self, NHTab::Document { uuid });
                }
                ProjectCommand::DuplicateDocument(_uuid) => {
                    // TODO:
                }
                ProjectCommand::DeleteDocument(uuid) => {
                    self.context.project_hierarchy.remove(&uuid);
                    self.context.documents.remove(&uuid);
                    if let Some(snt) = self.tree.find_tab(&NHTab::Document { uuid }) {
                        self.tree.remove_tab(snt);
                    }
                }
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
