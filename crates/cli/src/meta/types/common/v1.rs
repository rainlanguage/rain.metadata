use regex::Regex;
use once_cell::sync::Lazy;

/// Valid symbols in Rainlang are alpha prefixed alphanumeric kebab case.
pub static REGEX_RAIN_SYMBOL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z][0-9a-z]*(-[0-9a-z]+)*$").unwrap());

/// Strings in Rain are limited to printable ASCII chars and ASCII whitespace.
/// `\s` is Unicode `White_Space` in the regex crate, so the class is spelled out.
pub static REGEX_RAIN_STRING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[\t\n\x0B\x0C\r !-~]*$").unwrap());

#[cfg(test)]
mod test {
    use super::REGEX_RAIN_SYMBOL;
    use super::REGEX_RAIN_STRING;

    #[test]
    fn test_rain_symbol_validate() {
        // valids
        for i in ["a", "a0", "a-a", "a-0", "a-b-c"] {
            assert!(
                REGEX_RAIN_SYMBOL.is_match(i),
                "String '{}' considered invalid.",
                i
            );
        }

        // invalids
        for i in [
            "", "♥", "-", " ", "A", "A0", "a ", "0", "_", "0a", "0A", "\n", "\t", "\r", "aA", "a-",
            "a--b", "a-A", "a_b", "a-b_c", "a-b c", "a\na",
        ] {
            assert!(
                !REGEX_RAIN_SYMBOL.is_match(i),
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
            "a ", "0", "_", "0a", "0A", "`", "```", "\n", "\t", "\r", ":", "\u{b}", "\u{c}", "!",
            "~",
        ] {
            assert!(
                REGEX_RAIN_STRING.is_match(i),
                "String '{}' considered invalid.",
                i
            );
        }

        // invalids
        for i in [
            "♥", "∴", "\u{a0}", "\u{85}", "\u{2028}", "\u{2029}", "\u{2003}", "\u{3000}", "\u{7f}",
        ] {
            assert!(
                !REGEX_RAIN_STRING.is_match(i),
                "String '{}' considered valid.",
                i
            );
        }
    }
}
