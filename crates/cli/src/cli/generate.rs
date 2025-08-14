use std::path::PathBuf;
use std::io::{self, Read};
use std::fs;
use clap::{Args, Subcommand};

use crate::error::Error;
use crate::metaboard::{DotrainSourceEmitData, generate_dotrain_source_emit_tx_data};

/// Generate tx data to emit metadata
#[derive(Args)]
pub struct Generate {
    #[command(subcommand)]
    pub command: GenerateCommand,
}

/// Generate subcommands
#[derive(Subcommand)]
pub enum GenerateCommand {
    /// Generate deployment data for dotrain source code
    Source(SourceArgs),
}

/// Arguments for generating source deployment data
#[derive(Args)]
pub struct SourceArgs {
    /// Path to input .rain file. If not provided, reads from stdin
    #[arg(short = 'i', long = "input-path")]
    input_path: Option<PathBuf>,

    /// Path to output JSON file. If not provided, prints to stdout  
    #[arg(short = 'o', long = "output-path")]
    output_path: Option<PathBuf>,
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
fn write_output(data: &DotrainSourceEmitData, output_path: Option<PathBuf>) -> Result<(), Error> {
    // Serialize to pretty JSON
    let json_output = serde_json::to_string_pretty(data).map_err(Error::SerdeJsonError)?;

    match output_path {
        Some(path) => {
            // Ensure 
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        return Err(Error::InvalidInput(format!(
                            "Failed to create output directory '{}': {}",
                            parent.display(),
                            e
                        )));
                    }
                }
            }
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
    match args.command {
        GenerateCommand::Source(source_args) => generate_source(source_args),
    }
}

/// Execute the generate source command
fn generate_source(args: SourceArgs) -> anyhow::Result<()> {
    // Read input content
    let content = read_input_content(args.input_path)?;

    let tx_data = generate_dotrain_source_emit_tx_data(&content)?;

    // Write output
    write_output(&tx_data, args.output_path)?;

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
        let deployment_data = DotrainSourceEmitData {
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
            command: GenerateCommand::Source(SourceArgs {
                input_path: Some(input_file.path().to_path_buf()),
                output_path: Some(output_path.clone()),
            }),
        };

        generate(args).unwrap();

        // Verify output
        let output_content = fs::read_to_string(&output_path).unwrap();
        let parsed: DotrainSourceEmitData = serde_json::from_str(&output_content).unwrap();

        assert!(parsed.subject.starts_with("0x"));
        assert!(parsed.meta_bytes.starts_with("0x"));
        assert!(parsed.calldata.starts_with("0x"));
        assert_eq!(parsed.subject.len(), 66); // 0x + 64 hex chars
    }
}
