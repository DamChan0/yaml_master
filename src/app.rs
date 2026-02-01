use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::clipboard;
use crate::input::{InputAction, InputContext, VimInputHandler};
use crate::search::{next_match, prev_match};
use crate::yaml_model::{
    flatten_visible, parse_scalar_input, visible_row_by_path, NodePath, NodeType, TreeNode,
    VisibleRow, YamlModel,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    EditValue,
    RenameKey,
    AddKey,
    AddValue,
    ConfirmDelete,
    ConfirmQuit,
    SearchInput,
}

#[derive(Clone, Debug)]
pub struct InputLine {
    pub text: String,
    pub cursor: usize,
}

impl InputLine {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    pub fn set(&mut self, value: String) {
        self.text = value;
        self.cursor = self.text.len();
    }

    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.text.remove(self.cursor);
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        self.text.remove(self.cursor);
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub expires_at: Instant,
}

#[derive(Clone, Debug)]
pub struct RowHit {
    pub row_index: usize,
    pub y: u16,
    pub key_x_start: u16,
    pub key_x_end: u16,
}

#[derive(Clone, Debug)]
pub struct FilePickerState {
    pub paths: Vec<PathBuf>,
}

pub struct App {
    pub model: YamlModel,
    pub mode: Mode,
    pub selection: usize,
    pub scroll: usize,
    pub expanded: HashSet<String>,
    pub visible: Vec<VisibleRow>,
    pub tree_root: TreeNode,
    pub hit_map: Vec<RowHit>,
    pub dirty: bool,
    pub toast: Option<Toast>,
    pub input: InputLine,
    pub pending_key: Option<String>,
    pub search_query: Option<String>,
    pub matches: Vec<usize>,
    pub vim: VimInputHandler,
    pub file_picker: Option<FilePickerState>,
    /// After right-click, ignore 'a'/'r' for a short time (terminal often pastes on right-click).
    pub right_click_ignore_until: Option<Instant>,
}

impl App {
    pub fn new(path: &Path) -> Result<Self> {
        let model = YamlModel::load(path)?;
        let mut expanded = HashSet::new();
        expanded.insert(String::new());
        let tree_root = model.build_tree();
        let visible = flatten_visible(&tree_root, &expanded, None);
        Ok(Self {
            model,
            mode: Mode::Normal,
            selection: 0,
            scroll: 0,
            expanded,
            visible,
            tree_root,
            hit_map: Vec::new(),
            dirty: false,
            toast: None,
            input: InputLine::new(),
            pending_key: None,
            search_query: None,
            matches: Vec::new(),
            vim: VimInputHandler::new(),
            file_picker: None,
            right_click_ignore_until: None,
        })
    }

    /// Create app in file picker mode (no file loaded). Lists .yaml/.yml in current directory.
    pub fn new_for_picker() -> Result<Self> {
        let model = YamlModel::empty();
        let mut expanded = HashSet::new();
        expanded.insert(String::new());
        let tree_root = model.build_tree();
        let visible = flatten_visible(&tree_root, &expanded, None);
        let paths = list_yaml_files_in_current_dir()?;
        Ok(Self {
            model,
            mode: Mode::Normal,
            selection: 0,
            scroll: 0,
            expanded,
            visible,
            tree_root,
            hit_map: Vec::new(),
            dirty: false,
            toast: None,
            input: InputLine::new(),
            pending_key: None,
            search_query: None,
            matches: Vec::new(),
            vim: VimInputHandler::new(),
            file_picker: Some(FilePickerState { paths }),
            right_click_ignore_until: None,
        })
    }

    /// Load a file and switch from file picker to editor.
    pub fn open_file(&mut self, path: PathBuf) -> Result<()> {
        let model = YamlModel::load(&path)?;
        let mut expanded = HashSet::new();
        expanded.insert(String::new());
        let tree_root = model.build_tree();
        let visible = flatten_visible(&tree_root, &expanded, None);
        self.model = model;
        self.tree_root = tree_root;
        self.visible = visible;
        self.expanded = expanded;
        self.selection = 0;
        self.scroll = 0;
        self.file_picker = None;
        self.hit_map = Vec::new();
        self.dirty = false;
        self.mode = Mode::Normal;
        self.toast = None;
        self.input.set(String::new());
        self.pending_key = None;
        self.search_query = None;
        self.matches = Vec::new();
        self.right_click_ignore_until = None;
        Ok(())
    }

    pub fn is_file_picker(&self) -> bool {
        self.file_picker.is_some()
    }

    pub fn rebuild_visible(&mut self) {
        let selected_path = self.save_selection_path();
        self.tree_root = self.model.build_tree();
        self.visible = flatten_visible(
            &self.tree_root,
            &self.expanded,
            self.search_query.as_deref(),
        );
        if let Some(query) = &self.search_query {
            let lower = query.to_lowercase();
            self.matches = self
                .visible
                .iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    if row.path.dot_path().to_lowercase().contains(&lower)
                        || row.display_key.to_lowercase().contains(&lower)
                    {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
        }
        if let Some(path) = selected_path {
            self.restore_selection(Some(path));
        }
        if self.selection >= self.visible.len() {
            self.selection = self.visible.len().saturating_sub(1);
        }
    }

    pub fn current_row(&self) -> Option<&VisibleRow> {
        self.visible.get(self.selection)
    }

    pub fn update_hit_map(&mut self, hits: Vec<RowHit>) {
        self.hit_map = hits;
    }

    pub fn handle_key(&mut self, key: KeyEvent, area_height: usize) -> Result<bool> {
        use crossterm::event::{KeyCode, KeyModifiers};
        // After right-click, ignore 'a' and 'r' for 200ms (terminal often pastes on right-click).
        if self.mode == Mode::Normal
            && key.modifiers == KeyModifiers::NONE
            && matches!(key.code, KeyCode::Char('a') | KeyCode::Char('r'))
        {
            if let Some(until) = self.right_click_ignore_until {
                if Instant::now() < until {
                    return Ok(false);
                }
            }
        }
        self.right_click_ignore_until = None;
        if let Some(ref picker) = self.file_picker {
            match key.code {
                KeyCode::Enter => {
                    if self.selection < picker.paths.len() {
                        let path = picker.paths[self.selection].clone();
                        if let Err(e) = self.open_file(path) {
                            self.set_toast(e.to_string());
                        }
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
                KeyCode::Char('j') | KeyCode::Down => {
                    let max_idx = picker.paths.len().saturating_sub(1);
                    self.selection = (self.selection + 1).min(max_idx);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.selection = self.selection.saturating_sub(1);
                }
                _ => {}
            }
            return Ok(false);
        }
        if let Some(action) = self.vim.handle_key(InputContext {
            mode: &self.mode,
            key,
        }) {
            return self.apply_action(action, area_height);
        }
        Ok(false)
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, area_height: usize) -> Result<bool> {
        // Block right-click so it does not trigger selection or other actions.
        // Also set a short ignore window: many terminals paste on right-click, and the first
        // character ('a' or 'r') would otherwise trigger Add Key / Rename.
        if matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Up(MouseButton::Right)
        ) {
            self.right_click_ignore_until =
                Some(Instant::now() + Duration::from_millis(200));
            return Ok(false);
        }
        if let Some(ref picker) = self.file_picker {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.selection = self.selection.saturating_sub(1);
                }
                MouseEventKind::ScrollDown => {
                    let max_idx = picker.paths.len().saturating_sub(1);
                    self.selection = (self.selection + 1).min(max_idx);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(hit) = self.hit_map.iter().find(|hit| hit.y == mouse.row) {
                        if hit.row_index < picker.paths.len() {
                            let path = picker.paths[hit.row_index].clone();
                            if let Err(e) = self.open_file(path) {
                                self.set_toast(e.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
            return Ok(false);
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(1);
                let max_scroll = self.visible.len().saturating_sub(area_height);
                self.scroll = self.scroll.min(max_scroll);
                self.clamp_selection(area_height);
            }
            MouseEventKind::ScrollDown => {
                self.scroll = self.scroll.saturating_add(1);
                let max_scroll = self.visible.len().saturating_sub(area_height);
                self.scroll = self.scroll.min(max_scroll);
                self.clamp_selection(area_height);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self.hit_map.iter().find(|hit| hit.y == mouse.row) {
                    self.selection = hit.row_index;
                    let row_data = self.current_row().map(|r| (r.is_container, r.path.dot_path()));
                    if let Some((is_container, dot_path)) = row_data {
                        if is_container {
                            if self.expanded.contains(&dot_path) {
                                self.expanded.remove(&dot_path);
                            } else {
                                self.expanded.insert(dot_path);
                            }
                            self.rebuild_visible();
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub fn apply_action(&mut self, action: InputAction, area_height: usize) -> Result<bool> {
        match action {
            InputAction::Quit => return self.request_quit(),
            InputAction::Save => self.save()?,
            InputAction::MoveUp => self.move_selection(-1),
            InputAction::MoveDown => self.move_selection(1),
            InputAction::JumpTop => self.jump_top(),
            InputAction::JumpBottom => self.jump_bottom(),
            InputAction::PageUp => self.page_scroll(-(area_height as isize / 2)),
            InputAction::PageDown => self.page_scroll(area_height as isize / 2),
            InputAction::JumpLeft => self.scroll = 0,
            InputAction::Collapse => self.collapse_selected(),
            InputAction::Expand => self.expand_selected(),
            InputAction::ToggleExpand => self.toggle_expand(),
            InputAction::EditValue => self.start_edit_value()?,
            InputAction::RenameKey => self.start_rename_key()?,
            InputAction::AddChild => self.start_add_child()?,
            InputAction::DeleteNode => self.start_delete_node()?,
            InputAction::CopyPath => self.copy_current_path(),
            InputAction::ConfirmYes => {
                if self.confirm_yes()? {
                    return Ok(true);
                }
            }
            InputAction::ConfirmNo => self.confirm_no(),
            InputAction::StartSearch => self.start_search(),
            InputAction::SearchNext => self.search_next(),
            InputAction::SearchPrev => self.search_prev(),
            InputAction::Cancel => self.cancel_mode(),
            InputAction::InputChar(ch) => self.input.insert_char(ch),
            InputAction::InputBackspace => self.input.backspace(),
            InputAction::InputDelete => self.input.delete(),
            InputAction::InputLeft => self.input.move_left(),
            InputAction::InputRight => self.input.move_right(),
            InputAction::InputHome => self.input.move_home(),
            InputAction::InputEnd => self.input.move_end(),
            InputAction::InputCommit => self.commit_input()?,
        }
        self.ensure_visible(area_height);
        Ok(false)
    }

    fn ensure_visible(&mut self, area_height: usize) {
        if self.selection < self.scroll {
            self.scroll = self.selection;
        } else if self.selection >= self.scroll + area_height {
            self.scroll = self.selection.saturating_sub(area_height.saturating_sub(1));
        }
    }

    fn clamp_selection(&mut self, area_height: usize) {
        if self.selection < self.scroll {
            self.selection = self.scroll;
        } else if self.selection >= self.scroll + area_height {
            self.selection = self.scroll + area_height.saturating_sub(1);
            if self.selection >= self.visible.len() {
                self.selection = self.visible.len().saturating_sub(1);
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let next = self.selection as isize + delta;
        self.selection = next.clamp(0, self.visible.len().saturating_sub(1) as isize) as usize;
    }

    fn jump_top(&mut self) {
        self.selection = 0;
    }

    fn jump_bottom(&mut self) {
        if !self.visible.is_empty() {
            self.selection = self.visible.len() - 1;
        }
    }

    fn page_scroll(&mut self, delta: isize) {
        let new = (self.selection as isize + delta).max(0);
        self.selection = new.min(self.visible.len().saturating_sub(1) as isize) as usize;
    }

    fn expand_selected(&mut self) {
        if let Some(row) = self.current_row() {
            if row.is_container {
                self.expanded.insert(row.path.dot_path());
                self.rebuild_visible();
            }
        }
    }

    fn collapse_selected(&mut self) {
        if let Some(row) = self.current_row() {
            if row.is_container {
                self.expanded.remove(&row.path.dot_path());
                self.rebuild_visible();
            }
        }
    }

    fn toggle_expand(&mut self) {
        if let Some(row) = self.current_row() {
            if row.is_container {
                let dot = row.path.dot_path();
                if self.expanded.contains(&dot) {
                    self.expanded.remove(&dot);
                } else {
                    self.expanded.insert(dot);
                }
                self.rebuild_visible();
            } else {
                self.start_edit_value().ok();
            }
        }
    }

    fn start_edit_value(&mut self) -> Result<()> {
        let row_data = self
            .current_row()
            .map(|r| (r.is_container, r.display_value_preview.clone()));
        if let Some((is_container, display_value)) = row_data {
            if is_container {
                return Ok(());
            }
            self.mode = Mode::EditValue;
            self.input.set(display_value);
        }
        Ok(())
    }

    fn start_rename_key(&mut self) -> Result<()> {
        let row_data = self.current_row().map(|r| {
            let is_key = r
                .path
                .0
                .last()
                .map(|seg| matches!(seg, crate::yaml_model::PathSegment::Key(_)))
                == Some(true);
            (is_key, r.display_key.clone())
        });
        if let Some((is_key, display_key)) = row_data {
            if is_key {
                self.mode = Mode::RenameKey;
                self.input.set(display_key);
            } else {
                self.set_toast("Cannot rename sequence item".to_string());
            }
        }
        Ok(())
    }

    fn start_add_child(&mut self) -> Result<()> {
        if let Some(row) = self.current_row() {
            if row.node_type == NodeType::Map {
                self.mode = Mode::AddKey;
                self.input.set(String::new());
            } else if row.node_type == NodeType::Seq {
                self.mode = Mode::AddValue;
                self.input.set(String::new());
            } else {
                self.set_toast("Cannot add child to scalar".to_string());
            }
        }
        Ok(())
    }

    fn start_delete_node(&mut self) -> Result<()> {
        if self.current_row().is_some() {
            self.mode = Mode::ConfirmDelete;
        }
        Ok(())
    }

    fn copy_current_path(&mut self) {
        if let Some(row) = self.current_row() {
            let path = row.path.dot_path();
            if clipboard::copy_to_clipboard(&path).is_ok() {
                self.set_toast(format!("Copied: {path}"));
            } else {
                self.set_toast("Failed to copy path".to_string());
            }
        }
    }

    fn request_quit(&mut self) -> Result<bool> {
        self.mode = Mode::ConfirmQuit;
        Ok(false)
    }

    fn confirm_yes(&mut self) -> Result<bool> {
        match self.mode {
            Mode::ConfirmDelete => {
                let path = self.current_row().map(|r| r.path.clone());
                if let Some(path) = path {
                    self.model.delete_node(&path)?;
                    self.dirty = true;
                    self.rebuild_visible();
                }
                self.mode = Mode::Normal;
                Ok(false)
            }
            Mode::ConfirmQuit => Ok(true),
            _ => Ok(false),
        }
    }

    fn confirm_no(&mut self) {
        self.mode = Mode::Normal;
    }

    fn start_search(&mut self) {
        self.mode = Mode::SearchInput;
        self.input.set(String::new());
    }

    fn search_next(&mut self) {
        if let Some(next) = next_match(&self.matches, self.selection) {
            self.selection = next;
        }
    }

    fn search_prev(&mut self) {
        if let Some(prev) = prev_match(&self.matches, self.selection) {
            self.selection = prev;
        }
    }

    fn cancel_mode(&mut self) {
        if self.mode == Mode::SearchInput {
            self.search_query = None;
            self.matches.clear();
            self.rebuild_visible();
        }
        self.mode = Mode::Normal;
        self.input.set(String::new());
        self.pending_key = None;
    }

    fn commit_input(&mut self) -> Result<()> {
        match self.mode {
            Mode::EditValue => {
                let path = self.current_row().map(|r| r.path.clone());
                if let Some(path) = path {
                    let parsed = parse_scalar_input(&self.input.text)?;
                    self.model.edit_value(&path, parsed)?;
                    self.dirty = true;
                }
                self.mode = Mode::Normal;
                self.rebuild_visible();
            }
            Mode::RenameKey => {
                let path = self.current_row().map(|r| r.path.clone());
                if let Some(path) = path {
                    self.model.rename_key(&path, &self.input.text)?;
                    self.dirty = true;
                }
                self.mode = Mode::Normal;
                self.rebuild_visible();
            }
            Mode::AddKey => {
                self.pending_key = Some(self.input.text.clone());
                self.mode = Mode::AddValue;
                self.input.set(String::new());
            }
            Mode::AddValue => {
                let row_data = self
                    .current_row()
                    .map(|r| (r.path.clone(), r.node_type.clone()));
                if let Some((path, node_type)) = row_data {
                    let parsed = parse_scalar_input(&self.input.text)?;
                    if node_type == NodeType::Map {
                        if let Some(key) = self.pending_key.take() {
                            self.model.add_mapping_child(&path, &key, parsed)?;
                        }
                    } else if node_type == NodeType::Seq {
                        self.model.add_sequence_value(&path, parsed)?;
                    }
                    self.dirty = true;
                }
                self.mode = Mode::Normal;
                self.rebuild_visible();
            }
            Mode::SearchInput => {
                let query = self.input.text.clone();
                self.search_query = if query.is_empty() { None } else { Some(query) };
                self.mode = Mode::Normal;
                self.rebuild_visible();
                self.matches = self
                    .visible
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, row)| {
                        self.search_query.as_ref().and_then(|q| {
                            let lower = q.to_lowercase();
                            if row.path.dot_path().to_lowercase().contains(&lower)
                                || row.display_key.to_lowercase().contains(&lower)
                            {
                                Some(idx)
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        self.model.save()?;
        self.dirty = false;
        self.set_toast("Saved".to_string());
        Ok(())
    }

    pub fn set_toast(&mut self, message: String) {
        self.toast = Some(Toast {
            message,
            expires_at: Instant::now() + Duration::from_secs(2),
        });
    }

    pub fn update_toast(&mut self) {
        if let Some(toast) = &self.toast {
            if Instant::now() >= toast.expires_at {
                self.toast = None;
            }
        }
    }

    pub fn status_fields(&self) -> (String, usize, String, String) {
        if let Some(row) = self.current_row() {
            (
                row.path.dot_path(),
                row.path.depth(),
                row.node_type.to_string(),
                row.display_value_preview.clone(),
            )
        } else {
            (String::new(), 0, String::new(), String::new())
        }
    }

    fn save_selection_path(&self) -> Option<NodePath> {
        self.current_row().map(|row| row.path.clone())
    }

    pub fn restore_selection(&mut self, path: Option<NodePath>) {
        if let Some(path) = path {
            if let Some(index) = visible_row_by_path(&self.visible, &path) {
                self.selection = index;
            }
        }
    }
}

fn list_yaml_files_in_current_dir() -> Result<Vec<PathBuf>> {
    let current = std::env::current_dir()?;
    let mut paths: Vec<PathBuf> = fs::read_dir(current)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.is_file() {
                let ext = p.extension()?;
                let ext = ext.to_str()?;
                if ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") {
                    return Some(p);
                }
            }
            None
        })
        .collect();
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(paths)
}
