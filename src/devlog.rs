use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::git_scanner::GitStatus;
use crate::remote_checker::{CheckResult, RemoteStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevLogEntry {
    pub timestamp: DateTime<Local>,
    pub project: String,
    pub event: DevLogEvent,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DevLogEvent {
    NewCommit,
    StatusClean,
    StatusDirty,
    BranchChange,
    PushDetected,
    RemoteUp,
    RemoteDown,
}

impl std::fmt::Display for DevLogEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevLogEvent::NewCommit => write!(f, "commit"),
            DevLogEvent::StatusClean => write!(f, "clean"),
            DevLogEvent::StatusDirty => write!(f, "dirty"),
            DevLogEvent::BranchChange => write!(f, "branch"),
            DevLogEvent::PushDetected => write!(f, "push"),
            DevLogEvent::RemoteUp => write!(f, "up"),
            DevLogEvent::RemoteDown => write!(f, "down"),
        }
    }
}

pub struct DevLog {
    pub path: PathBuf,
    pub entries: Vec<DevLogEntry>,
    pub max_display: usize,
}

impl DevLog {
    pub fn new(path: PathBuf, max_display: usize) -> Self {
        let entries = Self::load_file(&path, max_display);
        Self {
            path,
            entries,
            max_display,
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("devpulse")
            .join("devlog.jsonl")
    }

    fn load_file(path: &PathBuf, max: usize) -> Vec<DevLogEntry> {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);
        let all: Vec<DevLogEntry> = reader
            .lines()
            .flatten()
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();
        // Keep only the most recent entries for display
        if all.len() > max {
            all[all.len() - max..].to_vec()
        } else {
            all
        }
    }

    pub fn append(&mut self, new_entries: &[DevLogEntry]) {
        if new_entries.is_empty() {
            return;
        }
        // Ensure parent dir exists
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            for entry in new_entries {
                if let Ok(json) = serde_json::to_string(entry) {
                    let _ = writeln!(file, "{}", json);
                }
            }
        }
        // Add to in-memory buffer and trim
        self.entries.extend_from_slice(new_entries);
        if self.entries.len() > self.max_display {
            let drain = self.entries.len() - self.max_display;
            self.entries.drain(..drain);
        }
    }

    pub fn load_all(&self) -> Vec<DevLogEntry> {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);
        reader
            .lines()
            .flatten()
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect()
    }

    pub fn load_filtered(
        &self,
        project: Option<&str>,
        since: Option<NaiveDate>,
        until: Option<NaiveDate>,
    ) -> Vec<DevLogEntry> {
        self.load_all()
            .into_iter()
            .filter(|e| {
                if let Some(p) = project {
                    if !e.project.eq_ignore_ascii_case(p) {
                        return false;
                    }
                }
                let date = e.timestamp.date_naive();
                if let Some(s) = since {
                    if date < s {
                        return false;
                    }
                }
                if let Some(u) = until {
                    if date > u {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    pub fn export_markdown(
        &self,
        project: Option<&str>,
        since: Option<NaiveDate>,
        until: Option<NaiveDate>,
    ) -> String {
        let entries = self.load_filtered(project, since, until);
        if entries.is_empty() {
            return "No devlog entries found for the given filter.\n".to_string();
        }

        let mut out = String::new();
        out.push_str("# DevLog\n\n");

        if let Some(p) = project {
            out.push_str(&format!("**Project:** {}\n", p));
        }
        if let Some(s) = since {
            out.push_str(&format!("**Since:** {}\n", s));
        }
        if let Some(u) = until {
            out.push_str(&format!("**Until:** {}\n", u));
        }
        out.push('\n');

        // Group by date
        let mut current_date: Option<NaiveDate> = None;
        for entry in &entries {
            let date = entry.timestamp.date_naive();
            if current_date != Some(date) {
                out.push_str(&format!("## {}\n\n", date.format("%Y-%m-%d")));
                current_date = Some(date);
            }
            out.push_str(&format!(
                "- **{}** `{}` {} — {}\n",
                entry.timestamp.format("%H:%M"),
                entry.event,
                entry.project,
                entry.detail,
            ));
        }

        out
    }
}

/// Compare old and new git snapshots, return events for anything that changed.
pub fn detect_git_changes(old: &[GitStatus], new: &[GitStatus]) -> Vec<DevLogEntry> {
    let now = Local::now();
    let mut events = Vec::new();

    for new_status in new {
        let old_status = old.iter().find(|o| o.name == new_status.name);

        match old_status {
            Some(old_s) => {
                // New commit detected (commit message changed)
                if new_status.last_commit_msg != old_s.last_commit_msg
                    && !new_status.last_commit_msg.is_empty()
                {
                    events.push(DevLogEntry {
                        timestamp: now,
                        project: new_status.name.clone(),
                        event: DevLogEvent::NewCommit,
                        detail: new_status.last_commit_msg.clone(),
                    });
                }

                // Branch changed
                if new_status.branch != old_s.branch && !new_status.branch.is_empty() {
                    events.push(DevLogEntry {
                        timestamp: now,
                        project: new_status.name.clone(),
                        event: DevLogEvent::BranchChange,
                        detail: format!("{} -> {}", old_s.branch, new_status.branch),
                    });
                }

                // Clean/dirty transitions
                if !old_s.clean && new_status.clean {
                    events.push(DevLogEntry {
                        timestamp: now,
                        project: new_status.name.clone(),
                        event: DevLogEvent::StatusClean,
                        detail: "repo is now clean".to_string(),
                    });
                } else if old_s.clean && !new_status.clean {
                    events.push(DevLogEntry {
                        timestamp: now,
                        project: new_status.name.clone(),
                        event: DevLogEvent::StatusDirty,
                        detail: format!(
                            "{}m {}s {}u",
                            new_status.modified, new_status.staged, new_status.untracked
                        ),
                    });
                }

                // Push detected (ahead count dropped)
                if new_status.ahead < old_s.ahead && old_s.ahead > 0 {
                    events.push(DevLogEntry {
                        timestamp: now,
                        project: new_status.name.clone(),
                        event: DevLogEvent::PushDetected,
                        detail: format!("ahead {} -> {}", old_s.ahead, new_status.ahead),
                    });
                }
            }
            None => {
                // First time seeing this project, don't log initial state
            }
        }
    }

    events
}

/// Compare old and new remote snapshots, return events for status changes.
pub fn detect_remote_changes(old: &[RemoteStatus], new: &[RemoteStatus]) -> Vec<DevLogEntry> {
    let now = Local::now();
    let mut events = Vec::new();

    for new_status in new {
        let old_status = old.iter().find(|o| o.name == new_status.name);

        if let Some(old_s) = old_status {
            if old_s.status != new_status.status {
                let (event, detail) = match (&old_s.status, &new_status.status) {
                    (_, CheckResult::Up) => (
                        DevLogEvent::RemoteUp,
                        format!("{} -> UP", old_s.status),
                    ),
                    (_, CheckResult::Down) => (
                        DevLogEvent::RemoteDown,
                        format!("{} -> DOWN", old_s.status),
                    ),
                    (_, new_s) => (
                        DevLogEvent::RemoteDown,
                        format!("{} -> {}", old_s.status, new_s),
                    ),
                };

                events.push(DevLogEntry {
                    timestamp: now,
                    project: new_status.name.clone(),
                    event,
                    detail,
                });
            }
        }
    }

    events
}
