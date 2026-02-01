use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::Mode;

#[derive(Clone, Debug)]
pub enum InputAction {
    Quit,
    Save,
    MoveUp,
    MoveDown,
    JumpTop,
    JumpBottom,
    PageUp,
    PageDown,
    JumpLeft,
    Collapse,
    Expand,
    ToggleExpand,
    EditValue,
    RenameKey,
    AddChild,
    AddMapToSequence,
    DeleteNode,
    DeleteLine,
    CopyPath,
    ConfirmYes,
    ConfirmNo,
    OpenAnother,
    StartSearch,
    SearchNext,
    SearchPrev,
    Cancel,
    InputChar(char),
    InputBackspace,
    InputDelete,
    InputLeft,
    InputRight,
    InputHome,
    InputEnd,
    InputCommit,
}

pub struct InputContext<'a> {
    pub mode: &'a Mode,
    pub key: KeyEvent,
}

pub struct VimInputHandler {
    pending_g: bool,
}

impl VimInputHandler {
    pub fn new() -> Self {
        Self { pending_g: false }
    }

    pub fn handle_key(&mut self, ctx: InputContext<'_>) -> Option<InputAction> {
        let key = ctx.key;
        match ctx.mode {
            Mode::EditValue
            | Mode::RenameKey
            | Mode::AddKey
            | Mode::AddValue
            | Mode::SearchInput
            | Mode::RawEditLine => return self.handle_input_mode(key),
            Mode::ConfirmDelete
            | Mode::ConfirmQuit
            | Mode::ConfirmOpenAnother
            | Mode::ConfirmRawDeleteLine => return self.handle_confirm(key),
            Mode::Normal => {}
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => Some(InputAction::Quit),
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => Some(InputAction::Save),
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => Some(InputAction::OpenAnother),
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                Some(InputAction::MoveDown)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                Some(InputAction::MoveUp)
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                if self.pending_g {
                    self.pending_g = false;
                    Some(InputAction::JumpTop)
                } else {
                    self.pending_g = true;
                    None
                }
            }
            (KeyCode::Char('G'), KeyModifiers::SHIFT) | (KeyCode::Char('G'), KeyModifiers::NONE) => {
                Some(InputAction::JumpBottom)
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                Some(InputAction::Collapse)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                Some(InputAction::Expand)
            }
            (KeyCode::Enter, _) => Some(InputAction::ToggleExpand),
            (KeyCode::Char('e'), KeyModifiers::NONE) => Some(InputAction::EditValue),
            (KeyCode::Char('r'), KeyModifiers::NONE) => Some(InputAction::RenameKey),
            (KeyCode::Char('a'), KeyModifiers::NONE) => Some(InputAction::AddChild),
            (KeyCode::Char('A'), KeyModifiers::SHIFT) => Some(InputAction::AddMapToSequence),
            (KeyCode::Char('d'), KeyModifiers::NONE) => Some(InputAction::DeleteNode),
            (KeyCode::Delete, KeyModifiers::SHIFT) => Some(InputAction::DeleteLine),
            (KeyCode::Char('y'), KeyModifiers::NONE) => Some(InputAction::CopyPath),
            (KeyCode::Char('n'), KeyModifiers::NONE) => Some(InputAction::SearchNext),
            (KeyCode::Char('N'), KeyModifiers::SHIFT) | (KeyCode::Char('N'), KeyModifiers::NONE) => {
                Some(InputAction::SearchPrev)
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => Some(InputAction::StartSearch),
            (KeyCode::Char('0'), KeyModifiers::NONE) => Some(InputAction::JumpLeft),
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => Some(InputAction::PageUp),
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => Some(InputAction::PageDown),
            _ => {
                self.pending_g = false;
                None
            }
        }
    }

    fn handle_input_mode(&mut self, key: KeyEvent) -> Option<InputAction> {
        self.pending_g = false;
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => Some(InputAction::Cancel),
            (KeyCode::Enter, _) => Some(InputAction::InputCommit),
            (KeyCode::Left, _) => Some(InputAction::InputLeft),
            (KeyCode::Right, _) => Some(InputAction::InputRight),
            (KeyCode::Home, _) => Some(InputAction::InputHome),
            (KeyCode::End, _) => Some(InputAction::InputEnd),
            (KeyCode::Backspace, _) => Some(InputAction::InputBackspace),
            (KeyCode::Delete, _) => Some(InputAction::InputDelete),
            (KeyCode::Char(ch), KeyModifiers::NONE) => Some(InputAction::InputChar(ch)),
            (KeyCode::Char(ch), KeyModifiers::SHIFT) => Some(InputAction::InputChar(ch)),
            _ => None,
        }
    }

    fn handle_confirm(&mut self, key: KeyEvent) -> Option<InputAction> {
        self.pending_g = false;
        match (key.code, key.modifiers) {
            (KeyCode::Char('y'), KeyModifiers::NONE) => Some(InputAction::ConfirmYes),
            (KeyCode::Char('n'), KeyModifiers::NONE) => Some(InputAction::ConfirmNo),
            (KeyCode::Esc, _) => Some(InputAction::ConfirmNo),
            _ => None,
        }
    }
}
