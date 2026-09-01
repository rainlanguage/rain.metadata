use validator::Validate;
use serde::{Serialize, Deserialize};
use super::super::{
    super::{KnownMagic, RainMetaDocumentV1Item, Error},
    common::v1::{RainTitle, RainSymbol, RainString, Description, SolidityIdentifier},
};

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;

type AbiPath = RainString;

/// InterpreterCaller metadata used by Rainlang.
/// Supports `IInterpreterCallerV2` Solidity contracts.
/// Required info about a contract that receives expression in at least one of
/// its methods.
#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct InterpreterCallerMeta {
    #[validate]
    pub name: RainTitle,
    /// Name of the contract corresponding to `contractName` field in the abi.
    #[validate]
    pub abi_name: SolidityIdentifier,
    /// Description of the caller.
    #[serde(default)]
    #[validate]
    pub desc: Description,
    /// Determines the repository source
    #[serde(default)]
    #[validate]
    pub source: Description,
    /// Alias of the caller used by Rainlang.
    #[serde(default)]
    #[validate]
    pub alias: Option<RainSymbol>,
    ///  Methods of the contract that receive at least one expression
    /// (EvaluableConfig) from arguments.
    #[validate(length(min = 1))]
    #[validate]
    pub methods: Vec<Method>,
}

impl TryFrom<Vec<u8>> for InterpreterCallerMeta {
    type Error = Error;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        match serde_json::from_slice::<Self>(&value) {
            Ok(t) => Ok(t.validate().map(|_| t)?),
            Err(e) => Err(e)?,
        }
    }
}

impl TryFrom<&[u8]> for InterpreterCallerMeta {
    type Error = Error;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match serde_json::from_slice::<Self>(value) {
            Ok(t) => Ok(t.validate().map(|_| t)?),
            Err(e) => Err(e)?,
        }
    }
}

impl TryFrom<RainMetaDocumentV1Item> for InterpreterCallerMeta {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        if value.magic != KnownMagic::InterpreterCallerMetaV1 {
            return Err(Error::InvalidMetaMagic(
                KnownMagic::InterpreterCallerMetaV1,
                value.magic,
            ));
        }
        Self::try_from(value.unpack()?)
    }
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Method {
    #[validate]
    pub name: RainTitle,
    #[validate]
    pub abi_name: SolidityIdentifier,
    #[serde(default)]
    #[validate]
    pub desc: Description,
    #[validate(length(min = 1))]
    #[validate]
    pub inputs: Vec<MethodInput>,
    #[validate(length(min = 1))]
    #[validate]
    pub expressions: Vec<Expression>,
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct MethodInput {
    #[validate]
    pub name: RainTitle,
    #[validate]
    pub abi_name: SolidityIdentifier,
    #[serde(default)]
    #[validate]
    pub desc: Description,
    #[validate]
    pub path: AbiPath,
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Expression {
    #[validate]
    pub name: RainTitle,
    #[validate]
    pub abi_name: SolidityIdentifier,
    #[serde(default)]
    #[validate]
    pub desc: Description,
    #[validate]
    pub path: AbiPath,
    #[serde(default)]
    pub signed_context: bool,
    #[serde(default)]
    pub caller_context: bool,
    #[serde(default)]
    #[validate(length(max = 256))]
    #[validate]
    pub context_columns: Vec<ContextColumn>,
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextColumn {
    #[validate]
    pub name: RainTitle,
    #[serde(default)]
    #[validate]
    pub desc: Description,
    #[serde(default)]
    #[validate]
    pub alias: Option<RainSymbol>,
    #[serde(default)]
    #[validate(length(max = 256))]
    #[validate]
    pub cells: Vec<ContextCell>,
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextCell {
    #[validate]
    pub name: RainTitle,
    #[serde(default)]
    #[validate]
    pub desc: Description,
    #[serde(default)]
    #[validate]
    pub alias: Option<RainSymbol>,
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic};
    use serde_json::json;

    /// A fully-populated valid InterpreterCallerMeta JSON document.
    fn valid_json() -> serde_json::Value {
        serde_json::json!({
            "name": "Test Caller",
            "abiName": "TestCaller",
            "desc": "A caller for tests.",
            "source": "https://github.com/rainlanguage/rain.metadata",
            "alias": "test-caller",
            "methods": [{
                "name": "Add Order",
                "abiName": "addOrder",
                "desc": "Adds an order.",
                "inputs": [{
                    "name": "Order",
                    "abiName": "order",
                    "desc": "The order.",
                    "path": "[0]"
                }],
                "expressions": [{
                    "name": "Calculate",
                    "abiName": "calculateOrder",
                    "desc": "Calculates.",
                    "path": "[0].evaluableConfig",
                    "signedContext": true,
                    "callerContext": true,
                    "contextColumns": [{
                        "name": "Base",
                        "desc": "Base column.",
                        "alias": "base",
                        "cells": [{
                            "name": "Sender",
                            "desc": "The sender.",
                            "alias": "sender"
                        }]
                    }]
                }]
            }]
        })
    }

    fn base_json() -> serde_json::Value {
        json!({
            "name": "Test Caller",
            "abiName": "TestCaller",
            "methods": [{
                "name": "Add Order",
                "abiName": "addOrder",
                "inputs": [{
                    "name": "Config",
                    "abiName": "config",
                    "path": "[7].inputs[0]"
                }],
                "expressions": [{
                    "name": "Calculate",
                    "abiName": "calculateIO",
                    "path": "[7].expressions[0]",
                    "contextColumns": [{
                        "name": "Base",
                        "cells": [{ "name": "Cell" }]
                    }]
                }]
            }]
        })
    }

    fn parse(v: &serde_json::Value) -> Result<InterpreterCallerMeta, Error> {
        InterpreterCallerMeta::try_from(serde_json::to_vec(v).unwrap())
    }

    /// Omitted optional fields parse and take their documented defaults:
    /// empty desc/source, no alias, both context flags false, no context
    /// columns, no cells.
    #[test]
    fn test_serde_defaults() {
        let v = serde_json::json!({
            "name": "Test Caller",
            "abiName": "TestCaller",
            "methods": [{
                "name": "Add Order",
                "abiName": "addOrder",
                "inputs": [{
                    "name": "Order",
                    "abiName": "order",
                    "path": "[0]"
                }],
                "expressions": [
                    {
                        "name": "Calculate",
                        "abiName": "calculateOrder",
                        "path": "[0].evaluableConfig",
                        "contextColumns": [{
                            "name": "Base"
                        }]
                    },
                    {
                        "name": "Handle",
                        "abiName": "handleOrder",
                        "path": "[1].evaluableConfig"
                    }
                ]
            }]
        });
        let parsed = parse(&v).unwrap();
        assert_eq!(parsed.desc.value, "");
        assert_eq!(parsed.source.value, "");
        assert!(parsed.alias.is_none());
        let method = &parsed.methods[0];
        assert_eq!(method.desc.value, "");
        assert_eq!(method.inputs[0].desc.value, "");
        let expression = &method.expressions[0];
        assert_eq!(expression.desc.value, "");
        assert!(!expression.signed_context);
        assert!(!expression.caller_context);
        let column = &expression.context_columns[0];
        assert_eq!(column.desc.value, "");
        assert!(column.alias.is_none());
        assert!(column.cells.is_empty());
        assert!(method.expressions[1].context_columns.is_empty());
    }

    /// Invalid nested values fail validation at every depth of the
    /// #[validate] chain: methods -> inputs and
    /// methods -> expressions -> context_columns -> cells.
    #[test]
    fn test_nested_validate_chain() {
        // Baseline sanity: the valid document parses and validates.
        assert!(parse(&valid_json()).is_ok());

        // A RainTitle must not begin with a space: " x" is invalid at
        // any depth.
        for pointer in [
            "/methods/0/name",
            "/methods/0/inputs/0/name",
            "/methods/0/expressions/0/name",
            "/methods/0/expressions/0/contextColumns/0/name",
            "/methods/0/expressions/0/contextColumns/0/cells/0/name",
        ] {
            let mut v = valid_json();
            *v.pointer_mut(pointer).unwrap() = serde_json::Value::String(" x".to_string());
            let err = parse(&v).unwrap_err();
            assert!(
                matches!(err, Error::ValidationErrors(_)),
                "expected validation error for {pointer}, got {err:?}"
            );
        }
    }

    /// methods, inputs and expressions require at least one element; both
    /// context matrix axes allow 256 elements, the addressable range of the
    /// byte each index occupies in the `context` operand - `LibOpContext`
    /// masks the column to the low byte and the cell to the second, so both
    /// run 0..=255.
    #[test]
    fn test_length_constraints() {
        let mut v = valid_json();
        *v.pointer_mut("/methods").unwrap() = serde_json::json!([]);
        assert!(matches!(parse(&v).unwrap_err(), Error::ValidationErrors(_)));

        let mut v = valid_json();
        *v.pointer_mut("/methods/0/inputs").unwrap() = serde_json::json!([]);
        assert!(matches!(parse(&v).unwrap_err(), Error::ValidationErrors(_)));

        let mut v = valid_json();
        *v.pointer_mut("/methods/0/expressions").unwrap() = serde_json::json!([]);
        assert!(matches!(parse(&v).unwrap_err(), Error::ValidationErrors(_)));

        let column = serde_json::json!({ "name": "Col" });
        let mut v = valid_json();
        *v.pointer_mut("/methods/0/expressions/0/contextColumns")
            .unwrap() = serde_json::Value::Array(vec![column.clone(); 256]);
        assert!(parse(&v).is_ok());

        let mut v = valid_json();
        *v.pointer_mut("/methods/0/expressions/0/contextColumns")
            .unwrap() = serde_json::Value::Array(vec![column; 257]);
        assert!(matches!(parse(&v).unwrap_err(), Error::ValidationErrors(_)));

        let cell = serde_json::json!({ "name": "Cell" });
        let mut v = valid_json();
        *v.pointer_mut("/methods/0/expressions/0/contextColumns/0/cells")
            .unwrap() = serde_json::Value::Array(vec![cell.clone(); 256]);
        assert!(parse(&v).is_ok());

        let mut v = valid_json();
        *v.pointer_mut("/methods/0/expressions/0/contextColumns/0/cells")
            .unwrap() = serde_json::Value::Array(vec![cell; 257]);
        assert!(matches!(parse(&v).unwrap_err(), Error::ValidationErrors(_)));
    }

    /// TryFrom<Vec<u8>> and TryFrom<&[u8]> validate after parsing:
    /// syntactically-valid JSON with semantically-invalid values errors,
    /// and valid documents round-trip with their values intact.
    #[test]
    fn test_try_from_bytes_validates() {
        let mut invalid = valid_json();
        *invalid.pointer_mut("/name").unwrap() = serde_json::Value::String(" x".to_string());
        let bytes = serde_json::to_vec(&invalid).unwrap();

        let err = InterpreterCallerMeta::try_from(bytes.clone()).unwrap_err();
        assert!(matches!(err, Error::ValidationErrors(_)));
        let err = InterpreterCallerMeta::try_from(bytes.as_slice()).unwrap_err();
        assert!(matches!(err, Error::ValidationErrors(_)));

        let valid_bytes = serde_json::to_vec(&valid_json()).unwrap();
        let ok = InterpreterCallerMeta::try_from(valid_bytes.clone()).unwrap();
        assert_eq!(ok.name.value, "Test Caller");
        assert_eq!(ok.abi_name.value, "TestCaller");
        let ok = InterpreterCallerMeta::try_from(valid_bytes.as_slice()).unwrap();
        assert_eq!(ok.methods[0].abi_name.value, "addOrder");
    }

    /// TryFrom<RainMetaDocumentV1Item> unpacks the payload per the item's
    /// content encoding before parsing.
    #[test]
    fn test_try_from_meta_item_unpacks_encoding() {
        let json_bytes = serde_json::to_vec(&valid_json()).unwrap();
        let item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(ContentEncoding::Deflate.encode(&json_bytes)),
            magic: KnownMagic::InterpreterCallerMetaV1,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::En,
            schema: None,
        };
        let parsed = InterpreterCallerMeta::try_from(item).unwrap();
        assert_eq!(parsed.name.value, "Test Caller");
        assert_eq!(parsed.methods.len(), 1);
    }

    /// The magic is the item's type discriminator, so a payload that parses
    /// as InterpreterCallerMeta is still not an InterpreterCallerMeta item
    /// under any other magic.
    #[test]
    fn test_try_from_meta_item_rejects_wrong_magic() {
        for magic in [
            KnownMagic::SolidityAbiV2,
            KnownMagic::OpMetaV1,
            KnownMagic::RainMetaDocumentV1,
        ] {
            let item = RainMetaDocumentV1Item {
                payload: serde_bytes::ByteBuf::from(serde_json::to_vec(&valid_json()).unwrap()),
                magic,
                content_type: ContentType::Json,
                content_encoding: ContentEncoding::None,
                content_language: ContentLanguage::En,
                schema: None,
            };
            match InterpreterCallerMeta::try_from(item).unwrap_err() {
                Error::InvalidMetaMagic(expected, actual) => {
                    assert_eq!(expected, KnownMagic::InterpreterCallerMetaV1);
                    assert_eq!(actual, magic);
                }
                other => panic!("Expected InvalidMetaMagic for {:?}, got {:?}", magic, other),
            }
        }
    }

    /// Unknown fields are rejected.
    #[test]
    fn test_deny_unknown_fields() {
        let mut v = valid_json();
        v.as_object_mut()
            .unwrap()
            .insert("unknownField".to_string(), serde_json::json!(1));
        let err = parse(&v).unwrap_err();
        assert!(matches!(err, Error::SerdeJsonError(_)));
    }

    /// Field names are camelCase on the wire; the snake_case spelling is
    /// an unknown field.
    #[test]
    fn test_camel_case_wire_format() {
        assert!(parse(&valid_json()).is_ok());

        let mut v = valid_json();
        let obj = v.as_object_mut().unwrap();
        let abi = obj.remove("abiName").unwrap();
        obj.insert("abi_name".to_string(), abi);
        assert!(parse(&v).is_err());
    }

    fn try_parse(value: &serde_json::Value) -> Result<InterpreterCallerMeta, Error> {
        InterpreterCallerMeta::try_from(serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn test_base_fixture_is_valid() {
        let meta = try_parse(&base_json()).unwrap();
        assert_eq!(meta.name.value, "Test Caller");
        assert_eq!(meta.abi_name.value, "TestCaller");
        assert_eq!(meta.methods.len(), 1);
        assert_eq!(meta.methods[0].inputs.len(), 1);
        assert_eq!(meta.methods[0].expressions[0].context_columns.len(), 1);
    }

    #[test]
    fn test_deny_unknown_fields_top_level() {
        let mut v = base_json();
        v["unknownField"] = json!(1);
        assert!(matches!(try_parse(&v), Err(Error::SerdeJsonError(_))));
    }

    #[test]
    fn test_deny_unknown_fields_method() {
        let mut v = base_json();
        v["methods"][0]["unknownField"] = json!(1);
        assert!(matches!(try_parse(&v), Err(Error::SerdeJsonError(_))));
    }

    #[test]
    fn test_deny_unknown_fields_method_input() {
        let mut v = base_json();
        v["methods"][0]["inputs"][0]["unknownField"] = json!(1);
        assert!(matches!(try_parse(&v), Err(Error::SerdeJsonError(_))));
    }

    #[test]
    fn test_deny_unknown_fields_expression() {
        let mut v = base_json();
        v["methods"][0]["expressions"][0]["unknownField"] = json!(1);
        assert!(matches!(try_parse(&v), Err(Error::SerdeJsonError(_))));
    }

    #[test]
    fn test_deny_unknown_fields_context_column() {
        let mut v = base_json();
        v["methods"][0]["expressions"][0]["contextColumns"][0]["unknownField"] = json!(1);
        assert!(matches!(try_parse(&v), Err(Error::SerdeJsonError(_))));
    }

    #[test]
    fn test_deny_unknown_fields_context_cell() {
        let mut v = base_json();
        v["methods"][0]["expressions"][0]["contextColumns"][0]["cells"][0]["unknownField"] =
            json!(1);
        assert!(matches!(try_parse(&v), Err(Error::SerdeJsonError(_))));
    }

    #[test]
    fn test_methods_min_length_one() {
        let mut v = base_json();
        v["methods"] = json!([]);
        assert!(matches!(try_parse(&v), Err(Error::ValidationErrors(_))));
    }

    #[test]
    fn test_method_inputs_min_length_one() {
        let mut v = base_json();
        v["methods"][0]["inputs"] = json!([]);
        assert!(matches!(try_parse(&v), Err(Error::ValidationErrors(_))));
    }

    #[test]
    fn test_context_columns_max_256() {
        let column = json!({ "name": "Base" });
        let mut v = base_json();
        v["methods"][0]["expressions"][0]["contextColumns"] =
            serde_json::Value::Array(vec![column.clone(); 256]);
        assert!(try_parse(&v).is_ok());
        v["methods"][0]["expressions"][0]["contextColumns"] =
            serde_json::Value::Array(vec![column; 257]);
        assert!(matches!(try_parse(&v), Err(Error::ValidationErrors(_))));
    }
}
