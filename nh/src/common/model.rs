use std::{collections::HashSet, sync::Arc};

use crate::common::{entity::Entity, uuid::ModelUuid};

pub trait Model: Entity + 'static {
    fn uuid(&self) -> Arc<ModelUuid>;
}

pub trait VisitableElement: Model {
    fn accept(&self, v: &mut dyn ElementVisitor<Self>)
    where
        Self: Sized,
    {
        v.visit_simple(self);
    }
}
pub trait VisitableDiagram: ContainerModel
where
    <Self as ContainerModel>::ElementT: VisitableElement,
{
    fn accept(&self, v: &mut dyn DiagramVisitor<Self>);
}

/// Index of a container partition. Note that 0 means "any owning partition"
/// and thus should not be used if container has multiple and/or non-owning buckets.
pub type BucketNoT = u8;
pub type PositionNoT = usize;

pub trait ContainerModel: Model {
    type ElementT: Model;

    fn find_element(&self, _uuid: &ModelUuid) -> Option<(Self::ElementT, ModelUuid)> {
        None
    }
    fn get_element_pos(&self, _uuid: &ModelUuid) -> Option<(BucketNoT, PositionNoT)> {
        None
    }
}

pub trait DiagramModel: ContainerModel {
    fn insert_element_into(
        &mut self,
        target: ModelUuid,
        bucket: BucketNoT,
        position: Option<PositionNoT>,
        element: Self::ElementT,
    ) -> Result<PositionNoT, Self::ElementT>;
    fn remove_element_from(
        &mut self,
        target: ModelUuid,
        uuid: &ModelUuid,
    ) -> Option<(BucketNoT, PositionNoT)>;
}

pub trait ElementVisitor<T: ?Sized> {
    fn open_complex(&mut self, e: &T);
    fn close_complex(&mut self, e: &T);
    fn visit_simple(&mut self, e: &T);
}
pub trait DiagramVisitor<T: ContainerModel>: ElementVisitor<T::ElementT> {
    fn open_diagram(&mut self, e: &T);
    fn close_diagram(&mut self, e: &T);
}

pub struct ModelTopSortInfo {
    pub required_models: HashSet<ModelUuid>,
    pub provided_models: HashSet<ModelUuid>,
}
