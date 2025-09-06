
use eframe::egui;
use egui_extras::{Column, TableBuilder};

use sophia::api::{prelude::SparqlDataset, sparql::Query};
use sophia_sparql::{ResultTerm, SparqlQuery, SparqlWrapper};

use crate::{common::eref::ERef, CustomTab};
use super::rdf_models::RdfDiagram;

pub struct SparqlQueriesTab {
    model: ERef<RdfDiagram>,
    selected_query: Option<uuid::Uuid>,
    query_name_buffer: String,
    query_value_buffer: String,
    debug_message: Option<String>,
    query_results: Option<Vec<Vec<Option<ResultTerm>>>>,
}

impl SparqlQueriesTab {
    pub fn new(model: ERef<RdfDiagram>) -> Self {
         Self {
            model,
            selected_query: None,
            query_name_buffer: "".to_owned(),
            query_value_buffer: "".to_owned(),
            debug_message: None,
            query_results: None,
        }
    }

    fn save(&mut self) {
        let mut model = self.model.write();

        if let Some(q) = self
            .selected_query
            .as_ref()
            .and_then(|uuid| model.stored_queries.get_mut(uuid))
        {
            q.0 = self.query_name_buffer.clone();
            q.1 = self.query_value_buffer.clone();
        } else {
            let uuid = uuid::Uuid::now_v7();
            model.stored_queries.insert(
                uuid.clone(),
                (
                    self.query_name_buffer.to_owned(),
                    self.query_value_buffer.to_owned(),
                ),
            );
            self.selected_query = Some(uuid);
        }
    }
    fn execute(&mut self) {
        let model = self.model.write();

        match SparqlQuery::parse(&self.query_value_buffer) {
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

    fn show(&mut self, /*context: &mut NHApp,*/ ui: &mut egui::Ui) {
        let mut model = self.model.write();

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
                .selected_text(if let Some(uuid) = &self.selected_query {
                    model.stored_queries.get(uuid).unwrap().0.clone()
                } else {
                    "".to_owned()
                })
                .show_ui(ui, |ui| {
                    for (k, q) in &model.stored_queries {
                        if ui
                            .selectable_value(
                                &mut self.selected_query,
                                Some(k.clone()),
                                q.0.clone(),
                            )
                            .clicked()
                        {
                            self.query_name_buffer = q.0.clone();
                            self.query_value_buffer = q.1.clone();
                        }
                    }
                });

            if ui.button("Add new").clicked() {
                let uuid = uuid::Uuid::now_v7();
                model
                    .stored_queries
                    .insert(uuid.clone(), ("".to_owned(), "".to_owned()));
                self.selected_query = Some(uuid);
            }

            if self.selected_query.is_none() {
                ui.disable();
            }

            if ui.button("Delete").clicked() {
                model.stored_queries.remove(&self.selected_query.unwrap());
                self.selected_query = None;
            }
        });

        if self.selected_query.is_none() {
            ui.disable();
        }

        ui.label("Query name:");
        let _r2 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::singleline(&mut self.query_name_buffer),
        );

        ui.label("Query:");
        let _r3 = ui.add_sized(
            (ui.available_width(), 20.0),
            egui::TextEdit::multiline(&mut self.query_value_buffer),
        );

        drop(model);

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                self.save();
            }

            if ui.button("Save & Execute").clicked() {
                self.save();
                self.execute();
            }

            if ui.button("Execute").clicked() {
                self.execute();
            }
        });

        if let Some(m) = &self.debug_message {
            ui.colored_label(egui::Color32::RED, m);
        }

        if let Some(results) = &self.query_results {
            ui.label("Results:");

            let mut tb = TableBuilder::new(ui);

            if let Some(max_cols) = results.iter().map(|e| e.len()).max() {
                for _ in 0..max_cols {
                    tb = tb.column(Column::auto().resizable(true));
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
