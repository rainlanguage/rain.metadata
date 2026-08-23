use std::sync::Arc;
use reqwest::Client;
use alloy::primitives::hex::decode;
use serde::{Deserialize, Serialize};
use graphql_client::{GraphQLQuery, Response, QueryBody};
use super::{
    RainMetaDocumentV1Item, KnownMagic, types::authoring::v1::AuthoringMeta, super::error::Error,
};

type Bytes = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/meta/query/schema.json",
    query_path = "src/meta/query/meta.graphql",
    response_derives = "Debug, Serialize, Deserialize"
)]
pub(super) struct MetaQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/meta/query/schema.json",
    query_path = "src/meta/query/deployer.graphql",
    response_derives = "Debug, Serialize, Deserialize"
)]
pub(super) struct DeployerQuery;

/// response data struct for a meta
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MetaResponse {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

/// response data struct for an ExpressionDeployer
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeployerResponse {
    #[serde(with = "serde_bytes")]
    pub tx_hash: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub bytecode_meta_hash: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub meta_hash: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub meta_bytes: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub bytecode: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub parser: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub store: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub interpreter: Vec<u8>,
}

impl DeployerResponse {
    /// get authoring meta bytes of this deployer meta
    pub fn get_authoring_meta(&self) -> Option<AuthoringMeta> {
        if let Ok(meta_maps) = RainMetaDocumentV1Item::cbor_decode(&self.meta_bytes) {
            for meta_map in &meta_maps {
                if meta_map.magic == KnownMagic::AuthoringMetaV1 {
                    if let Ok(v) = meta_map.unpack() {
                        match AuthoringMeta::abi_decode_validate(&v) {
                            Ok(am) => return Some(am),
                            Err(_) => return None,
                        }
                    }
                }
            }
            None
        } else {
            None
        }
    }
}

/// Process a response for a meta by resolving if a record was found or reject if nothing found or rejected with error
/// This is because graphql responses are not rejected even if there was no record found for the request
pub(super) async fn process_meta_query(
    client: Arc<Client>,
    request_body: &QueryBody<meta_query::Variables>,
    url: &str,
) -> Result<MetaResponse, Error> {
    Ok(MetaResponse {
        bytes: decode(
            client
                .post(url)
                .json(request_body)
                .send()
                .await
                .map_err(Error::ReqwestError)?
                .json::<Response<meta_query::ResponseData>>()
                .await
                .map_err(Error::ReqwestError)?
                .data
                .ok_or(Error::NoRecordFound)?
                .meta
                .ok_or(Error::NoRecordFound)?
                .raw_bytes,
        )
        .or(Err(Error::NoRecordFound))?,
    })
}

/// process a response for a deployer by resolving if a record was found or reject if nothing found or rejected with error
/// This is because graphql responses are not rejected even if there was no record found for the request
pub(super) async fn process_deployer_query(
    client: Arc<Client>,
    request_body: &QueryBody<deployer_query::Variables>,
    url: &str,
) -> Result<DeployerResponse, Error> {
    let res = client
        .post(url)
        .json(request_body)
        .send()
        .await
        .map_err(Error::ReqwestError)?
        .json::<Response<deployer_query::ResponseData>>()
        .await
        .map_err(Error::ReqwestError)?
        .data
        .ok_or(Error::NoRecordFound)?
        .expression_deployers;

    if !res.is_empty() {
        let bytecode = if let Some(v) = &res[0].bytecode {
            decode(v).or(Err(Error::NoRecordFound))?
        } else {
            return Err(Error::NoRecordFound);
        };
        let parser = if let Some(v) = &res[0].parser {
            decode(&v.parser.deployed_bytecode).or(Err(Error::NoRecordFound))?
        } else {
            return Err(Error::NoRecordFound);
        };
        let store = if let Some(v) = &res[0].store {
            decode(&v.store.deployed_bytecode).or(Err(Error::NoRecordFound))?
        } else {
            return Err(Error::NoRecordFound);
        };
        let interpreter = if let Some(v) = &res[0].interpreter {
            decode(&v.interpreter.deployed_bytecode).or(Err(Error::NoRecordFound))?
        } else {
            return Err(Error::NoRecordFound);
        };
        let bytecode_meta_hash = if res[0].meta.len() == 1 {
            decode(&res[0].meta[0].id).or(Err(Error::NoRecordFound))?
        } else {
            return Err(Error::NoRecordFound);
        };
        let tx_hash = if let Some(v) = &res[0].deploy_transaction {
            decode(&v.id).or(Err(Error::NoRecordFound))?
        } else {
            return Err(Error::NoRecordFound);
        };
        let meta_hash = decode(&res[0].constructor_meta_hash).or(Err(Error::NoRecordFound))?;
        let meta_bytes = decode(&res[0].constructor_meta).or(Err(Error::NoRecordFound))?;
        Ok(DeployerResponse {
            meta_hash,
            meta_bytes,
            bytecode,
            parser,
            store,
            interpreter,
            bytecode_meta_hash,
            tx_hash,
        })
    } else {
        Err(Error::NoRecordFound)
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use super::super::{
        types::authoring::v1::AuthoringMetaItem, ContentEncoding, ContentLanguage, ContentType,
    };
    use httpmock::Method::POST;
    use httpmock::MockServer;

    fn request_body(hash: &str) -> QueryBody<meta_query::Variables> {
        MetaQuery::build_query(meta_query::Variables {
            hash: Some(hash.to_string()),
        })
    }

    const HASH: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";

    /// A found meta resolves to exactly the hex-decoded rawBytes of the
    /// response, fetched with a POST carrying the query body.
    #[tokio::test]
    async fn test_process_meta_query_success_exact_bytes() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .json_body_partial(format!(r#"{{"variables":{{"hash":"{}"}}}}"#, HASH));
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data":{"meta":{"__typename":"RainMetaV1","rawBytes":"0xff0a89c674ee7874deadbeef"}}}"#);
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await
        .unwrap();
        assert_eq!(
            result.bytes,
            vec![0xff, 0x0a, 0x89, 0xc6, 0x74, 0xee, 0x78, 0x74, 0xde, 0xad, 0xbe, 0xef]
        );
    }

    /// A response with no data member at all is "no record found".
    #[tokio::test]
    async fn test_process_meta_query_missing_data_is_no_record_found() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data":null}"#);
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await;
        assert!(matches!(result, Err(Error::NoRecordFound)), "{result:?}");
    }

    /// A response with data but a null meta is "no record found".
    #[tokio::test]
    async fn test_process_meta_query_missing_meta_is_no_record_found() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data":{"meta":null}}"#);
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await;
        assert!(matches!(result, Err(Error::NoRecordFound)), "{result:?}");
    }

    /// rawBytes that do not hex-decode resolve to "no record found",
    /// never to successfully-returned bytes.
    #[tokio::test]
    async fn test_process_meta_query_bad_hex_is_no_record_found() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data":{"meta":{"__typename":"RainMetaV1","rawBytes":"zz-not-hex"}}}"#);
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await;
        assert!(matches!(result, Err(Error::NoRecordFound)), "{result:?}");
    }

    /// A transport failure surfaces as a reqwest error, not as a
    /// no-record-found result.
    #[tokio::test]
    async fn test_process_meta_query_send_error_is_reqwest_error() {
        // Bind and immediately release a local port so the request targets a
        // closed port.
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let url = format!("http://127.0.0.1:{port}/");
        let result =
            process_meta_query(Arc::new(Client::new()), &request_body(HASH), &url).await;
        assert!(matches!(result, Err(Error::ReqwestError(_))), "{result:?}");
    }

    /// A non-JSON response body surfaces as a reqwest decode error, not as
    /// a no-record-found result.
    #[tokio::test]
    async fn test_process_meta_query_non_json_is_reqwest_error() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "text/plain")
                .body("not json");
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await;
        assert!(matches!(result, Err(Error::ReqwestError(_))), "{result:?}");
    }

    fn authoring_meta() -> AuthoringMeta {
        AuthoringMeta(vec![AuthoringMetaItem {
            word: "some-word".to_string(),
            operand_parser_offset: 0,
            description: "a description".to_string(),
        }])
    }

    fn authoring_item() -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(
                authoring_meta().abi_encode_validate().unwrap(),
            ),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
    }

    fn deployer_response_with(meta_bytes: Vec<u8>) -> DeployerResponse {
        DeployerResponse {
            tx_hash: vec![],
            bytecode_meta_hash: vec![],
            meta_hash: vec![],
            meta_bytes,
            bytecode: vec![],
            parser: vec![],
            store: vec![],
            interpreter: vec![],
        }
    }

    /// An authoring-magic item whose payload fails to unpack is skipped;
    /// a later valid authoring meta item is still found.
    #[test]
    fn test_get_authoring_meta_skips_unpack_failure() {
        // Deflate-encoded item whose payload is not valid deflate data.
        let bad_unpack = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(vec![0xffu8, 0xff, 0xff, 0xff]),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::None,
            schema: None,
        };
        assert!(bad_unpack.unpack().is_err());
        let meta_bytes = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![bad_unpack, authoring_item()],
            KnownMagic::RainMetaDocumentV1,
        )
        .unwrap();
        assert_eq!(
            deployer_response_with(meta_bytes).get_authoring_meta(),
            Some(authoring_meta())
        );
    }

    /// The scan covers every item in the document: an authoring meta in the
    /// second position is found behind a non-authoring first item.
    #[test]
    fn test_get_authoring_meta_scans_beyond_first_item() {
        let other_magic = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(b"_: int-add(1 2);".to_vec()),
            magic: KnownMagic::RainlangV1,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let meta_bytes = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![other_magic, authoring_item()],
            KnownMagic::RainMetaDocumentV1,
        )
        .unwrap();
        assert_eq!(
            deployer_response_with(meta_bytes).get_authoring_meta(),
            Some(authoring_meta())
        );
    }
}
