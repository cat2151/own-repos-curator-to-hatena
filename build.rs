fn git_output(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn git_path(path: &str) -> Option<std::path::PathBuf> {
    git_output(&["rev-parse", "--git-path", path]).map(std::path::PathBuf::from)
}

fn main() {
    let hash = git_output(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_COMMIT_HASH={hash}");

    if let Some(head_path) = git_path("HEAD") {
        println!("cargo:rerun-if-changed={}", head_path.display());

        if let Ok(head) = std::fs::read_to_string(&head_path) {
            if let Some(ref_path_str) = head.trim().strip_prefix("ref: ") {
                let ref_path = std::path::Path::new(ref_path_str);
                if !ref_path.as_os_str().is_empty()
                    && ref_path
                        .components()
                        .all(|component| matches!(component, std::path::Component::Normal(_)))
                {
                    if let Some(ref_watch_path) = git_path(ref_path_str) {
                        println!("cargo:rerun-if-changed={}", ref_watch_path.display());
                    }
                }
            }
        }
    }

    if let Some(packed_refs_path) = git_path("packed-refs") {
        println!("cargo:rerun-if-changed={}", packed_refs_path.display());
    }
}
