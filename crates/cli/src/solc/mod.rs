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

impl ArtifactComponent {
    /// Key this component is read from in a solc artifact object.
    pub fn artifact_key(self) -> &'static str {
        match self {
            ArtifactComponent::Abi => "abi",
            ArtifactComponent::Bytecode => "bytecode",
            ArtifactComponent::DeployedBytecode => "deployedBytecode",
        }
    }
}

/// extracts the given section of a solidity artifact as [Value]
///
/// errors if the artifact is not a json object or carries no such key. a key
/// that is present and explicitly null is returned as [Value::Null].
/// The given data should be utf8 encoded json string bytes
pub fn extract_artifact_component_json(
    component: ArtifactComponent,
    data: &[u8],
) -> Result<Value, Error> {
    let json = serde_json::from_str::<Value>(std::str::from_utf8(data)?)?;
    let key = component.artifact_key();
    json.get(key)
        .cloned()
        .ok_or_else(|| Error::InvalidInput(format!("artifact has no \"{}\" component", key)))
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

    /// Every component keys off its own name, and each name is the one solc
    /// writes into the artifact.
    #[test]
    fn test_artifact_key_per_component() {
        assert_eq!(ArtifactComponent::Abi.artifact_key(), "abi");
        assert_eq!(ArtifactComponent::Bytecode.artifact_key(), "bytecode");
        assert_eq!(
            ArtifactComponent::DeployedBytecode.artifact_key(),
            "deployedBytecode"
        );
    }

    /// An absent component is an error naming the missing key, not a silent
    /// null. Asserted for every component so no arm can regress to indexing.
    #[test]
    fn test_missing_component_errors() {
        for (component, key) in [
            (ArtifactComponent::Abi, "abi"),
            (ArtifactComponent::Bytecode, "bytecode"),
            (ArtifactComponent::DeployedBytecode, "deployedBytecode"),
        ] {
            let err = extract_artifact_component_json(component, b"{}").unwrap_err();
            assert_eq!(
                err.to_string(),
                format!("invalid input: artifact has no \"{}\" component", key)
            );
        }
    }

    /// Only the requested component's absence is an error: the other keys
    /// being present does not satisfy the lookup.
    #[test]
    fn test_missing_component_errors_beside_present_siblings() {
        let data = br#"{"bytecode":{"object":"0x60"},"deployedBytecode":{"object":"0x60"}}"#;
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, data).is_err());
        assert!(extract_artifact_component_json(ArtifactComponent::Bytecode, data).is_ok());
    }

    /// A component present and explicitly null is a value, not an absence:
    /// it round trips as null while an absent key errors.
    #[test]
    fn test_explicit_null_component_is_returned() {
        assert_eq!(
            extract_artifact_component_json(ArtifactComponent::Abi, br#"{"abi":null}"#).unwrap(),
            Value::Null
        );
    }

    /// A json document that is not an object has no components at all.
    #[test]
    fn test_non_object_artifact_errors() {
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, b"[]").is_err());
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, b"null").is_err());
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, br#""abi""#).is_err());
    }

    /// Non-utf8 and non-json inputs error.
    #[test]
    fn test_invalid_input_errors() {
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, &[0xff, 0xfe]).is_err());
        assert!(extract_artifact_component_json(ArtifactComponent::Abi, b"not json").is_err());
    }
}
