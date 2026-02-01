use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Mode, RowHit};

pub fn draw(frame: &mut Frame<'_>, app: &mut App) -> Vec<RowHit> {
    let size = frame.size();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(size);

    let status_area = layout[0];
    let body_area = layout[1];
    let help_area = layout[2];

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(body_area);

    draw_status(frame, app, status_area);
    let hits = draw_tree(frame, app, body_layout[0]);
    draw_details(frame, app, body_layout[1]);
    draw_help(frame, app, help_area);
    draw_overlay(frame, app, size);
    hits
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let (path, depth, kind, preview) = app.status_fields();
    let text = Line::from(vec![
        Span::styled("PATH ", Style::default().fg(Color::Yellow)),
        Span::raw(path),
        Span::raw("  "),
        Span::styled("DEPTH ", Style::default().fg(Color::Yellow)),
        Span::raw(depth.to_string()),
        Span::raw("  "),
        Span::styled("TYPE ", Style::default().fg(Color::Yellow)),
        Span::raw(kind),
        Span::raw("  "),
        Span::styled("VALUE ", Style::default().fg(Color::Yellow)),
        Span::raw(preview),
    ]);
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::White));
    frame.render_widget(paragraph, area);
}

fn draw_tree(frame: &mut Frame<'_>, app: &mut App, area: Rect) -> Vec<RowHit> {
    let mut hits = Vec::new();
    let available_height = area.height.saturating_sub(2) as usize;
    let start = app.scroll;
    let end = (start + available_height).min(app.visible.len());

    let mut lines = Vec::new();
    for (idx, row) in app.visible.iter().enumerate().take(end).skip(start) {
        let indent = row.depth * 2;
        let expanded = app.expanded.contains(&row.path.dot_path());
        let indicator = if row.is_container {
            if expanded { "▾" } else { "▸" }
        } else {
            " "
        };
        let mut line = String::new();
        line.push_str(&" ".repeat(indent));
        line.push_str(indicator);
        line.push(' ');
        let key_start = indent + 2;
        line.push_str(&row.display_key);
        let key_end = key_start + row.display_key.width();
        if !row.is_container {
            if !row.display_value_preview.is_empty() {
                line.push_str(" = ");
                line.push_str(&row.display_value_preview);
            }
        }

        let mut style = Style::default();
        if idx == app.selection {
            style = style
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
        }

        lines.push(Line::from(Span::styled(line.clone(), style)));
        let row_y = area.y + 1 + (idx - start) as u16;
        hits.push(RowHit {
            row_index: idx,
            y: row_y,
            key_x_start: area.x + key_start as u16,
            key_x_end: area.x + key_end.saturating_sub(1) as u16,
        });
    }

    let block = Block::default().title("Tree").borders(Borders::ALL);
    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
    hits
}

fn draw_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = Block::default().title("Details").borders(Borders::ALL);
    let mut lines = Vec::new();
    if let Some(row) = app.current_row() {
        lines.push(Line::from(format!("Path: {}", row.path.dot_path())));
        lines.push(Line::from(format!("Depth: {}", row.path.depth())));
        lines.push(Line::from(format!("Type: {}", row.node_type)));
        lines.push(Line::from(format!("Value: {}", row.display_value_preview)));
    }

    if matches!(
        app.mode,
        Mode::EditValue | Mode::RenameKey | Mode::AddKey | Mode::AddValue | Mode::SearchInput
    ) {
        lines.push(Line::from(""));
        let input_label = match app.mode {
            Mode::EditValue => "Edit Value:",
            Mode::RenameKey => "Rename Key:",
            Mode::AddKey => "New Key:",
            Mode::AddValue => "New Value:",
            Mode::SearchInput => "Search:",
            _ => "Input:",
        };
        lines.push(Line::from(Span::styled(
            input_label,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        let cursor = app.input.cursor;
        let mut input_line = app.input.text.clone();
        if cursor <= input_line.len() {
            input_line.insert(cursor, '▌');
        }
        lines.push(Line::from(input_line));
    }

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_help(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mode_label = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::EditValue => "EDIT VALUE",
        Mode::RenameKey => "RENAME KEY",
        Mode::AddKey => "ADD KEY",
        Mode::AddValue => "ADD VALUE",
        Mode::ConfirmDelete => "CONFIRM",
        Mode::ConfirmQuit => "CONFIRM",
        Mode::SearchInput => "SEARCH",
    };
    let mode_span = Span::styled(
        format!(" {} ", mode_label),
        Style::default()
            .fg(Color::White)
            .bg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    );
    let help_text = " j/k:move h/l:fold Enter:toggle e:edit r:rename a:add d:del y:copy /:search Ctrl+s:save q:quit";
    let line = Line::from(vec![
        mode_span,
        Span::raw(" "),
        Span::styled(help_text, Style::default().fg(Color::Gray)),
    ]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn draw_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    // Draw confirm dialogs
    let confirm_message: Option<String> = match app.mode {
        Mode::ConfirmDelete => Some("Delete node? (y/n)".to_string()),
        Mode::ConfirmQuit => {
            if app.dirty {
                Some("Unsaved changes. Quit? (y/n)".to_string())
            } else {
                Some("Quit? (y/n)".to_string())
            }
        }
        _ => None,
    };
    if let Some(message) = confirm_message {
        let block = Block::default().borders(Borders::ALL).title("Confirm");
        let width = message.width().saturating_add(4) as u16;
        let height = 3;
        let rect = centered_rect(width, height, area);
        let paragraph = Paragraph::new(message.as_str()).block(block);
        frame.render_widget(paragraph, rect);
    }
    // Draw toast message in center
    if let Some(toast) = &app.toast {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title("Info");
        let width = toast.message.width().saturating_add(4) as u16;
        let height = 3;
        let rect = centered_rect(width.max(20), height, area);
        let paragraph = Paragraph::new(toast.message.as_str())
            .block(block)
            .style(Style::default().fg(Color::White));
        frame.render_widget(paragraph, rect);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}
