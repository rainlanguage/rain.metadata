// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
//! The meta cache, and the one way into it.
//!
//! A hash is a claim about the bytes it keys. Caching bytes that do not hash to
//! their key stores a lie the rest of the crate reads back as truth, and
//! `cas.md` puts the check at exactly this point - "before the content is
//! stored under the hash" - so everything downstream can stop asking.
//!
//! Keeping that as a convention did not hold. The map was a bare `HashMap`
//! field on `Store`, so any method could reach past the check, and several did:
//! `update` shipped without it, `search_deployer`, `set_deployer` and
//! `set_deployer_from_query_response` each wrote to the cache directly. Each
//! was found separately, after the fact.
//!
//! So the map lives here with a private field and no unguarded insert. Every
//! write goes through [MetaCache::insert_verified] because the type system
//! offers nothing else, including from code written long after this.

use std::collections::HashMap;

use alloy::primitives::{hex, keccak256};

use crate::error::Error;

/// Meta bytes keyed by their own keccak256 hash.
///
/// The key is not a name for the bytes, it is a digest of them, and this type
/// exists to make that true by construction rather than by discipline.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetaCache {
    inner: HashMap<Vec<u8>, Vec<u8>>,
}

impl MetaCache {
    /// Caches `bytes` under `hash`, and only if they hash to it.
    ///
    /// A mismatch is [Error::CorruptRecord] rather than a miss: the responder
    /// answered a question about one hash with bytes that are another, which
    /// is not the same fact as the hash being absent.
    /// rainlanguage/rain.metadata#234 and #213 settled that distinction for the
    /// query layer; this is the same distinction at the cache.
    pub fn insert_verified(&mut self, hash: &[u8], bytes: Vec<u8>) -> Result<&Vec<u8>, Error> {
        if keccak256(&bytes).0 != hash {
            return Err(Error::CorruptRecord(format!(
                "bytes do not hash to the requested {}",
                hex::encode_prefixed(hash)
            )));
        }
        self.inner.insert(hash.to_vec(), bytes);
        self.inner.get(hash).ok_or(Error::NoRecordFound)
    }

    /// The bytes cached under `hash`, if any.
    pub fn get(&self, hash: &[u8]) -> Option<&Vec<u8>> {
        self.inner.get(hash)
    }

    /// Whether anything is cached under `hash`.
    pub fn contains_key(&self, hash: &[u8]) -> bool {
        self.inner.contains_key(hash)
    }

    /// Drops whatever is cached under `hash`. Removing cannot break the
    /// invariant, so it needs no check.
    pub fn remove(&mut self, hash: &[u8]) {
        self.inner.remove(hash);
    }

    /// Every cached pair. Entries are verified by construction, so copying one
    /// into another [MetaCache] cannot introduce an unverified entry.
    pub fn iter(&self) -> impl Iterator<Item = (&Vec<u8>, &Vec<u8>)> {
        self.inner.iter()
    }

    /// Whether anything is cached at all.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// How many metas are cached.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashed(bytes: &[u8]) -> Vec<u8> {
        keccak256(bytes).0.to_vec()
    }

    /// Bytes that hash to their key are cached and readable back.
    #[test]
    fn test_insert_verified_accepts_matching_bytes() {
        let bytes = b"content".to_vec();
        let hash = hashed(&bytes);
        let mut cache = MetaCache::default();

        assert_eq!(cache.insert_verified(&hash, bytes.clone()).unwrap(), &bytes);
        assert_eq!(cache.get(&hash), Some(&bytes));
        assert!(cache.contains_key(&hash));
        assert_eq!(cache.len(), 1);
    }

    /// Bytes that do not hash to their key are refused, and refused as corrupt
    /// rather than as a miss, with the requested hash named.
    #[test]
    fn test_insert_verified_rejects_mismatched_bytes_as_corrupt() {
        let wrong_hash = vec![0x99u8; 32];
        let mut cache = MetaCache::default();

        match cache
            .insert_verified(&wrong_hash, b"content".to_vec())
            .unwrap_err()
        {
            Error::CorruptRecord(message) => assert!(
                message.contains(&hex::encode_prefixed(&wrong_hash)),
                "{}",
                message
            ),
            other => panic!("expected CorruptRecord, got {:?}", other),
        }

        // and nothing was cached on the way out
        assert!(cache.is_empty());
        assert!(!cache.contains_key(&wrong_hash));
    }
}
