mod app;
mod config;
mod git_ops;
mod git_scanner;
mod remote_checker;
mod ui;

use anyhow::Result;
use app::{App, Mode};
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Handle --init flag
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--init") {
        Config::init_default()?;
        return Ok(());
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
