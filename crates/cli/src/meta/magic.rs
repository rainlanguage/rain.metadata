/// All known Rain magic numbers
#[derive(
    serde::Serialize,
    Clone,
    Copy,
    strum::EnumIter,
    strum::EnumString,
    strum::Display,
    Debug,
    PartialEq,
    serde::Deserialize,
)]
#[strum(serialize_all = "kebab_case")]
#[serde(rename_all = "kebab-case")]
#[repr(u64)]
pub enum KnownMagic {
    /// Prefixes every rain meta document
    RainMetaDocumentV1 = 0xff0a89c674ee7874,

    /// Ops meta v1
    OpMetaV1 = 0xffe5282f43e495b4,
    /// Dotrain meta v1
    DotrainV1 = 0xffdac2f2f37be894,
    /// Rainlang meta v1
    RainlangV1 = 0xff1c198cec3b48a7,
    /// Solidity ABI meta v2
    SolidityAbiV2 = 0xffe5ffb4a3ff2cde,
    /// Authoring meta v1
    AuthoringMetaV1 = 0xffe9e3a02ca8e235,
    // Authoring meta v2
    AuthoringMetaV2 = 0xff52fe42f1a05093,
    /// InterpreterCaller meta v1
    InterpreterCallerMetaV1 = 0xffc21bbf86cc199b,
    /// ExpressionDeployer deployed bytecode meta v1
    ExpressionDeployerV2BytecodeV1 = 0xffdb988a8cd04d32,
    /// Rainlang source code meta v1
    RainlangSourceV1 = 0xff13109e41336ff2,
    //Address list meta
    AddressList = 0xffb2637608c09e38,
    /// Dotrain source code meta v1
    DotrainSourceV1 = 0xffa15ef0fc437099,
    /// Order builder state meta v1
    OrderBuilderStateV1 = 0xffda7b2fb167c286,
    /// Signed context oracle endpoint v1
    /// Payload is raw UTF-8 bytes containing the oracle endpoint URL.
    /// Used in order metadata to tell takers where to GET signed context data.
    RaindexSignedContextOracleV1 = 0xff7a1507ba4419ca,
    /// OffchainAsset schema reference for offchain asset data
    OaSchema = 0xffa8e8a9b9cf4a31,
    /// OffchainAsset IPFS hash list for offchain assets
    OaHashList = 0xff9fae3cc645f463,
    /// OffchainAsset structured data (e.g. receipt information)
    OaStructure = 0xffc47a6299e8a911,
    /// OffchainAsset token image metadata
    OaTokenImage = 0xff8cd2927c8c86cb,
    /// OffchainAsset token credential links
    OaTokenCredentialLinks = 0xffbc38eb14ad2209,
}

impl KnownMagic {
    pub fn to_prefix_bytes(&self) -> [u8; 8] {
        // Use big endian here as the magic numbers are for binary data prefixes.
        (*self as u64).to_be_bytes()
    }
}

impl TryFrom<u64> for KnownMagic {
    type Error = crate::error::Error;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            v if v == KnownMagic::OpMetaV1 as u64 => Ok(KnownMagic::OpMetaV1),
            v if v == KnownMagic::DotrainV1 as u64 => Ok(KnownMagic::DotrainV1),
            v if v == KnownMagic::RainlangV1 as u64 => Ok(KnownMagic::RainlangV1),
            v if v == KnownMagic::SolidityAbiV2 as u64 => Ok(KnownMagic::SolidityAbiV2),
            v if v == KnownMagic::AuthoringMetaV1 as u64 => Ok(KnownMagic::AuthoringMetaV1),
            v if v == KnownMagic::AuthoringMetaV2 as u64 => Ok(KnownMagic::AuthoringMetaV2),
            v if v == KnownMagic::AddressList as u64 => Ok(KnownMagic::AddressList),
            v if v == KnownMagic::RainMetaDocumentV1 as u64 => Ok(KnownMagic::RainMetaDocumentV1),
            v if v == KnownMagic::InterpreterCallerMetaV1 as u64 => {
                Ok(KnownMagic::InterpreterCallerMetaV1)
            }
            v if v == KnownMagic::ExpressionDeployerV2BytecodeV1 as u64 => {
                Ok(KnownMagic::ExpressionDeployerV2BytecodeV1)
            }
            v if v == KnownMagic::RainlangSourceV1 as u64 => Ok(KnownMagic::RainlangSourceV1),
            v if v == KnownMagic::DotrainSourceV1 as u64 => Ok(KnownMagic::DotrainSourceV1),
            v if v == KnownMagic::OrderBuilderStateV1 as u64 => Ok(KnownMagic::OrderBuilderStateV1),
            v if v == KnownMagic::RaindexSignedContextOracleV1 as u64 => {
                Ok(KnownMagic::RaindexSignedContextOracleV1)
            }
            v if v == KnownMagic::OaSchema as u64 => Ok(KnownMagic::OaSchema),
            v if v == KnownMagic::OaHashList as u64 => Ok(KnownMagic::OaHashList),
            v if v == KnownMagic::OaStructure as u64 => Ok(KnownMagic::OaStructure),
            v if v == KnownMagic::OaTokenImage as u64 => Ok(KnownMagic::OaTokenImage),
            v if v == KnownMagic::OaTokenCredentialLinks as u64 => {
                Ok(KnownMagic::OaTokenCredentialLinks)
            }
            _ => Err(crate::error::Error::UnknownMagic),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::KnownMagic;
    use alloy::primitives::hex;

    #[test]
    fn test_rain_meta_document_v1() {
        let magic_number = KnownMagic::RainMetaDocumentV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff0a89c674ee7874");
    }

    #[test]
    fn test_solidity_abi_v2() {
        let magic_number = KnownMagic::SolidityAbiV2;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffe5ffb4a3ff2cde");
    }

    #[test]
    fn test_op_meta_v1() {
        let magic_number = KnownMagic::OpMetaV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffe5282f43e495b4");
    }

    #[test]
    fn test_interpreter_caller_meta_v1() {
        let magic_number = KnownMagic::InterpreterCallerMetaV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffc21bbf86cc199b");
    }

    #[test]
    fn test_authoring_meta_v1() {
        let magic_number = KnownMagic::AuthoringMetaV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffe9e3a02ca8e235");
    }

    #[test]
    fn test_authoring_meta_v2() {
        let magic_number = KnownMagic::AuthoringMetaV2;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff52fe42f1a05093");
    }

    #[test]
    fn test_dotrain_meta_v1() {
        let magic_number = KnownMagic::DotrainV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffdac2f2f37be894");
    }

    #[test]
    fn test_rainlang_meta_v1() {
        let magic_number = KnownMagic::RainlangV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff1c198cec3b48a7");
    }

    #[test]
    fn test_expression_deployer_v2_bytecode_meta_v1() {
        let magic_number = KnownMagic::ExpressionDeployerV2BytecodeV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffdb988a8cd04d32");
    }

    #[test]
    fn test_rainlang_source_meta_v1() {
        let magic_number = KnownMagic::RainlangSourceV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff13109e41336ff2");
    }

    #[test]
    fn test_dotrain_source_meta_v1() {
        let magic_number = KnownMagic::DotrainSourceV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffa15ef0fc437099");
    }

    #[test]
    fn test_dotrain_instance_meta_v1() {
        let magic_number = KnownMagic::OrderBuilderStateV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffda7b2fb167c286");
    }

    #[test]
    fn test_signed_context_oracle_v1() {
        let magic_number = KnownMagic::RaindexSignedContextOracleV1;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff7a1507ba4419ca");
    }

    #[test]
    fn test_signed_context_oracle_v1_roundtrip() {
        let magic_number = KnownMagic::RaindexSignedContextOracleV1;
        let from_u64 = KnownMagic::try_from(magic_number as u64).unwrap();
        assert_eq!(magic_number, from_u64);
    }

    #[test]
    fn test_oa_schema() {
        let magic_number = KnownMagic::OaSchema;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffa8e8a9b9cf4a31");
    }

    #[test]
    fn test_oa_hash_list() {
        let magic_number = KnownMagic::OaHashList;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff9fae3cc645f463");
    }

    #[test]
    fn test_oa_structure() {
        let magic_number = KnownMagic::OaStructure;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffc47a6299e8a911");
    }

    #[test]
    fn test_oa_token_image() {
        let magic_number = KnownMagic::OaTokenImage;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ff8cd2927c8c86cb");
    }

    #[test]
    fn test_oa_token_credential_links() {
        let magic_number = KnownMagic::OaTokenCredentialLinks;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffbc38eb14ad2209");
    }

    #[test]
    fn test_oa_magics_roundtrip() {
        for magic_number in [
            KnownMagic::OaSchema,
            KnownMagic::OaHashList,
            KnownMagic::OaStructure,
            KnownMagic::OaTokenImage,
            KnownMagic::OaTokenCredentialLinks,
        ] {
            let from_u64 = KnownMagic::try_from(magic_number as u64).unwrap();
            assert_eq!(magic_number, from_u64);
        }
    }

    #[test]
    fn test_address_list() {
        let magic_number = KnownMagic::AddressList;
        let magic_number_after_prefix = magic_number.to_prefix_bytes();

        assert_eq!(hex::encode(magic_number_after_prefix), "ffb2637608c09e38");
    }

    /// Pins every discriminant to its published magic number so no variant can
    /// silently change value. Values for the documented types come from the
    /// metadata-v1 spec magic number table.
    #[test]
    fn test_all_discriminants_pinned() {
        use strum::IntoEnumIterator;
        let expected: [(KnownMagic, &str); 19] = [
            (KnownMagic::RainMetaDocumentV1, "ff0a89c674ee7874"),
            (KnownMagic::OpMetaV1, "ffe5282f43e495b4"),
            (KnownMagic::DotrainV1, "ffdac2f2f37be894"),
            (KnownMagic::RainlangV1, "ff1c198cec3b48a7"),
            (KnownMagic::SolidityAbiV2, "ffe5ffb4a3ff2cde"),
            (KnownMagic::AuthoringMetaV1, "ffe9e3a02ca8e235"),
            (KnownMagic::AuthoringMetaV2, "ff52fe42f1a05093"),
            (KnownMagic::InterpreterCallerMetaV1, "ffc21bbf86cc199b"),
            (
                KnownMagic::ExpressionDeployerV2BytecodeV1,
                "ffdb988a8cd04d32",
            ),
            (KnownMagic::RainlangSourceV1, "ff13109e41336ff2"),
            (KnownMagic::AddressList, "ffb2637608c09e38"),
            (KnownMagic::DotrainSourceV1, "ffa15ef0fc437099"),
            (KnownMagic::OrderBuilderStateV1, "ffda7b2fb167c286"),
            (KnownMagic::RaindexSignedContextOracleV1, "ff7a1507ba4419ca"),
            (KnownMagic::OaSchema, "ffa8e8a9b9cf4a31"),
            (KnownMagic::OaHashList, "ff9fae3cc645f463"),
            (KnownMagic::OaStructure, "ffc47a6299e8a911"),
            (KnownMagic::OaTokenImage, "ff8cd2927c8c86cb"),
            (KnownMagic::OaTokenCredentialLinks, "ffbc38eb14ad2209"),
        ];
        // every variant is pinned exactly once
        assert_eq!(expected.len(), KnownMagic::iter().count());
        for (magic, hex_str) in expected {
            assert_eq!(hex::encode(magic.to_prefix_bytes()), hex_str, "{:?}", magic);
        }
    }

    /// Every magic number begins with 0xff so a prefix can never be a valid
    /// utf-8 sequence, per the metadata-v1 spec.
    #[test]
    fn test_all_prefixes_start_with_0xff() {
        use strum::IntoEnumIterator;
        for magic in KnownMagic::iter() {
            assert_eq!(magic.to_prefix_bytes()[0], 0xff, "{:?}", magic);
        }
    }

    /// TryFrom<u64> roundtrips every variant back to itself.
    #[test]
    fn test_try_from_u64_roundtrip_all() {
        use strum::IntoEnumIterator;
        for magic in KnownMagic::iter() {
            let from_u64 = KnownMagic::try_from(magic as u64).unwrap();
            assert_eq!(magic, from_u64);
        }
    }

    /// Values that are not known magic numbers map to Error::UnknownMagic.
    #[test]
    fn test_try_from_u64_unknown_magic() {
        for unknown in [0u64, 1, 0xdeadbeef, 0xff0a89c674ee7875, u64::MAX] {
            assert!(matches!(
                KnownMagic::try_from(unknown),
                Err(crate::error::Error::UnknownMagic)
            ));
        }
    }

    /// Strum parse/display uses the kebab-case names that the CLI documents,
    /// e.g. the build command's default global magic "rain-meta-document-v1".
    #[test]
    fn test_strum_kebab_case_parse_display() {
        use std::str::FromStr;
        let cases: [(KnownMagic, &str); 19] = [
            (KnownMagic::RainMetaDocumentV1, "rain-meta-document-v1"),
            (KnownMagic::OpMetaV1, "op-meta-v1"),
            (KnownMagic::DotrainV1, "dotrain-v1"),
            (KnownMagic::RainlangV1, "rainlang-v1"),
            (KnownMagic::SolidityAbiV2, "solidity-abi-v2"),
            (KnownMagic::AuthoringMetaV1, "authoring-meta-v1"),
            (KnownMagic::AuthoringMetaV2, "authoring-meta-v2"),
            (
                KnownMagic::InterpreterCallerMetaV1,
                "interpreter-caller-meta-v1",
            ),
            (
                KnownMagic::ExpressionDeployerV2BytecodeV1,
                "expression-deployer-v2-bytecode-v1",
            ),
            (KnownMagic::RainlangSourceV1, "rainlang-source-v1"),
            (KnownMagic::AddressList, "address-list"),
            (KnownMagic::DotrainSourceV1, "dotrain-source-v1"),
            (KnownMagic::OrderBuilderStateV1, "order-builder-state-v1"),
            (
                KnownMagic::RaindexSignedContextOracleV1,
                "raindex-signed-context-oracle-v1",
            ),
            (KnownMagic::OaSchema, "oa-schema"),
            (KnownMagic::OaHashList, "oa-hash-list"),
            (KnownMagic::OaStructure, "oa-structure"),
            (KnownMagic::OaTokenImage, "oa-token-image"),
            (
                KnownMagic::OaTokenCredentialLinks,
                "oa-token-credential-links",
            ),
        ];
        for (magic, name) in cases {
            assert_eq!(magic.to_string(), name, "{:?}", magic);
            assert_eq!(KnownMagic::from_str(name).unwrap(), magic, "{}", name);
        }
    }

    /// Serde serializes to the same kebab-case names and deserializes them
    /// back.
    #[test]
    fn test_serde_kebab_case_roundtrip() {
        use strum::IntoEnumIterator;
        for magic in KnownMagic::iter() {
            let json = serde_json::to_string(&magic).unwrap();
            assert_eq!(json, format!("\"{}\"", magic), "{:?}", magic);
            let back: KnownMagic = serde_json::from_str(&json).unwrap();
            assert_eq!(back, magic);
        }
    }
}
