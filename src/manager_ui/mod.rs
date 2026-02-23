use std::path::{Path, PathBuf};

use crate::errors::AppError;

pub fn resolve_manager_dist() -> PathBuf {
    PathBuf::from("manager/dist")
}

pub fn secure_assets_path(base_dir: &Path, file_name: &str) -> Result<PathBuf, AppError> {
    if file_name.is_empty()
        || file_name.contains("..")
        || file_name.contains('\\')
        || file_name.starts_with('/')
    {
        return Err(AppError::forbidden("Forbidden"));
    }

    let path = base_dir.join("assets").join(file_name);
    let resolved_path = path.canonicalize().map_err(|_| AppError::not_found("File not found"))?;
    let resolved_assets = base_dir
        .join("assets")
        .canonicalize()
        .map_err(|_| AppError::not_found("File not found"))?;

    if !resolved_path.starts_with(&resolved_assets) {
        return Err(AppError::forbidden("Forbidden"));
    }

    Ok(resolved_path)
}
