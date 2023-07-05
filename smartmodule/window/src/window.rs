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
    S: Default + WindowState<K, V>,
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
            state.add(key, value);
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

pub trait WindowState<K, V> {
    fn add(&self, key: &K, value: &V);
}

mod util {
    use std::{
        sync::atomic::{AtomicU64, Ordering, AtomicU32},
        fmt,
        ops::{Deref, DerefMut},
    };

    use serde::{
        Serialize, Deserialize, Serializer, Deserializer,
        de::{Visitor, self},
    };

    #[derive(Debug, Default)]
    pub struct AtomicF64(AtomicU64);

    impl Deref for AtomicF64 {
        type Target = AtomicU64;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for AtomicF64 {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
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
            Self(AtomicU64::new(as_u64))
        }

        pub fn store(&self, value: f64) {
            let as_u64 = value.to_bits();
            self.0.store(as_u64, Ordering::SeqCst)
        }

        pub fn load(&self) -> f64 {
            let as_u64 = self.0.load(Ordering::SeqCst);
            f64::from_bits(as_u64)
        }

    }

    #[derive(Debug, Default,Serialize)]
    pub struct RollingMean {
        #[serde(skip)]
        count: AtomicU32,
        mean: AtomicF64
    }

    impl RollingMean {


        /// add to sample
        pub fn add(&self, value: f64) {
            let prev_mean = self.mean.load();
            let new_count = self.count.load(Ordering::SeqCst) + 1;
            let new_mean = prev_mean + (value - prev_mean) / (new_count as f64);
            self.mean.store(new_mean);
            self.count.store(new_count,Ordering::SeqCst);
        }

        pub fn mean(&self) -> f64 {
            self.mean.load()
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

        #[test]
        fn rolling_mean() {
            let rm: RollingMean = RollingMean::default();
            rm.add(3.2);
            assert_eq!(rm.mean(), 3.2);
            rm.add(4.2);
            assert_eq!(rm.mean(), 3.7);
        }
    }
}
