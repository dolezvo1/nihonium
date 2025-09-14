use std::{collections::HashMap, sync::Arc};

use eframe::egui;

use crate::{common::{canvas::Highlight, controller::{DiagramCommand, LabelProvider, Model, ProjectCommand, SimpleProjectCommand}, eref::ERef, uuid::{ModelUuid, ViewUuid}}, domains::umlclass::umlclass_models::{UmlClassClassifier, UmlClassGeneralization}, CustomTab};
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
        fn valid_direct_subtyping(s: &str, t: &str) -> bool {
            match (s, t) {
                ("kind" | "collective" | "quantity" | "relator" | "quality" | "mode" | "category" | "mixin", "category" | "mixin") => true,
                ("subkind" | "role", "kind" | "subkind" | "collective" | "quantity" | "relator" | "category" | "mixin" | "mode" | "quality") => true,
                ("phase", "kind" | "subkind" | "collective" | "quantity" | "relator" | "mixin" | "mode" | "quality" | "phase" | "phaseMixin") => true,
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
            stereotype: Arc<String>,
            identity_providers_no: usize,
            is_abstract: bool,
            in_disjoint_complete_set: bool,
            direct_mediations_opposing_lower_bounds: usize,
            direct_characterizations_toward: usize,
            supertype_generalizations: Vec<ERef<UmlClassGeneralization>>,
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
                    let e = element_infos.entry(*m.uuid).or_default();
                    e.stereotype = m.stereotype.clone();
                    if is_identity_provider(&*m.stereotype) {
                        e.identity_providers_no += 1;
                    }
                    if m.is_abstract {
                        e.is_abstract = true;
                    }
                }
                UmlClassElement::UmlClassGeneralization(inner) => {
                    let m = inner.read();
                    let identity_providers_no = m.targets.iter()
                        .filter(|t| is_identity_provider(&*t.read().stereotype) || requires_identity(&*t.read().stereotype)).count();
                    let weight = if m.set_is_disjoint { identity_providers_no.clamp(0, 1) } else { identity_providers_no };

                    for s in &m.sources {
                        let mut e = element_infos.entry(*s.read().uuid).or_default();
                        e.identity_providers_no += weight;
                        e.supertype_generalizations.push(inner.clone());
                        if m.set_is_disjoint && m.set_is_covering {
                            e.in_disjoint_complete_set = true;
                        }

                        for t in &m.targets {
                            if !valid_direct_subtyping(&*s.read().stereotype, &*t.read().stereotype) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    text: format!("«{}» cannot be subtype of «{}»", s.read().stereotype, t.read().stereotype),
                                });
                            }
                        }
                    }
                },
                UmlClassElement::UmlClassAssociation(inner) => {
                    fn parse_multiplicity(m: &str) -> Option<(usize, Option<usize>)> {
                        if m.is_empty() {
                            None
                        } else if m == "*" {
                            Some((0, None))
                        } else if let Ok(n) = str::parse::<usize>(m) {
                            Some((n, Some(n)))
                        } else {
                            let (lower, upper) = m.split_once("..")?;
                            if upper == "*" {
                                str::parse(lower).map(|l| (l, None)).ok()
                            } else {
                                str::parse(lower).and_then(|l| str::parse(upper).map(|u| (l, Some(u)))).ok()
                            }
                        }
                    }
                    let m = inner.read();
                    let source_multiplicity = parse_multiplicity(&*m.source_label_multiplicity);
                    let target_multiplicity = parse_multiplicity(&*m.target_label_multiplicity);

                    if source_multiplicity.zip(target_multiplicity).is_none() {
                        problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("invalid multiplicities") });
                    }
                    if let Some((lm1, um1)) = source_multiplicity && um1.is_some_and(|um| lm1 > um) {
                        problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("invalid multiplicities") });
                    }
                    if let Some((lm2, um2)) = target_multiplicity && um2.is_some_and(|um| lm2 > um) {
                        problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("invalid multiplicities") });
                    }

                    match m.stereotype.as_str() {
                        "mediation" => {
                            if let Some(((lm1, _), (lm2, _))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 < 1 || lm2 < 1) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«mediation» must have multiplicities of at least 1..*") });
                            }
                            if let Some((lm, um)) = &target_multiplicity {
                                let mut e = element_infos.entry(*m.source.uuid()).or_default();
                                e.direct_mediations_opposing_lower_bounds += lm;
                            }
                            if let Some((lm, um)) = &source_multiplicity {
                                let mut e = element_infos.entry(*m.target.uuid()).or_default();
                                e.direct_mediations_opposing_lower_bounds += lm;
                            }
                        }
                        "characterization" => {
                            if let Some(((lm1, um1), (lm2, _))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 < 1) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«characterization» must have multiplicities of 1..1 and at least 1..*") });
                            }
                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                let mut e = element_infos.entry(*t.uuid).or_default();

                                if t.stereotype.as_str() != "quality" && t.stereotype.as_str() != "mode" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«characterization» must have «quality» or «mode» on the target end") });
                                }

                                e.direct_characterizations_toward += 1;
                            }
                        },
                        "derivation" => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«derivation» must have multiplicities of 1..1") });
                            }
                            // source: Relator
                            // target: material
                        },
                        "structuration" => {
                            if let Some(((_, _), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«structuration» must have multiplicities of 1..1 on the target end") });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != "quality" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«structuration» must have «quality» on the source end") });
                                }
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != "quality" && t.stereotype.as_str() != "mode" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«structuration» must have «quality» or «mode» on the target end") });
                                }
                            }
                        }
                        "componentOf" => {
                            if let Some(((lm1, _), (_, _))) = source_multiplicity.zip(target_multiplicity)
                                && lm1 < 1 {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«componentOf» must have multiplicities of at least 1..* on the source end") });
                            }
                        }
                        "subcollectionOf" => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«subcollectionOf» must have multiplicities of 1..1") });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != "collective" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«subcollectionOf» must have «collective» on the source end") });
                                }
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != "collective" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«subcollectionOf» must have «collective» on the target end") });
                                }
                            }
                        }
                        "memberOf" => {
                            if let Some(((lm1, _), (lm2, _))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 < 1 || lm2 < 1) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«memberOf» must have multiplicities of at least 1..*") });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != "collective" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«memberOf» must have «collective» on the source end") });
                                }
                            }
                        }
                        "containment" => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«containment» must have multiplicities of 1..1") });
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != "quantity" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«containment» must have «quantity» on the target end") });
                                }
                            }
                        }
                        "subquantityOf" => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«subquantityOf» must have multiplicities of 1..1") });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != "quantity" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«subquantityOf» must have «quantity» on the source end") });
                                }
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != "quantity" {
                                    problems.push(ValidationProblem::Error { uuid: *m.uuid, text: format!("«subquantityOf» must have «quantity» on the target end") });
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {},
            }
        }
        let mut element_infos = HashMap::new();
        for e in &m.contained_elements {
            r_validate_subtyping(&mut problems, &mut element_infos, e);
        }
        for (k, info) in &element_infos {
            if requires_identity(&*info.stereotype) && info.identity_providers_no != 1 {
                problems.push(ValidationProblem::Error { uuid: *k, text: format!("element does not have exactly one identity provider (found {})", info.identity_providers_no) });
            }
            fn r_lowerbounds(infos: &HashMap<ModelUuid, ElementInfo>, uuid: &ModelUuid) -> usize {
                if let Some(e) = infos.get(uuid) {
                    let mut r = e.direct_mediations_opposing_lower_bounds;

                    for e in &e.supertype_generalizations {
                        let g = e.read();
                        r += if g.set_is_disjoint {
                            g.targets.iter().map(|e| r_lowerbounds(infos, &*e.read().uuid)).min().unwrap_or(0)
                        } else {
                            g.targets.iter().map(|e| r_lowerbounds(infos, &*e.read().uuid)).sum()
                        };
                    }

                    r
                } else {
                    0
                }
            }
            if info.stereotype.as_str() == "role" && r_lowerbounds(&element_infos, &k) == 0 {
                problems.push(ValidationProblem::Error { uuid: *k, text: format!("«role» must be connected to a «mediation»") });
            }
            if info.stereotype.as_str() == "relator" && r_lowerbounds(&element_infos, &k) < 2 {
                problems.push(ValidationProblem::Error { uuid: *k, text: format!("«relator» must have sum of lower bounds on the opposite sides of «mediation»s of at least 2") });
            }
            if info.stereotype.as_str() == "phase" && !info.in_disjoint_complete_set {
                problems.push(ValidationProblem::Error { uuid: *k, text: format!("«phase» must always be part of a generalization set which is disjoint and complete") });
            }
            if (info.stereotype.as_str() == "category"
                || info.stereotype.as_str() == "mixin"
                || info.stereotype.as_str() == "phaseMixin"
                || info.stereotype.as_str() == "roleMixin") && !info.is_abstract {
                problems.push(ValidationProblem::Error { uuid: *k, text: format!("«{}» must always be abstract", info.stereotype) });
            }
            if (info.stereotype.as_str() == "quality" || info.stereotype.as_str() == "mode")
                && info.direct_characterizations_toward < 1 {
                problems.push(ValidationProblem::Error { uuid: *k, text: format!("«{}» must be at the target end of at least one «characterization»", info.stereotype) });
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
