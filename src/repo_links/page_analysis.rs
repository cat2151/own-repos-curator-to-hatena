use regex::{Captures, Regex};
use reqwest::Url;
use std::{collections::HashSet, sync::OnceLock};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PagesSourceKind {
    ReadmeMd,
    ReadmeJa,
    Other,
}

#[derive(Debug, Clone)]
struct MatchScore {
    matched_chars: usize,
    match_count: usize,
    ratio: f64,
}

pub(super) fn detect_pages_source_kind_from_artifacts(
    owner: &str,
    repo_name: &str,
    pages_url: &str,
    pages_html: &str,
    has_explicit_index_page: bool,
    readme_markdown: Option<&str>,
    localized_readme_markdown: Option<&str>,
) -> PagesSourceKind {
    if has_explicit_index_page {
        return PagesSourceKind::Other;
    }

    detect_pages_source_from_html(pages_html, owner, repo_name, pages_url)
        .or_else(|| {
            detect_pages_source_by_content(pages_html, readme_markdown, localized_readme_markdown)
        })
        .unwrap_or(PagesSourceKind::Other)
}

fn detect_pages_source_from_html(
    html: &str,
    owner: &str,
    repo_name: &str,
    pages_url: &str,
) -> Option<PagesSourceKind> {
    extract_hrefs_from_html(html)
        .into_iter()
        .find_map(|href| detect_pages_source_from_href(&href, owner, repo_name, pages_url))
}

fn detect_pages_source_from_href(
    href: &str,
    owner: &str,
    repo_name: &str,
    base_url: &str,
) -> Option<PagesSourceKind> {
    let url = resolve_url(href, base_url)?;
    if !url.host_str()?.eq_ignore_ascii_case("github.com") {
        return None;
    }

    let segments: Vec<_> = url.path_segments()?.collect();
    if segments.len() < 5 {
        return None;
    }

    if !segments[0].eq_ignore_ascii_case(owner)
        || !segments[1].eq_ignore_ascii_case(repo_name)
        || !segments[2].eq_ignore_ascii_case("edit")
    {
        return None;
    }

    let last = *segments.last()?;
    let source_kind = if last.eq_ignore_ascii_case("README.ja.md") {
        PagesSourceKind::ReadmeJa
    } else if last.eq_ignore_ascii_case("README.md") {
        PagesSourceKind::ReadmeMd
    } else {
        return None;
    };

    let mut tail = &segments[3..segments.len() - 1];
    if let Some(last_dir) = tail.last() {
        if last_dir.eq_ignore_ascii_case(".github") || last_dir.eq_ignore_ascii_case("docs") {
            tail = &tail[..tail.len() - 1];
        }
    }

    if tail.is_empty() {
        None
    } else {
        Some(source_kind)
    }
}

fn resolve_url(href: &str, base_url: &str) -> Option<Url> {
    Url::parse(base_url).ok()?.join(href).ok()
}

fn detect_pages_source_by_content(
    html: &str,
    readme_markdown: Option<&str>,
    localized_readme_markdown: Option<&str>,
) -> Option<PagesSourceKind> {
    let page_text = extract_primary_page_text(html);
    let readme_md_score = score_page_text_against_markdown(&page_text, readme_markdown);
    let readme_ja_score = score_page_text_against_markdown(&page_text, localized_readme_markdown);

    if readme_md_score.match_count >= 3
        && readme_md_score.ratio >= 0.45
        && readme_md_score.matched_chars
            >= 160.max(scale_by_one_and_half(readme_ja_score.matched_chars))
    {
        return Some(PagesSourceKind::ReadmeMd);
    }

    if readme_ja_score.match_count >= 3
        && readme_ja_score.ratio >= 0.45
        && readme_ja_score.matched_chars
            >= 160.max(scale_by_one_and_half(readme_md_score.matched_chars))
    {
        return Some(PagesSourceKind::ReadmeJa);
    }

    None
}

fn score_page_text_against_markdown(page_text: &str, markdown: Option<&str>) -> MatchScore {
    let Some(markdown) = markdown else {
        return MatchScore {
            matched_chars: 0,
            match_count: 0,
            ratio: 0.0,
        };
    };

    let page = normalize_comparable_text(page_text);
    let candidate_lines = markdown_to_comparable_lines(markdown);
    if candidate_lines.is_empty() {
        return MatchScore {
            matched_chars: 0,
            match_count: 0,
            ratio: 0.0,
        };
    }

    let mut matched_chars = 0;
    let mut match_count = 0;
    let mut total_chars = 0;

    for line in candidate_lines {
        total_chars += line.len();
        if page.contains(&line) {
            matched_chars += line.len();
            match_count += 1;
        }
    }

    MatchScore {
        matched_chars,
        match_count,
        ratio: if total_chars == 0 {
            0.0
        } else {
            matched_chars as f64 / total_chars as f64
        },
    }
}

fn markdown_to_comparable_lines(markdown: &str) -> Vec<String> {
    let mut inside_fence = false;
    let mut lines = Vec::new();
    let mut seen = HashSet::new();

    for raw_line in markdown.lines() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            inside_fence = !inside_fence;
            continue;
        }

        let mut line = image_regex().replace_all(raw_line, " $1 ").into_owned();
        line = link_regex().replace_all(&line, " $1 ").into_owned();
        line = inline_code_regex().replace_all(&line, " $1 ").into_owned();
        line = heading_prefix_regex().replace(&line, "").into_owned();
        line = blockquote_prefix_regex().replace(&line, "").into_owned();
        line = unordered_list_prefix_regex()
            .replace(&line, "")
            .into_owned();
        line = ordered_list_prefix_regex().replace(&line, "").into_owned();
        line = table_prefix_regex().replace(&line, "").into_owned();
        line = table_suffix_regex().replace(&line, "").into_owned();
        line = markdown_decorations_regex()
            .replace_all(&line, "")
            .into_owned();

        if !inside_fence && table_divider_regex().is_match(&line) {
            continue;
        }

        let line = normalize_comparable_text(&line);
        if line.len() >= 16 && seen.insert(line.clone()) {
            lines.push(line);
        }
    }

    lines.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    lines.truncate(16);
    lines
}

fn extract_primary_page_text(html: &str) -> String {
    let source = body_regex()
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|body| body.as_str())
        .unwrap_or(html);
    let without_scripts = script_regex().replace_all(source, " ");
    let without_styles = style_regex().replace_all(&without_scripts, " ");
    let without_comments = html_comment_regex().replace_all(&without_styles, " ");
    let text = html_tag_regex().replace_all(&without_comments, " ");
    decode_html_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_hrefs_from_html(html: &str) -> Vec<String> {
    href_regex()
        .captures_iter(html)
        .filter_map(|captures| {
            captures
                .get(1)
                .or_else(|| captures.get(2))
                .or_else(|| captures.get(3))
                .map(|value| decode_html_entities(value.as_str().trim()))
        })
        .collect()
}

fn decode_html_entities(value: &str) -> String {
    let mut decoded = value
        .replace("&nbsp;", " ")
        .replace("&NBSP;", " ")
        .replace("&amp;", "&")
        .replace("&AMP;", "&")
        .replace("&lt;", "<")
        .replace("&LT;", "<")
        .replace("&gt;", ">")
        .replace("&GT;", ">")
        .replace("&quot;", "\"")
        .replace("&QUOT;", "\"")
        .replace("&#39;", "'");

    decoded = numeric_hex_entity_regex()
        .replace_all(&decoded, decode_numeric_entity(16))
        .into_owned();
    numeric_decimal_entity_regex()
        .replace_all(&decoded, decode_numeric_entity(10))
        .into_owned()
}

fn decode_numeric_entity(radix: u32) -> impl Fn(&Captures<'_>) -> String {
    move |captures: &Captures<'_>| {
        captures
            .get(1)
            .and_then(|value| u32::from_str_radix(value.as_str(), radix).ok())
            .and_then(char::from_u32)
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| captures[0].to_string())
    }
}

fn normalize_comparable_text(value: &str) -> String {
    value
        .nfkc()
        .collect::<String>()
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201C}', '\u{201D}'], "\"")
        .replace('\u{00A0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn scale_by_one_and_half(value: usize) -> usize {
    (value.saturating_mul(3).saturating_add(1)) / 2
}

fn body_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<body\b[^>]*>(.*?)</body>").unwrap())
}

fn script_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<script\b[^>]*>.*?</script>").unwrap())
}

fn style_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<style\b[^>]*>.*?</style>").unwrap())
}

fn html_comment_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").unwrap())
}

fn html_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<[^>]+>").unwrap())
}

fn href_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?is)<a\b[^>]*\bhref\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).unwrap()
    })
}

fn numeric_hex_entity_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)&#x([0-9a-f]+);").unwrap())
}

fn numeric_decimal_entity_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"&#([0-9]+);").unwrap())
}

fn image_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"!\[([^\]]*)\]\([^)]+\)").unwrap())
}

fn link_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap())
}

fn inline_code_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"`([^`]*)`").unwrap())
}

fn heading_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*#{1,6}\s*").unwrap())
}

fn blockquote_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*>+\s*").unwrap())
}

fn unordered_list_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*[-*+]\s+").unwrap())
}

fn ordered_list_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*\d+\.\s+").unwrap())
}

fn table_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*\|\s*").unwrap())
}

fn table_suffix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\s*\|\s*$").unwrap())
}

fn markdown_decorations_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"[*_~]").unwrap())
}

fn table_divider_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*[:\-|]{3,}\s*$").unwrap())
}
