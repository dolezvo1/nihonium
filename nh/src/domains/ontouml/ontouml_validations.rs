use std::collections::HashMap;

use eframe::egui;

use crate::{common::{canvas::Highlight, controller::{DiagramCommand, LabelProvider, ProjectCommand, SimpleProjectCommand}, eref::ERef, uuid::{ModelUuid, ViewUuid}}, CustomTab};
use super::super::umlclass::umlclass_models::{UmlClassDiagram, UmlClassElement};

pub struct OntoUMLValidationTab {
    model: ERef<UmlClassDiagram>,
    label_provider: ERef<dyn LabelProvider>,
    view_uuid: ViewUuid,
    check_errors: bool,
    check_antipatterns: bool,
    results: Option<Vec<ValidationProblem>>,
}

impl OntoUMLValidationTab {
    pub fn new(
        model: ERef<UmlClassDiagram>,
        label_provider: ERef<dyn LabelProvider>,
        view_uuid: ViewUuid,
    ) -> Self {
        Self {
            model,
            label_provider,
            view_uuid,
            check_errors: true,
            check_antipatterns: false,
            results: None,
        }
    }

    fn validate(&self, check_errors: bool, check_antipatterns: bool) -> Vec<ValidationProblem> {
        let mut problems = Vec::new();

        if check_errors {
            problems.extend(self.validate_structure());
        }

        if check_antipatterns {
            problems.extend(self.validate_antipatterns());
        }

        problems
    }

    fn validate_structure(&self) -> Vec<ValidationProblem> {
        let mut problems = Vec::new();
        let m = self.model.read();

        // Subtyping and identity providers validation
        fn valid_subtyping(s: &str, t: &str) -> bool {
            match (s, t) {
                ("kind" | "collective" | "quantity" | "relator" | "quality" | "mode" | "category" | "mixin", "category" | "mixin") => true,
                ("subkind" | "phase" | "role", "kind" | "subkind" | "collective" | "quantity" | "relator" | "category" | "mixin" | "mode" | "quality") => true,
                ("phase", "phase" | "phaseMixin") => true,
                ("role", "role" | "roleMixin") => true,
                ("phaseMixin", "mixin" | "phaseMixin" | "category") => true,
                ("roleMixin", "mixin" | "roleMixin" | "category" | "phaseMixin") => true,
                _ => false,
            }
        }
        fn is_identity_provider(s: &str) -> bool {
            ["kind", "collective", "quantity", "relator", "quality", "mode"].iter().find(|e| **e == s).is_some()
        }
        fn requires_identity(s: &str) -> bool {
            ["category", "mixin", "phaseMixin", "roleMixin"].iter().find(|e| **e == s).is_none()
        }
        #[derive(Default)]
        struct ElementInfo {
            requires_identity: bool,
            identity_providers_no: usize,
        }
        fn r_validate_subtyping(
            problems: &mut Vec<ValidationProblem>,
            element_infos: &mut HashMap<ModelUuid, ElementInfo>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_validate_subtyping(problems, element_infos, e);
                    }
                },
                UmlClassElement::UmlClass(inner) => {
                    let m = inner.read();
                    let mut e = element_infos.entry(*m.uuid).or_default();
                    e.requires_identity = requires_identity(&*m.stereotype);
                    if is_identity_provider(&*m.stereotype) {
                        e.identity_providers_no += 1;
                    }
                }
                UmlClassElement::UmlClassGeneralization(inner) => {
                    let m = inner.read();
                    let identity_providers_no = m.targets.iter()
                        .filter(|t| is_identity_provider(&*t.read().stereotype) || requires_identity(&*t.read().stereotype)).count();
                    let weight = if m.set_is_disjoint { identity_providers_no.clamp(0, 1) } else { identity_providers_no };

                    for s in &m.sources {
                        element_infos.entry(*s.read().uuid).or_default().identity_providers_no += weight;

                        for t in &m.targets {
                            if !valid_subtyping(&*s.read().stereotype, &*t.read().stereotype) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    text: format!("«{}» cannot be subtype of «{}»", s.read().stereotype, t.read().stereotype),
                                });
                            }
                        }
                    }
                },
                _ => {},
            }
        }
        let mut element_infos = HashMap::new();
        for e in &m.contained_elements {
            r_validate_subtyping(&mut problems, &mut element_infos, e);
        }
        for (k, info) in element_infos {
            if info.requires_identity && info.identity_providers_no != 1 {
                problems.push(ValidationProblem::Error { uuid: k, text: format!("element does not have exactly one identity provider (found {})", info.identity_providers_no) });
            }
        }

        problems
    }

    fn validate_antipatterns(&self) -> Vec<ValidationProblem> {
        let mut problems = Vec::new();
        let m = self.model.read();

        // DecInt (Decieving Intersection)
        fn r_decint_collect(
            parents: &mut HashMap<ModelUuid, usize>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_decint_collect(parents, e);
                    }
                },
                UmlClassElement::UmlClassGeneralization(inner) => {
                    let m = inner.read();
                    let weight = if m.set_is_disjoint { 1 } else { m.targets.len() };
                    for e in &m.sources {
                        *parents.entry(*e.read().uuid).or_default() += weight;
                    }
                },
                _ => {},
            }
        }
        let mut parents = HashMap::new();
        for e in &m.contained_elements {
            r_decint_collect(&mut parents, e);
        }
        for (k, v) in parents {
            if v > 1 {
                problems.push(ValidationProblem::AntiPattern { uuid: k, antipattern_type: AntiPatternType::DecInt });
            }
        }


        problems
    }
}

impl CustomTab for OntoUMLValidationTab {
    fn title(&self) -> String {
        "OntoUML Validations".to_owned()
    }

    fn show(&mut self, ui: &mut egui::Ui, commands: &mut Vec<ProjectCommand>) {
        ui.horizontal(|ui| {
            if ui.button("Clear highlighting").clicked() {
                commands.push(
                    SimpleProjectCommand::SpecificDiagramCommand(
                        self.view_uuid,
                        DiagramCommand::HighlightAllElements(false, Highlight::ALL),
                    ).into()
                );
            }

            ui.checkbox(&mut self.check_errors, "Check errors");
            ui.checkbox(&mut self.check_antipatterns, "Check antipatterns");

            if ui.button("Validate").clicked() {
                let results = self.validate(self.check_errors, self.check_antipatterns);

                commands.push(
                    SimpleProjectCommand::SpecificDiagramCommand(
                        self.view_uuid,
                        DiagramCommand::HighlightAllElements(false, Highlight::ALL),
                    ).into()
                );

                for rr in &results {
                    match rr {
                        ValidationProblem::Error { uuid, .. } => {
                            commands.push(
                                SimpleProjectCommand::SpecificDiagramCommand(
                                    self.view_uuid,
                                    DiagramCommand::HighlightElement((*uuid).into(), true, Highlight::INVALID),
                                ).into()
                            );
                        },
                        ValidationProblem::AntiPattern { uuid, .. } => {
                            commands.push(
                                SimpleProjectCommand::SpecificDiagramCommand(
                                    self.view_uuid,
                                    DiagramCommand::HighlightElement((*uuid).into(), true, Highlight::WARNING),
                                ).into()
                            );
                        },
                    }
                }

                self.results = Some(results);
            }
        });

        if let Some(results) = &self.results {
            if results.is_empty() {
                ui.label("No problems found");
            } else {
                ui.label("Results:");

                let mut tb = egui_extras::TableBuilder::new(ui)
                    .column(egui_extras::Column::auto().resizable(true))
                    .column(egui_extras::Column::auto().resizable(true))
                    .column(egui_extras::Column::remainder().resizable(true));

                tb.body(|mut body| {
                    for rr in results {
                        body.row(30.0, |mut row| {
                            let uuid = match rr {
                                ValidationProblem::Error { uuid, .. } => *uuid,
                                ValidationProblem::AntiPattern { uuid, .. } => *uuid,
                            };

                            row.col(|ui| {
                                ui.label(match rr {
                                    ValidationProblem::Error { .. } => "Error",
                                    ValidationProblem::AntiPattern { .. } => "Anti-Pattern",
                                });
                            });

                            row.col(|ui| {
                                if ui.label(&*self.label_provider.read().get(&uuid)).clicked() {
                                    commands.push(
                                        SimpleProjectCommand::SpecificDiagramCommand(
                                            self.view_uuid,
                                            DiagramCommand::HighlightAllElements(false, Highlight::SELECTED),
                                        ).into()
                                    );
                                    commands.push(
                                        SimpleProjectCommand::SpecificDiagramCommand(
                                            self.view_uuid,
                                            DiagramCommand::HighlightElement(uuid.into(), true, Highlight::SELECTED),
                                        ).into()
                                    );
                                }
                            });

                            match rr {
                                ValidationProblem::Error { uuid, text } => {
                                    row.col(|ui| {
                                        ui.label(text);
                                    });
                                },
                                ValidationProblem::AntiPattern { uuid, antipattern_type } => {
                                    row.col(|ui| {
                                        ui.label(format!("{:?}", antipattern_type));
                                    });
                                },
                            }
                        });
                    }
                });
            }
        }
    }
}

#[derive(Debug)]
enum ValidationProblem {
    Error {
        uuid: ModelUuid,
        text: String,
    },
    AntiPattern {
        uuid: ModelUuid,
        antipattern_type: AntiPatternType,
    },
}

#[derive(Debug)]
enum AntiPatternType {
    BinOver,
    DecInt,
    DepPhase,
    FreeRole,
    GSRig,
    HetColl,
    HomoFunc,
    ImpAbs,
    MixIden,
    MixRig,
    MultDep,
    PartOver,
    RelComp,
    RelOver,
    RelRig,
    RelSpec,
    RepRel,
    UndefFormal,
    UndefPhase,
    WholeOver,
}
