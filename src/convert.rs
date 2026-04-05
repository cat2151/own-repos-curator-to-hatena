use crate::model::{Repo, RepoData};
use std::collections::BTreeMap;

const GITHUB_BASE: &str = "https://github.com/cat2151";

pub fn build_markdown(data: &RepoData) -> String {
    let mut groups: BTreeMap<&str, Vec<&Repo>> = BTreeMap::new();
    for repo in &data.repos {
        groups.entry(&repo.group).or_default().push(repo);
    }

    let updated_at = &data.meta.last_json_commit_push_date;

    let mut out = String::new();

    // frontmatter (table形式, はてなブログ互換)
    out.push_str("| | |\n");
    out.push_str("| --- | --- |\n");
    out.push_str("| title | cat2151のGitHubリポジトリ一覧 |\n");
    out.push('\n');

    out.push_str("## 概要\n\n");
    out.push_str("cat2151のGitHubリポジトリをグループ別に一覧化したものです。\n\n");
    out.push_str(&format!("最終更新: {updated_at}\n\n"));

    for (group, repos) in &groups {
        out.push_str(&format!("## {group}\n\n"));
        for repo in repos {
            let url = format!("{GITHUB_BASE}/{}", repo.name);
            out.push_str(&format!("### [{}]({})\n\n", repo.name, url));

            if repo.desc_short.is_empty() {
                out.push_str("（説明なし）\n\n");
            } else {
                out.push_str(&format!("{}\n\n", repo.desc_short));
            }

            if !repo.desc_long.is_empty() {
                out.push_str(&format!("{}\n\n", repo.desc_long));
            }

            if !repo.tags.is_empty() {
                let tag_str = repo.tags.iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!("タグ: {tag_str}\n\n"));
            }
        }
    }

    out
}
