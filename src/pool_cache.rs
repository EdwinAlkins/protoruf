//! Global LRU cache for decoded [`DescriptorPool`] instances (free-function API).

use lru::LruCache;
use parking_lot::Mutex;
use prost_reflect::DescriptorPool;
use std::num::NonZeroUsize;
use std::sync::{Arc, LazyLock};

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
    // A borrowed lookup hashes the descriptor without allocating or copying it.
    if let Some(pool) = POOL_CACHE.lock().get(bytes) {
        return Ok(pool.clone());
    }

    // Decoding can be expensive. Allow other schemas to use the cache meanwhile.
    let pool = Arc::new(
        DescriptorPool::decode(bytes)
            .map_err(|e| format!("Failed to load descriptor pool: {}", e))?,
    );

    let mut cache = POOL_CACHE.lock();
    // Another caller may have inserted this schema while we decoded it.
    if let Some(existing) = cache.get(bytes) {
        return Ok(existing.clone());
    }
    cache.put(Arc::from(bytes), pool.clone());
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn concurrent_misses_share_one_cached_pool() {
        let files = HashMap::from([(
            "cache_race.proto".to_string(),
            "syntax = \"proto3\"; package cache_race; message Item { string id = 1; }".to_string(),
        )]);
        let bytes = Arc::new(
            crate::core::compile_proto_from_sources(files, "cache_race.proto", true).unwrap(),
        );
        let barrier = Arc::new(Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let bytes = bytes.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    load_descriptor_pool_cached(&bytes).unwrap()
                })
            })
            .collect();
        let pools: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert!(pools.iter().all(|pool| Arc::ptr_eq(pool, &pools[0])));
    }
}
