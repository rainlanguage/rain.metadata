//! Centralized enum for all supported metadata types
//!
//! This module provides `UnpackedMetadata` enum that unifies all supported Rain metadata types
//! and provides ergonomic parsing from `RainMetaDocumentV1Item`.

use super::{Error, KnownMagic, RainMetaDocumentV1Item};
use crate::meta::types::{
    authoring::v1::AuthoringMeta, authoring::v2::AuthoringMetaV2, dotrain::v1::DotrainMeta,
    dotrain::source_v1::DotrainSourceV1, dotrain::instance_v1::DotrainInstanceV1,
    expression_deployer_v2_bytecode::v1::ExpressionDeployerV2BytecodeMeta,
    interpreter_caller::v1::InterpreterCallerMeta, op::v1::OpMeta, rainlang::v1::RainlangMeta,
    rainlangsource::v1::RainlangSourceMeta, solidity_abi::v2::SolidityAbiMeta,
};
use serde::{Serialize, Deserialize};
use alloy::primitives::hex;

#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{prelude::*, impl_wasm_traits};

/// Macro to generate type checking methods for UnpackedMetadata variants
macro_rules! impl_type_checks {
    ($($variant:ident => $method:ident),* $(,)?) => {
        $(
            #[doc = concat!("Returns true if this is ", stringify!($variant), " metadata")]
            pub fn $method(&self) -> bool {
                matches!(self, UnpackedMetadata::$variant(_))
            }
        )*
    };
}

/// Centralized enum for all supported Rain metadata types
///
/// This enum provides a unified interface for working with all supported metadata types,
/// allowing ergonomic parsing and type-safe handling of Rain metadata.
///
/// The enum supports serialization and deserialization via serde, making it easy to
/// convert metadata to and from JSON or other formats. It also supports parsing from
/// hex-encoded metadata strings.
///
/// # Examples
///
/// ```rust
/// use rain_metadata::{RainMetaDocumentV1Item, UnpackedMetadata};
/// use rain_metadata::types::authoring::v1::AuthoringMeta;
///
/// // Create a simple authoring metadata
/// let authoring = AuthoringMeta(vec![]);
/// let unpacked = UnpackedMetadata::AuthoringV1(authoring);
///
/// // Use type guards to check the variant
/// assert!(unpacked.is_authoring_v1());
/// assert!(!unpacked.is_dotrain_v1());
///
/// // Serialize to JSON
/// let json = serde_json::to_string(&unpacked).unwrap();
/// // JSON will look like: {"AuthoringV1": []}
///
/// // Deserialize back
/// let deserialized: UnpackedMetadata = serde_json::from_str(&json).unwrap();
/// assert!(deserialized.is_authoring_v1());
///
/// // Parse from hex string (supports single or multiple documents)
/// let _metadata_items = UnpackedMetadata::parse_from_hex("ff0a89c674ee7874").unwrap_err(); // This will fail for demo
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub enum UnpackedMetadata {
    /// Authoring metadata V1
    AuthoringV1(AuthoringMeta),
    /// Authoring metadata V2
    AuthoringV2(AuthoringMetaV2),
    /// Dotrain metadata V1 (String content)
    DotrainV1(DotrainMeta),
    /// Dotrain source metadata V1
    DotrainSourceV1(DotrainSourceV1),
    /// Dotrain instance metadata V1
    DotrainInstanceV1(DotrainInstanceV1),
    /// Expression deployer V2 bytecode metadata (raw bytes)
    ExpressionDeployerV2BytecodeV1(ExpressionDeployerV2BytecodeMeta),
    /// Interpreter caller metadata V1
    InterpreterCallerV1(InterpreterCallerMeta),
    /// Op metadata V1
    OpV1(OpMeta),
    /// Rainlang metadata V1 (String content)
    RainlangV1(RainlangMeta),
    /// Rainlang source metadata V1 (String content)
    RainlangSourceV1(RainlangSourceMeta),
    /// Solidity ABI metadata V2
    SolidityAbiV2(SolidityAbiMeta),
    /// Address list metadata (raw bytes)
    AddressList(Vec<u8>),
}

#[cfg(target_family = "wasm")]
impl_wasm_traits!(UnpackedMetadata);

impl UnpackedMetadata {
    /// Returns the magic number associated with this metadata type
    pub fn magic(&self) -> KnownMagic {
        match self {
            UnpackedMetadata::AuthoringV1(_) => KnownMagic::AuthoringMetaV1,
            UnpackedMetadata::AuthoringV2(_) => KnownMagic::AuthoringMetaV2,
            UnpackedMetadata::DotrainV1(_) => KnownMagic::DotrainV1,
            UnpackedMetadata::DotrainSourceV1(_) => KnownMagic::DotrainSourceV1,
            UnpackedMetadata::DotrainInstanceV1(_) => KnownMagic::DotrainInstanceV1,
            UnpackedMetadata::ExpressionDeployerV2BytecodeV1(_) => {
                KnownMagic::ExpressionDeployerV2BytecodeV1
            }
            UnpackedMetadata::InterpreterCallerV1(_) => KnownMagic::InterpreterCallerMetaV1,
            UnpackedMetadata::OpV1(_) => KnownMagic::OpMetaV1,
            UnpackedMetadata::RainlangV1(_) => KnownMagic::RainlangV1,
            UnpackedMetadata::RainlangSourceV1(_) => KnownMagic::RainlangSourceV1,
            UnpackedMetadata::SolidityAbiV2(_) => KnownMagic::SolidityAbiV2,
            UnpackedMetadata::AddressList(_) => KnownMagic::AddressList,
        }
    }

    // Generate all type checking methods using macro
    impl_type_checks! {
        AuthoringV1 => is_authoring_v1,
        AuthoringV2 => is_authoring_v2,
        DotrainV1 => is_dotrain_v1,
        DotrainSourceV1 => is_dotrain_source_v1,
        DotrainInstanceV1 => is_dotrain_instance_v1,
        ExpressionDeployerV2BytecodeV1 => is_expression_deployer_v2_bytecode_v1,
        InterpreterCallerV1 => is_interpreter_caller_v1,
        OpV1 => is_op_v1,
        RainlangV1 => is_rainlang_v1,
        RainlangSourceV1 => is_rainlang_source_v1,
        SolidityAbiV2 => is_solidity_abi_v2,
        AddressList => is_address_list,
    }
}

impl TryFrom<RainMetaDocumentV1Item> for UnpackedMetadata {
    type Error = Error;

    fn try_from(item: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        match item.magic {
            KnownMagic::AuthoringMetaV1 => Ok(UnpackedMetadata::AuthoringV1(item.unpack_into()?)),
            KnownMagic::DotrainV1 => Ok(UnpackedMetadata::DotrainV1(item.unpack_into()?)),
            KnownMagic::DotrainSourceV1 => {
                Ok(UnpackedMetadata::DotrainSourceV1(item.unpack_into()?))
            }
            KnownMagic::DotrainInstanceV1 => {
                Ok(UnpackedMetadata::DotrainInstanceV1(item.unpack_into()?))
            }
            KnownMagic::ExpressionDeployerV2BytecodeV1 => Ok(
                UnpackedMetadata::ExpressionDeployerV2BytecodeV1(item.unpack_into()?),
            ),
            KnownMagic::InterpreterCallerMetaV1 => {
                Ok(UnpackedMetadata::InterpreterCallerV1(item.unpack_into()?))
            }
            KnownMagic::OpMetaV1 => Ok(UnpackedMetadata::OpV1(item.unpack_into()?)),
            KnownMagic::RainlangV1 => Ok(UnpackedMetadata::RainlangV1(item.unpack_into()?)),
            KnownMagic::RainlangSourceV1 => {
                Ok(UnpackedMetadata::RainlangSourceV1(item.unpack_into()?))
            }
            KnownMagic::SolidityAbiV2 => Ok(UnpackedMetadata::SolidityAbiV2(item.unpack_into()?)),
            KnownMagic::AddressList => Ok(UnpackedMetadata::AddressList(item.unpack_into()?)),

            // Special case for AuthoringMetaV2 (has different error type)
            KnownMagic::AuthoringMetaV2 => {
                let authoring_v2: AuthoringMetaV2 =
                    item.try_into().map_err(|_| Error::UnsupportedMeta)?;
                Ok(UnpackedMetadata::AuthoringV2(authoring_v2))
            }

            _ => Err(Error::UnsupportedMeta),
        }
    }
}

impl UnpackedMetadata {
    /// Parse metadata documents from a hex string
    ///
    /// This function can handle hex strings containing one or multiple Rain metadata documents
    /// and returns a vector of parsed UnpackedMetadata items.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rain_metadata::UnpackedMetadata;
    ///
    /// let hex_str = "ff0a89c674ee7874"; // Valid hex (but incomplete Rain metadata for demo)
    /// let result = UnpackedMetadata::parse_from_hex(hex_str);
    /// assert!(result.is_err()); // Will fail due to invalid format, which is expected for demo
    /// ```
    pub fn parse_from_hex(hex_str: &str) -> Result<Vec<Self>, Error> {
        // Remove 0x prefix if present
        let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);

        // Decode hex string to bytes
        let meta_bytes = hex::decode(hex_str).map_err(Error::DecodeHexStringError)?;

        // Check if it starts with RainMetaDocumentV1 prefix
        if !meta_bytes.starts_with(&KnownMagic::RainMetaDocumentV1.to_prefix_bytes()) {
            return Err(Error::CorruptMeta);
        }

        // Decode CBOR to get RainMetaDocumentV1Items
        let rain_meta_documents = RainMetaDocumentV1Item::cbor_decode(&meta_bytes)?;

        // Convert all documents to UnpackedMetadata
        rain_meta_documents
            .into_iter()
            .map(|doc| doc.try_into())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpacked_metadata_magic() {
        // Test that magic() returns correct values for each variant
        let authoring_v1 = UnpackedMetadata::AuthoringV1(AuthoringMeta(vec![]));
        assert_eq!(authoring_v1.magic(), KnownMagic::AuthoringMetaV1);

        let dotrain_v1 = UnpackedMetadata::DotrainV1("test".to_string());
        assert_eq!(dotrain_v1.magic(), KnownMagic::DotrainV1);
    }

    #[test]
    fn test_type_checking_methods() {
        let authoring_v1 = UnpackedMetadata::AuthoringV1(AuthoringMeta(vec![]));
        assert!(authoring_v1.is_authoring_v1());
        assert!(!authoring_v1.is_dotrain_v1());
        assert!(!authoring_v1.is_authoring_v2());

        let dotrain_v1 = UnpackedMetadata::DotrainV1("test".to_string());
        assert!(dotrain_v1.is_dotrain_v1());
        assert!(!dotrain_v1.is_authoring_v1());
    }

    #[test]
    fn test_serialization_deserialization() {
        // Test with DotrainV1 (String content)
        let original_dotrain = UnpackedMetadata::DotrainV1("test dotrain content".to_string());

        // Serialize to JSON
        let serialized = serde_json::to_string(&original_dotrain).unwrap();

        // Deserialize back
        let deserialized: UnpackedMetadata = serde_json::from_str(&serialized).unwrap();

        // Check that it's the same
        if let UnpackedMetadata::DotrainV1(content) = deserialized {
            assert_eq!(content, "test dotrain content");
        } else {
            panic!("Expected DotrainV1 variant");
        }

        // Test with AuthoringV1 (struct)
        let original_authoring = UnpackedMetadata::AuthoringV1(AuthoringMeta(vec![]));

        // Serialize to JSON
        let serialized = serde_json::to_string(&original_authoring).unwrap();

        // Deserialize back
        let deserialized: UnpackedMetadata = serde_json::from_str(&serialized).unwrap();

        // Check that it's the same
        if let UnpackedMetadata::AuthoringV1(authoring) = deserialized {
            assert_eq!(authoring.0, vec![]);
        } else {
            panic!("Expected AuthoringV1 variant");
        }

        // Test with AddressList (Vec<u8>)
        let original_address_list = UnpackedMetadata::AddressList(vec![1, 2, 3, 4]);

        // Serialize to JSON
        let serialized = serde_json::to_string(&original_address_list).unwrap();

        // Deserialize back
        let deserialized: UnpackedMetadata = serde_json::from_str(&serialized).unwrap();

        // Check that it's the same
        if let UnpackedMetadata::AddressList(bytes) = deserialized {
            assert_eq!(bytes, vec![1, 2, 3, 4]);
        } else {
            panic!("Expected AddressList variant");
        }
    }

    #[test]
    fn test_serialization_roundtrip_with_meta_item() {
        use crate::meta::{ContentType, ContentEncoding, ContentLanguage};

        // Create a RainMetaDocumentV1Item
        let meta_item = RainMetaDocumentV1Item {
            payload: b"test dotrain content".to_vec().into(),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::Identity,
            content_language: ContentLanguage::En,
        };

        // Unpack it to UnpackedMetadata
        let unpacked: UnpackedMetadata = meta_item.try_into().unwrap();

        // Serialize to JSON
        let serialized = serde_json::to_string(&unpacked).unwrap();

        // Deserialize back
        let deserialized: UnpackedMetadata = serde_json::from_str(&serialized).unwrap();

        // Check that it's the same
        assert!(deserialized.is_dotrain_v1());
        if let UnpackedMetadata::DotrainV1(content) = deserialized {
            assert_eq!(content, "test dotrain content");
        } else {
            panic!("Expected DotrainV1 variant");
        }
    }

    #[test]
    fn test_parse_from_hex_single_document() {
        let hex_str = "ff0a89c674ee7874a30058fda66c646f747261696e5f68617368582012121212121212121212121212121212121212121212121212121212121212126c6669656c645f76616c756573a166616d6f756e74a362696466616d6f756e74646e616d6566416d6f756e746576616c756563313030686465706f73697473a06d73656c6563745f746f6b656e73a16b696e7075742d746f6b656ea2676e6574776f726b68657468657265756d6761646472657373544242424242424242424242424242424242424242697661756c745f696473a267696e7075742d30697661756c742d313233686f75747075742d30f67373656c65637465645f6465706c6f796d656e74676d61696e6e6574011bffda7b2fb167c2860278186170706c69636174696f6e2f6f637465742d73747265616d";

        // Test parsing multiple documents
        let result = UnpackedMetadata::parse_from_hex(hex_str);
        assert!(
            result.is_ok(),
            "Failed to parse hex string: {:?}",
            result.err()
        );

        let metadata_items = result.unwrap();
        assert_eq!(metadata_items.len(), 1, "Expected 1 metadata document");

        let item = &metadata_items[0];
        assert!(
            item.is_dotrain_instance_v1(),
            "Expected DotrainInstanceV1 document"
        );
    }

    #[test]
    fn test_parse_from_hex_multiple_documents() {
        let hex_str = "ff0a89c674ee7874a3005824236d61696e205f205f3a20696e742d616464283120322920696e742d6164642832203329011bff13109e41336ff20278186170706c69636174696f6e2f6f637465742d73747265616da30058fda66c646f747261696e5f68617368582012121212121212121212121212121212121212121212121212121212121212126c6669656c645f76616c756573a166616d6f756e74a362696466616d6f756e74646e616d6566416d6f756e746576616c756563313030686465706f73697473a06d73656c6563745f746f6b656e73a16b696e7075742d746f6b656ea2676e6574776f726b68657468657265756d6761646472657373544242424242424242424242424242424242424242697661756c745f696473a267696e7075742d30697661756c742d313233686f75747075742d30f67373656c65637465645f6465706c6f796d656e74676d61696e6e6574011bffda7b2fb167c2860278186170706c69636174696f6e2f6f637465742d73747265616d";

        // Test parsing multiple documents
        let result = UnpackedMetadata::parse_from_hex(hex_str);
        assert!(
            result.is_ok(),
            "Failed to parse hex string: {:?}",
            result.err()
        );

        let metadata_items = result.unwrap();
        assert_eq!(metadata_items.len(), 2, "Expected 2 metadata documents");

        // Check that we have one RainlangSource and one DotrainInstanceV1
        let mut has_rainlang_source = false;
        let mut has_dotrain_instance = false;

        for item in &metadata_items {
            match item {
                UnpackedMetadata::RainlangSourceV1(_) => {
                    has_rainlang_source = true;
                }
                UnpackedMetadata::DotrainInstanceV1(_) => {
                    has_dotrain_instance = true;
                }
                _ => {
                    println!("Unexpected metadata type: {:?}", item.magic());
                }
            }
        }

        assert!(has_rainlang_source, "Expected RainlangSourceV1 document");
        assert!(has_dotrain_instance, "Expected DotrainInstanceV1 document");
    }

    #[test]
    fn test_parse_from_hex_error_cases() {
        // Test empty string
        let result = UnpackedMetadata::parse_from_hex("");
        assert!(result.is_err());

        // Test invalid hex
        let result = UnpackedMetadata::parse_from_hex("invalid_hex");
        assert!(result.is_err());

        // Test hex without proper prefix
        let result = UnpackedMetadata::parse_from_hex("deadbeef");
        assert!(result.is_err());

        // Test with 0x prefix but invalid hex
        let result = UnpackedMetadata::parse_from_hex("0xinvalid_hex");
        assert!(result.is_err());
    }
}
