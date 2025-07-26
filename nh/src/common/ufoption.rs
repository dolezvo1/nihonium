
/// Unflattening Option - works exactly like the standard option, but the variants are serialized without flattening
/// That allows for saner TOML serialization (citation needed)
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum UFOption<T> {
    None,
    Some(T)
}

impl<T> UFOption<T> {
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Self::None => None,
            Self::Some(e) => Some(e),
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        match *self {
            Self::None => None,
            Self::Some(ref mut e) => Some(e),
        }
    }

    pub fn is_some(&self) -> bool {
        match self {
            Self::None => false,
            Self::Some(_) => true,
        }
    }

    pub fn unwrap(self) -> T {
        match self {
            Self::None => panic!("UFOption::unwrap on UFOption::None"),
            Self::Some(e) => e,
        }
    }
}

impl<T> Default for UFOption<T> {
    fn default() -> Self {
        Self::None
    }
}

impl<T> From<UFOption<T>> for Option<T> {
    fn from(value: UFOption<T>) -> Self {
        match value {
            UFOption::None => None,
            UFOption::Some(e) => Some(e),
        }
    }
}

impl<T> From<Option<T>> for UFOption<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            None => Self::None,
            Some(e) => Self::Some(e),
        }
    }
}

impl<T> serde::Serialize for UFOption<T> where T: serde::Serialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        #[derive(serde::Serialize)]
        enum Helper<T> {
            None,
            Some(T)
        }
        let h = match self {
            Self::None => Helper::None,
            Self::Some(e) => Helper::Some(e),
        };
        h.serialize(serializer)
    }
}

impl<'a, T> serde::Deserialize<'a> for UFOption<T> where T: serde::Deserialize<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'a> {
        #[derive(serde::Deserialize)]
        enum Helper<T> {
            None,
            Some(T)
        }
        Helper::<T>::deserialize(deserializer)
            .map(|e| match e {
                Helper::None => Self::None,
                Helper::Some(e) => Self::Some(e),
            })
    }
}
