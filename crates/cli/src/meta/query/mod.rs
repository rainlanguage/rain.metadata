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

/// A graphql response carries an `errors` member independently of `data`, and
/// serves an empty `errors` array to mean no errors at all.
fn response_data<T>(response: Response<T>) -> Result<T, Error> {
    if let Some(errors) = response.errors.filter(|errors| !errors.is_empty()) {
        return Err(Error::SubgraphError(
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("; "),
        ));
    }
    response
        .data
        .ok_or_else(|| Error::SubgraphError("response carried neither data nor errors".to_string()))
}

/// A field of a record that was found: absent or unparsable makes the record
/// corrupt, never absent.
fn decode_field(field: &str, value: Option<&str>) -> Result<Vec<u8>, Error> {
    match value {
        Some(value) => decode(value).map_err(|e| Error::CorruptRecord(format!("{}: {}", field, e))),
        None => Err(Error::CorruptRecord(format!("{} is missing", field))),
    }
}

/// Process a response for a meta by resolving the record it holds, rejecting a
/// subgraph that has no such record as `NoRecordFound` and one that cannot
/// serve the record it has as `CorruptRecord`.
/// This is because graphql responses are not rejected even if there was no record found for the request
pub(super) async fn process_meta_query(
    client: Arc<Client>,
    request_body: &QueryBody<meta_query::Variables>,
    url: &str,
) -> Result<MetaResponse, Error> {
    let raw_bytes = response_data(
        client
            .post(url)
            .json(request_body)
            .send()
            .await
            .map_err(Error::ReqwestError)?
            .json::<Response<meta_query::ResponseData>>()
            .await
            .map_err(Error::ReqwestError)?,
    )?
    .meta
    .ok_or(Error::NoRecordFound)?
    .raw_bytes;

    Ok(MetaResponse {
        bytes: decode_field("rawBytes", Some(raw_bytes.as_str()))?,
    })
}

/// process a response for a deployer by resolving the record it holds, rejecting
/// a subgraph that has no such record as `NoRecordFound` and one that cannot
/// serve the record it has as `CorruptRecord`.
/// This is because graphql responses are not rejected even if there was no record found for the request
pub(super) async fn process_deployer_query(
    client: Arc<Client>,
    request_body: &QueryBody<deployer_query::Variables>,
    url: &str,
) -> Result<DeployerResponse, Error> {
    let res = response_data(
        client
            .post(url)
            .json(request_body)
            .send()
            .await
            .map_err(Error::ReqwestError)?
            .json::<Response<deployer_query::ResponseData>>()
            .await
            .map_err(Error::ReqwestError)?,
    )?
    .expression_deployers;

    let deployer = res.first().ok_or(Error::NoRecordFound)?;

    let bytecode_meta_hash = match deployer.meta.as_slice() {
        [meta] => decode_field("meta[0].id", Some(meta.id.as_str()))?,
        metas => {
            return Err(Error::CorruptRecord(format!(
                "expected exactly one meta, got {}",
                metas.len()
            )))
        }
    };

    Ok(DeployerResponse {
        meta_hash: decode_field(
            "constructorMetaHash",
            Some(deployer.constructor_meta_hash.as_str()),
        )?,
        meta_bytes: decode_field("constructorMeta", Some(deployer.constructor_meta.as_str()))?,
        bytecode: decode_field("bytecode", deployer.bytecode.as_deref())?,
        parser: decode_field(
            "parser",
            deployer
                .parser
                .as_ref()
                .map(|v| v.parser.deployed_bytecode.as_str()),
        )?,
        store: decode_field(
            "store",
            deployer
                .store
                .as_ref()
                .map(|v| v.store.deployed_bytecode.as_str()),
        )?,
        interpreter: decode_field(
            "interpreter",
            deployer
                .interpreter
                .as_ref()
                .map(|v| v.interpreter.deployed_bytecode.as_str()),
        )?,
        bytecode_meta_hash,
        tx_hash: decode_field(
            "deployTransaction",
            deployer.deploy_transaction.as_ref().map(|v| v.id.as_str()),
        )?,
    })
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::meta::types::authoring::v1::AuthoringMetaItem;
    use crate::meta::{ContentEncoding, ContentLanguage, ContentType};
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use serde_bytes::ByteBuf;
    use serde_json::json;

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

    /// A response with no data member and no errors member violates the
    /// graphql response shape: that is the subgraph failing, not absence.
    #[tokio::test]
    async fn test_process_meta_query_missing_data_is_subgraph_error() {
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
        match result {
            Err(Error::SubgraphError(message)) => {
                assert_eq!(message, "response carried neither data nor errors")
            }
            other => panic!("expected subgraph error, got {other:?}"),
        }
    }

    /// Top level graphql errors are the subgraph rejecting the query, and are
    /// reported as such rather than as absence, with every message carried.
    #[tokio::test]
    async fn test_process_meta_query_graphql_errors_is_subgraph_error() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data":null,"errors":[{"message":"first"},{"message":"second"}]}"#);
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await;
        match result {
            Err(Error::SubgraphError(message)) => {
                assert!(message.contains("first"), "{message}");
                assert!(message.contains("second"), "{message}");
            }
            other => panic!("expected subgraph error, got {other:?}"),
        }
    }

    /// An empty errors array is the graphql wire form for "no errors": it must
    /// not turn a served record into a failure.
    #[tokio::test]
    async fn test_process_meta_query_empty_errors_array_is_success() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{"data":{"meta":{"__typename":"RainMetaV1","rawBytes":"0x0102"}},"errors":[]}"#,
                );
        });
        let result = process_meta_query(
            Arc::new(Client::new()),
            &request_body(HASH),
            &server.url("/"),
        )
        .await
        .unwrap();
        assert_eq!(result.bytes, vec![0x01, 0x02]);
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

    /// rawBytes that do not hex-decode are a record the subgraph has but
    /// cannot serve intact: corrupt, never absent and never bytes.
    #[tokio::test]
    async fn test_process_meta_query_bad_hex_is_corrupt_record() {
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
        match result {
            Err(Error::CorruptRecord(message)) => {
                assert!(message.starts_with("rawBytes: "), "{message}")
            }
            other => panic!("expected corrupt record, got {other:?}"),
        }
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
        let result = process_meta_query(Arc::new(Client::new()), &request_body(HASH), &url).await;
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
            payload: serde_bytes::ByteBuf::from(authoring_meta().abi_encode_validate().unwrap()),
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

    /// A fully populated, valid expressionDeployers entry as the subgraph
    /// would return it: every hex field decodes and exactly one meta.
    fn deployer_entry() -> serde_json::Value {
        json!({
            "constructorMetaHash": "0x0102",
            "constructorMeta": "0x0304",
            "deployTransaction": { "id": "0x0506" },
            "bytecode": "0x0708",
            "parser": { "parser": { "deployedBytecode": "0x090a" } },
            "store": { "store": { "deployedBytecode": "0x0b0c" } },
            "interpreter": { "interpreter": { "deployedBytecode": "0x0d0e" } },
            "meta": [ { "__typename": "RainMetaV1", "id": "0x0f10" } ]
        })
    }

    async fn run_deployer_query(entry: serde_json::Value) -> Result<DeployerResponse, Error> {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .json_body(json!({ "data": { "expressionDeployers": [entry] } }));
        });
        let request_body = DeployerQuery::build_query(deployer_query::Variables {
            hash: Some("0xabcd".to_string()),
        });
        let client = Arc::new(Client::new());
        process_deployer_query(client, &request_body, &server.url("/")).await
    }

    #[tokio::test]
    async fn test_process_deployer_query_success_decodes_all_fields() {
        let res = run_deployer_query(deployer_entry()).await.unwrap();
        assert_eq!(res.meta_hash, vec![0x01, 0x02]);
        assert_eq!(res.meta_bytes, vec![0x03, 0x04]);
        assert_eq!(res.tx_hash, vec![0x05, 0x06]);
        assert_eq!(res.bytecode, vec![0x07, 0x08]);
        assert_eq!(res.parser, vec![0x09, 0x0a]);
        assert_eq!(res.store, vec![0x0b, 0x0c]);
        assert_eq!(res.interpreter, vec![0x0d, 0x0e]);
        assert_eq!(res.bytecode_meta_hash, vec![0x0f, 0x10]);
    }

    /// A deployer record that was found but cannot be served intact is
    /// corrupt: the message names the offending field.
    async fn deployer_corrupt_record(entry: serde_json::Value) -> String {
        match run_deployer_query(entry).await {
            Err(Error::CorruptRecord(message)) => message,
            other => panic!("expected corrupt record, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_bytecode_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["bytecode"] = serde_json::Value::Null;
        assert_eq!(deployer_corrupt_record(entry).await, "bytecode is missing");
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_parser_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["parser"] = serde_json::Value::Null;
        assert_eq!(deployer_corrupt_record(entry).await, "parser is missing");
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_store_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["store"] = serde_json::Value::Null;
        assert_eq!(deployer_corrupt_record(entry).await, "store is missing");
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_interpreter_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["interpreter"] = serde_json::Value::Null;
        assert_eq!(
            deployer_corrupt_record(entry).await,
            "interpreter is missing"
        );
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_deploy_transaction_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["deployTransaction"] = serde_json::Value::Null;
        assert_eq!(
            deployer_corrupt_record(entry).await,
            "deployTransaction is missing"
        );
    }

    #[tokio::test]
    async fn test_process_deployer_query_zero_metas_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["meta"] = json!([]);
        assert_eq!(
            deployer_corrupt_record(entry).await,
            "expected exactly one meta, got 0"
        );
    }

    #[tokio::test]
    async fn test_process_deployer_query_two_metas_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["meta"] = json!([
            { "__typename": "RainMetaV1", "id": "0x0f10" },
            { "__typename": "RainMetaV1", "id": "0x1112" }
        ]);
        assert_eq!(
            deployer_corrupt_record(entry).await,
            "expected exactly one meta, got 2"
        );
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_bytecode_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["bytecode"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("bytecode: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_parser_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["parser"]["parser"]["deployedBytecode"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("parser: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_store_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["store"]["store"]["deployedBytecode"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("store: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_interpreter_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["interpreter"]["interpreter"]["deployedBytecode"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("interpreter: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_meta_id_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["meta"][0]["id"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("meta[0].id: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_tx_id_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["deployTransaction"]["id"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("deployTransaction: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_constructor_meta_hash_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["constructorMetaHash"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("constructorMetaHash: "), "{message}");
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_constructor_meta_hex_is_corrupt_record() {
        let mut entry = deployer_entry();
        entry["constructorMeta"] = json!("0xZZ");
        let message = deployer_corrupt_record(entry).await;
        assert!(message.starts_with("constructorMeta: "), "{message}");
    }

    /// A deployer query the subgraph rejects is a subgraph error, not the
    /// absence of a deployer record.
    #[tokio::test]
    async fn test_process_deployer_query_graphql_errors_is_subgraph_error() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .json_body(json!({ "errors": [ { "message": "deployer boom" } ] }));
        });
        let request_body = DeployerQuery::build_query(deployer_query::Variables {
            hash: Some("0xabcd".to_string()),
        });
        let result =
            process_deployer_query(Arc::new(Client::new()), &request_body, &server.url("/")).await;
        match result {
            Err(Error::SubgraphError(message)) => {
                assert!(message.contains("deployer boom"), "{message}")
            }
            other => panic!("expected subgraph error, got {other:?}"),
        }
    }

    fn sample_authoring_meta() -> (AuthoringMeta, Vec<u8>) {
        let authoring_meta: AuthoringMeta = serde_json::from_str(
            r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#,
        )
        .unwrap();
        let abi = authoring_meta.abi_encode_validate().unwrap();
        (authoring_meta, abi)
    }

    fn authoring_doc(abi_payload: Vec<u8>, encoding: ContentEncoding) -> Vec<u8> {
        let payload = encoding.encode(&abi_payload);
        let item = RainMetaDocumentV1Item {
            payload: ByteBuf::from(payload),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Cbor,
            content_encoding: encoding,
            content_language: ContentLanguage::None,
            schema: None,
        };
        RainMetaDocumentV1Item::cbor_encode_seq(&vec![item], KnownMagic::RainMetaDocumentV1)
            .unwrap()
    }

    fn deployer_response(meta_bytes: Vec<u8>) -> DeployerResponse {
        DeployerResponse {
            tx_hash: vec![0x01],
            bytecode_meta_hash: vec![0x02],
            meta_hash: vec![0x03],
            meta_bytes,
            bytecode: vec![0x04],
            parser: vec![0x05],
            store: vec![0x06],
            interpreter: vec![0x07],
        }
    }

    /// A document carrying a valid AuthoringMetaV1 item yields the decoded
    /// authoring meta.
    #[test]
    fn test_get_authoring_meta_found() {
        let (authoring_meta, abi) = sample_authoring_meta();
        let response = deployer_response(authoring_doc(abi, ContentEncoding::None));
        assert_eq!(response.get_authoring_meta(), Some(authoring_meta));
    }

    /// The payload is unpacked per its content encoding before abi decoding:
    /// a deflate encoded authoring meta still decodes.
    #[test]
    fn test_get_authoring_meta_deflate_unpack() {
        let (authoring_meta, abi) = sample_authoring_meta();
        let response = deployer_response(authoring_doc(abi, ContentEncoding::Deflate));
        assert_eq!(response.get_authoring_meta(), Some(authoring_meta));
    }

    /// An authoring item that decodes but fails validation, or does not abi
    /// decode at all, yields None.
    #[test]
    fn test_get_authoring_meta_invalid_returns_none() {
        let invalid = AuthoringMeta(vec![AuthoringMetaItem {
            word: "NOTKEBAB".to_string(),
            operand_parser_offset: 0,
            description: "some description".to_string(),
        }]);
        let abi = invalid.abi_encode().unwrap();
        let response = deployer_response(authoring_doc(abi, ContentEncoding::None));
        assert_eq!(response.get_authoring_meta(), None);

        let undecodable = deployer_response(authoring_doc(
            vec![0xde, 0xad, 0xbe, 0xef],
            ContentEncoding::None,
        ));
        assert_eq!(undecodable.get_authoring_meta(), None);
    }

    /// A document without an AuthoringMetaV1 item, or meta bytes that do not
    /// cbor decode, yield None.
    #[test]
    fn test_get_authoring_meta_absent_or_undecodable() {
        let item = RainMetaDocumentV1Item {
            payload: ByteBuf::from("some dotrain".as_bytes()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let doc =
            RainMetaDocumentV1Item::cbor_encode_seq(&vec![item], KnownMagic::RainMetaDocumentV1)
                .unwrap();
        assert_eq!(deployer_response(doc).get_authoring_meta(), None);

        assert_eq!(
            deployer_response(vec![0xff, 0x00, 0x12]).get_authoring_meta(),
            None
        );
    }

    /// process_meta_query separates a rejected query, a genuinely absent
    /// record and a malformed one, and decodes a found record.
    #[tokio::test]
    async fn test_process_meta_query_paths() {
        use httpmock::prelude::*;
        let client = Arc::new(Client::builder().build().unwrap());
        let body = MetaQuery::build_query(meta_query::Variables {
            hash: Some("0xabc".to_string()),
        });
        let server = MockServer::start();

        let no_data = server.mock(|when, then| {
            when.method(POST).path("/nodata");
            then.status(200)
                .json_body(serde_json::json!({"errors": [{"message": "nope"}]}));
        });
        let result = process_meta_query(client.clone(), &body, &server.url("/nodata")).await;
        assert!(matches!(result, Err(Error::SubgraphError(_))), "{result:?}");
        no_data.assert();

        let _no_meta = server.mock(|when, then| {
            when.method(POST).path("/nometa");
            then.status(200)
                .json_body(serde_json::json!({"data": {"meta": null}}));
        });
        let result = process_meta_query(client.clone(), &body, &server.url("/nometa")).await;
        assert!(matches!(result, Err(Error::NoRecordFound)), "{result:?}");

        let _bad_hex = server.mock(|when, then| {
            when.method(POST).path("/badhex");
            then.status(200).json_body(serde_json::json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": "0xzz"}}
            }));
        });
        let result = process_meta_query(client.clone(), &body, &server.url("/badhex")).await;
        assert!(matches!(result, Err(Error::CorruptRecord(_))), "{result:?}");

        let _found = server.mock(|when, then| {
            when.method(POST).path("/found");
            then.status(200).json_body(serde_json::json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": "0x0102"}}
            }));
        });
        let result = process_meta_query(client.clone(), &body, &server.url("/found"))
            .await
            .unwrap();
        assert_eq!(result.bytes, vec![0x01, 0x02]);
    }

    /// process_deployer_query maps an empty expressionDeployers result to
    /// NoRecordFound.
    #[tokio::test]
    async fn test_process_deployer_query_empty_no_record() {
        use httpmock::prelude::*;
        let client = Arc::new(Client::builder().build().unwrap());
        let body = DeployerQuery::build_query(deployer_query::Variables {
            hash: Some("0xabc".to_string()),
        });
        let server = MockServer::start();
        let _empty = server.mock(|when, then| {
            when.method(POST);
            then.status(200)
                .json_body(serde_json::json!({"data": {"expressionDeployers": []}}));
        });
        let result = process_deployer_query(client, &body, &server.url("/")).await;
        assert!(matches!(result, Err(Error::NoRecordFound)));
    }
}
