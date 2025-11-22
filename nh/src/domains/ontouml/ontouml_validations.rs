use std::{collections::{HashMap, HashSet}, sync::{Arc, RwLockReadGuard}};

use eframe::egui;

use crate::{CustomTab, common::{canvas::Highlight, controller::{DiagramCommand, LabelProvider, Model, ProjectCommand, SimpleProjectCommand}, eref::ERef, uuid::{ModelUuid, ViewUuid}}, domains::{ontouml::ontouml_models, umlclass::umlclass_models::{UmlClass, UmlClassClassifier, UmlClassGeneralization}}};
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
                (ontouml_models::KIND | ontouml_models::COLLECTIVE | ontouml_models::QUANTITY | ontouml_models::RELATOR | ontouml_models::QUALITY | ontouml_models::MODE | ontouml_models::CATEGORY | ontouml_models::MIXIN, ontouml_models::CATEGORY | ontouml_models::MIXIN) => true,
                (ontouml_models::SUBKIND | ontouml_models::ROLE, ontouml_models::KIND | ontouml_models::SUBKIND | ontouml_models::COLLECTIVE | ontouml_models::QUANTITY | ontouml_models::RELATOR | ontouml_models::CATEGORY | ontouml_models::MIXIN | ontouml_models::MODE | ontouml_models::QUALITY) => true,
                (ontouml_models::PHASE, ontouml_models::KIND | ontouml_models::SUBKIND | ontouml_models::COLLECTIVE | ontouml_models::QUANTITY | ontouml_models::RELATOR | ontouml_models::MIXIN | ontouml_models::MODE | ontouml_models::QUALITY | ontouml_models::PHASE | ontouml_models::PHASE_MIXIN) => true,
                (ontouml_models::ROLE, ontouml_models::ROLE | ontouml_models::ROLE_MIXIN) => true,
                (ontouml_models::PHASE_MIXIN, ontouml_models::MIXIN | ontouml_models::PHASE_MIXIN | ontouml_models::CATEGORY) => true,
                (ontouml_models::ROLE_MIXIN, ontouml_models::MIXIN | ontouml_models::ROLE_MIXIN | ontouml_models::CATEGORY | ontouml_models::PHASE_MIXIN) => true,
                _ => false,
            }
        }
        fn is_identity_provider(s: &str) -> bool {
            [ontouml_models::KIND, ontouml_models::COLLECTIVE, ontouml_models::QUANTITY, ontouml_models::RELATOR, ontouml_models::QUALITY, ontouml_models::MODE].iter().find(|e| **e == s).is_some()
        }
        fn requires_identity(s: &str) -> bool {
            [ontouml_models::CATEGORY, ontouml_models::MIXIN, ontouml_models::PHASE_MIXIN, ontouml_models::ROLE_MIXIN].iter().find(|e| **e == s).is_none()
        }
        #[derive(Default)]
        struct ElementInfo {
            stereotype: Arc<String>,
            identity_providers_min: usize,
            identity_providers_max: usize,
            is_abstract: bool,
            in_disjoint_complete_set: bool,
            direct_mediations_opposing_lower_bounds: usize,
            direct_characterizations_toward: usize,
            parents: Vec<ERef<UmlClassGeneralization>>,
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

                    if ontouml_models::ontouml_class_stereotype_literal(&m.stereotype).is_none_or(|e| e == ontouml_models::NONE) {
                        problems.push(ValidationProblem::Error {
                            uuid: *m.uuid,
                            error_type: ErrorType::InvalidStereotype,
                            text: format!("Invalid or missing stereotype"),
                        });
                    }

                    e.stereotype = m.stereotype.clone();
                    if is_identity_provider(&*m.stereotype) {
                        e.identity_providers_min += 1;
                        e.identity_providers_max += 1;
                    }
                    if m.is_abstract {
                        e.is_abstract = true;
                    }
                }
                UmlClassElement::UmlClassGeneralization(inner) => {
                    let m = inner.read();
                    let identity_providers_no = m.targets.iter()
                        .filter(|t| is_identity_provider(&*t.read().stereotype) || requires_identity(&*t.read().stereotype)).count();
                    let (weight_min, weight_max) = if m.set_is_disjoint || (m.sources.len() == 1 && m.targets.len() == 1) {
                        (if identity_providers_no == m.targets.len() { 1 } else { 0 }, identity_providers_no.min(1))
                    } else {
                        if m.set_is_covering {
                            (identity_providers_no.min(1), identity_providers_no)
                        } else {
                            (0, identity_providers_no + 1)
                        }
                    };

                    for s in &m.sources {
                        let mut e = element_infos.entry(*s.read().uuid).or_default();
                        e.identity_providers_min += weight_min;
                        e.identity_providers_max += weight_max;
                        e.parents.push(inner.clone());
                        if m.set_is_disjoint && m.set_is_covering {
                            e.in_disjoint_complete_set = true;
                        }

                        for t in &m.targets {
                            if !valid_direct_subtyping(&*s.read().stereotype, &*t.read().stereotype) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidSubtyping,
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

                    if ontouml_models::ontouml_association_stereotype_literal(&m.stereotype).is_none_or(|e| e == ontouml_models::NONE) {
                        problems.push(ValidationProblem::Error {
                            uuid: *m.uuid,
                            error_type: ErrorType::InvalidStereotype,
                            text: format!("Invalid or missing stereotype"),
                        });
                    }

                    let source_multiplicity = parse_multiplicity(&*m.source_label_multiplicity);
                    let target_multiplicity = parse_multiplicity(&*m.target_label_multiplicity);

                    if source_multiplicity.zip(target_multiplicity).is_none() {
                        problems.push(ValidationProblem::Error {
                            uuid: *m.uuid,
                            error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                            text: format!("invalid multiplicities"),
                        });
                    }
                    if let Some((lm1, um1)) = source_multiplicity && um1.is_some_and(|um| lm1 > um) {
                        problems.push(ValidationProblem::Error {
                            uuid: *m.uuid,
                            error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                            text: format!("invalid multiplicities"),
                        });
                    }
                    if let Some((lm2, um2)) = target_multiplicity && um2.is_some_and(|um| lm2 > um) {
                        problems.push(ValidationProblem::Error {
                            uuid: *m.uuid,
                            error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                            text: format!("invalid multiplicities"),
                        });
                    }

                    match m.stereotype.as_str() {
                        ontouml_models::MEDIATION => {
                            if let Some(((lm1, _), (lm2, _))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 < 1 || lm2 < 1) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«mediation» must have multiplicities of at least 1..*"),
                                });
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
                        ontouml_models::CHARACTERIZATION => {
                            if let Some(((lm1, um1), (lm2, _))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 < 1) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«characterization» must have multiplicities of 1..1 and at least 1..*"),
                                });
                            }
                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                let mut e = element_infos.entry(*t.uuid).or_default();

                                if t.stereotype.as_str() != ontouml_models::QUALITY && t.stereotype.as_str() != ontouml_models::MODE {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::TargetStereotype),
                                        text: format!("«characterization» must have «quality» or «mode» on the target end"),
                                    });
                                }

                                e.direct_characterizations_toward += 1;
                            }
                        },
                        ontouml_models::STRUCTURATION => {
                            if let Some(((_, _), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«structuration» must have multiplicities of 1..1 on the target end"),
                                });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != ontouml_models::QUALITY {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::SourceStereotype),
                                        text: format!("«structuration» must have «quality» on the source end"),
                                    });
                                }
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() !=ontouml_models::QUALITY && t.stereotype.as_str() != ontouml_models::MODE {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::TargetStereotype),
                                        text: format!("«structuration» must have «quality» or «mode» on the target end"),
                                    });
                                }
                            }
                        }
                        ontouml_models::COMPONENT_OF => {
                            if let Some(((lm1, _), (_, _))) = source_multiplicity.zip(target_multiplicity)
                                && lm1 < 1 {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«componentOf» must have multiplicities of at least 1..* on the source end"),
                                });
                            }
                        }
                        ontouml_models::SUBCOLLECTION_OF => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«subcollectionOf» must have multiplicities of 1..1"),
                                });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != ontouml_models::COLLECTIVE {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::SourceStereotype),
                                        text: format!("«subcollectionOf» must have «collective» on the source end"),
                                    });
                                }
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != ontouml_models::COLLECTIVE {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::TargetStereotype),
                                        text: format!("«subcollectionOf» must have «collective» on the target end"),
                                    });
                                }
                            }
                        }
                        ontouml_models::MEMBER_OF => {
                            if let Some(((lm1, _), (lm2, _))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 < 1 || lm2 < 1) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«memberOf» must have multiplicities of at least 1..*"),
                                });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != ontouml_models::COLLECTIVE {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::SourceStereotype),
                                        text: format!("«memberOf» must have «collective» on the source end"),
                                    });
                                }
                            }
                        }
                        ontouml_models::CONTAINMENT => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«containment» must have multiplicities of 1..1"),
                                });
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != ontouml_models::QUANTITY {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::TargetStereotype),
                                        text: format!("«containment» must have «quantity» on the target end"),
                                    });
                                }
                            }
                        }
                        ontouml_models::SUBQUANTITY_OF => {
                            if let Some(((lm1, um1), (lm2, um2))) = source_multiplicity.zip(target_multiplicity)
                                && (lm1 != 1 || um1.is_none_or(|um| um != 1) || lm2 != 1 || um2.is_none_or(|um| um != 1)) {
                                problems.push(ValidationProblem::Error {
                                    uuid: *m.uuid,
                                    error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                                    text: format!("«subquantityOf» must have multiplicities of 1..1"),
                                });
                            }

                            if let UmlClassClassifier::UmlClass(s) = &m.source {
                                let s = s.read();
                                if s.stereotype.as_str() != ontouml_models::QUANTITY {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::SourceStereotype),
                                        text: format!("«subquantityOf» must have «quantity» on the source end"),
                                    });
                                }
                            }

                            if let UmlClassClassifier::UmlClass(t) = &m.target {
                                let t = t.read();
                                if t.stereotype.as_str() != ontouml_models::QUANTITY {
                                    problems.push(ValidationProblem::Error {
                                        uuid: *m.uuid,
                                        error_type: ErrorType::InvalidRelation(RelationError::TargetStereotype),
                                        text: format!("«subquantityOf» must have «quantity» on the target end"),
                                    });
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
            fn has_matching_supertype<F: Fn(&ElementInfo) -> bool>(
                infos: &HashMap<ModelUuid, ElementInfo>,
                a: ModelUuid,
                f: &F,
            ) -> bool {
                fn has_matching_supertype_inner<F: Fn(&ElementInfo) -> bool>(
                    visited: &mut HashSet<ModelUuid>,
                    infos: &HashMap<ModelUuid, ElementInfo>,
                    a: ModelUuid,
                    f: &F,
                ) -> bool {
                    if visited.contains(&a) {
                        return false;
                    }
                    visited.insert(a);

                    let e = infos.get(&a).unwrap();
                    let result = e.parents.iter().any(|e| e.read().targets.iter().any(|e| {
                        let uuid = *e.read().uuid;
                        f(infos.get(&uuid).unwrap()) || has_matching_supertype_inner(visited, infos, uuid, f)
                    }));

                    visited.remove(&a);
                    result
                }

                has_matching_supertype_inner(&mut HashSet::new(), infos, a, f)
            }

            if requires_identity(&*info.stereotype) && (info.identity_providers_min != 1 || info.identity_providers_max != 1) {
                problems.push(ValidationProblem::Error {
                    uuid: *k,
                    error_type: ErrorType::InvalidIdentity,
                    text: format!("element does not have exactly one identity provider (found {}..{})", info.identity_providers_min, info.identity_providers_max),
                });
            }
            fn r_lowerbounds(infos: &HashMap<ModelUuid, ElementInfo>, uuid: &ModelUuid) -> usize {
                if let Some(e) = infos.get(uuid) {
                    let mut r = e.direct_mediations_opposing_lower_bounds;

                    for e in &e.parents {
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
            if info.stereotype.as_str() == ontouml_models::ROLE && r_lowerbounds(&element_infos, &k) == 0 {
                problems.push(ValidationProblem::Error {
                    uuid: *k,
                    error_type: ErrorType::InvalidRole,
                    text: format!("«role» must be connected to a «mediation»"),
                });
            }

            if !info.is_abstract
                && (info.stereotype.as_str() == ontouml_models::RELATOR
                    || has_matching_supertype(&element_infos, *k, &|e| e.stereotype.as_str() == ontouml_models::RELATOR))
                && r_lowerbounds(&element_infos, &k) < 2 {
                problems.push(ValidationProblem::Error {
                    uuid: *k,
                    error_type: ErrorType::InvalidRelator,
                    text: format!("«relator» must have sum of lower bounds on the opposite sides of «mediation»s of at least 2"),
                });
            }
            if info.stereotype.as_str() == ontouml_models::PHASE && !info.in_disjoint_complete_set {
                problems.push(ValidationProblem::Error {
                    uuid: *k,
                    error_type: ErrorType::InvalidPhase,
                    text: format!("«phase» must always be part of a generalization set which is disjoint and complete"),
                });
            }
            if (info.stereotype.as_str() == ontouml_models::CATEGORY
                || info.stereotype.as_str() == ontouml_models::MIXIN
                || info.stereotype.as_str() == ontouml_models::PHASE_MIXIN
                || info.stereotype.as_str() == ontouml_models::ROLE_MIXIN) && !info.is_abstract {
                problems.push(ValidationProblem::Error {
                    uuid: *k,
                    error_type: ErrorType::InvalidNonabstractMixin,
                    text: format!("«{}» must always be abstract", info.stereotype),
                });
            }
            if (info.stereotype.as_str() == ontouml_models::QUALITY || info.stereotype.as_str() == ontouml_models::MODE)
                && info.direct_characterizations_toward < 1 {
                problems.push(ValidationProblem::Error {
                    uuid: *k,
                    error_type: ErrorType::InvalidMissingCharacterization,
                    text: format!("«{}» must be at the target end of at least one «characterization»", info.stereotype),
                });
            }
        }

        problems
    }

    fn validate_antipatterns(&self) -> Vec<ValidationProblem> {
        let mut problems = Vec::new();
        let m = self.model.read();

        validate_binover(&mut problems, &m);

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
                    let weight = if m.set_is_disjoint { 1 } else { m.targets.iter().filter(|e| !e.read().is_abstract).count() };
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


        // DepPhase (Relationally Dependent Phase)
        #[derive(Default)]
        struct DepPhaseInfo {
            stereotype: Option<Arc<String>>,
            assoc_mediation_count: usize,
        }
        fn r_depphase_collect(
            infos: &mut HashMap<ModelUuid, DepPhaseInfo>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_depphase_collect(infos, e);
                    }
                },
                UmlClassElement::UmlClass(inner) => {
                    let r = inner.read();
                    infos.entry(*r.uuid).or_default().stereotype = Some(r.stereotype.clone());
                },
                UmlClassElement::UmlClassAssociation(inner) => {
                    let r = inner.read();
                    if *r.stereotype == ontouml_models::MEDIATION {
                        infos.entry(*r.source.uuid()).or_default().assoc_mediation_count += 1;
                        infos.entry(*r.target.uuid()).or_default().assoc_mediation_count += 1;
                    }
                }
                _ => {}
            }
        }
        let mut infos = HashMap::new();
        for e in &m.contained_elements {
            r_depphase_collect(&mut infos, e);
        }
        for e in &infos {
            if let Some(s) = &e.1.stereotype && **s == ontouml_models::PHASE && e.1.assoc_mediation_count >= 1 {
                problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::DepPhase });
            }
        }


        // FreeRole (Free Role Specialization)
        #[derive(Default)]
        struct FreeRoleInfo {
            stereotype: Option<Arc<String>>,
            assoc_mediation_count: usize,
        }
        fn r_freerole_collect(
            infos: &mut HashMap<ModelUuid, FreeRoleInfo>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_freerole_collect(infos, e);
                    }
                },
                UmlClassElement::UmlClass(inner) => {
                    let r = inner.read();
                    infos.entry(*r.uuid).or_default().stereotype = Some(r.stereotype.clone());
                },
                UmlClassElement::UmlClassAssociation(inner) => {
                    let r = inner.read();
                    if *r.stereotype == ontouml_models::MEDIATION {
                        infos.entry(*r.source.uuid()).or_default().assoc_mediation_count += 1;
                        infos.entry(*r.target.uuid()).or_default().assoc_mediation_count += 1;
                    }
                }
                _ => {}
            }
        }
        let mut infos = HashMap::new();
        for e in &m.contained_elements {
            r_freerole_collect(&mut infos, e);
        }
        for e in &infos {
            if let Some(s) = &e.1.stereotype && **s == ontouml_models::ROLE && e.1.assoc_mediation_count == 0 {
                problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::FreeRole });
            }
        }


        // GSRig (Generalization Set with Mixed Rigidity)
        fn r_gsrig_test(
            problems: &mut Vec<ValidationProblem>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_gsrig_test(problems, e);
                    }
                },
                UmlClassElement::UmlClassGeneralization(inner) => {
                    let r = inner.read();
                    let has_rigid_children = r.sources.iter().any(|e| is_rigid(&e.read().stereotype));
                    let has_anti_rigid_children = r.sources.iter().any(|e| is_anti_rigid(&e.read().stereotype));

                    if has_rigid_children && has_anti_rigid_children {
                        problems.push(ValidationProblem::AntiPattern { uuid: *r.uuid, antipattern_type: AntiPatternType::GSRig });
                    }
                }
                _ => {}
            }
        }
        for e in &m.contained_elements {
            r_gsrig_test(&mut problems, e);
        }


        // HetColl (Heterogeneous Collective)
        #[derive(Default)]
        struct HetCollInfo {
            stereotype: Option<Arc<String>>,
            source_end_membership_count: usize,
        }
        fn r_hetcoll_collect(
            infos: &mut HashMap<ModelUuid, HetCollInfo>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_hetcoll_collect(infos, e);
                    }
                },
                UmlClassElement::UmlClass(inner) => {
                    let r = inner.read();
                    infos.entry(*r.uuid).or_default().stereotype = Some(r.stereotype.clone());
                },
                UmlClassElement::UmlClassAssociation(inner) => {
                    let r = inner.read();
                    if *r.stereotype == ontouml_models::MEMBER_OF {
                        infos.entry(*r.source.uuid()).or_default().source_end_membership_count += 1;
                    }
                }
                _ => {}
            }
        }
        let mut infos = HashMap::new();
        for e in &m.contained_elements {
            r_hetcoll_collect(&mut infos, e);
        }
        for e in &infos {
            if let Some(s) = &e.1.stereotype && **s == ontouml_models::COLLECTIVE && e.1.source_end_membership_count > 1 {
                problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::HetColl });
            }
        }


        // HomoFunc (Homogeneous Functional Complex)
        #[derive(Default)]
        struct HomoFuncInfo {
            source_end_component_count: usize,
        }
        fn r_homofunc_collect(
            infos: &mut HashMap<ModelUuid, HomoFuncInfo>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_homofunc_collect(infos, e);
                    }
                },
                UmlClassElement::UmlClass(inner) => {
                    let r = inner.read();
                    infos.entry(*r.uuid).or_default();
                },
                UmlClassElement::UmlClassAssociation(inner) => {
                    let r = inner.read();
                    if *r.stereotype == ontouml_models::COMPONENT_OF {
                        infos.entry(*r.source.uuid()).or_default().source_end_component_count += 1;
                    }
                }
                _ => {}
            }
        }
        let mut infos = HashMap::new();
        for e in &m.contained_elements {
            r_homofunc_collect(&mut infos, e);
        }
        for e in &infos {
            if e.1.source_end_component_count == 1 {
                problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::HomoFunc });
            }
        }


        // MixRig (Mixin With Same Rigidity)
        #[derive(Default)]
        struct MixRigInfo {
            stereotype: Option<Arc<String>>,
            has_rigid_children: bool,
            has_anti_rigid_children: bool,
        }
        fn r_mixrig_collect(
            infos: &mut HashMap<ModelUuid, MixRigInfo>,
            e: &UmlClassElement,
        ) {
            match e {
                UmlClassElement::UmlClassPackage(inner) => {
                    let m = inner.read();
                    for e in &m.contained_elements {
                        r_mixrig_collect(infos, e);
                    }
                },
                UmlClassElement::UmlClass(inner) => {
                    let r = inner.read();
                    infos.entry(*r.uuid).or_default().stereotype = Some(r.stereotype.clone());
                },
                UmlClassElement::UmlClassGeneralization(inner) => {
                    let r = inner.read();
                    let has_rigid_children = r.sources.iter().any(|e| is_rigid(&e.read().stereotype));
                    let has_anti_rigid_children = r.sources.iter().any(|e| is_anti_rigid(&e.read().stereotype));

                    for t in &r.targets {
                        let mut e = infos.entry(*t.read().uuid).or_default();
                        e.has_rigid_children |= has_rigid_children;
                        e.has_anti_rigid_children |= has_anti_rigid_children;
                    }
                }
                _ => {}
            }
        }
        let mut infos = HashMap::new();
        for e in &m.contained_elements {
            r_mixrig_collect(&mut infos, e);
        }
        for e in &infos {
            if let Some(s) = &e.1.stereotype && **s == ontouml_models::MIXIN && e.1.has_rigid_children != e.1.has_anti_rigid_children {
                problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::MixRig });
            }
        }


        // MultDep, RelRig
        validate_relators(&mut problems, &m);
        // UndefFormal, UndefPhase
        validate_undef(&mut problems, &m);


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
                                    commands.push(
                                        SimpleProjectCommand::SpecificDiagramCommand(
                                            self.view_uuid,
                                            DiagramCommand::PanToElement(uuid.into(), false),
                                        ).into()
                                    );
                                }
                            });

                            match rr {
                                ValidationProblem::Error { uuid, text, .. } => {
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

#[derive(PartialEq, Debug)]
enum ValidationProblem {
    Error {
        uuid: ModelUuid,
        error_type: ErrorType,
        text: String,
    },
    AntiPattern {
        uuid: ModelUuid,
        antipattern_type: AntiPatternType,
    },
}

#[derive(PartialEq, Debug)]
enum ErrorType {
    InvalidStereotype,
    InvalidSubtyping,
    InvalidRelation(RelationError),
    InvalidIdentity,
    InvalidMissingCharacterization,
    InvalidPhase,
    InvalidRole,
    InvalidRelator,
    InvalidNonabstractMixin,
}

#[derive(PartialEq, Debug)]
enum RelationError {
    SourceStereotype,
    TargetStereotype,
    Multiplicities,
}

#[derive(PartialEq, Debug)]
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

fn is_rigid(stereotype: &str) -> bool {
    match stereotype {
        ontouml_models::KIND
        | ontouml_models::SUBKIND
        | ontouml_models::COLLECTIVE
        | ontouml_models::QUANTITY
        | ontouml_models::RELATOR
        | ontouml_models::CATEGORY
        | ontouml_models::MODE
        | ontouml_models::QUALITY => true,
        _ => false,
    }
}

fn is_anti_rigid(stereotype: &str) -> bool {
    match stereotype {
        ontouml_models::ROLE
        | ontouml_models::PHASE
        | ontouml_models::PHASE_MIXIN
        | ontouml_models::ROLE_MIXIN => true,
        _ => false,
    }
}


fn validate_binover(
    problems: &mut Vec<ValidationProblem>,
    m: &RwLockReadGuard<'_, UmlClassDiagram>,
) {
    // BinOver (Binary Relation between Overlapping Types)
    #[derive(Default)]
    struct BinOverInfo {
        children: Vec<ERef<UmlClassGeneralization>>,
        parents: Vec<ERef<UmlClassGeneralization>>,
    }
    fn r_binover_collect(
        infos: &mut HashMap<ModelUuid, BinOverInfo>,
        e: &UmlClassElement,
    ) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let m = inner.read();
                for e in &m.contained_elements {
                    r_binover_collect(infos, e);
                }
            },
            UmlClassElement::UmlClass(inner) => {
                infos.entry(*inner.read().uuid).or_default();
            }
            UmlClassElement::UmlClassGeneralization(inner) => {
                let m = inner.read();
                for e in &m.sources {
                    infos.entry(*e.read().uuid).or_default().parents.push(inner.clone());
                }
                for e in &m.targets {
                    infos.entry(*e.read().uuid).or_default().children.push(inner.clone());
                }
            },
            _ => {},
        }
    }
    let mut infos = HashMap::new();
    for e in &m.contained_elements {
        r_binover_collect(&mut infos, e);
    }
    fn r_binover_test(
        problems: &mut Vec<ValidationProblem>,
        infos: &HashMap<ModelUuid, BinOverInfo>,
        e: &UmlClassElement,
    ) {
        fn is_subtype_of(
            infos: &HashMap<ModelUuid, BinOverInfo>,
            a: ModelUuid,
            b: ModelUuid,
        ) -> bool {
            fn is_subtype_of_inner(
                visited: &mut HashSet<ModelUuid>,
                infos: &HashMap<ModelUuid, BinOverInfo>,
                a: ModelUuid,
                b: ModelUuid,
            ) -> bool {
                if visited.contains(&a) {
                    return false;
                }
                visited.insert(a);

                let info = infos.get(&a).unwrap();
                let result = if info.parents.iter().any(|e| e.read().targets.iter().any(|e| *e.read().uuid == b)) {
                    true
                } else {
                    info.parents.iter().any(|e| e.read().targets.iter().any(|e| is_subtype_of_inner(visited, infos, *e.read().uuid, b)))
                };

                visited.remove(&a);
                result
            }

            is_subtype_of_inner(&mut HashSet::new(), infos, a, b)
        }
        fn is_subtype_of_or_self(
            infos: &HashMap<ModelUuid, BinOverInfo>,
            a: ModelUuid,
            b: ModelUuid,
        ) -> bool {
            a == b || is_subtype_of(infos, a, b)
        }
        fn is_nonprovider_sortal(c: &UmlClassClassifier) -> bool {
            match c {
                UmlClassClassifier::UmlClassObject(_) => false,
                UmlClassClassifier::UmlClass(inner) => {
                    [
                        ontouml_models::SUBKIND,
                        ontouml_models::ROLE,
                        ontouml_models::PHASE,
                    ].contains(&&**inner.read().stereotype)
                },
            }
        }
        fn is_relator(c: &UmlClassClassifier) -> bool {
            match c {
                UmlClassClassifier::UmlClassObject(_) => false,
                UmlClassClassifier::UmlClass(inner) => {
                    &*inner.read().stereotype == ontouml_models::RELATOR
                },
            }
        }
        fn is_mode(c: &UmlClassClassifier) -> bool {
            match c {
                UmlClassClassifier::UmlClassObject(_) => false,
                UmlClassClassifier::UmlClass(inner) => {
                    &*inner.read().stereotype == ontouml_models::MODE
                },
            }
        }
        fn is_mixin(c: &UmlClassClassifier) -> bool {
            match c {
                UmlClassClassifier::UmlClassObject(_) => false,
                UmlClassClassifier::UmlClass(inner) => {
                    [
                        ontouml_models::CATEGORY,
                        ontouml_models::PHASE_MIXIN,
                        ontouml_models::ROLE_MIXIN,
                        ontouml_models::MIXIN,
                    ].contains(&&**inner.read().stereotype)
                },
            }
        }
        fn are_disjoint_upwards(
            infos: &HashMap<ModelUuid, BinOverInfo>,
            a: ModelUuid,
            b: ModelUuid,
        ) -> bool {
            fn least_upper_bound(
                infos: &HashMap<ModelUuid, BinOverInfo>,
                a: ModelUuid,
                b: ModelUuid,
            ) -> Option<ERef<UmlClass>> {
                for g in &infos.get(&a).unwrap().parents {
                    for p in &g.read().targets {
                        if is_subtype_of(infos, b, *p.read().uuid) {
                            return Some(p.clone());
                        }
                        if let Some(lub) = least_upper_bound(infos, *p.read().uuid, b) {
                            return Some(lub);
                        }
                    }
                }
                None
            }

            match least_upper_bound(infos, a, b) {
                None => true,
                Some(lub) => {
                    let info = infos.get(&lub.read().uuid).unwrap();
                    let mut found_disjoint_generalization = false;
                    for e in &info.children {
                        let e = e.read();
                        if e.set_is_disjoint
                            && e.sources.iter().find(|e| is_subtype_of_or_self(infos, a, *e.read().uuid)).is_some()
                            && e.sources.iter().find(|e| is_subtype_of_or_self(infos, b, *e.read().uuid)).is_some() {
                            found_disjoint_generalization = true;
                        }
                    }
                    found_disjoint_generalization
                }
            }
        }
        fn are_disjoint_downwards(
            infos: &HashMap<ModelUuid, BinOverInfo>,
            a: ModelUuid,
            b: ModelUuid,
        ) -> bool {
            fn greatest_lower_bound(
                infos: &HashMap<ModelUuid, BinOverInfo>,
                a: ModelUuid,
                b: ModelUuid,
            ) -> Option<ERef<UmlClass>> {
                for g in &infos.get(&a).unwrap().children {
                    for p in &g.read().sources {
                        if is_subtype_of(infos, *p.read().uuid, b) {
                            return Some(p.clone());
                        }
                        if let Some(lub) = greatest_lower_bound(infos, *p.read().uuid, b) {
                            return Some(lub);
                        }
                    }
                }
                None
            }

            match greatest_lower_bound(infos, a, b) {
                None => true,
                Some(glb) => {
                    let info = infos.get(&glb.read().uuid).unwrap();
                    let mut found_disjoint_generalization = false;
                    for e in &info.parents {
                        let e = e.read();
                        if e.set_is_disjoint
                            && e.targets.iter().find(|e| is_subtype_of_or_self(infos, *e.read().uuid, a)).is_some()
                            && e.targets.iter().find(|e| is_subtype_of_or_self(infos, *e.read().uuid, b)).is_some() {
                            found_disjoint_generalization = true;
                        }
                    }
                    found_disjoint_generalization
                }
            }
        }

        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let m = inner.read();
                for e in &m.contained_elements {
                    r_binover_test(problems, infos, e);
                }
            },
            UmlClassElement::UmlClassAssociation(inner) => {
                let m = inner.read();
                if *m.source.uuid() == *m.target.uuid()
                    || is_subtype_of(infos, *m.source.uuid(), *m.target.uuid())
                    || is_subtype_of(infos, *m.target.uuid(), *m.source.uuid())
                    || (is_nonprovider_sortal(&m.source) && is_nonprovider_sortal(&m.target) && !are_disjoint_upwards(infos, *m.source.uuid(), *m.target.uuid()))
                    || (is_relator(&m.source) && is_relator(&m.target) && !are_disjoint_upwards(infos, *m.source.uuid(), *m.target.uuid()))
                    || (is_mode(&m.source) && is_mode(&m.target) && !are_disjoint_upwards(infos, *m.source.uuid(), *m.target.uuid()))
                    || (is_mixin(&m.source) && is_mixin(&m.target) && (
                        !are_disjoint_downwards(infos, *m.source.uuid(), *m.target.uuid())
                        || !are_disjoint_upwards(infos, *m.source.uuid(), *m.target.uuid())
                    ))
                {
                    problems.push(ValidationProblem::AntiPattern { uuid: *m.uuid, antipattern_type: AntiPatternType::BinOver });
                }
            },
            _ => {},
        }
    }
    for e in &m.contained_elements {
        r_binover_test(problems, &mut infos, e);
    }
}

fn validate_relators(
    problems: &mut Vec<ValidationProblem>,
    m: &RwLockReadGuard<'_, UmlClassDiagram>,
) {
    // MultDep (Multiple Relational Dependency), RelRig (Relator Mediating Rigid Types)
    #[derive(Default)]
    struct Info {
        stereotype: Option<Arc<String>>,
        has_associated_rigids: bool,
        parents: Vec<ERef<UmlClassGeneralization>>,
        associated_relators: Vec<ModelUuid>,
    }
    fn is_relator(
        infos: &HashMap<ModelUuid, Info>,
        a: ModelUuid,
    ) -> bool {
        fn is_relator_inner(
            visited: &mut HashSet<ModelUuid>,
            infos: &HashMap<ModelUuid, Info>,
            a: ModelUuid,
        ) -> bool {
            if visited.contains(&a) {
                return false;
            }
            visited.insert(a);

            let e = infos.get(&a).unwrap();
            let result = if let Some(s) = &e.stereotype && **s == ontouml_models::RELATOR {
                true
            } else {
                e.parents.iter().any(|e| e.read().targets.iter().any(|e| is_relator_inner(visited, infos, *e.read().uuid)))
            };

            visited.remove(&a);
            result
        }

        is_relator_inner(&mut HashSet::new(), infos, a)
    }

    fn r_collect1(
        infos: &mut HashMap<ModelUuid, Info>,
        e: &UmlClassElement,
    ) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let m = inner.read();
                for e in &m.contained_elements {
                    r_collect1(infos, e);
                }
            },
            UmlClassElement::UmlClass(inner) => {
                let r = inner.read();
                infos.entry(*r.uuid).or_default().stereotype = Some(r.stereotype.clone());
            },
            UmlClassElement::UmlClassGeneralization(inner) => {
                let r = inner.read();
                for e in &r.sources {
                    infos.entry(*e.read().uuid).or_default().parents.push(inner.clone());
                }
            }
            _ => {}
        }
    }
    let mut infos = HashMap::new();
    for e in &m.contained_elements {
        r_collect1(&mut infos, e);
    }
    fn r_collect2(
        infos: &mut HashMap<ModelUuid, Info>,
        e: &UmlClassElement,
    ) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let m = inner.read();
                for e in &m.contained_elements {
                    r_collect2(infos, e);
                }
            },
            UmlClassElement::UmlClassAssociation(inner) => {
                let r = inner.read();
                if *r.stereotype == ontouml_models::MEDIATION {
                    if let UmlClassClassifier::UmlClass(s) = &r.source
                        && is_relator(&infos, *s.read().uuid) {
                        infos.entry(*r.target.uuid()).or_default().associated_relators.push(*s.read().uuid);
                    }
                    if let UmlClassClassifier::UmlClass(t) = &r.target
                        && is_relator(&infos, *t.read().uuid) {
                        infos.entry(*r.source.uuid()).or_default().associated_relators.push(*t.read().uuid);
                    }

                    if let UmlClassClassifier::UmlClass(s) = &r.source
                        && is_rigid(&s.read().stereotype) {
                        infos.entry(*r.target.uuid()).or_default().has_associated_rigids = true;
                    }
                    if let UmlClassClassifier::UmlClass(t) = &r.target
                        && is_rigid(&t.read().stereotype) {
                        infos.entry(*r.source.uuid()).or_default().has_associated_rigids = true;
                    }
                }
            }
            _ => {}
        }
    }
    for e in &m.contained_elements {
        r_collect2(&mut infos, e);
    }
    for e in &infos {
        // TODO: test they are not ancestors?
        if e.1.associated_relators.len() > 1 {
            problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::MultDep });
        }

        if e.1.has_associated_rigids && is_relator(&infos, *e.0) {
            problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::RelRig });
        }
    }
}

fn validate_undef(
    problems: &mut Vec<ValidationProblem>,
    m: &RwLockReadGuard<'_, UmlClassDiagram>,
) {
    // UndefFormal (Undefined Formal Association), UndefPhase (Undefined Phase Partition)
    #[derive(Default)]
    struct UndefInfo {
        stereotype: Option<Arc<String>>,
        has_intrinsics: bool,
        parents: Vec<ERef<UmlClassGeneralization>>,
    }
    fn r_undef_collect(
        infos: &mut HashMap<ModelUuid, UndefInfo>,
        e: &UmlClassElement,
    ) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let m = inner.read();
                for e in &m.contained_elements {
                    r_undef_collect(infos, e);
                }
            },
            UmlClassElement::UmlClass(inner) => {
                let r = inner.read();
                let mut e = infos.entry(*r.uuid).or_default();
                e.stereotype = Some(r.stereotype.clone());
                if !r.properties.is_empty() {
                    e.has_intrinsics = true;
                }
            },
            UmlClassElement::UmlClassAssociation(inner) => {
                let r = inner.read();
                if *r.stereotype == ontouml_models::CHARACTERIZATION {
                    infos.entry(*r.source.uuid()).or_default().has_intrinsics = true;
                }
            }
            UmlClassElement::UmlClassGeneralization(inner) => {
                let r = inner.read();
                for e in &r.sources {
                    infos.entry(*e.read().uuid()).or_default().parents.push(inner.clone());
                }
            }
            _ => {}
        }
    }
    fn has_intrinsics_including_transitively(
        infos: &HashMap<ModelUuid, UndefInfo>,
        e: ModelUuid,
    ) -> bool {
        fn inner(
            visited: &mut HashSet<ModelUuid>,
            infos: &HashMap<ModelUuid, UndefInfo>,
            e: ModelUuid,
        ) -> bool {
            if visited.contains(&e) {
                return false;
            }
            visited.insert(e);

            let e2 = infos.get(&e).unwrap();
            let result = if e2.has_intrinsics {
                true
            } else {
                e2.parents.iter().any(|e| e.read().targets.iter().any(|e| inner(visited, infos, *e.read().uuid)))
            };

            visited.remove(&e);
            result
        }

        inner(&mut HashSet::new(), infos, e)
    }
    let mut infos = HashMap::new();
    for e in &m.contained_elements {
        r_undef_collect(&mut infos, e);
    }
    for e in &infos {
        if let Some(s) = &e.1.stereotype && **s == ontouml_models::PHASE
            && !e.1.parents.iter().any(|e| e.read().targets.iter().any(|e| has_intrinsics_including_transitively(&infos, *e.read().uuid))) {
            problems.push(ValidationProblem::AntiPattern { uuid: *e.0, antipattern_type: AntiPatternType::UndefPhase });
        }
    }
    fn r_undefformal_test(
        problems: &mut Vec<ValidationProblem>,
        infos: &HashMap<ModelUuid, UndefInfo>,
        e: &UmlClassElement,
    ) {
        match e {
            UmlClassElement::UmlClassPackage(inner) => {
                let m = inner.read();
                for e in &m.contained_elements {
                    r_undefformal_test(problems, infos, e);
                }
            },
            UmlClassElement::UmlClassAssociation(inner) => {
                let r = inner.read();
                if *r.stereotype == ontouml_models::FORMAL
                    && (!has_intrinsics_including_transitively(infos, *r.source.uuid())
                        || !has_intrinsics_including_transitively(infos, *r.target.uuid())){
                    problems.push(ValidationProblem::AntiPattern { uuid: *r.uuid, antipattern_type: AntiPatternType::UndefFormal });
                }
            }
            _ => {}
        }
    }
    for e in &m.contained_elements {
        r_undefformal_test(problems, &infos, e);
    }
}


mod test {
    use uuid::uuid;

    use crate::domains::umlclass::{umlclass_controllers::UmlClassLabelProvider, umlclass_models::{UmlClass, UmlClassAssociation}};

    use super::*;

    fn generate_modeluuid(id: u32) -> ModelUuid {
        uuid::Uuid::from_u128(id as u128).into()
    }

    fn new_class(id: u32, stereotype: &'static str, is_abstract: bool) -> ERef<UmlClass> {
        ERef::new(
            UmlClass::new(
                generate_modeluuid(id),
                "".to_owned(),
                stereotype.to_owned(),
                "".to_owned(),
                is_abstract,
                Vec::new(),
                Vec::new(),
            )
        )
    }

    fn new_generalization(
        id: u32,
        sources: Vec<ERef<UmlClass>>, targets: Vec<ERef<UmlClass>>,
        is_disjoint: bool, is_covering: bool,
    ) ->ERef<UmlClassGeneralization> {
        let mut g = UmlClassGeneralization::new(
            generate_modeluuid(id),
            sources,
            targets,
        );
        g.set_is_disjoint = is_disjoint;
        g.set_is_covering = is_covering;

        ERef::new(g)
    }

    fn new_association(
        id: u32,
        stereotype: &'static str,
        source: UmlClassClassifier,
        target: UmlClassClassifier,
    ) -> ERef<UmlClassAssociation> {
        ERef::new(
            UmlClassAssociation::new(
                generate_modeluuid(id),
                stereotype.to_owned(),
                "".to_owned(),
                source,
                target,
            )
        )
    }

    fn new_diagram(elements: Vec<UmlClassElement>) -> ERef<UmlClassDiagram> {
        ERef::new(
            UmlClassDiagram::new(
                uuid!("10000000-0000-0000-0000-000000000000").into(),
                "Test OntoUML Diagram".to_owned(),
                elements,
            )
        )
    }

    fn validate(elements: Vec<UmlClassElement>, check_errors: bool, check_antipatterns: bool) -> Vec<ValidationProblem> {
        let d = new_diagram(elements);
        let vt = super::OntoUMLValidationTab::new(d, ERef::new(UmlClassLabelProvider::default()), uuid::Uuid::nil().into());
        vt.validate(check_errors, check_antipatterns)
    }

    fn call_validate_binover(elements: Vec<UmlClassElement>) -> Vec<ValidationProblem> {
        let d = new_diagram(elements);
        let mut problems = Vec::new();
        validate_binover(&mut problems, &d.read());
        problems
    }
    fn call_validate_undef(elements: Vec<UmlClassElement>) -> Vec<ValidationProblem> {
        let d = new_diagram(elements);
        let mut problems = Vec::new();
        validate_undef(&mut problems, &d.read());
        problems
    }

    // Structure validations tests

    #[test]
    fn test_valid_stereotypes() {
        let kind = new_class(1, ontouml_models::KIND, false);
        let assoc = new_association(2, ontouml_models::COMPONENT_OF, kind.clone().into(), kind.clone().into());
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            validate(vec![kind.into(), assoc.into()], true, false),
            vec![],
        );
    }

    #[test]
    fn test_invalid_stereotypes1() {
        // missing stereotypes
        let kind = new_class(1, ontouml_models::NONE, false);
        let kind_uuid = *kind.read().uuid;
        let assoc = new_association(2, ontouml_models::NONE, kind.clone().into(), kind.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        let result = validate(vec![kind.into(), assoc.into()], true, false);
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidStereotype,
                    ..
                } if *uuid == kind_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidStereotype,
                    ..
                } if *uuid == assoc_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_stereotypes2() {
        // swapped stereotype domains
        let kind = new_class(3, ontouml_models::COMPONENT_OF, false);
        let kind_uuid = *kind.read().uuid;
        let assoc = new_association(4, ontouml_models::KIND, kind.clone().into(), kind.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        let result = validate(vec![kind.into(), assoc.into()], true, false);
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidStereotype,
                    ..
                } if *uuid == kind_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidStereotype,
                    ..
                } if *uuid == assoc_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_stereotypes3() {
        // garbage stereotypes
        let kind = new_class(5, "hello", false);
        let kind_uuid = *kind.read().uuid;
        let assoc = new_association(6, "world", kind.clone().into(), kind.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        let result = validate(vec![kind.into(), assoc.into()], true, false);
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidStereotype,
                    ..
                } if *uuid == kind_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidStereotype,
                    ..
                } if *uuid == assoc_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_subtyping() {
        let subkind = new_class(1, ontouml_models::SUBKIND, false);
        let kind = new_class(2, ontouml_models::KIND, false);
        let generalization = new_generalization(3, vec![subkind.clone()], vec![kind.clone()], true, true);

        assert_eq!(
            validate(vec![subkind.into(), kind.into(), generalization.into()], true, false),
            vec![],
        );
    }

    #[test]
    fn test_invalid_subtyping() {
        let subkind = new_class(1, ontouml_models::SUBKIND, false);
        let kind = new_class(2, ontouml_models::KIND, false);
        let generalization = new_generalization(3, vec![kind.clone()], vec![subkind.clone()], true, true);
        let gen_uuid = *generalization.read().uuid;

        assert!(
            validate(vec![subkind.into(), kind.into(), generalization.into()], true, false)
                .iter().find(|e| matches!(e, ValidationProblem::Error { uuid, error_type: ErrorType::InvalidSubtyping, .. } if *uuid == gen_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_relation1() {
        let quality = new_class(1, ontouml_models::QUALITY, false);
        let collective = new_class(2, ontouml_models::COLLECTIVE, false);
        let memberOf = new_association(3, ontouml_models::MEMBER_OF, collective.clone().into(), quality.clone().into());
        memberOf.write().source_label_multiplicity = Arc::new("1".to_owned());
        memberOf.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            quality.into(), collective.into(), memberOf.into(),
        ];

        assert_eq!(
            validate(elements, true, false)
                // for the sake of simplicity, ignore the MissingCharacterization errors
                .into_iter().filter(|e| !matches!(e, ValidationProblem::Error { error_type: ErrorType::InvalidMissingCharacterization, .. }))
                .collect::<Vec<ValidationProblem>>(),
            vec![],
        );
    }

    #[test]
    fn test_valid_relation2() {
        let mode = new_class(4, ontouml_models::MODE, false);
        let quantity = new_class(5, ontouml_models::QUANTITY, false);
        let containment = new_association(6, ontouml_models::CONTAINMENT, mode.clone().into(), quantity.clone().into());
        containment.write().source_label_multiplicity = Arc::new("1".to_owned());
        containment.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            mode.into(), quantity.into(), containment.into(),
        ];

        assert_eq!(
            validate(elements, true, false)
                // for the sake of simplicity, ignore the MissingCharacterization errors
                .into_iter().filter(|e| !matches!(e, ValidationProblem::Error { error_type: ErrorType::InvalidMissingCharacterization, .. }))
                .collect::<Vec<ValidationProblem>>(),
            vec![],
        );
    }

    #[test]
    fn test_invalid_relation1() {
        let quality = new_class(1, ontouml_models::QUALITY, false);
        let collective = new_class(2, ontouml_models::COLLECTIVE, false);
        let memberOf = new_association(3, ontouml_models::MEMBER_OF, quality.clone().into(), collective.clone().into());
        memberOf.write().source_label_multiplicity = Arc::new("1".to_owned());
        memberOf.write().target_label_multiplicity = Arc::new("1".to_owned());
        let memberOf_uuid = *memberOf.read().uuid;

        let elements = vec![
            quality.into(), collective.into(), memberOf.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidRelation(RelationError::SourceStereotype),
                    ..
                } if *uuid == memberOf_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_relation2() {
        let quality = new_class(1, ontouml_models::QUALITY, false);
        let mode = new_class(4, ontouml_models::MODE, false);
        let containment = new_association(5, ontouml_models::CONTAINMENT, quality.clone().into(), mode.clone().into());
        containment.write().source_label_multiplicity = Arc::new("1".to_owned());
        containment.write().target_label_multiplicity = Arc::new("1".to_owned());
        let containment_uuid = *containment.read().uuid;

        let elements = vec![
            quality.into(), mode.into(), containment.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid,
                    error_type: ErrorType::InvalidRelation(RelationError::TargetStereotype),
                    ..
                } if *uuid == containment_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_identity1() {
        // subkind, phase, role subtype of kind
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let subkind1 = new_class(2, ontouml_models::SUBKIND, false);
        let phase = new_class(3, ontouml_models::PHASE, false);
        let role = new_class(4, ontouml_models::ROLE, false);
        let generalization1 = new_generalization(5, vec![subkind1.clone(), phase.clone(), role.clone()], vec![kind1.clone()], true, true);

        let elements = vec![
            kind1.into(), subkind1.into(), phase.into(), role.into(), generalization1.into(),
        ];

        assert_eq!(
            validate(elements, true, false)
                // for the sake of simplicity, ignore missing dependencies
                .into_iter().filter(|e| !matches!(e, ValidationProblem::Error { error_type: ErrorType::InvalidPhase | ErrorType::InvalidRole, .. }))
                .collect::<Vec<ValidationProblem>>(),
            vec![],
        );
    }

    #[test]
    fn test_valid_identity2() {
        // disjoint generalization set of kinds
        let kind2 = new_class(6, ontouml_models::KIND, false);
        let kind3 = new_class(7, ontouml_models::KIND, false);
        let subkind2 = new_class(8, ontouml_models::SUBKIND, false);
        let generalization2 = new_generalization(9, vec![subkind2.clone()], vec![kind2.clone(), kind3.clone()], true, true);

        let elements = vec![
            kind2.into(), kind3.into(), subkind2.into(), generalization2.into(),
        ];

        assert_eq!(
            validate(elements, true, false)
                // for the sake of simplicity, ignore missing dependencies
                .into_iter().filter(|e| !matches!(e, ValidationProblem::Error { error_type: ErrorType::InvalidPhase | ErrorType::InvalidRole, .. }))
                .collect::<Vec<ValidationProblem>>(),
            vec![],
        );
    }

    #[test]
    fn test_invalid_identity1() {
        // kind subtype of subkind (with no identity provider)
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let kind1_uuid = *kind1.read().uuid;
        let subkind1 = new_class(2, ontouml_models::SUBKIND, false);
        let subkind1_uuid = *subkind1.read().uuid;
        let generalization1 = new_generalization(3, vec![kind1.clone()], vec![subkind1.clone()], true, true);

        let elements = vec![
            kind1.into(), subkind1.into(), generalization1.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidIdentity,
                    ..
                } if *uuid == kind1_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidIdentity,
                    ..
                } if *uuid == subkind1_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_identity2() {
        // disjoint generalization set where one possibility does not provide identity
        let kind2 = new_class(4, ontouml_models::KIND, false);
        let category = new_class(5, ontouml_models::CATEGORY, true);
        let subkind2 = new_class(6, ontouml_models::SUBKIND, false);
        let subkind2_uuid = *subkind2.read().uuid;
        let generalization2 = new_generalization(7, vec![subkind2.clone()], vec![kind2.clone(), category.clone()], true, true);

        let elements = vec![
            kind2.into(), category.into(), subkind2.into(), generalization2.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidIdentity,
                    ..
                } if *uuid == subkind2_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_missing_characterization() {
        // kind characterized by quality
        let kind = new_class(1, ontouml_models::KIND, false);
        let mode = new_class(2, ontouml_models::MODE, false);
        let mode_uuid = *mode.read().uuid;
        let characterization1 = new_association(3, ontouml_models::CHARACTERIZATION, kind.clone().into(), mode.clone().into());
        characterization1.write().source_label_multiplicity = Arc::new("1".to_owned());
        characterization1.write().target_label_multiplicity = Arc::new("1".to_owned());
        let quality = new_class(4, ontouml_models::QUALITY, false);
        let quality_uuid = *quality.read().uuid;
        let characterization2 = new_association(5, ontouml_models::CHARACTERIZATION, kind.clone().into(), quality.clone().into());
        characterization2.write().source_label_multiplicity = Arc::new("1".to_owned());
        characterization2.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            kind.into(), mode.into(), characterization1.into(),
            quality.into(), characterization2.into(),
        ];
        assert_eq!(validate(elements, true, false), vec![]);
    }

    #[test]
    fn test_invalid_missing_characterization() {
        let mode = new_class(1, ontouml_models::MODE, false);
        let mode_uuid = *mode.read().uuid;
        let quality = new_class(2, ontouml_models::QUALITY, false);
        let quality_uuid = *quality.read().uuid;

        let elements = vec![
            mode.into(), quality.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidMissingCharacterization,
                    ..
                } if *uuid == mode_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidMissingCharacterization,
                    ..
                } if *uuid == quality_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_phase() {
        // phase in a generalization set (disjoint, complete) from kind
        let kind = new_class(1, ontouml_models::KIND, false);
        let phase = new_class(2, ontouml_models::PHASE, false);
        let phase_uuid = *phase.read().uuid;
        let generalization = new_generalization(3, vec![phase.clone()], vec![kind.clone()], true, true);

        let elements = vec![
            kind.into(), phase.into(), generalization.into(),
        ];
        assert_eq!(validate(elements, true, false), vec![]);
    }

    #[test]
    fn test_invalid_phase1() {
        // phase without a generalization set
        let phase1 = new_class(1, ontouml_models::PHASE, false);
        let phase1_uuid = *phase1.read().uuid;

        let elements = vec![
            phase1.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidPhase,
                    ..
                } if *uuid == phase1_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_phase2() {
        // phase in a non-partition generalization set
        let kind = new_class(2, ontouml_models::KIND, false);
        let phase2 = new_class(3, ontouml_models::PHASE, false);
        let phase2_uuid = *phase2.read().uuid;
        let generalization = new_generalization(4, vec![phase2.clone()], vec![kind.clone()], false, false);

        let elements = vec![
            kind.into(), phase2.into(), generalization.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidPhase,
                    ..
                } if *uuid == phase2_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_role() {
        // role that has mediation
        let kind = new_class(1, ontouml_models::KIND, false);
        let role = new_class(2, ontouml_models::ROLE, false);
        let generalization = new_generalization(3, vec![role.clone()], vec![kind.clone()], true, true);
        let role_uuid = *role.read().uuid;
        let relator = new_class(4, ontouml_models::RELATOR, false);
        let mediation = new_association(5, ontouml_models::MEDIATION, role.clone().into(), relator.clone().into());
        mediation.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            kind.into(), role.into(), generalization.into(),
            relator.into(), mediation.into(),
        ];

        assert_eq!(
            validate(elements, true, false)
                // for the sake of simplicity, ignore relator errors
                .into_iter().filter(|e| !matches!(e, ValidationProblem::Error { error_type: ErrorType::InvalidRelator, .. }))
                .collect::<Vec<ValidationProblem>>(),
            vec![],
        );
    }

    #[test]
    fn test_invalid_role() {
        // role without a mediation
        let role = new_class(1, ontouml_models::ROLE, false);
        let role_uuid = *role.read().uuid;

        let result = validate(vec![role.into()], true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidRole,
                    ..
                } if *uuid == role_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_relator1() {
        // relator with one mediation with multiplicity >1
        let relator2 = new_class(2, ontouml_models::RELATOR, false);
        let relator2_uuid = *relator2.read().uuid;
        let kind1 = new_class(3, ontouml_models::KIND, false);
        let mediation1 = new_association(4, ontouml_models::MEDIATION, kind1.clone().into(), relator2.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("2".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            relator2.into(), kind1.into(), mediation1.into(),
        ];
        assert_eq!(validate(elements, true, false), vec![]);
    }

    #[test]
    fn test_valid_relator2() {
        // relator with two mediations with multiplicity =1
        let relator3 = new_class(5, ontouml_models::RELATOR, false);
        let kind2 = new_class(6, ontouml_models::KIND, false);
        let mediation2 = new_association(7, ontouml_models::MEDIATION, kind2.clone().into(), relator3.clone().into());
        mediation2.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation2.write().target_label_multiplicity = Arc::new("1".to_owned());
        let kind3 = new_class(8, ontouml_models::KIND, false);
        let mediation3 = new_association(9, ontouml_models::MEDIATION, kind3.clone().into(), relator3.clone().into());
        let mediation3_uuid = *mediation3.read().uuid;
        mediation3.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation3.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            relator3.into(), kind2.into(), mediation2.into(), kind3.into(), mediation3.into(),
        ];
        assert_eq!(validate(elements, true, false), vec![]);
    }

    #[test]
    fn test_valid_relator3() {
        // abstract parent relator
        let relator = new_class(1, ontouml_models::RELATOR, true);
        let subkind = new_class(2, ontouml_models::SUBKIND, false);
        let gen1 = new_generalization(3, vec![subkind.clone()], vec![relator.clone()], true, true);
        let kind1 = new_class(4, ontouml_models::KIND, false);
        let mediation1 = new_association(5, ontouml_models::MEDIATION, kind1.clone().into(), relator.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("1".to_owned());
        let kind2 = new_class(6, ontouml_models::KIND, false);
        let mediation2 = new_association(7, ontouml_models::MEDIATION, kind2.clone().into(), subkind.clone().into());
        mediation2.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation2.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            relator.into(), subkind.into(), gen1.into(),
            kind1.into(), mediation1.into(),
            kind2.into(), mediation2.into(),
        ];
        assert_eq!(validate(elements, true, false), vec![]);
    }

    #[test]
    fn test_invalid_relator1() {
        // relator with no mediation
        let relator1 = new_class(1, ontouml_models::RELATOR, false);
        let relator1_uuid = *relator1.read().uuid;

        let elements = vec![
            relator1.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidRelator,
                    ..
                } if *uuid == relator1_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_relator2() {
        // relator with one mediation with multiplicity =1
        let relator2 = new_class(2, ontouml_models::RELATOR, false);
        let relator2_uuid = *relator2.read().uuid;
        let kind1 = new_class(3, ontouml_models::KIND, false);
        let mediation1 = new_association(4, ontouml_models::MEDIATION, kind1.clone().into(), relator2.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            relator2.into(), kind1.into(), mediation1.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidRelator,
                    ..
                } if *uuid == relator2_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_relator3() {
        // relator with two mediations with multiplicity 0..1
        let relator3 = new_class(5, ontouml_models::RELATOR, false);
        let relator3_uuid = *relator3.read().uuid;
        let kind2 = new_class(6, ontouml_models::KIND, false);
        let mediation2 = new_association(7, ontouml_models::MEDIATION, kind2.clone().into(), relator3.clone().into());
        let mediation2_uuid = *mediation2.read().uuid;
        mediation2.write().source_label_multiplicity = Arc::new("0..1".to_owned());
        mediation2.write().target_label_multiplicity = Arc::new("1".to_owned());
        let kind3 = new_class(8, ontouml_models::KIND, false);
        let mediation3 = new_association(9, ontouml_models::MEDIATION, kind3.clone().into(), relator3.clone().into());
        mediation3.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation3.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            relator3.into(), kind2.into(), mediation2.into(), kind3.into(), mediation3.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidRelator,
                    ..
                } if *uuid == relator3_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidRelation(RelationError::Multiplicities),
                    ..
                } if *uuid == mediation2_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_relator4() {
        // abstract parent relator
        let relator = new_class(1, ontouml_models::RELATOR, true);
        let subkind = new_class(2, ontouml_models::SUBKIND, false);
        let subkind_uuid = *subkind.read().uuid;
        let gen1 = new_generalization(3, vec![subkind.clone()], vec![relator.clone()], true, true);
        let kind1 = new_class(4, ontouml_models::KIND, false);
        let mediation1 = new_association(5, ontouml_models::MEDIATION, kind1.clone().into(), relator.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("1".to_owned());

        let elements = vec![
            relator.into(), subkind.into(), gen1.into(),
            kind1.into(), mediation1.into(),
        ];
        let result = validate(elements, true, false);
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidRelator,
                    ..
                } if *uuid == subkind_uuid)).is_some()
        );
    }

    #[test]
    fn test_valid_mixins() {
        let category = new_class(1, ontouml_models::CATEGORY, true);
        let mixin = new_class(2, ontouml_models::MIXIN, true);
        let roleMixin = new_class(3, ontouml_models::ROLE_MIXIN, true);
        let phaseMixin = new_class(4, ontouml_models::PHASE_MIXIN, true);

        let elements = vec![
            category.into(), mixin.into(), roleMixin.into(), phaseMixin.into(),
        ];
        assert_eq!(validate(elements, true, false), vec![]);
    }

    #[test]
    fn test_invalid_mixins() {
        let category = new_class(1, ontouml_models::CATEGORY, false);
        let category_uuid = *category.read().uuid;
        let mixin = new_class(2, ontouml_models::MIXIN, false);
        let mixin_uuid = *mixin.read().uuid;
        let roleMixin = new_class(3, ontouml_models::ROLE_MIXIN, false);
        let roleMixin_uuid = *roleMixin.read().uuid;
        let phaseMixin = new_class(4, ontouml_models::PHASE_MIXIN, false);
        let phaseMixin_uuid = *phaseMixin.read().uuid;

        let elements = vec![
            category.into(), mixin.into(), roleMixin.into(), phaseMixin.into(),
        ];
        let result = validate(elements, true, false);

        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidNonabstractMixin,
                    ..
                } if *uuid == category_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidNonabstractMixin,
                    ..
                } if *uuid == mixin_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidNonabstractMixin,
                    ..
                } if *uuid == roleMixin_uuid)).is_some()
        );
        assert!(
            result
                .iter().find(|e| matches!(e, ValidationProblem::Error {
                    uuid, error_type: ErrorType::InvalidNonabstractMixin,
                    ..
                } if *uuid == phaseMixin_uuid)).is_some()
        );
    }

    // TODO: Antipatterns validations tests

    #[test]
    fn test_valid_binover1() {
        // unconnected
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let kind2 = new_class(2, ontouml_models::KIND, false);
        let assoc = new_association(3, ontouml_models::COMPONENT_OF, kind1.clone().into(), kind2.clone().into());
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![kind1.into(), kind2.into(), assoc.into()]),
            vec![],
        );
    }

    #[test]
    fn test_valid_binover2() {
        // disjoint
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let subkind1 = new_class(2, ontouml_models::SUBKIND, false);
        let subkind2 = new_class(3, ontouml_models::SUBKIND, false);
        let gen1 = new_generalization(4, vec![subkind1.clone().into(), subkind2.clone().into()], vec![kind1.clone().into()], true, true);
        let assoc = new_association(5, ontouml_models::COMPONENT_OF, subkind1.clone().into(), subkind2.clone().into());
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![kind1.into(), subkind1.into(), subkind2.into(), gen1.into(), assoc.into()]),
            vec![],
        );
    }

    #[test]
    fn test_valid_binover3() {
        // mixins with no common elements
        let mixin1 = new_class(1, ontouml_models::MIXIN, true);
        let mixin2 = new_class(2, ontouml_models::MIXIN, true);
        let assoc = new_association(3, ontouml_models::COMPONENT_OF, mixin1.clone().into(), mixin2.clone().into());
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![mixin1.into(), mixin2.into(), assoc.into()]),
            vec![],
        );
    }

    #[test]
    fn test_invalid_binover1() {
        // self
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let assoc = new_association(2, ontouml_models::COMPONENT_OF, kind1.clone().into(), kind1.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![kind1.into(), assoc.into()]),
            vec![
                ValidationProblem::AntiPattern { uuid: assoc_uuid, antipattern_type: AntiPatternType::BinOver }
            ],
        );
    }

    #[test]
    fn test_invalid_binover2() {
        // subtype
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let subkind1 = new_class(2, ontouml_models::SUBKIND, false);
        let gen1 = new_generalization(3, vec![subkind1.clone().into()], vec![kind1.clone().into()], false, true);
        let assoc = new_association(4, ontouml_models::COMPONENT_OF, kind1.clone().into(), subkind1.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![kind1.into(), subkind1.into(), gen1.into(), assoc.into()]),
            vec![
                ValidationProblem::AntiPattern { uuid: assoc_uuid, antipattern_type: AntiPatternType::BinOver }
            ],
        );
    }

    #[test]
    fn test_invalid_binover3() {
        // multiple
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let subkind1 = new_class(2, ontouml_models::SUBKIND, false);
        let subkind2 = new_class(3, ontouml_models::SUBKIND, false);
        let gen1 = new_generalization(4, vec![subkind1.clone().into()], vec![kind1.clone().into()], false, true);
        let gen2 = new_generalization(5, vec![subkind2.clone().into()], vec![kind1.clone().into()], false, true);
        let assoc = new_association(6, ontouml_models::COMPONENT_OF, subkind1.clone().into(), subkind2.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![kind1.into(), subkind1.into(), subkind2.into(), gen1.into(), gen2.into(), assoc.into()]),
            vec![
                ValidationProblem::AntiPattern { uuid: assoc_uuid, antipattern_type: AntiPatternType::BinOver }
            ],
        );
    }

    #[test]
    fn test_invalid_binover4() {
        // overlapping
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let subkind1 = new_class(2, ontouml_models::SUBKIND, false);
        let subkind2 = new_class(3, ontouml_models::SUBKIND, false);
        let gen1 = new_generalization(4, vec![subkind1.clone().into(), subkind2.clone().into()], vec![kind1.clone().into()], false, true);
        let assoc = new_association(5, ontouml_models::COMPONENT_OF, subkind1.clone().into(), subkind2.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![kind1.into(), subkind1.into(), subkind2.into(), gen1.into(), assoc.into()]),
            vec![
                ValidationProblem::AntiPattern { uuid: assoc_uuid, antipattern_type: AntiPatternType::BinOver }
            ],
        );
    }

    #[test]
    fn test_invalid_binover5() {
        // mixins generalizing common sortal
        let mixin1 = new_class(1, ontouml_models::MIXIN, true);
        let mixin2 = new_class(2, ontouml_models::MIXIN, true);
        let kind1 = new_class(3, ontouml_models::KIND, false);
        let gen1 = new_generalization(4, vec![kind1.clone()], vec![mixin1.clone()], false, false);
        let gen2 = new_generalization(5, vec![kind1.clone()], vec![mixin2.clone()], false, false);
        let assoc = new_association(6, ontouml_models::COMPONENT_OF, mixin1.clone().into(), mixin2.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert!(
            call_validate_binover(vec![
                mixin1.into(), mixin2.into(),
                kind1.into(), gen1.into(), gen2.into(),
                assoc.into(),
            ])
            .iter().find(|e| matches!(e, ValidationProblem::AntiPattern {
                uuid, antipattern_type: AntiPatternType::BinOver,
                ..
            } if *uuid == assoc_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_binover6() {
        // mixins generalized by a common mixin
        let mixin1 = new_class(1, ontouml_models::MIXIN, true);
        let mixin2 = new_class(2, ontouml_models::MIXIN, true);
        let mixin3 = new_class(3, ontouml_models::MIXIN, true);
        let gen1 = new_generalization(4, vec![mixin2.clone()], vec![mixin1.clone()], false, false);
        let gen2 = new_generalization(5, vec![mixin3.clone()], vec![mixin1.clone()], false, false);
        let assoc = new_association(6, ontouml_models::COMPONENT_OF, mixin2.clone().into(), mixin3.clone().into());
        let assoc_uuid = *assoc.read().uuid;
        assoc.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            call_validate_binover(vec![
                mixin1.into(),
                mixin2.into(), mixin3.into(),
                gen1.into(), gen2.into(),
                assoc.into(),
            ]),
            vec![
                ValidationProblem::AntiPattern { uuid: assoc_uuid, antipattern_type: AntiPatternType::BinOver }
            ],
        );
    }

    #[test]
    fn test_valid_decint() {
        // kind <- subkind -> category
        let kind = new_class(1, ontouml_models::KIND, false);
        let subkind = new_class(2, ontouml_models::SUBKIND, false);
        let category = new_class(3, ontouml_models::CATEGORY, true);
        let gen1 = new_generalization(4, vec![subkind.clone().into()], vec![kind.clone().into()], false, false);
        let gen2 = new_generalization(5, vec![subkind.clone().into()], vec![category.clone().into()], false, false);

        assert_eq!(
            validate(vec![kind.into(), subkind.into(), category.into(), gen1.into(), gen2.into()], false, true),
            vec![],
        );
    }

    #[test]
    fn test_invalid_decint() {
        // kind, kind <- subkind
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let kind2 = new_class(2, ontouml_models::KIND, false);
        let subkind = new_class(3, ontouml_models::SUBKIND, false);
        let subkind_uuid = *subkind.read().uuid;
        let gen1 = new_generalization(4, vec![subkind.clone().into()], vec![kind1.clone().into(), kind1.clone().into()], false, false);

        assert_eq!(
            validate(vec![kind1.into(), kind2.into(), subkind.into(), gen1.into()], false, true),
            vec![
                ValidationProblem::AntiPattern { uuid: subkind_uuid, antipattern_type: AntiPatternType::DecInt }
            ],
        );
    }

    #[test]
    fn test_invalid_depphase() {
        let kind = new_class(1, ontouml_models::KIND, false);
        let phase = new_class(2, ontouml_models::PHASE, false);
        let phase_uuid = *phase.read().uuid;
        let relator = new_class(3, ontouml_models::RELATOR, false);
        let gen1 = new_generalization(4, vec![phase.clone().into()], vec![kind.clone().into()], false, false);
        let mediation = new_association(5, ontouml_models::MEDIATION, phase.clone().into(), relator.clone().into());
        mediation.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation.write().target_label_multiplicity = Arc::new("1".to_owned());

        assert!(
            validate(vec![kind.into(), phase.into(), gen1.into(), relator.into(), mediation.into()], false, true)
            .iter().find(|e| matches!(e, ValidationProblem::AntiPattern {
                uuid, antipattern_type: AntiPatternType::DepPhase,
                ..
            } if *uuid == phase_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_freerole() {
        let kind = new_class(1, ontouml_models::KIND, false);
        let role1 = new_class(2, ontouml_models::ROLE, false);
        let role2 = new_class(3, ontouml_models::ROLE, false);
        let role2_uuid = *role2.read().uuid;
        let gen1 = new_generalization(4, vec![role1.clone().into()], vec![kind.clone().into()], false, false);
        let gen2 = new_generalization(5, vec![role2.clone().into()], vec![role1.clone().into()], false, false);
        let relator = new_class(6, ontouml_models::RELATOR, false);
        let mediation = new_association(7, ontouml_models::MEDIATION, role1.clone().into(), relator.clone().into());
        mediation.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation.write().target_label_multiplicity = Arc::new("1".to_owned());

        assert!(
            validate(vec![kind.into(), role1.into(), role2.into(), gen1.into(), gen2.into(), relator.into(), mediation.into()], false, true)
            .iter().find(|e| matches!(e, ValidationProblem::AntiPattern {
                uuid, antipattern_type: AntiPatternType::FreeRole,
                ..
            } if *uuid == role2_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_gsrig() {
        let kind = new_class(1, ontouml_models::KIND, false);
        let subkind = new_class(2, ontouml_models::SUBKIND, false);
        let role = new_class(3, ontouml_models::ROLE, false);
        let gen1 = new_generalization(4, vec![subkind.clone().into(), role.clone().into()], vec![kind.clone().into()], false, false);
        let gen1_uuid = *gen1.read().uuid;

        assert!(
            validate(vec![kind.into(), subkind.into(), role.into(), gen1.into()], false, true)
            .iter().find(|e| matches!(e, ValidationProblem::AntiPattern {
                uuid, antipattern_type: AntiPatternType::GSRig,
                ..
            } if *uuid == gen1_uuid)).is_some()
        );
    }

    #[test]
    fn test_invalid_hetcoll() {
        let collective = new_class(1, ontouml_models::COLLECTIVE, true);
        let collective_uuid = *collective.read().uuid;
        let kind1 = new_class(2, ontouml_models::KIND, false);
        let kind2 = new_class(3, ontouml_models::KIND, false);
        let assoc1 = new_association(4, ontouml_models::MEMBER_OF, collective.clone().into(), kind1.clone().into());
        assoc1.write().source_label_multiplicity = Arc::new("1".to_owned());
        let assoc2 = new_association(5, ontouml_models::MEMBER_OF, collective.clone().into(), kind2.clone().into());
        assoc2.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            validate(vec![collective.into(), kind1.into(), kind2.into(), assoc1.into(), assoc2.into()], false, true),
            vec![ValidationProblem::AntiPattern {
                uuid: collective_uuid,
                antipattern_type: AntiPatternType::HetColl,
            }]
        );
    }

    #[test]
    fn test_invalid_homofunc() {
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let kind1_uuid = *kind1.read().uuid;
        let kind2 = new_class(2, ontouml_models::KIND, false);
        let assoc1 = new_association(3, ontouml_models::COMPONENT_OF, kind1.clone().into(), kind2.clone().into());
        assoc1.write().source_label_multiplicity = Arc::new("1".to_owned());

        assert_eq!(
            validate(vec![kind1.into(), kind2.into(), assoc1.into()], false, true),
            vec![ValidationProblem::AntiPattern {
                uuid: kind1_uuid,
                antipattern_type: AntiPatternType::HomoFunc,
            }]
        );
    }

    #[test]
    fn test_valid_mixrig() {
        // one rigid and one antirigid
        let mixin = new_class(1, ontouml_models::MIXIN, true);
        let kind = new_class(2, ontouml_models::KIND, false);
        let gen1 = new_generalization(3, vec![kind.clone()], vec![mixin.clone()], false, false);
        let roleMixin = new_class(4, ontouml_models::ROLE_MIXIN, true);
        let gen2 = new_generalization(5, vec![roleMixin.clone()], vec![mixin.clone()], false, false);

        assert_eq!(
            validate(vec![mixin.into(), kind.into(), gen1.into(), roleMixin.into(), gen2.into()], false, true),
            vec![],
        );
    }

    #[test]
    fn test_invalid_mixrig1() {
        // only one rigid
        let mixin = new_class(1, ontouml_models::MIXIN, true);
        let mixin_uuid = *mixin.read().uuid;
        let kind = new_class(2, ontouml_models::KIND, false);
        let gen1 = new_generalization(3, vec![kind.clone()], vec![mixin.clone()], false, false);

        assert_eq!(
            validate(vec![mixin.into(), kind.into(), gen1.into()], false, true),
            vec![ValidationProblem::AntiPattern {
                uuid: mixin_uuid,
                antipattern_type: AntiPatternType::MixRig,
            }]
        );
    }

    #[test]
    fn test_invalid_mixrig2() {
        // only one antirigid
        let mixin = new_class(1, ontouml_models::MIXIN, true);
        let mixin_uuid = *mixin.read().uuid;
        let roleMixin = new_class(2, ontouml_models::ROLE_MIXIN, true);
        let gen1 = new_generalization(3, vec![roleMixin.clone()], vec![mixin.clone()], false, false);

        assert_eq!(
            validate(vec![mixin.into(), roleMixin.into(), gen1.into()], false, true),
            vec![ValidationProblem::AntiPattern {
                uuid: mixin_uuid,
                antipattern_type: AntiPatternType::MixRig,
            }]
        );
    }

    #[test]
    fn test_invalid_multdep1() {
        // two relators
        let kind = new_class(1, ontouml_models::KIND, false);
        let role = new_class(2, ontouml_models::ROLE, false);
        let role_uuid = *role.read().uuid;
        let gen1 = new_generalization(3, vec![role.clone()], vec![kind.clone()], true, true);

        let relator1 = new_class(4, ontouml_models::RELATOR, false);
        let mediation1 = new_association(5, ontouml_models::MEDIATION, relator1.clone().into(), role.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("2".to_owned());

        let relator2 = new_class(6, ontouml_models::RELATOR, false);
        let mediation2 = new_association(7, ontouml_models::MEDIATION, relator2.clone().into(), role.clone().into());
        mediation2.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation2.write().target_label_multiplicity = Arc::new("2".to_owned());

        assert_eq!(
            validate(vec![
                kind.into(), role.into(), gen1.into(),
                relator1.into(), mediation1.into(),
                relator2.into(), mediation2.into(),
            ], false, true),
            vec![
                ValidationProblem::AntiPattern {
                    uuid: role_uuid,
                    antipattern_type: AntiPatternType::MultDep,
                }
            ],
        );
    }

    #[test]
    fn test_invalid_multdep2() {
        // one relator, one subkind
        let kind = new_class(1, ontouml_models::KIND, false);
        let role = new_class(2, ontouml_models::ROLE, false);
        let role_uuid = *role.read().uuid;
        let gen1 = new_generalization(3, vec![role.clone()], vec![kind.clone()], true, true);

        let relator1 = new_class(4, ontouml_models::RELATOR, false);
        let mediation1 = new_association(5, ontouml_models::MEDIATION, relator1.clone().into(), role.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("2".to_owned());

        let relator2 = new_class(6, ontouml_models::RELATOR, true);
        let subkind = new_class(7, ontouml_models::SUBKIND, false);
        let gen2 = new_generalization(8, vec![subkind.clone()], vec![relator2.clone()], true, true);
        let mediation2 = new_association(8, ontouml_models::MEDIATION, subkind.clone().into(), role.clone().into());
        mediation2.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation2.write().target_label_multiplicity = Arc::new("2".to_owned());

        assert_eq!(
            validate(vec![
                kind.into(), role.into(), gen1.into(),
                relator1.into(), mediation1.into(),
                relator2.into(), subkind.into(), gen2.into(), mediation2.into(),
            ], false, true),
            vec![
                ValidationProblem::AntiPattern {
                    uuid: role_uuid,
                    antipattern_type: AntiPatternType::MultDep,
                }
            ],
        );
    }

    #[test]
    fn test_valid_relrig1() {
        // relator to role
        let kind = new_class(1, ontouml_models::KIND, false);
        let role = new_class(2, ontouml_models::ROLE, false);
        let gen1 = new_generalization(3, vec![role.clone()], vec![kind.clone()], true, true);
        let relator = new_class(4, ontouml_models::RELATOR, false);
        let mediation1 = new_association(5, ontouml_models::MEDIATION, relator.clone().into(), role.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("2".to_owned());

        assert_eq!(
            validate(vec![kind.into(), role.into(), gen1.into(), relator.into(), mediation1.into()], false, true),
            vec![],
        );
    }

    #[test]
    fn test_valid_relrig2() {
        // relator subkind to role
        let kind = new_class(1, ontouml_models::KIND, false);
        let role = new_class(2, ontouml_models::ROLE, false);
        let gen1 = new_generalization(3, vec![role.clone()], vec![kind.clone()], true, true);
        let relator = new_class(4, ontouml_models::RELATOR, true);
        let subkind = new_class(5, ontouml_models::SUBKIND, false);
        let gen2 = new_generalization(6, vec![subkind.clone()], vec![relator.clone()], true, true);
        let mediation1 = new_association(7, ontouml_models::MEDIATION, subkind.clone().into(), role.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("2".to_owned());

        assert_eq!(
            validate(vec![
                kind.into(), role.into(), gen1.into(),
                relator.into(), subkind.into(), gen2.into(),
                mediation1.into(),
            ], false, true),
            vec![],
        );
    }

    #[test]
    fn test_invalid_relrig1() {
        // relator to kind
        let kind = new_class(1, ontouml_models::KIND, false);
        let relator = new_class(2, ontouml_models::RELATOR, false);
        let relator_uuid = *relator.read().uuid;
        let mediation1 = new_association(3, ontouml_models::MEDIATION, relator.clone().into(), kind.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("2".to_owned());

        assert_eq!(
            validate(vec![kind.into(), relator.into(), mediation1.into()], false, true),
            vec![
                ValidationProblem::AntiPattern {
                    uuid: relator_uuid,
                    antipattern_type: AntiPatternType::RelRig,
                }
            ],
        );
    }

    #[test]
    fn test_invalid_relrig2() {
        // relator subkind to kind
        let kind = new_class(1, ontouml_models::KIND, false);
        let relator = new_class(2, ontouml_models::RELATOR, true);
        let subkind = new_class(3, ontouml_models::SUBKIND, false);
        let subkind_uuid = *subkind.read().uuid;
        let gen2 = new_generalization(4, vec![subkind.clone()], vec![relator.clone()], true, true);
        let mediation1 = new_association(5, ontouml_models::MEDIATION, subkind.clone().into(), kind.clone().into());
        mediation1.write().source_label_multiplicity = Arc::new("1".to_owned());
        mediation1.write().target_label_multiplicity = Arc::new("2".to_owned());

        assert_eq!(
            validate(vec![
                kind.into(),
                relator.into(), subkind.into(), gen2.into(),
                mediation1.into(),
            ], false, true),
            vec![
                ValidationProblem::AntiPattern {
                    uuid: subkind_uuid,
                    antipattern_type: AntiPatternType::RelRig,
                }
            ],
        );
    }

    #[test]
    fn test_valid_undefformal1() {
        // self
        let kind = new_class(1, ontouml_models::KIND, false);
        let mode = new_class(2, ontouml_models::MODE, false);
        let assoc1 = new_association(3, ontouml_models::CHARACTERIZATION, kind.clone().into(), mode.clone().into());
        let assoc2 = new_association(4, ontouml_models::FORMAL, kind.clone().into(), kind.clone().into());

        assert_eq!(
            call_validate_undef(vec![kind.into(), mode.into(), assoc1.into(), assoc2.into()]),
            vec![],
        );
    }

    #[test]
    fn test_valid_undefformal2() {
        // two kinds with both having qualities
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let kind2 = new_class(2, ontouml_models::KIND, false);
        let mode = new_class(3, ontouml_models::MODE, false);
        let assoc1 = new_association(4, ontouml_models::CHARACTERIZATION, kind1.clone().into(), mode.clone().into());
        let assoc2 = new_association(5, ontouml_models::CHARACTERIZATION, kind2.clone().into(), mode.clone().into());
        let assoc3 = new_association(6, ontouml_models::FORMAL, kind1.clone().into(), kind2.clone().into());

        assert_eq!(
            call_validate_undef(vec![
                kind1.into(), kind2.into(), mode.into(),
                assoc1.into(), assoc2.into(), assoc3.into(),
            ]),
            vec![],
        );
    }

    #[test]
    fn test_invalid_undefformal1() {
        // self with no qualities
        let kind = new_class(1, ontouml_models::KIND, false);
        let assoc = new_association(2, ontouml_models::FORMAL, kind.clone().into(), kind.clone().into());
        let assoc_uuid = *assoc.read().uuid;

        assert_eq!(
            call_validate_undef(vec![kind.into(), assoc.into()]),
            vec![
                ValidationProblem::AntiPattern {
                    uuid: assoc_uuid,
                    antipattern_type: AntiPatternType::UndefFormal,
                }
            ],
        );
    }

    #[test]
    fn test_invalid_undefformal2() {
        // two kinds with one missing any qualities
        let kind1 = new_class(1, ontouml_models::KIND, false);
        let kind2 = new_class(2, ontouml_models::KIND, false);
        let mode = new_class(3, ontouml_models::MODE, false);
        let assoc1 = new_association(4, ontouml_models::CHARACTERIZATION, kind1.clone().into(), mode.clone().into());
        let assoc2 = new_association(5, ontouml_models::FORMAL, kind1.clone().into(), kind2.clone().into());
        let assoc2_uuid = *assoc2.read().uuid;

        assert_eq!(
            call_validate_undef(vec![
                kind1.into(), kind2.into(), mode.into(),
                assoc1.into(), assoc2.into(),
            ]),
            vec![ValidationProblem::AntiPattern {
                uuid: assoc2_uuid,
                antipattern_type: AntiPatternType::UndefFormal,
            }],
        );
    }

    #[test]
    fn test_valid_undefphase1() {
        // simple
        let kind = new_class(1, ontouml_models::KIND, false);
        let mode = new_class(2, ontouml_models::MODE, false);
        let assoc = new_association(3, ontouml_models::CHARACTERIZATION, kind.clone().into(), mode.clone().into());
        let phase = new_class(4, ontouml_models::PHASE, false);
        let gen1 = new_generalization(5, vec![phase.clone()], vec![kind.clone()], true, true);

        assert_eq!(
            call_validate_undef(vec![kind.into(), mode.into(), assoc.into(), phase.into(), gen1.into()]),
            vec![],
        );
    }

    #[test]
    fn test_valid_undefphase2() {
        // ancestor
        let kind = new_class(1, ontouml_models::KIND, false);
        let mode = new_class(2, ontouml_models::MODE, false);
        let assoc = new_association(3, ontouml_models::CHARACTERIZATION, kind.clone().into(), mode.clone().into());
        let subkind = new_class(4, ontouml_models::KIND, false);
        let gen1 = new_generalization(6, vec![subkind.clone()], vec![kind.clone()], true, true);
        let phase = new_class(7, ontouml_models::PHASE, false);
        let gen2 = new_generalization(8, vec![phase.clone()], vec![subkind.clone()], true, true);

        assert_eq!(
            call_validate_undef(vec![
                kind.into(), mode.into(), assoc.into(),
                subkind.into(), gen1.into(),
                phase.into(), gen2.into(),
            ]),
            vec![],
        );
    }

    #[test]
    fn test_invalid_undefphase() {
        let kind = new_class(1, ontouml_models::KIND, false);
        let phase = new_class(2, ontouml_models::PHASE, false);
        let phase_uuid = *phase.read().uuid;
        let gen1 = new_generalization(3, vec![phase.clone()], vec![kind.clone()], true, true);

        assert_eq!(
            call_validate_undef(vec![kind.into(), phase.into(), gen1.into()]),
            vec![
                ValidationProblem::AntiPattern {
                    uuid: phase_uuid,
                    antipattern_type: AntiPatternType::UndefPhase,
                }
            ],
        );
    }
}
