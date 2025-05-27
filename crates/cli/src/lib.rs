#![doc(
    html_logo_url = "https://raw.githubusercontent.com/rainlanguage/rainlang-vscode/master/docs/images/rain-logo-icon.svg",
    html_favicon_url = "https://raw.githubusercontent.com/rainlanguage/rainlang-vscode/master/docs/images/rain-logo-icon.svg"
)]
//! # Rain Metadata Tooling
//!
//! A library that provides all the toolings and utilities in order to work with RainLanguage metadata.
//! Dotrain LSP/compiler, Rain Orderbook, etc are a few examples to mention that use this library under the hood.
//!
//! Also provides CLI app (executable binary) to generate desireable Rain cbor encoded metadata based on [Metadata Specs](https://github.com/rainprotocol/specs/blob/main/metadata-v1.md)
//! which for example is used in Rain deployment CI.
//!
//! ## Features
//!
//! `cli` and `json-schema` features are default however, in most cases non of the features are needed for using the lib
//! crate, so they can be disabled by using `default-features = false`, just be aware that `cli` feature is required for
//! building the binary.
//!
//! - `cli`: A [mod@clap] based CLI app module for functionalities of this library, this feature
//!   has [mod@tokio] dependency with features enabled that are compatible for `wasm` family target builds,
//!   Enabling this feature will also enable `json-schema` feature.
//!   This feature is required for building the binary crate.
//! - `json-schema`: Enables implementation of [Json Schema](schemars::JsonSchema) for different [types] of Rain meta.
//! - `tokio-full`: Installs [mod@tokio] with full features which is a dependency of `cli` feature, this
//!   allows for multi-threading of the CLI app (binary), however it results in erroneous builds for `wasm` target family
//!   as explained in [tokio docs](https://docs.rs/tokio/latest/tokio/#wasm-support).this feature is only effective for
//!   binary crate and using it for lib crate just installs a [mod@tokio] with full feature as a dependency as the entire
//!   lib crate doesn't depend on [mod@tokio]. This is because [mod@tokio] is only used as a runtime for binray crate.
//!
//! ## Example
//! ```ignore
//! use rain_meta::{*, types::authoring::v1::AuthoringMeta};
//!
//! let authoring_meta_content = r#"[
//!   {
//!      "word": "stack",
//!      "description": "Copies an existing value from the stack.",
//!      "operandParserOffset": 16
//!   },
//!   {
//!      "word": "constant",
//!      "description": "Copies a constant value onto the stack.",
//!      "operandParserOffset": 16
//!   }
//! ]"#;
//! let authoring_meta: AuthoringMeta = serde_json::from_str(authoring_meta_content).unwrap();
//!
//! // abi encode the authoring meta with performing validation
//! let authoring_meta_abi_encoded = authoring_meta.abi_encode_validate().unwrap();
//!
//! // Constructing a RainMeta item (cbor map)
//! let meta_map = RainMetaDocumentV1Item {
//!   payload: serde_bytes::ByteBuf::from(authoring_meta_abi_encoded),
//!   magic: KnownMagic::AuthoringMetaV1,
//!   content_type: ContentType::Cbor,
//!   content_encoding: ContentEncoding::None,
//!   content_language: ContentLanguage::None,
//! };
//!
//! // cbor encode the meta item
//! let cbor_encoded = meta_map.cbor_encode().unwrap();
//!
//! // decode the data back
//! let cbor_decoded_vec = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded).unwrap();
//!
//! // unpack the payload into AuthoringMeta
//! let unpacked_payload: AuthoringMeta = cbor_decoded_vec[0].clone().unpack_into().unwrap();
//! ```

pub(crate) mod solc;
pub mod meta;
pub mod error;
pub(crate) mod subgraph;

#[cfg(feature = "cli")]
pub mod cli;

// re-export main types and functionalities
pub use solc::*;
pub use meta::*;
pub use error::*;
pub use subgraph::*;
