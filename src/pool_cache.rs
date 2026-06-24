//! Global LRU cache for decoded [`DescriptorPool`] instances (cold-path API).

use lru::LruCache;
use prost_reflect::DescriptorPool;
use std::num::NonZeroUsize;
use std::sync::{Arc, LazyLock, Mutex};

const MAX_POOL_CACHE_ENTRIES: usize = 64;

/// LRU cache keyed by the raw descriptor-set bytes, holding decoded pools.
type PoolCache = LruCache<Arc<[u8]>, Arc<DescriptorPool>>;

static POOL_CACHE: LazyLock<Mutex<PoolCache>> = LazyLock::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(MAX_POOL_CACHE_ENTRIES).unwrap(),
    ))
});

/// Decode a descriptor set, memoizing the resulting pool keyed by the raw bytes.
pub fn load_descriptor_pool_cached(bytes: &[u8]) -> Result<Arc<DescriptorPool>, String> {
    let key = Arc::from(bytes);
    let mut cache = POOL_CACHE.lock().expect("pool cache mutex poisoned");

    if let Some(pool) = cache.get(&key) {
        return Ok(pool.clone());
    }

    let pool = Arc::new(
        DescriptorPool::decode(bytes)
            .map_err(|e| format!("Failed to load descriptor pool: {}", e))?,
    );
    cache.put(key, pool.clone());
    Ok(pool)
}
