use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};
use tempfile::NamedTempFile;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthData {
    pub raw: serde_json::Value,
}

pub struct AuthStore {
    path: String,
}

impl AuthStore {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    pub fn load(&self) -> Result<Option<AuthData>> {
        if !Path::new(&self.path).exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&self.path)?;
        let parsed = serde_json::from_str(&data)?;
        Ok(Some(parsed))
    }

    pub fn save_atomic(&self, data: &AuthData) -> Result<()> {
        let serialized = serde_json::to_string_pretty(data)?;

        let mut temp = NamedTempFile::new()?;
        temp.write_all(serialized.as_bytes())?;
        temp.flush()?;

        temp.persist(&self.path)?;
        Ok(())
    }
}