pub(super) fn extract_hatena_entry_id(existing_markdown: Option<&str>) -> Option<String> {
    let markdown = existing_markdown?;
    let mut lines = markdown.lines();
    if lines.next()? != "---" {
        return None;
    }

    let mut has_title = false;
    let mut hatena_entry_id: Option<String> = None;

    for line in lines {
        let trimmed_line = line.trim();

        if trimmed_line == "---" {
            return if has_title {
                match hatena_entry_id {
                    Some(value) if !value.is_empty() => Some(value),
                    _ => None,
                }
            } else {
                None
            };
        }

        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            continue;
        }

        let Some(separator_index) = trimmed_line.find(':') else {
            continue;
        };
        let key = trimmed_line[..separator_index].trim();
        let value = trimmed_line[separator_index + 1..].trim();
        match key {
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
    if value.starts_with('"') {
        if value.len() < 2 || !value.ends_with('"') {
            return None;
        }

        let mut out = String::new();
        let mut chars = value[1..value.len() - 1].chars();

        while let Some(ch) = chars.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }

            match chars.next()? {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
        }

        Some(out)
    } else {
        None
    }
}

pub(super) fn escape_yaml_double_quoted(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use crate::convert::{build_markdown, FRONT_MATTER_TITLE};
    use crate::model::{Meta, Repo, RepoData};

    #[test]
    fn preserves_existing_hatena_entry_id_from_yaml_front_matter() {
        let data = sample_data();

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
        let data = sample_data();

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

    #[test]
    fn preserves_entry_id_with_comments_and_blank_lines() {
        let data = sample_data();

        let existing_markdown = r#"---
# comment

title: "old title"
hatena_entry_id: "12345678901234567890"
---
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
    fn preserves_entry_id_with_yaml_sequence_items_in_front_matter() {
        let data = sample_data();

        let existing_markdown = r#"---
title: "old title"
tags:
  - rust
  - cli
hatena_entry_id: "12345678901234567890"
---
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
    fn rejects_invalid_quoted_entry_id() {
        let data = sample_data();

        let existing_markdown = r#"---
title: "old title"
hatena_entry_id: "12345678901234567890
---
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

    #[test]
    fn preserves_escaped_hatena_entry_id_from_yaml_front_matter() {
        let data = sample_data();

        let existing_markdown = r#"---
title: "old title"
hatena_entry_id: "line1\nline2\t\"quoted\"\\tail"
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
            "---\ntitle: \"{FRONT_MATTER_TITLE}\"\nhatena_entry_id: \"line1\\nline2\\t\\\"quoted\\\"\\\\tail\"\n---\n\n"
        )));
    }

    #[test]
    fn rejects_unquoted_hatena_entry_id() {
        let data = sample_data();

        let existing_markdown = r#"---
title: "old title"
hatena_entry_id: 12345678901234567890 # inline comment
---
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

    #[test]
    fn rejects_unquoted_yaml_scalars_without_inline_comments() {
        assert_eq!(super::parse_yaml_scalar("12345678901234567890"), None);
    }

    #[test]
    fn rejects_unquoted_yaml_scalars_with_inline_comments() {
        assert_eq!(
            super::parse_yaml_scalar("12345678901234567890 # inline comment"),
            None
        );
    }

    #[test]
    fn escapes_yaml_double_quoted_special_characters() {
        assert_eq!(
            super::escape_yaml_double_quoted("line1\nline2\t\"quoted\"\\tail"),
            "line1\\nline2\\t\\\"quoted\\\"\\\\tail"
        );
    }

    fn sample_data() -> RepoData {
        RepoData {
            meta: Meta {
                github_desc_updated_at: "2026-04-05".into(),
                last_json_commit_push_date: "2026-04-05".into(),
                owner: Some("someone".into()),
            },
            registered_tags: vec![],
            registered_groups: vec!["tools".into()],
            repos: vec![repo("tool-1", "tools")],
        }
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
