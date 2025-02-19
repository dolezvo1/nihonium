use std::sync::{Arc, RwLock};

// The Observer trait
pub trait Observer: Send + Sync {
    fn update(&mut self);
}

// The Observable trait
pub trait Observable: Send + Sync {
    fn notify_observers(&mut self);
    fn register_observer(&mut self, observer: Arc<RwLock<dyn Observer>>);
    fn unregister_observer(&mut self, observer: &Arc<RwLock<dyn Observer>>);
}

// Macro for generating Observable implementations
macro_rules! impl_observable {
    ($observable:ty) => {
        impl Observable for $observable {
            fn notify_observers(&mut self) {
                for observer in self.observers.iter() {
                    observer.write().unwrap().update();
                }
            }
            fn register_observer(&mut self, observer: Arc<RwLock<dyn Observer>>) {
                self.observers.push_back(observer);
            }
            fn unregister_observer(&mut self, observer: &Arc<RwLock<dyn Observer>>) {
                self.observers
                    .retain(|o| !std::ptr::addr_eq(Arc::as_ptr(&o), Arc::as_ptr(&observer)));
            }
        }
    };
}
pub(crate) use impl_observable;
