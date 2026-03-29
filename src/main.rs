mod app;
mod config;
mod devlog;
mod git_ops;
mod git_scanner;
mod remote_checker;
mod ui;

use anyhow::Result;
use app::{App, Mode, Panel};
use chrono::{Local, NaiveDate};
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use devlog::DevLog;
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle --init flag
    if args.iter().any(|a| a == "--init") {
        Config::init_default()?;
        return Ok(());
    }

    // Handle `log` subcommand
    if args.get(1).map(|s| s.as_str()) == Some("log") {
        return run_log_command(&args[2..]);
    }

    let config = Config::load()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal, config).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_log_command(args: &[String]) -> Result<()> {
    let config = Config::load()?;

    let path = config
        .devlog
        .path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(DevLog::default_path);
    let devlog = DevLog::new(path, config.devlog.max_display);

    let mut project: Option<&str> = None;
    let mut since: Option<NaiveDate> = None;
    let mut until: Option<NaiveDate> = None;
    let mut export = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--project" | "-p" => {
                i += 1;
                project = args.get(i).map(|s| s.as_str());
            }
            "--today" => {
                let today = Local::now().date_naive();
                since = Some(today);
                until = Some(today);
            }
            "--week" => {
                let today = Local::now().date_naive();
                since = Some(today - chrono::Duration::days(7));
                until = Some(today);
            }
            "--since" => {
                i += 1;
                if let Some(s) = args.get(i) {
                    since = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
                }
            }
            "--until" => {
                i += 1;
                if let Some(s) = args.get(i) {
                    until = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
                }
            }
            "--export" => {
                export = true;
            }
            _ => {}
        }
        i += 1;
    }

    if export {
        print!("{}", devlog.export_markdown(project, since, until));
    } else {
        let entries = devlog.load_filtered(project, since, until);
        if entries.is_empty() {
            println!("No devlog entries found.");
        } else {
            for entry in &entries {
                println!(
                    "{} {:>6}  {:<18} {}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    format!("{}", entry.event),
                    entry.project,
                    entry.detail,
                );
            }
            println!("\n{} entries", entries.len());
        }
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, config: Config) -> Result<()> {
    let mut app = App::new(config);
    let refresh_interval = Duration::from_secs(app.config.refresh_seconds);

    // Initial refresh
    app.refresh_all().await;

    loop {
        app.clear_stale_status();
        terminal.draw(|f| ui::draw(f, &app))?;

        // Auto-refresh check (only in browse mode)
        if app.mode == Mode::Browse {
            let needs_refresh = app
                .last_refresh
                .map(|t| t.elapsed() >= refresh_interval)
                .unwrap_or(true);

            if needs_refresh {
                app.refresh_all().await;
            }
        }

        // Poll for events with timeout so we can auto-refresh
        if event::poll(Duration::from_secs(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    Mode::Browse => match key.code {
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                            break;
                        }
                        KeyCode::Tab => app.toggle_panel(),
                        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
                        KeyCode::Enter => app.enter_actions(),
                        KeyCode::Char('r') => app.refresh_all().await,
                        KeyCode::Char('l') if app.devlog.is_some() => {
                            app.active_panel = Panel::DevLog;
                        }
                        _ => {}
                    },

                    Mode::Actions => match key.code {
                        KeyCode::Char('a') => app.do_stage_all(),
                        KeyCode::Char('c') => app.start_commit_input(),
                        KeyCode::Char('p') => app.do_push(),
                        KeyCode::Char('s') => app.start_quick_push_input(),
                        KeyCode::Esc => app.exit_mode(),
                        _ => {}
                    },

                    Mode::CommitInput => match key.code {
                        KeyCode::Enter => app.do_commit(),
                        KeyCode::Esc => app.exit_mode(),
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                        }
                        _ => {}
                    },

                    Mode::QuickPushInput => match key.code {
                        KeyCode::Enter => app.do_quick_push(),
                        KeyCode::Esc => app.exit_mode(),
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    Ok(())
}
