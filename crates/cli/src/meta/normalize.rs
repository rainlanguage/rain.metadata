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

#[cfg(test)]
mod tests {
    use crate::meta::KnownMeta;
    use crate::error::Error;

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
}
