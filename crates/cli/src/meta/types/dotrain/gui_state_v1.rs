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
pub struct TokenCfg {
    /// Network name where the token exists
    pub network: String,
    /// Token contract address
    #[cfg_attr(target_family = "wasm", tsify(type = "string"))]
    pub address: Address,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(TokenCfg);

/// Dotrain Instance V1 metadata - contains user's specific configuration
/// for a deployed order referencing a dotrain template
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct DotrainGuiStateV1 {
    /// Hash of the original dotrain template in Metaboard
    #[cfg_attr(target_family = "wasm", tsify(type = "string"))]
    pub dotrain_hash: B256,
    /// User-configured field values
    pub field_values: BTreeMap<String, ValueCfg>,
    /// Deposit configurations
    pub deposits: BTreeMap<String, ValueCfg>,
    /// Selected tokens for the order
    pub select_tokens: BTreeMap<String, TokenCfg>,
    /// Vault IDs mapping (input/output, index) -> vault_id
    pub vault_ids: BTreeMap<String, Option<String>>,
    /// Selected deployment name from the dotrain
    pub selected_deployment: String,
}
#[cfg(target_family = "wasm")]
impl_wasm_traits!(DotrainGuiStateV1);

impl DotrainGuiStateV1 {
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

    /// Extract DotrainGuiStateV1 from raw meta bytes
    ///
    /// This function attempts to decode CBOR data and find a DotrainGuiStateV1 document
    /// among potentially multiple metadata items.
    ///
    /// Returns:
    /// - Ok(Some(DotrainGuiStateV1)) if found and successfully parsed
    /// - Ok(None) if no DotrainGuiStateV1 document found in the meta bytes
    /// - Err(Error) if there are parsing/decoding errors
    pub fn extract_from_meta(meta_bytes: &[u8]) -> Result<Option<Self>, Error> {
        // Try to decode CBOR data
        let decoded_items = RainMetaDocumentV1Item::cbor_decode(meta_bytes)?;

        // Look for DotrainGuiStateV1 among the decoded items
        for item in decoded_items {
            if item.magic == KnownMagic::RainMetaDocumentV1 {
                if let Some(instance) = DotrainGuiStateV1::extract_from_meta(item.payload.as_ref())?
                {
                    return Ok(Some(instance));
                }
            }
            if item.magic == KnownMagic::DotrainGuiStateV1 {
                let instance = DotrainGuiStateV1::try_from(item)?;
                return Ok(Some(instance));
            }
        }

        // No DotrainGuiStateV1 found
        Ok(None)
    }
}

impl TryFrom<DotrainGuiStateV1> for RainMetaDocumentV1Item {
    type Error = Error;

    fn try_from(value: DotrainGuiStateV1) -> Result<Self, Self::Error> {
        // Serialize the struct to CBOR bytes
        let cbor_bytes = serde_cbor::to_vec(&value).map_err(Error::SerdeCborError)?;

        Ok(RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(cbor_bytes),
            magic: KnownMagic::DotrainGuiStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        })
    }
}

impl TryFrom<RainMetaDocumentV1Item> for DotrainGuiStateV1 {
    type Error = Error;

    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Error> {
        // Check magic type
        if value.magic != KnownMagic::DotrainGuiStateV1 {
            return Err(Error::InvalidMetaMagic(
                KnownMagic::DotrainGuiStateV1,
                value.magic,
            ));
        }

        // Deserialize CBOR from payload
        let instance = serde_cbor::from_slice::<DotrainGuiStateV1>(&value.payload)
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

    fn create_test_instance() -> DotrainGuiStateV1 {
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
            TokenCfg {
                network: "ethereum".to_string(),
                address: Address::from([0x42; 20]),
            },
        )]);

        let vault_ids = BTreeMap::from([
            ("input-0".to_string(), Some("vault-123".to_string())),
            ("output-0".to_string(), None),
        ]);

        DotrainGuiStateV1 {
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

        assert_eq!(document_item.magic, KnownMagic::DotrainGuiStateV1);
        assert_eq!(document_item.content_type, ContentType::OctetStream);
        assert_eq!(document_item.content_encoding, ContentEncoding::None);
        assert_eq!(document_item.content_language, ContentLanguage::None);

        // Verify payload contains valid CBOR that can be deserialized back
        let deserialized_instance =
            serde_cbor::from_slice::<DotrainGuiStateV1>(&document_item.payload).unwrap();
        assert_eq!(deserialized_instance, instance);
    }

    #[test]
    fn test_try_from_document_success() {
        let instance = create_test_instance();
        let cbor_bytes = serde_cbor::to_vec(&instance).unwrap();

        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(cbor_bytes),
            magic: KnownMagic::DotrainGuiStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };

        let recovered_instance = DotrainGuiStateV1::try_from(document_item).unwrap();
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
        };

        let result = DotrainGuiStateV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidMetaMagic(expected, actual) => {
                assert_eq!(expected, KnownMagic::DotrainGuiStateV1);
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
            magic: KnownMagic::DotrainGuiStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };

        let result = DotrainGuiStateV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::SerdeCborError(_) => {} // Expected
            _ => panic!("Expected SerdeCborError"),
        }
    }

    #[test]
    fn test_conversion_roundtrip() {
        let original_instance = create_test_instance();

        // DotrainGuiStateV1 -> RainMetaDocumentV1Item -> DotrainGuiStateV1
        let document_item: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();
        let recovered_instance = DotrainGuiStateV1::try_from(document_item).unwrap();

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

        // Convert back to DotrainGuiStateV1
        let decoded_instance = DotrainGuiStateV1::try_from(decoded_item).unwrap();

        // Verify roundtrip
        assert_eq!(decoded_instance, original_instance);
    }

    #[test]
    fn test_extract_from_meta_found() {
        let original_instance = create_test_instance();
        let document_item: RainMetaDocumentV1Item = original_instance.clone().try_into().unwrap();
        let cbor_bytes = document_item.cbor_encode().unwrap();

        let result = DotrainGuiStateV1::extract_from_meta(&cbor_bytes).unwrap();
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

        let result = DotrainGuiStateV1::extract_from_meta(&cbor_bytes).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_from_meta_multiple_documents() {
        // Create multiple documents, only one is DotrainGuiStateV1
        let instance = create_test_instance();
        let instance_doc: RainMetaDocumentV1Item = instance.clone().try_into().unwrap();
        let source = DotrainSourceV1("test code".to_string());
        let source_doc: RainMetaDocumentV1Item = source.into();

        // Encode them as a sequence
        let documents = vec![source_doc, instance_doc];
        let cbor_bytes =
            RainMetaDocumentV1Item::cbor_encode_seq(&documents, KnownMagic::RainMetaDocumentV1)
                .unwrap();

        let result = DotrainGuiStateV1::extract_from_meta(&cbor_bytes).unwrap();
        assert!(result.is_some());
        let extracted_instance = result.unwrap();
        assert_eq!(extracted_instance, instance);
    }

    #[test]
    fn test_extract_from_meta_invalid_cbor() {
        let invalid_cbor = vec![0xFF, 0xFE, 0xFD, 0xFC];

        let result = DotrainGuiStateV1::extract_from_meta(&invalid_cbor);
        assert!(result.is_err());
        // Should be a CBOR decode error
    }

    #[test]
    fn test_extract_from_meta_empty_data() {
        let empty_data = vec![];

        let result = DotrainGuiStateV1::extract_from_meta(&empty_data);
        assert!(result.is_err());
        // Should be a CBOR decode error for empty data
    }

    #[test]
    fn test_extract_from_meta_corrupted_instance_data() {
        // Create a document with DotrainGuiStateV1 magic but invalid CBOR payload
        let corrupted_doc = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("{ corrupted cbor }"),
            magic: KnownMagic::DotrainGuiStateV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };
        let cbor_bytes = corrupted_doc.cbor_encode().unwrap();

        let result = DotrainGuiStateV1::extract_from_meta(&cbor_bytes);
        assert!(result.is_err());
        // Should be a CBOR deserialization error
        match result.unwrap_err() {
            Error::SerdeCborError(_) => {} // Expected
            _ => panic!("Expected SerdeCborError"),
        }
    }
}
