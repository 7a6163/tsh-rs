use crate::{TshError, TshResult};
use crossterm::{
    cursor::{MoveTo, MoveToColumn},
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{size, Clear, ClearType},
    ExecutableCommand,
};
use std::collections::VecDeque;
use std::io::{stdout, Write};

const HISTORY_SIZE: usize = 1000;

/// Result of processing a key event (state change only, no display).
pub enum KeyAction {
    /// Exit signal (Ctrl+C, Ctrl+D on empty line)
    Exit,
    /// Send data to server
    Send(Vec<u8>),
    /// No data to send, but state may have changed and display needs refresh
    Redisplay,
    /// Clear screen then redisplay
    ClearScreen,
    /// Submit current line (Enter key)
    SubmitLine(Vec<u8>),
    /// No-op
    Noop,
}

/// Enhanced terminal handler with command history and line editing
pub struct TerminalHandler {
    /// Current input line
    current_line: String,
    /// Cursor position in the current line
    cursor_pos: usize,
    /// Command history
    history: VecDeque<String>,
    /// Current position in history (for up/down navigation)
    history_pos: Option<usize>,
    /// Terminal width and height
    term_size: (u16, u16),
    /// Current prompt
    prompt: String,
}

impl TerminalHandler {
    pub fn new() -> TshResult<Self> {
        let (cols, rows) = size().map_err(|e| TshError::Io(std::io::Error::other(e)))?;

        Ok(Self {
            current_line: String::new(),
            cursor_pos: 0,
            history: VecDeque::with_capacity(HISTORY_SIZE),
            history_pos: None,
            term_size: (cols, rows),
            prompt: String::from("$ "),
        })
    }

    /// Set custom prompt
    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt = prompt;
    }

    /// Display the prompt and current line
    pub fn display_prompt(&self) -> TshResult<()> {
        let mut stdout = stdout();

        // Clear current line and display prompt
        stdout.execute(MoveToColumn(0))?;
        stdout.execute(Clear(ClearType::CurrentLine))?;
        stdout.execute(SetForegroundColor(Color::Green))?;
        stdout.execute(Print(&self.prompt))?;
        stdout.execute(ResetColor)?;
        stdout.execute(Print(&self.current_line))?;

        // Position cursor correctly
        let cursor_col = self.prompt.len() + self.cursor_pos;
        stdout.execute(MoveToColumn(cursor_col as u16))?;
        stdout.flush()?;

        Ok(())
    }

    /// Handle terminal resize
    pub fn handle_resize(&mut self) -> TshResult<()> {
        let (cols, rows) = size().map_err(|e| TshError::Io(std::io::Error::other(e)))?;
        self.term_size = (cols, rows);
        self.display_prompt()?;
        Ok(())
    }

    /// Process a key event: mutate state and return the action to take.
    /// This is the pure-logic core, testable without a terminal.
    pub fn process_key_logic(&mut self, key_event: KeyEvent) -> KeyAction {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => KeyAction::Exit,

            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.current_line.is_empty() {
                    KeyAction::Exit
                } else {
                    KeyAction::Noop
                }
            }

            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => KeyAction::ClearScreen,

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if !self.current_line.is_empty() {
                    self.add_to_history(self.current_line.clone());
                }
                let mut data = self.current_line.as_bytes().to_vec();
                data.push(b'\n');
                self.current_line.clear();
                self.cursor_pos = 0;
                self.history_pos = None;
                KeyAction::SubmitLine(data)
            }

            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.backspace();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Delete,
                ..
            } => {
                self.delete_at_cursor();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Left,
                ..
            } => {
                self.move_cursor_left();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Right,
                ..
            } => {
                self.move_cursor_right();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.navigate_history_up_logic();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                self.navigate_history_down_logic();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.move_to_home();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.move_to_end();
                KeyAction::Redisplay
            }

            KeyEvent {
                code: KeyCode::Tab, ..
            } => KeyAction::Send(vec![b'\t']),

            KeyEvent {
                code: KeyCode::Char(c),
                ..
            } => {
                self.insert_char(c);
                KeyAction::Redisplay
            }

            _ => KeyAction::Noop,
        }
    }

    /// Process a key event and return data to send to server (with display side effects).
    pub async fn handle_key_event(&mut self, key_event: KeyEvent) -> TshResult<Option<Vec<u8>>> {
        let action = self.process_key_logic(key_event);
        match action {
            KeyAction::Exit => Ok(None),
            KeyAction::Send(data) => Ok(Some(data)),
            KeyAction::Noop => Ok(Some(vec![])),
            KeyAction::Redisplay => {
                self.display_prompt()?;
                Ok(Some(vec![]))
            }
            KeyAction::ClearScreen => {
                let mut stdout = stdout();
                stdout.execute(Clear(ClearType::All))?;
                stdout.execute(MoveTo(0, 0))?;
                self.display_prompt()?;
                Ok(Some(vec![]))
            }
            KeyAction::SubmitLine(data) => {
                println!();
                Ok(Some(data))
            }
        }
    }

    /// Add a command to history
    fn add_to_history(&mut self, command: String) {
        // Don't add duplicates of the last command
        if self.history.front() != Some(&command) {
            self.history.push_front(command);
            if self.history.len() > HISTORY_SIZE {
                self.history.pop_back();
            }
        }
    }

    /// Navigate up in history (pure logic, no display)
    fn navigate_history_up_logic(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_pos {
            None => {
                self.history_pos = Some(0);
                if let Some(cmd) = self.history.front() {
                    self.current_line = cmd.clone();
                    self.cursor_pos = self.current_line.len();
                }
            }
            Some(pos) => {
                if pos + 1 < self.history.len() {
                    self.history_pos = Some(pos + 1);
                    if let Some(cmd) = self.history.get(pos + 1) {
                        self.current_line = cmd.clone();
                        self.cursor_pos = self.current_line.len();
                    }
                }
            }
        }
    }

    /// Navigate down in history (pure logic, no display)
    fn navigate_history_down_logic(&mut self) {
        match self.history_pos {
            Some(0) => {
                self.history_pos = None;
                self.current_line.clear();
                self.cursor_pos = 0;
            }
            Some(pos) => {
                self.history_pos = Some(pos - 1);
                if let Some(cmd) = self.history.get(pos - 1) {
                    self.current_line = cmd.clone();
                    self.cursor_pos = self.current_line.len();
                }
            }
            None => {}
        }
    }

    // ─── Accessors (for testing and internal use) ─────────────────────────

    /// Get the current input line
    pub fn current_line(&self) -> &str {
        &self.current_line
    }

    /// Get the current cursor position
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Get the current prompt
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Get the number of items in history
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    // ─── Line editing primitives (side-effect-free, no display) ─────────

    /// Insert a character at the current cursor position
    pub fn insert_char(&mut self, c: char) {
        self.current_line.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
    }

    /// Remove the character before the cursor
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.current_line.remove(self.cursor_pos - 1);
            self.cursor_pos -= 1;
        }
    }

    /// Remove the character at the cursor
    pub fn delete_at_cursor(&mut self) {
        if self.cursor_pos < self.current_line.len() {
            self.current_line.remove(self.cursor_pos);
        }
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.current_line.len() {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to start
    pub fn move_to_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end
    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.current_line.len();
    }

    /// Clear the current line
    pub fn clear_line(&mut self) {
        self.current_line.clear();
        self.cursor_pos = 0;
    }

    /// Submit the current line: adds to history, clears input, returns the line
    pub fn submit_line(&mut self) -> String {
        let line = self.current_line.clone();
        if !line.is_empty() {
            self.add_to_history(line.clone());
        }
        self.current_line.clear();
        self.cursor_pos = 0;
        self.history_pos = None;
        line
    }

    /// Public wrapper for add_to_history (for testing)
    pub fn add_to_history_pub(&mut self, command: String) {
        self.add_to_history(command);
    }

    /// Navigate up in history (public, for testing)
    pub fn navigate_up(&mut self) {
        self.navigate_history_up_logic();
    }

    /// Navigate down in history (public, for testing)
    pub fn navigate_down(&mut self) {
        self.navigate_history_down_logic();
    }

    /// Handle incoming data from server
    pub fn handle_server_data(&self, data: &[u8]) -> TshResult<()> {
        // For now, just print the data
        // In the future, we can parse ANSI escape sequences for colors, etc.
        print!("{}", String::from_utf8_lossy(data));
        stdout().flush()?;
        Ok(())
    }
}
