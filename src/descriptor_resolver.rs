//! Shared descriptor pool + per-type [`MessageDescriptor`] memoization for all bindings.

use crate::core;
use crate::pool_cache;
use parking_lot::RwLock;
use prost_reflect::{DescriptorPool, MessageDescriptor};
use std::collections::HashMap;
use std::sync::Arc;

/// A decoded descriptor pool with memoized message-type lookups.
pub struct DescriptorResolver {
    pool: Arc<DescriptorPool>,
    descriptors: RwLock<HashMap<String, MessageDescriptor>>,
}

impl DescriptorResolver {
    /// Decode (or reuse from the global pool cache) and wrap for repeated conversions.
    pub fn from_descriptor_bytes(bytes: &[u8]) -> Result<Self, String> {
        let pool = pool_cache::load_descriptor_pool_cached(bytes)?;
        Ok(Self {
            pool,
            descriptors: RwLock::new(HashMap::new()),
        })
    }

    /// Resolve a message descriptor by fully-qualified name, caching the lookup.
    pub fn resolve(&self, message_type: &str) -> Result<MessageDescriptor, String> {
        if let Some(desc) = self.descriptors.read().get(message_type) {
            return Ok(desc.clone());
        }

        let mut cache = self.descriptors.write();
        if let Some(desc) = cache.get(message_type) {
            return Ok(desc.clone());
        }

        let desc = core::get_message_descriptor(&self.pool, message_type)?;
        cache.insert(message_type.to_string(), desc.clone());
        Ok(desc)
    }
}
