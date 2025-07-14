use super::error::Error;
use super::subgraph::KnownSubgraphs;
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
use alloy_ethers_typecast::transaction::{ReadContractParameters, ReadableClient};
use rain_erc::erc165::{IERC165, XorSelectors, supports_erc165};

pub mod magic;
pub(crate) mod normalize;
pub(crate) mod query;
pub mod types;
pub mod unpacked;

pub use magic::*;
pub use query::*;
pub use unpacked::*;

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
    DotrainGuiStateV1,
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
            KnownMagic::DotrainGuiStateV1 => Ok(KnownMeta::DotrainGuiStateV1),
            KnownMagic::ExpressionDeployerV2BytecodeV1 => {
                Ok(KnownMeta::ExpressionDeployerV2BytecodeV1)
            }
            KnownMagic::RainlangSourceV1 => Ok(KnownMeta::RainlangSourceV1),
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
            | KnownMagic::DotrainGuiStateV1
            | KnownMagic::RainlangSourceV1 => T::try_from(self),
            _ => Err(Error::UnsupportedMeta)?,
        }
    }

    /// Unpacks the metadata into the centralized `UnpackedMetadata` enum
    ///
    /// This convenience method automatically determines the correct metadata type
    /// based on the magic number and returns the parsed metadata.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rain_metadata::{RainMetaDocumentV1Item, UnpackedMetadata, KnownMagic};
    /// use rain_metadata::{ContentType, ContentEncoding, ContentLanguage};
    ///
    /// let meta_item = RainMetaDocumentV1Item {
    ///     payload: b"test dotrain content".to_vec().into(),
    ///     magic: KnownMagic::DotrainV1,
    ///     content_type: ContentType::Cbor,
    ///     content_encoding: ContentEncoding::Identity,
    ///     content_language: ContentLanguage::En,
    /// };
    /// let unpacked = meta_item.unpack_typed().unwrap();
    ///
    /// assert!(unpacked.is_dotrain_v1());
    /// ```
    pub fn unpack_typed(self) -> Result<UnpackedMetadata, Error> {
        self.try_into()
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
                let mut payload = None;
                let mut magic: Option<u64> = None;
                let mut content_type = None;
                let mut content_encoding = None;
                let mut content_language = None;
                while match map.next_key() {
                    Ok(Some(key)) => {
                        match key {
                            0 => payload = Some(map.next_value()?),
                            1 => magic = Some(map.next_value()?),
                            2 => content_type = Some(map.next_value()?),
                            3 => content_encoding = Some(map.next_value()?),
                            4 => content_language = Some(map.next_value()?),
                            other => Err(serde::de::Error::custom(format!(
                                "found unexpected key in the map: {other}"
                            )))?,
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
                })
            }
        }
        deserializer.deserialize_map(EncodedMap)
    }
}

/// searches for a meta matching the given hash in given subgraphs urls
pub async fn search(hash: &str, subgraphs: &Vec<String>) -> Result<query::MetaResponse, Error> {
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
pub async fn implements_i_described_by_meta_v1(
    client: &ReadableClient,
    contract_address: Address,
) -> bool {
    if !supports_erc165(client, contract_address).await {
        return false;
    }

    let interface_id_res = IDescribedByMetaV1::IDescribedByMetaV1Calls::xor_selectors();
    if interface_id_res.is_err() {
        return false;
    }

    let parameters = ReadContractParameters {
        address: contract_address,
        call: IERC165::supportsInterfaceCall {
            interfaceID: interface_id_res.unwrap().into(),
        },
        block_number: None,
        gas: None,
    };
    client.read(parameters).await.unwrap_or(false)
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
/// ```ignore
/// use rain_meta::Store;
/// use std::collections::HashMap;
///
///
/// // to instantiate with including default subgraphs
/// let mut store = Store::new();
///
/// // to instatiate with default rain subgraphs included
/// let mut store = Store::default();
///
/// // or to instantiate with initial values
/// let mut store = Store::create(
///     &vec!["sg-url-1".to_string()],
///     &HashMap::new(),
///     &HashMap::new(),
///     &HashMap::new(),
///     true
/// );
///
/// // add a new subgraph endpoint url to the subgraph list
/// store.add_subgraphs(&vec!["sg-url-2".to_string()]);
///
/// // update the store with another Store (merges the stores)
/// store.merge(&Store::default());
///
/// // hash of a meta to search and store
/// let hash = vec![0u8, 1u8, 2u8];
///
/// // updates the meta store with a new meta by searching through subgraphs
/// store.update(&hash);
///
/// // updates the meta store with a new meta hash and bytes
/// store.update_with(&hash, &vec![0u8, 1u8]);
///
/// // to get a record from store
/// let meta = store.get_meta(&hash);
///
/// // to get a deployer record from store
/// let deployer_record = store.get_deployer(&hash);
///
/// // path to a .rain file
/// let dotrain_uri = "path/to/file.rain";
///
/// // reading the dotrain content as an example,
/// // Store is agnostic to dotrain contents it just maps the hash of the content to the given
/// // uri and puts it as a new meta into the meta cache, so obtaining and passing the correct
/// // content is up to the implementer
/// let dotrain_content = std::fs::read_to_string(&dotrain_uri).unwrap_or(String::new());
///
/// // updates the dotrain cache for a dotrain text and uri
/// let (new_hash, old_hash) = store.set_dotrain(&dotrain_content, &dotrain_uri.to_string(), false).unwrap();
///
/// // to get dotrain meta bytes given a uri
/// let dotrain_meta_bytes = store.get_dotrain_meta(&dotrain_uri.to_string());
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
        Store {
            cache: HashMap::new(),
            dotrain_cache: HashMap::new(),
            deployer_cache: HashMap::new(),
            subgraphs: KnownSubgraphs::NPE2.map(|url| url.to_string()).to_vec(),
            deployer_hash_map: HashMap::new(),
        }
    }
}

impl Store {
    /// lazily creates a new instance
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
        include_rain_subgraphs: bool,
    ) -> Store {
        let mut store;
        if include_rain_subgraphs {
            store = Store::default();
        } else {
            store = Store::new();
        }
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
        for (hash, tx_hash) in &other.deployer_hash_map {
            self.deployer_hash_map.insert(hash.clone(), tx_hash.clone());
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
pub fn str_to_bytes32(text: &str) -> Result<[u8; 32], Error> {
    let bytes: &[u8] = text.as_bytes();
    if bytes.len() > 32 {
        return Err(Error::BiggerThan32Bytes);
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

#[cfg(test)]
mod tests {
    use super::{
        *, bytes32_to_str,
        magic::KnownMagic,
        str_to_bytes32,
        types::{authoring::v1::AuthoringMeta, dotrain::v1::DotrainMeta},
        ContentEncoding, ContentLanguage, ContentType, Error, RainMetaDocumentV1Item,
    };
    use alloy_ethers_typecast::transaction::ReadableClient;
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
        let mut cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 1 as we only had 1 encoded item
        assert_eq!(cbor_decoded.len(), 1);
        // decoded item must be equal to the original meta_map
        assert_eq!(cbor_decoded[0], meta_map);

        // unpack the payload into AuthoringMeta
        let unpacked_payload: AuthoringMeta = cbor_decoded.pop().unwrap().unpack_into()?;
        // must be equal to original meta
        assert_eq!(unpacked_payload, authoring_meta);

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
        let mut cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 1 as we only had 1 encoded item
        assert_eq!(cbor_decoded.len(), 1);
        // decoded item must be equal to the original meta_map
        assert_eq!(cbor_decoded[0], meta_map);

        // unpack the payload into DotrainMeta, should handle inflation of the payload internally
        let unpacked_payload: DotrainMeta = cbor_decoded.pop().unwrap().unpack_into()?;
        // must be equal to the original dotrain content
        assert_eq!(unpacked_payload, dotrain_content);

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
        let mut cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;
        // the length of decoded maps must be 2 as we had 2 encoded item
        assert_eq!(cbor_decoded.len(), 2);

        // decoded item 1 must be equal to the original meta_map_1
        assert_eq!(cbor_decoded[0], meta_map_1);
        // decoded item 2 must be equal to the original meta_map_2
        assert_eq!(cbor_decoded[1], meta_map_2);

        // unpack the payload of the second decoded map into DotrainMeta, should handle inflation of the payload internally
        let unpacked_payload_2: DotrainMeta = cbor_decoded.pop().unwrap().unpack_into()?;
        // must be equal to original meta
        assert_eq!(unpacked_payload_2, dotrain_content);

        // unpack the payload of first decoded map into AuthoringMeta
        let unpacked_payload_1: AuthoringMeta = cbor_decoded.pop().unwrap().unpack_into()?;
        // must be equal to the original dotrain content
        assert_eq!(unpacked_payload_1, authoring_meta);

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

    #[tokio::test]
    async fn test_implements_i_describe_by_meta_v1() {
        // makes new server/client with success response for erc165 check
        async fn new_server_client() -> (Asserter, ReadableClient) {
            let asserter = Asserter::new();
            let client = ReadableClient::new_mocked(asserter.clone());

            // Mock a responses for successful supports erc165 check
            asserter.push_success(
                &"0x0000000000000000000000000000000000000000000000000000000000000001",
            );
            asserter.push_success(
                &"0x0000000000000000000000000000000000000000000000000000000000000000",
            );

            (asserter, client)
        }

        let address = Address::random();

        // mock a true response for implements IDescribedByMetaV1
        let (asserter, client) = new_server_client().await;
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000001");
        let result = implements_i_described_by_meta_v1(&client, address).await;
        assert!(result);

        // mock a false response for implements IDescribedByMetaV1
        let (asserter, client) = new_server_client().await;
        asserter
            .push_success(&"0x0000000000000000000000000000000000000000000000000000000000000000");
        let result = implements_i_described_by_meta_v1(&client, address).await;
        assert!(!result);

        // mock a revert response for implements IDescribedByMetaV1
        let (asserter, client) = new_server_client().await;
        asserter.push_failure(ErrorPayload {
            code: -32003,
            message: "execution reverted".into(),
            data: Some(serde_json::value::to_raw_value(&json!("0x00")).unwrap()),
        });
        let result = implements_i_described_by_meta_v1(&client, address).await;
        assert!(!result);
    }
}
