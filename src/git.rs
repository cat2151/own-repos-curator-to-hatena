use anyhow::{anyhow, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn managed_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("failed to resolve local data dir"))?;
    Ok(base.join("own-repos-curator-to-hatena").join("repos"))
}

pub fn ensure_managed_clone(repo: &str) -> Result<PathBuf> {
    let base = managed_dir()?;
    std::fs::create_dir_all(&base)
        .with_context(|| format!("failed to create dir: {}", base.display()))?;

    let repo_name = repo.split('/').next_back().unwrap_or(repo);
    let repo_dir = base.join(repo_name);

    if !repo_dir.exists() {
        let clone_target = repo_dir.to_string_lossy().to_string();
        let output = Command::new("gh")
            .args(["repo", "clone", repo, &clone_target])
            .output()
            .with_context(|| format!("failed to clone {repo}"))?;
        ensure_success("gh repo clone", &output)?;
    }

    if !repo_dir.join(".git").exists() {
        return Err(anyhow!(
            "clone path is not a git repo: {}",
            repo_dir.display()
        ));
    }

    Ok(repo_dir)
}

pub fn run<const N: usize>(repo_dir: &Path, args: [&str; N]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .with_context(|| format!("failed to run: git {}", args.join(" ")))?;
    ensure_success(&format!("git {}", args.join(" ")), &output)
}

pub fn has_changes(repo_dir: &Path, path: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--", path])
        .current_dir(repo_dir)
        .output()
        .context("failed to run git status")?;
    ensure_success("git status", &output)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

fn ensure_success(command: &str, output: &std::process::Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        return Err(anyhow!("{command} failed with status {}", output.status));
    }
    Err(anyhow!("{command} failed: {stderr}"))
}
