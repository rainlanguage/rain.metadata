use alloy::{
    hex,
    primitives::{keccak256, B256},
};
use rain_metaboard_subgraph::{
    metaboard_client::{MetaboardSubgraphClient, MetaboardSubgraphClientError},
    types::metas::BigInt,
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
    /// Fetches the DotrainSourceV1 from the Metaboard by subject
    /// Returns Ok(Some(DotrainSourceV1)) if found, Ok(None) if not found
    pub async fn fetch_by_subject(
        subject: [u8; 32],
        subgraph_url: Url,
    ) -> Result<Option<Self>, Error> {
        let client = MetaboardSubgraphClient::new(subgraph_url);
        let subject_hex = hex::encode(subject);
        let subject_bigint = BigInt(subject_hex);

        match client.get_metabytes_by_subject(&subject_bigint).await {
            Ok(metabytes) => {
                if metabytes.is_empty() {
                    return Ok(None);
                }
                // Try to decode the first meta
                let decoded_items = RainMetaDocumentV1Item::cbor_decode(&metabytes[0])?;

                if decoded_items.is_empty() {
                    return Ok(None);
                }

                // Try to convert to DotrainSourceV1
                let dotrain_source = DotrainSourceV1::try_from(decoded_items[0].clone())?;
                Ok(Some(dotrain_source))
            }
            Err(MetaboardSubgraphClientError::Empty(_)) => {
                // No meta found for this subject
                Ok(None)
            }
            Err(e) => {
                // Convert subgraph client error to our error type
                Err(Error::MetaboardSubgraphClientError(e))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::KnownMagic;

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
    async fn test_fetch_by_subject_wrong_magic() {
        use httpmock::prelude::*;

        // Create a mock server
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        let subject = [0x42; 32];

        // Create a document with wrong magic
        let wrong_document = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("test content"),
            magic: KnownMagic::AuthoringMetaV1, // Wrong magic
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };
        let wrong_cbor_bytes = wrong_document.cbor_encode().unwrap();
        let wrong_cbor_hex = hex::encode(&wrong_cbor_bytes);

        // Mock response with wrong magic
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
                                "meta": format!("0x{}", wrong_cbor_hex),
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

        // Verify the result is an error (wrong magic)
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidMetaMagic(expected, actual) => {
                assert_eq!(expected, KnownMagic::DotrainSourceV1);
                assert_eq!(actual, KnownMagic::AuthoringMetaV1);
            }
            _ => panic!("Expected InvalidMetaMagic error"),
        }

        // Verify the mock was called
        mock.assert();
    }
}
