use crate::app::{App, Mode, Panel};
use crate::remote_checker::CheckResult;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let has_status = app.status_message.is_some();
    let has_input = matches!(app.mode, Mode::CommitInput | Mode::QuickPushInput);

    let footer_height = if has_status { 2 } else { 1 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),          // header
            Constraint::Min(0),            // main content
            Constraint::Length(footer_height), // footer + status
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    // Overlay: action menu
    if app.mode == Mode::Actions {
        draw_action_popup(f, app);
    }

    // Overlay: text input
    if has_input {
        draw_input_popup(f, app);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let (total_p, clean_p, dirty_p) = app.projects_summary();
    let (total_r, up_r, down_r) = app.remotes_summary();

    let refresh_text = match &app.last_refresh {
        Some(t) => format!("{}s ago", t.elapsed().as_secs()),
        None => "never".to_string(),
    };

    let header = Line::from(vec![
        Span::styled(" DEVPULSE ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(format!(" {} projects ", total_p), Style::default().fg(Color::White)),
        Span::styled(format!(" {}✓ ", clean_p), Style::default().fg(Color::Green)),
        Span::styled(format!(" {}✗ ", dirty_p), Style::default().fg(Color::Yellow)),
        Span::raw("  │  "),
        Span::styled(format!(" {} remotes ", total_r), Style::default().fg(Color::White)),
        Span::styled(format!(" {}↑ ", up_r), Style::default().fg(Color::Green)),
        Span::styled(format!(" {}↓ ", down_r), Style::default().fg(Color::Red)),
        Span::raw("  │  "),
        Span::styled(format!("⟳ {}", refresh_text), Style::default().fg(Color::DarkGray)),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(header).block(block);
    f.render_widget(paragraph, area);
}

fn draw_main(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // projects
            Constraint::Percentage(40), // remotes
        ])
        .split(area);

    draw_projects_table(f, app, chunks[0]);
    draw_remotes_table(f, app, chunks[1]);
}

fn draw_projects_table(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Projects;
    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

    let header = Row::new(vec![
        Cell::from("Project").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Branch").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Mod").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Stg").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Unt").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("↑↓").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Last Commit").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Age").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::DarkGray))
    .height(1);

    let rows: Vec<Row> = app
        .git_statuses
        .iter()
        .enumerate()
        .map(|(i, gs)| {
            let status_style = if gs.error.is_some() {
                Style::default().fg(Color::Red)
            } else if gs.clean {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let status_text = if gs.error.is_some() {
                "ERR"
            } else if gs.clean {
                "clean"
            } else {
                "dirty"
            };

            let ahead_behind = if gs.ahead > 0 || gs.behind > 0 {
                format!("↑{}↓{}", gs.ahead, gs.behind)
            } else {
                "—".to_string()
            };

            let row_style = if is_active && i == app.selected_project {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(gs.name.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(gs.branch.clone()),
                Cell::from(status_text).style(status_style),
                Cell::from(format!("{}", gs.modified)),
                Cell::from(format!("{}", gs.staged)),
                Cell::from(format!("{}", gs.untracked)),
                Cell::from(ahead_behind),
                Cell::from(truncate(&gs.last_commit_msg, 35)),
                Cell::from(gs.last_commit_age.clone()).style(Style::default().fg(Color::DarkGray)),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),  // project
            Constraint::Length(14),  // branch
            Constraint::Length(7),   // status
            Constraint::Length(4),   // modified
            Constraint::Length(4),   // staged
            Constraint::Length(4),   // untracked
            Constraint::Length(6),   // ahead/behind
            Constraint::Length(37),  // last commit
            Constraint::Length(8),   // age
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" ▸ Local Projects ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(1)),
    );

    let mut state = TableState::default();
    if is_active {
        state.select(Some(app.selected_project));
    }
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_remotes_table(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Remotes;
    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

    let header = Row::new(vec![
        Cell::from("Service").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Latency").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Detail").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::DarkGray))
    .height(1);

    let rows: Vec<Row> = app
        .remote_statuses
        .iter()
        .enumerate()
        .map(|(i, rs)| {
            let status_style = match rs.status {
                CheckResult::Up => Style::default().fg(Color::Green),
                CheckResult::Down => Style::default().fg(Color::Red),
                CheckResult::Degraded => Style::default().fg(Color::Yellow),
                CheckResult::Unknown => Style::default().fg(Color::DarkGray),
            };

            let latency_text = match rs.latency_ms {
                Some(ms) => format!("{}ms", ms),
                None => "—".to_string(),
            };

            let detail = if let Some(err) = &rs.error {
                truncate(err, 50)
            } else {
                rs.detail.clone()
            };

            let row_style = if is_active && i == app.selected_remote {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(rs.name.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{}", rs.kind)),
                Cell::from(format!("{}", rs.status)).style(status_style),
                Cell::from(latency_text),
                Cell::from(detail),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(25),  // service
            Constraint::Length(6),   // type
            Constraint::Length(6),   // status
            Constraint::Length(9),   // latency
            Constraint::Min(20),     // detail
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" ▸ Remote Services ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(1)),
    );

    let mut state = TableState::default();
    if is_active {
        state.select(Some(app.selected_remote));
    }
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = if let Some((msg, success)) = &app.status_message {
        let color = if *success { Color::Green } else { Color::Red };
        let status_line = Line::from(vec![
            Span::styled(" ● ", Style::default().fg(color)),
            Span::styled(msg.clone(), Style::default().fg(color)),
        ]);
        let keys_line = footer_keys(app);
        vec![status_line, keys_line]
    } else {
        vec![footer_keys(app)]
    };

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, area);
}

fn footer_keys(app: &App) -> Line<'static> {
    match app.mode {
        Mode::Browse => Line::from(vec![
            Span::styled(" q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" quit  "),
            Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" switch  "),
            Span::styled("↑↓/jk", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" nav  "),
            Span::styled("Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" actions  "),
            Span::styled("r", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" refresh"),
        ]),
        Mode::Actions => Line::from(vec![
            Span::styled(" a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" stage all  "),
            Span::styled("c", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" commit  "),
            Span::styled("p", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" push  "),
            Span::styled("s", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" quick push  "),
            Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" back"),
        ]),
        Mode::CommitInput | Mode::QuickPushInput => Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" confirm  "),
            Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]),
    }
}

fn draw_action_popup(f: &mut Frame, app: &App) {
    let project_name = app
        .git_statuses
        .get(app.selected_project)
        .map(|gs| gs.name.as_str())
        .unwrap_or("?");

    let area = centered_rect(40, 10, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  git add -A"),
        ]),
        Line::from(vec![
            Span::styled("  c", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  git commit"),
        ]),
        Line::from(vec![
            Span::styled("  p", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  git push"),
        ]),
        Line::from(vec![
            Span::styled("  s", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("  stage + commit + push"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Esc", Style::default().fg(Color::DarkGray)),
            Span::raw("  cancel"),
        ]),
    ];

    let block = Block::default()
        .title(format!(" ▸ {} ", project_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn draw_input_popup(f: &mut Frame, app: &App) {
    let title = match app.mode {
        Mode::CommitInput => " Commit Message ",
        Mode::QuickPushInput => " Quick Push Message ",
        _ => "",
    };

    let area = centered_rect(50, 5, f.area());
    f.render_widget(Clear, area);

    let input_line = Line::from(vec![
        Span::styled(" > ", Style::default().fg(Color::Cyan)),
        Span::raw(app.input_buffer.clone()),
        Span::styled("█", Style::default().fg(Color::White)),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(vec![Line::from(""), input_line]).block(block);
    f.render_widget(paragraph, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
    } else {
        s.to_string()
    }
}
