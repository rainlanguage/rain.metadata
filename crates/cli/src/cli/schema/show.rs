use clap::Parser;
use std::path::PathBuf;
use schemars::schema_for;
use crate::meta::KnownMeta;
use crate::cli::output::SupportedOutputEncoding;

#[derive(Parser)]
pub struct Show {
    /// One of a set of known JSON schemas that can be produced to match a subset
    /// of the validation performed on known metas. Additional validation beyond
    /// what can be expressed by JSON schema is performed when parsing and
    /// validating metadata.
    #[arg(value_parser = clap::value_parser!(KnownMeta))]
    schema: KnownMeta,
    /// If provided the schema will be written to the given path instead of
    /// stdout.
    #[arg(short, long)]
    output_path: Option<PathBuf>,
    /// If true the schema will be pretty printed. Defaults to false.
    #[arg(short, long)]
    pretty_print: bool,
}

pub fn show(s: Show) -> anyhow::Result<()> {
    let schema_json = match s.schema {
        KnownMeta::OpV1 => schema_for!(crate::meta::types::op::v1::OpMeta),
        KnownMeta::AuthoringMetaV1 => schema_for!(crate::meta::types::authoring::v1::AuthoringMeta),
        KnownMeta::SolidityAbiV2 => {
            schema_for!(crate::meta::types::solidity_abi::v2::SolidityAbiMeta)
        }
        KnownMeta::InterpreterCallerMetaV1 => {
            schema_for!(crate::meta::types::interpreter_caller::v1::InterpreterCallerMeta)
        }
        other => return Err(anyhow::anyhow!("Unsupported for {} meta", other)),
    };
    let schema_string = if s.pretty_print {
        serde_json::to_string_pretty(&schema_json)?
    } else {
        serde_json::to_string(&schema_json)?
    };

    crate::cli::output::output(
        &s.output_path,
        SupportedOutputEncoding::Binary,
        schema_string.as_bytes(),
    )
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    fn show_to_string(schema: KnownMeta, pretty_print: bool) -> anyhow::Result<String> {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        show(Show {
            schema,
            output_path: Some(path.clone()),
            pretty_print,
        })?;
        Ok(std::fs::read_to_string(&path).unwrap())
    }

    /// Each supported meta produces its own schema; compact output by
    /// default (no newlines).
    #[test]
    fn test_show_op_v1_schema_compact() {
        let s = show_to_string(KnownMeta::OpV1, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["title"], "OpMeta.");
        assert!(!s.contains('\n'));
    }

    /// The pretty flag pretty-prints the same schema.
    #[test]
    fn test_show_pretty_print() {
        let s = show_to_string(KnownMeta::OpV1, true).unwrap();
        assert!(s.starts_with("{\n"));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["title"], "OpMeta.");
    }

    /// All four supported arms return the schema of their own meta type.
    #[test]
    fn test_show_supported_schemas_are_distinct() {
        let op = show_to_string(KnownMeta::OpV1, false).unwrap();
        assert!(op.contains("OpMeta"));
        let authoring = show_to_string(KnownMeta::AuthoringMetaV1, false).unwrap();
        assert!(authoring.contains("AuthoringMeta"));
        let solidity = show_to_string(KnownMeta::SolidityAbiV2, false).unwrap();
        assert!(solidity.contains("SolidityAbi"));
        let caller = show_to_string(KnownMeta::InterpreterCallerMetaV1, false).unwrap();
        assert!(caller.contains("InterpreterCallerMeta"));
        for pair in [
            (&op, &authoring),
            (&op, &solidity),
            (&op, &caller),
            (&authoring, &solidity),
            (&authoring, &caller),
            (&solidity, &caller),
        ] {
            assert_ne!(pair.0, pair.1);
        }
    }

    /// Metas without a JSON schema error with the exact unsupported
    /// message.
    #[test]
    fn test_show_unsupported_meta_error() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let err = show(Show {
            schema: KnownMeta::DotrainV1,
            output_path: Some(file.path().to_path_buf()),
            pretty_print: false,
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "Unsupported for dotrain-v1 meta");
    }
}
