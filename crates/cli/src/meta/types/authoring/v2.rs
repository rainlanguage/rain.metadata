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
use rain_metadata_bindings::IDescribedByMetaV1;
use thiserror::Error;
use validator::Validate;
use super::super::super::implements_i_described_by_meta_v1;
use super::super::common::v1::{REGEX_RAIN_SYMBOL, REGEX_RAIN_STRING};
#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{prelude::*, impl_wasm_traits};

#[derive(Validate, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct AuthoringMetaV2Word {
    #[validate(regex(
        path = "REGEX_RAIN_SYMBOL",
        message = "Must be alphanumeric lower-kebab-case beginning with a letter.\n"
    ))]
    pub word: String,
    #[validate(regex(
        path = "REGEX_RAIN_STRING",
        message = "Must be printable ASCII characters and whitespace.\n"
    ))]
    pub description: String,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(AuthoringMetaV2Word);

#[derive(Validate, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct AuthoringMetaV2 {
    #[validate]
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
    #[error(transparent)]
    ValidationErrors(#[from] validator::ValidationErrors),
    #[error("Contract has no words")]
    HasNoWords,
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
    ///
    /// Deliberately does NOT apply the rain word grammar. v2 metas are already
    /// published on chain and immutable, so a read that rejects them would
    /// brick tooling rather than fix the data. Callers that need the grammar
    /// enforced use [`AuthoringMetaV2::abi_decode_validate`].
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

    /// Decodes the ABI encoded bytes then validates every word against the same
    /// rain grammar AuthoringMeta v1 enforces: `REGEX_RAIN_SYMBOL` for `word`
    /// and `REGEX_RAIN_STRING` for `description`.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The bytes to decode.
    ///
    /// # Returns
    ///
    /// An AuthoringMetaV2 struct if successful, or an AuthoringMetaV2Error if an error occurs.
    pub fn abi_decode_validate(bytes: &[u8]) -> Result<Self, AuthoringMetaV2Error> {
        let authoring_meta = AuthoringMetaV2::abi_decode(bytes)?;
        authoring_meta.validate()?;
        Ok(authoring_meta)
    }

    /// Fetches the authoring meta for a contract that implements IDescribedByMetaV1
    /// from the metaboard.
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The address of the contract.
    ///
    /// # Returns
    ///
    /// An empty result if successful, or a AuthoringMetaV2Error if an error occurs.
    pub async fn fetch_for_contract(
        contract_address: Address,
        rpcs: Vec<String>,
        metaboard_url: String,
    ) -> Result<Self, FetchAuthoringMetaV2WordError> {
        // build a read provider over the first RPC
        let url = rpcs
            .first()
            .cloned()
            .ok_or_else(|| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: AuthoringMetaV2Error::NoRpcs,
            })?
            .parse()
            .map_err(|error: url::ParseError| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            })?;
        let provider = ProviderBuilder::new().connect_http(url);

        // return "has no words" error if the contract does not implement IDescribeByMetaV2 interface
        if !implements_i_described_by_meta_v1(&provider, contract_address).await {
            return Err(FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: AuthoringMetaV2Error::HasNoWords,
            });
        }

        let call = IDescribedByMetaV1::describedByMetaV1Call {};
        let tx = TransactionRequest::default()
            .to(contract_address)
            .input(call.abi_encode().into());
        let bytes = provider
            .call(tx)
            .await
            .map_err(|error| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            })?;
        let FixedBytes(metahash) = IDescribedByMetaV1::describedByMetaV1Call::abi_decode_returns(
            &bytes,
        )
        .map_err(|error| FetchAuthoringMetaV2WordError {
            contract_address,
            rpcs: rpcs.clone(),
            metaboard_url: metaboard_url.clone(),
            error: error.into(),
        })?;

        // query the metaboard for the metas
        let subgraph_client = MetaboardSubgraphClient::new(metaboard_url.parse().map_err(
            |error: url::ParseError| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            },
        )?);

        let metas = subgraph_client
            .get_metabytes_by_hash(&metahash)
            .await
            .map_err(|error| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            })?;

        let meta_bytes = metas[0].as_slice();
        let meta_bytes_hash = keccak256(meta_bytes);
        if meta_bytes_hash.0 != metahash {
            return Err(FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: AuthoringMetaV2Error::MetaHashMismatch {
                    expected: metahash.into(),
                    actual: meta_bytes_hash,
                },
            });
        }

        let meta = RainMetaDocumentV1Item::cbor_decode(meta_bytes).map_err(|error| {
            FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            }
        })?[0]
            .clone()
            .try_into()
            .map_err(
                |error: AuthoringMetaV2Error| FetchAuthoringMetaV2WordError {
                    contract_address,
                    rpcs,
                    metaboard_url,
                    error,
                },
            )?;

        Ok(meta)
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

    use validator::ValidationErrorsKind;

    use crate::meta::{ContentEncoding, ContentLanguage, ContentType, str_to_bytes32};

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

    fn word_sol(word: &[u8; 32], description: &str) -> AuthoringMetaV2Sol {
        AuthoringMetaV2Sol {
            word: (*word).into(),
            description: description.to_string(),
        }
    }

    fn symbol_sol(word: &str, description: &str) -> AuthoringMetaV2Sol {
        word_sol(&str_to_bytes32(word).unwrap(), description)
    }

    #[tokio::test]
    async fn test_abi_decode_validate_accepts_grammatical_words() {
        let payload = decode::<String>(WORDS_PAYLOAD_HEX.into()).unwrap();
        let decoded = AuthoringMetaV2::abi_decode_validate(&payload).unwrap();
        assert_eq!(decoded, AuthoringMetaV2::abi_decode(&payload).unwrap());
    }

    /// The empty word (a bytes32 of NULs) is the case issue #168 reproduced:
    /// abi_decode stays lenient, abi_decode_validate is the enforcement point.
    #[tokio::test]
    async fn test_abi_decode_validate_rejects_empty_word() {
        let encoded = AuthoringMetasV2Sol::abi_encode(&vec![word_sol(&[0u8; 32], "")]);

        assert_eq!(
            AuthoringMetaV2::abi_decode(&encoded).unwrap().words[0].word,
            ""
        );

        match AuthoringMetaV2::abi_decode_validate(&encoded) {
            Err(AuthoringMetaV2Error::ValidationErrors(_)) => {}
            other => panic!("expected ValidationErrors, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_abi_decode_validate_rejects_words_outside_the_symbol_grammar() {
        for word in ["A", "a0A", "0a", "-a", "_", "a b", "a.b"] {
            let encoded = AuthoringMetasV2Sol::abi_encode(&vec![symbol_sol(word, "")]);

            assert!(
                AuthoringMetaV2::abi_decode(&encoded).is_ok(),
                "word '{}' rejected by the lenient decode",
                word
            );

            match AuthoringMetaV2::abi_decode_validate(&encoded) {
                Err(AuthoringMetaV2Error::ValidationErrors(_)) => {}
                other => panic!(
                    "word '{}': expected ValidationErrors, got {:?}",
                    word, other
                ),
            }
        }
    }

    #[tokio::test]
    async fn test_abi_decode_validate_accepts_the_full_symbol_grammar() {
        for word in ["a", "a-", "a-a", "a0", "abc-def-0"] {
            let encoded = AuthoringMetasV2Sol::abi_encode(&vec![symbol_sol(word, "")]);
            assert!(
                AuthoringMetaV2::abi_decode_validate(&encoded).is_ok(),
                "word '{}' rejected",
                word
            );
        }
    }

    #[tokio::test]
    async fn test_abi_decode_validate_rejects_non_printable_description() {
        for description in ["♥", "\u{7f}"] {
            let encoded = AuthoringMetasV2Sol::abi_encode(&vec![symbol_sol("word", description)]);

            assert!(AuthoringMetaV2::abi_decode(&encoded).is_ok());

            match AuthoringMetaV2::abi_decode_validate(&encoded) {
                Err(AuthoringMetaV2Error::ValidationErrors(_)) => {}
                other => panic!(
                    "description '{}': expected ValidationErrors, got {:?}",
                    description, other
                ),
            }
        }
    }

    /// A word set is rejected per index, so the caller can name the bad entry.
    #[tokio::test]
    async fn test_abi_decode_validate_reports_the_offending_index() {
        let encoded = AuthoringMetasV2Sol::abi_encode(&vec![
            symbol_sol("good", "fine"),
            symbol_sol("BAD", "fine"),
        ]);

        let error = match AuthoringMetaV2::abi_decode_validate(&encoded) {
            Err(AuthoringMetaV2Error::ValidationErrors(errors)) => errors,
            other => panic!("expected ValidationErrors, got {:?}", other),
        };

        match error.errors().get("words") {
            Some(ValidationErrorsKind::List(by_index)) => {
                assert!(!by_index.contains_key(&0));
                assert!(by_index[&1].errors().contains_key("word"));
            }
            other => panic!("expected a per-index list, got {:?}", other),
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
}
