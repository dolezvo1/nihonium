
use std::{any::Any, collections::HashMap, sync::{Arc, RwLock}};

use crate::common::controller::DiagramController;


pub fn deserialize(
    from: &HashMap<uuid::Uuid, toml::Value>,
    using_elements: &mut HashMap<uuid::Uuid, Arc<dyn Any>>
) -> Result<Arc<RwLock<dyn DiagramController>>, ()> {
    Err(())
}


