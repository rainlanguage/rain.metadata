use std::borrow::Cow;
use alloy::sol_types::SolType;
use alloy::sol;
use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationErrors, ValidationError};
use super::super::{
    super::{KnownMagic, RainMetaDocumentV1Item, str_to_bytes32, bytes32_to_str, Error},
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
                let mut annotation = ValidationError::new("index");
                annotation.add_param(Cow::from("index"), &index);
                e.add("at index", annotation);
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
        // The magic is the item's statement of what its payload is. Bytes
        // that happen to abi decode are not an authoring meta unless the
        // emitter said so.
        if value.magic != KnownMagic::AuthoringMetaV1 {
            return Err(Error::InvalidMetaMagic(
                KnownMagic::AuthoringMetaV1,
                value.magic,
            ));
        }
        AuthoringMeta::try_from(value.unpack()?)
    }
}

#[cfg(test)]
mod tests {
    use alloy::sol_types::SolType;
    use alloy::sol;
    use serde_json::json;
    use validator::ValidationErrorsKind;
    use super::{AuthoringMeta, AuthoringMetaItem};
    use crate::meta::{
        ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item,
    };
    use crate::{meta::str_to_bytes32, error::Error};

    /// The magic is checked before the payload is looked at, so an item of
    /// another type is rejected on its label rather than on whether its bytes
    /// happen to abi decode as an authoring meta.
    #[test]
    fn test_try_from_item_rejects_wrong_magic() {
        for magic in [
            KnownMagic::SolidityAbiV2,
            KnownMagic::AuthoringMetaV2,
            KnownMagic::OpMetaV1,
        ] {
            let item = RainMetaDocumentV1Item {
                payload: serde_bytes::ByteBuf::from(vec![0u8; 0]),
                magic,
                content_type: ContentType::OctetStream,
                content_encoding: ContentEncoding::None,
                content_language: ContentLanguage::None,
                schema: None,
            };
            match AuthoringMeta::try_from(item).unwrap_err() {
                Error::InvalidMetaMagic(expected, actual) => {
                    assert_eq!(expected, KnownMagic::AuthoringMetaV1);
                    assert_eq!(actual, magic);
                }
                other => panic!("expected InvalidMetaMagic for {:?}, got {:?}", magic, other),
            }
        }
    }

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

    #[test]
    fn test_item_encode_decode_roundtrip_offset_and_word_bytes() -> Result<(), Error> {
        let item = AuthoringMetaItem {
            word: "stack".to_string(),
            operand_parser_offset: 16u8,
            description: "some description.".to_string(),
        };
        let encoded = item.abi_encode()?;
        // ABI layout for a single dynamic (bytes32, uint8, string) value: one
        // indirection word (offset 0x20), then the tuple body whose first
        // word carries the word left-aligned and zero padded and whose second
        // word carries the uint8 in its last byte.
        assert_eq!(&encoded[0..31], &[0u8; 31][..]);
        assert_eq!(encoded[31], 0x20u8);
        assert_eq!(&encoded[32..37], &b"stack"[..]);
        assert_eq!(&encoded[37..64], &[0u8; 27][..]);
        assert_eq!(&encoded[64..95], &[0u8; 31][..]);
        assert_eq!(encoded[95], 16u8);
        let decoded = AuthoringMetaItem::abi_decode(&encoded)?;
        assert_eq!(decoded, item);
        Ok(())
    }

    #[test]
    fn test_item_abi_encode_validate_rejects_invalid_word() {
        let item = AuthoringMetaItem {
            // printable ASCII (passes RAIN_STRING) but not lower-kebab-case
            // (fails RAIN_SYMBOL), so word validation specifically must fire.
            word: "Bad Word".to_string(),
            operand_parser_offset: 0u8,
            description: "fine description.".to_string(),
        };
        // encoding itself works, so any failure below is validation
        assert!(item.abi_encode().is_ok());
        assert!(matches!(
            item.abi_encode_validate(),
            Err(Error::ValidationErrors(_))
        ));
    }

    #[test]
    fn test_item_abi_decode_validate_rejects_invalid_word() {
        let item = AuthoringMetaItem {
            word: "Bad Word".to_string(),
            operand_parser_offset: 0u8,
            description: "fine description.".to_string(),
        };
        let encoded = item.abi_encode().unwrap();
        // plain decode accepts the bytes
        assert_eq!(AuthoringMetaItem::abi_decode(&encoded).unwrap(), item);
        // validating decode rejects them
        assert!(matches!(
            AuthoringMetaItem::abi_decode_validate(&encoded),
            Err(Error::ValidationErrors(_))
        ));
    }

    #[test]
    fn test_array_validate_rejects_and_annotates_offending_index() {
        let good = AuthoringMetaItem {
            word: "stack".to_string(),
            operand_parser_offset: 0u8,
            description: "fine description.".to_string(),
        };
        let bad = AuthoringMetaItem {
            word: "Bad Word".to_string(),
            operand_parser_offset: 0u8,
            description: "fine description.".to_string(),
        };
        let annotated_index =
            |items: Vec<AuthoringMetaItem>| match AuthoringMeta(items).abi_encode_validate() {
                Err(Error::ValidationErrors(v)) => {
                    let errors = v.errors();
                    // the offending item's own errors, plus exactly one annotation
                    // under a key that does not vary with the index
                    assert_eq!(errors.len(), 2);
                    assert!(errors.contains_key("word"));
                    match errors.get("at index") {
                        Some(ValidationErrorsKind::Field(annotations)) => {
                            assert_eq!(annotations.len(), 1);
                            annotations[0].params["index"].clone()
                        }
                        other => panic!("expected a field annotation, got {:?}", other),
                    }
                }
                other => panic!("expected ValidationErrors, got {:?}", other.err()),
            };
        assert_eq!(
            annotated_index(vec![good.clone(), bad.clone()]),
            json!(1usize)
        );
        assert_eq!(
            annotated_index(vec![bad.clone(), good.clone()]),
            json!(0usize)
        );
        assert_eq!(
            annotated_index(vec![good.clone(), good.clone(), bad.clone()]),
            json!(2usize)
        );
    }

    #[test]
    fn test_try_from_bytes_json_fallback() -> Result<(), Error> {
        let json_bytes =
            br#"[{"word":"stack","description":"a description.","operandParserOffset":16}]"#
                .to_vec();
        let expected = AuthoringMeta(vec![AuthoringMetaItem {
            word: "stack".to_string(),
            operand_parser_offset: 16u8,
            description: "a description.".to_string(),
        }]);
        // json bytes resolve through the serde_json fallback arm
        let from_vec = AuthoringMeta::try_from(json_bytes.clone())?;
        assert_eq!(from_vec, expected);
        let from_slice = AuthoringMeta::try_from(json_bytes.as_slice())?;
        assert_eq!(from_slice, expected);
        // abi encoded bytes resolve through the abi_decode arm
        let encoded = expected.abi_encode()?;
        assert_eq!(AuthoringMeta::try_from(encoded)?, expected);
        Ok(())
    }

    #[test]
    fn test_try_from_meta_item_unpacks_content_encoding() -> Result<(), Error> {
        use crate::meta::{
            ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item,
        };
        let expected = AuthoringMeta(vec![AuthoringMetaItem {
            word: "stack".to_string(),
            operand_parser_offset: 16u8,
            description: "a description.".to_string(),
        }]);
        let encoded = expected.abi_encode_validate()?;
        let deflated = ContentEncoding::Deflate.encode(&encoded);
        assert_ne!(deflated, encoded);
        let item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(deflated),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::None,
            schema: None,
        };
        // TryFrom must unpack (inflate) the payload before decoding
        assert_eq!(AuthoringMeta::try_from(item)?, expected);
        Ok(())
    }
    #[test]
    fn test_description_rejects_unprintable_chars() {
        use validator::Validate;
        // printable ASCII description passes REGEX_RAIN_STRING
        let item = AuthoringMetaItem {
            word: "stack".to_string(),
            operand_parser_offset: 0u8,
            description: "All printable ASCII is fine.".to_string(),
        };
        assert!(item.validate().is_ok());

        // a non-printable control character is rejected, and the rejection
        // surfaces through abi_encode_validate as ValidationErrors
        let item = AuthoringMetaItem {
            word: "stack".to_string(),
            operand_parser_offset: 0u8,
            description: "bell \u{7} is not printable".to_string(),
        };
        assert!(item.validate().is_err());
        assert!(matches!(
            item.abi_encode_validate(),
            Err(Error::ValidationErrors(_))
        ));
    }
}
