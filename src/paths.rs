use anyhow::{anyhow, Result};
use std::path::PathBuf;

const APP_NAME: &str = "own-repos-curator-to-hatena";

pub fn repos_json_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("failed to resolve AppData\\Local"))?;
    Ok(base
        .join("own-repos-curator")
        .join("data")
        .join("repos.json"))
}

pub fn managed_repos_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("failed to resolve AppData\\Local"))?;
    Ok(base.join(APP_NAME).join("repos"))
}

pub fn url_cache_path() -> Result<PathBuf> {
    let base = dirs::cache_dir().ok_or_else(|| anyhow!("failed to resolve AppData\\Local"))?;
    Ok(base.join(APP_NAME).join("cache").join("url.json"))
}

#[cfg(test)]
mod tests {
    use super::url_cache_path;

    #[test]
    fn url_cache_path_uses_app_cache_dir() {
        let path = url_cache_path().unwrap();
        let normalized = path.to_string_lossy().replace('\\', "/");

        assert!(normalized.ends_with("own-repos-curator-to-hatena/cache/url.json"));
    }
}
