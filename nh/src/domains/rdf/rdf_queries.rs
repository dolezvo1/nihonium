use eframe::egui;

use sophia::api::{prelude::SparqlDataset, sparql::Query};
use sophia_sparql::{ResultTerm, SparqlQuery, SparqlWrapper};

use super::rdf_models::RdfDiagram;
use crate::{
    CustomTab,
    common::{
        controller::{GlobalDrawingContext, ProjectCommand},
        eref::ERef,
        uuid::ViewUuid,
    },
};

pub struct SparqlQueriesTab {
    model: ERef<RdfDiagram>,
    selected_query: Option<ViewUuid>,
    debug_message: Option<String>,
    query_results: Option<Vec<Vec<Option<ResultTerm>>>>,
}

impl SparqlQueriesTab {
    pub fn new(model: ERef<RdfDiagram>) -> Self {
        Self {
            model,
            selected_query: None,
            debug_message: None,
            query_results: None,
        }
    }

    fn execute(&mut self, query: &str) {
        let model = self.model.read();

        match SparqlQuery::parse(query) {
            Err(e) => {
                self.debug_message = Some(format!("{:?}", e));
            }
            Ok(query) => match SparqlWrapper(&model.graph())
                .query(&query)
                .map(|e| e.into_bindings())
            {
                Err(e) => {
                    self.debug_message = Some(format!("{:?}", e));
                }
                Ok(results) => {
                    self.debug_message = None;
                    self.query_results =
                        Some(results.into_iter().flat_map(|e| e.into_iter()).collect());
                }
            },
        }
    }
}

impl CustomTab for SparqlQueriesTab {
    fn title(&self) -> String {
        "SPARQL Queries".to_owned()
    }

    fn show(
        &mut self,
        gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        let model = self.model.read();

        ui.label("Select diagram");
        egui::ComboBox::from_id_salt("Select diagram")
            .selected_text(format!("{}", model.name))
            .show_ui(ui, |_ui| {
                // TODO: if ui.selectable_value(&mut self.diagram, e.clone(), format!("{:?}", e.name)).clicked() {}
                // TODO: zero out selected query?
            });

        ui.label("Select query");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("Select query")
                .selected_text(
                    self.selected_query
                        .and_then(|e| gdc.raw_resources.get(&e))
                        .map(|e| e.0.clone())
                        .unwrap_or_else(|| "".to_owned()),
                )
                .show_ui(ui, |ui| {
                    for (k, q) in gdc
                        .raw_resources
                        .iter()
                        .filter(|e| e.1.0.ends_with(".sparql"))
                    {
                        ui.selectable_value(&mut self.selected_query, Some(*k), q.0.clone());
                    }
                });

            if ui.button("Add new").clicked() {
                let uuid = ViewUuid::now_v7();
                commands.push(ProjectCommand::AddNewResource {
                    into: ViewUuid::nil(),
                    uuid,
                    name: "all_triples.sparql".to_owned(),
                    content: "SELECT ?s ?p ?o WHERE { ?s ?p ?o }".as_bytes().to_vec(),
                });
                self.selected_query = Some(uuid);
            }

            if self.selected_query.is_none() {
                ui.disable();
            }
        });

        if self.selected_query.is_none() {
            ui.disable();
        }

        drop(model);

        if let query = self.selected_query.and_then(|e| gdc.raw_resources.get(&e))
            && ui
                .add_enabled(query.is_some(), egui::Button::new("Execute"))
                .clicked()
            && let Some(query) = query.and_then(|e| str::from_utf8(&e.1).ok())
        {
            self.execute(query);
        }

        if let Some(m) = &self.debug_message {
            ui.colored_label(egui::Color32::RED, m);
        }

        if let Some(results) = &self.query_results {
            ui.label("Results:");

            let mut tb = egui_extras::TableBuilder::new(ui);

            if let Some(max_cols) = results.iter().map(|e| e.len()).max() {
                for _ in 0..max_cols {
                    tb = tb.column(egui_extras::Column::auto().resizable(true));
                }

                tb.body(|mut body| {
                    for rr in results {
                        body.row(30.0, |mut row| {
                            for ee in rr {
                                row.col(|ui| {
                                    ui.label(match ee {
                                        Some(term) => format!("{}", term),
                                        _ => "".to_owned(),
                                    });
                                });
                            }
                        });
                    }
                });
            }
        }
    }
}
