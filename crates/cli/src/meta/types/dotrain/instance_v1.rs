use alloy::primitives::{Address, B256};
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
    /// Token contract address
    pub address: Address,
}

/// Dotrain Instance V1 metadata - contains user's specific configuration
/// for a deployed order referencing a dotrain template
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DotrainInstanceV1 {
    /// Hash of the original dotrain template in Metaboard
    pub dotrain_hash: B256,
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
    pub fn dotrain_hash(&self) -> B256 {
        self.dotrain_hash
    }

    pub fn get_token_addresses(&self) -> Vec<Address> {
        self.select_tokens
            .values()
            .map(|token| token.address)
            .collect()
    }

    /// Get all non-empty vault IDs
    pub fn get_vault_ids(&self) -> Vec<String> {
        self.vault_ids
            .values()
            .filter_map(|id| id.as_ref())
            .cloned()
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
    use alloy::primitives::{B256};

    #[test]
    fn test_dotrain_hash() {
        let dotrain_hash = B256::from_slice(&[42u8; 32]);
        let instance = DotrainInstanceV1 {
            dotrain_hash,
            field_values: BTreeMap::new(),
            deposits: BTreeMap::new(),
            select_tokens: BTreeMap::new(),
            vault_ids: BTreeMap::new(),
            selected_deployment: "test".to_string(),
        };
        assert_eq!(instance.dotrain_hash(), dotrain_hash);
    }
}
