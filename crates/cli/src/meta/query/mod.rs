use std::sync::Arc;
use reqwest::Client;
use alloy::primitives::hex::decode;
use serde::{Deserialize, Serialize};
use graphql_client::{GraphQLQuery, Response, QueryBody};
use super::super::error::Error;

type Bytes = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/meta/query/schema.json",
    query_path = "src/meta/query/meta.graphql",
    response_derives = "Debug, Serialize, Deserialize"
)]
pub(super) struct MetaQuery;

/// response data struct for a meta
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MetaResponse {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
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

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

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
}
