
use crate::{ModelUuid, ViewUuid, ControllerUuid};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize, derive_more::From)]
pub enum EntityUuid {
    Model(ModelUuid),
    View(ViewUuid),
    Controller(ControllerUuid),
}

pub trait Entity {
    fn tagged_uuid(&self) -> EntityUuid;
}
