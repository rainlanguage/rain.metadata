use alloy::primitives::{Address, B256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item},
    error::Error,
};

/// Configuration for a value field in the dotrain instance
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ValueCfg {
    /// Unique identifier for the field
    pub id: String,
    /// Optional human-readable name
    pub name: Option<String>,
    /// The actual value as string
    pub value: String,
}

/// Configuration for a token selection
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TokenCfg {
    /// Network name where the token exists
    pub network: String,
    /// Token contract address
    pub address: Address,
}

/// Dotrain Instance V1 metadata - contains user's specific configuration
/// for a deployed order referencing a dotrain template
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DotrainInstanceV1 {
    /// Hash of the original dotrain template in Metaboard
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

impl DotrainInstanceV1 {
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
}

impl From<DotrainInstanceV1> for RainMetaDocumentV1Item {
    fn from(value: DotrainInstanceV1) -> Self {
        // Serialize the struct to JSON bytes
        let json_bytes = serde_json::to_vec(&value).expect("Failed to serialize DotrainInstanceV1");
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(json_bytes),
            magic: KnownMagic::DotrainInstanceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        }
    }
}

impl TryFrom<RainMetaDocumentV1Item> for DotrainInstanceV1 {
    type Error = Error;

    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Error> {
        // Check magic type
        if value.magic != KnownMagic::DotrainInstanceV1 {
            return Err(Error::InvalidMetaMagic(
                KnownMagic::DotrainInstanceV1,
                value.magic,
            ));
        }

        // Deserialize JSON from payload
        let instance = serde_json::from_slice::<DotrainInstanceV1>(&value.payload)
            .map_err(Error::SerdeJsonError)?;

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, B256};
    use crate::meta::KnownMagic;

    fn create_test_instance() -> DotrainInstanceV1 {
        let mut field_values = BTreeMap::new();
        field_values.insert(
            "amount".to_string(),
            ValueCfg {
                id: "amount".to_string(),
                name: Some("Amount".to_string()),
                value: "100".to_string(),
            },
        );

        let mut select_tokens = BTreeMap::new();
        select_tokens.insert(
            "input-token".to_string(),
            TokenCfg {
                network: "ethereum".to_string(),
                address: Address::from([0x42; 20]),
            },
        );

        let mut vault_ids = BTreeMap::new();
        vault_ids.insert("input-0".to_string(), Some("vault-123".to_string()));
        vault_ids.insert("output-0".to_string(), None);

        DotrainInstanceV1 {
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
        let document_item: RainMetaDocumentV1Item = instance.clone().into();

        assert_eq!(document_item.magic, KnownMagic::DotrainInstanceV1);
        assert_eq!(document_item.content_type, ContentType::OctetStream);
        assert_eq!(document_item.content_encoding, ContentEncoding::None);
        assert_eq!(document_item.content_language, ContentLanguage::None);

        // Verify payload contains valid JSON
        let json_str = std::str::from_utf8(&document_item.payload).unwrap();
        assert!(json_str.contains("dotrain_hash"));
        assert!(json_str.contains("field_values"));
    }

    #[test]
    fn test_try_from_document_success() {
        let instance = create_test_instance();
        let json_bytes = serde_json::to_vec(&instance).unwrap();

        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(json_bytes),
            magic: KnownMagic::DotrainInstanceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };

        let recovered_instance = DotrainInstanceV1::try_from(document_item).unwrap();
        assert_eq!(recovered_instance, instance);
    }

    #[test]
    fn test_try_from_document_invalid_magic() {
        let instance = create_test_instance();
        let json_bytes = serde_json::to_vec(&instance).unwrap();

        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(json_bytes),
            magic: KnownMagic::DotrainSourceV1, // Wrong magic
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };

        let result = DotrainInstanceV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidMetaMagic(expected, actual) => {
                assert_eq!(expected, KnownMagic::DotrainInstanceV1);
                assert_eq!(actual, KnownMagic::DotrainSourceV1);
            }
            _ => panic!("Expected InvalidMetaMagic error"),
        }
    }

    #[test]
    fn test_try_from_document_invalid_json() {
        let invalid_json = b"{ invalid json }";
        let document_item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(invalid_json.to_vec()),
            magic: KnownMagic::DotrainInstanceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };

        let result = DotrainInstanceV1::try_from(document_item);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::SerdeJsonError(_) => {} // Expected
            _ => panic!("Expected SerdeJsonError"),
        }
    }

    #[test]
    fn test_conversion_roundtrip() {
        let original_instance = create_test_instance();

        // DotrainInstanceV1 -> RainMetaDocumentV1Item -> DotrainInstanceV1
        let document_item: RainMetaDocumentV1Item = original_instance.clone().into();
        let recovered_instance = DotrainInstanceV1::try_from(document_item).unwrap();

        assert_eq!(recovered_instance, original_instance);
    }

    #[test]
    fn test_roundtrip_cbor() {
        let original_instance = create_test_instance();

        // Convert to document item
        let document_item: RainMetaDocumentV1Item = original_instance.clone().into();

        // Encode to CBOR
        let cbor_bytes = document_item.cbor_encode().unwrap();

        // Decode from CBOR
        let decoded_items = RainMetaDocumentV1Item::cbor_decode(&cbor_bytes).unwrap();
        assert_eq!(decoded_items.len(), 1);
        let decoded_item = decoded_items.into_iter().next().unwrap();

        // Convert back to DotrainInstanceV1
        let decoded_instance = DotrainInstanceV1::try_from(decoded_item).unwrap();

        // Verify roundtrip
        assert_eq!(decoded_instance, original_instance);
    }
}
