//! A scheme is read once during preflight; execution never reopens its path.
use super::*;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrozenProcessingScheme {
    pub content: serde_json::Value,
    pub sha256: String,
    pub fingerprint: FingerprintRecord,
}
impl FrozenProcessingScheme {
    pub fn capture(path: &Path) -> Result<Self, AutomationError> {
        let bytes = std::fs::read(path).map_err(|e| AutomationError::Execution(e.to_string()))?;
        let content =
            serde_json::from_slice(&bytes).map_err(|e| AutomationError::InvalidParameters {
                tool_id: "processing.apply_scheme".into(),
                message: format!("Invalid processing scheme: {e}"),
            })?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let frozen = Self {
            content,
            fingerprint: FingerprintRecord {
                role: "processing_scheme".into(),
                path: path.to_owned(),
                sha256: sha256.clone(),
                bytes: bytes.len() as u64,
            },
            sha256,
        };
        frozen.decode()?;
        Ok(frozen)
    }
    pub fn decode(&self) -> Result<crate::project::ProcessingScheme, AutomationError> {
        let scheme: crate::project::ProcessingScheme = serde_json::from_value(self.content.clone())
            .map_err(|e| AutomationError::InvalidParameters {
                tool_id: "processing.apply_scheme".into(),
                message: e.to_string(),
            })?;
        if scheme.schema_version != 1 {
            return Err(AutomationError::InvalidParameters {
                tool_id: "processing.apply_scheme".into(),
                message: "Processing scheme schema_version must be 1.".into(),
            });
        }
        Ok(scheme)
    }
}
