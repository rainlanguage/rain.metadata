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
