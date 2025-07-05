use serde::{Deserialize, Serialize};

use crate::{
    meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item},
    error::Error,
};

/// Dotrain Source V1 meta
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DotrainSourceV1(pub String);

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
}
