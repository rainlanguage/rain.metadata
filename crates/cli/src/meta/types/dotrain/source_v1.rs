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
    /// keccak256 over the raw source bytes. This is the metaboard subject a
    /// dotrain source is emitted under, so it is also the `fetch_by_subject`
    /// key for it.
    pub fn hash(&self) -> B256 {
        keccak256(self.0.as_bytes())
    }
    /// Fetches every `DotrainSourceV1` the metaboard carries under `subject`.
    ///
    /// A subject is whatever entity the metadata is about, and both directions
    /// of that relation are 1:N: any number of senders may emit under one
    /// subject, and any meta may carry any number of items. Nothing requires
    /// the sources found there to agree, so all of them are returned, in the
    /// order scanned: rows in the order the query pins, items in the order
    /// they are encoded. Nothing is deduplicated or reordered.
    /// An empty vec means the subject carries no dotrain source.
    ///
    /// A metaboard enforces the magic number and nothing else, so anyone may
    /// emit bytes under any subject that carry the prefix and are otherwise
    /// junk, and per `IMetaBoardV1_2` discarding those is this side's job.
    /// The unit discarded is one emission, because one emission is one
    /// emitter: a row that does not decode is dropped, and so is a row that
    /// decodes but puts something under the dotrain magic that is not a
    /// source — taking the readable half of that row would accept selected
    /// data from an emitter who just sent junk under the magic being read.
    ///
    /// Strictness stops at the emission for the same reason. Dropping a row
    /// costs only whoever emitted it, and they can emit again; failing the
    /// call would let any one emitter deny every source under the subject to
    /// every caller, permanently, on an append-only board. Errors reaching
    /// the metaboard still surface.
    ///
    /// The query is not paginated, so what is scanned is the subgraph's first
    /// page of metas under the subject in that order, currently 100 rows.
    pub async fn fetch_by_subject(
        subject: [u8; 32],
        subgraph_url: Url,
    ) -> Result<Vec<Self>, Error> {
        let client = MetaboardSubgraphClient::new(subgraph_url);
        let subject_bytes = Bytes(format!("0x{}", hex::encode(subject)));

        let metabytes = match client.get_metabytes_by_subject(&subject_bytes).await {
            Ok(metabytes) => metabytes,
            Err(MetaboardSubgraphClientError::Empty(_)) => return Ok(vec![]),
            Err(e) => return Err(Error::MetaboardSubgraphClientError(e)),
        };

        let mut sources = Vec::new();
        for meta_bytes in metabytes {
            let Ok(items) = RainMetaDocumentV1Item::cbor_decode(&meta_bytes) else {
                continue;
            };
            let row: Result<Vec<Self>, Error> = items
                .into_iter()
                .filter(|item| item.magic == KnownMagic::DotrainSourceV1)
                .map(DotrainSourceV1::try_from)
                .collect();
            if let Ok(row) = row {
                sources.extend(row);
            }
        }
        Ok(sources)
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
        let content = String::from_utf8(value.unpack()?).map_err(Error::FromUtf8Error)?;
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

    /// The whole of what `LibIMetaBoardV1_2.emitMeta` demands of an emitter:
    /// `checkMetaUnhashedV1` enforces the magic number and nothing else, so
    /// this is emittable by anyone under any subject.
    fn anon_junk_with_magic() -> Vec<u8> {
        let mut junk = KnownMagic::RainMetaDocumentV1.to_prefix_bytes().to_vec();
        junk.extend_from_slice(&[0xff, 0xff, 0xff]);
        junk
    }

    /// Also emittable by anyone: an item claiming the dotrain magic over a
    /// payload that is not utf-8, so it decodes but cannot be a source.
    fn non_utf8_dotrain_item() -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(vec![0xff, 0xfe]),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        }
    }

    /// One emission, carrying whatever items it is given.
    fn meta_of(items: Vec<RainMetaDocumentV1Item>) -> Vec<u8> {
        RainMetaDocumentV1Item::cbor_encode_seq(&items, KnownMagic::RainMetaDocumentV1).unwrap()
    }

    fn source_item(source: &str) -> RainMetaDocumentV1Item {
        DotrainSourceV1(source.to_string()).into()
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
        let sources = result.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].0, dotrain_code);

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
        assert!(result.unwrap().is_empty());

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_undecodable_row_yields_nothing() {
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
                .json_body(metas_response(&[vec![0xde, 0xad, 0xbe, 0xef]], subject));
        });

        // A row that does not decode carries no dotrain source. That is not
        // an error: the metaboard accepts junk from anyone and discarding it
        // is this side's job.
        assert!(DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap()
            .is_empty());

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_only_other_meta_types_yields_nothing() {
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
        assert!(result.is_empty());
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
    async fn test_fetch_by_subject_returns_every_item_in_one_meta() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        // One blob carrying two different sources: both come back, in
        // encoding order. Neither position nor content picks a winner.
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(
                    &[dotrain_meta(&["first", "second"])],
                    subject,
                ));
        });

        let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap();
        assert_eq!(
            sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_returns_divergent_metas_in_row_order() {
        use httpmock::prelude::*;
        let subject = [0x42; 32];

        // Two metas under one subject holding two different sources is
        // ordinary valid data, not a conflict to resolve: every source is
        // returned. Running the same fixture in both row orders pins that the
        // vec is the rows as they arrive - nothing sorted, nothing dropped.
        for expected in [vec!["first", "second"], vec!["second", "first"]] {
            let server = MockServer::start();
            let mock_url = Url::parse(&server.url("/")).unwrap();
            let rows: Vec<Vec<u8>> = expected.iter().map(|s| dotrain_meta(&[*s])).collect();
            let mock = server.mock(|when, then| {
                when.method(POST).path("/").body_contains("subject");
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(metas_response(&rows, subject));
            });

            let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
                .await
                .unwrap();
            assert_eq!(
                sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
                expected
            );
            mock.assert();
        }
    }

    #[tokio::test]
    async fn test_fetch_by_subject_keeps_duplicate_sources() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        // Two emissions of the same source are two metas, and how many there
        // are is the caller's to read: they are not collapsed into one.
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(
                    &[dotrain_meta(&["agreed"]), dotrain_meta(&["agreed"])],
                    subject,
                ));
        });

        let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap();
        assert_eq!(
            sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
            vec!["agreed", "agreed"]
        );
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

        let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap();
        assert_eq!(
            sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
            vec!["only one"]
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_junk_row_does_not_deny_the_subject() {
        use httpmock::prelude::*;
        let subject = [0x42; 32];

        // Anyone can emit junk carrying the magic number under anyone's
        // subject, so a junk row must not decide what the rest of the subject
        // carries. Both orders: the source survives whether the junk sorts
        // before or after it.
        for rows in [
            vec![dotrain_meta(&["legit"]), anon_junk_with_magic()],
            vec![anon_junk_with_magic(), dotrain_meta(&["legit"])],
        ] {
            let server = MockServer::start();
            let mock_url = Url::parse(&server.url("/")).unwrap();
            let mock = server.mock(|when, then| {
                when.method(POST).path("/").body_contains("subject");
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(metas_response(&rows, subject));
            });

            let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
                .await
                .unwrap();
            assert_eq!(
                sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
                vec!["legit"]
            );
            mock.assert();
        }
    }

    #[tokio::test]
    async fn test_fetch_by_subject_non_utf8_dotrain_does_not_deny_the_subject() {
        use httpmock::prelude::*;
        let subject = [0x42; 32];

        // An emission putting something under the dotrain magic that is not a
        // source is dropped whole, and dropping it does not reach the rows
        // around it. Both orders: the source survives whether the unreadable
        // emission sorts before or after it.
        for rows in [
            vec![
                meta_of(vec![source_item("legit")]),
                meta_of(vec![non_utf8_dotrain_item()]),
            ],
            vec![
                meta_of(vec![non_utf8_dotrain_item()]),
                meta_of(vec![source_item("legit")]),
            ],
        ] {
            let server = MockServer::start();
            let mock_url = Url::parse(&server.url("/")).unwrap();
            let mock = server.mock(|when, then| {
                when.method(POST).path("/").body_contains("subject");
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(metas_response(&rows, subject));
            });

            let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
                .await
                .unwrap();
            assert_eq!(
                sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
                vec!["legit"]
            );
            mock.assert();
        }
    }

    #[tokio::test]
    async fn test_fetch_by_subject_takes_nothing_from_an_emission_it_cannot_read() {
        use httpmock::prelude::*;
        let subject = [0x42; 32];

        // One emission is one emitter. An emitter that puts something under
        // the dotrain magic that is not a source does not get the rest of
        // that emission read: no item of it is returned, whichever side of
        // the unreadable one the readable one sits. Only that emitter loses
        // anything, so there is no reason to keep half of what they sent.
        for rows in [
            vec![meta_of(vec![source_item("legit"), non_utf8_dotrain_item()])],
            vec![meta_of(vec![non_utf8_dotrain_item(), source_item("legit")])],
        ] {
            let server = MockServer::start();
            let mock_url = Url::parse(&server.url("/")).unwrap();
            let mock = server.mock(|when, then| {
                when.method(POST).path("/").body_contains("subject");
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(metas_response(&rows, subject));
            });

            assert!(DotrainSourceV1::fetch_by_subject(subject, mock_url)
                .await
                .unwrap()
                .is_empty());
            mock.assert();
        }
    }

    #[tokio::test]
    async fn test_fetch_by_subject_unreadable_emission_does_not_reach_other_rows() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();
        let subject = [0x42; 32];

        // The row it is dropped from is the only row it costs.
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").body_contains("subject");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(metas_response(
                    &[
                        meta_of(vec![source_item("first")]),
                        meta_of(vec![source_item("dropped"), non_utf8_dotrain_item()]),
                        meta_of(vec![source_item("last")]),
                    ],
                    subject,
                ));
        });

        let sources = DotrainSourceV1::fetch_by_subject(subject, mock_url)
            .await
            .unwrap();
        assert_eq!(
            sources.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
            vec!["first", "last"]
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_by_subject_propagates_non_empty_client_errors() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mock_url = Url::parse(&server.url("/")).unwrap();

        // An HTTP-level failure is not "no meta found": it must surface as
        // Err(MetaboardSubgraphClientError), never an empty vec.
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

    #[test]
    fn test_try_from_unpacks_content_encoding() {
        let dotrain_code = "/* some dotrain code */".to_string();
        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(
                ContentEncoding::Deflate.encode(dotrain_code.as_bytes()),
            ),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let dotrain_source = DotrainSourceV1::try_from(document_item).unwrap();
        assert_eq!(dotrain_source.0, dotrain_code);
    }
}
