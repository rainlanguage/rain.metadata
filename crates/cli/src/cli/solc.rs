pub mod artifact;

use clap::Subcommand;
use artifact::Artifact;

/// command related to solc artifacts
#[derive(Subcommand)]
pub enum Solc {
    /// Parse data out of a solc artifact.
    Artifact(Artifact),
}

pub fn dispatch(solc: Solc) -> anyhow::Result<()> {
    match solc {
        Solc::Artifact(artifact) => artifact::artifact(artifact),
    }
}
