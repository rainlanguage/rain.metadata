use clap::Parser;
use std::path::PathBuf;
use crate::meta::KnownMeta;
use crate::cli::output::SupportedOutputEncoding;
use super::json_schema;

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
    let schema_json = json_schema(s.schema)
        .ok_or_else(|| anyhow::anyhow!("Unsupported for {} meta", s.schema))?;
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
    fn test_show_authoring_v1_schema_compact() {
        let s = show_to_string(KnownMeta::AuthoringMetaV1, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(s.contains("AuthoringMeta"), "{}", v["title"]);
        assert!(!s.contains('\n'));
    }

    /// OpV1 (#304), SolidityAbiV2 and InterpreterCallerMetaV1 (#317) are
    /// known metas with no schema here, so show refuses them rather than
    /// producing one.
    #[test]
    fn test_show_refuses_unmodelled_metas() {
        for meta in [
            KnownMeta::OpV1,
            KnownMeta::SolidityAbiV2,
            KnownMeta::InterpreterCallerMetaV1,
        ] {
            assert!(show_to_string(meta, false).is_err(), "{:?}", meta);
        }
    }

    /// The pretty flag pretty-prints the same schema.
    #[test]
    fn test_show_pretty_print() {
        let s = show_to_string(KnownMeta::AuthoringMetaV1, true).unwrap();
        assert!(s.starts_with("{\n"));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v["title"].is_string());
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
