use crate::model::{Repo, RepoData};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
};

const FRONT_MATTER_TITLE: &str = "（随時更新）GitHubの自分の公開リポジトリ一覧を自動生成させてみる";

pub fn build_markdown<F>(
    data: &RepoData,
    owner: &str,
    existing_markdown: Option<&str>,
    mut resolve_repo_url: F,
) -> String
where
    F: FnMut(&str, &str) -> String,
{
    let updated_at = &data.meta.last_json_commit_push_date;
    let groups = collect_groups(data);
    let total_repos = groups.iter().map(|group| group.repos.len()).sum::<usize>();
    let mut resolved_repos = 0usize;
    let hatena_entry_id = extract_hatena_entry_id(existing_markdown).unwrap_or_default();

    let mut out = String::new();

    out.push_str("---\n");
    out.push_str(&format!(
        "title: \"{}\"\n",
        escape_yaml_double_quoted(FRONT_MATTER_TITLE)
    ));
    out.push_str(&format!(
        "hatena_entry_id: \"{}\"\n",
        escape_yaml_double_quoted(&hatena_entry_id)
    ));
    out.push_str("---\n\n");

    out.push_str("## 目次\n\n");
    for group in &groups {
        let anchor = group_anchor(group.name);
        out.push_str(&format!(
            "- [{}](#{anchor}) ({}件)\n",
            group.name,
            group.repos.len()
        ));
    }
    out.push('\n');

    out.push_str("## 概要\n\n");
    out.push_str(&format!(
        "{owner}のGitHubリポジトリをグループ別に一覧化したものです。\n\n"
    ));
    out.push_str(&format!("最終更新: {updated_at}\n\n"));

    for group in groups {
        let anchor = group_anchor(group.name);
        out.push_str(&format!("<a id=\"{anchor}\"></a>\n\n"));
        out.push_str(&format!("## {}\n\n", group.name));
        for repo in group.repos {
            resolved_repos += 1;
            println!(
                "[url-resolve] ({resolved_repos}/{total_repos}) start: {owner}/{}",
                repo.name
            );
            let url = resolve_repo_url(owner, &repo.name);
            println!(
                "[url-resolve] ({resolved_repos}/{total_repos}) done: {owner}/{} -> {url}",
                repo.name
            );
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
                let tag_str = repo
                    .tags
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!("タグ: {tag_str}\n\n"));
            }
        }
    }

    out
}

fn extract_hatena_entry_id(existing_markdown: Option<&str>) -> Option<String> {
    let markdown = existing_markdown?;
    let mut lines = markdown.lines();
    if lines.next()? != "---" {
        return None;
    }

    let mut has_title = false;
    let mut hatena_entry_id: Option<String> = None;

    for line in lines {
        if line == "---" {
            return if has_title {
                match hatena_entry_id {
                    Some(value) if !value.is_empty() => Some(value),
                    _ => None,
                }
            } else {
                None
            };
        }

        let (key, value) = line.split_once(':')?;
        let value = value.trim();
        match key.trim() {
            "title" => {
                parse_yaml_scalar(value)?;
                has_title = true;
            }
            "hatena_entry_id" => {
                hatena_entry_id = Some(parse_yaml_scalar(value)?);
            }
            _ => {}
        }
    }

    None
}

fn parse_yaml_scalar(value: &str) -> Option<String> {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(
            value[1..value.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\"),
        )
    } else {
        Some(value.to_string())
    }
}

fn escape_yaml_double_quoted(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug)]
struct RepoGroup<'a> {
    name: &'a str,
    repos: Vec<&'a Repo>,
}

fn collect_groups<'a>(data: &'a RepoData) -> Vec<RepoGroup<'a>> {
    let mut grouped: BTreeMap<&str, Vec<&Repo>> = BTreeMap::new();
    for repo in &data.repos {
        grouped.entry(&repo.group).or_default().push(repo);
    }

    let registered_indices: HashMap<&str, usize> = data
        .registered_groups
        .iter()
        .enumerate()
        .map(|(idx, group)| (group.as_str(), idx))
        .collect();

    let mut groups: Vec<_> = grouped
        .into_iter()
        .map(|(name, repos)| RepoGroup { name, repos })
        .collect();

    groups.sort_by_key(|group| {
        (
            group_sort_bucket(group.name),
            Reverse(group.repos.len()),
            registered_indices
                .get(group.name)
                .copied()
                .unwrap_or(usize::MAX),
            group.name,
        )
    });

    groups
}

fn group_sort_bucket(group: &str) -> u8 {
    match group {
        "etc" => 1,
        "stub" => 2,
        _ => 0,
    }
}

fn group_anchor(group: &str) -> String {
    let mut anchor = String::from("group");
    let mut previous_was_separator = true;

    for ch in group.chars() {
        if ch.is_alphanumeric() {
            if previous_was_separator {
                anchor.push('-');
            }
            anchor.extend(ch.to_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            previous_was_separator = true;
        }
    }

    if anchor == "group" {
        "group-section".to_string()
    } else {
        anchor
    }
}

#[cfg(test)]
mod tests {
    use super::{build_markdown, FRONT_MATTER_TITLE};
    use crate::model::{Meta, Repo, RepoData};

    #[test]
    fn builds_toc_and_sorts_groups_by_count_with_etc_and_stub_last() {
        let data = RepoData {
            meta: Meta {
                github_desc_updated_at: "2026-04-05".into(),
                last_json_commit_push_date: "2026-04-05".into(),
                owner: None,
            },
            registered_tags: vec![],
            registered_groups: vec!["beta".into(), "alpha".into(), "etc".into(), "stub".into()],
            repos: vec![
                repo("beta-1", "beta"),
                repo("beta-2", "beta"),
                repo("alpha-1", "alpha"),
                repo("alpha-2", "alpha"),
                repo("etc-1", "etc"),
                repo("etc-2", "etc"),
                repo("etc-3", "etc"),
                repo("stub-1", "stub"),
                repo("gamma-1", "gamma"),
                repo("gamma-2", "gamma"),
            ],
        };

        let markdown = build_markdown(&data, "cat2151", None, |owner, repo_name| {
            format!("https://github.com/{owner}/{repo_name}")
        });

        let toc_pos = markdown.find("## 目次").unwrap();
        let overview_pos = markdown.find("## 概要").unwrap();
        let beta_pos = markdown.find("## beta").unwrap();
        let alpha_pos = markdown.find("## alpha").unwrap();
        let gamma_pos = markdown.find("## gamma").unwrap();
        let etc_pos = markdown.find("## etc").unwrap();
        let stub_pos = markdown.find("## stub").unwrap();

        assert!(toc_pos < overview_pos);
        assert!(beta_pos < alpha_pos);
        assert!(alpha_pos < gamma_pos);
        assert!(gamma_pos < etc_pos);
        assert!(etc_pos < stub_pos);

        assert!(markdown.contains("- [beta](#group-beta) (2件)"));
        assert!(markdown.contains("- [etc](#group-etc) (3件)"));
        assert!(markdown.contains("- [stub](#group-stub) (1件)"));
        assert!(markdown.contains("<a id=\"group-etc\"></a>"));
        assert!(markdown.contains("<a id=\"group-stub\"></a>"));
    }

    #[test]
    fn uses_generated_repo_target_when_available() {
        let data = RepoData {
            meta: Meta {
                github_desc_updated_at: "2026-04-05".into(),
                last_json_commit_push_date: "2026-04-05".into(),
                owner: None,
            },
            registered_tags: vec![],
            registered_groups: vec!["tools".into()],
            repos: vec![repo("cat-self-update", "tools")],
        };

        let markdown = build_markdown(&data, "cat2151", None, |owner, repo_name| {
            if repo_name == "cat-self-update" {
                format!("https://github.com/{owner}/{repo_name}/blob/HEAD/README.ja.md")
            } else {
                format!("https://github.com/{owner}/{repo_name}")
            }
        });

        assert!(markdown.contains(
            "### [cat-self-update](https://github.com/cat2151/cat-self-update/blob/HEAD/README.ja.md)"
        ));
    }

    #[test]
    fn uses_owner_in_title_and_overview() {
        let data = RepoData {
            meta: Meta {
                github_desc_updated_at: "2026-04-05".into(),
                last_json_commit_push_date: "2026-04-05".into(),
                owner: Some("someone".into()),
            },
            registered_tags: vec![],
            registered_groups: vec!["tools".into()],
            repos: vec![repo("tool-1", "tools")],
        };

        let markdown = build_markdown(&data, "someone", None, |owner, repo_name| {
            format!("https://github.com/{owner}/{repo_name}")
        });

        assert!(markdown.starts_with(&format!(
            "---\ntitle: \"{FRONT_MATTER_TITLE}\"\nhatena_entry_id: \"\"\n---\n\n"
        )));
        assert!(markdown.contains("someoneのGitHubリポジトリをグループ別に一覧化したものです。"));
    }

    #[test]
    fn preserves_existing_hatena_entry_id_from_yaml_front_matter() {
        let data = RepoData {
            meta: Meta {
                github_desc_updated_at: "2026-04-05".into(),
                last_json_commit_push_date: "2026-04-05".into(),
                owner: Some("someone".into()),
            },
            registered_tags: vec![],
            registered_groups: vec!["tools".into()],
            repos: vec![repo("tool-1", "tools")],
        };

        let existing_markdown = r#"---
title: "old title"
hatena_entry_id: "12345678901234567890"
---

old body
"#;

        let markdown = build_markdown(
            &data,
            "someone",
            Some(existing_markdown),
            |owner, repo_name| format!("https://github.com/{owner}/{repo_name}"),
        );

        assert!(markdown.starts_with(&format!(
            "---\ntitle: \"{FRONT_MATTER_TITLE}\"\nhatena_entry_id: \"12345678901234567890\"\n---\n\n"
        )));
    }

    #[test]
    fn does_not_preserve_hatena_entry_id_from_legacy_front_matter() {
        let data = RepoData {
            meta: Meta {
                github_desc_updated_at: "2026-04-05".into(),
                last_json_commit_push_date: "2026-04-05".into(),
                owner: Some("someone".into()),
            },
            registered_tags: vec![],
            registered_groups: vec!["tools".into()],
            repos: vec![repo("tool-1", "tools")],
        };

        let existing_markdown = r#"| | |
| --- | --- |
| title | someoneのGitHubリポジトリ一覧 |
| hatena_entry_id | 12345678901234567890 |
"#;

        let markdown = build_markdown(
            &data,
            "someone",
            Some(existing_markdown),
            |owner, repo_name| format!("https://github.com/{owner}/{repo_name}"),
        );

        assert!(markdown.starts_with(&format!(
            "---\ntitle: \"{FRONT_MATTER_TITLE}\"\nhatena_entry_id: \"\"\n---\n\n"
        )));
    }

    fn repo(name: &str, group: &str) -> Repo {
        Repo {
            name: name.into(),
            created_at: "2026-04-05T00:00:00Z".into(),
            updated_at: "2026-04-05T00:00:00Z".into(),
            github_desc: String::new(),
            desc_short: String::new(),
            desc_long: String::new(),
            group: group.into(),
            tags: vec![],
        }
    }
}
