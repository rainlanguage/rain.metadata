use alloy::primitives::{Bytes, FixedBytes, hex};
use alloy::sol_types::SolCall;
use rain_metadata_bindings::MetaBoard::emitMetaCall;
use serde::Serialize;

use crate::{Error, RainMetaDocumentV1Item};
use crate::meta::types::dotrain::source_v1::DotrainSourceV1;

/// Generate calldata for MetaBoard.emitMeta() function using raw bytes
fn generate_emit_data_calldata(subject: FixedBytes<32>, data: Vec<u8>) -> Vec<u8> {
    let call = emitMetaCall {
        subject,
        meta: Bytes::from(data),
    };
    call.abi_encode()
}

/// Generate calldata for MetaBoard.emitMeta() function from any RainMetaDocumentV1Item
pub fn generate_emit_meta_calldata(meta: RainMetaDocumentV1Item) -> Result<Vec<u8>, Error> {
    let meta_bytes = meta.cbor_encode()?;
    let hash = meta.hash(false)?;
    Ok(generate_emit_data_calldata(hash.into(), meta_bytes))
}

/// Deployment data for publishing to MetaBoard
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DeploymentData {
    /// Keccak256 hash of the content (hex-encoded with 0x prefix)
    pub subject: String,
    /// CBOR-encoded metadata bytes (hex-encoded with 0x prefix)
    pub meta_bytes: String,
    /// Complete calldata for MetaBoard.emitMeta() (hex-encoded with 0x prefix)
    pub calldata: String,
}

/// Validate dotrain content - basic checks
fn validate_dotrain_content(content: &str) -> Result<(), Error> {
    // Check if content is empty or only whitespace
    if content.trim().is_empty() {
        return Err(Error::InvalidInput(
            "Dotrain content cannot be empty".to_string(),
        ));
    }

    // Try to create DotrainSourceV1 to ensure it's valid
    let _dotrain_source = DotrainSourceV1(content.to_string());

    Ok(())
}

/// Generate deployment data for DotrainSourceV1 content
/// Validates content and returns subject hash, meta bytes, and calldata
pub fn generate_dotrain_deployment(content: &str) -> Result<DeploymentData, Error> {
    // Validate content
    validate_dotrain_content(content)?;

    // Create DotrainSourceV1
    let dotrain_source = DotrainSourceV1(content.to_string());

    // Convert to RainMetaDocumentV1Item
    let document: RainMetaDocumentV1Item = dotrain_source.into();

    // Generate CBOR bytes
    let meta_bytes = document.cbor_encode()?;

    // Calculate subject hash
    let subject_hash = document.hash(false)?;

    // Generate calldata
    let calldata = generate_emit_data_calldata(subject_hash.into(), meta_bytes.clone());

    Ok(DeploymentData {
        subject: hex::encode_prefixed(subject_hash),
        meta_bytes: hex::encode_prefixed(meta_bytes),
        calldata: hex::encode_prefixed(calldata),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic};

    fn create_test_meta(content: &str) -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(content),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::OctetStream,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        }
    }

    #[test]
    fn test_generate_emit_meta_calldata_success() {
        let meta = create_test_meta("test content");
        let calldata = generate_emit_meta_calldata(meta.clone()).unwrap();

        // Verify calldata is not empty
        assert!(!calldata.is_empty());

        // Decode and verify structure
        let decoded = emitMetaCall::abi_decode(&calldata).unwrap();
        let expected_hash = meta.hash(false).unwrap();
        let expected_meta_bytes = meta.cbor_encode().unwrap();

        assert_eq!(decoded.subject, FixedBytes::from(expected_hash));
        assert_eq!(decoded.meta.as_ref(), expected_meta_bytes.as_slice());
    }

    #[test]
    fn test_validate_dotrain_content() {
        // Valid content
        assert!(validate_dotrain_content("#main _ _: int-add(1 2)").is_ok());
        assert!(validate_dotrain_content("/* comment */\n#main _ _: 1").is_ok());

        // Invalid content
        assert!(validate_dotrain_content("").is_err());
        assert!(validate_dotrain_content("   ").is_err());
        assert!(validate_dotrain_content("\n\t\r").is_err());
    }

    #[test]
    fn test_generate_dotrain_deployment_success() {
        let content = "#main _ _: int-add(1 2) int-add(2 3)";
        let deployment = generate_dotrain_deployment(content).unwrap();

        // Check all fields have 0x prefix
        assert!(deployment.subject.starts_with("0x"));
        assert!(deployment.meta_bytes.starts_with("0x"));
        assert!(deployment.calldata.starts_with("0x"));

        // Check lengths are reasonable
        assert_eq!(deployment.subject.len(), 66); // 0x + 64 hex chars
        assert!(deployment.meta_bytes.len() > 10); // Should have some content
        assert!(deployment.calldata.len() > 10); // Should have some content
    }

    #[test]
    fn test_generate_dotrain_deployment_empty_content() {
        let result = generate_dotrain_deployment("");
        assert!(result.is_err());

        let result = generate_dotrain_deployment("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_cas_property_same_content() {
        // Content Addressable Storage: same content -> same calldata
        let meta1 = create_test_meta("identical content");
        let meta2 = create_test_meta("identical content");

        let calldata1 = generate_emit_meta_calldata(meta1).unwrap();
        let calldata2 = generate_emit_meta_calldata(meta2).unwrap();

        // Same content should produce identical calldata
        assert_eq!(calldata1, calldata2);
    }

    #[test]
    fn test_cas_property_different_content() {
        let meta1 = create_test_meta("content A");
        let meta2 = create_test_meta("content B");

        let calldata1 = generate_emit_meta_calldata(meta1).unwrap();
        let calldata2 = generate_emit_meta_calldata(meta2).unwrap();

        // Different content should produce different calldata
        assert_ne!(calldata1, calldata2);

        // Verify different subjects
        let decoded1 = emitMetaCall::abi_decode(&calldata1).unwrap();
        let decoded2 = emitMetaCall::abi_decode(&calldata2).unwrap();
        assert_ne!(decoded1.subject, decoded2.subject);
    }

    #[test]
    fn test_different_magic_same_content() {
        let meta1 = create_test_meta("same content");
        let mut meta2 = create_test_meta("same content");
        meta2.magic = KnownMagic::AuthoringMetaV1; // Different magic

        let calldata1 = generate_emit_meta_calldata(meta1).unwrap();
        let calldata2 = generate_emit_meta_calldata(meta2).unwrap();

        // Different magic should produce different calldata (even with same payload)
        assert_ne!(calldata1, calldata2);
    }

    #[test]
    fn test_hash_deterministic() {
        let meta = create_test_meta("test for deterministic hash");

        // Generate calldata multiple times
        let calldata1 = generate_emit_meta_calldata(meta.clone()).unwrap();
        let calldata2 = generate_emit_meta_calldata(meta.clone()).unwrap();
        let calldata3 = generate_emit_meta_calldata(meta).unwrap();

        // All should be identical (deterministic hashing)
        assert_eq!(calldata1, calldata2);
        assert_eq!(calldata2, calldata3);
    }

    #[test]
    fn test_subject_matches_document_hash() {
        let meta = create_test_meta("content for hash verification");
        let expected_hash = meta.hash(false).unwrap();

        let calldata = generate_emit_meta_calldata(meta).unwrap();
        let decoded = emitMetaCall::abi_decode(&calldata).unwrap();

        // Subject in calldata should match document's own hash
        assert_eq!(decoded.subject, FixedBytes::from(expected_hash));
    }
}
