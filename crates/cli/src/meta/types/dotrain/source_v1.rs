use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use base64::{engine::general_purpose::URL_SAFE, Engine};

/// Dotrain Source V1 meta
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DotrainSourceV1(String);

impl DotrainSourceV1 {
    pub fn get_hash(&self) -> String {
        URL_SAFE.encode(
            Sha256::digest(self.0.as_bytes())
        )
    }
}

impl std::fmt::Display for DotrainSourceV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_hash())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_trait() {
        let content = "display test".to_string();
        let dotrain = DotrainSourceV1(content.clone());
        let displayed = format!("{}", dotrain);
        let expected_hash = dotrain.get_hash();
        let expected = format!("{}", expected_hash);
        assert_eq!(displayed, expected);
    }
}
