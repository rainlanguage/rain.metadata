use validator::Validate;
use serde::{Serialize, Deserialize};
use super::super::{
    super::{RainMetaDocumentV1Item, Error},
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
    /// Name of the contract corresponding to `contractName` feild in the abi.
    #[validate]
    pub abi_name: SolidityIdentifier,
    /// Name of the caller corresponding to `contractName` feild in the abi.
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
    #[validate(length(max = "u8::MAX"))]
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
