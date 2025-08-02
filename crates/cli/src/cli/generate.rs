use std::path::PathBuf;
use std::io::{self, Read};
use std::fs;
use clap::Args;
use serde::Serialize;
use alloy::primitives::hex;

use crate::error::Error;
use crate::meta::types::dotrain::source_v1::DotrainSourceV1;
use crate::meta::RainMetaDocumentV1Item;
use crate::meta::KnownMagic;

/// Generate deployment data for DotrainSourceV1 content
#[derive(Args)]
pub struct Generate {
    /// Path to input .rain file. If not provided, reads from stdin
    #[arg(short = 'i', long = "input-path")]
    input_path: Option<PathBuf>,

    /// Path to output JSON file. If not provided, prints to stdout  
    #[arg(short = 'o', long = "output-path")]
    output_path: Option<PathBuf>,
}

/// Deployment data for publishing to MetaBoard
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DeploymentData {
    /// Keccak256 hash of the content (hex-encoded with 0x prefix)
    pub subject: String,
    /// CBOR-encoded metadata bytes (hex-encoded with 0x prefix)
    pub meta_bytes: String,
    /// Complete calldata for MetaBoard.emitMeta() (hex-encoded with 0x prefix)
    pub calldata: String,
}

/// Validate dotrain content - basic checks
fn validate_dotrain_content(content: &str) -> Result<(), Error> {
    // Check if content is empty or only whitespace
    if content.trim().is_empty() {
        return Err(Error::InvalidInput(
            "Dotrain content cannot be empty".to_string(),
        ));
    }

    // Try to create DotrainSourceV1 to ensure it's valid
    let _dotrain_source = DotrainSourceV1(content.to_string());

    Ok(())
}

/// Generate deployment data for DotrainSourceV1 content
/// Validates content and returns subject hash, meta bytes, and calldata
fn generate_dotrain_deployment(content: &str) -> Result<DeploymentData, Error> {
    // Validate content
    validate_dotrain_content(content)?;

    // Create DotrainSourceV1
    let dotrain_source = DotrainSourceV1(content.to_string());

    // Convert to RainMetaDocumentV1Item
    let document: RainMetaDocumentV1Item = dotrain_source.into();

    let documents = vec![document.clone()];
    // Generate CBOR bytes
    let meta_bytes =
        RainMetaDocumentV1Item::cbor_encode_seq(&documents, KnownMagic::RainMetaDocumentV1)?;

    // Calculate subject hash
    let subject_hash = document.hash(false)?;

    // Generate calldata (simplified version without the metaboard module)
    use alloy::primitives::{Bytes, FixedBytes};
    use alloy::sol_types::SolCall;
    use rain_metadata_bindings::MetaBoard::emitMetaCall;

    let call = emitMetaCall {
        subject: FixedBytes::from(subject_hash),
        meta: Bytes::from(meta_bytes.clone()),
    };
    let calldata = call.abi_encode();

    Ok(DeploymentData {
        subject: format!("0x{}", hex::encode(subject_hash)),
        meta_bytes: format!("0x{}", hex::encode(meta_bytes)),
        calldata: format!("0x{}", hex::encode(calldata)),
    })
}

/// Read content from input source (file or stdin)
fn read_input_content(input_path: Option<PathBuf>) -> Result<String, Error> {
    match input_path {
        Some(path) => {
            // Read from file
            fs::read_to_string(&path).map_err(|e| {
                Error::InvalidInput(format!("Failed to read file '{}': {}", path.display(), e))
            })
        }
        None => {
            // Read from stdin
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|e| Error::InvalidInput(format!("Failed to read from stdin: {}", e)))?;
            Ok(buffer)
        }
    }
}

/// Write output to destination (file or stdout)
fn write_output(data: &DeploymentData, output_path: Option<PathBuf>) -> Result<(), Error> {
    // Serialize to pretty JSON
    let json_output = serde_json::to_string_pretty(data).map_err(Error::SerdeJsonError)?;

    match output_path {
        Some(path) => {
            // Write to file
            fs::write(&path, json_output).map_err(|e| {
                Error::InvalidInput(format!("Failed to write file '{}': {}", path.display(), e))
            })
        }
        None => {
            // Write to stdout
            println!("{}", json_output);
            Ok(())
        }
    }
}

/// Execute the generate command
pub fn generate(args: Generate) -> anyhow::Result<()> {
    // Read input content
    let content = read_input_content(args.input_path)?;

    // Generate deployment data
    let deployment_data = generate_dotrain_deployment(&content)?;

    // Write output
    write_output(&deployment_data, args.output_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_input_content_from_file() {
        // Create a temporary file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "#main _ _: int-add(1 2)";
        writeln!(temp_file, "{}", test_content).unwrap();

        let content = read_input_content(Some(temp_file.path().to_path_buf())).unwrap();
        assert_eq!(content.trim(), test_content);
    }

    #[test]
    fn test_read_input_content_nonexistent_file() {
        let result = read_input_content(Some(PathBuf::from("/nonexistent/file.rain")));
        assert!(result.is_err());
    }

    #[test]
    fn test_write_output_to_file() {
        let deployment_data = DeploymentData {
            subject: "0x1234567890abcdef".to_string(),
            meta_bytes: "0xdeadbeef".to_string(),
            calldata: "0xcafebabe".to_string(),
        };

        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        write_output(&deployment_data, Some(temp_path.clone())).unwrap();

        let written_content = fs::read_to_string(&temp_path).unwrap();
        assert!(written_content.contains("0x1234567890abcdef"));
        assert!(written_content.contains("0xdeadbeef"));
        assert!(written_content.contains("0xcafebabe"));
    }

    #[test]
    fn test_full_generate_flow() {
        // Create input file
        let mut input_file = NamedTempFile::new().unwrap();
        let test_content = "#main _ _: int-add(1 2) int-add(2 3)";
        writeln!(input_file, "{}", test_content).unwrap();

        // Create output file
        let output_file = NamedTempFile::new().unwrap();
        let output_path = output_file.path().to_path_buf();

        // Execute generate command
        let args = Generate {
            input_path: Some(input_file.path().to_path_buf()),
            output_path: Some(output_path.clone()),
        };

        generate(args).unwrap();

        // Verify output
        let output_content = fs::read_to_string(&output_path).unwrap();
        let parsed: DeploymentData = serde_json::from_str(&output_content).unwrap();

        assert!(parsed.subject.starts_with("0x"));
        assert!(parsed.meta_bytes.starts_with("0x"));
        assert!(parsed.calldata.starts_with("0x"));
        assert_eq!(parsed.subject.len(), 66); // 0x + 64 hex chars
    }
}
