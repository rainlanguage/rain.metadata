pub mod ls;

use clap::Subcommand;
use strum::{EnumString, EnumIter};
use clap::Parser;

#[derive(Subcommand)]
pub enum Magic {
    /// Print all known magic numbers.
    Ls,
}

#[derive(Clone, Copy, EnumString, EnumIter, strum::Display)]
#[strum(serialize_all = "kebab_case")]
#[repr(u64)]
pub enum KnownMagic {
    RainMetaDocumentV1 = 0xff0a89c674ee7874,
    SolidityABIV2 = 0xffe5ffb4a3ff2cde,
    OpMetaV1 = 0xffe5282f43e495b4,
    InterpreterCallerMetaV1 = 0xffc21bbf86cc199b,
}

pub fn dispatch (magic: Magic) -> anyhow::Result<()> {
    match magic {
        Magic::Ls => ls::ls(),
    }
}