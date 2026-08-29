use std::collections::HashMap;

use crate::{
    NHTab, ResourceTabMode,
    common::{
        controller::{
            DiagramCommand, DiagramController, GlobalDrawingContext, ProjectCommand,
            SimpleProjectCommand,
        },
        entity::EntityUuid,
        eref::ERef,
        uuid::{ModelUuid, ResourceUuid, ViewUuid},
    },
};

pub mod ast;
pub mod parser;

pub trait FullTextSearchable {
    fn full_text_search(&self, acc: &mut ModelSearcher);
}

pub struct ModelSearcher<'a> {
    expr: &'a ast::Expr,
    current_component: ModelUuid,
    current_found_matches: Vec<ModelUuid>,
    completed_components: Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)>,
}

impl<'a> ModelSearcher<'a> {
    pub fn new(expr: &'a ast::Expr) -> Self {
        Self {
            expr,
            current_component: ModelUuid::nil(),
            current_found_matches: Vec::new(),
            completed_components: Vec::new(),
        }
    }

    pub fn open_component(&mut self, uuid: ModelUuid) {
        self.current_component = uuid;
    }
    pub fn close_component(&mut self, views: Vec<ViewUuid>) {
        let cfm = std::mem::take(&mut self.current_found_matches);
        if !cfm.is_empty() {
            self.completed_components
                .push((self.current_component, cfm, views));
        }
    }

    pub fn check_element(&mut self, uuid: ModelUuid, fields: &[&str]) {
        if check_model(&self.expr, fields) {
            self.current_found_matches.push(uuid);
        }
    }

    pub fn results(self) -> ModelSearchResults {
        self.completed_components.into()
    }
}

fn check_model(expr: &ast::Expr, fields: &[&str]) -> bool {
    match expr {
        ast::Expr::Literal(s) => fields.iter().any(|e| e.contains(s)),
        ast::Expr::Not(expr) => !check_model(expr, fields),
        ast::Expr::Or(lhs, rhs) => check_model(lhs, fields) || check_model(rhs, fields),
        ast::Expr::And(lhs, rhs) => check_model(lhs, fields) && check_model(rhs, fields),
    }
}

#[derive(Default)]
pub struct ModelSearchResults {
    results: Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)>,
}

impl From<Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)>> for ModelSearchResults {
    fn from(value: Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)>) -> Self {
        Self { results: value }
    }
}

impl ModelSearchResults {
    pub fn show(
        &self,
        builder: &mut egui_ltreeview::TreeViewBuilder<'_, EntityUuid>,
        gdc: &GlobalDrawingContext,
        last_focused_diagram: &Option<ViewUuid>,
        diagram_controllers: &HashMap<ViewUuid, ERef<dyn DiagramController>>,
        commands: &mut Vec<ProjectCommand>,
    ) {
        macro_rules! focus_element_in {
            ($diagram:expr, $element:expr) => {
                commands.push(ProjectCommand::OpenAndFocusTab(
                    NHTab::Diagram { uuid: *$diagram },
                    None,
                ));
                commands.extend_from_slice(
                    &[
                        DiagramCommand::HighlightAllElements(
                            false,
                            crate::common::canvas::Highlight::SELECTED,
                        ),
                        DiagramCommand::HighlightElement(
                            (*$element).into(),
                            true,
                            crate::common::canvas::Highlight::SELECTED,
                        ),
                        DiagramCommand::PanToElement((*$element).into(), true),
                    ]
                    .map(|e| SimpleProjectCommand::SpecificDiagramCommand(*$diagram, e).into()),
                );
            };
        }

        if !self.results.is_empty() {
            builder.dir(
                EntityUuid::Folder(uuid::uuid!("00000000-0000-0000-0000-000000000001").into()),
                format!(
                    "Models ({})",
                    self.results.iter().map(|e| e.1.len()).sum::<usize>()
                ),
            );
            for (component, sr, diagrams) in &self.results {
                builder.dir(
                    EntityUuid::from(*component),
                    &*gdc.model_labels.get(component),
                );
                for e in sr {
                    builder.node(
                        egui_ltreeview::NodeBuilder::leaf(e.clone().into())
                            .label(&*gdc.model_labels.get(e))
                            .context_menu(|ui| {
                                ui.set_min_width(crate::MIN_MENU_WIDTH);

                                if let Some(lfd) = last_focused_diagram
                                    && diagrams.contains(lfd)
                                    && ui
                                        .button(gdc.translate_0("nh-tab-search-jumptoincurrent"))
                                        .clicked()
                                {
                                    focus_element_in!(lfd, e);
                                }
                                ui.menu_button(gdc.translate_0("nh-tab-search-jumptoin"), |ui| {
                                    ui.set_min_width(crate::MIN_MENU_WIDTH);

                                    for d in diagrams {
                                        if ui
                                            .button(
                                                &*diagram_controllers
                                                    .get(d)
                                                    .unwrap()
                                                    .read()
                                                    .view_name(d),
                                            )
                                            .clicked()
                                        {
                                            focus_element_in!(d, e);
                                        }
                                    }
                                });
                                ui.menu_button(
                                    gdc.translate_0("nh-tab-search-createviewin"),
                                    |ui| {
                                        ui.set_min_width(crate::MIN_MENU_WIDTH);

                                        for d in diagrams {
                                            if ui
                                                .button(
                                                    &*diagram_controllers
                                                        .get(d)
                                                        .unwrap()
                                                        .read()
                                                        .view_name(d),
                                                )
                                                .clicked()
                                            {
                                                commands.push(
                                                    SimpleProjectCommand::SpecificDiagramCommand(
                                                        *d,
                                                        DiagramCommand::CreateViewFor(*e),
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                    },
                                );
                            }),
                    );
                }

                builder.close_dir();
            }
            builder.close_dir();
        }
    }
}

pub struct ResourceSearcher<'a> {
    expr: &'a ast::Expr,
    found_matches: Vec<(ResourceUuid, Vec<(usize, String)>)>,
}

impl<'a> ResourceSearcher<'a> {
    pub fn new(expr: &'a ast::Expr) -> Self {
        Self {
            expr,
            found_matches: Vec::new(),
        }
    }

    pub fn check_resource(&mut self, uuid: ResourceUuid, contents: &str) {
        if check_resource(&self.expr, contents) {
            self.found_matches.push((uuid, Vec::new()));
        }
    }

    pub fn results(self) -> ResourceSearchResults {
        self.found_matches.into()
    }
}

fn check_resource(expr: &ast::Expr, contents: &str) -> bool {
    match expr {
        ast::Expr::Literal(s) => contents.contains(s),
        ast::Expr::Not(expr) => !check_resource(expr, contents),
        ast::Expr::Or(lhs, rhs) => check_resource(lhs, contents) || check_resource(rhs, contents),
        ast::Expr::And(lhs, rhs) => check_resource(lhs, contents) && check_resource(rhs, contents),
    }
}

#[derive(Default)]
pub struct ResourceSearchResults {
    results: Vec<(ResourceUuid, Vec<(usize, String)>)>,
}

impl From<Vec<(ResourceUuid, Vec<(usize, String)>)>> for ResourceSearchResults {
    fn from(value: Vec<(ResourceUuid, Vec<(usize, String)>)>) -> Self {
        Self { results: value }
    }
}

impl ResourceSearchResults {
    pub fn show(
        &self,
        builder: &mut egui_ltreeview::TreeViewBuilder<'_, EntityUuid>,
        gdc: &GlobalDrawingContext,
        commands: &mut Vec<ProjectCommand>,
    ) {
        if !self.results.is_empty() {
            builder.dir(
                EntityUuid::Folder(uuid::uuid!("00000000-0000-0000-0000-000000000002").into()),
                format!("Resources ({})", self.results.len()),
            );
            for (resource, hits) in &self.results {
                if let Some((name, _)) = gdc.raw_resources.get(resource) {
                    builder.node(
                        egui_ltreeview::NodeBuilder::leaf(EntityUuid::Resource(*resource))
                            .label(name)
                            .context_menu(|ui| {
                                ui.set_min_width(crate::MIN_MENU_WIDTH);

                                if ui.button("Edit resource").clicked() {
                                    commands.push(ProjectCommand::OpenAndFocusTab(
                                        NHTab::Resource {
                                            uuid: *resource,
                                            mode: ResourceTabMode::Edit,
                                        },
                                        None,
                                    ));
                                }
                            }),
                    );
                }
            }
            builder.close_dir();
        }
    }
}
