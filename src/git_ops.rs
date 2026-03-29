use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitOpResult {
    pub success: bool,
    pub message: String,
}

pub fn stage_all(repo_path: &str) -> GitOpResult {
    run_git(repo_path, &["add", "-A"])
}

pub fn commit(repo_path: &str, message: &str) -> GitOpResult {
    run_git(repo_path, &["commit", "-m", message])
}

pub fn push(repo_path: &str) -> GitOpResult {
    run_git(repo_path, &["push"])
}

pub fn quick_push(repo_path: &str, message: &str) -> GitOpResult {
    let stage = stage_all(repo_path);
    if !stage.success {
        return stage;
    }

    let commit_result = commit(repo_path, message);
    if !commit_result.success {
        // "nothing to commit" is not really a failure for quick push
        if commit_result.message.contains("nothing to commit") {
            return GitOpResult {
                success: true,
                message: "Nothing to commit, pushing anyway...".to_string(),
            };
        }
        return commit_result;
    }

    let push_result = push(repo_path);
    if !push_result.success {
        return push_result;
    }

    GitOpResult {
        success: true,
        message: format!("Staged, committed, and pushed: \"{}\"", message),
    }
}

fn run_git(repo_path: &str, args: &[&str]) -> GitOpResult {
    match Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if stdout.is_empty() {
                stderr.clone()
            } else {
                stdout.clone()
            };
            // Trim to first line for display
            let first_line = combined.lines().next().unwrap_or("").to_string();

            GitOpResult {
                success: output.status.success(),
                message: first_line,
            }
        }
        Err(e) => GitOpResult {
            success: false,
            message: format!("Failed to run git: {}", e),
        },
    }
}
