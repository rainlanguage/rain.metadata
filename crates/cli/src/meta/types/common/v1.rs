use regex::Regex;
use validator::Validate;
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;

/// Valid symbols in Rainlang are alpha prefixed alphanumeric kebab case.
pub static REGEX_RAIN_SYMBOL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z][0-9a-z-]*$").unwrap());

/// An identifier in solidity has to start with a letter, a dollar-sign or an
/// sunderscore and may additionally contain numbers after the first symbol.
///
/// <https://docs.soliditylang.org/en/latest/grammar.html#a4.SolidityLexer.Identifier>
pub static REGEX_SOLIDITY_IDENTIFIER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z$_][a-zA-Z0-9$_]*$").unwrap());

/// Strings in Rain are limited to printable ASCII chars and whitespace.
pub static REGEX_RAIN_STRING: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\s!-~]*$").unwrap());

/// Titles in Rain are limited to printable ASCII chars and the space character.
/// The title MUST NOT begin or end with a space.
pub static REGEX_RAIN_TITLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[!-~]([ -~]*[!-~]|[!-~]*)$").unwrap());

/// keccak256 hash pattern
pub static HASH_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^0x[a-fA-F0-9]{64}$").unwrap());

/// Rain symbols are a subset of kebab case.
#[derive(Validate, Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(transparent)]
#[repr(transparent)]
pub struct RainSymbol {
    #[validate(regex(
        path = "REGEX_RAIN_SYMBOL",
        message = "Must be alphanumeric lower-kebab-case beginning with a letter.\n"
    ))]
    pub value: String,
}

#[derive(Validate, Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(transparent)]
#[repr(transparent)]
pub struct RainTitle {
    #[validate(regex(
        path = "REGEX_RAIN_TITLE",
        message = "Must be alphanumeric ASCII letters and spaces.\n"
    ))]
    pub value: String,
}

#[derive(Validate, Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(transparent)]
#[repr(transparent)]
pub struct RainString {
    #[validate(regex(
        path = "REGEX_RAIN_STRING",
        message = "Must be printable ASCII characters and whitespace.\n"
    ))]
    pub value: String,
}

pub type Description = RainString;

#[derive(Validate, Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(transparent)]
#[repr(transparent)]
pub struct SolidityIdentifier {
    #[validate(regex(
        path = "REGEX_SOLIDITY_IDENTIFIER",
        message = "Must be a valid Solidity identifier.\n"
    ))]
    pub value: String,
}

#[cfg(test)]
mod test {
    use crate::meta::types::common::v1::HASH_PATTERN;

    use super::RainSymbol;
    use super::RainString;
    use super::RainTitle;
    use super::SolidityIdentifier;
    use validator::Validate;

    #[test]
    fn test_rain_symbol_validate() {
        // valids
        for i in ["a", "a-", "a-a", "a0"] {
            assert!(
                RainSymbol {
                    value: i.to_string()
                }
                .validate()
                .is_ok(),
                "String '{}' considered invalid.",
                i
            );
        }

        // invalids
        for i in [
            "", "♥", "-", " ", "A", "A0", "a ", "0", "_", "0a", "0A", "\n", "\t", "\r",
        ] {
            assert!(
                RainSymbol {
                    value: i.to_string()
                }
                .validate()
                .is_err(),
                "String '{}' considered valid.",
                i
            );
        }
    }

    #[test]
    fn test_rain_title_validate() {
        // valids
        for i in [
            "a", "a-", "a-a", "a0", "a a", "-", "A", "A0", "0", "_", "0a", "0A",
        ] {
            assert!(
                RainTitle {
                    value: i.to_string()
                }
                .validate()
                .is_ok(),
                "String '{}' considered invalid.",
                i
            );
        }

        // invalids
        for i in ["", " ", " a", "a ", "♥", "\n", "\t", "\r"] {
            assert!(
                RainTitle {
                    value: i.to_string()
                }
                .validate()
                .is_err(),
                "String '{}' considered valid.",
                i
            );
        }
    }

    #[test]
    fn test_rain_string_validate() {
        // valids
        for i in [
            "a", "aa", "aA", "aAa", "a0", "aa0", "aA0", "aA0a", "aA0a0", "", "a-", "a-a", "-", " ",
            "a ", "0", "_", "0a", "0A", "`", "```", "\n", "\t", "\r", ":",
        ] {
            assert!(
                RainString {
                    value: i.to_string()
                }
                .validate()
                .is_ok(),
                "String '{}' considered invalid.",
                i
            );
        }

        // invalids
        for i in ["♥", "∴"] {
            assert!(
                RainString {
                    value: i.to_string()
                }
                .validate()
                .is_err(),
                "String '{}' considered valid.",
                i
            );
        }
    }

    #[test]
    fn test_solidity_identifier_validate() {
        // valids
        for i in [
            "A", "AA", "A0", "Raindex", "$", "$$", "_", "__", "a", "aa", "a_", "A_", "a$", "A",
            "A$", "a0",
        ] {
            assert!(
                SolidityIdentifier {
                    value: i.to_string()
                }
                .validate()
                .is_ok(),
                "String '{}' considered invalid.",
                i
            );
        }

        // invalids
        for i in [
            "", "a-", "a-a", "♥", "-", " ", "a ", "0", "0a", "0A", "\n", "\t", "\r",
        ] {
            assert!(
                SolidityIdentifier {
                    value: i.to_string()
                }
                .validate()
                .is_err(),
                "String '{}' considered valid.",
                i
            );
        }
    }

    #[test]
    fn test_hash_pattern() {
        // valids
        for i in [
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x78fd1edb0bdb928db6015990fecafbb964b44692e2d435693062dd4efc6254dd",
            "0x78FD1EDB0BDB928DB6015990FECAFBB964B44692E2D435693062DD4EFC6254DD",
            "0x78fD1eDb0BdB928dB6015990fEcAfBb964B44692e2D435693062dD4eFc6254Dd",
        ] {
            assert!(HASH_PATTERN.is_match(i), "String '{}' considered valid.", i);
        }

        // invalids
        for i in [
            "0",
            "_",
            "0x",
            "0x1",
            "0xk",
            "0x12.",
            "0x123456789abcdef",
            "something",
            "ox1234567890abcdefABCDEF",
            "x1234567890abcdefABCDEF",
            "1234567890abcdefABCDEF",
            "0X1234567890abcdefABCDEF",
            "0x1234567890abcdefABCDEF",
            "0x1234567890abcdefABCDEFG",
            "0x78fd1edb0bdb928db6015990fecafbb964b44692e2d435693062dd4efc6254dd1",
            "0x78fd1edb0bdb928db6015990fecafbb964b44692e2d435693062dd4efc6254d",
            "0x78fd1edb0bdb928db6015990fecafbb964b44692e2d435693062dd4efc6254dd ",
            " 0x78fd1edb0bdb928db6015990fecafbb964b44692e2d435693062dd4efc6254dd",
            "0x78fd1edb0bdb928db6015990fecafbb9 64b44692e2d435693062dd4efc6254dd",
        ] {
            assert!(
                !HASH_PATTERN.is_match(i),
                "String '{}' considered invalid.",
                i
            );
        }
    }
}
