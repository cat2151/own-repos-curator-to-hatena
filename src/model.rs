use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoData {
    pub meta: Meta,
    pub registered_tags: Vec<String>,
    pub registered_groups: Vec<String>,
    pub repos: Vec<Repo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Meta {
    pub github_desc_updated_at: String,
    pub last_json_commit_push_date: String,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub github_desc: String,
    pub desc_short: String,
    pub desc_long: String,
    pub group: String,
    pub tags: Vec<String>,
}
