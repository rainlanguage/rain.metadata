use super::error::Error;

pub mod cache;
use cache::MetaCache;
use alloy::primitives::{hex, keccak256};
use futures::future;
use graphql_client::GraphQLQuery;
use rain_metadata_bindings::IDescribedByMetaV1;
use reqwest::Client;
use serde::de::{Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::{
    collections::{BTreeSet, HashMap},
    convert::TryFrom,
    fmt::Debug,
    sync::Arc,
};
use strum::{EnumIter, EnumString};
use alloy::sol_types::private::Address;
use alloy::providers::Provider;
use alloy::contract::Error as ContractError;
use rain_erc::erc165::{Erc165Error, IERC165, XorSelectors, supports_erc165};

pub mod magic;
pub(crate) mod normalize;
pub(crate) mod query;
pub mod types;

pub use magic::*;
pub use query::*;

/// All known meta identifiers
#[derive(Copy, Clone, EnumString, EnumIter, strum::Display, Debug, PartialEq)]
#[strum(serialize_all = "kebab-case")]
pub enum KnownMeta {
    /// Ops meta v1. Still a known meta - the magic number is in the
    /// metadata-v1 table and an item can legitimately carry it - but this
    /// crate no longer models or validates the payload. The interpreter
    /// describes its words as AuthoringMetaV2 now (`LibAllStandardOps`
    /// publishes exactly that), and the only surviving op meta references in
    /// the org are deprecated IExpressionDeployer interfaces.
    OpV1,
    DotrainV1,
    RainlangV1,
    SolidityAbiV2,
    AuthoringMetaV1,
    AuthoringMetaV2,
    InterpreterCallerMetaV1,
    ExpressionDeployerV2BytecodeV1,
    RainlangSourceV1,
    AddressList,
    DotrainSourceV1,
    OrderBuilderStateV1,
    RaindexSignedContextOracleV1,
}

impl TryFrom<KnownMagic> for KnownMeta {
    type Error = Error;
    fn try_from(value: KnownMagic) -> Result<Self, Self::Error> {
        match value {
            KnownMagic::DotrainV1 => Ok(KnownMeta::DotrainV1),
            KnownMagic::RainlangV1 => Ok(KnownMeta::RainlangV1),
            KnownMagic::SolidityAbiV2 => Ok(KnownMeta::SolidityAbiV2),
            KnownMagic::OpMetaV1 => Ok(KnownMeta::OpV1),
            KnownMagic::AuthoringMetaV1 => Ok(KnownMeta::AuthoringMetaV1),
            KnownMagic::AuthoringMetaV2 => Ok(KnownMeta::AuthoringMetaV2),
            KnownMagic::AddressList => Ok(KnownMeta::AddressList),
            KnownMagic::InterpreterCallerMetaV1 => Ok(KnownMeta::InterpreterCallerMetaV1),
            KnownMagic::DotrainSourceV1 => Ok(KnownMeta::DotrainSourceV1),
            KnownMagic::OrderBuilderStateV1 => Ok(KnownMeta::OrderBuilderStateV1),
            KnownMagic::ExpressionDeployerV2BytecodeV1 => {
                Ok(KnownMeta::ExpressionDeployerV2BytecodeV1)
            }
            KnownMagic::RainlangSourceV1 => Ok(KnownMeta::RainlangSourceV1),
            KnownMagic::RaindexSignedContextOracleV1 => Ok(KnownMeta::RaindexSignedContextOracleV1),
            _ => Err(Error::UnsupportedMeta),
        }
    }
}

/// Content type of a cbor meta map
#[derive(
    Copy,
    Clone,
    Debug,
    EnumIter,
    PartialEq,
    EnumString,
    strum::Display,
    serde::Serialize,
    serde::Deserialize,
)]
#[strum(serialize_all = "kebab-case")]
pub enum ContentType {
    None,
    #[serde(rename = "application/json")]
    Json,
    #[serde(rename = "application/cbor")]
    Cbor,
    #[serde(rename = "application/octet-stream")]
    OctetStream,
}

/// Content encoding of a cbor meta map
#[derive(
    Copy,
    Clone,
    Debug,
    EnumIter,
    PartialEq,
    EnumString,
    strum::Display,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ContentEncoding {
    None,
    Identity,
    Deflate,
}

impl ContentEncoding {
    /// encode the data based on the variant
    pub fn encode(&self, data: &[u8]) -> Vec<u8> {
        match self {
            ContentEncoding::None | ContentEncoding::Identity => data.to_vec(),
            ContentEncoding::Deflate => deflate::deflate_bytes_zlib(data),
        }
    }

    /// decode the data based on the variant
    pub fn decode(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(match self {
            ContentEncoding::None | ContentEncoding::Identity => data.to_vec(),
            ContentEncoding::Deflate => match inflate::inflate_bytes_zlib(data) {
                Ok(v) => v,
                Err(error) => match inflate::inflate_bytes(data) {
                    Ok(v) => v,
                    Err(_) => Err(Error::InflateError(error))?,
                },
            },
        })
    }
}

/// Content language of a cbor meta map
#[derive(
    Copy,
    Clone,
    Debug,
    EnumIter,
    PartialEq,
    EnumString,
    strum::Display,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ContentLanguage {
    None,
    En,
}

/// # Rain Meta Document v1 Item (meta map)
///
/// represents a rain meta data and configuration that can be cbor encoded or unpacked back to the meta types
#[derive(PartialEq, Debug, Clone)]
pub struct RainMetaDocumentV1Item {
    pub payload: serde_bytes::ByteBuf,
    pub magic: KnownMagic,
    pub content_type: ContentType,
    pub content_encoding: ContentEncoding,
    pub content_language: ContentLanguage,
    /// optional reference to the schema of the payload, encoded under the
    /// [KnownMagic::OaSchema] magic number as an additional cbor map key
    /// beyond the standard 0-4 keys
    pub schema: Option<String>,
}

// this implementation is mainly used by Rainlang and Dotrain metas as they are aliased type for String
impl TryFrom<RainMetaDocumentV1Item> for String {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        Ok(String::from_utf8(value.unpack()?)?)
    }
}

// this implementation is mainly used by ExpressionDeployerV2Bytecode meta as it is aliased type for Vec<u8>
impl TryFrom<RainMetaDocumentV1Item> for Vec<u8> {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        value.unpack()
    }
}

impl RainMetaDocumentV1Item {
    fn len(&self) -> usize {
        let mut l = 2;
        if !matches!(self.content_type, ContentType::None) {
            l += 1;
        }
        if !matches!(self.content_encoding, ContentEncoding::None) {
            l += 1;
        }
        if !matches!(self.content_language, ContentLanguage::None) {
            l += 1;
        }
        if self.schema.is_some() {
            l += 1;
        }
        l
    }

    /// method to hash(keccak256) the cbor encoded bytes of this instance
    pub fn hash(&self, as_rain_meta_document: bool) -> Result<[u8; 32], Error> {
        if as_rain_meta_document {
            Ok(keccak256(Self::cbor_encode_seq(
                &vec![self.clone()],
                KnownMagic::RainMetaDocumentV1,
            )?)
            .0)
        } else {
            Ok(keccak256(self.cbor_encode()?).0)
        }
    }

    /// method to cbor encode
    pub fn cbor_encode(&self) -> Result<Vec<u8>, Error> {
        let mut bytes: Vec<u8> = vec![];
        Ok(serde_cbor::to_writer(&mut bytes, &self).map(|_| bytes)?)
    }

    /// builds a cbor sequence from given MetaMaps
    pub fn cbor_encode_seq(
        seq: &Vec<RainMetaDocumentV1Item>,
        magic: KnownMagic,
    ) -> Result<Vec<u8>, Error> {
        let mut bytes: Vec<u8> = magic.to_prefix_bytes().to_vec();
        for item in seq {
            serde_cbor::to_writer(&mut bytes, &item)?;
        }
        Ok(bytes)
    }

    /// method to cbor decode from given bytes
    pub fn cbor_decode(data: &[u8]) -> Result<Vec<RainMetaDocumentV1Item>, Error> {
        let mut metas: Vec<RainMetaDocumentV1Item> = vec![];
        let mut consumed: usize = 0;
        let mut is_rain_document_meta = false;
        let mut len = data.len();
        if data.starts_with(&KnownMagic::RainMetaDocumentV1.to_prefix_bytes()) {
            is_rain_document_meta = true;
            len -= 8;
        }
        let mut deserializer = match is_rain_document_meta {
            true => serde_cbor::Deserializer::from_slice(&data[8..]),
            false => serde_cbor::Deserializer::from_slice(data),
        };
        // straight off the stream, not via serde_cbor::Value, whose BTreeMap
        // silently collapses the duplicate keys the visitor has to reject.
        //
        // ItemOrDropped rather than Self: this is the sequence decoder, so an
        // item this version does not read is skipped over instead of taking
        // the items beside it down. Its bytes are still consumed and still
        // counted, so the trailing len check below stays a statement about
        // the whole document.
        while match ItemOrDropped::deserialize(&mut deserializer) {
            Ok(decoded) => {
                consumed = deserializer.byte_offset();
                if let ItemOrDropped::Item(meta) = decoded {
                    metas.push(meta);
                }
                true
            }
            Err(error) => {
                if error.is_eof() {
                    false
                } else {
                    Err(Error::SerdeCborError(error))?
                }
            }
        } {}

        if metas.is_empty() || len != consumed {
            Err(Error::CorruptMeta)?
        }
        Ok(metas)
    }

    // unpack the payload based on the configuration
    pub fn unpack(&self) -> Result<Vec<u8>, Error> {
        ContentEncoding::decode(&self.content_encoding, self.payload.as_ref())
    }

    // unpacks the payload to given meta type based on configuration
    pub fn unpack_into<T: TryFrom<Self, Error = Error>>(self) -> Result<T, Error> {
        match self.magic {
            KnownMagic::OpMetaV1
            | KnownMagic::DotrainV1
            | KnownMagic::RainlangV1
            | KnownMagic::SolidityAbiV2
            | KnownMagic::AuthoringMetaV1
            | KnownMagic::AuthoringMetaV2
            | KnownMagic::AddressList
            | KnownMagic::InterpreterCallerMetaV1
            | KnownMagic::ExpressionDeployerV2BytecodeV1
            | KnownMagic::DotrainSourceV1
            | KnownMagic::OrderBuilderStateV1
            | KnownMagic::RainlangSourceV1
            | KnownMagic::RaindexSignedContextOracleV1 => T::try_from(self),
            _ => Err(Error::UnsupportedMeta)?,
        }
    }
}

impl Serialize for RainMetaDocumentV1Item {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        map.serialize_entry(&0, &self.payload)?;
        map.serialize_entry(&1, &(self.magic as u64))?;
        match self.content_type {
            ContentType::None => {}
            content_type => map.serialize_entry(&2, &content_type)?,
        }
        match self.content_encoding {
            ContentEncoding::None => {}
            content_encoding => map.serialize_entry(&3, &content_encoding)?,
        }
        match self.content_language {
            ContentLanguage::None => {}
            content_language => map.serialize_entry(&4, &content_language)?,
        }
        if let Some(schema) = &self.schema {
            map.serialize_entry(&(KnownMagic::OaSchema as u64), schema)?;
        }
        map.end()
    }
}

/// What a cbor item in a rain meta sequence turned out to be.
///
/// The spec draws a line the decoder has to draw too. Some malformed input is
/// the *document's* problem and stops the decode; other input is a well formed
/// cbor item that this version simply does not read, and the spec is explicit
/// that those are dropped rather than fatal:
///
/// - "any CBOR item that omits these keys MUST be treated as unexpected (cbor
///   terminology) and dropped/ignored", of the mandatory indexes 0 and 1;
/// - "Tooling can efficiently O(1) drop/ignore meta that it does not need or
///   support decoding and parsing for", which the per-item magic exists to
///   make possible, alongside "feel free to build systems and applications
///   with your own numbers and interpretations".
///
/// Dropping is therefore how the format stays extensible, and refusing a whole
/// document over one item nobody claimed this tool would understand is what
/// breaks that. rainlanguage/rain.metadata#188 and #186.
enum ItemOrDropped {
    Item(RainMetaDocumentV1Item),
    /// Well formed cbor, but not an item this version reads.
    Dropped,
}

impl<'de> Deserialize<'de> for ItemOrDropped {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// RFC 8949 §5.6: a map with duplicate keys is not valid, so a second
        /// sighting of a key is an error rather than an overwrite.
        fn set_once<V, E: serde::de::Error>(
            slot: &mut Option<V>,
            value: V,
            field: &'static str,
        ) -> Result<(), E> {
            if slot.is_some() {
                return Err(serde::de::Error::duplicate_field(field));
            }
            *slot = Some(value);
            Ok(())
        }

        struct EncodedMap;
        impl<'de> Visitor<'de> for EncodedMap {
            type Value = ItemOrDropped;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("rain meta cbor encoded bytes")
            }

            fn visit_map<T: serde::de::MapAccess<'de>>(
                self,
                mut map: T,
            ) -> Result<Self::Value, T::Error> {
                const OA_SCHEMA_KEY: u64 = KnownMagic::OaSchema as u64;
                let mut payload = None;
                let mut magic: Option<u64> = None;
                let mut content_type = None;
                let mut content_encoding = None;
                let mut content_language = None;
                let mut schema = None;
                // the recognised keys guard themselves through set_once; an
                // unknown key has no slot to be occupied, so its repeats are
                // tracked here
                let mut unknown_keys: BTreeSet<u64> = BTreeSet::new();
                while match map.next_key::<u64>() {
                    Ok(Some(key)) => {
                        match key {
                            0 => set_once(&mut payload, map.next_value()?, "payload")?,
                            1 => set_once(&mut magic, map.next_value()?, "magic number")?,
                            2 => set_once(&mut content_type, map.next_value()?, "content type")?,
                            3 => set_once(
                                &mut content_encoding,
                                map.next_value()?,
                                "content encoding",
                            )?,
                            4 => set_once(
                                &mut content_language,
                                map.next_value()?,
                                "content language",
                            )?,
                            OA_SCHEMA_KEY => set_once(&mut schema, map.next_value()?, "schema")?,
                            // the map structure exists so later conventions can
                            // add indexes that older tooling adopts "or not" in
                            // a backwards compatible way, so an index this
                            // version does not know is skipped, not an error.
                            // §5.6 does not ask whether the decoder understands
                            // a key, so the repeat of one is still invalid even
                            // though the value behind it is never read.
                            _ => {
                                if !unknown_keys.insert(key) {
                                    return Err(serde::de::Error::custom(format!(
                                        "duplicate map key: {key}"
                                    )));
                                }
                                map.next_value::<serde::de::IgnoredAny>()?;
                            }
                        };
                        true
                    }
                    Ok(None) => false,
                    Err(error) => Err(error)?,
                } {}
                // indexes 0 and 1 are the mandatory ones, and an item without
                // them is the spec's "unexpected"
                let (Some(payload), Some(magic_number)) = (payload, magic) else {
                    return Ok(ItemOrDropped::Dropped);
                };

                // A nested document prefix is not somebody else's magic
                // number, it is this crate's own in a position it cannot
                // occupy: the document prefix is the first 8 bytes of the
                // whole document, never an item's cbor key 1. That is
                // malformed structure rather than a type this version does
                // not read, so it stops the document instead of being
                // dropped with the unknown numbers below.
                // rainlanguage/rain.metadata#204.
                if magic_number == KnownMagic::RainMetaDocumentV1 as u64 {
                    return Err(serde::de::Error::custom(
                        "rain meta document magic number as an item magic number",
                    ));
                }

                let Ok(magic) = KnownMagic::try_from(magic_number) else {
                    return Ok(ItemOrDropped::Dropped);
                };

                let content_type = content_type.unwrap_or(ContentType::None);
                let content_encoding = content_encoding.unwrap_or(ContentEncoding::None);
                let content_language = content_language.unwrap_or(ContentLanguage::None);

                Ok(ItemOrDropped::Item(RainMetaDocumentV1Item {
                    payload,
                    magic,
                    content_type,
                    content_encoding,
                    content_language,
                    schema,
                }))
            }
        }
        deserializer.deserialize_map(EncodedMap)
    }
}

/// Decoding one item on its own is strict.
///
/// The drop rule is about a *sequence*: an item nobody claimed this tool would
/// read must not take the items beside it down with it. Asking for a single
/// item names the thing you want, so not getting it is an error rather than a
/// silent absence, and there is nothing beside it to protect.
impl<'de> Deserialize<'de> for RainMetaDocumentV1Item {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ItemOrDropped::deserialize(deserializer)? {
            ItemOrDropped::Item(item) => Ok(item),
            ItemOrDropped::Dropped => Err(serde::de::Error::custom(
                "not a rain meta item this version reads: a mandatory key is missing or the magic number is unknown",
            )),
        }
    }
}

/// searches for a meta matching the given hash in given subgraphs urls
pub async fn search(hash: &str, subgraphs: &Vec<String>) -> Result<query::MetaResponse, Error> {
    // future::select_ok panics on an empty iterator.
    if subgraphs.is_empty() {
        return Err(Error::NoRecordFound);
    }
    let request_body = query::MetaQuery::build_query(query::meta_query::Variables {
        hash: Some(hash.to_ascii_lowercase()),
    });
    let mut promises = vec![];

    let client = Arc::new(Client::builder().build().map_err(Error::ReqwestError)?);
    for url in subgraphs {
        promises.push(Box::pin(query::process_meta_query(
            client.clone(),
            &request_body,
            url,
        )));
    }
    let response_value = future::select_ok(promises.drain(..)).await?.0;
    Ok(response_value)
}

/// checks if the given contract implements IDescribeByMetaV1 interface
///
/// `Err` is rain-erc's "answer unknown": a transport or decode failure stopped
/// the probe, never a contract that lacks the interface. Both probes here can
/// raise it - the erc165 one this delegates to rain-erc, and the interface id
/// one below - and neither may be flattened into `Ok(false)`, which is the
/// contract answering "no".
pub async fn implements_i_described_by_meta_v1<P: Provider>(
    provider: &P,
    contract_address: Address,
) -> Result<bool, Erc165Error> {
    if !supports_erc165(provider, contract_address).await? {
        return Ok(false);
    }

    let interface_id_res = IDescribedByMetaV1::IDescribedByMetaV1Calls::xor_selectors();
    if interface_id_res.is_err() {
        return Ok(false);
    }

    match IERC165::new(contract_address, provider)
        .supportsInterface(interface_id_res.unwrap().into())
        .call()
        .await
    {
        Ok(supported) => Ok(supported),
        Err(error)
            if error.as_revert_data().is_some()
                || matches!(error, ContractError::ZeroData(_, _)) =>
        {
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

/// # Meta Storage(CAS)
///
/// In-memory CAS (content addressed storage) for Rain metadata which basically stores
/// k/v pairs of meta hash and meta bytes, as well as providing functionalities to
/// easliy read/write to the CAS.
///
/// Hashes are normal bytes and meta bytes are valid cbor encoded as data bytes.
///
/// ## Examples
///
/// ```
/// use rain_metadata::Store;
/// use rain_metadata::meta::cache::MetaCache;
/// use std::collections::HashMap;
///
/// // to instantiate with an empty subgraph list
/// let mut store = Store::new();
///
/// // or to instantiate with initial values
/// let mut store = Store::create(
///     &vec!["sg-url-1".to_string()],
///     &MetaCache::default(),
///     &HashMap::new(),
/// );
///
/// // add a new subgraph endpoint url to the subgraph list
/// store.add_subgraphs(&vec!["sg-url-2".to_string()]);
///
/// // merge another Store into this one
/// store.merge(&Store::new());
///
/// // updates the meta store with some bytes and the hash they hash to - a
/// // pair that does not is refused, so the hash is derived rather than picked
/// let bytes = vec![0u8, 1u8];
/// let hash = alloy::primitives::keccak256(&bytes).0.to_vec();
/// store.update_with(&hash, &bytes).unwrap();
///
/// // `Store::update(&hash)` is async; it searches each subgraph for `hash` and
/// // populates the cache with the result. Call it from an async context with `.await`.
///
/// // to get a record from the store
/// let _meta = store.get_meta(&hash);
///
/// // Store is agnostic to dotrain contents — it just maps the hash of the content
/// // to the given uri and puts it as a new meta into the meta cache.
/// let dotrain_uri = "path/to/file.rain";
/// let dotrain_content = "/* some dotrain source */";
/// let (_new_hash, _old_hash) = store
///     .set_dotrain(dotrain_content, dotrain_uri, false)
///     .unwrap();
///
/// // to get dotrain meta bytes given a uri
/// let _dotrain_meta_bytes = store.get_dotrain_meta(dotrain_uri);
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Store {
    subgraphs: Vec<String>,
    cache: MetaCache,
    dotrain_cache: HashMap<String, Vec<u8>>,
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl Store {
    /// lazily creates a new instance with no subgraphs
    /// it is recommended to use create() instead with initial values
    pub fn new() -> Store {
        Store {
            subgraphs: vec![],
            cache: MetaCache::default(),
            dotrain_cache: HashMap::new(),
        }
    }

    /// creates new instance of Store with given initial values
    /// it checks the validity of each item of the provided values and only stores those that are valid
    pub fn create(
        subgraphs: &Vec<String>,
        cache: &MetaCache,
        dotrain_cache: &HashMap<String, Vec<u8>>,
    ) -> Store {
        let mut store = Store::new();
        store.add_subgraphs(subgraphs);
        for (hash, bytes) in cache.iter() {
            let _ = store.update_with(hash, bytes);
        }
        for (uri, hash) in dotrain_cache {
            if !store.dotrain_cache.contains_key(uri) && store.cache.contains_key(hash) {
                store.dotrain_cache.insert(uri.clone(), hash.clone());
            }
        }
        store
    }

    /// all subgraph endpoints in this instance
    pub fn subgraphs(&self) -> &Vec<String> {
        &self.subgraphs
    }

    /// add new subgraph endpoints
    pub fn add_subgraphs(&mut self, subgraphs: &Vec<String>) {
        for sg in subgraphs {
            if !self.subgraphs.contains(sg) {
                self.subgraphs.push(sg.to_string());
            }
        }
    }

    /// getter method for the whole meta cache
    pub fn cache(&self) -> &MetaCache {
        &self.cache
    }

    /// get the corresponding meta bytes of the given hash if it exists
    pub fn get_meta(&self, hash: &[u8]) -> Option<&Vec<u8>> {
        self.cache.get(hash)
    }

    /// getter method for the whole dotrain cache
    pub fn dotrain_cache(&self) -> &HashMap<String, Vec<u8>> {
        &self.dotrain_cache
    }

    /// get the corresponding dotrain hash of the given dotrain uri if it exists
    pub fn get_dotrain_hash(&self, uri: &str) -> Option<&Vec<u8>> {
        self.dotrain_cache.get(uri)
    }

    /// get the corresponding uri of the given dotrain hash if it exists
    pub fn get_dotrain_uri(&self, hash: &[u8]) -> Option<&String> {
        for (uri, h) in &self.dotrain_cache {
            if h == hash {
                return Some(uri);
            }
        }
        None
    }

    /// get the corresponding meta bytes of the given dotrain uri if it exists
    pub fn get_dotrain_meta(&self, uri: &str) -> Option<&Vec<u8>> {
        self.get_meta(self.dotrain_cache.get(uri)?)
    }

    /// deletes a dotrain record given a uri
    pub fn delete_dotrain(&mut self, uri: &str, keep_meta: bool) {
        if let Some(kv) = self.dotrain_cache.remove_entry(uri) {
            if !keep_meta {
                self.cache.remove(&kv.1);
            }
        };
    }

    /// lazilly merges another Store to the current one, avoids duplicates
    /// every map keeps the entry this Store already has on a key collision
    pub fn merge(&mut self, other: &Store) {
        self.add_subgraphs(&other.subgraphs);
        for (hash, bytes) in other.cache.iter() {
            if !self.cache.contains_key(hash) {
                // entries are verified by construction, so copying one cannot
                // introduce an unverified entry
                let _ = self.cache.insert_verified(hash, bytes.clone());
            }
        }
        for (uri, hash) in &other.dotrain_cache {
            if !self.dotrain_cache.contains_key(uri) {
                self.dotrain_cache.insert(uri.clone(), hash.clone());
            }
        }
    }

    /// Caches `bytes` under `hash` via [MetaCache::insert_verified], then
    /// unpacks the items they carry into the cache too. The gate itself lives
    /// on [MetaCache], which has no other way in.
    fn insert_verified(&mut self, hash: &[u8], bytes: Vec<u8>) -> Result<&Vec<u8>, Error> {
        self.cache.insert_verified(hash, bytes.clone())?;
        self.store_content(&bytes);
        self.get_meta(hash).ok_or(Error::NoRecordFound)
    }

    /// updates the meta cache by searching through all subgraphs for the given
    /// hash, and returns the reference to the meta bytes in the cache if it was
    /// found. Refreshes unconditionally; [Self::update_check] is the variant
    /// that leaves an already cached hash alone.
    pub async fn update(&mut self, hash: &[u8]) -> Result<&Vec<u8>, Error> {
        let meta = search(&hex::encode_prefixed(hash), &self.subgraphs).await?;
        self.insert_verified(hash, meta.bytes)
    }

    /// first checks if the meta is stored, if not will perform update()
    pub async fn update_check(&mut self, hash: &[u8]) -> Result<&Vec<u8>, Error> {
        // The NoRecordFound arm is unreachable, contains_key having just
        // proved the key is present. It is spelled this way rather than as
        // `if let Some(cached) = self.get_meta(hash)` because that holds an
        // immutable borrow of self across the mutable call below it, which
        // the borrow checker refuses.
        if self.cache.contains_key(hash) {
            return self.get_meta(hash).ok_or(Error::NoRecordFound);
        }
        self.update(hash).await
    }

    /// updates the meta cache with the given hash and meta bytes, and returns
    /// the reference to the bytes if they were accepted. Leaves an already
    /// cached hash alone, as [Self::update_check] does for the subgraph path.
    pub fn update_with(&mut self, hash: &[u8], bytes: &[u8]) -> Result<&Vec<u8>, Error> {
        // The NoRecordFound arm is unreachable, contains_key having just
        // proved the key is present. It is spelled this way rather than as
        // `if let Some(cached) = self.get_meta(hash)` because that holds an
        // immutable borrow of self across the mutable call below it, which
        // the borrow checker refuses.
        if self.cache.contains_key(hash) {
            return self.get_meta(hash).ok_or(Error::NoRecordFound);
        }
        self.insert_verified(hash, bytes.to_vec())
    }

    /// stores (or updates in case the URI already exists) the given dotrain text as meta into the store cache
    /// and maps it to the given uri (path), it should be noted that reading the content of the dotrain is not in
    /// the scope of Store and handling and passing on a correct URI (path) for the given text must be handled
    /// externally by the implementer
    pub fn set_dotrain(
        &mut self,
        text: &str,
        uri: &str,
        keep_old: bool,
    ) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let bytes = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(text.as_bytes()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
        .cbor_encode()?;
        let new_hash = keccak256(&bytes).0.to_vec();
        if let Some(h) = self.dotrain_cache.get(uri) {
            let old_hash = h.clone();
            if new_hash == old_hash {
                self.cache.insert_verified(&new_hash, bytes)?;
                Ok((new_hash, vec![]))
            } else {
                self.cache.insert_verified(&new_hash, bytes)?;
                self.dotrain_cache.insert(uri.to_string(), new_hash.clone());
                if !keep_old {
                    self.cache.remove(&old_hash);
                }
                Ok((new_hash, old_hash))
            }
        } else {
            self.dotrain_cache.insert(uri.to_string(), new_hash.clone());
            self.cache.insert_verified(&new_hash, bytes)?;
            Ok((new_hash, vec![]))
        }
    }

    /// decodes each meta and stores the inner meta items into the cache
    /// if any of the inner items is an authoring meta, stores it in authoring meta cache as well
    /// returns the reference to the authoring bytes if the meta bytes contained any
    fn store_content(&mut self, bytes: &[u8]) {
        if let Ok(meta_maps) = RainMetaDocumentV1Item::cbor_decode(bytes) {
            if bytes.starts_with(&KnownMagic::RainMetaDocumentV1.to_prefix_bytes()) {
                for meta_map in &meta_maps {
                    if let Ok(encoded_bytes) = meta_map.cbor_encode() {
                        // the key is this item's own digest, so the gate can
                        // only pass - routing through it anyway means no
                        // reader has to work that out
                        let _ = self
                            .cache
                            .insert_verified(&keccak256(&encoded_bytes).0, encoded_bytes);
                    }
                }
            }
        }
    }
}

/// converts string to bytes32
///
/// Right padding with `0u8` is the encoding, so [`bytes32_to_str`] ends the
/// string at the first `0u8` and cannot carry one. An input holding a nul is
/// rejected rather than round tripped into a shorter string.
pub fn str_to_bytes32(text: &str) -> Result<[u8; 32], Error> {
    let bytes: &[u8] = text.as_bytes();
    if bytes.len() > 32 {
        return Err(Error::BiggerThan32Bytes);
    }
    if bytes.contains(&0u8) {
        return Err(Error::NulByteInInput);
    }
    let mut b32 = [0u8; 32];
    b32[..bytes.len()].copy_from_slice(bytes);
    Ok(b32)
}

/// converts bytes32 to string
pub fn bytes32_to_str(bytes: &[u8; 32]) -> Result<&str, Error> {
    let mut len = 32;
    if let Some((pos, _)) = itertools::Itertools::find_position(&mut bytes.iter(), |b| **b == 0u8) {
        len = pos;
    };
    Ok(std::str::from_utf8(&bytes[..len])?)
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::{
        *, bytes32_to_str, magic::KnownMagic, str_to_bytes32, types::authoring::v1::AuthoringMeta,
        ContentEncoding, ContentLanguage, ContentType, Error, RainMetaDocumentV1Item,
    };
    use alloy::providers::ProviderBuilder;
    use alloy::{providers::mock::Asserter, rpc::json_rpc::ErrorPayload, sol_types::SolType};
    use serde_json::json;

    /// Roundtrip test for an authoring meta
    /// original content -> pack -> MetaMap -> cbor encode -> cbor decode -> MetaMap -> unpack -> original content,
    #[test]
    fn authoring_meta_roundtrip() -> Result<(), Error> {
        let authoring_meta_content = r#"[
            {
                "word": "stack",
                "description": "Copies an existing value from the stack.",
                "operandParserOffset": 16
            },
            {
                "word": "constant",
                "description": "Copies a constant value onto the stack.",
                "operandParserOffset": 16
            }
        ]"#;
        let authoring_meta: AuthoringMeta = serde_json::from_str(authoring_meta_content)?;

        // abi encode the authoring meta with performing validation
        let authoring_meta_abi_encoded = authoring_meta.abi_encode_validate()?;
        let expected_abi_encoded = <alloy::sol!((bytes32, uint8, string)[])>::abi_encode(&vec![
            (
                str_to_bytes32("stack")?,
                16u8,
                "Copies an existing value from the stack.".to_string(),
            ),
            (
                str_to_bytes32("constant")?,
                16u8,
                "Copies a constant value onto the stack.".to_string(),
            ),
        ]);
        // check the encoded bytes agaiinst the expected
        assert_eq!(authoring_meta_abi_encoded, expected_abi_encoded);

        let meta_map = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(authoring_meta_abi_encoded.clone()),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let cbor_encoded = meta_map.cbor_encode()?;

        // cbor map with 3 keys
        assert_eq!(cbor_encoded[0], 0xa3);
        // key 0
        assert_eq!(cbor_encoded[1], 0x00);
        // major type 2 (bytes) length 512
        assert_eq!(cbor_encoded[2], 0b010_11001);
        assert_eq!(cbor_encoded[3], 0b000_00010);
        assert_eq!(cbor_encoded[4], 0b000_00000);
        // payload
        assert_eq!(cbor_encoded[5..517], authoring_meta_abi_encoded);
        // key 1
        assert_eq!(cbor_encoded[517], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[518], 0b000_11011);
        // magic number
        assert_eq!(
            &cbor_encoded[519..527],
            KnownMagic::AuthoringMetaV1.to_prefix_bytes()
        );
        // key 2
        assert_eq!(cbor_encoded[527], 0x02);
        // text string application/cbor length 16
        assert_eq!(cbor_encoded[528], 0b011_10000);
        // the string application/cbor, must be the end of data
        assert_eq!(&cbor_encoded[529..], "application/cbor".as_bytes());

        // decode the data back to MetaMap
        let cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 1 as we only had 1 encoded item
        assert_eq!(cbor_decoded.len(), 1);
        // decoded item must be equal to the original meta_map
        assert_eq!(cbor_decoded[0], meta_map);

        Ok(())
    }

    /// Roundtrip test for a dotrain meta
    /// original content -> pack -> MetaMap -> cbor encode -> cbor decode -> MetaMap -> unpack -> original content,
    #[test]
    fn dotrain_meta_roundtrip() -> Result<(), Error> {
        let dotrain_content = "#main _ _: int-add(1 2) int-add(2 3)";
        let dotrain_content_bytes = dotrain_content.as_bytes().to_vec();

        let content_encoding = ContentEncoding::Deflate;
        let deflated_payload = content_encoding.encode(&dotrain_content_bytes);

        let meta_map = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(deflated_payload.clone()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::OctetStream,
            content_encoding,
            content_language: ContentLanguage::En,
            schema: None,
        };
        let cbor_encoded = meta_map.cbor_encode()?;

        // cbor map with 5 keys
        assert_eq!(cbor_encoded[0], 0xa5);
        // key 0
        assert_eq!(cbor_encoded[1], 0x00);
        // major type 2 (bytes) length 36
        assert_eq!(cbor_encoded[2], 0b010_11000);
        assert_eq!(cbor_encoded[3], 0b001_00100);
        // assert_eq!(cbor_encoded[4], 0b000_00000);
        // payload
        assert_eq!(cbor_encoded[4..40], deflated_payload);
        // key 1
        assert_eq!(cbor_encoded[40], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[41], 0b000_11011);
        // magic number
        assert_eq!(
            &cbor_encoded[42..50],
            KnownMagic::DotrainV1.to_prefix_bytes()
        );
        // key 2
        assert_eq!(cbor_encoded[50], 0x02);
        // text string application/octet-stream length 24
        assert_eq!(cbor_encoded[51], 0b011_11000);
        assert_eq!(cbor_encoded[52], 0b000_11000);
        // the string application/octet-stream
        assert_eq!(&cbor_encoded[53..77], "application/octet-stream".as_bytes());
        // key 3
        assert_eq!(cbor_encoded[77], 0x03);
        // text string deflate length 7
        assert_eq!(cbor_encoded[78], 0b011_00111);
        // the string deflate
        assert_eq!(&cbor_encoded[79..86], "deflate".as_bytes());
        // key 4
        assert_eq!(cbor_encoded[86], 0x04);
        // text string en length 2
        assert_eq!(cbor_encoded[87], 0b011_00010);
        // the string identity, must be the end of data
        assert_eq!(&cbor_encoded[88..], "en".as_bytes());

        // decode the data back to MetaMap
        let cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 1 as we only had 1 encoded item
        assert_eq!(cbor_decoded.len(), 1);
        // decoded item must be equal to the original meta_map
        assert_eq!(cbor_decoded[0], meta_map);

        Ok(())
    }

    /// Roundtrip test for a meta sequence
    /// original content -> pack -> MetaMap -> cbor encode -> cbor decode -> MetaMap -> unpack -> original content,
    #[test]
    fn meta_seq_roundtrip() -> Result<(), Error> {
        let authoring_meta_content = r#"[
            {
                "word": "stack",
                "description": "Copies an existing value from the stack.",
                "operandParserOffset": 16
            },
            {
                "word": "constant",
                "description": "Copies a constant value onto the stack.",
                "operandParserOffset": 16
            }
        ]"#;
        let authoring_meta: AuthoringMeta = serde_json::from_str(authoring_meta_content)?;
        let authoring_meta_abi_encoded = authoring_meta.abi_encode_validate()?;
        let meta_map_1 = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(authoring_meta_abi_encoded.clone()),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let dotrain_content = "#main _ _: int-add(1 2) int-add(2 3)";
        let dotrain_content_bytes = dotrain_content.as_bytes().to_vec();
        let content_encoding = ContentEncoding::Deflate;
        let deflated_payload = content_encoding.encode(&dotrain_content_bytes);
        let meta_map_2 = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(deflated_payload.clone()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::OctetStream,
            content_encoding,
            content_language: ContentLanguage::En,
            schema: None,
        };

        // cbor encode as RainMetaDocument sequence
        let cbor_encoded = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![meta_map_1.clone(), meta_map_2.clone()],
            KnownMagic::RainMetaDocumentV1,
        )?;

        // 8 byte magic number prefix
        assert_eq!(
            &cbor_encoded[0..8],
            KnownMagic::RainMetaDocumentV1.to_prefix_bytes()
        );

        // first item in the encoded bytes
        // cbor map with 3 keys
        assert_eq!(cbor_encoded[8], 0xa3);
        // key 0
        assert_eq!(cbor_encoded[9], 0x00);
        // major type 2 (bytes) length 512
        assert_eq!(cbor_encoded[10], 0b010_11001);
        assert_eq!(cbor_encoded[11], 0b000_00010);
        assert_eq!(cbor_encoded[12], 0b000_00000);
        // payload
        assert_eq!(cbor_encoded[13..525], authoring_meta_abi_encoded);
        // key 1
        assert_eq!(cbor_encoded[525], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[526], 0b000_11011);
        // magic number
        assert_eq!(
            &cbor_encoded[527..535],
            KnownMagic::AuthoringMetaV1.to_prefix_bytes()
        );
        // key 2
        assert_eq!(cbor_encoded[535], 0x02);
        // text string application/cbor length 16
        assert_eq!(cbor_encoded[536], 0b011_10000);
        // the string application/cbor, must be the end of data
        assert_eq!(&cbor_encoded[537..553], "application/cbor".as_bytes());

        // second item in the encoded bytes
        // cbor map with 5 keys
        assert_eq!(cbor_encoded[553], 0xa5);
        // key 0
        assert_eq!(cbor_encoded[554], 0x00);
        // major type 2 (bytes) length 36
        assert_eq!(cbor_encoded[555], 0b010_11000);
        assert_eq!(cbor_encoded[556], 0b001_00100);
        // assert_eq!(cbor_encoded[4], 0b000_00000);
        // payload
        assert_eq!(cbor_encoded[557..593], deflated_payload);
        // key 1
        assert_eq!(cbor_encoded[593], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[594], 0b000_11011);
        // magic number
        assert_eq!(
            &cbor_encoded[595..603],
            KnownMagic::DotrainV1.to_prefix_bytes()
        );
        // key 2
        assert_eq!(cbor_encoded[603], 0x02);
        // text string application/octet-stream length 24
        assert_eq!(cbor_encoded[604], 0b011_11000);
        assert_eq!(cbor_encoded[605], 0b000_11000);
        // the string application/octet-stream
        assert_eq!(
            &cbor_encoded[606..630],
            "application/octet-stream".as_bytes()
        );
        // key 3
        assert_eq!(cbor_encoded[630], 0x03);
        // text string deflate length 7
        assert_eq!(cbor_encoded[631], 0b011_00111);
        // the string deflate
        assert_eq!(&cbor_encoded[632..639], "deflate".as_bytes());
        // key 4
        assert_eq!(cbor_encoded[639], 0x04);
        // text string en length 2
        assert_eq!(cbor_encoded[640], 0b011_00010);
        // the string identity, must be the end of data
        assert_eq!(&cbor_encoded[641..], "en".as_bytes());

        // decode the data back to MetaMap
        let cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 2 as we had 2 encoded item
        assert_eq!(cbor_decoded.len(), 2);

        // decoded item 1 must be equal to the original meta_map_1
        assert_eq!(cbor_decoded[0], meta_map_1);
        // decoded item 2 must be equal to the original meta_map_2
        assert_eq!(cbor_decoded[1], meta_map_2);

        Ok(())
    }

    #[test]
    fn test_bytes32_to_str() {
        let text_bytes_list = vec![
            (
                "",
                hex!("0000000000000000000000000000000000000000000000000000000000000000"),
            ),
            (
                "A",
                hex!("4100000000000000000000000000000000000000000000000000000000000000"),
            ),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
                hex!("4142434445464748494a4b4c4d4e4f505152535455565758595a303132333435"),
            ),
            (
                "!@#$%^&*(),./;'[]",
                hex!("21402324255e262a28292c2e2f3b275b5d000000000000000000000000000000"),
            ),
        ];

        for (text, bytes) in text_bytes_list {
            assert_eq!(text, bytes32_to_str(&bytes).unwrap());
        }
    }

    #[test]
    fn test_str_to_bytes32() {
        let text_bytes_list = vec![
            (
                "",
                hex!("0000000000000000000000000000000000000000000000000000000000000000"),
            ),
            (
                "A",
                hex!("4100000000000000000000000000000000000000000000000000000000000000"),
            ),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
                hex!("4142434445464748494a4b4c4d4e4f505152535455565758595a303132333435"),
            ),
            (
                "!@#$%^&*(),./;'[]",
                hex!("21402324255e262a28292c2e2f3b275b5d000000000000000000000000000000"),
            ),
        ];

        for (text, bytes) in text_bytes_list {
            assert_eq!(bytes, str_to_bytes32(text).unwrap());
        }
    }

    #[test]
    fn test_str_to_bytes32_long() {
        assert!(matches!(
            str_to_bytes32("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456").unwrap_err(),
            Error::BiggerThan32Bytes
        ));
    }

    /// A nul cannot survive the padding convention bytes32_to_str decodes, so
    /// it is rejected on the way in wherever it sits, including the pair the
    /// issue collides ("a" and "a\0").
    #[test]
    fn test_str_to_bytes32_rejects_nul() {
        for text in [
            "\0",
            "\0a",
            "a\0",
            "a\0b",
            "abcdefghijklmnopqrstuvwxyz01234\0",
        ] {
            assert!(
                matches!(str_to_bytes32(text), Err(Error::NulByteInInput)),
                "nul bearing input {:?} accepted",
                text
            );
        }
    }

    /// Everything str_to_bytes32 accepts comes back out of bytes32_to_str
    /// unchanged, and no two of them share a bytes32.
    #[test]
    fn test_str_to_bytes32_round_trip() -> Result<(), Error> {
        let mut seen: Vec<[u8; 32]> = vec![];
        for text in [
            "",
            "a",
            "stack",
            "!@#$%^&*(),./;'[]",
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
        ] {
            let bytes = str_to_bytes32(text)?;
            assert_eq!(bytes32_to_str(&bytes)?, text);
            assert!(!seen.contains(&bytes), "input {:?} collided", text);
            seen.push(bytes);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_implements_i_describe_by_meta_v1() {
        // makes new server/client with success response for erc165 check
        async fn new_server_client() -> (Asserter, impl Provider) {
            let asserter = Asserter::new();
            let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());

            // Mock a responses for successful supports erc165 check
            asserter.push_success(
                &"0x0000000000000000000000000000000000000000000000000000000000000001",
            );
            asserter.push_success(
                &"0x0000000000000000000000000000000000000000000000000000000000000000",
            );

            (asserter, provider)
        }

        let address = Address::random();

        // mock a true response for implements IDescribedByMetaV1
        let (asserter, provider) = new_server_client().await;
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        let result = implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap();
        assert!(result);

        // mock a false response for implements IDescribedByMetaV1
        let (asserter, provider) = new_server_client().await;
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        let result = implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap();
        assert!(!result);

        // mock a revert response for implements IDescribedByMetaV1
        let (asserter, provider) = new_server_client().await;
        asserter.push_failure(ErrorPayload {
            code: -32003,
            message: "execution reverted".into(),
            data: Some(serde_json::value::to_raw_value(&json!("0x00")).unwrap()),
        });
        let result = implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap();
        assert!(!result);
    }

    /// Roundtrip test for a meta map carrying the OaSchema magic number as an
    /// additional CBOR map key beyond the standard 0-4 keys.
    /// MetaMap (with schema) -> cbor encode -> cbor decode -> MetaMap, assert equality
    #[test]
    fn oa_schema_map_key_roundtrip() -> Result<(), Error> {
        let payload = vec![0x01, 0x02, 0x03];
        // an IPFS hash referencing the schema of the payload, as written by
        // the SFT frontend under the OaSchema map key
        let schema = "QmSchemaHash1234567890abcdefghijklmnopqrstuvwx".to_string();
        assert_eq!(schema.len(), 46);

        let meta_map = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(payload.clone()),
            magic: KnownMagic::OaStructure,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::None,
            schema: Some(schema.clone()),
        };
        let cbor_encoded = meta_map.cbor_encode()?;

        // cbor map with 5 keys (0, 1, 2, 3 and the OaSchema magic)
        assert_eq!(cbor_encoded[0], 0xa5);
        // key 0
        assert_eq!(cbor_encoded[1], 0x00);
        // major type 2 (bytes) length 3
        assert_eq!(cbor_encoded[2], 0b010_00011);
        // payload
        assert_eq!(cbor_encoded[3..6], payload);
        // key 1
        assert_eq!(cbor_encoded[6], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[7], 0b000_11011);
        // magic number
        assert_eq!(
            &cbor_encoded[8..16],
            KnownMagic::OaStructure.to_prefix_bytes()
        );
        // key 2
        assert_eq!(cbor_encoded[16], 0x02);
        // text string application/json length 16
        assert_eq!(cbor_encoded[17], 0b011_10000);
        assert_eq!(&cbor_encoded[18..34], "application/json".as_bytes());
        // key 3
        assert_eq!(cbor_encoded[34], 0x03);
        // text string deflate length 7
        assert_eq!(cbor_encoded[35], 0b011_00111);
        assert_eq!(&cbor_encoded[36..43], "deflate".as_bytes());
        // the OaSchema magic as key, major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[43], 0b000_11011);
        assert_eq!(
            &cbor_encoded[44..52],
            KnownMagic::OaSchema.to_prefix_bytes()
        );
        // schema value, text string length 46
        assert_eq!(cbor_encoded[52], 0b011_11000);
        assert_eq!(cbor_encoded[53], 46);
        // the schema hash string, must be the end of data
        assert_eq!(&cbor_encoded[54..], schema.as_bytes());

        // decode the data back to MetaMap
        let cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 1 as we only had 1 encoded item
        assert_eq!(cbor_decoded.len(), 1);
        // decoded item must be equal to the original meta_map
        assert_eq!(cbor_decoded[0], meta_map);

        Ok(())
    }

    /// A meta map without the schema key must keep encoding exactly as before
    /// (no schema entry on the wire) and roundtrip with schema None
    #[test]
    fn no_schema_key_encodes_as_before_roundtrip() -> Result<(), Error> {
        let payload = vec![0x0a, 0x0b];
        let meta_map = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(payload.clone()),
            magic: KnownMagic::OaStructure,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let cbor_encoded = meta_map.cbor_encode()?;

        // cbor map with only the 2 mandatory keys
        assert_eq!(cbor_encoded[0], 0xa2);
        // key 0
        assert_eq!(cbor_encoded[1], 0x00);
        // major type 2 (bytes) length 2
        assert_eq!(cbor_encoded[2], 0b010_00010);
        // payload
        assert_eq!(cbor_encoded[3..5], payload);
        // key 1
        assert_eq!(cbor_encoded[5], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(cbor_encoded[6], 0b000_11011);
        // magic number, must be the end of data
        assert_eq!(
            &cbor_encoded[7..],
            KnownMagic::OaStructure.to_prefix_bytes()
        );

        let cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        assert_eq!(cbor_decoded.len(), 1);
        assert_eq!(cbor_decoded[0], meta_map);

        Ok(())
    }

    /// A map key this version has no meaning for is a future index, so it is
    /// skipped and the rest of the map decodes
    #[test]
    fn unknown_map_key_index_is_ignored() -> Result<(), Error> {
        let mut bytes: Vec<u8> = vec![
            // cbor map with 3 keys
            0xa3, // key 0, bytes payload of length 0
            0x00, 0x40, // key 1, unsigned integer magic number
            0x01, 0x1b,
        ];
        bytes.extend_from_slice(&KnownMagic::DotrainSourceV1.to_prefix_bytes());
        // key 5, a plausible future index, unsigned integer value 7
        bytes.extend_from_slice(&[0x05, 0x07]);

        let decoded = RainMetaDocumentV1Item::cbor_decode(&bytes)?;
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], plain_item(KnownMagic::DotrainSourceV1, vec![]));

        Ok(())
    }

    /// A magic number other than OaSchema used as an extra map key is a future
    /// magic keyed entry, skipped the same way, and leaves schema unset
    #[test]
    fn non_oa_schema_extra_map_key_is_ignored() -> Result<(), Error> {
        // build a map identical to a valid 2 key meta map but with an extra
        // OaHashList magic key carrying a text string
        let mut bytes: Vec<u8> = vec![
            // cbor map with 3 keys
            0xa3, // key 0, bytes payload of length 1
            0x00, 0x41, 0xff, // key 1, unsigned integer magic number
            0x01, 0x1b,
        ];
        bytes.extend_from_slice(&KnownMagic::OaStructure.to_prefix_bytes());
        // the OaHashList magic as key
        bytes.push(0x1b);
        bytes.extend_from_slice(&KnownMagic::OaHashList.to_prefix_bytes());
        // text string value of length 2
        bytes.extend_from_slice(&[0x62, 0x68, 0x69]);

        let decoded = RainMetaDocumentV1Item::cbor_decode(&bytes)?;
        assert_eq!(decoded.len(), 1);
        let expected = plain_item(KnownMagic::OaStructure, vec![0xff]);
        assert_eq!(decoded[0], expected);
        assert_eq!(decoded[0].schema, None);

        Ok(())
    }

    /// The whole value of an unknown key is consumed however nested, so the
    /// item that follows it in the sequence still decodes
    #[test]
    fn unknown_map_key_consumes_its_whole_value() -> Result<(), Error> {
        let mut bytes: Vec<u8> = vec![0xa3, 0x00, 0x41, 0x01, 0x01, 0x1b];
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        // key 5, value {42: [1, 2]}
        bytes.extend_from_slice(&[0x05, 0xa1, 0x18, 0x2a, 0x82, 0x01, 0x02]);
        bytes.extend_from_slice(&handwritten_map());

        let decoded = RainMetaDocumentV1Item::cbor_decode(&bytes)?;
        assert_eq!(decoded.len(), 2);
        let expected = plain_item(KnownMagic::DotrainV1, vec![0x01]);
        assert_eq!(decoded[0], expected);
        assert_eq!(decoded[1], expected);

        Ok(())
    }

    /// An ignored key is not re-encoded, so the item's hash is the hash of what
    /// this version can represent and not of the bytes it decoded
    #[test]
    fn ignored_map_key_is_absent_from_the_reencoding() -> Result<(), Error> {
        let mut bytes: Vec<u8> = vec![0xa3, 0x00, 0x41, 0x01, 0x01, 0x1b];
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        bytes.extend_from_slice(&[0x05, 0x07]);

        let decoded = RainMetaDocumentV1Item::cbor_decode(&bytes)?;
        assert_eq!(decoded[0].cbor_encode()?, handwritten_map());
        assert_eq!(decoded[0].hash(false)?, keccak256(handwritten_map()).0);
        assert_ne!(decoded[0].hash(false)?, keccak256(&bytes).0);

        Ok(())
    }

    /// Only integer keys are indexes. The spec rules out the HTTP header names
    /// as keys, so a key that is not an unsigned integer is not a future index
    /// to skip over
    #[test]
    fn non_integer_map_key_errors() {
        let mut text_key: Vec<u8> = vec![0xa3, 0x00, 0x41, 0x01, 0x01, 0x1b];
        text_key.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        // key "5", unsigned integer value 7
        text_key.extend_from_slice(&[0x61, 0x35, 0x07]);
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&text_key),
            Err(Error::SerdeCborError(_))
        ));

        let mut negative_key: Vec<u8> = vec![0xa3, 0x00, 0x41, 0x01, 0x01, 0x1b];
        negative_key.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        // key -1, unsigned integer value 7
        negative_key.extend_from_slice(&[0x20, 0x07]);
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&negative_key),
            Err(Error::SerdeCborError(_))
        ));
    }

    /// An unknown key is skipped, never counted as one of the mandatory keys,
    /// so a map carrying one instead of key 0 is still missing its payload and
    /// is dropped rather than decoding with the unknown key standing in.
    #[test]
    fn unknown_map_key_does_not_stand_in_for_a_mandatory_key() {
        let mut bytes: Vec<u8> = vec![0xa2, 0x05, 0x07, 0x01, 0x1b];
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::CorruptMeta)
        ));
    }

    fn plain_item(magic: KnownMagic, payload: Vec<u8>) -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(payload),
            magic,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
    }

    // ---- helpers for the CAS / search tests ----

    fn sample_authoring_doc() -> (AuthoringMeta, Vec<u8>) {
        let authoring_meta: AuthoringMeta = serde_json::from_str(
            r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#,
        )
        .unwrap();
        let abi = authoring_meta.abi_encode_validate().unwrap();
        let item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(abi),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let doc =
            RainMetaDocumentV1Item::cbor_encode_seq(&vec![item], KnownMagic::RainMetaDocumentV1)
                .unwrap();
        (authoring_meta, doc)
    }

    fn sample_dotrain_item() -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("some dotrain body".as_bytes()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
    }

    /// Handwritten canonical cbor for {0: h'01', 1: DotrainV1 magic}, written
    /// out byte by byte from the cbor spec, independent of cbor_encode.
    fn handwritten_map() -> Vec<u8> {
        vec![
            0xa2, // map(2)
            0x00, // key 0
            0x41, 0x01, // bytes(1) 0x01
            0x01, // key 1
            0x1b, 0xff, 0xda, 0xc2, 0xf2, 0xf3, 0x7b, 0xe8, 0x94, // u64 DotrainV1
        ]
    }

    /// hash(false) is keccak256 of the bare cbor map and hash(true) is
    /// keccak256 of the rain meta document prefix followed by the same map,
    /// pinned against independently handwritten bytes.
    #[test]
    fn test_hash_bare_vs_document() -> Result<(), Error> {
        let map_bytes = handwritten_map();
        let mut doc_bytes: Vec<u8> = vec![0xff, 0x0a, 0x89, 0xc6, 0x74, 0xee, 0x78, 0x74];
        doc_bytes.extend_from_slice(&map_bytes);

        let item = plain_item(KnownMagic::DotrainV1, vec![0x01]);
        assert_eq!(item.hash(false)?, keccak256(&map_bytes).0);
        assert_eq!(item.hash(true)?, keccak256(&doc_bytes).0);
        assert_ne!(item.hash(false)?, item.hash(true)?);
        Ok(())
    }

    /// Empty input and a bare document prefix with no items are corrupt metas.
    #[test]
    fn test_cbor_decode_empty_is_corrupt() {
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&[]),
            Err(Error::CorruptMeta)
        ));
        let prefix = KnownMagic::RainMetaDocumentV1.to_prefix_bytes();
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&prefix),
            Err(Error::CorruptMeta)
        ));
    }

    /// A valid map followed by truncated trailing bytes must not decode: the
    /// data does not end exactly at the last complete item.
    #[test]
    fn test_cbor_decode_trailing_truncated_is_corrupt() {
        let mut bytes = handwritten_map();
        bytes.push(0x1b); // u64 header with all 8 payload bytes missing
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::CorruptMeta)
        ));
    }

    /// Every way an item can run out of bytes is corrupt meta, not a serde
    /// cbor error: a truncated sole item, a map header promising entries the
    /// input does not carry, and a truncated item after a complete one.
    #[test]
    fn test_cbor_decode_truncated_item_is_corrupt() {
        let mut sole = handwritten_map();
        sole.pop();
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&sole),
            Err(Error::CorruptMeta)
        ));

        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&[0xa2, 0x00, 0x41, 0x01]),
            Err(Error::CorruptMeta)
        ));

        let mut after_complete = handwritten_map();
        after_complete.extend_from_slice(&[0xa2, 0x00]);
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&after_complete),
            Err(Error::CorruptMeta)
        ));
    }

    /// A valid map followed by a byte that is not valid cbor surfaces the
    /// serde cbor error.
    #[test]
    fn test_cbor_decode_trailing_garbage_errors() {
        let mut bytes = handwritten_map();
        bytes.push(0xff); // lone break byte
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::SerdeCborError(_))
        ));
    }

    /// A map without the mandatory payload key 0 is the spec's "unexpected"
    /// and is dropped. Alone it leaves nothing to return, which is
    /// CorruptMeta - the document as a whole carried no meta.
    #[test]
    fn test_cbor_decode_missing_payload_is_dropped() {
        let mut bytes: Vec<u8> = vec![0xa1, 0x01, 0x1b]; // {1: DotrainV1}
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::CorruptMeta)
        ));
    }

    /// A map without the mandatory magic key 1, likewise.
    #[test]
    fn test_cbor_decode_missing_magic_is_dropped() {
        let bytes: Vec<u8> = vec![0xa1, 0x00, 0x41, 0x01]; // {0: h'01'}
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::CorruptMeta)
        ));
    }

    /// A map carrying a magic number this version does not know is dropped,
    /// not an error: the spec invites other people's numbers, and the whole
    /// point of the per item magic is O(1) drop/ignore of meta a tool does not
    /// support.
    #[test]
    fn test_cbor_decode_unknown_magic_is_dropped() {
        let mut bytes: Vec<u8> = vec![0xa2, 0x00, 0x41, 0x01, 0x01, 0x1b];
        bytes.extend_from_slice(&0xdeadbeefdeadbeefu64.to_be_bytes());
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::CorruptMeta)
        ));
    }

    /// The point of dropping rather than failing: an item this version cannot
    /// read does not take the items beside it with it. Each of the three drop
    /// cases sits next to a good item, and the good item still decodes.
    #[test]
    fn test_cbor_decode_drops_only_the_unreadable_item() {
        let good = plain_item(KnownMagic::DotrainV1, vec![0x42])
            .cbor_encode()
            .unwrap();

        let mut unknown_magic: Vec<u8> = vec![0xa2, 0x00, 0x41, 0x01, 0x01, 0x1b];
        unknown_magic.extend_from_slice(&0xdeadbeefdeadbeefu64.to_be_bytes());
        let no_magic: Vec<u8> = vec![0xa1, 0x00, 0x41, 0x01];
        let mut no_payload: Vec<u8> = vec![0xa1, 0x01, 0x1b];
        no_payload.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());

        for dropped in [unknown_magic, no_magic, no_payload] {
            // the dropped item first, so a decoder that stopped at it would
            // return nothing rather than the good item behind it
            let mut document = KnownMagic::RainMetaDocumentV1.to_prefix_bytes().to_vec();
            document.extend_from_slice(&dropped);
            document.extend_from_slice(&good);

            let items = RainMetaDocumentV1Item::cbor_decode(&document).unwrap();
            assert_eq!(items.len(), 1, "expected only the good item");
            assert_eq!(items[0].magic, KnownMagic::DotrainV1);
            assert_eq!(items[0].payload.as_ref(), &[0x42]);
        }
    }

    /// The document prefix is not a magic number an item may carry: it is the
    /// first 8 bytes of a whole document. An item claiming it is malformed
    /// structure rather than a type this version does not read, so it stops
    /// the document instead of being dropped like an unknown number.
    #[test]
    fn test_cbor_decode_nested_document_magic_is_not_dropped() {
        let good = plain_item(KnownMagic::DotrainV1, vec![0x42])
            .cbor_encode()
            .unwrap();

        let mut nested: Vec<u8> = vec![0xa2, 0x00, 0x41, 0x01, 0x01, 0x1b];
        nested.extend_from_slice(&KnownMagic::RainMetaDocumentV1.to_prefix_bytes());

        let mut document = KnownMagic::RainMetaDocumentV1.to_prefix_bytes().to_vec();
        document.extend_from_slice(&nested);
        document.extend_from_slice(&good);

        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&document),
            Err(Error::SerdeCborError(_))
        ));
    }

    /// A cbor map header plus the given already encoded key/value pairs,
    /// written per the cbor spec rather than through the encoder, so a map can
    /// carry a key twice which no encoder emits.
    fn handwritten_entries(entries: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
        assert!(entries.len() < 24);
        let mut bytes = vec![0xa0 | entries.len() as u8];
        for (key, value) in entries {
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(value);
        }
        bytes
    }

    /// cbor unsigned integer of the magic number, as a map key or a value.
    fn handwritten_magic(magic: KnownMagic) -> Vec<u8> {
        let mut bytes = vec![0x1b];
        bytes.extend_from_slice(&magic.to_prefix_bytes());
        bytes
    }

    /// cbor text string of a string shorter than 24 bytes.
    fn handwritten_text(text: &str) -> Vec<u8> {
        assert!(text.len() < 24);
        let mut bytes = vec![0x60 | text.len() as u8];
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    /// The repro from #191: a map repeating key 0 must not decode with the
    /// last payload winning.
    #[test]
    fn test_cbor_decode_duplicate_payload_key_errors() {
        let mut bytes: Vec<u8> = vec![0xa3, 0x00, 0x41, 0x01, 0x00, 0x41, 0x02, 0x01, 0x1b];
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        let error = RainMetaDocumentV1Item::cbor_decode(&bytes).unwrap_err();
        assert!(matches!(error, Error::SerdeCborError(_)));
        assert!(
            error.to_string().contains("duplicate field `payload`"),
            "{error}"
        );
    }

    /// RFC 8949 §5.6: every recognised key is rejected when the map carries it
    /// twice, whether the repeated value differs from the first or matches it.
    #[test]
    fn test_cbor_decode_duplicate_any_key_errors() -> Result<(), Error> {
        let cases: [(Vec<u8>, Vec<u8>, Vec<u8>); 6] = [
            (vec![0x00], vec![0x41, 0x01], vec![0x41, 0x02]),
            (
                vec![0x01],
                handwritten_magic(KnownMagic::DotrainV1),
                handwritten_magic(KnownMagic::RainlangV1),
            ),
            (
                vec![0x02],
                handwritten_text("application/cbor"),
                handwritten_text("application/json"),
            ),
            (
                vec![0x03],
                handwritten_text("identity"),
                handwritten_text("deflate"),
            ),
            (vec![0x04], handwritten_text("en"), handwritten_text("none")),
            (
                handwritten_magic(KnownMagic::OaSchema),
                handwritten_text("hi"),
                handwritten_text("bye"),
            ),
        ];
        let base: Vec<(Vec<u8>, Vec<u8>)> = cases
            .iter()
            .map(|(key, value, _)| (key.clone(), value.clone()))
            .collect();

        let decoded = RainMetaDocumentV1Item::cbor_decode(&handwritten_entries(&base))?;
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].payload.as_ref(), &[0x01]);
        assert_eq!(decoded[0].magic, KnownMagic::DotrainV1);
        assert_eq!(decoded[0].content_type, ContentType::Cbor);
        assert_eq!(decoded[0].content_encoding, ContentEncoding::Identity);
        assert_eq!(decoded[0].content_language, ContentLanguage::En);
        assert_eq!(decoded[0].schema.as_deref(), Some("hi"));

        for (key, value, other_value) in &cases {
            for repeated in [value, other_value] {
                let mut entries = base.clone();
                entries.push((key.clone(), repeated.clone()));
                let error = RainMetaDocumentV1Item::cbor_decode(&handwritten_entries(&entries))
                    .unwrap_err();
                assert!(
                    matches!(error, Error::SerdeCborError(_)),
                    "{key:?} {error:?}"
                );
                assert!(
                    error.to_string().contains("duplicate field"),
                    "{key:?} {error}"
                );
            }
        }
        Ok(())
    }

    /// An index this version has no meaning for is skipped rather than
    /// rejected, but RFC 8949 §5.6 does not ask whether a key is understood:
    /// a map that carries an unknown index twice is invalid too, whether that
    /// index is a plain integer or a magic number other than OaSchema.
    #[test]
    fn test_cbor_decode_duplicate_unknown_key_errors() -> Result<(), Error> {
        let base: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (vec![0x00], vec![0x41, 0x01]),
            (vec![0x01], handwritten_magic(KnownMagic::DotrainV1)),
        ];
        // key 5 as a plausible future index and the OaHashList magic as a
        // future magic keyed entry, each with the value cbor to repeat it with
        let unknown: [(Vec<u8>, Vec<u8>, Vec<u8>); 2] = [
            (vec![0x05], vec![0x07], vec![0x08]),
            (
                handwritten_magic(KnownMagic::OaHashList),
                handwritten_text("hi"),
                handwritten_text("bye"),
            ),
        ];

        for (key, value, other_value) in &unknown {
            let mut entries = base.clone();
            entries.push((key.clone(), value.clone()));
            let decoded = RainMetaDocumentV1Item::cbor_decode(&handwritten_entries(&entries))?;
            assert_eq!(decoded, vec![plain_item(KnownMagic::DotrainV1, vec![0x01])]);

            for repeated in [value, other_value] {
                let mut duplicated = entries.clone();
                duplicated.push((key.clone(), repeated.clone()));
                let error = RainMetaDocumentV1Item::cbor_decode(&handwritten_entries(&duplicated))
                    .unwrap_err();
                assert!(
                    matches!(error, Error::SerdeCborError(_)),
                    "{key:?} {error:?}"
                );
                assert!(
                    error.to_string().contains("duplicate map key"),
                    "{key:?} {error}"
                );
            }
        }
        Ok(())
    }

    /// A handwritten item map carrying the rain meta document magic under key
    /// 1 decodes, so accepting the document magic as an item magic is the
    /// decoder's own behaviour and not an artefact of this crate's encoder.
    #[test]
    fn test_cbor_decode_handwritten_document_magic_item() {
        let bytes: Vec<u8> = vec![
            0xa2, // map(2)
            0x00, // key 0
            0x41, 0x01, // bytes(1) 0x01
            0x01, // key 1
            0x1b, 0xff, 0x0a, 0x89, 0xc6, 0x74, 0xee, 0x78, 0x74, // u64 RainMetaDocumentV1
        ];
        // The document magic in an item's magic position is structurally
        // invalid, so the meta carrying it does not decode.
        // rainlanguage/rain.metadata#204.
        assert!(RainMetaDocumentV1Item::cbor_decode(&bytes).is_err());
    }

    /// The document magic as an item's own magic marks a payload that is
    /// itself a complete rain meta document, which
    /// `OrderBuilderStateV1::extract_from_meta` recurses into, so the codec
    /// must carry such an item in both directions and leave its payload byte
    /// for byte intact.
    #[test]
    fn test_document_magic_item_carries_a_nested_document() -> Result<(), Error> {
        let inner = plain_item(KnownMagic::DotrainV1, vec![0x01]);
        let inner_doc = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![inner.clone()],
            KnownMagic::RainMetaDocumentV1,
        )?;
        let outer = plain_item(KnownMagic::RainMetaDocumentV1, inner_doc.clone());
        let outer_doc = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![outer.clone()],
            KnownMagic::RainMetaDocumentV1,
        )?;

        // Encoding can still write the document magic into an item's magic
        // position, and the payload really is a whole document. Decoding
        // refuses it anyway: a nested document is not a shape to descend into,
        // it is a corrupt meta, and the usable item inside does not rescue it.
        // rainlanguage/rain.metadata#204.
        assert!(RainMetaDocumentV1Item::cbor_decode(&outer_doc).is_err());
        assert_eq!(
            RainMetaDocumentV1Item::cbor_decode(&inner_doc)?,
            vec![inner]
        );
        Ok(())
    }

    /// Nesting is not a leaf meta type: the unpack layer rejects the document
    /// magic so that no payload conversion is ever handed a whole document.
    #[test]
    fn test_document_magic_item_is_not_unpackable() {
        assert!(matches!(
            KnownMeta::try_from(KnownMagic::RainMetaDocumentV1),
            Err(Error::UnsupportedMeta)
        ));
        assert!(matches!(
            plain_item(KnownMagic::RainMetaDocumentV1, vec![0x01]).unpack_into::<Vec<u8>>(),
            Err(Error::UnsupportedMeta)
        ));
    }

    /// unpack decodes the payload according to the content encoding.
    #[test]
    fn test_unpack_decodes_content_encoding() -> Result<(), Error> {
        let content = b"unpack me via deflate".to_vec();
        let packed = ContentEncoding::Deflate.encode(&content);
        assert_ne!(packed, content);
        let mut item = plain_item(KnownMagic::DotrainV1, packed);
        item.content_encoding = ContentEncoding::Deflate;
        assert_eq!(item.unpack()?, content);

        let item = plain_item(KnownMagic::DotrainV1, content.clone());
        assert_eq!(item.unpack()?, content);
        Ok(())
    }

    /// The 13 meta magics unpack; the document magic, the web data magic and
    /// the Oa magics are rejected with UnsupportedMeta.
    #[test]
    fn test_unpack_into_whitelist() {
        use strum::IntoEnumIterator;
        let supported = [
            KnownMagic::OpMetaV1,
            KnownMagic::DotrainV1,
            KnownMagic::RainlangV1,
            KnownMagic::SolidityAbiV2,
            KnownMagic::AuthoringMetaV1,
            KnownMagic::AuthoringMetaV2,
            KnownMagic::AddressList,
            KnownMagic::InterpreterCallerMetaV1,
            KnownMagic::ExpressionDeployerV2BytecodeV1,
            KnownMagic::DotrainSourceV1,
            KnownMagic::OrderBuilderStateV1,
            KnownMagic::RainlangSourceV1,
            KnownMagic::RaindexSignedContextOracleV1,
        ];
        for magic in supported {
            let unpacked: Vec<u8> = plain_item(magic, vec![0x61]).unpack_into().unwrap();
            assert_eq!(unpacked, vec![0x61], "{:?}", magic);
        }
        let unsupported = [
            KnownMagic::RainMetaDocumentV1,
            KnownMagic::WebDataV1,
            KnownMagic::OaSchema,
            KnownMagic::OaHashList,
            KnownMagic::OaStructure,
            KnownMagic::OaTokenImage,
            KnownMagic::OaTokenCredentialLinks,
        ];
        for magic in unsupported {
            let result: Result<Vec<u8>, Error> = plain_item(magic, vec![0x61]).unpack_into();
            assert!(matches!(result, Err(Error::UnsupportedMeta)), "{:?}", magic);
        }
        // together the two lists cover every variant
        assert_eq!(
            supported.len() + unsupported.len(),
            KnownMagic::iter().count()
        );
    }

    /// Invalid utf8 payloads error when unpacking into String rather than
    /// being replaced lossily.
    #[test]
    fn test_try_into_string_invalid_utf8_errors() {
        let item = plain_item(KnownMagic::DotrainV1, vec![0xff, 0xfe]);
        let result: Result<String, Error> = item.try_into();
        assert!(matches!(result, Err(Error::FromUtf8Error(_))));
    }

    /// Unpacking into Vec<u8> decodes the content encoding first.
    #[test]
    fn test_try_into_vec_decodes_encoding() -> Result<(), Error> {
        let content = b"raw bytecode bytes \x00\x01\x02".to_vec();
        let packed = ContentEncoding::Deflate.encode(&content);
        let mut item = plain_item(KnownMagic::ExpressionDeployerV2BytecodeV1, packed.clone());
        item.content_encoding = ContentEncoding::Deflate;
        let unpacked: Vec<u8> = item.try_into()?;
        assert_eq!(unpacked, content);
        assert_ne!(unpacked, packed);
        Ok(())
    }

    /// Deflate encode produces a zlib stream (RFC1950 CMF byte 0x78) that is
    /// actually compressed and roundtrips through decode.
    #[test]
    fn test_content_encoding_deflate_roundtrip() -> Result<(), Error> {
        let content = b"hello rain deflate fixture hello rain deflate fixture".to_vec();
        let encoded = ContentEncoding::Deflate.encode(&content);
        assert_ne!(encoded, content);
        assert_eq!(encoded[0], 0x78);
        assert_eq!(ContentEncoding::Deflate.decode(&encoded)?, content);
        Ok(())
    }

    /// None and Identity pass data through unchanged on encode and decode.
    #[test]
    fn test_content_encoding_passthrough() -> Result<(), Error> {
        let data = vec![0x00, 0xff, 0x10];
        for encoding in [ContentEncoding::None, ContentEncoding::Identity] {
            assert_eq!(encoding.encode(&data), data, "{:?}", encoding);
            assert_eq!(encoding.decode(&data)?, data, "{:?}", encoding);
        }
        Ok(())
    }

    /// Decode accepts a zlib stream and falls back to a raw deflate stream.
    /// Fixtures generated out of band from "hello rain deflate fixture".
    #[test]
    fn test_content_encoding_decode_fixtures() -> Result<(), Error> {
        let content = b"hello rain deflate fixture".to_vec();
        let zlib: Vec<u8> = vec![
            120, 156, 203, 72, 205, 201, 201, 87, 40, 74, 204, 204, 83, 72, 73, 77, 203, 73, 44,
            73, 85, 72, 203, 172, 40, 41, 45, 74, 5, 0, 132, 64, 9, 251,
        ];
        let raw: Vec<u8> = vec![
            203, 72, 205, 201, 201, 87, 40, 74, 204, 204, 83, 72, 73, 77, 203, 73, 44, 73, 85, 72,
            203, 172, 40, 41, 45, 74, 5, 0,
        ];
        assert_eq!(ContentEncoding::Deflate.decode(&zlib)?, content);
        assert_eq!(ContentEncoding::Deflate.decode(&raw)?, content);
        Ok(())
    }

    /// Data that is neither a zlib stream nor a raw deflate stream errors
    /// with InflateError instead of returning bytes.
    #[test]
    fn test_content_encoding_decode_garbage_errors() {
        let garbage = [0xffu8, 0xff, 0xff, 0xff];
        assert!(matches!(
            ContentEncoding::Deflate.decode(&garbage),
            Err(Error::InflateError(_))
        ));
    }

    /// The CLI-facing strum names for the content headers are kebab-case.
    #[test]
    fn test_content_headers_strum_names() {
        use std::str::FromStr;
        assert_eq!(
            ContentEncoding::from_str("deflate").unwrap(),
            ContentEncoding::Deflate
        );
        assert_eq!(
            ContentEncoding::from_str("identity").unwrap(),
            ContentEncoding::Identity
        );
        assert_eq!(
            ContentEncoding::from_str("none").unwrap(),
            ContentEncoding::None
        );
        assert_eq!(ContentEncoding::Deflate.to_string(), "deflate");
        assert_eq!(
            ContentType::from_str("octet-stream").unwrap(),
            ContentType::OctetStream
        );
        assert_eq!(ContentType::from_str("json").unwrap(), ContentType::Json);
        assert_eq!(ContentType::Json.to_string(), "json");
        assert_eq!(
            ContentLanguage::from_str("en").unwrap(),
            ContentLanguage::En
        );
    }

    /// Every documented meta magic maps to its KnownMeta while the document
    /// magic and the Oa magics are unsupported.
    #[test]
    fn test_known_meta_try_from_magic() {
        let cases: [(KnownMagic, KnownMeta); 13] = [
            (KnownMagic::OpMetaV1, KnownMeta::OpV1),
            (KnownMagic::DotrainV1, KnownMeta::DotrainV1),
            (KnownMagic::RainlangV1, KnownMeta::RainlangV1),
            (KnownMagic::SolidityAbiV2, KnownMeta::SolidityAbiV2),
            (KnownMagic::AuthoringMetaV1, KnownMeta::AuthoringMetaV1),
            (KnownMagic::AuthoringMetaV2, KnownMeta::AuthoringMetaV2),
            (KnownMagic::AddressList, KnownMeta::AddressList),
            (
                KnownMagic::InterpreterCallerMetaV1,
                KnownMeta::InterpreterCallerMetaV1,
            ),
            (
                KnownMagic::ExpressionDeployerV2BytecodeV1,
                KnownMeta::ExpressionDeployerV2BytecodeV1,
            ),
            (KnownMagic::RainlangSourceV1, KnownMeta::RainlangSourceV1),
            (KnownMagic::DotrainSourceV1, KnownMeta::DotrainSourceV1),
            (
                KnownMagic::OrderBuilderStateV1,
                KnownMeta::OrderBuilderStateV1,
            ),
            (
                KnownMagic::RaindexSignedContextOracleV1,
                KnownMeta::RaindexSignedContextOracleV1,
            ),
        ];
        for (magic, meta) in cases {
            assert_eq!(KnownMeta::try_from(magic).unwrap(), meta, "{:?}", magic);
        }
        for magic in [
            KnownMagic::RainMetaDocumentV1,
            KnownMagic::WebDataV1,
            KnownMagic::OaSchema,
            KnownMagic::OaHashList,
            KnownMagic::OaStructure,
            KnownMagic::OaTokenImage,
            KnownMagic::OaTokenCredentialLinks,
        ] {
            assert!(
                matches!(KnownMeta::try_from(magic), Err(Error::UnsupportedMeta)),
                "{:?}",
                magic
            );
        }
    }

    /// KnownMeta parses from and displays as the kebab-case names used by the
    /// CLI (validate --meta, build, schema show).
    #[test]
    fn test_known_meta_strum_parse_display() {
        use std::str::FromStr;
        assert_eq!(
            KnownMeta::from_str("solidity-abi-v2").unwrap(),
            KnownMeta::SolidityAbiV2
        );
        assert_eq!(
            KnownMeta::from_str("interpreter-caller-meta-v1").unwrap(),
            KnownMeta::InterpreterCallerMetaV1
        );
        assert_eq!(KnownMeta::SolidityAbiV2.to_string(), "solidity-abi-v2");
    }

    /// search() lowercases the hash before building the query variables.
    #[tokio::test]
    async fn test_search_lowercases_hash() {
        use httpmock::prelude::*;
        let (_, doc) = sample_authoring_doc();
        let hash_upper = format!("0x{}", "AB".repeat(32));
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .body_contains(hash_upper.to_ascii_lowercase());
            then.status(200).json_body(json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": hex::encode_prefixed(&doc)}}
            }));
        });
        let response = search(&hash_upper, &vec![server.url("/sg")]).await.unwrap();
        assert_eq!(response.bytes, doc);
        mock.assert();
    }

    /// search() queries every subgraph and the first success wins even when
    /// an earlier subgraph fails.
    #[tokio::test]
    async fn test_search_first_success_wins() {
        use httpmock::prelude::*;
        let (_, doc) = sample_authoring_doc();
        let bad = MockServer::start();
        let _bad_mock = bad.mock(|when, then| {
            when.method(POST);
            then.status(500).body("subgraph down");
        });
        let good = MockServer::start();
        let _good_mock = good.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": hex::encode_prefixed(&doc)}}
            }));
        });
        let response = search(
            &format!("0x{}", "11".repeat(32)),
            &vec![bad.url("/sg"), good.url("/sg")],
        )
        .await
        .unwrap();
        assert_eq!(response.bytes, doc);
    }

    /// An empty subgraph list has nothing to fan out to, so the search
    /// reports a miss rather than reaching futures::select_ok, which panics on
    /// an empty iterator.
    #[tokio::test]
    async fn test_search_empty_subgraphs_is_a_miss() {
        let hash = format!("0x{}", "33".repeat(32));
        assert!(matches!(
            search(&hash, &vec![]).await,
            Err(Error::NoRecordFound)
        ));
    }

    /// The erc165 gate short circuits: neither answer reaches the
    /// IDescribedByMetaV1 supportsInterface call, so the queued "true" is
    /// never consumed and cannot be mistaken for the contract's own answer.
    ///
    /// The two answers are not the same fact. A contract that says no is
    /// `Ok(false)`; a probe that could not finish is an error, because a
    /// transport failure is not a contract declining an interface. rain-erc
    /// makes that distinction itself - "callers can treat that as answer
    /// unknown rather than silently reading no support" - and an
    /// `unwrap_or(false)` here threw it away.
    #[tokio::test]
    async fn test_implements_erc165_gate_short_circuits() {
        let address = Address::random();

        // erc165 check1 answers false
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        assert!(!implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap());

        // erc165 probe errors. Getting an error back is itself the proof of
        // the short circuit: had the queued "true" been consumed by the
        // interface probe, this would be Ok(true).
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter.push_failure(ErrorPayload {
            code: -32000,
            message: "connection reset".into(),
            data: None,
        });
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        assert!(implements_i_described_by_meta_v1(&provider, address)
            .await
            .is_err());
    }

    /// An empty eth_call response is a contract that did not answer, which
    /// ERC-165 reads as "does not implement".
    #[tokio::test]
    async fn test_implements_empty_response_is_false() {
        let address = Address::random();
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        asserter.push_success(&"0x");
        assert!(!implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap());
    }

    /// A non-revert failure of the IDescribedByMetaV1 supportsInterface call is
    /// "answer unknown": it propagates as Err rather than reading as "does not
    /// implement".
    #[tokio::test]
    async fn test_implements_described_by_call_error_is_unknown() {
        let address = Address::random();
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        asserter.push_failure(ErrorPayload {
            code: -32005,
            message: "rate limit exceeded".into(),
            data: None,
        });
        let error = implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("rate limit exceeded"));
    }

    /// A non-empty IDescribedByMetaV1 supportsInterface response that does not
    /// decode as bool is a decode failure, so "answer unknown" rather than
    /// "does not implement".
    #[tokio::test]
    async fn test_implements_undecodable_response_is_unknown() {
        let address = Address::random();
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        asserter.push_success(&"0xdeadbeef");
        implements_i_described_by_meta_v1(&provider, address)
            .await
            .unwrap_err();
    }

    /// No constructor injects a subgraph the caller did not ask for, and a
    /// store with none resolves every network lookup to None rather than
    /// reaching the select_ok panic.
    #[tokio::test]
    async fn test_store_constructors_inject_no_subgraphs() {
        assert!(Store::new().subgraphs().is_empty());
        assert!(Store::default().subgraphs().is_empty());
        assert!(
            Store::create(&vec![], &MetaCache::default(), &HashMap::new())
                .subgraphs()
                .is_empty()
        );

        let hash = [0u8; 32];
        let mut store = Store::default();
        assert!(store.update(&hash).await.is_err());
    }

    /// create() takes only the given subgraphs, and keeps a dotrain uri only
    /// when its hash is present in the cache.
    ///
    /// This used to assert create() dropped a cache entry whose bytes did not
    /// hash to its key. That entry is no longer constructible: create() takes
    /// a [MetaCache], which has no way to hold one, so there is nothing left
    /// for create() to validate.
    #[test]
    fn test_store_create_validates_entries() {
        let (_, doc) = sample_authoring_doc();
        let good_hash = keccak256(&doc).0.to_vec();
        let mut cache = MetaCache::default();
        cache.insert_verified(&good_hash, doc.clone()).unwrap();
        let mut dotrain_cache = HashMap::new();
        dotrain_cache.insert("a.rain".to_string(), good_hash.clone());
        dotrain_cache.insert("missing.rain".to_string(), vec![0x44u8; 32]);

        let store = Store::create(
            &vec!["https://example.com/custom-sg".to_string()],
            &cache,
            &dotrain_cache,
        );

        assert_eq!(
            store.subgraphs(),
            &vec!["https://example.com/custom-sg".to_string()]
        );
        assert_eq!(store.get_meta(&good_hash), Some(&doc));
        assert_eq!(store.get_dotrain_hash("a.rain"), Some(&good_hash));
        assert_eq!(store.get_dotrain_hash("missing.rain"), None);
    }

    /// add_subgraphs skips urls already present.
    #[test]
    fn test_store_add_subgraphs_dedupe() {
        let mut store = Store::new();
        store.add_subgraphs(&vec!["sg-a".to_string()]);
        store.add_subgraphs(&vec!["sg-a".to_string(), "sg-b".to_string()]);
        assert_eq!(
            store.subgraphs(),
            &vec!["sg-a".to_string(), "sg-b".to_string()]
        );
    }

    /// set_dotrain on a fresh uri returns (new_hash, empty), keyed by the
    /// keccak of the cbor encoded DotrainV1 meta item, and every dotrain
    /// getter resolves it.
    #[test]
    fn test_store_dotrain_getters_and_set_fresh() {
        let mut store = Store::new();
        let text = "some dotrain content";
        let (hash, old) = store.set_dotrain(text, "file.rain", false).unwrap();
        assert!(old.is_empty());
        let expected_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(text.as_bytes()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let expected_bytes = expected_item.cbor_encode().unwrap();
        assert_eq!(hash, keccak256(&expected_bytes).0.to_vec());
        assert_eq!(store.get_dotrain_hash("file.rain"), Some(&hash));
        assert_eq!(store.get_dotrain_uri(&hash), Some(&"file.rain".to_string()));
        assert_eq!(store.get_dotrain_meta("file.rain"), Some(&expected_bytes));
        assert_eq!(store.get_dotrain_hash("other.rain"), None);
        assert_eq!(store.get_dotrain_uri(&[0u8; 32]), None);
        assert_eq!(store.get_dotrain_meta("other.rain"), None);
    }

    /// set_dotrain branches: same content keeps the meta and reports no old
    /// hash; different content remaps the uri and drops or keeps the old
    /// meta per keep_old.
    #[test]
    fn test_store_set_dotrain_branches() {
        let mut store = Store::new();
        let (hash_one, _) = store.set_dotrain("text one", "a.rain", false).unwrap();

        // same content again: same hash, no old hash, meta retained
        let (hash_same, old_same) = store.set_dotrain("text one", "a.rain", false).unwrap();
        assert_eq!(hash_same, hash_one);
        assert!(old_same.is_empty());
        assert!(store.get_meta(&hash_one).is_some());

        // different content, keep_old = false: remap and drop the old meta
        let (hash_two, old_two) = store.set_dotrain("text two", "a.rain", false).unwrap();
        assert_ne!(hash_two, hash_one);
        assert_eq!(old_two, hash_one);
        assert_eq!(store.get_dotrain_hash("a.rain"), Some(&hash_two));
        assert!(store.get_meta(&hash_one).is_none());
        assert!(store.get_meta(&hash_two).is_some());

        // different content, keep_old = true: old meta kept
        let (hash_three, old_three) = store.set_dotrain("text three", "a.rain", true).unwrap();
        assert_eq!(old_three, hash_two);
        assert_eq!(store.get_dotrain_hash("a.rain"), Some(&hash_three));
        assert!(store.get_meta(&hash_two).is_some());
        assert!(store.get_meta(&hash_three).is_some());
    }

    /// delete_dotrain removes the uri mapping and honors keep_meta for the
    /// cached meta bytes.
    #[test]
    fn test_store_delete_dotrain_keep_meta() {
        let mut store = Store::new();
        let (hash, _) = store.set_dotrain("dotrain body", "d.rain", false).unwrap();
        store.delete_dotrain("d.rain", false);
        assert_eq!(store.get_dotrain_hash("d.rain"), None);
        assert!(store.get_meta(&hash).is_none());

        let (hash_again, _) = store.set_dotrain("dotrain body", "d.rain", false).unwrap();
        store.delete_dotrain("d.rain", true);
        assert_eq!(store.get_dotrain_hash("d.rain"), None);
        assert!(store.get_meta(&hash_again).is_some());
    }

    /// merge keeps this store's entry in every map on a key collision, takes
    /// the keys it does not already hold, and unions the subgraphs.
    #[test]
    fn test_store_merge_semantics() {
        let mut ours = Store::new();
        let mut theirs = Store::new();

        // meta cache: two different metas cannot share a key - the key IS
        // their digest - so merge takes the other store's entry rather than
        // choosing between them
        let mine = b"mine".to_vec();
        let yours = b"yours".to_vec();
        let mine_hash = keccak256(&mine).0.to_vec();
        let yours_hash = keccak256(&yours).0.to_vec();
        ours.update_with(&mine_hash, &mine).unwrap();
        theirs.update_with(&yours_hash, &yours).unwrap();

        // same dotrain uri, different content
        let (hash_ours, _) = ours.set_dotrain("content a", "x.rain", false).unwrap();
        let (_hash_theirs, _) = theirs.set_dotrain("content b", "x.rain", false).unwrap();

        theirs.add_subgraphs(&vec!["sg-their".to_string()]);

        ours.merge(&theirs);

        assert_eq!(ours.get_meta(&mine_hash), Some(&mine));
        assert_eq!(ours.get_meta(&yours_hash), Some(&yours));
        // dotrain: existing uri mapping wins
        assert_eq!(ours.get_dotrain_hash("x.rain"), Some(&hash_ours));
        // subgraphs merged
        assert!(ours.subgraphs().contains(&"sg-their".to_string()));
    }

    /// update() stores the fetched bytes under the requested hash and each
    /// inner meta item under the keccak of its own encoding; update_check
    /// serves a cached hash without any network access.
    #[tokio::test]
    async fn test_store_update_and_update_check() {
        use httpmock::prelude::*;
        let authoring_meta: AuthoringMeta = serde_json::from_str(
            r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#,
        )
        .unwrap();
        let item_one = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(authoring_meta.abi_encode_validate().unwrap()),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let item_two = sample_dotrain_item();
        let doc = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![item_one.clone(), item_two.clone()],
            KnownMagic::RainMetaDocumentV1,
        )
        .unwrap();
        let requested = keccak256(&doc).0.to_vec();
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": hex::encode_prefixed(&doc)}}
            }));
        });
        let mut store = Store::new();
        store.add_subgraphs(&vec![server.url("/sg")]);
        let fetched = store.update(&requested).await.cloned().unwrap();
        assert_eq!(fetched, doc);
        assert_eq!(store.get_meta(&requested), Some(&doc));
        let inner_one = item_one.cbor_encode().unwrap();
        let inner_two = item_two.cbor_encode().unwrap();
        assert_eq!(
            store.get_meta(keccak256(&inner_one).0.as_ref()),
            Some(&inner_one)
        );
        assert_eq!(
            store.get_meta(keccak256(&inner_two).0.as_ref()),
            Some(&inner_two)
        );

        // update_check: cached hash short-circuits, no subgraphs needed
        let mut cached_store = Store::new();
        let bytes = b"standalone meta bytes".to_vec();
        let hash = keccak256(&bytes).0.to_vec();
        assert!(cached_store.update_with(&hash, &bytes).is_ok());
        assert_eq!(cached_store.update_check(&hash).await.unwrap(), &bytes);
    }

    /// update() applies the same keccak gate as update_with to the subgraph
    /// response, so bytes that do not hash to the requested hash poison
    /// neither the requested key nor the inner item keys.
    #[tokio::test]
    async fn test_store_update_rejects_hash_mismatch() {
        use httpmock::prelude::*;
        let (_, doc) = sample_authoring_doc();
        let requested = keccak256(b"the real content").0.to_vec();
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": hex::encode_prefixed(&doc)}}
            }));
        });
        let mut store = Store::new();
        store.add_subgraphs(&vec![server.url("/sg")]);
        assert!(store.update(&requested).await.is_err());
        assert!(store.get_meta(&requested).is_none());
        assert!(store.cache().is_empty());
        // the miss is not cached either, so update_check retries and misses again
        assert!(store.update_check(&requested).await.is_err());
    }

    /// Store::new() starts with no subgraphs, so every uncached lookup that
    /// reaches the network on it resolves to None instead of panicking.
    #[tokio::test]
    async fn test_store_no_subgraphs_lookups_return_none() {
        let hash = [0u8; 32];
        let mut store = Store::new();
        assert!(store.update(&hash).await.is_err());
        assert!(store.update_check(&hash).await.is_err());
        assert!(store.cache().is_empty());
    }

    /// update_with enforces keccak(bytes) == hash, leaves an existing entry
    /// untouched, and unpacks inner items only for RainMetaDocumentV1
    /// prefixed bytes.
    #[test]
    fn test_store_update_with_validation_and_content() {
        // hash mismatch rejected
        let mut store = Store::new();
        let bytes = b"payload bytes".to_vec();
        let wrong_hash = vec![0x99u8; 32];
        // A mismatch is CorruptRecord, not NoRecordFound: the responder
        // answered about one hash with bytes that are another, which is not
        // the same fact as the hash being absent. #234 and #213 settled that
        // distinction for the query layer.
        match store.update_with(&wrong_hash, &bytes).unwrap_err() {
            Error::CorruptRecord(message) => {
                assert!(
                    message.contains(&hex::encode_prefixed(&wrong_hash)),
                    "{}",
                    message
                )
            }
            other => panic!("expected CorruptRecord, got {:?}", other),
        }
        assert!(store.get_meta(&wrong_hash).is_none());
        // valid pair stored
        let hash = keccak256(&bytes).0.to_vec();
        assert_eq!(store.update_with(&hash, &bytes).unwrap(), &bytes);

        // an already cached key returns its entry rather than inserting again.
        // Note what is no longer expressible here: the old version of this
        // block seeded one hash with unrelated bytes and asserted a later write
        // did not overwrite them. Different bytes cannot share a key when the
        // key is their digest, so "overwritten with something else" is not a
        // state [MetaCache] can be in.
        let mut seeded = Store::new();
        let planted = b"planted value".to_vec();
        let planted_hash = keccak256(&planted).0.to_vec();
        seeded.update_with(&planted_hash, &planted).unwrap();
        assert_eq!(seeded.cache().len(), 1);
        assert_eq!(
            seeded.update_with(&planted_hash, &planted).unwrap(),
            &planted
        );
        assert_eq!(seeded.cache().len(), 1);

        // prefixed document: inner item stored under keccak of its encoding
        let (_, doc) = sample_authoring_doc();
        let doc_hash = keccak256(&doc).0.to_vec();
        let mut doc_store = Store::new();
        assert!(doc_store.update_with(&doc_hash, &doc).is_ok());
        let inner = doc[8..].to_vec();
        assert_eq!(store_inner_lookup(&doc_store, &inner), Some(inner.clone()));

        // bare cbor sequence without the document prefix: no inner extraction
        let item_a = sample_dotrain_item().cbor_encode().unwrap();
        let (_, doc_b) = sample_authoring_doc();
        let item_b = doc_b[8..].to_vec();
        let seq = [item_a.clone(), item_b].concat();
        let seq_hash = keccak256(&seq).0.to_vec();
        let mut seq_store = Store::new();
        assert!(seq_store.update_with(&seq_hash, &seq).is_ok());
        assert_eq!(store_inner_lookup(&seq_store, &item_a), None);
    }

    fn store_inner_lookup(store: &Store, inner_encoded: &[u8]) -> Option<Vec<u8>> {
        store.get_meta(keccak256(inner_encoded).0.as_ref()).cloned()
    }

    /// bytes32_to_str propagates invalid utf8 as an error instead of
    /// swallowing it.
    #[test]
    fn test_bytes32_to_str_invalid_utf8() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0xf0;
        bytes[1] = 0x28;
        bytes[2] = 0x8c;
        bytes[3] = 0x28;
        assert!(matches!(bytes32_to_str(&bytes), Err(Error::Utf8Error(_))));
        let no_nul = [0xffu8; 32];
        assert!(matches!(bytes32_to_str(&no_nul), Err(Error::Utf8Error(_))));
    }
}
