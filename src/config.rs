use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_refresh")]
    pub refresh_seconds: u64,
    #[serde(default)]
    pub scan_dirs: Vec<String>,
    #[serde(default)]
    pub projects: Vec<ProjectConfig>,
    #[serde(default)]
    pub remotes: Vec<RemoteConfig>,
    #[serde(default)]
    pub devlog: DevLogConfig,
}

fn default_refresh() -> u64 {
    30
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RemoteConfig {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub ssh_host: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    #[serde(default)]
    pub ssh_user: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DevLogConfig {
    #[serde(default = "default_devlog_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default = "default_devlog_max_display")]
    pub max_display: usize,
}

impl Default for DevLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
            max_display: 100,
        }
    }
}

fn default_devlog_enabled() -> bool {
    true
}

fn default_devlog_max_display() -> usize {
    100
}

fn default_ssh_port() -> u16 {
    22
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            anyhow::bail!(
                "Config not found at {}. Run `devpulse --init` to create one.",
                config_path.display()
            );
        }
        let contents =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;
        let mut config: Config = toml::from_str(&contents).context("Failed to parse config")?;

        // Auto-discover git repos from scan_dirs
        let discovered = config.discover_projects();
        config.merge_discovered(discovered);

        Ok(config)
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;
        Ok(config_dir.join("devpulse").join("config.toml"))
    }

    pub fn init_default() -> Result<()> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            println!("Config already exists at {}", config_path.display());
            return Ok(());
        }
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&config_path, DEFAULT_CONFIG)?;
        println!("Created config at {}", config_path.display());
        Ok(())
    }

    fn discover_projects(&self) -> Vec<ProjectConfig> {
        let mut discovered = Vec::new();

        for scan_dir in &self.scan_dirs {
            let scan_path = PathBuf::from(scan_dir);
            if !scan_path.is_dir() {
                continue;
            }

            let entries = match fs::read_dir(&scan_path) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let entry_path = entry.path();
                if !entry_path.is_dir() {
                    continue;
                }

                // Skip hidden directories
                let dir_name = entry
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                if dir_name.starts_with('.') {
                    continue;
                }

                // Check if it's a git repo
                let git_dir = entry_path.join(".git");
                if git_dir.exists() {
                    discovered.push(ProjectConfig {
                        name: dir_name,
                        path: entry_path.to_string_lossy().to_string(),
                        tags: vec!["auto".to_string()],
                    });
                }
            }
        }

        // Sort by name for consistent ordering
        discovered.sort_by(|a, b| a.name.cmp(&b.name));
        discovered
    }

    fn merge_discovered(&mut self, discovered: Vec<ProjectConfig>) {
        // Collect paths already manually listed
        let existing_paths: std::collections::HashSet<String> = self
            .projects
            .iter()
            .map(|p| p.path.clone())
            .collect();

        // Add discovered projects that aren't already manually listed
        for project in discovered {
            if !existing_paths.contains(&project.path) {
                self.projects.push(project);
            }
        }
    }
}

const DEFAULT_CONFIG: &str = r#"# DevPulse Configuration
refresh_seconds = 30

# Directories to auto-scan for git repos (one level deep)
# Every subdirectory with a .git folder will be added as a project
scan_dirs = []

# Manual project entries (these take priority over auto-discovered ones)
# [[projects]]
# name = "my-project"
# path = "/home/user/projects/my-project"
# tags = ["rust", "active"]

# Remote services to check
# [[remotes]]
# name = "api.example.com"
# url = "https://api.example.com/health"
# tags = ["production"]

# [[remotes]]
# name = "prod-server"
# ssh_host = "192.168.1.100"
# ssh_port = 22
# ssh_user = "deploy"
# tags = ["production"]

# DevLog settings
[devlog]
enabled = true
# path = "/custom/path/to/devlog.jsonl"  # default: ~/.local/share/devpulse/devlog.jsonl
max_display = 100  # max entries shown in the TUI panel
"#;
