use super::super::umlclass::{
        umlclass_models::UmlClassDiagram,
        umlclass_controllers::{LinkType, PlantUmlTab, UmlClassDiagramAdapter, UmlClassDomain, UmlClassElementView, UmlClassPalette, UmlClassProfile, UmlClassToolStage, StereotypeController, UmlClassElementOrVertex, new_umlclass_association, new_umlclass_class, new_umlclass_comment, new_umlclass_commentlink, new_umlclass_generalization, new_umlclass_package},
};
use crate::common::{
    controller::{
        DiagramController, DiagramControllerGen2, ElementControllerGen2, InsensitiveCommand, LabelProvider, ProjectCommand, View
    },
    eref::ERef,
    project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer},
    uuid::ViewUuid,
};
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
    views: [(UmlClassToolStage, &'static str, UmlClassElementView<OntoUmlProfile>); 12],
}

impl Default for UmlClassPlaceholderViews {
    fn default() -> Self {
        let (kind, kind_view) = new_umlclass_class("Animal", "kind", false, Vec::new(), Vec::new(), egui::Pos2::ZERO);
        let kind1 = (kind.clone(), kind_view.clone().into());
        let kind2 = (kind.clone().into(), kind_view.clone().into());
        let kind3 = (kind.clone().into(), kind_view.clone().into());
        let (subkind, subkind_view) = new_umlclass_class("Human", "subkind", false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 75.0));
        let subkind = (subkind, subkind_view.into());
        let (phase, phase_view) = new_umlclass_class("Adult", "phase", false, Vec::new(), Vec::new(), egui::Pos2::new(200.0, 150.0));
        let phase2 = (phase.clone().into(), phase_view.clone().into());
        let phase3 = ((), phase_view.clone().into());
        let (role, role_view) = new_umlclass_class("Customer", "role", false, Vec::new(), Vec::new(), egui::Pos2::ZERO);
        let role = (role, role_view.into());

        let (_gen, gen_view) = new_umlclass_generalization(None, kind1.clone(), subkind.clone());
        let (assoc, assoc_view) = new_umlclass_association("", None, kind2.clone(), phase2.clone());
        assoc.write().source_label_multiplicity = Arc::new("".to_owned());
        assoc.write().target_label_multiplicity = Arc::new("".to_owned());
        assoc_view.write().refresh_buffers();
        let (mediation, mediation_view) = new_umlclass_association("mediation", None, kind2.clone(), phase2.clone());
        mediation.write().source_label_multiplicity = Arc::new("".to_owned());
        mediation.write().target_label_multiplicity = Arc::new("".to_owned());
        mediation_view.write().refresh_buffers();
        let (chara, char_view) = new_umlclass_association("characterization", None, kind2.clone(), phase2.clone());
        chara.write().source_label_multiplicity = Arc::new("".to_owned());
        chara.write().target_label_multiplicity = Arc::new("".to_owned());
        char_view.write().refresh_buffers();
        let (comp, comp_view) = new_umlclass_association("componentOf", None, kind2.clone(), phase2.clone());
        comp.write().source_label_multiplicity = Arc::new("".to_owned());
        comp.write().target_label_multiplicity = Arc::new("".to_owned());
        comp_view.write().refresh_buffers();

        let (_package, package_view) = new_umlclass_package("a package", egui::Rect { min: egui::Pos2::ZERO, max: egui::Pos2::new(100.0, 50.0) });
        let (comment, comment_view) = new_umlclass_comment("a comment", egui::Pos2::new(-100.0, -75.0));
        let comment = (comment, comment_view.into());
        let commentlink = new_umlclass_commentlink(None, comment.clone(), kind3.clone());

        Self {
            views: [
                (UmlClassToolStage::Class { name: "Animal", stereotype: "kind" }, "Kind", kind3.1),
                (UmlClassToolStage::Class { name: "Human", stereotype: "subkind" }, "Subkind", subkind.1),
                (UmlClassToolStage::Class { name: "Adult", stereotype: "phase" }, "Phase", phase3.1),
                (UmlClassToolStage::Class { name: "Customer", stereotype: "role" }, "Role", role.1),

                (UmlClassToolStage::LinkStart { link_type: LinkType::Generalization }, "Generalization (Set)", gen_view.into()),
                (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "" } }, "Association", assoc_view.into()),
                (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "mediation" } }, "Mediation", mediation_view.into()),
                (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "characterization" } }, "Characterization",char_view.into()),
                (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "componentOf" } }, "ComponentOf",comp_view.into()),

                (UmlClassToolStage::PackageStart, "Package", package_view.into()),
                (UmlClassToolStage::Comment, "Comment", comment.1),
                (UmlClassToolStage::CommentLinkStart, "Comment Link", commentlink.1.into()),
            ]
        }
    }
}

impl UmlClassPalette<OntoUmlProfile> for UmlClassPlaceholderViews {
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (UmlClassToolStage, &'static str, UmlClassElementView<OntoUmlProfile>)> {
        self.views.iter_mut()
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
    let (animal_model, animal_view) = new_umlclass_class("Animal", "kind", false, Vec::new(), Vec::new(), egui::Pos2::new(150.0, 200.0));
    let (human_model, human_view) = new_umlclass_class("Human", "subkind", false, Vec::new(), Vec::new(), egui::Pos2::new(150.0, 350.0));
    let (alive_model, alive_view) = new_umlclass_class("Alive", "phase", false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 160.0));
    let (dead_model, dead_view) = new_umlclass_class("Dead", "phase", false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 250.0));
    let (marriage_model, marriage_view) = new_umlclass_class("Marriage", "relator", false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 350.0));

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
        "mediation", None,
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

fn ontouml_class_stereotype_literal(e: &str) -> &'static str {
    match e {
        // Sortals
        "kind" => "kind",
        "subkind" => "subkind",
        "phase" => "phase",
        "role" => "role",
        "collective" => "collective",
        "quantity" => "quantity",
        "relator" => "relator",
        // Nonsortals
        "category" => "category",
        "phaseMixin" => "phaseMixin",
        "roleMixin" => "roleMixin",
        "mixin" => "mixin",
        // Aspects
        "mode" => "mode",
        "quality" => "quality",
        _ => unreachable!(),
    }
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
                    // Sortals
                    ("kind", "Kind"),
                    ("subkind", "Subkind"),
                    ("phase", "Phase"),
                    ("role", "Role"),
                    ("collective", "Collective"),
                    ("quantity", "Quantity"),
                    ("relator", "Relator"),
                    // Nonsortals
                    ("category", "Category"),
                    ("phaseMixin", "PhaseMixin"),
                    ("roleMixin", "RoleMixin"),
                    ("mixin", "Mixin"),
                    // Aspects
                    ("mode", "Mode"),
                    ("quality", "Quality"),
                ] {
                    if ui.selectable_value(&mut self.buffer, e.0, e.1).changed() {
                        ret = true;
                    }
                }
            });
        ret
    }
    fn get(&mut self) -> Arc<String> {
        self.buffer.to_owned().into()
    }
    fn refresh(&mut self, new_value: &str) {
        self.buffer = ontouml_class_stereotype_literal(new_value);
        self.display_string = if self.buffer.is_empty() {
            "None".to_owned()
        } else {
            format!("«{}»", self.buffer)
        };
    }
}

fn ontouml_association_stereotype_literal(e: &str) -> &'static str {
    match e {
        "" => "",
        "formal" => "formal",
        "mediation" => "mediation",
        "characterization" => "characterization",
        "structuration" => "structuration",

        "componentOf" => "componentOf",
        "containment" => "containment",
        "memberOf" => "memberOf",
        "subcollectionOf" => "subcollectionOf",
        "subquantityOf" => "subquantityOf",
        _ => unreachable!(),
    }
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
                    ("", "None"),
                    ("formal", "Formal"),
                    ("mediation", "Mediation"),
                    ("characterization", "Characterization"),
                    ("structuration", "Structuration"),

                    ("componentOf", "ComponentOf"),
                    ("containment", "Containment"),
                    ("memberOf", "MemberOf"),
                    ("subcollectionOf", "SubcollectionOf"),
                    ("subquantityOf", "SubquantityOf"),
                ] {
                    if ui
                        .selectable_value(&mut self.buffer, sv.0, sv.1)
                        .changed()
                    {
                        ret = true;
                    }
                }
            });
        ret
    }
    fn get(&mut self) -> Arc<String> {
        self.buffer.to_owned().into()
    }
    fn refresh(&mut self, new_value: &str) {
        self.buffer = ontouml_association_stereotype_literal(new_value);
        self.display_string = if self.buffer.is_empty() {
            "None".to_owned()
        } else {
            format!("«{}»", self.buffer)
        };
    }
}
