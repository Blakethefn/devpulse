use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_refresh")]
    pub refresh_seconds: u64,
    #[serde(default)]
    pub projects: Vec<ProjectConfig>,
    #[serde(default)]
    pub remotes: Vec<RemoteConfig>,
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
        let config: Config = toml::from_str(&contents).context("Failed to parse config")?;
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
}

const DEFAULT_CONFIG: &str = r#"# DevPulse Configuration
refresh_seconds = 30

# Local projects to monitor
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
"#;
