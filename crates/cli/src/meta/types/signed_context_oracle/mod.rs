use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item},
};

#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{impl_wasm_traits, prelude::*};

/// Signed Context Oracle V1 meta.
///
/// Contains a single URL string pointing to a GET endpoint that returns
/// signed context data for use in Rain order evaluation.
///
/// The endpoint must return JSON: `{signer, context, signature}` mapping
/// directly to `SignedContextV1`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(target_family = "wasm", derive(Tsify))]
pub struct SignedContextOracleV1(pub String);

#[cfg(target_family = "wasm")]
impl_wasm_traits!(SignedContextOracleV1);

impl SignedContextOracleV1 {
    /// Create a new SignedContextOracleV1 from a URL string.
    pub fn new(url: String) -> Self {
        Self(url)
    }

    /// Get the oracle URL.
    pub fn url(&self) -> &str {
        &self.0
    }

    /// Encode this oracle descriptor as a `RainMetaDocumentV1Item`.
    pub fn to_meta_item(&self) -> RainMetaDocumentV1Item {
        RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(self.0.as_bytes().to_vec()),
            magic: KnownMagic::SignedContextOracleV1,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        }
    }

    /// Encode as a complete Rain meta document (with magic prefix).
    pub fn cbor_encode(&self) -> Result<Vec<u8>, Error> {
        RainMetaDocumentV1Item::cbor_encode_seq(
            &vec![self.to_meta_item()],
            KnownMagic::RainMetaDocumentV1,
        )
    }

    /// Try to extract a `SignedContextOracleV1` from decoded meta items.
    /// Returns `None` if no oracle meta is found.
    pub fn find_in_items(items: &[RainMetaDocumentV1Item]) -> Option<Self> {
        items
            .iter()
            .find(|item| matches!(item.magic, KnownMagic::SignedContextOracleV1))
            .and_then(|item| Self::try_from(item.clone()).ok())
    }
}

impl TryFrom<RainMetaDocumentV1Item> for SignedContextOracleV1 {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        if !matches!(value.magic, KnownMagic::SignedContextOracleV1) {
            return Err(Error::UnsupportedMeta);
        }
        let bytes = value.unpack()?;
        let url = String::from_utf8(bytes)?;
        Ok(Self(url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let url = "https://oracle.example.com/prices/eth-usd";
        let oracle = SignedContextOracleV1::new(url.to_string());

        // Encode to Rain meta document
        let encoded = oracle.cbor_encode().unwrap();

        // Should start with Rain meta document magic
        assert!(encoded.starts_with(&KnownMagic::RainMetaDocumentV1.to_prefix_bytes()));

        // Decode back
        let items = RainMetaDocumentV1Item::cbor_decode(&encoded).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].magic, KnownMagic::SignedContextOracleV1);

        // Extract oracle
        let decoded = SignedContextOracleV1::try_from(items[0].clone()).unwrap();
        assert_eq!(decoded.url(), url);
    }

    #[test]
    fn test_find_in_items() {
        let url = "https://oracle.example.com/prices/eth-usd";
        let oracle = SignedContextOracleV1::new(url.to_string());
        let item = oracle.to_meta_item();

        let found = SignedContextOracleV1::find_in_items(&[item]).unwrap();
        assert_eq!(found.url(), url);
    }

    #[test]
    fn test_find_in_items_missing() {
        let items: Vec<RainMetaDocumentV1Item> = vec![];
        assert!(SignedContextOracleV1::find_in_items(&items).is_none());
    }

    #[test]
    fn test_new_and_url() {
        let oracle = SignedContextOracleV1::new("https://example.com/feed".to_string());
        assert_eq!(oracle.url(), "https://example.com/feed");
    }

    #[test]
    fn test_wrong_magic_fails() {
        let item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(b"https://example.com".to_vec()),
            magic: KnownMagic::DotrainSourceV1,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };
        assert!(SignedContextOracleV1::try_from(item).is_err());
    }
}
