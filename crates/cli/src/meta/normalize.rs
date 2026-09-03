use super::{
    KnownMeta,
    super::error::Error,
    types::{
        authoring::v1::AuthoringMeta, authoring::v2::AuthoringMetaV2,
        solidity_abi::v2::SolidityAbiMeta, interpreter_caller::v1::InterpreterCallerMeta,
    },
};

fn normalize_json<'de, T>(data: &'de [u8]) -> Result<Vec<u8>, Error>
where
    T: serde::Deserialize<'de> + serde::Serialize + validator::Validate,
{
    let parsed = serde_json::from_str::<T>(std::str::from_utf8(data)?)?;
    parsed.validate()?;
    Ok(serde_json::to_string(&parsed)?.as_bytes().to_vec())
}

impl KnownMeta {
    /// normalizes meta types and also performs validation on those that need validation
    pub fn normalize(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(match self {
            KnownMeta::SolidityAbiV2 => normalize_json::<SolidityAbiMeta>(data)?,
            KnownMeta::InterpreterCallerMetaV1 => normalize_json::<InterpreterCallerMeta>(data)?,
            KnownMeta::AuthoringMetaV1 => {
                // for AuthoringMeta since it can be a json or abi encoded bytes, we try to abi
                // decode first and then json deserialize if that fails, if either succeeds
                // then the result of that will be abi encoded with validation
                match AuthoringMeta::abi_decode(data) {
                    Ok(am) => am.abi_encode_validate()?,
                    _ => AuthoringMeta::abi_encode_validate(
                        &serde_json::from_str::<AuthoringMeta>(std::str::from_utf8(data)?)?,
                    )?,
                }
            }
            KnownMeta::AuthoringMetaV2 => {
                // v2 is abi encoded onchain and this crate has no encoder for
                // it, so validation is a decode gate over the input as is,
                // carrying the rain word grammar
                AuthoringMetaV2::abi_decode_validate(data)
                    .map_err(|e| Error::InvalidInput(e.to_string()))?;
                data.to_vec()
            }
            // rest of meta types are only pure bytes (ut8 strings or binary)
            // so no normalization/validation can happen for them at this level
            _ => data.to_vec(),
        })
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use alloy::sol_types::SolValue;
    use crate::error::Error;
    use crate::meta::types::authoring::v1::{AuthoringMeta, AuthoringMetaItem};
    use crate::meta::types::authoring::v2::AuthoringMetaV2Sol;
    use crate::meta::KnownMeta;

    fn authoring_meta_v2_abi(word: [u8; 32], description: &str) -> Vec<u8> {
        vec![AuthoringMetaV2Sol {
            word: word.into(),
            description: description.to_string(),
        }]
        .abi_encode()
    }

    /// OpV1 is a known meta this crate does not model, so normalize passes
    /// its bytes through rather than validating them. It reaches the same
    /// fallthrough as every other unmodelled type.
    #[test]
    fn test_normalize_op_v1_is_a_passthrough() {
        let bytes = b"{  \"name\" : \"add\" }";
        assert_eq!(KnownMeta::OpV1.normalize(bytes).unwrap(), bytes.to_vec());
    }

    /// SolidityAbiV2 rejects data that is not json and data that is not utf8.
    #[test]
    fn test_normalize_solidity_abi_v2_rejects_bad_input() {
        assert!(matches!(
            KnownMeta::SolidityAbiV2.normalize(b"not json at all"),
            Err(Error::SerdeJsonError(_))
        ));
        assert!(matches!(
            KnownMeta::SolidityAbiV2.normalize(&[0xff, 0xfe]),
            Err(Error::Utf8Error(_))
        ));
    }

    /// SolidityAbiV2 normalizes whitespace away to the canonical compact form.
    #[test]
    fn test_normalize_solidity_abi_v2_canonicalizes() {
        assert_eq!(
            KnownMeta::SolidityAbiV2.normalize(b"[ ]").unwrap(),
            b"[]".to_vec()
        );
    }

    /// InterpreterCallerMetaV1 parses but rejects metadata failing validation:
    /// at least one method is required.
    #[test]
    fn test_normalize_interpreter_caller_rejects_empty_methods() {
        let invalid = br#"{"name":"Test Caller","abiName":"TestCaller","methods":[]}"#;
        assert!(matches!(
            KnownMeta::InterpreterCallerMetaV1.normalize(invalid),
            Err(Error::ValidationErrors(_))
        ));
    }

    /// Meta types with no json schema at this level pass through untouched.
    #[test]
    fn test_normalize_passthrough_for_binary_metas() {
        let data = vec![0x00, 0x01, 0xff];
        assert_eq!(
            KnownMeta::ExpressionDeployerV2BytecodeV1
                .normalize(&data)
                .unwrap(),
            data
        );
    }

    fn sample_authoring_meta() -> AuthoringMeta {
        serde_json::from_str(
            r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#,
        )
        .unwrap()
    }

    /// Valid abi encoded input takes the abi-decode path and is re-encoded
    /// with validation, byte identically.
    #[test]
    fn test_normalize_authoring_meta_v1_abi_path() {
        let authoring_meta = sample_authoring_meta();
        let abi = authoring_meta.abi_encode_validate().unwrap();
        let normalized = KnownMeta::AuthoringMetaV1.normalize(&abi).unwrap();
        assert_eq!(normalized, abi);
    }

    /// Abi-decodable input that fails validation must be rejected: the abi
    /// path re-encodes via abi_encode_validate, not a passthrough.
    #[test]
    fn test_normalize_authoring_meta_v1_abi_invalid_rejected() {
        let invalid = AuthoringMeta(vec![AuthoringMetaItem {
            word: "NOTKEBAB".to_string(),
            operand_parser_offset: 0,
            description: "some description".to_string(),
        }]);
        // encode WITHOUT validation so the bytes are decodable but invalid
        let abi = invalid.abi_encode().unwrap();
        let result = KnownMeta::AuthoringMetaV1.normalize(&abi);
        assert!(matches!(result, Err(Error::ValidationErrors(_))));
    }

    /// Json input falls back to serde parse and is abi encoded with
    /// validation: output is the abi encoding, not the raw json bytes.
    #[test]
    fn test_normalize_authoring_meta_v1_json_fallback() {
        let json = r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#;
        let expected = sample_authoring_meta().abi_encode_validate().unwrap();
        let normalized = KnownMeta::AuthoringMetaV1
            .normalize(json.as_bytes())
            .unwrap();
        assert_eq!(normalized, expected);
        assert_ne!(normalized, json.as_bytes().to_vec());
    }

    /// AuthoringMetaV2 has a concrete abi encoding, so a decodable payload is
    /// valid and passes through byte identically.
    #[test]
    fn test_normalize_authoring_meta_v2_abi_passthrough() {
        let mut word = [0u8; 32];
        word[..5].copy_from_slice(b"stack");
        let abi = authoring_meta_v2_abi(word, "Copies an existing value from the stack.");
        assert_eq!(KnownMeta::AuthoringMetaV2.normalize(&abi).unwrap(), abi);
    }

    /// AuthoringMetaV2 is not pure bytes: bytes that cannot abi decode as
    /// AuthoringMetaV2Sol[] are rejected rather than passed through.
    #[test]
    fn test_normalize_authoring_meta_v2_rejects_arbitrary_bytes() {
        assert!(matches!(
            KnownMeta::AuthoringMetaV2.normalize(&[0xde, 0xad]),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            KnownMeta::AuthoringMetaV2.normalize(b"[]"),
            Err(Error::InvalidInput(_))
        ));
    }

    /// The decode gate carries the word utf8 requirement: abi shaped bytes
    /// whose word is not utf8 before its first NUL are rejected.
    #[test]
    fn test_normalize_authoring_meta_v2_rejects_non_utf8_word() {
        let mut word = [0u8; 32];
        // 0xc3 followed by 0x28 is an invalid utf8 sequence, before any NUL
        word[0] = 0xc3;
        word[1] = 0x28;
        let abi = authoring_meta_v2_abi(word, "bad word bytes");
        assert!(matches!(
            KnownMeta::AuthoringMetaV2.normalize(&abi),
            Err(Error::InvalidInput(_))
        ));
    }

    /// The decode gate carries the rain word grammar: a word the grammar
    /// rejects does not reach the board through this crate.
    #[test]
    fn test_normalize_authoring_meta_v2_rejects_a_word_outside_the_grammar() {
        let mut word = [0u8; 32];
        word[..3].copy_from_slice(b"BAD");
        let abi = authoring_meta_v2_abi(word, "fine");
        assert!(matches!(
            KnownMeta::AuthoringMetaV2.normalize(&abi),
            Err(Error::InvalidInput(_))
        ));
    }

    /// Meta types without a structured normal form pass raw bytes through
    /// unchanged.
    #[test]
    fn test_normalize_default_arm_passthrough() {
        let data = b"some dotrain text".to_vec();
        assert_eq!(KnownMeta::DotrainV1.normalize(&data).unwrap(), data);
        let binary = vec![0xffu8, 0x00, 0x01];
        assert_eq!(KnownMeta::RainlangV1.normalize(&binary).unwrap(), binary);
    }
}
