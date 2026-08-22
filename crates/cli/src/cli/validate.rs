use clap::Parser;
use std::path::PathBuf;
use crate::meta::KnownMeta;

/// command for validating a meta
#[derive(Parser)]
pub struct Validate {
    /// The known meta to validate against.
    #[arg(short, long)]
    meta: KnownMeta,
    /// The input path to the json serialized metadata to validate against the
    /// known schema.
    #[arg(short, long)]
    input_path: PathBuf,
}

pub fn validate(v: Validate) -> anyhow::Result<()> {
    let data: Vec<u8> = std::fs::read(v.input_path)?;
    // If we can normalize the input data then it is valid.
    let _normalized = v.meta.normalize(&data)?;
    Ok(())
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use std::io::Write;

    /// A meta that normalizes is valid.
    #[test]
    fn test_validate_ok_for_valid_meta() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"[]").unwrap();
        let v = Validate {
            meta: KnownMeta::SolidityAbiV2,
            input_path: file.path().to_path_buf(),
        };
        assert!(validate(v).is_ok());
    }

    /// A meta that does not normalize is invalid: validity IS
    /// normalizability.
    #[test]
    fn test_validate_err_for_invalid_meta() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"{\"not\": \"an abi\"}").unwrap();
        let v = Validate {
            meta: KnownMeta::SolidityAbiV2,
            input_path: file.path().to_path_buf(),
        };
        assert!(validate(v).is_err());
    }
}
