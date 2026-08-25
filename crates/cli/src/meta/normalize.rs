use super::{
    KnownMeta,
    super::error::Error,
    types::{
        op::v1::OpMeta, authoring::v1::AuthoringMeta, solidity_abi::v2::SolidityAbiMeta,
        interpreter_caller::v1::InterpreterCallerMeta,
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
            KnownMeta::OpV1 => normalize_json::<OpMeta>(data)?,
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
            // rest of meta types are only pure bytes (ut8 strings or binary)
            // so no normalization/validation can happen for them at this level
            _ => data.to_vec(),
        })
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use crate::error::Error;
    use crate::meta::types::authoring::v1::{AuthoringMeta, AuthoringMetaItem};
    use crate::meta::KnownMeta;

    /// OpV1 normalizes valid metadata to its canonical compact json form.
    #[test]
    fn test_normalize_op_v1_canonicalizes() {
        let spaced = b"{  \"name\" : \"add\" ,\n  \"desc\" : \"adds numbers\" }";
        let normalized = KnownMeta::OpV1.normalize(spaced).unwrap();
        assert_eq!(
            String::from_utf8(normalized).unwrap(),
            r#"{"name":"add","desc":"adds numbers","operand":[],"inputs":[],"outputs":[],"aliases":[]}"#
        );
    }

    /// OpV1 rejects metadata that parses but fails validation: opcode names
    /// must be lower-kebab-case rain symbols.
    #[test]
    fn test_normalize_op_v1_rejects_invalid_symbol() {
        let invalid = br#"{"name":"NOT_A_RAIN_SYMBOL"}"#;
        assert!(matches!(
            KnownMeta::OpV1.normalize(invalid),
            Err(Error::ValidationErrors(_))
        ));
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
