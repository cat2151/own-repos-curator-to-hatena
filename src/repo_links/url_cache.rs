use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(super) struct UrlCache {
    path: PathBuf,
    updated_on: String,
    entries: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct UrlCacheFile {
    updated_on: String,
    #[serde(default)]
    entries: HashMap<String, String>,
}

impl UrlCache {
    pub(super) fn load(path: PathBuf, today: NaiveDate) -> Result<Self> {
        let updated_on = today.to_string();
        let entries = read_cache_file(&path)?
            .filter(|cache| cache.updated_on == updated_on)
            .map(|cache| cache.entries)
            .unwrap_or_default();

        Ok(Self {
            path,
            updated_on,
            entries,
        })
    }

    pub(super) fn get(&self, owner: &str, repo_name: &str) -> Option<&str> {
        self.entries
            .get(&cache_key(owner, repo_name))
            .map(String::as_str)
    }

    pub(super) fn insert(&mut self, owner: &str, repo_name: &str, url: String) -> Result<()> {
        let key = cache_key(owner, repo_name);
        if self.entries.get(&key).is_some_and(|cached| cached == &url) {
            return Ok(());
        }

        self.entries.insert(key, url);
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create dir: {}", parent.display()))?;
        }

        let body = serde_json::to_string_pretty(&UrlCacheFile {
            updated_on: self.updated_on.clone(),
            entries: self.entries.clone(),
        })
        .context("failed to serialize url cache")?;

        fs::write(&self.path, body)
            .with_context(|| format!("failed to write {}", self.path.display()))
    }
}

fn read_cache_file(path: &Path) -> Result<Option<UrlCacheFile>> {
    if !path.exists() {
        return Ok(None);
    }

    let body =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    match serde_json::from_str(&body) {
        Ok(cache) => Ok(Some(cache)),
        Err(err) => {
            eprintln!(
                "[url-cache] ignoring invalid cache file {}: {err}",
                path.display()
            );
            Ok(None)
        }
    }
}

fn cache_key(owner: &str, repo_name: &str) -> String {
    format!(
        "{}/{}",
        owner.to_ascii_lowercase(),
        repo_name.to_ascii_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::UrlCache;
    use chrono::NaiveDate;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn stores_and_reuses_entries_for_the_same_day() {
        let path = temp_cache_path("same-day");
        let today = date("2026-04-05");

        let mut cache = UrlCache::load(path.clone(), today).unwrap();
        cache
            .insert(
                "cat2151",
                "cat-self-update",
                "https://github.com/cat2151/cat-self-update/blob/HEAD/README.ja.md".into(),
            )
            .unwrap();

        let cache = UrlCache::load(path.clone(), today).unwrap();
        assert_eq!(
            cache.get("cat2151", "cat-self-update"),
            Some("https://github.com/cat2151/cat-self-update/blob/HEAD/README.ja.md")
        );

        cleanup_parent(path);
    }

    #[test]
    fn drops_previous_day_entries() {
        let path = temp_cache_path("stale");
        let previous_day = date("2026-04-04");
        let today = date("2026-04-05");

        let mut cache = UrlCache::load(path.clone(), previous_day).unwrap();
        cache
            .insert(
                "cat2151",
                "cat-self-update",
                "https://github.com/cat2151/cat-self-update/blob/HEAD/README.ja.md".into(),
            )
            .unwrap();

        let cache = UrlCache::load(path.clone(), today).unwrap();
        assert_eq!(cache.get("cat2151", "cat-self-update"), None);

        cleanup_parent(path);
    }

    #[test]
    fn ignores_invalid_cache_files() {
        let path = temp_cache_path("invalid");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, "{ invalid json").unwrap();

        let cache = UrlCache::load(path.clone(), date("2026-04-05")).unwrap();
        assert_eq!(cache.get("cat2151", "cat-self-update"), None);

        cleanup_parent(path);
    }

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    fn temp_cache_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("own-repos-curator-to-hatena-{label}-{unique}"))
            .join("url.json")
    }

    fn cleanup_parent(path: PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
