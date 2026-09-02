use alloy::{
    primitives::{keccak256, FixedBytes, B256},
    sol_types::SolType,
};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::SolCall;
use alloy::transports::{RpcError, TransportErrorKind};
use alloy::primitives::hex::FromHexError;
use alloy::sol_types::private::Address;
use alloy::sol;
use rain_metaboard_subgraph::metaboard_client::*;
use serde::{Deserialize, Serialize};
use crate::meta::{KnownMagic, RainMetaDocumentV1Item};
use rain_erc::erc165::Erc165Error;
use rain_metadata_bindings::IDescribedByMetaV1;
use thiserror::Error;
use super::super::super::implements_i_described_by_meta_v1;
#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{prelude::*, impl_wasm_traits};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct AuthoringMetaV2Word {
    pub word: String,
    pub description: String,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(AuthoringMetaV2Word);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct AuthoringMetaV2 {
    pub words: Vec<AuthoringMetaV2Word>,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(AuthoringMetaV2);

sol!(
    struct AuthoringMetaV2Sol {
        // `word` is referenced directly in assembly so don't move the field. It MUST
        // be the first item.
        bytes32 word;
        string description;
    }
);

type AuthoringMetasV2Sol = sol! { AuthoringMetaV2Sol[] };

#[derive(Error, Debug)]
pub enum AuthoringMetaV2Error {
    #[error(transparent)]
    FromHexError(#[from] FromHexError),
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
    #[error(transparent)]
    MetaboardSubgraphError(
        #[from] rain_metaboard_subgraph::metaboard_client::MetaboardSubgraphClientError,
    ),
    #[error("Meta bytes do not start with RainMetaDocumentV1 Magic")]
    MetaMagicNumberMismatch,
    #[error(transparent)]
    AbiDecodeError(#[from] alloy::sol_types::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    MetaError(#[from] crate::error::Error),
    #[error("Contract has no words")]
    HasNoWords,
    #[error(transparent)]
    Erc165Error(#[from] Erc165Error),
    #[error("no RPC URLs provided")]
    NoRpcs,
    #[error("Metaboard meta bytes hash {actual} does not match describedByMetaV1 hash {expected}")]
    MetaHashMismatch { expected: B256, actual: B256 },
}

#[derive(Error, Debug)]
#[error("Error fetching authoring meta for contract {contract_address}, RPCs {rpcs:?}, Metaboard URL {metaboard_url}: {error}")]
pub struct FetchAuthoringMetaV2WordError {
    contract_address: Address,
    rpcs: Vec<String>,
    metaboard_url: String,
    #[source]
    error: AuthoringMetaV2Error,
}

/// Implementation of the AuthoringMetaV2 struct.
impl AuthoringMetaV2 {
    /// Decodes the ABI encoded bytes into an AuthoringMetaV2 struct.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The bytes to decode.
    ///
    /// # Returns
    ///
    /// An AuthoringMetaV2 struct if successful, or an AuthoringMetaV2Error if an error occurs.
    pub fn abi_decode(bytes: &[u8]) -> Result<Self, AuthoringMetaV2Error> {
        let decoded = AuthoringMetasV2Sol::abi_decode(bytes)?;

        let mut words = Vec::new();

        for item in decoded.iter() {
            let trimmed_word = &item.word.as_slice()[..item
                .word
                .as_slice()
                .iter()
                .position(|&x| x == 0)
                .unwrap_or(item.word.as_slice().len())];
            words.push(AuthoringMetaV2Word {
                word: String::from_utf8(trimmed_word.into())?,
                description: item.description.clone(),
            });
        }

        Ok(AuthoringMetaV2 { words })
    }

    /// Reads the meta hash the contract is described by over a single RPC.
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The address of the contract.
    /// * `rpc` - The RPC URL to read over.
    ///
    /// # Returns
    ///
    /// The meta hash if successful, or an AuthoringMetaV2Error if an error occurs.
    async fn fetch_metahash(
        contract_address: Address,
        rpc: &str,
    ) -> Result<[u8; 32], AuthoringMetaV2Error> {
        let provider = ProviderBuilder::new().connect_http(rpc.parse()?);

        if !implements_i_described_by_meta_v1(&provider, contract_address).await? {
            return Err(AuthoringMetaV2Error::HasNoWords);
        }

        let call = IDescribedByMetaV1::describedByMetaV1Call {};
        let tx = TransactionRequest::default()
            .to(contract_address)
            .input(call.abi_encode().into());
        let bytes = provider.call(tx).await?;
        let FixedBytes(metahash) =
            IDescribedByMetaV1::describedByMetaV1Call::abi_decode_returns(&bytes)?;

        Ok(metahash)
    }

    /// Fetches the authoring meta for a contract that implements IDescribedByMetaV1
    /// from the metaboard.
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The address of the contract.
    /// * `rpcs` - The RPC URLs, tried in order until one yields the meta hash.
    ///   When none does, the failure of the last one tried is reported.
    /// * `metaboard_url` - The metaboard subgraph URL to query for the meta.
    ///
    /// # Returns
    ///
    /// An empty result if successful, or a AuthoringMetaV2Error if an error occurs.
    pub async fn fetch_for_contract(
        contract_address: Address,
        rpcs: Vec<String>,
        metaboard_url: String,
    ) -> Result<Self, FetchAuthoringMetaV2WordError> {
        let wrap_error = |error: AuthoringMetaV2Error| FetchAuthoringMetaV2WordError {
            contract_address,
            rpcs: rpcs.clone(),
            metaboard_url: metaboard_url.clone(),
            error,
        };

        let mut metahash = None;
        let mut rpc_error = AuthoringMetaV2Error::NoRpcs;
        for rpc in &rpcs {
            match Self::fetch_metahash(contract_address, rpc).await {
                Ok(hash) => {
                    metahash = Some(hash);
                    break;
                }
                // The contract not implementing the interface is an answer, not
                // a failed read, and it is the same answer on every RPC of the
                // chain. Asking the rest cannot change it, and continuing would
                // replace it with whichever transport error the last RPC gave.
                Err(error @ AuthoringMetaV2Error::HasNoWords) => return Err(wrap_error(error)),
                Err(error) => rpc_error = error,
            }
        }
        let Some(metahash) = metahash else {
            return Err(wrap_error(rpc_error));
        };

        // query the metaboard for the metas
        let subgraph_client = MetaboardSubgraphClient::new(
            metaboard_url
                .parse()
                .map_err(|error: url::ParseError| wrap_error(error.into()))?,
        );

        let metas = subgraph_client
            .get_metabytes_by_hash(&metahash)
            .await
            .map_err(|error| wrap_error(error.into()))?;

        // the metaboard returns every meta carrying this hash in an order the
        // query does not pin down, and each one is a cbor sequence, so scan all
        // of them for an authoring meta before giving up, reporting the first
        // failure encountered if none is found. the endpoint is not trusted to
        // answer with the bytes it was asked for, so each meta is hashed and
        // checked against the describedByMetaV1 hash before it is decoded: a
        // meta that does not match is skipped without ever being parsed, and
        // only words that the contract's own hash commits to can be returned.
        let mut first_error: Option<AuthoringMetaV2Error> = None;
        for meta_bytes in &metas {
            let meta_bytes = meta_bytes.as_slice();
            let meta_bytes_hash = keccak256(meta_bytes);
            if meta_bytes_hash.0 != metahash {
                first_error.get_or_insert(AuthoringMetaV2Error::MetaHashMismatch {
                    expected: metahash.into(),
                    actual: meta_bytes_hash,
                });
                continue;
            }

            let items = match RainMetaDocumentV1Item::cbor_decode(meta_bytes) {
                Ok(items) => items,
                Err(error) => {
                    first_error.get_or_insert(error.into());
                    continue;
                }
            };
            for item in items {
                match AuthoringMetaV2::try_from(item) {
                    Ok(meta) => return Ok(meta),
                    // not claiming to be authoring meta - an abi beside the
                    // words, say - so it is skipped
                    Err(error @ AuthoringMetaV2Error::MetaMagicNumberMismatch) => {
                        first_error.get_or_insert(error);
                    }
                    // claiming the magic and failing the claim, inside the
                    // document the contract's own hash commits to. A broken
                    // claim is not mined for its readable parts: a later item
                    // is not consulted, the same rule fetch_by_subject and the
                    // cbor decoder apply to their emissions.
                    Err(error) => return Err(wrap_error(error)),
                }
            }
        }

        Err(wrap_error(
            first_error.unwrap_or(AuthoringMetaV2Error::MetaMagicNumberMismatch),
        ))
    }
}

impl TryFrom<RainMetaDocumentV1Item> for AuthoringMetaV2 {
    type Error = AuthoringMetaV2Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, AuthoringMetaV2Error> {
        if value.magic != KnownMagic::AuthoringMetaV2 {
            return Err(AuthoringMetaV2Error::MetaMagicNumberMismatch);
        }
        let payload = value.unpack()?;
        AuthoringMetaV2::abi_decode(&payload)
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use alloy::primitives::hex::{decode, encode};
    use serde_bytes::ByteBuf;
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use reqwest::Url;

    use crate::meta::{ContentEncoding, ContentLanguage, ContentType};

    use super::*;

    #[tokio::test]
    async fn test_try_from_valid() {
        let magic = KnownMagic::AuthoringMetaV2;

        // encoded with chisel
        let payload = decode::<String>("0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000016074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e20310000000000000000000000000000000000000074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e20320000000000000000000000000000000000000074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e203300000000000000000000000000000000000000".into()).unwrap();
        let item = RainMetaDocumentV1Item {
            magic,
            payload: ByteBuf::from(payload),
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
            content_type: ContentType::None,
        };

        let result = AuthoringMetaV2::try_from(item);

        assert!(result.is_ok());

        let words = result.unwrap().words;
        assert!(words.len() == 3);
        assert!(words[0].word == "test");
        assert!(words[0].description == "description 1");
        assert!(words[1].word == "test");
        assert!(words[1].description == "description 2");
        assert!(words[2].word == "test");
        assert!(words[2].description == "description 3");
    }

    #[tokio::test]
    async fn test_try_from_invalid_magic() {
        let magic = KnownMagic::AuthoringMetaV1;
        // encoded with chisel
        let payload = decode::<String>("0x00".into()).unwrap();

        let item = RainMetaDocumentV1Item {
            magic,
            payload: ByteBuf::from(payload),
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
            content_type: ContentType::None,
        };

        let result = AuthoringMetaV2::try_from(item);

        assert!(result.is_err());

        let error = result.unwrap_err();

        match error {
            AuthoringMetaV2Error::MetaMagicNumberMismatch => {}
            _ => panic!("Unexpected error: {:?}", error),
        }
    }

    #[tokio::test]
    async fn test_abi_decode_valid() {
        let payload = decode::<String>("0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000016074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e20310000000000000000000000000000000000000074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e20320000000000000000000000000000000000000074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e203300000000000000000000000000000000000000".into()).unwrap();
        let result = AuthoringMetaV2::abi_decode(&payload);

        assert!(result.is_ok());

        let words = result.unwrap().words;
        assert!(words.len() == 3);
        assert!(words[0].word == "test");
        assert!(words[0].description == "description 1");
        assert!(words[1].word == "test");
        assert!(words[1].description == "description 2");
        assert!(words[2].word == "test");
        assert!(words[2].description == "description 3");
    }

    #[tokio::test]
    async fn test_abi_decode_invalid() {
        let payload = decode::<String>("0x00".into()).unwrap();
        let result = AuthoringMetaV2::abi_decode(&payload);

        assert!(result.is_err());

        let error = result.unwrap_err();

        match error {
            AuthoringMetaV2Error::AbiDecodeError(_) => {}
            _ => panic!("Unexpected error: {:?}", error),
        }
    }

    #[tokio::test]
    async fn test_get_metabytes_by_hash_success() {
        let hash = [1u8; 32];

        let rpc_server = MockServer::start_async().await;
        let rpc_url = Url::parse(&rpc_server.url("/")).unwrap();

        // Mock a successful response
        rpc_server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": format!("0x{}", encode(hash))
                })
            });
        });

        let metaboard_server = MockServer::start_async().await;
        let metaboard_url = Url::parse(&metaboard_server.url("/")).unwrap();

        // Mock a successful response
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash)); // You need to tailor this to the actual body sent
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                             "meta": "0x01",
                             "metaHash": "0x00",
                             "sender": "0x00",
                             "id": "0x00",
                             "metaBoard": {
                                 "id": "0x00",
                                 "metas": [],
                                 "address": "0x00",
                             },
                             "subject": "0x00",
                            },
                            {
                                "meta": "0x02",
                                "metaHash": "0x00",
                                "sender": "0x00",
                                "id": "0x00",
                                "metaBoard": {
                                    "id": "0x00",
                                    "metas": [],
                                    "address": "0x00",
                                },
                                "subject": "0x00",
                               }
                        ]
                    }
                })
            });
        });

        let authoring_meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_url.to_string()],
            metaboard_url.to_string(),
        )
        .await;

        match authoring_meta {
            Ok(_) => panic!("Expected error"),
            Err(error) => {
                let FetchAuthoringMetaV2WordError {
                    contract_address,
                    rpcs,
                    metaboard_url: err_metaboard_url,
                    error,
                } = error;
                assert_eq!(contract_address, Address::from([0u8; 20]));
                assert_eq!(rpcs, vec![rpc_url.to_string()]);
                assert_eq!(err_metaboard_url, metaboard_url.to_string());
                match error {
                    AuthoringMetaV2Error::HasNoWords => {}
                    _ => panic!("Unexpected error: {:?}", error),
                }
            }
        }
    }

    // ---- helpers for fetch_for_contract tests ----

    /// hex payload of an abi encoded AuthoringMetaV2Sol[] with three words
    /// ("test" with descriptions 1..3), same fixture as the decode tests.
    static WORDS_PAYLOAD_HEX: &str = "0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000016074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e20310000000000000000000000000000000000000074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e20320000000000000000000000000000000000000074657374000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000d6465736372697074696f6e203300000000000000000000000000000000000000";

    fn bool_word(b: bool) -> String {
        let mut s = "0x".to_string();
        s.push_str(&"0".repeat(63));
        s.push_str(if b { "1" } else { "0" });
        s
    }

    /// Mocks the full JSON-RPC flow implements_i_described_by_meta_v1 walks
    /// (erc165 check1, check2, interface check) plus the describedByMetaV1
    /// call returning `metahash`.
    fn mock_described_by_rpc(rpc_server: &MockServer, metahash: [u8; 32]) {
        let sel = encode(IDescribedByMetaV1::describedByMetaV1Call::SELECTOR);
        // erc165 check1: supportsInterface(0x01ffc9a7) -> true
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a701ffc9a7");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        // erc165 check2: supportsInterface(0xffffffff) -> false
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a7ffffffff");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(false)
            }));
        });
        // supportsInterface(IDescribedByMetaV1 interface id) -> true
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("01ffc9a7{}", sel));
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        // describedByMetaV1() -> metahash
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("0x{}\"", sel));
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "result": format!("0x{}", encode(metahash))
            }));
        });
    }

    fn metaboard_meta_entry(meta_hex: &str) -> serde_json::Value {
        serde_json::json!({
            "meta": meta_hex,
            "metaHash": "0x00",
            "sender": "0x00",
            "id": "0x00",
            "metaBoard": {
                "id": "0x00",
                "metas": [],
                "address": "0x00",
            },
            "subject": "0x00",
        })
    }

    /// The hash the metaboard indexes `meta_hex` under: keccak256 of the raw
    /// emitted bytes, as `LibDescribedByMeta.emitForDescribedAddress` and the
    /// subgraph mapping both compute it.
    fn meta_hex_hash(meta_hex: &str) -> [u8; 32] {
        keccak256(decode::<String>(meta_hex.into()).unwrap()).0
    }

    fn document(magic: KnownMagic, payload: Vec<u8>) -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            magic,
            payload: ByteBuf::from(payload),
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
            content_type: ContentType::None,
        }
    }

    /// RainMetaDocumentV1Item carrying the three word payload under the
    /// AuthoringMetaV2 magic.
    fn authoring_meta_v2_document() -> RainMetaDocumentV1Item {
        let payload = decode::<String>(WORDS_PAYLOAD_HEX.into()).unwrap();
        document(KnownMagic::AuthoringMetaV2, payload)
    }

    /// A well formed item under a magic fetch_for_contract does not want.
    fn other_magic_document() -> RainMetaDocumentV1Item {
        document(KnownMagic::AuthoringMetaV1, vec![0u8])
    }

    /// hex of a cbor sequence of the given documents, as one metaboard meta
    fn cbor_seq_hex(documents: Vec<RainMetaDocumentV1Item>) -> String {
        format!(
            "0x{}",
            encode(
                RainMetaDocumentV1Item::cbor_encode_seq(&documents, KnownMagic::RainMetaDocumentV1)
                    .unwrap()
            )
        )
    }

    /// cbor encoded RainMetaDocumentV1Item carrying the three word payload
    /// under the AuthoringMetaV2 magic.
    fn authoring_meta_v2_cbor_hex() -> String {
        let payload = decode::<String>(WORDS_PAYLOAD_HEX.into()).unwrap();
        let item = RainMetaDocumentV1Item {
            magic: KnownMagic::AuthoringMetaV2,
            payload: ByteBuf::from(payload),
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
            content_type: ContentType::None,
        };
        format!("0x{}", encode(item.cbor_encode().unwrap()))
    }

    /// Mocks a metaboard answering `metahash` with the three word authoring meta.
    /// `metahash` must be that document's own hash, or the returned bytes are
    /// rejected before they are decoded.
    fn mock_metaboard_words(metaboard_server: &MockServer, metahash: [u8; 32]) {
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(metahash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [metaboard_meta_entry(&authoring_meta_v2_cbor_hex())] }
            }));
        });
    }

    /// Mocks an RPC that fails every request at the transport level.
    fn mock_dead_rpc(rpc_server: &MockServer) -> httpmock::Mock<'_> {
        rpc_server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(500).body("rpc is down");
        })
    }

    #[tokio::test]
    async fn test_abi_decode_full_32_byte_word_kept_whole() {
        let word = [b'a'; 32];
        let encoded = AuthoringMetasV2Sol::abi_encode(&vec![AuthoringMetaV2Sol {
            word: word.into(),
            description: "full width".to_string(),
        }]);
        let decoded = AuthoringMetaV2::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.words.len(), 1);
        // no NUL anywhere: the full 32 bytes are the word
        assert_eq!(decoded.words[0].word, "a".repeat(32));
        assert_eq!(decoded.words[0].description, "full width");
    }

    #[tokio::test]
    async fn test_abi_decode_invalid_utf8_word_is_utf8_error() {
        let mut word = [0u8; 32];
        // 0xc3 followed by 0x28 is an invalid utf8 sequence, before any NUL
        word[0] = 0xc3;
        word[1] = 0x28;
        let encoded = AuthoringMetasV2Sol::abi_encode(&vec![AuthoringMetaV2Sol {
            word: word.into(),
            description: "bad word bytes".to_string(),
        }]);
        let result = AuthoringMetaV2::abi_decode(&encoded);
        match result {
            Err(AuthoringMetaV2Error::Utf8Error(_)) => {}
            other => panic!("expected Utf8Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_empty_rpcs_is_no_rpcs_error() {
        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([7u8; 20]),
            vec![],
            "http://metaboard.test/".to_string(),
        )
        .await;
        let error = result.unwrap_err();
        assert_eq!(error.contract_address, Address::from([7u8; 20]));
        assert!(error.rpcs.is_empty());
        assert_eq!(error.metaboard_url, "http://metaboard.test/");
        match error.error {
            AuthoringMetaV2Error::NoRpcs => {}
            other => panic!("expected NoRpcs, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_invalid_rpc_url_is_url_parse_error() {
        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([7u8; 20]),
            vec!["not a url".to_string()],
            "http://metaboard.test/".to_string(),
        )
        .await;
        let error = result.unwrap_err();
        assert_eq!(error.rpcs, vec!["not a url".to_string()]);
        match error.error {
            AuthoringMetaV2Error::UrlParseError(_) => {}
            other => panic!("expected UrlParseError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_rpc_error_on_describe_call() {
        let rpc_server = MockServer::start_async().await;
        let sel = encode(IDescribedByMetaV1::describedByMetaV1Call::SELECTOR);
        // erc165 detection succeeds
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a701ffc9a7");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a7ffffffff");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(false)
            }));
        });
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("01ffc9a7{}", sel));
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        // the describedByMetaV1 call itself errors at the rpc level
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("0x{}\"", sel));
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "error": { "code": -32000, "message": "boom" }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            "http://metaboard.test/".to_string(),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::RpcError(_) => {}
            other => panic!("expected RpcError, got {:?}", other),
        }
    }

    /// A transport failure of the erc165 probe - the first of the two, which
    /// rain-erc runs - is "answer unknown" for the same reason as the second,
    /// and was flattened by its own `unwrap_or(false)`.
    #[tokio::test]
    async fn test_fetch_for_contract_erc165_transport_error_is_not_has_no_words() {
        let rpc_server = MockServer::start_async().await;
        let probe = rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a701ffc9a7");
            then.status(500).body("rpc down");
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            "http://metaboard.test/".to_string(),
        )
        .await;
        probe.assert();
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::Erc165Error(_) => {}
            other => panic!("expected Erc165Error, got {:?}", other),
        }
    }

    /// A transport failure of the IDescribedByMetaV1 supportsInterface call is
    /// "answer unknown", so it must surface as the erc165 error and never as
    /// the definitive HasNoWords.
    #[tokio::test]
    async fn test_fetch_for_contract_described_by_probe_error_is_not_has_no_words() {
        let rpc_server = MockServer::start_async().await;
        let sel = encode(IDescribedByMetaV1::describedByMetaV1Call::SELECTOR);
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a701ffc9a7");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a7ffffffff");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(false)
            }));
        });
        let probe = rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("01ffc9a7{}", sel));
            then.status(500).body("rpc down");
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            "http://metaboard.test/".to_string(),
        )
        .await;
        probe.assert();
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::Erc165Error(_) => {}
            other => panic!("expected Erc165Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_abi_decode_error_on_describe_call() {
        let rpc_server = MockServer::start_async().await;
        let sel = encode(IDescribedByMetaV1::describedByMetaV1Call::SELECTOR);
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a701ffc9a7");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a7ffffffff");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(false)
            }));
        });
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("01ffc9a7{}", sel));
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(true)
            }));
        });
        // describedByMetaV1 succeeds at the rpc level but returns bytes that
        // cannot decode as bytes32
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("0x{}\"", sel));
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": "0x"
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            "http://metaboard.test/".to_string(),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::AbiDecodeError(_) => {}
            other => panic!("expected AbiDecodeError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_invalid_metaboard_url_is_url_parse_error() {
        let hash = [1u8; 32];
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            "not a url".to_string(),
        )
        .await;
        let error = result.unwrap_err();
        assert_eq!(error.metaboard_url, "not a url");
        match error.error {
            AuthoringMetaV2Error::UrlParseError(_) => {}
            other => panic!("expected UrlParseError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_empty_metaboard_response_is_subgraph_error() {
        let hash = [1u8; 32];
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [] }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::MetaboardSubgraphError(_) => {}
            other => panic!("expected MetaboardSubgraphError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_success_decodes_first_meta() {
        let meta_hex = authoring_meta_v2_cbor_hex();
        let hash = meta_hex_hash(&meta_hex);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": {
                    "metaV1S": [
                        // the first meta is the authoring meta document and is
                        // the one that must be decoded
                        metaboard_meta_entry(&meta_hex),
                        // a trailing non-decodable meta must be ignored
                        metaboard_meta_entry("0x00"),
                    ]
                }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let meta = result.unwrap();
        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[0].word, "test");
        assert_eq!(meta.words[0].description, "description 1");
        assert_eq!(meta.words[2].description, "description 3");
    }

    #[tokio::test]
    async fn test_try_from_deflate_encoded_item_unpacks() {
        let payload = decode::<String>(WORDS_PAYLOAD_HEX.into()).unwrap();
        let deflated = ContentEncoding::Deflate.encode(&payload);
        assert_ne!(deflated, payload);
        let item = RainMetaDocumentV1Item {
            magic: KnownMagic::AuthoringMetaV2,
            payload: ByteBuf::from(deflated),
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::None,
            schema: None,
            content_type: ContentType::None,
        };
        let result = AuthoringMetaV2::try_from(item).unwrap();
        assert_eq!(result.words.len(), 3);
        assert_eq!(result.words[0].word, "test");
        assert_eq!(result.words[1].description, "description 2");
    }
    #[tokio::test]
    async fn test_fetch_for_contract_invalid_cbor_is_meta_error() {
        let hash = meta_hex_hash("0x01");
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        // the metaboard answers with bytes that are not valid cbor, so the
        // pipeline must surface the cbor_decode failure as MetaError rather
        // than any other variant
        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [metaboard_meta_entry("0x01")] }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::MetaError(_) => {}
            other => panic!("expected MetaError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_falls_over_to_a_later_rpc() {
        let hash = meta_hex_hash(&authoring_meta_v2_cbor_hex());
        let dead_rpc_server = MockServer::start_async().await;
        let dead_rpc = mock_dead_rpc(&dead_rpc_server);
        let live_rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&live_rpc_server, hash);
        let metaboard_server = MockServer::start_async().await;
        mock_metaboard_words(&metaboard_server, hash);

        let meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![dead_rpc_server.url("/"), live_rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await
        .unwrap();

        assert!(dead_rpc.hits() > 0);
        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[0].word, "test");
        assert_eq!(meta.words[2].description, "description 3");
    }

    #[tokio::test]
    async fn test_fetch_for_contract_falls_over_an_unparseable_rpc_url() {
        let hash = meta_hex_hash(&authoring_meta_v2_cbor_hex());
        let live_rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&live_rpc_server, hash);
        let metaboard_server = MockServer::start_async().await;
        mock_metaboard_words(&metaboard_server, hash);

        let meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec!["not a url".to_string(), live_rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await
        .unwrap();

        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[1].description, "description 2");
    }

    /// A contract that says it does not implement the interface has answered,
    /// and the answer is a property of the contract, so it is the same on every
    /// RPC of the chain. The later RPCs are not asked, and HasNoWords is
    /// reported rather than being overwritten by whatever the last RPC said.
    ///
    /// Failing over here was harmless only while a dead RPC also produced
    /// HasNoWords. Once #175 and #154 made a failed probe its own error, the
    /// two stopped being the same thing.
    #[tokio::test]
    async fn test_fetch_for_contract_does_not_fall_over_a_contract_without_words() {
        let rpc_server = MockServer::start_async().await;
        // erc165 check1 answers false, so the contract declines the interface
        rpc_server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("01ffc9a701ffc9a7");
            then.status(200).json_body_obj(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": bool_word(false)
            }));
        });
        let unused_rpc_server = MockServer::start_async().await;
        let unused_rpc = mock_dead_rpc(&unused_rpc_server);

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/"), unused_rpc_server.url("/")],
            "http://metaboard.test/".to_string(),
        )
        .await;

        assert_eq!(unused_rpc.hits(), 0, "a later rpc was asked anyway");
        match result.unwrap_err().error {
            AuthoringMetaV2Error::HasNoWords => {}
            other => panic!("expected HasNoWords, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_stops_at_the_first_rpc_that_answers() {
        let hash = meta_hex_hash(&authoring_meta_v2_cbor_hex());
        let live_rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&live_rpc_server, hash);
        let unused_rpc_server = MockServer::start_async().await;
        let unused_rpc = mock_dead_rpc(&unused_rpc_server);
        let metaboard_server = MockServer::start_async().await;
        mock_metaboard_words(&metaboard_server, hash);

        let meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![live_rpc_server.url("/"), unused_rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await
        .unwrap();

        assert_eq!(unused_rpc.hits(), 0);
        assert_eq!(meta.words.len(), 3);
    }

    #[tokio::test]
    async fn test_fetch_for_contract_all_rpcs_failing_reports_the_last_failure() {
        let dead_rpc_server = MockServer::start_async().await;
        let dead_rpc = mock_dead_rpc(&dead_rpc_server);

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([7u8; 20]),
            vec![dead_rpc_server.url("/"), "not a url".to_string()],
            "http://metaboard.test/".to_string(),
        )
        .await;

        let error = result.unwrap_err();
        assert!(dead_rpc.hits() > 0);
        assert_eq!(
            error.rpcs,
            vec![dead_rpc_server.url("/"), "not a url".to_string()]
        );
        // the last rpc tried is the unparseable one, so its error is the reported one
        match error.error {
            AuthoringMetaV2Error::UrlParseError(_) => {}
            other => panic!("expected UrlParseError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_rejects_meta_bytes_not_matching_the_hash() {
        let hash = [1u8; 32];
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        // a well formed authoring meta document served under a hash that is
        // not its own: it decodes cleanly and must still be rejected
        let meta_hex = authoring_meta_v2_cbor_hex();
        assert_ne!(meta_hex_hash(&meta_hex), hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [metaboard_meta_entry(&meta_hex)] }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::MetaHashMismatch { expected, actual } => {
                assert_eq!(expected, B256::from(hash));
                assert_eq!(actual, B256::from(meta_hex_hash(&meta_hex)));
            }
            other => panic!("expected MetaHashMismatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_for_contract_checks_the_hash_before_decoding() {
        let hash = [1u8; 32];
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        // bytes that are neither the hashed content nor valid cbor: the hash
        // mismatch is what is reported, so nothing unverified is ever decoded
        assert_ne!(meta_hex_hash("0x01"), hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [metaboard_meta_entry("0x01")] }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::MetaHashMismatch { .. } => {}
            other => panic!("expected MetaHashMismatch, got {:?}", other),
        }
    }
    #[tokio::test]
    async fn test_fetch_for_contract_success_authoring_meta_first() {
        let meta_hex = authoring_meta_v2_cbor_hex();
        let hash = meta_hex_hash(&meta_hex);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": {
                    "metaV1S": [
                        metaboard_meta_entry(&meta_hex),
                        // a trailing non-decodable meta must be ignored
                        metaboard_meta_entry("0x00"),
                    ]
                }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let meta = result.unwrap();
        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[0].word, "test");
        assert_eq!(meta.words[0].description, "description 1");
        assert_eq!(meta.words[2].description, "description 3");
    }

    /// the metaboard does not pin down the order of the metas it returns, so an
    /// authoring meta behind an entry that fails the hash check and does not
    /// even cbor decode is still found
    #[tokio::test]
    async fn test_fetch_for_contract_authoring_meta_after_unmatched_undecodable_meta() {
        let meta_hex = authoring_meta_v2_cbor_hex();
        let hash = meta_hex_hash(&meta_hex);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": {
                    "metaV1S": [
                        metaboard_meta_entry("0x00"),
                        metaboard_meta_entry(&meta_hex),
                    ]
                }
            }));
        });

        let meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await
        .unwrap();
        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[0].description, "description 1");
        assert_eq!(meta.words[2].description, "description 3");
    }

    /// an entry that is well formed cbor under some other magic is skipped for
    /// failing the hash check rather than being taken as the answer, so it does
    /// not hide the authoring meta behind it: the hash is what decides, not
    /// whether the leading bytes happen to parse
    #[tokio::test]
    async fn test_fetch_for_contract_authoring_meta_after_unmatched_decodable_meta() {
        let meta_hex = authoring_meta_v2_cbor_hex();
        let hash = meta_hex_hash(&meta_hex);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": {
                    "metaV1S": [
                        metaboard_meta_entry(&cbor_seq_hex(vec![other_magic_document()])),
                        metaboard_meta_entry(&meta_hex),
                    ]
                }
            }));
        });

        let meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await
        .unwrap();
        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[2].description, "description 3");
    }

    /// one meta is a cbor sequence, so an authoring meta that is not the first
    /// document within it is still found
    #[tokio::test]
    async fn test_fetch_for_contract_authoring_meta_is_second_cbor_document() {
        let seq = cbor_seq_hex(vec![other_magic_document(), authoring_meta_v2_document()]);
        let hash = meta_hex_hash(&seq);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [metaboard_meta_entry(&seq)] }
            }));
        });

        let meta = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await
        .unwrap();
        assert_eq!(meta.words.len(), 3);
        assert_eq!(meta.words[2].description, "description 3");
    }

    /// scanning every meta and every document and finding no authoring meta is
    /// a magic mismatch, not a success
    #[tokio::test]
    async fn test_fetch_for_contract_no_authoring_meta_anywhere_is_magic_mismatch() {
        let seq = cbor_seq_hex(vec![other_magic_document(), other_magic_document()]);
        let hash = meta_hex_hash(&seq);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": {
                    "metaV1S": [
                        metaboard_meta_entry(&seq),
                        metaboard_meta_entry(&cbor_seq_hex(vec![other_magic_document()])),
                    ]
                }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        let error = result.unwrap_err();
        match error.error {
            AuthoringMetaV2Error::MetaMagicNumberMismatch => {}
            other => panic!("expected MetaMagicNumberMismatch, got {:?}", other),
        }
    }

    /// An item claiming the authoring magic whose payload does not decode is
    /// a broken claim inside the document the contract's hash commits to. The
    /// good item behind it is not returned: a broken claim is not mined for
    /// its readable parts, the rule fetch_by_subject and the cbor decoder
    /// apply to their emissions.
    #[tokio::test]
    async fn test_fetch_for_contract_does_not_scan_past_a_broken_authoring_claim() {
        // junk under the authoring magic, then real words
        let seq = cbor_seq_hex(vec![
            document(KnownMagic::AuthoringMetaV2, vec![0xff, 0xfe]),
            authoring_meta_v2_document(),
        ]);
        let hash = meta_hex_hash(&seq);
        let rpc_server = MockServer::start_async().await;
        mock_described_by_rpc(&rpc_server, hash);

        let metaboard_server = MockServer::start_async().await;
        metaboard_server.mock(|when, then| {
            when.method(POST).path("/").body_contains(encode(hash));
            then.status(200).json_body_obj(&serde_json::json!({
                "data": { "metaV1S": [ metaboard_meta_entry(&seq) ] }
            }));
        });

        let result = AuthoringMetaV2::fetch_for_contract(
            Address::from([0u8; 20]),
            vec![rpc_server.url("/")],
            metaboard_server.url("/"),
        )
        .await;
        match result.unwrap_err().error {
            AuthoringMetaV2Error::AbiDecodeError(_) => {}
            other => panic!("expected the broken claim's decode error, got {:?}", other),
        }
    }
}
