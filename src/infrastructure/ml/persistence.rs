#![allow(dead_code)]
use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;

/// Save a model binary to the filesystem in addition to DB storage
pub async fn save_model_to_disk(model_id: &str, binary: &[u8]) -> Result<()> {
    let mut path = PathBuf::from("models");
    if !path.exists() {
        fs::create_dir_all(&path).await?;
    }
    path.push(format!("{}.bin", model_id));
    fs::write(path, binary).await?;
    Ok(())
}

pub async fn load_model_from_disk(model_id: &str) -> Result<Vec<u8>> {
    let mut path = PathBuf::from("models");
    path.push(format!("{}.bin", model_id));
    let data = fs::read(path).await?;
    Ok(data)
}
