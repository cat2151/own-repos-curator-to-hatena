mod convert;
mod git;
mod model;

use anyhow::{Context, Result};
use std::{env, fs, path::PathBuf, process::Command};

const HATENA_REPO: &str = "cat2151/cat2151-hatena-blog-contents";
const POST_FILE: &str = "posts/own-repos-curator.md";

fn repos_json_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("failed to resolve AppData\\Local"))?;
    Ok(base.join("own-repos-curator").join("data").join("repos.json"))
}

fn main() -> Result<()> {
    let dry_run = env::args().any(|a| a == "--dry-run");

    let json_path = repos_json_path()?;
    let json = fs::read_to_string(&json_path)
        .with_context(|| format!("failed to read {}", json_path.display()))?;
    let data: model::RepoData = serde_json::from_str(&json)
        .context("failed to parse repos.json")?;

    let markdown = convert::build_markdown(&data);

    if dry_run {
        let out_path = PathBuf::from("own-repos-curator.md");
        fs::write(&out_path, &markdown)
            .with_context(|| format!("failed to write {}", out_path.display()))?;
        println!("[dry-run] written: {}", out_path.display());
        return Ok(());
    }

    ensure_gh_auth()?;

    let repo_dir = git::ensure_managed_clone(HATENA_REPO)?;
    git::run(&repo_dir, ["pull", "--ff-only"])?;

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
