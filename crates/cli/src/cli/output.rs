use std::io::Write;
use strum::EnumIter;
use strum::EnumString;
use std::path::PathBuf;

#[derive(serde::Serialize, Clone, Copy, EnumString, EnumIter, strum::Display)]
#[strum(serialize_all = "kebab_case")]
#[serde(rename_all = "kebab-case")]
#[repr(u64)]
pub enum SupportedOutputEncoding {
    Binary,
    Hex,
}

pub fn output(
    output_path: &Option<PathBuf>,
    output_encoding: SupportedOutputEncoding,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let hex_encoded: String;
    let encoded_bytes: &[u8] = match output_encoding {
        SupportedOutputEncoding::Binary => bytes,
        SupportedOutputEncoding::Hex => {
            hex_encoded = alloy::primitives::hex::encode_prefixed(bytes);
            hex_encoded.as_bytes()
        }
    };
    if let Some(output_path) = output_path {
        std::fs::write(output_path, encoded_bytes)?
    } else {
        std::io::stdout().write_all(encoded_bytes)?
    }
    Ok(())
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    /// Binary encoding writes the bytes through unchanged.
    #[test]
    fn test_output_binary_writes_exact_bytes_to_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        output(
            &Some(path.clone()),
            SupportedOutputEncoding::Binary,
            &[0x00, 0x01, 0xff],
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), vec![0x00, 0x01, 0xff]);
    }

    /// Hex encoding writes 0x-prefixed lowercase hex.
    #[test]
    fn test_output_hex_writes_prefixed_hex_to_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        output(
            &Some(path.clone()),
            SupportedOutputEncoding::Hex,
            &[0x00, 0x01, 0xff],
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "0x0001ff");
    }
}
