use std::{marker::PhantomData, collections::HashMap};
use std::hash::{Hash};

pub use util::AtomicF64;

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

mod util {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        fmt,
    };

    use serde::{
        Serialize, Deserialize, Serializer, Deserializer,
        de::{Visitor, self},
    };

    #[derive(Debug, Default)]
    pub struct AtomicF64 {
        storage: AtomicU64,
    }

    impl Serialize for AtomicF64 {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_f64(self.load())
        }
    }

    struct AtomicF64Visitor;

    impl<'de> Visitor<'de> for AtomicF64Visitor {
        type Value = AtomicF64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an float between -2^31 and 2^31")
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            use std::f64;
            if value >= f64::from(f64::MIN) && value <= f64::from(f64::MAX) {
                Ok(AtomicF64::new(value))
            } else {
                Err(E::custom(format!("f64 out of range: {}", value)))
            }
        }
    }

    impl<'de> Deserialize<'de> for AtomicF64 {
        fn deserialize<D>(deserializer: D) -> Result<AtomicF64, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_f64(AtomicF64Visitor)
        }
    }

    impl AtomicF64 {
        pub fn new(value: f64) -> Self {
            let as_u64 = value.to_bits();
            Self {
                storage: AtomicU64::new(as_u64),
            }
        }

        pub fn store(&self, value: f64) {
            let as_u64 = value.to_bits();
            self.storage.store(as_u64, Ordering::SeqCst)
        }
        pub fn load(&self) -> f64 {
            let as_u64 = self.storage.load(Ordering::SeqCst);
            f64::from_bits(as_u64)
        }
    }

    mod test {

        use super::*;

        #[derive(Serialize, Deserialize)]
        struct Sample {
            speed: AtomicF64,
        }

        #[test]
        fn test_f64_serialize() {
            let test = Sample {
                speed: AtomicF64::new(3.2),
            };
            let json = serde_json::to_string(&test).expect("serialize");
            assert_eq!(json, r#"{"speed":3.2}"#);
        }

        #[test]
        fn test_f64_de_serialize() {
            let input_str = r#"{"speed":9.13}"#;
            let test: Sample = serde_json::from_str(input_str).expect("serialize");
            assert_eq!(test.speed.load(), 9.13);
        }
    }
}
