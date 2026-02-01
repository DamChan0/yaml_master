use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

mod app;
mod clipboard;
mod input;
mod search;
mod ui;
mod widgets;
mod yaml_model;

use crate::app::App;

#[derive(Parser)]
#[command(name = "yed", version, about = "YAML TUI editor")]
struct Cli {
    /// YAML file to open. If omitted, TUI opens with a file list to select from (current directory).
    path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut terminal = init_terminal()?;
    let result = run_app(&mut terminal, cli.path);
    restore_terminal(&mut terminal)?;
    if let Err(err) = result {
        eprintln!("{err}");
    }
    Ok(())
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    path: Option<PathBuf>,
) -> Result<()> {
    let mut app = match path {
        Some(ref p) => match App::new(p) {
            Ok(a) => a,
            Err(err) => {
                show_fatal_error(terminal, &err.to_string())?;
                return Ok(());
            }
        },
        None => match App::new_for_picker() {
            Ok(a) => a,
            Err(err) => {
                show_fatal_error(terminal, &err.to_string())?;
                return Ok(());
            }
        },
    };
    loop {
        app.update_toast();
        if let Err(err) = app.check_and_reload_if_changed() {
            app.set_toast(err.to_string());
        }
        terminal.draw(|frame| {
            let hits = ui::draw(frame, &mut app);
            app.update_hit_map(hits);
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let area_height = terminal.size()?.height.saturating_sub(4) as usize;
                    let should_quit = match app.handle_key(key, area_height) {
                        Ok(quit) => quit,
                        Err(err) => {
                            app.set_toast(err.to_string());
                            false
                        }
                    };
                    if app.mode == app::Mode::ConfirmQuit && should_quit {
                        break;
                    }
                    if should_quit {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    let area_height = terminal.size()?.height.saturating_sub(4) as usize;
                    if let Err(err) = app.handle_mouse(mouse, area_height) {
                        app.set_toast(err.to_string());
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}

fn show_fatal_error(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    message: &str,
) -> Result<()> {
    terminal.draw(|frame| {
        let size = frame.size();
        let block = ratatui::widgets::Block::default()
            .title("YAML Parse Error")
            .borders(ratatui::widgets::Borders::ALL);
        let paragraph = ratatui::widgets::Paragraph::new(message).block(block);
        frame.render_widget(paragraph, size);
    })?;
    loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }
    }
    Ok(())
}
