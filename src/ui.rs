use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Mode, PickerEntry, RowHit};

pub fn draw(frame: &mut Frame<'_>, app: &mut App) -> Vec<RowHit> {
    let size = frame.size();
    let has_parse_error = !app.is_file_picker() && app.parse_error.is_some();
    let constraints: Vec<Constraint> = if has_parse_error {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ]
    };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(&constraints)
        .split(size);

    let (status_area, body_area, help_area) = if has_parse_error {
        draw_parse_error(frame, app, layout[0]);
        (layout[1], layout[2], layout[3])
    } else {
        (layout[0], layout[1], layout[2])
    };

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(body_area);

    draw_status(frame, app, status_area);
    let hits = if app.is_file_picker() {
        draw_file_picker(frame, app, body_layout[0])
    } else {
        draw_tree(frame, app, body_layout[0])
    };
    draw_details(frame, app, body_layout[1]);
    draw_help(frame, app, help_area);
    draw_overlay(frame, app, size);
    hits
}

fn draw_parse_error(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let msg = app
        .parse_error
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(area.width as usize)
        .collect::<String>();
    let line = Line::from(Span::styled(
        format!("PARSE ERROR: {}", msg),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    ));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.is_file_picker() {
        let dir = app
            .file_picker
            .as_ref()
            .map(|p| p.current_dir.display().to_string())
            .unwrap_or_else(|| "?".to_string());
        let text = Line::from(vec![
            Span::styled("DIR ", Style::default().fg(Color::Yellow)),
            Span::raw(dir),
            Span::raw("  "),
            Span::styled(".. = up  Enter = open  q = quit", Style::default().fg(Color::Gray)),
        ]);
        let paragraph = Paragraph::new(text).style(Style::default().fg(Color::White));
        frame.render_widget(paragraph, area);
        return;
    }
    let (path, depth, kind, preview) = app.status_fields();
    let mut spans = vec![
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
    ];
    if let Some(_) = app.search_query.as_ref() {
        let total = app.matches.len();
        let current = app
            .matches
            .iter()
            .position(|&i| i == app.selection)
            .map(|p| p + 1)
            .unwrap_or(0);
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "Search ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
        if total == 0 {
            spans.push(Span::styled(
                "0/0",
                Style::default().fg(Color::Gray),
            ));
        } else {
            spans.push(Span::raw(format!("{}/{}", current, total)));
        }
    }
    let text = Line::from(spans);
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::White));
    frame.render_widget(paragraph, area);
}

fn draw_file_picker(frame: &mut Frame<'_>, app: &mut App, area: Rect) -> Vec<RowHit> {
    let mut hits = Vec::new();
    let picker = match &app.file_picker {
        Some(p) => p,
        None => return hits,
    };
    let available_height = area.height.saturating_sub(2) as usize;
    let len = picker.entries.len();
    if len == 0 {
        let block = Block::default().title("Select file").borders(Borders::ALL);
        let paragraph = Paragraph::new("No .yaml or .yml files in current directory.")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(paragraph, area);
        return hits;
    }
    let start = (app.selection + 1)
        .saturating_sub(available_height)
        .max(0)
        .min(len.saturating_sub(available_height));
    let end = (start + available_height).min(len);
    let mut lines = Vec::new();
    for (idx, entry) in picker.entries.iter().enumerate().take(end).skip(start) {
        let (name, is_dir) = match entry {
            PickerEntry::Parent => ("..".to_string(), true),
            PickerEntry::Dir(p) => (
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| format!("{}/", s))
                    .unwrap_or_else(|| "?/".to_string()),
                true,
            ),
            PickerEntry::File(p) => (
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string(),
                false,
            ),
        };
        let mut style = Style::default();
        if idx == app.selection {
            style = style
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
        } else if app.hover_row == Some(idx) {
            style = style.bg(Color::DarkGray);
        } else if is_dir {
            style = style.fg(Color::Yellow);
        }
        lines.push(Line::from(Span::styled(name.clone(), style)));
        let row_y = area.y + 1 + (idx - start) as u16;
        let key_end = name.width().saturating_add(2);
        hits.push(RowHit {
            row_index: idx,
            y: row_y,
            key_x_start: area.x + 1,
            key_x_end: area.x + key_end as u16,
        });
    }
    let block = Block::default()
        .title("Select file (.. = parent, dir/ = enter, .yaml/.yml = open)")
        .borders(Borders::ALL);
    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
    hits
}

fn draw_tree(frame: &mut Frame<'_>, app: &mut App, area: Rect) -> Vec<RowHit> {
    let mut hits = Vec::new();
    let available_height = area.height.saturating_sub(2) as usize;

    if let Some(raw_lines) = app.raw_lines() {
        let len = raw_lines.len();
        if len == 0 {
            let block = Block::default().title("Raw (parse error - fix and Ctrl+s)").borders(Borders::ALL);
            let paragraph = Paragraph::new("Empty file.").block(block).style(Style::default().fg(Color::Gray));
            frame.render_widget(paragraph, area);
            return hits;
        }
        let start = app.scroll;
        let end = (start + available_height).min(len);
        let mut lines = Vec::new();
        for (idx, line_str) in raw_lines.iter().enumerate().take(end).skip(start) {
            let line_num = format!("{:4} ", idx + 1);
            let mut style = Style::default();
            if idx == app.selection {
                style = style
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD);
            } else if app.hover_row == Some(idx) {
                style = style.bg(Color::DarkGray);
            }
            let display = format!("{}{}", line_num, line_str);
            lines.push(Line::from(Span::styled(display.clone(), style)));
            let row_y = area.y + 1 + (idx - start) as u16;
            let key_end = display.width().saturating_add(2);
            hits.push(RowHit {
                row_index: idx,
                y: row_y,
                key_x_start: area.x + 1,
                key_x_end: area.x + key_end as u16,
            });
        }
        let block = Block::default()
            .title("Raw (parse error - e: edit line, Ctrl+s: save & re-parse)")
            .borders(Borders::ALL);
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
        return hits;
    }

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
        } else if app.hover_row == Some(idx) {
            style = style.bg(Color::DarkGray);
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
    if app.is_file_picker() {
        if let Some(picker) = &app.file_picker {
            lines.push(Line::from(format!("Dir: {}", picker.current_dir.display())));
            if app.selection < picker.entries.len() {
                let hint = match &picker.entries[app.selection] {
                    PickerEntry::Parent => "Enter = go up",
                    PickerEntry::Dir(_) => "Enter = open folder",
                    PickerEntry::File(_) => "Enter = open file",
                };
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    hint,
                    Style::default().fg(Color::Gray),
                )));
            }
        }
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
        return;
    }
    if let Some(row) = app.current_row() {
        lines.push(Line::from(format!("Path: {}", row.path.dot_path())));
        lines.push(Line::from(format!("Depth: {}", row.path.depth())));
        lines.push(Line::from(format!("Type: {}", row.node_type)));
        lines.push(Line::from(format!("Value: {}", row.display_value_preview)));
    }

    if matches!(
        app.mode,
        Mode::EditValue | Mode::RenameKey | Mode::AddKey | Mode::AddValue | Mode::SearchInput | Mode::RawEditLine
    ) {
        lines.push(Line::from(""));
        let input_label = match app.mode {
            Mode::EditValue => "Edit Value:",
            Mode::RenameKey => "Rename Key:",
            Mode::AddKey => "New Key:",
            Mode::AddValue => "New Value:",
            Mode::SearchInput => "Search:",
            Mode::RawEditLine => "Edit Line:",
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
    if app.is_file_picker() {
        let mode_span = Span::styled(
            " FILE PICKER ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        );
        let help_text = " j/k:move Enter:open q:quit";
        let line = Line::from(vec![
            mode_span,
            Span::raw(" "),
            Span::styled(help_text, Style::default().fg(Color::Gray)),
        ]);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
        return;
    }
    let (mode_label, mode_bg) = match app.mode {
        Mode::Normal => ("NORMAL", Color::Magenta),
        Mode::EditValue => ("EDIT VALUE", Color::Blue),
        Mode::RenameKey => ("RENAME KEY", Color::Yellow),
        Mode::AddKey => ("ADD KEY", Color::Green),
        Mode::AddValue => ("ADD VALUE", Color::LightGreen),
        Mode::ConfirmDelete => ("CONFIRM", Color::Red),
        Mode::ConfirmQuit => ("CONFIRM", Color::Red),
        Mode::ConfirmOpenAnother => ("CONFIRM", Color::Red),
        Mode::ConfirmRawDeleteLine => ("CONFIRM", Color::Red),
        Mode::SearchInput => ("SEARCH", Color::Cyan),
        Mode::RawEditLine => ("EDIT LINE", Color::LightCyan),
    };
    let mode_span = Span::styled(
        format!(" {} ", mode_label),
        Style::default()
            .fg(Color::White)
            .bg(mode_bg)
            .add_modifier(Modifier::BOLD),
    );
    let help_text = " j/k:move h/l:fold Enter:toggle e:edit r:rename a:add Shift+A:add object d:del Shift+Del:del line y:copy /:search Ctrl+s:save Ctrl+o:open another q:quit";
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
        Mode::ConfirmOpenAnother => {
            Some("Open another file? Unsaved changes will be lost. (y/n)".to_string())
        }
        Mode::ConfirmRawDeleteLine => Some("Delete this line? (y/n)".to_string()),
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
