use alloy::primitives::{Address, B256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item},
    error::Error,
};

#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{prelude::*, impl_wasm_traits};

/// Configuration for a value field in the dotrain instance
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct ValueCfg {
    /// Unique identifier for the field
    pub id: String,
    /// Optional human-readable name
    pub name: Option<String>,
    /// The actual value as string
    pub value: String,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(ValueCfg);

/// Configuration for a token selection
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct ShortenedTokenCfg {
    /// Network name where the token exists
    pub network: String,
    /// Token contract address
    #[cfg_attr(target_family = "wasm", tsify(type = "`0x${string}`"))]
    pub address: Address,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(ShortenedTokenCfg);

/// Dotrain Instance V1 metadata - contains user's specific configuration
/// for a deployed order referencing a dotrain template
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct OrderBuilderStateV1 {
    /// Hash of the original dotrain template in Metaboard
    #[cfg_attr(target_family = "wasm", tsify(type = "`0x${string}`"))]
    pub dotrain_hash: B256,
    /// User-configured field values
    pub field_values: BTreeMap<String, ValueCfg>,
    /// Deposit configurations
    pub deposits: BTreeMap<String, ValueCfg>,
    /// Selected tokens for the order
    pub select_tokens: BTreeMap<String, ShortenedTokenCfg>,
    /// Vault IDs mapping (input/output, index) -> vault_id
    pub vault_ids: BTreeMap<String, Option<String>>,
    /// Selected deployment name from the dotrain
    pub selected_deployment: String,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(OrderBuilderStateV1);

/// Meta bytes reaching [OrderBuilderStateV1::extract_from_meta] are attacker
/// influenceable, so its walk into nested documents is bounded by a budget
/// rather than by the stack.
pub const MAX_NESTED_DOCUMENT_DEPTH: usize = 32;

impl OrderBuilderStateV1 {
    /// Get the template hash
    pub fn dotrain_hash(&self) -> B256 {
        self.dotrain_hash
    }

    pub fn get_token_addresses(&self) -> Vec<Address> {
        self.select_tokens
            .values()
            .map(|token| token.address)
            .collect()
    }

    /// Get all non-empty vault IDs
    pub fn get_vault_ids(&self) -> Vec<String> {
        self.vault_ids
            .values()
            .filter_map(|id| id.as_ref())
            .cloned()
            .collect()
    }

    /// Extract OrderBuilderStateV1 from raw meta bytes
    ///
    /// This function attempts to decode CBOR data and find a OrderBuilderStateV1 document
    /// among potentially multiple metadata items.
    ///
    /// Returns:
    /// - Ok(Some(OrderBuilderStateV1)) if found and successfully parsed
    /// - Ok(None) if no OrderBuilderStateV1 document found in the meta bytes
    /// - Err(Error) if there are parsing/decoding errors, or if nested
    ///   documents run deeper than [MAX_NESTED_DOCUMENT_DEPTH]
    pub fn extract_from_meta(meta_bytes: &[u8]) -> Result<Option<Self>, Error> {
        OrderBuilderStateV1::extract_from_meta_within(meta_bytes, MAX_NESTED_DOCUMENT_DEPTH)
    }

    fn extract_from_meta_within(
        meta_bytes: &[u8],
        remaining_depth: usize,
    ) -> Result<Option<Self>, Error> {
        // Try to decode CBOR data
        let decoded_items = RainMetaDocumentV1Item::cbor_decode(meta_bytes)?;

        // Look for OrderBuilderStateV1 among the decoded items
        for item in decoded_items {
            if item.magic == KnownMagic::RainMetaDocumentV1 {
                let inner_depth = remaining_depth
                    .checked_sub(1)
                    .ok_or(Error::MetaNestingTooDeep(MAX_NESTED_DOCUMENT_DEPTH))?;
                if let Some(instance) = OrderBuilderStateV1::extract_from_meta_within(
                    item.payload.as_ref(),
                    inner_depth,
                )? {
                    return Ok(Some(instance));
                }
            }
            if item.magic == KnownMagic::OrderBuilderStateV1 {
                let instance = OrderBuilderStateV1::try_from(item)?;
                return Ok(Some(instance));
            }
        }

        // No OrderBuilderStateV1 found
        Ok(None)
    }
}

impl TryFrom<OrderBuilderStateV1> for RainMetaDocumentV1Item {
    type Error = Error;

    fn try_from(value: OrderBuilderStateV1) -> Result<Self, Self::Error> {
        // Serialize the struct to CBOR bytes
        let cbor_bytes = serde_cbor::to_vec(&value).map_err(Error::SerdeCborError)?;

        Ok(RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(cbor_bytes),
            magic: KnownMagic::OrderBuilderStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        })
    }
}

impl TryFrom<RainMetaDocumentV1Item> for OrderBuilderStateV1 {
    type Error = Error;

    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Error> {
        // Check magic type
        if value.magic != KnownMagic::OrderBuilderStateV1 {
            return Err(Error::InvalidMetaMagic(
                KnownMagic::OrderBuilderStateV1,
                value.magic,
            ));
        }

        // Deserialize CBOR from payload
        let instance = serde_cbor::from_slice::<OrderBuilderStateV1>(&value.payload)
            .map_err(Error::SerdeCborError)?;

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, B256};
    use crate::meta::KnownMagic;
    use crate::meta::types::dotrain::source_v1::DotrainSourceV1;

    fn create_test_instance() -> OrderBuilderStateV1 {
        let field_values = BTreeMap::from([(
            "amount".to_string(),
            ValueCfg {
                id: "amount".to_string(),
                name: Some("Amount".to_string()),
                value: "100".to_string(),
            },
        )]);

        let select_tokens = BTreeMap::from([(
            "input-token".to_string(),
            ShortenedTokenCfg {
                network: "ethereum".to_string(),
                address: Address::from([0x42; 20]),
            },
        )]);

        let vault_ids = BTreeMap::from([
            ("input-0".to_string(), Some("vault-123".to_string())),
            ("output-0".to_string(), None),
        ]);

        OrderBuilderStateV1 {
            dotrain_hash: B256::from([0x12; 32]),
            field_values,
            deposits: BTreeMap::new(),
            select_tokens,
            vault_ids,
            selected_deployment: "mainnet".to_string(),
        }
    }

    #[test]
    fn test_get_token_addresses() {
        let instance = create_test_instance();
        let addresses = instance.get_token_addresses();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0], Address::from([0x42; 20]));
    }

    #[test]
    fn test_get_vault_ids() {
        let instance = create_test_instance();
        let vault_ids = instance.get_vault_ids();
        assert_eq!(vault_ids.len(), 1);
        assert_eq!(vault_ids[0], "vault-123");
    }

    #[test]
    fn test_into_document() {
        let instance = create_test_instance();
        let document_item: RainMetaDocumentV1Item = instance.clone().try_into().unwrap();

        assert_eq!(document_item.magic, KnownMagic::OrderBuilderStateV1);
        assert_eq!(document_item.content_type, ContentType::OctetStream);
        assert_eq!(document_item.content_encoding, ContentEncoding::None);
        assert_eq!(document_item.content_language, ContentLanguage::None);

        // Verify payload contains valid CBOR that can be deserialized back
        let deserialized_instance =
            serde_cbor::from_slice::<OrderBuilderStateV1>(&document_item.payload).unwrap();
        assert_eq!(deserialized_instance, instance);
    }

    #[test]
    fn test_try_from_document_success() {
        let instance = create_test_instance();
        let cbor_bytes = serde_cbor::to_vec(&instance).unwrap();

        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(cbor_bytes),
            magic: KnownMagic::OrderBuilderStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let recovered_instance = OrderBuilderStateV1::try_from(document_item).unwrap();
        assert_eq!(recovered_instance, instance);
    }

    #[test]
    fn test_try_from_document_invalid_magic() {
        let instance = create_test_instance();
        let cbor_bytes = serde_cbor::to_vec(&instance).unwrap();

        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(cbor_bytes),
            magic: KnownMagic::DotrainSourceV1, // Wrong magic
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let result = OrderBuilderStateV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidMetaMagic(expected, actual) => {
                assert_eq!(expected, KnownMagic::OrderBuilderStateV1);
                assert_eq!(actual, KnownMagic::DotrainSourceV1);
            }
            _ => panic!("Expected InvalidMetaMagic error"),
        }
    }

    #[test]
    fn test_try_from_document_invalid_cbor() {
        let invalid_cbor = b"{ invalid cbor }";
        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(invalid_cbor.to_vec()),
            magic: KnownMagic::OrderBuilderStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };

        let result = OrderBuilderStateV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::SerdeCborError(_) => {} // Expected
            _ => panic!("Expected SerdeCborError"),
        }
    }

    #[test]
    fn test_conversion_roundtrip() {
        let original_instance = create_test_instance();

        // OrderBuilderStateV1 -> RainMetaDocumentV1Item -> OrderBuilderStateV1
        let document_item: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();
        let recovered_instance = OrderBuilderStateV1::try_from(document_item).unwrap();

        assert_eq!(recovered_instance, original_instance);
    }

    #[test]
    fn test_roundtrip_cbor() {
        let original_instance = create_test_instance();

        // Convert to document item
        let document_item: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();

        // Encode to CBOR
        let cbor_bytes = document_item.cbor_encode().unwrap();

        // Decode from CBOR
        let decoded_items = RainMetaDocumentV1Item::cbor_decode(&cbor_bytes).unwrap();
        assert_eq!(decoded_items.len(), 1);
        let decoded_item = decoded_items.into_iter().next().unwrap();

        // Convert back to OrderBuilderStateV1
        let decoded_instance = OrderBuilderStateV1::try_from(decoded_item).unwrap();

        // Verify roundtrip
        assert_eq!(decoded_instance, original_instance);
    }

    #[test]
    fn test_extract_from_meta_found() {
        let original_instance = create_test_instance();
        let document_item: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();
        let cbor_bytes = document_item.cbor_encode().unwrap();

        let result = OrderBuilderStateV1::extract_from_meta(&cbor_bytes).unwrap();
        assert!(result.is_some());
        let extracted_instance = result.unwrap();
        assert_eq!(extracted_instance, original_instance);
    }

    #[test]
    fn test_extract_from_meta_not_found() {
        // Create a different type of document
        let source = DotrainSourceV1("test code".to_string());
        let document_item: RainMetaDocumentV1Item = source.into();
        let cbor_bytes = document_item.cbor_encode().unwrap();

        let result = OrderBuilderStateV1::extract_from_meta(&cbor_bytes).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_from_meta_multiple_documents() {
        // Create multiple documents, only one is OrderBuilderStateV1
        let instance = create_test_instance();
        let instance_doc: RainMetaDocumentV1Item = instance.clone().try_into().unwrap();
        let source = DotrainSourceV1("test code".to_string());
        let source_doc: RainMetaDocumentV1Item = source.into();

        // Encode them as a sequence
        let documents = vec![source_doc, instance_doc];
        let cbor_bytes =
            RainMetaDocumentV1Item::cbor_encode_seq(&documents, KnownMagic::RainMetaDocumentV1)
                .unwrap();

        let result = OrderBuilderStateV1::extract_from_meta(&cbor_bytes).unwrap();
        assert!(result.is_some());
        let extracted_instance = result.unwrap();
        assert_eq!(extracted_instance, instance);
    }

    #[test]
    fn test_extract_from_meta_invalid_cbor() {
        let invalid_cbor = vec![0xFF, 0xFE, 0xFD, 0xFC];

        let result = OrderBuilderStateV1::extract_from_meta(&invalid_cbor);
        assert!(result.is_err());
        // Should be a CBOR decode error
    }

    #[test]
    fn test_extract_from_meta_empty_data() {
        let empty_data = vec![];

        let result = OrderBuilderStateV1::extract_from_meta(&empty_data);
        assert!(result.is_err());
        // Should be a CBOR decode error for empty data
    }

    #[test]
    fn test_extract_from_meta_corrupted_instance_data() {
        // Create a document with OrderBuilderStateV1 magic but invalid CBOR payload
        let corrupted_doc = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("{ corrupted cbor }"),
            magic: KnownMagic::OrderBuilderStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let cbor_bytes = corrupted_doc.cbor_encode().unwrap();

        let result = OrderBuilderStateV1::extract_from_meta(&cbor_bytes);
        assert!(result.is_err());
        // Should be a CBOR deserialization error
        match result.unwrap_err() {
            Error::SerdeCborError(_) => {} // Expected
            _ => panic!("Expected SerdeCborError"),
        }
    }

    #[test]
    fn test_extract_from_meta_nested_rain_document() {
        // A decoded item whose magic is RainMetaDocumentV1 carries a complete
        // prefixed document as payload; extract_from_meta must recurse into
        // it and surface the instance found inside.
        let original_instance = create_test_instance();
        let inner_item: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();
        let inner_doc_bytes = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![inner_item],
            KnownMagic::RainMetaDocumentV1,
        )
        .unwrap();

        let outer_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(inner_doc_bytes),
            magic: KnownMagic::RainMetaDocumentV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let outer_bytes = RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![outer_item],
            KnownMagic::RainMetaDocumentV1,
        )
        .unwrap();

        let extracted = OrderBuilderStateV1::extract_from_meta(&outer_bytes)
            .unwrap()
            .unwrap();
        assert_eq!(extracted, original_instance);
    }

    /// A document carrying `leaf` under `depth` levels of RainMetaDocumentV1
    /// nesting, so `extract_from_meta` needs exactly `depth` descents to reach
    /// it.
    fn nest_document(leaf: RainMetaDocumentV1Item, depth: usize) -> Vec<u8> {
        let mut bytes =
            RainMetaDocumentV1Item::cbor_encode_seq(&vec![leaf], KnownMagic::RainMetaDocumentV1)
                .unwrap();
        for _ in 0..depth {
            let wrapper = RainMetaDocumentV1Item {
                payload: serde_bytes::ByteBuf::from(bytes),
                magic: KnownMagic::RainMetaDocumentV1,
                content_type: ContentType::OctetStream,
                content_encoding: ContentEncoding::None,
                content_language: ContentLanguage::None,
                schema: None,
            };
            bytes = RainMetaDocumentV1Item::cbor_encode_seq(
                &vec![wrapper],
                KnownMagic::RainMetaDocumentV1,
            )
            .unwrap();
        }
        bytes
    }

    #[test]
    fn test_extract_from_meta_at_the_nesting_bound() {
        let original_instance = create_test_instance();
        let leaf: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();
        let bytes = nest_document(leaf, MAX_NESTED_DOCUMENT_DEPTH);

        let extracted = OrderBuilderStateV1::extract_from_meta(&bytes)
            .unwrap()
            .unwrap();
        assert_eq!(extracted, original_instance);
    }

    #[test]
    fn test_extract_from_meta_past_the_nesting_bound() {
        let leaf: RainMetaDocumentV1Item = create_test_instance().try_into().unwrap();
        let bytes = nest_document(leaf, MAX_NESTED_DOCUMENT_DEPTH + 1);

        match OrderBuilderStateV1::extract_from_meta(&bytes).unwrap_err() {
            Error::MetaNestingTooDeep(max) => assert_eq!(max, MAX_NESTED_DOCUMENT_DEPTH),
            other => panic!("Expected MetaNestingTooDeep, got {:?}", other),
        }
    }

    /// The bound is what keeps attacker-shaped nesting off the stack: 5000
    /// levels drive 5000 frames unbounded, so this runs on a stack far too
    /// small to hold them and must still return an error rather than abort.
    #[test]
    fn test_extract_from_meta_deep_nesting_does_not_exhaust_the_stack() {
        let leaf: RainMetaDocumentV1Item = DotrainSourceV1("leaf".to_string()).into();
        let bytes = nest_document(leaf, 5000);

        let extracted = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(move || {
                matches!(
                    OrderBuilderStateV1::extract_from_meta(&bytes),
                    Err(Error::MetaNestingTooDeep(_))
                )
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(extracted);
    }
}
