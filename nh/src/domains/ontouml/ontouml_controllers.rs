use super::super::umlclass::{
        umlclass_models::UmlClassDiagram,
        umlclass_controllers::{LinkType, PlantUmlTab, UmlClassDiagramAdapter, UmlClassDomain, UmlClassElementView, UmlClassPalette, UmlClassProfile, UmlClassToolStage, StereotypeController, UmlClassElementOrVertex, new_umlclass_association, new_umlclass_class, new_umlclass_comment, new_umlclass_commentlink, new_umlclass_generalization, new_umlclass_package},
};
use crate::{common::{
    controller::{
        DiagramController, DiagramControllerGen2, ElementControllerGen2, InsensitiveCommand, LabelProvider, ProjectCommand, View
    },
    eref::ERef,
    project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer},
    uuid::ViewUuid,
}, domains::ontouml::ontouml_models};
use eframe::egui;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

#[derive(Clone, Default)]
pub struct OntoUmlProfile;
impl UmlClassProfile for OntoUmlProfile {
    type Palette = UmlClassPlaceholderViews;
    type ClassStereotypeController = OntoUmlClassStereotypeController;
    type AssociationStereotypeController = OntoUmlAssociationStereotypeController;

    fn view_type() -> &'static str {
        "umlclass-diagram-view-ontouml"
    }

    fn menubar_options_fun(
        model: &ERef<UmlClassDiagram>,
        view_uuid: &ViewUuid,
        label_provider: &ERef<dyn LabelProvider>,
        ui: &mut egui::Ui,
        commands: &mut Vec<ProjectCommand>,
    ) {
        if ui.button("PlantUML description").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(PlantUmlTab::new(model.clone()))),
            ));
        }
        if ui.button("OntoUML Validations").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(super::ontouml_validations::OntoUMLValidationTab::new(model.clone(), label_provider.clone(), *view_uuid))),
            ));
        }
        ui.separator();
    }
}

#[derive(Clone)]
pub struct UmlClassPlaceholderViews {
    views: [(&'static str, Vec<(UmlClassToolStage, &'static str, UmlClassElementView<OntoUmlProfile>)>); 3],
}

impl Default for UmlClassPlaceholderViews {
    fn default() -> Self {
        let mut classes = Vec::new();
        for (stereotype, label, name, is_abstract) in [
            // Sortals
            (ontouml_models::KIND, "Kind", "Animal", false),
            (ontouml_models::SUBKIND, "Subkind", "Human", false),
            (ontouml_models::PHASE, "Phase", "Adult", false),
            (ontouml_models::ROLE, "Role", "Married", false),
            (ontouml_models::COLLECTIVE, "Collective", "Forest", false),
            (ontouml_models::QUANTITY, "Quantity", "Petroleum", false),
            (ontouml_models::RELATOR, "Relator", "Subscription", false),
            // Nonsortals
            (ontouml_models::CATEGORY, "Category", "Living thing", true),
            (ontouml_models::PHASE_MIXIN, "Phase Mixin", "Broken", true),
            (ontouml_models::ROLE_MIXIN, "Role Mixin", "Customer", true),
            (ontouml_models::MIXIN, "Mixin", "Luxury good", true),
            // Aspects
            (ontouml_models::MODE, "Mode", "Intention", false),
            (ontouml_models::QUALITY, "Quality", "Height", false),
        ] {
            let (_c, c_view) = new_umlclass_class(name, stereotype, is_abstract, Vec::new(), Vec::new(), egui::Pos2::ZERO);
            c_view.write().refresh_buffers();
            classes.push(
                (UmlClassToolStage::Class { name, stereotype }, label, c_view.into()),
            );
        }

        let mut relationships = Vec::new();
        let dummy1 = new_umlclass_class("dummy1", ontouml_models::NONE, false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 75.0));
        let dummy2 = new_umlclass_class("dummy1", ontouml_models::NONE, false, Vec::new(), Vec::new(), egui::Pos2::new(200.0, 150.0));
        let (_gen, gen_view) = new_umlclass_generalization(None, (dummy1.0.clone(), dummy1.1.clone().into()), (dummy2.0.clone(), dummy2.1.clone().into()));
        relationships.push(
            (UmlClassToolStage::LinkStart { link_type: LinkType::Generalization }, "Generalization (Set)", gen_view.into()),
        );
        for (stereotype, label) in [
            (ontouml_models::FORMAL, "Formal"),
            (ontouml_models::MEDIATION, "Mediation"),
            (ontouml_models::CHARACTERIZATION, "Characterization"),
            (ontouml_models::STRUCTURATION, "Structuration"),
            (ontouml_models::COMPONENT_OF, "ComponentOf"),
            (ontouml_models::CONTAINMENT, "Containment"),
            (ontouml_models::MEMBER_OF, "MemberOf"),
            (ontouml_models::SUBCOLLECTION_OF, "SubcollectionOf"),
            (ontouml_models::SUBQUANTITY_OF, "SubquantityOf"),
        ] {
            let (m, m_view) = new_umlclass_association(stereotype, "", None, (dummy1.0.clone().into(), dummy1.1.clone().into()), (dummy2.0.clone().into(), dummy2.1.clone().into()));
            m.write().source_label_multiplicity = Arc::new("".to_owned());
            m.write().target_label_multiplicity = Arc::new("".to_owned());
            m_view.write().refresh_buffers();
            relationships.push(
                (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype } }, label, m_view.into()),
            );
        }

        let (_package, package_view) = new_umlclass_package("a package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });
        let (comment, comment_view) = new_umlclass_comment("a comment", egui::Pos2::new(-100.0, -75.0));
        let comment = (comment, comment_view.into());
        let commentlink = new_umlclass_commentlink(None, comment.clone(), (dummy2.0.clone().into(), dummy2.1.clone().into()));

        Self {
            views: [
                ("Classes", classes),
                ("Relationships", relationships),
                ("Other", vec![
                    (UmlClassToolStage::PackageStart, "Package", package_view.into()),
                    (UmlClassToolStage::Comment, "Comment", comment.1),
                    (UmlClassToolStage::CommentLinkStart, "Comment Link", commentlink.1.into()),
                ]),
            ]
        }
    }
}

impl UmlClassPalette<OntoUmlProfile> for UmlClassPlaceholderViews {
    fn iter_mut(&mut self) -> impl Iterator<Item = (&str, &mut Vec<(UmlClassToolStage, &'static str, UmlClassElementView<OntoUmlProfile>)>)> {
        self.views.iter_mut().map(|e| (e.0, &mut e.1))
    }
}

pub fn new(no: u32) -> ERef<dyn DiagramController> {
    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("New OntoUML diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        model_uuid,
        name.clone(),
        vec![],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        UmlClassDiagramAdapter::<OntoUmlProfile>::new(diagram.clone()),
        Vec::new(),
    )
}

pub fn demo(no: u32) -> ERef<dyn DiagramController> {
    let (animal_model, animal_view) = new_umlclass_class("Animal", ontouml_models::KIND, false, Vec::new(), Vec::new(), egui::Pos2::new(150.0, 200.0));
    let (human_model, human_view) = new_umlclass_class("Human", ontouml_models::SUBKIND, false, Vec::new(), Vec::new(), egui::Pos2::new(150.0, 350.0));
    let (alive_model, alive_view) = new_umlclass_class("Alive", ontouml_models::PHASE, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 160.0));
    let (dead_model, dead_view) = new_umlclass_class("Dead", ontouml_models::PHASE, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 250.0));
    let (marriage_model, marriage_view) = new_umlclass_class("Marriage", ontouml_models::RELATOR, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 350.0));

    let (gen_phase_model, gen_phase_view) = new_umlclass_generalization(
        Some((uuid::Uuid::now_v7().into(), egui::Pos2::new(280.0, 200.0))),
        (alive_model.clone(), alive_view.clone().into()),
        (animal_model.clone(), animal_view.clone().into()),
    );
    gen_phase_model.write().set_is_covering = true;
    gen_phase_model.write().set_is_disjoint = true;
    let gen_uuid = *gen_phase_view.read().uuid();
    gen_phase_view.write().apply_command(
        &InsensitiveCommand::AddDependency(gen_uuid, 0, UmlClassElementOrVertex::Element(dead_view.clone().into()), true),
        &mut Vec::new(),
        &mut HashSet::new(),
    );

    let (gen_human_model, gen_human_view) = new_umlclass_generalization(
        None,
        (human_model.clone(), human_view.clone().into()),
        (animal_model.clone(), animal_view.clone().into()),
    );

    let (mediation_model, mediation_view) = new_umlclass_association(
        ontouml_models::MEDIATION, "", None,
        (human_model.clone().into(), human_view.clone().into()),
        (marriage_model.clone().into(), marriage_view.clone().into()),
    );
    mediation_model.write().source_label_multiplicity = Arc::new("2..*".to_owned());
    mediation_model.write().target_label_multiplicity = Arc::new("1..1".to_owned());

    let view_uuid = uuid::Uuid::now_v7().into();
    let model_uuid = uuid::Uuid::now_v7().into();
    let name = format!("Demo OntoUML diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        model_uuid,
        name.clone(),
        vec![
            animal_model.into(),
            human_model.into(),
            alive_model.into(),
            dead_model.into(),
            marriage_model.into(),
            gen_phase_model.into(),
            gen_human_model.into(),
            mediation_model.into(),
        ],
    ));
    DiagramControllerGen2::new(
        Arc::new(view_uuid),
        name.clone().into(),
        UmlClassDiagramAdapter::<OntoUmlProfile>::new(diagram.clone()),
        vec![
            animal_view.into(),
            human_view.into(),
            alive_view.into(),
            dead_view.into(),
            marriage_view.into(),
            gen_phase_view.into(),
            gen_human_view.into(),
            mediation_view.into(),
        ],
    )
}

pub fn deserializer(uuid: ViewUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<DiagramControllerGen2<UmlClassDomain<OntoUmlProfile>, UmlClassDiagramAdapter<OntoUmlProfile>>>(&uuid)?)
}

pub fn ontouml_class_stereotype_literal(e: &str) -> Option<&'static str> {
    let e = match e {
        ontouml_models::NONE => ontouml_models::NONE,
        // Sortals
        ontouml_models::KIND => ontouml_models::KIND,
        ontouml_models::SUBKIND => ontouml_models::SUBKIND,
        ontouml_models::PHASE => ontouml_models::PHASE,
        ontouml_models::ROLE => ontouml_models::ROLE,
        ontouml_models::COLLECTIVE => ontouml_models::COLLECTIVE,
        ontouml_models::QUANTITY => ontouml_models::QUANTITY,
        ontouml_models::RELATOR => ontouml_models::RELATOR,
        // Nonsortals
        ontouml_models::CATEGORY => ontouml_models::CATEGORY,
        ontouml_models::PHASE_MIXIN => ontouml_models::PHASE_MIXIN,
        ontouml_models::ROLE_MIXIN => ontouml_models::ROLE_MIXIN,
        ontouml_models::MIXIN => ontouml_models::MIXIN,
        // Aspects
        ontouml_models::MODE => ontouml_models::MODE,
        ontouml_models::QUALITY => ontouml_models::QUALITY,
        _ => return None,
    };
    Some(e)
}

#[derive(Clone, Default)]
pub struct OntoUmlClassStereotypeController {
    display_string: String,
    buffer: &'static str,
}

impl StereotypeController for OntoUmlClassStereotypeController {
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut ret = false;
        egui::ComboBox::from_id_salt("Class stereotype:")
            .selected_text(&self.display_string)
            .show_ui(ui, |ui| {
                for e in [
                    (ontouml_models::NONE, "None"),
                    // Sortals
                    (ontouml_models::KIND, "Kind"),
                    (ontouml_models::SUBKIND, "Subkind"),
                    (ontouml_models::PHASE, "Phase"),
                    (ontouml_models::ROLE, "Role"),
                    (ontouml_models::COLLECTIVE, "Collective"),
                    (ontouml_models::QUANTITY, "Quantity"),
                    (ontouml_models::RELATOR, "Relator"),
                    // Nonsortals
                    (ontouml_models::CATEGORY, "Category"),
                    (ontouml_models::PHASE_MIXIN, "PhaseMixin"),
                    (ontouml_models::ROLE_MIXIN, "RoleMixin"),
                    (ontouml_models::MIXIN, "Mixin"),
                    // Aspects
                    (ontouml_models::MODE, "Mode"),
                    (ontouml_models::QUALITY, "Quality"),
                ] {
                    if ui.selectable_value(&mut self.buffer, e.0, e.1).changed() {
                        ret = true;
                        self.refresh(e.0);
                    }
                }
            });
        ret
    }
    fn get(&mut self) -> Arc<String> {
        self.buffer.to_owned().into()
    }
    fn is_valid(&self, value: &str) -> bool {
        ontouml_class_stereotype_literal(value).is_some()
    }
    fn refresh(&mut self, new_value: &str) {
        if let Some(new_value) = ontouml_class_stereotype_literal(new_value) {
            self.buffer = new_value;
        }
        self.display_string = if self.buffer.is_empty() {
            "None".to_owned()
        } else {
            format!("«{}»", self.buffer)
        };
    }
}

pub fn ontouml_association_stereotype_literal(e: &str) -> Option<&'static str> {
    let e = match e {
        ontouml_models::NONE => ontouml_models::NONE,
        ontouml_models::FORMAL => ontouml_models::FORMAL,
        ontouml_models::MEDIATION => ontouml_models::MEDIATION,
        ontouml_models::CHARACTERIZATION => ontouml_models::CHARACTERIZATION,
        ontouml_models::STRUCTURATION => ontouml_models::STRUCTURATION,
        ontouml_models::COMPONENT_OF => ontouml_models::COMPONENT_OF,
        ontouml_models::CONTAINMENT => ontouml_models::CONTAINMENT,
        ontouml_models::MEMBER_OF => ontouml_models::MEMBER_OF,
        ontouml_models::SUBCOLLECTION_OF => ontouml_models::SUBCOLLECTION_OF,
        ontouml_models::SUBQUANTITY_OF => ontouml_models::SUBQUANTITY_OF,
        _ => return None,
    };
    Some(e)
}

#[derive(Clone, Default)]
pub struct OntoUmlAssociationStereotypeController {
    display_string: String,
    buffer: &'static str,
}

impl StereotypeController for OntoUmlAssociationStereotypeController {
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut ret = false;
        egui::ComboBox::from_id_salt("Association stereotype:")
            .selected_text(&self.display_string)
            .show_ui(ui, |ui| {
                for sv in [
                    (ontouml_models::NONE, "None"),
                    (ontouml_models::FORMAL, "Formal"),
                    (ontouml_models::MEDIATION, "Mediation"),
                    (ontouml_models::CHARACTERIZATION, "Characterization"),
                    (ontouml_models::STRUCTURATION, "Structuration"),
                    (ontouml_models::COMPONENT_OF, "ComponentOf"),
                    (ontouml_models::CONTAINMENT, "Containment"),
                    (ontouml_models::MEMBER_OF, "MemberOf"),
                    (ontouml_models::SUBCOLLECTION_OF, "SubcollectionOf"),
                    (ontouml_models::SUBQUANTITY_OF, "SubquantityOf"),
                ] {
                    if ui
                        .selectable_value(&mut self.buffer, sv.0, sv.1)
                        .changed()
                    {
                        ret = true;
                        self.refresh(sv.0);
                    }
                }
            });
        ret
    }
    fn get(&mut self) -> Arc<String> {
        self.buffer.to_owned().into()
    }
    fn is_valid(&self, value: &str) -> bool {
        ontouml_association_stereotype_literal(value).is_some()
    }
    fn refresh(&mut self, new_value: &str) {
        if let Some(new_value) = ontouml_association_stereotype_literal(new_value) {
            self.buffer = new_value;
        }
        self.display_string = if self.buffer.is_empty() {
            "None".to_owned()
        } else {
            format!("«{}»", self.buffer)
        };
    }
}
