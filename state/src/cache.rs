use std::hash::Hash;

type Mokabase<K, V> = mini_moka::sync::Cache<K, V>;

pub trait CacheValue<K>: Clone + Send + Sync {
    fn cache_weight(&self) -> u32;
}

#[derive(Debug, Clone)]
pub struct Cache<K: Eq + Hash, V> {
    name: &'static str,
    cache: Option<Mokabase<K, V>>,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: CacheValue<K> + 'static,
{
    pub fn new(name: &'static str, capacity: u64) -> Self {
        let cache = if capacity == 0 {
            None
        } else {
            Some(
                Mokabase::<K, V>::builder()
                    .weigher(|_, value| value.cache_weight())
                    .max_capacity(capacity)
                    .build(),
            )
        };
        Self { name, cache }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.cache.as_ref()?.get(key)
    }

    pub fn insert(&self, key: K, value: V) {
        let Some(cache) = self.cache.as_ref() else {
            return;
        };
        cache.insert(key, value);
    }

    pub fn remove(&self, key: &K) {
        if let Some(cache) = &self.cache {
            cache.invalidate(key);
        }
    }

    pub fn clear(&self) {
        if let Some(cache) = &self.cache {
            cache.invalidate_all();
        }
    }
}
