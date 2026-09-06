use crate::common::entity::Entity;
use std::sync::Arc;

/// Entity Reference - newtype to express entity boundaries
pub struct ERef<T: ?Sized>(Arc<parking_lot::RwLock<T>>);

unsafe impl<T: ?Sized> Send for ERef<T> {}
unsafe impl<T: ?Sized> Sync for ERef<T> {}

impl<T: ?Sized> Clone for ERef<T> {
    fn clone(&self) -> Self {
        ERef(self.0.clone())
    }
}

const DEADLOCK_DURATION: std::time::Duration = std::time::Duration::from_secs(10);

impl<T: ?Sized> ERef<T> {
    pub fn new(element: T) -> Self
    where
        T: Sized,
    {
        Self(Arc::new(parking_lot::RwLock::new(element)))
    }

    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn read(&self) -> parking_lot::MappedRwLockReadGuard<'_, T> {
        let guard = if cfg!(debug_assertions) {
            self.0.try_read_for(DEADLOCK_DURATION).unwrap_or_else(|| {
                panic!(
                    "DEBUG PANIC: Failed to acquire RwLock read after {}s. Deadlock?",
                    DEADLOCK_DURATION.as_secs()
                )
            })
        } else {
            self.0.read()
        };
        parking_lot::RwLockReadGuard::map(guard, |v| v)
    }

    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn write(&self) -> parking_lot::MappedRwLockWriteGuard<'_, T> {
        let guard = if cfg!(debug_assertions) {
            self.0.try_write_for(DEADLOCK_DURATION).unwrap_or_else(|| {
                panic!(
                    "DEBUG PANIC: Failed to acquire RwLock write after {}s. Deadlock?",
                    DEADLOCK_DURATION.as_secs()
                )
            })
        } else {
            self.0.write()
        };
        parking_lot::RwLockWriteGuard::map(guard, |v| v)
    }
}

impl<T, U> std::ops::CoerceUnsized<ERef<U>> for ERef<T>
where
    T: std::marker::Unsize<U> + ?Sized,
    U: ?Sized,
{
}

impl<T> serde::Serialize for ERef<T>
where
    T: Entity,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0
            .try_read()
            .unwrap()
            .tagged_uuid()
            .serialize(serializer)
    }
}
