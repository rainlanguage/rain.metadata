use alloy::{
    hex,
    primitives::{keccak256, B256},
};
use rain_metaboard_subgraph::{
    metaboard_client::{MetaboardSubgraphClient, MetaboardSubgraphClientError},
    types::metas::Bytes,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::Error,
    meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item},
};

#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{prelude::*, impl_wasm_traits};

/// Dotrain Source V1 meta
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct DotrainSourceV1(pub String);

#[cfg(target_family = "wasm")]
impl_wasm_traits!(DotrainSourceV1);

impl DotrainSourceV1 {
    /// Returns the hash of the dotrain source code
    pub fn hash(&self) -> B256 {
        keccak256(self.0.as_bytes())
    }
    /// Fetches the DotrainSourceV1 from the Metaboard by subject.
    /// The metaboard is append-only and its emitters are untrusted, so a
    /// subject can carry any number of dotrain sources. Returns
    /// Ok(Some(DotrainSourceV1)) when every one of them is the same source,
    /// Ok(None) when there is none, and Err(AmbiguousSubject) when they
    /// disagree, because nothing here can tell which of them the subject names.
    pub async fn fetch_by_subject(
        subject: [u8; 32],
        subgraph_url: Url,
    ) -> Result<Option<Self>, Error> {
        let client = MetaboardSubgraphClient::new(subgraph_url);
        let subject_hex = format!("0x{}", hex::encode(subject));
        let subject_bytes = Bytes(subject_hex.clone());

        let metabytes = match client.get_metabytes_by_subject(&subject_bytes).await {
            Ok(metabytes) => metabytes,
            Err(MetaboardSubgraphClientError::Empty(_)) => return Ok(None),
            Err(e) => return Err(Error::MetaboardSubgraphClientError(e)),
        };

        let mut found: Option<Self> = None;
        for meta_bytes in metabytes {
            for item in RainMetaDocumentV1Item::cbor_decode(&meta_bytes)? {
                if item.magic != KnownMagic::DotrainSourceV1 {
                    continue;
                }
                let source = DotrainSourceV1::try_from(item)?;
                if let Some(first) = &found {
                    if first.0 != source.0 {
                        return Err(Error::AmbiguousSubject(subject_hex));
                    }
                } else {
                    found = Some(source);
                }
            }
        }
        Ok(found)
    }
}

impl From<DotrainSourceV1> for RainMetaDocumentV1Item {
    fn from(value: DotrainSourceV1) -> Self {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(value.0),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
    }
}

impl TryFrom<RainMetaDocumentV1Item> for DotrainSourceV1 {
    type Error = Error;

    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Error> {
        if value.magic != KnownMagic::DotrainSourceV1 {
            return Err(Error::InvalidMetaMagic(
                KnownMagic::DotrainSourceV1,
                value.magic,
            ));
        }
        let content = String::from_utf8(value.payload.to_vec()).map_err(Error::FromUtf8Error)?;
        Ok(DotrainSourceV1(content))
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::meta::KnownMagic;

    fn dotrain_meta(sources: &[&str]) -> Vec<u8> {
        let items: Vec<RainMetaDocumentV1Item> = sources
            .iter()
            .map(|source| DotrainSourceV1(source.to_string()).into())
            .collect();
        RainMetaDocumentV1Item::cbor_encode_seq(&items, KnownMagic::RainMetaDocumentV1).unwrap()
    }

    fn other_meta() -> Vec<u8> {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("test content"),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
        .cbor_encode()
        .unwrap()
    }

    fn metas_response(metas: &[Vec<u8>], subject: [u8; 32]) -> serde_json::Value {
        let rows: Vec<serde_json::Value> = metas
            .iter()
            .map(|meta| {
                serde_json::json!({
                    "meta": hex::encode_prefixed(meta),
                    "metaHash": "0x1234567890abcdef",
                    "sender": "0x1234567890123456789012345678901234567890",
                    "id": "0x123",
                    "metaBoard": {
                        "address": "0x1234567890123456789012345678901234567890"
                    },
                    "subject": hex::encode(subject)
                })
            })
            .collect();
        serde_json::json!({ "data": { "metaV1S": rows } })
    }

    #[test]
    fn test_into_document() {
        let dotrain_code = "/* some dotrain code */".to_string();
        let dotrain_source = DotrainSourceV1(dotrain_code.clone());

        let document_item: RainMetaDocumentV1Item = dotrain_source.into();

        assert_eq!(document_item.magic, KnownMagic::DotrainSourceV1);
        assert_eq!(document_item.content_type, ContentType::OctetStream);
        assert_eq!(document_item.content_encoding, ContentEncoding::None);
        assert_eq!(document_item.content_language, ContentLanguage::None);
        assert_eq!(document_item.payload.as_ref(), dotrain_code.as_bytes());
    }

    #[test]
    fn test_try_from_document_success() {
        let dotrain_code = "/* some dotrain code */".to_string();
        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(dotrain_code.clone()),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let dotrain_source = DotrainSourceV1::try_from(document_item).unwrap();
        assert_eq!(dotrain_source.0, dotrain_code);
    }

    #[test]
    fn test_try_from_document_invalid_magic() {
        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("test"),
            magic: KnownMagic::AuthoringMetaV1, // Wrong magic
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let result = DotrainSourceV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidMetaMagic(expected, actual) => {
                assert_eq!(expected, KnownMagic::DotrainSourceV1);
                assert_eq!(actual, KnownMagic::AuthoringMetaV1);
            }
            _ => panic!("Expected InvalidMetaMagic error"),
        }
    }

    #[test]
    fn test_try_from_document_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD]; // Invalid UTF-8 sequence
        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(invalid_utf8),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let result = DotrainSourceV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::FromUtf8Error(_) => {} // Expected
            _ => panic!("Expected FromUtf8Error"),
        }
    }

    #[test]
    fn test_document_conversion_roundtrip() {
        let original_code = "rain-metadata-test-code".to_string();
        let original_source = DotrainSourceV1(original_code.clone());

        // DotrainSourceV1 -> RainMetaDocumentV1Item -> DotrainSourceV1
        let document_item: RainMetaDocumentV1Item = original_source.into();
        let recovered_source: DotrainSourceV1 = document_item.try_into().unwrap();

        assert_eq!(recovered_source.0, original_code);
    }

    #[test]
    fn test_roundtrip_cbor() {
        // Encode to CBOR
        let original_code = "/* dotrain source code */\nlet x = 42;".to_string();
        let original_source = DotrainSourceV1(original_code.clone());
        let document_item: RainMetaDocumentV1Item = original_source.into();
        let cbor_bytes = document_item.cbor_encode().unwrap();

        // Decode from CBOR
        let decoded_items = RainMetaDocumentV1Item::cbor_decode(&cbor_bytes).unwrap();
        assert_eq!(decoded_items.len(), 1);
        let decoded_item = decoded_items.into_iter().next().unwrap();
        let decoded_source = DotrainSourceV1::try_from(decoded_item).unwrap();

        // Verify roundtrip
        assert_eq!(decoded_source.0, original_code);
    }

    #[test]
    fn test_hash() {
        let dotrain_code = "/* test dotrain code */".to_string();
        let dotrain_source = DotrainSourceV1(dotrain_code.clone());

        let hash1 = dotrain_source.hash();
        let hash2 = DotrainSourceV1(dotrain_code).hash();

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Different content should produce different hash
        let different_source = DotrainSourceV1("different content".to_string());
        let hash3 = different_source.hash();
        assert_ne!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_fetch_by_subject_found() {
        use httpmock::prelude::*;

        // Create a mock server
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        let subject = [0x42; 32];
        let dotrain_code = "/* test dotrain code */";
        let dotrain_source = DotrainSourceV1(dotrain_code.to_string());
        let document: RainMetaDocumentV1Item = dotrain_source.into();
        let cbor_bytes = document.cbor_encode().unwrap();
        let cbor_hex = hex::encode(&cbor_bytes);

        // Mock the GraphQL response
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("content-type", "application/json")
                .body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                                "meta": format!("0x{}", cbor_hex),
                                "metaHash": "0x1234567890abcdef",
                                "sender": "0x1234567890123456789012345678901234567890",
                                "id": "0x123",
                                "metaBoard": {
                                    "address": "0x1234567890123456789012345678901234567890"
                                },
                                "subject": hex::encode(subject)
                            }
                        ]
                    }
                }));
        });

        // Test the function
        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url).await;

        // Verify the result
        assert!(result.is_ok());
        let dotrain_source = result.unwrap();
        assert!(dotrain_source.is_some());
        let dotrain_source = dotrain_source.unwrap();
        assert_eq!(dotrain_source.0, dotrain_code);

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_not_found() {
        use httpmock::prelude::*;

        // Create a mock server
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        let subject = [0x42; 32];

        // Mock empty response
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("content-type", "application/json")
                .body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "data": {
                        "metaV1S": []
                    }
                }));
        });

        // Test the function
        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url).await;

        // Verify the result
        assert!(result.is_ok());
        let dotrain_source = result.unwrap();
        assert!(dotrain_source.is_none());

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_invalid_cbor() {
        use httpmock::prelude::*;

        // Create a mock server
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        let subject = [0x42; 32];

        // Mock response with invalid CBOR
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("content-type", "application/json")
                .body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "data": {
                        "metaV1S": [
                            {
                                "meta": "0xdeadbeef", // Invalid CBOR
                                "metaHash": "0x1234567890abcdef",
                                "sender": "0x1234567890123456789012345678901234567890",
                                "id": "0x123",
                                "metaBoard": {
                                    "address": "0x1234567890123456789012345678901234567890"
                                },
                                "subject": hex::encode(subject)
                            }
                        ]
                    }
                }));
        });

        // Test the function
        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url).await;

        // Verify the result is an error
        assert!(result.is_err());

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_only_other_meta_types_is_not_found() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(&[other_meta()], subject));
        });

        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap();
        assert!(result.is_none());
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_sends_0x_prefixed_hex() {
        // Pin the wire-format change made when MetaV1.subject migrated
        // BigInt -> Bytes: the 32-byte subject must be sent as a hex
        // string with a `0x` prefix so the GraphQL Bytes scalar accepts
        // it. body_contains() makes the mock require the exact string,
        // and mock.assert() panics if no request matched.
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        let subject = [0x42u8; 32];
        let expected_hex = format!("0x{}", "42".repeat(32));

        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains(&expected_hex);
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({ "data": { "metaV1S": [] } }));
        });

        let _ = DotrainSourceV1::fetch_by_subject(subject, mock_url).await;
        mock.assert();
    }

    #[test]
    fn test_hash_known_keccak256_vectors() {
        // Vectors derived from the Keccak-256 reference values (independent
        // of this implementation): keccak256("") and keccak256("hello world").
        // Pins hash() to keccak256 over the exact utf8 bytes of the source.
        assert_eq!(
            DotrainSourceV1(String::new()).hash(),
            "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                .parse::<B256>()
                .unwrap()
        );
        assert_eq!(
            DotrainSourceV1("hello world".to_string()).hash(),
            "0x47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad"
                .parse::<B256>()
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_fetch_by_subject_conflicting_items_is_ambiguous() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(
                    &[dotrain_meta(&["first", "second"])],
                    subject,
                ));
        });

        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url).await;
        match result {
            Err(Error::AmbiguousSubject(s)) => {
                assert_eq!(s, format!("0x{}", hex::encode(subject)))
            }
            other => panic!("Expected Err(AmbiguousSubject), got {:?}", other),
        }
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_conflicting_metas_is_ambiguous_in_either_order() {
        use httpmock::prelude::*;
        let subject = [0x42; 32];

        for rows in [
            vec![dotrain_meta(&["first"]), dotrain_meta(&["second"])],
            vec![dotrain_meta(&["second"]), dotrain_meta(&["first"])],
        ] {
            let server = MockServer::start();
            let mock_url = Url::parse(&server.url("/")).unwrap();
            let mock = server.mock(|when, then| {
                when.method(POST).path("/").body_contains("subject");
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(metas_response(&rows, subject));
            });

            let result = DotrainSourceV1::fetch_by_subject(subject, mock_url).await;
            match result {
                Err(Error::AmbiguousSubject(s)) => {
                    assert_eq!(s, format!("0x{}", hex::encode(subject)))
                }
                other => panic!("Expected Err(AmbiguousSubject), got {:?}", other),
            }
            mock.assert();
        }
    }

    #[tokio::test]
    async fn test_fetch_by_subject_agreeing_metas_yield_the_source() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(
                    &[dotrain_meta(&["agreed"]), dotrain_meta(&["agreed"])],
                    subject,
                ));
        });

        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.0, "agreed");
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_ignores_other_meta_types_alongside() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(
                    &[other_meta(), dotrain_meta(&["only one"])],
                    subject,
                ));
        });

        let result = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.0, "only one");
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_propagates_non_empty_client_errors() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        // An HTTP-level failure is not "no meta found": it must surface as
        // Err(MetaboardSubgraphClientError), never Ok(None).
        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(500);
        });

        let result = DotrainSourceV1::fetch_by_subject([0x42; 32], mock_url).await;
        match result {
            Err(Error::MetaboardSubgraphClientError(_)) => {}
            other => panic!(
                "Expected Err(MetaboardSubgraphClientError), got {:?}",
                other
            ),
        }
        mock.assert();
    }
}
