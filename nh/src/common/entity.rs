
use crate::{ModelUuid, ViewUuid};

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EntityUuid {
    Model(ModelUuid),
    View(ViewUuid),
}

pub trait Entity {
    fn tagged_uuid(&self) -> EntityUuid;
}
