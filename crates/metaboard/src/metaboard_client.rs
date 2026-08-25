use crate::cynic_client::{CynicClient, CynicClientError};
use crate::types::metas::*;
use alloy::primitives::{
    hex::{decode, encode, FromHexError},
    Address,
};
use core::str::FromStr;
use reqwest::Url;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetaboardSubgraphClientError {
    #[error("Request Error for metahash {metahash}: {source}")]
    RequestErrorByHash {
        metahash: String,
        #[source]
        source: CynicClientError,
    },
    #[error("Request Error for subject {subject}: {source}")]
    RequestErrorBySubject {
        subject: String,
        #[source]
        source: CynicClientError,
    },
    #[error("Subgraph query returned no data for metahash {0}")]
    Empty(String),
    #[error("Error decoding metahash {metahash}: {source}")]
    FromHexError {
        metahash: String,
        #[source]
        source: FromHexError,
    },
    #[error("Error parsing metaboard address {address}: {source}")]
    AddressParseError {
        address: String,
        #[source]
        source: <Address as FromStr>::Err,
    },
    #[error("Request error fetching metaboard addresses: {source}")]
    RequestErrorMetaBoards {
        #[source]
        source: CynicClientError,
    },
}

pub struct MetaboardSubgraphClient {
    url: Url,
}

impl CynicClient for MetaboardSubgraphClient {
    fn get_base_url(&self) -> Url {
        self.url.clone()
    }
}

impl MetaboardSubgraphClient {
    pub fn new(url: Url) -> Self {
        Self { url }
    }

    /// Find all metas with a given hash
    pub async fn get_metabytes_by_hash(
        &self,
        metahash: &[u8; 32],
    ) -> Result<Vec<Vec<u8>>, MetaboardSubgraphClientError> {
        let hex_string = encode(metahash);
        let metahash = format!("0x{}", hex_string);

        let data = self
            .query::<MetasByHash, MetasByHashVariables>(MetasByHashVariables {
                metahash: Some(Bytes(metahash.clone())),
            })
            .await
            .map_err(|e| MetaboardSubgraphClientError::RequestErrorByHash {
                metahash: metahash.clone(),
                source: e,
            })?;

        if data.meta_v1_s.is_empty() {
            return Err(MetaboardSubgraphClientError::Empty(metahash));
        }

        // decode all the metas
        let mut meta_bytes = Vec::new();
        for meta in data.meta_v1_s {
            meta_bytes.push(decode(&meta.meta.0).map_err(|e| {
                MetaboardSubgraphClientError::FromHexError {
                    metahash: metahash.clone(),
                    source: e,
                }
            })?);
        }

        Ok(meta_bytes)
    }

    /// Find all metas with a given subject
    pub async fn get_metabytes_by_subject(
        &self,
        subject: &Bytes,
    ) -> Result<Vec<Vec<u8>>, MetaboardSubgraphClientError> {
        let data = self
            .query::<MetasBySubject, MetasBySubjectVariables>(MetasBySubjectVariables {
                subject: Some(subject.clone()),
            })
            .await
            .map_err(|e| MetaboardSubgraphClientError::RequestErrorBySubject {
                subject: subject.0.clone(),
                source: e,
            })?;

        if data.meta_v1_s.is_empty() {
            return Err(MetaboardSubgraphClientError::Empty(subject.0.clone()));
        }

        // decode all the metas
        let mut meta_bytes = Vec::new();
        for meta in data.meta_v1_s {
            meta_bytes.push(decode(&meta.meta.0).map_err(|e| {
                MetaboardSubgraphClientError::FromHexError {
                    metahash: encode(&meta.meta_hash.0),
                    source: e,
                }
            })?);
        }

        Ok(meta_bytes)
    }

    /// Fetch MetaBoard contract addresses from the subgraph.
    pub async fn get_metaboard_addresses(
        &self,
        first: Option<i32>,
        skip: Option<i32>,
    ) -> Result<Vec<Address>, MetaboardSubgraphClientError> {
        let data =
            self.query::<MetaBoardAddresses, MetaBoardAddressesVariables>(
                MetaBoardAddressesVariables { first, skip },
            )
            .await
            .map_err(|e| MetaboardSubgraphClientError::RequestErrorMetaBoards { source: e })?;

        let mut addresses = Vec::with_capacity(data.meta_boards.len());
        for board in data.meta_boards {
            let address_hex = board.address.0;
            let address = Address::from_str(&address_hex).map_err(|e| {
                MetaboardSubgraphClientError::AddressParseError {
                    address: address_hex.clone(),
                    source: e,
                }
            })?;

            addresses.push(address);
        }

        Ok(addresses)
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use alloy::primitives::hex::encode;
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use reqwest::Url;

    //
    // By hash
    //
    #[tokio::test]
    async fn test_get_metabytes_by_hash_success() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let hash = [1u8; 32];

        // Mock a successful response. body_contains pins the wire shape: the
        // hash must be sent 0x-prefixed and the query must filter on the
        // metaHash field (rendering verified against cynic's output).
        server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains(format!("0x{}", encode(hash)))
                .body_contains("where: {metaHash: $metahash}");
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

        let client = MetaboardSubgraphClient::new(url);

        let result = client.get_metabytes_by_hash(&hash).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![1]);
        assert_eq!(result[1], vec![2]);
    }

    #[tokio::test]
    async fn test_get_metabytes_by_hash_empty() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        // Mock an empty response
        server.mock(|when, then| {
            when.method(POST).path("/").body_contains("metahash");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": []
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let hash = [0u8; 32];

        let result = client.get_metabytes_by_hash(&hash).await;

        assert!(result.is_err());
        match result {
            Err(MetaboardSubgraphClientError::Empty(metahash)) => {
                assert_eq!(metahash, format!("0x{}", encode(hash)));
            }
            _ => panic!("Unexpected result: {:?}", result),
        }
    }

    //
    // By subject
    //
    #[tokio::test]
    async fn test_get_metabytes_by_subject_success() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let subject = Bytes("0x7b".to_string());

        // Mock a successful response. body_contains pins the wire shape:
        // the subject Bytes value must be sent verbatim in the request
        // (not coerced to a number, not stripped of `0x`).
        server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("where: {subject: $subject}")
                .body_contains("0x7b");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                                "meta": "0x03",
                                "metaHash": "0x01",
                                "sender": "0x02",
                                "id": "0x01",
                                "metaBoard": {
                                    "address": "0x01",
                                },
                                "subject": "123",
                            },
                            {
                                "meta": "0x04",
                                "metaHash": "0x02",
                                "sender": "0x03",
                                "id": "0x02",
                                "metaBoard": {
                                    "address": "0x02",
                                },
                                "subject": "456",
                               }
                        ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);

        let result = client.get_metabytes_by_subject(&subject).await;

        let result = result.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![3]);
        assert_eq!(result[1], vec![4]);
    }

    #[tokio::test]
    async fn test_get_metabytes_by_subject_empty() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        // Mock an empty response
        server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": []
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let subject = Bytes("0x315".to_string());

        let result = client.get_metabytes_by_subject(&subject).await;

        assert!(result.is_err());
        match result {
            Err(MetaboardSubgraphClientError::Empty(s)) => assert_eq!(s, "0x315"),
            _ => panic!("Unexpected result: {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_get_metaboard_addresses_success() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        // body_contains pins the wire shape: the query paginates with
        // first/skip in that argument order, and the variables carry the
        // caller's first=10, skip=0 under their own names.
        server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .body_contains("metaBoards(first: $first, skip: $skip)")
                .body_contains("\"first\":10")
                .body_contains("\"skip\":0");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaBoards": [
                            {
                                "address": "0x0000000000000000000000000000000000000001",
                            },
                            {
                                "address": "0x0000000000000000000000000000000000000002",
                            }
                        ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);

        let result = client
            .get_metaboard_addresses(Some(10), Some(0))
            .await
            .unwrap();

        assert_eq!(result.len(), 2);

        assert_eq!(
            result[0],
            Address::from_str("0x0000000000000000000000000000000000000001").unwrap()
        );
        assert_eq!(
            result[1],
            Address::from_str("0x0000000000000000000000000000000000000002").unwrap()
        );
    }

    #[tokio::test]
    async fn test_get_metaboard_addresses_empty() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        server.mock(|when, then| {
            when.method(POST).path("/").body_contains("metaBoards");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaBoards": []
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);

        let result = client.get_metaboard_addresses(Some(5), None).await.unwrap();

        assert!(result.is_empty());
    }
    //
    // CynicClient error surface
    //

    /// A response carrying a graphql errors array is a GraphqlError carrying
    /// those errors, never silently treated as data.
    #[tokio::test]
    async fn test_get_metabytes_by_hash_graphql_error() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let hash = [7u8; 32];

        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": { "metaV1S": [] },
                    "errors": [ { "message": "boom" } ]
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metabytes_by_hash(&hash).await;

        match result {
            Err(MetaboardSubgraphClientError::RequestErrorByHash {
                metahash,
                source: CynicClientError::GraphqlError(errors),
            }) => {
                assert_eq!(metahash, format!("0x{}", encode(hash)));
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].message, "boom");
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// A response with null data and no errors never reaches the
    /// `data.ok_or(Empty)` arm: cynic's `GraphQlResponse` deserializer
    /// rejects any body without data or errors ("Either data or errors must
    /// be present in a GraphQL response"), so it surfaces as a Request
    /// decode error. This pins the deserializer boundary and documents that
    /// `CynicClientError::Empty` is unreachable from `query` while that
    /// deserializer holds (see the audit issue on the dead arm).
    #[tokio::test]
    async fn test_get_metabytes_by_hash_null_data_is_request_decode_error() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let hash = [9u8; 32];

        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .json_body_obj(&serde_json::json!({ "data": null }));
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metabytes_by_hash(&hash).await;

        match result {
            Err(MetaboardSubgraphClientError::RequestErrorByHash {
                metahash,
                source: CynicClientError::Request(e),
            }) => {
                assert_eq!(metahash, format!("0x{}", encode(hash)));
                assert!(e.is_decode(), "expected a decode error: {:?}", e);
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// A transport failure surfaces as CynicClientError::Request, not as
    /// Empty or a graphql error.
    #[tokio::test]
    async fn test_get_metabytes_by_hash_connection_error_is_request() {
        // Nothing listens here: the connection is refused immediately.
        let url = Url::parse("http://127.0.0.1:1/").unwrap();
        let client = MetaboardSubgraphClient::new(url);
        let hash = [1u8; 32];

        let result = client.get_metabytes_by_hash(&hash).await;

        match result {
            Err(MetaboardSubgraphClientError::RequestErrorByHash {
                metahash,
                source: CynicClientError::Request(_),
            }) => {
                assert_eq!(metahash, format!("0x{}", encode(hash)));
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// A non-2xx response is a Status error naming the code and carrying the
    /// body, never a decode error indistinguishable from a malformed 200.
    #[tokio::test]
    async fn test_get_metabytes_by_hash_http_error_status() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let hash = [3u8; 32];

        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(404).body("no such subgraph");
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metabytes_by_hash(&hash).await;

        match result {
            Err(MetaboardSubgraphClientError::RequestErrorByHash {
                metahash,
                source: CynicClientError::Status { status, body },
            }) => {
                assert_eq!(metahash, format!("0x{}", encode(hash)));
                assert_eq!(status.as_u16(), 404);
                assert_eq!(body, "no such subgraph");
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// A non-2xx body that does decode as a graphql response is still a Status
    /// error: its data is never handed back as if the query had succeeded.
    #[tokio::test]
    async fn test_get_metabytes_by_subject_http_error_status_with_decodable_body() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let subject = Bytes("0x7d".to_string());

        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(500).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                                "meta": "0x05",
                                "metaHash": "0x00",
                                "sender": "0x00",
                                "id": "0x00",
                                "metaBoard": { "address": "0x00" },
                                "subject": "0x7d",
                            }
                        ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metabytes_by_subject(&subject).await;

        match result {
            Err(MetaboardSubgraphClientError::RequestErrorBySubject {
                subject,
                source: CynicClientError::Status { status, .. },
            }) => {
                assert_eq!(subject, "0x7d");
                assert_eq!(status.as_u16(), 500);
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// The gate is any 2xx, not 200 alone: a 202 carrying a graphql response
    /// decodes as normal.
    #[tokio::test]
    async fn test_get_metaboard_addresses_non_200_success_status() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        server.mock(|when, then| {
            when.method(POST).path("/").body_contains("metaBoards");
            then.status(202).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaBoards": [
                            { "address": "0x0000000000000000000000000000000000000003" }
                        ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metaboard_addresses(None, None).await.unwrap();

        assert_eq!(
            result,
            vec![Address::from_str("0x0000000000000000000000000000000000000003").unwrap()]
        );
    }

    /// A meta whose hex payload does not decode is a FromHexError keyed by
    /// the queried hash, never silently dropped.
    #[tokio::test]
    async fn test_get_metabytes_by_hash_invalid_hex_meta() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let hash = [2u8; 32];

        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                                "meta": "0xzz",
                                "metaHash": "0x00",
                                "sender": "0x00",
                                "id": "0x00",
                                "metaBoard": { "address": "0x00" },
                                "subject": "0x00",
                            }
                        ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metabytes_by_hash(&hash).await;

        match result {
            Err(MetaboardSubgraphClientError::FromHexError { metahash, .. }) => {
                assert_eq!(metahash, format!("0x{}", encode(hash)));
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// Same failure on the subject path: a FromHexError, keyed by the
    /// offending meta's own hash rather than by the subject.
    /// NOTE: the exact key text is NOT pinned here because the current
    /// implementation hex-encodes the UTF-8 bytes of the (already hex) hash
    /// string; see the audit issue on the double encoding. The variant and a
    /// non-empty key are the undisputed part of the contract.
    #[tokio::test]
    async fn test_get_metabytes_by_subject_invalid_hex_meta() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        let subject = Bytes("0x7c".to_string());

        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                                "meta": "0xzz",
                                "metaHash": "0xabcd",
                                "sender": "0x00",
                                "id": "0x00",
                                "metaBoard": { "address": "0x00" },
                                "subject": "0x7c",
                            }
                        ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metabytes_by_subject(&subject).await;

        match result {
            Err(MetaboardSubgraphClientError::FromHexError { metahash, .. }) => {
                assert!(!metahash.is_empty());
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    /// A board address that does not parse is an AddressParseError carrying
    /// the offending string verbatim, never silently defaulted.
    #[tokio::test]
    async fn test_get_metaboard_addresses_invalid_address() {
        let server = MockServer::start_async().await;
        let url = Url::parse(&server.url("/")).unwrap();

        server.mock(|when, then| {
            when.method(POST).path("/").body_contains("metaBoards");
            then.status(200).json_body_obj(&{
                serde_json::json!({
                    "data": {
                        "metaBoards": [ { "address": "not-an-address" } ]
                    }
                })
            });
        });

        let client = MetaboardSubgraphClient::new(url);
        let result = client.get_metaboard_addresses(None, None).await;

        match result {
            Err(MetaboardSubgraphClientError::AddressParseError { address, .. }) => {
                assert_eq!(address, "not-an-address");
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }
}
