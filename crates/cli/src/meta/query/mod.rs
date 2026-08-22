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
    use crate::meta::{ContentEncoding, ContentLanguage, ContentType};
    use crate::meta::types::authoring::v1::AuthoringMetaItem;
    use serde_bytes::ByteBuf;

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

    /// process_meta_query maps missing data, missing meta record and
    /// malformed hex all to NoRecordFound, and decodes a found record.
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
        assert!(matches!(result, Err(Error::NoRecordFound)));
        no_data.assert();

        let _no_meta = server.mock(|when, then| {
            when.method(POST).path("/nometa");
            then.status(200)
                .json_body(serde_json::json!({"data": {"meta": null}}));
        });
        let result = process_meta_query(client.clone(), &body, &server.url("/nometa")).await;
        assert!(matches!(result, Err(Error::NoRecordFound)));

        let _bad_hex = server.mock(|when, then| {
            when.method(POST).path("/badhex");
            then.status(200).json_body(serde_json::json!({
                "data": {"meta": {"__typename": "RainMetaV1", "rawBytes": "0xzz"}}
            }));
        });
        let result = process_meta_query(client.clone(), &body, &server.url("/badhex")).await;
        assert!(matches!(result, Err(Error::NoRecordFound)));

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
