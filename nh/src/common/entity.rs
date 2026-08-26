use crate::{
    ControllerUuid, ModelUuid, ViewUuid,
    common::uuid::{FolderUuid, ResourceUuid},
};

#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    derive_more::From,
)]
pub enum EntityUuid {
    Model(ModelUuid),
    View(ViewUuid),
    Controller(ControllerUuid),
    Resource(ResourceUuid),
    Folder(FolderUuid),
}

pub trait Entity {
    fn tagged_uuid(&self) -> EntityUuid;
}
