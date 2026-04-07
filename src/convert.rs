use crate::model::{Repo, RepoData};
use regex::{Captures, Regex};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};

mod yaml_frontmatter;

use yaml_frontmatter::{escape_yaml_double_quoted, extract_hatena_entry_id};

const MARKDOWN_TEMPLATE: &str = include_str!("../templates/template.md");

#[cfg(test)]
pub(crate) fn template_front_matter_title() -> &'static str {
    MARKDOWN_TEMPLATE
        .lines()
        .find_map(|line| line.strip_prefix("title: \"")?.strip_suffix('"'))
        .expect("template should contain a quoted title line")
}

#[cfg(test)]
pub(crate) fn template_data_index_link() -> &'static str {
    MARKDOWN_TEMPLATE
        .lines()
        .find(|line| line.starts_with("[own-repos-curator-data]("))
        .expect("template should contain own-repos-curator-data link")
}

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
    let toc = build_toc(&groups);
    let group_sections = build_group_sections(
        &groups,
        owner,
        total_repos,
        &mut resolved_repos,
        &mut resolve_repo_url,
    );

    apply_template(
        MARKDOWN_TEMPLATE,
        &[
            (
                "HATENA_ENTRY_ID",
                &escape_yaml_double_quoted(&hatena_entry_id),
            ),
            ("TOC", &toc),
            ("OWNER", owner),
            ("UPDATED_AT", updated_at),
            ("GROUPS", &group_sections),
        ],
    )
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

fn build_toc(groups: &[RepoGroup<'_>]) -> String {
    let mut toc = String::new();
    for group in groups {
        let anchor = group_anchor(group.name);
        toc.push_str(&format!(
            "- [{}](#{anchor}) ({}件)\n",
            group.name,
            group.repos.len()
        ));
    }
    toc
}

fn build_group_sections<F>(
    groups: &[RepoGroup<'_>],
    owner: &str,
    total_repos: usize,
    resolved_repos: &mut usize,
    resolve_repo_url: &mut F,
) -> String
where
    F: FnMut(&str, &str) -> String,
{
    let mut sections = String::new();

    for (idx, group) in groups.iter().enumerate() {
        let anchor = group_anchor(group.name);
        sections.push_str(&format!("<a id=\"{anchor}\"></a>\n\n"));
        sections.push_str(&format!("## {}\n\n", group.name));
        for repo in &group.repos {
            *resolved_repos += 1;
            println!(
                "[url-resolve] ({resolved_repos}/{total_repos}) start: {owner}/{}",
                repo.name
            );
            let url = resolve_repo_url(owner, &repo.name);
            println!(
                "[url-resolve] ({resolved_repos}/{total_repos}) done: {owner}/{} -> {url}",
                repo.name
            );
            sections.push_str(&format!("### [{}]({})\n\n", repo.name, url));

            if repo.desc_short.is_empty() {
                sections.push_str("（説明なし）\n\n");
            } else {
                sections.push_str(&format!("{}\n\n", repo.desc_short));
            }

            if !repo.desc_long.is_empty() {
                sections.push_str(&format!("{}\n\n", repo.desc_long));
            }

            if !repo.tags.is_empty() {
                let tag_str = repo
                    .tags
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(" ");
                sections.push_str(&format!("タグ: {tag_str}\n\n"));
            }
        }

        if idx + 1 < groups.len() {
            sections.push_str("---\n\n");
        }
    }

    sections
}

fn apply_template(template: &str, values: &[(&str, &str)]) -> String {
    static PLACEHOLDER_REGEX: OnceLock<Regex> = OnceLock::new();

    let lookup: HashMap<_, _> = values.iter().copied().collect();
    PLACEHOLDER_REGEX
        .get_or_init(|| {
            Regex::new(r"\{\{([A-Z_]+)\}\}")
                .expect("failed to compile template placeholder regex pattern")
        })
        .replace_all(template, |captures: &Captures<'_>| {
            let placeholder_name = captures.get(1).map_or("", |name| name.as_str());
            let fallback = captures.get(0).map_or("", |full| full.as_str());
            lookup
                .get(placeholder_name)
                .copied()
                .unwrap_or(fallback)
                .to_string()
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::{build_markdown, template_data_index_link, template_front_matter_title};
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
        let data_index_link_pos = markdown.find(template_data_index_link()).unwrap();
        let overview_pos = markdown.find("## 概要").unwrap();
        let beta_pos = markdown.find("## beta").unwrap();
        let alpha_pos = markdown.find("## alpha").unwrap();
        let gamma_pos = markdown.find("## gamma").unwrap();
        let etc_pos = markdown.find("## etc").unwrap();
        let stub_pos = markdown.find("## stub").unwrap();

        assert!(toc_pos < data_index_link_pos);
        assert!(data_index_link_pos < overview_pos);
        assert!(beta_pos < alpha_pos);
        assert!(alpha_pos < gamma_pos);
        assert!(gamma_pos < etc_pos);
        assert!(etc_pos < stub_pos);

        assert!(markdown.contains("- [beta](#group-beta) (2件)"));
        assert!(markdown.contains("- [etc](#group-etc) (3件)"));
        assert!(markdown.contains("- [stub](#group-stub) (1件)"));
        assert!(markdown.contains(template_data_index_link()));
        assert!(markdown.contains("<a id=\"group-etc\"></a>"));
        assert!(markdown.contains("<a id=\"group-stub\"></a>"));
        assert!(markdown.contains("（説明なし）\n\n---\n\n<a id=\"group-alpha\"></a>"));
        assert!(!markdown.ends_with("---\n\n"));
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
            "---\ntitle: \"{}\"\nhatena_entry_id: \"\"\n---\n\n",
            template_front_matter_title()
        )));
        assert!(markdown.contains("someoneのGitHubリポジトリをグループ別に一覧化したものです。"));
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
