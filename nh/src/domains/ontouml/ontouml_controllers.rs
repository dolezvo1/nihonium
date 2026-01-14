use super::super::umlclass::{
        umlclass_models::UmlClassDiagram,
        umlclass_controllers::{LinkType, PlantUmlTab, UmlClassDiagramAdapter, UmlClassDomain, UmlClassElementView, UmlClassPalette, UmlClassProfile, UmlClassToolStage, StereotypeController, UmlClassElementOrVertex, new_umlclass_association, new_umlclass_class, new_umlclass_comment, new_umlclass_commentlink, new_umlclass_generalization, new_umlclass_package},
};
use crate::{common::{
    controller::{
        BucketNoT, ControllerAdapter, DiagramController, DiagramControllerGen2, ElementControllerGen2, GlobalDrawingContext, InsensitiveCommand, MultiDiagramController, PositionNoT, ProjectCommand, View
    },
    eref::ERef,
    project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer},
    uuid::{ControllerUuid, ModelUuid, ViewUuid},
}, domains::{ontouml::ontouml_models, umlclass::umlclass_models::UmlClassElement}};
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

    fn menubar_options_fun(
        model: &ERef<UmlClassDiagram>,
        view_uuid: &ViewUuid,
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
                Arc::new(RwLock::new(super::ontouml_validations::OntoUMLValidationTab::new(model.clone(), *view_uuid))),
            ));
        }
        ui.separator();
    }

    fn allows_class_properties() -> bool {
        false
    }
    fn allows_class_operations() -> bool {
        false
    }
}


#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct OntoUmlControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
}

impl ControllerAdapter<UmlClassDomain<OntoUmlProfile>> for OntoUmlControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<UmlClassDomain<OntoUmlProfile>, UmlClassDiagramAdapter<OntoUmlProfile>>;

    fn model(&self) -> ERef<UmlClassDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<UmlClassDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "umlclass-ontouml"
    }

    fn model_transitive_closure(&self, when_deleting: HashSet<ModelUuid>) -> HashSet<ModelUuid> {
        super::super::umlclass::umlclass_models::transitive_closure(&self.model.read(), when_deleting)
    }

    fn insert_element(&mut self, parent: ModelUuid, element: UmlClassElement, b: BucketNoT, p: Option<PositionNoT>) -> Result<(), ()> {
        self.model.write().insert_element_into(parent, element, b, p)
    }

    fn delete_elements(&mut self, uuids: &HashSet<ModelUuid>, undo: &mut Vec<(ModelUuid, UmlClassElement, BucketNoT, PositionNoT)>) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(&self, _gdc: &GlobalDrawingContext, ui: &mut egui::Ui) -> Option<ERef<Self::DiagramViewT>> {
        if ui.button("OntoUML Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared OntoUML Diagram".to_owned().into(),
                UmlClassDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
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
            let (_c, c_view) = new_umlclass_class(name, stereotype, is_abstract, Vec::new(), Vec::new(), egui::Pos2::ZERO, false);
            c_view.write().refresh_buffers();
            classes.push(
                (UmlClassToolStage::Class { name, stereotype, render_as_stick_figure: false }, label, c_view.into()),
            );
        }

        let mut relationships = Vec::new();
        let dummy1 = new_umlclass_class("dummy1", ontouml_models::NONE, false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 75.0), false);
        let dummy2 = new_umlclass_class("dummy2", ontouml_models::NONE, false, Vec::new(), Vec::new(), egui::Pos2::new(200.0, 150.0), false);
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

fn new_controlller(
    model: ERef<UmlClassDiagram>,
    name: String,
    elements: Vec<UmlClassElementView<OntoUmlProfile>>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(
            MultiDiagramController::new(
                ControllerUuid::now_v7(),
                OntoUmlControllerAdapter { model: model.clone() },
                vec![
                    DiagramControllerGen2::new(
                        uuid.into(),
                        name.into(),
                        UmlClassDiagramAdapter::<OntoUmlProfile>::new(model),
                        elements,
                    )
                ]
            )
        )
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New OntoUML diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (animal_model, animal_view) = new_umlclass_class("Animal", ontouml_models::KIND, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 200.0), false);
    let (temp_model, temp_view) = new_umlclass_class("Body Temperature", ontouml_models::QUALITY, false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 200.0), false);
    let (human_model, human_view) = new_umlclass_class("Human", ontouml_models::SUBKIND, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 350.0), false);
    let (alive_model, alive_view) = new_umlclass_class("Alive", ontouml_models::PHASE, false, Vec::new(), Vec::new(), egui::Pos2::new(550.0, 160.0), false);
    let (dead_model, dead_view) = new_umlclass_class("Dead", ontouml_models::PHASE, false, Vec::new(), Vec::new(), egui::Pos2::new(550.0, 250.0), false);
    let (spouse_model, spouse_view) = new_umlclass_class("Spouse", ontouml_models::ROLE, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 500.0), false);
    let (marriage_model, marriage_view) = new_umlclass_class("Marriage", ontouml_models::RELATOR, false, Vec::new(), Vec::new(), egui::Pos2::new(550.0, 500.0), false);

    let (gen_phase_model, gen_phase_view) = new_umlclass_generalization(
        Some((ViewUuid::now_v7(), egui::Pos2::new(480.0, 200.0))),
        (alive_model.clone(), alive_view.clone().into()),
        (animal_model.clone(), animal_view.clone().into()),
    );
    gen_phase_model.write().set_is_covering = true;
    gen_phase_model.write().set_is_disjoint = true;
    let gen_uuid = *gen_phase_view.read().uuid();
    gen_phase_view.write().apply_command(
        &InsensitiveCommand::AddDependency(gen_uuid, 0, None, UmlClassElementOrVertex::Element(dead_view.clone().into()), true),
        &mut Vec::new(),
        &mut HashSet::new(),
    );

    let (gen_human_model, gen_human_view) = new_umlclass_generalization(
        None,
        (human_model.clone(), human_view.clone().into()),
        (animal_model.clone(), animal_view.clone().into()),
    );

    let (gen_spouse_model, gen_spouse_view) = new_umlclass_generalization(
        None,
        (spouse_model.clone(), spouse_view.clone().into()),
        (human_model.clone(), human_view.clone().into()),
    );

    let (char_model, char_view) = new_umlclass_association(
        ontouml_models::CHARACTERIZATION, "", None,
        (animal_model.clone().into(), animal_view.clone().into()),
        (temp_model.clone().into(), temp_view.clone().into()),
    );
    char_model.write().source_label_multiplicity = Arc::new("1".to_owned());
    char_model.write().target_label_multiplicity = Arc::new("1".to_owned());

    let (mediation_model, mediation_view) = new_umlclass_association(
        ontouml_models::MEDIATION, "", None,
        (spouse_model.clone().into(), spouse_view.clone().into()),
        (marriage_model.clone().into(), marriage_view.clone().into()),
    );
    mediation_model.write().source_label_multiplicity = Arc::new("2..*".to_owned());
    mediation_model.write().target_label_multiplicity = Arc::new("1..1".to_owned());

    let name = format!("Demo OntoUML diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            animal_model.into(),
            temp_model.into(),
            char_model.into(),
            human_model.into(),
            gen_human_model.into(),
            alive_model.into(),
            dead_model.into(),
            gen_phase_model.into(),
            spouse_model.into(),
            gen_spouse_model.into(),
            marriage_model.into(),
            mediation_model.into(),
        ],
    ));
    new_controlller(
        diagram,
        name,
        vec![
            animal_view.into(),
            temp_view.into(),
            char_view.into(),
            human_view.into(),
            gen_human_view.into(),
            alive_view.into(),
            dead_view.into(),
            gen_phase_view.into(),
            spouse_view.into(),
            gen_spouse_view.into(),
            marriage_view.into(),
            mediation_view.into(),
        ],
    )
}

pub fn deserializer(uuid: ControllerUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<UmlClassDomain<OntoUmlProfile>, OntoUmlControllerAdapter, DiagramControllerGen2<UmlClassDomain<OntoUmlProfile>, UmlClassDiagramAdapter<OntoUmlProfile>>>>(&uuid)?)
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
        ontouml_models::ontouml_class_stereotype_literal(value).is_some()
    }
    fn refresh(&mut self, new_value: &str) {
        if let Some(new_value) = ontouml_models::ontouml_class_stereotype_literal(new_value) {
            self.buffer = new_value;
        }
        self.display_string = if self.buffer.is_empty() {
            "None".to_owned()
        } else {
            format!("«{}»", self.buffer)
        };
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
        ontouml_models::ontouml_association_stereotype_literal(value).is_some()
    }
    fn refresh(&mut self, new_value: &str) {
        if let Some(new_value) = ontouml_models::ontouml_association_stereotype_literal(new_value) {
            self.buffer = new_value;
        }
        self.display_string = if self.buffer.is_empty() {
            "None".to_owned()
        } else {
            format!("«{}»", self.buffer)
        };
    }
}
