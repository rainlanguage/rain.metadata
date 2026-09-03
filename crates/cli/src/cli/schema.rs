pub mod ls;
pub mod show;

use clap::Subcommand;
use schemars::schema::RootSchema;
use schemars::schema_for;
use show::Show;
use crate::meta::KnownMeta;

/// command related to meta json schema
#[derive(Subcommand)]
pub enum Schema {
    /// Print all known schemas.
    Ls,
    /// Print a given known schema.
    Show(Show),
}

pub fn dispatch(schema: Schema) -> anyhow::Result<()> {
    match schema {
        Schema::Ls => ls::ls(),
        Schema::Show(s) => show::show(s),
    }
}

/// Single source of truth for which metas `ls` advertises and `show` can
/// produce. Matched exhaustively so a new [`KnownMeta`] cannot be silently
/// absent from both.
pub fn json_schema(meta: KnownMeta) -> Option<RootSchema> {
    match meta {
        KnownMeta::AuthoringMetaV1 => Some(schema_for!(
            crate::meta::types::authoring::v1::AuthoringMeta
        )),
        // OpV1, SolidityAbiV2 and InterpreterCallerMetaV1 are here rather
        // than absent: rainlanguage/rain.metadata#304 and #317 removed the
        // models without removing the metas, so they stay magic numbers this
        // crate can name and `magic ls` still lists them. They have no type
        // left to derive a schema from, which is exactly what None says.
        KnownMeta::OpV1
        | KnownMeta::SolidityAbiV2
        | KnownMeta::InterpreterCallerMetaV1
        | KnownMeta::DotrainV1
        | KnownMeta::RainlangV1
        | KnownMeta::AuthoringMetaV2
        | KnownMeta::ExpressionDeployerV2BytecodeV1
        | KnownMeta::RainlangSourceV1
        | KnownMeta::AddressList
        | KnownMeta::DotrainSourceV1
        | KnownMeta::OrderBuilderStateV1
        | KnownMeta::RaindexSignedContextOracleV1 => None,
    }
}
