use serde_json::Value;
use strum::EnumString;
use crate::error::Error;

/// Represent section of a solidity artifact to extract
#[derive(Copy, Clone, EnumString, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum ArtifactComponent {
    Abi,
    Bytecode,
    DeployedBytecode,
}

/// extracts the given section of a solidity artifact as [Value]
///
/// does not perform any checks on the returned [Value] such as if
/// it is null or not.
/// The given data should be utf8 encoded json string bytes
pub fn extract_artifact_component_json(
    component: ArtifactComponent,
    data: &[u8],
) -> Result<Value, Error> {
    let json = serde_json::from_str::<Value>(std::str::from_utf8(data)?)?;
    match component {
        ArtifactComponent::Abi => Ok(json["abi"].clone()),
        ArtifactComponent::Bytecode => Ok(json["bytecode"].clone()),
        ArtifactComponent::DeployedBytecode => Ok(json["deployedBytecode"].clone()),
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    fn artifact_json() -> Vec<u8> {
        serde_json::json!({
            "abi": [{ "type": "function", "name": "foo" }],
            "bytecode": { "object": "0x6001" },
            "deployedBytecode": { "object": "0x6002" }
        })
        .to_string()
        .into_bytes()
    }

    /// Each component arm extracts exactly its own key.
    #[test]
    fn test_extract_each_component() {
        let data = artifact_json();
        assert_eq!(
            extract_artifact_component_json(ArtifactComponent::Abi, &data).unwrap(),
            serde_json::json!([{ "type": "function", "name": "foo" }])
        );
        assert_eq!(
            extract_artifact_component_json(ArtifactComponent::Bytecode, &data).unwrap(),
            serde_json::json!({ "object": "0x6001" })
        );
        assert_eq!(
            extract_artifact_component_json(ArtifactComponent::DeployedBytecode, &data).unwrap(),
            serde_json::json!({ "object": "0x6002" })
        );
    }

    /// Documented: no null check is performed — a missing component is
    /// returned as JSON null, not an error.
    #[test]
    fn test_missing_component_returns_null() {
        assert_eq!(
            extract_artifact_component_json(ArtifactComponent::Abi, b"{}").unwrap(),
            serde_json::Value::Null
        );
    }

    /// Non-utf8 and non-json inputs error.
    #[test]
    fn test_invalid_input_errors() {
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, &[0xff, 0xfe]).is_err());
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, b"not json").is_err());
    }
}
