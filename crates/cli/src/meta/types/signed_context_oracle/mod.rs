use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::Error,
    meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item},
};

#[cfg(target_family = "wasm")]
use wasm_bindgen_utils::{impl_wasm_traits, prelude::*};

/// Signed Context Oracle V1 meta.
///
/// Contains a validated URL pointing to a GET endpoint that returns
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
    /// Create a new SignedContextOracleV1 from a URL.
    /// Validates that the input is a well-formed URL.
    pub fn new(url: Url) -> Self {
        Self(url.to_string())
    }

    /// Parse and create a new SignedContextOracleV1 from a string.
    /// Returns an error if the string is not a valid URL.
    pub fn parse(url: &str) -> Result<Self, Error> {
        let parsed = Url::parse(url).map_err(|e| Error::InvalidUrl(e.to_string()))?;
        Ok(Self::new(parsed))
    }

    /// Get the oracle URL as a string.
    pub fn url(&self) -> &str {
        &self.0
    }

    /// Get the oracle URL as a parsed Url.
    pub fn parsed_url(&self) -> Result<Url, Error> {
        Url::parse(&self.0).map_err(|e| Error::InvalidUrl(e.to_string()))
    }

    /// Encode this oracle descriptor as a `RainMetaDocumentV1Item`.
    /// The payload is raw UTF-8 bytes of the URL string.
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
    /// Returns `Ok(None)` if no oracle meta is found.
    /// Returns `Err` if oracle meta is found but cannot be decoded.
    pub fn find_in_items(items: &[RainMetaDocumentV1Item]) -> Result<Option<Self>, Error> {
        match items
            .iter()
            .find(|item| matches!(item.magic, KnownMagic::SignedContextOracleV1))
        {
            Some(item) => Ok(Some(Self::try_from(item.clone())?)),
            None => Ok(None),
        }
    }
}

impl TryFrom<RainMetaDocumentV1Item> for SignedContextOracleV1 {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        if !matches!(value.magic, KnownMagic::SignedContextOracleV1) {
            return Err(Error::UnsupportedMeta);
        }
        let bytes = value.unpack()?;
        let url_str = String::from_utf8(bytes)?;
        Self::parse(&url_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let url = "https://oracle.example.com/prices/eth-usd";
        let oracle = SignedContextOracleV1::parse(url).unwrap();

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
        let oracle = SignedContextOracleV1::parse(url).unwrap();
        let item = oracle.to_meta_item();

        let found = SignedContextOracleV1::find_in_items(&[item]).unwrap().unwrap();
        assert_eq!(found.url(), url);
    }

    #[test]
    fn test_find_in_items_missing() {
        let items: Vec<RainMetaDocumentV1Item> = vec![];
        assert!(SignedContextOracleV1::find_in_items(&items).unwrap().is_none());
    }

    #[test]
    fn test_find_in_items_decode_error() {
        // Oracle magic but invalid payload (not valid UTF-8 URL)
        let item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(vec![0xFF, 0xFE]),
            magic: KnownMagic::SignedContextOracleV1,
            content_type: ContentType::None,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::None,
        };
        assert!(SignedContextOracleV1::find_in_items(&[item]).is_err());
    }

    #[test]
    fn test_new_with_url() {
        let url = Url::parse("https://example.com/feed").unwrap();
        let oracle = SignedContextOracleV1::new(url);
        assert_eq!(oracle.url(), "https://example.com/feed");
    }

    #[test]
    fn test_parse_valid_url() {
        let oracle = SignedContextOracleV1::parse("https://example.com/feed").unwrap();
        assert_eq!(oracle.url(), "https://example.com/feed");
    }

    #[test]
    fn test_parse_invalid_url() {
        assert!(SignedContextOracleV1::parse("not a url").is_err());
    }

    #[test]
    fn test_parse_empty_url() {
        assert!(SignedContextOracleV1::parse("").is_err());
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

    #[test]
    fn test_parsed_url() {
        let oracle = SignedContextOracleV1::parse("https://example.com/feed?pair=eth-usd").unwrap();
        let parsed = oracle.parsed_url().unwrap();
        assert_eq!(parsed.host_str(), Some("example.com"));
        assert_eq!(parsed.path(), "/feed");
    }
}
