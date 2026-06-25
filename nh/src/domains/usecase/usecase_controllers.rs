use super::super::umlclass::{
    umlclass_controllers::{
        LinkType, UmlClassDiagramAdapter, UmlClassDomain, UmlClassElementView, UmlClassProfile,
        UmlClassToolStage, new_umlclass_association, new_umlclass_class,
        new_umlclass_generalization, new_umlclass_package,
    },
    umlclass_models::UmlClassDiagram,
};
use crate::{
    DefaultSettingsF, DeserializeControllerF, DeserializeSettingsF, DiagramConstructorF,
    DiagramCreationData, DiagramInfo, ShowSettingsF,
    common::{
        controller::{
            BucketNoT, ControllerAdapter, DiagramController, DiagramControllerGen2,
            DiagramSettings, ElementControllerGen2, GlobalDrawingContext, InsensitiveCommand,
            MGlobalColor, MultiDiagramController, PositionNoT, View,
        },
        eref::ERef,
        project_serde::{NHDeserializeError, NHDeserializeInstantiator, NHDeserializer},
        uuid::{ControllerUuid, ModelUuid, ViewUuid},
    },
    domains::{
        umlclass::{
            umlclass_controllers::{
                PartialUmlClassElement, UmlClassElementOrVertex, UmlClassRenderStyle,
                new_uml_usecase,
            },
            umlclass_models::{UmlClass, UmlClassElement, UmlClassInstance, UmlClassPackageKind},
        },
        usecase::usecase_models,
    },
};
use eframe::egui;
use std::collections::HashSet;

#[derive(Clone, Default)]
pub struct UseCaseProfile;
impl UmlClassProfile for UseCaseProfile {
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
    type DiagramViewT = DiagramControllerGen2<
        UmlClassDomain<UseCaseProfile>,
        UmlClassDiagramAdapter<UseCaseProfile>,
    >;

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
        super::super::umlclass::umlclass_models::transitive_closure(
            &self.model.read(),
            when_deleting,
        )
    }

    fn insert_element(
        &mut self,
        parent: ModelUuid,
        element: UmlClassElement,
        b: BucketNoT,
        p: Option<PositionNoT>,
    ) -> Result<(), ()> {
        self.model
            .write()
            .insert_element_into(parent, element, b, p)
    }

    fn delete_elements(
        &mut self,
        uuids: &HashSet<ModelUuid>,
        undo: &mut Vec<(ModelUuid, UmlClassElement, BucketNoT, PositionNoT)>,
    ) {
        self.model.write().delete_elements(uuids, undo)
    }

    fn show_add_shared_diagram_menu(
        &self,
        _gdc: &GlobalDrawingContext,
        ui: &mut egui::Ui,
    ) -> Option<ERef<Self::DiagramViewT>> {
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

fn new_controlller(
    model: ERef<UmlClassDiagram>,
    name: String,
    elements: Vec<UmlClassElementView<UseCaseProfile>>,
) -> (ViewUuid, ERef<dyn DiagramController>) {
    let uuid = ViewUuid::now_v7();
    (
        uuid,
        ERef::new(MultiDiagramController::new(
            ControllerUuid::now_v7(),
            OntoUmlControllerAdapter {
                model: model.clone(),
            },
            vec![DiagramControllerGen2::new(
                uuid.into(),
                name.into(),
                UmlClassDiagramAdapter::<UseCaseProfile>::new(model),
                elements,
            )],
        )),
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
    let (customer_model, customer_view) = new_umlclass_class(
        "Customer",
        usecase_models::ACTOR,
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(300.0, 200.0),
        UmlClassRenderStyle::StickFigure,
        MGlobalColor::None,
    );
    let (bank_model, bank_view) = new_umlclass_class(
        "Bank Customer",
        usecase_models::ACTOR,
        false,
        Vec::new(),
        Vec::new(),
        egui::Pos2::new(300.0, 400.0),
        UmlClassRenderStyle::Class,
        MGlobalColor::None,
    );
    let (usecase_model, usecase_view) = new_uml_usecase(
        "Registration",
        usecase_models::NONE,
        false,
        egui::Pos2::new(550.0, 200.0),
        MGlobalColor::None,
    );

    let (gen_model, gen_view) = new_umlclass_generalization(
        "",
        None,
        (bank_model.clone(), bank_view.clone().into()),
        (customer_model.clone(), customer_view.clone().into()),
    );
    let (assoc_model, assoc_view) = new_umlclass_association(
        "",
        "",
        "0..*",
        "1..1",
        None,
        (customer_model.clone().into(), customer_view.clone().into()),
        (usecase_model.into(), usecase_view.clone().into()),
    );

    let (boundary, boundary_view) = new_umlclass_package(
        "E-Shop",
        "business",
        UmlClassPackageKind::Boundary,
        egui::Rect::from_x_y_ranges(400.0..=750.0, 100.0..=500.0),
    );
    {
        let mut w = boundary_view.write();
        let boundary_uuid = *w.uuid();
        let (mut u, mut a) = Default::default();
        for e in [usecase_view.into()] {
            w.apply_command(
                &InsensitiveCommand::AddDependency {
                    target: boundary_uuid,
                    bucket: 0,
                    position: None,
                    element: UmlClassElementOrVertex::Element(e),
                    into_model: true,
                },
                &mut u,
                &mut a,
            );
        }
    }

    let name = format!("Demo Use Case diagram {}", no);
    let diagram = ERef::new(UmlClassDiagram::new(
        ModelUuid::now_v7(),
        name.clone(),
        vec![
            customer_model.into(),
            bank_model.into(),
            gen_model.into(),
            assoc_model.into(),
            boundary.into(),
        ],
    ));
    new_controlller(
        diagram,
        name,
        vec![
            customer_view.into(),
            bank_view.into(),
            gen_view.into(),
            assoc_view.into(),
            boundary_view.into(),
        ],
    )
}

pub fn deserializer(
    uuid: ControllerUuid,
    d: &mut NHDeserializer,
) -> Result<ERef<dyn DiagramController>, NHDeserializeError> {
    Ok(d.get_entity::<MultiDiagramController<
        UmlClassDomain<UseCaseProfile>,
        OntoUmlControllerAdapter,
        DiagramControllerGen2<
            UmlClassDomain<UseCaseProfile>,
            UmlClassDiagramAdapter<UseCaseProfile>,
        >,
    >>(&uuid)?)
}

mod buttons {
    use super::*;
    use std::sync::LazyLock;

    fn instance_association(
        m: ERef<UmlClassInstance>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UseCaseProfile>,
        bool,
    ) {
        let link_type = LinkType::Association {
            stereotype: "".to_owned(),
            source_multiplicity: "0..*".to_owned(),
            target_multiplicity: "1..1".to_owned(),
        };
        (
            UmlClassToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlClassToolStage::LinkEnd,
            PartialUmlClassElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    type InstanceButtonF = dyn Fn(
        ERef<UmlClassInstance>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UseCaseProfile>,
        bool,
    );
    pub const INSTANCE_BUTTONS: LazyLock<
        Vec<(usize, usize, &'static str, &'static InstanceButtonF)>,
    > = LazyLock::new(|| vec![(0, 0, "\\", &instance_association as &InstanceButtonF)]);
    fn class_association(
        m: ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UseCaseProfile>,
        bool,
    ) {
        let link_type = LinkType::Association {
            stereotype: "".to_owned(),
            source_multiplicity: "0..*".to_owned(),
            target_multiplicity: "1..1".to_owned(),
        };
        (
            UmlClassToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlClassToolStage::LinkEnd,
            PartialUmlClassElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    fn class_generalization(
        m: ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UseCaseProfile>,
        bool,
    ) {
        let link_type = LinkType::Generalization {
            set_name: "".to_owned(),
        };
        (
            UmlClassToolStage::LinkStart {
                link_type: link_type.clone(),
            },
            UmlClassToolStage::LinkEnd,
            PartialUmlClassElement::Link {
                link_type,
                source: m.into(),
                dest: None,
            },
            true,
        )
    }
    type ClassButtonF = dyn Fn(
        ERef<UmlClass>,
    ) -> (
        UmlClassToolStage,
        UmlClassToolStage,
        PartialUmlClassElement<UseCaseProfile>,
        bool,
    );
    pub const CLASS_BUTTONS: LazyLock<Vec<(usize, usize, &'static str, &'static ClassButtonF)>> =
        LazyLock::new(|| {
            vec![
                (0, 0, "\\", &class_association as &ClassButtonF),
                (0, 1, "↘", &class_generalization as &ClassButtonF),
            ]
        });
}

pub fn default_settings() -> Box<dyn DiagramSettings> {
    let classes = vec![
        (
            UmlClassToolStage::Class {
                name: "Customer".to_owned(),
                stereotype: usecase_models::ACTOR.to_owned(),
                is_abstract: false,
                render_style: UmlClassRenderStyle::StickFigure,
                background_color: MGlobalColor::None,
            },
            "Actor",
            Some(egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Num1,
            )),
        ),
        (
            UmlClassToolStage::Class {
                name: "Customer".to_owned(),
                stereotype: usecase_models::ACTOR.to_owned(),
                is_abstract: false,
                render_style: UmlClassRenderStyle::Class,
                background_color: MGlobalColor::None,
            },
            "Class Actor",
            Some(egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Num2,
            )),
        ),
        (
            UmlClassToolStage::UseCase {
                name: "Registration".to_owned(),
                stereotype: usecase_models::NONE.to_owned(),
                is_abstract: false,
                background_color: MGlobalColor::None,
            },
            "Use case",
            Some(egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Num3,
            )),
        ),
    ];

    let mut relationships = Vec::new();
    relationships.push((
        UmlClassToolStage::LinkStart {
            link_type: LinkType::Generalization {
                set_name: "".to_owned(),
            },
        },
        "Generalization (Set)",
        Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Num5,
        )),
    ));
    relationships.push((
        UmlClassToolStage::LinkStart {
            link_type: LinkType::Association {
                stereotype: "".to_owned(),
                source_multiplicity: "0..*".to_owned(),
                target_multiplicity: "1..1".to_owned(),
            },
        },
        "Association",
        Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Num6,
        )),
    ));
    for (stereotype, label) in [
        (usecase_models::EXTEND, "Extend"),
        (usecase_models::INCLUDE, "Include"),
    ] {
        relationships.push((
            UmlClassToolStage::LinkStart {
                link_type: LinkType::Dependency {
                    target_arrow_open: true,
                    stereotype: stereotype.to_owned(),
                    name: "".to_owned(),
                },
            },
            label,
            None,
        ));
    }

    let palette_items = vec![
        ("Classes", classes),
        ("Relationships", relationships),
        (
            "Other",
            vec![
                (
                    UmlClassToolStage::PackageStart {
                        name: "Boundary".to_owned(),
                        stereotype: "".to_owned(),
                        kind: UmlClassPackageKind::Boundary,
                    },
                    "Boundary",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num8,
                    )),
                ),
                (
                    UmlClassToolStage::Comment {
                        stereotype: "".to_owned(),
                        text: "a comment".to_owned(),
                        align: egui::Align2::CENTER_CENTER,
                    },
                    "Comment",
                    Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::Num9,
                    )),
                ),
                (UmlClassToolStage::CommentLinkStart, "Comment Link", None),
            ],
        ),
    ];

    super::super::umlclass::umlclass_controllers::default_settings_helper::<UseCaseProfile>(
        palette_items,
        buttons::INSTANCE_BUTTONS.clone(),
        buttons::CLASS_BUTTONS.clone(),
    )
}

pub fn settings_deserializer(value: toml::Value) -> Result<Box<dyn DiagramSettings>, ()> {
    super::super::umlclass::umlclass_controllers::settings_deserializer_helper::<UseCaseProfile>(
        value,
        buttons::INSTANCE_BUTTONS.clone(),
        buttons::CLASS_BUTTONS.clone(),
    )
}

pub fn settings_function(
    gdc: &mut GlobalDrawingContext,
    ui: &mut egui::Ui,
    s: &mut Box<dyn DiagramSettings>,
) {
    super::super::umlclass::umlclass_controllers::settings_function_helper::<UseCaseProfile>(
        gdc, ui, s,
    );
}

inventory::submit! {DiagramInfo {
    type_indentifier: "umlclass-usecase",
    pretty_name: "Use Case diagram",
    default_settings: &(default_settings as DefaultSettingsF),
    settings_deserializer: &(settings_deserializer as DeserializeSettingsF),
    show_settings_function: &(settings_function as ShowSettingsF),
    diagram_creation_data: DiagramCreationData {
        directory: "/Unified Modeling Language",
        description: "Use Case diagram (users, use cases, etc.)",
        constructors: &[
            ("empty", &(new as DiagramConstructorF)),
            ("demo", &(demo as DiagramConstructorF)),
        ],
    },
    deserializer: &(deserializer as DeserializeControllerF),
}}
