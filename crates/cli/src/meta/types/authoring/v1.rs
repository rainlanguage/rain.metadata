use alloy::sol_types::SolType;
use alloy::sol;
use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationErrors, ValidationError};
use super::super::{
    super::{RainMetaDocumentV1Item, str_to_bytes32, bytes32_to_str, Error},
    common::v1::{REGEX_RAIN_SYMBOL, REGEX_RAIN_STRING},
};

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;

/// authoring meta struct
pub type AuthoringMetaStruct = sol!((bytes32, uint8, string));

/// array of authoring meta struct
pub type AuthoringMetaStructArray = sol!((bytes32, uint8, string)[]);

/// Array of native parser opcode metadata
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct AuthoringMeta(pub Vec<AuthoringMetaItem>);

/// AuthoringMeta single item
#[derive(Validate, Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct AuthoringMetaItem {
    /// Primary word used to identify the opcode.
    #[validate(regex(
        path = "REGEX_RAIN_SYMBOL",
        message = "Must be alphanumeric lower-kebab-case beginning with a letter.\n"
    ))]
    pub word: String,
    /// Operand offest
    pub operand_parser_offset: u8,
    /// Brief description of the opcode.
    #[serde(default)]
    #[validate(regex(
        path = "REGEX_RAIN_STRING",
        message = "Must be printable ASCII characters and whitespace.\n"
    ))]
    pub description: String,
}

impl AuthoringMetaItem {
    pub fn abi_encode(&self) -> Result<Vec<u8>, Error> {
        Ok(AuthoringMetaStruct::abi_encode(&(
            str_to_bytes32(self.word.as_str())?,
            self.operand_parser_offset,
            self.description.clone(),
        )))
    }

    // validates and abi encodes
    pub fn abi_encode_validate(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        self.abi_encode()
    }

    pub fn abi_decode(data: &[u8]) -> Result<AuthoringMetaItem, Error> {
        let result = AuthoringMetaStruct::abi_decode(data)?;
        Ok(AuthoringMetaItem {
            word: bytes32_to_str(&result.0)?.to_string(),
            operand_parser_offset: result.1,
            description: result.2.to_string(),
        })
    }

    // abi decodes and validates
    pub fn abi_decode_validate(data: &[u8]) -> Result<AuthoringMetaItem, Error> {
        let am = AuthoringMetaItem::abi_decode(data)?;
        am.validate()?;
        Ok(am)
    }
}

impl AuthoringMeta {
    /// abi encodes array of AuthoringMeta items
    pub fn abi_encode(&self) -> Result<Vec<u8>, Error> {
        let mut v = vec![];
        for item in &self.0 {
            v.push((
                str_to_bytes32(item.word.as_str())?,
                item.operand_parser_offset,
                item.description.clone(),
            ))
        }
        Ok(AuthoringMetaStructArray::abi_encode(&v))
    }

    /// abi encodes array of AuthoringMeta items after validating each
    pub fn abi_encode_validate(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        self.abi_encode()
    }

    /// abi decodes some data into array of AuthoringMeta
    pub fn abi_decode(data: &[u8]) -> Result<AuthoringMeta, Error> {
        let decoded_items = AuthoringMetaStructArray::abi_decode(data)?;
        let authoring_meta_items = decoded_items
            .into_iter()
            .map(|item| {
                Ok::<_, Error>(AuthoringMetaItem {
                    word: bytes32_to_str(&item.0)?.to_string(),
                    operand_parser_offset: item.1,
                    description: item.2.to_string(),
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(AuthoringMeta(authoring_meta_items))
    }

    /// abi decodes some data into array of AuthoringMeta and validates each decoded item
    pub fn abi_decode_validate(data: &[u8]) -> Result<AuthoringMeta, Error> {
        let authoring_meta = AuthoringMeta::abi_decode(data)?;
        authoring_meta.validate()?;
        Ok(authoring_meta)
    }
}

impl Validate for AuthoringMeta {
    fn validate(&self) -> Result<(), ValidationErrors> {
        for (index, item) in self.0.iter().enumerate() {
            if let Err(mut e) = item.validate() {
                e.add(
                    Box::leak(format!("at index {}", index).into_boxed_str()),
                    ValidationError::new(""),
                );
                return Err(e);
            }
        }
        Ok(())
    }
}

impl TryFrom<Vec<u8>> for AuthoringMeta {
    type Error = Error;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        match AuthoringMeta::abi_decode(&value) {
            Ok(am) => Ok(am),
            Err(_e) => Ok(serde_json::from_str::<AuthoringMeta>(std::str::from_utf8(
                &value,
            )?)?),
        }
    }
}

impl TryFrom<&[u8]> for AuthoringMeta {
    type Error = Error;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match AuthoringMeta::abi_decode(value) {
            Ok(am) => Ok(am),
            Err(_e) => Ok(serde_json::from_str::<AuthoringMeta>(std::str::from_utf8(
                value,
            )?)?),
        }
    }
}

impl TryFrom<RainMetaDocumentV1Item> for AuthoringMeta {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        AuthoringMeta::try_from(value.unpack()?)
    }
}

#[cfg(test)]
mod tests {
    use alloy::sol_types::SolType;
    use alloy::sol;
    use super::{AuthoringMeta, AuthoringMetaItem};
    use crate::{meta::str_to_bytes32, error::Error};

    #[test]
    fn test_encode_decode_validate() -> Result<(), Error> {
        let authoring_meta_content = r#"[
            {
                "word": "stack",
                "description": "Copies an existing value from the stack.",
                "operandParserOffset": 16
            },
            {
                "word": "constant",
                "description": "Copies a constant value onto the stack.",
                "operandParserOffset": 16
            }
        ]"#;
        // check the deserialization
        let authoring_meta: AuthoringMeta = serde_json::from_str(authoring_meta_content)?;
        let expected_authoring_meta = AuthoringMeta(vec![
            AuthoringMetaItem {
                word: "stack".to_string(),
                operand_parser_offset: 16u8,
                description: "Copies an existing value from the stack.".to_string(),
            },
            AuthoringMetaItem {
                word: "constant".to_string(),
                operand_parser_offset: 16u8,
                description: "Copies a constant value onto the stack.".to_string(),
            },
        ]);
        assert_eq!(authoring_meta, expected_authoring_meta);

        // abi encode the authoring meta with performing validation
        let authoring_meta_abi_encoded = authoring_meta.abi_encode_validate()?;
        let expected_abi_encoded_data = <sol!((bytes32, uint8, string)[])>::abi_encode(&vec![
            (
                str_to_bytes32("stack")?,
                16u8,
                "Copies an existing value from the stack.".to_string(),
            ),
            (
                str_to_bytes32("constant")?,
                16u8,
                "Copies a constant value onto the stack.".to_string(),
            ),
        ]);
        // check the encoded bytes agaiinst the expected
        assert_eq!(authoring_meta_abi_encoded, expected_abi_encoded_data);

        let authoring_meta_abi_decoded =
            AuthoringMeta::abi_decode_validate(&authoring_meta_abi_encoded)?;
        assert_eq!(authoring_meta_abi_decoded, expected_authoring_meta);

        Ok(())
    }
}
