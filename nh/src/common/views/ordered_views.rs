use std::collections::{HashMap, HashSet};

use crate::common::{controller::{Arrangement, View}, project_serde::{NHContextDeserialize, NHContextSerialize, NHDeserializeError, NHDeserializer, NHSerializeError, NHSerializer}, uuid::ViewUuid};


pub struct OrderedViews<T> where T: View {
    order: Vec<ViewUuid>,
    views: HashMap<ViewUuid, T>,
}

impl<T> OrderedViews<T> where T: View {
    pub fn new(ov: Vec<T>) -> Self {
        let order = ov.iter().map(|e| *e.uuid()).collect();
        let views = ov.into_iter().map(|e| { let uuid = *e.uuid(); (uuid, e) }).collect();
        Self { order, views }
    }

    pub fn iter_event_order_keys(&self) -> impl Iterator<Item = ViewUuid> {
        self.order.iter().cloned()
    }

    pub fn iter_event_order_pairs(&self) -> impl Iterator<Item = (ViewUuid, &T)> {
        self.order.iter().flat_map(|k| self.views.get(k).and_then(|e| Some((*k, e))))
    }

    pub fn event_order_foreach(&self, f: impl FnMut(&T)) {
        self.order.iter().flat_map(|e| self.views.get(e)).for_each(f);
    }

    pub fn event_order_foreach_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for k in self.order.iter() {
            if let Some(v) = self.views.get_mut(k) {
                f(v);
            }
        }
    }

    pub fn draw_order_foreach_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for k in self.order.iter().rev() {
            if let Some(v) = self.views.get_mut(k) {
                f(v);
            }
        }
    }

    pub fn event_order_find_mut<U>(&mut self, mut f: impl FnMut(&mut T) -> Option<U>) -> Option<U> {
        for k in self.order.iter() {
            if let Some(u) = self.views.get_mut(k).and_then(&mut f) {
                return Some(u);
            }
        }
        None
    }

    pub fn push(&mut self, uuid: ViewUuid, view: T) {
        self.views.insert(uuid, view);
        self.order.push(uuid);
    }

    pub fn retain(&mut self, mut f: impl FnMut(&ViewUuid, &T) -> bool) {
        let mut keep = HashSet::new();
        self.views.retain(|k, v| if f(k, v) { keep.insert(*k); true } else { false });
        self.order.retain(|k| keep.contains(k));
    }

    pub fn apply_arrangement(&mut self, uuids: &HashSet<ViewUuid>, arr: Arrangement) {
        match arr {
            Arrangement::BringToFront
            | Arrangement::SendToBack => {
                let mut modified = vec![];
                let mut remainder = vec![];
                for e in self.order.drain(..) {
                    if uuids.contains(&e) {
                        modified.push(e);
                    } else {
                        remainder.push(e);
                    }
                }
                match arr {
                    Arrangement::BringToFront => {
                        self.order.extend(modified);
                        self.order.extend(remainder);
                    }
                    Arrangement::SendToBack => {
                        self.order.extend(remainder);
                        self.order.extend(modified);
                    }
                    _ => unreachable!(),
                }
            },
            Arrangement::ForwardOne => {
                if self.order.len() > 1 {
                    if uuids.contains(&self.order[0])
                        && uuids.contains(&self.order[1]) {
                        self.order.swap(0, 1);
                    }
                    for ii in 0..self.order.len()-1 {
                        if uuids.contains(&self.order[ii+1]) {
                            self.order.swap(ii, ii+1);
                        }
                    }
                }
            },
            Arrangement::BackwardOne => {
                let ll = self.order.len();
                if ll > 1 {
                    if uuids.contains(&self.order[ll-2])
                        && uuids.contains(&self.order[ll-1]) {
                        self.order.swap(ll-2, ll-1);
                    }
                    for ii in (0..self.order.len()-1).rev() {
                        if uuids.contains(&self.order[ii]) {
                            self.order.swap(ii, ii+1);
                        }
                    }
                }
            },
        }
    }
}

impl<T> serde::Serialize for OrderedViews<T> where T: View + serde::Serialize + Clone {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        self.order.iter().flat_map(|e| self.views.get(e).cloned()).collect::<Vec<_>>().serialize(serializer)
    }
}

impl<T> NHContextSerialize for OrderedViews<T> where T: View + NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        for e in self.views.values() {
            e.serialize_into(into)?;
        }
        Ok(())
    }
}

impl<T> NHContextDeserialize for OrderedViews<T> where T: View + NHContextDeserialize {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        Ok(Self::new(<Vec<T>>::deserialize(source, deserializer)?))
    }
}
