use std::any::Any;

use crate::{
    SetShortcut,
    common::controller::{Domain, ElementControllerGen2, GlobalDrawingContext, Tool, View},
};
use eframe::egui;
use egui_ltreeview::DirPosition;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaletteEditingSelection {
    None,
    Group(uuid::Uuid),
    Tool(uuid::Uuid),
}

impl PaletteEditingSelection {
    pub fn uuid(&self) -> Option<&uuid::Uuid> {
        match self {
            Self::None => None,
            Self::Group(uuid) => Some(uuid),
            Self::Tool(uuid) => Some(uuid),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum PaletteEditBuffer<T: Clone, V: Clone> {
    None,
    Group(uuid::Uuid, String),
    Tool(uuid::Uuid, String, T, V, Option<egui::KeyboardShortcut>),
}

impl<T: Clone, V: Clone> PaletteEditBuffer<T, V> {
    pub fn uuid(&self) -> Option<&uuid::Uuid> {
        match self {
            Self::None => None,
            Self::Group(uuid, ..) => Some(uuid),
            Self::Tool(uuid, ..) => Some(uuid),
        }
    }
}

pub struct ToolPalette<S: Clone, DomainT: Domain> {
    elements: Vec<(
        uuid::Uuid,
        String,
        Vec<(
            uuid::Uuid,
            S,
            String,
            DomainT::CommonElementViewT,
            Option<egui::KeyboardShortcut>,
        )>,
    )>,
    selection: PaletteEditingSelection,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ToolPaletteHelper<S: Clone> {
    elements: Vec<(uuid::Uuid, String, Vec<ToolPaletteItemHelper<S>>)>,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ToolPaletteItemHelper<S: Clone> {
    uuid: uuid::Uuid,
    stage: S,
    name: String,
    keyboard_shortcut: Option<egui::KeyboardShortcut>,
}

impl<S: Clone, DomainT: Domain> ToolPalette<S, DomainT> {
    pub fn new(
        elements: Vec<(
            &str,
            Vec<(
                S,
                &str,
                DomainT::CommonElementViewT,
                Option<egui::KeyboardShortcut>,
            )>,
        )>,
    ) -> Self {
        let elements = elements
            .into_iter()
            .map(|e| {
                (
                    uuid::Uuid::now_v7(),
                    e.0.to_owned(),
                    e.1.into_iter()
                        .map(|e| (uuid::Uuid::now_v7(), e.0, e.1.to_owned(), e.2, e.3))
                        .collect(),
                )
            })
            .collect();
        Self {
            elements,
            selection: PaletteEditingSelection::None,
        }
    }

    pub fn for_each_mut<F>(&mut self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    S,
                    String,
                    DomainT::CommonElementViewT,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        ),
    {
        self.elements.iter_mut().for_each(f);
    }

    pub fn show_treeview(&mut self, _gdc: &mut GlobalDrawingContext, ui: &mut egui::Ui) {
        #[derive(Clone, Eq, Hash, PartialEq, Debug)]
        enum TreeElement {
            Root,
            Group(uuid::Uuid),
            Tool(uuid::Uuid),
        }

        enum TreeCommand {
            AddGroup(String),
            Duplicate(uuid::Uuid),
            Delete(uuid::Uuid),
        }
        let mut command = None;

        ui.label("Toolbar items");

        egui::ScrollArea::neither()
            .max_height(400.0)
            .show(ui, |ui| {
                let (_r, a) = egui_ltreeview::TreeView::new(ui.id().with("toolbar items"))
                    .allow_multi_selection(false)
                    .show(ui, |b| {
                        b.dir(TreeElement::Root, "Toolbar root");
                        for (group_id, group_label, elements) in &self.elements {
                            let add_options = |ui: &mut egui::Ui| {
                                if ui.button("Add group").clicked() {
                                    return Some(TreeCommand::AddGroup(group_label.to_owned()));
                                }
                                None
                            };
                            let group_node =
                                egui_ltreeview::NodeBuilder::dir(TreeElement::Group(*group_id))
                                    .label(group_label)
                                    .context_menu(|ui| {
                                        command = command.take().or(add_options(ui));

                                        if ui
                                            .add_enabled(
                                                elements.is_empty(),
                                                egui::Button::new("Delete"),
                                            )
                                            .clicked()
                                        {
                                            command = Some(TreeCommand::Delete(*group_id));
                                        }
                                    });
                            b.node(group_node);

                            for (tool_id, _s, tool_label, _v, _ksc) in elements {
                                let tool_node =
                                    egui_ltreeview::NodeBuilder::leaf(TreeElement::Tool(*tool_id))
                                        .label(tool_label)
                                        .context_menu(|ui| {
                                            command = command.take().or(add_options(ui));

                                            if ui.button("Duplicate").clicked() {
                                                command = Some(TreeCommand::Duplicate(*tool_id));
                                            }
                                            if ui.button("Delete").clicked() {
                                                command = Some(TreeCommand::Delete(*tool_id));
                                            }
                                        });
                                b.node(tool_node);
                            }
                            b.close_dir();
                        }
                        b.close_dir();
                    });
                for e in a {
                    if let egui_ltreeview::Action::SetSelected(e) = &e {
                        match e.first() {
                            Some(TreeElement::Group(id)) => {
                                self.selection = PaletteEditingSelection::Group(*id);
                            }
                            Some(TreeElement::Tool(id)) => {
                                self.selection = PaletteEditingSelection::Tool(*id);
                            }
                            _ => {
                                self.selection = PaletteEditingSelection::None;
                            }
                        }
                    }
                    if let egui_ltreeview::Action::Move(e) = e {
                        let egui_ltreeview::DragAndDrop {
                            source,
                            target,
                            position,
                            ..
                        } = e;
                        let position = match position {
                            egui_ltreeview::DirPosition::First => {
                                egui_ltreeview::DirPosition::First
                            }
                            egui_ltreeview::DirPosition::Last => egui_ltreeview::DirPosition::Last,
                            egui_ltreeview::DirPosition::Before(e) => match e {
                                TreeElement::Root => continue,
                                TreeElement::Group(e) | TreeElement::Tool(e) => {
                                    egui_ltreeview::DirPosition::Before(e)
                                }
                            },
                            egui_ltreeview::DirPosition::After(e) => match e {
                                TreeElement::Root => continue,
                                TreeElement::Group(e) | TreeElement::Tool(e) => {
                                    egui_ltreeview::DirPosition::After(e)
                                }
                            },
                        };

                        for src in source {
                            match src {
                                TreeElement::Root => continue,
                                TreeElement::Group(src) => {
                                    self.move_group(src, position);
                                }
                                TreeElement::Tool(src) => {
                                    let TreeElement::Group(target) = target else {
                                        continue;
                                    };
                                    self.move_tool(src, target, position);
                                }
                            }
                        }
                    }
                }
            });
        match command {
            None => {}
            Some(TreeCommand::AddGroup(name)) => {
                self.elements.push((uuid::Uuid::now_v7(), name, Vec::new()));
            }
            Some(TreeCommand::Duplicate(id)) => self.duplicate_tool(id),
            Some(TreeCommand::Delete(id)) => self.delete_node(id),
        }
    }
    pub fn get_selected(&self) -> PaletteEditingSelection {
        self.selection
    }
    pub fn get_buffer(
        &self,
        s: Option<uuid::Uuid>,
    ) -> PaletteEditBuffer<S, DomainT::CommonElementViewT> {
        let Some(id) = s else {
            return PaletteEditBuffer::None;
        };

        if let Some(e) = self.elements.iter().find(|e| e.0 == id) {
            return PaletteEditBuffer::Group(id, e.1.clone());
        }

        if let Some(e) = self
            .elements
            .iter()
            .find_map(|e| e.2.iter().find(|e| e.0 == id))
        {
            return PaletteEditBuffer::Tool(id, e.2.clone(), e.1.clone(), e.3.clone(), e.4);
        }

        PaletteEditBuffer::None
    }
    pub fn set_from_buffer(&mut self, b: PaletteEditBuffer<S, DomainT::CommonElementViewT>) {
        match b {
            PaletteEditBuffer::None => {}
            PaletteEditBuffer::Group(uuid, name) => {
                for e in self.elements.iter_mut() {
                    if e.0 == uuid {
                        e.1 = name;
                        return;
                    }
                }
            }
            PaletteEditBuffer::Tool(uuid, name, tool, view, ksc) => {
                for e in self.elements.iter_mut().flat_map(|e| e.2.iter_mut()) {
                    if e.0 == uuid {
                        e.2 = name;
                        e.1 = tool;
                        e.3 = view;
                        e.4 = ksc;
                        return;
                    }
                }
            }
        }
    }

    fn move_group(&mut self, src: uuid::Uuid, pos: DirPosition<uuid::Uuid>) {
        let Some(g) = self
            .elements
            .iter()
            .position(|e| e.0 == src)
            .map(|p| self.elements.remove(p))
        else {
            return;
        };
        let pos = match pos {
            DirPosition::First => 0,
            DirPosition::Last => self.elements.len(),
            DirPosition::After(g2) | DirPosition::Before(g2) => {
                let idx_bonus = match pos {
                    DirPosition::After(_) => 1,
                    DirPosition::Before(_) => 0,
                    _ => unreachable!(),
                };

                self.elements
                    .iter()
                    .position(|e| e.0 == g2 || e.2.iter().find(|e| e.0 == g2).is_some())
                    .unwrap()
                    + idx_bonus
            }
        };
        self.elements.insert(pos, g);
    }
    fn move_tool(&mut self, src: uuid::Uuid, target: uuid::Uuid, pos: DirPosition<uuid::Uuid>) {
        let mut t = None;
        for (_, _, elements) in self.elements.iter_mut() {
            if let Some(pos) = elements.iter().position(|e| e.0 == src) {
                t = Some(elements.remove(pos));
                break;
            }
        }
        let Some(t) = t else {
            return;
        };

        let (_, _, elements) = self.elements.iter_mut().find(|e| e.0 == target).unwrap();
        let pos = match pos {
            DirPosition::First => 0,
            DirPosition::Last => elements.len(),
            DirPosition::After(t2) | DirPosition::Before(t2) => {
                let idx_bonus = match pos {
                    DirPosition::After(_) => 1,
                    DirPosition::Before(_) => 0,
                    _ => unreachable!(),
                };

                elements.iter().position(|e| e.0 == t2).unwrap() + idx_bonus
            }
        };
        elements.insert(pos, t);
    }
    fn duplicate_tool(&mut self, target: uuid::Uuid) {
        for (_, _, elements) in self.elements.iter_mut() {
            if let Some(e) = elements.iter().find(|e| e.0 == target) {
                let new_view = {
                    let (mut tlc, mut c, mut m) = Default::default();
                    e.3.deep_copy_clone(&|_| false, &mut tlc, &mut c, &mut m);
                    tlc.iter_mut().for_each(|e| {
                        e.1.deep_copy_relink(&c, &m);
                    });
                    tlc.get(&e.3.uuid()).cloned().unwrap()
                };

                let new_e = (
                    uuid::Uuid::now_v7(),
                    e.1.clone(),
                    e.2.to_owned(),
                    new_view,
                    None,
                );
                elements.push(new_e);
            }
        }
    }
    fn delete_node(&mut self, target: uuid::Uuid) {
        self.elements.retain(|e| e.0 != target);
        self.elements
            .iter_mut()
            .for_each(|e| e.2.retain(|e| e.0 != target));
    }

    pub fn find_matching_tool_stage(
        &self,
        modifiers: egui::Modifiers,
        key: egui::Key,
    ) -> Option<(uuid::Uuid, S)> {
        for e in self.elements.iter() {
            for e in e.2.iter() {
                if e.4.is_some_and(|e| {
                    modifiers.matches_logically(e.modifiers) && e.logical_key == key
                }) {
                    return Some((e.0, e.1.clone()));
                }
            }
        }
        None
    }
    pub fn set_shortcut(&mut self, tool: uuid::Uuid, shortcut: Option<egui::KeyboardShortcut>) {
        for e in self.elements.iter_mut() {
            for e in e.2.iter_mut() {
                if e.0 == tool {
                    e.4 = shortcut;
                }
            }
        }
    }

    pub fn serialize(&self) -> Result<toml::Value, ()>
    where
        S: serde::Serialize,
    {
        toml::Value::try_from(ToolPaletteHelper {
            elements: self
                .elements
                .iter()
                .map(|e| {
                    (
                        e.0,
                        e.1.clone(),
                        e.2.iter()
                            .map(|e| ToolPaletteItemHelper {
                                uuid: e.0,
                                stage: e.1.clone(),
                                name: e.2.clone(),
                                keyboard_shortcut: e.4,
                            })
                            .collect(),
                    )
                })
                .collect(),
        })
        .map_err(|_| ())
    }

    pub fn deserialize<'a, F>(value: toml::Value, view_for_stage: F) -> Result<Self, ()>
    where
        S: serde::Deserialize<'a>,
        F: Fn(&S) -> DomainT::CommonElementViewT,
    {
        let e: ToolPaletteHelper<S> = value.try_into().map_err(|_| ())?;
        Ok(Self {
            elements: e
                .elements
                .into_iter()
                .map(|e| {
                    (
                        e.0,
                        e.1,
                        e.2.into_iter()
                            .map(|e| {
                                let v = view_for_stage(&e.stage);
                                (e.uuid, e.stage, e.name, v, e.keyboard_shortcut)
                            })
                            .collect(),
                    )
                })
                .collect(),
            selection: PaletteEditingSelection::None,
        })
    }
}
pub enum ShortCutStatus {
    NoChange,
    Cleared,
    Set,
    CancelSet,
}
pub fn show_shortcut(
    ui: &mut egui::Ui,
    ksc: &mut Option<egui::KeyboardShortcut>,
    is_being_set: bool,
) -> ShortCutStatus {
    ui.horizontal(|ui| {
        ui.label("Keyboard shorcut");

        if let Some(ksc) = ksc.as_ref() {
            ui.label(ui.format_shortcut(ksc));
        } else {
            ui.label("[none]");
        }

        if is_being_set {
            if ui.button("Cancel").clicked() {
                return ShortCutStatus::CancelSet;
            }
        } else {
            if ui.button("Set").clicked() {
                return ShortCutStatus::Set;
            }
        }

        if ksc.is_some() && ui.button("Clear").clicked() {
            *ksc = None;
            return ShortCutStatus::Cleared;
        }

        ShortCutStatus::NoChange
    })
    .inner
}

pub enum ShowSettingsResult {
    None,
    SetShortcut(uuid::Uuid),
    CancelShortcutSetting,
}

pub trait DiagramSettings: Any {
    fn show(
        &mut self,
        gdc: &mut GlobalDrawingContext,
        ui: &mut egui::Ui,
        shortcut_being_set: &Option<SetShortcut>,
    ) -> ShowSettingsResult;
    fn try_set_shortcut(&mut self, tool: uuid::Uuid, shortcut: egui::KeyboardShortcut);
    fn serialize(&self) -> Result<toml::Value, ()>;
}
pub trait DiagramSettings2<DomainT: Domain>: DiagramSettings {
    fn palette_for_each_mut<F>(&self, f: F)
    where
        F: FnMut(
            &mut (
                uuid::Uuid,
                String,
                Vec<(
                    uuid::Uuid,
                    <<DomainT as Domain>::ToolT as Tool<DomainT>>::Stage,
                    String,
                    DomainT::CommonElementViewT,
                    Option<egui::KeyboardShortcut>,
                )>,
            ),
        );
}
