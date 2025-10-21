
use std::sync::{Arc, RwLock};
use crate::common::entity::Entity;

/// Entity Reference - newtype to express entity boundaries
pub struct ERef<T: ?Sized>(Arc<RwLock<T>>);

unsafe impl<T: ?Sized> Send for ERef<T> {}
unsafe impl<T: ?Sized> Sync for ERef<T> {}

impl<T: ?Sized> Clone for ERef<T> {
    fn clone(&self) -> Self {
        ERef(self.0.clone())
    }
}

impl<T: ?Sized> ERef<T> {
    pub fn new(element: T) -> Self where T: Sized {
        Self(Arc::new(RwLock::new(element)))
    }

    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
        self.0.read().unwrap()
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.0.write().unwrap()
    }
}

impl<T, U> std::ops::CoerceUnsized<ERef<U>> for ERef<T>
where
    T: std::marker::Unsize<U> + ?Sized,
    U: ?Sized,
{}

impl<T> serde::Serialize for ERef<T> where T: Entity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
       self.0.read().unwrap().tagged_uuid().serialize(serializer)
    }
}
