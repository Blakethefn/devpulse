# DevPulse

A terminal dashboard for monitoring your local git projects and remote services. Think `htop` for your dev ecosystem.

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Local git monitoring** — branch, modified/staged/untracked file counts, ahead/behind remote, last commit and age
- **Remote HTTP health checks** — status codes, latency, degradation detection
- **Remote SSH checks** — TCP connectivity, SSH handshake, optional auth verification
- **Git actions from the TUI** — stage, commit, push, or quick-push without leaving the dashboard
- **Auto-refresh** — configurable polling interval
- **htop-style TUI** — color-coded tables, keyboard navigation, designed to stay pinned open

## Install

```bash
# Clone and build
git clone https://github.com/yourusername/devpulse.git
cd devpulse
cargo build --release

# The binary is at ./target/release/devpulse
# Optionally copy it somewhere on your PATH:
cp target/release/devpulse ~/.cargo/bin/
```

### Dependencies

Requires system libraries for libgit2, libssh2, and OpenSSL. On Debian/Ubuntu:

```bash
sudo apt install libssl-dev libssh2-1-dev pkg-config cmake
```

## Setup

```bash
# Generate a starter config
devpulse --init

# Or copy the example config
cp config.example.toml ~/.config/devpulse/config.toml
```

Edit `~/.config/devpulse/config.toml` to add your projects and remotes. See [config.example.toml](config.example.toml) for the full format.

## Usage

```bash
devpulse
```

### Keybindings

#### Browse Mode

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Switch between Projects and Remotes panels |
| `↑`/`↓` or `j`/`k` | Navigate rows |
| `Enter` | Open git actions for selected project |
| `r` | Manual refresh |

#### Action Mode (after Enter on a project)

| Key | Action |
|-----|--------|
| `a` | `git add -A` (stage all changes) |
| `c` | `git commit` (prompts for message) |
| `p` | `git push` |
| `s` | Quick push: stage + commit + push (editable message, defaults to "update") |
| `Esc` | Cancel / back to browse |

## Config Reference

```toml
# Auto-refresh interval in seconds
refresh_seconds = 30

# Local git repositories
[[projects]]
name = "my-project"
path = "/path/to/repo"
tags = ["rust", "active"]  # optional, for future filtering

# Remote HTTP services
[[remotes]]
name = "api.example.com"
url = "https://api.example.com/health"
tags = ["production"]

# Remote SSH services
[[remotes]]
name = "prod-server"
ssh_host = "192.168.1.100"
ssh_port = 22          # optional, defaults to 22
ssh_user = "deploy"    # optional, for auth check via ssh-agent
tags = ["production"]
```

## License

MIT
# devpulse
