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
    ConfirmOpenAnother,
    ConfirmRawDeleteLine,
    SearchInput,
    /// Editing a line in raw view (parse error).
    RawEditLine,
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
pub enum PickerEntry {
    Parent,
    Dir(PathBuf),
    File(PathBuf),
}

#[derive(Clone, Debug)]
pub struct FilePickerState {
    pub current_dir: PathBuf,
    pub entries: Vec<PickerEntry>,
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
    /// Row index under mouse (for hover highlight).
    pub hover_row: Option<usize>,
    /// Parse error when YAML is invalid (file still opened with empty doc).
    pub parse_error: Option<String>,
    /// Raw file content when parse failed (so user can edit and fix).
    pub raw_content: Option<String>,
    /// File mtime when loaded (for external change detection).
    pub last_modified: Option<std::time::SystemTime>,
    /// Last time we checked file on disk (for throttling).
    pub last_file_check: Option<Instant>,
}

impl App {
    pub fn new(path: &Path) -> Result<Self> {
        let (model, parse_error, raw_content) = YamlModel::load_with_error(path)?;
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
            hover_row: None,
            parse_error,
            raw_content,
            last_modified: std::fs::metadata(path).and_then(|m| m.modified()).ok(),
            last_file_check: None,
        })
    }

    /// Create app in file picker mode (no file loaded). Lists current dir with .., subdirs, .yaml/.yml.
    pub fn new_for_picker() -> Result<Self> {
        let model = YamlModel::empty();
        let mut expanded = HashSet::new();
        expanded.insert(String::new());
        let tree_root = model.build_tree();
        let visible = flatten_visible(&tree_root, &expanded, None);
        let current_dir = std::env::current_dir()?;
        let entries = list_picker_entries(&current_dir)?;
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
            file_picker: Some(FilePickerState {
                current_dir,
                entries,
            }),
            right_click_ignore_until: None,
            hover_row: None,
            parse_error: None,
            raw_content: None,
            last_modified: None,
            last_file_check: None,
        })
    }

    /// In file picker: enter selected item (change dir or open file). Returns true if dir was changed (refresh UI).
    pub fn picker_enter_selected(&mut self) -> Result<bool> {
        let picker = match &self.file_picker {
            Some(p) => p.clone(),
            None => return Ok(false),
        };
        let entry = match picker.entries.get(self.selection) {
            Some(e) => e.clone(),
            None => return Ok(false),
        };
        match entry {
            PickerEntry::Parent => {
                if let Some(parent) = picker.current_dir.parent() {
                    let parent = parent.to_path_buf();
                    std::env::set_current_dir(&parent)?;
                    let entries = list_picker_entries(&parent)?;
                    if let Some(ref mut fp) = self.file_picker {
                        fp.current_dir = parent;
                        fp.entries = entries;
                    }
                    self.selection = 0;
                    return Ok(true);
                }
            }
            PickerEntry::Dir(path) => {
                if path.is_dir() {
                    std::env::set_current_dir(&path)?;
                    let entries = list_picker_entries(&path)?;
                    if let Some(ref mut fp) = self.file_picker {
                        fp.current_dir = path;
                        fp.entries = entries;
                    }
                    self.selection = 0;
                    return Ok(true);
                }
            }
            PickerEntry::File(path) => {
                if let Err(e) = self.open_file(path) {
                    self.set_toast(e.to_string());
                }
            }
        }
        Ok(false)
    }

    /// Refresh file picker entries (e.g. after changing directory).
    pub fn picker_refresh(&mut self) -> Result<()> {
        if let Some(ref mut fp) = self.file_picker {
            fp.entries = list_picker_entries(&fp.current_dir)?;
            if self.selection >= fp.entries.len() {
                self.selection = fp.entries.len().saturating_sub(1);
            }
        }
        Ok(())
    }

    /// Switch from editor back to file picker (current file's directory).
    pub fn switch_to_file_picker(&mut self) -> Result<()> {
        let current_dir = if self.model.file_path().is_empty() {
            std::env::current_dir()?
        } else {
            PathBuf::from(self.model.file_path())
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        };
        let _ = std::env::set_current_dir(&current_dir);
        let entries = list_picker_entries(&current_dir)?;
        self.file_picker = Some(FilePickerState {
            current_dir,
            entries,
        });
        self.selection = 0;
        self.mode = Mode::Normal;
        Ok(())
    }

    /// Load a file and switch from file picker to editor.
    pub fn open_file(&mut self, path: PathBuf) -> Result<()> {
        let (model, parse_error, raw_content) = YamlModel::load_with_error(&path)?;
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
        self.hover_row = None;
        self.parse_error = parse_error;
        self.raw_content = raw_content;
        self.last_modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        self.last_file_check = None;
        Ok(())
    }

    /// When parse failed, lines of the file for raw edit view.
    pub fn raw_lines(&self) -> Option<Vec<String>> {
        self.raw_content
            .as_ref()
            .map(|s| s.lines().map(String::from).collect::<Vec<_>>())
    }

    /// Replace line at index in raw_content (for raw edit).
    pub fn raw_replace_line(&mut self, line_index: usize, new_line: &str) {
        if let Some(ref mut raw) = self.raw_content {
            let mut lines: Vec<String> = raw.lines().map(String::from).collect();
            if line_index < lines.len() {
                lines[line_index] = new_line.lines().next().unwrap_or("").to_string();
                *raw = lines.join("\n");
            }
        }
    }

    /// Remove line at index from raw_content (raw view: d or Shift+Del).
    pub fn raw_delete_line(&mut self, line_index: usize) {
        if let Some(ref mut raw) = self.raw_content {
            let mut lines: Vec<String> = raw.lines().map(String::from).collect();
            if line_index < lines.len() {
                lines.remove(line_index);
                *raw = lines.join("\n");
                self.dirty = true;
                if self.selection >= lines.len() && !lines.is_empty() {
                    self.selection = lines.len() - 1;
                } else if lines.is_empty() {
                    self.selection = 0;
                }
            }
        }
    }

    /// Save raw content to file and re-parse; clear parse_error if successful.
    pub fn save_raw_and_reparse(&mut self) -> Result<()> {
        let raw = match &self.raw_content {
            Some(r) => r.clone(),
            None => return Ok(()),
        };
        let path = PathBuf::from(self.model.file_path());
        std::fs::write(&path, &raw)?;
        let (model, parse_error, raw_content) = YamlModel::load_with_error(&path)?;
        self.model = model;
        self.parse_error = parse_error.clone();
        self.raw_content = raw_content;
        self.dirty = false;
        if parse_error.is_none() {
            let mut expanded = HashSet::new();
            expanded.insert(String::new());
            self.tree_root = self.model.build_tree();
            self.visible = flatten_visible(&self.tree_root, &expanded, None);
            self.selection = 0;
            self.scroll = 0;
            self.set_toast("Saved and parsed successfully".to_string());
        } else {
            self.set_toast("Saved; parse still has errors".to_string());
        }
        Ok(())
    }

    pub fn is_file_picker(&self) -> bool {
        self.file_picker.is_some()
    }

    /// If file was modified externally and we have no unsaved changes, reload from disk.
    pub fn check_and_reload_if_changed(&mut self) -> Result<()> {
        if self.file_picker.is_some() {
            return Ok(());
        }
        let path_str = self.model.file_path();
        if path_str.is_empty() {
            return Ok(());
        }
        if self.dirty {
            return Ok(());
        }
        let now = Instant::now();
        let check_interval = Duration::from_millis(1500);
        if let Some(last) = self.last_file_check {
            if now.duration_since(last) < check_interval {
                return Ok(());
            }
        }
        self.last_file_check = Some(now);
        let path = PathBuf::from(path_str);
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };
        let modified = match meta.modified() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        if let Some(last) = self.last_modified {
            if modified <= last {
                return Ok(());
            }
        }
        self.last_modified = Some(modified);
        let (model, parse_error, raw_content) = YamlModel::load_with_error(&path)?;
        self.model = model;
        self.parse_error = parse_error;
        self.raw_content = raw_content;
        let mut expanded = HashSet::new();
        expanded.insert(String::new());
        self.tree_root = self.model.build_tree();
        self.visible = flatten_visible(&self.tree_root, &expanded, None);
        if self.raw_content.is_some() {
            let len = self.raw_lines().map(|l| l.len()).unwrap_or(0);
            if len > 0 && self.selection >= len {
                self.selection = len - 1;
            }
        } else if self.selection >= self.visible.len() {
            self.selection = self.visible.len().saturating_sub(1);
        }
        self.set_toast("File changed on disk, reloaded".to_string());
        Ok(())
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
                    let _ = self.picker_enter_selected();
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
                KeyCode::Char('j') | KeyCode::Down => {
                    let max_idx = picker.entries.len().saturating_sub(1);
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
        // Hover: update hover_row from hit_map (works for both tree and file picker).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            self.hover_row = self
                .hit_map
                .iter()
                .find(|hit| hit.y == mouse.row)
                .map(|hit| hit.row_index);
            return Ok(false);
        }
        // Block right-click so it does not trigger selection or other actions.
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
                    let max_idx = picker.entries.len().saturating_sub(1);
                    self.selection = (self.selection + 1).min(max_idx);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(hit) = self.hit_map.iter().find(|hit| hit.y == mouse.row) {
                        if hit.row_index < picker.entries.len() {
                            self.selection = hit.row_index;
                            let _ = self.picker_enter_selected();
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
                let max_scroll = self.visible_len().saturating_sub(area_height);
                self.scroll = self.scroll.min(max_scroll);
                self.clamp_selection(area_height);
            }
            MouseEventKind::ScrollDown => {
                self.scroll = self.scroll.saturating_add(1);
                let max_scroll = self.visible_len().saturating_sub(area_height);
                self.scroll = self.scroll.min(max_scroll);
                self.clamp_selection(area_height);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self.hit_map.iter().find(|hit| hit.y == mouse.row) {
                    self.selection = hit.row_index;
                    if self.raw_content.is_none() {
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
            }
            _ => {}
        }
        Ok(false)
    }

    pub fn apply_action(&mut self, action: InputAction, area_height: usize) -> Result<bool> {
        let in_raw_mode = self.raw_content.is_some();
        match action {
            InputAction::Quit => return self.request_quit(),
            InputAction::Save => {
                if in_raw_mode {
                    self.save_raw_and_reparse()?;
                } else {
                    self.save()?;
                }
            }
            InputAction::MoveUp => self.move_selection(area_height, -1),
            InputAction::MoveDown => self.move_selection(area_height, 1),
            InputAction::JumpTop => self.jump_top(area_height),
            InputAction::JumpBottom => self.jump_bottom(area_height),
            InputAction::PageUp => self.page_scroll(area_height, -(area_height as isize / 2)),
            InputAction::PageDown => self.page_scroll(area_height, area_height as isize / 2),
            InputAction::JumpLeft => self.scroll = 0,
            InputAction::Collapse => self.collapse_selected(),
            InputAction::Expand => self.expand_selected(),
            InputAction::ToggleExpand => self.toggle_expand(),
            InputAction::EditValue => {
                if in_raw_mode {
                    self.start_raw_edit_line()?;
                } else {
                    self.start_edit_value()?;
                }
            }
            InputAction::RenameKey => {
                if self.raw_content.is_some() {
                    self.set_toast("Key rename: fix parse errors or save to use tree view".to_string());
                } else {
                    self.start_rename_key()?;
                }
            }
            InputAction::AddChild => {
                if self.raw_content.is_some() {
                    self.set_toast("Add child: fix parse errors or save to use tree view".to_string());
                } else {
                    self.start_add_child()?;
                }
            }
            InputAction::AddMapToSequence => {
                if self.raw_content.is_some() {
                    self.set_toast("Add object: fix parse errors or save to use tree view".to_string());
                } else {
                    self.start_add_map_to_sequence()?;
                }
            }
            InputAction::DeleteNode => {
                if in_raw_mode {
                    self.mode = Mode::ConfirmRawDeleteLine;
                } else {
                    self.start_delete_node()?;
                }
            }
            InputAction::DeleteLine => {
                if in_raw_mode {
                    self.mode = Mode::ConfirmRawDeleteLine;
                }
            }
            InputAction::CopyPath => self.copy_current_path(),
            InputAction::ConfirmYes => {
                if self.confirm_yes()? {
                    return Ok(true);
                }
            }
            InputAction::ConfirmNo => self.confirm_no(),
            InputAction::OpenAnother => {
                if self.dirty {
                    self.mode = Mode::ConfirmOpenAnother;
                } else {
                    self.switch_to_file_picker()?;
                }
            }
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

    fn start_raw_edit_line(&mut self) -> Result<()> {
        let lines = match self.raw_lines() {
            Some(l) => l,
            None => return Ok(()),
        };
        if self.selection < lines.len() {
            self.mode = Mode::RawEditLine;
            self.input.set(lines[self.selection].clone());
        }
        Ok(())
    }

    fn visible_len(&self) -> usize {
        if self.raw_content.is_some() {
            self.raw_lines().map(|l| l.len()).unwrap_or(0)
        } else {
            self.visible.len()
        }
    }

    fn ensure_visible(&mut self, area_height: usize) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        if self.selection < self.scroll {
            self.scroll = self.selection;
        } else if self.selection >= self.scroll + area_height {
            self.scroll = self.selection.saturating_sub(area_height.saturating_sub(1));
        }
    }

    fn clamp_selection(&mut self, area_height: usize) {
        let len = self.visible_len();
        if self.selection < self.scroll {
            self.selection = self.scroll;
        } else if self.selection >= self.scroll + area_height {
            self.selection = self.scroll + area_height.saturating_sub(1);
            if self.selection >= len {
                self.selection = len.saturating_sub(1);
            }
        }
    }

    fn move_selection(&mut self, area_height: usize, delta: isize) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        let next = self.selection as isize + delta;
        self.selection = next.clamp(0, len.saturating_sub(1) as isize) as usize;
        self.ensure_visible(area_height);
    }

    fn jump_top(&mut self, _area_height: usize) {
        self.selection = 0;
    }

    fn jump_bottom(&mut self, _area_height: usize) {
        let len = self.visible_len();
        if len > 0 {
            self.selection = len - 1;
        }
    }

    fn page_scroll(&mut self, area_height: usize, delta: isize) {
        let len = self.visible_len();
        let new = (self.selection as isize + delta).max(0);
        self.selection = new.min(len.saturating_sub(1) as isize) as usize;
        self.ensure_visible(area_height);
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
            let is_root = r.path.0.is_empty();
            (is_key, is_root, r.display_key.clone())
        });
        if let Some((is_key, is_root, display_key)) = row_data {
            if is_key {
                self.mode = Mode::RenameKey;
                self.input.set(display_key);
            } else if is_root {
                self.set_toast("Root has no key to rename".to_string());
            } else {
                self.set_toast("Cannot rename sequence item".to_string());
            }
        }
        Ok(())
    }

    fn start_add_child(&mut self) -> Result<()> {
        let row_data = self.current_row().map(|r| {
            let is_mapping_key = r
                .path
                .0
                .last()
                .map(|seg| matches!(seg, crate::yaml_model::PathSegment::Key(_)))
                == Some(true);
            (r.path.clone(), r.node_type.clone(), is_mapping_key)
        });
        if let Some((path, node_type, is_mapping_key)) = row_data {
            if node_type == NodeType::Map {
                self.mode = Mode::AddKey;
                self.input.set(String::new());
            } else if node_type == NodeType::Seq {
                self.mode = Mode::AddValue;
                self.input.set(String::new());
            } else if is_mapping_key {
                if let Err(e) = self.model.convert_to_empty_map(&path) {
                    self.set_toast(e.to_string());
                } else {
                    self.dirty = true;
                    self.rebuild_visible();
                    self.mode = Mode::AddKey;
                    self.input.set(String::new());
                }
            } else {
                self.set_toast("Cannot add child to scalar".to_string());
            }
        }
        Ok(())
    }

    /// Add an empty map to the current sequence, then start AddKey on the new element.
    /// Use Shift+A on a sequence (list) to add a new object and type its first key.
    fn start_add_map_to_sequence(&mut self) -> Result<()> {
        let path = self.current_row().map(|r| (r.path.clone(), r.node_type.clone()));
        if let Some((path, node_type)) = path {
            if node_type != NodeType::Seq {
                self.set_toast("Shift+A: only on a sequence (list). Use 'a' to add a value.".to_string());
                return Ok(());
            }
            match self.model.add_sequence_empty_map(&path) {
                Ok(new_path) => {
                    self.dirty = true;
                    self.expanded.insert(path.dot_path());
                    self.rebuild_visible();
                    self.restore_selection(Some(new_path));
                    self.mode = Mode::AddKey;
                    self.input.set(String::new());
                }
                Err(e) => self.set_toast(e.to_string()),
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
            Mode::ConfirmOpenAnother => {
                self.switch_to_file_picker()?;
                self.mode = Mode::Normal;
                Ok(false)
            }
            Mode::ConfirmRawDeleteLine => {
                self.raw_delete_line(self.selection);
                self.mode = Mode::Normal;
                Ok(false)
            }
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
                    let key_trimmed = self.input.text.trim();
                    if key_trimmed.is_empty() {
                        self.set_toast("Key cannot be empty".to_string());
                    } else if let Err(e) = self.model.rename_key(&path, key_trimmed) {
                        self.set_toast(e.to_string());
                    } else {
                        self.dirty = true;
                        self.mode = Mode::Normal;
                        self.rebuild_visible();
                    }
                } else {
                    self.mode = Mode::Normal;
                }
            }
            Mode::AddKey => {
                let key_trimmed = self.input.text.trim().to_string();
                if key_trimmed.is_empty() {
                    self.set_toast("Key cannot be empty".to_string());
                } else {
                    self.pending_key = Some(key_trimmed);
                    self.mode = Mode::AddValue;
                    self.input.set(String::new());
                }
            }
            Mode::AddValue => {
                let row_data = self
                    .current_row()
                    .map(|r| (r.path.clone(), r.node_type.clone()));
                if let Some((path, node_type)) = row_data {
                    match parse_scalar_input(self.input.text.trim()) {
                        Ok(parsed) => {
                            if node_type == NodeType::Map {
                                if let Some(key) = self.pending_key.take() {
                                    if let Err(e) =
                                        self.model.add_mapping_child(&path, key.trim(), parsed)
                                    {
                                        self.set_toast(e.to_string());
                                    } else {
                                        self.dirty = true;
                                        self.mode = Mode::Normal;
                                        self.rebuild_visible();
                                    }
                                } else {
                                    self.mode = Mode::Normal;
                                }
                            } else if node_type == NodeType::Seq {
                                if let Err(e) = self.model.add_sequence_value(&path, parsed) {
                                    self.set_toast(e.to_string());
                                } else {
                                    self.dirty = true;
                                    self.mode = Mode::Normal;
                                    self.rebuild_visible();
                                }
                            } else {
                                self.mode = Mode::Normal;
                            }
                        }
                        Err(e) => self.set_toast(e.to_string()),
                    }
                } else {
                    self.mode = Mode::Normal;
                }
            }
            Mode::SearchInput => {
                let query = self.input.text.trim().to_string();
                self.search_query = if query.is_empty() { None } else { Some(query.clone()) };
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
                if !query.is_empty() && self.matches.is_empty() {
                    self.set_toast("No matches found".to_string());
                } else if !self.matches.is_empty() {
                    self.selection = self.matches[0];
                }
            }
            Mode::RawEditLine => {
                let text = self.input.text.clone();
                self.raw_replace_line(self.selection, &text);
                self.mode = Mode::Normal;
                self.dirty = true;
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
        if let Some(lines) = self.raw_lines() {
            if self.selection < lines.len() {
                let line_no = self.selection + 1;
                let content = lines[self.selection].clone();
                return (
                    format!("Line {}", line_no),
                    self.selection,
                    "raw".to_string(),
                    content.chars().take(40).collect::<String>(),
                );
            }
            return (String::new(), 0, String::new(), String::new());
        }
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

fn list_picker_entries(dir: &Path) -> Result<Vec<PickerEntry>> {
    let mut entries = Vec::new();
    if dir.parent().is_some() {
        entries.push(PickerEntry::Parent);
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            dirs.push(p);
        } else if p.is_file() {
            let ext = p.extension().and_then(|e| e.to_str());
            if ext.map(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml")) == Some(true) {
                files.push(p);
            }
        }
    }
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    entries.extend(dirs.into_iter().map(PickerEntry::Dir));
    entries.extend(files.into_iter().map(PickerEntry::File));
    Ok(entries)
}
