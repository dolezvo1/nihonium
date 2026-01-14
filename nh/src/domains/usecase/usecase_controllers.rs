
use super::super::umlclass::{
        umlclass_models::UmlClassDiagram,
        umlclass_controllers::{LinkType, UmlClassDiagramAdapter, UmlClassDomain, UmlClassElementView, UmlClassPalette, UmlClassProfile, UmlClassToolStage, new_umlclass_association, new_umlclass_class, new_umlclass_comment, new_umlclass_commentlink, new_umlclass_generalization, new_umlclass_package},
};
use crate::{common::{
    controller::{
        BucketNoT, ControllerAdapter, DiagramController, DiagramControllerGen2, ElementControllerGen2, GlobalDrawingContext, MultiDiagramController, PositionNoT, ProjectCommand,
    },
    eref::ERef,
    project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer},
    uuid::{ControllerUuid, ModelUuid, ViewUuid},
}, domains::{umlclass::{umlclass_controllers::{new_uml_usecase, new_umlclass_dependency}, umlclass_models::UmlClassElement}, usecase::usecase_models}};
use eframe::egui;
use std::{
    collections::HashSet,
    sync::Arc,
};


#[derive(Clone, Default)]
pub struct UseCaseProfile;
impl UmlClassProfile for UseCaseProfile {
    type Palette = UmlClassPlaceholderViews;

    fn menubar_options_fun(
        _model: &ERef<UmlClassDiagram>,
        _view_uuid: &ViewUuid,
        ui: &mut egui::Ui,
        _commands: &mut Vec<ProjectCommand>,
    ) {
        /* TODO: PlantUML support?
        if ui.button("PlantUML description").clicked() {
            let uuid = uuid::Uuid::now_v7();
            commands.push(ProjectCommand::AddCustomTab(
                uuid,
                Arc::new(RwLock::new(PlantUmlTab::new(model.clone()))),
            ));
        }
        */
        ui.separator();
    }

    fn allows_class_rendering_as_stick_figure() -> bool {
        true
    }
}


#[derive(serde::Serialize, nh_derive::NHContextSerialize, nh_derive::NHContextDeserialize)]
pub struct OntoUmlControllerAdapter {
    #[nh_context_serde(entity)]
    model: ERef<UmlClassDiagram>,
}

impl ControllerAdapter<UmlClassDomain<UseCaseProfile>> for OntoUmlControllerAdapter {
    type DiagramViewT = DiagramControllerGen2<UmlClassDomain<UseCaseProfile>, UmlClassDiagramAdapter<UseCaseProfile>>;

    fn model(&self) -> ERef<UmlClassDiagram> {
        self.model.clone()
    }
    fn clone_with_model(&self, new_model: ERef<UmlClassDiagram>) -> Self {
        Self { model: new_model }
    }
    fn controller_type(&self) -> &'static str {
        "umlclass-usecase"
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
        if ui.button("Use Case Diagram").clicked() {
            return Some(Self::DiagramViewT::new(
                ViewUuid::now_v7().into(),
                "New Shared Use Case Diagram".to_owned().into(),
                UmlClassDiagramAdapter::new(self.model.clone()),
                vec![],
            ));
        }
        None
    }
}


#[derive(Clone)]
pub struct UmlClassPlaceholderViews {
    views: [(&'static str, Vec<(UmlClassToolStage, &'static str, UmlClassElementView<UseCaseProfile>)>); 3],
}

impl Default for UmlClassPlaceholderViews {
    fn default() -> Self {
        let mut classes = Vec::new();
        let (_actor, actor_view) = new_umlclass_class("Customer", usecase_models::ACTOR, false, Vec::new(), Vec::new(), egui::Pos2::ZERO, true);
        classes.push(
            (UmlClassToolStage::Class { name: "Customer", stereotype: usecase_models::ACTOR, render_as_stick_figure: true }, "Actor", actor_view.into())
        );
        let (_class, class_view) = new_umlclass_class("Customer", usecase_models::ACTOR, false, Vec::new(), Vec::new(), egui::Pos2::ZERO, false);
        class_view.write().refresh_buffers();
        classes.push(
            (UmlClassToolStage::Class { name: "Customer", stereotype: usecase_models::ACTOR, render_as_stick_figure: false }, "Class Actor", class_view.into())
        );
        let (_usecase, usecase_view) = new_uml_usecase("Registration", usecase_models::NONE, false, egui::Pos2::ZERO);
        usecase_view.write().refresh_buffers();
        classes.push(
            (UmlClassToolStage::UseCase { name: "Registration", stereotype: usecase_models::NONE }, "Use case", usecase_view.into())
        );


        let mut relationships = Vec::new();
        let dummy1 = new_umlclass_class("dummy1", usecase_models::NONE, false, Vec::new(), Vec::new(), egui::Pos2::new(100.0, 75.0), false);
        let dummy2 = new_umlclass_class("dummy2", usecase_models::NONE, false, Vec::new(), Vec::new(), egui::Pos2::new(200.0, 150.0), false);
        {
            let (_gen, gen_view) = new_umlclass_generalization(None, (dummy1.0.clone(), dummy1.1.clone().into()), (dummy2.0.clone(), dummy2.1.clone().into()));
            relationships.push(
                (UmlClassToolStage::LinkStart { link_type: LinkType::Generalization }, "Generalization (Set)", gen_view.into()),
            );
        }
        {
            let (m, m_view) = new_umlclass_association("", "", None, (dummy1.0.clone().into(), dummy1.1.clone().into()), (dummy2.0.clone().into(), dummy2.1.clone().into()));
            m.write().source_label_multiplicity = Arc::new("".to_owned());
            m.write().target_label_multiplicity = Arc::new("".to_owned());
            m_view.write().refresh_buffers();
            relationships.push(
                (UmlClassToolStage::LinkStart { link_type: LinkType::Association { stereotype: "" } }, "Association", m_view.into()),
            );
        }
        for (stereotype, label) in [(usecase_models::EXTEND, "Extend"), (usecase_models::INCLUDE, "Include")] {
            let (_d, d_view) = new_umlclass_dependency(stereotype, "", true, None, (dummy1.0.clone().into(), dummy1.1.clone().into()), (dummy2.0.clone().into(), dummy2.1.clone().into()));
            relationships.push(
                (UmlClassToolStage::LinkStart { link_type: LinkType::Dependency { target_arrow_open: true, stereotype } }, label, d_view.into()),
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

impl UmlClassPalette<UseCaseProfile> for UmlClassPlaceholderViews {
    fn iter_mut(&mut self) -> impl Iterator<Item = (&str, &mut Vec<(UmlClassToolStage, &'static str, UmlClassElementView<UseCaseProfile>)>)> {
        self.views.iter_mut().map(|e| (e.0, &mut e.1))
    }
}



fn new_controlller(
    model: ERef<UmlClassDiagram>,
    name: String,
    elements: Vec<UmlClassElementView<UseCaseProfile>>,
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
                        UmlClassDiagramAdapter::<UseCaseProfile>::new(model),
                        elements,
                    )
                ]
            )
        )
    )
}

pub fn new(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let name = format!("New Use Case diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![],
    ));
    new_controlller(diagram, name, vec![])
}

pub fn demo(no: u32) -> (ViewUuid, ERef<dyn DiagramController>) {
    let (customer_model, customer_view) = new_umlclass_class("Customer", usecase_models::ACTOR, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 200.0), true);
    let (bank_model, bank_view) = new_umlclass_class("Bank Customer", usecase_models::ACTOR, false, Vec::new(), Vec::new(), egui::Pos2::new(350.0, 300.0), false);
    let (usecase_model, usecase_view) = new_uml_usecase("Registration", usecase_models::NONE, false, egui::Pos2::new(450.0, 200.0));


    let name = format!("Demo Use Case diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            customer_model.into(),
            bank_model.into(),
            usecase_model.into(),
        ],
    ));
    new_controlller(
        diagram,
        name,
        vec![
            customer_view.into(),
            bank_view.into(),
            usecase_view.into(),
        ],
    )
}

pub fn deserializer(uuid: ControllerUuid, d: &mut NHDeserializer) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<UmlClassDomain<UseCaseProfile>, OntoUmlControllerAdapter, DiagramControllerGen2<UmlClassDomain<UseCaseProfile>, UmlClassDiagramAdapter<UseCaseProfile>>>>(&uuid)?)
}
