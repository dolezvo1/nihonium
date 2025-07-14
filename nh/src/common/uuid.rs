
use serde::{Deserialize, Serialize};

macro_rules! impl_uuid {
    ($struct_name:ty) => {
        impl $struct_name {
            pub fn is_nil(&self) -> bool {
                self.inner.is_nil()
            }
        }

        impl From<uuid::Uuid> for $struct_name {
            fn from(value: uuid::Uuid) -> Self {
                Self { inner: value }
            }
        }

        impl ToString for $struct_name {
            fn to_string(&self) -> String {
                self.inner.to_string()
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelUuid {
    inner: uuid::Uuid,
}

impl_uuid!(ModelUuid);

#[derive(Clone, Copy, Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewUuid {
    inner: uuid::Uuid,
}

impl_uuid!(ViewUuid);
