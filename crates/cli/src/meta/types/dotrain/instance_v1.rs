use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Configuration for a value field in the dotrain instance
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ValueCfg {
    /// Unique identifier for the field
    pub id: String,
    /// Optional human-readable name
    pub name: Option<String>,
    /// The actual value as string
    pub value: String,
}

/// Configuration for a token selection
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TokenCfg {
    /// Network name where the token exists
    pub network: String,
    /// Token contract address as hex string
    pub address: String,
}

/// Dotrain Instance V1 metadata - contains user's specific configuration
/// for a deployed order referencing a dotrain template
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DotrainInstanceV1 {
    /// Hash of the original dotrain template in Metaboard
    pub dotrain_hash: String,
    /// User-configured field values
    pub field_values: BTreeMap<String, ValueCfg>,
    /// Deposit configurations
    pub deposits: BTreeMap<String, ValueCfg>,
    /// Selected tokens for the order
    pub select_tokens: BTreeMap<String, TokenCfg>,
    /// Vault IDs mapping (input/output, index) -> vault_id
    pub vault_ids: BTreeMap<String, Option<String>>,
    /// Selected deployment name from the dotrain
    pub selected_deployment: String,
}

impl DotrainInstanceV1 {
    /// Get the template hash
    pub fn dotrain_hash(&self) -> &str {
        &self.dotrain_hash
    }

    pub fn get_token_addresses(&self) -> Vec<&str> {
        self.select_tokens
            .values()
            .map(|token| token.address.as_str())
            .collect()
    }

    /// Get all non-empty vault IDs
    pub fn get_vault_ids(&self) -> Vec<&str> {
        self.vault_ids
            .values()
            .filter_map(|id| id.as_ref())
            .map(|s| s.as_str())
            .collect()
    }
}

impl std::fmt::Display for DotrainInstanceV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.dotrain_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use base64::{engine::general_purpose::URL_SAFE, Engine};

    #[test]
    fn test_dotrain_hash() {
        let hash = Sha256::digest(b"test content");
        let dotrain_hash = URL_SAFE.encode(hash);
        let instance = DotrainInstanceV1 {
            dotrain_hash: dotrain_hash.clone(),
            field_values: BTreeMap::new(),
            deposits: BTreeMap::new(),
            select_tokens: BTreeMap::new(),
            vault_ids: BTreeMap::new(),
            selected_deployment: "test".to_string(),
        };
        assert_eq!(instance.dotrain_hash(), dotrain_hash);
    }
}
