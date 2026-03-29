use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use git2::{Repository, StatusOptions};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GitStatus {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub modified: usize,
    pub staged: usize,
    pub untracked: usize,
    pub ahead: usize,
    pub behind: usize,
    pub last_commit_msg: String,
    pub last_commit_age: String,
    pub clean: bool,
    pub error: Option<String>,
}

pub fn scan_repo(name: &str, path: &str) -> GitStatus {
    match scan_repo_inner(path) {
        Ok(mut status) => {
            status.name = name.to_string();
            status.path = path.to_string();
            status
        }
        Err(e) => GitStatus {
            name: name.to_string(),
            path: path.to_string(),
            branch: String::new(),
            modified: 0,
            staged: 0,
            untracked: 0,
            ahead: 0,
            behind: 0,
            last_commit_msg: String::new(),
            last_commit_age: String::new(),
            clean: false,
            error: Some(e.to_string()),
        },
    }
}

fn scan_repo_inner(path: &str) -> Result<GitStatus> {
    let repo = Repository::open(Path::new(path))?;

    // Branch name
    let branch = match repo.head() {
        Ok(head) => head
            .shorthand()
            .unwrap_or("detached")
            .to_string(),
        Err(_) => "no commits".to_string(),
    };

    // File statuses
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false);
    let statuses = repo.statuses(Some(&mut opts))?;

    let mut modified = 0;
    let mut staged = 0;
    let mut untracked = 0;

    for entry in statuses.iter() {
        let s = entry.status();
        if s.intersects(
            git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE,
        ) {
            modified += 1;
        }
        if s.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE,
        ) {
            staged += 1;
        }
        if s.intersects(git2::Status::WT_NEW) {
            untracked += 1;
        }
    }

    // Last commit
    let (last_commit_msg, last_commit_age) = match repo.head() {
        Ok(head) => {
            let commit = head.peel_to_commit()?;
            let msg = commit
                .summary()
                .unwrap_or("")
                .to_string();
            let time = commit.time();
            let age = format_commit_age(time.seconds());
            (msg, age)
        }
        Err(_) => ("no commits".to_string(), String::new()),
    };

    // Ahead/behind
    let (ahead, behind) = match get_ahead_behind(&repo) {
        Ok(ab) => ab,
        Err(_) => (0, 0),
    };

    let clean = modified == 0 && staged == 0 && untracked == 0;

    Ok(GitStatus {
        name: String::new(),
        path: String::new(),
        branch,
        modified,
        staged,
        untracked,
        ahead,
        behind,
        last_commit_msg,
        last_commit_age,
        clean,
        error: None,
    })
}

fn get_ahead_behind(repo: &Repository) -> Result<(usize, usize)> {
    let head = repo.head()?;
    let local_oid = head.target().ok_or_else(|| anyhow::anyhow!("no HEAD target"))?;

    let branch_name = head
        .shorthand()
        .ok_or_else(|| anyhow::anyhow!("no branch name"))?;

    let upstream_name = format!("refs/remotes/origin/{}", branch_name);
    let upstream_ref = repo.find_reference(&upstream_name)?;
    let upstream_oid = upstream_ref
        .target()
        .ok_or_else(|| anyhow::anyhow!("no upstream target"))?;

    let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
    Ok((ahead, behind))
}

fn format_commit_age(epoch_secs: i64) -> String {
    let commit_time = match Local.timestamp_opt(epoch_secs, 0) {
        chrono::LocalResult::Single(t) => t,
        _ => return "unknown".to_string(),
    };
    let now: DateTime<Local> = Local::now();
    let duration = now.signed_duration_since(commit_time);

    if duration.num_days() > 30 {
        format!("{}mo ago", duration.num_days() / 30)
    } else if duration.num_days() > 0 {
        format!("{}d ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m ago", duration.num_minutes())
    } else {
        "just now".to_string()
    }
}
