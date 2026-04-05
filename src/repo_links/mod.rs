mod page_analysis;
mod url_cache;

use chrono::Local;
use page_analysis::{detect_pages_source_kind_from_artifacts, PagesSourceKind};
use reqwest::{
    blocking::{Client, Response},
    header::ACCEPT,
    redirect::Policy,
};
use std::time::Duration;
use url_cache::UrlCache;

const EXPLICIT_INDEX_FILE_NAMES: [&str; 4] =
    ["index.html", "index.htm", "index.md", "index.markdown"];
const LOCALIZED_README_HTML_PATH: &str = "README.ja.html";
const LOCALIZED_README_MARKDOWN_PATH: &str = "README.ja.md";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub struct RepoLinkResolver {
    client: Client,
    cache: Option<UrlCache>,
}

impl RepoLinkResolver {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .redirect(Policy::limited(10))
            .timeout(Duration::from_secs(10))
            .user_agent(USER_AGENT)
            .build()?;

        Ok(Self {
            client,
            cache: load_url_cache(),
        })
    }

    pub fn resolve_preferred_repo_url(&mut self, owner: &str, repo_name: &str) -> String {
        if let Some(url) = self
            .cache
            .as_ref()
            .and_then(|cache| cache.get(owner, repo_name))
        {
            return url.to_string();
        }

        let resolved = self.resolve_preferred_repo_url_uncached(owner, repo_name);

        if let Some(cache) = self.cache.as_mut() {
            if let Err(err) = cache.insert(owner, repo_name, resolved.clone()) {
                eprintln!("[url-cache] failed to persist cache: {err}");
            }
        }

        resolved
    }

    fn resolve_preferred_repo_url_uncached(&self, owner: &str, repo_name: &str) -> String {
        let repo_top_url = get_repo_top_url(owner, repo_name);
        let pages_url = get_pages_fallback_url(owner, repo_name);
        let localized_readme_url =
            get_github_blob_head_url(owner, repo_name, LOCALIZED_README_MARKDOWN_PATH);
        let pages_html = self.fetch_text(&pages_url, "text/html,application/xhtml+xml");
        let localized_readme_markdown = self.fetch_text(
            &get_raw_github_head_url(owner, repo_name, LOCALIZED_README_MARKDOWN_PATH),
            "text/plain,text/markdown,*/*",
        );

        let has_localized_pages = localized_readme_markdown.as_ref().is_some_and(|_| {
            self.url_exists(
                &get_pages_localized_readme_url(owner, repo_name),
                "text/html,application/xhtml+xml",
            )
        });

        let Some(pages_html) = pages_html else {
            return if has_localized_pages {
                get_pages_localized_readme_url(owner, repo_name)
            } else if localized_readme_markdown.is_some() {
                localized_readme_url
            } else {
                repo_top_url
            };
        };

        let Some(localized_readme_markdown) = localized_readme_markdown else {
            return pages_url;
        };

        let readme_markdown = self.fetch_text(
            &get_raw_github_head_url(owner, repo_name, "README.md"),
            "text/plain,text/markdown,*/*",
        );

        resolve_repo_target_from_artifacts(RepoTargetArtifacts {
            owner,
            repo_name,
            pages_html: Some(&pages_html),
            has_localized_pages,
            has_explicit_index_page: self.has_explicit_index_page(owner, repo_name),
            readme_markdown: readme_markdown.as_deref(),
            localized_readme_markdown: Some(&localized_readme_markdown),
        })
    }

    fn has_explicit_index_page(&self, owner: &str, repo_name: &str) -> bool {
        EXPLICIT_INDEX_FILE_NAMES.iter().any(|path| {
            self.url_exists(
                &get_raw_github_head_url(owner, repo_name, path),
                "text/plain,*/*",
            )
        })
    }

    fn fetch_text(&self, url: &str, accept: &str) -> Option<String> {
        self.fetch_response(url, false, accept)
            .and_then(|response| response.text().ok())
    }

    fn url_exists(&self, url: &str, accept: &str) -> bool {
        self.fetch_response(url, true, accept).is_some()
    }

    fn fetch_response(&self, url: &str, head_only: bool, accept: &str) -> Option<Response> {
        let request = if head_only {
            self.client.head(url).header(ACCEPT, accept)
        } else {
            self.client.get(url).header(ACCEPT, accept)
        };

        match request.send() {
            Ok(response) if response.status().is_success() => Some(response),
            _ => None,
        }
    }
}

fn load_url_cache() -> Option<UrlCache> {
    let today = Local::now().date_naive();
    match crate::paths::url_cache_path().and_then(|path| UrlCache::load(path, today)) {
        Ok(cache) => Some(cache),
        Err(err) => {
            eprintln!("[url-cache] disabled: {err}");
            None
        }
    }
}

pub(super) fn get_repo_top_url(owner: &str, repo_name: &str) -> String {
    format!(
        "https://github.com/{}/{}",
        encode_path_segment(owner),
        encode_path_segment(repo_name)
    )
}

fn get_pages_fallback_url(owner: &str, repo_name: &str) -> String {
    if repo_name.eq_ignore_ascii_case(&format!("{owner}.github.io")) {
        return format!("https://{owner}.github.io/");
    }

    format!(
        "https://{owner}.github.io/{}/",
        encode_path_segment(repo_name)
    )
}

fn get_pages_localized_readme_url(owner: &str, repo_name: &str) -> String {
    format!(
        "{}{}",
        get_pages_fallback_url(owner, repo_name),
        LOCALIZED_README_HTML_PATH
    )
}

pub(super) fn get_github_blob_head_url(owner: &str, repo_name: &str, path: &str) -> String {
    format!(
        "https://github.com/{}/{}/blob/HEAD/{}",
        encode_path_segment(owner),
        encode_path_segment(repo_name),
        encode_path(path)
    )
}

fn get_raw_github_head_url(owner: &str, repo_name: &str, path: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/HEAD/{}",
        encode_path_segment(owner),
        encode_path_segment(repo_name),
        encode_path(path)
    )
}

struct RepoTargetArtifacts<'a> {
    owner: &'a str,
    repo_name: &'a str,
    pages_html: Option<&'a str>,
    has_localized_pages: bool,
    has_explicit_index_page: bool,
    readme_markdown: Option<&'a str>,
    localized_readme_markdown: Option<&'a str>,
}

fn resolve_repo_target_from_artifacts(artifacts: RepoTargetArtifacts<'_>) -> String {
    let repo_top_url = get_repo_top_url(artifacts.owner, artifacts.repo_name);
    let pages_url = get_pages_fallback_url(artifacts.owner, artifacts.repo_name);
    let localized_pages_url = get_pages_localized_readme_url(artifacts.owner, artifacts.repo_name);
    let localized_readme_url = get_github_blob_head_url(
        artifacts.owner,
        artifacts.repo_name,
        LOCALIZED_README_MARKDOWN_PATH,
    );

    if artifacts.has_localized_pages {
        return localized_pages_url;
    }

    if let Some(pages_html) = artifacts.pages_html {
        if artifacts.localized_readme_markdown.is_some()
            && detect_pages_source_kind_from_artifacts(
                artifacts.owner,
                artifacts.repo_name,
                &pages_url,
                pages_html,
                artifacts.has_explicit_index_page,
                artifacts.readme_markdown,
                artifacts.localized_readme_markdown,
            ) == PagesSourceKind::ReadmeMd
        {
            return localized_readme_url;
        }

        return pages_url;
    }

    if artifacts.localized_readme_markdown.is_some() {
        localized_readme_url
    } else {
        repo_top_url
    }
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_path_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_path_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        get_github_blob_head_url, get_pages_fallback_url, get_pages_localized_readme_url,
        get_repo_top_url, resolve_repo_target_from_artifacts, RepoTargetArtifacts,
    };

    #[test]
    fn keeps_github_pages_when_explicit_index_exists() {
        let owner = "cat2151";
        let repo_name = "own-repos-curator-data";

        assert_eq!(
            resolve_repo_target_from_artifacts(RepoTargetArtifacts {
                owner,
                repo_name,
                pages_html: Some(
                    "<!DOCTYPE html><html lang=\"ja\"><head><title>repositories</title></head><body><div id=\"app\"></div></body></html>"
                ),
                has_localized_pages: false,
                has_explicit_index_page: true,
                readme_markdown: Some(
                    "# own-repos-curator-data\n\nThis is a static site for visualizing repos.json."
                ),
                localized_readme_markdown: Some(
                    "# own-repos-curator-data\n\n`repos.json` を可視化する静的サイトです。"
                ),
            }),
            get_pages_fallback_url(owner, repo_name)
        );
    }

    #[test]
    fn detects_readme_md_derived_pages_from_edit_link() {
        let owner = "cat2151";
        let repo_name = "cat-self-update";

        assert_eq!(
            resolve_repo_target_from_artifacts(RepoTargetArtifacts {
                owner,
                repo_name,
                pages_html: Some(
                    "<!DOCTYPE html><html lang=\"en-US\"><body><div class=\"markdown-body\"><h1>cat-self-update</h1><p>Currently dogfooding.</p><a href=\"https://github.com/cat2151/cat-self-update/edit/main/README.md\">Improve this page</a></div></body></html>"
                ),
                has_localized_pages: false,
                has_explicit_index_page: false,
                readme_markdown: Some("# cat-self-update\n\n## Status\nCurrently dogfooding."),
                localized_readme_markdown: Some(
                    "# cat-self-update\n\n## 状況\nドッグフーディング中です。"
                ),
            }),
            get_github_blob_head_url(owner, repo_name, "README.ja.md")
        );
    }

    #[test]
    fn detects_readme_md_derived_custom_theme_pages_by_content() {
        let owner = "cat2151";
        let repo_name = "claude-chat-code";

        assert_eq!(
            resolve_repo_target_from_artifacts(RepoTargetArtifacts {
                owner,
                repo_name,
                pages_html: Some(
                    "<!DOCTYPE html><html lang=\"en-US\"><body><div id=\"header_wrap\"><a href=\"https://github.com/cat2151/claude-chat-code\">View on GitHub</a></div><section id=\"main_content\"><h1>claude-chat-code</h1><p>A Windows TUI that monitors for zip file downloads from Claude chat, then automatically builds and launches the code. Written in Rust.</p><h2>Installation</h2><p>Rust is required.</p><pre><code>cargo install --force --git https://github.com/cat2151/claude-chat-code</code></pre><h2>Challenges and Solutions</h2><p>When generating or modifying code with Claude chat, the following steps were traditionally required every time:</p><ol><li>Download the zip from Claude chat.</li><li>Back up the working directory.</li><li>Delete old files.</li></ol></section></body></html>"
                ),
                has_localized_pages: false,
                has_explicit_index_page: false,
                readme_markdown: Some(
                    "# claude-chat-code\n\nA Windows TUI that monitors for zip file downloads from Claude chat, then automatically builds and launches the code. Written in Rust.\n\n## Installation\n\nRust is required.\n\n```powershell\ncargo install --force --git https://github.com/cat2151/claude-chat-code\n```\n\n## Challenges and Solutions\n\nWhen generating or modifying code with Claude chat, the following steps were traditionally required every time:\n\n1. Download the zip from Claude chat.\n2. Back up the working directory.\n3. Delete old files."
                ),
                localized_readme_markdown: Some(
                    "# claude-chat-code\n\nClaude chat からzipダウンロードしたか監視して自動ビルドと起動をする Windows 用 TUI 。Rustで書かれています。\n\n## インストール\n\nRustが必要です。"
                ),
            }),
            get_github_blob_head_url(owner, repo_name, "README.ja.md")
        );
    }

    #[test]
    fn falls_back_to_repo_top_when_pages_and_localized_readme_are_missing() {
        assert_eq!(
            resolve_repo_target_from_artifacts(RepoTargetArtifacts {
                owner: "cat2151",
                repo_name: "unknown-repo",
                pages_html: None,
                has_localized_pages: false,
                has_explicit_index_page: false,
                readme_markdown: None,
                localized_readme_markdown: None,
            }),
            get_repo_top_url("cat2151", "unknown-repo")
        );
    }

    #[test]
    fn prefers_localized_github_pages_when_available() {
        let owner = "cat2151";
        let repo_name = "cat-self-update";

        assert_eq!(
            resolve_repo_target_from_artifacts(RepoTargetArtifacts {
                owner,
                repo_name,
                pages_html: Some(
                    "<!DOCTYPE html><html lang=\"en-US\"><body><div class=\"markdown-body\"><h1>cat-self-update</h1></div></body></html>"
                ),
                has_localized_pages: true,
                has_explicit_index_page: false,
                readme_markdown: Some("# cat-self-update\n\n## Status\nCurrently dogfooding."),
                localized_readme_markdown: Some(
                    "# cat-self-update\n\n## 状況\nドッグフーディング中です。"
                ),
            }),
            get_pages_localized_readme_url(owner, repo_name)
        );
    }
}
