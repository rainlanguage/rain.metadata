use super::error::Error;
use alloy::primitives::{hex, keccak256};
use futures::future;
use graphql_client::GraphQLQuery;
use rain_metadata_bindings::IDescribedByMetaV1;
use reqwest::Client;
use serde::de::{Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::{collections::HashMap, convert::TryFrom, fmt::Debug, sync::Arc};
use strum::{EnumIter, EnumString};
use types::authoring::v1::AuthoringMeta;
use alloy::sol_types::private::Address;
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::SolCall;
use rain_erc::erc165::{IERC165, XorSelectors, supports_erc165};

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
            KnownMagic::OpMetaV1 => Ok(KnownMeta::OpV1),
            KnownMagic::DotrainV1 => Ok(KnownMeta::DotrainV1),
            KnownMagic::RainlangV1 => Ok(KnownMeta::RainlangV1),
            KnownMagic::SolidityAbiV2 => Ok(KnownMeta::SolidityAbiV2),
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
        let mut track: Vec<usize> = vec![];
        let mut metas: Vec<RainMetaDocumentV1Item> = vec![];
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
        while match serde_cbor::Value::deserialize(&mut deserializer) {
            Ok(cbor_map) => {
                track.push(deserializer.byte_offset());
                match serde_cbor::value::from_value(cbor_map) {
                    Ok(meta) => metas.push(meta),
                    Err(error) => Err(Error::SerdeCborError(error))?,
                };
                true
            }
            Err(error) => {
                if error.is_eof() {
                    if error.offset() == len as u64 {
                        false
                    } else {
                        Err(Error::SerdeCborError(error))?
                    }
                } else {
                    Err(Error::SerdeCborError(error))?
                }
            }
        } {}

        if metas.is_empty()
            || track.is_empty()
            || track.len() != metas.len()
            || len != track[track.len() - 1]
        {
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

impl<'de> Deserialize<'de> for RainMetaDocumentV1Item {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EncodedMap;
        impl<'de> Visitor<'de> for EncodedMap {
            type Value = RainMetaDocumentV1Item;

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
                while match map.next_key::<u64>() {
                    Ok(Some(key)) => {
                        match key {
                            0 => payload = Some(map.next_value()?),
                            1 => magic = Some(map.next_value()?),
                            2 => content_type = Some(map.next_value()?),
                            3 => content_encoding = Some(map.next_value()?),
                            4 => content_language = Some(map.next_value()?),
                            OA_SCHEMA_KEY => schema = Some(map.next_value()?),
                            // the map structure exists so later conventions can
                            // add indexes that older tooling adopts "or not" in
                            // a backwards compatible way, so an index this
                            // version does not know is skipped, not an error
                            _ => {
                                map.next_value::<serde::de::IgnoredAny>()?;
                            }
                        };
                        true
                    }
                    Ok(None) => false,
                    Err(error) => Err(error)?,
                } {}
                let payload = payload.ok_or_else(|| serde::de::Error::missing_field("payload"))?;
                let magic = match magic
                    .ok_or_else(|| serde::de::Error::missing_field("magic number"))?
                    .try_into()
                {
                    Ok(m) => m,
                    _ => Err(serde::de::Error::custom("unknown magic number"))?,
                };
                let content_type = content_type.unwrap_or(ContentType::None);
                let content_encoding = content_encoding.unwrap_or(ContentEncoding::None);
                let content_language = content_language.unwrap_or(ContentLanguage::None);

                Ok(RainMetaDocumentV1Item {
                    payload,
                    magic,
                    content_type,
                    content_encoding,
                    content_language,
                    schema,
                })
            }
        }
        deserializer.deserialize_map(EncodedMap)
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

/// searches for an ExpressionDeployer matching the given hash in given subgraphs urls
pub async fn search_deployer(
    hash: &str,
    subgraphs: &Vec<String>,
) -> Result<DeployerResponse, Error> {
    // future::select_ok panics on an empty iterator.
    if subgraphs.is_empty() {
        return Err(Error::NoRecordFound);
    }
    let request_body = query::DeployerQuery::build_query(query::deployer_query::Variables {
        hash: Some(hash.to_ascii_lowercase()),
    });
    let mut promises = vec![];

    let client = Arc::new(Client::builder().build().map_err(Error::ReqwestError)?);
    for url in subgraphs {
        promises.push(Box::pin(query::process_deployer_query(
            client.clone(),
            &request_body,
            url,
        )));
    }
    let response_value = future::select_ok(promises.drain(..)).await?.0;
    Ok(response_value)
}

/// checks if the given contract implements IDescribeByMetaV1 interface
pub async fn implements_i_described_by_meta_v1<P: Provider>(
    provider: &P,
    contract_address: Address,
) -> bool {
    if !supports_erc165(provider, contract_address)
        .await
        .unwrap_or(false)
    {
        return false;
    }

    let interface_id_res = IDescribedByMetaV1::IDescribedByMetaV1Calls::xor_selectors();
    if interface_id_res.is_err() {
        return false;
    }

    let call = IERC165::supportsInterfaceCall {
        interfaceID: interface_id_res.unwrap().into(),
    };
    let tx = TransactionRequest::default()
        .to(contract_address)
        .input(call.abi_encode().into());
    match provider.call(tx).await {
        Ok(bytes) => IERC165::supportsInterfaceCall::abi_decode_returns(&bytes).unwrap_or(false),
        Err(_) => false,
    }
}

/// All required NPE2 ExpressionDeployer data for reproducing it on a local evm
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NPE2Deployer {
    /// constructor meta hash
    #[serde(with = "serde_bytes")]
    pub meta_hash: Vec<u8>,
    /// constructor meta bytes
    #[serde(with = "serde_bytes")]
    pub meta_bytes: Vec<u8>,
    /// RainterpreterExpressionDeployerNPE2 contract bytecode
    #[serde(with = "serde_bytes")]
    pub bytecode: Vec<u8>,
    /// RainterpreterParserNPE2 contract bytecode
    #[serde(with = "serde_bytes")]
    pub parser: Vec<u8>,
    /// RainterpreterStoreNPE2 contract bytecode
    #[serde(with = "serde_bytes")]
    pub store: Vec<u8>,
    /// RainterpreterNPE2 contract bytecode
    #[serde(with = "serde_bytes")]
    pub interpreter: Vec<u8>,
    /// RainterpreterExpressionDeployerNPE2 authoring meta
    pub authoring_meta: Option<AuthoringMeta>,
}

impl NPE2Deployer {
    pub fn is_corrupt(&self) -> bool {
        if self.meta_hash.is_empty() {
            return true;
        }
        if self.meta_bytes.is_empty() {
            return true;
        }
        if self.bytecode.is_empty() {
            return true;
        }
        if self.parser.is_empty() {
            return true;
        }
        if self.store.is_empty() {
            return true;
        }
        if self.interpreter.is_empty() {
            return true;
        }
        false
    }
}

/// # Meta Storage(CAS)
///
/// In-memory CAS (content addressed storage) for Rain metadata which basically stores
/// k/v pairs of meta hash, meta bytes and ExpressionDeployer reproducible data as well
/// as providing functionalities to easliy read/write to the CAS.
///
/// Hashes are normal bytes and meta bytes are valid cbor encoded as data bytes.
/// ExpressionDeployers data are in form of a struct mapped to deployedBytecode meta hash
/// and deploy transaction hash.
///
/// ## Examples
///
/// ```
/// use rain_metadata::Store;
/// use std::collections::HashMap;
///
/// // to instantiate with an empty subgraph list
/// let mut store = Store::new();
///
/// // or to instantiate with initial values
/// let mut store = Store::create(
///     &vec!["sg-url-1".to_string()],
///     &HashMap::new(),
///     &HashMap::new(),
///     &HashMap::new(),
/// );
///
/// // add a new subgraph endpoint url to the subgraph list
/// store.add_subgraphs(&vec!["sg-url-2".to_string()]);
///
/// // merge another Store into this one
/// store.merge(&Store::new());
///
/// // updates the meta store with a new meta hash and bytes
/// let hash = vec![0u8, 1u8, 2u8];
/// store.update_with(&hash, &vec![0u8, 1u8]);
///
/// // `Store::update(&hash)` is async; it searches each subgraph for `hash` and
/// // populates the cache with the result. Call it from an async context with `.await`.
///
/// // to get a record from the store
/// let _meta = store.get_meta(&hash);
///
/// // to get a deployer record from the store
/// let _deployer_record = store.get_deployer(&hash);
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
    cache: HashMap<Vec<u8>, Vec<u8>>,
    dotrain_cache: HashMap<String, Vec<u8>>,
    deployer_cache: HashMap<Vec<u8>, NPE2Deployer>,
    deployer_hash_map: HashMap<Vec<u8>, Vec<u8>>,
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
            cache: HashMap::new(),
            dotrain_cache: HashMap::new(),
            deployer_cache: HashMap::new(),
            deployer_hash_map: HashMap::new(),
        }
    }

    /// creates new instance of Store with given initial values
    /// it checks the validity of each item of the provided values and only stores those that are valid
    pub fn create(
        subgraphs: &Vec<String>,
        cache: &HashMap<Vec<u8>, Vec<u8>>,
        deployer_cache: &HashMap<Vec<u8>, NPE2Deployer>,
        dotrain_cache: &HashMap<String, Vec<u8>>,
    ) -> Store {
        let mut store = Store::new();
        store.add_subgraphs(subgraphs);
        for (hash, bytes) in cache {
            store.update_with(hash, bytes);
        }
        for (hash, deployer) in deployer_cache {
            store.set_deployer(hash, deployer, None);
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
    pub fn cache(&self) -> &HashMap<Vec<u8>, Vec<u8>> {
        &self.cache
    }

    /// get the corresponding meta bytes of the given hash if it exists
    pub fn get_meta(&self, hash: &[u8]) -> Option<&Vec<u8>> {
        self.cache.get(hash)
    }

    /// getter method for the whole authoring meta cache
    pub fn deployer_cache(&self) -> &HashMap<Vec<u8>, NPE2Deployer> {
        &self.deployer_cache
    }

    /// get the corresponding DeployerNPRecord of the given deployer hash if it exists
    pub fn get_deployer(&self, hash: &[u8]) -> Option<&NPE2Deployer> {
        if self.deployer_cache.contains_key(hash) {
            self.deployer_cache.get(hash)
        } else if let Some(h) = self.deployer_hash_map.get(hash) {
            self.deployer_cache.get(h)
        } else {
            None
        }
    }

    /// searches for DeployerNPRecord in the subgraphs given the deployer hash
    pub async fn search_deployer(&mut self, hash: &[u8]) -> Option<&NPE2Deployer> {
        match search_deployer(&hex::encode_prefixed(hash), &self.subgraphs).await {
            Ok(res) => {
                self.cache
                    .insert(res.meta_hash.clone(), res.meta_bytes.clone());
                let authoring_meta = res.get_authoring_meta();
                self.deployer_cache.insert(
                    res.bytecode_meta_hash.clone(),
                    NPE2Deployer {
                        meta_hash: res.meta_hash.clone(),
                        meta_bytes: res.meta_bytes,
                        bytecode: res.bytecode,
                        parser: res.parser,
                        store: res.store,
                        interpreter: res.interpreter,
                        authoring_meta,
                    },
                );
                self.deployer_hash_map.insert(res.tx_hash, res.meta_hash);
                self.deployer_cache.get(hash)
            }
            Err(_e) => None,
        }
    }

    /// if the NPE2Deployer record already is cached it returns it immediately else
    /// searches for NPE2Deployer in the subgraphs given the deployer hash
    pub async fn search_deployer_check(&mut self, hash: &[u8]) -> Option<&NPE2Deployer> {
        if self.deployer_cache.contains_key(hash) {
            self.get_deployer(hash)
        } else if self.deployer_hash_map.contains_key(hash) {
            let b_hash = self.deployer_hash_map.get(hash).unwrap();
            self.get_deployer(b_hash)
        } else {
            self.search_deployer(hash).await
        }
    }

    /// sets deployer record from the deployer query response
    pub fn set_deployer_from_query_response(
        &mut self,
        deployer_query_response: DeployerResponse,
    ) -> NPE2Deployer {
        let authoring_meta = deployer_query_response.get_authoring_meta();
        let tx_hash = deployer_query_response.tx_hash;
        let bytecode_meta_hash = deployer_query_response.bytecode_meta_hash;
        let result = NPE2Deployer {
            meta_hash: deployer_query_response.meta_hash.clone(),
            meta_bytes: deployer_query_response.meta_bytes,
            bytecode: deployer_query_response.bytecode,
            parser: deployer_query_response.parser,
            store: deployer_query_response.store,
            interpreter: deployer_query_response.interpreter,
            authoring_meta,
        };
        self.cache
            .insert(deployer_query_response.meta_hash, result.meta_bytes.clone());
        self.deployer_hash_map
            .insert(tx_hash, bytecode_meta_hash.clone());
        self.deployer_cache
            .insert(bytecode_meta_hash, result.clone());
        result
    }

    /// sets NPE2Deployer record
    /// skips if the given hash is invalid
    pub fn set_deployer(
        &mut self,
        hash: &[u8],
        npe2_deployer: &NPE2Deployer,
        tx_hash: Option<&[u8]>,
    ) {
        self.cache.insert(
            npe2_deployer.meta_hash.clone(),
            npe2_deployer.meta_bytes.clone(),
        );
        self.deployer_cache
            .insert(hash.to_vec(), npe2_deployer.clone());
        if let Some(v) = tx_hash {
            self.deployer_hash_map.insert(v.to_vec(), hash.to_vec());
        }
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
        for (hash, bytes) in &other.cache {
            if !self.cache.contains_key(hash) {
                self.cache.insert(hash.clone(), bytes.clone());
            }
        }
        for (hash, deployer) in &other.deployer_cache {
            if !self.deployer_cache.contains_key(hash) {
                self.deployer_cache.insert(hash.clone(), deployer.clone());
            }
        }
        for (tx_hash, hash) in &other.deployer_hash_map {
            if !self.deployer_hash_map.contains_key(tx_hash) {
                self.deployer_hash_map.insert(tx_hash.clone(), hash.clone());
            }
        }
        for (uri, hash) in &other.dotrain_cache {
            if !self.dotrain_cache.contains_key(uri) {
                self.dotrain_cache.insert(uri.clone(), hash.clone());
            }
        }
    }

    /// updates the meta cache by searching through all subgraphs for the given hash
    /// returns the reference to the meta bytes in the cache if it was found
    pub async fn update(&mut self, hash: &[u8]) -> Option<&Vec<u8>> {
        if let Ok(meta) = search(&hex::encode_prefixed(hash), &self.subgraphs).await {
            self.store_content(&meta.bytes);
            self.cache.insert(hash.to_vec(), meta.bytes);
            self.get_meta(hash)
        } else {
            None
        }
    }

    /// first checks if the meta is stored, if not will perform update()
    pub async fn update_check(&mut self, hash: &[u8]) -> Option<&Vec<u8>> {
        if !self.cache.contains_key(hash) {
            self.update(hash).await
        } else {
            self.get_meta(hash)
        }
    }

    /// updates the meta cache by the given hash and meta bytes, checks the hash to bytes
    /// validity returns the reference to the bytes if the updated meta bytes contained any
    pub fn update_with(&mut self, hash: &[u8], bytes: &[u8]) -> Option<&Vec<u8>> {
        if !self.cache.contains_key(hash) {
            if keccak256(bytes).0 == hash {
                self.store_content(bytes);
                self.cache.insert(hash.to_vec(), bytes.to_vec());
                self.cache.get(hash)
            } else {
                None
            }
        } else {
            self.get_meta(hash)
        }
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
                self.cache.insert(new_hash.clone(), bytes);
                Ok((new_hash, vec![]))
            } else {
                self.cache.insert(new_hash.clone(), bytes);
                self.dotrain_cache.insert(uri.to_string(), new_hash.clone());
                if !keep_old {
                    self.cache.remove(&old_hash);
                }
                Ok((new_hash, old_hash))
            }
        } else {
            self.dotrain_cache.insert(uri.to_string(), new_hash.clone());
            self.cache.insert(new_hash.clone(), bytes);
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
                        self.cache
                            .insert(keccak256(&encoded_bytes).0.to_vec(), encoded_bytes);
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
        let result = implements_i_described_by_meta_v1(&provider, address).await;
        assert!(result);

        // mock a false response for implements IDescribedByMetaV1
        let (asserter, provider) = new_server_client().await;
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        let result = implements_i_described_by_meta_v1(&provider, address).await;
        assert!(!result);

        // mock a revert response for implements IDescribedByMetaV1
        let (asserter, provider) = new_server_client().await;
        asserter.push_failure(ErrorPayload {
            code: -32003,
            message: "execution reverted".into(),
            data: Some(serde_json::value::to_raw_value(&json!("0x00")).unwrap()),
        });
        let result = implements_i_described_by_meta_v1(&provider, address).await;
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

    /// An unknown key is skipped, never counted as one of the mandatory keys
    #[test]
    fn unknown_map_key_does_not_stand_in_for_a_mandatory_key() {
        let mut bytes: Vec<u8> = vec![0xa2, 0x05, 0x07, 0x01, 0x1b];
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::SerdeCborError(_))
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

    /// A map without the mandatory payload key 0 must not decode.
    #[test]
    fn test_cbor_decode_missing_payload_errors() {
        let mut bytes: Vec<u8> = vec![0xa1, 0x01, 0x1b]; // {1: DotrainV1}
        bytes.extend_from_slice(&KnownMagic::DotrainV1.to_prefix_bytes());
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::SerdeCborError(_))
        ));
    }

    /// A map without the mandatory magic key 1 must not decode.
    #[test]
    fn test_cbor_decode_missing_magic_errors() {
        let bytes: Vec<u8> = vec![0xa1, 0x00, 0x41, 0x01]; // {0: h'01'}
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::SerdeCborError(_))
        ));
    }

    /// A map carrying an unknown magic number value must not decode.
    #[test]
    fn test_cbor_decode_unknown_magic_errors() {
        let mut bytes: Vec<u8> = vec![0xa2, 0x00, 0x41, 0x01, 0x01, 0x1b];
        bytes.extend_from_slice(&0xdeadbeefdeadbeefu64.to_be_bytes());
        assert!(matches!(
            RainMetaDocumentV1Item::cbor_decode(&bytes),
            Err(Error::SerdeCborError(_))
        ));
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
        assert_eq!(KnownMeta::from_str("op-v1").unwrap(), KnownMeta::OpV1);
        assert_eq!(
            KnownMeta::from_str("solidity-abi-v2").unwrap(),
            KnownMeta::SolidityAbiV2
        );
        assert_eq!(
            KnownMeta::from_str("interpreter-caller-meta-v1").unwrap(),
            KnownMeta::InterpreterCallerMetaV1
        );
        assert_eq!(KnownMeta::SolidityAbiV2.to_string(), "solidity-abi-v2");
        assert_eq!(KnownMeta::OpV1.to_string(), "op-v1");
    }

    fn sample_deployer(meta_hash: &[u8], meta_bytes: &[u8]) -> NPE2Deployer {
        NPE2Deployer {
            meta_hash: meta_hash.to_vec(),
            meta_bytes: meta_bytes.to_vec(),
            bytecode: vec![0xb1],
            parser: vec![0xb2],
            store: vec![0xb3],
            interpreter: vec![0xb4],
            authoring_meta: None,
        }
    }

    fn deployer_json_body(
        meta_hash_hex: &str,
        meta_bytes_hex: &str,
        tx_hex: &str,
        bytecode_meta_id_hex: &str,
    ) -> serde_json::Value {
        json!({
            "data": {
                "expressionDeployers": [{
                    "constructorMetaHash": meta_hash_hex,
                    "constructorMeta": meta_bytes_hex,
                    "deployTransaction": {"id": tx_hex},
                    "bytecode": "0x01",
                    "parser": {"parser": {"deployedBytecode": "0x02"}},
                    "store": {"store": {"deployedBytecode": "0x03"}},
                    "interpreter": {"interpreter": {"deployedBytecode": "0x04"}},
                    "meta": [{"__typename": "RainMetaV1", "id": bytecode_meta_id_hex}]
                }]
            }
        })
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

    /// search_deployer() lowercases the hash before building the query
    /// variables.
    #[tokio::test]
    async fn test_search_deployer_lowercases_hash() {
        use httpmock::prelude::*;
        let (_, doc) = sample_authoring_doc();
        let meta_hash_hex = hex::encode_prefixed(keccak256(&doc).0);
        let hash_upper = format!("0x{}", "CD".repeat(32));
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .body_contains(hash_upper.to_ascii_lowercase());
            then.status(200).json_body(deployer_json_body(
                &meta_hash_hex,
                &hex::encode_prefixed(&doc),
                &format!("0x{}", "77".repeat(32)),
                &meta_hash_hex,
            ));
        });
        let response = search_deployer(&hash_upper, &vec![server.url("/sg")])
            .await
            .unwrap();
        assert_eq!(response.meta_bytes, doc);
        assert_eq!(response.bytecode, vec![0x01]);
        mock.assert();
    }

    /// search_deployer() queries every subgraph and the first success wins
    /// even when an earlier subgraph fails.
    #[tokio::test]
    async fn test_search_deployer_first_success_wins() {
        use httpmock::prelude::*;
        let (_, doc) = sample_authoring_doc();
        let meta_hash_hex = hex::encode_prefixed(keccak256(&doc).0);
        let bad = MockServer::start();
        let _bad_mock = bad.mock(|when, then| {
            when.method(POST);
            then.status(500).body("subgraph down");
        });
        let good = MockServer::start();
        let _good_mock = good.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(deployer_json_body(
                &meta_hash_hex,
                &hex::encode_prefixed(&doc),
                &format!("0x{}", "77".repeat(32)),
                &meta_hash_hex,
            ));
        });
        let response = search_deployer(
            &format!("0x{}", "22".repeat(32)),
            &vec![bad.url("/sg"), good.url("/sg")],
        )
        .await
        .unwrap();
        assert_eq!(response.meta_bytes, doc);
    }

    /// An empty subgraph list has nothing to fan out to, so both searches
    /// report a miss rather than reaching futures::select_ok, which panics on
    /// an empty iterator.
    #[tokio::test]
    async fn test_search_empty_subgraphs_is_a_miss() {
        let hash = format!("0x{}", "33".repeat(32));
        assert!(matches!(
            search(&hash, &vec![]).await,
            Err(Error::NoRecordFound)
        ));
        assert!(matches!(
            search_deployer(&hash, &vec![]).await,
            Err(Error::NoRecordFound)
        ));
    }

    /// When the erc165 probe answers false or errors, the result is false
    /// WITHOUT making the IDescribedByMetaV1 supportsInterface call: a queued
    /// "true" response must never be consumed.
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
        assert!(!implements_i_described_by_meta_v1(&provider, address).await);

        // erc165 probe errors
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter.push_failure(ErrorPayload {
            code: -32000,
            message: "connection reset".into(),
            data: None,
        });
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        assert!(!implements_i_described_by_meta_v1(&provider, address).await);
    }

    /// An eth_call response that does not decode as bool must read as "does
    /// not implement", not silently as true.
    #[tokio::test]
    async fn test_implements_undecodable_response_is_false() {
        let address = Address::random();
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        asserter.push_success(&"0x");
        assert!(!implements_i_described_by_meta_v1(&provider, address).await);
    }

    /// Each of the six required fields independently marks the record
    /// corrupt when empty; a fully populated record is not corrupt.
    #[test]
    fn test_npe2_deployer_is_corrupt_per_field() {
        let full = NPE2Deployer {
            meta_hash: vec![1],
            meta_bytes: vec![2],
            bytecode: vec![3],
            parser: vec![4],
            store: vec![5],
            interpreter: vec![6],
            authoring_meta: None,
        };
        assert!(!full.is_corrupt());
        for field in 0..6usize {
            let mut record = full.clone();
            match field {
                0 => record.meta_hash = vec![],
                1 => record.meta_bytes = vec![],
                2 => record.bytecode = vec![],
                3 => record.parser = vec![],
                4 => record.store = vec![],
                5 => record.interpreter = vec![],
                _ => unreachable!(),
            }
            assert!(record.is_corrupt(), "empty field {} must corrupt", field);
        }
    }

    /// No constructor injects a subgraph the caller did not ask for, and a
    /// store with none resolves every network lookup to None rather than
    /// reaching the select_ok panic.
    #[tokio::test]
    async fn test_store_constructors_inject_no_subgraphs() {
        assert!(Store::new().subgraphs().is_empty());
        assert!(Store::default().subgraphs().is_empty());
        assert!(
            Store::create(&vec![], &HashMap::new(), &HashMap::new(), &HashMap::new())
                .subgraphs()
                .is_empty()
        );

        let hash = [0u8; 32];
        let mut store = Store::default();
        assert!(store.update(&hash).await.is_none());
        assert!(store.search_deployer(&hash).await.is_none());
    }

    /// create() takes only the given subgraphs, validates cache entries via
    /// the keccak gate, and keeps a dotrain uri only when its hash is present
    /// in the cache.
    #[test]
    fn test_store_create_validates_entries() {
        let (_, doc) = sample_authoring_doc();
        let good_hash = keccak256(&doc).0.to_vec();
        let bad_hash = vec![0xEEu8; 32];
        let mut cache = HashMap::new();
        cache.insert(good_hash.clone(), doc.clone());
        cache.insert(bad_hash.clone(), b"does not hash to bad_hash".to_vec());
        let mut deployer_cache = HashMap::new();
        let deployer = sample_deployer(&[0xAA; 32], b"dep-meta");
        let deployer_key = vec![0x33u8; 32];
        deployer_cache.insert(deployer_key.clone(), deployer.clone());
        let mut dotrain_cache = HashMap::new();
        dotrain_cache.insert("a.rain".to_string(), good_hash.clone());
        dotrain_cache.insert("missing.rain".to_string(), vec![0x44u8; 32]);

        let store = Store::create(
            &vec!["https://example.com/custom-sg".to_string()],
            &cache,
            &deployer_cache,
            &dotrain_cache,
        );

        assert_eq!(
            store.subgraphs(),
            &vec!["https://example.com/custom-sg".to_string()]
        );
        assert_eq!(store.get_meta(&good_hash), Some(&doc));
        assert_eq!(store.get_meta(&bad_hash), None);
        assert_eq!(store.get_deployer(&deployer_key), Some(&deployer));
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

    /// get_deployer resolves a direct cache hit, then the tx-hash
    /// indirection, then None; set_deployer populates all three maps.
    #[test]
    fn test_store_get_deployer_lookup_chain() {
        let mut store = Store::new();
        let deployer = sample_deployer(&[0xAB; 32], b"dep-meta-bytes");
        let key = vec![0x01u8; 32];
        let tx = vec![0x02u8; 32];
        store.set_deployer(&key, &deployer, Some(&tx));
        assert_eq!(store.get_deployer(&key), Some(&deployer));
        assert_eq!(store.get_deployer(&tx), Some(&deployer));
        assert_eq!(store.get_deployer(&[0x03u8; 32]), None);
        assert_eq!(
            store.get_meta(&deployer.meta_hash),
            Some(&deployer.meta_bytes)
        );
    }

    /// A successful subgraph search populates the meta cache, the deployer
    /// cache keyed by the bytecode meta hash, and the tx-hash map, and
    /// returns the record for the searched hash.
    #[tokio::test]
    async fn test_store_search_deployer_populates_caches() {
        use httpmock::prelude::*;
        let (authoring_meta, doc) = sample_authoring_doc();
        let meta_hash = keccak256(&doc).0.to_vec();
        let meta_hash_hex = hex::encode_prefixed(&meta_hash);
        let tx = vec![0x77u8; 32];
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(deployer_json_body(
                &meta_hash_hex,
                &hex::encode_prefixed(&doc),
                &hex::encode_prefixed(&tx),
                &meta_hash_hex,
            ));
        });
        let mut store = Store::new();
        store.add_subgraphs(&vec![server.url("/sg")]);

        let record = store.search_deployer(&meta_hash).await.cloned().unwrap();
        assert_eq!(record.meta_hash, meta_hash);
        assert_eq!(record.meta_bytes, doc);
        assert_eq!(record.bytecode, vec![0x01]);
        assert_eq!(record.parser, vec![0x02]);
        assert_eq!(record.store, vec![0x03]);
        assert_eq!(record.interpreter, vec![0x04]);
        assert_eq!(record.authoring_meta, Some(authoring_meta));
        assert_eq!(store.get_meta(&meta_hash), Some(&doc));
        assert_eq!(store.get_deployer(&tx), Some(&record));
    }

    /// A failed subgraph search returns None and stores nothing.
    #[tokio::test]
    async fn test_store_search_deployer_error_returns_none() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(500).body("subgraph down");
        });
        let mut store = Store::new();
        store.add_subgraphs(&vec![server.url("/sg")]);
        assert!(store.search_deployer(&[0x0Du8; 32]).await.is_none());
        assert!(store.cache().is_empty());
        assert!(store.deployer_cache().is_empty());
    }

    /// search_deployer_check returns from the deployer cache or the tx-hash
    /// map without any network round trip, and only falls back to the
    /// subgraphs when neither hits.
    #[tokio::test]
    async fn test_store_search_deployer_check_branches() {
        use httpmock::prelude::*;
        // cached branches: no subgraphs registered at all
        let mut store = Store::new();
        let deployer = sample_deployer(&[0xAC; 32], b"cached-meta");
        let key = vec![0x11u8; 32];
        let tx = vec![0x22u8; 32];
        store.set_deployer(&key, &deployer, Some(&tx));
        assert_eq!(store.search_deployer_check(&key).await, Some(&deployer));
        assert_eq!(store.search_deployer_check(&tx).await, Some(&deployer));

        // network fallback
        let (_, doc) = sample_authoring_doc();
        let meta_hash = keccak256(&doc).0.to_vec();
        let meta_hash_hex = hex::encode_prefixed(&meta_hash);
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(deployer_json_body(
                &meta_hash_hex,
                &hex::encode_prefixed(&doc),
                &format!("0x{}", "66".repeat(32)),
                &meta_hash_hex,
            ));
        });
        let mut fresh = Store::new();
        fresh.add_subgraphs(&vec![server.url("/sg")]);
        let found = fresh
            .search_deployer_check(&meta_hash)
            .await
            .cloned()
            .unwrap();
        assert_eq!(found.meta_bytes, doc);
    }

    /// set_deployer_from_query_response fills the meta cache, the tx-hash
    /// map and the deployer cache, and returns the assembled record.
    #[test]
    fn test_store_set_deployer_from_query_response() {
        let (authoring_meta, doc) = sample_authoring_doc();
        let meta_hash = vec![0x0Au8; 32];
        let bytecode_meta_hash = vec![0x0Bu8; 32];
        let tx = vec![0x0Cu8; 32];
        let response = DeployerResponse {
            tx_hash: tx.clone(),
            bytecode_meta_hash: bytecode_meta_hash.clone(),
            meta_hash: meta_hash.clone(),
            meta_bytes: doc.clone(),
            bytecode: vec![0xE1],
            parser: vec![0xE2],
            store: vec![0xE3],
            interpreter: vec![0xE4],
        };
        let mut store = Store::new();
        let record = store.set_deployer_from_query_response(response);
        assert_eq!(record.meta_hash, meta_hash);
        assert_eq!(record.meta_bytes, doc);
        assert_eq!(record.bytecode, vec![0xE1]);
        assert_eq!(record.authoring_meta, Some(authoring_meta));
        assert_eq!(store.get_meta(&meta_hash), Some(&doc));
        assert_eq!(store.get_deployer(&bytecode_meta_hash), Some(&record));
        assert_eq!(store.get_deployer(&tx), Some(&record));
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
        let shared_meta_hash = vec![0x5Au8; 32];
        let deployer_ours = sample_deployer(&shared_meta_hash, b"ours");
        let deployer_theirs = sample_deployer(&shared_meta_hash, b"theirs");
        let shared_tx = vec![0x0Fu8; 32];
        let their_tx = vec![0x1Eu8; 32];

        let mut ours = Store::new();
        let mut theirs = Store::new();
        ours.set_deployer(&[0x01u8; 32], &deployer_ours, Some(&shared_tx));
        theirs.set_deployer(&[0x02u8; 32], &deployer_theirs, Some(&shared_tx));
        theirs.set_deployer(&[0x02u8; 32], &deployer_theirs, Some(&their_tx));

        // same deployer cache key in both stores
        let contested_key = vec![0x03u8; 32];
        let deployer_a = sample_deployer(&[0x04; 32], b"deployer-a");
        let deployer_b = sample_deployer(&[0x05; 32], b"deployer-b");
        ours.set_deployer(&contested_key, &deployer_a, None);
        theirs.set_deployer(&contested_key, &deployer_b, None);

        // same dotrain uri, different content
        let (hash_ours, _) = ours.set_dotrain("content a", "x.rain", false).unwrap();
        let (_hash_theirs, _) = theirs.set_dotrain("content b", "x.rain", false).unwrap();

        theirs.add_subgraphs(&vec!["sg-their".to_string()]);

        ours.merge(&theirs);

        // meta cache: existing entry wins
        assert_eq!(ours.get_meta(&shared_meta_hash), Some(&b"ours".to_vec()));
        // deployer cache: existing entry wins
        assert_eq!(ours.get_deployer(&contested_key), Some(&deployer_a));
        // tx-hash map: existing mapping wins
        assert_eq!(ours.get_deployer(&shared_tx), Some(&deployer_ours));
        // tx-hash map: a mapping only the other store holds is taken
        assert_eq!(ours.get_deployer(&their_tx), Some(&deployer_theirs));
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
        assert!(cached_store.update_with(&hash, &bytes).is_some());
        assert_eq!(cached_store.update_check(&hash).await, Some(&bytes));
    }

    /// Store::new() starts with no subgraphs, so every uncached lookup that
    /// reaches the network on it resolves to None instead of panicking.
    #[tokio::test]
    async fn test_store_no_subgraphs_lookups_return_none() {
        let hash = [0u8; 32];
        let mut store = Store::new();
        assert!(store.update(&hash).await.is_none());
        assert!(store.update_check(&hash).await.is_none());
        assert!(store.search_deployer(&hash).await.is_none());
        assert!(store.search_deployer_check(&hash).await.is_none());
        assert!(store.cache().is_empty());
        assert!(store.deployer_cache().is_empty());
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
        assert!(store.update_with(&wrong_hash, &bytes).is_none());
        assert!(store.get_meta(&wrong_hash).is_none());
        // valid pair stored
        let hash = keccak256(&bytes).0.to_vec();
        assert_eq!(store.update_with(&hash, &bytes), Some(&bytes));

        // existing entry is returned untouched, not overwritten
        let mut seeded = Store::new();
        let content = b"real content".to_vec();
        let content_hash = keccak256(&content).0.to_vec();
        let planted = sample_deployer(&content_hash, b"planted value");
        seeded.set_deployer(&[0x77u8; 32], &planted, None);
        assert_eq!(
            seeded.update_with(&content_hash, &content),
            Some(&b"planted value".to_vec())
        );
        assert_eq!(
            seeded.get_meta(&content_hash),
            Some(&b"planted value".to_vec())
        );

        // prefixed document: inner item stored under keccak of its encoding
        let (_, doc) = sample_authoring_doc();
        let doc_hash = keccak256(&doc).0.to_vec();
        let mut doc_store = Store::new();
        assert!(doc_store.update_with(&doc_hash, &doc).is_some());
        let inner = doc[8..].to_vec();
        assert_eq!(store_inner_lookup(&doc_store, &inner), Some(inner.clone()));

        // bare cbor sequence without the document prefix: no inner extraction
        let item_a = sample_dotrain_item().cbor_encode().unwrap();
        let (_, doc_b) = sample_authoring_doc();
        let item_b = doc_b[8..].to_vec();
        let seq = [item_a.clone(), item_b].concat();
        let seq_hash = keccak256(&seq).0.to_vec();
        let mut seq_store = Store::new();
        assert!(seq_store.update_with(&seq_hash, &seq).is_some());
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
