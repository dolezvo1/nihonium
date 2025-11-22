
// "NONE"
pub const NONE: &str = "";

// Class stereotypes
// Sortals
pub const KIND: &str = "kind";
pub const SUBKIND: &str = "subkind";
pub const PHASE: &str = "phase";
pub const ROLE: &str = "role";
pub const COLLECTIVE: &str = "collective";
pub const QUANTITY: &str = "quantity";
pub const RELATOR: &str = "relator";
// Nonsortals
pub const CATEGORY: &str = "category";
pub const PHASE_MIXIN: &str = "phaseMixin";
pub const ROLE_MIXIN: &str = "roleMixin";
pub const MIXIN: &str = "mixin";
// Aspects
pub const MODE: &str = "mode";
pub const QUALITY: &str = "quality";

pub fn ontouml_class_stereotype_literal(e: &str) -> Option<&'static str> {
    let e = match e {
        NONE => NONE,
        // Sortals
        KIND => KIND,
        SUBKIND => SUBKIND,
        PHASE => PHASE,
        ROLE => ROLE,
        COLLECTIVE => COLLECTIVE,
        QUANTITY => QUANTITY,
        RELATOR => RELATOR,
        // Nonsortals
        CATEGORY => CATEGORY,
        PHASE_MIXIN => PHASE_MIXIN,
        ROLE_MIXIN => ROLE_MIXIN,
        MIXIN => MIXIN,
        // Aspects
        MODE => MODE,
        QUALITY => QUALITY,
        _ => return None,
    };
    Some(e)
}

// Association stereotypes
pub const FORMAL: &str = "formal";
pub const MEDIATION: &str = "mediation";
pub const CHARACTERIZATION: &str = "characterization";
pub const STRUCTURATION: &str = "structuration";
pub const COMPONENT_OF: &str = "componentOf";
pub const CONTAINMENT: &str = "containment";
pub const MEMBER_OF: &str = "memberOf";
pub const SUBCOLLECTION_OF: &str = "subcollectionOf";
pub const SUBQUANTITY_OF: &str = "subquantityOf";

pub fn ontouml_association_stereotype_literal(e: &str) -> Option<&'static str> {
    let e = match e {
        NONE => NONE,
        FORMAL => FORMAL,
        MEDIATION => MEDIATION,
        CHARACTERIZATION => CHARACTERIZATION,
        STRUCTURATION => STRUCTURATION,
        COMPONENT_OF => COMPONENT_OF,
        CONTAINMENT => CONTAINMENT,
        MEMBER_OF => MEMBER_OF,
        SUBCOLLECTION_OF => SUBCOLLECTION_OF,
        SUBQUANTITY_OF => SUBQUANTITY_OF,
        _ => return None,
    };
    Some(e)
}
