use std::sync::atomic::{AtomicU64, Ordering};
use std::{marker::PhantomData, collections::HashMap};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct TumblingWindow<K, V, S> {
    phantom: PhantomData<K>,
    phantom2: PhantomData<V>,
    store: HashMap<K, S>,
}

impl<K, V, S> TumblingWindow<K, V, S>
where
    S: Default + WindowState,
    K: PartialEq + Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
            phantom2: PhantomData,
            store: HashMap::new(),
        }
    }

    /// add new value to state
    pub fn add(&self, key: &K, value: &V) {
        if let Some(state) = self.store.get(&key) {
            state.add(value, state);
        } else {
            /*
            let mut state = S::default();
            state.add(key, value);
            self.store.insert(_key, state);
            */
        }
    }

    fn get_state(&self, key: &K) -> Option<&S> {
        self.store.get(key)
    }
}

pub trait WindowState {
    fn add<K, V>(&self, key: K, value: &V);
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AtomicF64 {
    storage: AtomicU64,
}

impl AtomicF64 {
    pub fn new(value: f64) -> Self {
        let as_u64 = value.to_bits();
        Self {
            storage: AtomicU64::new(as_u64),
        }
    }
    pub fn store(&self, value: f64, ordering: Ordering) {
        let as_u64 = value.to_bits();
        self.storage.store(as_u64, ordering)
    }
    pub fn load(&self, ordering: Ordering) -> f64 {
        let as_u64 = self.storage.load(ordering);
        f64::from_bits(as_u64)
    }
}
