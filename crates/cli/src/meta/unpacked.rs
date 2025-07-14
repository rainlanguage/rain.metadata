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
/// ```
#[derive(Debug, Clone)]
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
}
