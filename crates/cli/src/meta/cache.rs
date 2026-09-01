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

use std::collections::BTreeMap;

use alloy::primitives::{hex, keccak256};
use serde::{Deserialize, Deserializer};

use crate::error::Error;
use crate::meta::NPE2Deployer;

/// Meta bytes keyed by their own keccak256 hash.
///
/// The key is not a name for the bytes, it is a digest of them, and this type
/// exists to make that true by construction rather than by discipline.
/// The map is a [BTreeMap] so serializing twice gives the same bytes.
/// [std::collections::HashMap] iterates in an order randomized per process,
/// which would make a serialized cache unreproducible for no gain - every
/// access here is by key.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct MetaCache {
    inner: BTreeMap<Vec<u8>, Vec<u8>>,
}

/// Deserializing is a way into the cache, so it goes through the same gate.
///
/// A derived impl would build `inner` directly, which is how the invariant
/// leaked the first time this type was written: entries refused by
/// [MetaCache::insert_verified] were accepted wholesale off the wire. A cache
/// is only as good as the worst entry in it, so one bad pair rejects the whole
/// map rather than being dropped quietly - unlike a responder's single answer,
/// a serialized cache is something this process wrote and should not be able
/// to get wrong.
impl<'de> Deserialize<'de> for MetaCache {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            inner: BTreeMap<Vec<u8>, Vec<u8>>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let mut cache = MetaCache::default();
        for (hash, bytes) in wire.inner {
            cache
                .insert_verified(&hash, bytes)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(cache)
    }
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

    /// Deserializing is a way in, so it is gated too. A derived impl would
    /// build the map directly and accept off the wire exactly what
    /// insert_verified refuses in process.
    #[test]
    fn test_deserialize_rejects_an_unverified_entry() {
        #[derive(serde::Serialize)]
        struct Wire {
            inner: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
        }
        let planted = Wire {
            inner: std::collections::BTreeMap::from([(
                vec![0x99u8; 32],
                b"not the preimage".to_vec(),
            )]),
        };

        let wire = serde_cbor::to_vec(&planted).unwrap();
        let round: Result<MetaCache, _> = serde_cbor::from_slice(&wire);
        assert!(round.is_err(), "an unverified entry round tripped in");
    }

    /// A verified entry survives the round trip, so the gate rejects lies
    /// rather than everything.
    #[test]
    fn test_deserialize_keeps_a_verified_entry() {
        let bytes = b"content".to_vec();
        let hash = hashed(&bytes);
        let mut cache = MetaCache::default();
        cache.insert_verified(&hash, bytes.clone()).unwrap();

        let wire = serde_cbor::to_vec(&cache).unwrap();
        let round: MetaCache = serde_cbor::from_slice(&wire).unwrap();
        assert_eq!(round.get(&hash), Some(&bytes));
    }

    /// Serializing the same cache twice gives the same bytes, which a
    /// HashMap would not guarantee across processes.
    #[test]
    fn test_serialization_is_deterministic() {
        let mut cache = MetaCache::default();
        for content in [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()] {
            let hash = hashed(&content);
            cache.insert_verified(&hash, content).unwrap();
        }
        let a = serde_cbor::to_vec(&cache).unwrap();
        let b = serde_cbor::to_vec(&cache.clone()).unwrap();
        assert_eq!(a, b);
    }
}

/// Deployer records keyed by their bytecode meta hash.
///
/// The key here is not a digest of the value - a deployer is keyed by its
/// bytecode meta hash while carrying a constructor meta of its own - so the
/// invariant is internal: `meta_bytes` must hash to `meta_hash`. A record that
/// gets that wrong describes a deployer whose constructor meta is not the meta
/// it names, and [crate::meta::Store] copies exactly those bytes into the
/// [MetaCache] under exactly that hash.
///
/// Same shape as [MetaCache], for the same reason: the check was a convention
/// spread across call sites, and `set_deployer` and
/// `set_deployer_from_query_response` both missed it.
/// rainlanguage/rain.metadata#170.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct DeployerCache {
    inner: BTreeMap<Vec<u8>, NPE2Deployer>,
}

impl DeployerCache {
    /// Caches `deployer` under `key`, and only if its own meta bytes hash to
    /// the meta hash it claims for them.
    pub fn insert_verified(
        &mut self,
        key: &[u8],
        deployer: NPE2Deployer,
    ) -> Result<&NPE2Deployer, Error> {
        if keccak256(&deployer.meta_bytes).0.as_slice() != deployer.meta_hash.as_slice() {
            return Err(Error::CorruptRecord(format!(
                "deployer meta bytes do not hash to its own meta hash {}",
                hex::encode_prefixed(&deployer.meta_hash)
            )));
        }
        self.inner.insert(key.to_vec(), deployer);
        self.inner.get(key).ok_or(Error::NoRecordFound)
    }

    /// The deployer cached under `key`, if any.
    pub fn get(&self, key: &[u8]) -> Option<&NPE2Deployer> {
        self.inner.get(key)
    }

    /// Whether anything is cached under `key`.
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.inner.contains_key(key)
    }

    /// Every cached pair. Entries are verified by construction.
    pub fn iter(&self) -> impl Iterator<Item = (&Vec<u8>, &NPE2Deployer)> {
        self.inner.iter()
    }

    /// Whether anything is cached at all.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Deserializing is a way in, so it goes through the gate, as for [MetaCache].
impl<'de> Deserialize<'de> for DeployerCache {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            inner: BTreeMap<Vec<u8>, NPE2Deployer>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let mut cache = DeployerCache::default();
        for (key, deployer) in wire.inner {
            cache
                .insert_verified(&key, deployer)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(cache)
    }
}
