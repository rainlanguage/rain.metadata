use alloy::sol_types::SolType;
use alloy_ethers_typecast::transaction::{
    ReadContractParametersBuilder, ReadContractParametersBuilderError, ReadableClient,
    ReadableClientError,
};
use alloy::primitives::hex::FromHexError;
use alloy::sol_types::private::Address;
use alloy::sol;
use rain_metaboard_subgraph::metaboard_client::*;
use serde::{Deserialize, Serialize};
use crate::meta::{KnownMagic, RainMetaDocumentV1Item};
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
    ReadableClientError(#[from] ReadableClientError),
    #[error(transparent)]
    ReadContractParametersError(#[from] ReadContractParametersBuilderError),
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
        let decoded = AuthoringMetasV2Sol::abi_decode(bytes, true)?;

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
        // get the metahash
        let client = ReadableClient::new_from_urls(rpcs.clone()).map_err(|error| {
            FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            }
        })?;

        // return "has no words" error if the contract does not implement IDescribeByMetaV2 interface
        if !implements_i_described_by_meta_v1(&client, contract_address).await {
            return Err(FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: AuthoringMetaV2Error::HasNoWords,
            });
        }

        let parameters = ReadContractParametersBuilder::default()
            .address(contract_address)
            .call(IDescribedByMetaV1::describedByMetaV1Call {})
            .build()
            .map_err(|error| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            })?;
        let metahash = client
            .read(parameters)
            .await
            .map_err(|error| FetchAuthoringMetaV2WordError {
                contract_address,
                rpcs: rpcs.clone(),
                metaboard_url: metaboard_url.clone(),
                error: error.into(),
            })?
            ._0;

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

        let meta = RainMetaDocumentV1Item::cbor_decode(metas[0].as_slice()).map_err(|error| {
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

#[cfg(test)]
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
                    metaboard_url,
                    error,
                } = error;
                assert_eq!(contract_address, Address::from([0u8; 20]));
                assert_eq!(rpcs, vec![rpc_url.to_string()]);
                assert_eq!(metaboard_url, metaboard_url.to_string());
                match error {
                    AuthoringMetaV2Error::HasNoWords => {}
                    _ => panic!("Unexpected error: {:?}", error),
                }
            }
        }
    }
}
