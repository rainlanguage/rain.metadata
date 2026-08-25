use serde::{
    Serialize, Deserialize, Deserializer,
    de::{
        MapAccess, SeqAccess, Visitor,
        value::{MapAccessDeserializer, SeqAccessDeserializer},
    },
};
use validator::{Validate, ValidationError, ValidationErrors};
use super::super::{
    super::{RainMetaDocumentV1Item, Error},
    common::v1::{RainSymbol, RainString, Description},
};

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;

pub type Computation = RainString;

/// Operands in the standard interpreter are `u16` values.
#[derive(Validate, Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(transparent)]
#[repr(transparent)]
pub struct Operand {
    pub value: u16,
}

/// BitIntegers are zero indexed.
pub const MIN_BIT_INTEGER: usize = 0;
/// BitIntegers cannot range past the size of an Operand in bits, zero indexed.
pub const MAX_BIT_INTEGER: usize = (std::mem::size_of::<Operand>() * 8) - 1;

/// # BitInteger
/// Counts or ranges bits in an operand. Ranges are 0 indexed.
#[derive(Validate, Debug, Clone, Serialize, Deserialize, PartialOrd, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(transparent)]
pub struct BitInteger {
    #[validate(range(min = "MIN_BIT_INTEGER", max = "MAX_BIT_INTEGER"))]
    pub value: u8,
}

/// # BitIntegerRange
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct BitIntegerRange(BitInteger, BitInteger);

impl Validate for BitIntegerRange {
    fn validate(&self) -> Result<(), ValidationErrors> {
        ValidationErrors::merge_all(
            if self.0 <= self.1 {
                Ok(())
            } else {
                let mut errors = ValidationErrors::new();
                errors.add("range", ValidationError::new("Bad bit integer range.\n"));
                Err(errors)
            },
            "range",
            vec![self.0.validate(), self.1.validate()],
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum OperandArgRange {
    Exact(Operand),
    Range(Operand, Operand),
}

impl Validate for OperandArgRange {
    fn validate(&self) -> Result<(), ValidationErrors> {
        ValidationErrors::merge_all(
            match self {
                OperandArgRange::Exact(_) => Ok(()),
                OperandArgRange::Range(min, max) => {
                    if min <= max {
                        Ok(())
                    } else {
                        let mut errors = ValidationErrors::new();
                        errors.add("range", ValidationError::new("Bad operand arg range.\n"));
                        Err(errors)
                    }
                }
            },
            "range",
            match self {
                OperandArgRange::Exact(exact) => vec![exact.validate()],
                OperandArgRange::Range(min, max) => vec![min.validate(), max.validate()],
            },
        )
    }
}

/// # OpMeta.
/// Opcodes metadata used by Rainlang.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct OpMeta(pub Vec<OpMetaItem>);

impl<'de> Deserialize<'de> for OpMeta {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OpMetaVisitor;

        impl<'de> Visitor<'de> for OpMetaVisitor {
            type Value = OpMeta;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an opcode object or an array of opcode objects")
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                Ok(OpMeta(vec![OpMetaItem::deserialize(
                    MapAccessDeserializer::new(map),
                )?]))
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                Ok(OpMeta(Vec::<OpMetaItem>::deserialize(
                    SeqAccessDeserializer::new(seq),
                )?))
            }
        }

        deserializer.deserialize_any(OpMetaVisitor)
    }
}

impl Validate for OpMeta {
    fn validate(&self) -> Result<(), ValidationErrors> {
        for item in &self.0 {
            item.validate()?;
        }
        Ok(())
    }
}

/// # OpMetaItem.
/// Metadata of a single opcode.
#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct OpMetaItem {
    /// # Name
    /// Primary word used to identify the opcode.
    #[validate]
    pub name: RainSymbol,
    /// # Description
    /// Brief description of the opcode.
    #[serde(default)]
    #[validate]
    pub desc: Description,
    /// # Operand
    /// Data required to calculate and format the operand.
    #[serde(default)]
    #[validate]
    pub operand: Vec<OperandArg>,
    /// # Inputs
    /// Data required to specify the inputs of the opcode. 0 for opcodes with no
    /// input, for opcodes with constant number of inputs, the length of
    /// "parameters" array defines the number of inputs and for opcodes with
    /// dynamic number of inputs, "bits" field must be specified which determines
    /// this opcode has dynamic inputs and number of inputs will be derived from
    /// the operand bits with "computation" field applied if specified.
    #[serde(default)]
    #[validate]
    pub inputs: Vec<Input>,
    /// # Outputs
    /// Data required to specify the outputs of the opcode. An integer specifies
    /// the number of outputs for opcodes with constants number of outputs and
    /// for opcodes with dynamic outputs the "bits" field will determine the
    /// number of outputs with "computation" field applied if specified.
    #[serde(default)]
    #[validate]
    pub outputs: Vec<Output>,
    /// # Aliases
    /// Other words used to reference the opcode.
    #[serde(default)]
    #[validate]
    pub aliases: Vec<RainSymbol>,
}

impl TryFrom<Vec<u8>> for OpMeta {
    type Error = Error;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        match serde_json::from_slice::<Self>(&value) {
            Ok(t) => Ok(t.validate().map(|_| t)?),
            Err(e) => Err(e)?,
        }
    }
}

impl TryFrom<RainMetaDocumentV1Item> for OpMeta {
    type Error = Error;
    fn try_from(value: RainMetaDocumentV1Item) -> Result<Self, Self::Error> {
        Self::try_from(value.unpack()?)
    }
}

/// # Input
/// Data type of opcode's inputs that determines the number of inputs an opcode
/// has and provide information about them.
#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct Input {
    /// # Parameters
    /// List of InputParameters, may be empty.
    #[serde(default)]
    #[validate]
    pub parameters: Vec<InputParameter>,
    /// # Inputs-Allocated Operand Bits
    /// Specifies bits of the operand allocated for number of inputs. Determines
    /// the number of inputs for a computed opcode inputs. Required only for
    /// computed (non-constant) inputs.
    #[serde(default)]
    #[validate]
    pub bits: Option<BitIntegerRange>,
    /// # Inputs-Allocated Operand Bits Computation
    /// Specifies any arithmetical operation that will be applied to the value of
    /// the extracted operand bits. The "bits" keyword is reserved for accessing
    /// the extracted value, example: "(bits + 1) * 2". Required only for
    /// computed (non-constant) inputs.
    #[serde(default)]
    #[validate]
    pub computation: Option<Computation>,
}

/// # Input Parameter
/// Data type for opcode's inputs parameters, the length determines the number of
/// inputs for constant (non-computed) inputs.
#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct InputParameter {
    /// # Input Parameter Name
    /// Name of the input parameter.
    #[validate]
    pub name: RainSymbol,
    /// # Input Parameter Description
    /// Description of the input parameter.
    #[serde(default)]
    #[validate]
    pub desc: Description,
    /// # Parameter Spread
    /// Specifies if an argument is dynamic in length, default is false, so only
    /// needs to be defined if an argument is spread.
    #[serde(default)]
    pub spread: bool,
}

/// # Output
/// Data type of opcode's outputs that determines the number of outputs an opcode
/// has and provide information about them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum Output {
    Exact(Operand),
    Computed(BitIntegerRange, Computation),
}

impl Validate for Output {
    fn validate(&self) -> Result<(), ValidationErrors> {
        ValidationErrors::merge_all(
            Ok(()),
            "output",
            match self {
                Output::Exact(operand) => vec![operand.validate()],
                Output::Computed(range, computation) => {
                    vec![range.validate(), computation.validate()]
                }
            },
        )
    }
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct OperandArg {
    /// # Allocated Operand Bits
    /// Specifies the bits to allocate to this operand argument.
    #[validate]
    pub bits: BitIntegerRange,
    /// # Operand Argument Name
    /// Name of the operand argument. Argument with the name of "inputs" is
    /// reserved so that it wont be be typed inside <> and its value needed to
    /// construct the operand will be the number of items inside the opcode's
    /// parens (computation will apply to this value if provided).
    #[validate]
    pub name: RainSymbol,
    /// # Operand Argument Description
    /// Description of the operand argument.
    #[serde(default)]
    #[validate]
    pub desc: Description,
    /// # Allocated Operand Bits Computation
    /// Specifies any arithmetical operation that needs to be applied to the
    /// value of this operand argument. It will apply to the value before it be
    /// validated by the provided range. The "arg" keyword is reserved for
    /// accessing the value of this operand argument, example: "(arg + 1) * 2".
    #[serde(default)]
    #[validate]
    pub computation: Option<Computation>,
    /// # Operand Argument Range
    /// Determines the valid range of the operand argument after computation
    /// applied. For example an operand argument can be any value between range
    /// of 1 - 10: \[\[1, 10\]\] or an operand argument can only be certain exact
    /// values: \[\[2\], \[3\], \[9\]\], meaning it can only be 2 or 3 or 9.
    #[serde(default)]
    #[validate]
    pub valid_range: Option<Vec<OperandArgRange>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{ContentEncoding, ContentLanguage, ContentType, KnownMagic};

    #[test]
    fn test_bit_integer_bounds() {
        // Operand is u16, so operand bits are zero-indexed 0..=15.
        assert_eq!(MIN_BIT_INTEGER, 0);
        assert_eq!(MAX_BIT_INTEGER, 15);
        assert!(BitInteger { value: 0 }.validate().is_ok());
        assert!(BitInteger { value: 15 }.validate().is_ok());
        assert!(BitInteger { value: 16 }.validate().is_err());
        assert!(BitInteger { value: 255 }.validate().is_err());
    }

    #[test]
    fn test_bit_integer_range_order() {
        let range = |a, b| BitIntegerRange(BitInteger { value: a }, BitInteger { value: b });
        assert!(range(1, 2).validate().is_ok());
        assert!(range(3, 3).validate().is_ok());
        assert!(range(2, 1).validate().is_err());
    }

    // NOTE: no test asserts that BitIntegerRange rejects in-order ranges
    // with out-of-bounds ends (e.g. (0, 16)): the hand-rolled
    // ValidationErrors::merge_all call currently drops the per-end
    // BitInteger results, so (0, 16) validates Ok today. Pinning either
    // outcome is wrong while that is unresolved — see
    // rainlanguage/rain.metadata#173.

    #[test]
    fn test_operand_arg_range_exact_is_valid() {
        assert!(OperandArgRange::Exact(Operand { value: 0 })
            .validate()
            .is_ok());
        assert!(OperandArgRange::Exact(Operand { value: u16::MAX })
            .validate()
            .is_ok());
    }

    #[test]
    fn test_operand_arg_range_min_max() {
        let range = |a, b| OperandArgRange::Range(Operand { value: a }, Operand { value: b });
        // Equal bounds are a valid (degenerate) range.
        assert!(range(5, 5).validate().is_ok());
        assert!(range(2, 9).validate().is_ok());
        assert!(range(6, 5).validate().is_err());
    }

    #[test]
    fn test_output_validation_accepts_well_formed() {
        let good_range = || BitIntegerRange(BitInteger { value: 0 }, BitInteger { value: 3 });
        let computation = |s: &str| RainString {
            value: s.to_string(),
        };
        assert!(Output::Exact(Operand { value: 2 }).validate().is_ok());
        assert!(Output::Computed(good_range(), computation("bits * 2"))
            .validate()
            .is_ok());
        // NOTE: no rejection cases are asserted here. Output::validate
        // currently drops its sub-validation results (merge_all misuse:
        // the parent is a literal Ok and child errors are not Struct-kind
        // under the "output" key), so a Computed output with an
        // out-of-bounds range or non-ASCII computation validates Ok today.
        // Pinning either outcome is wrong while that is unresolved — see
        // rainlanguage/rain.metadata#173.
    }

    #[test]
    fn test_opmeta_minimal_json_defaults() {
        // Only `name` is required; everything else defaults.
        let meta = OpMeta::try_from(br#"{"name":"add"}"#.to_vec()).unwrap();
        let op = &meta.0[0];
        assert_eq!(op.name.value, "add");
        assert_eq!(op.desc.value, "");
        assert!(op.operand.is_empty());
        assert!(op.inputs.is_empty());
        assert!(op.outputs.is_empty());
        assert!(op.aliases.is_empty());
    }

    /// An op meta v1 document is an array of opcodes: every entry is kept,
    /// in order.
    #[test]
    fn test_opmeta_document_is_an_array_of_opcodes() {
        let meta = OpMeta::try_from(br#"[{"name":"add"},{"name":"sub"},{"name":"mul"}]"#.to_vec())
            .unwrap();
        assert_eq!(
            meta.0
                .iter()
                .map(|op| op.name.value.as_str())
                .collect::<Vec<_>>(),
            vec!["add", "sub", "mul"]
        );
    }

    /// A bare opcode object is a document of one opcode.
    #[test]
    fn test_opmeta_document_lifts_single_object() {
        let meta = OpMeta::try_from(br#"{"name":"add"}"#.to_vec()).unwrap();
        assert_eq!(meta.0.len(), 1);
    }

    /// An empty document holds no opcodes rather than erroring.
    #[test]
    fn test_opmeta_document_accepts_empty_array() {
        assert!(OpMeta::try_from(b"[]".to_vec()).unwrap().0.is_empty());
    }

    /// Every entry of the array is validated, not just the first.
    #[test]
    fn test_opmeta_document_validates_every_opcode() {
        assert!(OpMeta::try_from(br#"[{"name":"add"},{"name":"ok-too"}]"#.to_vec()).is_ok());
        assert!(OpMeta::try_from(br#"[{"name":"add"},{"name":"NOT-A-SYMBOL"}]"#.to_vec()).is_err());
    }

    /// Json that is neither an opcode object nor an array of them is not a
    /// document.
    #[test]
    fn test_opmeta_document_rejects_other_json() {
        assert!(OpMeta::try_from(br#""add""#.to_vec()).is_err());
        assert!(OpMeta::try_from(b"5".to_vec()).is_err());
        assert!(OpMeta::try_from(b"null".to_vec()).is_err());
    }

    /// A document serializes back as an array whichever shape it was read
    /// from.
    #[test]
    fn test_opmeta_document_serializes_as_array() {
        let meta = OpMeta(vec![]);
        assert_eq!(serde_json::to_string(&meta).unwrap(), "[]");
        let meta = OpMeta::try_from(br#"{"name":"add"}"#.to_vec()).unwrap();
        assert!(serde_json::to_string(&meta).unwrap().starts_with("[{"));
    }

    #[test]
    fn test_opmeta_try_from_validates() {
        // Parses as JSON but must fail RainSymbol validation on `name`.
        assert!(OpMeta::try_from(br#"{"name":"NOT-A-SYMBOL"}"#.to_vec()).is_err());
    }

    #[test]
    fn test_opmeta_aliases_validated() {
        assert!(OpMeta::try_from(br#"{"name":"add","aliases":["ok-alias"]}"#.to_vec()).is_ok());
        assert!(OpMeta::try_from(br#"{"name":"add","aliases":["BAD"]}"#.to_vec()).is_err());
    }

    #[test]
    fn test_opmeta_input_bits_validated() {
        assert!(OpMeta::try_from(br#"{"name":"add","inputs":[{"bits":[0,15]}]}"#.to_vec()).is_ok());
        // An out-of-order range fails BitIntegerRange's own order check,
        // which must propagate through Input.bits' nested #[validate].
        // (The per-end bounds check, e.g. bits [0,16], is currently
        // dropped by merge_all misuse — see rainlanguage/rain.metadata#173 —
        // so only the order violation is pinned here.)
        assert!(
            OpMeta::try_from(br#"{"name":"add","inputs":[{"bits":[16,0]}]}"#.to_vec()).is_err()
        );
    }

    #[test]
    fn test_opmeta_input_computation_validated() {
        assert!(OpMeta::try_from(
            br#"{"name":"add","inputs":[{"computation":"bits + 1"}]}"#.to_vec()
        )
        .is_ok());
        assert!(OpMeta::try_from(
            "{\"name\":\"add\",\"inputs\":[{\"computation\":\"\u{2665}\"}]}"
                .as_bytes()
                .to_vec()
        )
        .is_err());
    }

    #[test]
    fn test_input_parameter_spread_defaults_false() {
        let meta = OpMeta::try_from(
            br#"{"name":"add","inputs":[{"parameters":[{"name":"lhs"}]}]}"#.to_vec(),
        )
        .unwrap();
        assert!(!meta.0[0].inputs[0].parameters[0].spread);
    }

    #[test]
    fn test_opmeta_try_from_item_unpacks_content_encoding() {
        // TryFrom<RainMetaDocumentV1Item> must unpack() (honouring
        // content_encoding), not read the raw payload.
        let json = br#"{"name":"add"}"#.to_vec();
        let item = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(ContentEncoding::Deflate.encode(&json)),
            magic: KnownMagic::OpMetaV1,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::None,
            schema: None,
        };
        let meta = OpMeta::try_from(item).unwrap();
        assert_eq!(meta.0[0].name.value, "add");
    }
}
