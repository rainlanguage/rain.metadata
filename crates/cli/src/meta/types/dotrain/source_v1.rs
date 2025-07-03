use alloy::primitives::B256;
use serde::{Deserialize, Serialize};

/// Dotrain Source V1 meta
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DotrainSourceV1(String);

impl DotrainSourceV1 {
    pub fn hash(&self) -> B256 {
        use alloy::primitives::keccak256;
        keccak256(self.0.as_bytes())
    }
}

impl std::fmt::Display for DotrainSourceV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hash())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{keccak256};

    #[test]
    fn test_hash_method() {
        let content = "test content".to_string();
        let dotrain = DotrainSourceV1(content.clone());
        assert_eq!(dotrain.hash(), keccak256(content.as_bytes()));
    }

    #[test]
    fn test_display_trait() {
        let content = "display test".to_string();
        let dotrain = DotrainSourceV1(content.clone());
        let displayed = format!("{}", dotrain);
        let expected = keccak256(content.as_bytes()).to_string();
        assert_eq!(displayed, expected);
    }
}
