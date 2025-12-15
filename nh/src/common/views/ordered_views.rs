use std::collections::{HashMap, HashSet};

use crate::common::{controller::{Arrangement, View}, eref::ERef, project_serde::{NHContextDeserialize, NHContextSerialize, NHDeserializeError, NHDeserializer, NHSerializeError, NHSerializer}, uuid::ViewUuid};


pub struct OrderedViews<T> where T: View {
    draw_order: Vec<ViewUuid>,
    views: HashMap<ViewUuid, T>,
}

impl<T> OrderedViews<T> where T: View {
    pub fn new(ov: Vec<T>) -> Self {
        let draw_order = ov.iter().map(|e| *e.uuid()).rev().collect();
        let views = ov.into_iter().map(|e| { let uuid = *e.uuid(); (uuid, e) }).collect();
        Self { draw_order, views }
    }

    pub fn iter_event_order_keys(&self) -> impl Iterator<Item = ViewUuid> {
        self.draw_order.iter().rev().cloned()
    }

    pub fn iter_event_order_pairs(&self) -> impl Iterator<Item = (ViewUuid, &T)> {
        self.draw_order.iter().rev().flat_map(|k| self.views.get(k).and_then(|e| Some((*k, e))))
    }

    pub fn event_order_foreach(&self, f: impl FnMut(&T)) {
        self.draw_order.iter().rev().flat_map(|e| self.views.get(e)).for_each(f);
    }

    pub fn event_order_foreach_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for k in self.draw_order.iter().rev() {
            if let Some(v) = self.views.get_mut(k) {
                f(v);
            }
        }
    }

    pub fn draw_order_foreach_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for k in self.draw_order.iter() {
            if let Some(v) = self.views.get_mut(k) {
                f(v);
            }
        }
    }

    pub fn event_order_find_mut<U>(&mut self, mut f: impl FnMut(&mut T) -> Option<U>) -> Option<U> {
        for k in self.draw_order.iter().rev() {
            if let Some(u) = self.views.get_mut(k).and_then(&mut f) {
                return Some(u);
            }
        }
        None
    }

    pub fn push(&mut self, uuid: ViewUuid, view: T) {
        self.views.insert(uuid, view);
        self.draw_order.push(uuid);
    }

    pub fn retain(&mut self, mut f: impl FnMut(&ViewUuid, &T) -> bool) {
        let mut keep = HashSet::new();
        self.views.retain(|k, v| if f(k, v) { keep.insert(*k); true } else { false });
        self.draw_order.retain(|k| keep.contains(k));
    }

    pub fn apply_arrangement(&mut self, uuids: &HashSet<ViewUuid>, arr: Arrangement) {
        match arr {
            Arrangement::BringToFront
            | Arrangement::SendToBack => {
                let mut modified = vec![];
                let mut remainder = vec![];
                for e in self.draw_order.drain(..) {
                    if uuids.contains(&e) {
                        modified.push(e);
                    } else {
                        remainder.push(e);
                    }
                }
                match arr {
                    Arrangement::BringToFront => {
                        self.draw_order.extend(remainder);
                        self.draw_order.extend(modified);
                    }
                    Arrangement::SendToBack => {
                        self.draw_order.extend(modified);
                        self.draw_order.extend(remainder);
                    }
                    _ => unreachable!(),
                }
            },
            Arrangement::BackwardOne => {
                if self.draw_order.len() > 1 {
                    if uuids.contains(&self.draw_order[0])
                        && uuids.contains(&self.draw_order[1]) {
                        self.draw_order.swap(0, 1);
                    }
                    for ii in 0..self.draw_order.len()-1 {
                        if uuids.contains(&self.draw_order[ii+1]) {
                            self.draw_order.swap(ii, ii+1);
                        }
                    }
                }
            },
            Arrangement::ForwardOne => {
                let ll = self.draw_order.len();
                if ll > 1 {
                    if uuids.contains(&self.draw_order[ll-2])
                        && uuids.contains(&self.draw_order[ll-1]) {
                        self.draw_order.swap(ll-2, ll-1);
                    }
                    for ii in (0..self.draw_order.len()-1).rev() {
                        if uuids.contains(&self.draw_order[ii]) {
                            self.draw_order.swap(ii, ii+1);
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
        self.draw_order.iter().rev().flat_map(|e| self.views.get(e).cloned()).collect::<Vec<_>>().serialize(serializer)
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
        Ok(Self::new(<Vec<T>>::deserialize(source, deserializer)?.into_iter().rev().collect()))
    }
}


// TODO: this should not exist - just use OrderedViews with a single-variant enum
pub struct OrderedViewRefs<T: View> {
    order: Vec<ViewUuid>,
    views: HashMap<ViewUuid, ERef<T>>,
}

impl<T: View> OrderedViewRefs<T> {
    pub fn new(views: Vec<ERef<T>>) -> Self {
        let order = views.iter().map(|e| *e.read().uuid()).rev().collect();
        let views = views.into_iter().map(|e| { let uuid = *e.read().uuid(); (uuid, e) }).collect();
        Self { order, views }
    }

    pub fn get(&self, uuid: &ViewUuid) -> Option<&ERef<T>> {
        self.views.get(uuid)
    }

    pub fn push(&mut self, uuid: ViewUuid, view: ERef<T>) {
        self.views.insert(uuid, view);
        self.order.push(uuid);
    }

    pub fn draw_order_foreach_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for k in self.order.iter() {
            if let Some(v) = self.views.get(k) {
                let mut w = v.write();
                f(&mut w);
            }
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &ViewUuid> {
        self.order.iter()
    }
}

impl<T> serde::Serialize for OrderedViewRefs<T> where T: View {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        self.order.iter().rev().flat_map(|e| self.views.get(e).cloned()).collect::<Vec<_>>().serialize(serializer)
    }
}

impl<T> NHContextSerialize for OrderedViewRefs<T> where T: View + NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        for e in self.views.values() {
            e.serialize_into(into)?;
        }
        Ok(())
    }
}

impl<T> NHContextDeserialize for OrderedViewRefs<T> where T: View + NHContextDeserialize + 'static {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError> {
        Ok(Self::new(<Vec<ERef<T>>>::deserialize(source, deserializer)?.into_iter().rev().collect()))
    }
}
