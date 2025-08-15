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
    InflateError(String),
    InvalidInput(String),
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
            Error::InvalidInput(v) => write!(f, "invalid input: {}", v),
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
