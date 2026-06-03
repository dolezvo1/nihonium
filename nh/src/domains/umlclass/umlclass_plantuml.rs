
use crate::{common::{controller::Model, uuid::ModelUuid}, domains::umlclass::umlclass_models::{UmlClass, UmlClassAssociation, UmlClassAssociationAggregation, UmlClassAssociationNavigability, UmlClassComment, UmlClassCommentLink, UmlClassDependency, UmlClassGeneralization, UmlClassInstance, UmlClassPackage, UmlClassVisitor, UmlUseCase, UmlUseCaseGeneralization}};


pub struct UmlClassPlantUmlCollector {
    plantuml_structures: String,
    plantuml_links: String,
}

impl UmlClassPlantUmlCollector {
    pub fn new() -> Self {
        Self {
            plantuml_structures: "".to_owned(),
            plantuml_links: "".to_owned(),
        }
    }
    pub fn finish(mut self) -> String {
        self.plantuml_structures.push_str(&self.plantuml_links);
        self.plantuml_structures
    }

    fn stringify_uuid(uuid: &ModelUuid) -> String {
        "m_".chars().chain(uuid.to_string().chars().filter(|e| *e != '-')).collect()
    }
}

impl UmlClassVisitor for UmlClassPlantUmlCollector {
    fn visit_package(&mut self, package: &UmlClassPackage) {
        self.plantuml_structures
            .push_str(&format!("package {} as {:?} ", Self::stringify_uuid(&package.uuid), package.name));
        if !package.stereotype.is_empty() {
            self.plantuml_structures.push_str(&format!("<<{}>> ", package.stereotype));
        }
        self.plantuml_structures.push_str("{\n");

        for e in &package.contained_elements {
            e.accept_uml(self);
        }

        self.plantuml_structures.push_str("}\n");
    }
    fn visit_instance(&mut self, instance: &UmlClassInstance) {
        self.plantuml_structures.push_str(&format!(
            "object {} as {:?}\n",
            Self::stringify_uuid(&instance.uuid),
            if instance.instance_name.is_empty() {
                format!(":{}", instance.instance_type)
            } else {
                format!("{}: {}", instance.instance_name, instance.instance_type)
            },
        ));
    }
    fn visit_class(&mut self, class: &UmlClass) {
        self.plantuml_structures.push_str(&format!(
            "class {} as {:?} ",
            Self::stringify_uuid(&class.uuid),
            class.name,
        ));

        if !class.stereotype.is_empty() {
            self.plantuml_structures.push_str(&format!("<<{}>> ", class.stereotype));
        }
        self.plantuml_structures.push_str("{\n");
        for e in &class.properties {
            let r = e.read();
            let visibility = r.visibility.as_ref().map(|e| e.as_char()).unwrap_or("");
            let value_type = if !r.value_type.is_empty() {
                format!(": {}", r.value_type)
            } else {
                "".to_owned()
            };
            self.plantuml_structures.push_str(&format!("  {}{}{}\n", visibility, r.name, value_type));
        }
        for e in &class.operations {
            let r = e.read();
            let visibility = r.visibility.as_ref().map(|e| e.as_char()).unwrap_or("");
            let return_type = if !r.return_type.is_empty() {
                format!(": {}", r.return_type)
            } else {
                "".to_owned()
            };
            self.plantuml_structures.push_str(&format!("  {}{}({}){}\n", visibility, r.name, r.parameters, return_type));
        }
        self.plantuml_structures.push_str("}\n");
    }
    fn visit_generalization(&mut self, link: &UmlClassGeneralization) {
        for source in link.sources.iter().map(|e| Self::stringify_uuid(&e.read().uuid)) {
            for target in link.targets.iter().map(|e| Self::stringify_uuid(&e.read().uuid)) {
                self.plantuml_links.push_str(&source);
                self.plantuml_links.push_str(" --|> ");
                self.plantuml_links.push_str(&target);
                self.plantuml_links.push_str("\n");
            }
        }
    }
    fn visit_dependency(&mut self, link: &UmlClassDependency) {
        let source = Self::stringify_uuid(&link.source.uuid());
        let target = Self::stringify_uuid(&link.target.uuid());

        self.plantuml_links.push_str(&source);
        if link.target_arrow_open {
            self.plantuml_links.push_str(" ..> ");
        } else {
            self.plantuml_links.push_str(" ..|> ");
        }
        self.plantuml_links.push_str(&target);
        if !link.stereotype.is_empty() {
            self.plantuml_links.push_str(&format!(": <<{}>>", link.stereotype));
        }
        self.plantuml_links.push_str("\n");
    }
    fn visit_association(&mut self, link: &UmlClassAssociation) {
        let source = Self::stringify_uuid(&link.source.uuid());
        let target = Self::stringify_uuid(&link.target.uuid());

        self.plantuml_links.push_str(&source);
        if !link.source_label_multiplicity.is_empty() {
            self.plantuml_links
                .push_str(&format!(" {:?}", link.source_label_multiplicity));
        }
        fn ah(
            target: bool,
            n: UmlClassAssociationNavigability,
            a: UmlClassAssociationAggregation,
        ) -> &'static str {
            match a {
                UmlClassAssociationAggregation::None => match n {
                    UmlClassAssociationNavigability::Unspecified => "",
                    UmlClassAssociationNavigability::NonNavigable => "x",
                    UmlClassAssociationNavigability::Navigable => if !target { "<" } else { ">" },
                }
                UmlClassAssociationAggregation::Shared => "o",
                UmlClassAssociationAggregation::Composite => "*",
            }
        }
        self.plantuml_links.push_str(
            &format!(
                " {}-{} ",
                ah(false, link.source_navigability, link.source_aggregation),
                ah(true, link.target_navigability, link.target_aggregation),
            )
        );
        if !link.target_label_multiplicity.is_empty() {
            self.plantuml_links
                .push_str(&format!("{:?} ", link.target_label_multiplicity));
        }
        self.plantuml_links.push_str(&target);
        if !link.stereotype.is_empty() {
            self.plantuml_links.push_str(&format!(": <<{}>>", link.stereotype));
        }
        self.plantuml_links.push_str("\n");
    }
    fn visit_comment(&mut self, comment: &UmlClassComment) {
        let s = {
            let mut s = String::new();
            if !comment.stereotype.is_empty() {
                s.push_str("<<");
                s.push_str(&comment.stereotype);
                s.push_str(">>\n");
            }
            s.push_str(&comment.text);
            s
        };
        self.plantuml_structures.push_str(&format!("note {:?} as {}\n", s, Self::stringify_uuid(&comment.uuid)));
    }
    fn visit_commentlink(&mut self, comment_link: &UmlClassCommentLink) {
        self.plantuml_links.push_str(&format!(
            "{} .. {}\n",
            Self::stringify_uuid(&comment_link.source.read().uuid),
            Self::stringify_uuid(&comment_link.target.uuid()),
        ));
    }

    fn visit_usecase(&mut self, usecase: &UmlUseCase) {
        self.plantuml_structures.push_str(&format!(
            "class {} as {:?} <<usecase>> ",
            Self::stringify_uuid(&usecase.uuid),
            usecase.name,
        ));

        if !usecase.stereotype.is_empty() {
            self.plantuml_structures.push_str(&format!("<<{}>> ", usecase.stereotype));
        }

        self.plantuml_structures.push_str("{}\n");
    }
    fn visit_usecasegeneralization(&mut self, g: &UmlUseCaseGeneralization) {
        for source in g.sources.iter().map(|e| Self::stringify_uuid(&e.read().uuid)) {
            for target in g.targets.iter().map(|e| Self::stringify_uuid(&e.read().uuid)) {
                self.plantuml_links.push_str(&source);
                self.plantuml_links.push_str(" --|> ");
                self.plantuml_links.push_str(&target);
                self.plantuml_links.push_str("\n");
            }
        }
    }
}
