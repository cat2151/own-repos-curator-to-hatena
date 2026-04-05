mod convert;
mod git;
mod model;
mod paths;
mod repo_links;

use anyhow::{Context, Result};
use cat_self_update_lib::{check_remote_commit, self_update};
use clap::{Parser, Subcommand};
use std::{fs, io::ErrorKind, path::PathBuf, process::Command};

const BUILD_COMMIT_HASH: &str = env!("BUILD_COMMIT_HASH");
const REPO_OWNER: &str = "cat2151";
const REPO_NAME: &str = "own-repos-curator-to-hatena";
const MAIN_BRANCH: &str = "main";
const HATENA_REPO: &str = "cat2151/cat2151-hatena-blog-contents";
const POST_FILE: &str = "posts/own-repos-curator.md";

#[derive(Parser)]
#[command(name = "own-repos-curator-to-hatena")]
#[command(about = "Publish own-repos-curator output to the Hatena contents repository")]
struct Cli {
    #[arg(long, global = true, help = "Write markdown locally without pushing")]
    dry_run: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Self-update the application from GitHub
    Update,
    /// Print the build-time commit hash
    Hash,
    /// Compare the build-time commit hash with the remote main branch
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Update) => run_self_update(),
        Some(Commands::Hash) => {
            println!("{BUILD_COMMIT_HASH}");
            Ok(())
        }
        Some(Commands::Check) => run_check(),
        None => run_publish(cli.dry_run),
    }
}

fn run_publish(dry_run: bool) -> Result<()> {
    let json_path = paths::repos_json_path()?;
    let json = fs::read_to_string(&json_path)
        .with_context(|| format!("failed to read {}", json_path.display()))?;
    let data: model::RepoData =
        serde_json::from_str(&json).context("failed to parse repos.json")?;
    let owner = data
        .meta
        .owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .unwrap_or(REPO_OWNER);
    let mut existing_markdown = None;

    if !dry_run {
        ensure_gh_auth()?;
    }

    let repo_dir = if dry_run {
        None
    } else {
        let repo_dir = git::ensure_managed_clone(HATENA_REPO)?;
        git::run(&repo_dir, ["pull", "--ff-only"])?;

        let post_path = repo_dir.join(POST_FILE);
        existing_markdown = match fs::read_to_string(&post_path) {
            Ok(markdown) => Some(markdown),
            Err(err) if err.kind() == ErrorKind::NotFound => None,
            Err(err) => {
                return Err(err).with_context(|| format!("failed to read {}", post_path.display()));
            }
        };
        Some(repo_dir)
    };

    let mut link_resolver =
        repo_links::RepoLinkResolver::new().context("failed to initialize repo link resolver")?;

    let markdown = convert::build_markdown(
        &data,
        owner,
        existing_markdown.as_deref(),
        |owner, repo_name| link_resolver.resolve_preferred_repo_url(owner, repo_name),
    );

    if dry_run {
        let out_path = PathBuf::from("own-repos-curator.md");
        fs::write(&out_path, &markdown)
            .with_context(|| format!("failed to write {}", out_path.display()))?;
        println!("[dry-run] written: {}", out_path.display());
        return Ok(());
    }

    let repo_dir = repo_dir.expect("repo_dir should exist when dry_run is false");

    let post_path = repo_dir.join(POST_FILE);
    if let Some(parent) = post_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&post_path, &markdown)
        .with_context(|| format!("failed to write {}", post_path.display()))?;

    if !git::has_changes(&repo_dir, POST_FILE)? {
        println!("no changes, skipping push");
        return Ok(());
    }

    let today = chrono::Utc::now().date_naive().to_string();
    let message = format!("chore: update own-repos-curator ({today})");
    git::run(&repo_dir, ["add", "--", POST_FILE])?;
    git::run(&repo_dir, ["commit", "-m", &message])?;
    git::run(&repo_dir, ["push"])?;

    println!("pushed: {POST_FILE}");
    Ok(())
}

fn run_self_update() -> Result<()> {
    self_update(REPO_OWNER, REPO_NAME, &["own-repos-curator-to-hatena"])
        .map_err(|err| anyhow::anyhow!("self update failed: {err}"))
}

fn run_check() -> Result<()> {
    let result = check_remote_commit(REPO_OWNER, REPO_NAME, MAIN_BRANCH, BUILD_COMMIT_HASH)
        .map_err(|err| anyhow::anyhow!("check failed: {err}"))?;
    println!("{result}");
    Ok(())
}

fn ensure_gh_auth() -> Result<()> {
    let output = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .context("failed to run `gh auth status`")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!("gh auth status failed: {stderr}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn parses_without_subcommand() {
        let cli = Cli::parse_from(["own-repos-curator-to-hatena"]);
        assert!(!cli.dry_run);
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_dry_run_without_subcommand() {
        let cli = Cli::parse_from(["own-repos-curator-to-hatena", "--dry-run"]);
        assert!(cli.dry_run);
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_update_subcommand() {
        let cli = Cli::parse_from(["own-repos-curator-to-hatena", "update"]);
        assert!(!cli.dry_run);
        assert!(matches!(cli.command, Some(Commands::Update)));
    }
}
