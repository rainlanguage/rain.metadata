use std::{string::FromUtf8Error, str::Utf8Error};

use rain_metaboard_subgraph::metaboard_client::MetaboardSubgraphClientError;

use crate::meta::KnownMagic;

/// Covers all errors variants of Rain Metadat lib functionalities
#[derive(Debug)]
pub enum Error {
    CorruptMeta,
    InvalidHash,
    UnknownMeta,
    UnknownMagic,
    NoRecordFound,
    UnsupportedMeta,
    BiggerThan32Bytes,
    UnsupportedNetwork,
    NotRainMetaDocumentV1,
    InflateError(String),
    InvalidInput(String),
    InvalidUrl(String),
    Utf8Error(Utf8Error),
    FromUtf8Error(FromUtf8Error),
    ReqwestError(reqwest::Error),
    SerdeCborError(serde_cbor::Error),
    SerdeJsonError(serde_json::Error),
    AbiCoderError(alloy::sol_types::Error),
    ValidationErrors(validator::ValidationErrors),
    DecodeHexStringError(alloy::primitives::hex::FromHexError),
    InvalidMetaMagic(KnownMagic, KnownMagic),
    MetaboardSubgraphClientError(MetaboardSubgraphClientError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CorruptMeta => f.write_str("corrupt meta"),
            Error::UnknownMeta => f.write_str("unknown meta"),
            Error::UnknownMagic => f.write_str("unknown magic"),
            Error::UnsupportedMeta => f.write_str("unsupported meta"),
            Error::InvalidHash => f.write_str("invalid keccak256 hash"),
            Error::NoRecordFound => f.write_str("found no matching record"),
            Error::UnsupportedNetwork => {
                f.write_str("no rain subgraph is deployed for this network")
            }
            Error::BiggerThan32Bytes => {
                f.write_str("unexpected input size, must be 32 bytes or less")
            }
            Error::NotRainMetaDocumentV1 => {
                f.write_str("data does not begin with the rain meta document v1 magic number")
            }
            Error::InvalidInput(v) => write!(f, "invalid input: {}", v),
            Error::InvalidUrl(v) => write!(f, "invalid URL: {}", v),
            Error::ReqwestError(v) => write!(f, "{}", v),
            Error::InflateError(v) => write!(f, "{}", v),
            Error::Utf8Error(v) => write!(f, "{}", v),
            Error::AbiCoderError(v) => write!(f, "{}", v),
            Error::SerdeCborError(v) => write!(f, "{}", v),
            Error::SerdeJsonError(v) => write!(f, "{}", v),
            Error::FromUtf8Error(v) => write!(f, "{}", v),
            Error::DecodeHexStringError(v) => write!(f, "{}", v),
            Error::ValidationErrors(v) => write!(f, "{}", v),
            Error::InvalidMetaMagic(expected, actual) => {
                write!(
                    f,
                    "invalid meta magic: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            Error::MetaboardSubgraphClientError(v) => write!(f, "{}", v),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::SerdeJsonError(value)
    }
}

impl From<serde_cbor::Error> for Error {
    fn from(value: serde_cbor::Error) -> Self {
        Error::SerdeCborError(value)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(value: FromUtf8Error) -> Self {
        Error::FromUtf8Error(value)
    }
}

impl From<Utf8Error> for Error {
    fn from(value: Utf8Error) -> Self {
        Error::Utf8Error(value)
    }
}

impl From<validator::ValidationErrors> for Error {
    fn from(value: validator::ValidationErrors) -> Self {
        Error::ValidationErrors(value)
    }
}

impl From<alloy::sol_types::Error> for Error {
    fn from(value: alloy::sol_types::Error) -> Self {
        Error::AbiCoderError(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed-string Display arms are part of the CLI's user-facing surface:
    /// pin them exactly.
    #[test]
    fn test_display_fixed_strings() {
        assert_eq!(Error::CorruptMeta.to_string(), "corrupt meta");
        assert_eq!(Error::UnknownMeta.to_string(), "unknown meta");
        assert_eq!(Error::UnknownMagic.to_string(), "unknown magic");
        assert_eq!(Error::UnsupportedMeta.to_string(), "unsupported meta");
        assert_eq!(Error::InvalidHash.to_string(), "invalid keccak256 hash");
        assert_eq!(Error::NoRecordFound.to_string(), "found no matching record");
        assert_eq!(
            Error::UnsupportedNetwork.to_string(),
            "no rain subgraph is deployed for this network"
        );
        assert_eq!(
            Error::BiggerThan32Bytes.to_string(),
            "unexpected input size, must be 32 bytes or less"
        );
        assert_eq!(
            Error::NotRainMetaDocumentV1.to_string(),
            "data does not begin with the rain meta document v1 magic number"
        );
    }

    /// Formatted wrappers carry the wrapped value in the rendered message.
    #[test]
    fn test_display_formatted_wrappers() {
        assert_eq!(
            Error::InvalidInput("abc".to_string()).to_string(),
            "invalid input: abc"
        );
        assert_eq!(
            Error::InvalidUrl("not-a-url".to_string()).to_string(),
            "invalid URL: not-a-url"
        );
        assert_eq!(Error::InflateError("boom".to_string()).to_string(), "boom");
    }

    /// InvalidMetaMagic renders expected first, actual second.
    #[test]
    fn test_display_invalid_meta_magic_field_order() {
        let err = Error::InvalidMetaMagic(KnownMagic::RainMetaDocumentV1, KnownMagic::OpMetaV1);
        assert_eq!(
            err.to_string(),
            "invalid meta magic: expected RainMetaDocumentV1, got OpMetaV1"
        );
    }

    /// From impls must route each source error to its own variant, preserving
    /// the source (observable through the rendered message).
    #[test]
    fn test_from_serde_json_routes_to_serde_json_error() {
        let src = serde_json::from_str::<serde_json::Value>("{oops").unwrap_err();
        let msg = src.to_string();
        let err: Error = src.into();
        match err {
            Error::SerdeJsonError(e) => assert_eq!(e.to_string(), msg),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn test_from_utf8_error_routes_to_utf8_error() {
        // The invalid byte is deliberate: it manufactures the source error.
        #[allow(invalid_from_utf8)]
        let src = std::str::from_utf8(&[0xff]).unwrap_err();
        let err: Error = src.into();
        match err {
            Error::Utf8Error(e) => assert_eq!(e, src),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn test_from_from_utf8_error_routes_to_from_utf8_error() {
        let src = String::from_utf8(vec![0xff]).unwrap_err();
        let msg = src.to_string();
        let err: Error = src.into();
        match err {
            Error::FromUtf8Error(e) => assert_eq!(e.to_string(), msg),
            other => panic!("wrong variant: {:?}", other),
        }
    }
}
