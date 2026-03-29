use crate::config::Config;
use crate::git_ops::{self, GitOpResult};
use crate::git_scanner::{self, GitStatus};
use crate::remote_checker::{self, CheckResult, RemoteStatus};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Projects,
    Remotes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Browse,
    Actions,
    CommitInput,
    QuickPushInput,
}

pub struct App {
    pub config: Config,
    pub git_statuses: Vec<GitStatus>,
    pub remote_statuses: Vec<RemoteStatus>,
    pub active_panel: Panel,
    pub selected_project: usize,
    pub selected_remote: usize,
    pub last_refresh: Option<Instant>,
    pub refreshing: bool,
    pub should_quit: bool,
    pub mode: Mode,
    pub input_buffer: String,
    pub status_message: Option<(String, bool)>, // (message, is_success)
    pub status_time: Option<Instant>,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            git_statuses: Vec::new(),
            remote_statuses: Vec::new(),
            active_panel: Panel::Projects,
            selected_project: 0,
            selected_remote: 0,
            last_refresh: None,
            refreshing: false,
            should_quit: false,
            mode: Mode::Browse,
            input_buffer: String::new(),
            status_message: None,
            status_time: None,
        }
    }

    pub fn selected_project_path(&self) -> Option<&str> {
        self.git_statuses
            .get(self.selected_project)
            .map(|gs| gs.path.as_str())
    }

    pub fn enter_actions(&mut self) {
        if self.active_panel == Panel::Projects && !self.git_statuses.is_empty() {
            self.mode = Mode::Actions;
        }
    }

    pub fn exit_mode(&mut self) {
        self.mode = Mode::Browse;
        self.input_buffer.clear();
    }

    pub fn start_commit_input(&mut self) {
        self.mode = Mode::CommitInput;
        self.input_buffer.clear();
    }

    pub fn start_quick_push_input(&mut self) {
        self.mode = Mode::QuickPushInput;
        self.input_buffer = "update".to_string();
    }

    pub fn set_status(&mut self, result: &GitOpResult) {
        self.status_message = Some((result.message.clone(), result.success));
        self.status_time = Some(Instant::now());
    }

    pub fn clear_stale_status(&mut self) {
        if let Some(t) = self.status_time {
            if t.elapsed().as_secs() > 5 {
                self.status_message = None;
                self.status_time = None;
            }
        }
    }

    pub fn do_stage_all(&mut self) {
        if let Some(path) = self.selected_project_path().map(|s| s.to_string()) {
            let result = git_ops::stage_all(&path);
            self.set_status(&result);
            self.refresh_git();
            self.mode = Mode::Browse;
        }
    }

    pub fn do_commit(&mut self) {
        let msg = self.input_buffer.clone();
        if msg.is_empty() {
            return;
        }
        if let Some(path) = self.selected_project_path().map(|s| s.to_string()) {
            let result = git_ops::commit(&path, &msg);
            self.set_status(&result);
            self.refresh_git();
            self.mode = Mode::Browse;
            self.input_buffer.clear();
        }
    }

    pub fn do_push(&mut self) {
        if let Some(path) = self.selected_project_path().map(|s| s.to_string()) {
            let result = git_ops::push(&path);
            self.set_status(&result);
            self.refresh_git();
            self.mode = Mode::Browse;
        }
    }

    pub fn do_quick_push(&mut self) {
        let msg = self.input_buffer.clone();
        if msg.is_empty() {
            return;
        }
        if let Some(path) = self.selected_project_path().map(|s| s.to_string()) {
            let result = git_ops::quick_push(&path, &msg);
            self.set_status(&result);
            self.refresh_git();
            self.mode = Mode::Browse;
            self.input_buffer.clear();
        }
    }

    pub fn refresh_git(&mut self) {
        self.git_statuses = self
            .config
            .projects
            .iter()
            .map(|p| git_scanner::scan_repo(&p.name, &p.path))
            .collect();
    }

    pub async fn refresh_remotes(&mut self) {
        let mut statuses = Vec::new();

        for remote in &self.config.remotes {
            if let Some(url) = &remote.url {
                statuses.push(remote_checker::check_http(&remote.name, url).await);
            }
            if let Some(host) = &remote.ssh_host {
                statuses.push(remote_checker::check_ssh(
                    &remote.name,
                    host,
                    remote.ssh_port,
                    &remote.ssh_user,
                ));
            }
        }

        self.remote_statuses = statuses;
    }

    pub async fn refresh_all(&mut self) {
        self.refreshing = true;
        self.refresh_git();
        self.refresh_remotes().await;
        self.last_refresh = Some(Instant::now());
        self.refreshing = false;
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Projects => Panel::Remotes,
            Panel::Remotes => Panel::Projects,
        };
    }

    pub fn select_next(&mut self) {
        match self.active_panel {
            Panel::Projects => {
                if !self.git_statuses.is_empty() {
                    self.selected_project =
                        (self.selected_project + 1) % self.git_statuses.len();
                }
            }
            Panel::Remotes => {
                if !self.remote_statuses.is_empty() {
                    self.selected_remote =
                        (self.selected_remote + 1) % self.remote_statuses.len();
                }
            }
        }
    }

    pub fn select_prev(&mut self) {
        match self.active_panel {
            Panel::Projects => {
                if !self.git_statuses.is_empty() {
                    self.selected_project = if self.selected_project == 0 {
                        self.git_statuses.len() - 1
                    } else {
                        self.selected_project - 1
                    };
                }
            }
            Panel::Remotes => {
                if !self.remote_statuses.is_empty() {
                    self.selected_remote = if self.selected_remote == 0 {
                        self.remote_statuses.len() - 1
                    } else {
                        self.selected_remote - 1
                    };
                }
            }
        }
    }

    pub fn projects_summary(&self) -> (usize, usize, usize) {
        let total = self.git_statuses.len();
        let clean = self.git_statuses.iter().filter(|g| g.clean).count();
        let dirty = total - clean;
        (total, clean, dirty)
    }

    pub fn remotes_summary(&self) -> (usize, usize, usize) {
        let total = self.remote_statuses.len();
        let up = self
            .remote_statuses
            .iter()
            .filter(|r| r.status == CheckResult::Up)
            .count();
        let down = total - up;
        (total, up, down)
    }
}
