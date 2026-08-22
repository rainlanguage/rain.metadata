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
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use serde_json::json;

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

    #[tokio::test]
    async fn test_process_deployer_query_null_bytecode_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["bytecode"] = serde_json::Value::Null;
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_parser_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["parser"] = serde_json::Value::Null;
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_store_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["store"] = serde_json::Value::Null;
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_interpreter_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["interpreter"] = serde_json::Value::Null;
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_null_deploy_transaction_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["deployTransaction"] = serde_json::Value::Null;
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_zero_metas_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["meta"] = json!([]);
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_two_metas_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["meta"] = json!([
            { "__typename": "RainMetaV1", "id": "0x0f10" },
            { "__typename": "RainMetaV1", "id": "0x1112" }
        ]);
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_bytecode_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["bytecode"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_parser_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["parser"]["parser"]["deployedBytecode"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_store_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["store"]["store"]["deployedBytecode"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_interpreter_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["interpreter"]["interpreter"]["deployedBytecode"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_meta_id_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["meta"][0]["id"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_tx_id_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["deployTransaction"]["id"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_constructor_meta_hash_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["constructorMetaHash"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }

    #[tokio::test]
    async fn test_process_deployer_query_invalid_constructor_meta_hex_is_no_record_found() {
        let mut entry = deployer_entry();
        entry["constructorMeta"] = json!("0xZZ");
        assert!(matches!(
            run_deployer_query(entry).await,
            Err(Error::NoRecordFound)
        ));
    }
}
