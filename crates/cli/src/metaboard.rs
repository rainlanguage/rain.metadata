use alloy::primitives::{Bytes, FixedBytes};
use alloy::sol_types::SolCall;
use rain_metadata_bindings::MetaBoard::emitMetaCall;

use crate::{Error, RainMetaDocumentV1Item};

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
